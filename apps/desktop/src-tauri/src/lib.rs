//! # sr-desktop — Tauri shell
//!
//! Purpose: opens the window, starts logging and registers the IPC commands
//!          the React UI can call.
//! Solves:  connects the UI to the Rust core while staying *thin* — command
//!          handlers only translate between IPC and library crates, so a bug is
//!          either in the UI, in this glue, or in a library, never smeared
//!          across all three (HANDOFF §7.1).
//! Rules:   no business logic here. New features get a library crate first,
//!          then a one-function command in `commands/`.
//! See also: `apps/desktop/src/shared/ipc/commands.ts` (the TypeScript caller).

mod commands;
mod state;

use sr_diagnostics::{LogOptions, init_logging};
use tauri::Manager;

/// Builds and runs the desktop application. Blocks until the window closes.
pub fn run() {
    let result = tauri::Builder::default()
        // Native folder / file pickers for the start screen (ADR 0006 §1).
        .plugin(tauri_plugin_dialog::init())
        // ── Startup ─────────────────────────────────────────────────────────
        // Why: logs must start before anything else can fail, so even
        //      startup problems end up in the log file (HANDOFF NFR-07).
        //      A logging failure is reported but does not stop the app.
        .setup(|app| {
            let log_dir = app.path().app_log_dir().ok();
            let guard = match init_logging(LogOptions {
                log_dir,
                default_level: None,
            }) {
                Ok(guard) => Some(guard),
                Err(err) => {
                    eprintln!("{err}");
                    None
                }
            };
            // ── Project store ──
            // Paths come from Tauri so `sr-project` stays Tauri-free. If the
            // OS can't name a folder (very unusual), fall back to the home
            // or temp folder rather than refusing to start.
            let fallback = app
                .path()
                .home_dir()
                .unwrap_or_else(|_| std::env::temp_dir());
            let config_dir = app
                .path()
                .app_config_dir()
                .unwrap_or_else(|_| fallback.join(".scrollrecord"));
            let documents = app.path().document_dir().unwrap_or(fallback);
            let projects = sr_project::ProjectStore::new(
                config_dir,
                documents.join("ScrollRecord"),
                env!("CARGO_PKG_VERSION"),
            );
            app.manage(state::AppState::new(guard, projects));
            tracing::info!(
                version = env!("CARGO_PKG_VERSION"),
                "ScrollRecord desktop started"
            );
            Ok(())
        })
        // ── IPC commands ────────────────────────────────────────────────────
        // Every command the UI may call must be listed here. Keep this list in
        // sync with `commands/mod.rs` and the "IPC map" in PROJECT_MAP.md.
        .invoke_handler(tauri::generate_handler![
            commands::app::app_info,
            commands::project::project_home,
            commands::project::create_project,
            commands::project::open_project,
            commands::project::save_project,
            commands::project::close_project,
            commands::project::forget_recent,
            commands::project::import_flow,
            commands::project::export_flow,
            commands::project::portable_site,
        ])
        .build(tauri::generate_context!());

    // `build` only fails if the window system could not start.
    let app = match result {
        Ok(app) => app,
        Err(err) => {
            tracing::error!(error = %err, "desktop app failed to start");
            eprintln!("ScrollRecord failed to start: {err}");
            std::process::exit(1);
        }
    };

    // ── Shutdown ────────────────────────────────────────────────────────────
    // Why: open projects hold a lock file; leaving it behind would make the
    //      next start ask "open anyway?" for every restored tab.
    app.run(|handle, event| {
        if let tauri::RunEvent::Exit = event
            && let Some(state) = handle.try_state::<state::AppState>()
        {
            state.release_all();
        }
    });
}
