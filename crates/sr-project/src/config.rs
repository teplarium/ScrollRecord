//! `projects.json` in the app's config folder: recent projects, the last
//! location used for new projects, and the tabs that were open.
//!
//! Purpose: the start screen's recent list (FR-PRJ-03), the "location" field
//!          of the new-project form (HANDOFF D-37) and restoring the open
//!          tabs at the next start (D-39).
//! Rules:   kept by Rust, not in the webview's storage (ADR 0006 §1). A
//!          missing or broken file is never an error: the app starts with an
//!          empty list and logs a warning.

use std::path::Path;

use serde::{Deserialize, Serialize};
use sr_core::AppResult;

use crate::fsio;

/// File name inside the app's config folder.
pub const CONFIG_FILE: &str = "projects.json";
/// How many recent projects are remembered.
pub const MAX_RECENT: usize = 10;

/// Contents of `projects.json`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectsConfig {
    /// Most recently opened first.
    pub recent: Vec<RecentEntry>,
    /// Folder where the last new project was created.
    pub last_location: Option<String>,
    /// Project folders open as tabs, in tab order.
    pub open_tabs: Vec<String>,
}

/// One recent project.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecentEntry {
    /// Project folder.
    pub path: String,
    /// Project name at the time it was last opened.
    pub name: String,
    /// When it was last opened, milliseconds since 1970.
    pub opened_ms: u64,
}

impl ProjectsConfig {
    /// Reads the config; missing or broken → empty config (logged).
    pub fn load(config_dir: &Path) -> Self {
        let path = config_dir.join(CONFIG_FILE);
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        serde_json::from_str(&text).unwrap_or_else(|err| {
            tracing::warn!(path = %path.display(), error = %err, "projects.json is broken, starting with an empty list");
            Self::default()
        })
    }

    /// Writes the config atomically, creating the config folder if needed.
    ///
    /// Errors: `SR-1002`.
    pub fn save(&self, config_dir: &Path) -> AppResult<()> {
        std::fs::create_dir_all(config_dir).map_err(|err| fsio::write_error(config_dir, &err))?;
        fsio::write_json(&config_dir.join(CONFIG_FILE), self)
    }

    /// Records an opened project: first in the recent list (no duplicates,
    /// at most [`MAX_RECENT`]) and present among the open tabs.
    pub fn note_opened(&mut self, dir: &str, name: &str) {
        let key = fsio::path_key(dir);
        self.recent.retain(|r| fsio::path_key(&r.path) != key);
        self.recent.insert(
            0,
            RecentEntry {
                path: dir.to_owned(),
                name: name.to_owned(),
                opened_ms: fsio::now_ms(),
            },
        );
        self.recent.truncate(MAX_RECENT);
        if !self.open_tabs.iter().any(|t| fsio::path_key(t) == key) {
            self.open_tabs.push(dir.to_owned());
        }
    }

    /// Records a closed tab (the project stays in the recent list).
    pub fn note_closed(&mut self, dir: &str) {
        let key = fsio::path_key(dir);
        self.open_tabs.retain(|t| fsio::path_key(t) != key);
    }

    /// Removes a project from the recent list and the open tabs.
    pub fn forget(&mut self, dir: &str) {
        let key = fsio::path_key(dir);
        self.recent.retain(|r| fsio::path_key(&r.path) != key);
        self.open_tabs.retain(|t| fsio::path_key(t) != key);
    }
}
