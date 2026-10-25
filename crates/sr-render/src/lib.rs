//! # sr-render — the whole pipeline in one place
//!
//! Purpose: runs every step to turn a scenario into videos:
//!          plan → serve site → start browser → emulate device → load &
//!          prepare page → measure blocks → probe action targets (then
//!          reload) → build timeline → capture → draw cursor → encode →
//!          write `timeline.json` (HANDOFF §7.2).
//! Solves:  the CLI today and the desktop app in Phase 3 call the same code,
//!          so a render behaves identically everywhere. Each step fails with
//!          its own error code, which tells you which crate to look at
//!          (PROJECT_MAP §5).
//! Rules:   orchestration only; no browser protocol, maths or encoding here.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{SyncSender, sync_channel};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde::Serialize;
use sr_browser::{
    BlockMeasure, Browser, Emulation, LaunchOptions, Measurements, Page, PrepareOptions,
};
use sr_capture::{CapturedFrame, FrameSink, Progress};
use sr_compositor::Compositor;
use sr_core::flow::{CursorKind, MeasureItem, Target, WaitCondition};
use sr_core::{AppError, AppResult, ErrorCode, Flow, FlowPlan};
use sr_cursor::CursorPack;
use sr_encoder::{EncodeSettings, Quality, VideoSink};
use sr_site::{ServedSite, SiteSource};
use sr_timeline::{BlockBox, CursorFrame, CursorTrack, PageMetrics, Timeline};

/// Longest page time a block may take to finish appearing before its
/// buttons are measured (the probe).
const PROBE_SETTLE_MS: f64 = 2500.0;

/// Settings that are not part of the scenario.
#[derive(Debug, Clone)]
pub struct RenderOptions {
    /// Site override (`--site`): URL or folder (relative to the working dir).
    pub site: Option<String>,
    /// Browser executable override (`--browser`).
    pub browser: Option<PathBuf>,
    /// ffmpeg override (`--ffmpeg`).
    pub ffmpeg: Option<PathBuf>,
    /// Encoding speed/size.
    pub quality: Quality,
    /// Browser start options.
    pub launch: LaunchOptions,
    /// How long a page may take to load.
    pub page_load_timeout: Duration,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            site: None,
            browser: None,
            ffmpeg: None,
            quality: Quality::Fast,
            launch: LaunchOptions::default(),
            page_load_timeout: Duration::from_secs(45),
        }
    }
}

/// Summary of one rendered target.
#[derive(Debug, Clone, Serialize)]
pub struct RenderReport {
    /// Target id.
    pub target: String,
    /// Video file (or PNG folder).
    pub output: PathBuf,
    /// The `timeline.json` written next to it.
    pub timeline_file: PathBuf,
    /// Frames captured.
    pub frames: u32,
    /// Video length in ms.
    pub duration_ms: f64,
    /// Output width.
    pub width: u32,
    /// Output height.
    pub height: u32,
    /// Frames per second.
    pub fps: u32,
    /// Non-fatal notes.
    pub warnings: Vec<String>,
    /// Wall-clock render time in seconds.
    pub elapsed_s: f64,
}

/// A started render session: the site is served and the browser is running.
/// Render as many targets as needed, then call [`Renderer::finish`].
#[derive(Debug)]
pub struct Renderer {
    site: ServedSite,
    browser: Browser,
    browser_version: String,
    opts: RenderOptions,
    /// Scenario folder: relative cursor-pack folders are resolved from here.
    flow_dir: PathBuf,
}

impl Renderer {
    /// Serves the site and starts the browser.
    /// `flow_dir` is the scenario's folder (for relative `meta.site` paths).
    pub fn start(flow: &Flow, flow_dir: &Path, opts: RenderOptions) -> AppResult<Renderer> {
        // ── Site: --site wins over meta.site ──
        let (spec, base) = match (&opts.site, &flow.meta.site) {
            (Some(cli), _) => (cli.clone(), std::env::current_dir().unwrap_or_default()),
            (None, Some(meta)) => (meta.clone(), flow_dir.to_path_buf()),
            (None, None) => {
                return Err(AppError::new(
                    ErrorCode::SiteNotSpecified,
                    "no meta.site in the scenario and no --site given",
                ));
            }
        };
        let site = sr_site::serve(&SiteSource::parse(&spec, &base)?)?;

        // ── Browser ──
        let exe = sr_browser::find_browser(opts.browser.as_deref())?;
        let mut browser = Browser::launch(&exe, &opts.launch)?;
        let browser_version = browser.version().unwrap_or_else(|_| "unknown".into());
        tracing::info!(site = site.base_url(), browser = %browser_version, "render session started");
        Ok(Renderer {
            site,
            browser,
            browser_version,
            opts,
            flow_dir: flow_dir.to_path_buf(),
        })
    }

