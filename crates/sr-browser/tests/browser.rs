//! Integration test against a real browser.
//!
//! Runs only when Chrome/Edge/Chromium is installed (CI runners have Chrome);
//! otherwise it prints "skipped" and passes, so `cargo test` works anywhere.

// Test helpers may panic on failure: that is how a test reports it.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::time::Duration;

use sr_browser::{
    BlockMeasure, Browser, Emulation, FrameDriver, LaunchOptions, PointerInput, PrepareOptions,
    PressEvent, find_browser,
};
use sr_core::flow::{CursorKind, Locator, MeasureItem};

const PAGE: &str = "data:text/html,<html><head><style>body{margin:0} \
    .pulse{animation:fade 1000ms linear infinite} @keyframes fade{to{opacity:0}}</style></head>\
    <body><div data-sr='a' style='height:1000px'>A</div>\
    <div data-sr='b' class='pulse' style='height:600px'>B</div>\
    <div class='dup'>x</div><div class='dup'>y</div></body></html>";

fn start() -> Option<Browser> {
    start_with(&[])
}

/// Like `start`, with extra browser flags.
fn start_with(extra: &[&str]) -> Option<Browser> {
    start_custom(|opts| {
        opts.extra_args
            .extend(extra.iter().map(|a| (*a).to_owned()));
    })
}

/// Like `start`, with changed launch options.
fn start_custom(change: impl FnOnce(&mut LaunchOptions)) -> Option<Browser> {
    let Ok(path) = find_browser(None) else {
        eprintln!("skipped: no Chromium-based browser installed");
        return None;
    };
    let mut opts = LaunchOptions::default();
    if cfg!(target_os = "linux") {
        // CI containers often forbid Chrome's sandbox; the test page is local and trusted.
        opts.extra_args.push("--no-sandbox".into());
    }
    change(&mut opts);
    Some(Browser::launch(&path, &opts).expect("launch"))
}

