## What and why
<!-- Plain language: what changes for the user, and why. Link the issue (Fixes #123) and HANDOFF IDs (FR-…, D-…). -->

## How it was tested
<!-- Commands run, tests added, screenshots for UI changes. -->

## Checklist
- [ ] CI is green (Rust + UI on Windows and Linux)
- [ ] Bug fix: a test that failed before the fix is included
- [ ] Comments explain *why* / *what it solves* for each new block of code
- [ ] `PROJECT_MAP.md` updated (if files, types or IPC commands changed)
- [ ] `CHANGELOG.md` updated (if users will notice the change)
- [ ] New error codes: `error.rs` + `docs/ERRORS.md` + `en/common.json`
- [ ] No visible text hard-coded in JSX