    /// Browser product/version string (for reports).
    pub fn browser_version(&self) -> &str {
        &self.browser_version
    }

    /// Renders one target into `output` (a file, or a folder for PNG).
    pub fn render(
        &mut self,
        flow: &Flow,
        target_id: &str,
        output: &Path,
        on_progress: &mut dyn FnMut(Progress),
    ) -> AppResult<RenderReport> {
        let started = Instant::now();
        let plan = flow.plan(target_id)?;
        let mut warnings = plan.warnings.clone();
        let g = plan.geometry;

        // ── Cursor pack first: a broken pack fails before the slow part ──
        let pack = CursorPack::load(&plan.cursor.pack, &self.flow_dir)?;
        warnings.extend(pack.warnings.iter().cloned());

        // ── Page: device, load, prepare ──
        let mut page = self.browser.new_page()?;
        page.emulate(&Emulation {
            css_width: g.css_width,
            css_height: g.css_height,
            scale: g.scale,
            mobile: plan.viewport.mobile,
            touch: plan.viewport.has_touch(),
            user_agent: plan.viewport.user_agent.clone(),
        })?;
        let url = self.site.page_url(&plan.page.url);
        let prepare = PrepareOptions {
            hide: flow.prepare.hide.clone(),
            css: flow.prepare.css.clone(),
            preload_lazy: flow.prepare.preload_lazy,
            network_idle: flow.prepare.wait == WaitCondition::NetworkIdle,
        };
        load(
            &mut page,
            &url,
            &prepare,
            self.opts.page_load_timeout,
            &mut warnings,
        )?;

        // ── Measure → probe action targets → timeline ──
        let blocks: Vec<MeasureItem> = plan
            .measure
            .iter()
            .filter(|m| m.within.is_none())
            .cloned()
            .collect();
        let mut metrics = page_metrics(page.measure(&blocks)?)?;
        if probe_targets(&mut page, &plan, &mut metrics)? {
            // The probe scrolled and revealed things: start over from a fresh page.
            load(
                &mut page,
                &url,
                &prepare,
                self.opts.page_load_timeout,
                &mut Vec::new(),
            )?;
        }
        let timeline = Timeline::build(&plan, &metrics)?;
        warnings.extend(timeline.warnings.iter().cloned());
        tracing::info!(
            target = target_id,
            frames = timeline.frame_count(),
            duration_ms = timeline.duration_ms,
            clicks = timeline.cursor.clicks.len(),
            "timeline ready"
        );

        // ── Capture → cursor → encode ──
        let settings = EncodeSettings {
            width: g.out_width,
            height: g.out_height,
            fps: timeline.fps,
            format: plan.target.format,
            quality: self.opts.quality,
        };
        let mut sink = CompositeSink::start(
            Compositor::new(pack, g.scale, plan.cursor.scale),
            sr_encoder::open_sink(output, &settings, self.opts.ffmpeg.as_deref())?,
        );
        let stats = sr_capture::capture(&mut page, &timeline, &mut sink, on_progress)?;
        if stats.max_scroll_drift_px > 1.0 {
            warnings.push(format!(
                "the page scrolled up to {:.0}px less than planned (it got shorter while recording)",
                stats.max_scroll_drift_px
            ));
        }
        let _ = page.close();

        // ── timeline.json next to the video (basis for the future editor) ──
        let timeline_file = timeline_path(output, plan.target.format);
        write_timeline(&timeline_file, &plan, &url, &metrics, &timeline)?;
        if !warnings.is_empty() {
            tracing::warn!(
                target = target_id,
                ?warnings,
                "render finished with warnings"
            );
        }

        Ok(RenderReport {
            target: target_id.to_owned(),
            output: output.to_path_buf(),
            timeline_file,
            frames: stats.frames,
            duration_ms: timeline.duration_ms,
            width: g.out_width,
            height: g.out_height,
            fps: timeline.fps,
            warnings,
            elapsed_s: started.elapsed().as_secs_f64(),
        })
    }

