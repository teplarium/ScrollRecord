//! # sr-capture — frame-by-frame capture
//!
//! Purpose: walks the [`Timeline`] frame by frame: tells the page where to
//!          scroll, how much time passed and what the pointer does (moves,
//!          presses), takes a screenshot and passes it — with the cursor plan
//!          and the cursor image the page asked for — to a [`FrameSink`]
//!          (the renderer's compositor + encoder).
//! Solves:  this loop is what makes the video smooth. Each frame is drawn
//!          completely before it's captured, so a slow machine only makes
//!          rendering slower, never the video choppier (HANDOFF §7.3).
//! Rules:   knows nothing about browsers or file formats: it talks to a
//!          [`FrameDriver`] and a [`FrameSink`], so both are swappable and
//!          the loop is tested with fakes.

use std::time::{Duration, Instant};

use sr_browser::{FrameDriver, PointerInput, PressEvent};
use sr_core::AppResult;
use sr_core::flow::CursorKind;
use sr_timeline::{CursorFrame, FramePlan, PointerEventKind, Timeline};

/// One captured frame, handed to the sink.
#[derive(Debug, Clone, Copy)]
pub struct CapturedFrame<'a> {
    /// The plan of this frame (index, time, scroll, cursor).
    pub plan: &'a FramePlan,
    /// Screenshot as PNG bytes.
    pub png: &'a [u8],
    /// Cursor image to draw: what the page asks for under the pointer
    /// (`Touch` on touch devices).
    pub cursor_kind: CursorKind,
}

/// Receives captured frames in order.
pub trait FrameSink {
    /// Takes one frame.
    fn push(&mut self, frame: &CapturedFrame<'_>) -> AppResult<()>;

    /// Called once after the last frame (flush, close files, finish encoding).
    fn finish(&mut self) -> AppResult<()>;
}

/// Progress of a capture, reported after every frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Progress {
    /// Frames captured so far.
    pub done: u32,
    /// Total frames.
    pub total: u32,
}

/// Facts about a finished capture.
///
/// The three times are the busy time of each part of the loop, so a render
/// report can show where the time goes (HANDOFF §14.1). They run one after
/// another on the calling thread, so their sum never exceeds the capture's
/// wall-clock time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CaptureStats {
    /// Frames captured.
    pub frames: u32,
    /// Largest difference between planned and applied scroll position, in
    /// CSS px. Non-zero means the page was shorter than measured (it changed
    /// height during recording).
    pub max_scroll_drift_px: f64,
    /// Time in [`FrameDriver::step`] (scroll, page time, pointer), warm-up
    /// steps included.
    pub step_time: Duration,
    /// Time in [`FrameDriver::capture_png`].
    pub screenshot_time: Duration,
    /// Time in [`FrameSink::push`] and [`FrameSink::finish`]. A sink that
    /// works on another thread blocks here only when it falls behind, so a
    /// large value means the sink (cursor drawing, encoding) is the slow part.
    pub sink_time: Duration,
}

/// Pointer input for a frame: position, live flag and due presses.
fn pointer_input(c: &CursorFrame, touch: bool, with_events: bool) -> PointerInput {
    PointerInput {
        x: c.pos.x,
        y: c.pos.y,
        live: c.live,
        detect_kind: !touch && c.opacity > 0.0,
        events: if with_events {
            c.events
                .iter()
                .map(|e| PressEvent {
                    down: e.kind == PointerEventKind::Press,
                    touch: e.touch,
                    x: e.pos.x,
                    y: e.pos.y,
                })
                .collect()
        } else {
            Vec::new()
        },
    }
}