/// The OS priority of a process: the Windows priority class
/// ("Normal", "BelowNormal") or the Unix niceness ("0", "10").
fn priority(pid: u32) -> String {
    let out = if cfg!(windows) {
        std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command"])
            .arg(format!("(Get-Process -Id {pid}).PriorityClass"))
            .output()
    } else {
        std::process::Command::new("ps")
            .args(["-o", "ni=", "-p", &pid.to_string()])
            .output()
    }
    .expect("ask the OS for the priority");
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

/// Scroll + time only, no pointer.
fn step(page: &mut impl FrameDriver, y: f64, dt: f64) -> f64 {
    page.step(y, dt, &PointerInput::default()).unwrap().scroll_y
}

fn item(id: &str, locators: Vec<Locator>, within: Option<&str>) -> MeasureItem {
    MeasureItem {
        id: id.into(),
        locators,
        within: within.map(str::to_owned),
    }
}

#[test]
fn virtual_time_scrolling_measuring_and_capture() {
    let Some(mut browser) = start() else { return };
    let mut page = browser.new_page().expect("page");
    page.emulate(&Emulation {
        css_width: 800,
        css_height: 600,
        scale: 1.0,
        mobile: false,
        touch: false,
        user_agent: None,
    })
    .expect("emulate");
    page.navigate(PAGE, Duration::from_secs(30))
        .expect("navigate");
    page.prepare(&PrepareOptions::default()).expect("prepare");

    // ── Time is frozen until we advance it ──
    assert_eq!(page.eval("performance.now()").unwrap(), 0.0);
    page.eval("window.hits = 0; setTimeout(() => hits++, 50); 1")
        .unwrap();
    step(&mut page, 0.0, 40.0);
    assert_eq!(
        page.eval("hits").unwrap(),
        0.0,
        "timer must not fire before 50ms"
    );
    step(&mut page, 0.0, 20.0);
    assert_eq!(
        page.eval("hits").unwrap(),
        1.0,
        "timer fires once page time passes 50ms"
    );

    // ── CSS animations follow page time exactly ──
    let t = page
        .eval("document.getAnimations()[0].currentTime")
        .unwrap();
    assert!(
        (t.as_f64().unwrap() - 60.0).abs() < 0.5,
        "animation time {t} should equal page time"
    );

    // ── Scrolling is applied and clamped by the browser ──
    assert_eq!(step(&mut page, 700.0, 16.0), 700.0);
    let max = page
        .eval("document.documentElement.scrollHeight - innerHeight")
        .unwrap();
    assert_eq!(
        step(&mut page, 99_999.0, 16.0),
        max.as_f64().unwrap(),
        "clamped at the bottom"
    );

    // ── Measuring blocks ──
    let blocks = vec![
        item("a", vec![Locator::sr("a")], None),
        item("b", vec![Locator::sr("missing"), Locator::sr("b")], None),
        item(
            "dup",
            vec![Locator {
                css: Some(".dup".into()),
                ..Locator::default()
            }],
            None,
        ),
        item("none", vec![Locator::sr("nope")], None),
    ];
    step(&mut page, 0.0, 0.0);
    let m = page.measure(&blocks).expect("measure");
    assert_eq!(m.viewport_height, 600.0);
    assert!(m.scroll_height >= 1600.0);
    assert_eq!(
        m.blocks["a"],
        BlockMeasure::Found {
            top: 0.0,
            height: 1000.0,
            left: 0.0,
            width: 800.0,
            matched: 0
        }
    );
    assert_eq!(
        m.blocks["b"],
        BlockMeasure::Found {
            top: 1000.0,
            height: 600.0,
            left: 0.0,
            width: 800.0,
            matched: 1
        }
    );
    assert!(matches!(m.blocks["dup"], BlockMeasure::Ambiguous { .. }));
    assert!(matches!(m.blocks["none"], BlockMeasure::NotFound { .. }));

    // ── Screenshot is a PNG of the viewport ──
    let png = page.capture_png().expect("capture");
    assert_eq!(&png[1..4], b"PNG");
    let width = u32::from_be_bytes([png[16], png[17], png[18], png[19]]);
    let height = u32::from_be_bytes([png[20], png[21], png[22], png[23]]);
    assert_eq!((width, height), (800, 600));

    page.close().expect("close tab");
    browser.close();
}

/// Two sections with a "Buy" button each; the second slides in on reveal.
const POINTER_PAGE: &str = "data:text/html,<!doctype html><html><head><style>\
    body{margin:0;font:16px sans-serif} section{height:600px;position:relative}\
    .btn{position:absolute;left:100px;top:100px;width:200px;height:50px;background:rgb(0,0,255);cursor:pointer}\
    .btn:hover{background:rgb(255,0,0)}\
    .slide{transform:translateY(80px);transition:transform 300ms linear}.slide.in{transform:none}\
    </style></head><body>\
    <section data-sr='one'><p style='margin:0;padding:300px 0 0 20px'>Plain words here</p>\
      <a class='btn' href='%23'>Buy</a></section>\
    <section data-sr='two'><div class='slide'><a class='btn' href='%23'>Buy</a></div></section>\
    <div style='height:600px'></div>\
    <script>new IntersectionObserver(es=>es.forEach(e=>e.isIntersecting&&e.target.classList.add('in')))\
      .observe(document.querySelector('.slide'));\
      window.clicks=0;document.addEventListener('click',e=>{e.preventDefault();clicks++})</script>\
    </body></html>";

#[test]
fn pointer_hover_click_cursor_kind_and_probe() {
    let Some(mut browser) = start() else { return };
    let mut page = browser.new_page().expect("page");
    page.emulate(&Emulation {
        css_width: 800,
        css_height: 600,
        scale: 1.0,
        mobile: false,
        touch: false,
        user_agent: None,
    })
    .unwrap();
    page.navigate(POINTER_PAGE, Duration::from_secs(30))
        .unwrap();
    page.prepare(&PrepareOptions::default()).unwrap();
    let bg = |page: &mut sr_browser::Page<'_>, n: usize| {
        page.eval(&format!(
            "getComputedStyle(document.querySelectorAll('.btn')[{n}]).backgroundColor"
        ))
        .unwrap()
    };

    // ── Action targets are searched inside their block ──
    let buy = || {
        vec![Locator {
            text: Some("Buy".into()),
            ..Locator::default()
        }]
    };
    let items = [
        item("one", vec![Locator::sr("one")], None),
        item("two", vec![Locator::sr("two")], None),
        item("two#0", buy(), Some("two")),
    ];
    let m = page.measure(&items).unwrap();
    let BlockMeasure::Found { top: before, .. } = m.blocks["two#0"] else {
        panic!("{:?}", m.blocks["two#0"])
    };
    assert_eq!(before, 600.0 + 100.0 + 80.0, "measured before the slide-in");

    // ── Probe: after the reveal the button sits 80px higher ──
    let probed = page.probe(600.0, 2000.0, &items).unwrap();
    let BlockMeasure::Found {
        top, left, width, ..
    } = probed.blocks["two#0"]
    else {
        panic!()
    };
    assert_eq!((top, left, width), (700.0, 100.0, 200.0));

    // Fresh page for the pointer part (the probe changed it).
    page.navigate(POINTER_PAGE, Duration::from_secs(30))
        .unwrap();
    page.prepare(&PrepareOptions::default()).unwrap();

    // ── Live pointer: hover reacts at once, cursor kind follows CSS ──
    let at = |x: f64, y: f64, live: bool| PointerInput {
        x,
        y,
        live,
        detect_kind: true,
        events: Vec::new(),
    };
    let r = page.step(0.0, 16.0, &at(150.0, 120.0, true)).unwrap();
    assert_eq!(r.cursor_kind, Some(CursorKind::Pointer));
    assert_eq!(
        bg(&mut page, 0),
        "rgb(255, 0, 0)",
        "hovered in the same frame"
    );
    let r = page.step(0.0, 16.0, &at(40.0, 309.0, true)).unwrap();
    assert_eq!(
        r.cursor_kind,
        Some(CursorKind::Text),
        "I-beam over plain text"
    );
    assert_eq!(bg(&mut page, 0), "rgb(0, 0, 255)");
    let r = page.step(0.0, 16.0, &at(600.0, 500.0, true)).unwrap();
    assert_eq!(r.cursor_kind, Some(CursorKind::Arrow));

    // ── A still pointer over scrolling content re-hovers deterministically ──
    page.step(0.0, 16.0, &at(150.0, 120.0, true)).unwrap();
    page.step(600.0, 16.0, &at(150.0, 120.0, true)).unwrap();
    assert_eq!(
        bg(&mut page, 0),
        "rgb(0, 0, 255)",
        "first button scrolled away"
    );

    // ── Visual mode takes the mouse off the page ──
    page.step(0.0, 16.0, &at(150.0, 120.0, false)).unwrap();
    assert_eq!(bg(&mut page, 0), "rgb(0, 0, 255)");

    // ── Real click ──
    let mut click = at(150.0, 120.0, true);
    click.events = vec![
        PressEvent {
            down: true,
            touch: false,
            x: 150.0,
            y: 120.0,
        },
        PressEvent {
            down: false,
            touch: false,
            x: 150.0,
            y: 120.0,
        },
    ];
    page.step(0.0, 16.0, &click).unwrap();
    assert_eq!(page.eval("clicks").unwrap(), 1.0);

    page.close().unwrap();
    browser.close();
}