    /// Stops the browser and the local server.
    pub fn finish(self) {
        self.browser.close();
    }
}

// ── Loading ─────────────────────────────────────────────────────────────────
fn load(
    page: &mut Page<'_>,
    url: &str,
    prepare: &PrepareOptions,
    timeout: Duration,
    warnings: &mut Vec<String>,
) -> AppResult<()> {
    page.navigate(url, timeout)?;
    let prepared = page.prepare(prepare)?;
    for sel in prepared.invalid_selectors {
        warnings.push(format!(
            "prepare.hide: '{sel}' is not a valid CSS selector (ignored)"
        ));
    }
    Ok(())
}

// ── Probe: where are the buttons once their block has appeared? ─────────────
// Why: blocks often slide or fade in when scrolled into view, so a button
// measured at page load sits 20–60px away from where it ends up. The probe
// scrolls to each block stop, lets the page settle, and measures there.
// Returns whether the page was touched (then it must be reloaded).
fn probe_targets(
    page: &mut Page<'_>,
    plan: &FlowPlan,
    metrics: &mut PageMetrics,
) -> AppResult<bool> {
    let mut probed = false;
    for block in &plan.blocks {
        let items: Vec<MeasureItem> = plan
            .measure
            .iter()
            .filter(|m| m.id == block.id || m.within.as_deref() == Some(block.id.as_str()))
            .cloned()
            .collect();
        if items.len() < 2 {
            continue; // the block itself only: no action targets
        }
        let stop_y = metrics.align_y(&block.id, block.align, block.offset_px)?;
        let found = page_metrics(page.probe(stop_y, PROBE_SETTLE_MS, &items)?)?;
        for (id, b) in found.blocks {
            if id != block.id {
                metrics.blocks.insert(id, b);
            }
        }
        probed = true;
    }
    Ok(probed)
}

/// Frames from the browser → cursor drawn → encoder, on a worker thread.
///
/// Why a thread: decoding a screenshot, drawing the cursor and feeding
/// ffmpeg take about as long as the browser needs for the next frame.
/// Doing both at once makes renders noticeably faster. Frames stay in order
/// (one worker, FIFO channel), so output is unchanged.
struct CompositeSink {
    tx: Option<SyncSender<Job>>,
    worker: Option<JoinHandle<AppResult<()>>>,
}

enum Job {
    Frame {
        index: u32,
        png: Vec<u8>,
        cursor: CursorFrame,
        kind: CursorKind,
    },
    /// All frames sent: finish the file. (A channel closed without this
    /// means the render failed; the encoder is dropped and the file discarded.)
    Finish,
}

impl CompositeSink {
    fn start(mut compositor: Compositor, mut video: Box<dyn VideoSink + Send>) -> CompositeSink {
        // A few frames of buffer: enough to smooth out timing, small in memory.
        let (tx, rx) = sync_channel::<Job>(3);
        let worker = std::thread::spawn(move || -> AppResult<()> {
            for job in rx {
                match job {
                    Job::Frame {
                        index,
                        png,
                        cursor,
                        kind,
                    } => {
                        let done = compositor.compose(&png, &cursor, kind)?;
                        video.push(index, done.width, done.height, &done.rgba)?;
                    }
                    Job::Finish => return video.finish(),
                }
            }
            Ok(())
        });
        CompositeSink {
            tx: Some(tx),
            worker: Some(worker),
        }
    }

    /// Stops the worker and returns its result.
    fn join(&mut self) -> AppResult<()> {
        self.tx = None;
        match self.worker.take() {
            Some(handle) => handle.join().unwrap_or_else(|_| {
                Err(AppError::new(
                    ErrorCode::Internal,
                    "the frame compositing thread crashed",
                ))
            }),
            None => Ok(()),
        }
    }

