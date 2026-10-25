//! # sr-cursor — cursor packs (HANDOFF §5.8, §8.3)
//!
//! Purpose: loads a cursor pack (built-in or a folder with a
//!          `cursor-pack.json` and SVGs), checks it, and draws its cursors as
//!          pixels at any size, rotation and with the pack's shadow.
//! Solves:  the cursor is drawn by ScrollRecord, not by the site, so it is
//!          sharp at any resolution and fully customisable (FR-CUR-01,
//!          FR-CUR-08/09). Packs are plain files, so people and AI models can
//!          make their own.
//! Safety:  SVGs are drawn by resvg, which never runs scripts. A pack may not
//!          read files outside its folder, and an SVG that links to an outside
//!          file is rejected (FR-CUR-11).
//! Layout:  `manifest.rs` (JSON format) → `raster.rs` (SVG → pixels) → this
//!          file (loading, fallbacks, click animation, sprite cache).

pub mod manifest;
mod raster;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

pub use raster::{Sprite, blit};
pub use resvg::tiny_skia;
use resvg::usvg;
use sr_core::flow::CursorKind;
use sr_core::{AppError, AppResult, Easing, ErrorCode};

use manifest::{Keyframe, Manifest, Rgba, Ripple, State, parse_color};

/// A built-in pack: id, `cursor-pack.json` text, and its files (name, SVG).
type BuiltIn = (
    &'static str,
    &'static str,
    &'static [(&'static str, &'static str)],
);

/// Built-in packs. Drawn for ScrollRecord, CC0.
const BUILT_IN: &[BuiltIn] = &[
    (
        "classic-dark",
        include_str!("../packs/classic-dark/cursor-pack.json"),
        &[
            ("arrow.svg", include_str!("../packs/classic-dark/arrow.svg")),
            (
                "pointer.svg",
                include_str!("../packs/classic-dark/pointer.svg"),
            ),
            ("text.svg", include_str!("../packs/classic-dark/text.svg")),
            ("touch.svg", include_str!("../packs/classic-dark/touch.svg")),
        ],
    ),
    (
        "classic-light",
        include_str!("../packs/classic-light/cursor-pack.json"),
        &[
            (
                "arrow.svg",
                include_str!("../packs/classic-light/arrow.svg"),
            ),
            (
                "pointer.svg",
                include_str!("../packs/classic-light/pointer.svg"),
            ),
            ("text.svg", include_str!("../packs/classic-light/text.svg")),
            (
                "touch.svg",
                include_str!("../packs/classic-light/touch.svg"),
            ),
        ],
    ),
];

/// Ids of the built-in packs.
pub fn built_in_ids() -> Vec<&'static str> {
    BUILT_IN.iter().map(|(id, _, _)| *id).collect()
}

/// Neutral click pose used when a pack defines no click animation.
const DEFAULT_CLICK: &[Keyframe] = &[
    Keyframe {
        t: 0.0,
        scale: Some(1.0),
        rotate: None,
        opacity: None,
        offset: None,
    },
    Keyframe {
        t: 80.0,
        scale: Some(0.85),
        rotate: None,
        opacity: None,
        offset: None,
    },
    Keyframe {
        t: 220.0,
        scale: Some(1.0),
        rotate: None,
        opacity: None,
        offset: None,
    },
];

/// The cursor's pose at one moment of a click animation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pose {
    /// Size multiplier.
    pub scale: f64,
    /// Degrees clockwise around the hotspot.
    pub rotate: f64,
    /// 0..1.
    pub opacity: f64,
    /// Shift in CSS px.
    pub offset: [f64; 2],
}

impl Pose {
    /// No change.
    pub const NEUTRAL: Pose = Pose {
        scale: 1.0,
        rotate: 0.0,
        opacity: 1.0,
        offset: [0.0, 0.0],
    };
}

/// A ripple ready to draw.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RippleStyle {
    /// Straight RGBA.
    pub color: Rgba,
    /// Final radius in CSS px.
    pub radius: f64,
    /// Duration in ms.
    pub duration_ms: f64,
}

/// A loaded, checked cursor pack.
pub struct CursorPack {
    /// The pack description.
    pub manifest: Manifest,
    /// Where it came from (`built-in:classic-dark` or a folder path).
    pub origin: String,
    /// Non-fatal notes (features not drawn yet).
    pub warnings: Vec<String>,
    trees: HashMap<CursorKind, usvg::Tree>,
    cache: HashMap<(CursorKind, i64, i64), Sprite>,
}

impl std::fmt::Debug for CursorPack {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CursorPack")
            .field("id", &self.manifest.id)
            .field("origin", &self.origin)
            .finish_non_exhaustive()
    }
}

