# Contributing to ScrollRecord

Thanks for helping! This project is maintained largely through AI assistants and community contributions, so **clear issues, tests and comments matter more than anything else**.

- Found a bug? Use the issue forms. Include the `SR-xxxx` error code and the output of `scrollrecord info` (or a screenshot of the app footer).
- Writing code? Read [PROJECT_MAP.md](PROJECT_MAP.md) and [AGENTS.md](AGENTS.md) first. The rules there apply to humans too.

## 1. Set up your machine

| Tool | Why | Windows | Linux (Ubuntu/Debian) |
|---|---|---|---|
| Rust (via rustup) | Core + desktop shell. The exact version is pinned in `rust-toolchain.toml` and installs automatically | [rustup.rs](https://rustup.rs) | `curl https://sh.rustup.rs -sSf \| sh` |
| C/C++ build tools | Rust links native code | Visual Studio Build Tools, workload "Desktop development with C++" | `sudo apt install build-essential` |
| WebView | Renders the UI | WebView2 (preinstalled on Windows 10/11) | `sudo apt install libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf libxdo-dev` |
| Node.js 22+ | UI tooling | [nodejs.org](https://nodejs.org) | via your package manager or nvm |
| pnpm | JS package manager (version pinned in `package.json`) | `corepack enable` | `corepack enable` |

Then, from the repository root:

```sh
pnpm install
pnpm dev                 # desktop app with hot reload
```

## 2. Everyday commands

| Command | What it does |
|---|---|
| `pnpm dev` | Run the desktop app (Rust + UI) with hot reload |
| `pnpm dev:ui` | UI only in a browser. Rust calls fail with `SR-0003`, which is expected |
| `pnpm build` | Build installers into `target/release/bundle/` |
| `pnpm check` | UI: lint + format check + typecheck + tests |
| `pnpm format` | Auto-format UI code |
| `cargo test --workspace` | Rust tests. **Also regenerates TypeScript bindings**, so commit them if they change |
| `cargo clippy --workspace --all-targets -- -D warnings` | Rust lints |
| `cargo fmt --all` | Auto-format Rust code |
| `cargo run -p sr-cli -- info` | The `scrollrecord` CLI: shows the browser and ffmpeg it found |
| `cargo run --release -p sr-cli -- render examples/01-plain-landing.srflow.json` | Render the example videos (needs Chrome/Edge/Chromium + ffmpeg) |

Set `SR_LOG=debug` to get verbose logs. `SR_BROWSER` and `SR_FFMPEG` point to a specific browser or ffmpeg.

Rendering tests (`sr-browser`, `sr-render`) run only when a Chromium-based browser is installed; otherwise they print "skipped" and pass.

## 3. Pull requests

1. One topic per PR. Explain **what** and **why** in plain language.
2. CI must be green: Rust and UI checks on Windows and Linux.
3. Bug fix → add a test that failed before the fix.
4. Structure changed → update `PROJECT_MAP.md`. User-visible change → add a line to `CHANGELOG.md`.

## 4. Recipes

### Add a UI language
1. Copy `apps/desktop/src/i18n/locales/en/` to `apps/desktop/src/i18n/locales/<code>/` (e.g. `uk`, `sk`, `de`).
2. Translate the **values**, never the keys. Keep `{{placeholders}}` unchanged. Set `meta.languageName` to the language's own name ("Українська").
3. Run `pnpm test`. It checks that no key is missing and that the placeholders match.
4. The language appears in the picker automatically.

### Add an error code
1. Add one line to the `error_codes!` list in `crates/sr-core/src/error.rs`, inside the right range.
2. Add a `## SR-xxxx` section to `docs/ERRORS.md`: meaning, causes, what to do.
3. Add `errors.SR-xxxx` to `apps/desktop/src/i18n/locales/en/common.json`.
4. Run `cargo test --workspace` (regenerates bindings) and `pnpm test`. Both fail until all three steps are done.

### Add an IPC command
1. Put the logic in a library crate. Put any new data types in `crates/sr-ipc` with `#[derive(TS)] #[ts(export)]`.
2. Add a thin handler in `apps/desktop/src-tauri/src/commands/<area>.rs` and register it in `src/lib.rs`.
3. Add a typed wrapper in `apps/desktop/src/shared/ipc/commands.ts`.
4. Add a row to the IPC map in `PROJECT_MAP.md`.

### Record an architecture decision
Copy `docs/adr/0000-template.md` to `docs/adr/NNNN-short-title.md` and fill it in.
