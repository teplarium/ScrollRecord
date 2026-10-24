# sr-capture

The frame loop: warm-up, then for each frame `step(scroll_y, 1/fps)` → `capture_png()` → `FrameSink::push`. Browser- and format-agnostic (tested with fakes).

| File | What it holds |
|---|---|
| `src/lib.rs` | `FrameSink` trait, `capture()`, `Progress`, `CaptureStats` |

```sh
cargo test -p sr-capture
```
