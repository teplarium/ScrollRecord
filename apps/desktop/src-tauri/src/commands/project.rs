//! Project commands: start screen data, create / open / save / close a
//! project, import / export a scenario file (HANDOFF FR-PRJ-01…04).
//!
//! Thin glue over `sr-project::ProjectStore`; every rule lives and is tested
//! there. All commands are `async`: Tauri runs non-async commands on the main
//! thread, which would freeze the window during file IO (ADR 0006 §4).
//! A project is identified by its folder (`dir`), because several projects
//! can be open at once as tabs (HANDOFF D-38).

use std::path::Path;

use sr_ipc::{AppError, Flow, ImportedFlow, NewProject, OpenedProject, ProjectHome};
use tauri::State;

use crate::state::AppState;

/// Recent projects, tabs to restore, suggested location for a new project.
#[tauri::command]
pub async fn project_home(state: State<'_, AppState>) -> Result<ProjectHome, AppError> {
    Ok(state.projects().home())
}

/// Creates a project folder and opens it. Errors: SR-1006, SR-1002, and
/// SR-1001/SR-2001/SR-2003 for a scenario file to import.
#[tauri::command]
pub async fn create_project(
    state: State<'_, AppState>,
    request: NewProject,
) -> Result<OpenedProject, AppError> {
    let project = state.projects().create(&request)?;
    state.mark_open(&project.dir);
    Ok(project)
}

/// Opens a project folder. `force` opens it even if another window holds
/// it. Errors: SR-1003, SR-1004, SR-1005, SR-2001, SR-2003.
#[tauri::command]
pub async fn open_project(
    state: State<'_, AppState>,
    dir: String,
    force: bool,
) -> Result<OpenedProject, AppError> {
    let project = state.projects().open(Path::new(&dir), force)?;
    state.mark_open(&project.dir);
    Ok(project)
}

/// Saves the scenario and layout (autosave). Errors: SR-1003, SR-1002.
#[tauri::command]
pub async fn save_project(
    state: State<'_, AppState>,
    dir: String,
    flow: Flow,
    layout: serde_json::Value,
) -> Result<(), AppError> {
    state.projects().save(Path::new(&dir), &flow, &layout)
}

/// Closes a project tab: releases its lock, removes it from the open tabs.
#[tauri::command]
pub async fn close_project(state: State<'_, AppState>, dir: String) -> Result<(), AppError> {
    state.projects().close(Path::new(&dir));
    state.mark_closed(&dir);
    Ok(())
}

/// Removes a project from the recent list (the folder is not touched).
#[tauri::command]
pub async fn forget_recent(state: State<'_, AppState>, dir: String) -> Result<(), AppError> {
    state.projects().forget(Path::new(&dir));
    Ok(())
}

/// Reads a scenario file to import into the open project `dir`. Nothing is
/// written: the editor puts it into its undo history. Errors: SR-1001,
/// SR-2001, SR-2003.
#[tauri::command]
pub async fn import_flow(
    state: State<'_, AppState>,
    dir: String,
    file: String,
) -> Result<ImportedFlow, AppError> {
    state
        .projects()
        .import_flow(Path::new(&dir), Path::new(&file))
}

/// How a picked site folder or address is written into the project's
/// scenario (`meta.site`), e.g. after "Locate site folder…". `null` for an
/// empty input.
#[tauri::command]
pub async fn portable_site(
    state: State<'_, AppState>,
    dir: String,
    site: String,
) -> Result<Option<String>, AppError> {
    Ok(state.projects().portable_site(Path::new(&dir), &site))
}

/// Writes the project's scenario as one file with the layout embedded.
/// Errors: SR-1003, SR-2001, SR-1002.
#[tauri::command]
pub async fn export_flow(
    state: State<'_, AppState>,
    dir: String,
    dest: String,
) -> Result<(), AppError> {
    state
        .projects()
        .export_flow(Path::new(&dir), Path::new(&dest))
}
