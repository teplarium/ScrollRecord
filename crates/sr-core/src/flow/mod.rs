//! # Flow — the scenario file format (`*.srflow.json`), version 1
//!
//! Purpose: the typed model of a recording scenario: render targets, pages,
//!          nodes (start / block / navigate / end), arrows and per-device
//!          overrides (HANDOFF §8.1).
//! Solves:  the canvas editor, the CLI and AI models all read and write the
//!          same file. Strict parsing (`deny_unknown_fields`) turns typos like
//!          `"hold_sm"` into clear errors instead of silently ignored settings.
//! Phases:  fields for later phases (`actions`, `cursor`, `layout`, `navigate`)
//!          are already accepted, so files written today stay valid. The
//!          renderer reports them as "not implemented yet" or ignores them
//!          with a warning.
//! See also: `path.rs` (validation and the ordered path), `target.rs`.

mod action;
mod cursor;
mod locator;
mod path;
mod target;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub use action::{Action, ActionKind, Point, PointerMode};
pub use cursor::{CursorKind, CursorSettings, DEFAULT_CURSOR_PACK, ScrollCursor};
pub use locator::Locator;
pub use path::{FlowPlan, MeasureItem, PlanAction, PlanBlock, PlanEnd, PlanStart, ScrollMove};
pub use target::{
    Aspect, Fit, Frame, OutputFormat, OutputGeometry, Resolution, Target, Viewport, ViewportRef,
};

use crate::easing::Easing;
use crate::error::{AppError, AppResult, ErrorCode};

/// The format version this build reads and writes.
pub const FORMAT_VERSION: u32 = 1;

/// A complete recording scenario.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(deny_unknown_fields)]
pub struct Flow {
    /// Optional JSON Schema link for editors.
    #[serde(rename = "$schema", default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    /// Must be [`FORMAT_VERSION`].
    pub format_version: u32,
    /// Name and site.
    pub meta: Meta,
    /// Videos to render.
    pub targets: Vec<Target>,
    /// Values used when a node or arrow doesn't set its own.
    #[serde(default)]
    pub defaults: Defaults,
    /// Page preparation before recording.
    #[serde(default)]
    pub prepare: Prepare,
    /// Pages of the site (Phase 1 renders one page per flow).
    pub pages: Vec<Page>,
    /// Canvas nodes.
    pub nodes: Vec<Node>,
    /// Arrows between nodes.
    pub edges: Vec<Edge>,
    /// Per-device changes, keyed by viewport preset name or target id.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub overrides: BTreeMap<String, Override>,
    /// Editor-only data (positions, colours). Opaque to the renderer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<serde_json::Value>,
}

/// Scenario name and the site it records.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(deny_unknown_fields)]
pub struct Meta {
    /// Human-readable name.
    pub name: String,
    /// `http(s)://…` URL, or a folder path relative to the scenario file.
    /// The command line `--site` overrides it.
    #[serde(default, alias = "base_url", skip_serializing_if = "Option::is_none")]
    pub site: Option<String>,
}

/// Scroll settings of an arrow (or the defaults).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(deny_unknown_fields)]
pub struct ScrollSpec {
    /// Fixed duration of the move. Wins over `speed_px_s`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u32>,
    /// Average speed in CSS pixels per second.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed_px_s: Option<f64>,
    /// Timing curve.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub easing: Option<Easing>,
}

impl ScrollSpec {
    /// Fills unset fields from `fallback`.
    pub fn or(self, fallback: ScrollSpec) -> ScrollSpec {
        ScrollSpec {
            duration_ms: self.duration_ms.or(fallback.duration_ms),
            speed_px_s: self.speed_px_s.or(fallback.speed_px_s),
            easing: self.easing.or(fallback.easing),
        }
    }
}

/// Scenario-wide defaults.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(deny_unknown_fields)]
pub struct Defaults {
    /// Default scroll move.
    #[serde(default = "Defaults::default_scroll")]
    pub scroll: ScrollSpec,
    /// Default pause on a block, in ms (0 = scroll through without stopping).
    #[serde(default = "Defaults::default_hold")]
    pub hold_ms: u32,
    /// Cursor look and behaviour.
    #[serde(default)]
    pub cursor: CursorSettings,
}

