//! Integration test against a real browser.
//!
//! Runs only when Chrome/Edge/Chromium is installed (CI runners have Chrome);
//! otherwise it prints "skipped" and passes, so `cargo test` works anywhere.

use std::time::Duration;

use sr_browser::{
    BlockMeasure, Browser, Emulation, FrameDriver, LaunchOptions, PrepareOptions, find_browser,
};
use sr_core::flow::Locator;

const PAGE: &str = "data:text/html,<html><head><style>body{margin:0} \
    .pulse{animation:fade 1000ms linear infinite} @keyframes fade{to{opacity:0}}</style></head>\
    <body><div data-sr='a' style='height:1000px'>A</div>\
    <div data-sr='b' class='pulse' style='height:600px'>B</div>\
    <div class='dup'>x</div><div class='dup'>y</div></body></html>";

#[test]
fn virtual_time_scrolling_measuring_and_capture() {
    let Ok(path) = find_browser(None) else {
        eprintln!("skipped: no Chromium-based browser installed");
        return;
    };
    let mut opts = LaunchOptions::default();
    if cfg!(target_os = "linux") {
        // CI containers often forbid Chrome's sandbox; the test page is local and trusted.
        opts.extra_args.push("--no-sandbox".into());
    }
    let mut browser = Browser::launch(&path, &opts).expect("launch");
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
    page.step(0.0, 40.0).unwrap();
    assert_eq!(
        page.eval("hits").unwrap(),
        0.0,
        "timer must not fire before 50ms"
    );
    page.step(0.0, 20.0).unwrap();
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
    assert_eq!(page.step(700.0, 16.0).unwrap(), 700.0);
    let max = page
        .eval("document.documentElement.scrollHeight - innerHeight")
        .unwrap();
    assert_eq!(
        page.step(99_999.0, 16.0).unwrap(),
        max.as_f64().unwrap(),
        "clamped at the bottom"
    );

    // ── Measuring blocks ──
    let blocks = vec![
        ("a".to_owned(), vec![Locator::sr("a")]),
        (
            "b".to_owned(),
            vec![Locator::sr("missing"), Locator::sr("b")],
        ),
        (
            "dup".to_owned(),
            vec![Locator {
                css: Some(".dup".into()),
                ..Locator::default()
            }],
        ),
        ("none".to_owned(), vec![Locator::sr("nope")]),
    ];
    page.step(0.0, 0.0).unwrap();
    let m = page.measure(&blocks).expect("measure");
    assert_eq!(m.viewport_height, 600.0);
    assert!(m.scroll_height >= 1600.0);
    assert_eq!(
        m.blocks["a"],
        BlockMeasure::Found {
            top: 0.0,
            height: 1000.0,
            matched: 0
        }
    );
    assert_eq!(
        m.blocks["b"],
        BlockMeasure::Found {
            top: 1000.0,
            height: 600.0,
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
