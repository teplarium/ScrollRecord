ScrollRecord (command line) - smooth scroll videos of websites
=============================================================

WHAT YOU NEED
  - A Chromium-based browser: Microsoft Edge (already on Windows 10/11),
    Google Chrome or Chromium.
  - ffmpeg, for MP4/WebM videos. Easiest on Windows: open "Terminal" and run
        winget install Gyan.FFmpeg
    Or download ffmpeg.exe and put it next to scrollrecord.exe.
    Without ffmpeg you can still render PNG frames ("format": "png").

TRY THE EXAMPLE
  Windows: double-click render-example.bat
  Linux:   ./render-example.sh
  It renders a small landing page for desktop (16:9) and iPhone (9:16),
  1080p, 60 fps. This takes a few minutes. The videos land in example/renders.

YOUR OWN SITE
  1. Copy example/lumen.srflow.json and open it in a text editor.
  2. "site": your site folder (the one with index.html), relative to the
     scenario file, or an address such as "http://localhost:5173".
  3. "nodes": the blocks to show. Easiest: mark sections in your HTML,
     e.g. <section data-sr="pricing">, and use { "sr": "pricing" }.
     "hold_ms": pause on the block in ms (0 = scroll through without stopping).
  4. Windows: drag the .srflow.json file onto render.bat.
     Linux:   ./render.sh path/to/file.srflow.json
     Videos appear in a "renders" folder next to the scenario file.

  Check a scenario without rendering:
     scrollrecord validate path/to/file.srflow.json
  See which browser and ffmpeg will be used:
     scrollrecord info

IF WINDOWS WARNS ABOUT THE FILES
  This test build is not signed yet. Before unzipping, right-click the zip,
  choose Properties and tick "Unblock". Or click "More info" > "Run anyway".

IF SOMETHING GOES WRONG
  Every error has a code like SR-5001. What it means and how to fix it:
    https://github.com/teplarium/ScrollRecord/blob/main/docs/ERRORS.md
  Report a problem:
    https://github.com/teplarium/ScrollRecord/issues

License: MIT OR Apache-2.0 (see LICENSE-MIT, LICENSE-APACHE).
