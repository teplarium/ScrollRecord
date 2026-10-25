//! # sr-compositor — the cursor goes onto the frame
//!
//! Purpose: takes a captured frame (PNG from the browser) and the planned
//!          cursor for that frame ([`CursorFrame`]) and draws, in order:
//!          click ripples → touch indicators → the cursor (with its press
//!          animation). Returns raw RGBA pixels for the encoder.
//! Solves:  the cursor is drawn by ScrollRecord, never by the page, so it is
//!          sharp at any resolution and looks the same on every site
//!          (HANDOFF FR-CUR-01, FR-CUR-04, FR-CUR-06).
//! Rules:   pure pixels: no browser, no files, no clock. Same input → same
//!          output, which keeps renders byte-identical.

use resvg_tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Stroke, Transform};
use sr_core::flow::CursorKind;
use sr_core::{AppError, AppResult, Easing, ErrorCode};
use sr_cursor::{CursorPack, Pose, RippleStyle, blit, tiny_skia as resvg_tiny_skia};
use sr_timeline::{CursorFrame, PRESS_HOLD_MS, TAP_LEAD_MS};

/// Width of the ripple ring in CSS px.
const RING_WIDTH: f64 = 2.5;
/// A touch indicator fades out this long after the finger lifts.
const TAP_FADE_MS: f64 = 300.0;

/// One finished frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Composed {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// RGBA, 4 bytes per pixel, rows top to bottom. Opaque.
    pub rgba: Vec<u8>,
}

/// Draws cursors onto frames for one render.
#[derive(Debug)]
pub struct Compositor {
    pack: CursorPack,
    /// Device pixels per CSS pixel (from the output resolution).
    device_scale: f64,
    /// `cursor.scale` from the scenario.
    cursor_scale: f64,
}

impl Compositor {
    /// `device_scale`: output pixels per CSS pixel; `cursor_scale`: the
    /// scenario's `cursor.scale`.
    pub fn new(pack: CursorPack, device_scale: f64, cursor_scale: f64) -> Compositor {
        Compositor {
            pack,
            device_scale,
            cursor_scale,
        }
    }

    /// Decodes `png`, draws the cursor of this frame, returns the pixels.
    ///
    /// Errors: `SR-4001` when the screenshot isn't a readable PNG.
    pub fn compose(
        &mut self,
        png: &[u8],
        cursor: &CursorFrame,
        kind: CursorKind,
    ) -> AppResult<Composed> {
        let mut pixmap = decode(png)?;
        self.draw(&mut pixmap, cursor, kind);
        Ok(Composed {
            width: pixmap.width(),
            height: pixmap.height(),
            rgba: pixmap.take(),
        })
    }

    /// Draws everything for one frame onto `pixmap` (exposed for tests).
    pub fn draw(&mut self, pixmap: &mut Pixmap, cursor: &CursorFrame, kind: CursorKind) {
        let s = self.device_scale;
        let ripple = self.pack.ripple();

        // ── 1. Ripples under everything ──
        if let Some(r) = ripple {
            for c in &cursor.clicks {
                if c.age_ms >= 0.0 && c.age_ms <= r.duration_ms {
                    draw_ripple(
                        pixmap,
                        &r,
                        c.pos.x * s,
                        c.pos.y * s,
                        c.age_ms,
                        s * self.cursor_scale,
                    );
                }
            }
        }

        // ── 2. Touch indicators: appear, press, lift and fade ──
        for c in cursor.clicks.iter().filter(|c| c.touch) {
            let Some((scale, opacity)) = tap_pose(c.age_ms) else {
                continue;
            };
            let px = s * self.cursor_scale * scale;
            if let Some(sprite) = self.pack.sprite(CursorKind::Touch, px, 0.0) {
                blit(pixmap, sprite, c.pos.x * s, c.pos.y * s, opacity);
            }
        }

        // ── 3. The cursor, squashed while a click is in progress ──
        if cursor.opacity <= 0.0 {
            return;
        }
        let pose = cursor
            .clicks
            .iter()
            .filter(|c| !c.touch && c.age_ms >= 0.0)
            .map(|c| c.age_ms)
            .reduce(f64::min)
            .map_or(Pose::NEUTRAL, |age| self.pack.click_pose(age));
        let px = s * self.cursor_scale * pose.scale;
        let x = (cursor.pos.x + pose.offset[0]) * s;
        let y = (cursor.pos.y + pose.offset[1]) * s;
        if let Some(sprite) = self.pack.sprite(kind, px, pose.rotate) {
            blit(pixmap, sprite, x, y, cursor.opacity * pose.opacity);
        }
    }
}

