# Third-party files (not covered by this repository's licence)

| File | Library | Version | Licence |
|---|---|---|---|
| `lottie_light.min.js` | [lottie-web](https://github.com/airbnb/lottie-web) (light build, SVG renderer) | 5.13.0 | MIT — `LICENSE-lottie-web.md` |

Copied unchanged from the `lottie-web` npm package (`build/player/`).
Used only as a test page for ScrollRecord's capture engine (so tests run offline).
To update: `npm pack lottie-web@<version>` and copy `package/build/player/lottie_light.min.js`.

The files in `../media/` were made for this fixture (no third-party content):
`counter.webm` with ffmpeg (`geq` stripes = frame number, VP9), `blink.gif` with Pillow, `move.json` by hand.
