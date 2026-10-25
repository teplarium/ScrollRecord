//! SVG cursor → pixels, with an optional soft shadow.
//!
//! Why its own module: drawing must be crisp at any device scale (a 4K
//! render draws the cursor at 2×, HANDOFF FR-CUR-01), so cursors stay
//! vectors until this point and are drawn at exactly the size needed.

use resvg::tiny_skia::{Pixmap, PixmapPaint, Transform};

use crate::manifest::Shadow;

/// A drawn cursor and where its hotspot is inside the image.
#[derive(Debug, Clone, PartialEq)]
pub struct Sprite {
    /// Premultiplied RGBA image.
    pub pixmap: Pixmap,
    /// Hotspot x in device pixels inside `pixmap`.
    pub hotspot_x: f64,
    /// Hotspot y in device pixels inside `pixmap`.
    pub hotspot_y: f64,
}

/// What to draw: an SVG tree with its pack geometry.
pub(crate) struct Shape<'a> {
    pub tree: &'a resvg::usvg::Tree,
    /// Cursor size in its own pixels (longer side).
    pub size: f64,
    /// Hotspot in the cursor's own pixels.
    pub hotspot: [f64; 2],
}

/// Draws `shape` at `px` device pixels per cursor pixel, rotated by
/// `rotate_deg` around the hotspot, with `shadow` (sizes in CSS px).
/// Returns `None` only for absurd sizes (nothing to draw).
pub(crate) fn draw(
    shape: &Shape<'_>,
    px: f64,
    rotate_deg: f64,
    shadow: Option<&Shadow>,
) -> Option<Sprite> {
    let svg = shape.tree.size();
    let longest = f64::from(svg.width().max(svg.height())).max(1.0);
    let k = shape.size * px / longest;

    // ── Canvas: centred on the hotspot, big enough for any rotation + shadow ──
    let reach = shape.size * px * std::f64::consts::SQRT_2;
    let pad = shadow.map_or(0.0, |s| {
        s.blur * px * 3.0 + s.offset[0].abs().max(s.offset[1].abs()) * px
    });
    let half = (reach + pad + 2.0).ceil();
    let side = (half * 2.0) as u32;
    let mut cursor = Pixmap::new(side, side)?;
    let transform = Transform::from_translate(half as f32, half as f32)
        .pre_rotate(rotate_deg as f32)
        .pre_translate(
            (-shape.hotspot[0] * px) as f32,
            (-shape.hotspot[1] * px) as f32,
        )
        .pre_scale(k as f32, k as f32);
    resvg::render(shape.tree, transform, &mut cursor.as_mut());

    let Some(shadow) = shadow.filter(|s| s.opacity > 0.0) else {
        return Some(Sprite {
            pixmap: cursor,
            hotspot_x: half,
            hotspot_y: half,
        });
    };

    // ── Shadow: blurred copy of the cursor's alpha, shifted and dimmed ──
    let mut alpha: Vec<f32> = cursor
        .pixels()
        .iter()
        .map(|p| f32::from(p.alpha()) / 255.0)
        .collect();
    let sigma = shadow.blur * px / 2.0;
    blur(&mut alpha, side as usize, sigma);
    let mut out = Pixmap::new(side, side)?;
    let (dx, dy) = (
        (shadow.offset[0] * px).round() as i64,
        (shadow.offset[1] * px).round() as i64,
    );
    let n = side as i64;
    let data = out.data_mut();
    for y in 0..n {
        for x in 0..n {
            let (sx, sy) = (x - dx, y - dy);
            if sx < 0 || sy < 0 || sx >= n || sy >= n {
                continue;
            }
            let a = alpha[(sy * n + sx) as usize] * shadow.opacity as f32;
            // Black, premultiplied: colour channels stay 0.
            data[((y * n + x) * 4 + 3) as usize] = (a.clamp(0.0, 1.0) * 255.0).round() as u8;
        }
    }
    out.draw_pixmap(
        0,
        0,
        cursor.as_ref(),
        &PixmapPaint::default(),
        Transform::identity(),
        None,
    );
    Some(Sprite {
        pixmap: out,
        hotspot_x: half,
        hotspot_y: half,
    })
}

/// Approximate Gaussian blur: three box blurs in each direction.
fn blur(values: &mut [f32], side: usize, sigma: f64) {
    if sigma < 0.3 {
        return;
    }
    // Box width for 3 passes ≈ sqrt(12σ²/3 + 1).
    let r = (((4.0 * sigma * sigma + 1.0).sqrt() - 1.0) / 2.0)
        .round()
        .max(1.0) as usize;
    let mut tmp = vec![0.0f32; values.len()];
    for _ in 0..3 {
        box_pass(values, &mut tmp, side, r, true);
        box_pass(&tmp, values, side, r, false);
    }
}

fn box_pass(src: &[f32], dst: &mut [f32], side: usize, r: usize, horizontal: bool) {
    let width = (2 * r + 1) as f32;
    for line in 0..side {
        let at = |i: usize| {
            if horizontal {
                line * side + i
            } else {
                i * side + line
            }
        };
        let mut sum = 0.0f32;
        for i in 0..=r.min(side - 1) {
            sum += src[at(i)];
        }
        for i in 0..side {
            dst[at(i)] = sum / width;
            if i + r + 1 < side {
                sum += src[at(i + r + 1)];
            }
            if i >= r {
                sum -= src[at(i - r)];
            }
        }
    }
}

/// Adds `sprite` to `pixmap` with its hotspot at (`x`, `y`) device px.
pub fn blit(pixmap: &mut Pixmap, sprite: &Sprite, x: f64, y: f64, opacity: f64) {
    let left = (x - sprite.hotspot_x).round() as i32;
    let top = (y - sprite.hotspot_y).round() as i32;
    let paint = PixmapPaint {
        opacity: opacity.clamp(0.0, 1.0) as f32,
        ..PixmapPaint::default()
    };
    pixmap.draw_pixmap(
        left,
        top,
        sprite.pixmap.as_ref(),
        &paint,
        Transform::identity(),
        None,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn box_blur_keeps_total_and_spreads() {
        let side = 21;
        let mut v = vec![0.0f32; side * side];
        v[10 * side + 10] = 1.0;
        blur(&mut v, side, 2.0);
        let total: f32 = v.iter().sum();
        assert!((total - 1.0).abs() < 1e-3, "{total}");
        assert!(v[10 * side + 10] < 0.2 && v[10 * side + 12] > 0.0);
    }
}
