# PROJECT_MAP — where everything lives

> **Read this first.** It lets you (human or AI) find the right file without scanning the repo.
> **Update it in the same commit** whenever you add, rename or remove a crate, a feature folder, a public type or an IPC command.
> Requirements and the roadmap are in [`HANDOFF.md`](HANDOFF.md) (Russian). Rules for AI assistants are in [`AGENTS.md`](AGENTS.md).

**Current phase:** 3 — editor (plan and progress: [`docs/phase-3-plan.md`](docs/phase-3-plan.md)) · Done: 0, 1, 1b, 2

---

## 1. Overview (30 seconds)

ScrollRecord renders smooth, frame-perfect scroll videos of websites for any device (Windows + Linux first, macOS later). It has two layers:

- **Rust core** (`crates/`): all logic, as small independent libraries. The `scrollrecord` CLI renders videos today.
- **Desktop app** (`apps/desktop/`): a Tauri shell (`src-tauri/`, thin Rust glue) plus a React UI (`src/`). Phase 0 start screen only; the editor arrives in Phase 3.

**Render pipeline** (all in `sr-render::Renderer`):

```
*.srflow.json ──► sr-core: Flow::from_json → validate → plan(target)   (ordered start → blocks → end)
                     │
sr-site: folder → local HTTP server (or URL); ranges, clean URLs, SPA fallback
                     │
sr-cursor: load the cursor pack (built-in or folder) — fails early if broken
                     │
sr-browser: find Chrome/Edge → launch headless → new tab → emulate device
            → navigate → prepare (hide, lazy images, fonts) → measure blocks
            → probe: scroll to each block with actions, let it appear,
              measure its buttons → reload the page (ADR 0003)
                     │
sr-timeline: plan + measurements → scroll segments + cursor track
             (moves, clicks, hide/follow) → frame list (scroll_y + cursor per frame)
                     │
sr-capture: per frame: page.step(scroll_y, 1/fps, pointer) ──► page.capture_png()
            (pointer = mouse position, presses; page reports arrow/hand/I-beam)
                     │                                     │
                     │   js/runtime.js inside the page:    ▼  worker thread
                     │   frozen clock, timers, rAF,     sr-compositor: PNG → pixels,
                     │   idle callbacks, CSS animations, ripples, touch circles, cursor
                     │   scroll events, observers,         │
                     │   framework tasks, video/audio,     │
                     │   animated images, cursor kind      │
                     │                                     ▼
                     │                                  sr-encoder: VideoSink (raw RGBA)
                     │                                  ffmpeg (MP4/WebM) or PNG folder
                     ▼
              <name>_<target>.mp4  +  <name>_<target>.timeline.json
```

The UI talks to Rust only through **IPC commands** whose types are **generated** from Rust into TypeScript.

---

## 2. Modules

### 2.1 Rust crates (`crates/`)

