# sr-core

The foundation crate: domain types, the `SR-xxxx` error-code catalog and validation rules.
Pure data and pure functions — **no IO, no async, no browser, no UI**.

| File | What it holds |
|---|---|
| `src/lib.rs` | Crate overview, re-exports, `CORE_VERSION` |
| `src/error.rs` | `ErrorCode` (single-source list), `AppError`, `AppResult` |

Planned (Phase 1+): `flow/` (scenario model), `target.rs`, `cursor_pack.rs`, `validate/`, `migrate/`.

```sh
cargo test -p sr-core   # also regenerates TypeScript bindings for the UI
```
