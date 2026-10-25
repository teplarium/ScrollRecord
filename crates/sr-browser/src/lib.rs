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
use sr_core::flow::CursorKind;

pub use locate::{BROWSER_ENV_VAR, find_browser};
pub use page::{
    BlockMeasure, Browser, Emulation, Measurements, Page, PrepareOptions, PrepareReport,
};
pub use process::LaunchOptions;

/// Pointer input for one frame (viewport CSS px).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PointerInput {
    /// Cursor position.
    pub x: f64,
    /// Cursor position.
    pub y: f64,
    /// The page receives the position (hover effects react). When this turns
    /// off, the mouse is taken off the page.
    pub live: bool,
    /// Report which cursor image the page shows at (x, y).
    pub detect_kind: bool,
    /// Presses and releases to send, in order.
    pub events: Vec<PressEvent>,
}

/// A mouse button or finger going down or up.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PressEvent {
    /// Down (true) or up.
    pub down: bool,
    /// Touch instead of mouse.
    pub touch: bool,
    /// Position.
    pub x: f64,
    /// Position.
    pub y: f64,
}

/// What one step did.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StepResult {
    /// Scroll position actually applied (the browser clamps it at the page
    /// bottom).
    pub scroll_y: f64,
    /// Cursor image the page asks for, if `detect_kind` was set.
    pub cursor_kind: Option<CursorKind>,
}

/// The two things frame capture needs from a page.
///
/// Why a trait: `sr-capture` can be tested with a fake page, and another
/// engine could be plugged in later without touching the capture loop.
pub trait FrameDriver {
    /// Scrolls to `scroll_y` (CSS px), moves page time forward by
    /// `advance_ms`, then applies `pointer` (position, presses).
    fn step(
        &mut self,
        scroll_y: f64,
        advance_ms: f64,
        pointer: &PointerInput,
    ) -> AppResult<StepResult>;

    /// Captures the current viewport as PNG bytes.
    fn capture_png(&mut self) -> AppResult<Vec<u8>>;
}
