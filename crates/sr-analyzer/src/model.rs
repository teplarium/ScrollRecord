//! The site map's data, shared with the UI (exported to TypeScript).
//!
//! Field names are camelCase on the wire; `SiteBlock` and `SiteElement` are
//! also what `js/sitemap.js` returns, so a renamed field must change there too.

use serde::{Deserialize, Serialize};
use sr_core::flow::Locator;
use ts_rs::TS;

/// Site map of one device (target): every page of the scenario.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct SiteMap {
    /// Target (video format) id the pages were opened for.
    pub target: String,
    /// The emulated screen.
    pub viewport: SiteViewport,
    /// Pages in scenario order.
    pub pages: Vec<PageMap>,
    /// How long the analysis took.
    #[ts(type = "number")]
    pub elapsed_ms: u64,
}

/// The emulated screen, in CSS px.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct SiteViewport {
    /// Width in CSS px.
    pub width: u32,
    /// Height in CSS px.
    pub height: u32,
    /// Mobile layout.
    pub mobile: bool,
    /// Touch device (no hover).
    pub touch: bool,
}

/// One page of the site.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct PageMap {
    /// Page id from the scenario.
    pub page: String,
    /// Its path or address.
    pub url: String,
    /// Full page height in CSS px.
    pub scroll_height: f64,
    /// Blocks top to bottom; blocks not found on this device come last.
    pub blocks: Vec<SiteBlock>,
    /// Things worth telling the user (unfound scenario blocks, bad `hide` selectors).
    pub warnings: Vec<String>,
}

/// A block: a `data-sr` element and/or a block the scenario names.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct SiteBlock {
    /// `data-sr` name, or the scenario node id for unmarked blocks.
    pub name: String,
    /// Marked with `data-sr` on the site.
    pub marked: bool,
    /// Scenario block node that points at it, if any.
    pub node: Option<String>,
    /// How the scenario finds it (`[{ "sr": name }]` for marked blocks).
    pub locators: Vec<Locator>,
    /// Found on this device?
    pub status: BlockStatus,
    /// Box in page coordinates (CSS px); `None` when not found.
    pub rect: Option<Rect>,
    /// Interactive elements inside it (not inside a nested block), in page order.
    pub elements: Vec<SiteElement>,
    /// For unfound blocks: what each locator matched.
    pub tried: Vec<String>,
}

/// Whether a block was found on this device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum BlockStatus {
    /// Exactly one element.
    Found,
    /// No element (e.g. hidden on phones).
    NotFound,
    /// Several elements, none unique.
    Ambiguous,
}

/// An interactive element the cursor can hover, click or drag.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct SiteElement {
    /// What it is.
    pub kind: ElementKind,
    /// Its visible name (accessible name, placeholder or tag), at most 80 chars.
    pub label: String,
    /// Box in page coordinates (CSS px).
    pub rect: Rect,
    /// Locators that find exactly this element inside its block, most robust
    /// first. Empty if none is unique (the UI says so).
    pub locators: Vec<Locator>,
}

/// Kinds of interactive elements. An open list (HANDOFF D-38): plain text or
/// other kinds can be added later.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "lowercase")]
pub enum ElementKind {
    /// Button, submit, menu item.
    Button,
    /// Link.
    Link,
    /// Text, email, date… field.
    Input,
    /// Checkbox or switch.
    Checkbox,
    /// Radio button.
    Radio,
    /// Drop-down list or combobox.
    Select,
    /// Multi-line text or an editable area.
    Textarea,
    /// Tab of a tab bar.
    Tab,
    /// Slider (range).
    Slider,
    /// Something that can be dragged.
    Draggable,
    /// Anything else focusable.
    Other,
}

/// A box in page coordinates, CSS px.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Rect {
    /// Left edge.
    pub x: f64,
    /// Top edge (from the top of the page, not the screen).
    pub y: f64,
    /// Width.
    pub width: f64,
    /// Height.
    pub height: f64,
}
