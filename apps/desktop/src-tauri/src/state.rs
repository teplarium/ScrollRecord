//! Process-wide state managed by Tauri (`app.manage(...)`).
//!
//! Why: some objects must live exactly as long as the app — the log guard
//! (dropping it would stop file logging) and the project store with the list
//! of projects open in this window (their lock files are released on exit).
//! Commands read from here via `tauri::State<AppState>`.

use std::path::Path;
use std::sync::Mutex;

use sr_diagnostics::LogGuard;
use sr_project::ProjectStore;

/// Shared state of the running desktop app.
pub struct AppState {
    /// Keeps file logging alive; `None` if logging to file failed (SR-0101).
    log_guard: Mutex<Option<LogGuard>>,
    /// All project file operations (`sr-project`).
    projects: ProjectStore,
    /// Project folders open as tabs in this window.
    open: Mutex<Vec<String>>,
}

impl AppState {
    /// Wraps the log guard and the project store created during startup.
    pub fn new(log_guard: Option<LogGuard>, projects: ProjectStore) -> Self {
        Self {
            log_guard: Mutex::new(log_guard),
            projects,
            open: Mutex::new(Vec::new()),
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

    /// The project store.
    pub fn projects(&self) -> &ProjectStore {
        &self.projects
    }

    /// Remembers that a project is open in this window.
    pub fn mark_open(&self, dir: &str) {
        if let Ok(mut open) = self.open.lock()
            && !open.iter().any(|d| d == dir)
        {
            open.push(dir.to_owned());
        }
    }

    /// Forgets a project whose tab was closed.
    pub fn mark_closed(&self, dir: &str) {
        if let Ok(mut open) = self.open.lock() {
            open.retain(|d| d != dir);
        }
    }

    /// App exit: release the lock of every open project. The tabs stay in
    /// the recent-projects config, so they come back at the next start.
    pub fn release_all(&self) {
        let dirs = self
            .open
            .lock()
            .map(|open| open.clone())
            .unwrap_or_default();
        for dir in dirs {
            self.projects.release(Path::new(&dir));
        }
    }
}
