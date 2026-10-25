//! # Cursor track — where the cursor is on every frame
//!
//! Purpose: plans the visual cursor along the scroll timeline: human-like
//!          moves to buttons, rests (hover), clicks with press/ripple, taps on
//!          touch screens, and what happens while the page scrolls.
//! Solves:  mechanical straight-line cursors look fake. Moves here follow a
//!          gently curved path (Bézier), take longer for far/small targets
//!          (Fitts's law) and slow down on arrival. The curve's shape comes
//!          from a seeded random generator, so every render of the same
//!          scenario moves identically (HANDOFF FR-CUR-02).
//! Units:   positions are CSS pixels in *viewport* coordinates (what the
//!          viewer sees), times are ms of video time.

use serde::Serialize;
use sr_core::Easing;
use sr_core::flow::{ActionKind, CursorSettings, PlanAction, PlanBlock, PointerMode, ScrollCursor};

use crate::PageMetrics;

// ── Timing constants (tuned by eye; see ADR 0003) ───────────────────────────
const MOVE_BASE_MS: f64 = 230.0;
const MOVE_LOG_MS: f64 = 140.0;
const MOVE_MIN_MS: f64 = 220.0;
const MOVE_MAX_MS: f64 = 1400.0;
/// Short pause between arriving on a button and pressing it.
const CLICK_SETTLE_MS: f64 = 120.0;
/// A person needs a moment after the page stops before moving the mouse.
pub const REACTION_MS: f64 = 250.0;
/// Moves longer than this (CSS px) may overshoot a little and correct.
const OVERSHOOT_MIN_PX: f64 = 220.0;
/// Share of long moves that overshoot (decided by the seed).
const OVERSHOOT_CHANCE: f64 = 0.55;
/// Time between mouse-down and mouse-up.
pub const PRESS_HOLD_MS: f64 = 90.0;
/// Rest after a click so the ripple can be seen.
const CLICK_AFTER_MS: f64 = 380.0;
/// A finger indicator appears this long before the tap.
pub const TAP_LEAD_MS: f64 = 150.0;
/// Fade time when the cursor hides during scrolling.
const FADE_MS: f64 = 150.0;
/// Keep the cursor this far inside the frame.
const EDGE_MARGIN: f64 = 8.0;
/// Clicks stay "active" (for press/ripple drawing) this long.
const CLICK_VISIBLE_MS: f64 = 1000.0;

/// A point in viewport CSS pixels.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Vec2 {
    /// Horizontal.
    pub x: f64,
    /// Vertical.
    pub y: f64,
}

impl Vec2 {
    fn lerp(self, o: Vec2, t: f64) -> Vec2 {
        Vec2 {
            x: self.x + (o.x - self.x) * t,
            y: self.y + (o.y - self.y) * t,
        }
    }
    /// Straight-line distance.
    pub fn dist(self, o: Vec2) -> f64 {
        (o.x - self.x).hypot(o.y - self.y)
    }
}

/// A piece of cursor motion.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CursorSegment {
    /// Curved move from `from` to `to` (cubic Bézier with `c1`, `c2`).
    Move {
        /// Start time.
        start_ms: f64,
        /// End time.
        end_ms: f64,
        /// Start point.
        from: Vec2,
        /// First control point.
        c1: Vec2,
        /// Second control point.
        c2: Vec2,
        /// End point.
        to: Vec2,
        /// Which action, e.g. `"hero#0 hover"`.
        label: String,
    },
    /// Cursor pinned to the page while it scrolls (`cursor: "follow"`).
    Follow {
        /// Start time.
        start_ms: f64,
        /// End time.
        end_ms: f64,
        /// Position at the start.
        from: Vec2,
        /// Position at the end.
        to: Vec2,
        /// Scroll position at the start.
        scroll_from: f64,
    },
}

