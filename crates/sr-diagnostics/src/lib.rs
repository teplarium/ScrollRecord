//! # sr-diagnostics
//!
//! Purpose: everything that helps find out *why* something broke — today the
//!          logging setup; from Phase 6 also the "Report a problem" bundle
//!          (logs + scenario + versions zipped together, HANDOFF §12).
//! Solves:  one place configures logging for both the desktop app and the CLI,
//!          so logs always have the same format and land in a known folder.
//! Rules:   never panics; failures come back as `AppError` with code
//!          `SR-0101` so the app can keep running without file logs.
//! See also: `docs/ERRORS.md#sr-0101`.

use std::path::{Path, PathBuf};

use sr_core::{AppError, AppResult, ErrorCode};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{Builder as RollingBuilder, Rotation};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer, fmt};

/// Environment variable that overrides the log level, e.g. `SR_LOG=debug`.
pub const LOG_ENV_VAR: &str = "SR_LOG";
/// File name prefix of log files: `scrollrecord.YYYY-MM-DD.log`.
pub const LOG_FILE_PREFIX: &str = "scrollrecord";
/// How many daily log files are kept before the oldest is deleted.
pub const MAX_LOG_FILES: usize = 14;

/// Options for [`init_logging`].
#[derive(Debug, Clone, Default)]
pub struct LogOptions {
    /// Folder for log files. `None` = log to the terminal (stderr) only.
    pub log_dir: Option<PathBuf>,
    /// Default level when `SR_LOG` is not set, e.g. `"info"` or `"debug"`.
    pub default_level: Option<String>,
}

/// Keeps the background log writer alive. Logs written right before the
/// program exits are flushed when this guard is dropped, so the owner (the
/// app or the CLI) must hold it for its whole lifetime.
#[must_use = "dropping the guard stops file logging"]
#[derive(Debug)]
pub struct LogGuard {
    _file_writer: Option<WorkerGuard>,
    log_dir: Option<PathBuf>,
}

impl LogGuard {
    /// Folder that receives the log files, if file logging is active.
    pub fn log_dir(&self) -> Option<&Path> {
        self.log_dir.as_deref()
    }
}

// ── Logging setup ───────────────────────────────────────────────────────────
// Why: a bug report is only useful with logs. Two outputs are configured:
//      the terminal (for developers) and daily-rotated files (for users, so
//      "Report a problem" can attach them). The level is controlled by SR_LOG
//      so a user can be asked to "run with SR_LOG=debug" without a rebuild.

/// Starts logging for the whole process. Call once, as early as possible.
pub fn init_logging(options: LogOptions) -> AppResult<LogGuard> {
    let default_level = options.default_level.as_deref().unwrap_or("info");
    let filter =
        || EnvFilter::try_from_env(LOG_ENV_VAR).unwrap_or_else(|_| EnvFilter::new(default_level));

    let terminal_layer = fmt::layer()
        .with_writer(std::io::stderr)
        .with_filter(filter());

    // File layer is optional: if the folder cannot be created we still want
    // terminal logs, but the caller gets a clear SR-0101 error to report.
    let (file_layer, file_guard) = match &options.log_dir {
        Some(dir) => {
            let (writer, guard) = tracing_appender::non_blocking(open_rolling_file(dir)?);
            let layer = fmt::layer()
                .with_ansi(false)
                .with_writer(writer)
                .with_filter(filter());
            (Some(layer), Some(guard))
        }
        None => (None, None),
    };

    tracing_subscriber::registry()
        .with(terminal_layer)
        .with(file_layer)
        .try_init()
        .map_err(|err| {
            AppError::new(
                ErrorCode::LoggingInitFailed,
                format!("logger already set: {err}"),
            )
            .with_param("reason", "already-initialised")
        })?;

    tracing::info!(log_dir = ?options.log_dir, "logging started");
    Ok(LogGuard {
        _file_writer: file_guard,
        log_dir: options.log_dir,
    })
}

/// Creates the log folder and a daily-rotating file appender inside it.
fn open_rolling_file(dir: &Path) -> AppResult<tracing_appender::rolling::RollingFileAppender> {
    std::fs::create_dir_all(dir).map_err(|err| {
        AppError::new(
            ErrorCode::LoggingInitFailed,
            format!("cannot create log dir {}: {err}", dir.display()),
        )
        .with_param("path", dir.display())
    })?;

    RollingBuilder::new()
        .rotation(Rotation::DAILY)
        .filename_prefix(LOG_FILE_PREFIX)
        .filename_suffix("log")
        .max_log_files(MAX_LOG_FILES)
        .build(dir)
        .map_err(|err| {
            AppError::new(
                ErrorCode::LoggingInitFailed,
                format!("cannot open log file in {}: {err}", dir.display()),
            )
            .with_param("path", dir.display())
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    // One test only: a process can have just one global logger.
    // Uses a folder with Cyrillic letters and spaces on purpose (HANDOFF NFR-02).
    #[test]
    fn writes_log_file_into_unicode_folder() {
        let dir = std::env::temp_dir()
            .join(format!("sr-diagnostics-test-{}", std::process::id()))
            .join("Загрузки с пробелом");
        let guard = init_logging(LogOptions {
            log_dir: Some(dir.clone()),
            default_level: None,
        })
        .unwrap();
        tracing::info!("hello from test");
        assert_eq!(guard.log_dir(), Some(dir.as_path()));
        drop(guard); // flush

        let files: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().collect();
        assert!(
            !files.is_empty(),
            "no log file created in {}",
            dir.display()
        );
        let content = std::fs::read_to_string(files[0].path()).unwrap();
        assert!(content.contains("hello from test"));

        // A second init must fail with SR-0101 instead of panicking.
        let second = init_logging(LogOptions::default()).unwrap_err();
        assert_eq!(second.code, ErrorCode::LoggingInitFailed);

        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }
}
