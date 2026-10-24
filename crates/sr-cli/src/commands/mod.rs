//! One file per subcommand. Shared helpers live here.

pub mod info;
pub mod render;
pub mod validate;

use std::path::{Path, PathBuf};

use sr_core::{AppError, AppResult, ErrorCode, Flow};

/// Reads and parses a scenario file. Returns the flow and its folder.
pub fn load_flow(path: &Path) -> AppResult<(Flow, PathBuf)> {
    let text = std::fs::read_to_string(path).map_err(|err| {
        AppError::new(
            ErrorCode::FileReadFailed,
            format!("{}: {err}", path.display()),
        )
        .with_param("path", path.display())
    })?;
    let flow = Flow::from_json(&text)?;
    let dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
    Ok((flow, dir))
}

/// Prints scenario warnings in a consistent style.
pub fn print_warnings(warnings: &[String]) {
    for w in warnings {
        eprintln!("warning: {w}");
    }
}
