# Test sites

Small, self-contained pages used to check that capture works (HANDOFF §7.3, Phase 1).
Each folder is a static site. ScrollRecord serves it through its built-in local server, exactly like a user's own site folder.

| Folder | Status | What it exercises |
|---|---|---|
| `01-plain-landing/` | ✅ verified | Plain HTML/CSS/JS (the owner's typical stack): `data-sr` blocks, looping CSS keyframe animation, scroll-reveal via IntersectionObserver with staggered delays, hover states, sticky header, burger menu on mobile. Smooth and byte-identical across renders (ADR 0002) |
| `02-css-transitions/` | ✅ verified | Staggered `animation-delay`, a 2 s transition with delay started from JS, effects chained on `animationend` → `transitionend` → Web Animations `finished`, the "paused until in view" reveal (`animation-play-state`), native scroll-driven animation (`animation-timeline: view()`), `prefers-reduced-motion` / `prefers-color-scheme` markers |
| `03-gsap-scrolltrigger/` | ✅ verified | GSAP 3.15 intro timeline, ScrollTrigger `scrub: true` and `scrub: 0.5`, a pinned section with horizontal movement (pin spacer +1200 px), scroll-started reveal tweens |
| `04-lenis-smooth-scroll/` | ✅ verified | Lenis 1.3 driven by GSAP's ticker (official recipe), velocity "skew" effect, hide-header-on-scroll-down (direction), ScrollTrigger parallax. Six scripts/styles: also the page-load regression test for the site server |
| `05-react-spa/` | ✅ verified | React 19 app rendered in the browser: entrance from `useEffect`, count-up re-rendered on every frame, content from `fetch()`, tabs (real click → state), client-side route `/pricing` (deep link via the server's SPA fallback). Source in `src/`, built `app.js` committed (how to rebuild: `README.md` in that folder) |
| `06-media/` | ✅ verified | Muted autoplay loop `<video>` and a "play when visible" one (`media/counter.webm` shows its own frame number as stripes), lottie-web player, 2D canvas and WebGL redrawn per frame, animated GIF (4 colours × 250 ms) |

Tests: `crates/sr-render/tests/render_fixture.rs` (01) and `crates/sr-render/tests/heavy_sites.rs` (02–06); why and what was found: `docs/adr/0002-…` and `docs/adr/0004-heavy-sites.md`.

Rules: no external network requests (fonts, CDNs). Everything must be inside the folder so tests run offline and give the same result every time.
Third-party libraries live in each fixture's `vendor/` folder with a `README.md` naming their licence (GSAP keeps its own free licence; it is not covered by this repository's MIT/Apache licence).
