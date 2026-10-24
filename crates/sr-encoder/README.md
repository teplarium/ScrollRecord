# sr-encoder

Frame sinks for the output formats: MP4 (H.264) and WebM (VP9) via an external `ffmpeg` (frames piped to stdin, BT.709 colour tagging), or a PNG folder that needs no ffmpeg.

| File | What it holds |
|---|---|
| `src/lib.rs` | `open_sink`, `find_ffmpeg` (explicit → `SR_FFMPEG` → `PATH`), `FfmpegEncoder`, `PngSequence`, `ffmpeg_args` |

```sh
cargo test -p sr-encoder
```
