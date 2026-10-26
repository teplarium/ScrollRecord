# Phase 3 — the editor: plan and progress

Working document shared by Claude Code sessions (see CLAUDE.md). Keep it short: tick items, keep "Next session starts here" current, move details into code, ADRs and PROJECT_MAP.

**Goal (HANDOFF §14):** the Phase 1 scenario (`examples/01-plain-landing.srflow.json`) is built with the mouse in a couple of minutes.
**Requirements:** HANDOFF §5.1 (FR-PRJ), §5.2 (FR-SRC-01/02, FR-VP), §5.3 (FR-CNV), §5.4 (FR-BLK, FR-NAV shapes only), §5.5 (FR-ANC), §5.9 (FR-RND, FR-PRV), §5.11 (FR-I18N), §6 (NFR-02, NFR-05), §7.4 (why cards are snapshots).
**Already decided:** D-01 Tauri v2 · D-02 React + TS + Vite · D-03 React Flow (`@xyflow/react`, MIT; not tldraw) · D-05 snapshot cards + one live block · D-10/D-17/D-18 types from Rust via ts-rs, contract in `sr-ipc` · D-12 i18next · state: Zustand + undo/redo (HANDOFF §7.6).
**Starting point:** the desktop app shows a start screen; `create_project` / `open_project` answer SR-0002 "planned". `sr-render::Renderer` renders from the CLI with a progress callback, but has no cancel and no events for a UI.

