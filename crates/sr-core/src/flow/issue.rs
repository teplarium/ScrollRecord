//! # Scenario problems, one by one
//!
//! Purpose: what [`Flow::issues`](super::Flow::issues) returns — every problem
//!          of a scenario as data: how bad (`severity`), what (`kind`), where
//!          (`node` id or `edge` index) and the values for its text (`params`).
//! Solves:  the CLI needs one readable report (`SR-2002`), the editor needs to
//!          outline the broken shapes and arrows and show each reason in the
//!          user's language (FR-CNV-09). Both come from the same checks in
//!          `path.rs`, so the rules live once, in Rust (ADR 0006 §2).
//! Rules:   `message` is the English text (CLI, logs) and never changes when
//!          texts are translated; the UI shows `issues.<kind>` from its locale
//!          files with `params` (a test checks every kind has a text). A new
//!          check = a new `IssueKind` variant + its English text in
//!          `apps/desktop/src/i18n/locales/en/common.json`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// How bad a problem is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Severity {
    /// The video can't be rendered until it is fixed.
    Error,
    /// Rendered anyway; something is ignored.
    Warning,
}

macro_rules! issue_kinds {
    ($($(#[$doc:meta])* $variant:ident = $key:literal,)*) => {
        /// What kind of problem it is; the UI's text key is `issues.<key>`.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
        #[ts(export)]
        pub enum IssueKind {
            $($(#[$doc])* #[serde(rename = $key)] $variant,)*
        }

        impl IssueKind {
            /// Every kind, for the UI's text check.
            pub const ALL: &'static [IssueKind] = &[$(IssueKind::$variant,)*];

            /// The key used in JSON and in the UI texts.
            pub fn key(self) -> &'static str {
                match self {
                    $(IssueKind::$variant => $key,)*
                }
            }
        }
    };
}

issue_kinds! {
    /// `format_version` is not the one this build reads.
    FormatVersion = "formatVersion",
    /// No render targets.
    NoTargets = "noTargets",
    /// Two targets with one id.
    DuplicateTarget = "duplicateTarget",
    /// fps outside 1..120.
    BadFps = "badFps",
    /// Unknown device or impossible output size.
    BadTarget = "badTarget",
    /// `frame: "block"` without `frame_block`.
    FrameBlockMissing = "frameBlockMissing",
    /// `frame_block` names something that isn't a block.
    FrameBlockNotBlock = "frameBlockNotBlock",
    /// No pages.
    NoPages = "noPages",
    /// Two pages with one id.
    DuplicatePage = "duplicatePage",
    /// A page without an address.
    EmptyUrl = "emptyUrl",
    /// A node with an empty id.
    EmptyNodeId = "emptyNodeId",
    /// Two nodes with one id.
    DuplicateNode = "duplicateNode",
    /// Start/End anchored to something that isn't a block.
    AnchorNotBlock = "anchorNotBlock",
    /// Start/End not anchored yet (empty block name: a new Start before its
    /// first arrow). Same CLI text as `AnchorNotBlock`; a separate kind so
    /// the editor can treat it as "not finished yet" rather than "broken".
    AnchorMissing = "anchorMissing",
    /// Anchor progress outside 0..1.
    AnchorProgress = "anchorProgress",
    /// A block on a page the scenario doesn't have.
    UnknownPage = "unknownPage",
    /// A block without any way to find it.
    NoLocator = "noLocator",
    /// A broken locator.
    BadLocator = "badLocator",
    /// A broken cursor action.
    BadAction = "badAction",
    /// Broken cursor defaults.
    BadCursor = "badCursor",
    /// A navigation shape with an unknown page.
    NavigateUnknownPage = "navigateUnknownPage",
    /// Not exactly one Start.
    StartCount = "startCount",
    /// Not exactly one End.
    EndCount = "endCount",
    /// An arrow to or from a node that doesn't exist.
    ArrowUnknownNode = "arrowUnknownNode",
    /// An arrow from a node to itself.
    ArrowToItself = "arrowToItself",
    /// A scroll speed of zero or less.
    BadSpeed = "badSpeed",
    /// A second arrow out of one node (branches aren't supported yet).
    Branch = "branch",
    /// More than one arrow into one node.
    ManyIncoming = "manyIncoming",
    /// An arrow into Start.
    StartHasIncoming = "startHasIncoming",
    /// The arrows go round in a circle.
    Loop = "loop",
    /// The path from Start stops before End.
    PathNotToEnd = "pathNotToEnd",
    /// A shape no arrow path reaches (ignored).
    NotOnPath = "notOnPath",
    /// A device override for a target or device the scenario doesn't use.
    OverrideUnused = "overrideUnused",
    /// A device override for a node that doesn't exist.
    OverrideUnknownNode = "overrideUnknownNode",
    /// `before` actions in an override (ignored until Phase 4).
    BeforeIgnored = "beforeIgnored",
}

/// One problem of a scenario.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Issue {
    /// Error (no render) or warning.
    pub severity: Severity,
    /// What it is; also the UI text key.
    pub kind: IssueKind,
    /// English text (CLI and logs).
    pub message: String,
    /// Values for the translated text (`{{name}}` placeholders).
    pub params: BTreeMap<String, String>,
    /// The shape it is about, if any.
    pub node: Option<String>,
    /// The arrow it is about: index in `edges`, if any.
    pub edge: Option<usize>,
}

impl Issue {
    /// A problem that stops rendering.
    pub(crate) fn error(kind: IssueKind, message: impl Into<String>) -> Self {
        Self::new(Severity::Error, kind, message.into())
    }

    /// A problem that is only reported.
    pub(crate) fn warning(kind: IssueKind, message: impl Into<String>) -> Self {
        Self::new(Severity::Warning, kind, message.into())
    }

    fn new(severity: Severity, kind: IssueKind, message: String) -> Self {
        Self {
            severity,
            kind,
            message,
            params: BTreeMap::new(),
            node: None,
            edge: None,
        }
    }

    /// Points it at a shape.
    pub(crate) fn node(mut self, id: impl Into<String>) -> Self {
        self.node = Some(id.into());
        self
    }

    /// Points it at an arrow (index in `edges`).
    pub(crate) fn edge(mut self, index: usize) -> Self {
        self.edge = Some(index);
        self
    }

    /// Adds a value for the translated text.
    pub(crate) fn param(mut self, name: &str, value: impl ToString) -> Self {
        self.params.insert(name.to_owned(), value.to_string());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_have_unique_keys_and_serialize_as_them() {
        let mut keys: Vec<&str> = IssueKind::ALL.iter().map(|k| k.key()).collect();
        for kind in IssueKind::ALL {
            assert_eq!(
                serde_json::to_value(kind).unwrap(),
                serde_json::Value::String(kind.key().to_owned())
            );
        }
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), IssueKind::ALL.len());
    }

    // ── Export the kind list for the UI ──
    // Why: the UI test checks that every kind has a text (like error codes).
    #[test]
    fn export_issue_kind_list() {
        let Ok(dir) = std::env::var("TS_RS_EXPORT_DIR") else {
            return;
        };
        let keys: Vec<&str> = IssueKind::ALL.iter().map(|k| k.key()).collect();
        let mut json = serde_json::to_string_pretty(&keys).unwrap();
        json.push('\n');
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(std::path::Path::new(&dir).join("issue-kinds.json"), json).unwrap();
    }
}
