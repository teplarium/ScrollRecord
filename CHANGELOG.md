# Changelog

Written in plain language for users and the project owner. Newest first.
Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Versions follow [SemVer](https://semver.org/).

## [Unreleased]

### Editor, step 3.4a — building the scenario with the mouse
- The canvas is controlled like Figma: the pointer selects. Click a shape, or drag on an empty spot to select every shape the box touches; Shift or Ctrl+click adds shapes to the selection; dragging a selected shape moves the whole group (one Ctrl+Z puts it back). The wheel scrolls up and down, Shift+wheel left and right, Ctrl+wheel (or a touchpad pinch) zooms towards the cursor. To move the view by dragging, hold Space or use the middle mouse button.
- Put Start, End and blocks on the canvas from the site map: drag a row to the place you want, or click it — it appears in a free spot in the middle of the view. Clicking a row that is already on the canvas (red dot) shows it there; drag a block again to use it a second time.
- Pull an arrow out of a shape: hover it, grab one of the round handles on its sides and drop it on another shape (anywhere on it). A shape has one arrow out and one in, so a new arrow replaces the old one. The first arrow from Start to a block also tells Start where the video begins.
- Move shapes by dragging; Delete (or Backspace) removes the selected shapes or arrows.
- Ctrl+Z undoes, Ctrl+Shift+Z or Ctrl+Y redoes (up to 200 steps per project tab, kept while you switch tabs). The shortcuts work with any keyboard layout, Russian included.
- Everything is saved by itself about a second after a change, and before a tab or the window closes; Ctrl+S saves at once. The canvas corner says "Saved", "Saving…" or "Not saved". If saving fails, the error is shown and the tab stays open; closing the window a second time quits anyway.
- Positions are shared by all devices. On a device with taller cards (a phone) shapes that would overlap are shown moved down; nothing moves in the file until you drag a shape yourself.
- The site map is not read again after every change: only when the site, pages or the scenario's blocks change (moves and arrows don't count).

### Editor, step 3.3 — the site map lists the site's blocks and buttons
- The "Site map" panel now shows each page's blocks (marked with `data-sr`, plus those the scenario uses) and, when you open a block, its buttons, links, fields and other things the cursor can use — each with a small icon, like layers in Figma. A red dot means the block is already in the scenario; a warning means the scenario uses a block this device doesn't have.
- The site is read automatically when a project opens and when you switch the device (the map can differ: a phone shows "Open menu" instead of the menu links). While it reads, a progress line with "Cancel" is shown.
- Results are kept in the project's `cache` folder, so reopening a project or switching back to a device shows the map at once. The cache is renewed automatically when the site's files or the scenario's blocks change. "Read the site again" (↻) ignores it, "Clear cached site data" deletes it. For a site given as an address (`http://localhost…`) the last map is shown dimmed while it is read again.

### Editor, step 3.3a — reading the site for the site map (under the hood)
- ScrollRecord can now read a website the way the video will see it (same browser, same device, same page preparation) and list its blocks — the parts marked with `data-sr` plus the blocks the scenario already uses — and, inside each block, the buttons, links, fields, checkboxes, tabs and other things the cursor can use. For each of them it also works out how to find it again when recording, and keeps only ways that find exactly that element. One page takes about a second. The site map panel shows this in the next step.
- A block the scenario names but the site doesn't have (on this device) is listed with a warning instead of failing.
- New error code SR-0004 (stopped by the user), for long jobs that get a "Cancel" button.

