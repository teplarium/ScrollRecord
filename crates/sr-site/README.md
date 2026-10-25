# sr-site

Where the website comes from: a local folder (served by a tiny built-in web server on `127.0.0.1`) or a URL.

| File | What it holds |
|---|---|
| `src/lib.rs` | `SiteSource::parse` (URL or folder relative to the scenario), `serve`, `ServedSite::page_url` |
| `src/server.rs` | `LocalServer` (standard library only): a thread per connection, one request per connection, GET/HEAD only, no caching, byte ranges (video seeking), clean URLs (`/about` → `about.html`), single-page-app fallback for page loads, blocks `..` path traversal, Unicode file names |

```sh
cargo test -p sr-site
```
