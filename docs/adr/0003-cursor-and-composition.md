# ADR 0003 — Cursor and composition

**Status:** accepted (2026-10-25) · HANDOFF D-24…D-28

- The cursor is drawn by ScrollRecord (`sr-compositor`) on top of each screenshot, from SVG cursor packs (`sr-cursor`, resvg). Frames go to ffmpeg as raw RGBA. Sharp at any resolution, independent of site CSS.
- `real` hover: the mouse position is re-sent to the browser on every "live" frame after scrolling. A scroll alone updates `:hover` only on the browser's next real frame (timing-dependent); re-sending makes renders byte-identical. Leaving live mode moves the mouse off the page.
- `visual` click: the page sees the pointer moving (hover styles) but no press. Clicks default to visual so forms and links are never triggered by accident.
- Probe: before recording, the page scrolls to each block with actions, lets reveal effects finish (`__sr.settleAt`), measures the targets there, then the page is reloaded.
- Motion (all randomness from `cursor.seed`): 250 ms reaction, Fitts's-law duration, curved Bézier path, ease-out landing, sometimes a few pixels of overshoot with a quick correction on long moves.
- Touch devices: hover/move skipped with a warning; clicks become taps (finger circle appears, presses, fades).
- Compositing + encoding run on a worker thread, overlapping with the browser.
