# 0002 — Deterministic frame-by-frame capture

- **Status:** accepted; animation control and the known limits amended by [0004](0004-heavy-sites.md)
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
- **CSS & Web animations.** ~~Every animation is paused the first time it appears, then positioned explicitly~~ — replaced in [0004](0004-heavy-sites.md): running animations are moved forward by each frame's time and never paused by ScrollRecord, so the site's own play-state and end events keep working.
- **Browser animation clock at speed 0** (`Animation.setPlaybackRate(0)`, re-applied after each navigation). Without it, a transition the page starts between frames (e.g. a scroll-reveal fired during load) plays in real time and may finish before ScrollRecord sees it. On a busy machine the first frames then differed between runs. Found by the e2e test under load.
- **IntersectionObserver delivery.** The browser computes "element entered the viewport" during real-time rendering and normally calls the page back in a task whose timing depends on machine load. Waiting for it was not enough: in 1 of about 7 runs a reveal started one frame later. ScrollRecord therefore wraps `IntersectionObserver`, waits two real rendering steps after scrolling (intersections are computed by then), takes the pending records with `takeRecords()` and calls the page's callback itself at that fixed point. Pages without observers skip all of this.
- **Real scrolling.** `window.scrollTo` per frame (instant), plus a synthetic `scroll` event, so sticky headers, scroll-linked effects and lazy content behave like for a real visitor.
- **Own minimal CDP client** (`cdp.rs`, synchronous, ~150 lines) instead of a large async library: about a dozen commands are needed, and small explicit code is easier to maintain for this project (HANDOFF D-19).

## Results (fixture 01, this ADR's date)
- 0 repeated frames during scroll segments (desktop 1080p60, iPhone 9:16 1080p60).
- Renders are byte-identical: desktop 519/519 frames, mobile 577/577. Six mobile renders, three at a time under heavy load, gave identical frames. Encoding identical frames gives byte-identical MP4s.
- How the last race was found: two MP4s of the same scenario differed. Frame comparison showed one reveal card one frame behind, and the encoder's later decisions made every following frame differ slightly. Lesson: compare PNG frames, not MP4 files, when checking determinism.
- Cost: about 5–9 rendered frames per second at 1080p on the build machine. The observer settle adds roughly 30 % on pages that use IntersectionObserver.

## Consequences / known limits
- Phase 1b (ADR [0004](0004-heavy-sites.md)) verified GSAP + ScrollTrigger, Lenis, a React app, `<video>`, Lottie, canvas/WebGL and animated GIFs, and lists what is still not handled (iframes, Web Workers, transform-based smooth scrolling, CSS background GIFs).
- Revisit option 2 if a future need justifies shipping `chrome-headless-shell`.
