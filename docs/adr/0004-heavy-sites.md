# 0004 — Capture on heavy sites (Phase 1b)

- **Status:** accepted (amends 0002)
- **Date:** 2026-10-25
- **Related:** HANDOFF §14 Phase 1b, D-29 … D-34, risks 1–3 and 12; NFR-03 (determinism), NFR-04 (no dropped frames)

## Context
ADR 0002 proved frame-by-frame capture on a plain HTML/CSS/JS page. Phase 1b checks it on the kinds of sites that usually break such capture. Five test sites were added (`tests/fixtures/sites/02…06`), each with a behaviour test and a render-twice test (`crates/sr-render/tests/heavy_sites.rs`).

What they found:

| Site | Broken before Phase 1b |
|---|---|
| 02 CSS | "Paused until in view" reveals (`animation-play-state: paused → running`) stayed invisible forever; `animation-timeline: view()` froze; chains on the Web Animations `finished` promise never continued |
| 03 GSAP + ScrollTrigger | Nothing: GSAP's ticker already ran on page time; pin spacers are in place before measuring |
| 04 Lenis | Velocity/direction effects read 0 (every position reached the page twice); the page sometimes never finished loading (local server, see below) |
| 05 React SPA | Deep links (`/pricing`) gave 404; React's own tasks ran in real time (no failure seen, but timing was not guaranteed) |
| 06 Media | `<video>` and animated GIFs played in real time: different frames on every render |

## Decision

