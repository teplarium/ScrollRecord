//! `project.json` — what makes a folder a ScrollRecord project.
//!
//! Purpose: the project's name, format version, timestamps, which scenario
//!          file it uses and project-wide render settings (ADR 0006 §1).
//! Solves:  recognising a project folder, and refusing (with a clear message)
//!          a project saved by a newer app instead of misreading it.
//! Rules:   no machine-specific paths in here, so a project folder can be
//!          copied to another computer ("Export project", HANDOFF §14.1).
//!          `flow` names the scenario file, which leaves room for several
//!          scenarios per project later (HANDOFF D-39).

use std::path::Path;

use serde::{Deserialize, Serialize};
use sr_core::{AppError, AppResult, ErrorCode};
use ts_rs::TS;

use crate::fsio;

/// The project format this build reads and writes.
pub const PROJECT_FORMAT: u32 = 1;
/// File that marks a project folder.
pub const MANIFEST_FILE: &str = "project.json";
/// Default scenario file inside a project.
pub const FLOW_FILE: &str = "flow.srflow.json";
/// Editor-only data: node positions, canvas viewport.
pub const LAYOUT_FILE: &str = "layout.json";
/// Page snapshots and other rebuildable data.
pub const CACHE_DIR: &str = "cache";
/// Rendered videos.
pub const RENDERS_DIR: &str = "renders";

/// Contents of `project.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ProjectManifest {
    /// Must be ≤ [`PROJECT_FORMAT`].
    pub project_format: u32,
    /// Project name shown in tabs and the recent list.
    pub name: String,
    /// ScrollRecord version that last saved the project.
    pub app_version: String,
    /// Creation time, milliseconds since 1970.
    #[ts(type = "number")]
    pub created_ms: u64,
    /// Last save, milliseconds since 1970.
    #[ts(type = "number")]
    pub modified_ms: u64,
    /// Scenario file, relative to the project folder.
    #[serde(default = "default_flow_file")]
    pub flow: String,
    /// Project-wide render settings.
    #[serde(default)]
    pub render: RenderSettings,
}

/// Render settings stored with the project.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RenderSettings {
    /// Output file name template (HANDOFF FR-RND-03).
    pub file_name: String,
}

impl Default for RenderSettings {
    fn default() -> Self {
        Self {
            file_name: "{project}_{target}_{date}".to_owned(),
        }
    }
}

fn default_flow_file() -> String {
    FLOW_FILE.to_owned()
}

impl ProjectManifest {
    /// A fresh manifest for a new project.
    pub fn new(name: &str, app_version: &str) -> Self {
        let now = fsio::now_ms();
        Self {
            project_format: PROJECT_FORMAT,
            name: name.to_owned(),
            app_version: app_version.to_owned(),
            created_ms: now,
            modified_ms: now,
            flow: default_flow_file(),
            render: RenderSettings::default(),
        }
    }

    /// Reads `project.json` from a project folder.
    ///
    /// Errors: `SR-1003` if the file is missing or broken, `SR-1004` if it was
    /// written by a newer app.
    pub fn read(dir: &Path) -> AppResult<Self> {
        let path = dir.join(MANIFEST_FILE);
        if !path.is_file() {
            return Err(not_a_project(dir, "no project.json"));
        }
        let text = fsio::read_text(&path)?;
        // Check the version first, so a future file gets "update the app"
        // instead of a confusing shape error (same rule as `Flow::from_json`).
        let value: serde_json::Value =
            serde_json::from_str(&text).map_err(|err| not_a_project(dir, &err.to_string()))?;
        if let Some(v) = value
            .get("project_format")
            .and_then(serde_json::Value::as_u64)
            && v > u64::from(PROJECT_FORMAT)
        {
            return Err(AppError::new(
                ErrorCode::ProjectVersionUnsupported,
                format!("project uses project_format {v}, this build reads {PROJECT_FORMAT}"),
            )
            .with_param("version", v)
            .with_param("supported", PROJECT_FORMAT));
        }
        serde_json::from_value(value).map_err(|err| not_a_project(dir, &err.to_string()))
    }

    /// Writes `project.json` atomically.
    pub fn write(&self, dir: &Path) -> AppResult<()> {
        fsio::write_json(&dir.join(MANIFEST_FILE), self)
    }
}

fn not_a_project(dir: &Path, detail: &str) -> AppError {
    AppError::new(
        ErrorCode::NotAProject,
        format!("{} is not a ScrollRecord project: {detail}", dir.display()),
    )
    .with_param("path", dir.display())
    .with_param("detail", detail)
}