/// Captures every frame of `timeline`.
///
/// Before frame 0 the page "lives" for `warmup_ms` (in frame-sized steps so
/// animation-frame code runs as usual) at the start position, with the
/// pointer already where frame 0 has it (so a hover shows from the start).
pub fn capture(
    driver: &mut dyn FrameDriver,
    timeline: &Timeline,
    sink: &mut dyn FrameSink,
    on_progress: &mut dyn FnMut(Progress),
) -> AppResult<CaptureStats> {
    let interval = timeline.frame_interval_ms();
    let total = timeline.frame_count();
    let mut frames = timeline.frames();
    let mut stats = CaptureStats {
        frames: 0,
        max_scroll_drift_px: 0.0,
        step_time: Duration::ZERO,
        screenshot_time: Duration::ZERO,
        sink_time: Duration::ZERO,
    };
    let Some(first) = frames.next() else {
        return Ok(stats);
    };

    let touch = timeline.cursor.touch;

    // ── Warm-up: skip intros without recording them ──
    let mut left = f64::from(timeline.warmup_ms);
    let warm_pointer = pointer_input(&first.cursor, touch, false);
    let started = Instant::now();
    while left > 0.0 {
        let dt = left.min(interval);
        driver.step(first.scroll_y, dt, &warm_pointer)?;
        left -= dt;
    }
    stats.step_time += started.elapsed();

    // ── Frames: frame 0 shows time 0; every next frame advances one interval ──
    // The page reports the cursor image only while the cursor is visible;
    // keep the last one so a fade-in starts with the right image.
    let mut kind = CursorKind::Arrow;
    for frame in std::iter::once(first).chain(frames) {
        let dt = if frame.index == 0 { 0.0 } else { interval };
        let started = Instant::now();
        let result = driver.step(
            frame.scroll_y,
            dt,
            &pointer_input(&frame.cursor, touch, true),
        )?;
        stats.step_time += started.elapsed();
        stats.max_scroll_drift_px = stats
            .max_scroll_drift_px
            .max((result.scroll_y - frame.scroll_y).abs());
        if let Some(k) = result.cursor_kind {
            kind = k;
        }

        let started = Instant::now();
        let png = driver.capture_png()?;
        stats.screenshot_time += started.elapsed();

        let started = Instant::now();
        sink.push(&CapturedFrame {
            plan: &frame,
            png: &png,
            cursor_kind: if touch { CursorKind::Touch } else { kind },
        })?;
        stats.sink_time += started.elapsed();
        stats.frames += 1;
        on_progress(Progress {
            done: stats.frames,
            total,
        });
    }
    let started = Instant::now();
    sink.finish()?;
    stats.sink_time += started.elapsed();
    if stats.max_scroll_drift_px > 1.0 {
        tracing::warn!(
            drift = stats.max_scroll_drift_px,
            "page scrolled less than planned (page got shorter while recording?)"
        );
    }
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sr_browser::StepResult;
    use sr_timeline::{ClickMark, CursorTrack, Segment, SegmentKind, Vec2};

    /// Records every call instead of talking to a browser. Pretends the
    /// page shows a hand cursor right of x = 300.
    #[derive(Default)]
    struct FakePage {
        steps: Vec<(f64, f64)>,
        pointers: Vec<PointerInput>,
        clamp_at: Option<f64>,
        /// Pretend each step / screenshot takes this long.
        step_delay: Duration,
        capture_delay: Duration,
    }

    impl FrameDriver for FakePage {
        fn step(
            &mut self,
            scroll_y: f64,
            advance_ms: f64,
            pointer: &PointerInput,
        ) -> AppResult<StepResult> {
            std::thread::sleep(self.step_delay);
            self.steps.push((scroll_y, advance_ms));
            self.pointers.push(pointer.clone());
            let kind = if pointer.x > 300.0 {
                CursorKind::Pointer
            } else {
                CursorKind::Arrow
            };
            Ok(StepResult {
                scroll_y: self.clamp_at.map_or(scroll_y, |max| scroll_y.min(max)),
                cursor_kind: pointer.detect_kind.then_some(kind),
            })
        }
        fn capture_png(&mut self) -> AppResult<Vec<u8>> {
            std::thread::sleep(self.capture_delay);
            Ok(vec![self.steps.len() as u8])
        }
    }

    #[derive(Default)]
    struct FakeSink {
        frames: Vec<(u32, Vec<u8>, CursorKind)>,
        finished: bool,
        /// Pretend each frame takes this long to hand over.
        push_delay: Duration,
    }

    impl FrameSink for FakeSink {
        fn push(&mut self, f: &CapturedFrame<'_>) -> AppResult<()> {
            std::thread::sleep(self.push_delay);
            self.frames
                .push((f.plan.index, f.png.to_vec(), f.cursor_kind));
            Ok(())
        }
        fn finish(&mut self) -> AppResult<()> {
            self.finished = true;
            Ok(())
        }
    }

    fn timeline(warmup_ms: u32) -> Timeline {
        Timeline {
            fps: 10,
            scale: 1.0,
            warmup_ms,
            duration_ms: 1000.0,
            segments: vec![Segment {
                kind: SegmentKind::Scroll,
                start_ms: 0.0,
                end_ms: 1000.0,
                from_y: 0.0,
                to_y: 100.0,
                easing: sr_core::Easing::Linear,
                label: "a → b".into(),
            }],
            cursor: CursorTrack {
                start: Vec2 { x: 400.0, y: 50.0 },
                visible: true,
                touch: false,
                segments: Vec::new(),
                clicks: vec![ClickMark {
                    at_ms: 450.0,
                    pos: Vec2 { x: 400.0, y: 50.0 },
                    real: true,
                    touch: false,
                    label: "b#0 click".into(),
                }],
                hidden: Vec::new(),
                not_live: Vec::new(),
            },
            warnings: Vec::new(),
        }
    }

    #[test]
    fn captures_every_frame_in_order_with_exact_time_steps() {
        let mut page = FakePage::default();
        let mut sink = FakeSink::default();
        let mut seen = Vec::new();
        let stats = capture(&mut page, &timeline(0), &mut sink, &mut |p| {
            seen.push(p.done)
        })
        .unwrap();

        assert_eq!(stats.frames, 10);
        assert!(sink.finished);
        assert_eq!(
            sink.frames.iter().map(|f| f.0).collect::<Vec<_>>(),
            (0..10).collect::<Vec<_>>()
        );
        assert_eq!(page.steps[0], (0.0, 0.0), "frame 0 = time 0");
        assert!(
            page.steps[1..].iter().all(|s| (s.1 - 100.0).abs() < 1e-9),
            "100ms per frame at 10fps"
        );
        assert_eq!(page.steps[5].0, 50.0, "linear scroll halfway");
        assert_eq!(seen.last(), Some(&10));
        assert!(
            sink.frames.iter().all(|f| f.2 == CursorKind::Pointer),
            "kind from the page"
        );
    }

    #[test]
    fn presses_are_sent_once_on_the_right_frames() {
        let mut page = FakePage::default();
        let mut sink = FakeSink::default();
        capture(&mut page, &timeline(0), &mut sink, &mut |_| {}).unwrap();
        let downs: Vec<usize> = (0..page.pointers.len())
            .filter(|&i| page.pointers[i].events.iter().any(|e| e.down))
            .collect();
        let ups: Vec<usize> = (0..page.pointers.len())
            .filter(|&i| page.pointers[i].events.iter().any(|e| !e.down))
            .collect();
        // Click at 450 ms → frame at 500 ms; release at 540 ms → frame at 600 ms.
        assert_eq!((downs, ups), (vec![5], vec![6]));
        assert!(page.pointers.iter().all(|p| p.live && p.x == 400.0));
    }

    #[test]
    fn warmup_runs_before_the_first_frame_without_capturing() {
        let mut page = FakePage::default();
        let mut sink = FakeSink::default();
        capture(&mut page, &timeline(250), &mut sink, &mut |_| {}).unwrap();
        assert_eq!(&page.steps[..3], &[(0.0, 100.0), (0.0, 100.0), (0.0, 50.0)]);
        assert!(
            page.pointers[..3]
                .iter()
                .all(|p| p.events.is_empty() && p.live),
            "pointer in place, no clicks"
        );
        assert_eq!(sink.frames.len(), 10, "warm-up adds no frames");
    }

    #[test]
    fn reports_scroll_drift_when_the_page_is_shorter() {
        let mut page = FakePage {
            clamp_at: Some(80.0),
            ..FakePage::default()
        };
        let mut sink = FakeSink::default();
        let stats = capture(&mut page, &timeline(0), &mut sink, &mut |_| {}).unwrap();
        assert!(
            (stats.max_scroll_drift_px - 10.0).abs() < 1e-9,
            "planned 90, applied 80"
        );
    }

    #[test]
    fn times_steps_screenshots_and_sink_separately() {
        // 10 frames + 3 warm-up steps. Each part must reach its own minimum
        // while all three together fit into the wall-clock time, so a part
        // that is missed or counted twice fails. (Only lower bounds: a sleep
        // may take longer than asked on a busy machine.)
        let ms = Duration::from_millis;
        let mut page = FakePage {
            step_delay: ms(2),
            capture_delay: ms(6),
            ..FakePage::default()
        };
        let mut sink = FakeSink {
            push_delay: ms(12),
            ..FakeSink::default()
        };
        let started = Instant::now();
        let stats = capture(&mut page, &timeline(250), &mut sink, &mut |_| {}).unwrap();
        let wall = started.elapsed();

        assert!(stats.step_time >= ms(2) * 13, "{:?}", stats.step_time);
        assert!(
            stats.screenshot_time >= ms(6) * 10,
            "{:?}",
            stats.screenshot_time
        );
        assert!(stats.sink_time >= ms(12) * 10, "{:?}", stats.sink_time);
        assert!(
            stats.step_time + stats.screenshot_time + stats.sink_time <= wall,
            "parts run one after another: {stats:?} vs {wall:?}"
        );
    }
}
