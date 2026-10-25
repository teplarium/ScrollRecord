//! Phase 1b: capture on "heavy" sites (HANDOFF §14, ADR 0004).
//!
//! One group of tests per test site in `tests/fixtures/sites/02…06`:
//! - a **behaviour test** drives the real browser frame by frame and checks
//!   the exact thing that used to break capture on that kind of site (CSS
//!   play-state reveals, GSAP pinning, Lenis velocity, React re-renders,
//!   video/GIF frames) against page time;
//! - a **render test** renders the site twice through the full pipeline and
//!   checks that the frames are identical (determinism) and that every frame
//!   of a scroll differs from the previous one (no stutter).
//!
//! Runs only where Chrome/Edge/Chromium is installed; otherwise each test
//! prints "skipped" and passes. PNG output is used, so no ffmpeg is needed.

// Test helpers may panic on failure: that is how a test reports it.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Value;
use sr_browser::{
    Browser, Emulation, FrameDriver, LaunchOptions, Page, PointerInput, PrepareOptions, PressEvent,
};
use sr_core::Flow;
use sr_render::{RenderOptions, Renderer};
use sr_site::{ServedSite, SiteSource};

// ── Shared helpers ──────────────────────────────────────────────────────────

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/sites")
        .join(name)
}

fn launch_options() -> LaunchOptions {
    let mut opts = LaunchOptions::default();
    if cfg!(target_os = "linux") {
        // CI containers often forbid Chrome's sandbox; test pages are local and trusted.
        opts.extra_args.push("--no-sandbox".into());
    }
    opts
}

fn browser() -> Option<Browser> {
    let Ok(path) = sr_browser::find_browser(None) else {
        eprintln!("skipped: no Chromium-based browser installed");
        return None;
    };
    Some(Browser::launch(&path, &launch_options()).expect("launch browser"))
}

fn serve(name: &str) -> ServedSite {
    let source = SiteSource::parse(&fixture(name).display().to_string(), Path::new("."))
        .expect("fixture folder");
    sr_site::serve(&source).expect("serve fixture")
}

/// Opens `url` like the renderer does: 800×450 desktop, then prepare.
fn open<'b>(browser: &'b mut Browser, url: &str, network_idle: bool) -> Page<'b> {
    let mut page = browser.new_page().expect("tab");
    page.emulate(&Emulation {
        css_width: 800,
        css_height: 450,
        scale: 1.0,
        mobile: false,
        touch: false,
        user_agent: None,
    })
    .expect("emulate");
    page.navigate(url, Duration::from_secs(30))
        .expect("navigate");
    page.prepare(&PrepareOptions {
        network_idle,
        ..PrepareOptions::default()
    })
    .expect("prepare");
    page
}

/// One video frame like the capture loop makes it: scroll + time, then a
/// screenshot (the browser renders, so end-of-animation events are sent).
fn frame(page: &mut Page<'_>, y: f64, dt: f64) -> Pixels {
    page.step(y, dt, &PointerInput::default()).expect("step");
    Pixels::decode(&page.capture_png().expect("screenshot"))
}

fn js(page: &mut Page<'_>, expression: &str) -> Value {
    page.eval(expression)
        .unwrap_or_else(|e| panic!("{expression}: {e:?}"))
}

fn num(page: &mut Page<'_>, expression: &str) -> f64 {
    let v = js(page, expression);
    v.as_f64()
        .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
        .unwrap_or_else(|| panic!("{expression} = {v} is not a number"))
}

fn text(page: &mut Page<'_>, expression: &str) -> String {
    let v = js(page, expression);
    v.as_str().map_or_else(|| v.to_string(), str::to_owned)
}

/// Horizontal scale or offset of an element's CSS transform.
fn matrix(page: &mut Page<'_>, selector: &str, field: &str) -> f64 {
    num(
        page,
        &format!(
            "new DOMMatrix(getComputedStyle(document.querySelector('{selector}')).transform).{field}"
        ),
    )
}

