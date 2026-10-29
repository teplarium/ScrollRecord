//! # Cursor settings (HANDOFF §5.8)
//!
//! Purpose: how the visual cursor looks and behaves for a whole scenario:
//!          which cursor pack, size, starting point, speed, and the random
//!          seed that shapes its "human" curves.
//! Solves:  the same `seed` always gives the same curves, so videos stay
//!          reproducible while the movement still looks natural.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::Point;

/// Built-in cursor pack used when nothing else is set.
pub const DEFAULT_CURSOR_PACK: &str = "classic-dark";

/// Scenario-wide cursor settings (`defaults.cursor`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(deny_unknown_fields)]
pub struct CursorSettings {
    /// Built-in pack id (`classic-dark`, `classic-light`) or a folder with a
    /// `cursor-pack.json`, relative to the scenario file.
    #[serde(default = "default_pack")]
    pub pack: String,
    /// Size multiplier (1 = normal system size).
    #[serde(default = "one")]
    pub scale: f64,
    /// Shapes the random-looking curves. Same seed → same movement.
    #[serde(default = "default_seed")]
    pub seed: u64,
    /// Draw the cursor. `false` still sends real hover/click events (useful
    /// for recording hover animations without a visible cursor).
    #[serde(default = "yes")]
    pub visible: bool,
    /// Where the cursor sits when the video starts (fractions of the frame).
    #[serde(default = "default_start")]
    pub start: Point,
    /// Movement speed multiplier (2 = twice as fast).
    #[serde(default = "one")]
    pub speed: f64,
}

fn default_pack() -> String {
    DEFAULT_CURSOR_PACK.to_owned()
}
fn one() -> f64 {
    1.0
}
fn default_seed() -> u64 {
    1
}
fn yes() -> bool {
    true
}
fn default_start() -> Point {
    Point { x: 0.68, y: 0.62 }
}

impl Default for CursorSettings {
    fn default() -> Self {
        Self {
            pack: default_pack(),
            scale: 1.0,
            seed: default_seed(),
            visible: true,
            start: default_start(),
            speed: 1.0,
        }
    }
}

impl CursorSettings {
    /// Checks value ranges.
    pub fn check(&self) -> Result<(), String> {
        if !(0.25..=4.0).contains(&self.scale) {
            return Err(format!("cursor.scale must be 0.25..4, got {}", self.scale));
        }
        if !(0.1..=10.0).contains(&self.speed) {
            return Err(format!("cursor.speed must be 0.1..10, got {}", self.speed));
        }
        if !(0.0..=1.0).contains(&self.start.x) || !(0.0..=1.0).contains(&self.start.y) {
            return Err("cursor.start x and y must be between 0 and 1".into());
        }
        if self.pack.trim().is_empty() {
            return Err("cursor.pack is empty".into());
        }
        Ok(())
    }
}

/// What the cursor does while the page scrolls along an arrow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum ScrollCursor {
    /// Stays where it is on screen (like a hand resting on the mouse).
    #[default]
    Still,
    /// Moves with the content, as if pinned to the page.
    Follow,
    /// Fades out while scrolling and back in at the stop.
    Hide,
}

/// The cursor image to show, decided by the page under the pointer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum CursorKind {
    /// Normal arrow.
    Arrow,
    /// Hand over links and buttons (CSS `cursor: pointer`).
    Pointer,
    /// I-beam over text.
    Text,
    /// Finger touch indicator (touch devices).
    Touch,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_ranges() {
        let s: CursorSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(s, CursorSettings::default());
        assert!(s.check().is_ok());
        let big: CursorSettings = serde_json::from_str(r#"{ "scale": 9 }"#).unwrap();
        assert!(big.check().is_err());
        let mode: ScrollCursor = serde_json::from_str(r#""hide""#).unwrap();
        assert_eq!(mode, ScrollCursor::Hide);
    }
}
