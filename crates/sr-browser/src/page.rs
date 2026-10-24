//! High-level browser and page API used by the renderer.
//!
//! Why this layer: the renderer should say "open this URL as an iPhone",
//! "measure these blocks", "next frame", not juggle CDP messages. Each method
//! below is one clear step with its own error code.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::{Duration, Instant};

use base64::Engine as _;
use serde::Deserialize;
use serde_json::{Value, json};
use sr_core::flow::Locator;
use sr_core::{AppError, AppResult, ErrorCode};

use crate::FrameDriver;
use crate::cdp::Cdp;
use crate::process::{BrowserProcess, LaunchOptions};

/// The page runtime (time control, prepare, measure). See `js/runtime.js`.
const RUNTIME_JS: &str = include_str!("../js/runtime.js");

/// Default timeout for quick commands.
const CALL_TIMEOUT: Duration = Duration::from_secs(30);
/// Timeout for one screenshot (large pages at 4K can be slow).
const CAPTURE_TIMEOUT: Duration = Duration::from_secs(60);

/// A running browser.
#[derive(Debug)]
pub struct Browser {
    process: BrowserProcess,
    cdp: Cdp,
}

impl Browser {
    /// Starts the browser at `executable` and connects to it.
    pub fn launch(executable: &Path, opts: &LaunchOptions) -> AppResult<Browser> {
        let (mut process, ws_url) = BrowserProcess::launch(executable, opts)?;
        match Cdp::connect(&ws_url) {
            Ok(cdp) => Ok(Browser { process, cdp }),
            Err(err) => {
                process.stop();
                Err(err)
            }
        }
    }

    /// Browser product and version, e.g. `HeadlessChrome/131.0.0.0`.
    pub fn version(&mut self) -> AppResult<String> {
        let v = self
            .cdp
            .call(None, "Browser.getVersion", json!({}), CALL_TIMEOUT)?;
        Ok(v.get("product")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_owned())
    }

    /// Opens a new blank tab.
    pub fn new_page(&mut self) -> AppResult<Page<'_>> {
        let target = self.cdp.call(
            None,
            "Target.createTarget",
            json!({ "url": "about:blank" }),
            CALL_TIMEOUT,
        )?;
        let target_id = str_field(&target, "targetId")?;
        let attached = self.cdp.call(
            None,
            "Target.attachToTarget",
            json!({ "targetId": target_id, "flatten": true }),
            CALL_TIMEOUT,
        )?;
        let session = str_field(&attached, "sessionId")?;
        let mut page = Page {
            browser: self,
            session,
            target_id,
        };
        page.call("Page.enable", json!({}))?;
        page.freeze_animation_clock()?;
        page.call(
            "Page.addScriptToEvaluateOnNewDocument",
            json!({ "source": RUNTIME_JS }),
        )?;
        Ok(page)
    }

    /// Closes the browser and removes its temporary profile.
    pub fn close(mut self) {
        let _ = self
            .cdp
            .call(None, "Browser.close", json!({}), Duration::from_secs(5));
        self.process.stop();
    }
}

fn str_field(v: &Value, key: &str) -> AppResult<String> {
    v.get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| {
            AppError::new(
                ErrorCode::BrowserProtocolError,
                format!("browser reply has no '{key}'"),
            )
        })
}

/// Device emulation for a page.
#[derive(Debug, Clone, PartialEq)]
pub struct Emulation {
    /// Viewport width in CSS px.
    pub css_width: u32,
    /// Viewport height in CSS px.
    pub css_height: u32,
    /// Device scale factor (output pixels per CSS pixel).
    pub scale: f64,
    /// Mobile layout.
    pub mobile: bool,
    /// Touch events.
    pub touch: bool,
    /// User-Agent override.
    pub user_agent: Option<String>,
}