impl CursorSegment {
    fn span(&self) -> (f64, f64) {
        match self {
            CursorSegment::Move {
                start_ms, end_ms, ..
            }
            | CursorSegment::Follow {
                start_ms, end_ms, ..
            } => (*start_ms, *end_ms),
        }
    }
    fn end_pos(&self) -> Vec2 {
        match self {
            CursorSegment::Move { to, .. } | CursorSegment::Follow { to, .. } => *to,
        }
    }
}

/// A click (desktop) or tap (touch).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ClickMark {
    /// Time of mouse-down / touch-start.
    pub at_ms: f64,
    /// Where.
    pub pos: Vec2,
    /// Send real events to the page.
    pub real: bool,
    /// Touch tap instead of mouse click.
    pub touch: bool,
    /// Which action.
    pub label: String,
}

/// A time window.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Window {
    /// Start.
    pub start_ms: f64,
    /// End.
    pub end_ms: f64,
}

impl Window {
    fn contains(&self, t: f64) -> bool {
        t >= self.start_ms && t <= self.end_ms
    }
}

/// The planned cursor for a whole video.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CursorTrack {
    /// Position at video start.
    pub start: Vec2,
    /// Whether the arrow is drawn at all (false on touch devices or when
    /// `cursor.visible` is false).
    pub visible: bool,
    /// Touch device.
    pub touch: bool,
    /// Moves, in time order, non-overlapping.
    pub segments: Vec<CursorSegment>,
    /// Clicks and taps.
    pub clicks: Vec<ClickMark>,
    /// Windows where the cursor fades out (scrolling with `cursor: "hide"`).
    pub hidden: Vec<Window>,
    /// Windows where the page gets no pointer events (`mode: "visual"` hovers).
    pub not_live: Vec<Window>,
}

/// Mouse or touch event to send to the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerEventKind {
    /// Button down / finger down.
    Press,
    /// Button up / finger up.
    Release,
}

/// An event due on a frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointerEvent {
    /// Down or up.
    pub kind: PointerEventKind,
    /// Where (viewport CSS px).
    pub pos: Vec2,
    /// Touch instead of mouse.
    pub touch: bool,
}

/// A click that is still animating on a frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActiveClick {
    /// Where.
    pub pos: Vec2,
    /// ms since mouse-down (negative = finger about to touch).
    pub age_ms: f64,
    /// Touch tap.
    pub touch: bool,
}

/// Everything about the cursor on one frame.
#[derive(Debug, Clone, PartialEq)]
pub struct CursorFrame {
    /// Position (viewport CSS px).
    pub pos: Vec2,
    /// 0 = invisible, 1 = fully visible.
    pub opacity: f64,
    /// The page should receive the pointer position on this frame.
    pub live: bool,
    /// Events to send before capturing this frame.
    pub events: Vec<PointerEvent>,
    /// Clicks/taps still animating.
    pub clicks: Vec<ActiveClick>,
}

// ── Evaluation ──────────────────────────────────────────────────────────────

impl CursorTrack {
    /// Cursor position at time `t` (needs the scroll position for `follow`).
    pub fn position_at(&self, t: f64, scroll_at: &dyn Fn(f64) -> f64) -> Vec2 {
        let mut pos = self.start;
        for seg in &self.segments {
            let (start, end) = seg.span();
            if t < start {
                break;
            }
            if t >= end {
                pos = seg.end_pos();
                continue;
            }
            let p = ((t - start) / (end - start).max(f64::EPSILON)).clamp(0.0, 1.0);
            return match seg {
                CursorSegment::Move {
                    from, c1, c2, to, ..
                } => bezier(*from, *c1, *c2, *to, move_easing().apply(p)),
                CursorSegment::Follow {
                    from,
                    scroll_from,
                    to,
                    ..
                } => {
                    let y = from.y - (scroll_at(t) - scroll_from);
                    Vec2 {
                        x: from.x,
                        y: y.clamp(to.y.min(from.y), to.y.max(from.y)),
                    }
                }
            };
        }
        pos
    }

