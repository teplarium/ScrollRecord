//! # sr-capture — frame-by-frame capture
//!
//! Purpose: walks the [`Timeline`] frame by frame: tells the page where to
//!          scroll and how much time passed, takes a screenshot and passes it
//!          to a [`FrameSink`] (encoder, PNG folder, later the compositor).
//! Solves:  this loop is what makes the video smooth. Each frame is drawn
//!          completely before it's captured, so a slow machine only makes
//!          rendering slower, never the video choppier (HANDOFF §7.3).
//! Rules:   knows nothing about browsers or file formats: it talks to a
//!          [`FrameDriver`] and a [`FrameSink`], so both are swappable and
//!          the loop is tested with fakes.

use sr_browser::FrameDriver;
use sr_core::AppResult;
use sr_timeline::Timeline;

/// Receives captured frames in order.
pub trait FrameSink {
    /// Takes frame number `index` as PNG bytes.
    fn push(&mut self, index: u32, png: &[u8]) -> AppResult<()>;

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
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CaptureStats {
    /// Frames captured.
    pub frames: u32,
    /// Largest difference between planned and applied scroll position, in
    /// CSS px. Non-zero means the page was shorter than measured (it changed
    /// height during recording).
    pub max_scroll_drift_px: f64,
}

/// Captures every frame of `timeline`.
///
/// Before frame 0 the page "lives" for `warmup_ms` (in frame-sized steps so
/// animation-frame code runs as usual) at the start position.
pub fn capture(
    driver: &mut dyn FrameDriver,
    timeline: &Timeline,
    sink: &mut dyn FrameSink,
    on_progress: &mut dyn FnMut(Progress),
) -> AppResult<CaptureStats> {
    let interval = timeline.frame_interval_ms();
    let total = timeline.frame_count();
    let mut frames = timeline.frames();
    let Some(first) = frames.next() else {
        return Ok(CaptureStats {
            frames: 0,
            max_scroll_drift_px: 0.0,
        });
    };

    // ── Warm-up: skip intros without recording them ──
    let mut left = f64::from(timeline.warmup_ms);
    while left > 0.0 {
        let dt = left.min(interval);
        driver.step(first.scroll_y, dt)?;
        left -= dt;
    }

    // ── Frames: frame 0 shows time 0; every next frame advances one interval ──
    let mut drift: f64 = 0.0;
    let mut done = 0;
    for frame in std::iter::once(first).chain(frames) {
        let dt = if frame.index == 0 { 0.0 } else { interval };
        let applied = driver.step(frame.scroll_y, dt)?;
        drift = drift.max((applied - frame.scroll_y).abs());
        let png = driver.capture_png()?;
        sink.push(frame.index, &png)?;
        done += 1;
        on_progress(Progress { done, total });
    }
    sink.finish()?;
    if drift > 1.0 {
        tracing::warn!(
            drift,
            "page scrolled less than planned (page got shorter while recording?)"
        );
    }
    Ok(CaptureStats {
        frames: done,
        max_scroll_drift_px: drift,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sr_timeline::{Segment, SegmentKind};

    /// Records every call instead of talking to a browser.
    #[derive(Default)]
    struct FakePage {
        steps: Vec<(f64, f64)>,
        clamp_at: Option<f64>,
    }

    impl FrameDriver for FakePage {
        fn step(&mut self, scroll_y: f64, advance_ms: f64) -> AppResult<f64> {
            self.steps.push((scroll_y, advance_ms));
            Ok(self.clamp_at.map_or(scroll_y, |max| scroll_y.min(max)))
        }
        fn capture_png(&mut self) -> AppResult<Vec<u8>> {
            Ok(vec![self.steps.len() as u8])
        }
    }

    #[derive(Default)]
    struct FakeSink {
        frames: Vec<(u32, Vec<u8>)>,
        finished: bool,
    }

    impl FrameSink for FakeSink {
        fn push(&mut self, index: u32, png: &[u8]) -> AppResult<()> {
            self.frames.push((index, png.to_vec()));
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
    }

    #[test]
    fn warmup_runs_before_the_first_frame_without_capturing() {
        let mut page = FakePage::default();
        let mut sink = FakeSink::default();
        capture(&mut page, &timeline(250), &mut sink, &mut |_| {}).unwrap();
        assert_eq!(&page.steps[..3], &[(0.0, 100.0), (0.0, 100.0), (0.0, 50.0)]);
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
}
