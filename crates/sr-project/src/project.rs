//! Project operations: create, open, save, close, import, export, home.
//!
//! Purpose: the functions behind the project IPC commands. Each takes the
//!          project folder explicitly, because several projects can be open
//!          at once as tabs (HANDOFF D-38).
//! Rules:   the UI sends and receives the scenario as plain JSON (`Flow`);
//!          the layout is free JSON owned by the canvas (step 3.2). Rust
//!          never keeps a copy of an open project, only its lock file.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use sr_core::flow::Flow;
use sr_core::{AppError, AppResult, ErrorCode};
use ts_rs::TS;

use crate::config::ProjectsConfig;
use crate::manifest::{CACHE_DIR, LAYOUT_FILE, MANIFEST_FILE, ProjectManifest, RENDERS_DIR};
use crate::{fsio, lock, site};

// ── IPC data ────────────────────────────────────────────────────────────────

/// What the new-project form sends (HANDOFF D-37, D-39).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct NewProject {
    /// Folder the project folder is created in.
    pub location: String,
    /// Project name; empty or missing → "Untitled N".
    #[serde(default)]
    #[ts(optional)]
    pub name: Option<String>,
    /// Translated "Untitled" for the default name (the UI knows the language).
    #[serde(default)]
    pub untitled_base: String,
    /// Site folder (absolute) or `http(s)://` address.
    #[serde(default)]
    #[ts(optional)]
    pub site: Option<String>,
    /// A `*.srflow.json` to start from instead of an empty canvas.
    #[serde(default)]
    #[ts(optional)]
    pub import_from: Option<String>,
}

/// An opened project, everything the editor tab needs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct OpenedProject {
    /// Project folder (the tab's identity; pass it back to every command).
    pub dir: String,
    /// `project.json`.
    pub manifest: ProjectManifest,
    /// The scenario, without `layout` (that is in the field below).
    pub flow: Flow,
    /// `layout.json` (free JSON owned by the canvas).
    pub layout: serde_json::Value,
    /// The scenario names a site folder that doesn't exist (moved or on
    /// another computer): the UI asks the user to locate it.
    pub site_missing: bool,
}

/// A scenario file read for importing into an open project.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ImportedFlow {
    /// The scenario, `meta.site` rewritten for the project folder.
    pub flow: Flow,
    /// Its embedded layout, or `{}`.
    pub layout: serde_json::Value,
}

/// What the start screen shows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ProjectHome {
    /// Recently opened projects, newest first.
    pub recent: Vec<RecentProject>,
    /// Project folders that were open as tabs (still existing ones only).
    pub open_tabs: Vec<String>,
    /// Pre-filled location for a new project: the last one used, else
    /// `Documents/ScrollRecord`.
    pub suggested_location: String,
}

/// One entry of the recent list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct RecentProject {
    /// Project folder.
    pub path: String,
    /// Name when last opened.
    pub name: String,
    /// When it was last opened, milliseconds since 1970.
    #[ts(type = "number")]
    pub opened_ms: u64,
    /// False if the folder is gone (shown greyed out, removable).
    pub exists: bool,
}

// ── The store ───────────────────────────────────────────────────────────────

/// Entry point for all project operations of one app instance.
///
/// Holds only where the app's config lives and defaults; the desktop shell
/// creates one at startup (paths come from Tauri, so this crate has no
/// Tauri dependency).
pub struct ProjectStore {
    config_dir: PathBuf,
    default_location: PathBuf,
    app_version: String,
    // Why: commands run concurrently (async IPC); read-modify-write of
    //      `projects.json` must not lose an update.
    config_guard: Mutex<()>,
}

impl ProjectStore {
    /// `config_dir`: the app's config folder (for `projects.json`).
    /// `default_location`: first suggestion for new projects
    /// (`Documents/ScrollRecord`). `app_version`: written into `project.json`.
    pub fn new(config_dir: PathBuf, default_location: PathBuf, app_version: &str) -> Self {
        Self {
            config_dir,
            default_location,
            app_version: app_version.to_owned(),
            config_guard: Mutex::new(()),
        }
    }

