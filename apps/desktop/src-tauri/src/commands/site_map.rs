//! Site map commands (left panel of the editor). Logic: `crates/sr-analyzer`.

use std::path::Path;

use sr_analyzer::{AnalyzeOptions, SiteMapEvent, SiteMapJob};
use sr_ipc::{AppError, Flow};
use sr_project::CACHE_DIR;
use tauri::State;
use tauri::ipc::Channel;

use crate::state::AppState;

/// Starts reading the site map of `flow` for target `target` (or answers from
/// the project cache unless `fresh`). Returns the job id at once; events
/// (progress, stale, done, failed) arrive on `events`. Never fails itself:
/// problems arrive as a `failed` event.
#[tauri::command]
pub async fn start_site_map(
    state: State<'_, AppState>,
    dir: String,
    flow: Flow,
    target: String,
    fresh: bool,
    events: Channel<SiteMapEvent>,
) -> Result<u64, AppError> {
    let (job, cancel) = state.jobs().start();
    let jobs = state.jobs_handle();
    std::thread::spawn(move || {
        let project = Path::new(&dir);
        let cache_root = project.join(CACHE_DIR);
        let request = SiteMapJob {
            flow: &flow,
            project_dir: project,
            cache_root: &cache_root,
            target_id: &target,
            fresh,
        };
        sr_analyzer::site_map_job(
            &request,
            &AnalyzeOptions::default(),
            &cancel,
            &mut |event| {
                // The tab may be gone already; nothing to do then.
                let _ = events.send(event);
            },
        );
        jobs.finish(job);
    });
    Ok(job)
}

/// Asks a running job to stop (it then reports SR-0004). Unknown ids are ignored.
#[tauri::command]
pub async fn cancel_job(state: State<'_, AppState>, job: u64) -> Result<(), AppError> {
    state.jobs().cancel(job);
    Ok(())
}

/// Deletes the cached site maps of the project at `dir`. Errors: SR-1002.
#[tauri::command]
pub async fn clear_cache(dir: String) -> Result<(), AppError> {
    sr_analyzer::clear_cache(&Path::new(&dir).join(CACHE_DIR))
}
