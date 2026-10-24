//! End-to-end test: scenario → local site → real browser → PNG frames.
//!
//! Proves the whole Phase 1 pipeline on every CI machine (Windows + Linux):
//! frames are produced, the page really scrolls, `timeline.json` is written,
//! and two renders are pixel-identical (determinism, HANDOFF NFR-03).
//! Runs only where Chrome/Edge/Chromium is installed; otherwise it is skipped.
//! PNG output is used so the test needs no ffmpeg.

// Test helpers may panic on failure: that is how a test reports it.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::path::{Path, PathBuf};

use sr_core::Flow;
use sr_render::{RenderOptions, Renderer};

fn fixture_site() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/sites/01-plain-landing")
}

/// Small and short on purpose: 480×320, 10 fps, about 2 s.
const FLOW: &str = r#"{
  "format_version": 1,
  "meta": { "name": "e2e" },
  "targets": [{
    "id": "small",
    "viewport": { "width": 480, "height": 320 },
    "resolution": { "scale": 1 },
    "fps": 10,
    "format": "png"
  }],
  "defaults": { "hold_ms": 300, "scroll": { "speed_px_s": 4000 } },
  "pages": [{ "id": "home", "url": "/" }],
  "nodes": [
    { "id": "start", "kind": "start", "at": { "block": "hero" } },
    { "id": "hero", "kind": "block", "page": "home", "target": [{ "sr": "hero" }] },
    { "id": "features", "kind": "block", "page": "home", "target": [{ "sr": "features" }], "hold_ms": 0 },
    { "id": "steps", "kind": "block", "page": "home", "target": [{ "sr": "steps" }] },
    { "id": "footer", "kind": "block", "page": "home", "target": [{ "sr": "footer" }] },
    { "id": "end", "kind": "end", "at": { "block": "footer", "anchor": "bottom" }, "hold_ms": 100 }
  ],
  "edges": [
    { "from": "start", "to": "hero" },
    { "from": "hero", "to": "features" },
    { "from": "features", "to": "steps" },
    { "from": "steps", "to": "end" }
  ]
}"#;

fn render_once(flow: &Flow, out: &Path) -> Vec<Vec<u8>> {
    let mut opts = RenderOptions {
        site: Some(fixture_site().display().to_string()),
        ..RenderOptions::default()
    };
    if cfg!(target_os = "linux") {
        // CI containers often forbid Chrome's sandbox; test pages are local and trusted.
        opts.launch.extra_args.push("--no-sandbox".into());
    }
    let mut renderer = Renderer::start(flow, Path::new("."), opts).expect("start renderer");
    let report = renderer
        .render(flow, "small", out, &mut |_| {})
        .expect("render");
    renderer.finish();

    assert!(
        report.frames >= 15,
        "about 2 s at 10 fps, got {}",
        report.frames
    );
    assert_eq!((report.width, report.height), (480, 320));
    assert!(report.timeline_file.is_file(), "timeline.json written");

    let mut files: Vec<_> = std::fs::read_dir(out)
        .expect("frames folder")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "png"))
        .collect();
    files.sort();
    assert_eq!(files.len() as u32, report.frames);
    files
        .iter()
        .map(|f| std::fs::read(f).expect("read frame"))
        .collect()
}

#[test]
fn renders_fixture_deterministically() {
    if sr_browser::find_browser(None).is_err() {
        eprintln!("skipped: no Chromium-based browser installed");
        return;
    }
    let flow = Flow::from_json(FLOW).expect("flow");
    let base = std::env::temp_dir().join(format!("sr-render-e2e-{}", std::process::id()));
    let first = render_once(&flow, &base.join("a"));
    let second = render_once(&flow, &base.join("b"));

    // The page moved: first and last frames differ.
    assert_ne!(first.first(), first.last(), "the video should scroll");
    // Same scenario → same pixels.
    let differing: Vec<usize> = first
        .iter()
        .zip(&second)
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(i, _)| i)
        .collect();
    assert!(
        differing.is_empty(),
        "renders differ in frames {differing:?}"
    );

    let _ = std::fs::remove_dir_all(&base);
}
