//! `scrollrecord render <flow>` — renders videos.
//!
//! Why here and not in a library: only argument handling and printing live in
//! this file; the work is done by `sr-render`, which the desktop app will use too.

use std::io::IsTerminal;
use std::path::PathBuf;

use clap::Args;
use sr_core::{AppError, AppResult, ErrorCode};
use sr_encoder::Quality;
use sr_render::{RenderOptions, RenderReport, Renderer, StageTimings};

use super::{load_flow, print_warnings};

/// Arguments of `render`.
#[derive(Debug, Args)]
pub struct RenderArgs {
    /// Scenario file (*.srflow.json).
    pub flow: PathBuf,
    /// Render only these targets (repeatable). Default: all targets.
    #[arg(short, long, value_name = "ID")]
    pub target: Vec<String>,
    /// Output folder. Default: `renders/` next to the scenario file.
    #[arg(short, long, value_name = "DIR")]
    pub out: Option<PathBuf>,
    /// Site to record (folder or URL); overrides `meta.site`.
    #[arg(long, value_name = "FOLDER|URL")]
    pub site: Option<String>,
    /// Browser executable (Chrome, Edge, Chromium). Default: auto-detect.
    #[arg(long, value_name = "PATH")]
    pub browser: Option<PathBuf>,
    /// ffmpeg executable. Default: SR_FFMPEG or PATH.
    #[arg(long, value_name = "PATH")]
    pub ffmpeg: Option<PathBuf>,
    /// Slower encoding, smaller files.
    #[arg(long)]
    pub best: bool,
    /// Show the browser window while rendering (debugging).
    #[arg(long)]
    pub headful: bool,
    /// Run the browser and ffmpeg at normal priority. By default they run
    /// below normal, so the computer stays responsive; the video is the same.
    #[arg(long)]
    pub normal_priority: bool,
    /// Print a JSON report at the end.
    #[arg(long)]
    pub json: bool,
}

/// Runs the command.
pub fn run(args: RenderArgs) -> AppResult<()> {
    let (flow, flow_dir) = load_flow(&args.flow)?;
    print_warnings(&flow.validate()?);

    let targets: Vec<String> = if args.target.is_empty() {
        flow.targets.iter().map(|t| t.id.clone()).collect()
    } else {
        for id in &args.target {
            flow.target(id)?; // fail early on a typo, before starting the browser
        }
        args.target.clone()
    };
    let out_dir = args.out.clone().unwrap_or_else(|| flow_dir.join("renders"));

    let mut opts = RenderOptions {
        site: args.site.clone(),
        browser: args.browser.clone(),
        ffmpeg: args.ffmpeg.clone(),
        quality: if args.best {
            Quality::Best
        } else {
            Quality::Fast
        },
        low_priority: !args.normal_priority,
        ..RenderOptions::default()
    };
    opts.launch.headless = !args.headful;

    let mut renderer = Renderer::start(&flow, &flow_dir, opts)?;
    eprintln!("browser: {}", renderer.browser_version());
    let mut reports: Vec<RenderReport> = Vec::new();
    let mut failure: Option<AppError> = None;
    for id in &targets {
        let target = flow.target(id)?;
        let output = sr_render::output_path(&out_dir, &args.flow, target);
        let mut progress = progress_printer(id);
        match renderer.render(&flow, id, &output, &mut progress) {
            Ok(report) => {
                print_warnings(&report.warnings);
                eprintln!(
                    "✓ {:<10} {}  ({}x{}, {} fps, {:.1} s, {} frames) in {:.1} s",
                    report.target,
                    report.output.display(),
                    report.width,
                    report.height,
                    report.fps,
                    report.duration_ms / 1000.0,
                    report.frames,
                    report.elapsed_s,
                );
                print_stages(&report.stages);
                reports.push(report);
            }
            Err(err) => {
                // Keep going: one broken target must not stop the others (FR-RND-02).
                eprintln!(
                    "✗ {id}: [{}] {}",
                    err.code,
                    err.message.lines().next().unwrap_or("")
                );
                failure.get_or_insert(err);
            }
        }
    }
    renderer.finish();

    if args.json {
        let json = serde_json::to_string_pretty(&reports)
            .map_err(|e| AppError::new(ErrorCode::Internal, format!("json output: {e}")))?;
        println!("{json}");
    }
    failure.map_or(Ok(()), Err)
}

// ── Stage timings ───────────────────────────────────────────────────────────
// Two lines, because the worker runs alongside the browser: the first line
// happens one stage after another, the second at the same time as it. Seeing
// that split is what tells whether faster encoding would help (HANDOFF §14.1).
fn print_stages(s: &StageTimings) {
    let mut first = Vec::new();
    if s.browser_start_s > 0.0 {
        first.push(format!("browser start {:.1} s", s.browser_start_s));
    }
    first.extend([
        format!("page load {:.1} s", s.page_load_s),
        format!("probe {:.1} s", s.probe_s),
        format!("page steps {:.1} s", s.page_step_s),
        format!("screenshots {:.1} s", s.screenshot_s),
        format!("waiting for the worker {:.1} s", s.worker_wait_s),
    ]);
    eprintln!("    stages: {}", first.join(" · "));
    eprintln!(
        "    alongside, in a worker: decode + cursor {:.1} s · encoding {:.1} s",
        s.compose_s, s.encode_s
    );
}

// ── Progress line ───────────────────────────────────────────────────────────
// In a terminal: one line updated in place. In logs/CI: a line every 25 %.
fn progress_printer(target: &str) -> impl FnMut(sr_capture::Progress) + '_ {
    let tty = std::io::stderr().is_terminal();
    let mut last_quarter = 0;
    move |p| {
        let pct = p.done * 100 / p.total.max(1);
        if tty {
            eprint!("\r  {target}: frame {}/{} ({pct}%)   ", p.done, p.total);
            if p.done == p.total {
                eprint!("\r{}\r", " ".repeat(60));
            }
        } else if pct / 25 > last_quarter {
            last_quarter = pct / 25;
            eprintln!("  {target}: {pct}% ({}/{} frames)", p.done, p.total);
        }
    }
}
