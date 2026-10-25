//! # sr-timeline — the frame-by-frame plan
//!
//! Purpose: turns a [`FlowPlan`] (start → blocks → end) plus the measured
//!          block positions into a list of timed segments ("scroll from y=0
//!          to y=1840 in 1.6 s, ease-in-out", "hold 1.5 s") and answers
//!          "where is the page scrolled at frame N?".
//! Solves:  rendering is deterministic only if every frame is known before
//!          the browser starts drawing (HANDOFF §7.1, principle 4). It also
//!          makes the total video length known up front, and it's pure math,
//!          so every rule is unit-tested without a browser.
//! Rules:   no IO, no browser. Positions are CSS pixels in page coordinates.
//! Key rule: blocks with `hold_ms: 0` are *waypoints*. Consecutive moves
//!          through them merge into one smooth move, so the scroll doesn't
//!          brake at every block (HANDOFF D-06, "проскролить без остановки").
//!          A waypoint that has actions becomes a stop: the cursor needs the
//!          page to stand still while it moves to a button.
//! Cursor:  the cursor track is planned in the same pass ([`cursor`]), so a
//!          block stop lasts `max(hold_ms, time the actions need)`.

pub mod cursor;

use std::collections::BTreeMap;

use serde::Serialize;
use sr_core::flow::{Align, Anchor, AnchorPoint, ScrollCursor, ScrollMove};
use sr_core::{AppError, AppResult, Easing, ErrorCode, FlowPlan};

use cursor::CursorBuilder;
pub use cursor::{
    ActiveClick, ClickMark, CursorFrame, CursorSegment, CursorTrack, PRESS_HOLD_MS, PointerEvent,
    PointerEventKind, TAP_LEAD_MS, Vec2,
};

/// Shortest scroll move, so tiny distances don't look like a jump.
const MIN_MOVE_MS: f64 = 300.0;
/// Moves shorter than this (CSS px) count as "no move".
const MIN_DISTANCE_PX: f64 = 0.5;

/// What the page measured (CSS px, page coordinates).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PageMetrics {
    /// Viewport width (cursor positions are clamped to it).
    pub viewport_width: f64,
    /// Viewport height.
    pub viewport_height: f64,
    /// Largest possible scroll position (page height − viewport height).
    pub max_scroll: f64,
    /// Sticky/fixed header height; `align: top` lands blocks below it.
    pub top_inset: f64,
    /// Measured blocks and action targets by id (`block` or `block#n`).
    pub blocks: BTreeMap<String, BlockBox>,
}

/// Position of one block or element (page coordinates).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct BlockBox {
    /// Top edge.
    pub top: f64,
    /// Height.
    pub height: f64,
    /// Left edge.
    pub left: f64,
    /// Width.
    pub width: f64,
}

/// Kind of a timeline segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SegmentKind {
    /// The page scrolls.
    Scroll,
    /// The page stays still (animations keep playing).
    Hold,
}

/// One timed piece of the video.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Segment {
    /// Scroll or hold.
    pub kind: SegmentKind,
    /// Start time in ms (video time, after warm-up).
    pub start_ms: f64,
    /// End time in ms.
    pub end_ms: f64,
    /// Scroll position at the start.
    pub from_y: f64,
    /// Scroll position at the end.
    pub to_y: f64,
    /// Curve used for scroll segments.
    #[serde(serialize_with = "easing_as_text")]
    pub easing: Easing,
    /// Node ids this segment belongs to, e.g. `"hero → features → pricing"`.
    pub label: String,
}

fn easing_as_text<S: serde::Serializer>(e: &Easing, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&e.to_string())
}

/// The complete plan for one video.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Timeline {
    /// Frames per second.
    pub fps: u32,
    /// Device scale factor; scroll positions snap to whole device pixels.
    pub scale: f64,
    /// Page time to skip before frame 0.
    pub warmup_ms: u32,
    /// Video length in ms.
    pub duration_ms: f64,
    /// Segments in time order, without gaps.
    pub segments: Vec<Segment>,
    /// The cursor: moves, clicks, hidden windows.
    pub cursor: CursorTrack,
    /// Non-fatal notes found while planning (e.g. a button outside the frame).
    pub warnings: Vec<String>,
}

