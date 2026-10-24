//! `scrollrecord validate <flow>` — checks a scenario without rendering.
//!
//! Why: the fastest feedback loop for people and AI models writing scenario
//! files. It shows every problem at once and what each target will produce.

use std::path::PathBuf;

use clap::Args;
use sr_core::AppResult;

use super::{load_flow, print_warnings};

/// Arguments of `validate`.
#[derive(Debug, Args)]
pub struct ValidateArgs {
    /// Scenario file (*.srflow.json).
    pub flow: PathBuf,
}

/// Runs the command.
pub fn run(args: &ValidateArgs) -> AppResult<()> {
    let (flow, _) = load_flow(&args.flow)?;
    let warnings = flow.validate()?;
    print_warnings(&warnings);
    println!("✓ {} is valid", args.flow.display());
    for target in &flow.targets {
        let plan = flow.plan(&target.id)?;
        let g = plan.geometry;
        let path: Vec<&str> = std::iter::once(plan.start.id.as_str())
            .chain(plan.blocks.iter().map(|b| b.id.as_str()))
            .chain(std::iter::once(plan.end.id.as_str()))
            .collect();
        println!(
            "  {:<12} {}x{} @{} fps, {} ({}x{} css, scale {:.3})  path: {}",
            target.id,
            g.out_width,
            g.out_height,
            target.fps,
            target.format.extension(),
            g.css_width,
            g.css_height,
            g.scale,
            path.join(" → ")
        );
    }
    Ok(())
}
