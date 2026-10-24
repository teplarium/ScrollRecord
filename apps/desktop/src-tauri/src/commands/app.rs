//! App-level commands: versions, environment.

use sr_ipc::{AppError, AppInfo};

use crate::state::AppState;

/// Returns versions and platform info for the UI footer and bug reports.
#[tauri::command]
pub fn app_info(state: tauri::State<'_, AppState>) -> Result<AppInfo, AppError> {
    Ok(AppInfo::current(env!("CARGO_PKG_VERSION"), state.log_dir()))
}