impl Defaults {
    /// Built-in scroll speed (px/s) when nothing else is set.
    pub const SPEED_PX_S: f64 = 900.0;
    /// Built-in pause on a block (ms).
    pub const HOLD_MS: u32 = 1500;

    fn default_scroll() -> ScrollSpec {
        ScrollSpec {
            duration_ms: None,
            speed_px_s: Some(Self::SPEED_PX_S),
            easing: Some(Easing::EaseInOut),
        }
    }

    fn default_hold() -> u32 {
        Self::HOLD_MS
    }
}

impl Default for Defaults {
    fn default() -> Self {
        Self {
            scroll: Self::default_scroll(),
            hold_ms: Self::HOLD_MS,
            cursor: CursorSettings::default(),
        }
    }
}

/// When the page counts as "ready" before recording starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum WaitCondition {
    /// Document, fonts and images loaded.
    #[default]
    Load,
    /// `load` plus no new network requests for 500 ms.
    NetworkIdle,
}

/// Page preparation (HANDOFF FR-VP-04).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(deny_unknown_fields)]
pub struct Prepare {
    /// CSS selectors of elements to hide (cookie banners, chat widgets, dev bars).
    #[serde(default)]
    pub hide: Vec<String>,
    /// Load `loading="lazy"` images up front so they don't pop in on camera.
    #[serde(default = "yes")]
    pub preload_lazy: bool,
    /// Ready condition.
    #[serde(default)]
    pub wait: WaitCondition,
    /// Extra CSS injected into the page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub css: Option<String>,
}

fn yes() -> bool {
    true
}

impl Default for Prepare {
    fn default() -> Self {
        Self {
            hide: Vec::new(),
            preload_lazy: true,
            wait: WaitCondition::Load,
            css: None,
        }
    }
}

/// A page of the site.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(deny_unknown_fields)]
pub struct Page {
    /// Unique id referenced by nodes.
    pub id: String,
    /// Path (`/pricing`) relative to the site, or an absolute URL.
    pub url: String,
}

// ── Nodes ───────────────────────────────────────────────────────────────────

/// A shape on the canvas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, rename = "FlowNode")]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Node {
    /// Oval: where recording starts.
    Start(StartNode),
    /// Rectangle: a section of the page.
    Block(BlockNode),
    /// Click a link and continue on another page (Phase 4).
    Navigate(NavigateNode),
    /// Oval: where recording ends.
    End(EndNode),
}

impl Node {
    /// Node id.
    pub fn id(&self) -> &str {
        match self {
            Node::Start(n) => &n.id,
            Node::Block(n) => &n.id,
            Node::Navigate(n) => &n.id,
            Node::End(n) => &n.id,
        }
    }
}

/// Start of the recording, attached to a block (HANDOFF R-18).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(deny_unknown_fields)]
pub struct StartNode {
    /// Unique node id.
    pub id: String,
    /// Scroll position where the video begins.
    pub at: Anchor,
    /// Let the page "live" this long before the first frame (skip intros).
    #[serde(default)]
    pub warmup_ms: u32,
}

/// A page section the video scrolls to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(deny_unknown_fields)]
pub struct BlockNode {
    /// Unique node id.
    pub id: String,
    /// Page id.
    pub page: String,
    /// Locators, tried in order (first unique match wins).
    pub target: Vec<Locator>,
    /// Where the block lands in the frame.
    #[serde(default)]
    pub align: Align,
    /// Extra shift in CSS px (positive = scroll further down).
    #[serde(default)]
    pub offset_px: f64,
    /// Pause on this block. `0` = scroll through without stopping.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hold_ms: Option<u32>,
    /// Cursor steps on this block (hover, click, move, wait), in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<Action>,
}

/// Navigation to another page (Phase 4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(deny_unknown_fields)]
pub struct NavigateNode {
    /// Unique node id.
    pub id: String,
    /// Page the click happens on.
    pub page: String,
    /// How to navigate (`{ "click": [locators] }` or `{ "url": "…" }`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub via: Option<serde_json::Value>,
    /// Destination page id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_page: Option<String>,
    /// Ready condition on the new page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wait: Option<serde_json::Value>,
}