| Crate | Status | Purpose | Key files |
|---|---|---|---|
| `sr-core` | ✅ | Scenario format v1, easing, `SR-xxxx` error catalog, validation, per-target plan. **No IO** | `flow/mod.rs` (types, parsing), `flow/path.rs` (validation + `plan()`), `flow/target.rs` (devices, aspect, resolution → pixel geometry), `flow/locator.rs`, `flow/action.rs` (hover/click/move/wait), `flow/cursor.rs` (cursor settings, scroll behaviour), `easing.rs`, `error.rs` |
| `sr-project` | ✅ | Project folder: create / open / save / close, atomic writes with `.bak`, lock file, recent projects + open tabs (`projects.json`), portable site paths, scenario import/export. No Tauri | `project.rs` (`ProjectStore`, IPC types), `manifest.rs` (`project.json`), `fsio.rs` (atomic write, path clean-up), `lock.rs`, `config.rs`, `site.rs`; tests: `tests/project.rs` |
| `sr-ipc` | ✅ | IPC contract DTOs, exported to TypeScript; re-exports `Flow` (sr-core) and the project types (sr-project) | `app_info.rs`, `lib.rs` |
| `sr-diagnostics` | ✅ | Logging setup (later: problem-report zip) | `lib.rs` |
| `sr-site` | ✅ | Site source: folder → built-in HTTP server on 127.0.0.1 (std only: thread per connection, byte ranges, clean URLs, single-page-app fallback), or URL | `lib.rs` (`SiteSource`, `serve`), `server.rs` |
| `sr-browser` | ✅ | Find & launch Chrome/Edge/Chromium; minimal DevTools-protocol client; device emulation (incl. fixed media preferences); mouse/touch input; page runtime | `locate.rs`, `process.rs`, `cdp.rs`, `page.rs` (incl. `probe`, pointer input), **`js/runtime.js`** (virtual time, animations, scroll events, framework tasks, video/audio, animated images, prepare, measure, locators, cursor kind, settle) |
| `sr-timeline` | ✅ | Plan + measurements → segments (scroll/hold) + cursor track → frames. Pure maths | `lib.rs` (`PageMetrics`, `Timeline`, waypoint merging, stops), `cursor.rs` (human-like moves, clicks, taps, hide/follow, seeded randomness) |
| `sr-cursor` | ✅ | Cursor packs: `cursor-pack.json` format, built-in `classic-dark` / `classic-light`, safe SVG → pixels, click animation | `manifest.rs`, `raster.rs`, `lib.rs`, **`packs/`** (built-in SVGs) |
| `sr-compositor` | ✅ | Draws ripples, touch circles and the cursor onto each captured frame → RGBA | `lib.rs` |
| `sr-capture` | ✅ | Frame loop: warm-up, step (+ pointer), capture, push `CapturedFrame` to a `FrameSink` | `lib.rs` |
| `sr-encoder` | ✅ | `VideoSink`s for raw RGBA frames: ffmpeg pipe (MP4 H.264, WebM VP9) and PNG folder | `lib.rs` |
| `sr-render` | ✅ | Orchestrator used by CLI (and later the app): `Renderer::start/render/finish`, probe + reload, compositing worker thread, output naming, `timeline.json` | `lib.rs`; tests: `tests/render_fixture.rs` (fixture 01), `tests/heavy_sites.rs` (fixtures 02–06) |
| `sr-cli` | ✅ | `scrollrecord` command: `render`, `validate`, `info`, `errors` | `main.rs`, `commands/*.rs` |
| `sr-analyzer` | ⏳ Phase 3 | Site map for the editor: blocks (`data-sr` + scenario blocks), their interactive elements, boxes, locators, pictures (ADR 0006 update, D-38/D-39); later block auto-detection (Phase 4), site digest (Phase 5) | — |
| `sr-skill` | ⏳ Phase 5 | AI skill packages | — |

Crates are created only when their phase starts (HANDOFF D-18).

### 2.2 Desktop shell (`apps/desktop/src-tauri/`)

| Path | Purpose |
|---|---|
| `src/main.rs` | Binary entry; hides the console window on Windows release builds |
| `src/lib.rs` | `run()`: logging start, state, **list of registered IPC commands** |
| `src/state.rs` | `AppState`: process-wide state (log guard, `ProjectStore`, projects open in this window — locks released on exit) |
| `src/commands/` | One file per feature area; thin handlers returning `Result<T, AppError>` |
| `tauri.conf.json` | Window, bundle targets (NSIS, deb, AppImage), identifier `io.github.teplarium.scrollrecord` |
| `capabilities/default.json` | Permissions of the main window: `core:default`, `dialog:allow-open` (folder/file pickers) |
| `icons/` | Generated from `assets/brand/app-icon.png` with `pnpm tauri icon` |

### 2.3 UI (`apps/desktop/src/`)

