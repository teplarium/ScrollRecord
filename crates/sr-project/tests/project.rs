//! Project folder behaviour end to end, on real temporary folders.
//!
//! Every test works under a folder with Cyrillic in its name, because the
//! owner's own repository path has it and such paths must keep working
//! (HANDOFF NFR-02).

#![allow(clippy::unwrap_used)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use sr_core::ErrorCode;
use sr_core::flow::Flow;
use sr_project::{LAYOUT_FILE, LOCK_FILE, MANIFEST_FILE, MAX_RECENT, NewProject, ProjectStore};

// ── Test sandbox ────────────────────────────────────────────────────────────

/// A fresh folder `…/sr-project-<pid>-<n>/Проекты`, deleted on drop.
struct Sandbox {
    root: PathBuf,
}

impl Sandbox {
    fn new() -> Self {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let root = std::env::temp_dir().join(format!(
            "sr-project-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("Проекты")).unwrap();
        Self { root }
    }

    /// Where projects are created.
    fn location(&self) -> PathBuf {
        self.root.join("Проекты")
    }

    fn store(&self) -> ProjectStore {
        ProjectStore::new(
            self.root.join("config"),
            self.root.join("Documents").join("ScrollRecord"),
            "9.9.9",
        )
    }

    fn new_project(&self, name: Option<&str>) -> NewProject {
        NewProject {
            location: self.location().display().to_string(),
            name: name.map(str::to_owned),
            untitled_base: "Untitled".to_owned(),
            ..NewProject::default()
        }
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn example(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples")
        .join(name)
}

// ── Create, open, save ──────────────────────────────────────────────────────

#[test]
fn new_project_is_an_empty_canvas_and_saves_safely() {
    let sb = Sandbox::new();
    let store = sb.store();
    let site = sb.location().join("сайт");
    fs::create_dir_all(&site).unwrap();

    let mut req = sb.new_project(Some("Промо"));
    req.site = Some(site.display().to_string());
    let opened = store.create(&req).unwrap();

    // ── Folder layout and the scenario of an empty canvas (D-39) ──
    let dir = PathBuf::from(&opened.dir);
    assert_eq!(dir, sb.location().join("Промо"));
    for file in [MANIFEST_FILE, "flow.srflow.json", LAYOUT_FILE, LOCK_FILE] {
        assert!(dir.join(file).is_file(), "{file} missing");
    }
    assert!(dir.join("cache").is_dir() && dir.join("renders").is_dir());
    assert_eq!(opened.manifest.name, "Промо");
    assert!(opened.flow.nodes.is_empty() && opened.flow.edges.is_empty());
    assert_eq!(opened.flow.targets[0].id, "desktop");
    assert_eq!(opened.flow.meta.name, "Промо");
    // Site next to the project → stored relative, so the pair can be moved.
    assert_eq!(opened.flow.meta.site.as_deref(), Some("../сайт"));
    assert!(!opened.site_missing);
    assert_eq!(opened.layout, serde_json::json!({}));

    // ── Save: new content on reopen, previous version kept as .bak ──
    let mut flow = opened.flow.clone();
    flow.meta.name = "Промо v2".to_owned();
    let layout = serde_json::json!({ "nodes": { "start": { "x": 10, "y": 20 } } });
    store.save(&dir, &flow, &layout).unwrap();
    let again = store.open(&dir, false).unwrap();
    assert_eq!(again.flow, flow);
    assert_eq!(again.layout, layout);
    assert!(dir.join("flow.srflow.json.bak").is_file());
    assert!(!dir.join("flow.srflow.json.tmp").exists());
    // The scenario file stays the plain CLI format, without the layout.
    let text = fs::read_to_string(dir.join("flow.srflow.json")).unwrap();
    assert!(!text.contains("\"layout\""));
    assert!(Flow::from_json(&text).is_ok());
}

#[test]
fn untitled_names_take_the_smallest_free_number() {
    let sb = Sandbox::new();
    let store = sb.store();
    fs::create_dir(sb.location().join("Untitled 1")).unwrap();
    fs::create_dir(sb.location().join("Untitled 3")).unwrap();

    let a = store.create(&sb.new_project(None)).unwrap();
    let b = store.create(&sb.new_project(Some("   "))).unwrap();
    assert_eq!(a.manifest.name, "Untitled 2");
    assert_eq!(b.manifest.name, "Untitled 4");

    let mut russian = sb.new_project(None);
    russian.untitled_base = "Без названия".to_owned();
    assert_eq!(
        store.create(&russian).unwrap().manifest.name,
        "Без названия 1"
    );
}

#[test]
fn an_existing_folder_is_never_overwritten() {
    let sb = Sandbox::new();
    fs::create_dir(sb.location().join("Promo")).unwrap();
    let err = sb
        .store()
        .create(&sb.new_project(Some("Promo")))
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::ProjectExists);
    assert_eq!(err.params["name"], "Promo");
}

// ── Opening problems ────────────────────────────────────────────────────────

#[test]
fn a_project_open_in_another_window_needs_force() {
    let sb = Sandbox::new();
    let store = sb.store();
    let dir = PathBuf::from(store.create(&sb.new_project(Some("P"))).unwrap().dir);

    // Another ScrollRecord process (or one that crashed) holds the lock.
    fs::write(dir.join(LOCK_FILE), r#"{"pid":4294967295,"since_ms":0}"#).unwrap();
    let err = store.open(&dir, false).unwrap_err();
    assert_eq!(err.code, ErrorCode::ProjectLocked);
    assert_eq!(err.params["pid"], "4294967295");

    // "Open anyway" takes the lock over; closing the tab releases it.
    store.open(&dir, true).unwrap();
    assert!(store.open(&dir, false).is_ok(), "own lock must not block");
    store.close(&dir);
    assert!(!dir.join(LOCK_FILE).exists());
}

#[test]
fn folders_that_are_not_projects_are_refused_clearly() {
    let sb = Sandbox::new();
    let store = sb.store();
    let plain = sb.location().join("просто папка");
    fs::create_dir(&plain).unwrap();
    assert_eq!(
        store.open(&plain, false).unwrap_err().code,
        ErrorCode::NotAProject
    );

    fs::write(plain.join(MANIFEST_FILE), "{ not json").unwrap();
    assert_eq!(
        store.open(&plain, false).unwrap_err().code,
        ErrorCode::NotAProject
    );
}

#[test]
fn a_project_from_a_newer_app_is_refused() {
    let sb = Sandbox::new();
    let store = sb.store();
    let dir = PathBuf::from(store.create(&sb.new_project(Some("P"))).unwrap().dir);
    let path = dir.join(MANIFEST_FILE);
    let text = fs::read_to_string(&path)
        .unwrap()
        .replace("\"project_format\": 1", "\"project_format\": 99");
    fs::write(&path, text).unwrap();
    let err = store.open(&dir, false).unwrap_err();
    assert_eq!(err.code, ErrorCode::ProjectVersionUnsupported);
    assert_eq!(err.params["version"], "99");
}

#[test]
fn a_moved_site_folder_is_reported_not_fatal() {
    let sb = Sandbox::new();
    let store = sb.store();
    let site = sb.location().join("site");
    fs::create_dir(&site).unwrap();
    let mut req = sb.new_project(Some("P"));
    req.site = Some(site.display().to_string());
    let dir = PathBuf::from(store.create(&req).unwrap().dir);

    fs::remove_dir(&site).unwrap();
    let opened = store.open(&dir, false).unwrap();
    assert!(opened.site_missing);

    // "Locate site folder…": the new place is stored portably and, once
    // saved, the project no longer reports a missing site.
    let moved = sb.location().join("новый сайт");
    fs::create_dir(&moved).unwrap();
    let mut flow = opened.flow;
    flow.meta.site = store.portable_site(&dir, &moved.display().to_string());
    assert_eq!(flow.meta.site.as_deref(), Some("../новый сайт"));
    store.save(&dir, &flow, &opened.layout).unwrap();
    assert!(!store.open(&dir, false).unwrap().site_missing);
}

// ── Import and export ───────────────────────────────────────────────────────

#[test]
fn importing_and_exporting_keep_the_scenario() {
    let sb = Sandbox::new();
    let store = sb.store();
    let source = example("01-plain-landing.srflow.json");
    let original = Flow::from_json(&fs::read_to_string(&source).unwrap()).unwrap();

    // ── New project from the example ──
    let mut req = sb.new_project(Some("Импорт"));
    req.import_from = Some(source.display().to_string());
    let opened = store.create(&req).unwrap();
    let dir = PathBuf::from(&opened.dir);
    assert_eq!(opened.flow.nodes, original.nodes);
    assert_eq!(opened.flow.targets, original.targets);
    // The example's relative site path still points at the fixture site.
    assert!(!opened.site_missing, "site: {:?}", opened.flow.meta.site);
    let site = PathBuf::from(opened.flow.meta.site.clone().unwrap());
    assert!(site.join("index.html").is_file(), "{}", site.display());

    // ── Export with a layout, then import that file into the project ──
    let layout = serde_json::json!({ "nodes": { "hero": { "x": 1, "y": 2 } } });
    store.save(&dir, &opened.flow, &layout).unwrap();
    let out_dir = sb.root.join("экспорт");
    fs::create_dir(&out_dir).unwrap();
    let exported = out_dir.join("promo.srflow.json");
    store.export_flow(&dir, &exported).unwrap();
    let text = fs::read_to_string(&exported).unwrap();
    assert_eq!(Flow::from_json(&text).unwrap().layout, Some(layout.clone()));
    assert!(!out_dir.join("promo.srflow.json.bak").exists());

    let imported = store.import_flow(&dir, &exported).unwrap();
    assert_eq!(imported.layout, layout);
    assert_eq!(imported.flow.nodes, original.nodes);
    assert_eq!(imported.flow.meta.site, opened.flow.meta.site);
}

// ── Recent list and tabs ────────────────────────────────────────────────────

#[test]
fn recent_list_is_capped_and_tabs_come_back() {
    let sb = Sandbox::new();
    let store = sb.store();
    let mut dirs = Vec::new();
    for i in 0..12 {
        dirs.push(PathBuf::from(
            store
                .create(&sb.new_project(Some(&format!("P{i}"))))
                .unwrap()
                .dir,
        ));
    }
    let home = store.home();
    assert_eq!(home.recent.len(), MAX_RECENT);
    assert_eq!(home.recent[0].name, "P11");
    assert!(home.recent.iter().all(|r| r.exists));
    assert_eq!(home.open_tabs.len(), 12);
    assert_eq!(home.suggested_location, sb.location().display().to_string());

    // Reopening moves a project to the top without a duplicate.
    store.open(&dirs[5], false).unwrap();
    let home = store.home();
    assert_eq!(home.recent[0].name, "P5");
    assert_eq!(home.recent.iter().filter(|r| r.name == "P5").count(), 1);

    // Closing a tab removes it from the tabs, not from the recent list;
    // releasing at app exit keeps the tab for the next start.
    store.close(&dirs[5]);
    store.release(&dirs[6]);
    let home = store.home();
    assert!(!home.open_tabs.contains(&dirs[5].display().to_string()));
    assert!(home.open_tabs.contains(&dirs[6].display().to_string()));
    assert_eq!(home.recent[0].name, "P5");

    // A deleted project shows as missing; "forget" removes it.
    fs::remove_dir_all(&dirs[11]).unwrap();
    let home = store.home();
    assert!(!home.recent.iter().find(|r| r.name == "P11").unwrap().exists);
    assert!(!home.open_tabs.contains(&dirs[11].display().to_string()));
    store.forget(&dirs[11]);
    assert!(store.home().recent.iter().all(|r| r.name != "P11"));
}

#[test]
fn a_broken_recent_list_starts_empty() {
    let sb = Sandbox::new();
    let config = sb.root.join("config");
    fs::create_dir_all(&config).unwrap();
    fs::write(config.join("projects.json"), "garbage").unwrap();
    let home = sb.store().home();
    assert!(home.recent.is_empty());
    assert!(home.suggested_location.ends_with("ScrollRecord"));
}
