# Phase 3 — the editor: plan and progress

Working document shared by Claude Code sessions (see CLAUDE.md). Keep it short: tick items, keep "Next session starts here" current, move details into code, ADRs and PROJECT_MAP.

**Goal (HANDOFF §14):** the Phase 1 scenario (`examples/01-plain-landing.srflow.json`) is built with the mouse in a couple of minutes.
**Requirements:** HANDOFF §5.1 (FR-PRJ), §5.2 (FR-SRC-01/02, FR-VP), §5.3 (FR-CNV), §5.4 (FR-BLK, FR-NAV shapes only), §5.5 (FR-ANC), §5.9 (FR-RND, FR-PRV), §5.11 (FR-I18N), §6 (NFR-02, NFR-05), §7.4 (why cards are snapshots).
**Already decided:** D-01 Tauri v2 · D-02 React + TS + Vite · D-03 React Flow (`@xyflow/react`, MIT; not tldraw) · D-05 snapshot cards + one live block · D-10/D-17/D-18 types from Rust via ts-rs, contract in `sr-ipc` · D-12 i18next · state: Zustand + undo/redo (HANDOFF §7.6).
**Starting point:** the desktop app shows a start screen; `create_project` / `open_project` answer SR-0002 "planned". `sr-render::Renderer` renders from the CLI with a progress callback, but has no cancel and no events for a UI.

## Next session starts here
Steps 1b-fix and **1c are done** (1c on 2026-10-28: stage timings in the render report, Chrome/ffmpeg below normal priority; numbers and the GPU-encoding verdict in HANDOFF §14.1 — postponed, it would not speed renders up). Waiting for the owner's push and a green CI run on both OSes; the Linux side of the priority test (`nice`, `ps -o ni=`) has only run on CI. If a later CI run shows "renders differ", its message says where the frames differ (`heavy_sites.rs` → `Pixels::difference`).
The repository lives at `E:\Проекты\ScrollRecord` (NVMe, Cyrillic kept for NFR-02).
Next work: Phase 3 **beginning with step 3.0.** Carry into 3.0/3.7: render targets in parallel (1c showed the CPU mostly idle). The other ideas the owner accepted for later (GPU video encoding, cache limits, audio) are in HANDOFF §14.1; only the cache note belongs to step 3.0.
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
  Done: `background_command` in `process.rs` and `sr-encoder`; `low_priority` in `LaunchOptions`, `EncodeSettings`, `RenderOptions` (the one switch); `Browser::pid()`; CLI `--normal-priority`. Tests: `browser_runs_below_normal_priority_unless_turned_off`, ffmpeg priority in `raw_frames_become_a_video_when_ffmpeg_is_installed`. **Found:** the plan's "children inherit it" is only half true on Windows — Chrome raises its tab (normal) and GPU process (above normal) itself; no switch stops it (HANDOFF §14.1, comment in `process.rs`). Linux not checked on a real machine yet: CI runs the test.
- [x] CHANGELOG, PROJECT_MAP (§3 types, §5), HANDOFF §14.1 (mark done), ERRORS.md only if a new code appears (none).
- Side finding, not fixed: 9:16 at 4K on `iphone-15` gives 2160×3842, not 2160×3840 (`sr-core/src/flow/target.rs` → `geometry` derives the output height from the rounded CSS height). Plays fine; fix only together with a decision on crop vs. slight stretch.

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
(From 1c: try rendering 2–3 targets at the same time; one render keeps only ≈ 1.3 of 12 cores busy. Show `RenderReport.stages` in the queue.)
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
