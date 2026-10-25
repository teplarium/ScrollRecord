//! # `cursor-pack.json` — the pack description (HANDOFF §8.3)
//!
//! Purpose: which SVG belongs to which cursor state, where its tip is, how
//!          big it is, how it reacts to a click, and the effects around it.
//! Solves:  one small JSON file that people, the future Cursor Studio and AI
//!          models can all write. Unknown fields are errors, so a typo like
//!          `"hotpsot"` is reported instead of silently ignored.
//! Limits:  `animations.hover`, `animations.idle` and `effects.trail` are
//!          accepted (so packs written for later versions still load) but not
//!          drawn yet; loading reports a warning.

use serde::Deserialize;
use serde_json::Value;
use sr_core::Easing;

/// The only pack format version this build understands.
pub const PACK_FORMAT_VERSION: u32 = 1;

/// Whole `cursor-pack.json`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// Must be 1.
    pub format_version: u32,
    /// Short id, e.g. `neon`.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Who made it.
    #[serde(default)]
    pub author: Option<String>,
    /// Licence of the drawings, e.g. `CC0-1.0`.
    #[serde(default)]
    pub license: Option<String>,
    /// One-line description.
    #[serde(default)]
    pub description: Option<String>,
    /// Optional preview image (shown by the future Cursor Studio).
    #[serde(default)]
    pub preview: Option<String>,
    /// Cursor images.
    pub states: States,
    /// Movement of the cursor itself (click squash…).
    #[serde(default)]
    pub animations: Animations,
    /// Things drawn around the cursor (ripple, shadow).
    #[serde(default)]
    pub effects: Effects,
}

/// One image per cursor state. Only `arrow` is required: missing states
/// fall back to it (and `touch` to the built-in finger circle).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct States {
    /// Normal arrow.
    pub arrow: State,
    /// Hand over links and buttons.
    #[serde(default)]
    pub pointer: Option<State>,
    /// I-beam over text.
    #[serde(default)]
    pub text: Option<State>,
    /// Finger indicator on touch devices.
    #[serde(default)]
    pub touch: Option<State>,
}

/// One cursor image.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct State {
    /// SVG file inside the pack folder.
    pub svg: String,
    /// The pixel that "clicks" (arrow tip, finger tip), in the cursor's own
    /// pixels: 0..size from the top-left corner.
    pub hotspot: [f64; 2],
    /// Size in CSS pixels at `cursor.scale: 1` (the longer side of the SVG).
    #[serde(default = "default_size")]
    pub size: f64,
}

fn default_size() -> f64 {
    32.0
}

/// Cursor animations.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Animations {
    /// What the cursor does when it clicks.
    #[serde(default)]
    pub click: Option<ClickAnimation>,
    /// Reserved (MVP+): reaction when resting on an element.
    #[serde(default)]
    pub hover: Option<Value>,
    /// Reserved (MVP+): frame-by-frame idle animation.
    #[serde(default)]
    pub idle: Option<Value>,
}

/// Keyframes played from the moment the button goes down.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClickAnimation {
    /// At least one keyframe, `t` ascending.
    pub keyframes: Vec<Keyframe>,
    /// Curve between keyframes (CSS names or `cubic-bezier(...)`).
    #[serde(default)]
    pub easing: Option<Easing>,
    /// Effect started with the click: `"ripple"` or none.
    #[serde(default)]
    pub effect: Option<String>,
}

/// Pose of the cursor at time `t` (ms after the press). Missing values
/// keep the neutral pose (scale 1, no rotation, fully visible, no shift).
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Keyframe {
    /// Time in ms after the press.
    pub t: f64,
    /// Size multiplier.
    #[serde(default)]
    pub scale: Option<f64>,
    /// Rotation in degrees (clockwise), around the hotspot.
    #[serde(default)]
    pub rotate: Option<f64>,
    /// 0..1.
    #[serde(default)]
    pub opacity: Option<f64>,
    /// Shift in CSS px.
    #[serde(default)]
    pub offset: Option<[f64; 2]>,
}

/// Effects drawn around the cursor.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Effects {
    /// Expanding ring where a click or tap happens.
    #[serde(default)]
    pub ripple: Option<Ripple>,
    /// Soft shadow under the cursor.
    #[serde(default)]
    pub shadow: Option<Shadow>,
    /// Reserved (MVP+): motion trail.
    #[serde(default)]
    pub trail: Option<Value>,
}

/// Click ripple.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ripple {
    /// `#RRGGBB` or `#RRGGBBAA`.
    pub color: String,
    /// Final radius in CSS px.
    pub radius: f64,
    /// How long it lasts.
    pub duration_ms: f64,
}

/// Drop shadow.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Shadow {
    /// Blur radius in CSS px (like CSS `drop-shadow`).
    pub blur: f64,
    /// 0..1.
    pub opacity: f64,
    /// Shift in CSS px (default: 1px down).
    #[serde(default = "default_shadow_offset")]
    pub offset: [f64; 2],
}

fn default_shadow_offset() -> [f64; 2] {
    [0.0, 1.0]
}

/// A colour as straight (not premultiplied) RGBA bytes.
pub type Rgba = [u8; 4];

/// Parses `#RGB`, `#RRGGBB` or `#RRGGBBAA`.
pub fn parse_color(s: &str) -> Option<Rgba> {
    let hex = s.strip_prefix('#')?;
    let byte = |i: usize| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok();
    let nibble = |i: usize| u8::from_str_radix(hex.get(i..=i)?, 16).ok().map(|v| v * 17);
    match hex.len() {
        3 => Some([nibble(0)?, nibble(1)?, nibble(2)?, 255]),
        6 => Some([byte(0)?, byte(2)?, byte(4)?, 255]),
        8 => Some([byte(0)?, byte(2)?, byte(4)?, byte(6)?]),
        _ => None,
    }
}