    fn send(&mut self, job: Job) -> AppResult<()> {
        let sent = self.tx.as_ref().is_some_and(|tx| tx.send(job).is_ok());
        if sent {
            return Ok(());
        }
        // The worker stopped early: its error explains why.
        self.join()?;
        Err(AppError::new(
            ErrorCode::Internal,
            "the frame compositing thread stopped",
        ))
    }
}

impl FrameSink for CompositeSink {
    fn push(&mut self, frame: &CapturedFrame<'_>) -> AppResult<()> {
        self.send(Job::Frame {
            index: frame.plan.index,
            png: frame.png.to_vec(),
            cursor: frame.plan.cursor.clone(),
            kind: frame.cursor_kind,
        })
    }

    fn finish(&mut self) -> AppResult<()> {
        self.send(Job::Finish)?;
        self.join()
    }
}

impl Drop for CompositeSink {
    fn drop(&mut self) {
        let _ = self.join();
    }
}

// ── Measurements → PageMetrics (with clear locator errors) ──────────────────
fn page_metrics(m: Measurements) -> AppResult<PageMetrics> {
    let mut blocks = std::collections::BTreeMap::new();
    let mut problems: Vec<(ErrorCode, String, String)> = Vec::new();
    for (id, result) in m.blocks {
        match result {
            BlockMeasure::Found {
                top,
                height,
                left,
                width,
                ..
            } => {
                blocks.insert(
                    id,
                    BlockBox {
                        top,
                        height,
                        left,
                        width,
                    },
                );
            }
            BlockMeasure::NotFound { tried } => {
                problems.push((ErrorCode::LocatorNotFound, id, tried.join("; ")));
            }
            BlockMeasure::Ambiguous { tried } => {
                problems.push((ErrorCode::LocatorAmbiguous, id, tried.join("; ")));
            }
        }
    }
    if let Some((code, block, tried)) = problems.first().cloned() {
        let all: Vec<String> = problems
            .iter()
            .map(|(c, id, t)| {
                let what = if *c == ErrorCode::LocatorNotFound {
                    "not found"
                } else {
                    "ambiguous"
                };
                // Action targets are stored as "block#index".
                match id.split_once('#') {
                    Some((block, i)) => {
                        format!("block '{block}': target of actions[{i}] {what} ({t})")
                    }
                    None => format!("block '{id}' {what} ({t})"),
                }
            })
            .collect();
        return Err(AppError::new(code, all.join("\n"))
            .with_param("block", block)
            .with_param("tried", tried)
            .with_param("count", problems.len()));
    }
    Ok(PageMetrics {
        viewport_width: m.viewport_width,
        viewport_height: m.viewport_height,
        max_scroll: (m.scroll_height - m.viewport_height).max(0.0),
        top_inset: m.top_inset,
        blocks,
    })
}

// ── Output naming ───────────────────────────────────────────────────────────

/// Default output path: `<out_dir>/<scenario name>_<target>.<ext>`
/// (for PNG a folder `<scenario name>_<target>_frames`).
pub fn output_path(out_dir: &Path, flow_file: &Path, target: &Target) -> PathBuf {
    let name = flow_file
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| {
            n.trim_end_matches(".json")
                .trim_end_matches(".srflow")
                .to_owned()
        })
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "scrollrecord".into());
    let base = format!("{name}_{}", target.id);
    match target.format {
        sr_core::flow::OutputFormat::Png => out_dir.join(format!("{base}_frames")),
        f => out_dir.join(format!("{base}.{}", f.extension())),
    }
}

fn timeline_path(output: &Path, format: sr_core::flow::OutputFormat) -> PathBuf {
    match format {
        sr_core::flow::OutputFormat::Png => output.join("timeline.json"),
        _ => output.with_extension("timeline.json"),
    }
}

/// Machine-readable record of what was rendered (HANDOFF FR-RND-05).
#[derive(Serialize)]
struct TimelineFile<'a> {
    format: &'static str,
    version: u32,
    target: &'a str,
    url: &'a str,
    fps: u32,
    frames: u32,
    duration_ms: f64,
    warmup_ms: u32,
    output: Size,
    viewport: Viewport,
    page: &'a PageMetrics,
    segments: &'a [sr_timeline::Segment],
    cursor_pack: &'a str,
    cursor: &'a CursorTrack,
}

