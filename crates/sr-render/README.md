# sr-render

The whole pipeline in one place, used by the CLI now and by the desktop app later:
plan → serve site → browser → emulate → load & prepare → measure → timeline → capture → encode → `timeline.json`.

| File | What it holds |
|---|---|
| `src/lib.rs` | `Renderer::start / render / finish`, `RenderOptions`, `RenderReport`, `output_path` |
| `tests/render_fixture.rs` | End-to-end test on fixture 01 incl. determinism (needs a browser, skipped otherwise) |

```sh
cargo test -p sr-render
```
