# Phase 3 — the editor: plan and progress

Working document shared by Claude Code sessions (see CLAUDE.md). Keep it short: tick items, keep "Next session starts here" current, move details into code, ADRs and PROJECT_MAP.

**Goal (HANDOFF §14):** the Phase 1 scenario (`examples/01-plain-landing.srflow.json`) is built with the mouse in a couple of minutes.
**Requirements:** HANDOFF §5.1 (FR-PRJ), §5.2 (FR-SRC-01/02, FR-VP), §5.3 (FR-CNV), §5.4 (FR-BLK, FR-NAV shapes only), §5.5 (FR-ANC), §5.9 (FR-RND, FR-PRV), §5.11 (FR-I18N), §6 (NFR-02, NFR-05), §7.4 (why cards are snapshots).
**Already decided:** D-01 Tauri v2 · D-02 React + TS + Vite · D-03 React Flow (`@xyflow/react`, MIT; not tldraw) · D-05 snapshot cards + one live block · D-10/D-17/D-18 types from Rust via ts-rs, contract in `sr-ipc` · D-12 i18next · state: Zustand + undo/redo (HANDOFF §7.6).
**Starting point:** the desktop app shows a start screen; `create_project` / `open_project` answer SR-0002 "planned". `sr-render::Renderer` renders from the CLI with a progress callback, but has no cancel and no events for a UI.

## Next session starts here
Step 1b-fix is done, including its side issue (browsers sharing a profile folder, fixed 2026-10-28 in `sr-browser/src/process.rs`; waiting for the owner's push and a green CI run). If a later CI run shows "renders differ", its message says where the frames differ (`heavy_sites.rs` → `Pixels::difference`).
The repository now lives at `E:\Проекты\ScrollRecord` (NVMe, Cyrillic kept for NFR-02). The copy had turned pnpm's links in `node_modules` into empty folders; `pnpm install --frozen-lockfile --force` fixed it (lockfile unchanged). Claude Code memory was copied to the new folder.
Next work: Phase 3, **beginning with step 3.0.** Ideas the owner accepted for later (GPU video encoding, lower render priority, cache limits, audio) are in HANDOFF §14.1. They are not part of Phase 3, apart from the cache note in step 3.0.
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
- [x] Found on the way (separate problem): under CI-like load (all heavy tests, 4 test threads on 4 logical CPUs, `--disable-gpu`) the browser connection sometimes dropped — SR-3003 "Connection reset without closing handshake" (3 of 8 runs), sometimes "Session with given id not found" (also once in a normal run, 2026-10-28). **Cause** (found with a temporary log of every CDP message plus Chrome's own log): Chrome did not crash. Two tests that start their browser right away got the *same* profile folder (name = process id + clock, and both read the clock in the same 100 ns tick — every time the first two launches of a run). The second Chrome handed over to the first and exited with code 21; both tests then drove one browser, and the first to finish closed it under the other. **Fix:** `process.rs` → `claim_profile_dir` creates the folder with `create_dir` and adds a number when the name is taken. Regression test `launches_in_the_same_clock_tick_never_share_a_profile_folder` (fails before the fix: 100 claims got 1 folder). After the fix: 8 of 8 CI-like runs green, every test had its own browser. CHANGELOG, PROJECT_MAP §5/§6, ERRORS.md SR-3003 updated.

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
