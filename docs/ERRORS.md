# Error catalog

Every error ScrollRecord reports carries a stable code like `SR-0101`.
If you see one, search this page for it. Each entry explains what it means, the usual causes and what to do.
If the fix doesn't help, please [open an issue](https://github.com/teplarium/ScrollRecord/issues/new/choose) and include the code.

**For contributors:** codes are declared once in `crates/sr-core/src/error.rs`. A test fails if a code has no `## SR-xxxx` section here, and another fails if it has no English UI text in `apps/desktop/src/i18n/locales/en/common.json`.

| Range | Area |
|---|---|
| `SR-00xx` | App / general |
| `SR-01xx` | Diagnostics & logging |
| `SR-1xxx` | Projects & files |
| `SR-2xxx` | Flow format & validation |
| `SR-3xxx` | Browser |
| `SR-4xxx` | Capture |
| `SR-5xxx` | Encoder |
| `SR-6xxx` | Cursor |
| `SR-7xxx` | AI & skills |
| `SR-8xxx` | Site sources |

---

## SR-0001

**Unexpected internal error.** The app reached a state its code did not expect. This is always a bug.

- **What to do:** open an issue with the steps that led to it. Once "Report a problem" ships (Phase 6), attach its zip; until then, attach the latest log file (see `PROJECT_MAP.md` → *Runtime data locations*).
- **For developers:** search the logs for `SR-0001`. The `message` field says where it came from.

## SR-0002

**Feature not implemented yet.** You used something that is planned but not built yet. The message names the feature and the roadmap phase (see `HANDOFF.md` §14).

- **What to do:** nothing is broken. Wait for that phase, or follow progress on GitHub.

## SR-0003

**Desktop app not detected.** The interface is running outside the ScrollRecord desktop app, usually because it was opened in a normal web browser during development (`pnpm dev` without Tauri). The interface can't reach the Rust core there.

- **What to do:** start it with `pnpm tauri dev` (developers), or use the installed app.

## SR-0101

**Logging could not start.** The log folder or log file could not be created, or logging was started twice. The app keeps running, but without log files, so bug reports will lack detail.

- **Common causes:** no write permission for the app-data folder; disk full; antivirus blocking the folder.
- **What to do:** check free disk space and folder permissions. The `path` parameter shows which folder failed.
- **For developers:** `reason = already-initialised` means `init_logging` was called twice in one process.
