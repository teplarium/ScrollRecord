# sr-browser

Finds Chrome/Edge/Chromium, starts it headless with a throw-away profile and drives it through the Chrome DevTools Protocol.

| File | What it holds |
|---|---|
| `src/locate.rs` | `find_browser`: explicit path → `SR_BROWSER` → Chrome → Edge → Chromium |
| `src/process.rs` | Launch flags, `DevToolsActivePort`, clean shutdown, profile cleanup |
| `src/cdp.rs` | Minimal synchronous CDP client (JSON over WebSocket) |
| `src/page.rs` | `Browser`, `Page`: emulate device, navigate, prepare, measure, `FrameDriver` (step + capture) |
| `js/runtime.js` | Injected into every page: virtual clock (timers, rAF, idle callbacks), animation control, one scroll event per position, framework-task (MessageChannel) wait, observer settle, video/audio on page time, animated images (GIF) on page time, prepare, locators, measuring |

Why it works this way: `docs/adr/0002-deterministic-capture.md` and `docs/adr/0004-heavy-sites.md`.

```sh
cargo test -p sr-browser          # the integration test needs Chrome/Edge/Chromium (skipped otherwise)
SR_BROWSER=/path/to/chrome cargo test -p sr-browser
```
