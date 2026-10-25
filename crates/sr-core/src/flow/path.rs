//! # Validation and the per-target plan
//!
//! Purpose: checks a [`Flow`] for mistakes, then reduces it for one render
//!          target to a plain ordered list: start → blocks → end, with every
//!          default and device override already applied.
//! Solves:  users (and AI models) draw arrows in any order and add device
//!          overrides on top. Later stages must not care about any of that;
//!          they get one straight path with final numbers. All problems are
//!          collected and reported together (`SR-2002`) with node ids, so the
//!          editor can highlight every broken shape at once.

use std::collections::{HashMap, HashSet};

use super::{
    Action, ActionKind, Align, Anchor, CursorSettings, Fit, Flow, Frame, Locator, Node,
    NodeOverride, OutputGeometry, Page, ScrollCursor, Target, Viewport,
};
use crate::easing::Easing;
use crate::error::{AppError, AppResult, ErrorCode};
use crate::flow::{Defaults, FORMAT_VERSION};

/// How to move from the previous position to the next one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrollMove {
    /// Fixed duration, if the arrow sets one.
    pub duration_ms: Option<u32>,
    /// Speed used when no duration is set (CSS px per second).
    pub speed_px_s: f64,
    /// Timing curve.
    pub easing: Easing,
}

/// Where and how the recording starts.
#[derive(Debug, Clone, PartialEq)]
pub struct PlanStart {
    /// Start node id.
    pub id: String,
    /// Starting scroll position.
    pub at: Anchor,
    /// Page time to skip before the first frame.
    pub warmup_ms: u32,
}

/// One block stop with final values for this target.
#[derive(Debug, Clone, PartialEq)]
pub struct PlanBlock {
    /// Block node id.
    pub id: String,
    /// Locators, in priority order.
    pub target: Vec<Locator>,
    /// Alignment in the frame.
    pub align: Align,
    /// Extra shift in CSS px.
    pub offset_px: f64,
    /// Pause after arriving (0 = pass through). Actions extend it if needed.
    pub hold_ms: u32,
    /// Move that arrives at this block.
    pub arrive: ScrollMove,
    /// Cursor behaviour during that move.
    pub arrive_cursor: ScrollCursor,
    /// Actions for this target, in order (filtered by only/except and device).
    pub actions: Vec<PlanAction>,
}

/// An action of a block, with its position in the block's `actions` list.
#[derive(Debug, Clone, PartialEq)]
pub struct PlanAction {
    /// Index in the block's original `actions` list (used in ids and messages).
    pub index: usize,
    /// The action.
    pub action: Action,
}

impl PlanAction {
    /// Measurement id of this action's target element: `"<block>#<index>"`.
    pub fn measure_id(block: &str, index: usize) -> String {
        format!("{block}#{index}")
    }
}

/// One element to measure on the page.
#[derive(Debug, Clone, PartialEq)]
pub struct MeasureItem {
    /// Id the result is stored under (block id, or `block#n` for action targets).
    pub id: String,
    /// Locators in priority order.
    pub locators: Vec<Locator>,
    /// Search inside this block's element first (action targets).
    pub within: Option<String>,
}

/// Where and how the recording ends.
#[derive(Debug, Clone, PartialEq)]
pub struct PlanEnd {
    /// End node id.
    pub id: String,
    /// Final position (`None` = stay at the last block).
    pub at: Option<Anchor>,
    /// Final pause.
    pub hold_ms: u32,
    /// Move that arrives at the final position.
    pub arrive: ScrollMove,
    /// Cursor behaviour during that move.
    pub arrive_cursor: ScrollCursor,
}

/// Everything one target needs to render, in order.
#[derive(Debug, Clone, PartialEq)]
pub struct FlowPlan {
    /// The render target.
    pub target: Target,
    /// Resolved device.
    pub viewport: Viewport,
    /// Pixel sizes.
    pub geometry: OutputGeometry,
    /// The page being recorded.
    pub page: Page,
    /// Start.
    pub start: PlanStart,
    /// Block stops, in order (skipped ones removed).
    pub blocks: Vec<PlanBlock>,
    /// End.
    pub end: PlanEnd,
    /// What to measure on the page: the block stops, blocks used only as
    /// start/end anchors, and the target elements of actions.
    pub measure: Vec<MeasureItem>,
    /// Cursor settings.
    pub cursor: CursorSettings,
    /// Touch device: taps instead of a mouse cursor.
    pub touch: bool,
    /// Non-fatal notes (ignored phase-2 settings, unconnected nodes…).
    pub warnings: Vec<String>,
}

