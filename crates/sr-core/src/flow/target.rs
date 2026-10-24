//! # Render targets: device, aspect ratio, resolution, format
//!
//! Purpose: describes *what* video to produce (HANDOFF §5.7) and computes the
//!          exact pixel geometry for it.
//! Solves:  "iPhone, 9:16, 1080p" must become concrete numbers the browser and
//!          encoder agree on: CSS viewport size, device scale factor and the
//!          final even-sized frame (H.264 requires even width and height).

use std::fmt;

use serde::{Deserialize, Serialize};

/// One video to render from a scenario.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    /// Unique id, used on the command line (`--target mobile`) and in file names.
    pub id: String,
    /// Device preset name (`"iphone-15"`) or a custom viewport object.
    pub viewport: ViewportRef,
    /// Output proportions: `"16:9"`, `"9:16"`, `"1:1"`, any `"W:H"`, or `"native"`.
    #[serde(default)]
    pub aspect: Aspect,
    /// `"720p"`, `"1080p"`, `"1440p"`, `"2160p"`/`"4k"` (short side), or `{ "scale": 2 }`.
    #[serde(default)]
    pub resolution: Resolution,
    /// Frames per second (1–120). Default 60.
    #[serde(default = "default_fps")]
    pub fps: u32,
    /// Output container/codec.
    #[serde(default)]
    pub format: OutputFormat,
    /// How the site fills the frame. Only `fill-viewport` until Phase 4.
    #[serde(default)]
    pub fit: Fit,
    /// What the camera frames. `block` (Block clip) arrives in Phase 4.
    #[serde(default)]
    pub frame: Frame,
    /// Block node id to frame when `frame` is `block`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame_block: Option<String>,
    /// Background colour or `"transparent"` (used by `contain` / Block clip, Phase 4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
}

fn default_fps() -> u32 {
    60
}

/// Output container / codec.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OutputFormat {
    /// H.264 in MP4 — plays everywhere.
    #[default]
    Mp4,
    /// VP9 in WebM — good for the web.
    Webm,
    /// A folder of numbered PNG files — lossless, needs no ffmpeg.
    Png,
}

impl OutputFormat {
    /// File extension (or folder suffix for PNG sequences).
    pub fn extension(self) -> &'static str {
        match self {
            OutputFormat::Mp4 => "mp4",
            OutputFormat::Webm => "webm",
            OutputFormat::Png => "png",
        }
    }
}

/// How the page is placed in the video frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Fit {
    /// The browser viewport is resized to the output proportions; the site fills the frame.
    #[default]
    FillViewport,
    /// The site sits in a window on a background (Phase 4).
    Contain,
}

/// What the camera follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Frame {
    /// The whole viewport.
    #[default]
    Viewport,
    /// Only one block, cropped to its bounds (Block clip, Phase 4).
    Block,
}

// ── Viewports ───────────────────────────────────────────────────────────────

/// A device preset name or an inline custom viewport.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ViewportRef {
    /// Name of a built-in preset, see [`Viewport::preset`].
    Preset(String),
    /// Fully custom device.
    Custom(Viewport),
}

impl ViewportRef {
    /// Resolves the preset name or returns the custom viewport.
    pub fn resolve(&self) -> Result<Viewport, String> {
        match self {
            ViewportRef::Custom(v) => Ok(v.clone()),
            ViewportRef::Preset(name) => Viewport::preset(name).ok_or_else(|| {
                format!(
                    "unknown viewport preset '{name}' (known: {})",
                    Viewport::PRESET_NAMES.join(", ")
                )
            }),
        }
    }

    /// Preset name, if this is a preset (used to match `overrides` keys).
    pub fn preset_name(&self) -> Option<&str> {
        match self {
            ViewportRef::Preset(name) => Some(name),
            ViewportRef::Custom(_) => None,
        }
    }
}

/// A browser viewport in CSS pixels.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Viewport {
    /// Width in CSS pixels.
    pub width: u32,
    /// Height in CSS pixels (for `aspect: "native"`).
    pub height: u32,
    /// Device pixel ratio of the real device (informative; the render
    /// resolution decides the actual scale).
    #[serde(default = "one")]
    pub scale: f64,
    /// Mobile emulation (mobile layout, meta viewport, no scrollbars).
    #[serde(default)]
    pub mobile: bool,
    /// Touch events. Defaults to `mobile`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub touch: Option<bool>,
    /// User-Agent string; `None` keeps the browser's own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,
}