/// Scale and opacity of a touch indicator `age_ms` after the finger
/// touches (negative = about to touch). `None` = not visible.
fn tap_pose(age_ms: f64) -> Option<(f64, f64)> {
    let ease = Easing::EaseOut;
    if age_ms < -TAP_LEAD_MS {
        None
    } else if age_ms < 0.0 {
        let p = ease.apply((age_ms + TAP_LEAD_MS) / TAP_LEAD_MS);
        Some((0.6 + 0.4 * p, p))
    } else if age_ms < PRESS_HOLD_MS {
        Some((1.0 - 0.12 * ease.apply(age_ms / PRESS_HOLD_MS), 1.0))
    } else if age_ms < PRESS_HOLD_MS + TAP_FADE_MS {
        let p = ease.apply((age_ms - PRESS_HOLD_MS) / TAP_FADE_MS);
        Some((0.88 + 0.22 * p, 1.0 - p))
    } else {
        None
    }
}

/// Expanding, fading ring with a faint fill.
fn draw_ripple(pixmap: &mut Pixmap, r: &RippleStyle, cx: f64, cy: f64, age_ms: f64, px: f64) {
    let p = (age_ms / r.duration_ms).clamp(0.0, 1.0);
    let radius = r.radius * px * (0.3 + 0.7 * Easing::EaseOut.apply(p));
    let fade = (1.0 - p).powf(1.5);
    let alpha = f64::from(r.color[3]) / 255.0 * fade;
    let Some(circle) = PathBuilder::from_circle(cx as f32, cy as f32, radius as f32) else {
        return;
    };
    let mut paint = Paint {
        anti_alias: true,
        ..Paint::default()
    };
    let [red, green, blue, _] = r.color;
    paint.set_color(Color::from_rgba8(
        red,
        green,
        blue,
        (alpha * 0.35 * 255.0).round() as u8,
    ));
    pixmap.fill_path(
        &circle,
        &paint,
        FillRule::Winding,
        Transform::identity(),
        None,
    );
    paint.set_color(Color::from_rgba8(
        red,
        green,
        blue,
        (alpha * 255.0).round() as u8,
    ));
    let stroke = Stroke {
        width: (RING_WIDTH * px) as f32,
        ..Stroke::default()
    };
    pixmap.stroke_path(&circle, &paint, &stroke, Transform::identity(), None);
}

