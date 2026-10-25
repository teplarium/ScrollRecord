//! End-to-end test: scenario → local site → real browser → PNG frames.
//!
//! Proves the whole pipeline on every CI machine (Windows + Linux): frames
//! are produced, the page really scrolls, a *real* hover changes the button
//! on the page, the cursor is drawn where the timeline says, `timeline.json`
//! is written, and two renders are pixel-identical (HANDOFF NFR-03).
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

/// Small and short on purpose: 800×450, 10 fps, about 3 s.
const FLOW: &str = r#"{
  "format_version": 1,
  "meta": { "name": "e2e" },
  "targets": [{
    "id": "small",
    "viewport": { "width": 800, "height": 450 },
    "resolution": { "scale": 1 },
    "fps": 10,
    "format": "png"
  }],
  "defaults": { "hold_ms": 300, "scroll": { "speed_px_s": 4000 }, "cursor": { "seed": 3 } },
  "pages": [{ "id": "home", "url": "/" }],
  "nodes": [
    { "id": "start", "kind": "start", "at": { "block": "hero" } },
    { "id": "hero", "kind": "block", "page": "home", "target": [{ "sr": "hero" }],
      "actions": [{ "do": "hover", "target": [{ "text": "Get started" }], "duration_ms": 500 }] },
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

/// Decoded frame: width and RGBA pixels.
struct Pixels {
    width: usize,
    rgba: Vec<u8>,
}

impl Pixels {
    fn decode(png_bytes: &[u8]) -> Pixels {
        let mut dec = png::Decoder::new(std::io::Cursor::new(png_bytes));
        dec.set_transformations(png::Transformations::normalize_to_color8());
        let mut reader = dec.read_info().expect("png header");
        let mut buf = vec![0; reader.output_buffer_size().expect("size")];
        let info = reader.next_frame(&mut buf).expect("png data");
        assert_eq!(info.color_type, png::ColorType::Rgba, "encoder writes RGBA");
        Pixels {
            width: info.width as usize,
            rgba: buf,
        }
    }

    fn at(&self, x: f64, y: f64) -> [u8; 3] {
        let i = (y.round() as usize * self.width + x.round() as usize) * 4;
        [self.rgba[i], self.rgba[i + 1], self.rgba[i + 2]]
    }
}

fn close_to(c: [u8; 3], want: [u8; 3]) -> bool {
    c.iter().zip(want).all(|(a, b)| a.abs_diff(b) <= 12)
}

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
        report.frames >= 20,
        "about 3 s at 10 fps, got {}",
        report.frames
    );
    assert_eq!((report.width, report.height), (800, 450));
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
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

    // ── Real hover and the drawn cursor, checked against timeline.json ──
    let timeline: serde_json::Value =
        serde_json::from_slice(&std::fs::read(base.join("a").join("timeline.json")).unwrap())
            .unwrap();
    let button = &timeline["page"]["blocks"]["hero#0"];
    let num = |v: &serde_json::Value| v.as_f64().expect("number");
    let hold = timeline["segments"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["label"] == "hold hero")
        .expect("hero stop");
    let landed = &timeline["cursor"]["segments"][0]["to"];
    // Last frame of the hero stop: the cursor has rested on the button.
    let frame = (num(&hold["end_ms"]) / 100.0).floor() as usize - 1;
    let scroll = num(&hold["from_y"]);
    let (bx, by) = (
        num(&button["left"]) + 8.0,
        num(&button["top"]) + num(&button["height"]) / 2.0 - scroll,
    );
    let start = Pixels::decode(&first[0]);
    let hovered = Pixels::decode(&first[frame]);
    assert!(
        close_to(start.at(bx, by), [0x15, 0x17, 0x1c]),
        "button is dark before the hover: {:?}",
        start.at(bx, by)
    );
    assert!(
        close_to(hovered.at(bx, by), [0xff, 0x5e, 0x3a]),
        "the page received the hover (button turned orange): {:?}",
        hovered.at(bx, by)
    );
    let (cx, cy) = (num(&landed["x"]), num(&landed["y"]));
    assert!(
        hovered.at(cx + 2.0, cy + 9.0).iter().all(|&c| c < 60),
        "the cursor is drawn at its planned spot: {:?}",
        hovered.at(cx + 2.0, cy + 9.0)
    );

    let _ = std::fs::remove_dir_all(&base);
}