impl Manifest {
    /// Checks values that JSON types alone can't. Returns the problems
    /// (empty = fine) and warnings about features not drawn yet.
    pub fn check(&self) -> (Vec<String>, Vec<String>) {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        if self.format_version != PACK_FORMAT_VERSION {
            errors.push(format!(
                "format_version {} is not supported (this ScrollRecord reads {PACK_FORMAT_VERSION})",
                self.format_version
            ));
        }
        let states = [
            ("arrow", Some(&self.states.arrow)),
            ("pointer", self.states.pointer.as_ref()),
            ("text", self.states.text.as_ref()),
            ("touch", self.states.touch.as_ref()),
        ];
        for (name, state) in states {
            let Some(s) = state else { continue };
            if !(8.0..=256.0).contains(&s.size) {
                errors.push(format!("states.{name}.size must be 8..256, got {}", s.size));
            }
            if s.hotspot.iter().any(|v| !(0.0..=s.size).contains(v)) {
                errors.push(format!(
                    "states.{name}.hotspot must be inside 0..size ({})",
                    s.size
                ));
            }
            if !safe_file_name(&s.svg) {
                errors.push(format!(
                    "states.{name}.svg '{}' must be a file inside the pack folder",
                    s.svg
                ));
            }
        }
        if let Some(click) = &self.animations.click {
            if click.keyframes.is_empty() {
                errors.push("animations.click.keyframes is empty".into());
            }
            if click.keyframes.windows(2).any(|w| w[1].t < w[0].t) {
                errors.push("animations.click.keyframes must be sorted by t".into());
            }
            match click.effect.as_deref() {
                None | Some("ripple") => {}
                Some(other) => errors.push(format!(
                    "animations.click.effect '{other}' is unknown (use \"ripple\")"
                )),
            }
        }
        if let Some(r) = &self.effects.ripple {
            if parse_color(&r.color).is_none() {
                errors.push(format!(
                    "effects.ripple.color '{}' is not #RRGGBB or #RRGGBBAA",
                    r.color
                ));
            }
            if !(r.radius > 0.0 && r.duration_ms > 0.0) {
                errors.push("effects.ripple radius and duration_ms must be positive".into());
            }
        }
        if let Some(s) = &self.effects.shadow
            && (s.blur < 0.0 || !(0.0..=1.0).contains(&s.opacity))
        {
            errors.push("effects.shadow: blur must be ≥ 0 and opacity 0..1".into());
        }
        for (field, value) in [
            ("animations.hover", &self.animations.hover),
            ("animations.idle", &self.animations.idle),
            ("effects.trail", &self.effects.trail),
        ] {
            if value.as_ref().is_some_and(|v| !v.is_null()) {
                warnings.push(format!(
                    "cursor pack '{}': {field} is not supported yet and is ignored",
                    self.id
                ));
            }
        }
        (errors, warnings)
    }
}

/// A plain relative path without `..`, so a pack can't read files
/// outside its own folder.
fn safe_file_name(name: &str) -> bool {
    let path = std::path::Path::new(name);
    !name.is_empty()
        && path.is_relative()
        && path
            .components()
            .all(|c| matches!(c, std::path::Component::Normal(_)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(extra_state: &str) -> String {
        format!(
            r#"{{ "format_version": 1, "id": "t", "name": "T",
                "states": {{ "arrow": {{ "svg": "a.svg", "hotspot": [3, 2] }}{extra_state} }} }}"#
        )
    }

    #[test]
    fn minimal_pack_and_defaults() {
        let m: Manifest = serde_json::from_str(&manifest("")).unwrap();
        assert_eq!(m.states.arrow.size, 32.0);
        assert!(m.states.pointer.is_none());
        assert_eq!(m.check(), (vec![], vec![]));
    }

    #[test]
    fn typos_and_bad_values_are_reported() {
        let typo = r#"{ "format_version": 1, "id": "t", "name": "T",
            "states": { "arrow": { "svg": "a.svg", "hotpsot": [3, 2] } } }"#;
        assert!(serde_json::from_str::<Manifest>(typo).is_err());
        let m: Manifest = serde_json::from_str(&manifest(
            r#", "touch": { "svg": "../../secret.svg", "hotspot": [99, 1], "size": 48 }"#,
        ))
        .unwrap();
        let (errors, _) = m.check();
        assert_eq!(errors.len(), 2, "{errors:?}");
        assert!(errors[0].contains("hotspot") && errors[1].contains("inside the pack folder"));
    }

    #[test]
    fn later_features_only_warn() {
        let json = r##"{ "format_version": 1, "id": "t", "name": "T",
            "states": { "arrow": { "svg": "a.svg", "hotspot": [3, 2] } },
            "animations": { "idle": { "frames": ["a.svg"], "fps": 12 } },
            "effects": { "ripple": { "color": "#FFFFFF80", "radius": 28, "duration_ms": 400 }, "trail": null } }"##;
        let m: Manifest = serde_json::from_str(json).unwrap();
        let (errors, warnings) = m.check();
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(warnings.len(), 1, "only idle (trail is null)");
    }

    #[test]
    fn colors() {
        assert_eq!(parse_color("#FFFFFF80"), Some([255, 255, 255, 128]));
        assert_eq!(parse_color("#102030"), Some([16, 32, 48, 255]));
        assert_eq!(parse_color("#fff"), Some([255, 255, 255, 255]));
        assert_eq!(parse_color("red"), None);
        assert_eq!(parse_color("#12345"), None);
    }
}
