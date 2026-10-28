# 0005 — Lower render priority

- **Status:** accepted
- **Date:** 2026-10-28
- **Related:** HANDOFF §14.1 ("Пониженный приоритет рендера"), `docs/phase-3-plan.md` step 1c

## Context
The owner wants the computer to stay responsive while ScrollRecord renders: Chrome and ffmpeg should run below normal priority, so a render only gets slower when other programs are busy, never fails and never changes the frames.
Constraints: `unsafe_code = "forbid"` (no direct Win32/libc calls), no new dependency for this, and every wait on the browser has a time limit (startup 30 s, DevTools calls 30 s, screenshot 60 s, page load 45 s) that detects a hung browser.
What we measured (2026-10-28):
- **Windows:** `BELOW_NORMAL_PRIORITY_CLASS` at process creation works for ffmpeg and Chrome's main process (which also encodes the screenshots). Chrome then raises its own tab process to normal and its GPU process to above normal; no command-line switch prevents that (`--disable-renderer-backgrounding` is already set; `--disable-features=GpuProcessHighPriorityWin` changes nothing). Those two did about 17 % of a 4K render's CPU work; ffmpeg and the main process about 65 %.
- **Linux:** starting Chrome through `nice -n 10` made all four browser tests fail on GitHub's Linux runner (run #11): Chrome never opened its DevTools port within 30 s. At normal priority it had started in every earlier run. In a clean Ubuntu 24.04 container a niced Chrome started fine when idle, but 4–6× slower (1 s → 4–6 s) once normal-priority programs kept every core busy, and the gap grows with the load. So on a busy machine a niced browser can miss a time limit: "slower" turns into "failed".

## Options considered
1. **`nice` the browser on Linux too, with longer time limits**: keeps the feature everywhere. But we could not reproduce the runner's failure, so no limit is known to be long enough, and a hung browser would be reported minutes later.
2. **Start at normal priority, lower after startup**: avoids the slow start. Needs `setpriority` on every thread (unsafe) or putting Chrome into its own process group for `renice -g`; with its own group, Ctrl+C on the CLI would leave Chrome running headless. Rejected.
3. **Browser below normal on Windows only; ffmpeg below normal everywhere**: Windows (the owner's platform, where it was measured and its CI passed) keeps the benefit; Linux keeps the start that always worked. ffmpeg is safe to lower on every OS because nothing waits on it with a time limit (pushing frames just blocks longer).

## Decision
Option 3. `LaunchOptions::low_priority` affects the browser on Windows only (`sr-browser/src/process.rs` → `background_command`); `EncodeSettings::low_priority` lowers ffmpeg on every OS (`nice -n 10` on Unix when `nice` exists). On by default, `--normal-priority` turns both off.

## Consequences
- Test `browser_priority_is_lowered_only_on_windows` (`sr-browser/tests/browser.rs`) fails if `nice` comes back for the browser on Linux; the ffmpeg priority is checked in `raw_frames_become_a_video_when_ffmpeg_is_installed`.
- On Linux a render still loads the machine through Chrome. Revisit if Linux users complain: option 1 with time limits that measure the browser's CPU progress instead of wall-clock time.
- On Windows a heavy normal-priority job that keeps every core busy (a game, a video export) can starve Chrome's main process; Windows' anti-starvation boost keeps it moving, but very slowly. If renders then fail with a time-limit error, the same revisit applies there.