    /// The cursor on the frame at time `t`; `dt` is the frame interval.
    pub fn frame(&self, t: f64, dt: f64, scroll_at: &dyn Fn(f64) -> f64) -> CursorFrame {
        let pos = self.position_at(t, scroll_at);
        let mut opacity: f64 = if self.visible { 1.0 } else { 0.0 };
        for w in &self.hidden {
            if w.contains(t) {
                let fade = (1.0 - (t - w.start_ms) / FADE_MS).max(1.0 - (w.end_ms - t) / FADE_MS);
                opacity = opacity.min(fade.clamp(0.0, 1.0));
            }
        }
        let live = !self.touch && !self.not_live.iter().any(|w| w.contains(t));

        // Events that happened since the previous frame are sent now, so
        // each one is sent exactly once whatever the frame rate.
        let due = |te: f64| te <= t && te > t - dt;
        let mut events = Vec::new();
        let mut clicks = Vec::new();
        for c in &self.clicks {
            if c.real && due(c.at_ms) {
                events.push(PointerEvent {
                    kind: PointerEventKind::Press,
                    pos: c.pos,
                    touch: c.touch,
                });
            }
            if c.real && due(c.at_ms + PRESS_HOLD_MS) {
                events.push(PointerEvent {
                    kind: PointerEventKind::Release,
                    pos: c.pos,
                    touch: c.touch,
                });
            }
            let age = t - c.at_ms;
            let lead = if c.touch { TAP_LEAD_MS } else { 0.0 };
            if age >= -lead && age < CLICK_VISIBLE_MS {
                clicks.push(ActiveClick {
                    pos: c.pos,
                    age_ms: age,
                    touch: c.touch,
                });
            }
        }
        CursorFrame {
            pos,
            opacity,
            live,
            events,
            clicks,
        }
    }
}

/// Speed profile of a move: quick start, long soft landing — like a hand.
fn move_easing() -> Easing {
    Easing::CubicBezier([0.3, 0.0, 0.2, 1.0])
}

fn bezier(p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2, t: f64) -> Vec2 {
    let a = p0.lerp(p1, t);
    let b = p1.lerp(p2, t);
    let c = p2.lerp(p3, t);
    a.lerp(b, t).lerp(b.lerp(c, t), t)
}

// ── Seeded randomness ───────────────────────────────────────────────────────
// Why: natural-looking variation that is identical on every render.

/// SplitMix64 — tiny, well-known generator; enough for jitter and curves.
struct Rng(u64);

impl Rng {
    fn new(seed: u64, salt: &str) -> Rng {
        // FNV-1a of the salt mixes in which action this is.
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in salt.bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
        Rng(seed ^ h)
    }
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^= z >> 31;
        (z >> 11) as f64 / (1u64 << 53) as f64
    }
    /// Uniform in `[-1, 1]`.
    fn signed(&mut self) -> f64 {
        self.next() * 2.0 - 1.0
    }
}

// ── Building ────────────────────────────────────────────────────────────────

/// Builds a [`CursorTrack`] step by step while the scroll timeline is built.
pub(crate) struct CursorBuilder {
    pos: Vec2,
    vw: f64,
    vh: f64,
    settings: CursorSettings,
    track: CursorTrack,
    pub(crate) warnings: Vec<String>,
}

impl CursorBuilder {
    pub(crate) fn new(plan: &sr_core::FlowPlan, metrics: &PageMetrics) -> CursorBuilder {
        let (vw, vh) = (metrics.viewport_width, metrics.viewport_height);
        let s = plan.cursor.clone();
        let start = Vec2 {
            x: s.start.x * vw,
            y: s.start.y * vh,
        };
        CursorBuilder {
            pos: start,
            vw,
            vh,
            track: CursorTrack {
                start,
                visible: s.visible && !plan.touch,
                touch: plan.touch,
                segments: Vec::new(),
                clicks: Vec::new(),
                hidden: Vec::new(),
                not_live: Vec::new(),
            },
            settings: s,
            warnings: Vec::new(),
        }
    }