    /// Recent projects, tabs to restore and the suggested location.
    pub fn home(&self) -> ProjectHome {
        let config = ProjectsConfig::load(&self.config_dir);
        let is_project = |dir: &str| Path::new(dir).join(MANIFEST_FILE).is_file();
        ProjectHome {
            recent: config
                .recent
                .iter()
                .map(|r| RecentProject {
                    path: r.path.clone(),
                    name: r.name.clone(),
                    opened_ms: r.opened_ms,
                    exists: is_project(&r.path),
                })
                .collect(),
            open_tabs: config
                .open_tabs
                .into_iter()
                .filter(|dir| is_project(dir))
                .collect(),
            suggested_location: config
                .last_location
                .unwrap_or_else(|| self.default_location.display().to_string()),
        }
    }

    /// Creates a project folder and opens it.
    ///
    /// Errors: `SR-1006` if the named folder exists, `SR-1002` if folders or
    /// files can't be written, `SR-1001`/`SR-2001`/`SR-2003` for a broken
    /// file to import.
    pub fn create(&self, req: &NewProject) -> AppResult<OpenedProject> {
        let location = PathBuf::from(req.location.trim());
        fs::create_dir_all(&location).map_err(|err| fsio::write_error(&location, &err))?;

        // ── Name and folder ──
        // A typed name is cleaned for the file system; an empty one becomes
        // "Untitled N" with the smallest free N (owner's answer, D-37).
        let typed = req.name.as_deref().map(folder_name).unwrap_or_default();
        let name = if typed.is_empty() {
            untitled_name(&location, req.untitled_base.trim())
        } else {
            typed
        };
        let dir = location.join(&name);
        if dir.exists() {
            return Err(AppError::new(
                ErrorCode::ProjectExists,
                format!("{} already exists", dir.display()),
            )
            .with_param("path", dir.display())
            .with_param("name", &name));
        }
        fs::create_dir(&dir).map_err(|err| fsio::write_error(&dir, &err))?;

        // ── Contents ──
        // If anything fails after the folder exists, remove the half-made
        // project (it was created by this call, nothing else is in it).
        let written = self.write_new(&dir, &location, &name, req);
        if let Err(err) = written {
            let _ = fs::remove_dir_all(&dir);
            return Err(err);
        }
        self.update_config(|c| c.last_location = Some(location.display().to_string()));
        self.open(&dir, false)
    }

    fn write_new(
        &self,
        dir: &Path,
        location: &Path,
        name: &str,
        req: &NewProject,
    ) -> AppResult<()> {
        for sub in [CACHE_DIR, RENDERS_DIR] {
            let path = dir.join(sub);
            fs::create_dir(&path).map_err(|err| fsio::write_error(&path, &err))?;
        }
        let (mut flow, layout) = match req.import_from.as_deref() {
            Some(file) if !file.trim().is_empty() => {
                let imported = read_flow_file(Path::new(file.trim()), dir)?;
                (imported.flow, imported.layout)
            }
            _ => (blank_flow()?, empty_layout()),
        };
        flow.meta.name = name.to_owned();
        if let Some(chosen) = req.site.as_deref().filter(|s| !s.trim().is_empty()) {
            flow.meta.site = site::rebase(Some(chosen), location, dir);
        }
        let manifest = ProjectManifest::new(name, &self.app_version);
        write_contents(dir, &manifest, &flow, &layout)?;
        manifest.write(dir)
    }