fn style(page: &mut Page<'_>, selector: &str, property: &str) -> String {
    text(
        page,
        &format!("getComputedStyle(document.querySelector('{selector}')).{property}"),
    )
}

fn assert_close(got: f64, want: f64, tolerance: f64, what: &str) {
    assert!(
        (got - want).abs() <= tolerance,
        "{what}: got {got}, want {want} ± {tolerance}"
    );
}

const GREEN: &str = "rgb(0, 160, 80)";

/// Decoded screenshot (RGB(A) pixels).
struct Pixels {
    width: usize,
    channels: usize,
    data: Vec<u8>,
}

impl Pixels {
    fn decode(png_bytes: &[u8]) -> Pixels {
        let mut dec = png::Decoder::new(std::io::Cursor::new(png_bytes));
        dec.set_transformations(png::Transformations::normalize_to_color8());
        let mut reader = dec.read_info().expect("png header");
        let mut data = vec![0; reader.output_buffer_size().expect("size")];
        let info = reader.next_frame(&mut data).expect("png data");
        let channels = match info.color_type {
            png::ColorType::Rgba => 4,
            png::ColorType::Rgb => 3,
            other => panic!("unexpected PNG colour type {other:?}"),
        };
        Pixels {
            width: info.width as usize,
            channels,
            data,
        }
    }

    fn at(&self, x: usize, y: usize) -> [u8; 3] {
        let i = (y * self.width + x) * self.channels;
        [self.data[i], self.data[i + 1], self.data[i + 2]]
    }

    /// Alpha of a pixel (255 when the screenshot has no alpha channel).
    fn alpha(&self, x: usize, y: usize) -> u8 {
        if self.channels == 4 {
            self.data[(y * self.width + x) * 4 + 3]
        } else {
            255
        }
    }

    /// Where two screenshots differ, for failure messages: "N px in
    /// x0,y0–x1,y1, max Δ d". A single row at an element's edge points to
    /// the browser's drawing (ADR 0004), a large area to the page itself.
    fn difference(a: &[u8], b: &[u8]) -> String {
        let (a, b) = (Pixels::decode(a), Pixels::decode(b));
        let height = a.data.len() / a.channels / a.width;
        let (mut count, mut max) = (0, 0);
        let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0, 0);
        for y in 0..height {
            for x in 0..a.width {
                let (pa, pb) = (a.at(x, y), b.at(x, y));
                let d = (0..3)
                    .map(|c| pa[c].abs_diff(pb[c]))
                    .chain([a.alpha(x, y).abs_diff(b.alpha(x, y))])
                    .max()
                    .unwrap_or(0);
                if d > 0 {
                    count += 1;
                    max = max.max(d);
                    (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
                }
            }
        }
        format!("{count} px in {x0},{y0}–{x1},{y1}, max Δ {max}")
    }
}

fn near(c: [u8; 3], want: [u8; 3]) -> bool {
    c.iter().zip(want).all(|(a, b)| a.abs_diff(b) <= 24)
}

// ── Render twice: identical frames, no stutter while scrolling ──────────────