    pub(crate) fn finish(self) -> CursorTrack {
        self.track
    }

    /// The page scrolls from `from_y` to `to_y` during `[start, end]`.
    pub(crate) fn on_scroll(
        &mut self,
        start: f64,
        end: f64,
        from_y: f64,
        to_y: f64,
        mode: ScrollCursor,
    ) {
        match mode {
            ScrollCursor::Still => {}
            ScrollCursor::Hide => self.track.hidden.push(Window {
                start_ms: start,
                end_ms: end,
            }),
            ScrollCursor::Follow => {
                let from = self.pos;
                let to = Vec2 {
                    x: from.x,
                    y: (from.y - (to_y - from_y)).clamp(EDGE_MARGIN, self.vh - EDGE_MARGIN),
                };
                self.track.segments.push(CursorSegment::Follow {
                    start_ms: start,
                    end_ms: end,
                    from,
                    to,
                    scroll_from: from_y,
                });
                self.pos = to;
            }
        }
    }

    fn clamp_in_view(&self, p: Vec2) -> Vec2 {
        Vec2 {
            x: p.x
                .clamp(EDGE_MARGIN, (self.vw - EDGE_MARGIN).max(EDGE_MARGIN)),
            y: p.y
                .clamp(EDGE_MARGIN, (self.vh - EDGE_MARGIN).max(EDGE_MARGIN)),
        }
    }

    // ── Human-like move ─────────────────────────────────────────────────────
    // Duration follows Fitts's law (far or small targets take longer); the
    // path bends slightly to one side, as a wrist does.
    fn move_to(&mut self, t: f64, to: Vec2, target_size: f64, rng: &mut Rng, label: &str) -> f64 {
        let from = self.pos;
        let d = from.dist(to);
        if d < 1.0 {
            return t;
        }
        let fitts = MOVE_BASE_MS + MOVE_LOG_MS * (1.0 + d / target_size.max(24.0)).log2();
        let ms = fitts.clamp(MOVE_MIN_MS, MOVE_MAX_MS) / self.settings.speed;
        let dir = Vec2 {
            x: (to.x - from.x) / d,
            y: (to.y - from.y) / d,
        };
        let normal = Vec2 {
            x: -dir.y,
            y: dir.x,
        };

        // ── Long moves sometimes fly a few pixels past and come back (FR-CUR-02) ──
        let overshoot = d > OVERSHOOT_MIN_PX && rng.next() < OVERSHOOT_CHANCE;
        let land = if overshoot {
            let past = (d * 0.035).min(12.0);
            let side = rng.signed() * 3.0;
            self.clamp_in_view(Vec2 {
                x: to.x + dir.x * past + normal.x * side,
                y: to.y + dir.y * past + normal.y * side,
            })
        } else {
            to
        };

        let bend = (0.03 + 0.07 * rng.next()) * if rng.next() < 0.5 { -1.0 } else { 1.0 } * d;
        let curve = |a: Vec2, b: Vec2, bend: f64| {
            let c1 = a.lerp(b, 0.3);
            let c2 = a.lerp(b, 0.75);
            (
                Vec2 {
                    x: c1.x + normal.x * bend,
                    y: c1.y + normal.y * bend,
                },
                Vec2 {
                    x: c2.x + normal.x * bend * 0.5,
                    y: c2.y + normal.y * bend * 0.5,
                },
            )
        };
        let (c1, c2) = curve(from, land, bend);
        self.track.segments.push(CursorSegment::Move {
            start_ms: t,
            end_ms: t + ms,
            from,
            c1,
            c2,
            to: land,
            label: label.to_owned(),
        });
        let mut end = t + ms;
        if overshoot {
            let fix_ms = (120.0 + 60.0 * rng.next()) / self.settings.speed;
            let (c1, c2) = curve(land, to, 0.0);
            self.track.segments.push(CursorSegment::Move {
                start_ms: end,
                end_ms: end + fix_ms,
                from: land,
                c1,
                c2,
                to,
                label: format!("{label} (correction)"),
            });
            end += fix_ms;
        }
        self.pos = to;
        end
    }

