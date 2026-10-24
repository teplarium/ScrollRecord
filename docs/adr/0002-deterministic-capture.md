# 0002 — Deterministic frame-by-frame capture

- **Status:** accepted
- **Date:** 2026-10-24
- **Related:** HANDOFF §7.3, NFR-03 (determinism), NFR-04 (no dropped frames), D-19, D-20

## Context
Screen recording in real time drops or repeats frames whenever the browser can't draw 60 frames per second, which is common on long pages, at 4K and on slow machines. ScrollRecord must produce perfectly smooth videos on any machine, and the same scenario must always give the same video. That also makes golden-image regression tests possible.

## Options considered
1. **CDP screencast (real time).** Simple, but frames depend on machine speed. Kept for previews only (Phase 3).
2. **`HeadlessExperimental.beginFrame`** (compositor-level frame control). Fully deterministic, but only works with `chrome-headless-shell`, not with regular Chrome or Edge, and not on macOS. It would force a separate browser download.
3. **Virtual time in the page + screenshot per frame** (approach of timesnap/timecut, extended). Works with any installed Chrome/Edge/Chromium on every OS.

## Decision
Option 3, implemented in `crates/sr-browser/js/runtime.js` plus two CDP settings:

- **Frozen page clock.** `setTimeout`/`setInterval`, `requestAnimationFrame`, `performance.now` and `Date` are replaced before any page script runs. Page time starts at 0 and moves only when ScrollRecord advances it by exactly `1000/fps` ms per frame.
- **CSS & Web animations.** Every animation is paused the first time it appears, then positioned explicitly (`currentTime = page time since it appeared`). Animations that the site itself paused are left alone.
- **Browser animation clock at speed 0** (`Animation.setPlaybackRate(0)`, re-applied after each navigation). Without it, a transition the page starts between frames (e.g. a scroll-reveal fired during load) plays in real time and may finish before ScrollRecord sees it. On a busy machine the first frames then differed between runs. Found by the e2e test under load.
- **IntersectionObserver settle.** "Element entered the viewport" callbacks are computed by the browser in real time. If the page created observers, each frame waits two real rendering steps so the callbacks run before the screenshot. Pages without observers skip the wait.
- **Real scrolling.** `window.scrollTo` per frame (instant), plus a synthetic `scroll` event, so sticky headers, scroll-linked effects and lazy content behave like for a real visitor.
- **Own minimal CDP client** (`cdp.rs`, synchronous, ~150 lines) instead of a large async library: about a dozen commands are needed, and small explicit code is easier to maintain for this project (HANDOFF D-19).

## Results (fixture 01, this ADR's date)
- 0 repeated frames during scroll segments (desktop 1080p60, iPhone 9:16 1080p60).
- Two renders are byte-identical: desktop 519/519 frames, mobile 577/577. The e2e test passes repeatedly with 3 renders running in parallel.
- Cost: about 5–9 rendered frames per second at 1080p on the build machine. The observer settle adds roughly 30 % on pages that use IntersectionObserver.

## Consequences / known limits
- `<video>`, canvas/WebGL driven by their own clocks, and Web Workers keep real time. Planned: fixture `06-media`, "freeze or hide media" option.
- Scroll-hijacking libraries (Lenis, Locomotive) and GSAP ScrollTrigger are not verified yet. Planned: fixtures 03–04, adapters if needed.
- Iframes get the runtime too (document-start script), but this is untested.
- Revisit option 2 if a future need justifies shipping `chrome-headless-shell`.
