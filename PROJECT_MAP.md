# PROJECT_MAP — where everything lives

> **Read this first.** It lets you (human or AI) find the right file without scanning the repo.
> **Update it in the same commit** whenever you add, rename or remove a crate, a feature folder, a public type or an IPC command.
> Requirements and the roadmap are in [`HANDOFF.md`](HANDOFF.md) (Russian). Rules for AI assistants are in [`AGENTS.md`](AGENTS.md).

**Current phase:** 1 — render prototype (CLI) ✅ · **Next:** Phase 2 — cursor & composition

---

## 1. Overview (30 seconds)

ScrollRecord renders smooth, frame-perfect scroll videos of websites for any device (Windows + Linux first, macOS later). It has two layers:

- **Rust core** (`crates/`): all logic, as small independent libraries. The `scrollrecord` CLI renders videos today.
- **Desktop app** (`apps/desktop/`): a Tauri shell (`src-tauri/`, thin Rust glue) plus a React UI (`src/`). Phase 0 start screen only; the editor arrives in Phase 3.

**Render pipeline** (all in `sr-render::Renderer`):

```
*.srflow.json ──► sr-core: Flow::from_json → validate → plan(target)   (ordered start → blocks → end)
                     │
sr-site: folder → local HTTP server (or URL)
                     │
sr-browser: find Chrome/Edge → launch headless → new tab → emulate device
            → navigate → prepare (hide, lazy images, fonts) → measure blocks
                     │
sr-timeline: plan + measurements → timed segments → frame list (scroll_y per frame)
                     │
sr-capture: per frame: page.step(scroll_y, 1/fps)  ──►  page.capture_png()
                     │                                     │
                     │   js/runtime.js inside the page:    ▼
                     │   frozen clock, timers, rAF,     sr-encoder: FrameSink
                     │   CSS animations, observers      ffmpeg (MP4/WebM) or PNG folder
                     ▼
              <name>_<target>.mp4  +  <name>_<target>.timeline.json
```

The UI talks to Rust only through **IPC commands** whose types are **generated** from Rust into TypeScript.

---

## 2. Modules

### 2.1 Rust crates (`crates/`)

| Crate | Status | Purpose | Key files |
|---|---|---|---|
| `sr-core` | ✅ | Scenario format v1, easing, `SR-xxxx` error catalog, validation, per-target plan. **No IO** | `flow/mod.rs` (types, parsing), `flow/path.rs` (validation + `plan()`), `flow/target.rs` (devices, aspect, resolution → pixel geometry), `flow/locator.rs`, `easing.rs`, `error.rs` |
| `sr-ipc` | ✅ | IPC contract DTOs, exported to TypeScript | `app_info.rs` |
| `sr-diagnostics` | ✅ | Logging setup (later: problem-report zip) | `lib.rs` |
| `sr-site` | ✅ | Site source: folder → built-in HTTP server on 127.0.0.1, or URL | `lib.rs` (`SiteSource`, `serve`), `server.rs` |
| `sr-browser` | ✅ | Find & launch Chrome/Edge/Chromium; minimal DevTools-protocol client; device emulation; page runtime | `locate.rs`, `process.rs`, `cdp.rs`, `page.rs`, **`js/runtime.js`** (virtual time, prepare, measure, locators) |
| `sr-timeline` | ✅ | Plan + measurements → segments (scroll/hold) → frames. Pure maths | `lib.rs` (`PageMetrics`, `Timeline`, waypoint merging) |
| `sr-capture` | ✅ | Frame loop: warm-up, step, capture, push to `FrameSink` | `lib.rs` |
| `sr-encoder` | ✅ | `FrameSink`s: ffmpeg pipe (MP4 H.264, WebM VP9) and PNG folder | `lib.rs` |
| `sr-render` | ✅ | Orchestrator used by CLI (and later the app): `Renderer::start/render/finish`, output naming, `timeline.json` | `lib.rs` |
| `sr-cli` | ✅ | `scrollrecord` command: `render`, `validate`, `info`, `errors` | `main.rs`, `commands/*.rs` |
| `sr-cursor` | ⏳ Phase 2 | Cursor packs, paths, rendering | — |
| `sr-compositor` | ⏳ Phase 2 | Stage: aspect, fit, background, block crop, cursor overlay | — |
| `sr-analyzer` | ⏳ Phase 3 | DOM boxes for the editor, block auto-detection, tiles, site digest | — |
| `sr-skill` | ⏳ Phase 5 | AI skill packages | — |

