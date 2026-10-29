//! Small file helpers shared by the project modules.
//!
//! Purpose: one place for "save without ever leaving a broken file", reading
//!          text with a proper error code, and lexical path clean-up.
//! Solves:  a crash or power cut in the middle of a save must never leave a
//!          half-written `flow.srflow.json` (HANDOFF FR-PRJ-02, ADR 0006 §1).

use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use sr_core::{AppError, AppResult, ErrorCode};

// ── Atomic write ────────────────────────────────────────────────────────────
// Why: writing straight into the real file leaves it half-written if the app
//      dies mid-write. Instead the new content goes to `name.tmp` (flushed to
//      disk), the current file is *copied* to `name.bak` (so the real file
//      exists at every moment), and the temp file is renamed over the real one.
//      `std::fs::rename` replaces an existing file on Windows and Linux.

/// Writes `bytes` to `path` so that `path` is always either the old or the
/// new complete content. With `keep_backup` the previous version is kept as
/// `<name>.bak` (project files; not for exports into the user's folders).
///
/// Errors: `SR-1002` (with `path`) if any step fails.
pub(crate) fn write_atomic(path: &Path, bytes: &[u8], keep_backup: bool) -> AppResult<()> {
    let tmp = with_suffix(path, ".tmp");
    let result = (|| {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        if keep_backup && path.exists() {
            fs::copy(path, with_suffix(path, ".bak"))?;
        }
        fs::rename(&tmp, path)
    })();
    result.map_err(|err| {
        let _ = fs::remove_file(&tmp);
        write_error(path, &err)
    })
}

/// Serializes `value` as pretty JSON (2-space indent, trailing newline) and
/// writes it with [`write_atomic`].
pub(crate) fn write_json<T: Serialize>(path: &Path, value: &T) -> AppResult<()> {
    let mut text = serde_json::to_string_pretty(value).map_err(|err| {
        AppError::new(
            ErrorCode::Internal,
            format!("serialize {}: {err}", path.display()),
        )
    })?;
    text.push('\n');
    write_atomic(path, text.as_bytes(), true)
}

/// Reads a UTF-8 text file. Errors: `SR-1001` (with `path`).
pub(crate) fn read_text(path: &Path) -> AppResult<String> {
    fs::read_to_string(path).map_err(|err| {
        AppError::new(
            ErrorCode::FileReadFailed,
            format!("read {}: {err}", path.display()),
        )
        .with_param("path", path.display())
    })
}

/// `SR-1002` for a failed write or folder creation at `path`.
pub(crate) fn write_error(path: &Path, err: &std::io::Error) -> AppError {
    AppError::new(
        ErrorCode::FileWriteFailed,
        format!("write {}: {err}", path.display()),
    )
    .with_param("path", path.display())
}

/// `path` with `suffix` appended to its file name (`a.json` → `a.json.bak`).
pub(crate) fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(suffix);
    path.with_file_name(name)
}

/// Milliseconds since 1970 (0 if the clock is before 1970).
pub(crate) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

// ── Paths ───────────────────────────────────────────────────────────────────
// Why: `Path::join("../site")` keeps the `..` (`C:\a\project\..\site`), so
//      "is this folder inside or next to the project?" would give wrong
//      answers. Lexical clean-up removes `.` and `..` without touching the
//      disk (the folder may not exist yet, and `canonicalize` adds `\\?\` on
//      Windows).

/// Removes `.` and resolves `..` components without touching the disk.
pub(crate) fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// True for `http://…` / `https://…` site addresses (as opposed to folders).
pub(crate) fn is_url(site: &str) -> bool {
    let lower = site.trim().to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

/// A key for comparing two folder paths as the OS does (case-insensitive and
/// either slash on Windows; no trailing separator).
pub(crate) fn path_key(path: &str) -> String {
    let unified = if cfg!(windows) {
        path.replace('/', "\\").to_lowercase()
    } else {
        path.to_owned()
    };
    unified.trim_end_matches(['/', '\\']).to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_resolves_parent_components() {
        let p = normalize(Path::new("/a/project/../site/./x"));
        assert_eq!(p, PathBuf::from("/a/site/x"));
    }

    #[test]
    fn suffix_is_appended_to_the_file_name() {
        assert_eq!(
            with_suffix(Path::new("dir/flow.srflow.json"), ".bak"),
            PathBuf::from("dir/flow.srflow.json.bak")
        );
    }

    #[test]
    fn urls_are_told_apart_from_folders() {
        assert!(is_url("HTTP://localhost:5173"));
        assert!(!is_url("../site"));
    }
}
