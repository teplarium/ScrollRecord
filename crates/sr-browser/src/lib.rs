//! # sr-browser — controls a Chromium-based browser
//!
//! Purpose: finds Chrome/Edge/Chromium, starts it headless with a throw-away
//!          profile, emulates the target device and drives the page frame by
//!          frame through the Chrome DevTools Protocol (HANDOFF §7.3).
//! Solves:  identical rendering on every OS (always the Chromium engine) and
//!          full control over time, scrolling and screenshots.
//! Rules:   no knowledge of scenarios or timelines; it only executes simple
//!          commands. The page-side logic lives in `js/runtime.js`.
//! Layout:  `locate.rs` (find the executable) → `process.rs` (start/stop) →
//!          `cdp.rs` (protocol) → `page.rs` (high-level API).

mod cdp;
mod locate;
mod page;
mod process;

use sr_core::AppResult;

pub use locate::{BROWSER_ENV_VAR, find_browser};
pub use page::{
    BlockMeasure, Browser, Emulation, Measurements, Page, PrepareOptions, PrepareReport,
};
pub use process::LaunchOptions;

/// The two things frame capture needs from a page.
///
/// Why a trait: `sr-capture` can be tested with a fake page, and another
/// engine could be plugged in later without touching the capture loop.
pub trait FrameDriver {
    /// Scrolls to `scroll_y` (CSS px) and moves page time forward by
    /// `advance_ms`. Returns the scroll position actually applied (the
    /// browser clamps it at the page bottom).
    fn step(&mut self, scroll_y: f64, advance_ms: f64) -> AppResult<f64>;

    /// Captures the current viewport as PNG bytes.
    fn capture_png(&mut self) -> AppResult<Vec<u8>>;
}