impl CursorPack {
    // ── Loading ─────────────────────────────────────────────────────────────

    /// Loads `spec`: a built-in id, or a pack folder (relative to
    /// `base_dir`, usually the scenario's folder).
    ///
    /// Errors: `SR-6001` (no such pack), `SR-6002` (broken pack).
    pub fn load(spec: &str, base_dir: &Path) -> AppResult<CursorPack> {
        if let Some((id, manifest, files)) = BUILT_IN.iter().find(|(id, _, _)| *id == spec) {
            return Self::from_parts(manifest, &format!("built-in:{id}"), |name| {
                files
                    .iter()
                    .find(|(f, _)| *f == name)
                    .map(|(_, svg)| svg.as_bytes().to_vec())
                    .ok_or_else(|| format!("{name}: not in the built-in pack"))
            });
        }
        let dir = base_dir.join(spec);
        let not_found = |why: &str| {
            AppError::new(
                ErrorCode::CursorPackNotFound,
                format!(
                    "cursor pack '{spec}' {why}; built-in packs: {}",
                    built_in_ids().join(", ")
                ),
            )
            .with_param("pack", spec)
            .with_param("path", dir.display())
        };
        if dir.extension().is_some_and(|e| e == "srcursor") && dir.is_file() {
            return Err(not_found(
                "is a zipped .srcursor file; zipped packs come later, unzip it into a folder",
            ));
        }
        let manifest_path = dir.join("cursor-pack.json");
        let manifest = std::fs::read_to_string(&manifest_path)
            .map_err(|_| not_found("is not built in, and no cursor-pack.json was found there"))?;
        Self::from_parts(&manifest, &dir.display().to_string(), |name| {
            let path: PathBuf = dir.join(name);
            std::fs::read(&path).map_err(|err| format!("{}: {err}", path.display()))
        })
    }

    /// Builds a pack from its manifest text and a file reader.
    pub fn from_parts(
        manifest_json: &str,
        origin: &str,
        read: impl Fn(&str) -> Result<Vec<u8>, String>,
    ) -> AppResult<CursorPack> {
        let invalid = |msg: String| {
            AppError::new(ErrorCode::CursorPackInvalid, format!("{origin}: {msg}"))
                .with_param("pack", origin)
                .with_param("detail", msg)
        };
        let manifest: Manifest = serde_json::from_str(manifest_json)
            .map_err(|err| invalid(format!("cursor-pack.json: {err}")))?;
        let (errors, warnings) = manifest.check();
        if !errors.is_empty() {
            return Err(invalid(errors.join("; ")));
        }
        let mut trees = HashMap::new();
        for (kind, state) in states(&manifest) {
            let bytes = read(&state.svg).map_err(&invalid)?;
            let tree = parse_svg(&bytes).map_err(|e| invalid(format!("{}: {e}", state.svg)))?;
            trees.insert(kind, tree);
        }
        Ok(CursorPack {
            manifest,
            origin: origin.to_owned(),
            warnings,
            trees,
            cache: HashMap::new(),
        })
    }

    // ── Drawing ─────────────────────────────────────────────────────────────

    /// The state actually used for `kind` (missing states fall back to
    /// the arrow; touch falls back to the built-in finger circle).
    fn resolve(&self, kind: CursorKind) -> (CursorKind, &State) {
        let s = &self.manifest.states;
        match kind {
            CursorKind::Pointer if s.pointer.is_some() => {
                (kind, s.pointer.as_ref().unwrap_or(&s.arrow))
            }
            CursorKind::Text if s.text.is_some() => (kind, s.text.as_ref().unwrap_or(&s.arrow)),
            CursorKind::Touch if s.touch.is_some() => (kind, s.touch.as_ref().unwrap_or(&s.arrow)),
            _ => (CursorKind::Arrow, &s.arrow),
        }
    }

    /// The cursor `kind` drawn at `px` device pixels per CSS pixel (device
    /// scale × cursor scale × pose scale), rotated by `rotate_deg`.
    /// Cached, so animating frames cost nothing after the first.
    pub fn sprite(&mut self, kind: CursorKind, px: f64, rotate_deg: f64) -> Option<&Sprite> {
        // Draw at the cached step size, so a sprite never depends on which
        // frame happened to draw it first (renders stay byte-identical).
        let px = (px * 100.0).round() / 100.0;
        let rotate_deg = (rotate_deg * 10.0).round() / 10.0;
        if kind == CursorKind::Touch && self.manifest.states.touch.is_none() {
            return fallback_touch(&mut self.cache, px, rotate_deg);
        }
        let (kind, size, hotspot) = {
            let (kind, state) = self.resolve(kind);
            (kind, state.size, state.hotspot)
        };
        let key = (
            kind,
            (px * 100.0).round() as i64,
            (rotate_deg * 10.0).round() as i64,
        );
        if let std::collections::hash_map::Entry::Vacant(slot) = self.cache.entry(key) {
            let tree = self.trees.get(&kind)?;
            let shape = raster::Shape {
                tree,
                size,
                hotspot,
            };
            // Touch indicators get no drop shadow: they are already translucent.
            let shadow = (kind != CursorKind::Touch)
                .then_some(self.manifest.effects.shadow.as_ref())
                .flatten();
            slot.insert(raster::draw(&shape, px, rotate_deg, shadow)?);
        }
        self.cache.get(&key)
    }