    /// Opens a project folder: reads it, takes the lock, adds it to the
    /// recent list and the open tabs.
    ///
    /// Errors: `SR-1003`/`SR-1004` for the folder, `SR-2001`/`SR-2003` for
    /// the scenario, `SR-1005` if another window holds it and `force` is false.
    pub fn open(&self, dir: &Path, force: bool) -> AppResult<OpenedProject> {
        let manifest = ProjectManifest::read(dir)?;
        let (flow, layout) = read_contents(dir, &manifest)?;
        let site_missing =
            site::folder(flow.meta.site.as_deref(), dir).is_some_and(|folder| !folder.is_dir());
        lock::acquire(dir, force)?;
        let shown = dir.display().to_string();
        self.update_config(|c| c.note_opened(&shown, &manifest.name));
        Ok(OpenedProject {
            dir: shown,
            manifest,
            flow,
            layout,
            site_missing,
        })
    }

    /// Saves the scenario and layout (autosave, Ctrl+S).
    ///
    /// Errors: `SR-1003` if the folder is no longer a project, `SR-1002`.
    pub fn save(&self, dir: &Path, flow: &Flow, layout: &serde_json::Value) -> AppResult<()> {
        let mut manifest = ProjectManifest::read(dir)?;
        manifest.modified_ms = fsio::now_ms();
        manifest.app_version.clone_from(&self.app_version);
        write_contents(dir, &manifest, flow, layout)?;
        manifest.write(dir)
    }

    /// Closes a tab: releases the lock and removes it from the open tabs.
    pub fn close(&self, dir: &Path) {
        lock::release(dir);
        let shown = dir.display().to_string();
        self.update_config(|c| c.note_closed(&shown));
    }

    /// Releases the lock but keeps the tab for the next start (app exit).
    pub fn release(&self, dir: &Path) {
        lock::release(dir);
    }

    /// Removes a project from the recent list (the folder is not touched).
    pub fn forget(&self, dir: &Path) {
        let shown = dir.display().to_string();
        self.update_config(|c| c.forget(&shown));
    }

    /// How a site folder or address picked by the user is written into the
    /// scenario of the project at `dir` (relative when inside or next to the
    /// project, else absolute; URLs unchanged). The UI puts the result into
    /// `flow.meta.site` itself, so the change is part of its undo history.
    pub fn portable_site(&self, dir: &Path, site: &str) -> Option<String> {
        site::rebase(Some(site), dir, dir)
    }

    /// Reads a scenario file for importing into the project at `dir`
    /// (nothing is written: the UI puts it into its undo history).
    ///
    /// Errors: `SR-1001`, `SR-2001`, `SR-2003`.
    pub fn import_flow(&self, dir: &Path, file: &Path) -> AppResult<ImportedFlow> {
        read_flow_file(file, dir)
    }

    /// Writes the project's scenario as one file with the layout embedded,
    /// `meta.site` rewritten for the file's folder (FR-PRJ-04).
    ///
    /// Errors: `SR-1003`, `SR-2001`, `SR-1002`.
    pub fn export_flow(&self, dir: &Path, dest: &Path) -> AppResult<()> {
        let manifest = ProjectManifest::read(dir)?;
        let (mut flow, layout) = read_contents(dir, &manifest)?;
        let dest_dir = dest.parent().unwrap_or(Path::new("."));
        flow.meta.site = site::rebase(flow.meta.site.as_deref(), dir, dest_dir);
        if layout.as_object().is_none_or(|o| !o.is_empty()) {
            flow.layout = Some(layout);
        }
        fsio::write_atomic(dest, flow.to_json()?.as_bytes(), false)
    }

