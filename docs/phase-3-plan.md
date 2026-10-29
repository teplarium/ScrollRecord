# Phase 3 — the editor: plan and progress

Working document shared by Claude Code sessions (see CLAUDE.md). Keep it short: tick items, keep "Next session starts here" current, move details into code, ADRs and PROJECT_MAP.

**Goal (HANDOFF §14):** the Phase 1 scenario (`examples/01-plain-landing.srflow.json`) is built with the mouse in a couple of minutes.
**Architecture:** ADR 0006 (read it before any Phase 3 step), including its "Update 2026-10-29" (editor layout, D-38).
**Requirements:** HANDOFF §5.1 (FR-PRJ), §5.2 (FR-SRC-01/02, FR-VP), §5.3 (FR-CNV, incl. -16 tabs), §5.4 (FR-BLK incl. -06 auto-detection, FR-NAV shapes only), §5.5 (FR-ANC), §5.9 (FR-RND, FR-PRV), §5.11 (FR-I18N), §6 (NFR-02, NFR-05), §7.4 (why cards are snapshots).
**Already decided:** D-01 Tauri v2 · D-02 React + TS + Vite · D-03 React Flow (`@xyflow/react`, MIT; not tldraw) · D-05 snapshot cards + one live block · D-10/D-17/D-18 types from Rust via ts-rs, contract in `sr-ipc` · D-12 i18next · state: Zustand + undo/redo (HANDOFF §7.6) · D-36/D-37 project folder, autosave · **D-38 layout: project tabs on top · left "Site map" (Start/End + page → blocks → interactive elements, preview switch) · centre canvas with the scenario only (no page board, no bottom toolbar; nodes dragged/clicked in from the site map, arrows pulled from node edges) · right properties panel with device / preview / render / queue on top.**
**Starting point:** the desktop app shows a start screen; `create_project` / `open_project` answer SR-0002 "planned". `sr-render::Renderer` renders from the CLI with a progress callback, but has no cancel and no events for a UI.

## Next session starts here
Steps 1b-fix and **1c are done** (1c on 2026-10-28: stage timings in the render report, ffmpeg and — on Windows — Chrome below normal priority; numbers and the GPU-encoding verdict in HANDOFF §14.1 — postponed, it would not speed renders up). CI run #11 failed on Linux because Chrome was started through `nice`; fixed the same day (step 1c, "CI fix", ADR 0005). Waiting for the owner's push and a green CI run on both OSes. Linux can be reproduced locally: Podman container `ubuntu:24.04` + Google Chrome (see the Claude Code memory note). If a later CI run shows "renders differ", its message says where the frames differ (`heavy_sites.rs` → `Pixels::difference`).
The repository lives at `E:\Проекты\ScrollRecord` (NVMe, Cyrillic kept for NFR-02).
**Step 3.0 is done** (2026-10-28): ADR 0006 "Editor architecture" is accepted, with the owner's answers at its end, and the steps below are re-split.
**2026-10-29:** the owner described the editor layout (sketch over a Figma screenshot) → **D-38** in HANDOFF, "Update 2026-10-29" in ADR 0006, steps 3.1–3.9 below re-split (page boards and inspector mode dropped; site map in the left panel; block auto-detection moved into Phase 3; project tabs). The owner is still answering the other questions of that session: is a project one site + one scenario or several videos; are devices/formats chosen when creating a project; does a new project start empty (Start → End); what a block card shows; one device at a time or side by side; what else in HANDOFF is wrong. Write the answers into HANDOFF/ADR/this plan **before** starting 3.1a (answer 1 may change the project folder format).
Next work: **step 3.1a** (crate `sr-project`, scenario types to TypeScript, project commands) once those answers are recorded. Carry into 3.7: render targets in parallel (1c showed the CPU mostly idle). The other ideas the owner accepted for later (GPU video encoding, audio) are in HANDOFF §14.1.
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

### 1c Before the editor: render stage timings and lower priority — 1 session ✅ 2026-10-28
HANDOFF §14.1 ("step 0" of GPU encoding, and lower render priority). CLI/core only, no UI; the Phase 3 render queue will show these numbers later.
- [x] **Stage timings** in `sr-render::RenderReport`: busy time per stage — browser start, page load + prepare, probe + reload, per-frame `page.step`, screenshot, PNG decode + cursor compositing (worker thread), encoding (time blocked writing to ffmpeg + waiting in `finish`). Compositing runs in parallel with capture, so stages don't add up to `elapsed_s`: say so in the field docs and the CLI output. Where: `sr-render/src/lib.rs` (`render`, compositing worker), `sr-capture/src/lib.rs` (`capture` loop), `sr-encoder/src/lib.rs` (`VideoSink`), `sr-cli/src/commands/render.rs` (summary). Test: report has every stage, each > 0, for fixture 01 (PNG output, no ffmpeg needed).
  Done: `StageTimings` (`report.stages`), times in `sr-capture::CaptureStats`, worker times in `CompositeSink`; one extra stage `worker_wait_s` (render thread blocked on the worker = cursor/encoding is the bottleneck). Browser start is counted in the session's first target. Tests: `render_fixture.rs` (every stage > 0, render-thread stages ≤ `elapsed_s`), `sr-capture` `times_steps_screenshots_and_sink_separately`.
