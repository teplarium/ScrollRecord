# ScrollRecord

**Smooth website scroll videos, from desktop to mobile.**

ScrollRecord is a free, open-source desktop app that renders frame-perfect scroll videos of websites for any device, with a visual cursor that shows hovers and clicks. You plan the video on a FigJam-like canvas: blocks of the page as cards, arrows for the order, start and end points. It can also record a single block's animation, cropped to that block, even with a transparent background. Scenarios are plain JSON, so AI models can write them for you using the skill you download from the app.

> **Status: Phase 0 — foundation.** The app starts and its architecture is in place, but it doesn't record anything yet. Recording arrives in Phase 1. See the [roadmap](HANDOFF.md#14-дорожная-карта).

## Planned features

- Deterministic, stutter-free 30/60 fps rendering of real scrolling (scroll animations, sticky headers and lazy-loading all work)
- Device presets from 1920 px desktops to phones, with one scenario for all of them
- Output in 16:9, 9:16, 1:1, 4:5, 4:3, 3:4, 2:3, 21:9 or native size; MP4, WebM, PNG sequence, ProRes
- Human-like cursor with hover/click effects, touch indicators on mobile, and custom cursor packs (SVG)
- **Block clip**: record one section's animation, time-based or scroll-driven, cropped to the block
- Multi-page navigation (menus, burger menus, SPA routes)
- AI skills: let Claude, Gemini or a local model write scenarios and cursor packs
- Windows and Linux first, macOS later. UI in English first, with more languages planned

## Download

No releases yet. Every successful CI run attaches test installers for Windows (`.exe`) and Linux (`.deb`, `.AppImage`) under **Actions → CI → Artifacts**.

## Build from source

Prerequisites, and what each one is for, are listed in [CONTRIBUTING.md](CONTRIBUTING.md#1-set-up-your-machine).

```sh
pnpm install
pnpm dev            # run the desktop app with hot reload
pnpm check          # UI lint + types + tests
cargo test --workspace
```

## Documentation

| Document | For |
|---|---|
| [PROJECT_MAP.md](PROJECT_MAP.md) | Where everything lives (start here) |
| [HANDOFF.md](HANDOFF.md) | Full requirements, decisions and roadmap (Russian) |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Setup, workflow, adding a language or an error code |
| [AGENTS.md](AGENTS.md) | Rules for AI coding assistants |
| [docs/ERRORS.md](docs/ERRORS.md) | What each `SR-xxxx` error code means and how to fix it |
| [docs/adr/](docs/adr/) | Why the architecture is the way it is |

## License

Dual-licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
Unless you explicitly state otherwise, any contribution you intentionally submit for inclusion in this project, as defined in the Apache-2.0 license, is dual-licensed as above, without any additional terms or conditions.