Crates are created only when their phase starts (HANDOFF D-18).

### 2.2 Desktop shell (`apps/desktop/src-tauri/`)

| Path | Purpose |
|---|---|
| `src/main.rs` | Binary entry; hides the console window on Windows release builds |
| `src/lib.rs` | `run()`: logging start, state, **list of registered IPC commands** |
| `src/state.rs` | `AppState`: process-wide state (log guard) |
| `src/commands/` | One file per feature area; thin handlers returning `Result<T, AppError>` |
| `tauri.conf.json` | Window, bundle targets (NSIS, deb, AppImage), identifier `io.github.teplarium.scrollrecord` |
| `capabilities/default.json` | Permissions of the main window (only `core:default`) |
| `icons/` | Generated from `assets/brand/app-icon.png` with `pnpm tauri icon` |

### 2.3 UI (`apps/desktop/src/`)

| Path | Purpose |
|---|---|
| `main.tsx` | Mounts React, loads i18n and global styles |
| `app/App.tsx`, `app/AppFooter.tsx` | Window layout; footer with versions from `app_info` and the language picker |
| `features/projects/StartScreen.tsx` | Start screen: new / open project, recent list |
| `features/settings/LanguageSelect.tsx` | UI language picker |
| `features/README.md` | Planned feature folders and the "no cross-feature imports" rule |
| `shared/ipc/commands.ts` | **Typed wrappers for every IPC command** (returns `IpcResult<T>`) |
| `shared/ipc/errors.ts`, `useAppInfo.ts` | Error normalisation; `AppInfo` hook |
| `shared/ipc/generated/` | **Generated by `cargo test`, never edit** |
| `shared/ui/` | `ErrorBanner`, `icons` |
| `i18n/` | i18next setup, locale discovery, `formatError()`; **all English text** in `locales/en/common.json` |
| `styles/global.css` | Design tokens for dark and light themes |

### 2.4 Everything else

| Path | Purpose |
|---|---|
| `packaging/cli/` | Files shipped in the CLI package: `README.txt`, `render(.bat/.sh)` (drag & drop), `render-example(.bat/.sh)` |
| `scripts/package-cli.sh` | Builds `dist/ScrollRecord-CLI/` (used by CI job `cli`) |
| `examples/` | Example scenarios; `01-plain-landing.srflow.json` renders the fixture site (desktop 16:9 + iPhone 9:16) |
| `tests/fixtures/sites/` | Test pages for capture (`01-plain-landing/`: plain HTML/CSS/JS with `data-sr` blocks) |
| `HANDOFF.md` | Requirements, decisions (D-xx), roadmap (Russian) |
| `AGENTS.md` | Rules for AI assistants (`CLAUDE.md` and `GEMINI.md` point here) |
| `CONTRIBUTING.md` | Dev setup, workflow, recipes |
| `docs/ERRORS.md` | Error catalog: one `## SR-xxxx` section per code |
| `docs/adr/` | Decisions: `0001` stack, `0002` deterministic capture |
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
| `FlowPlan`, `PlanBlock`, `ScrollMove` | `sr-core/src/flow/path.rs` | Validated, ordered plan for one target |
| `Easing` | `sr-core/src/easing.rs` | CSS-style timing curves |
| `ErrorCode`, `AppError`, `AppResult` | `sr-core/src/error.rs` | Error catalog (TS: `generated/`) |
| `SiteSource`, `ServedSite`, `LocalServer` | `sr-site/src/` | Site folder/URL |
| `Browser`, `Page`, `Emulation`, `Measurements`, `FrameDriver` | `sr-browser/src/` | Browser control |
| `PageMetrics`, `Timeline`, `Segment`, `FramePlan` | `sr-timeline/src/lib.rs` | Frame plan |
| `FrameSink`, `capture()`, `Progress` | `sr-capture/src/lib.rs` | Frame loop |
| `FfmpegEncoder`, `PngSequence`, `EncodeSettings`, `Quality` | `sr-encoder/src/lib.rs` | Output |
| `Renderer`, `RenderOptions`, `RenderReport` | `sr-render/src/lib.rs` | Whole pipeline |
| `AppInfo` | `sr-ipc/src/app_info.rs` | Versions for UI/bug reports |
| `IpcResult<T>`, `commands` | `apps/desktop/src/shared/ipc/commands.ts` | UI → Rust calls |