// ── Validation ──────────────────────────────────────────────────────────────

/// Collects problems; turns into one `SR-2002` error if any.
#[derive(Default)]
struct Report {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl Report {
    fn err(&mut self, msg: impl Into<String>) {
        self.errors.push(msg.into());
    }
    fn warn(&mut self, msg: impl Into<String>) {
        self.warnings.push(msg.into());
    }
}

impl Flow {
    /// Checks the whole scenario. Returns warnings, or `SR-2002` listing
    /// every problem (one per line).
    pub fn validate(&self) -> AppResult<Vec<String>> {
        let mut r = Report::default();
        let _path = self.check(&mut r);
        if r.errors.is_empty() {
            Ok(r.warnings)
        } else {
            Err(AppError::new(ErrorCode::FlowInvalid, r.errors.join("\n"))
                .with_param("count", r.errors.len())
                .with_param("first", r.errors.first().cloned().unwrap_or_default()))
        }
    }

    /// Runs all checks; returns the ordered node path if the graph is sound.
    fn check(&self, r: &mut Report) -> Option<Vec<&Node>> {
        if self.format_version != FORMAT_VERSION {
            r.err(format!("format_version must be {FORMAT_VERSION}"));
        }
        self.check_targets(r);
        let pages = self.check_pages(r);
        let nodes = self.check_nodes(r, &pages);
        self.check_cursor_and_actions(r);
        let path = self.check_graph(r, &nodes);
        self.check_overrides(r, &nodes);
        path
    }

    fn check_targets(&self, r: &mut Report) {
        if self.targets.is_empty() {
            r.err("targets: at least one render target is required");
        }
        let mut seen = HashSet::new();
        for t in &self.targets {
            if !seen.insert(t.id.as_str()) {
                r.err(format!("targets: duplicate id '{}'", t.id));
            }
            if !(1..=120).contains(&t.fps) {
                r.err(format!(
                    "target '{}': fps must be 1..120, got {}",
                    t.id, t.fps
                ));
            }
            match t.viewport.resolve() {
                Ok(vp) => {
                    if let Err(e) = t.geometry(&vp) {
                        r.err(format!("target '{}': {e}", t.id));
                    }
                }
                Err(e) => r.err(format!("target '{}': {e}", t.id)),
            }
            if t.frame == Frame::Block && t.frame_block.is_none() {
                r.err(format!(
                    "target '{}': frame 'block' needs frame_block",
                    t.id
                ));
            }
        }
    }

    fn check_pages(&self, r: &mut Report) -> HashSet<&str> {
        if self.pages.is_empty() {
            r.err("pages: at least one page is required");
        }
        let mut ids = HashSet::new();
        for p in &self.pages {
            if !ids.insert(p.id.as_str()) {
                r.err(format!("pages: duplicate id '{}'", p.id));
            }
            if p.url.trim().is_empty() {
                r.err(format!("page '{}': url is empty", p.id));
            }
        }
        ids
    }

