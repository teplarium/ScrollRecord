//! Scenario commands for the editor. Logic: `sr_core::flow` (`path.rs` checks).

use sr_ipc::{AppError, Flow, Issue};

/// Every problem of the scenario the editor holds (also unsaved), each with
/// the shape or arrow it is about (FR-CNV-09). Never fails: a scenario that
/// can't even be read never reaches the editor.
#[tauri::command]
pub fn validate_flow(flow: Flow) -> Result<Vec<Issue>, AppError> {
    Ok(flow.issues())
}
