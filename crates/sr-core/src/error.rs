//! # Error codes and the shared error type
//!
//! Purpose: one catalog of every error the app can report, each with a stable
//!          `SR-xxxx` code, plus `AppError` — the single error type that crosses
//!          crate boundaries and the IPC bridge to the UI.
//! Solves:  the project is maintained without a full-time programmer
//!          (HANDOFF §12). A stable code lets anyone paste "SR-0101" into an
//!          issue, and lets an AI assistant jump straight to the cause via
//!          `docs/ERRORS.md` and a code search.
//!
//! Code ranges (keep new codes inside their range):
//! - `SR-00xx` app / general        - `SR-01xx` diagnostics & logging
//! - `SR-1xxx` projects & files      - `SR-2xxx` flow format & validation
//! - `SR-3xxx` browser               - `SR-4xxx` capture
//! - `SR-5xxx` encoder               - `SR-6xxx` cursor
//! - `SR-7xxx` AI & skills           - `SR-8xxx` site sources
//!
//! Adding a code: add one line to the `error_codes!` list below, then document
//! it in `docs/ERRORS.md` and add an English UI text under `errors.<code>` in
//! `apps/desktop/src/i18n/locales/en/common.json`. Tests fail until all three
//! exist, so a code can never be half-added.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

// ── Single-source error-code list ───────────────────────────────────────────
// Why: the enum variant, its `SR-xxxx` string and its one-line summary must
//      never drift apart. Declaring all three in one macro line guarantees it
//      and also produces `ErrorCode::ALL`, which tests use to check that every
//      code is documented and translated.
macro_rules! error_codes {
    ($( $(#[doc = $doc:literal])* $variant:ident = $code:literal, $summary:literal; )+) => {
        /// Stable identifier of an error, serialized as its `SR-xxxx` string.
        ///
        /// The string form is what users see, what the UI translates and what
        /// appears in logs. Never change or reuse an existing code — add a new one.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
        #[ts(export)]
        pub enum ErrorCode {
            $( $(#[doc = $doc])* #[serde(rename = $code)] $variant, )+
        }

        impl ErrorCode {
            /// Every known code, in declaration order.
            pub const ALL: &'static [ErrorCode] = &[$(ErrorCode::$variant),+];

            /// The `SR-xxxx` string of this code.
            pub const fn as_str(self) -> &'static str {
                match self { $(ErrorCode::$variant => $code),+ }
            }

            /// One-line English summary for developers (CLI `errors` command,
            /// logs). User-facing texts live in the UI locale files instead.
            pub const fn summary(self) -> &'static str {
                match self { $(ErrorCode::$variant => $summary),+ }
            }
        }
    };
}

error_codes! {
    /// A bug: something happened that the code did not expect.
    Internal = "SR-0001", "Unexpected internal error (this is a bug, please report it)";
    /// The user triggered a feature that is planned but not built yet.
    NotImplemented = "SR-0002", "Feature is planned but not implemented yet";
    /// The UI is running outside the desktop app (e.g. in a plain browser
    /// during development), so it cannot reach the Rust core.
    DesktopShellUnavailable = "SR-0003", "UI is not running inside the desktop app";
    /// The log folder or log file could not be created.
    LoggingInitFailed = "SR-0101", "Could not initialise logging";
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ── AppError: the one error type that crosses boundaries ────────────────────
// Why: crates, the CLI and the UI need the same shape of error. `message` is a
//      technical English text for logs; the UI shows a translated text chosen
//      by `code` and filled with `params` (HANDOFF FR-I18N-04).

/// Error returned by every public ScrollRecord API and every IPC command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, thiserror::Error)]
#[ts(export)]
#[error("[{code}] {message}")]
pub struct AppError {
    /// Stable error code, e.g. `SR-0101`.
    pub code: ErrorCode,
    /// Technical English description for logs and bug reports.
    pub message: String,
    /// Named values the UI can insert into the translated text
    /// (e.g. `{ "feature": "projects", "phase": "3" }`).
    pub params: BTreeMap<String, String>,
}

impl AppError {
    /// Creates an error with a code and a technical message.
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            params: BTreeMap::new(),
        }
    }

    /// Adds a named parameter for the translated UI message (builder style).
    #[must_use]
    pub fn with_param(mut self, key: impl Into<String>, value: impl ToString) -> Self {
        self.params.insert(key.into(), value.to_string());
        self
    }

    /// Shortcut for "this feature arrives in roadmap phase N".
    pub fn not_implemented(feature: &str, phase: u8) -> Self {
        Self::new(
            ErrorCode::NotImplemented,
            format!("'{feature}' is planned for roadmap phase {phase}"),
        )
        .with_param("feature", feature)
        .with_param("phase", phase)
    }
}

/// Result alias used across the workspace.
pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    // ── Catalog integrity ───────────────────────────────────────────────────
    // Why: these tests are the guard rails that make "add a code in three
    //      places" impossible to get wrong.

    #[test]
    fn codes_are_unique_and_well_formed() {
        let mut seen = HashSet::new();
        for code in ErrorCode::ALL {
            let s = code.as_str();
            assert!(seen.insert(s), "duplicate error code {s}");
            assert_eq!(s.len(), 7, "{s} must look like SR-1234");
            assert!(s.starts_with("SR-"), "{s} must start with SR-");
            assert!(
                s[3..].chars().all(|c| c.is_ascii_digit()),
                "{s} must end with 4 digits"
            );
        }
    }

    #[test]
    fn serde_name_matches_code_string() {
        for code in ErrorCode::ALL {
            let json = serde_json::to_string(code).unwrap();
            assert_eq!(json, format!("\"{}\"", code.as_str()));
        }
    }

    #[test]
    fn every_code_is_documented_in_errors_md() {
        let doc = include_str!("../../../docs/ERRORS.md");
        for code in ErrorCode::ALL {
            assert!(
                doc.contains(&format!("## {}", code.as_str())),
                "docs/ERRORS.md has no section '## {}'",
                code.as_str()
            );
        }
    }

    #[test]
    fn app_error_serializes_for_the_ui() {
        let err = AppError::not_implemented("projects", 3);
        let json = serde_json::to_value(&err).unwrap();
        assert_eq!(json["code"], "SR-0002");
        assert_eq!(json["params"]["phase"], "3");
        assert_eq!(
            err.to_string(),
            "[SR-0002] 'projects' is planned for roadmap phase 3"
        );
    }

    // ── Export the code list for the UI ─────────────────────────────────────
    // Why: ts-rs exports the *type* `ErrorCode`, but the UI also needs the
    //      runtime list (to recognise errors and to test that every code has a
    //      translation). Written next to the ts-rs output on every `cargo test`.
    #[test]
    fn export_error_code_list() {
        let Ok(dir) = std::env::var("TS_RS_EXPORT_DIR") else {
            return; // running outside the workspace config — nothing to export
        };
        let codes: Vec<&str> = ErrorCode::ALL.iter().map(|c| c.as_str()).collect();
        let mut json = serde_json::to_string_pretty(&codes).unwrap();
        json.push('\n');
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(std::path::Path::new(&dir).join("error-codes.json"), json).unwrap();
    }
}