/// One frame to capture.
#[derive(Debug, Clone, PartialEq)]
pub struct FramePlan {
    /// 0-based frame number.
    pub index: u32,
    /// Video time of this frame in ms.
    pub time_ms: f64,
    /// Scroll position (CSS px, snapped to device pixels).
    pub scroll_y: f64,
    /// Cursor on this frame (position, opacity, events to send).
    pub cursor: CursorFrame,
}

// ── Positions ───────────────────────────────────────────────────────────────

impl PageMetrics {
    fn block(&self, id: &str) -> AppResult<BlockBox> {
        self.blocks.get(id).copied().ok_or_else(|| {
            AppError::new(
                ErrorCode::Internal,
                format!("block '{id}' was not measured"),
            )
        })
    }

    fn clamp(&self, y: f64) -> f64 {
        y.clamp(0.0, self.max_scroll.max(0.0))
    }

    /// Scroll position that shows `block` with the given alignment.
    pub fn align_y(&self, id: &str, align: Align, offset_px: f64) -> AppResult<f64> {
        let b = self.block(id)?;
        let vh = self.viewport_height;
        let y = match align {
            Align::Top => b.top - self.top_inset,
            Align::Center => b.top + b.height / 2.0 - (vh + self.top_inset) / 2.0,
            Align::Bottom => b.top + b.height - vh,
        };
        Ok(self.clamp(y + offset_px))
    }

    /// Scroll position of an anchor (HANDOFF FR-ANC-01).
    pub fn anchor_y(&self, a: &Anchor) -> AppResult<f64> {
        let b = self.block(&a.block)?;
        let vh = self.viewport_height;
        let enter = b.top - vh;
        let exit = b.top + b.height;
        let y = match (a.progress, a.anchor) {
            (Some(p), _) => enter + p * (exit - enter),
            (None, AnchorPoint::Top) => b.top - self.top_inset,
            (None, AnchorPoint::Center) => b.top + b.height / 2.0 - (vh + self.top_inset) / 2.0,
            (None, AnchorPoint::Bottom) => b.top + b.height - vh,
            (None, AnchorPoint::Enter) => enter,
            (None, AnchorPoint::Exit) => exit,
        };
        Ok(self.clamp(y + a.offset_px))
    }
}

// ── Building the timeline ───────────────────────────────────────────────────

/// A move waiting to be turned into segments.
struct Pending {
    to_y: f64,
    mv: ScrollMove,
    cursor: ScrollCursor,
    label: String,
}

struct Builder {
    t: f64,
    y: f64,
    segments: Vec<Segment>,
    pending: Vec<Pending>,
    cursor: CursorBuilder,
}

/// Cursor mode of a merged run: the most noticeable one wins, so a
/// `hide` on any arrow of the run is never silently ignored.
fn run_cursor(run: &[Pending]) -> ScrollCursor {
    let has = |m: ScrollCursor| run.iter().any(|p| p.cursor == m);
    if has(ScrollCursor::Hide) {
        ScrollCursor::Hide
    } else if has(ScrollCursor::Follow) {
        ScrollCursor::Follow
    } else {
        ScrollCursor::Still
    }
}

impl Builder {
    fn duration_of(distance: f64, mv: ScrollMove) -> f64 {
        match mv.duration_ms {
            Some(ms) => f64::from(ms),
            None if distance < MIN_DISTANCE_PX => 0.0,
            None => (distance / mv.speed_px_s.max(1.0) * 1000.0).max(MIN_MOVE_MS),
        }
    }

    // Merges pending waypoint moves into as few smooth segments as possible.
    // A new segment starts only where the scroll direction reverses.
    fn flush(&mut self) {
        let pending = std::mem::take(&mut self.pending);
        let mut run: Vec<Pending> = Vec::new();
        let mut run_dir = 0.0_f64;
        let mut cursor = self.y;
        for p in pending {
            let dir = (p.to_y - cursor).signum()
                * f64::from(u8::from((p.to_y - cursor).abs() >= MIN_DISTANCE_PX));
            if dir != 0.0 && run_dir != 0.0 && dir != run_dir {
                self.emit_run(std::mem::take(&mut run));
                run_dir = 0.0;
            }
            if dir != 0.0 {
                run_dir = dir;
            }
            cursor = p.to_y;
            run.push(p);
        }
        self.emit_run(run);
    }

