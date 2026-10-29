//! # Actions — what the cursor does inside a block (HANDOFF FR-BLK-02, FR-CUR-05)
//!
//! Purpose: "hover the Get started button", "click Choose Pro", "move to the
//!          right side", "wait half a second" — written as small JSON objects
//!          in a block's `actions` list.
//! Solves:  one flat shape for every action (`"do": "hover"` plus optional
//!          fields) is easy for people and AI models to write. `check()`
//!          explains exactly which field is missing or doesn't fit.
//! Rules:   `mode: "real"` sends real mouse/touch events, so the site reacts
//!          (hover styles, menus) — and a real click can submit forms or
//!          follow links. That's why clicks default to `visual` (no press).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::Locator;

/// What an action does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum ActionKind {
    /// Move to an element and rest on it (hover styles appear).
    Hover,
    /// Move to an element and click it.
    Click,
    /// Move to an element or a point of the block, without resting.
    Move,
    /// Do nothing for `duration_ms`.
    Wait,
}

/// Whether the site receives the pointer events (HANDOFF D-25).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum PointerMode {
    /// Real events: the site reacts like for a visitor.
    Real,
    /// Shown in the video only. A visual hover takes the mouse off the
    /// page, so nothing reacts. A visual click still lets the page see the
    /// pointer arrive (the button lights up) but never sends the press,
    /// so nothing is clicked, submitted or opened.
    Visual,
}

/// A point inside a block: fractions of its width and height (0..1).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(deny_unknown_fields)]
pub struct Point {
    /// 0 = left edge, 1 = right edge.
    pub x: f64,
    /// 0 = top edge, 1 = bottom edge.
    pub y: f64,
}

/// One step inside a block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(deny_unknown_fields)]
pub struct Action {
    /// `hover`, `click`, `move` or `wait`.
    #[serde(rename = "do")]
    pub kind: ActionKind,
    /// Element to go to (hover, click, move). Searched inside the block first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub target: Vec<Locator>,
    /// Point inside the block (move only), instead of `target`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<Point>,
    /// hover: how long to rest (default 800 ms); wait: how long to wait.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u32>,
    /// `real` or `visual`. Default: hover real, click visual.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<PointerMode>,
    /// Pause before the action starts.
    #[serde(default)]
    pub delay_ms: u32,
    /// Run only on these targets / viewport presets (empty = everywhere).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub only: Vec<String>,
    /// Skip on these targets / viewport presets.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub except: Vec<String>,
}

impl Action {
    /// Default rest time of a hover.
    pub const HOVER_MS: u32 = 800;

    /// Real or visual, with the safe defaults applied.
    pub fn effective_mode(&self) -> PointerMode {
        self.mode.unwrap_or(match self.kind {
            ActionKind::Click => PointerMode::Visual,
            _ => PointerMode::Real,
        })
    }

    /// Does this action run for a target with id `target_id` and viewport
    /// preset `preset` (if any)?
    pub fn applies_to(&self, target_id: &str, preset: Option<&str>) -> bool {
        let matches = |list: &[String]| {
            list.iter()
                .any(|k| k == target_id || Some(k.as_str()) == preset)
        };
        (self.only.is_empty() || matches(&self.only)) && !matches(&self.except)
    }

    /// Checks that the fields fit the action kind.
    pub fn check(&self) -> Result<(), String> {
        let needs_target = matches!(self.kind, ActionKind::Hover | ActionKind::Click);
        if needs_target && self.target.is_empty() {
            return Err(format!("'{}' needs a target", self.kind_name()));
        }
        match self.kind {
            ActionKind::Move => {
                if self.target.is_empty() == self.to.is_none() {
                    return Err("'move' needs exactly one of target or to".into());
                }
            }
            ActionKind::Wait => {
                if self.duration_ms.is_none() {
                    return Err("'wait' needs duration_ms".into());
                }
                if !self.target.is_empty() || self.to.is_some() {
                    return Err("'wait' takes no target or to".into());
                }
            }
            ActionKind::Hover | ActionKind::Click => {
                if self.to.is_some() {
                    return Err(format!("'{}' uses target, not to", self.kind_name()));
                }
            }
        }
        if self.mode.is_some() && !matches!(self.kind, ActionKind::Hover | ActionKind::Click) {
            return Err(format!(
                "'mode' only applies to hover and click, not '{}'",
                self.kind_name()
            ));
        }
        if let Some(p) = self.to
            && (!(0.0..=1.0).contains(&p.x) || !(0.0..=1.0).contains(&p.y))
        {
            return Err("'to' x and y must be between 0 and 1".into());
        }
        for (i, loc) in self.target.iter().enumerate() {
            loc.check().map_err(|e| format!("target[{i}]: {e}"))?;
        }
        Ok(())
    }

    /// `hover`, `click`, … (for messages).
    pub fn kind_name(&self) -> &'static str {
        match self.kind {
            ActionKind::Hover => "hover",
            ActionKind::Click => "click",
            ActionKind::Move => "move",
            ActionKind::Wait => "wait",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(json: &str) -> Action {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn defaults_are_safe_and_filters_work() {
        let click =
            parse(r#"{ "do": "click", "target": [{ "sr": "buy" }], "except": ["iphone-15"] }"#);
        assert_eq!(
            click.effective_mode(),
            PointerMode::Visual,
            "clicks never submit by default"
        );
        assert!(click.check().is_ok());
        assert!(click.applies_to("desktop", Some("desktop-1920")));
        assert!(!click.applies_to("mobile", Some("iphone-15")));
        let hover = parse(r#"{ "do": "hover", "target": [{ "sr": "x" }], "only": ["desktop"] }"#);
        assert_eq!(hover.effective_mode(), PointerMode::Real);
        assert!(!hover.applies_to("mobile", None));
    }

    #[test]
    fn explains_wrong_combinations() {
        assert!(
            parse(r#"{ "do": "hover" }"#)
                .check()
                .unwrap_err()
                .contains("needs a target")
        );
        assert!(
            parse(r#"{ "do": "wait" }"#)
                .check()
                .unwrap_err()
                .contains("duration_ms")
        );
        let both =
            parse(r#"{ "do": "move", "target": [{ "sr": "a" }], "to": { "x": 0.5, "y": 0.5 } }"#);
        assert!(both.check().is_err());
        let off = parse(r#"{ "do": "move", "to": { "x": 1.5, "y": 0.5 } }"#);
        assert!(off.check().unwrap_err().contains("between 0 and 1"));
        let mode = parse(r#"{ "do": "wait", "duration_ms": 5, "mode": "real" }"#);
        assert!(mode.check().unwrap_err().contains("only applies"));
        assert!(serde_json::from_str::<Action>(r#"{ "do": "hover", "targett": [] }"#).is_err());
    }
}