    /// Cursor pose `age_ms` after a mouse-down (neutral outside the animation).
    pub fn click_pose(&self, age_ms: f64) -> Pose {
        let (frames, easing) = match &self.manifest.animations.click {
            Some(c) => (c.keyframes.as_slice(), c.easing.unwrap_or(Easing::EaseOut)),
            None => (DEFAULT_CLICK, Easing::EaseOut),
        };
        let (Some(first), Some(last)) = (frames.first(), frames.last()) else {
            return Pose::NEUTRAL;
        };
        if age_ms < first.t || age_ms > last.t {
            return Pose::NEUTRAL;
        }
        let pose = |k: &Keyframe| Pose {
            scale: k.scale.unwrap_or(1.0),
            rotate: k.rotate.unwrap_or(0.0),
            opacity: k.opacity.unwrap_or(1.0),
            offset: k.offset.unwrap_or([0.0, 0.0]),
        };
        let Some(i) = frames.windows(2).position(|w| age_ms <= w[1].t) else {
            return pose(last);
        };
        let (a, b) = (pose(&frames[i]), pose(&frames[i + 1]));
        let span = (frames[i + 1].t - frames[i].t).max(f64::EPSILON);
        let p = easing.apply(((age_ms - frames[i].t) / span).clamp(0.0, 1.0));
        let mix = |x: f64, y: f64| x + (y - x) * p;
        Pose {
            scale: mix(a.scale, b.scale),
            rotate: mix(a.rotate, b.rotate),
            opacity: mix(a.opacity, b.opacity),
            offset: [mix(a.offset[0], b.offset[0]), mix(a.offset[1], b.offset[1])],
        }
    }

    /// Ripple drawn at clicks and taps, if the pack has one (and, for the
    /// mouse, its click animation asks for it).
    pub fn ripple(&self) -> Option<RippleStyle> {
        let wants = self
            .manifest
            .animations
            .click
            .as_ref()
            .is_none_or(|c| c.effect.as_deref() == Some("ripple"));
        let r: &Ripple = self.manifest.effects.ripple.as_ref().filter(|_| wants)?;
        Some(RippleStyle {
            color: parse_color(&r.color)?,
            radius: r.radius,
            duration_ms: r.duration_ms,
        })
    }
}

/// States present in the manifest.
fn states(m: &Manifest) -> Vec<(CursorKind, &State)> {
    let s = &m.states;
    let mut v = vec![(CursorKind::Arrow, &s.arrow)];
    v.extend(s.pointer.as_ref().map(|x| (CursorKind::Pointer, x)));
    v.extend(s.text.as_ref().map(|x| (CursorKind::Text, x)));
    v.extend(s.touch.as_ref().map(|x| (CursorKind::Touch, x)));
    v
}

/// Parses an SVG safely: no fonts, no outside files.
fn parse_svg(bytes: &[u8]) -> Result<usvg::Tree, String> {
    let external = AtomicBool::new(false);
    let opts = usvg::Options {
        resources_dir: None,
        image_href_resolver: usvg::ImageHrefResolver {
            resolve_data: usvg::ImageHrefResolver::default_data_resolver(),
            resolve_string: Box::new(|_href, _opts| {
                external.store(true, Ordering::Relaxed);
                None
            }),
        },
        ..usvg::Options::default()
    };
    let tree = usvg::Tree::from_data(bytes, &opts).map_err(|e| e.to_string())?;
    if external.load(Ordering::Relaxed) {
        return Err("links to an outside file (not allowed; embed images as data: URLs)".into());
    }
    Ok(tree)
}

