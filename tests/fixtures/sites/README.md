# Test sites

Small, self-contained pages used to check that capture works (HANDOFF §7.3, Phase 1).
Each folder is a static site. ScrollRecord serves it through its built-in local server, exactly like a user's own site folder.

| Folder | Status | What it exercises |
|---|---|---|
| `01-plain-landing/` | ✅ | Plain HTML/CSS/JS (the owner's typical stack): `data-sr` blocks, CSS keyframe animation, scroll-reveal via IntersectionObserver, hover states, sticky header, burger menu on mobile |
| `02-css-transitions/` | ⏳ Phase 1 | Long transitions, `animation-delay`, `prefers-reduced-motion` |
| `03-gsap-scrolltrigger/` | ⏳ Phase 1 | Scroll-driven timelines, pinning |
| `04-lenis-smooth-scroll/` | ⏳ Phase 1 | Scroll hijacking library |
| `05-react-spa/` | ⏳ Phase 1 | Client-side routing, hydration (framework sites, HANDOFF D-15) |
| `06-media/` | ⏳ Phase 1 | `<video>`, Lottie, canvas |

Rules: no external network requests (fonts, CDNs). Everything must be inside the folder so tests run offline and give the same result every time.