/// Renders `flow` (target "small", PNG) twice and checks determinism and
/// smoothness. Returns the frames of the first render and its timeline.
fn render_twice(site: &str, flow_json: &str) -> Option<(Vec<Vec<u8>>, Value)> {
    if sr_browser::find_browser(None).is_err() {
        eprintln!("skipped: no Chromium-based browser installed");
        return None;
    }
    let flow = Flow::from_json(flow_json).expect("flow");
    let base = std::env::temp_dir().join(format!("sr-heavy-{site}-{}", std::process::id()));
    let render = |out: &Path| {
        let opts = RenderOptions {
            site: Some(fixture(site).display().to_string()),
            launch: launch_options(),
            ..RenderOptions::default()
        };
        let mut renderer = Renderer::start(&flow, Path::new("."), opts).expect("start renderer");
        let report = renderer
            .render(&flow, "small", out, &mut |_| {})
            .expect("render");
        renderer.finish();
        assert!(
            report.warnings.is_empty(),
            "{site}: unexpected warnings {:?}",
            report.warnings
        );
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
            .map(|f| std::fs::read(f).expect("frame"))
            .collect::<Vec<_>>()
    };
    let first = render(&base.join("a"));
    let second = render(&base.join("b"));
    let timeline: Value =
        serde_json::from_slice(&std::fs::read(base.join("a").join("timeline.json")).unwrap())
            .unwrap();

    // ── Same scenario → same pixels ──
    let differing: Vec<usize> = (0..first.len().min(second.len()))
        .filter(|&i| first[i] != second[i])
        .collect();
    assert_eq!(first.len(), second.len(), "{site}: frame counts differ");
    assert!(
        differing.is_empty(),
        "{site}: renders differ in frames {differing:?} ({}; frames kept in {})",
        differing
            .iter()
            .map(|&i| format!("{i}: {}", Pixels::difference(&first[i], &second[i])))
            .collect::<Vec<_>>()
            .join(", "),
        base.display()
    );

    // ── Smooth: every frame inside a scroll shows a new picture ──
    let fps = timeline["fps"].as_f64().unwrap();
    for seg in timeline["segments"].as_array().unwrap() {
        if seg["kind"] != "scroll" {
            continue;
        }
        let start = seg["start_ms"].as_f64().unwrap() * fps / 1000.0;
        let end = seg["end_ms"].as_f64().unwrap() * fps / 1000.0;
        let frames = (start.floor() as usize + 1)..(end.ceil() as usize).min(first.len());
        for i in frames {
            assert_ne!(
                first[i],
                first[i - 1],
                "{site}: frame {i} repeats the previous one during '{}'",
                seg["label"]
            );
        }
    }
    let _ = std::fs::remove_dir_all(&base);
    Some((first, timeline))
}

/// Scenario skeleton: 800×450, 10 fps, PNG, fast scrolls.
fn flow(nodes: &str, edges: &str, prepare: &str) -> String {
    format!(
        r#"{{
  "format_version": 1,
  "meta": {{ "name": "heavy" }},
  "targets": [{{ "id": "small", "viewport": {{ "width": 800, "height": 450 }},
                "resolution": {{ "scale": 1 }}, "fps": 10, "format": "png" }}],
  "defaults": {{ "hold_ms": 300, "scroll": {{ "speed_px_s": 3000 }}, "cursor": {{ "seed": 5 }} }},
  "prepare": {prepare},
  "pages": [{{ "id": "home", "url": "/" }}],
  "nodes": [{nodes}],
  "edges": [{edges}]
}}"#
    )
}