    /// Plans the actions of a block stop starting at `t0` with the page at
    /// `scroll_y`. Returns how long they take.
    pub(crate) fn run_actions(
        &mut self,
        t0: f64,
        scroll_y: f64,
        block: &PlanBlock,
        metrics: &PageMetrics,
    ) -> f64 {
        // The hand reacts a moment after the page stops (waits don't move it).
        let moves = block
            .actions
            .iter()
            .any(|a| a.action.kind != ActionKind::Wait);
        let mut t = t0
            + if moves && !self.track.touch {
                REACTION_MS
            } else {
                0.0
            };
        for a in &block.actions {
            t += f64::from(a.action.delay_ms);
            let label = format!("{}#{} {}", block.id, a.index, a.action.kind_name());
            let mut rng = Rng::new(self.settings.seed, &label);
            match a.action.kind {
                ActionKind::Wait => t += f64::from(a.action.duration_ms.unwrap_or(0)),
                ActionKind::Hover => {
                    let (p, size) = self.target_point(block, a, scroll_y, metrics, &mut rng);
                    let start = t;
                    t = self.move_to(t, p, size, &mut rng, &label);
                    t += f64::from(
                        a.action
                            .duration_ms
                            .unwrap_or(sr_core::flow::Action::HOVER_MS),
                    );
                    if a.action.effective_mode() == PointerMode::Visual {
                        self.track.not_live.push(Window {
                            start_ms: start,
                            end_ms: t,
                        });
                    }
                }
                ActionKind::Move => {
                    let (p, size) = self.target_point(block, a, scroll_y, metrics, &mut rng);
                    t = self.move_to(t, p, size, &mut rng, &label);
                }
                ActionKind::Click => {
                    let (p, size) = self.target_point(block, a, scroll_y, metrics, &mut rng);
                    let real = a.action.effective_mode() == PointerMode::Real;
                    if self.track.touch {
                        // A finger doesn't travel visibly: it appears, taps, lifts.
                        t += TAP_LEAD_MS;
                        self.pos = p;
                    } else {
                        t = self.move_to(t, p, size, &mut rng, &label);
                        t += CLICK_SETTLE_MS;
                    }
                    self.track.clicks.push(ClickMark {
                        at_ms: t,
                        pos: p,
                        real,
                        touch: self.track.touch,
                        label,
                    });
                    t += PRESS_HOLD_MS + CLICK_AFTER_MS;
                }
            }
        }
        t - t0
    }

