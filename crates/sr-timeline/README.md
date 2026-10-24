# sr-timeline

Pure maths: a scenario plan plus measured block positions become timed segments (scroll / hold) and a scroll position for every frame.
Blocks with `hold_ms: 0` are waypoints: consecutive moves through them merge into one smooth move.

| File | What it holds |
|---|---|
| `src/lib.rs` | `PageMetrics` (`align_y`, `anchor_y`), `Timeline::build`, `frames()` |

```sh
cargo test -p sr-timeline
```