/// A straight path start → blocks… → end (at the last block's bottom).
fn straight(blocks: &[(&str, u32)], actions: &[(&str, &str)], prepare: &str) -> String {
    let first = blocks[0].0;
    let last = blocks[blocks.len() - 1].0;
    let mut nodes = vec![format!(
        r#"{{ "id": "start", "kind": "start", "at": {{ "block": "{first}" }} }}"#
    )];
    for (id, hold) in blocks {
        let acts = actions
            .iter()
            .find(|(b, _)| b == id)
            .map_or(String::new(), |(_, a)| format!(r#", "actions": [{a}]"#));
        nodes.push(format!(
            r#"{{ "id": "{id}", "kind": "block", "page": "home", "target": [{{ "sr": "{id}" }}], "hold_ms": {hold}{acts} }}"#
        ));
    }
    nodes.push(format!(
        r#"{{ "id": "end", "kind": "end", "at": {{ "block": "{last}", "anchor": "bottom" }}, "hold_ms": 100 }}"#
    ));
    let mut ids: Vec<&str> = vec!["start"];
    ids.extend(blocks.iter().map(|(id, _)| *id));
    ids.push("end");
    let edges: Vec<String> = ids
        .windows(2)
        .map(|w| format!(r#"{{ "from": "{}", "to": "{}" }}"#, w[0], w[1]))
        .collect();
    flow(&nodes.join(",\n"), &edges.join(", "), prepare)
}

// ── 02 · CSS transitions and animations ─────────────────────────────────────

#[test]
fn css_effects_follow_page_time() {
    let Some(mut browser) = browser() else { return };
    let site = serve("02-css-transitions");
    let mut page = open(&mut browser, &site.page_url("/"), false);

    // ── A long transition with a delay, started from requestAnimationFrame ──
    for _ in 0..15 {
        frame(&mut page, 0.0, 100.0);
    }
    // Started at the first frame (t = 100), waits 500 ms, fills over 2000 ms.
    assert_close(matrix(&mut page, "#bar", "a"), 0.45, 0.001, "bar at t=1500");

    // ── Effects chained on animationend → transitionend → `finished` ──
    for _ in 0..10 {
        frame(&mut page, 0.0, 100.0);
    }
    for id in ["b", "c", "d"] {
        assert_eq!(
            style(&mut page, &format!("#chain-{id}"), "backgroundColor"),
            GREEN,
            "chain box {id} must be reached (its predecessor reported its end)"
        );
    }

    // ── The computer's settings never reach the page ──
    assert_eq!(style(&mut page, "#motion-marker", "backgroundColor"), GREEN);
    assert_eq!(style(&mut page, "#scheme-marker", "backgroundColor"), GREEN);

    // ── "Paused until in view": play-state turns to running and plays ──
    assert_eq!(
        num(
            &mut page,
            "getComputedStyle(document.getElementById('ps-3')).opacity"
        ),
        0.0
    );
    for _ in 0..12 {
        frame(&mut page, 900.0, 100.0);
    }
    for id in ["ps-1", "ps-2", "ps-3"] {
        assert_eq!(
            num(
                &mut page,
                &format!("getComputedStyle(document.getElementById('{id}')).opacity")
            ),
            1.0,
            "{id} finished its entrance"
        );
    }

    // ── Scroll-driven animation follows the scroll position only ──
    // The bar is at page y 1550..1590; "cover" runs from scroll 1100 to 1590.
    frame(&mut page, 1350.0, 100.0);
    assert_close(
        matrix(&mut page, "#sd-bar", "a"),
        250.0 / 490.0,
        0.01,
        "scroll-driven at 1350",
    );
    frame(&mut page, 1700.0, 100.0);
    assert_close(
        matrix(&mut page, "#sd-bar", "a"),
        1.0,
        0.001,
        "scroll-driven at 1700",
    );

    page.close().unwrap();
    browser.close();
}

#[test]
fn css_fixture_renders_identically() {
    let flow = straight(
        &[
            ("intro", 2500),
            ("chain", 0),
            ("playstate", 1200),
            ("timeline", 0),
            ("media", 300),
        ],
        &[],
        "{}",
    );
    render_twice("02-css-transitions", &flow);
}

// ── 03 · GSAP + ScrollTrigger ───────────────────────────────────────────────

#[test]
fn gsap_scrolltrigger_follows_page_time_and_scroll() {
    let Some(mut browser) = browser() else { return };
    let site = serve("03-gsap-scrolltrigger");
    let mut page = open(&mut browser, &site.page_url("/"), false);

    // The pin spacer (+1200 px) is in place before anything is measured.
    assert_eq!(
        num(&mut page, "document.documentElement.scrollHeight"),
        3500.0
    );

    // ── Intro timeline runs on page time ──
    frame(&mut page, 0.0, 100.0);
    let early = num(
        &mut page,
        "getComputedStyle(document.querySelector('.hero__title')).opacity",
    );
    assert!(
        (0.2..0.7).contains(&early),
        "title is fading in at t=100: {early}"
    );
    for _ in 0..6 {
        frame(&mut page, 0.0, 100.0);
    }
    assert_eq!(
        num(
            &mut page,
            "getComputedStyle(document.querySelector('.hero__title')).opacity"
        ),
        1.0
    );

    // ── scrub: true depends on the scroll position only ──
    frame(&mut page, 300.0, 100.0);
    assert_close(
        matrix(&mut page, "#scrub-exact", "a"),
        300.0 / 1050.0,
        0.001,
        "scrub",
    );

    // ── Pinned section stays on screen; its row moves with the scroll ──
    frame(&mut page, 1050.0, 100.0);
    frame(&mut page, 1500.0, 100.0);
    assert_eq!(
        num(
            &mut page,
            "document.querySelector('.pin').getBoundingClientRect().top"
        ),
        0.0
    );
    assert_close(
        matrix(&mut page, "#pin-row", "e"),
        -450.0,
        0.5,
        "pinned row offset",
    );

    // ── Reveal tweens started by scrolling play to the end ──
    for _ in 0..10 {
        frame(&mut page, 2700.0, 100.0);
    }
    assert_eq!(
        num(
            &mut page,
            "getComputedStyle(document.querySelectorAll('.card')[2]).opacity"
        ),
        1.0
    );

    page.close().unwrap();
    browser.close();
}

#[test]
fn gsap_fixture_renders_identically() {
    let flow = straight(
        &[
            ("hero", 1500),
            ("scrub", 0),
            ("pin", 0),
            ("reveal", 1000),
            ("footer", 0),
        ],
        &[],
        "{}",
    );
    render_twice("03-gsap-scrolltrigger", &flow);
}

// ── 04 · Lenis smooth scroll ────────────────────────────────────────────────

/// Regression: with the old built-in web server (tiny_http) a page loading
/// six files at once often never finished loading (7 of 8 tries), and the
/// render failed with SR-3004. Every load must complete.
#[test]
fn pages_with_many_files_always_load() {
    let Some(mut browser) = browser() else { return };
    for round in 0..8 {
        let site = serve("04-lenis-smooth-scroll");
        let mut page = browser.new_page().expect("tab");
        if let Err(err) = page.navigate(&site.page_url("/"), Duration::from_secs(15)) {
            let loaded = page.eval(
                "performance.getEntriesByType('resource').map(e => e.name.split('/').pop()).join(', ')",
            );
            panic!("load {round} did not finish: {err:?}; files received: {loaded:?}");
        }
        page.close().unwrap();
    }
    browser.close();
}

#[test]
fn lenis_sees_each_scroll_position_once() {
    let Some(mut browser) = browser() else { return };
    let site = serve("04-lenis-smooth-scroll");
    let mut page = open(&mut browser, &site.page_url("/"), false);
    js(
        &mut page,
        "window.__scrolls = 0; addEventListener('scroll', () => __scrolls++); 1",
    );
    let skew = |page: &mut Page<'_>| text(page, "document.getElementById('skew').style.transform");

    // ── While scrolling: one event per frame, so velocity is not zero ──
    for (i, y) in [100.0, 200.0, 300.0].into_iter().enumerate() {
        frame(&mut page, y, 100.0);
        assert_eq!(
            num(&mut page, "__scrolls"),
            (i + 1) as f64,
            "one scroll event per frame"
        );
        assert_ne!(
            skew(&mut page),
            "skewY(0deg)",
            "Lenis velocity reached the page"
        );
    }
    assert_eq!(
        text(&mut page, "document.getElementById('topbar').className"),
        "topbar is-hidden"
    );

    // ── Standing still: no events; Lenis settles after its 400 ms ──
    for _ in 0..6 {
        frame(&mut page, 300.0, 100.0);
    }
    assert_eq!(
        num(&mut page, "__scrolls"),
        3.0,
        "no scroll events while the page stands still"
    );
    assert_eq!(skew(&mut page), "skewY(0deg)");
    assert_eq!(
        js(
            &mut page,
            "document.documentElement.classList.contains('lenis-scrolling')"
        ),
        Value::Bool(false)
    );

    page.close().unwrap();
    browser.close();
}

#[test]
fn lenis_fixture_renders_identically() {
    let flow = straight(
        &[
            ("hero", 500),
            ("speed", 600),
            ("parallax", 300),
            ("footer", 0),
        ],
        &[],
        "{}",
    );
    render_twice("04-lenis-smooth-scroll", &flow);
}

// ── 05 · React single-page app ──────────────────────────────────────────────

#[test]
fn react_app_routes_fetches_and_rerenders_in_frame() {
    let Some(mut browser) = browser() else { return };
    let site = serve("05-react-spa");

    // ── Deep link to a client-side route (needs the SPA fallback) ──
    let mut page = open(&mut browser, &site.page_url("/pricing"), false);
    assert!(text(&mut page, "document.body.innerText").starts_with("Pricing"));
    page.close().unwrap();

    let mut page = open(&mut browser, &site.page_url("/"), true);
    // ── Content from fetch() is there after "network idle" ──
    assert_eq!(
        num(
            &mut page,
            "document.querySelectorAll('.features li').length"
        ),
        3.0
    );

    // ── A React re-render on every animation frame lands in that frame ──
    let mut values = Vec::new();
    for _ in 0..14 {
        frame(&mut page, 450.0, 100.0);
        values.push(num(
            &mut page,
            "Number(document.querySelector('.stat').dataset.value)",
        ));
    }
    let counting: Vec<f64> = values
        .iter()
        .copied()
        .filter(|v| *v > 0.0 && *v < 1000.0)
        .collect();
    assert!(counting.len() >= 5, "count-up seen: {values:?}");
    assert!(
        counting.windows(2).all(|w| w[1] - w[0] == 100.0),
        "each 100 ms frame adds exactly 100: {values:?}"
    );
    assert_eq!(values.last(), Some(&1000.0));

    // ── A real click updates React state within the same frame ──
    frame(&mut page, 1350.0, 100.0);
    let rect = js(
        &mut page,
        "(() => { const r = document.querySelectorAll('.tabs__bar button')[1].getBoundingClientRect(); return [r.x + r.width / 2, r.y + r.height / 2]; })()",
    );
    let (x, y) = (rect[0].as_f64().unwrap(), rect[1].as_f64().unwrap());
    let press = |down| PressEvent {
        down,
        touch: false,
        x,
        y,
    };
    page.step(
        1350.0,
        100.0,
        &PointerInput {
            x,
            y,
            live: true,
            detect_kind: false,
            events: vec![press(true), press(false)],
        },
    )
    .unwrap();
    assert_eq!(
        text(
            &mut page,
            "document.querySelector('.tabs__panel').dataset.tab"
        ),
        "1"
    );

    page.close().unwrap();
    browser.close();
}

#[test]
fn react_fixture_renders_identically() {
    let flow = straight(
        &[
            ("hero", 800),
            ("stats", 1200),
            ("features", 0),
            ("tabs", 600),
            ("footer", 0),
        ],
        &[(
            "tabs",
            r#"{ "do": "click", "target": [{ "text": "Build" }], "mode": "real" }"#,
        )],
        r#"{ "wait": "network_idle" }"#,
    );
    render_twice("05-react-spa", &flow);
}

// ── 06 · Video, Lottie, canvas, GIF ─────────────────────────────────────────

/// Frame number shown by `media/counter.webm`: 6 black/white stripes, 64 px
/// wide on screen, video at (40, top) in viewport coordinates.
fn video_frame(px: &Pixels, top: usize) -> u32 {
    (0..6)
        .filter(|k| px.at(40 + 64 * k + 32, top + 108)[0] > 128)
        .map(|k| 1 << k)
        .sum()
}

#[test]
fn media_follows_page_time() {
    let Some(mut browser) = browser() else { return };
    let site = serve("06-media");
    let mut page = open(&mut browser, &site.page_url("/"), true);
    js(
        &mut page,
        "window.__clip = []; const c = document.getElementById('clip'); \
         for (const e of ['play', 'playing', 'pause', 'ended']) \
           c.addEventListener(e, () => __clip.push(e + '@' + __sr.now() + (c.paused ? ' paused' : ''))); 1",
    );

    // ── Autoplay video: frame = page time × 30 fps (it starts at page time 0) ──
    for i in 1..=8u32 {
        let px = frame(&mut page, 0.0, 100.0);
        assert_eq!(
            video_frame(&px, 40),
            i * 3,
            "autoplay video at t={}",
            i * 100
        );
    }

    // ── load() restarts the video; Chrome sets its rate back to 1 meanwhile ──
    // ScrollRecord must notice the real element moved by itself and put it
    // back to the media time before the next screenshot.
    js(&mut page, "document.getElementById('loop').load(); 1");
    std::thread::sleep(Duration::from_millis(700));
    for i in 0..3u32 {
        let px = frame(&mut page, 0.0, 100.0);
        assert_eq!(
            video_frame(&px, 40),
            (i + 1) * 3,
            "{} ms after load()",
            (i + 1) * 100
        );
    }

    // ── "Play when visible" video starts at frame 0 when it comes on screen ──
    for i in 0..5u32 {
        let px = frame(&mut page, 450.0, 100.0);
        assert_eq!(video_frame(&px, 40), i * 3, "clip {i} frames after play()");
    }

    // ── Lottie and canvas players draw from animation-frame timestamps ──
    frame(&mut page, 900.0, 100.0);
    // The player starts on its first animation frame (t = 100) and moves the
    // box from x = 20 to 180 every 2 s.
    let t = num(&mut page, "__sr.now()");
    let want_x = 20.0 + 160.0 * ((t - 100.0) % 2000.0) / 2000.0;
    let lottie_x = num(
        &mut page,
        "new DOMMatrix(document.querySelector('#lottie svg g g').getAttribute('transform')).e",
    );
    assert_close(lottie_x, want_x, 3.0, "lottie box x");
    let px = frame(&mut page, 1350.0, 100.0);
    let t = num(&mut page, "__sr.now()");
    let filled = (t % 2000.0) / 2000.0 * 400.0;
    let bar_end = 40 + filled as usize;
    assert!(
        near(px.at(bar_end - 4, 60), [150, 60, 200]),
        "2D canvas filled to x={bar_end}"
    );
    assert!(
        near(px.at(bar_end + 4, 60), [230, 227, 220]),
        "2D canvas empty after x={bar_end}"
    );

    // ── Animated GIF: 250 ms per colour, from the moment it is on screen ──
    let colours = [[220, 30, 30], [0, 160, 80], [40, 90, 220], [240, 200, 0]];
    for i in 0..10usize {
        let px = frame(&mut page, 1800.0, 100.0);
        let want = colours[(i * 100 / 250) % 4];
        assert!(
            near(px.at(100, 100), want),
            "GIF {} ms after appearing: {:?}, want {want:?}",
            i * 100,
            px.at(100, 100)
        );
    }

    // ── The page hears play/playing when the clip starts and pause/ended
    //    exactly 2 s (its length) of page time later ──
    for _ in 0..5 {
        frame(&mut page, 1800.0, 100.0);
    }
    let events = text(&mut page, "__clip.join(', ')");
    let start: f64 = events
        .split_once('@')
        .and_then(|(_, rest)| rest.split(|c: char| !c.is_ascii_digit()).next())
        .and_then(|n| n.parse().ok())
        .unwrap_or_else(|| panic!("clip events: {events}"));
    let end = start + 2000.0;
    assert_eq!(
        events,
        format!("play@{start}, playing@{start}, pause@{end} paused, ended@{end} paused")
    );

    page.close().unwrap();
    browser.close();
}

#[test]
fn media_fixture_renders_identically() {
    let flow = straight(
        &[
            ("video", 1000),
            ("clip", 1000),
            ("lottie", 300),
            ("canvas", 300),
            ("gif", 1000),
        ],
        &[],
        r#"{ "wait": "network_idle" }"#,
    );
    let Some((frames, _)) = render_twice("06-media", &flow) else {
        return;
    };
    // The video really plays in the recording: the first and the tenth frame
    // of the "video" stop show different video frames.
    let first = Pixels::decode(&frames[0]);
    let later = Pixels::decode(&frames[9]);
    assert_ne!(video_frame(&first, 40), video_frame(&later, 40));
}
