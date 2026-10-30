//! Site map cache in the project folder, and the "site map job" built on it.
//!
//! Purpose: opening a tab or switching the device shows the site map at once
//!          when nothing changed, instead of reading the site again (~1 s per
//!          page plus the browser start).
//! Layout:  `<cache root>/sitemap/<target id>/<fingerprint>.json`; the 2
//!          newest fingerprints per target are kept (ADR 0006 §5 cache rule).
//! Fingerprint: everything that changes the result — this build's analyzer
//!          (version + in-page script), the target's device, the pages, the
//!          `prepare` step, the scenario's blocks, the site spec and, for a
//!          site folder, every file's path, size and modification time. A site
//!          given as an address (`http://localhost…`) has no file list: its
//!          last map is shown as "stale" and read again (`SiteMapEvent::Stale`).
//! Rules:   the cache is disposable: a broken or unreadable file is a miss,
//!          never an error; `DefaultHasher` may change between Rust versions,
//!          which only costs one re-read.

use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};
use sr_core::flow::Node;
use sr_core::{AppError, AppResult, Flow};
use sr_site::SiteSource;
use ts_rs::TS;

use crate::{AnalyzeOptions, AnalyzeProgress, SITEMAP_JS, SiteMap, analyze};

/// Sub-folder of the project cache that holds site maps.
const SITEMAP_DIR: &str = "sitemap";
/// Fingerprints kept per target.
const KEEP: usize = 2;
/// A site folder with more files than this is not listed (a huge folder is
/// probably not a built site); its map is then always read again.
const MAX_FILES: usize = 20_000;

/// What a site map job reports to the UI (over a Tauri `Channel`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum SiteMapEvent {
    /// A step started.
    Progress {
        /// How far the analysis is.
        progress: AnalyzeProgress,
    },
    /// An older map to show while the site is read again (address sites).
    Stale {
        /// The last map of this target.
        map: SiteMap,
    },
    /// The final map.
    Done {
        /// The site map.
        map: SiteMap,
        /// Came from the cache (the site was not opened).
        from_cache: bool,
    },
    /// The job stopped: an error, or SR-0004 when cancelled.
    Failed {
        /// What went wrong.
        error: AppError,
    },
}

/// What a site map job works on.
#[derive(Debug, Clone, Copy)]
pub struct SiteMapJob<'a> {
    /// The scenario (pages, device, the blocks it already uses).
    pub flow: &'a Flow,
    /// Where a relative `meta.site` starts (the project folder).
    pub project_dir: &'a Path,
    /// The project's `cache/` folder.
    pub cache_root: &'a Path,
    /// Target (device) id.
    pub target_id: &'a str,
    /// Ignore the cache ("Read the site again").
    pub fresh: bool,
}

/// Runs one site map job: answers from `cache_root` when the fingerprint
/// matches (unless `fresh`), otherwise reads the site and stores the result.
/// Every outcome, including errors, is reported through `emit`.
pub fn site_map_job(
    job: &SiteMapJob<'_>,
    opts: &AnalyzeOptions,
    cancel: &AtomicBool,
    emit: &mut dyn FnMut(SiteMapEvent),
) {
    let SiteMapJob {
        flow,
        project_dir,
        cache_root,
        target_id,
        fresh,
    } = *job;
    let folder = cache_root.join(SITEMAP_DIR).join(safe_name(target_id));
    let fp = fingerprint(flow, project_dir, target_id);

    // ── Cached answer ──
    if let Some(fp) = &fp
        && !fresh
        && let Some(map) = read(&folder.join(format!("{fp}.json")))
    {
        emit(SiteMapEvent::Done {
            map,
            from_cache: true,
        });
        return;
    }
    if fp.is_none()
        && let Some(map) = newest(&folder)
    {
        emit(SiteMapEvent::Stale { map });
    }

    // ── Read the site ──
    match analyze(
        flow,
        project_dir,
        target_id,
        opts,
        cancel,
        &mut |progress| {
            emit(SiteMapEvent::Progress { progress });
        },
    ) {
        Ok(map) => {
            let key = fp.unwrap_or_else(|| "address".to_owned());
            store(&folder, &key, &map);
            emit(SiteMapEvent::Done {
                map,
                from_cache: false,
            });
        }
        Err(error) => emit(SiteMapEvent::Failed { error }),
    }
}