#[derive(Serialize)]
struct Size {
    width: u32,
    height: u32,
}

#[derive(Serialize)]
struct Viewport {
    css_width: u32,
    css_height: u32,
    scale: f64,
    mobile: bool,
}

fn write_timeline(
    path: &Path,
    plan: &FlowPlan,
    url: &str,
    metrics: &PageMetrics,
    t: &Timeline,
) -> AppResult<()> {
    let file = TimelineFile {
        format: "scrollrecord-timeline",
        version: 1,
        target: &plan.target.id,
        url,
        fps: t.fps,
        frames: t.frame_count(),
        duration_ms: t.duration_ms,
        warmup_ms: t.warmup_ms,
        output: Size {
            width: plan.geometry.out_width,
            height: plan.geometry.out_height,
        },
        viewport: Viewport {
            css_width: plan.geometry.css_width,
            css_height: plan.geometry.css_height,
            scale: plan.geometry.scale,
            mobile: plan.viewport.mobile,
        },
        page: metrics,
        segments: &t.segments,
        cursor_pack: &plan.cursor.pack,
        cursor: &t.cursor,
    };
    let mut json = serde_json::to_string_pretty(&file)
        .map_err(|err| AppError::new(ErrorCode::Internal, format!("timeline json: {err}")))?;
    json.push('\n');
    std::fs::write(path, json).map_err(|err| {
        AppError::new(
            ErrorCode::FileWriteFailed,
            format!("{}: {err}", path.display()),
        )
        .with_param("path", path.display())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sr_core::flow::{Aspect, Fit, Frame, OutputFormat, Resolution, ViewportRef};

    fn target(format: OutputFormat) -> Target {
        Target {
            id: "mobile".into(),
            viewport: ViewportRef::Preset("iphone-15".into()),
            aspect: Aspect::Native,
            resolution: Resolution::default(),
            fps: 60,
            format,
            fit: Fit::FillViewport,
            frame: Frame::Viewport,
            frame_block: None,
            background: None,
        }
    }

    #[test]
    fn output_names_come_from_scenario_and_target() {
        let flow = Path::new("examples/01-plain-landing.srflow.json");
        assert_eq!(
            output_path(Path::new("out"), flow, &target(OutputFormat::Mp4)),
            Path::new("out/01-plain-landing_mobile.mp4")
        );
        let png = output_path(Path::new("out"), flow, &target(OutputFormat::Png));
        assert_eq!(png, Path::new("out/01-plain-landing_mobile_frames"));
        assert_eq!(
            timeline_path(&png, OutputFormat::Png),
            png.join("timeline.json")
        );
        assert_eq!(
            timeline_path(Path::new("out/a_b.mp4"), OutputFormat::Mp4),
            Path::new("out/a_b.timeline.json")
        );
    }

    #[test]
    fn missing_blocks_are_all_listed() {
        let mut blocks = std::collections::BTreeMap::new();
        blocks.insert(
            "a".into(),
            BlockMeasure::NotFound {
                tried: vec!["sr=a: 0 matches".into()],
            },
        );
        blocks.insert(
            "b".into(),
            BlockMeasure::Ambiguous {
                tried: vec!["css=.x: 2 matches".into()],
            },
        );
        let m = Measurements {
            viewport_width: 1.0,
            viewport_height: 1.0,
            scroll_height: 1.0,
            top_inset: 0.0,
            blocks,
        };
        let err = page_metrics(m).unwrap_err();
        assert_eq!(err.code, ErrorCode::LocatorNotFound);
        assert_eq!(err.params["block"], "a");
        assert!(err.message.contains("block 'b' ambiguous"));

        let mut targets = std::collections::BTreeMap::new();
        targets.insert(
            "pricing#2".into(),
            BlockMeasure::NotFound {
                tried: vec!["text=Buy: 0 matches".into()],
            },
        );
        let err = page_metrics(Measurements {
            viewport_width: 1.0,
            viewport_height: 1.0,
            scroll_height: 1.0,
            top_inset: 0.0,
            blocks: targets,
        })
        .unwrap_err();
        assert!(
            err.message
                .contains("block 'pricing': target of actions[2] not found"),
            "{}",
            err.message
        );
    }
}
