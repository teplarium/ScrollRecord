# Changelog

Written in plain language for users and the project owner. Newest first.
Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Versions follow [SemVer](https://semver.org/).

## [Unreleased]

### Before the editor — render speed report and lower priority
- After each video, `scrollrecord render` now shows where the time went: browser start, page loading, finding the buttons, moving the page frame by frame, screenshots, drawing the cursor and encoding. `--json` includes the same numbers (`stages`). This tells us what to speed up next.
- While rendering, Chrome and ffmpeg now run at "below normal" priority, so the computer stays responsive (music, browser, typing). The video is exactly the same; it only takes longer when other programs are busy (on an idle computer the speed is unchanged). `--normal-priority` turns this off. On Windows, Chrome keeps the page itself and its graphics part at normal priority; the heaviest parts (video encoding, screenshots) run lower.

### Phase 1b — tested on "heavy" sites
- Five new test sites: CSS animations, GSAP + ScrollTrigger, Lenis smooth scroll, a React app, and a page with video, Lottie, canvas/WebGL and an animated GIF. Each one now renders smoothly and gives exactly the same frames twice. Example scenarios for them are in `examples/` (02–06).

### Fixed
- When two browsers started at the very same moment (for example several renders at once), both could end up controlling one browser, and whichever finished first closed it under the other: the other one stopped with error SR-3003 ("connection reset" or "session not found"). Each browser now always gets its own private folder.
- On computers without a graphics card (servers, virtual machines, remote desktops), two renders of the same scenario could differ in a single pixel row at the edge of an element that slides in (seen with GSAP cards on the Windows test server). The browser reused leftover picture memory there; ScrollRecord now tells it to reuse only memory of exactly the right size, which makes the frames repeatable.
- Cards that are "paused until they come into view" (a very common CSS trick) stayed invisible in videos. They now play when they appear.
- Native CSS scroll animations (`animation-timeline`) froze; they now follow the scroll.
- Effects that start when another animation ends never started; they do now.
- Effects that depend on scroll speed or direction (for example Lenis "skew while scrolling", headers that hide when scrolling down) disappeared, because the page heard every scroll position twice. Now it hears it once.
- Videos on the page played in real time, so every render showed different video frames. Videos now follow the video's own timeline exactly (at 0.8 s you always see the frame of 0.8 s), and play/pause/end events reach the site at the right frame.
- Animated GIFs (and animated WebP/PNG) also played in real time; they now follow the video's timeline too.
- A local site that loads several files at once could sometimes never finish loading (error SR-3004). The built-in web server was replaced by a simpler, sturdier one.
- The computer's own settings ("reduce motion", dark theme) no longer change the video: ScrollRecord always records full motion and the light theme.

### Added
- Local site folders now behave like on common hosting: `/about` opens `about.html`, any address of a single-page app (React, Vue, …) opens the app, and videos can be rewound.
- React and other frameworks finish their updates inside the frame that caused them; delayed "do it when idle" work also happens at a fixed moment.

### Phase 2 — cursor
- **Cursor in videos (Phase 2).** The cursor moves like a hand (pause, curved path, soft stop, small overshoot), hovers so the site really reacts, clicks with a press and a ripple, and shows a finger circle for taps on phones. Two built-in cursor packs: `classic-dark`, `classic-light`. Custom packs: a folder with `cursor-pack.json` + SVGs.
- Buttons inside blocks that slide in are measured after they appear, so the cursor lands on them.
- `scrollrecord validate` also checks the cursor pack and shows how many actions each device gets.
- New error codes: SR-6001 (cursor pack not found), SR-6002 (cursor pack broken).

### Added — ready-to-use command-line package
- Every CI run now builds `ScrollRecord-CLI-Windows` (and `-Linux`): `scrollrecord.exe`, an example site with its scenario, and launchers. Double-click `render-example.bat` to render the example, or drag your own `*.srflow.json` onto `render.bat`. The videos folder opens when done.
- ffmpeg is also found next to `scrollrecord.exe` and right after `winget install Gyan.FFmpeg` (no new terminal needed). The "ffmpeg not found" message says how to install it.

### Fixed
- Scroll-reveal effects could, rarely, start one frame later in one render than in another. ScrollRecord now delivers "element entered the viewport" events to the page itself at a fixed moment, so repeated renders are identical even on a busy machine.

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
