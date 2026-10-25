//! Built-in static web server for site folders.
//!
//! Why a server and not `file://`: browsers restrict `file://` pages (no
//! `fetch`, ES modules or some fonts), so a folder would not behave like the
//! deployed site. This server is deliberately tiny: GET/HEAD only, files only,
//! localhost only, no caching, standard library only.
//! It answers like common static hosts (Netlify, Vercel, GitHub Pages), so a
//! built site behaves the same here as when deployed:
//! - byte ranges (`Range: bytes=…`), which browsers need to seek in videos;
//! - clean URLs: `/pricing` serves `pricing.html` when there is no such folder;
//! - single-page apps: a page address that matches no file (`/pricing` of a
//!   React app) gets the root `index.html`, so the app's router can show it.
//!   Only for page loads (`Accept: text/html`); missing images, scripts and
//!   `fetch()` data still get 404.
//!
//! Why not a server library: with the previous one (tiny_http 0.12) a page
//! that loads several files at once sometimes never got one of them, so it
//! never finished loading (SR-3004; 7 of 8 loads of fixture 04 on a 2-core
//! machine). Here every connection gets its own thread, answers exactly one
//! request (`Connection: close`) and gives up on silent clients after a
//! timeout, so no connection can hold up another one. Test:
//! `crates/sr-render/tests/heavy_sites.rs` → `pages_with_many_files_always_load`.

use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use percent_encoding::percent_decode_str;
use sr_core::{AppError, AppResult, ErrorCode};

/// How long a connection may stay silent before it is dropped (browsers
/// open spare connections and may never use them).
const READ_TIMEOUT: Duration = Duration::from_secs(20);
/// Largest request head accepted (request line + headers).
const MAX_HEAD: usize = 64 * 1024;

/// A running local server. Stops when dropped.
pub struct LocalServer {
    port: u16,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl LocalServer {
    /// Starts serving `root` on `127.0.0.1` with a free port.
    pub fn start(root: &Path) -> AppResult<LocalServer> {
        let failed = |what: &str, err: std::io::Error| {
            AppError::new(ErrorCode::SiteServerFailed, format!("{what}: {err}"))
        };
        let listener =
            TcpListener::bind(("127.0.0.1", 0)).map_err(|e| failed("bind 127.0.0.1", e))?;
        let port = listener
            .local_addr()
            .map_err(|e| failed("local address", e))?
            .port();
        let stop = Arc::new(AtomicBool::new(false));
        let root = Arc::new(root.to_path_buf());
        let thread = std::thread::Builder::new()
            .name("sr-site-server".into())
            .spawn({
                let stop = Arc::clone(&stop);
                move || accept_loop(&listener, &root, &stop)
            })
            .map_err(|e| failed("spawn", e))?;
        tracing::info!(port, "local site server started");
        Ok(LocalServer {
            port,
            stop,
            thread: Some(thread),
        })
    }

    /// `http://127.0.0.1:<port>`.
    pub fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }
}

impl std::fmt::Debug for LocalServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalServer")
            .field("port", &self.port)
            .finish_non_exhaustive()
    }
}

impl Drop for LocalServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        // `accept()` blocks: one dummy connection wakes it up to see `stop`.
        let _ = TcpStream::connect(("127.0.0.1", self.port));
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

// ── Connections ─────────────────────────────────────────────────────────────

fn accept_loop(listener: &TcpListener, root: &Arc<PathBuf>, stop: &AtomicBool) {
    for conn in listener.incoming() {
        if stop.load(Ordering::Acquire) {
            break;
        }
        let Ok(stream) = conn else { continue };
        let root = Arc::clone(root);
        let spawned = std::thread::Builder::new()
            .name("sr-site-conn".into())
            .spawn(move || serve_connection(&root, stream));
        if let Err(err) = spawned {
            tracing::warn!(error = %err, "site server: cannot start a connection thread");
        }
    }
}

