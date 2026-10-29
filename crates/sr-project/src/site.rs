//! Portable `meta.site` paths.
//!
//! Purpose: decide how the site folder is written into the scenario so the
//!          project still works after the whole folder tree is copied to
//!          another computer (ADR 0006 §1, "Export project" in HANDOFF §14.1).
//! Rule:    a site folder inside the project or next to it (same parent) is
//!          stored relative, with `/` separators (`site`, `../site`);
//!          anything else stays absolute; URLs are kept as they are.
//!          Relative paths are always relative to the scenario file's folder,
//!          the same rule the CLI uses (`sr-site` → `SiteSource::parse`).

use std::path::{Path, PathBuf};

use crate::fsio;

/// Rewrites `site` (relative to `from`, or absolute, or a URL) so that it is
/// correct relative to `to`, using the portable form when possible.
pub(crate) fn rebase(site: Option<&str>, from: &Path, to: &Path) -> Option<String> {
    let site = site?.trim();
    if site.is_empty() {
        return None;
    }
    if fsio::is_url(site) {
        return Some(site.to_owned());
    }
    let absolute = fsio::normalize(&from.join(site));
    Some(portable(&absolute, &fsio::normalize(to)))
}

/// The site folder as an absolute path, or `None` for a URL / no site.
pub(crate) fn folder(site: Option<&str>, base: &Path) -> Option<PathBuf> {
    let site = site?.trim();
    if site.is_empty() || fsio::is_url(site) {
        return None;
    }
    Some(fsio::normalize(&base.join(site)))
}

fn portable(folder: &Path, base: &Path) -> String {
    if let Ok(inside) = folder.strip_prefix(base) {
        return slashes(inside, "");
    }
    if let Some(parent) = base.parent()
        && let Ok(next_to) = folder.strip_prefix(parent)
    {
        return slashes(next_to, "../");
    }
    folder.display().to_string()
}

fn slashes(relative: &Path, prefix: &str) -> String {
    let parts: Vec<String> = relative
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    let joined = parts.join("/");
    if joined.is_empty() && prefix.is_empty() {
        ".".to_owned()
    } else {
        format!("{prefix}{joined}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> PathBuf {
        if cfg!(windows) {
            PathBuf::from(r"E:\Проекты")
        } else {
            PathBuf::from("/home/u/Проекты")
        }
    }

    #[test]
    fn site_next_to_the_project_becomes_relative() {
        let project = root().join("Promo");
        let site = root().join("site");
        let stored = rebase(site.to_str(), &project, &project);
        assert_eq!(stored.as_deref(), Some("../site"));
    }

    #[test]
    fn site_inside_the_project_becomes_relative() {
        let project = root().join("Promo");
        let site = project.join("www").join("dist");
        assert_eq!(
            rebase(site.to_str(), &project, &project).as_deref(),
            Some("www/dist")
        );
    }

    #[test]
    fn far_away_site_stays_absolute() {
        let project = root().join("a").join("Promo");
        let site = root().join("b").join("site");
        assert_eq!(
            rebase(site.to_str(), &project, &project),
            Some(site.display().to_string())
        );
    }

    #[test]
    fn relative_site_is_rebased_to_the_new_folder() {
        // examples/x.srflow.json says "../tests/site" → seen from a project
        // in examples/Promo it is "../../tests/site", which is neither inside
        // nor next to the project, so it becomes absolute.
        let examples = root().join("repo").join("examples");
        let project = examples.join("Promo");
        let stored = rebase(Some("../tests/site"), &examples, &project);
        assert_eq!(
            stored,
            Some(
                root()
                    .join("repo")
                    .join("tests")
                    .join("site")
                    .display()
                    .to_string()
            )
        );
    }

    #[test]
    fn urls_are_kept() {
        let p = root();
        assert_eq!(
            rebase(Some("http://localhost:5173"), &p, &p).as_deref(),
            Some("http://localhost:5173")
        );
        assert_eq!(folder(Some("http://localhost:5173"), &p), None);
    }
}