    fn check_nodes<'a>(
        &'a self,
        r: &mut Report,
        pages: &HashSet<&str>,
    ) -> HashMap<&'a str, &'a Node> {
        let mut nodes = HashMap::new();
        for n in &self.nodes {
            if n.id().trim().is_empty() {
                r.err("nodes: a node has an empty id");
            }
            if nodes.insert(n.id(), n).is_some() {
                r.err(format!("nodes: duplicate id '{}'", n.id()));
            }
        }
        let is_block = |id: &str| matches!(nodes.get(id), Some(Node::Block(_)));
        let check_anchor = |r: &mut Report, owner: &str, a: &Anchor| {
            if !is_block(&a.block) {
                r.err(format!(
                    "node '{owner}': anchor block '{}' is not a block node",
                    a.block
                ));
            }
            if let Some(p) = a.progress
                && !(0.0..=1.0).contains(&p)
            {
                r.err(format!(
                    "node '{owner}': anchor progress must be 0..1, got {p}"
                ));
            }
        };
        for n in &self.nodes {
            match n {
                Node::Start(s) => check_anchor(r, &s.id, &s.at),
                Node::End(e) => {
                    if let Some(a) = &e.at {
                        check_anchor(r, &e.id, a);
                    }
                }
                Node::Block(b) => {
                    if !pages.contains(b.page.as_str()) {
                        r.err(format!("block '{}': unknown page '{}'", b.id, b.page));
                    }
                    if b.target.is_empty() {
                        r.err(format!(
                            "block '{}': target needs at least one locator",
                            b.id
                        ));
                    }
                    for (i, loc) in b.target.iter().enumerate() {
                        if let Err(e) = loc.check() {
                            r.err(format!("block '{}': target[{i}]: {e}", b.id));
                        }
                    }
                }
                Node::Navigate(nav) => {
                    if !pages.contains(nav.page.as_str()) {
                        r.err(format!(
                            "navigate '{}': unknown page '{}'",
                            nav.id, nav.page
                        ));
                    }
                    if let Some(to) = &nav.to_page
                        && !pages.contains(to.as_str())
                    {
                        r.err(format!("navigate '{}': unknown to_page '{to}'", nav.id));
                    }
                }
            }
        }
        for t in &self.targets {
            if let Some(fb) = &t.frame_block
                && !is_block(fb)
            {
                r.err(format!(
                    "target '{}': frame_block '{fb}' is not a block node",
                    t.id
                ));
            }
        }
        let starts = self
            .nodes
            .iter()
            .filter(|n| matches!(n, Node::Start(_)))
            .count();
        let ends = self
            .nodes
            .iter()
            .filter(|n| matches!(n, Node::End(_)))
            .count();
        if starts != 1 {
            r.err(format!(
                "nodes: exactly one start node is required, found {starts}"
            ));
        }
        if ends != 1 {
            r.err(format!(
                "nodes: exactly one end node is required, found {ends}"
            ));
        }
        nodes
    }