/// Reads one request, sends one response, closes the connection.
fn serve_connection(root: &Path, stream: TcpStream) {
    let _ = stream.set_read_timeout(Some(READ_TIMEOUT));
    let _ = stream.set_write_timeout(Some(READ_TIMEOUT));
    let reply = match read_request(&stream) {
        Ok(Some(request)) => handle(root, &request),
        Ok(None) => return, // connected and closed without asking anything
        Err(status) => Reply::text(status, "bad request"),
    };
    if let Err(err) = reply.write_to(&stream) {
        tracing::debug!(error = %err, "site server: client went away");
    }
    let _ = stream.shutdown(Shutdown::Write);
}

/// The parts of an HTTP request this server uses.
#[derive(Debug, Default)]
struct Request {
    method: String,
    target: String,
    headers: Vec<(String, String)>,
}

impl Request {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

/// Parses the request line and headers. `Ok(None)`: the client sent nothing.
/// `Err(status)`: malformed request.
fn read_request(stream: &TcpStream) -> Result<Option<Request>, u16> {
    let mut reader = BufReader::new(stream);
    let mut request = Request::default();
    let mut total = 0;
    let mut line = String::new();
    loop {
        line.clear();
        let n = reader.read_line(&mut line).map_err(|_| 400u16)?;
        if n == 0 {
            return if total == 0 { Ok(None) } else { Err(400) };
        }
        total += n;
        if total > MAX_HEAD {
            return Err(431);
        }
        let text = line.trim_end_matches(['\r', '\n']);
        if request.method.is_empty() {
            if text.is_empty() {
                continue; // tolerate stray empty lines before the request
            }
            let mut parts = text.split_whitespace();
            match (parts.next(), parts.next(), parts.next()) {
                (Some(m), Some(t), Some(v)) if v.starts_with("HTTP/1.") => {
                    request.method = m.to_owned();
                    request.target = t.to_owned();
                }
                _ => return Err(400),
            }
        } else if text.is_empty() {
            return Ok(Some(request));
        } else if let Some((name, value)) = text.split_once(':') {
            request
                .headers
                .push((name.trim().to_owned(), value.trim().to_owned()));
        }
    }
}

/// A response ready to send.
struct Reply {
    status: u16,
    headers: Vec<(&'static str, String)>,
    body: Body,
    /// HEAD: send the headers only.
    head_only: bool,
}

/// What follows the headers.
enum Body {
    /// A short text (errors).
    Text(Vec<u8>),
    /// Part of a file, streamed from disk: videos can be hundreds of MB and
    /// are asked for in pieces on every seek, so they are never read whole.
    File { path: PathBuf, start: u64, len: u64 },
}

impl Body {
    fn len(&self) -> u64 {
        match self {
            Body::Text(bytes) => bytes.len() as u64,
            Body::File { len, .. } => *len,
        }
    }
}

impl Reply {
    fn text(status: u16, body: &str) -> Reply {
        Reply {
            status,
            headers: vec![("Content-Type", "text/plain; charset=utf-8".into())],
            body: Body::Text(body.as_bytes().to_vec()),
            head_only: false,
        }
    }