/// Deletes every cached site map under `cache_root`.
///
/// Errors: SR-1002 if the folder exists but can't be removed.
pub fn clear(cache_root: &Path) -> AppResult<()> {
    let dir = cache_root.join(SITEMAP_DIR);
    match fs::remove_dir_all(&dir) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(AppError::new(
            sr_core::ErrorCode::FileWriteFailed,
            format!("could not clear {}: {err}", dir.display()),
        )
        .with_param("path", dir.display())),
    }
}

// ── Fingerprint ─────────────────────────────────────────────────────────────

/// `None` when the result can't be trusted to stay the same (an address
/// site, an unknown target, a folder too big to list).
pub fn fingerprint(flow: &Flow, project_dir: &Path, target_id: &str) -> Option<String> {
    let mut h = DefaultHasher::new();
    env!("CARGO_PKG_VERSION").hash(&mut h);
    SITEMAP_JS.hash(&mut h);
    let target = flow.targets.iter().find(|t| t.id == target_id)?;
    // JSON text: the flow types don't implement `Hash`, but serialize stably.
    json(&target.viewport).hash(&mut h);
    json(&target.aspect).hash(&mut h);
    json(&flow.pages).hash(&mut h);
    json(&flow.prepare).hash(&mut h);
    for node in &flow.nodes {
        if let Node::Block(b) = node {
            (&b.id, &b.page, json(&b.target)).hash(&mut h);
        }
    }
    let spec = flow.meta.site.as_deref()?;
    spec.hash(&mut h);
    match SiteSource::parse(spec, project_dir).ok()? {
        SiteSource::Url(_) => return None,
        SiteSource::Folder(root) => hash_folder(&root, &mut h)?,
    }
    Some(format!("{:016x}", h.finish()))
}

/// Hashes relative path, size and modification time of every file.
fn hash_folder(root: &Path, h: &mut DefaultHasher) -> Option<()> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).ok()?.flatten() {
            let path = entry.path();
            let meta = entry.metadata().ok()?;
            if meta.is_dir() {
                stack.push(path);
            } else {
                let modified = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map_or(0, |d| d.as_nanos());
                let rel = path
                    .strip_prefix(root)
                    .ok()?
                    .to_string_lossy()
                    .replace('\\', "/");
                files.push((rel, meta.len(), modified));
                if files.len() > MAX_FILES {
                    return None;
                }
            }
        }
    }
    files.sort();
    files.hash(h);
    Some(())
}

/// Flow parts as JSON text, for hashing.
fn json<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_default()
}

// ── Files ───────────────────────────────────────────────────────────────────

fn read(path: &Path) -> Option<SiteMap> {
    serde_json::from_str(&fs::read_to_string(path).ok()?).ok()
}

/// The most recently written map in `folder`.
fn newest(folder: &Path) -> Option<SiteMap> {
    entries(folder)
        .into_iter()
        .next()
        .and_then(|(path, _)| read(&path))
}

/// `.json` files in `folder`, newest first.
fn entries(folder: &Path) -> Vec<(PathBuf, std::time::SystemTime)> {
    let mut list: Vec<_> = fs::read_dir(folder)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .filter_map(|e| Some((e.path(), e.metadata().ok()?.modified().ok()?)))
        .collect();
    list.sort_by_key(|e| std::cmp::Reverse(e.1));
    list
}

/// Writes the map and keeps only the newest [`KEEP`] files. Failures are
/// logged only: a missing cache just means reading the site next time.
fn store(folder: &Path, key: &str, map: &SiteMap) {
    let path = folder.join(format!("{key}.json"));
    let written = fs::create_dir_all(folder).and_then(|()| {
        let text = serde_json::to_string(map).map_err(std::io::Error::other)?;
        fs::write(&path, text)
    });
    if let Err(err) = written {
        tracing::warn!(path = %path.display(), error = %err, "could not cache the site map");
        return;
    }
    for (old, _) in entries(folder).into_iter().skip(KEEP) {
        if old != path {
            let _ = fs::remove_file(old);
        }
    }
}