    fn update_config(&self, change: impl FnOnce(&mut ProjectsConfig)) {
        // A poisoned guard only means another command panicked mid-update;
        // the file on disk is still whole (atomic write), so carry on.
        let _guard = self
            .config_guard
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut config = ProjectsConfig::load(&self.config_dir);
        change(&mut config);
        // The recent list is a convenience: failing to save it must not
        // stop the user from opening or creating a project.
        if let Err(err) = config.save(&self.config_dir) {
            tracing::warn!(error = %err, "could not save the recent projects list");
        }
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Scenario of a new project (HANDOFF D-39): empty canvas, one default
/// target "Desktop 16:9 1080p", the home page.
fn blank_flow() -> AppResult<Flow> {
    let value = serde_json::json!({
        "format_version": sr_core::flow::FORMAT_VERSION,
        "meta": { "name": "" },
        "targets": [{ "id": "desktop", "viewport": "desktop-1920", "aspect": "16:9", "resolution": "1080p" }],
        "pages": [{ "id": "home", "url": "/" }],
        "nodes": [],
        "edges": []
    });
    serde_json::from_value(value)
        .map_err(|err| AppError::new(ErrorCode::Internal, format!("blank flow: {err}")))
}

fn empty_layout() -> serde_json::Value {
    serde_json::Value::Object(serde_json::Map::new())
}

/// Reads a `*.srflow.json`, moves its layout out and rebases `meta.site`
/// from the file's folder to `project_dir`.
fn read_flow_file(file: &Path, project_dir: &Path) -> AppResult<ImportedFlow> {
    let mut flow = Flow::from_json(&fsio::read_text(file)?)?;
    let layout = flow.layout.take().unwrap_or_else(empty_layout);
    let from = file.parent().unwrap_or(Path::new("."));
    flow.meta.site = site::rebase(flow.meta.site.as_deref(), from, project_dir);
    Ok(ImportedFlow { flow, layout })
}

/// Reads the scenario and `layout.json` of a project.
fn read_contents(dir: &Path, manifest: &ProjectManifest) -> AppResult<(Flow, serde_json::Value)> {
    let mut flow = Flow::from_json(&fsio::read_text(&dir.join(&manifest.flow))?)?;
    // A layout embedded by hand or by an AI tool is used only when there is
    // no layout.json yet; the editor always keeps it in layout.json.
    let embedded = flow.layout.take();
    let path = dir.join(LAYOUT_FILE);
    let layout = match fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_else(|err| {
            // Positions only: a broken file must not block the project.
            // The previous version is in layout.json.bak.
            tracing::warn!(path = %path.display(), error = %err, "layout.json is broken, auto-layout will be used");
            empty_layout()
        }),
        Err(_) => embedded.unwrap_or_else(empty_layout),
    };
    Ok((flow, layout))
}

/// Writes the scenario (without layout) and `layout.json`.
fn write_contents(
    dir: &Path,
    manifest: &ProjectManifest,
    flow: &Flow,
    layout: &serde_json::Value,
) -> AppResult<()> {
    let mut flow = flow.clone();
    flow.layout = None;
    fsio::write_atomic(&dir.join(&manifest.flow), flow.to_json()?.as_bytes(), true)?;
    fsio::write_json(&dir.join(LAYOUT_FILE), layout)
}

/// A project name made safe as a folder name on Windows and Linux:
/// characters Windows forbids become `-`, trailing dots and spaces go.
fn folder_name(name: &str) -> String {
    let cleaned: String = name
        .trim()
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') {
                '-'
            } else {
                c
            }
        })
        .collect();
    cleaned.trim_end_matches(['.', ' ']).to_owned()
}

/// "Untitled N" with the smallest N that has no folder in `location`.
fn untitled_name(location: &Path, base: &str) -> String {
    let base = match folder_name(base) {
        b if b.is_empty() => "Untitled".to_owned(),
        b => b,
    };
    (1..)
        .map(|n| format!("{base} {n}"))
        .find(|candidate| !location.join(candidate).exists())
        .unwrap_or(base)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_names_lose_forbidden_characters() {
        assert_eq!(folder_name(" Promo: v2/final. "), "Promo- v2-final");
        assert_eq!(folder_name("Лендинг «Люмен»"), "Лендинг «Люмен»");
    }

    #[test]
    fn blank_flow_is_an_empty_canvas_with_one_target() {
        let flow = blank_flow().unwrap();
        assert!(flow.nodes.is_empty() && flow.edges.is_empty());
        assert_eq!(flow.targets.len(), 1);
        assert_eq!(flow.targets[0].id, "desktop");
        assert_eq!(flow.pages[0].url, "/");
    }
}