    /// Where to aim for an action: element centre (with a little seeded
    /// jitter, people never hit the exact centre) or a point of the block.
    fn target_point(
        &mut self,
        block: &PlanBlock,
        a: &PlanAction,
        scroll_y: f64,
        m: &PageMetrics,
        rng: &mut Rng,
    ) -> (Vec2, f64) {
        let id = PlanAction::measure_id(&block.id, a.index);
        let (raw, size) = if let (Some(to), Some(b)) = (a.action.to, m.blocks.get(&block.id)) {
            (
                Vec2 {
                    x: b.left + to.x * b.width,
                    y: b.top + to.y * b.height - scroll_y,
                },
                24.0,
            )
        } else if let Some(el) = m.blocks.get(&id) {
            let jx = rng.signed() * (el.width * 0.15).min(6.0);
            let jy = rng.signed() * (el.height * 0.15).min(4.0);
            (
                Vec2 {
                    x: el.left + el.width / 2.0 + jx,
                    y: el.top + el.height / 2.0 - scroll_y + jy,
                },
                el.width.min(el.height).max(1.0),
            )
        } else {
            self.warnings.push(format!(
                "block '{}': action {} target was not measured",
                block.id, a.index
            ));
            return (self.pos, 24.0);
        };
        let p = self.clamp_in_view(raw);
        if p.dist(raw) > 2.0 {
            self.warnings.push(format!(
                "block '{}': actions[{}] target is outside the visible area at this stop; the cursor goes to the edge",
                block.id, a.index
            ));
        }
        (p, size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track_with(segments: Vec<CursorSegment>, clicks: Vec<ClickMark>) -> CursorTrack {
        CursorTrack {
            start: Vec2 { x: 100.0, y: 100.0 },
            visible: true,
            touch: false,
            segments,
            clicks,
            hidden: vec![Window {
                start_ms: 2000.0,
                end_ms: 2600.0,
            }],
            not_live: vec![Window {
                start_ms: 3000.0,
                end_ms: 3100.0,
            }],
        }
    }

    #[test]
    fn rng_is_deterministic_and_salted() {
        let a: Vec<f64> = (0..3).map(|_| Rng::new(7, "x").next()).collect();
        assert_eq!(a[0], a[1]);
        let mut r1 = Rng::new(7, "hero#0");
        let mut r2 = Rng::new(7, "hero#1");
        assert_ne!(r1.next(), r2.next());
        let mut r = Rng::new(1, "s");
        assert!(
            (0..1000)
                .map(|_| r.signed())
                .all(|v| (-1.0..=1.0).contains(&v))
        );
    }

    #[test]
    fn moves_follow_the_curve_and_land_exactly() {
        let from = Vec2 { x: 100.0, y: 100.0 };
        let to = Vec2 { x: 500.0, y: 300.0 };
        let seg = CursorSegment::Move {
            start_ms: 0.0,
            end_ms: 500.0,
            from,
            c1: Vec2 { x: 200.0, y: 120.0 },
            c2: Vec2 { x: 400.0, y: 280.0 },
            to,
            label: "m".into(),
        };
        let track = track_with(vec![seg], vec![]);
        let scroll = |_t: f64| 0.0;
        assert_eq!(track.position_at(0.0, &scroll), from);
        assert_eq!(track.position_at(500.0, &scroll), to);
        assert_eq!(
            track.position_at(9000.0, &scroll),
            to,
            "rests after arriving"
        );
        let mid = track.position_at(250.0, &scroll);
        assert!(
            mid.x > 300.0,
            "quick start, soft landing: past halfway at half time ({mid:?})"
        );
    }

    #[test]
    fn clicks_produce_events_once_and_animate() {
        let click = ClickMark {
            at_ms: 1000.0,
            pos: Vec2 { x: 50.0, y: 60.0 },
            real: true,
            touch: false,
            label: "c".into(),
        };
        let track = track_with(vec![], vec![click]);
        let scroll = |_t: f64| 0.0;
        let dt = 1000.0 / 60.0;
        let mut presses = 0;
        let mut releases = 0;
        for i in 0..200 {
            let f = track.frame(f64::from(i) * dt, dt, &scroll);
            presses += f
                .events
                .iter()
                .filter(|e| e.kind == PointerEventKind::Press)
                .count();
            releases += f
                .events
                .iter()
                .filter(|e| e.kind == PointerEventKind::Release)
                .count();
        }
        assert_eq!((presses, releases), (1, 1), "exactly one down and one up");
        let f = track.frame(1100.0, dt, &scroll);
        assert_eq!(f.clicks.len(), 1);
        assert!((f.clicks[0].age_ms - 100.0).abs() < 1e-9);
    }

    #[test]
    fn hide_windows_fade_and_visual_windows_are_not_live() {
        let track = track_with(vec![], vec![]);
        let scroll = |_t: f64| 0.0;
        let dt = 16.0;
        assert_eq!(track.frame(1999.0, dt, &scroll).opacity, 1.0);
        assert_eq!(track.frame(2300.0, dt, &scroll).opacity, 0.0);
        let fading = track.frame(2075.0, dt, &scroll).opacity;
        assert!((fading - 0.5).abs() < 1e-9, "{fading}");
        assert!(!track.frame(3050.0, dt, &scroll).live);
        assert!(track.frame(3200.0, dt, &scroll).live);
    }
}
