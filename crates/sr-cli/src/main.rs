//! # sr-cli — the `scrollrecord` command
//!
//! Purpose: headless access to everything the desktop app can do
//!          (HANDOFF NFR-09): `render`, `validate`, `info`, `errors`.
//!          Later phases add `digest` and `export-skills`.
//! Solves:  bugs can be reproduced and fixes verified without the UI — by a
//!          developer, by CI, or by an AI assistant running a command.
//! Rules:   no business logic here. Each subcommand (one file in `commands/`)
//!          parses arguments, calls a library crate and prints the result.

mod commands;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use sr_core::AppError;
use sr_diagnostics::{LogOptions, init_logging};

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
    /// Render videos from a scenario file (*.srflow.json).
    Render(commands::render::RenderArgs),
    /// Check a scenario file without rendering.
    Validate(commands::validate::ValidateArgs),
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

fn main() -> ExitCode {
    let cli = Cli::parse();

    // Keep the guard alive until main returns so file logs are flushed.
    let _log_guard = match init_logging(LogOptions {
        log_dir: cli.log_dir.clone(),
        default_level: Some(if cli.verbose { "debug" } else { "warn" }.to_owned()),
    }) {
        Ok(guard) => Some(guard),
        Err(err) => {
            report(&err);
            None
        }
    };

    let result = match cli.command {
        Command::Render(args) => commands::render::run(args),
        Command::Validate(args) => commands::validate::run(&args),
        Command::Info { json } => commands::info::run(json, cli.log_dir),
        Command::Errors { json } => commands::info::errors(json),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            report(&err);
            ExitCode::FAILURE
        }
    }
}

// ── Error output ────────────────────────────────────────────────────────────
// Why: every failure shows its SR code and where to read about it, so a user
//      can fix it or paste it into an issue (HANDOFF §12).
fn report(err: &AppError) {
    eprintln!("error[{}]: {}", err.code, err.code.summary());
    for line in err.message.lines() {
        eprintln!("  {line}");
    }
    eprintln!(
        "  → https://github.com/teplarium/ScrollRecord/blob/main/docs/ERRORS.md#{}",
        err.code.as_str().to_ascii_lowercase()
    );
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

        let cli = Cli::try_parse_from([
            "scrollrecord",
            "render",
            "a.srflow.json",
            "-t",
            "mobile",
            "-t",
            "desktop",
            "--best",
        ])
        .unwrap();
        let Command::Render(args) = cli.command else {
            panic!("render expected")
        };
        assert_eq!(args.target, ["mobile", "desktop"]);
        assert!(args.best);
    }
}