fn one() -> f64 {
    1.0
}

const UA_IPHONE: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Mobile/15E148 Safari/604.1";
const UA_IPAD: &str = "Mozilla/5.0 (iPad; CPU OS 17_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Mobile/15E148 Safari/604.1";
const UA_ANDROID: &str = "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Mobile Safari/537.36";

impl Viewport {
    /// Names accepted by [`Viewport::preset`] (HANDOFF FR-VP-01).
    pub const PRESET_NAMES: &'static [&'static str] = &[
        "desktop-1920",
        "laptop-1440",
        "laptop-1366",
        "tablet-ipad",
        "iphone-15",
        "android-pixel",
    ];

    /// Built-in device presets.
    pub fn preset(name: &str) -> Option<Viewport> {
        let desktop = |width, height| Viewport {
            width,
            height,
            scale: 1.0,
            mobile: false,
            touch: None,
            user_agent: None,
        };
        let mobile = |width, height, scale, ua: &str| Viewport {
            width,
            height,
            scale,
            mobile: true,
            touch: Some(true),
            user_agent: Some(ua.to_owned()),
        };
        Some(match name {
            "desktop-1920" => desktop(1920, 1080),
            "laptop-1440" => desktop(1440, 900),
            "laptop-1366" => desktop(1366, 768),
            "tablet-ipad" => mobile(820, 1180, 2.0, UA_IPAD),
            "iphone-15" => mobile(393, 852, 3.0, UA_IPHONE),
            "android-pixel" => mobile(412, 915, 2.625, UA_ANDROID),
            _ => return None,
        })
    }

    /// Whether touch events are emulated.
    pub fn has_touch(&self) -> bool {
        self.touch.unwrap_or(self.mobile)
    }
}

// ── Aspect ratio ────────────────────────────────────────────────────────────

/// Output proportions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum Aspect {
    /// Same proportions as the device screen.
    #[default]
    Native,
    /// Width : height, e.g. 16:9 or 9:16.
    Ratio {
        /// Width part.
        w: u32,
        /// Height part.
        h: u32,
    },
}

impl TryFrom<String> for Aspect {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let v = value.trim();
        if v.eq_ignore_ascii_case("native") {
            return Ok(Aspect::Native);
        }
        let (w, h) = v.split_once(':').ok_or_else(|| {
            format!("aspect must be 'native' or 'W:H' like '16:9', got '{value}'")
        })?;
        let w: u32 = w
            .trim()
            .parse()
            .map_err(|_| format!("bad aspect width in '{value}'"))?;
        let h: u32 = h
            .trim()
            .parse()
            .map_err(|_| format!("bad aspect height in '{value}'"))?;
        if w == 0 || h == 0 {
            return Err(format!("aspect parts must be positive: '{value}'"));
        }
        Ok(Aspect::Ratio { w, h })
    }
}

impl From<Aspect> for String {
    fn from(value: Aspect) -> Self {
        value.to_string()
    }
}

impl fmt::Display for Aspect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Aspect::Native => f.write_str("native"),
            Aspect::Ratio { w, h } => write!(f, "{w}:{h}"),
        }
    }
}

// ── Resolution ──────────────────────────────────────────────────────────────

/// Output size: a named class (short side in pixels) or a plain scale factor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Resolution {
    /// `"720p"`, `"1080p"`, `"1440p"`, `"2160p"` or `"4k"`.
    Named(String),
    /// Multiply the CSS viewport by this factor, e.g. `{ "scale": 2 }`.
    Scale {
        /// Device pixels per CSS pixel.
        scale: f64,
    },
}

impl Default for Resolution {
    fn default() -> Self {
        Resolution::Named("1080p".to_owned())
    }
}

impl Resolution {
    /// Short side in pixels for named resolutions.
    pub fn short_side(name: &str) -> Result<u32, String> {
        match name.trim().to_ascii_lowercase().as_str() {
            "720p" => Ok(720),
            "1080p" => Ok(1080),
            "1440p" => Ok(1440),
            "2160p" | "4k" => Ok(2160),
            _ => Err(format!(
                "unknown resolution '{name}' (use 720p, 1080p, 1440p, 2160p/4k or {{\"scale\": n}})"
            )),
        }
    }
}

// ── Geometry ────────────────────────────────────────────────────────────────