- [x] Measure on this PC (RTX 3050, ffmpeg 9 with NVENC in WinGet): example 01 as MP4 at Full HD and at 4K. Write the numbers into HANDOFF §14.1 and recommend to the owner whether GPU encoding is worth it.
  Result: screenshots ≈ ⅔ and page steps ≈ ⅓ of the time; the render waited for encoding ≤ 0.3 s, even at 4K. GPU encoding would not speed renders up (it would only cut CPU load at 4K) → postponed. The CPU is mostly idle (≈ 1.3 of 12 cores), so rendering targets in parallel is the big win → try in step 3.7.
- [x] **Lower priority** for Chrome and ffmpeg: Windows `BELOW_NORMAL_PRIORITY_CLASS` (0x4000) next to `CREATE_NO_WINDOW` in `process.rs` → `hide_console_window` and in `sr-encoder` → `spawn` (Chrome's own child processes inherit it); Linux: start through `nice -n 10` if `nice` exists (it execs the program, so the pid stays the browser's). On by default, an option to turn it off (`LaunchOptions`, `EncodeSettings`; CLI flag). Frames stay identical (only slower) — the existing "renders identically" tests prove it. Test: the launched browser really runs below normal (Windows: `Get-Process -Id <pid>` → `PriorityClass`; Linux: `ps -o ni= -p <pid>`).
  Done: `background_command` in `process.rs` and `sr-encoder`; `low_priority` in `LaunchOptions`, `EncodeSettings`, `RenderOptions` (the one switch); `Browser::pid()`; CLI `--normal-priority`. Tests: `browser_priority_is_lowered_only_on_windows`, ffmpeg priority in `raw_frames_become_a_video_when_ffmpeg_is_installed`. **Found:** the plan's "children inherit it" is only half true on Windows — Chrome raises its tab (normal) and GPU process (above normal) itself; no switch stops it (HANDOFF §14.1, comment in `process.rs`). On Linux the browser is not lowered at all (CI fix below, ADR 0005).
