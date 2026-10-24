# Changelog

Written in plain language for users and the project owner. Newest first.
Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Versions follow [SemVer](https://semver.org/).

## [Unreleased]

### Added — Phase 1: first real videos
- `scrollrecord render scenario.srflow.json` records smooth scroll videos of a website: MP4 (H.264), WebM (VP9) or PNG frames.
- Every frame is drawn completely before it's saved, so videos never stutter, even on slow machines. Rendering the same scenario twice gives exactly the same frames.
- Sites can be a local folder (served automatically) or a URL such as `http://localhost:5173`.
- Device presets: desktop 1920, laptops 1440/1366, iPad, iPhone, Android. Any aspect ratio (16:9, 9:16, 1:1, 4:5, …) at 720p, 1080p, 1440p or 4K.
- Scenario format v1: start/end points attached to blocks, pauses per block, scrolling through blocks without stopping (`hold_ms: 0`), alignment, per-device overrides, page preparation (hide elements, load lazy images).
- Blocks are found by `data-sr="name"` attributes, CSS selectors, text or roles. If a block isn't found, the error says what was tried.
- Sticky headers are detected, so blocks land just below them.
- `scrollrecord validate` checks a scenario and shows what each device will produce.
- A `timeline.json` next to every video records what happened when (basis for the future editor).
- `scrollrecord info` now shows which browser and ffmpeg it will use.
- Example scenario and a longer test site in `examples/` and `tests/fixtures/`.

### Added — Phase 0: foundation
- The desktop app starts and shows a start screen (in English). Its buttons answer "planned for phase 3".
- The footer shows the app version, core version and platform, read from the Rust core.
- Language picker. Only English for now, and new languages are picked up automatically.
- Every error has a code (`SR-xxxx`) explained in `docs/ERRORS.md`.
- Log files are written to the app's data folder, and the last 14 days are kept.
- `scrollrecord` command-line tool with `info` and `errors`.
- Automatic checks (CI) on Windows and Linux, which also build test installers.
- Project map, rules for AI assistants, contribution guide and issue forms.
