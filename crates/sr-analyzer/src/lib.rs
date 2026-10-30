//! # sr-analyzer — the editor's site map
//!
//! Purpose: opens each page of a scenario exactly like the renderer does
//!          (same site server, browser, device emulation and `prepare`) and
//!          lists its **blocks** (`data-sr` elements + blocks the scenario
//!          names) and the **interactive elements** inside each block, with
//!          boxes and locator candidates (HANDOFF D-38, D-39; ADR 0006 §5 and
//!          its 2026-10-29 update).
//! Solves:  the editor offers blocks and buttons to drag onto the canvas
//!          without the user typing selectors, and what it offers is what the
//!          recording will find: same page state, same locator engine.
//! Rules:   read-only on the page (never scrolls or clicks); no heuristic
//!          block auto-detection (Phase 4, FR-BLK-06); works on an empty
//!          scenario (a new project), so it doesn't go through `Flow::plan`;
//!          checks the cancel flag between steps (SR-0004).
//! Related: `js/sitemap.js` (the in-page part), `sr-browser/js/runtime.js`
//!          (`__sr.locate`, `__sr.measure`), `sr-render` (same page set-up).

mod cache;
mod model;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use sr_browser::{Browser, Emulation, LaunchOptions, PrepareOptions};
use sr_core::flow::{Locator, Node, WaitCondition};
use sr_core::{AppError, AppResult, ErrorCode, Flow};
use sr_site::SiteSource;
use ts_rs::TS;

pub use cache::{SiteMapEvent, SiteMapJob, clear as clear_cache, fingerprint, site_map_job};
pub use model::{
    BlockStatus, ElementKind, PageMap, Rect, SiteBlock, SiteElement, SiteMap, SiteViewport,
};

/// The in-page script; `__INPUT__` is replaced with the scenario's blocks.
pub(crate) const SITEMAP_JS: &str = include_str!("../js/sitemap.js");

/// How to run an analysis.
#[derive(Debug, Clone)]
pub struct AnalyzeOptions {
    /// Browser executable; `None` = found automatically (`SR_BROWSER`, then
    /// installed Chrome / Edge / Chromium).
    pub browser: Option<PathBuf>,
    /// Browser start options (tests add `--no-sandbox` on Linux CI).
    pub launch: LaunchOptions,
    /// Longest wait for one page to load.
    pub page_timeout: Duration,
}

impl Default for AnalyzeOptions {
    fn default() -> Self {
        Self {
            browser: None,
            launch: LaunchOptions::default(),
            page_timeout: Duration::from_secs(45),
        }
    }
}

/// What the analysis is doing now (for a progress line in the UI).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum AnalyzeStage {
    /// Serving the site and starting the browser.
    StartingBrowser,
    /// Loading and preparing a page.
    OpeningPage,
    /// Listing the page's blocks and elements.
    ReadingPage,
}

/// Progress of one analysis: `done` of `total` steps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzeProgress {
    /// Steps finished.
    pub done: u32,
    /// All steps (1 + 2 per page).
    pub total: u32,
    /// Current step.
    pub stage: AnalyzeStage,
    /// Page id of the current step, if it is about a page.
    pub page: Option<String>,
}

/// One scenario block, as `__sr.measure` takes it.
#[derive(Serialize)]
struct ScenarioBlock<'a> {
    id: &'a str,
    locators: &'a [Locator],
}

/// What `sitemap.js` returns for one page.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPage {
    scroll_height: f64,
    blocks: Vec<SiteBlock>,
}