    fn emit_run(&mut self, run: Vec<Pending>) {
        let Some(first) = run.first() else { return };
        let easing = first.mv.easing;
        let mut duration = 0.0;
        let mut cursor = self.y;
        for p in &run {
            duration += Self::duration_of((p.to_y - cursor).abs(), p.mv);
            cursor = p.to_y;
        }
        if duration <= 0.0 {
            self.y = cursor;
            return;
        }
        let label: Vec<&str> = run.iter().map(|p| p.label.as_str()).collect();
        let kind = if (cursor - self.y).abs() < MIN_DISTANCE_PX {
            SegmentKind::Hold
        } else {
            SegmentKind::Scroll
        };
        self.segments.push(Segment {
            kind,
            start_ms: self.t,
            end_ms: self.t + duration,
            from_y: self.y,
            to_y: cursor,
            easing,
            label: label.join(" → "),
        });
        self.cursor
            .on_scroll(self.t, self.t + duration, self.y, cursor, run_cursor(&run));
        self.t += duration;
        self.y = cursor;
    }

    fn hold(&mut self, ms: f64, label: &str) {
        self.flush();
        if ms <= 0.0 {
            return;
        }
        self.segments.push(Segment {
            kind: SegmentKind::Hold,
            start_ms: self.t,
            end_ms: self.t + ms,
            from_y: self.y,
            to_y: self.y,
            easing: Easing::Linear,
            label: format!("hold {label}"),
        });
        self.t += ms;
    }
}

impl Timeline {
    /// Builds the timeline for one target.
    pub fn build(plan: &FlowPlan, metrics: &PageMetrics) -> AppResult<Timeline> {
        let start_y = metrics.anchor_y(&plan.start.at)?;
        let mut b = Builder {
            t: 0.0,
            y: start_y,
            segments: Vec::new(),
            pending: Vec::new(),
            cursor: CursorBuilder::new(plan, metrics),
        };

        for block in &plan.blocks {
            let to_y = metrics.align_y(&block.id, block.align, block.offset_px)?;
            b.pending.push(Pending {
                to_y,
                mv: block.arrive,
                cursor: block.arrive_cursor,
                label: block.id.clone(),
            });
            // ── Stop: arrive, run the actions, hold ──
            // The stop lasts as long as the longer of the hold and the
            // actions, so a hover is never cut off by the next scroll.
            if block.hold_ms > 0 || !block.actions.is_empty() {
                b.flush();
                let actions_ms = b.cursor.run_actions(b.t, b.y, block, metrics);
                b.hold(f64::from(block.hold_ms).max(actions_ms), &block.id);
            }
        }
        if let Some(at) = &plan.end.at {
            let to_y = metrics.anchor_y(at)?;
            b.pending.push(Pending {
                to_y,
                mv: plan.end.arrive,
                cursor: plan.end.arrive_cursor,
                label: plan.end.id.clone(),
            });
        }
        b.hold(f64::from(plan.end.hold_ms), &plan.end.id);

        let fps = plan.target.fps.max(1);
        let one_frame = 1000.0 / f64::from(fps);
        let warnings = std::mem::take(&mut b.cursor.warnings);
        Ok(Timeline {
            fps,
            scale: plan.geometry.scale,
            warmup_ms: plan.start.warmup_ms,
            duration_ms: b.t.max(one_frame),
            segments: b.segments,
            cursor: b.cursor.finish(),
            warnings,
        })
    }

    /// Number of frames in the video (at least 1).
    pub fn frame_count(&self) -> u32 {
        ((self.duration_ms * f64::from(self.fps) / 1000.0).round() as u32).max(1)
    }

    /// Time between frames in ms.
    pub fn frame_interval_ms(&self) -> f64 {
        1000.0 / f64::from(self.fps)
    }

    /// Scroll position at video time `t_ms` (unsnapped).
    pub fn scroll_at(&self, t_ms: f64) -> f64 {
        let Some(first) = self.segments.first() else {
            return 0.0;
        };
        if t_ms <= first.start_ms {
            return first.from_y;
        }
        for s in &self.segments {
            if t_ms < s.end_ms {
                let span = (s.end_ms - s.start_ms).max(f64::EPSILON);
                let p = s.easing.apply((t_ms - s.start_ms) / span);
                return s.from_y + (s.to_y - s.from_y) * p;
            }
        }
        self.segments.last().map_or(0.0, |s| s.to_y)
    }