/// Concrete pixel numbers for one render target.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OutputGeometry {
    /// Browser viewport width in CSS pixels.
    pub css_width: u32,
    /// Browser viewport height in CSS pixels.
    pub css_height: u32,
    /// Device scale factor the browser renders with.
    pub scale: f64,
    /// Final video width (even).
    pub out_width: u32,
    /// Final video height (even).
    pub out_height: u32,
}

/// Rounds to the nearest even number (≥ 2). H.264 needs even frame sizes.
fn even(x: f64) -> u32 {
    let v = ((x / 2.0).round() * 2.0) as u32;
    v.max(2)
}

impl Target {
    /// Computes viewport and output sizes (HANDOFF FR-OUT-01, FR-OUT-02).
    ///
    /// `fill-viewport`: the device *width* is kept, the viewport height is
    /// adjusted to the aspect ratio, and the scale factor is chosen so the
    /// output has the requested resolution.
    pub fn geometry(&self, viewport: &Viewport) -> Result<OutputGeometry, String> {
        if viewport.width < 100 || viewport.height < 100 {
            return Err(format!(
                "viewport too small: {}x{}",
                viewport.width, viewport.height
            ));
        }
        let css_width = viewport.width;
        let css_height = match self.aspect {
            Aspect::Native => viewport.height,
            Aspect::Ratio { w, h } => {
                (f64::from(css_width) * f64::from(h) / f64::from(w)).round() as u32
            }
        };
        let (cw, ch) = (f64::from(css_width), f64::from(css_height));
        let (out_width, out_height) = match &self.resolution {
            Resolution::Named(name) => {
                let short = f64::from(Resolution::short_side(name)?);
                if cw >= ch {
                    (even(short * cw / ch), even(short))
                } else {
                    (even(short), even(short * ch / cw))
                }
            }
            Resolution::Scale { scale } => {
                if !(0.25..=8.0).contains(scale) {
                    return Err(format!(
                        "resolution scale must be within 0.25..8, got {scale}"
                    ));
                }
                (even(cw * scale), even(ch * scale))
            }
        };
        if out_width > 8192 || out_height > 8192 {
            return Err(format!(
                "output {out_width}x{out_height} is larger than 8192 px"
            ));
        }
        Ok(OutputGeometry {
            css_width,
            css_height,
            scale: f64::from(out_width) / cw,
            out_width,
            out_height,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(viewport: &str, aspect: &str, resolution: &str) -> (Target, Viewport) {
        let t = Target {
            id: "t".into(),
            viewport: ViewportRef::Preset(viewport.into()),
            aspect: Aspect::try_from(aspect.to_owned()).unwrap(),
            resolution: Resolution::Named(resolution.into()),
            fps: 60,
            format: OutputFormat::Mp4,
            fit: Fit::FillViewport,
            frame: Frame::Viewport,
            frame_block: None,
            background: None,
        };
        let vp = t.viewport.resolve().unwrap();
        (t, vp)
    }

    #[test]
    fn desktop_16_9_1080p_is_full_hd() {
        let (t, vp) = target("desktop-1920", "16:9", "1080p");
        let g = t.geometry(&vp).unwrap();
        assert_eq!(
            (g.css_width, g.css_height, g.out_width, g.out_height),
            (1920, 1080, 1920, 1080)
        );
        assert!((g.scale - 1.0).abs() < 1e-9);
    }

    #[test]
    fn iphone_9_16_1080p_is_vertical_full_hd() {
        let (t, vp) = target("iphone-15", "9:16", "1080p");
        let g = t.geometry(&vp).unwrap();
        assert_eq!((g.css_width, g.css_height), (393, 699));
        assert_eq!((g.out_width, g.out_height), (1080, 1920));
        assert!((g.scale - 1080.0 / 393.0).abs() < 1e-9);
    }

    #[test]
    fn native_keeps_device_proportions_and_even_sizes() {
        let (t, vp) = target("iphone-15", "native", "1080p");
        let g = t.geometry(&vp).unwrap();
        assert_eq!((g.css_width, g.css_height), (393, 852));
        assert_eq!(g.out_width, 1080);
        assert_eq!(g.out_height % 2, 0);
    }

    #[test]
    fn parses_aspect_and_rejects_garbage() {
        assert_eq!(
            Aspect::try_from("21:9".to_owned()),
            Ok(Aspect::Ratio { w: 21, h: 9 })
        );
        assert!(Aspect::try_from("wide".to_owned()).is_err());
        assert!(Aspect::try_from("0:9".to_owned()).is_err());
        assert!(Resolution::short_side("4K").is_ok());
        assert!(Resolution::short_side("999p").is_err());
    }
}
