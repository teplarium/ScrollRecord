//! App-level commands: versions, environment.

use sr_ipc::{AppError, AppInfo, DevicePreset, Viewport};

use crate::state::AppState;

/// Returns versions and platform info for the UI footer and bug reports.
#[tauri::command]
pub fn app_info(state: tauri::State<'_, AppState>) -> Result<AppInfo, AppError> {
    Ok(AppInfo::current(env!("CARGO_PKG_VERSION"), state.log_dir()))
}

/// Built-in device presets (name and viewport size) for the editor's device
/// switcher and block cards. Logic: `sr_core::flow::Viewport::presets`.
#[tauri::command]
pub fn device_presets() -> Result<Vec<DevicePreset>, AppError> {
    Ok(Viewport::presets())
}
