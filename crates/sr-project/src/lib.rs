//! # sr-project — the project folder
//!
//! Purpose: create, open, save and close ScrollRecord projects; import and
//!          export a scenario as one file; remember recent projects and open
//!          tabs (HANDOFF FR-PRJ-01…04, FR-CNV-16, D-36…D-39, ADR 0006 §1).
//! Solves:  all project file IO lives here, so the desktop shell stays thin
//!          (AGENTS §3) and every rule is tested without a window.
//! Layout of a project folder:
//! ```text
//! <Project name>/
//!   project.json        name, format version, render settings (manifest.rs)
//!   flow.srflow.json    the scenario — exactly the CLI format
//!   layout.json         editor-only: node positions, canvas viewport
//!   cache/              rebuildable page snapshots
//!   renders/            videos
//!   .scrollrecord.lock  "open in a window" (lock.rs)
//! ```
//! Rules:   every file is written atomically with a `.bak` of the previous
//!          version (fsio.rs); no machine-specific paths inside a project
//!          when avoidable (site.rs); several projects may be open at once
//!          (tabs), so nothing here keeps a "current project".
//! See also: `apps/desktop/src-tauri/src/commands/project.rs` (IPC),
//!          `docs/ERRORS.md#sr-1003` … `#sr-1006`.

mod config;
mod fsio;
mod lock;
mod manifest;
mod project;
mod site;

pub use config::{CONFIG_FILE, MAX_RECENT, ProjectsConfig, RecentEntry};
pub use lock::LOCK_FILE;
pub use manifest::{
    CACHE_DIR, FLOW_FILE, LAYOUT_FILE, MANIFEST_FILE, PROJECT_FORMAT, ProjectManifest, RENDERS_DIR,
    RenderSettings,
};
pub use project::{
    ImportedFlow, NewProject, OpenedProject, ProjectHome, ProjectStore, RecentProject,
};
