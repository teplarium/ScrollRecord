# AGENTS.md — rules for AI coding assistants

This file applies to every AI assistant working on ScrollRecord (Claude, Gemini, Codex, local models).
The project owner is **not a programmer**. Your code is checked by CI, by tests and by user reports, not by a human code reviewer. Be careful and explicit.

## 1. Before you touch code

1. Read [`PROJECT_MAP.md`](PROJECT_MAP.md). It tells you which files matter. **Do not scan the whole repository.**
2. For requirements or "why" questions, read the relevant section of [`HANDOFF.md`](HANDOFF.md) (Russian). Requirement IDs look like `FR-CUR-05`, decisions like `D-12`.
3. If a task belongs to a later roadmap phase (HANDOFF §14), say so before building it.

## 2. Comment standard (mandatory)

Comment **blocks of meaning**, not single lines. Each comment answers **why it exists** and **what problem it solves**. Write all comments in English.

- **File/module header** (`//!` in Rust, `/** */` at the top of TS files): purpose, what it solves, rules/limits, related files.
- **Public API** (`///` / JSDoc): what it does and which errors it can return.
- **Sections inside long functions**: `// ── Title ──` followed by 1–3 lines of why/solves.
- Don't narrate obvious code (`// increment i`).

See `crates/sr-core/src/error.rs` and `apps/desktop/src/shared/ipc/commands.ts` for reference style.

## 3. Hard rules

| Rule | Why |
|---|---|
| No `unwrap()`, `expect()` or `panic!` outside tests (clippy denies them) | A crash loses the user's work; return `AppError` instead |
| Every error has an `ErrorCode`. New code = one line in the `error_codes!` list in `crates/sr-core/src/error.rs` + a `## SR-xxxx` section in `docs/ERRORS.md` + `errors.SR-xxxx` in `apps/desktop/src/i18n/locales/en/common.json` | Users report codes; tests fail if any of the three is missing |
| No visible text in JSX. Use `t("key")` and add the key to `en/common.json` | UI must be translatable (ESLint enforces it) |
| Never edit `apps/desktop/src/shared/ipc/generated/` or `schema/` by hand | Generated from Rust by `cargo test` |
| The UI calls Rust only via `apps/desktop/src/shared/ipc/commands.ts` | One place to fix when a command changes |
| Tauri command handlers contain no business logic | Logic belongs in a library crate under `crates/` |
| Respect dependency rules (PROJECT_MAP §7) | Keeps modules independently fixable |
| **Every bug fix adds a test that failed before the fix** | No human reviewer; tests are the safety net |
| Update `PROJECT_MAP.md` in the same change when structure changes | Otherwise the map lies to the next assistant |
| Add a line to `CHANGELOG.md` (plain language, under *Unreleased*) | The owner reads the changelog, not the diff |
| Don't add a dependency without saying why in the PR | Every dependency is a long-term maintenance cost |

## 4. Before you say "done"

Run from the repository root and make sure everything passes:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace          # also regenerates TS bindings: commit them if they changed
pnpm check                      # lint + format check + typecheck + UI tests
```

Rendering tests need Chrome, Edge or Chromium (set `SR_BROWSER=<path>` if it isn't found automatically). Without one they print "skipped" and pass, so say so in your report: skipped is not tested.

Then report in plain language: what changed, what was tested, what is still missing.

## 5. Working from a bug report

1. Find the `SR-xxxx` code in the report, then its section in `docs/ERRORS.md`, then search the code for the variant name.
2. Reproduce it with the CLI if possible (`scrollrecord …`). Add a fixture under `tests/fixtures/` if a site is involved.
3. Write the failing test first, then fix.
