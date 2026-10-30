//! Site map on the real test sites (`tests/fixtures/sites`), in a real browser.
//!
//! The key promise checked here: every locator the site map suggests finds
//! exactly that element when the recording looks for it, because the check
//! goes through the renderer's own `__sr.measure` on a freshly opened page.
//!
//! Runs only where Chrome/Edge/Chromium is installed; otherwise each test
//! prints "skipped" and passes.

// Test helpers may panic on failure: that is how a test reports it.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use sr_analyzer::{
    AnalyzeOptions, AnalyzeStage, BlockStatus, ElementKind, SiteBlock, SiteMap, analyze,
};
use sr_browser::{BlockMeasure, Browser, Emulation, LaunchOptions, PrepareOptions};
use sr_core::flow::MeasureItem;
use sr_core::{ErrorCode, Flow};
use sr_site::SiteSource;

// ── Helpers ─────────────────────────────────────────────────────────────────

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn options() -> Option<AnalyzeOptions> {
    if sr_browser::find_browser(None).is_err() {
        eprintln!("skipped: no Chromium-based browser installed");
        return None;
    }
    let mut launch = LaunchOptions::default();
    if cfg!(target_os = "linux") {
        // CI containers often forbid Chrome's sandbox; test pages are local and trusted.
        launch.extra_args.push("--no-sandbox".into());
    }
    Some(AnalyzeOptions {
        launch,
        ..AnalyzeOptions::default()
    })
}

/// An example scenario (its `meta.site` is relative to `examples/`).
fn example(name: &str) -> Flow {
    let text = std::fs::read_to_string(repo().join("examples").join(name)).unwrap();
    Flow::from_json(&text).unwrap()
}

fn run(flow: &Flow, target: &str, opts: &AnalyzeOptions) -> SiteMap {
    let cancel = AtomicBool::new(false);
    analyze(
        flow,
        &repo().join("examples"),
        target,
        opts,
        &cancel,
        &mut |_| {},
    )
    .expect("site map")
}

fn block<'a>(map: &'a SiteMap, name: &str) -> &'a SiteBlock {
    map.pages[0]
        .blocks
        .iter()
        .find(|b| b.name == name)
        .unwrap_or_else(|| panic!("no block '{name}'"))
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[test]
fn plain_landing_lists_marked_blocks_in_page_order_with_their_buttons() {
    let Some(opts) = options() else { return };
    let flow = example("01-plain-landing.srflow.json");
    let map = run(&flow, "desktop", &opts);

    assert_eq!(map.target, "desktop");
    assert_eq!((map.viewport.width, map.viewport.mobile), (1920, false));
    let page = &map.pages[0];
    let names: Vec<&str> = page.blocks.iter().map(|b| b.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "header", "hero", "features", "steps", "quote", "pricing", "footer"
        ]
    );
    assert!(page.warnings.is_empty(), "{:?}", page.warnings);

    // Marked blocks the scenario uses are one block with its node id.
    let hero = block(&map, "hero");
    assert!(hero.marked && hero.status == BlockStatus::Found);
    assert_eq!(hero.node.as_deref(), Some("hero"));
    assert_eq!(block(&map, "header").node, None);

    // The buttons the example scenario hovers and clicks are listed.
    let start = hero
        .elements
        .iter()
        .find(|e| e.label == "Get started")
        .unwrap();
    assert_eq!(start.kind, ElementKind::Link);
    assert!(!start.locators.is_empty());
    let pricing = block(&map, "pricing");
    assert!(pricing.elements.iter().any(|e| e.label == "Choose Pro"));
}