/// A green card with a letter on it: an element with text on an opaque
/// background, like the GSAP reveal cards of fixture 03.
const CARD_PAGE: &str = "data:text/html,<!doctype html><html><head><style>\
    body{margin:0;background:rgb(251,250,247)}\
    .card{width:160px;height:160px;margin:40px;display:grid;place-items:center;\
    font:40px sans-serif;color:white;background:rgb(0,160,80)}\
    </style></head><body><div class='card' id='c'>A</div></body></html>";

/// Regression (CI on Windows, phase-3-plan step 1b-fix, ADR 0004): without a
/// GPU, Chrome drew the edge of a card that moved by a fraction of a pixel
/// from leftover tile memory, so the same frame differed between renders
/// depending on what that memory held. Here the card moves 20px → 8.8889px
/// once right away and once after 7 s of idling: Chrome frees unused tile
/// memory after 5 s, so the second time no leftover tile is available.
/// Both screenshots must be identical.
#[test]
fn software_frames_do_not_depend_on_leftover_tile_memory() {
    // Software drawing, as on machines without a GPU (CI runners, VMs).
    let Some(mut browser) = start_with(&["--disable-gpu"]) else {
        return;
    };
    let mut page = browser.new_page().expect("page");
    page.emulate(&Emulation {
        css_width: 800,
        css_height: 450,
        scale: 1.0,
        mobile: false,
        touch: false,
        user_agent: None,
    })
    .expect("emulate");
    let mut shot = |idle: Duration| {
        page.navigate(CARD_PAGE, Duration::from_secs(30))
            .expect("navigate");
        page.prepare(&PrepareOptions::default()).expect("prepare");
        let mut place = |y: &str| {
            page.eval(&format!(
                "document.getElementById('c').style.transform = 'translate3d(0px, {y}px, 0px)'"
            ))
            .expect("move card");
            page.capture_png().expect("capture")
        };
        place("20");
        std::thread::sleep(idle);
        place("8.8889")
    };
    let right_away = shot(Duration::ZERO);
    let after_idle = shot(Duration::from_secs(7));
    assert!(
        right_away == after_idle,
        "the card's edge depends on leftover tile memory"
    );

    page.close().expect("close tab");
    browser.close();
}

/// A render must not make the computer sluggish: on Windows the browser
/// runs below normal priority by default (HANDOFF §14.1); turned off, it
/// gets the same priority as the program that started it.
/// On Linux/macOS it must keep the starter's priority even by default: a
/// niced Chrome missed its 30 s startup limit on GitHub's Linux runner
/// (ADR 0005), so this fails if `nice` comes back there.
#[test]
fn browser_priority_is_lowered_only_on_windows() {
    let own = priority(std::process::id());
    let Some(low) = start() else {
        return;
    };
    let expected = if cfg!(windows) {
        "BelowNormal".to_owned()
    } else {
        own.clone()
    };
    assert_eq!(priority(low.pid()), expected, "default");
    low.close();

    let normal = start_custom(|o| o.low_priority = false).expect("browser");
    assert_eq!(priority(normal.pid()), own, "turned off: unchanged");
    normal.close();
}
