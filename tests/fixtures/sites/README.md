# Test sites

Small, self-contained pages used to check that capture works (HANDOFF §7.3, Phase 1).
Each folder is a static site. ScrollRecord serves it through its built-in local server, exactly like a user's own site folder.

| Folder | Status | What it exercises |
|---|---|---|
| `01-plain-landing/` | ✅ verified | Plain HTML/CSS/JS (the owner's typical stack): `data-sr` blocks, looping CSS keyframe animation, scroll-reveal via IntersectionObserver with staggered delays, hover states, sticky header, burger menu on mobile. Smooth and byte-identical across renders (ADR 0002) |
| `02-css-transitions/` | ⏳ Phase 1b | Long transitions, `animation-delay`, `prefers-reduced-motion` |
| `03-gsap-scrolltrigger/` | ⏳ Phase 1b | Scroll-driven timelines, pinning |
| `04-lenis-smooth-scroll/` | ⏳ Phase 1b | Scroll hijacking library |
| `05-react-spa/` | ⏳ Phase 1b | Client-side routing, hydration (framework sites, HANDOFF D-15) |
| `06-media/` | ⏳ Phase 1b | `<video>`, Lottie, canvas |

Rules: no external network requests (fonts, CDNs). Everything must be inside the folder so tests run offline and give the same result every time.
