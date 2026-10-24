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

## SR-1001

**A file could not be read.** Usually the scenario file passed to `scrollrecord render`.

- **Common causes:** wrong path or a typo in the file name; the file is open and locked by another program; no permission.
- **What to do:** check the path shown in the `path` parameter. Paths with spaces need quotes on the command line.

## SR-1002

**A file or folder could not be written.** Usually the output video or its folder.

- **Common causes:** the output folder doesn't exist and can't be created; disk full; the video is still open in a player (Windows locks open files); no permission.
- **What to do:** close any player showing the old video, check free space, or choose another output folder with `--out`.

## SR-2001

**The scenario file could not be parsed.** It's not valid JSON, or a field has the wrong name or type. The message includes the `line` and `column`.

- **Common causes:** a missing comma or bracket; a typo in a field name (e.g. `"hold_sm"`), since unknown fields are rejected on purpose; text where a number is expected.
- **What to do:** open the file at the given line. If an AI wrote it, paste this error back to the AI and ask it to fix the file.

## SR-2002

**The scenario is not valid.** The file parses, but something doesn't fit together. The message lists every problem, one per line.

- **Common causes:** no `start` or `end` node, or more than one of either; an arrow points to a node id that doesn't exist; the path from start doesn't reach the end; a block has no locator; an anchor points to a node that isn't a block.
- **What to do:** fix each listed line. Running `scrollrecord validate <file>` shows the same list without rendering.

## SR-2003

**Scenario format version not supported.** The file was written for a newer ScrollRecord.

- **What to do:** update ScrollRecord, or ask whoever made the file for a version-1 export.

## SR-2004

**Render target not found.** `--target <id>` names a target that isn't in the scenario's `targets` list.

- **What to do:** use one of the ids from the `available` parameter, or leave out `--target` to render all targets.

## SR-2101

**Block element not found on the page.** None of the block's locators matched an element.

- **Common causes:** a typo in the selector or `data-sr` name; the element only exists on desktop (or only on mobile); the element appears only after a click or a delay.
- **What to do:** check the locators of the block named in `block`. For device-specific blocks, add an override with `"skip": true` for that device. For late content, set `warmup_ms` on the start node.

## SR-2102

**Block locator matches several elements.** The first unique locator wins. This block only had locators that match more than one element.

- **What to do:** make the locator more specific, add `"nth": 0` to pick the first match, or mark the element with a unique `data-sr="…"` attribute (recommended).

## SR-3001

**No browser found.** ScrollRecord renders with a Chromium-based browser: Google Chrome, Microsoft Edge or Chromium.

- **What to do:** install Chrome or Edge (Edge is preinstalled on Windows 10/11), or point to a browser with `--browser <path>` or the `SR_BROWSER` environment variable. Automatic download is planned for Phase 6.

## SR-3002

**The browser failed to start.** A browser was found, but it didn't start or didn't open its control port in time.

- **Common causes:** antivirus blocking the launch; a broken installation; a very slow machine.
- **What to do:** try again. Try another browser with `--browser`. Run with `-v` and attach the log to an issue.

## SR-3003

**Browser communication failed.** The browser returned an error to a command, closed unexpectedly or stopped answering.

- **Common causes:** the page crashed (out of memory on very long pages); the browser was closed by something else.
- **What to do:** run again with `-v`. If it repeats on the same page, open an issue with the log and the scenario.

## SR-3004

**The page failed to load.** The URL couldn't be opened, or the page didn't finish loading in time.

- **Common causes:** the local dev server isn't running; a wrong URL or page path; no internet for online sites.
- **What to do:** open the URL from the `url` parameter in a normal browser. For local folders, check that `index.html` exists.

## SR-3005

**A script inside the page failed.** ScrollRecord runs small helper scripts in the page (time control, measuring blocks). One of them threw an error.

- **Common causes:** an unusual page that overrides built-in browser functions; an invalid CSS selector in a locator.
- **What to do:** check the selector in the message. Otherwise open an issue with the scenario and the page.

## SR-4001

**Frame capture failed.** The browser couldn't produce a screenshot for a frame.

- **Common causes:** the page crashed; the resolution is too large for the machine's memory.
- **What to do:** try a lower `resolution`. If it repeats, open an issue with the log.

## SR-5001

**ffmpeg not found.** MP4 and WebM output need ffmpeg. PNG sequence output (`"format": "png"`) works without it.

- **What to do (Windows):** open Terminal and run `winget install Gyan.FFmpeg`, then render again. ScrollRecord finds winget's ffmpeg right away. Alternatively, put `ffmpeg.exe` next to `scrollrecord.exe`.
- **What to do (Linux/macOS):** `sudo apt install ffmpeg` / `brew install ffmpeg`.
- You can also point to it with `--ffmpeg <path>` or the `SR_FFMPEG` environment variable. Automatic download is planned for Phase 6.

## SR-5002

**Video encoding failed.** ffmpeg started but reported an error. The `detail` parameter contains its last output lines.

- **Common causes:** your ffmpeg build lacks the codec (H.264 needs `libx264`, WebM needs `libvpx-vp9`); disk full.
- **What to do:** check `ffmpeg -encoders` for the codec, or install a full build. As a fallback, render to `"format": "png"`.

## SR-8001

**No site specified.** The scenario has no `meta.site` and no `--site` was given on the command line.

- **What to do:** add `"site": "path/to/site-folder"` or `"site": "http://localhost:5173"` to `meta`, or pass `--site`.

## SR-8002

**Site folder not found, or it has no index.html.** Folder paths are relative to the scenario file.

- **What to do:** check the `path` parameter. The folder must contain `index.html` at its top level.

## SR-8003

**The local site server failed.** ScrollRecord serves site folders through a small built-in web server on `127.0.0.1`. It couldn't start.

- **Common causes:** a firewall or security tool blocking local ports.
- **What to do:** allow ScrollRecord in the firewall, or serve the folder yourself (any local server) and pass its URL with `--site`.
