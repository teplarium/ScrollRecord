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
            app.manage(state::AppState::new(guard));
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
            commands::project::create_project,
            commands::project::open_project,
        ])
        .run(tauri::generate_context!());

    // `run` only returns an error if the window system could not start.
    if let Err(err) = result {
        tracing::error!(error = %err, "desktop app failed to start");
        eprintln!("ScrollRecord failed to start: {err}");
        std::process::exit(1);
    }
}