// ── PNG → pixels ────────────────────────────────────────────────────────────
// Browser screenshots are opaque 8-bit RGB(A). Opaque pixels are the same
// premultiplied or not, so they can be drawn on directly.
fn decode(png: &[u8]) -> AppResult<Pixmap> {
    let bad = |what: String| {
        AppError::new(
            ErrorCode::CaptureFailed,
            format!("screenshot is not a readable PNG: {what}"),
        )
    };
    let mut decoder = png::Decoder::new(std::io::Cursor::new(png));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(|e| bad(e.to_string()))?;
    let size = reader
        .output_buffer_size()
        .ok_or_else(|| bad("too large".into()))?;
    let mut buf = vec![0u8; size];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|e| bad(e.to_string()))?;
    buf.truncate(info.buffer_size());
    let pixels = (info.width as usize) * (info.height as usize);
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf,
        png::ColorType::Rgb => {
            let mut out = Vec::with_capacity(pixels * 4);
            for px in buf.chunks_exact(3) {
                out.extend_from_slice(&[px[0], px[1], px[2], 255]);
            }
            out
        }
        png::ColorType::Grayscale => buf.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::GrayscaleAlpha => buf
            .chunks_exact(2)
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        png::ColorType::Indexed => return Err(bad("unexpected palette image".into())),
    };
    let mut rgba = rgba;
    // Premultiply the rare non-opaque pixel (tiny-skia stores premultiplied colour).
    for p in rgba.chunks_exact_mut(4).filter(|p| p[3] < 255) {
        let a = u16::from(p[3]);
        for c in &mut p[..3] {
            *c = ((u16::from(*c) * a + 127) / 255) as u8;
        }
    }
    let size = resvg_tiny_skia::IntSize::from_wh(info.width, info.height)
        .ok_or_else(|| bad("empty image".into()))?;
    Pixmap::from_vec(rgba, size).ok_or_else(|| bad("size mismatch".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sr_timeline::{ActiveClick, Vec2};

    fn white(w: u32, h: u32) -> Pixmap {
        let mut p = Pixmap::new(w, h).unwrap();
        p.fill(Color::WHITE);
        p
    }

    fn frame(x: f64, y: f64, opacity: f64, clicks: Vec<ActiveClick>) -> CursorFrame {
        CursorFrame {
            pos: Vec2 { x, y },
            opacity,
            live: true,
            events: Vec::new(),
            clicks,
        }
    }

    fn dark_pixels(p: &Pixmap) -> usize {
        p.pixels().iter().filter(|c| c.red() < 60).count()
    }

    fn compositor(scale: f64) -> Compositor {
        let pack = CursorPack::load("classic-dark", std::path::Path::new(".")).unwrap();
        Compositor::new(pack, scale, 1.0)
    }

    #[test]
    fn the_cursor_is_drawn_at_its_hotspot_and_scales_with_resolution() {
        let mut c = compositor(1.0);
        let mut p = white(200, 200);
        c.draw(&mut p, &frame(100.0, 100.0, 1.0, vec![]), CursorKind::Arrow);
        let one = dark_pixels(&p);
        assert!(one > 80, "{one}");
        // Nothing up-left of the tip, ink just below-right of it.
        assert_eq!(p.pixel(95, 95).unwrap().red(), 255);
        assert!(p.pixel(101, 104).unwrap().red() < 60);

        let mut c2 = compositor(2.0);
        let mut p2 = white(400, 400);
        c2.draw(
            &mut p2,
            &frame(100.0, 100.0, 1.0, vec![]),
            CursorKind::Arrow,
        );
        let ratio = dark_pixels(&p2) as f64 / one as f64;
        assert!(
            (3.5..4.5).contains(&ratio),
            "2× resolution → 4× area ({ratio})"
        );
    }

    #[test]
    fn hidden_cursor_draws_nothing_and_kinds_differ() {
        let mut c = compositor(1.0);
        let mut p = white(100, 100);
        c.draw(&mut p, &frame(50.0, 50.0, 0.0, vec![]), CursorKind::Arrow);
        assert_eq!(dark_pixels(&p), 0);
        let mut a = white(100, 100);
        let mut b = white(100, 100);
        c.draw(&mut a, &frame(50.0, 50.0, 1.0, vec![]), CursorKind::Arrow);
        c.draw(&mut b, &frame(50.0, 50.0, 1.0, vec![]), CursorKind::Pointer);
        assert_ne!(a, b);
    }

    #[test]
    fn clicks_squash_the_cursor_and_ripple() {
        let mut c = compositor(1.0);
        let mut normal = white(200, 200);
        c.draw(
            &mut normal,
            &frame(100.0, 100.0, 1.0, vec![]),
            CursorKind::Arrow,
        );
        let click = ActiveClick {
            pos: Vec2 { x: 100.0, y: 100.0 },
            age_ms: 80.0,
            touch: false,
        };
        let mut pressed = white(200, 200);
        c.draw(
            &mut pressed,
            &frame(100.0, 100.0, 1.0, vec![click]),
            CursorKind::Arrow,
        );
        // The ripple (radius ≈ 13 px at 80 ms) tints pixels around the tip
        // that the cursor doesn't cover.
        assert!(
            pressed.pixel(100, 92).unwrap().red() < 250,
            "ripple above the tip"
        );
        assert_eq!(normal.pixel(100, 92).unwrap().red(), 255);
        assert_ne!(dark_pixels(&pressed), dark_pixels(&normal), "squashed");
        let late = ActiveClick {
            age_ms: 5000.0,
            ..click
        };
        let mut after = white(200, 200);
        c.draw(
            &mut after,
            &frame(100.0, 100.0, 1.0, vec![late]),
            CursorKind::Arrow,
        );
        assert_eq!(
            after, normal,
            "everything back to normal after the animation"
        );
    }

    #[test]
    fn taps_show_a_finger_circle_that_comes_and_goes() {
        let mut c = compositor(3.0);
        let tap = |age_ms| ActiveClick {
            pos: Vec2 { x: 50.0, y: 50.0 },
            age_ms,
            touch: true,
        };
        let draw_at = |c: &mut Compositor, age| {
            let mut p = white(300, 300);
            c.draw(
                &mut p,
                &frame(0.0, 0.0, 0.0, vec![tap(age)]),
                CursorKind::Touch,
            );
            p.pixel(150, 150).unwrap().red()
        };
        assert_eq!(draw_at(&mut c, -500.0), 255, "not yet");
        assert!(
            draw_at(&mut c, 40.0) < 200,
            "pressed: visible at the tap point"
        );
        assert_eq!(draw_at(&mut c, 2000.0), 255, "gone");
        assert!(tap_pose(-TAP_LEAD_MS / 2.0).is_some_and(|(s, o)| s < 1.0 && o < 1.0));
    }

    #[test]
    fn png_round_trip() {
        let mut p = white(4, 3);
        p.pixels_mut()[0] =
            resvg_tiny_skia::PremultipliedColorU8::from_rgba(10, 20, 30, 255).unwrap();
        let png = p.encode_png().unwrap();
        let back = decode(&png).unwrap();
        assert_eq!(back.width(), 4);
        assert_eq!(back.pixel(0, 0).unwrap().green(), 20);
        assert!(decode(b"not a png").is_err());
    }
}