    fn write_to(&self, mut stream: &TcpStream) -> std::io::Result<()> {
        let mut head = format!("HTTP/1.1 {} {}\r\n", self.status, reason(self.status));
        for (name, value) in &self.headers {
            head.push_str(&format!("{name}: {value}\r\n"));
        }
        head.push_str(&format!(
            "Content-Length: {}\r\nConnection: close\r\n\r\n",
            self.body.len()
        ));
        stream.write_all(head.as_bytes())?;
        if !self.head_only {
            match &self.body {
                Body::Text(bytes) => stream.write_all(bytes)?,
                Body::File { path, start, len } => {
                    let mut file = std::fs::File::open(path)?;
                    file.seek(SeekFrom::Start(*start))?;
                    // Stops early (error) when the browser has what it needs
                    // and closes the connection.
                    std::io::copy(&mut file.take(*len), &mut stream)?;
                }
            }
        }
        stream.flush()
    }
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        206 => "Partial Content",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        416 => "Range Not Satisfiable",
        431 => "Request Header Fields Too Large",
        _ => "Error",
    }
}

// ── Request handling ────────────────────────────────────────────────────────

fn handle(root: &Path, request: &Request) -> Reply {
    let head_only = request.method == "HEAD";
    if request.method != "GET" && !head_only {
        return Reply::text(405, "method not allowed");
    }
    // Browsers send "/path"; proxies and some tools send "http://host/path".
    let target = match request.target.split_once("://") {
        Some((_, rest)) => rest.find('/').map_or("/", |i| &rest[i..]),
        None => request.target.as_str(),
    };
    let wants_page = request
        .header("Accept")
        .is_some_and(|a| a.contains("text/html"));
    let size = |p: &Path| std::fs::metadata(p).ok().map(|m| m.len());
    let mut reply = match resolve(root, target, wants_page) {
        Some(path) => match size(&path) {
            Some(len) => file_reply(path, len, request.header("Range")),
            None => Reply::text(404, "not found"),
        },
        None => Reply::text(404, "not found"),
    };
    reply.head_only = head_only;
    reply
}

/// A whole file, or the requested part of it (`206 Partial Content`).
fn file_reply(path: PathBuf, len: u64, range: Option<&str>) -> Reply {
    let size = usize::try_from(len).unwrap_or(usize::MAX);
    let (status, start, part_len, content_range) =
        match range.map_or(ByteRange::Whole, |r| parse_range(r, size)) {
            ByteRange::Whole => (200, 0, len, None),
            ByteRange::Part(start, end) => (
                206,
                start as u64,
                (end - start + 1) as u64,
                Some(format!("bytes {start}-{end}/{len}")),
            ),
            ByteRange::Unsatisfiable => {
                let mut reply = Reply::text(416, "range not satisfiable");
                reply
                    .headers
                    .push(("Content-Range", format!("bytes */{len}")));
                return reply;
            }
        };
    let mut headers = vec![
        ("Content-Type", content_type(&path).to_owned()),
        ("Cache-Control", "no-store".to_owned()),
        ("Accept-Ranges", "bytes".to_owned()),
    ];
    if let Some(r) = content_range {
        headers.push(("Content-Range", r));
    }
    Reply {
        status,
        headers,
        body: Body::File {
            path,
            start,
            len: part_len,
        },
        head_only: false,
    }
}

/// What part of a file a `Range` header asks for.
#[derive(Debug, PartialEq, Eq)]
enum ByteRange {
    /// No usable range (absent, malformed or several ranges): send everything.
    Whole,
    /// Bytes `start..=end`.
    Part(usize, usize),
    /// The range starts beyond the end of the file (`416`).
    Unsatisfiable,
}

/// Parses `bytes=START-END`, `bytes=START-` and `bytes=-SUFFIX`.
fn parse_range(spec: &str, len: usize) -> ByteRange {
    let Some(r) = spec.trim().strip_prefix("bytes=") else {
        return ByteRange::Whole;
    };
    let Some((a, b)) = r.split_once('-') else {
        return ByteRange::Whole;
    };
    if r.contains(',') {
        return ByteRange::Whole;
    }
    let (a, b) = (a.trim(), b.trim());
    let last = len.saturating_sub(1);
    let part = if a.is_empty() {
        match b.parse::<usize>() {
            Ok(n) if n > 0 && len > 0 => Some((len.saturating_sub(n), last)),
            Ok(_) => None,
            Err(_) => return ByteRange::Whole,
        }
    } else {
        let Ok(start) = a.parse::<usize>() else {
            return ByteRange::Whole;
        };
        let end = if b.is_empty() {
            last
        } else {
            match b.parse::<usize>() {
                Ok(e) => e.min(last),
                Err(_) => return ByteRange::Whole,
            }
        };
        (start < len && start <= end).then_some((start, end))
    };
    match part {
        Some((start, end)) => ByteRange::Part(start, end),
        None => ByteRange::Unsatisfiable,
    }
}

/// Maps a URL path to a file inside `root`, or `None` if it's missing or
/// tries to escape the folder (`..`, absolute paths, drive letters).
/// `page` = the browser is loading a page (not an image/script/fetch): then
/// clean URLs and the single-page-app fallback apply.
fn resolve(root: &Path, url: &str, page: bool) -> Option<PathBuf> {
    let path = url.split(['?', '#']).next().unwrap_or("/");
    let decoded = percent_decode_str(path).decode_utf8().ok()?;
    let mut file = root.to_path_buf();
    for part in Path::new(decoded.trim_start_matches('/')).components() {
        match part {
            Component::Normal(name) => file.push(name),
            Component::CurDir => {}
            _ => return None,
        }
    }
    if file.is_dir() {
        file.push("index.html");
    }
    if file.is_file() {
        return Some(file);
    }
    if !page || file.extension().is_some() {
        return None;
    }
    // ── Clean URLs: /pricing → pricing.html ──
    let html = file.with_extension("html");
    if html.is_file() {
        return Some(html);
    }
    // ── Single-page app: unknown page address → the app's index.html ──
    let index = root.join("index.html");
    if index.is_file() {
        tracing::info!(path = %decoded, "no such page: serving index.html (single-page app fallback)");
        return Some(index);
    }
    None
}

fn content_type(path: &Path) -> &'static str {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "json" | "map" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "wasm" => "application/wasm",
        "txt" => "text/plain; charset=utf-8",
        "xml" => "application/xml",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpStream;

