//! # sr-core
//!
//! Purpose: the foundation every other ScrollRecord crate builds on:
//!          the scenario format (`flow`), easing curves, the error-code
//!          catalog and validation rules.
//! Solves:  keeps the "language" of the app in one place, so the CLI, the
//!          desktop shell and (via generated TypeScript) the UI all agree on
//!          what a scenario or an error looks like.
//! Rules:   no IO, no async, no browser, no UI. Pure data + pure functions.
//!          Nothing in the workspace is allowed to be a dependency of this crate
//!          except small, stable libraries (serde, serde_json, thiserror, ts-rs).
//! See also: `docs/ERRORS.md` (human-readable error catalog), `PROJECT_MAP.md`.

pub mod easing;
pub mod error;
pub mod flow;

pub use easing::Easing;
pub use error::{AppError, AppResult, ErrorCode};
pub use flow::{FORMAT_VERSION, Flow, FlowPlan};

/// Version of the core library, taken from the workspace `Cargo.toml`.
/// Shown in the UI and in diagnostic reports so bug reports always say
/// exactly which build they came from.
pub const CORE_VERSION: &str = env!("CARGO_PKG_VERSION");
