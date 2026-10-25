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
- The site server serves `index.html` for unknown page addresses: a typo in `pages[].url` shows the home page (blocks are then usually "not found", SR-2101); the server log says "single-page app fallback".
