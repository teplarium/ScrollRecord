# sr-ipc

The IPC contract between the desktop shell (Rust) and the UI (TypeScript).
Plain data types only; each one is exported to `apps/desktop/src/shared/ipc/generated/` by `ts-rs`.

| File | What it holds |
|---|---|
| `src/lib.rs` | Crate overview, re-exports `AppError` / `ErrorCode` from `sr-core` |
| `src/app_info.rs` | `AppInfo` — versions and environment for the footer and bug reports |

```sh
cargo test -p sr-ipc   # regenerates the TypeScript bindings
```
