# Example scenarios

| File | What it shows |
|---|---|
| `01-plain-landing.srflow.json` | The fixture landing page (`tests/fixtures/sites/01-plain-landing`) on desktop (16:9) and iPhone (9:16): holds, a pass-through block, `align: center`, end anchor, a device override. Cursor: real hover on *Get started* (desktop only), hover on the Pro plan, a visual click on *Choose Pro* (a tap on the iPhone), cursor fading out on the last scroll |

Render (from the repository root):

```sh
cargo run --release -p sr-cli -- render examples/01-plain-landing.srflow.json
# → examples/renders/01-plain-landing_desktop.mp4, …_mobile.mp4 (+ .timeline.json)
cargo run --release -p sr-cli -- validate examples/01-plain-landing.srflow.json
```

Needs Chrome, Edge or Chromium, and `ffmpeg` for MP4. Set `"format": "png"` on a target to get PNG frames without ffmpeg.

## Cursor actions in a block

```json
"actions": [
  { "do": "hover", "target": [{ "text": "Get started" }], "duration_ms": 900, "only": ["desktop"] },
  { "do": "click", "target": [{ "text": "Choose Pro" }], "delay_ms": 150 },
  { "do": "move", "to": { "x": 0.8, "y": 0.5 } },
  { "do": "wait", "duration_ms": 400 }
]
```

- `target` is searched **inside the block first**, so `{ "text": "Start" }` means this block's Start button.
- `hover` is `real` by default (the site reacts); `click` is `visual` by default (the press is only shown; nothing is clicked). Set `"mode"` to change it.
- `only` / `except`: target ids or device presets. On touch devices `hover` and `move` are skipped and clicks become taps.
- A block with actions always stops, even with `hold_ms: 0`; the stop lasts as long as the hold or the actions, whichever is longer.
- Scenario-wide look: `"defaults": { "cursor": { "pack": "classic-dark", "scale": 1, "seed": 7, "speed": 1 } }`. Packs: `classic-dark`, `classic-light`, or a folder with `cursor-pack.json` (format: HANDOFF §8.3).
- While scrolling along an arrow: `"cursor": "still"` (default), `"follow"` (moves with the page) or `"hide"` (fades out), set on the edge.
- `timeline.json` next to each video records where the cursor went and every click.