/// Builds the site map of every page of `flow` for target `target_id`.
///
/// `project_dir` is where a relative `meta.site` starts (the project or
/// scenario folder). `cancel` is checked between steps; `on_progress` is
/// called before each step.
///
/// Errors: SR-8001…8003 (site), SR-2001-style `FlowInvalid` (unknown target
/// or device), SR-3001…3005 (browser), SR-0004 (cancelled).
pub fn analyze(
    flow: &Flow,
    project_dir: &Path,
    target_id: &str,
    opts: &AnalyzeOptions,
    cancel: &AtomicBool,
    on_progress: &mut dyn FnMut(AnalyzeProgress),
) -> AppResult<SiteMap> {
    let started = Instant::now();
    let total = 1 + 2 * u32::try_from(flow.pages.len()).unwrap_or(u32::MAX / 2);
    let mut done = 0;
    let mut step = |stage, page: Option<&str>| -> AppResult<()> {
        if cancel.load(Ordering::Relaxed) {
            return Err(AppError::new(
                ErrorCode::Cancelled,
                "site analysis cancelled",
            ));
        }
        on_progress(AnalyzeProgress {
            done,
            total,
            stage,
            page: page.map(str::to_owned),
        });
        done += 1;
        Ok(())
    };

    // ── Device: the same numbers the renderer emulates ──
    let target = flow.target(target_id)?;
    let viewport = target
        .viewport
        .resolve()
        .map_err(|e| AppError::new(ErrorCode::FlowInvalid, e))?;
    let geometry = target
        .geometry(&viewport)
        .map_err(|e| AppError::new(ErrorCode::FlowInvalid, e))?;
    let emulation = Emulation {
        css_width: geometry.css_width,
        css_height: geometry.css_height,
        scale: geometry.scale,
        mobile: viewport.mobile,
        touch: viewport.has_touch(),
        user_agent: viewport.user_agent.clone(),
    };
    let prepare = PrepareOptions {
        hide: flow.prepare.hide.clone(),
        css: flow.prepare.css.clone(),
        preload_lazy: flow.prepare.preload_lazy,
        network_idle: flow.prepare.wait == WaitCondition::NetworkIdle,
    };

    // ── Site and browser, once for all pages ──
    step(AnalyzeStage::StartingBrowser, None)?;
    let Some(spec) = &flow.meta.site else {
        return Err(AppError::new(
            ErrorCode::SiteNotSpecified,
            "the scenario names no website (meta.site)",
        ));
    };
    let site = sr_site::serve(&SiteSource::parse(spec, project_dir)?)?;
    let exe = sr_browser::find_browser(opts.browser.as_deref())?;
    let mut browser = Browser::launch(&exe, &opts.launch)?;

    let mut pages = Vec::with_capacity(flow.pages.len());
    for page in &flow.pages {
        // ── Open like the renderer: device, load, prepare ──
        step(AnalyzeStage::OpeningPage, Some(&page.id))?;
        let mut tab = browser.new_page()?;
        tab.emulate(&emulation)?;
        let url = site.page_url(&page.url);
        tab.navigate(&url, opts.page_timeout)?;
        let prepared = tab.prepare(&prepare)?;
        let mut warnings: Vec<String> = prepared
            .invalid_selectors
            .into_iter()
            .map(|sel| format!("prepare.hide: '{sel}' is not a valid CSS selector (ignored)"))
            .collect();

        // ── Read blocks and elements ──
        step(AnalyzeStage::ReadingPage, Some(&page.id))?;
        let scenario: Vec<ScenarioBlock<'_>> = flow
            .nodes
            .iter()
            .filter_map(|node| match node {
                Node::Block(b) if b.page == page.id => Some(ScenarioBlock {
                    id: &b.id,
                    locators: &b.target,
                }),
                _ => None,
            })
            .collect();
        let input = serde_json::json!({ "scenario": scenario });
        let value = tab.eval(&SITEMAP_JS.replace("__INPUT__", &input.to_string()))?;
        let raw: RawPage = serde_json::from_value(value).map_err(|err| {
            AppError::new(
                ErrorCode::Internal,
                format!("site map script result: {err}"),
            )
        })?;
        for block in raw.blocks.iter().filter(|b| b.status != BlockStatus::Found) {
            warnings.push(format!(
                "block '{}' of the scenario was not found on this device ({})",
                block.name,
                block.tried.join("; ")
            ));
        }
        if let Err(err) = tab.close() {
            tracing::debug!(error = %err, "closing the analysis tab failed");
        }
        pages.push(PageMap {
            page: page.id.clone(),
            url: page.url.clone(),
            scroll_height: raw.scroll_height,
            blocks: raw.blocks,
            warnings,
        });
    }
    browser.close();

    Ok(SiteMap {
        target: target.id.clone(),
        viewport: SiteViewport {
            width: geometry.css_width,
            height: geometry.css_height,
            mobile: viewport.mobile,
            touch: viewport.has_touch(),
        },
        pages,
        elapsed_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
    })
}
