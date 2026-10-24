//! Project commands (create / open).
//!
//! Phase 0: placeholders that return `SR-0002 NotImplemented`.
//! Why they exist already: the start screen buttons call them, which proves
//! the full error path end to end — Rust error → IPC → typed `AppError` in
//! TypeScript → translated message on screen. Phase 3 replaces the bodies.

use sr_ipc::AppError;

/// Creates a new project. Planned for roadmap phase 3.
#[tauri::command]
pub fn create_project() -> Result<(), AppError> {
    Err(AppError::not_implemented("projects", 3))
}

/// Opens an existing project. Planned for roadmap phase 3.
#[tauri::command]
pub fn open_project() -> Result<(), AppError> {
    Err(AppError::not_implemented("projects", 3))
}
