//! # Easing curves
//!
//! Purpose: turns linear progress (0→1) into eased progress for scroll moves.
//! Solves:  scrolling at constant speed looks robotic. Easing makes it start
//!          and stop softly, like a person scrolling. The curves are the CSS
//!          standard ones, so designers can reuse values from their CSS.
//! Format:  `"linear"`, `"ease"`, `"ease-in"`, `"ease-out"`, `"ease-in-out"`
//!          or `"cubic-bezier(x1, y1, x2, y2)"`.

use std::fmt;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A timing curve, written in scenario files exactly like in CSS.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize, TS)]
#[ts(export, type = "string")]
#[serde(try_from = "String", into = "String")]
pub enum Easing {
    /// Constant speed.
    Linear,
    /// CSS `ease` — quick start, soft stop.
    Ease,
    /// Slow start.
    EaseIn,
    /// Slow stop.
    EaseOut,
    /// Slow start and slow stop (default: most natural for scrolling).
    #[default]
    EaseInOut,
    /// Custom curve: control points `(x1, y1, x2, y2)`, x values in 0..=1.
    CubicBezier([f64; 4]),
}

impl Easing {
    /// Control points of the curve (linear is the straight diagonal).
    fn points(self) -> [f64; 4] {
        match self {
            Easing::Linear => [0.0, 0.0, 1.0, 1.0],
            Easing::Ease => [0.25, 0.1, 0.25, 1.0],
            Easing::EaseIn => [0.42, 0.0, 1.0, 1.0],
            Easing::EaseOut => [0.0, 0.0, 0.58, 1.0],
            Easing::EaseInOut => [0.42, 0.0, 0.58, 1.0],
            Easing::CubicBezier(p) => p,
        }
    }

    /// Eased progress for linear progress `t` (clamped to 0..=1).
    pub fn apply(self, t: f64) -> f64 {
        let t = t.clamp(0.0, 1.0);
        if matches!(self, Easing::Linear) {
            return t;
        }
        let [x1, y1, x2, y2] = self.points();
        let s = solve_bezier_param(t, x1, x2);
        bezier(s, y1, y2)
    }
}

// ── Cubic Bézier math ───────────────────────────────────────────────────────
// Why: a CSS curve is defined as y(x), but the Bézier formula gives x(s) and
//      y(s) for a hidden parameter s. We find s for the requested x (Newton's
//      method, with bisection as a safe fallback) and then evaluate y(s).

/// One coordinate of a cubic Bézier with endpoints 0 and 1.
fn bezier(s: f64, p1: f64, p2: f64) -> f64 {
    let u = 1.0 - s;
    3.0 * u * u * s * p1 + 3.0 * u * s * s * p2 + s * s * s
}

/// Derivative of [`bezier`] with respect to `s`.
fn bezier_slope(s: f64, p1: f64, p2: f64) -> f64 {
    let u = 1.0 - s;
    3.0 * u * u * p1 + 6.0 * u * s * (p2 - p1) + 3.0 * s * s * (1.0 - p2)
}

/// Finds the parameter `s` where the curve's x equals `x`.
fn solve_bezier_param(x: f64, x1: f64, x2: f64) -> f64 {
    let mut s = x;
    for _ in 0..8 {
        let err = bezier(s, x1, x2) - x;
        if err.abs() < 1e-7 {
            return s;
        }
        let slope = bezier_slope(s, x1, x2);
        if slope.abs() < 1e-6 {
            break;
        }
        s -= err / slope;
    }
    // Newton did not converge (flat slope): bisection always does.
    let (mut lo, mut hi) = (0.0, 1.0);
    s = x;
    for _ in 0..60 {
        let v = bezier(s, x1, x2);
        if (v - x).abs() < 1e-7 {
            break;
        }
        if v < x {
            lo = s;
        } else {
            hi = s;
        }
        s = (lo + hi) / 2.0;
    }
    s
}

// ── Text form (scenario files) ──────────────────────────────────────────────

impl TryFrom<String> for Easing {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let v = value.trim().to_ascii_lowercase();
        Ok(match v.as_str() {
            "linear" => Easing::Linear,
            "ease" => Easing::Ease,
            "ease-in" => Easing::EaseIn,
            "ease-out" => Easing::EaseOut,
            "ease-in-out" => Easing::EaseInOut,
            _ => {
                let inner = v
                    .strip_prefix("cubic-bezier(")
                    .and_then(|rest| rest.strip_suffix(')'))
                    .ok_or_else(|| format!("unknown easing '{value}'"))?;
                let nums: Vec<f64> = inner
                    .split(',')
                    .map(|n| n.trim().parse::<f64>())
                    .collect::<Result<_, _>>()
                    .map_err(|_| format!("cubic-bezier needs 4 numbers: '{value}'"))?;
                let [x1, y1, x2, y2] = <[f64; 4]>::try_from(nums)
                    .map_err(|_| format!("cubic-bezier needs 4 numbers: '{value}'"))?;
                if !(0.0..=1.0).contains(&x1) || !(0.0..=1.0).contains(&x2) {
                    return Err(format!(
                        "cubic-bezier x values must be within 0..1: '{value}'"
                    ));
                }
                Easing::CubicBezier([x1, y1, x2, y2])
            }
        })
    }
}

impl From<Easing> for String {
    fn from(value: Easing) -> Self {
        value.to_string()
    }
}

impl fmt::Display for Easing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Easing::Linear => f.write_str("linear"),
            Easing::Ease => f.write_str("ease"),
            Easing::EaseIn => f.write_str("ease-in"),
            Easing::EaseOut => f.write_str("ease-out"),
            Easing::EaseInOut => f.write_str("ease-in-out"),
            Easing::CubicBezier([a, b, c, d]) => write!(f, "cubic-bezier({a}, {b}, {c}, {d})"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoints_are_exact() {
        for e in [
            Easing::Linear,
            Easing::Ease,
            Easing::EaseIn,
            Easing::EaseOut,
            Easing::EaseInOut,
        ] {
            assert!((e.apply(0.0)).abs() < 1e-6, "{e}");
            assert!((e.apply(1.0) - 1.0).abs() < 1e-6, "{e}");
        }
    }

    #[test]
    fn ease_in_out_is_symmetric_and_monotonic() {
        let e = Easing::EaseInOut;
        assert!((e.apply(0.5) - 0.5).abs() < 1e-4);
        let mut prev = 0.0;
        for i in 1..=100 {
            let v = e.apply(i as f64 / 100.0);
            assert!(v >= prev, "not monotonic at {i}");
            prev = v;
        }
        assert!(e.apply(0.1) < 0.1, "should start slowly");
    }

    #[test]
    fn parses_css_syntax() {
        assert_eq!(
            Easing::try_from("Ease-In-Out".to_owned()),
            Ok(Easing::EaseInOut)
        );
        assert_eq!(
            Easing::try_from("cubic-bezier(0.2, 0, 0, 1)".to_owned()),
            Ok(Easing::CubicBezier([0.2, 0.0, 0.0, 1.0]))
        );
        assert!(Easing::try_from("bounce".to_owned()).is_err());
        assert!(Easing::try_from("cubic-bezier(2, 0, 0, 1)".to_owned()).is_err());
    }
}