| Path | Purpose |
|---|---|
| `main.tsx` | Mounts React, loads i18n and global styles |
| `app/App.tsx`, `app/AppFooter.tsx` | Window layout (tabs / screen / footer), restores last session's tabs; footer with versions and the language picker |
| `app/TabBar.tsx`, `app/tabs.ts` | Project tabs (Home, one per project, "+"); tab rules as a tested reducer (`tabs.test.ts`) |
| `features/projects/StartScreen.tsx`, `NewProjectForm.tsx`, `RecentList.tsx` | Home tab: new project form, open project ("Open anyway" on SR-1005), recent list |
| `features/editor/EditorPlaceholder.tsx` | Project tab until the canvas (3.2): name, folder, site, "Locate site folder…" |
| `features/settings/LanguageSelect.tsx` | UI language picker |
| `features/README.md` | Planned feature folders and the "no cross-feature imports" rule |
| `shared/ipc/commands.ts` | **Typed wrappers for every IPC command** (returns `IpcResult<T>`) |
| `shared/ipc/dialogs.ts` | Native folder/file pickers (`tauri-plugin-dialog`); `null` outside the app |
| `shared/ipc/errors.ts`, `useAppInfo.ts` | Error normalisation; `AppInfo` hook |
| `shared/ipc/generated/` | **Generated by `cargo test`, never edit** |
| `shared/ui/` | `ErrorBanner`, `icons` |
| `styles/global.css` (bottom) | Shared controls: `.button`, `.input`, `.segmented` |
| `i18n/` | i18next setup, locale discovery, `formatError()`; **all English text** in `locales/en/common.json` |
| `styles/global.css` | Design tokens for dark and light themes |

### 2.4 Everything else

| Path | Purpose |
|---|---|
| `packaging/cli/` | Files shipped in the CLI package: `README.txt`, `render(.bat/.sh)` (drag & drop), `render-example(.bat/.sh)` |
| `scripts/package-cli.sh` | Builds `dist/ScrollRecord-CLI/` (used by CI job `cli`) |
| `examples/` | Example scenarios; `01-plain-landing.srflow.json` renders the fixture site (desktop 16:9 + iPhone 9:16) with hover, click and tap; `02…06` render the heavy test sites |
| `tests/fixtures/sites/` | Test pages for capture: `01-plain-landing/` (plain HTML/CSS/JS), `02-css-transitions/`, `03-gsap-scrolltrigger/`, `04-lenis-smooth-scroll/`, `05-react-spa/` (source + built `app.js`), `06-media/` (video, Lottie, canvas, GIF). Third-party libraries in each `vendor/` with a licence note. Overview: `tests/fixtures/sites/README.md` |
| `HANDOFF.md` | Requirements, decisions (D-xx), roadmap (Russian) |
| `AGENTS.md` | Rules for AI assistants (`CLAUDE.md` and `GEMINI.md` import it) |
| `CLAUDE.md` | Claude Code sessions: talking to the owner, saving context, session-to-session handover |
| `docs/phase-3-plan.md` | Phase 3 steps, progress and "Next session starts here" (memory between AI sessions) |
| `CONTRIBUTING.md` | Dev setup, workflow, recipes |
| `docs/ERRORS.md` | Error catalog: one `## SR-xxxx` section per code |
| `docs/adr/` | Decisions: `0001` stack, `0002` deterministic capture, `0003` cursor & composition, `0004` heavy sites (animations, scroll events, media, site server), `0005` render priority (browser lowered on Windows only), `0006` editor architecture (Phase 3: project folder, state, IPC jobs, site-map pictures; layout update 2026-10-29) |
| `.github/` | CI (Rust + UI on Windows & Linux; artifacts: CLI package + installers), issue forms |
| `.cargo/config.toml` | Where ts-rs writes TypeScript bindings |
| `rust-toolchain.toml` | Pinned Rust version (1.97.0) |

---

## 3. Key types index