---

## 4. IPC map

| Command (Rust) | Handler file | TS wrapper | Returns | Used in |
|---|---|---|---|---|
| `app_info` | `src-tauri/src/commands/app.rs` | `commands.appInfo()` | `AppInfo` | `useAppInfo` → `AppFooter` |
| `create_project` | `src-tauri/src/commands/project.rs` | `commands.createProject()` | `null` (now: SR-0002) | `StartScreen` |
| `open_project` | `src-tauri/src/commands/project.rs` | `commands.openProject()` | `null` (now: SR-0002) | `StartScreen` |

---

## 5. If X breaks, look at Y

| Symptom | Where to look |
|---|---|
| Video stutters / repeated frames while scrolling | `sr-capture` loop; `js/runtime.js` → `frame()`; check `timeline.json` segments |
| Two renders of the same scenario differ | `js/runtime.js` (time shim, `syncAnimations`, observer settle); `page.rs` → `freeze_animation_clock`; ADR 0002 |
| An animation plays too fast / doesn't play | `js/runtime.js` → `syncAnimations` (site-paused animations are skipped on purpose); `warmup_ms` on the start node |
| Scroll-reveal content missing | `runtime.js` IntersectionObserver wrapper; `prepare.preload_lazy` |
| Block lands under the sticky header / wrong place | `runtime.js` → `topInset()`; `sr-timeline` `align_y` / `anchor_y`; `offset_px` |
| `SR-2101` / `SR-2102` block not found / ambiguous | Locators in the scenario; `runtime.js` → `findAll()`; message lists what was tried |
| `SR-2001` / `SR-2002` scenario errors | `sr-core/src/flow/` (`mod.rs` parsing, `path.rs` checks). Run `scrollrecord validate` |
| `SR-3001`–`SR-3005` browser problems | `sr-browser` (`locate.rs`, `process.rs`, `cdp.rs`, `page.rs`); run with `-v` |
| `SR-5001` / `SR-5002` encoding | `sr-encoder` (`find_ffmpeg` / `ffmpeg_candidates` search order, `ffmpeg_args`); `detail` param has ffmpeg's output |
| `render.bat` / CLI package problems | `packaging/cli/`, `scripts/package-cli.sh`, CI job `cli` |
| `SR-8001`–`SR-8003` site | `sr-site` (`SiteSource::parse`, `server.rs`) |
| Wrong video size / aspect | `sr-core/src/flow/target.rs` → `Target::geometry`; ffmpeg `scale=` in `sr-encoder` |
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
| Temporary browser profile (deleted after each render) | `%TEMP%\scrollrecord-browser-<pid>-<n>\` | `/tmp/scrollrecord-browser-<pid>-<n>/` |
| Render output (CLI default) | `renders\` next to the scenario file | same |
| UI preferences (language) | WebView local storage | WebView local storage |

Environment variables: `SR_LOG=debug` (log level), `SR_BROWSER=<path>` (browser), `SR_FFMPEG=<path>` (ffmpeg).

---

## 7. Dependency rules

1. `sr-core` depends on no other ScrollRecord crate.
2. Library crates never depend on `sr-cli` or `sr-desktop`.
3. Order (lower may not import higher): `core` → `ipc` / `diagnostics` / `site` / `browser` / `timeline` → `capture` → `encoder` → `render` → `cli` / `desktop`.
4. UI features import from `shared/` and `i18n/`, never from another feature.
5. The UI reaches Rust **only** through `shared/ipc/commands.ts`.

---

## 8. Structure changelog

| Date | Change |
|---|---|
| 2026-10-24 | Phase 0: workspace, `sr-core`, `sr-ipc`, `sr-diagnostics`, `sr-cli`, Tauri + React shell, i18n, CI, docs |
| 2026-10-24 | CLI package: `packaging/cli/`, `scripts/package-cli.sh`, CI job `cli`; IntersectionObserver delivery made deterministic (ADR 0002) |
| 2026-10-24 | Phase 1: scenario format v1 in `sr-core`; new crates `sr-site`, `sr-browser`, `sr-timeline`, `sr-capture`, `sr-encoder`, `sr-render`; CLI `render`/`validate`; `examples/`; fixture 01 extended; ADR 0002 |