- [x] CHANGELOG, PROJECT_MAP (§3 types, §5), HANDOFF §14.1 (mark done), ERRORS.md only if a new code appears (none).
- [x] **CI fix (run #11, commit `b584b56`):** on `Rust (ubuntu-24.04)` all four `sr-browser/tests/browser.rs` tests failed: "browser did not open its DevTools port within 30s" — the first run with Chrome started through `nice` (run #10 without it was green). Reproduced in a Podman container (Ubuntu 24.04 + Chrome 156, as a non-root user): a niced Chrome starts fine when idle, but 4–6× slower once normal-priority programs keep every core busy; the runner's exact load could not be reproduced. Lowering only after startup needs unsafe calls or a separate process group (Ctrl+C would orphan Chrome). **Decision (ADR 0005):** the browser is lowered on Windows only; ffmpeg stays niced on Unix (nothing waits on it with a time limit). Test renamed to `browser_priority_is_lowered_only_on_windows`; on Linux it fails if `nice` comes back. (owner) Push and check both CI jobs.
- Side finding, not fixed: 9:16 at 4K on `iphone-15` gives 2160×3842, not 2160×3840 (`sr-core/src/flow/target.rs` → `geometry` derives the output height from the rounded CSS height). Plays fine; fix only together with a decision on crop vs. slight stretch.

### 3.0 Architecture and questions — 1 session ✅ 2026-10-28
- [x] Read the desktop shell (`apps/desktop/src/`, `apps/desktop/src-tauri/src/`), `crates/sr-ipc`, the public API of `crates/sr-render`. Found: only `AppInfo`/`AppError`/`ErrorCode` reach TypeScript; no React Flow, Zustand or file dialog yet; `Flow` already has an optional `layout` field.
- [x] ADR 0006 "Editor architecture" (accepted): `sr-project` crate + folder format + atomic writes + recent list in Rust; `sr-core::flow` types exported with ts-rs; validation stays in Rust (`validate_flow` → issues per node); Zustand document store + view store + own undo history; async commands; long jobs = job id + Tauri `Channel` + cancel flag (`src-tauri/src/jobs.rs`); `sr-analyzer` tiles on disk in `cache/`, served by a custom `sr-tile://` protocol, RAM hard limit in the UI. New deps: `tauri-plugin-dialog` (3.1), `@xyflow/react` + `zustand` (3.2), `tauri-plugin-opener` (3.7).
- [x] Steps re-split (owner agreed 2026-10-28): projects first, then the canvas in two parts — "see" before "edit" — so the owner can judge the canvas early.
- [x] (owner) Answered the 5 questions (2026-10-28, end of ADR 0006): user picks the folder (first suggestion `Documents\ScrollRecord\`, then the last one used); autosave, no Save button; empty name → "Untitled N"; canvas follows the system theme; cache keeps 2 snapshot versions + "Clear cache". New idea for later: "Export project" for another computer (HANDOFF §14.1).

Re-split 2026-10-29 for D-38 (editor layout). The old "3.5 Page boards" and "3.6 Inspector mode" are gone: the site map in the left panel replaces both.

### 3.1 Projects and tabs — FR-PRJ-01…04, FR-SRC-01/02, FR-CNV-16 — 2 sessions
**3.1a Rust side (1 session)**
- [ ] New crate `sr-project` (ADR 0006 §1): `project.json`, `flow.srflow.json`, `layout.json`, `cache/`, `renders/`; create / open / save with atomic writes (`.tmp` → rename, `.bak` kept); lock file per project; recent list + last location used, in the app config folder. Default name "Untitled N" (smallest free N in the chosen folder); first suggested location `Documents\ScrollRecord\`. Keep paths portable (relative `meta.site` when possible) for the later "Export project". Tests incl. a Cyrillic path (NFR-02), a project next to its site folder, and "Untitled 1/2/3" numbering with gaps.
- [ ] Several projects open at once (tabs, D-38): Rust keeps no "current project"; every project command takes the project folder. Opening a project that is already open in this window is the UI's job (switch to its tab), not a lock-file error.
- [ ] `sr-core::flow` types → TypeScript (ts-rs, `#[ts(type = …)]` for custom serde forms); a test that a parsed example round-trips through JSON unchanged.
- [ ] Import a `*.srflow.json` (its `layout` → `layout.json`) and export one (layout embedded).
- [ ] Async Tauri commands `create_project`, `open_project`, `save_project`, `recent_projects`, `import_flow`, `export_flow` + wrappers in `commands.ts`; new error codes (not a project / newer format / locked / site folder missing) with ERRORS.md + `en/common.json`.
**3.1b Tabs and start screen (1 session)**
- [ ] Tab bar at the very top (D-38): one tab per open project + "+"; switch, close. "+" (and an app with no open project) shows the start screen inside a tab.
- [ ] `tauri-plugin-dialog` for folder/file pickers. Start screen: new project form (name — empty means "Untitled N"; location with "Choose…", first `Documents\ScrollRecord\` then the last one used; site folder or URL; empty scenario or import), open, recent list (missing folders shown and removable).
- [ ] A project tab shows an editor placeholder with the project name and site; autosave wiring comes with the store in 3.4.
- [ ] (owner) Create a project for `tests/fixtures/sites/01-plain-landing`, open a second one, switch tabs, close the app, reopen both from the recent list.

### 3.2 Window layout and canvas, part 1 — see the scenario — FR-CNV-01, -07, -08 (display), -10 (frame), -12 — 1 session
- [ ] Editor layout per D-38: tabs on top · left "Site map" panel (for now: Start and End at the top, the scenario's `pages` below, no blocks yet) · canvas · right panel (empty properties area; top row with device switcher, preview, render and queue buttons as placeholders). No bottom toolbar.
- [ ] React Flow (`@xyflow/react`): pan, zoom, "fit all", minimap. Node types: Start/End oval, Block card (name + hold time, picture area empty until 3.6), Navigate shape (drawing only; navigation itself is Phase 4). Arrows from the scenario's edges.
- [ ] Own auto-layout for scenarios without layout (column in path order, branches offset) (FR-CNV-12). Stays smooth with 200+ nodes (NFR-05): a generated 250-node test scenario.
- [ ] (owner) Open the example 01 project: three panels as in the sketch; the scenario appears as a chain Start → hero → … → End; zoom, pan, minimap work.

### 3.3 Site map, part 1 — structure — FR-CNV-02 (list mode), FR-BLK-06 — 2 sessions
- [ ] New crate `sr-analyzer` (ADR 0006 §5 + update): open each page exactly like the renderer (same `sr-site`, `sr-browser`, emulation, `prepare`); find the **blocks** (`data-sr` first, then `header`/`section`/`footer`, large children of `main`/`body`, `id` anchors) and, inside each block, its **interactive elements** (buttons, links, inputs, checkboxes, selects, date pickers, draggable elements; element kinds an open list so plain text can be added later), with boxes and locator candidates (`data-sr` first, then id / role+name / CSS / text). Tests on the fixture sites (01 plain landing at least; 05 React SPA).
- [ ] First long job: `src-tauri/src/jobs.rs` (job id, `Channel` progress, cancel flag) + "Cancelled" error code. Results cached in the project's `cache/<page>/<device>/<fingerprint>/`, last 2 fingerprints kept, "Clear cache".
- [ ] Left panel tree in list mode (preview switch off): page → blocks → elements as "icon + name" rows, expand/collapse like Figma layers; Start/End stay at the top.
- [ ] (owner) Open the example 01 project: the site map lists the page, its blocks and the buttons inside them.

### 3.4 Canvas, part 2 — edit — FR-CNV-07, -08, -09, FR-PRJ-02, -05 — 1–2 sessions
- [ ] `shared/store`: per tab a document store `{flow, layout}` + view store (selection, zoom, device) + own undo history (ADR 0006 §3) with unit tests (drag = one step, typing coalesced, 200-step cap).
- [ ] Nodes come from the site map: drag a block (or Start/End) onto the canvas, or click it in the panel → it appears in the centre of the visible canvas. Arrows pulled out of a node's edge/handle onto another node. Delete arrows and nodes, move nodes (positions → `layout.json`); Ctrl+Z / Ctrl+Shift+Z.
- [ ] Autosave (per owner answer 2) + save on close / tab close; lock-file warning.
- [ ] `sr-core` issues get node/edge ids; command `validate_flow`; problem nodes highlighted with the message on hover (FR-CNV-09).
- [ ] (owner) Drag "hero" onto the canvas, connect Start → hero → End by pulling arrows, undo/redo, close and reopen: the change is there.

### 3.5 Properties panel and targets — FR-CNV-10, FR-BLK-01/02/04/05, FR-ANC, FR-VP — 1–2 sessions
- [ ] Block: target locators, align + offset, `hold_ms`; actions list (hover / click / move / wait: delay, duration, mode, only/except). An action's element is picked from that block's elements in the site map.
- [ ] Start/End: block + anchor + offset, `warmup_ms`. Arrow: scroll speed or duration, easing, cursor still/follow/hide.
- [ ] Scenario defaults (cursor pack, seed, scroll) and targets (device presets, aspect, resolution, fps, format); per-device overrides. Device switcher in the right panel's top row.

### 3.6 Site map, part 2 — pictures — FR-CNV-02 (preview mode), -04, -11 — 1–2 sessions
- [ ] `sr-analyzer` pictures: the page's first screen, one image per block, one per interactive element (lossy, sized for the panel and the cards), into the same cache.
- [ ] Custom protocol (ADR 0006 §5 + update) serving only the `cache/` folders of the open projects. Only visible pictures are loaded, RAM hard limit.
- [ ] "Show block previews" switch: collapsed page = first screen; expanded page = its blocks in order with gaps; expanded block = its elements one by one. Block cards on the canvas show their block's picture. Switching the device re-measures the site map and the cards.
- [ ] Watch WebKitGTK performance with many images (HANDOFF risk 4): CI, and the Podman Linux container (memory note).

### 3.7 Render queue and dry run — FR-RND-02/03/04 — 1–2 sessions
(From 1c: try rendering 2–3 targets at the same time; one render keeps only ≈ 1.3 of 12 cores busy. Show `RenderReport.stages` in the queue.)
- [ ] All targets with one button (render and queue buttons in the right panel's top row); progress over the job `Channel` (ADR 0006 §4); cancel; one failing target does not stop the others; file name template `{project}_{target}_{date}`; open the output folder (`tauri-plugin-opener`).
- [ ] Dry run: check every locator on every device without recording.
- [ ] `sr-render` gets a cancel flag checked once per frame; the CLI keeps working the same (NFR-09).

### 3.8 Preview — FR-PRV-01 — 1 session
- [ ] Low-quality real-time frame stream of the scenario in the app (CDP screencast, ADR 0002 option 1), opened from the preview button in the right panel's top row.

### 3.9 Acceptance — 1 session
- [ ] (owner) Build the example 01 scenario with the mouse in a couple of minutes and render it.
- [ ] HANDOFF: Phase 3 ✅ in §14, new decisions; PROJECT_MAP; CHANGELOG; ADRs up to date.

## Not in Phase 3
Live block FR-CNV-05 (MVP+), mini timeline, multi-select and snapping (MVP+), executing `navigate` (Phase 4; block auto-detection moved into 3.3 by D-38), plain text as an action target (later; keep it easy to add), page boards and inspector mode (dropped by D-38), AI skills and Cursor Studio (Phase 5).