/// Built-in finger circle for packs without a `touch` state.
fn fallback_touch(
    cache: &mut HashMap<(CursorKind, i64, i64), Sprite>,
    px: f64,
    rotate: f64,
) -> Option<&Sprite> {
    let key = (
        CursorKind::Touch,
        (px * 100.0).round() as i64,
        (rotate * 10.0).round() as i64,
    );
    if let std::collections::hash_map::Entry::Vacant(slot) = cache.entry(key) {
        let svg = include_str!("../packs/classic-dark/touch.svg");
        let tree = parse_svg(svg.as_bytes()).ok()?;
        let shape = raster::Shape {
            tree: &tree,
            size: 48.0,
            hotspot: [24.0, 24.0],
        };
        slot.insert(raster::draw(&shape, px, rotate, None)?);
    }
    cache.get(&key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_packs_load_and_draw_every_state() {
        for id in built_in_ids() {
            let mut pack = CursorPack::load(id, Path::new(".")).unwrap();
            assert!(pack.warnings.is_empty());
            for kind in [
                CursorKind::Arrow,
                CursorKind::Pointer,
                CursorKind::Text,
                CursorKind::Touch,
            ] {
                let s = pack.sprite(kind, 1.0, 0.0).unwrap();
                // Touch circles are translucent on purpose: count what's clearly visible.
                let seen = s.pixmap.pixels().iter().filter(|p| p.alpha() > 60).count();
                assert!(seen > 40, "{id} {kind:?} draws something ({seen} px)");
            }
            assert!(pack.ripple().is_some());
        }
    }

    #[test]
    fn hotspot_is_the_arrow_tip_and_scales_with_dpr() {
        let mut pack = CursorPack::load("classic-dark", Path::new(".")).unwrap();
        for px in [1.0, 2.0, 3.0] {
            let s = pack.sprite(CursorKind::Arrow, px, 0.0).unwrap().clone();
            let (hx, hy) = (s.hotspot_x as u32, s.hotspot_y as u32);
            // The tip pixel itself is covered; far up-left of it is empty.
            let at = |x: u32, y: u32| s.pixmap.pixel(x, y).map_or(0, |p| p.alpha());
            assert!(
                at(hx + px as u32, hy + 2 * px as u32) > 100,
                "tip at hotspot (px {px})"
            );
            assert_eq!(at(hx - 4 * px as u32, hy - 4 * px as u32), 0);
        }
    }

    #[test]
    fn click_pose_follows_keyframes_and_is_neutral_outside() {
        let pack = CursorPack::load("classic-dark", Path::new(".")).unwrap();
        assert_eq!(pack.click_pose(-1.0), Pose::NEUTRAL);
        assert_eq!(pack.click_pose(500.0), Pose::NEUTRAL);
        assert!((pack.click_pose(80.0).scale - 0.85).abs() < 1e-9);
        let mid = pack.click_pose(40.0).scale;
        assert!(mid < 1.0 && mid > 0.85, "{mid}");
    }

    #[test]
    fn folder_packs_missing_states_and_bad_svgs() {
        let dir = std::env::temp_dir().join(format!("sr-cursor-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><rect width="64" height="64"/></svg>"#;
        std::fs::write(dir.join("a.svg"), svg).unwrap();
        std::fs::write(
            dir.join("cursor-pack.json"),
            r#"{ "format_version": 1, "id": "box", "name": "Box",
                 "states": { "arrow": { "svg": "a.svg", "hotspot": [0, 0], "size": 16 } } }"#,
        )
        .unwrap();
        let mut pack = CursorPack::load(
            dir.file_name().unwrap().to_str().unwrap(),
            &std::env::temp_dir(),
        )
        .unwrap();
        // A 64-unit SVG drawn at size 16: a 16×16 square from the hotspot.
        let s = pack.sprite(CursorKind::Pointer, 1.0, 0.0).unwrap().clone();
        let (hx, hy) = (s.hotspot_x as u32, s.hotspot_y as u32);
        assert_eq!(
            s.pixmap.pixel(hx + 15, hy + 15).unwrap().alpha(),
            255,
            "pointer falls back to arrow"
        );
        assert_eq!(s.pixmap.pixel(hx + 17, hy + 17).unwrap().alpha(), 0);
        assert!(
            pack.sprite(CursorKind::Touch, 1.0, 0.0).is_some(),
            "built-in finger circle"
        );

        // An SVG pointing outside the pack is refused.
        std::fs::write(
            dir.join("a.svg"),
            r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" viewBox="0 0 8 8"><image xlink:href="/etc/passwd.png" width="8" height="8"/></svg>"#,
        )
        .unwrap();
        let err = CursorPack::load(dir.to_str().unwrap(), Path::new("/")).unwrap_err();
        assert_eq!(err.code, ErrorCode::CursorPackInvalid);
        assert!(err.message.contains("outside file"), "{}", err.message);

        let missing = CursorPack::load("no-such-pack", &dir).unwrap_err();
        assert_eq!(missing.code, ErrorCode::CursorPackNotFound);
        assert!(missing.message.contains("classic-dark"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
