//! Process-wide state managed by Tauri (`app.manage(...)`).
//!
//! Why: some objects must live exactly as long as the app — today the log
//! guard (dropping it would stop file logging). Commands read from here via
//! `tauri::State<AppState>`.

use std::sync::Mutex;

use sr_diagnostics::LogGuard;

/// Shared state of the running desktop app.
pub struct AppState {
    /// Keeps file logging alive; `None` if logging to file failed (SR-0101).
    log_guard: Mutex<Option<LogGuard>>,
}

impl AppState {
    /// Wraps the log guard created during startup.
    pub fn new(log_guard: Option<LogGuard>) -> Self {
        Self {
            log_guard: Mutex::new(log_guard),
        }
    }

    /// Folder with log files, if file logging is active.
    pub fn log_dir(&self) -> Option<String> {
        let guard = self.log_guard.lock().ok()?;
        guard
            .as_ref()?
            .log_dir()
            .map(|dir| dir.display().to_string())
    }
}