| Type | Defined in | Role |
|---|---|---|
| `Flow`, `Node`, `Edge`, `Anchor`, `Align`, `Prepare`, `Defaults`, `Override` | `sr-core/src/flow/mod.rs` | Scenario file format v1 |
| `Target`, `Viewport`, `Aspect`, `Resolution`, `OutputFormat`, `OutputGeometry` | `sr-core/src/flow/target.rs` | What to render, pixel maths |
| `Locator` | `sr-core/src/flow/locator.rs` | How a block finds its element |
| `Action`, `ActionKind`, `PointerMode`, `Point` | `sr-core/src/flow/action.rs` | What the cursor does in a block |
| `CursorSettings`, `ScrollCursor`, `CursorKind` | `sr-core/src/flow/cursor.rs` | Cursor look/behaviour; arrow/hand/I-beam/touch |
| `FlowPlan`, `PlanBlock`, `PlanAction`, `MeasureItem`, `ScrollMove` | `sr-core/src/flow/path.rs` | Validated, ordered plan for one target |
| `Easing` | `sr-core/src/easing.rs` | CSS-style timing curves |
| `ErrorCode`, `AppError`, `AppResult` | `sr-core/src/error.rs` | Error catalog (TS: `generated/`) |
| `SiteSource`, `ServedSite`, `LocalServer` | `sr-site/src/` | Site folder/URL |
| `Browser`, `Page`, `Emulation`, `Measurements`, `FrameDriver`, `PointerInput`, `StepResult` | `sr-browser/src/` | Browser control |
| `PageMetrics`, `Timeline`, `Segment`, `FramePlan` | `sr-timeline/src/lib.rs` | Frame plan |
| `CursorTrack`, `CursorSegment`, `ClickMark`, `CursorFrame` | `sr-timeline/src/cursor.rs` | Cursor plan and per-frame cursor |
| `CursorPack`, `Manifest`, `Sprite`, `Pose` | `sr-cursor/src/` | Cursor packs |
| `Compositor`, `Composed` | `sr-compositor/src/lib.rs` | Cursor onto frames |
| `FrameSink`, `CapturedFrame`, `capture()`, `Progress` | `sr-capture/src/lib.rs` | Frame loop |
| `VideoSink`, `FfmpegEncoder`, `PngSequence`, `EncodeSettings`, `Quality` | `sr-encoder/src/lib.rs` | Output |
| `Renderer`, `RenderOptions`, `RenderReport`, `StageTimings` | `sr-render/src/lib.rs` | Whole pipeline; per-stage busy times in the report (`CaptureStats` in `sr-capture` measures the frame loop) |
| `ProjectStore`, `NewProject`, `OpenedProject`, `ImportedFlow`, `ProjectHome`, `RecentProject` | `sr-project/src/project.rs` | Project operations and their IPC data |
| `ProjectManifest`, `RenderSettings` | `sr-project/src/manifest.rs` | `project.json` |
| `ProjectsConfig`, `RecentEntry` | `sr-project/src/config.rs` | `projects.json` (recent list, last location, open tabs) |
| `AppInfo` | `sr-ipc/src/app_info.rs` | Versions for UI/bug reports |
| `IpcResult<T>`, `commands` | `apps/desktop/src/shared/ipc/commands.ts` | UI → Rust calls |

---

## 4. IPC map

| Command (Rust) | Handler file | TS wrapper | Returns | Used in |
|---|---|---|---|---|
| `app_info` | `src-tauri/src/commands/app.rs` | `commands.appInfo()` | `AppInfo` | `useAppInfo` → `AppFooter` |
| `project_home` | `src-tauri/src/commands/project.rs` | `commands.projectHome()` | `ProjectHome` | `App` (restore tabs), `StartScreen` |
| `create_project` | `src-tauri/src/commands/project.rs` | `commands.createProject(request)` | `OpenedProject` | `NewProjectForm` |
| `open_project` | `src-tauri/src/commands/project.rs` | `commands.openProject(dir, force)` | `OpenedProject` | `App`, `StartScreen` |
| `save_project` | `src-tauri/src/commands/project.rs` | `commands.saveProject(dir, flow, layout)` | `null` | `EditorPlaceholder` (later: 3.4 autosave) |
| `close_project` | `src-tauri/src/commands/project.rs` | `commands.closeProject(dir)` | `null` | `App` (tab close) |
| `forget_recent` | `src-tauri/src/commands/project.rs` | `commands.forgetRecent(dir)` | `null` | `StartScreen` (recent list) |
| `portable_site` | `src-tauri/src/commands/project.rs` | `commands.portableSite(dir, site)` | `string \| null` | `EditorPlaceholder` ("Locate site folder…") |
| `import_flow` | `src-tauri/src/commands/project.rs` | `commands.importFlow(dir, file)` | `ImportedFlow` | (editor) |
| `export_flow` | `src-tauri/src/commands/project.rs` | `commands.exportFlow(dir, dest)` | `null` | (editor) |

