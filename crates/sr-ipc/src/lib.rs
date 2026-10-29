//! # sr-ipc
//!
//! Purpose: the IPC contract — every data type that travels between the Tauri
//!          desktop shell (Rust) and the React UI (TypeScript), apart from the
//!          domain types that already live in `sr-core`.
//! Solves:  keeps the contract in one small crate that compiles without Tauri,
//!          so TypeScript bindings can be regenerated and checked in CI quickly
//!          (HANDOFF D-18). The shell's command handlers stay thin: they only
//!          call library code and return these types.
//! Rules:   only plain data (serde + ts-rs). No logic beyond simple
//!          constructors. Every exported type gets `#[ts(export)]`.
//!          Types owned by a library crate (errors and the scenario in
//!          `sr-core`, project data in `sr-project`) are defined there and
//!          re-exported here, so the whole contract is visible in one place.
//! See also: `apps/desktop/src/shared/ipc/` (TypeScript side of the contract).

pub mod app_info;

pub use app_info::AppInfo;
pub use sr_core::flow::Flow;
pub use sr_core::{AppError, AppResult, ErrorCode};
pub use sr_project::{
    ImportedFlow, NewProject, OpenedProject, ProjectHome, ProjectManifest, RecentProject,
};
