# sr-diagnostics

Logging setup shared by the desktop app and the CLI. Phase 6 adds the "Report a problem" bundle here.

| File | What it holds |
|---|---|
| `src/lib.rs` | `init_logging`, `LogOptions`, `LogGuard`; log level via `SR_LOG` env var |

Log files: `scrollrecord.YYYY-MM-DD.log`, daily rotation, last 14 days kept.

```sh
cargo test -p sr-diagnostics
```