**Animations (replaces 0002's "pause & seek").** The browser's animation clock stays at speed 0. Every frame, each *running* animation on the document timeline is moved forward by exactly the frame's time (`currentTime += dt × playbackRate`). New animations start at the frame's time (currentTime 0). ScrollRecord never calls `pause()`/`play()` itself, so CSS `animation-play-state` and the site's own pause/play keep working, and animations really finish: `animationend`/`transitionend` fire and `finished`/`onfinish` resolve. Scroll-driven animations (`animation-timeline`) are left to the browser; they follow the scroll position. Checked: no animation moves between frames on its own (1.5 s real wait, identical pixels).

**Scroll events.** `frame()` announces a new scroll position itself (so handlers run before the frame's animation callbacks), only when the position changed. The browser's own `scroll` event for the same position is stopped in a capture listener registered before any page script. The page hears each position exactly once, like in a real browser.

**Framework tasks and idle work.** `MessagePort.postMessage` is counted; when there were posts, `frame()` waits until the chain of message tasks has run (its own message queued behind them, at most 20 rounds), so React re-renders land in the frame that caused them. `requestIdleCallback` runs once per frame after animation-frame callbacks, with a deterministic `timeRemaining()`. Timers nested more than 5 levels get the browsers' 4 ms minimum, so `setTimeout(loop, 0)` cannot spin inside a frame.

**Video and audio.** One virtual "media time" per element, computed from page time. The real element never plays: it stays paused and visible videos are seeked to the media time before each frame (then `seeked` is awaited and re-checked). The `autoplay` attribute is taken over by a `MutationObserver` as soon as an element appears, before Chrome could start it (it autoplays only once data has arrived). Reason: a video Chrome had played and that was paused afterwards sometimes kept showing an old picture after a seek (1 of 120 under load); a never-played video did not (0 of 240). To the site the element still behaves like a playing video: `paused`, `ended`, `currentTime`, `playbackRate`, `autoplay` report the virtual state; play / playing / pause / ended events are dispatched at the frame where they happen; the browser's own play/playing/pause events of managed elements are hidden.

**Animated images.** GIF and animated WebP/PNG/AVIF in `<img>` are decoded once with `ImageDecoder` (built into Chrome/Edge); each frame becomes a still PNG (blob URL) and the `<img>` shows the frame that belongs to page time, counted from when the image is first on screen.

**Browser settings.** `Emulation.setEmulatedMedia`: `prefers-reduced-motion: no-preference`, `prefers-color-scheme: light`, so the computer's settings never change the video. `--autoplay-policy=no-user-gesture-required`.

**Browser drawing without a GPU (added 2026-10-25, after CI on Windows).** `--disallow-non-exact-resource-reuse` is passed to the browser (`sr-browser/src/process.rs`). What it was: `gsap_fixture_renders_identically` failed once on the Windows CI runner with frames 25–31 different — exactly the frames in which a reveal card is mid-tween. The page state (scroll, GSAP time, every card's opacity/transform) was identical in both renders; only one pixel row at the bottom or right edge of a moving card differed. Cause, found in Chromium's `cc` sources and reproduced here:
- On machines without a GPU (CI runners, VMs, remote desktops) Chrome rasterises tiles in software and keeps released tile memory in a pool (`cc::ResourcePool`) for reuse.
- A composited layer with text on an opaque background at a fractional position (GSAP `translate3d(0, 8.8889px, 0)`) is rastered with a fractional "raster translation". Chrome clears the partially covered top/left texels before drawing, but not the bottom/right ones (`CalculateClearForOpaqueRasterRects` only checks scale for those). Whatever the reused memory held shows through in that row.
- By default the pool may hand out any *larger* unused tile (up to 2× the area), from any layer. Which one is free at that moment depends on raster-thread timing and on idle clean-up timers, so the row differed between renders. Reproduced on a 12-thread Windows PC in software mode (`--disable-gpu`), fixture 03 rendered like the renderer does: with the test pinned to 2 CPUs next to 4 busy processes, frames 26–31 differed in 4 of 7 comparisons; with a 7 s pause between frames (unused tiles are freed after 5 s, `kDefaultExpirationDelay`) frames 25–31 differed every time. With the flag: 0 of 11 under the same load. Like the CI runner (all heavy tests, 4 test threads on 4 logical CPUs, `--disable-gpu`): without the flag 3 of 6 runs had different renders, one with exactly CI's message (`03-gsap-scrolltrigger: renders differ in frames [25, …, 31]`), one in fixture 02; with the flag 0 of 8.
- With exact-size reuse only, a new tile can get only memory of its own size (in practice, the same kind of element's earlier tile), which is the same on every render. Regression test: `software_frames_do_not_depend_on_leftover_tile_memory` in `crates/sr-browser/tests/browser.rs` (fails without the flag 3 of 3, passes with it). Android Chrome always runs with this setting, so it is a supported mode; the cost is a few more tile allocations.
- Checked and *not* helping: `--disable-partial-raster`, `--disable-lcd-text`, `--run-all-compositor-stages-before-draw`, `--num-raster-threads=1`, `--enable-zero-copy`, disabling the idle tile-reclaim features. With a GPU (this PC: NVIDIA) the problem never appeared.

**Local site server.** Own minimal server on the standard library instead of tiny_http: one thread per connection, one request per connection (`Connection: close`), read timeout, byte ranges (`206`, needed to seek videos), clean URLs (`/pricing` → `pricing.html`) and single-page-app fallback (unknown page address with `Accept: text/html` → `/index.html`). Reason: with tiny_http 0.12 a page loading six files at once often never got one of them (7 of 8 loads of fixture 04), and the render failed with SR-3004.

**Third-party libraries in test sites** are copied into each fixture's `vendor/` with a licence note (GSAP under its own free "Standard No Charge" licence; Lenis, lottie-web and React under MIT), so tests run offline.

## Results (2026-10-25, 800×450, 10 fps, 2-core machine)
- Every fixture renders byte-identically twice; every frame of a scroll differs from the previous one. Four runs of all browser tests in a row passed.
- Exact timing checks: a delayed transition at 45 % at t = 1500 ms; video frame = page time × 30 fps (also right after `load()`); GIF colour every 250 ms; Lottie and canvas follow animation-frame timestamps; React count-up +100 per 100 ms frame; scrub at scroll 300 = 300/1050.

## Consequences / known limits
- More of the page's API is replaced (media elements, `MessagePort.postMessage`, `requestIdleCallback`). Each replacement is documented in `runtime.js` with why it exists.
- Seeking a video every frame decodes from the previous keyframe: long videos with rare keyframes render slowly (correctly, just slower). Only on-screen videos are seeked.
- Not handled yet (documented, not verified): animated GIFs as CSS backgrounds or in `<picture><source>`; cross-origin animated images without CORS; iframes (e.g. YouTube embeds) and Web Workers keep real time; SVG SMIL animations; `scheduler.postTask`.
- Not supported yet: sites that scroll inside an element instead of the window, and transform-based smooth scrolling (Locomotive Scroll v4, GSAP ScrollSmoother) — Phase 7 adapters.
- Without a GPU, if one frame takes longer than about 2.5 s to render (5 s between a tile being freed and the next one of the same size being needed), Chrome may have dropped the freed tile, and the bottom/right pixel row of an element with text that moves by fractions of a pixel can still differ by a few colour levels between renders. Not visible in a video; cannot be fixed from outside Chrome (the pool's 5 s timer has no switch). Renders with a GPU are not affected.
- In software mode the screenshot of such a frame can contain semi-transparent pixels in that row (PNG with alpha). `sr-compositor` premultiplies them, so in the video that one row is slightly darker. Same on every render; with a GPU it does not happen.
- The site server serves `index.html` for unknown page addresses: a typo in `pages[].url` shows the home page (blocks are then usually "not found", SR-2101); the server log says "single-page app fallback".