/// End of the recording.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(deny_unknown_fields)]
pub struct EndNode {
    /// Unique node id.
    pub id: String,
    /// Final scroll position; `None` = stay where the last block left off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<Anchor>,
    /// Keep recording this long on the final position.
    #[serde(default)]
    pub hold_ms: u32,
}

/// Where a block lands in the frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum Align {
    /// Block top at the top of the frame (below a sticky header).
    #[default]
    Top,
    /// Block centred.
    Center,
    /// Block bottom at the bottom of the frame.
    Bottom,
}

/// A scroll position defined relative to a block (HANDOFF FR-ANC-01).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(deny_unknown_fields)]
pub struct Anchor {
    /// Block node id.
    pub block: String,
    /// Named point. Ignored when `progress` is set.
    #[serde(default)]
    pub anchor: AnchorPoint,
    /// 0 = block entering at the bottom, 1 = block leaving at the top.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub progress: Option<f64>,
    /// Extra shift in CSS px.
    #[serde(default)]
    pub offset_px: f64,
}

/// Named anchor points.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum AnchorPoint {
    /// Block top at the frame top.
    #[default]
    Top,
    /// Block centred.
    Center,
    /// Block bottom at the frame bottom.
    Bottom,
    /// Block top at the frame bottom (just starting to enter).
    Enter,
    /// Block bottom at the frame top (just left).
    Exit,
}

/// An arrow.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(deny_unknown_fields)]
pub struct Edge {
    /// Source node id.
    pub from: String,
    /// Destination node id.
    pub to: String,
    /// Scroll move along this arrow.
    #[serde(default)]
    pub scroll: ScrollSpec,
    /// Cursor during the move: `still` (default), `follow` or `hide`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<ScrollCursor>,
}

/// Per-device changes.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(deny_unknown_fields)]
pub struct Override {
    /// Changes per node id.
    #[serde(default)]
    pub nodes: BTreeMap<String, NodeOverride>,
}

/// Changes to one node on one device.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(deny_unknown_fields)]
pub struct NodeOverride {
    /// Leave this block out on this device.
    #[serde(default)]
    pub skip: bool,
    /// Different pause.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hold_ms: Option<u32>,
    /// Different alignment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub align: Option<Align>,
    /// Different offset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset_px: Option<f64>,
    /// Extra actions before this node, e.g. opening a burger menu (Phase 2).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub before: Vec<serde_json::Value>,
}

// ── Parsing ─────────────────────────────────────────────────────────────────

impl Flow {
    /// Parses a scenario from JSON text.
    ///
    /// Errors: `SR-2001` for syntax or shape problems (with line/column),
    /// `SR-2003` for a newer `format_version`.
    pub fn from_json(text: &str) -> AppResult<Flow> {
        // Check the version first, so a future file gets a clear message
        // instead of a confusing "unknown field" error.
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(text)
            && let Some(v) = value
                .get("format_version")
                .and_then(serde_json::Value::as_u64)
            && v > u64::from(FORMAT_VERSION)
        {
            return Err(AppError::new(
                ErrorCode::FlowVersionUnsupported,
                format!("scenario uses format_version {v}, this build reads {FORMAT_VERSION}"),
            )
            .with_param("version", v)
            .with_param("supported", FORMAT_VERSION));
        }
        serde_json::from_str(text).map_err(|err| {
            AppError::new(ErrorCode::FlowParseFailed, format!("scenario JSON: {err}"))
                .with_param("line", err.line())
                .with_param("column", err.column())
                .with_param("detail", err)
        })
    }

    /// Serializes to pretty JSON (2-space indent, trailing newline).
    pub fn to_json(&self) -> AppResult<String> {
        let mut out = serde_json::to_string_pretty(self)
            .map_err(|err| AppError::new(ErrorCode::Internal, format!("serialize flow: {err}")))?;
        out.push('\n');
        Ok(out)
    }