    fn get(base: &str, path: &str) -> (u16, String) {
        request(base, path, "")
    }

    /// GET with extra header lines (each ending in `\r\n`).
    fn request(base: &str, path: &str, headers: &str) -> (u16, String) {
        let addr = base.trim_start_matches("http://");
        let mut stream = TcpStream::connect(addr).unwrap();
        write!(
            stream,
            "GET {path} HTTP/1.1\r\nHost: {addr}\r\n{headers}Connection: close\r\n\r\n"
        )
        .unwrap();
        let mut out = String::new();
        stream.read_to_string(&mut out).unwrap();
        let code = out.split_whitespace().nth(1).unwrap().parse().unwrap();
        (code, out)
    }

    fn body(response: &str) -> &str {
        response.split_once("\r\n\r\n").map_or("", |(_, b)| b)
    }

    #[test]
    fn byte_ranges_for_video_seeking() {
        assert_eq!(parse_range("bytes=0-3", 10), ByteRange::Part(0, 3));
        assert_eq!(parse_range("bytes=4-", 10), ByteRange::Part(4, 9));
        assert_eq!(parse_range("bytes=-3", 10), ByteRange::Part(7, 9));
        assert_eq!(parse_range("bytes=5-100", 10), ByteRange::Part(5, 9));
        assert_eq!(parse_range("bytes=10-", 10), ByteRange::Unsatisfiable);
        assert_eq!(parse_range("bytes=0-1,4-5", 10), ByteRange::Whole);
        assert_eq!(parse_range("items=0-1", 10), ByteRange::Whole);
        assert_eq!(parse_range("bytes=x-1", 10), ByteRange::Whole);

        let root = std::env::temp_dir().join(format!("sr-site-range-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("index.html"), "<p>home</p>").unwrap();
        std::fs::write(root.join("clip.webm"), "0123456789").unwrap();
        let server = LocalServer::start(&root).unwrap();
        let base = server.base_url();

        let (code, out) = request(&base, "/clip.webm", "Range: bytes=2-5\r\n");
        assert_eq!(code, 206, "{out}");
        assert_eq!(body(&out), "2345");
        assert!(
            out.to_ascii_lowercase()
                .contains("content-range: bytes 2-5/10"),
            "{out}"
        );
        let (code, out) = get(&base, "/clip.webm");
        assert_eq!(code, 200);
        assert!(
            out.to_ascii_lowercase().contains("accept-ranges: bytes"),
            "{out}"
        );
        assert_eq!(request(&base, "/clip.webm", "Range: bytes=20-\r\n").0, 416);

        // HEAD: headers with the full length, no body.
        let addr = base.trim_start_matches("http://");
        let mut stream = TcpStream::connect(addr).unwrap();
        write!(stream, "HEAD /clip.webm HTTP/1.1\r\nHost: {addr}\r\n\r\n").unwrap();
        let mut out = String::new();
        stream.read_to_string(&mut out).unwrap();
        assert!(out.starts_with("HTTP/1.1 200"), "{out}");
        assert!(
            out.to_ascii_lowercase().contains("content-length: 10"),
            "{out}"
        );
        assert_eq!(body(&out), "");

        drop(server);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn clean_urls_and_single_page_app_fallback() {
        let root = std::env::temp_dir().join(format!("sr-site-spa-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("index.html"), "<p>app shell</p>").unwrap();
        std::fs::write(root.join("about.html"), "<p>about page</p>").unwrap();
        let server = LocalServer::start(&root).unwrap();
        let base = server.base_url();
        let page = "Accept: text/html,application/xhtml+xml,*/*;q=0.8\r\n";

        // Clean URL: /about → about.html.
        let (code, out) = request(&base, "/about", page);
        assert_eq!((code, body(&out)), (200, "<p>about page</p>"));
        // Router path of a single-page app → index.html.
        let (code, out) = request(&base, "/pricing/pro?x=1", page);
        assert_eq!((code, body(&out)), (200, "<p>app shell</p>"));
        // Not for files and data requests: those stay real 404s.
        assert_eq!(request(&base, "/missing.png", page).0, 404);
        assert_eq!(request(&base, "/api/items", "Accept: */*\r\n").0, 404);
        assert_eq!(
            get(&base, "/pricing").0,
            404,
            "no Accept header: not a page load"
        );

        drop(server);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn serves_files_including_unicode_names_and_blocks_traversal() {
        let root = std::env::temp_dir().join(format!("sr-site-test-{}", std::process::id()));
        std::fs::create_dir_all(root.join("Загрузки")).unwrap();
        std::fs::write(root.join("index.html"), "<h1>home</h1>").unwrap();
        std::fs::write(root.join("Загрузки").join("файл 1.css"), "body{}").unwrap();
        std::fs::write(root.parent().unwrap().join("secret.txt"), "secret").unwrap();

        let server = LocalServer::start(&root).unwrap();
        let base = server.base_url();

        let (code, body) = get(&base, "/");
        assert_eq!(code, 200);
        assert!(body.contains("<h1>home</h1>"));
        assert!(body.to_ascii_lowercase().contains("text/html"));

        let (code, body) = get(
            &base,
            "/%D0%97%D0%B0%D0%B3%D1%80%D1%83%D0%B7%D0%BA%D0%B8/%D1%84%D0%B0%D0%B9%D0%BB%201.css?v=2",
        );
        assert_eq!(code, 200, "{body}");
        assert!(body.contains("text/css"));

        assert_eq!(get(&base, "/../secret.txt").0, 404);
        assert_eq!(get(&base, "/%2e%2e/secret.txt").0, 404);
        assert_eq!(get(&base, "/missing.png").0, 404);

        drop(server);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Sends one GET and returns whether a 200 answer arrived within 3 s.
    fn answered(addr: &str, mut stream: TcpStream) -> bool {
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(3)))
            .unwrap();
        if write!(
            stream,
            "GET / HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"
        )
        .is_err()
        {
            return false;
        }
        let mut out = Vec::new();
        let _ = stream.read_to_end(&mut out);
        String::from_utf8_lossy(&out).starts_with("HTTP/1.1 200")
    }

    #[test]
    fn connections_are_answered_while_others_stay_open() {
        // Browsers keep spare connections open while they open new ones for
        // the next files. Idle connections must never hold up the others.
        let root = std::env::temp_dir().join(format!("sr-site-burst-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("index.html"), "<p>home</p>").unwrap();
        let server = LocalServer::start(&root).unwrap();
        let addr = server.base_url().trim_start_matches("http://").to_owned();

        let idle: Vec<TcpStream> = (0..6).map(|_| TcpStream::connect(&addr).unwrap()).collect();
        let mut lost = 0;
        for _ in 0..30 {
            // One finished request frees a worker; then a burst arrives at once.
            assert!(answered(&addr, TcpStream::connect(&addr).unwrap()));
            let burst: Vec<TcpStream> =
                (0..3).map(|_| TcpStream::connect(&addr).unwrap()).collect();
            lost += burst
                .into_iter()
                .filter(|s| !answered(&addr, s.try_clone().unwrap()))
                .count();
        }
        assert_eq!(lost, 0, "connections left without an answer");
        drop(idle);
        drop(server);
        let _ = std::fs::remove_dir_all(&root);
    }
}