    // ── Graph → one ordered path ────────────────────────────────────────────
    // Why: one video = one path Start → … → End (HANDOFF Q-05: no branches yet).
    fn check_graph<'a>(
        &'a self,
        r: &mut Report,
        nodes: &HashMap<&'a str, &'a Node>,
    ) -> Option<Vec<&'a Node>> {
        let mut out: HashMap<&str, &str> = HashMap::new();
        let mut incoming: HashMap<&str, usize> = HashMap::new();
        for e in &self.edges {
            let mut ok = true;
            for end in [&e.from, &e.to] {
                if !nodes.contains_key(end.as_str()) {
                    r.err(format!("arrow {} → {}: unknown node '{end}'", e.from, e.to));
                    ok = false;
                }
            }
            if e.from == e.to {
                r.err(format!(
                    "arrow {0} → {0}: a node cannot point to itself",
                    e.from
                ));
                ok = false;
            }
            if !ok {
                continue;
            }
            if out.insert(&e.from, &e.to).is_some() {
                r.err(format!(
                    "node '{}' has more than one outgoing arrow (branches are not supported yet)",
                    e.from
                ));
            }
            *incoming.entry(&e.to).or_default() += 1;
            if let Some(spec) = e.scroll.speed_px_s
                && spec <= 0.0
            {
                r.err(format!(
                    "arrow {} → {}: speed_px_s must be positive",
                    e.from, e.to
                ));
            }
        }
        for (id, count) in &incoming {
            if *count > 1 {
                r.err(format!(
                    "node '{id}' has {count} incoming arrows; one path needs exactly one"
                ));
            }
        }

        let start = self.nodes.iter().find(|n| matches!(n, Node::Start(_)))?;
        if incoming.contains_key(start.id()) {
            r.err(format!(
                "start '{}' must not have incoming arrows",
                start.id()
            ));
        }
        let mut path = vec![start];
        let mut visited: HashSet<&str> = HashSet::from([start.id()]);
        let mut current = start.id();
        while let Some(next) = out.get(current) {
            if !visited.insert(next) {
                r.err(format!("arrows form a loop at node '{next}'"));
                return None;
            }
            let node = nodes.get(next)?;
            path.push(node);
            current = next;
        }
        match path.last() {
            Some(Node::End(_)) => {}
            Some(last) => {
                r.err(format!(
                    "the path from start stops at '{}' without reaching the end node",
                    last.id()
                ));
            }
            None => {}
        }
        // Blocks off the path are fine when an anchor or a Block clip uses them.
        let mut referenced: HashSet<&str> = HashSet::new();
        for n in &self.nodes {
            match n {
                Node::Start(s) => {
                    referenced.insert(&s.at.block);
                }
                Node::End(e) => {
                    if let Some(a) = &e.at {
                        referenced.insert(&a.block);
                    }
                }
                _ => {}
            }
        }
        referenced.extend(self.targets.iter().filter_map(|t| t.frame_block.as_deref()));
        for n in &self.nodes {
            if !visited.contains(n.id()) && !referenced.contains(n.id()) {
                r.warn(format!(
                    "node '{}' is not on the path and will be ignored",
                    n.id()
                ));
            }
        }
        if r.errors.is_empty() {
            Some(path)
        } else {
            None
        }
    }

    fn check_overrides(&self, r: &mut Report, nodes: &HashMap<&str, &Node>) {
        for (key, ov) in &self.overrides {
            let matches = self
                .targets
                .iter()
                .any(|t| t.id == *key || t.viewport.preset_name() == Some(key.as_str()));
            if !matches {
                r.warn(format!(
                    "overrides '{key}': no target or used viewport preset has this name"
                ));
            }
            for id in ov.nodes.keys() {
                if !nodes.contains_key(id.as_str()) {
                    r.err(format!("overrides '{key}': unknown node '{id}'"));
                }
            }
        }
    }

    fn check_cursor_and_actions(&self, r: &mut Report) {
        if let Err(e) = self.defaults.cursor.check() {
            r.err(format!("defaults: {e}"));
        }
        for n in &self.nodes {
            if let Node::Block(b) = n {
                for (i, a) in b.actions.iter().enumerate() {
                    if let Err(e) = a.check() {
                        r.err(format!(
                            "block '{}': actions[{i}] ({}): {e}",
                            b.id,
                            a.kind_name()
                        ));
                    }
                }
            }
        }
        for (key, ov) in &self.overrides {
            for (id, n) in &ov.nodes {
                if !n.before.is_empty() {
                    r.warn(format!(
                        "overrides '{key}' / '{id}': 'before' actions arrive with navigation (Phase 4) and are ignored for now"
                    ));
                }
            }
        }
    }

    // ── Plan for one target ─────────────────────────────────────────────────

    /// Validates, then builds the ordered plan for `target_id`.
    pub fn plan(&self, target_id: &str) -> AppResult<FlowPlan> {
        let mut r = Report::default();
        let path = self.check(&mut r);
        if !r.errors.is_empty() {
            return Err(AppError::new(ErrorCode::FlowInvalid, r.errors.join("\n"))
                .with_param("count", r.errors.len())
                .with_param("first", r.errors.first().cloned().unwrap_or_default()));
        }
        let path =
            path.ok_or_else(|| AppError::new(ErrorCode::Internal, "valid flow without path"))?;
        let target = self.target(target_id)?.clone();
        if target.fit == Fit::Contain {
            return Err(AppError::not_implemented("fit: contain", 4));
        }
        if target.frame == Frame::Block {
            return Err(AppError::not_implemented("frame: block (Block clip)", 4));
        }
        let viewport = target
            .viewport
            .resolve()
            .map_err(|e| AppError::new(ErrorCode::FlowInvalid, e))?;
        let geometry = target
            .geometry(&viewport)
            .map_err(|e| AppError::new(ErrorCode::FlowInvalid, e))?;

        let overrides = self.overrides_for(&target);
        let touch = viewport.has_touch();
        let preset = target.viewport.preset_name();
        let mut warnings = r.warnings;
        let cursor_into = |id: &str| -> ScrollCursor {
            self.edges
                .iter()
                .find(|e| e.to == id)
                .and_then(|e| e.cursor)
                .unwrap_or_default()
        };
        let edge_into = |id: &str| -> ScrollMove {
            let spec = self
                .edges
                .iter()
                .find(|e| e.to == id)
                .map(|e| e.scroll)
                .unwrap_or_default()
                .or(self.defaults.scroll);
            ScrollMove {
                duration_ms: spec.duration_ms,
                speed_px_s: spec.speed_px_s.unwrap_or(Defaults::SPEED_PX_S),
                easing: spec.easing.unwrap_or_default(),
            }
        };

        let mut start = None;
        let mut end = None;
        let mut blocks = Vec::new();
        let mut page_id: Option<&str> = None;
        for node in path {
            match node {
                Node::Start(s) => {
                    start = Some(PlanStart {
                        id: s.id.clone(),
                        at: s.at.clone(),
                        warmup_ms: s.warmup_ms,
                    });
                }
                Node::Block(b) => {
                    let ov = overrides.get(b.id.as_str()).cloned().unwrap_or_default();
                    if ov.skip {
                        continue;
                    }
                    match page_id {
                        Some(p) if p != b.page => {
                            return Err(AppError::not_implemented(
                                "multi-page flows (navigate)",
                                4,
                            ));
                        }
                        _ => page_id = Some(&b.page),
                    }
                    // Actions for this device. Touch screens have no hover and
                    // no visible pointer, so only clicks (taps) and waits remain.
                    let mut actions = Vec::new();
                    for (index, action) in b.actions.iter().enumerate() {
                        if !action.applies_to(&target.id, preset) {
                            continue;
                        }
                        if touch && matches!(action.kind, ActionKind::Hover | ActionKind::Move) {
                            warnings.push(format!(
                                "block '{}': actions[{index}] ({}) is skipped on touch device '{}'",
                                b.id,
                                action.kind_name(),
                                target.id
                            ));
                            continue;
                        }
                        actions.push(PlanAction {
                            index,
                            action: action.clone(),
                        });
                    }
                    blocks.push(PlanBlock {
                        id: b.id.clone(),
                        target: b.target.clone(),
                        align: ov.align.unwrap_or(b.align),
                        offset_px: ov.offset_px.unwrap_or(b.offset_px),
                        hold_ms: ov.hold_ms.or(b.hold_ms).unwrap_or(self.defaults.hold_ms),
                        arrive: edge_into(&b.id),
                        arrive_cursor: cursor_into(&b.id),
                        actions,
                    });
                }
                Node::Navigate(_) => return Err(AppError::not_implemented("navigate", 4)),
                Node::End(e) => {
                    end = Some(PlanEnd {
                        id: e.id.clone(),
                        at: e.at.clone(),
                        hold_ms: e.hold_ms,
                        arrive: edge_into(&e.id),
                        arrive_cursor: cursor_into(&e.id),
                    });
                }
            }
        }
        let (Some(start), Some(end)) = (start, end) else {
            return Err(AppError::new(
                ErrorCode::Internal,
                "plan without start or end",
            ));
        };

        // The page: the one the blocks are on, else the page of the start anchor's block.
        let anchor_page = |id: &str| match self.node(id) {
            Some(Node::Block(b)) => Some(b.page.as_str()),
            _ => None,
        };
        let page_id = page_id
            .or_else(|| anchor_page(&start.at.block))
            .ok_or_else(|| AppError::new(ErrorCode::Internal, "no page for plan"))?;
        let page = self
            .pages
            .iter()
            .find(|p| p.id == page_id)
            .cloned()
            .ok_or_else(|| AppError::new(ErrorCode::Internal, "page vanished"))?;

        // Measure only what this target needs: its blocks, anchor blocks and
        // the targets of its actions.
        let mut measure: Vec<MeasureItem> = Vec::new();
        let mut add = |id: String, locators: Vec<Locator>, within: Option<String>| {
            if !measure.iter().any(|m| m.id == id) {
                measure.push(MeasureItem {
                    id,
                    locators,
                    within,
                });
            }
        };
        for b in &blocks {
            add(b.id.clone(), b.target.clone(), None);
            for a in &b.actions {
                if !a.action.target.is_empty() {
                    let id = PlanAction::measure_id(&b.id, a.index);
                    add(id, a.action.target.clone(), Some(b.id.clone()));
                }
            }
        }
        for anchor in std::iter::once(&start.at).chain(end.at.as_ref()) {
            if let Some(Node::Block(b)) = self.node(&anchor.block) {
                add(b.id.clone(), b.target.clone(), None);
            }
        }

        Ok(FlowPlan {
            target,
            viewport,
            geometry,
            page,
            start,
            blocks,
            end,
            measure,
            cursor: self.defaults.cursor.clone(),
            touch,
            warnings,
        })
    }

    /// Node overrides for a target: viewport-preset key first, then target id
    /// (the more specific key wins field by field).
    fn overrides_for(&self, target: &Target) -> HashMap<&str, NodeOverride> {
        let mut merged: HashMap<&str, NodeOverride> = HashMap::new();
        let keys = [target.viewport.preset_name(), Some(target.id.as_str())];
        for key in keys.into_iter().flatten() {
            if let Some(ov) = self.overrides.get(key) {
                for (id, n) in &ov.nodes {
                    let slot = merged.entry(id.as_str()).or_default();
                    slot.skip |= n.skip;
                    slot.hold_ms = n.hold_ms.or(slot.hold_ms);
                    slot.align = n.align.or(slot.align);
                    slot.offset_px = n.offset_px.or(slot.offset_px);
                }
            }
        }
        merged
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flow::AnchorPoint;

    fn flow(json: &str) -> Flow {
        Flow::from_json(json).unwrap()
    }

    const BASE: &str = r#"{
      "format_version": 1,
      "meta": { "name": "t" },
      "targets": [
        { "id": "desktop", "viewport": "desktop-1920", "aspect": "16:9" },
        { "id": "mobile", "viewport": "iphone-15", "aspect": "9:16" }
      ],
      "defaults": { "hold_ms": 1000 },
      "pages": [{ "id": "home", "url": "/" }],
      "nodes": [
        { "id": "end", "kind": "end", "at": { "block": "footer", "anchor": "bottom" } },
        { "id": "b", "kind": "block", "page": "home", "target": [{ "sr": "b" }], "hold_ms": 0 },
        { "id": "start", "kind": "start", "at": { "block": "a" } },
        { "id": "a", "kind": "block", "page": "home", "target": [{ "sr": "a" }] },
        { "id": "footer", "kind": "block", "page": "home", "target": [{ "css": "footer" }] }
      ],
      "edges": [
        { "from": "b", "to": "end", "scroll": { "duration_ms": 800 } },
        { "from": "start", "to": "a" },
        { "from": "a", "to": "b", "scroll": { "easing": "linear" } }
      ],
      "overrides": { "iphone-15": { "nodes": { "a": { "hold_ms": 500 }, "b": { "skip": true } } } }
    }"#;

    #[test]
    fn orders_path_regardless_of_declaration_order() {
        let plan = flow(BASE).plan("desktop").unwrap();
        let ids: Vec<_> = plan.blocks.iter().map(|b| b.id.as_str()).collect();
        assert_eq!(ids, ["a", "b"]);
        assert_eq!(plan.blocks[0].hold_ms, 1000, "default hold");
        assert_eq!(plan.blocks[1].hold_ms, 0, "explicit pass-through");
        assert_eq!(plan.blocks[1].arrive.easing, Easing::Linear);
        assert_eq!(plan.end.arrive.duration_ms, Some(800));
        assert_eq!(
            plan.end.at.as_ref().map(|a| a.anchor),
            Some(AnchorPoint::Bottom)
        );
        assert!(
            plan.measure.iter().any(|m| m.id == "footer"),
            "anchor-only blocks are measured too"
        );
        assert_eq!(
            (plan.geometry.out_width, plan.geometry.out_height),
            (1920, 1080)
        );
    }

    #[test]
    fn applies_device_overrides() {
        let plan = flow(BASE).plan("mobile").unwrap();
        let ids: Vec<_> = plan.blocks.iter().map(|b| b.id.as_str()).collect();
        assert_eq!(ids, ["a"], "b is skipped on iphone-15");
        assert_eq!(plan.blocks[0].hold_ms, 500);
        assert_eq!(
            plan.end.arrive.duration_ms,
            Some(800),
            "arrow into end still applies"
        );
    }

    #[test]
    fn reports_all_graph_problems_at_once() {
        let broken = BASE
            .replace(r#"{ "from": "start", "to": "a" },"#, "")
            .replace(r#""target": [{ "css": "footer" }]"#, r#""target": []"#);
        let err = flow(&broken).validate().unwrap_err();
        assert_eq!(err.code, ErrorCode::FlowInvalid);
        assert!(
            err.message.contains("without reaching the end"),
            "{}",
            err.message
        );
        assert!(err.message.contains("footer"), "{}", err.message);
        assert_eq!(err.params["count"], "2");
    }

    #[test]
    fn detects_loops_and_branches() {
        let looped = BASE.replace(
            r#"{ "from": "b", "to": "end", "scroll": { "duration_ms": 800 } }"#,
            r#"{ "from": "b", "to": "a" }"#,
        );
        let err = flow(&looped).validate().unwrap_err();
        assert!(
            err.message.contains("incoming arrows") || err.message.contains("loop"),
            "{}",
            err.message
        );
    }

    #[test]
    fn actions_are_filtered_per_device_and_measured() {
        let with_actions = BASE.replace(
            r#"{ "id": "a", "kind": "block", "page": "home", "target": [{ "sr": "a" }] }"#,
            r#"{ "id": "a", "kind": "block", "page": "home", "target": [{ "sr": "a" }], "actions": [
                { "do": "hover", "target": [{ "sr": "btn" }] },
                { "do": "click", "target": [{ "sr": "btn" }], "only": ["mobile"] },
                { "do": "wait", "duration_ms": 200 }
            ] }"#,
        );
        let flow = flow(&with_actions);
        let desktop = flow.plan("desktop").unwrap();
        let kinds: Vec<_> = desktop.blocks[0].actions.iter().map(|a| a.index).collect();
        assert_eq!(kinds, [0, 2], "click is mobile-only");
        assert!(
            desktop
                .measure
                .iter()
                .any(|m| m.id == "a#0" && m.within.as_deref() == Some("a"))
        );
        assert!(!desktop.touch);

        let mobile = flow.plan("mobile").unwrap();
        assert!(mobile.touch);
        let kinds: Vec<_> = mobile.blocks[0].actions.iter().map(|a| a.index).collect();
        assert_eq!(kinds, [1, 2], "hover is skipped on touch screens");
        assert!(
            mobile
                .warnings
                .iter()
                .any(|w| w.contains("skipped on touch"))
        );
    }

    #[test]
    fn invalid_actions_are_reported() {
        let bad = BASE.replace(
            r#"{ "id": "a", "kind": "block", "page": "home", "target": [{ "sr": "a" }] }"#,
            r#"{ "id": "a", "kind": "block", "page": "home", "target": [{ "sr": "a" }], "actions": [{ "do": "click" }] }"#,
        );
        let err = flow(&bad).validate().unwrap_err();
        assert!(
            err.message
                .contains("block 'a': actions[0] (click): 'click' needs a target"),
            "{}",
            err.message
        );
    }

    #[test]
    fn later_phase_features_are_explicit() {
        let nav = BASE.replace(
            r#"{ "id": "b", "kind": "block", "page": "home", "target": [{ "sr": "b" }], "hold_ms": 0 }"#,
            r#"{ "id": "b", "kind": "navigate", "page": "home" }"#,
        );
        let err = flow(&nav).plan("desktop").unwrap_err();
        assert_eq!(err.code, ErrorCode::NotImplemented);
        assert_eq!(err.params["phase"], "4");
    }
}