---

## 5. If X breaks, look at Y

| Symptom | Where to look |
|---|---|
| Video stutters / repeated frames while scrolling | `sr-capture` loop; `js/runtime.js` → `frame()`; check `timeline.json` segments |
| Two renders of the same scenario differ | `js/runtime.js` (time shim, `syncAnimations`, observer settle); `page.rs` → `freeze_animation_clock`; ADR 0002 |
| An animation plays too fast / doesn't play | `js/runtime.js` → `syncAnimations` (only *running* animations on the page clock are moved; paused and scroll-driven ones are left to the site/browser); `warmup_ms` on the start node; fixture 02 |
| Effect that depends on scroll speed/direction is missing (Lenis, "hide header on scroll") | `runtime.js` → `announceScroll` and the capture `scroll` listener (one event per changed position); fixture 04 |
| React/Vue content appears one frame late or differs between renders | `runtime.js` → `drainMessages` (MessageChannel tasks), `requestIdleCallback`; fixture 05 |
| A `<video>` shows the wrong frame / differs between renders | `runtime.js` → "Video and audio" (`syncMedia`, autoplay take-over); server byte ranges in `sr-site/src/server.rs`; fixture 06, ADR 0004 |
| Animated GIF plays at the wrong speed / differs | `runtime.js` → `syncImages` / `decodeAnimation` (ImageDecoder); CSS background GIFs are not handled (ADR 0004) |
| Page never finishes loading (SR-3004) on a local folder | `sr-site/src/server.rs` (thread per connection); test `pages_with_many_files_always_load` |
| Deep link of a React/Vue app (`/pricing`) gives 404, or `/about` doesn't open `about.html` | `sr-site/src/server.rs` → `resolve` (clean URLs, single-page-app fallback only for `Accept: text/html`) |
| Video looks different on another computer (dark theme, "reduce motion") | `page.rs` → `emulate` (`Emulation.setEmulatedMedia`) |
| Scroll-reveal content missing | `runtime.js` IntersectionObserver wrapper; `prepare.preload_lazy` |
| Block lands under the sticky header / wrong place | `runtime.js` → `topInset()`; `sr-timeline` `align_y` / `anchor_y`; `offset_px` |
| `SR-2101` / `SR-2102` block not found / ambiguous | Locators in the scenario; `runtime.js` → `findAll()`; message lists what was tried. `block 'x': target of actions[n]` = an action's target (searched inside its block first) |
| Cursor misses the button / hover doesn't react | `timeline.json` → `page.blocks["block#n"]` (measured box) and `cursor`; `sr-render` → `probe_targets`; `runtime.js` → `settleAt`; warning "outside the visible area" = the button isn't on screen at that stop (change `align`/`offset_px`) |
| Hover differs between two renders | `page.rs` → `send_pointer` (position re-sent every live frame); `runtime.js` → `pointer()`; ADR 0003 |
| Two renders differ only in one pixel row at the edge of a moving element (machine without a GPU) | Browser drawing, not the page: `process.rs` → `flags()` (`--disallow-non-exact-resource-reuse`); test `software_frames_do_not_depend_on_leftover_tile_memory`; ADR 0004 "Browser drawing without a GPU". `heavy_sites.rs` failure messages show where frames differ |
| Wrong cursor image (arrow vs hand vs I-beam) | `runtime.js` → `cursorKind()` (CSS `cursor` of the element under the pointer) |
| Cursor looks wrong / blurry / misplaced tip | `sr-cursor` (`raster.rs`, pack `hotspot`/`size`); `sr-compositor` → `draw()` |
| Cursor moves robotically / too fast | `sr-timeline/src/cursor.rs` constants (Fitts timing, reaction, overshoot); scenario `cursor.speed`, `cursor.seed` |
| `SR-6001` / `SR-6002` cursor pack | `sr-cursor` (`CursorPack::load`, `manifest.rs` checks, `parse_svg`); run `scrollrecord validate` |
| `SR-2001` / `SR-2002` scenario errors | `sr-core/src/flow/` (`mod.rs` parsing, `path.rs` checks). Run `scrollrecord validate` |
| `SR-3003` "Connection reset without closing handshake" / "Session with given id not found" while several browsers run at once (parallel tests, renders) | Two browsers on one profile folder (Chrome exit code 21 = handed over to the browser already using that folder): `process.rs` → `claim_profile_dir`; test `launches_in_the_same_clock_tick_never_share_a_profile_folder` |
| `SR-3001`–`SR-3005` browser problems | `sr-browser` (`locate.rs`, `process.rs`, `cdp.rs`, `page.rs`); run with `-v` |
| `SR-5001` / `SR-5002` encoding | `sr-encoder` (`find_ffmpeg` / `ffmpeg_candidates` search order, `ffmpeg_args`); `detail` param has ffmpeg's output |
| Render is slow — which stage? | CLI prints `stages:` lines after each target (`--json`: `stages`). Measured in `sr-render` → `render` / `CompositeSink` worker and `sr-capture` → `capture` (`CaptureStats`). High "waiting for the worker" = cursor drawing or encoding is the bottleneck |
| Computer sluggish while rendering, or render much slower than before | Below-normal priority by default: `background_command` in `sr-browser/src/process.rs` (browser, **Windows only**) and `sr-encoder/src/lib.rs` (ffmpeg; Windows `BELOW_NORMAL_PRIORITY_CLASS`, Unix `nice -n 10`); CLI `--normal-priority` turns it off; tests `browser_priority_is_lowered_only_on_windows`, `raw_frames_become_a_video_when_ffmpeg_is_installed`; ADR 0005 |
| Linux: "browser did not open its DevTools port within 30s" (SR-3002) | Was caused by starting Chrome through `nice` (CI run #11); the browser must stay at normal priority on Unix (ADR 0005). Otherwise: Chrome's stderr tail in the message, `--no-sandbox` as root |
| `render.bat` / CLI package problems | `packaging/cli/`, `scripts/package-cli.sh`, CI job `cli` |
| `SR-8001`–`SR-8003` site | `sr-site` (`SiteSource::parse`, `server.rs`) |
| Wrong video size / aspect | `sr-core/src/flow/target.rs` → `Target::geometry`; ffmpeg `-s` (input) and `scale=` (output) in `sr-encoder` |
| Colours look washed out | `sr-encoder` → `ffmpeg_args` (BT.709 tagging) |
| Footer shows `SR-0003` | UI opened outside the desktop app. Use `pnpm dev`, not `pnpm dev:ui` |
| New Rust type missing in TypeScript | `#[derive(TS)] #[ts(export)]`, then `cargo test` |
| CI: bindings out of date | Run `cargo test --workspace`, commit `apps/desktop/src/shared/ipc/generated/` |
| CI: error code tests fail | Add `## SR-xxxx` to `docs/ERRORS.md` and `errors.SR-xxxx` to `en/common.json` |
| ESLint `react/jsx-no-literals` | Move visible text to the locale file, use `t("...")` |
| Window doesn't open on Linux | Install `libwebkit2gtk-4.1-0` |

---

## 6. Runtime data locations

| What | Windows | Linux |
|---|---|---|
| App log files (`scrollrecord.YYYY-MM-DD.log`, last 14 kept) | `%LOCALAPPDATA%\io.github.teplarium.scrollrecord\logs\` | `~/.local/share/io.github.teplarium.scrollrecord/logs/` |
| CLI log files | only with `--log-dir <DIR>` | same |
| Temporary browser profile (deleted after each render) | `%TEMP%\scrollrecord-browser-<pid>-<time>-<n>\` | `/tmp/scrollrecord-browser-<pid>-<time>-<n>/` |
| Render output (CLI default) | `renders\` next to the scenario file | same |
| UI preferences (language) | WebView local storage | WebView local storage |
| Recent projects, last location, open tabs (`projects.json`) | `%APPDATA%\io.github.teplarium.scrollrecord\` | `~/.config/io.github.teplarium.scrollrecord/` |
| New projects (first suggestion) | `Documents\ScrollRecord\` | `~/Documents/ScrollRecord/` |
| Project lock (deleted when the tab closes or the app exits) | `<project>\.scrollrecord.lock` | same |

Environment variables: `SR_LOG=debug` (log level), `SR_BROWSER=<path>` (browser), `SR_FFMPEG=<path>` (ffmpeg).

---

## 7. Dependency rules

1. `sr-core` depends on no other ScrollRecord crate.
2. Library crates never depend on `sr-cli` or `sr-desktop`.
3. Order (lower may not import higher): `core` → `project` → `ipc` / `diagnostics` / `site` / `browser` / `timeline` / `cursor` → `capture` / `compositor` → `encoder` → `render` → `cli` / `desktop`. (`encoder` needs neither `capture` nor `compositor`: it takes plain RGBA.)
4. UI features import from `shared/` and `i18n/`, never from another feature.
5. The UI reaches Rust **only** through `shared/ipc/commands.ts`.

---

## 8. Structure changelog

| Date | Change |
|---|---|
| 2026-10-24 | Phase 0: workspace, `sr-core`, `sr-ipc`, `sr-diagnostics`, `sr-cli`, Tauri + React shell, i18n, CI, docs |
| 2026-10-24 | CLI package: `packaging/cli/`, `scripts/package-cli.sh`, CI job `cli`; IntersectionObserver delivery made deterministic (ADR 0002) |
| 2026-10-24 | Phase 1: scenario format v1 in `sr-core`; new crates `sr-site`, `sr-browser`, `sr-timeline`, `sr-capture`, `sr-encoder`, `sr-render`; CLI `render`/`validate`; `examples/`; fixture 01 extended; ADR 0002 |
| 2026-10-25 | Phase 2: `sr-core` `flow/action.rs`, `flow/cursor.rs`; `sr-timeline/src/cursor.rs`; new crates `sr-cursor` (with `packs/`), `sr-compositor`; `sr-capture` `FrameSink` takes `CapturedFrame`; `sr-encoder` `VideoSink` takes raw RGBA (no longer depends on `sr-capture`); `sr-browser` pointer input + probe; codes SR-6001/6002; ADR 0003 |
| 2026-10-25 | Phase 1b: fixtures `02`–`06` (+ `vendor/` folders), `sr-render/tests/heavy_sites.rs`, examples `02`–`06`; `runtime.js` animation control reworked (ADR 0004), media/animated-image/scroll-event/framework-task handling; `sr-site` server rewritten on std (tiny_http removed): ranges, clean URLs, SPA fallback; ADR 0004 |
| 2026-10-25 | `CLAUDE.md` rewritten for Claude Code sessions; `docs/phase-3-plan.md` added (Phase 3 started) |
| 2026-10-28 | Step 1c: `RenderReport.stages` (`StageTimings`), `CaptureStats` times; `low_priority` in `LaunchOptions` / `EncodeSettings` / `RenderOptions`, `Browser::pid()`; CLI `--normal-priority`; ADR 0005 |
| 2026-10-28 | Step 3.0: ADR 0006 (editor architecture, accepted with the owner's answers); Phase 3 steps re-split in `docs/phase-3-plan.md`. No code changes |
| 2026-10-29 | Editor layout from the owner (HANDOFF D-38, D-39): project tabs, left "Site map" panel, canvas with the scenario only, right properties panel; no page boards / inspector mode. ADR 0006 "Update 2026-10-29", Phase 3 steps re-split. No code changes |
| 2026-10-29 | Step 3.1a: new crate `sr-project`; scenario types (`sr-core::flow`) exported to TypeScript (`Node` is `FlowNode` there); project IPC commands + `commands.ts` wrappers; `AppState` holds the `ProjectStore`; codes SR-1003…1006 |
| 2026-10-29 | Step 3.1b: `tauri-plugin-dialog` (+ `dialog:allow-open`); `app/TabBar.tsx`, `app/tabs.ts`; `features/projects/NewProjectForm.tsx`, `RecentList.tsx`; `features/editor/` (placeholder); `shared/ipc/dialogs.ts`; command `portable_site`; shared control styles in `global.css` |
