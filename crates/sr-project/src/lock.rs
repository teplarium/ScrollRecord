//! `.scrollrecord.lock` — "this project is open in a ScrollRecord window".
//!
//! Purpose: warn before the same project is opened twice (ADR 0006 §3).
//! Solves:  two windows autosaving one project overwrite each other's work.
//! Rules:   the lock is a warning, not a wall: after a crash it is left
//!          behind, so the user can always choose "open anyway" (`force`).
//!          Projects open in *this* app (several tabs) carry this process's
//!          pid, which never counts as "locked".

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sr_core::{AppError, AppResult, ErrorCode};

use crate::fsio;

/// Lock file name inside the project folder.
pub const LOCK_FILE: &str = ".scrollrecord.lock";

#[derive(Debug, Serialize, Deserialize)]
struct LockInfo {
    /// Process id of the app that holds the project open.
    pid: u32,
    /// When it was opened, milliseconds since 1970.
    since_ms: u64,
}

/// Marks the project as open by this process.
///
/// Errors: `SR-1005` (with `pid`) if another process holds the lock and
/// `force` is false; `SR-1002` if the lock file can't be written.
pub(crate) fn acquire(dir: &Path, force: bool) -> AppResult<()> {
    let path = dir.join(LOCK_FILE);
    let me = std::process::id();
    // An unreadable or garbled lock is treated as stale: it can only come
    // from a crash or a hand edit, and blocking the user would be worse.
    if !force
        && let Ok(text) = fs::read_to_string(&path)
        && let Ok(other) = serde_json::from_str::<LockInfo>(&text)
        && other.pid != me
    {
        return Err(AppError::new(
            ErrorCode::ProjectLocked,
            format!("{} is locked by process {}", dir.display(), other.pid),
        )
        .with_param("path", dir.display())
        .with_param("pid", other.pid));
    }
    let info = LockInfo {
        pid: me,
        since_ms: fsio::now_ms(),
    };
    let text = serde_json::to_string(&info)
        .map_err(|err| AppError::new(ErrorCode::Internal, format!("serialize lock: {err}")))?;
    fs::write(&path, text).map_err(|err| fsio::write_error(&path, &err))
}

/// The process that has the project open, if any (this app included).
/// A garbled lock counts as none, like in [`acquire`].
pub(crate) fn holder(dir: &Path) -> Option<u32> {
    fs::read_to_string(dir.join(LOCK_FILE))
        .ok()
        .and_then(|text| serde_json::from_str::<LockInfo>(&text).ok())
        .map(|info| info.pid)
}

/// Removes the lock if this process holds it (a lock taken over by another
/// window after "open anyway" is left alone).
pub(crate) fn release(dir: &Path) {
    let path = dir.join(LOCK_FILE);
    let ours = fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str::<LockInfo>(&text).ok())
        .is_some_and(|info| info.pid == std::process::id());
    if ours && let Err(err) = fs::remove_file(&path) {
        tracing::warn!(path = %path.display(), error = %err, "could not remove project lock");
    }
}