    /// Finds a node by id.
    pub fn node(&self, id: &str) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id() == id)
    }

    /// Finds a target by id.
    pub fn target(&self, id: &str) -> AppResult<&Target> {
        self.targets.iter().find(|t| t.id == id).ok_or_else(|| {
            let known: Vec<&str> = self.targets.iter().map(|t| t.id.as_str()).collect();
            AppError::new(
                ErrorCode::TargetNotFound,
                format!("no target '{id}' (available: {})", known.join(", ")),
            )
            .with_param("target", id)
            .with_param("available", known.join(", "))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL: &str = r#"{
      "format_version": 1,
      "meta": { "name": "t" },
      "targets": [{ "id": "desktop", "viewport": "desktop-1920" }],
      "pages": [{ "id": "home", "url": "/" }],
      "nodes": [
        { "id": "start", "kind": "start", "at": { "block": "hero" } },
        { "id": "hero", "kind": "block", "page": "home", "target": [{ "sr": "hero" }] },
        { "id": "end", "kind": "end" }
      ],
      "edges": [{ "from": "start", "to": "hero" }, { "from": "hero", "to": "end" }]
    }"#;

    #[test]
    fn parses_minimal_flow_with_defaults() {
        let flow = Flow::from_json(MINIMAL).unwrap();
        assert_eq!(flow.defaults.hold_ms, Defaults::HOLD_MS);
        assert!(flow.prepare.preload_lazy);
        assert_eq!(flow.targets[0].fps, 60);
        assert!(matches!(flow.node("hero"), Some(Node::Block(_))));
    }

    #[test]
    fn typo_in_field_name_is_an_error_with_position() {
        let bad = MINIMAL.replace("\"target\":", "\"targett\":");
        let err = Flow::from_json(&bad).unwrap_err();
        assert_eq!(err.code, ErrorCode::FlowParseFailed);
        assert!(err.message.contains("targett"), "{}", err.message);
        assert!(err.params.contains_key("line"));
    }

    #[test]
    fn future_version_is_reported_clearly() {
        let future = MINIMAL.replace("\"format_version\": 1", "\"format_version\": 7");
        assert_eq!(
            Flow::from_json(&future).unwrap_err().code,
            ErrorCode::FlowVersionUnsupported
        );
    }

    #[test]
    fn round_trips_through_json() {
        let flow = Flow::from_json(MINIMAL).unwrap();
        let again = Flow::from_json(&flow.to_json().unwrap()).unwrap();
        assert_eq!(flow, again);
    }

    // Why: the editor loads a scenario, edits it as plain JSON in the UI and
    //      writes it back. Every shipped example must survive that trip with
    //      nothing lost, and saving twice must give the same text (no
    //      ever-changing diffs in a project folder).
    #[test]
    fn every_example_round_trips_through_json() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
        let mut checked = 0;
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if !path.to_string_lossy().ends_with(".srflow.json") {
                continue;
            }
            let flow = Flow::from_json(&std::fs::read_to_string(&path).unwrap())
                .unwrap_or_else(|err| panic!("{}: {err}", path.display()));
            let text = flow.to_json().unwrap();
            let again = Flow::from_json(&text).unwrap();
            assert_eq!(flow, again, "{}", path.display());
            assert_eq!(text, again.to_json().unwrap(), "{}", path.display());
            checked += 1;
        }
        assert!(
            checked >= 6,
            "expected the examples folder, found {checked} files"
        );
    }

    #[test]
    fn empty_draft_scenario_parses() {
        // A new project starts with an empty canvas (HANDOFF D-39): no nodes,
        // no arrows. Parsing must accept it; validation reports it at render.
        let draft = r#"{ "format_version": 1, "meta": { "name": "t" },
          "targets": [], "pages": [], "nodes": [], "edges": [] }"#;
        let flow = Flow::from_json(draft).unwrap();
        assert!(flow.nodes.is_empty() && flow.edges.is_empty());
    }

    #[test]
    fn unknown_target_lists_available_ones() {
        let flow = Flow::from_json(MINIMAL).unwrap();
        let err = flow.target("phone").unwrap_err();
        assert_eq!(err.code, ErrorCode::TargetNotFound);
        assert_eq!(err.params["available"], "desktop");
    }
}
