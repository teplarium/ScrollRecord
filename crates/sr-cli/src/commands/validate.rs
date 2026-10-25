//! `scrollrecord validate <flow>` — checks a scenario without rendering.
//!
//! Why: the fastest feedback loop for people and AI models writing scenario
//! files. It shows every problem at once, what each target will produce,
//! and checks the cursor pack, so nothing fails late in a long render.

use std::path::PathBuf;

use clap::Args;
use sr_core::AppResult;
use sr_cursor::CursorPack;

use super::{load_flow, print_warnings};

/// Arguments of `validate`.
#[derive(Debug, Args)]
pub struct ValidateArgs {
    /// Scenario file (*.srflow.json).
    pub flow: PathBuf,
}

/// Runs the command.
pub fn run(args: &ValidateArgs) -> AppResult<()> {
    let (flow, flow_dir) = load_flow(&args.flow)?;
    let warnings = flow.validate()?;
    print_warnings(&warnings);
    let pack = CursorPack::load(&flow.defaults.cursor.pack, &flow_dir)?;
    print_warnings(&pack.warnings);
    println!("✓ {} is valid", args.flow.display());
    println!("  cursor pack: {} ({})", pack.manifest.name, pack.origin);
    for target in &flow.targets {
        let plan = flow.plan(&target.id)?;
        // Device-specific notes, e.g. hovers dropped on touch screens.
        for w in plan.warnings.iter().filter(|w| !warnings.contains(w)) {
            eprintln!("warning: {w}");
        }
        let actions: usize = plan.blocks.iter().map(|b| b.actions.len()).sum();
        let g = plan.geometry;
        let path: Vec<&str> = std::iter::once(plan.start.id.as_str())
            .chain(plan.blocks.iter().map(|b| b.id.as_str()))
            .chain(std::iter::once(plan.end.id.as_str()))
            .collect();
        println!(
            "  {:<12} {}x{} @{} fps, {} ({}x{} css, scale {:.3})  path: {}  actions: {}",
            target.id,
            g.out_width,
            g.out_height,
            target.fps,
            target.format.extension(),
            g.css_width,
            g.css_height,
            g.scale,
            path.join(" → "),
            actions
        );
    }
    Ok(())
}
