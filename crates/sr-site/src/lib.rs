//! # sr-site — where the website comes from
//!
//! Purpose: turns "a folder on disk" or "a URL" into a base URL the browser
//!          can open (HANDOFF FR-SRC-01, FR-SRC-02, D-15).
//! Solves:  the owner's sites are plain HTML/CSS/JS folders. Opening them as
//!          `file://` breaks fetch, modules and fonts, so a folder is served
//!          by a tiny built-in web server on `127.0.0.1` with a random port,
//!          exactly like a real site.
//! Rules:   the server only answers on the local machine and never serves
//!          files outside the chosen folder (path traversal is rejected).
//! See also: `docs/ERRORS.md#sr-8001` … `#sr-8003`.

mod server;

use std::path::{Path, PathBuf};

use sr_core::{AppError, AppResult, ErrorCode};

pub use server::LocalServer;

/// Where a site comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SiteSource {
    /// A folder with `index.html`, served locally.
    Folder(PathBuf),
    /// An `http(s)://` URL (dev server or live site).
    Url(String),
}

impl SiteSource {
    /// Parses a site spec: a URL, or a folder path resolved against `base_dir`
    /// (the scenario file's folder). Folders must contain `index.html`.
    pub fn parse(spec: &str, base_dir: &Path) -> AppResult<SiteSource> {
        let spec = spec.trim();
        if spec.is_empty() {
            return Err(AppError::new(ErrorCode::SiteNotSpecified, "site is empty"));
        }
        let lower = spec.to_ascii_lowercase();
        if lower.starts_with("http://") || lower.starts_with("https://") {
            return Ok(SiteSource::Url(spec.trim_end_matches('/').to_owned()));
        }
        let path = base_dir.join(spec);
        let folder_error = |why: &str| {
            AppError::new(
                ErrorCode::SiteFolderInvalid,
                format!("{why}: {}", path.display()),
            )
            .with_param("path", path.display())
        };
        if !path.is_dir() {
            return Err(folder_error("site folder not found"));
        }
        if !path.join("index.html").is_file() {
            return Err(folder_error("site folder has no index.html"));
        }
        let path = path
            .canonicalize()
            .map_err(|_| folder_error("cannot resolve site folder"))?;
        Ok(SiteSource::Folder(path))
    }
}

/// A site ready to be opened. Keeps the local server alive while it exists.
#[derive(Debug)]
pub struct ServedSite {
    base_url: String,
    _server: Option<LocalServer>,
}

impl ServedSite {
    /// Base URL without a trailing slash, e.g. `http://127.0.0.1:51234`.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Full URL for a page path from the scenario (`/`, `/pricing`, or absolute).
    pub fn page_url(&self, page: &str) -> String {
        let lower = page.to_ascii_lowercase();
        if lower.starts_with("http://") || lower.starts_with("https://") {
            return page.to_owned();
        }
        format!("{}/{}", self.base_url, page.trim_start_matches('/'))
    }
}

/// Makes a site reachable: starts the local server for folders.
pub fn serve(source: &SiteSource) -> AppResult<ServedSite> {
    match source {
        SiteSource::Url(url) => Ok(ServedSite {
            base_url: url.clone(),
            _server: None,
        }),
        SiteSource::Folder(root) => {
            let server = LocalServer::start(root)?;
            Ok(ServedSite {
                base_url: server.base_url(),
                _server: Some(server),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_are_kept_and_folders_need_index_html() {
        let tmp = std::env::temp_dir();
        assert_eq!(
            SiteSource::parse("http://localhost:5173/", &tmp).unwrap(),
            SiteSource::Url("http://localhost:5173".into())
        );
        let err = SiteSource::parse("definitely-missing-folder", &tmp).unwrap_err();
        assert_eq!(err.code, ErrorCode::SiteFolderInvalid);
        assert_eq!(
            SiteSource::parse("  ", &tmp).unwrap_err().code,
            ErrorCode::SiteNotSpecified
        );
    }

    #[test]
    fn page_urls_join_cleanly() {
        let site = ServedSite {
            base_url: "http://127.0.0.1:9".into(),
            _server: None,
        };
        assert_eq!(site.page_url("/"), "http://127.0.0.1:9/");
        assert_eq!(site.page_url("/pricing"), "http://127.0.0.1:9/pricing");
        assert_eq!(site.page_url("https://x.dev/a"), "https://x.dev/a");
    }
}