/// Page preparation options (mirrors `sr_core::flow::Prepare`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PrepareOptions {
    /// Selectors to hide.
    pub hide: Vec<String>,
    /// Extra CSS.
    pub css: Option<String>,
    /// Make lazy images load immediately.
    pub preload_lazy: bool,
    /// Also wait until no new network requests start for 500 ms.
    pub network_idle: bool,
}

/// What `prepare` found.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareReport {
    /// `hide` selectors that are not valid CSS (ignored).
    pub invalid_selectors: Vec<String>,
}

/// Page geometry and block positions, in CSS px.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Measurements {
    /// `innerWidth`.
    pub viewport_width: f64,
    /// `innerHeight`.
    pub viewport_height: f64,
    /// Total scrollable height.
    pub scroll_height: f64,
    /// Height of sticky/fixed headers at the top.
    pub top_inset: f64,
    /// Result per block id.
    pub blocks: BTreeMap<String, BlockMeasure>,
}

/// Measurement result of one block.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum BlockMeasure {
    /// Found exactly one element.
    Found {
        /// Top edge in page coordinates.
        top: f64,
        /// Height.
        height: f64,
        /// Index of the locator that matched.
        matched: usize,
    },
    /// No locator matched anything.
    NotFound {
        /// What was tried, e.g. `"sr=hero: 0 matches"`.
        tried: Vec<String>,
    },
    /// Locators matched only several elements.
    Ambiguous {
        /// What was tried.
        tried: Vec<String>,
    },
}

/// One browser tab, controlled by ScrollRecord.
#[derive(Debug)]
pub struct Page<'b> {
    browser: &'b mut Browser,
    session: String,
    target_id: String,
}