/// A target id as a folder name (ids are user-chosen text).
fn safe_name(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::SiteViewport;

    /// A temporary project with a one-file site, under a Cyrillic folder (NFR-02).
    fn sandbox(name: &str) -> (PathBuf, Flow) {
        let root = std::env::temp_dir().join(format!("sr-analyzer-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let site = root.join("Сайт");
        fs::create_dir_all(&site).unwrap();
        fs::write(site.join("index.html"), "<p>hi</p>").unwrap();
        let flow = Flow::from_json(
            r#"{ "format_version": 1, "meta": { "name": "t", "site": "Сайт" },
                 "targets": [{ "id": "desktop", "viewport": "desktop-1920" }],
                 "pages": [{ "id": "home", "url": "/" }], "nodes": [], "edges": [] }"#,
        )
        .unwrap();
        (root, flow)
    }

    fn map(target: &str) -> SiteMap {
        SiteMap {
            target: target.to_owned(),
            viewport: SiteViewport {
                width: 1,
                height: 1,
                mobile: false,
                touch: false,
            },
            pages: Vec::new(),
            elapsed_ms: 0,
        }
    }

    #[test]
    fn fingerprint_follows_the_site_files_and_the_device() {
        let (root, mut flow) = sandbox("fp");
        let first = fingerprint(&flow, &root, "desktop").unwrap();
        assert_eq!(
            fingerprint(&flow, &root, "desktop").unwrap(),
            first,
            "stable"
        );

        fs::write(root.join("Сайт").join("app.css"), "p{}").unwrap();
        let second = fingerprint(&flow, &root, "desktop").unwrap();
        assert_ne!(second, first, "a new file changes it");

        flow.targets[0].viewport = sr_core::flow::ViewportRef::Preset("iphone-15".into());
        assert_ne!(
            fingerprint(&flow, &root, "desktop").unwrap(),
            second,
            "device changes it"
        );
        assert!(fingerprint(&flow, &root, "unknown").is_none());

        flow.meta.site = Some("http://localhost:5173".into());
        assert!(
            fingerprint(&flow, &root, "desktop").is_none(),
            "an address has no files"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_cached_map_answers_without_opening_the_site() {
        let (root, flow) = sandbox("hit");
        let cache = root.join("cache");
        let fp = fingerprint(&flow, &root, "desktop").unwrap();
        store(
            &cache.join(SITEMAP_DIR).join("desktop"),
            &fp,
            &map("desktop"),
        );

        let mut events = Vec::new();
        let opts = AnalyzeOptions {
            // A browser that doesn't exist: the job must not need one.
            browser: Some(root.join("no-browser.exe")),
            ..AnalyzeOptions::default()
        };
        site_map_job(
            &SiteMapJob {
                flow: &flow,
                project_dir: &root,
                cache_root: &cache,
                target_id: "desktop",
                fresh: false,
            },
            &opts,
            &AtomicBool::new(false),
            &mut |e| {
                events.push(e);
            },
        );
        assert_eq!(
            events,
            [SiteMapEvent::Done {
                map: map("desktop"),
                from_cache: true
            }]
        );

        // "Rescan" ignores the cache, so it now fails on the missing browser.
        events.clear();
        site_map_job(
            &SiteMapJob {
                flow: &flow,
                project_dir: &root,
                cache_root: &cache,
                target_id: "desktop",
                fresh: true,
            },
            &opts,
            &AtomicBool::new(false),
            &mut |e| {
                events.push(e);
            },
        );
        assert!(
            matches!(events.last(), Some(SiteMapEvent::Failed { .. })),
            "{events:?}"
        );

        clear(&cache).unwrap();
        assert!(!cache.join(SITEMAP_DIR).exists());
        clear(&cache).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn only_the_newest_maps_are_kept_and_broken_files_are_misses() {
        let (root, _) = sandbox("keep");
        let folder = root.join("cache").join(SITEMAP_DIR).join("desktop");
        for key in ["a", "b", "c"] {
            store(&folder, key, &map(key));
            // Modification times must differ for "newest first".
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert_eq!(entries(&folder).len(), KEEP);
        assert_eq!(newest(&folder).unwrap().target, "c");
        fs::write(folder.join("c.json"), "{ broken").unwrap();
        assert!(read(&folder.join("c.json")).is_none());
        assert_eq!(safe_name("my phone/9:16"), "my_phone_9_16");
        fs::remove_dir_all(root).unwrap();
    }
}