### Editor, step 3.2 — the editor window and the scenario canvas
- Deleting a project is now deliberate: the trash can on its row in "Recent projects" asks first and then moves the project folder to the Recycle Bin (it can be restored from there; the website is not touched). A project that is open in a tab can't be deleted — close its tab first. The × on a tab only closes the project. Before, the × in the list removed even an open project from the list, which then also vanished from the tabs restored at the next start.
- New error code SR-1007 (the project couldn't be moved to the Recycle Bin; nothing was deleted).
- Choosing files is clearer: the website is chosen by picking its start page (`index.html`) and an existing project by picking its `project.json` — the windows show files instead of an empty-looking folder. Every file window also offers "All files".
- A project tab is now the editor: "Site map" on the left (the website, Start, End and the site's pages; blocks and buttons arrive in the next step), the canvas in the middle, properties on the right.
- The canvas shows the scenario: Start and End as ovals, each block as a card with its name and pause ("1.5 s", "No stop"), and the arrows between them. Move around with the mouse wheel or touchpad (or by dragging), zoom with Ctrl+wheel; small buttons zoom and "show the whole scenario", the mini map shows where you are. Editing on the canvas comes in step 3.4.
- Block cards have the shape of the chosen device: wide on a desktop, tall on a phone (until the site is measured, a card is one screen of that device). The device is picked at the top of the right panel; the arrows stay the same. Preview, Render and Queue buttons are there but not active yet.
- A scenario without saved positions (for example a file written by an AI) arranges itself: one column in the order of the arrows, branches to the right, unconnected shapes in their own column.
- Clicking a shape shows its kind and name on the right (its settings become editable in step 3.5).
- A project opens showing the beginning of its scenario at a readable size (a short scenario is shown whole); the "show the whole scenario" button fits everything.
- For checking speed with long scenarios: `tests/fixtures/flows/long-250.srflow.json` (250 shapes).

### Editor, step 3.1b — project tabs and the start screen
- Projects open as tabs at the top of the window, like files in Figma: a Home tab, one tab per project, and "+" for a new project. Closing the app and opening it again brings the same tabs back.
- "New project" asks for a name (empty gives "Untitled 1", "Untitled 2"…), where to create it (first `Documents\ScrollRecord`, then the last folder used), the website (a folder, or a `http://localhost…` address) and whether to start empty or from a scenario file.
- "Open project" and the recent list open a project folder; a project that is already open just switches to its tab. A project left "open" by a crash can be opened with "Open anyway". Projects whose folder is gone are shown greyed out and can be removed from the list.
- A project tab shows the project's name, folder, website and video formats until the canvas arrives. If the site folder was moved, it says so and offers "Locate site folder…".

### Editor, step 3.1a — projects under the hood
- The app can now create, open, save and close projects (the buttons on the start screen arrive in the next step). A project is a folder with the scenario (the same file the command line renders), the canvas layout, a cache folder and a renders folder.
- Saving can never leave a half-written file, even if the computer switches off mid-save; the previous version of each file is kept as `.bak`.
- A new project without a name is called "Untitled 1", "Untitled 2"… A new project starts with an empty canvas and one video format, Desktop 16:9 1080p.
- If the same project is already open in another ScrollRecord window, or was left "open" by a crash, ScrollRecord says so (SR-1005) and lets you open it anyway.
- The site folder is stored relative to the project when it is inside or next to it, so the two can be moved or copied together.
- A scenario file can be imported into a project and exported from it (with the canvas layout inside).
- The app remembers the last 10 projects, the folder used for the last new project, and which projects were open, to reopen them next time.
- New error codes SR-1003…SR-1006 (not a project, project from a newer version, project already open, folder name taken).

### Before the editor — render speed report and lower priority
- After each video, `scrollrecord render` now shows where the time went: browser start, page loading, finding the buttons, moving the page frame by frame, screenshots, drawing the cursor and encoding. `--json` includes the same numbers (`stages`). This tells us what to speed up next.
- While rendering, Chrome and ffmpeg now run at "below normal" priority, so the computer stays responsive (music, browser, typing). The video is exactly the same; it only takes longer when other programs are busy (on an idle computer the speed is unchanged). `--normal-priority` turns this off. On Windows, Chrome keeps the page itself and its graphics part at normal priority; the heaviest parts (video encoding, screenshots) run lower. On Linux only ffmpeg runs lower: a low-priority Chrome could take too long to start on a busy computer, so the browser keeps normal priority there.

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
