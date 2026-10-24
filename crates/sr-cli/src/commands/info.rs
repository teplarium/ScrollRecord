//! `scrollrecord info` and `scrollrecord errors`.

use std::path::PathBuf;

use sr_core::{AppError, AppResult, ErrorCode};
use sr_ipc::AppInfo;

fn json_error(err: &serde_json::Error) -> AppError {
    AppError::new(ErrorCode::Internal, format!("json output: {err}"))
}

/// Same data the desktop app shows in its footer, plus the tools it finds.
pub fn run(json: bool, log_dir: Option<PathBuf>) -> AppResult<()> {
    let info = AppInfo::current(
        env!("CARGO_PKG_VERSION"),
        log_dir.map(|dir| dir.display().to_string()),
    );
    let browser = sr_browser::find_browser(None).map(|p| p.display().to_string());
    let ffmpeg = sr_encoder::find_ffmpeg(None).map(|p| p.display().to_string());
    if json {
        let value = serde_json::json!({
            "app": info,
            "browser": browser.as_ref().ok(),
            "ffmpeg": ffmpeg.as_ref().ok(),
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&value).map_err(|e| json_error(&e))?
        );
    } else {
        println!("{} {}", info.app_name, info.app_version);
        println!("core     {}", info.core_version);
        println!("platform {}-{}", info.os, info.arch);
        println!(
            "browser  {}",
            browser.unwrap_or_else(|e| format!("not found ({})", e.code))
        );
        println!(
            "ffmpeg   {}",
            ffmpeg.unwrap_or_else(|e| format!("not found ({})", e.code))
        );
        if let Some(dir) = &info.log_dir {
            println!("logs     {dir}");
        }
    }
    Ok(())
}

/// The error catalog from `sr-core` (details: docs/ERRORS.md).
pub fn errors(json: bool) -> AppResult<()> {
    if json {
        let list: Vec<_> = ErrorCode::ALL
            .iter()
            .map(|c| serde_json::json!({ "code": c.as_str(), "summary": c.summary() }))
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&list).map_err(|e| json_error(&e))?
        );
    } else {
        for code in ErrorCode::ALL {
            println!("{}  {}", code.as_str(), code.summary());
        }
        println!("\nDetails and fixes: docs/ERRORS.md");
    }
    Ok(())
}
