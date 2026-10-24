//! Finding a Chromium-based browser on this machine.
//!
//! Why: users already have Chrome or Edge (Edge ships with Windows 10/11), so
//! Phase 1 uses the installed browser instead of downloading one (automatic
//! download is planned for Phase 6). Order: explicit path → `SR_BROWSER`
//! environment variable → Chrome → Edge → Chromium.

use std::path::{Path, PathBuf};

use sr_core::{AppError, AppResult, ErrorCode};

/// Environment variable with a browser path that overrides auto-detection.
pub const BROWSER_ENV_VAR: &str = "SR_BROWSER";

/// Returns the browser executable to use.
pub fn find_browser(explicit: Option<&Path>) -> AppResult<PathBuf> {
    if let Some(path) = explicit {
        return check(path);
    }
    if let Some(path) = std::env::var_os(BROWSER_ENV_VAR).filter(|v| !v.is_empty()) {
        return check(Path::new(&path));
    }
    let candidates = candidates();
    candidates
        .iter()
        .find(|p| p.is_file())
        .cloned()
        .ok_or_else(|| {
            AppError::new(
                ErrorCode::BrowserNotFound,
                format!(
                    "no Chrome, Edge or Chromium found; looked in {} places",
                    candidates.len()
                ),
            )
        })
}

fn check(path: &Path) -> AppResult<PathBuf> {
    if path.is_file() {
        Ok(path.to_path_buf())
    } else {
        Err(AppError::new(
            ErrorCode::BrowserNotFound,
            format!("browser not found at {}", path.display()),
        )
        .with_param("path", path.display()))
    }
}

// ── Known install locations per OS ──────────────────────────────────────────

#[cfg(windows)]
fn candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let roots: Vec<PathBuf> = ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"]
        .iter()
        .filter_map(|var| std::env::var_os(var).map(PathBuf::from))
        .collect();
    for rel in [
        r"Google\Chrome\Application\chrome.exe",
        r"Microsoft\Edge\Application\msedge.exe",
        r"Chromium\Application\chrome.exe",
    ] {
        for root in &roots {
            out.push(root.join(rel));
        }
    }
    out.extend(on_path(&["chrome.exe", "msedge.exe"]));
    out
}

#[cfg(target_os = "macos")]
fn candidates() -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = [
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
        "/Applications/Chromium.app/Contents/MacOS/Chromium",
    ]
    .iter()
    .map(PathBuf::from)
    .collect();
    out.extend(on_path(&["google-chrome", "chromium"]));
    out
}

#[cfg(all(unix, not(target_os = "macos")))]
fn candidates() -> Vec<PathBuf> {
    let mut out = on_path(&[
        "google-chrome",
        "google-chrome-stable",
        "chromium",
        "chromium-browser",
        "microsoft-edge",
        "microsoft-edge-stable",
    ]);
    out.extend(
        [
            "/opt/google/chrome/chrome",
            "/snap/bin/chromium",
            "/opt/microsoft/msedge/msedge",
        ]
        .iter()
        .map(PathBuf::from),
    );
    out
}

/// Every `name` found in the folders of the `PATH` environment variable.
fn on_path(names: &[&str]) -> Vec<PathBuf> {
    let Some(path) = std::env::var_os("PATH") else {
        return Vec::new();
    };
    std::env::split_paths(&path)
        .flat_map(|dir| names.iter().map(move |n| dir.join(n)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_missing_path_is_a_clear_error() {
        let err = find_browser(Some(Path::new("/definitely/not/chrome"))).unwrap_err();
        assert_eq!(err.code, ErrorCode::BrowserNotFound);
        assert!(err.params.contains_key("path"));
    }

    #[test]
    fn candidate_list_is_never_empty() {
        assert!(!candidates().is_empty());
    }
}
