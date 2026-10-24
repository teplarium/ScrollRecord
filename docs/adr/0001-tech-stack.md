# 0001 — Phase 0 tech stack

- **Status:** accepted
- **Date:** 2026-10-24
- **Related:** HANDOFF D-01 … D-18

## Context
ScrollRecord must run on Windows and Linux first, and macOS later. The core must be Rust, and the UI needs a FigJam-like canvas. The project is open source (MIT OR Apache-2.0) and maintained by a non-programmer, with AI assistants and contributors. So we favour mainstream, well-documented tools and compile-time checks over clever ones.

## Decisions

| Area | Choice | Why | Rejected |
|---|---|---|---|
| App shell | **Tauri 2** (2.12.x) | Rust backend, small installers, mature on Windows and Linux | Tauri 3 (alpha at the time), egui (canvas UX costs more to build), Electron (not Rust, heavy) |
| UI | **React 19 + TypeScript 5.9 + Vite 8** | Most common stack, so AI models and contributors know it best | TypeScript 7 (native compiler; typescript-eslint did not support it yet) |
| Canvas (Phase 3) | **React Flow** (`@xyflow/react`, MIT) | Nodes, edges, pan/zoom, minimap out of the box | tldraw (licence requires a watermark or a paid key) |
| Rust → TS types | **ts-rs 12** | Stable and simple. Bindings regenerate on `cargo test`, and CI fails if they're stale | tauri-specta (also generates command wrappers, but only release candidates existed: 2.0.0-rc.26) |
| IPC contract | Separate crate **`sr-ipc`** | Bindings can be checked without compiling Tauri | DTOs inside the shell crate |
| i18n | **i18next + react-i18next** (JSON) | De-facto React standard. Supports CLDR plurals for Slavic languages | Fluent (fewer contributors know it) |
| Errors | `SR-xxxx` codes in one macro list (`sr-core`) + `thiserror` | Codes are reportable by users and searchable by AI. Tests enforce docs and translations | Free-form error strings |
| Logging | `tracing` + `tracing-appender` (daily files, 14 kept) | Structured logs, level via `SR_LOG` | `log` + `env_logger` |
| Lint | clippy deny `unwrap`/`expect`/`panic` outside tests. ESLint 9 with `react/jsx-no-literals` | No crashes from lazy error handling. No untranslatable text | ESLint 10 (eslint-plugin-react didn't support it yet) |
| Video encoding (Phase 1) | **ffmpeg as an external process**, downloaded on first run (Windows) or from the system (Linux) | All formats, no GPL linking issues, small installer | Bundled ffmpeg, `openh264` |
| Browser for capture (Phase 1) | **Chromium via CDP** | Same rendering engine on every OS, full control over time, input and viewport | System WebViews (different engine per OS) |

## Consequences
- Two languages (Rust + TS) mean more moving parts. This is mitigated by generated types, a thin IPC layer and a Result-based TS wrapper.
- Pinned versions (Rust 1.97.0, exact npm versions) make builds reproducible. Upgrades are deliberate PRs.
- `csp` is `null` in `tauri.conf.json` during early phases. **Set a strict Content-Security-Policy before the 1.0 release (Phase 6).**
- ESLint 9 is marked deprecated upstream. Move to ESLint 10 when `eslint-plugin-react` supports it.