#[test]
fn every_suggested_locator_finds_exactly_its_element_when_recording() {
    let Some(opts) = options() else { return };
    let flow = example("01-plain-landing.srflow.json");
    let map = run(&flow, "desktop", &opts);

    // ── Measure every candidate with the recording's own engine ──
    let mut items = Vec::new();
    let mut expected = Vec::new();
    for b in map.pages[0].blocks.iter() {
        items.push(MeasureItem {
            id: b.name.clone(),
            locators: b.locators.clone(),
            within: None,
        });
        for (i, element) in b.elements.iter().enumerate() {
            for (j, loc) in element.locators.iter().enumerate() {
                let id = format!("{}#{i}.{j}", b.name);
                items.push(MeasureItem {
                    id: id.clone(),
                    locators: vec![loc.clone()],
                    within: Some(b.name.clone()),
                });
                expected.push((id, element.rect));
            }
        }
    }
    assert!(
        expected.len() > 10,
        "too few candidates: {}",
        expected.len()
    );

    let mut browser =
        Browser::launch(&sr_browser::find_browser(None).unwrap(), &opts.launch).expect("browser");
    let site = sr_site::serve(
        &SiteSource::parse(
            "../tests/fixtures/sites/01-plain-landing",
            &repo().join("examples"),
        )
        .unwrap(),
    )
    .unwrap();
    let mut page = browser.new_page().unwrap();
    page.emulate(&Emulation {
        css_width: 1920,
        css_height: 1080,
        scale: 1.0,
        mobile: false,
        touch: false,
        user_agent: None,
    })
    .unwrap();
    page.navigate(&site.page_url("/"), Duration::from_secs(30))
        .unwrap();
    page.prepare(&PrepareOptions::default()).unwrap();
    let measured = page.measure(&items).unwrap();

    for (id, rect) in expected {
        let m = &measured.blocks[&id];
        let BlockMeasure::Found { top, left, .. } = m else {
            panic!("{id}: {m:?}");
        };
        assert!(
            (top - rect.y).abs() < 1.0 && (left - rect.x).abs() < 1.0,
            "{id} found another element at ({left}, {top}), expected {rect:?}"
        );
    }
}

#[test]
fn a_scenario_block_missing_on_the_site_is_listed_and_reported() {
    let Some(opts) = options() else { return };
    let mut flow = example("01-plain-landing.srflow.json");
    let text = flow
        .to_json()
        .unwrap()
        .replace(r#""sr": "features""#, r#""sr": "no-such-block""#);
    flow = Flow::from_json(&text).unwrap();
    let map = run(&flow, "desktop", &opts);

    let page = &map.pages[0];
    let missing = page.blocks.last().unwrap();
    assert_eq!(missing.name, "features");
    assert_eq!(missing.status, BlockStatus::NotFound);
    assert!(missing.rect.is_none() && !missing.marked);
    assert!(
        page.warnings[0].contains("'features'"),
        "{:?}",
        page.warnings
    );
    // The marked block of the same name is still there (it just isn't the
    // scenario's "features" any more).
    assert!(page.blocks.iter().any(|b| b.name == "features" && b.marked));
}

#[test]
fn an_empty_scenario_still_gets_a_site_map_and_progress() {
    let Some(opts) = options() else { return };
    let mut flow = example("01-plain-landing.srflow.json");
    flow.nodes.clear();
    flow.edges.clear();
    let cancel = AtomicBool::new(false);
    let mut stages = Vec::new();
    let map = analyze(
        &flow,
        &repo().join("examples"),
        "mobile",
        &opts,
        &cancel,
        &mut |p| {
            stages.push((p.done, p.total, p.stage));
        },
    )
    .unwrap();

    assert!(map.viewport.mobile && map.viewport.touch);
    assert_eq!(map.pages[0].blocks.len(), 7);
    assert!(map.pages[0].blocks.iter().all(|b| b.node.is_none()));
    assert_eq!(
        stages,
        [
            (0, 3, AnalyzeStage::StartingBrowser),
            (1, 3, AnalyzeStage::OpeningPage),
            (2, 3, AnalyzeStage::ReadingPage),
        ]
    );
}

#[test]
fn react_blocks_rendered_after_load_are_found() {
    let Some(opts) = options() else { return };
    let flow = example("05-react-spa.srflow.json");
    let target = flow.targets[0].id.clone();
    let map = run(&flow, &target, &opts);

    let names: Vec<&str> = map.pages[0]
        .blocks
        .iter()
        .map(|b| b.name.as_str())
        .collect();
    for want in ["hero", "stats", "tabs", "footer"] {
        assert!(names.contains(&want), "{want} missing in {names:?}");
    }
    let tabs = block(&map, "tabs");
    assert!(
        tabs.elements.iter().any(|e| e.kind == ElementKind::Tab),
        "{:?}",
        tabs.elements
    );
}

#[test]
fn cancelling_stops_with_sr_0004() {
    let Some(opts) = options() else { return };
    let flow = example("01-plain-landing.srflow.json");
    let cancel = AtomicBool::new(true);
    let err = analyze(
        &flow,
        &repo().join("examples"),
        "desktop",
        &opts,
        &cancel,
        &mut |_| {},
    )
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::Cancelled);
}
