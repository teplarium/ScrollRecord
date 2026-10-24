# Example scenarios

| File | What it shows |
|---|---|
| `01-plain-landing.srflow.json` | The fixture landing page (`tests/fixtures/sites/01-plain-landing`) on desktop (16:9) and iPhone (9:16): holds, a pass-through block, `align: center`, end anchor, a device override |

Render (from the repository root):

```sh
cargo run --release -p sr-cli -- render examples/01-plain-landing.srflow.json
# → examples/renders/01-plain-landing_desktop.mp4, …_mobile.mp4 (+ .timeline.json)
cargo run --release -p sr-cli -- validate examples/01-plain-landing.srflow.json
```

Needs Chrome, Edge or Chromium, and `ffmpeg` for MP4. Set `"format": "png"` on a target to get PNG frames without ffmpeg.
