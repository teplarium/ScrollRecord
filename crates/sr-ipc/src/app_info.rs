//! `AppInfo` — what the UI shows in its footer and what bug reports include.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Build and environment information about the running app.
///
/// Why: when a user reports a problem, the exact versions and OS are the
/// first thing needed to reproduce it (HANDOFF §12).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    /// Product name shown to the user.
    pub app_name: String,
    /// Version of the desktop app (from the workspace `Cargo.toml`).
    pub app_version: String,
    /// Version of `sr-core`.
    pub core_version: String,
    /// Operating system family, e.g. `windows`, `linux`, `macos`.
    pub os: String,
    /// CPU architecture, e.g. `x86_64`, `aarch64`.
    pub arch: String,
    /// Folder that holds the log files, if logging to file is active.
    pub log_dir: Option<String>,
}

impl AppInfo {
    /// Collects info about the current process.
    /// `app_version` is passed in by the caller because only the binary crate
    /// knows its own package version.
    pub fn current(app_version: &str, log_dir: Option<String>) -> Self {
        Self {
            app_name: "ScrollRecord".to_owned(),
            app_version: app_version.to_owned(),
            core_version: sr_core::CORE_VERSION.to_owned(),
            os: std::env::consts::OS.to_owned(),
            arch: std::env::consts::ARCH.to_owned(),
            log_dir,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_with_camel_case_keys_for_typescript() {
        let info = AppInfo::current("9.9.9", None);
        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["appVersion"], "9.9.9");
        assert_eq!(json["coreVersion"], sr_core::CORE_VERSION);
        assert!(json["logDir"].is_null());
    }
}