    /// Start position for timelines without segments.
    fn start_y(&self) -> f64 {
        self.segments.first().map_or(0.0, |s| s.from_y)
    }

    /// All frames in order.
    pub fn frames(&self) -> impl Iterator<Item = FramePlan> + '_ {
        let interval = self.frame_interval_ms();
        (0..self.frame_count()).map(move |index| {
            let time_ms = f64::from(index) * interval;
            let raw = if self.segments.is_empty() {
                self.start_y()
            } else {
                self.scroll_at(time_ms)
            };
            // Snap to whole device pixels: avoids sub-pixel shimmer of text.
            let scroll_y = (raw * self.scale).round() / self.scale;
            let cursor = self.cursor.frame(time_ms, interval, &|t| self.scroll_at(t));
            FramePlan {
                index,
                time_ms,
                scroll_y,
                cursor,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sr_core::Flow;

    fn plan(nodes: &str, edges: &str, hold: u32) -> FlowPlan {
        let json = format!(
            r#"{{"format_version":1,"meta":{{"name":"t"}},
            "targets":[{{"id":"d","viewport":"desktop-1920","aspect":"16:9","fps":50}}],
            "defaults":{{"hold_ms":{hold},"scroll":{{"speed_px_s":1000,"easing":"linear"}}}},
            "pages":[{{"id":"p","url":"/"}}],"nodes":[{nodes}],"edges":[{edges}]}}"#
        );
        Flow::from_json(&json).unwrap().plan("d").unwrap()
    }

    fn metrics() -> PageMetrics {
        let mut blocks = BTreeMap::new();
        blocks.insert(
            "a".into(),
            BlockBox {
                top: 0.0,
                height: 1000.0,
                left: 0.0,
                width: 1920.0,
            },
        );
        blocks.insert(
            "b".into(),
            BlockBox {
                top: 1000.0,
                height: 1000.0,
                left: 0.0,
                width: 1920.0,
            },
        );
        blocks.insert(
            "c".into(),
            BlockBox {
                top: 2000.0,
                height: 2000.0,
                left: 0.0,
                width: 1920.0,
            },
        );
        PageMetrics {
            viewport_width: 1920.0,
            viewport_height: 1080.0,
            max_scroll: 2920.0,
            top_inset: 80.0,
            blocks,
        }
    }

    const NODES: &str = r#"{"id":"s","kind":"start","at":{"block":"a"}},
        {"id":"a","kind":"block","page":"p","target":[{"sr":"a"}]},
        {"id":"b","kind":"block","page":"p","target":[{"sr":"b"}],"hold_ms":0},
        {"id":"c","kind":"block","page":"p","target":[{"sr":"c"}]},
        {"id":"e","kind":"end","hold_ms":500}"#;
    const EDGES: &str = r#"{"from":"s","to":"a"},{"from":"a","to":"b"},{"from":"b","to":"c"},{"from":"c","to":"e"}"#;

    #[test]
    fn anchors_and_alignment_respect_sticky_header_and_page_end() {
        let m = metrics();
        assert_eq!(
            m.align_y("b", Align::Top, 0.0).unwrap(),
            920.0,
            "below the 80px header"
        );
        assert_eq!(
            m.align_y("c", Align::Bottom, 0.0).unwrap(),
            2920.0,
            "clamped to max scroll"
        );
        let enter = Anchor {
            block: "b".into(),
            anchor: AnchorPoint::Enter,
            progress: None,
            offset_px: 0.0,
        };
        assert_eq!(m.anchor_y(&enter).unwrap(), 0.0, "1000 - 1080 clamps to 0");
        let half = Anchor {
            progress: Some(0.5),
            ..enter
        };
        assert_eq!(
            m.anchor_y(&half).unwrap(),
            960.0,
            "midway between enter and exit"
        );
    }

    #[test]
    fn waypoints_merge_into_one_smooth_move() {
        let t = Timeline::build(&plan(NODES, EDGES, 1000), &metrics()).unwrap();
        let kinds: Vec<_> = t
            .segments
            .iter()
            .map(|s| (s.kind, s.label.as_str()))
            .collect();
        assert_eq!(
            kinds,
            [
                (SegmentKind::Hold, "hold a"),
                (SegmentKind::Scroll, "b → c"),
                (SegmentKind::Hold, "hold c"),
                (SegmentKind::Hold, "hold e"),
            ]
        );
        let mv = &t.segments[1];
        assert_eq!((mv.from_y, mv.to_y), (0.0, 1920.0));
        assert!(
            (mv.end_ms - mv.start_ms - 1920.0).abs() < 1e-6,
            "1920px at 1000px/s"
        );
        assert!((t.duration_ms - (1000.0 + 1920.0 + 1000.0 + 500.0)).abs() < 1e-6);
    }

    #[test]
    fn frames_follow_the_segments_and_snap_to_pixels() {
        let t = Timeline::build(&plan(NODES, EDGES, 1000), &metrics()).unwrap();
        assert_eq!(
            t.frame_count(),
            (t.duration_ms * 50.0 / 1000.0).round() as u32
        );
        let frames: Vec<FramePlan> = t.frames().collect();
        assert_eq!(frames[0].scroll_y, 0.0);
        assert_eq!(frames.last().unwrap().scroll_y, 1920.0);
        assert!(
            frames.iter().all(|f| f.scroll_y.fract() == 0.0),
            "scale 1 → whole pixels"
        );
        assert!(
            frames.windows(2).all(|w| w[1].scroll_y >= w[0].scroll_y),
            "only scrolls down here"
        );
    }

    #[test]
    fn direction_change_splits_the_move() {
        let nodes = NODES.replace(
            r#""target":[{"sr":"c"}]}"#,
            r#""target":[{"sr":"c"}],"hold_ms":0}"#,
        );
        let nodes = format!(
            r#"{nodes},{{"id":"back","kind":"block","page":"p","target":[{{"sr":"a"}}],"hold_ms":0}}"#
        );
        let edges = EDGES.replace(
            r#"{"from":"c","to":"e"}"#,
            r#"{"from":"c","to":"back"},{"from":"back","to":"e"}"#,
        );
        let mut m = metrics();
        m.blocks.insert(
            "back".into(),
            BlockBox {
                top: 0.0,
                height: 1000.0,
                left: 0.0,
                width: 1920.0,
            },
        );
        let t = Timeline::build(&plan(&nodes, &edges, 1000), &m).unwrap();
        let scrolls: Vec<_> = t
            .segments
            .iter()
            .filter(|s| s.kind == SegmentKind::Scroll)
            .collect();
        assert_eq!(scrolls.len(), 2, "down run and up run");
        assert_eq!(scrolls[1].to_y, 0.0);
    }

    #[test]
    fn serializes_for_timeline_json() {
        let t = Timeline::build(&plan(NODES, EDGES, 1000), &metrics()).unwrap();
        let v = serde_json::to_value(&t).unwrap();
        assert_eq!(v["segments"][1]["easing"], "linear");
        assert_eq!(v["segments"][1]["kind"], "scroll");
    }

    // ── Cursor integration ──────────────────────────────────────────────────

    /// Metrics with a 200×60 button at (900, 1400) inside block "b".
    fn metrics_with_button() -> PageMetrics {
        let mut m = metrics();
        m.blocks.insert(
            "b#0".into(),
            BlockBox {
                top: 1400.0,
                height: 60.0,
                left: 900.0,
                width: 200.0,
            },
        );
        m
    }

    fn nodes_with(action: &str, b_hold: u32) -> String {
        NODES.replace(
            r#""target":[{"sr":"b"}],"hold_ms":0}"#,
            &format!(r#""target":[{{"sr":"b"}}],"hold_ms":{b_hold},"actions":[{action}]}}"#),
        )
    }

    #[test]
    fn a_waypoint_with_actions_becomes_a_stop_long_enough_for_them() {
        let nodes = nodes_with(
            r#"{"do":"hover","target":[{"sr":"buy"}],"duration_ms":600}"#,
            0,
        );
        let t = Timeline::build(&plan(&nodes, EDGES, 1000), &metrics_with_button()).unwrap();
        let hold_b = t.segments.iter().find(|s| s.label == "hold b").unwrap();
        let mv = t.cursor.segments.first().unwrap();
        let CursorSegment::Move { start_ms, .. } = mv else {
            panic!("expected a move, got {mv:?}")
        };
        assert_eq!(
            *start_ms,
            hold_b.start_ms + cursor::REACTION_MS,
            "the move starts a moment after the page stops"
        );
        // A long first move may overshoot: the hold ends after the last move + rest.
        let CursorSegment::Move { end_ms, to, .. } = t.cursor.segments.last().unwrap() else {
            panic!()
        };
        assert!(
            (hold_b.end_ms - (end_ms + 600.0)).abs() < 1e-6,
            "hold = reaction + move + rest"
        );
        // Block b stops at 920 (below the header): button centre is ~1000 - 920 + 30.
        assert!(
            (to.y - (1430.0 - 920.0)).abs() <= 4.0 && (to.x - 1000.0).abs() <= 6.0,
            "{to:?}"
        );
    }

    #[test]
    fn a_long_hold_is_not_extended_by_short_actions() {
        let nodes = nodes_with(r#"{"do":"hover","target":[{"sr":"buy"}]}"#, 5000);
        let t = Timeline::build(&plan(&nodes, EDGES, 1000), &metrics_with_button()).unwrap();
        let hold_b = t.segments.iter().find(|s| s.label == "hold b").unwrap();
        assert!((hold_b.end_ms - hold_b.start_ms - 5000.0).abs() < 1e-6);
    }

    #[test]
    fn cursor_paths_are_reproducible_and_change_with_the_seed() {
        let nodes = nodes_with(r#"{"do":"click","target":[{"sr":"buy"}],"mode":"real"}"#, 0);
        let build = |seed: u64| {
            let mut p = plan(&nodes, EDGES, 1000);
            p.cursor.seed = seed;
            Timeline::build(&p, &metrics_with_button()).unwrap()
        };
        assert_eq!(build(1).cursor, build(1).cursor, "same seed → same path");
        assert_ne!(build(1).cursor, build(2).cursor, "other seed → other curve");

        let t = build(1);
        let frames: Vec<FramePlan> = t.frames().collect();
        let events: Vec<_> = frames.iter().flat_map(|f| f.cursor.events.iter()).collect();
        assert_eq!(events.len(), 2, "one press, one release");
        assert_eq!(events[0].kind, PointerEventKind::Press);
        let press_frame = frames.iter().find(|f| !f.cursor.events.is_empty()).unwrap();
        let hold_b = t.segments.iter().find(|s| s.label == "hold b").unwrap();
        assert!(press_frame.time_ms > hold_b.start_ms && press_frame.time_ms < hold_b.end_ms);
        assert!(press_frame.cursor.live && press_frame.cursor.opacity == 1.0);
    }

    #[test]
    fn long_moves_may_overshoot_but_always_land_on_target() {
        let nodes = nodes_with(r#"{"do":"hover","target":[{"sr":"buy"}]}"#, 0);
        let mut overshoots = 0;
        for seed in 0..40 {
            let mut p = plan(&nodes, EDGES, 1000);
            p.cursor.seed = seed;
            p.cursor.start = sr_core::flow::Point { x: 0.05, y: 0.05 };
            let t = Timeline::build(&p, &metrics_with_button()).unwrap();
            let moves: Vec<_> = t.cursor.segments.iter().collect();
            let Some(CursorSegment::Move {
                to,
                start_ms,
                end_ms,
                ..
            }) = moves.last()
            else {
                panic!()
            };
            assert!((to.x - 1000.0).abs() <= 6.0 && (to.y - 510.0).abs() <= 4.0);
            if moves.len() == 2 {
                overshoots += 1;
                let CursorSegment::Move { to: past, .. } = moves[0] else {
                    panic!()
                };
                assert!(past.dist(*to) <= 13.0, "a few pixels past, not more");
                assert!(end_ms - start_ms <= 180.0, "the correction is quick");
            }
        }
        assert!(
            (10..35).contains(&overshoots),
            "some, not all: {overshoots}/40"
        );
    }

    #[test]
    fn hide_fades_out_during_the_scroll_only() {
        let edges = EDGES.replace(
            r#"{"from":"b","to":"c"}"#,
            r#"{"from":"b","to":"c","cursor":"hide"}"#,
        );
        let t = Timeline::build(&plan(NODES, &edges, 1000), &metrics()).unwrap();
        let scroll = t
            .segments
            .iter()
            .find(|s| s.kind == SegmentKind::Scroll)
            .unwrap();
        assert_eq!(
            t.cursor.hidden.len(),
            1,
            "waypoint b merges into one hidden run"
        );
        let mid = (scroll.start_ms + scroll.end_ms) / 2.0;
        let opacity = |time: f64| t.cursor.frame(time, 20.0, &|x| t.scroll_at(x)).opacity;
        assert_eq!(opacity(mid), 0.0);
        assert_eq!(opacity(scroll.end_ms + 1.0), 1.0);
        assert_eq!(opacity(scroll.start_ms - 1.0), 1.0);
    }

    #[test]
    fn follow_moves_the_cursor_with_the_page() {
        let edges = EDGES.replace(
            r#"{"from":"b","to":"c"}"#,
            r#"{"from":"b","to":"c","cursor":"follow"}"#,
        );
        let t = Timeline::build(&plan(NODES, &edges, 1000), &metrics()).unwrap();
        let start = t.cursor.start;
        let scroll = t
            .segments
            .iter()
            .find(|s| s.kind == SegmentKind::Scroll)
            .unwrap();
        let at = |time: f64| t.cursor.position_at(time, &|x| t.scroll_at(x));
        let a = at(scroll.start_ms + 100.0);
        assert!((a.y - (start.y - t.scroll_at(scroll.start_ms + 100.0))).abs() < 1e-6);
        assert!(at(scroll.end_ms).y >= 8.0, "never leaves the frame");
        assert_eq!(a.x, start.x);
    }

    #[test]
    fn touch_taps_show_an_indicator_without_a_cursor() {
        let json = format!(
            r#"{{"format_version":1,"meta":{{"name":"t"}},
            "targets":[{{"id":"m","viewport":"iphone-15","aspect":"9:16","fps":30}}],
            "defaults":{{"hold_ms":1000}},
            "pages":[{{"id":"p","url":"/"}}],"nodes":[{}],"edges":[{EDGES}]}}"#,
            nodes_with(
                r#"{"do":"hover","target":[{"sr":"x"}]},{"do":"click","target":[{"sr":"buy"}]}"#,
                0
            )
        );
        let p = Flow::from_json(&json).unwrap().plan("m").unwrap();
        assert!(p.touch);
        let mut m = metrics_with_button();
        m.blocks.insert("b#1".into(), m.blocks["b#0"]);
        m.viewport_width = 393.0;
        m.viewport_height = 852.0;
        m.blocks.get_mut("b#1").unwrap().left = 100.0;
        let t = Timeline::build(&p, &m).unwrap();
        assert!(
            !t.cursor.visible && t.cursor.segments.is_empty(),
            "a finger doesn't glide"
        );
        assert_eq!(t.cursor.clicks.len(), 1, "hover was dropped, the tap stays");
        let tap = &t.cursor.clicks[0];
        assert!(tap.touch && !tap.real, "taps are visual by default");
        let f = t.cursor.frame(tap.at_ms - 50.0, 33.0, &|x| t.scroll_at(x));
        assert_eq!(f.opacity, 0.0);
        assert!(!f.live, "no mouse on a phone");
        assert_eq!(f.clicks.len(), 1, "indicator appears just before the touch");
    }

    #[test]
    fn targets_outside_the_frame_are_clamped_with_a_warning() {
        let nodes = nodes_with(r#"{"do":"hover","target":[{"sr":"buy"}]}"#, 0);
        let mut m = metrics_with_button();
        m.blocks.get_mut("b#0").unwrap().top = 3000.0;
        let t = Timeline::build(&plan(&nodes, EDGES, 1000), &m).unwrap();
        assert!(
            t.warnings
                .iter()
                .any(|w| w.contains("outside the visible area")),
            "{:?}",
            t.warnings
        );
        let CursorSegment::Move { to, .. } = &t.cursor.segments[0] else {
            panic!()
        };
        assert!(to.y <= 1080.0 - 8.0);
    }
}
