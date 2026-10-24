# Changelog

Written in plain language for users and the project owner. Newest first.
Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Versions follow [SemVer](https://semver.org/).

## [Unreleased]

### Added — Phase 0: foundation
- The desktop app starts and shows a start screen (in English). Its buttons answer "planned for phase 3".
- The footer shows the app version, core version and platform, read from the Rust core.
- Language picker. Only English for now, and new languages are picked up automatically.
- Every error has a code (`SR-xxxx`) explained in `docs/ERRORS.md`.
- Log files are written to the app's data folder, and the last 14 days are kept.
- `scrollrecord` command-line tool with `info` and `errors`.
- Automatic checks (CI) on Windows and Linux, which also build test installers.
- Project map, rules for AI assistants, contribution guide and issue forms.