## Next session starts here
Step 1b-fix is done and CI is green on both OSes (commit `4205a6b`). If a later CI run shows "renders differ", its message says where the frames differ (`heavy_sites.rs` → `Pixels::difference`).
**The owner is moving the repository from the HDD (`D:\Загрузки\ScrollRecord`) to the NVMe disk E:** (suggested `E:\Проекты\ScrollRecord`, keeping Cyrillic in the path so NFR-02 is still exercised locally). First thing in the new folder: `git status --short`, then the AGENTS.md §4 checks once (`target\` is rebuilt from scratch). Claude Code's memory is stored per folder: copy `%USERPROFILE%\.claude\projects\D-----------ScrollRecord\memory\` into the new folder's memory directory (same path pattern, created on the first session there), then fix the paths in it.
Next work, in this order: (1) the open side issue, last item of 1b-fix. The session that started on it was interrupted overnight before it found anything and changed no files, so start fresh from that item. (2) Phase 3, **beginning with step 3.0.** Ideas the owner accepted for later (GPU video encoding, lower render priority, cache limits, audio) are in HANDOFF §14.1. They are not part of Phase 3, apart from the cache note in step 3.0.
This PC has Rust 1.97 (`%USERPROFILE%\.cargo\bin`, prefix PATH in PowerShell), VS 2022 Build Tools and pnpm (via corepack). Google Chrome 155 is installed and is the browser the tests use.

## Steps
Each step ends with every check green (AGENTS.md §4), docs updated and a Russian report. "(owner)" = the owner clicks through the app to confirm.

### 1b-fix Windows CI: the GSAP render is not identical — before 3.0
CI on commit `85c565f`, job `Rust (windows-latest)`: `gsap_fixture_renders_identically` (`crates/sr-render/tests/heavy_sites.rs`) failed with "renders differ in frames [25, 26, 27, 28, 29, 30, 31]". Linux passes, also 10 of 10 runs under heavy CPU load in the cloud, so it is Windows-specific.
What those frames are (10 fps, see that test's `timeline.json`): the page scrolls 0 → 2700 px from 1500 to 2650 ms, then holds on `reveal`. Frames 25–31 are exactly the reveal tween of cards A/B/C (`toggleActions: "play …"`, 0.6 s + 0.1 s stagger, starts when the cards cross `top 85%` during the scroll). Frames before and after are identical, so the layout is the same and only that tween differs.
- [x] Reproduce on this Windows PC. With the GPU (NVIDIA) it never failed (60+ runs, also under CPU load). It fails like on CI only in **software drawing** (`--disable-gpu`, what a GPU-less CI runner uses): under CPU load, frames 26–31 differed in 4 of 7 comparisons; with 7 s pauses between frames, frames 25–31 always.
- [x] Tell the causes apart: page state per frame was identical in all runs (scroll, GSAP time, card opacity/transform, ScrollTrigger starts; no stray `resize`/`visibilitychange`/trusted `scroll`). Only one pixel row at the bottom/right edge of a moving card differed → browser drawing, not the page.
- [x] Fix in the engine: `--disallow-non-exact-resource-reuse` in `sr-browser/src/process.rs` → `flags()` (cause in ADR 0004). Regression test `software_frames_do_not_depend_on_leftover_tile_memory` (`crates/sr-browser/tests/browser.rs`) fails 3/3 without the flag. Remaining limit (frames slower than ~2.5 s without a GPU) documented in ADR 0004.
- [x] Update CHANGELOG (Fixed), ADR 0004, PROJECT_MAP §5, this plan.
- [x] (owner) Push; check that "Rust (windows-latest)" in GitHub Actions is green (commit `4205a6b`, green on 2026-10-26).
- [ ] Found on the way, not fixed (separate problem, also without the new flag): under CI-like load (all heavy tests, 4 test threads on 4 logical CPUs, `--disable-gpu`) the browser connection sometimes drops — `css_effects_follow_page_time` fails in `frame()` → `page.step` with SR-3003 "Runtime.evaluate: connection error: … Connection reset without closing handshake" (2 of 6 runs before the fix, 3 of 8 after), once a render failed with "Session with given id not found". Seen once with the GPU too, under heavier load. Not seen in normal runs or on CI so far. Next: keep the browser's stderr log (`browser-stderr.log` in the profile folder, deleted by `BrowserProcess::stop`) when a test fails, find out whether Chrome crashes, then fix.

### 3.0 Architecture and questions — 1 session
- [ ] Read the desktop shell (`apps/desktop/src/`, `apps/desktop/src-tauri/src/`), `crates/sr-ipc`, the public API of `crates/sr-render`.
- [ ] ADR 0005 "Editor architecture": project folder format and where its logic lives (proposal: new crate `sr-project`, all file IO there); editor state (Zustand store of Flow + layout, undo/redo as a command history); IPC commands vs events (render progress, preview frames); long jobs off the UI thread with cancel; how page snapshots/tiles are made and cached (proposal: new crate `sr-analyzer`, HANDOFF §7.5). Cache rule from the owner (HANDOFF §14.1): recent snapshots in RAM under a hard limit, the rest on disk in the project folder; no RAM disk.
- [ ] Adjust steps 3.1–3.9 if the code suggests a better split.
- [ ] Ask the owner the open questions in one message (e.g. default folder for new projects).

### 3.1 Projects — FR-PRJ-01…04, FR-SRC-01/02 — 1–2 sessions
- [ ] Project folder: `project.json`, `flow.srflow.json`, `layout.json`, `cache/`, `renders/`. Create / open / save with atomic writes. Test with a Cyrillic path like `D:\Загрузки\…` (NFR-02).
- [ ] Start screen works: new, open, recent projects (replace SR-0002).
- [ ] Site source: folder or URL. Import an existing `*.srflow.json`, export the scenario.
- [ ] Autosave and recovery after a crash (FR-PRJ-02).
- [ ] (owner) Create a project for `tests/fixtures/sites/01-plain-landing`, close the app, reopen the project.

### 3.2 Editor screen and state — 1 session
- [ ] Layout: canvas · properties panel · top bar (device switcher, targets, Render button).
- [ ] Store: Flow + layout, undo/redo of every edit (FR-PRJ-05), dirty flag → autosave.
- [ ] Validation from Rust (`sr-core` plan/validate) as an IPC command returning issues per node; problem nodes highlighted (FR-CNV-09).

### 3.3 Canvas: nodes and arrows — FR-CNV-01, -07, -08, -12 — 1–2 sessions
- [ ] React Flow: pan, zoom, fit, minimap. Start/End ovals, Block cards (plain for now), Navigate shape (drawing only; navigation itself is Phase 4).
- [ ] Arrows between nodes; create from toolbar and shortcuts; delete; edges carry scroll/cursor settings.
- [ ] Auto-layout for scenarios without `layout` (FR-CNV-12). Stays smooth with 200+ nodes (NFR-05).

### 3.4 Properties panel and targets — FR-CNV-10, FR-BLK-01/02/04/05, FR-ANC, FR-VP — 1–2 sessions
- [ ] Block: target locators, align + offset, `hold_ms`; actions list (hover / click / move / wait: delay, duration, mode, only/except).
- [ ] Start/End: block + anchor + offset, `warmup_ms`. Arrow: scroll speed or duration, easing, cursor still/follow/hide.
- [ ] Scenario defaults (cursor pack, seed, scroll) and targets (device presets, aspect, resolution, fps, format); per-device overrides.

### 3.5 Page boards and block cards — FR-CNV-02, -04, -11 — 2 sessions
- [ ] `sr-analyzer`: open the page exactly like the renderer (same Chromium, emulation, prepare), full-page screenshot cut into tiles with levels of detail, DOM boxes of elements; cached in the project's `cache/` per site + device.
- [ ] Page board on the canvas (tiles load only where visible); block cards show the crop of their section; switching the device re-measures.
- [ ] Watch WebKitGTK performance with big images (HANDOFF risk 4) — CI only, unless the owner has Linux.

### 3.6 Inspector mode — FR-CNV-03 — 1 session
- [ ] Hover on a board highlights elements (boxes from 3.5, no live DOM); click creates a block with generated locators (`data-sr` first, then id / CSS / text).

### 3.7 Render queue and dry run — FR-RND-02/03/04 — 1–2 sessions
- [ ] All targets with one button; progress as IPC events; cancel; one failing target does not stop the others; file name template `{project}_{target}_{date}`; open the output folder.
- [ ] Dry run: check every locator on every device without recording.
- [ ] `sr-render` gets cancel + progress events; the CLI keeps working the same (NFR-09).

### 3.8 Preview — FR-PRV-01 — 1 session
- [ ] Low-quality real-time frame stream of the scenario in the app (CDP screencast, ADR 0002 option 1).

### 3.9 Acceptance — 1 session
- [ ] (owner) Build the example 01 scenario with the mouse in a couple of minutes and render it.
- [ ] HANDOFF: Phase 3 ✅ in §14, new decisions; PROJECT_MAP; CHANGELOG; ADRs up to date.

## Not in Phase 3
Live block FR-CNV-05 (MVP+), mini timeline, multi-select and snapping (MVP+), executing `navigate` and auto-detecting blocks (Phase 4), AI skills and Cursor Studio (Phase 5).
