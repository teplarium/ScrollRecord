# Third-party files (not covered by this repository's licence)

| File | Library | Version | Licence |
|---|---|---|---|
| `gsap.min.js` | [GSAP](https://gsap.com) | 3.15.0 | GreenSock Standard "No Charge" License — <https://gsap.com/standard-license> |
| `ScrollTrigger.min.js` | GSAP ScrollTrigger plugin | 3.15.0 | same as above |

Copied unchanged from the `gsap` npm package (`dist/`), with their copyright headers.
They are used **only** as a test page for ScrollRecord's capture engine (so tests run offline) and are not part of ScrollRecord itself.
GSAP is free to use but is not open-source licensed; it stays under its own licence, not under MIT/Apache-2.0.
To update: `npm pack gsap@<version>` and copy the two files from `package/dist/`.
