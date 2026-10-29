# 0006 — Editor architecture (Phase 3)

- **Status:** accepted (owner's answers to the product questions: 2026-10-28, see the end); **editor layout changed 2026-10-29 (D-38) — read "Update 2026-10-29" at the end before §1, §5 and §6**
- **Date:** 2026-10-28
- **Related:** HANDOFF §7.4 (D-05 hybrid canvas), §7.5, §7.6, D-03 (React Flow), D-18 (types from Rust); FR-PRJ-01…05, FR-SRC-01/02, FR-CNV-01…12, FR-RND-02…04, FR-PRV-01, NFR-02, NFR-05; ADR 0002 (capture), ADR 0005 (priority)

## Context
Phase 3 turns the CLI renderer into an editor: a project you create and reopen, a FigJam-like canvas of the scenario, a properties panel, real page screenshots on the canvas, and rendering from the app.
Starting point (read 2026-10-28): the desktop shell has a start screen whose two buttons call `create_project` / `open_project`, which answer SR-0002. Only `AppInfo`, `AppError` and `ErrorCode` reach TypeScript; the scenario types (`sr-core::flow`) do not. No canvas library, no state library, no file dialog. `sr-render` renders from the CLI with a per-frame progress callback, but has no cancel and no events. The scenario file already has an optional `layout` field (free JSON).
Constraints: no business logic in Tauri handlers (AGENTS §3); `unsafe_code = "forbid"`; every dependency must be justified; Cyrillic paths (NFR-02); 200+ nodes and 20 000 px pages stay smooth (NFR-05); the UI on Linux is WebKitGTK, the video is Chromium (HANDOFF §7.4).

## Decisions

### 1. Project folder — new crate `sr-project`
```
<Project name>/
  project.json        name, project format version, app version, created/modified, render settings (file-name template)
  flow.srflow.json    the scenario — exactly the CLI format; `scrollrecord render` works on it unchanged
  layout.json         editor-only: node positions, canvas viewport, board positions
  cache/              page snapshots, tiles, element boxes — can be deleted at any time, rebuilt on demand
  renders/            videos and their timeline.json
```
- All file IO for projects lives in `sr-project` (depends on `sr-core` only). Tauri commands just call it.
- **Layout stays out of the scenario file** inside a project (clean diffs, AI skills read a smaller file). Export of a single scenario file embeds the layout into `flow.layout`; import moves `flow.layout` into `layout.json` (FR-PRJ-04, FR-CNV-12: no layout → auto-layout).
- **Site source** has one home: `flow.meta.site` (already resolved relative to the scenario's folder). A site folder inside or next to the project is stored relative, anything else absolute; a missing folder on open asks the user to locate it instead of failing.
- **Atomic writes:** write `name.tmp` in the same folder, then rename over the old file; the previous version is kept as `name.bak`. A crash can never leave a half-written file.
- **Recent projects** are kept by Rust in the app's config folder (`recent.json`, last 10, plus the last location used for new projects), not in the webview's storage (`shared/storage.ts` is only for UI conveniences).
- **Naming and location** of new projects: owner's answers 1 and 3 at the end ("Untitled N", user-chosen folder, first suggestion `Documents\ScrollRecord\`).
- Folder pickers: **new dependency `tauri-plugin-dialog`** (official Tauri plugin; native open/save dialogs; nothing in std or Tauri core does it). Later, "open the output folder": `tauri-plugin-opener` (official), added in step 3.7.

### 2. Scenario types reach the UI from Rust
- `sr-core::flow` types get `#[derive(TS)] #[ts(export)]` (sr-core already depends on ts-rs). Types with custom serde forms (e.g. `"16:9"`, `"1080p"`, viewport preset or object) get explicit `#[ts(type = …)]`. The UI edits a plain object that *is* the scenario JSON, so save = serialize, nothing to translate.
- **Rules stay in Rust:** a command `validate_flow(flow, layout?) -> Issue[]` returns structured issues (severity, code, node or edge, message key). The UI never re-implements validation; it calls this (debounced ~300 ms) after edits and highlights nodes (FR-CNV-09). `sr-core` validation gets node/edge ids on its issues for this.
- Edges have no id in the format; the UI uses `from→to` as id.

### 3. Editor state in the UI — Zustand + own history
- One **document store** (Zustand): `{ flow, layout }` — everything that is saved. A separate **view store**: selection, zoom/pan, open panels, current device — never saved in history.
- **Undo/redo (FR-PRJ-05):** snapshot history with structural sharing (immutable updates), capped at 200 steps. A drag is one step (committed on drag end); typing in a field is one step per field focus. Own ~80-line module with tests instead of `zundo`: we need coalescing rules and the document/view split anyway.
- **New dependencies:** `@xyflow/react` (React Flow, MIT — D-03) and `zustand` (MIT, ~1 KB — HANDOFF §7.6).
- **Saving:** autosave ~1 s after the last change and on window close; Ctrl+S saves at once; no Save button (owner, answer 2). With atomic writes, "recovery after a crash" (FR-PRJ-02) means reopening loses at most about a second; `.bak` gives one step back. A lock file in the project folder warns when the same project is open twice.
- **Auto-layout (FR-CNV-12):** the scenario is a path (start → blocks → end), so a simple own layout (column in path order, branches offset) is enough; no graph-layout dependency.

### 4. IPC: quick commands, and long jobs with progress and cancel
- Quick request/response stays as Tauri commands, but **always async** (Tauri v2 runs non-async commands on the main thread, which would freeze the window during file IO).
- **Long jobs** (page snapshots, dry run, render, preview): `start_*` returns a job id at once; the work runs on its own thread; progress, log lines and the result travel over a **Tauri `Channel`** passed by the UI (ordered, per job, made for streaming — better than global events). `cancel_job(id)` flips a cancel flag that the work checks between frames/tiles/steps and then stops with a dedicated error code (to add: "Cancelled").
- The job registry (threads, ids, cancel flags) is shell glue in `src-tauri/src/jobs.rs`; the work itself is library code (`sr-render`, `sr-analyzer`). `sr-render` gets a cancel flag (checked once per frame) and keeps its CLI behaviour (NFR-09).
- **Binary data never goes through JSON IPC** (no base64 images): tiles are files; preview frames go over a channel as raw bytes or through the same protocol as tiles (decided in step 3.8).

### 5. Page boards and block cards — new crate `sr-analyzer` (superseded in part by the 2026-10-29 update: no page boards)
- Opens the page exactly like the renderer (same `sr-site` server, `sr-browser` launch, device emulation, `prepare`), so the canvas shows what the video will show (D-05). Produces: a full-page screenshot cut into **tiles** (e.g. 512 px, levels of detail 1, ½, ¼ …) and **element boxes** with locator candidates (for the inspector, FR-CNV-03).
- Tiles are display-only, so they may be lossy (JPEG/WebP) — the video is always captured fresh.
- **Cache rule (owner, HANDOFF §14.1):** on disk in the project's `cache/<page>/<device>/<fingerprint>/` (fingerprint = site files' sizes and dates for a folder; manual "refresh" for a URL); the last 2 fingerprints per page and device are kept, older ones deleted. In RAM: the UI keeps only the tiles in view plus a small recently-used set under a **hard limit** (~200 MB of decoded images); the OS file cache keeps hot files fast. No RAM disk.
- Tiles reach the webview through a **custom URI protocol** registered by the shell (`sr-tile://…`) that serves only files under the open project's `cache/` — tighter than opening Tauri's general asset protocol to arbitrary folders.
- Linux (WebKitGTK) performance with big images: tiles keep each image small; checked on CI and in the Podman container.

### 6. UI layout (superseded by the 2026-10-29 update)
- `app/App.tsx` switches between the start screen and the editor (two screens: no router library).
- Feature folders as in HANDOFF §7.5: `features/projects`, `features/editor` (screen layout, top bar, device switcher), `features/canvas` (React Flow node types: Start/End oval, Block card, Navigate shape, page board), `features/inspector` (properties panel), `features/render-queue`, `features/preview`; `shared/store` (document + view stores, history).
- The canvas shows one device at a time; positions are shared by all devices; switching the device re-measures cards and boards (FR-CNV-11).

## Options considered (main alternatives)
1. **Editor state in Rust** (UI sends edit commands, Rust keeps the scenario and history): one source of truth, but every drag frame would cross IPC, and undo would live far from the UI that triggers it. Rejected: HANDOFF §7.6 already chose Zustand; Rust keeps the *rules* (validation) instead.
2. **Layout inside the scenario file** (the existing `layout` field): one file, but noisy diffs and bigger files for AI skills. Kept only for single-file export/import.
3. **`zundo` for undo**: less code, but no document/view split and coalescing would be bolted on. Own module is small and tested.
4. **Global Tauri events for progress**: simpler API, but unordered across jobs and no back-pressure. Channels are per job.
5. **Tauri asset protocol for tiles**: no custom code, but needs the folder scope opened at runtime. A small custom protocol serves exactly the cache.

## Consequences
- Step 3.1 adds `sr-project`, the dialog plugin and the scenario types in TypeScript; nothing else depends on React Flow yet.
- The first long job (page snapshots, step 3.5) brings `jobs.rs` and the cancel flag; step 3.7 reuses them for rendering.
- New error codes expected: project folder problems (not a project / newer format / locked), cancelled job, missing site folder. Each with `docs/ERRORS.md` + `en/common.json`.
- Revisit if 200+ nodes are not smooth (NFR-05): React Flow's own virtualization first, then memoized node components.

## Product questions — owner's answers (2026-10-28)
1. **Where new projects go:** the user always chooses the folder. The "new project" form has a location field with a "Choose…" button; the very first time it points to `Documents\ScrollRecord\` (created when needed), afterwards to the last location used (kept by Rust next to the recent list).
2. **Saving:** automatic ~1 s after each change, Ctrl+S at once, no Save button. Later the owner wants an **"Export project"** button: one folder (or archive) with everything needed to render on another computer — scenario, layout, settings and the site itself when it is a folder (HANDOFF §14.1). So paths inside a project must stay portable: keep `meta.site` relative whenever possible and never write machine-specific paths into `project.json`.
3. **New project:** name + site (folder or address), then an empty scenario or an imported scenario file. (Changed 2026-10-29, D-39: "empty" means an empty canvas — no Start/End; devices and formats are chosen in the editor, default one target "Desktop 16:9 1080p".) An empty name becomes **"Untitled N"** (translated via i18n; Russian later "Без названия N"), N = the smallest number with no folder of that name in the chosen location — no upper limit.
4. **Canvas colours:** follow the system light/dark theme (the app's existing light/dark tokens).
5. **Disk cache:** keep the last 2 versions of each page snapshot plus a "Clear cache" button (accepted as proposed).

## Update 2026-10-29 — editor layout (owner, HANDOFF D-38)
The owner described the editor (sketch over a Figma screenshot). It replaces the page boards and the inspector mode on the canvas.

**Window, top to bottom / left to right**
- **Tab bar** at the very top: one tab per open project, like file tabs in Figma; a "+" tab opens the start screen (new / open / recent). Several projects are open at once (FR-CNV-16).
- **Left: "Site map" panel.** Start and End at the top (dragged onto the canvas). Below, a tree: page → its blocks → the interactive elements of each block (anything the cursor can act on: buttons, links, checkboxes, inputs, selects, date pickers, draggable elements; plain text as a target later — keep the element kinds an open list). A "show block previews" switch: off = rows "icon + name" like Figma layers; on = pictures — a collapsed page shows its first screen, an expanded page shows its blocks in order with gaps between them, an expanded block shows its elements one by one.
- **Centre: canvas** (FigJam style) with the scenario only: Start/End ovals, block cards, Navigate shape, arrows. A node arrives by dragging it from the site map, or by clicking it there (placed in the centre of the visible canvas). Arrows are pulled out of a node's edge/handle. No bottom toolbar.
- **Right: properties panel** of the selected node or arrow; at its top a compact row: device switcher, preview, render, render queue. No profile/accounts.

**What changes in the decisions above**
- §1: `layout.json` holds node positions and the canvas viewport only (no board positions). Rust keeps **no "current project"**: several projects are open at once, so every project command takes the project folder; one lock file per project.
- §5: `sr-analyzer` produces the **site map** instead of tiled full-page boards: for each page of the scenario and each device, the blocks — elements marked `data-sr` (D-16) plus the blocks the scenario already names (its locators resolved); **no heuristic auto-detection** (FR-BLK-06 stays MVP+/Phase 4, D-39) — and each block's interactive elements found simply by type (`button`, `a[href]`, `input`, `select`, `textarea`, `[role=button]`…), with boxes and locator candidates; pictures = the page's first screen, one image per block, one per element (lossy, sized for the panel and the cards). No levels-of-detail tiles. The custom protocol (name decided in that step) serves only the `cache/` folders of the open projects. Cache rule, RAM hard limit and "load only visible pictures" stay.
- §6: `App.tsx` holds the tab list instead of switching two screens; each tab owns its project's document store, view store and history. Feature folders: `features/projects` (start screen, tabs), `features/editor` (window layout, the right panel's top row), `features/site-map` (left panel), `features/canvas` (Start/End, block card, Navigate, arrows), `features/inspector` (properties panel), `features/render-queue`, `features/preview`. Switching the device re-measures the site map and the cards (FR-CNV-11).
- Inspector mode (FR-CNV-03) is dropped: blocks and elements are chosen in the site map.
- Follow-up answers (D-39): a project is one site + one video for now (keep room for several scenarios: don't make "one flow per project" impossible to extend); the open tabs are restored at start (kept by Rust with the recent list); a block card has the block's own shape on the current device, at most ~800 px on its long side; page areas on the canvas (FigJam-like sections, can be turned off) and the "header without scrolling to the top" navigation come in Phase 4 with `navigate`.
