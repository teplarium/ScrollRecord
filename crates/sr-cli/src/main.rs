//! # sr-cli — the `scrollrecord` command
//!
//! Purpose: headless access to everything the desktop app can do
//!          (HANDOFF NFR-09). Phase 0 ships `info` and `errors`; Phase 1 adds
//!          `render`, `validate`, `digest`; Phase 5 adds `export-skills`.
//! Solves:  bugs can be reproduced and fixes verified without the UI — by a
//!          developer, by CI, or by an AI assistant running a command.
//! Rules:   no business logic here. Each subcommand parses arguments, calls a
//!          library crate and prints the result (human text or `--json`).

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use sr_core::ErrorCode;
use sr_diagnostics::{LogOptions, init_logging};
use sr_ipc::AppInfo;

// ── Command-line definition ─────────────────────────────────────────────────
// Why: clap derives parsing, `--help` text and validation from these structs,
//      so the help output is always in sync with the code.

/// ScrollRecord — record smooth website scroll videos.
#[derive(Debug, Parser)]
#[command(name = "scrollrecord", version, about, propagate_version = true)]
struct Cli {
    /// Also write logs to files in this folder.
    #[arg(long, global = true, value_name = "DIR")]
    log_dir: Option<PathBuf>,

    /// Show debug logs (same as SR_LOG=debug).
    #[arg(short, long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Print versions and environment (paste this into bug reports).
    Info {
        /// Print machine-readable JSON instead of text.
        #[arg(long)]
        json: bool,
    },
    /// List every error code with a one-line summary.
    Errors {
        /// Print machine-readable JSON instead of text.
        #[arg(long)]
        json: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Keep the guard alive until main returns so file logs are flushed.
    let _log_guard = init_logging(LogOptions {
        log_dir: cli.log_dir.clone(),
        default_level: Some(if cli.verbose { "debug" } else { "warn" }.to_owned()),
    })?;

    match cli.command {
        Command::Info { json } => print_info(json, cli.log_dir),
        Command::Errors { json } => print_errors(json),
    }
}

// ── Subcommand handlers ─────────────────────────────────────────────────────

/// `scrollrecord info` — same data the desktop app shows in its footer.
fn print_info(json: bool, log_dir: Option<PathBuf>) -> Result<()> {
    let info = AppInfo::current(
        env!("CARGO_PKG_VERSION"),
        log_dir.map(|dir| dir.display().to_string()),
    );
    if json {
        println!("{}", serde_json::to_string_pretty(&info)?);
    } else {
        println!("{} {}", info.app_name, info.app_version);
        println!("core     {}", info.core_version);
        println!("platform {}-{}", info.os, info.arch);
        if let Some(dir) = &info.log_dir {
            println!("logs     {dir}");
        }
    }
    Ok(())
}

/// `scrollrecord errors` — the catalog from `sr-core` (details: docs/ERRORS.md).
fn print_errors(json: bool) -> Result<()> {
    if json {
        let list: Vec<_> = ErrorCode::ALL
            .iter()
            .map(|c| serde_json::json!({ "code": c.as_str(), "summary": c.summary() }))
            .collect();
        println!("{}", serde_json::to_string_pretty(&list)?);
    } else {
        for code in ErrorCode::ALL {
            println!("{}  {}", code.as_str(), code.summary());
        }
        println!("\nDetails and fixes: docs/ERRORS.md");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn parses_subcommands_and_global_flags() {
        let cli = Cli::try_parse_from(["scrollrecord", "-v", "errors", "--json"]).unwrap();
        assert!(cli.verbose);
        assert!(matches!(cli.command, Command::Errors { json: true }));
    }
}