impl Page<'_> {
    fn call(&mut self, method: &str, params: Value) -> AppResult<Value> {
        self.browser
            .cdp
            .call(Some(&self.session), method, params, CALL_TIMEOUT)
    }

    // ── Animation clock ─────────────────────────────────────────────────────
    // Why: CSS transitions started by the page between our frames (e.g. a
    // scroll-reveal fired while the page loads) would otherwise play in real
    // time and could finish before ScrollRecord positions them — on a busy
    // machine the first frames would differ. With the browser's own
    // animation clock at speed 0, nothing moves unless runtime.js moves it.
    fn freeze_animation_clock(&mut self) -> AppResult<()> {
        self.call("Animation.setPlaybackRate", json!({ "playbackRate": 0 }))?;
        Ok(())
    }

    // ── Device emulation ────────────────────────────────────────────────────
    /// Makes the tab behave like the target device (size, density, touch, UA).
    pub fn emulate(&mut self, e: &Emulation) -> AppResult<()> {
        self.call(
            "Emulation.setDeviceMetricsOverride",
            json!({
                "width": e.css_width,
                "height": e.css_height,
                "deviceScaleFactor": e.scale,
                "mobile": e.mobile,
            }),
        )?;
        self.call(
            "Emulation.setTouchEmulationEnabled",
            json!({ "enabled": e.touch, "maxTouchPoints": if e.touch { 5 } else { 1 } }),
        )?;
        if let Some(ua) = &e.user_agent {
            self.call("Emulation.setUserAgentOverride", json!({ "userAgent": ua }))?;
        }
        Ok(())
    }

    // ── Loading ─────────────────────────────────────────────────────────────
    /// Opens `url` and waits until the document has finished loading.
    pub fn navigate(&mut self, url: &str, timeout: Duration) -> AppResult<()> {
        let load_error = |why: String| {
            AppError::new(ErrorCode::PageLoadFailed, format!("{url}: {why}")).with_param("url", url)
        };
        let nav = self.call("Page.navigate", json!({ "url": url }))?;
        if let Some(text) = nav
            .get("errorText")
            .and_then(Value::as_str)
            .filter(|t| !t.is_empty())
        {
            return Err(load_error(text.to_owned()));
        }
        let deadline = Instant::now() + timeout;
        loop {
            let state = self.eval("document.readyState")?;
            if state.as_str() == Some("complete") {
                // A new document may get a fresh animation clock: freeze it again.
                return self.freeze_animation_clock();
            }
            if Instant::now() > deadline {
                return Err(load_error(format!(
                    "not loaded after {}s",
                    timeout.as_secs()
                )));
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    /// Hides elements, loads lazy images, waits for fonts and images.
    pub fn prepare(&mut self, opts: &PrepareOptions) -> AppResult<PrepareReport> {
        let arg = json!({
            "hide": opts.hide,
            "css": opts.css,
            "preloadLazy": opts.preload_lazy,
            "networkIdle": opts.network_idle,
        });
        let report = self.eval(&format!("__sr.prepare({arg})"))?;
        serde_json::from_value(report).map_err(|err| {
            AppError::new(
                ErrorCode::PageScriptFailed,
                format!("prepare result: {err}"),
            )
        })
    }

    /// Measures blocks. `blocks` = (block id, locators in priority order).
    pub fn measure(&mut self, blocks: &[(String, Vec<Locator>)]) -> AppResult<Measurements> {
        let arg: Vec<Value> = blocks
            .iter()
            .map(|(id, locators)| json!({ "id": id, "locators": locators }))
            .collect();
        let raw = self.eval(&format!("__sr.measure({})", Value::Array(arg)))?;
        serde_json::from_value(raw).map_err(|err| {
            AppError::new(
                ErrorCode::PageScriptFailed,
                format!("measure result: {err}"),
            )
        })
    }

    // ── Script evaluation ───────────────────────────────────────────────────
    /// Runs a JS expression in the page; awaits promises; returns the value.
    pub fn eval(&mut self, expression: &str) -> AppResult<Value> {
        let reply = self.call(
            "Runtime.evaluate",
            json!({ "expression": expression, "awaitPromise": true, "returnByValue": true }),
        )?;
        if let Some(ex) = reply.get("exceptionDetails") {
            let text = ex
                .pointer("/exception/description")
                .or_else(|| ex.get("text"))
                .and_then(Value::as_str)
                .unwrap_or("script error");
            let short: String = expression.chars().take(80).collect();
            return Err(AppError::new(
                ErrorCode::PageScriptFailed,
                format!("{text} (in: {short})"),
            )
            .with_param("detail", text));
        }
        Ok(reply
            .pointer("/result/value")
            .cloned()
            .unwrap_or(Value::Null))
    }

    /// Closes the tab.
    pub fn close(self) -> AppResult<()> {
        let target_id = self.target_id.clone();
        self.browser.cdp.call(
            None,
            "Target.closeTarget",
            json!({ "targetId": target_id }),
            CALL_TIMEOUT,
        )?;
        Ok(())
    }
}

// ── Frame-by-frame control ──────────────────────────────────────────────────
impl FrameDriver for Page<'_> {
    fn step(&mut self, scroll_y: f64, advance_ms: f64) -> AppResult<f64> {
        let applied = self.eval(&format!("__sr.frame({scroll_y}, {advance_ms})"))?;
        Ok(applied.as_f64().unwrap_or(scroll_y))
    }

    fn capture_png(&mut self) -> AppResult<Vec<u8>> {
        let reply = self
            .browser
            .cdp
            .call(
                Some(&self.session),
                "Page.captureScreenshot",
                json!({ "format": "png", "optimizeForSpeed": true, "fromSurface": true }),
                CAPTURE_TIMEOUT,
            )
            .map_err(|err| AppError::new(ErrorCode::CaptureFailed, err.message))?;
        let data = reply
            .get("data")
            .and_then(Value::as_str)
            .unwrap_or_default();
        base64::engine::general_purpose::STANDARD
            .decode(data)
            .map_err(|err| {
                AppError::new(ErrorCode::CaptureFailed, format!("screenshot data: {err}"))
            })
    }
}
