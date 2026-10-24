//! Built-in static web server for site folders.
//!
//! Why a server and not `file://`: browsers restrict `file://` pages (no
//! `fetch`, ES modules or some fonts), so a folder would not behave like the
//! deployed site. This server is deliberately tiny: GET/HEAD only, files only,
//! localhost only, no caching, one background thread.

use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::thread::JoinHandle;

use percent_encoding::percent_decode_str;
use sr_core::{AppError, AppResult, ErrorCode};
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

/// A running local server. Stops when dropped.
pub struct LocalServer {
    server: Arc<Server>,
    port: u16,
    thread: Option<JoinHandle<()>>,
}

impl LocalServer {
    /// Starts serving `root` on `127.0.0.1` with a free port.
    pub fn start(root: &Path) -> AppResult<LocalServer> {
        let server = Server::http("127.0.0.1:0").map_err(|err| {
            AppError::new(
                ErrorCode::SiteServerFailed,
                format!("bind 127.0.0.1: {err}"),
            )
        })?;
        let port = server
            .server_addr()
            .to_ip()
            .map(|addr| addr.port())
            .ok_or_else(|| AppError::new(ErrorCode::SiteServerFailed, "server has no TCP port"))?;
        let server = Arc::new(server);
        let worker = Arc::clone(&server);
        let root = root.to_path_buf();
        let thread = std::thread::Builder::new()
            .name("sr-site-server".into())
            .spawn(move || {
                for request in worker.incoming_requests() {
                    handle(&root, request);
                }
            })
            .map_err(|err| AppError::new(ErrorCode::SiteServerFailed, format!("spawn: {err}")))?;
        tracing::info!(port, "local site server started");
        Ok(LocalServer {
            server,
            port,
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
        self.server.unblock();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

// ── Request handling ────────────────────────────────────────────────────────

fn handle(root: &Path, request: Request) {
    let method = request.method().clone();
    let response = if method != Method::Get && method != Method::Head {
        text(405, "method not allowed")
    } else {
        match resolve(root, request.url()) {
            Some(path) => match std::fs::read(&path) {
                Ok(bytes) => {
                    let mut resp = Response::from_data(bytes);
                    let headers = [
                        header("Content-Type", content_type(&path)),
                        header("Cache-Control", "no-store"),
                    ];
                    for h in headers.into_iter().flatten() {
                        resp.add_header(h);
                    }
                    resp
                }
                Err(_) => text(404, "not found"),
            },
            None => text(404, "not found"),
        }
    };
    if let Err(err) = request.respond(response) {
        tracing::debug!(error = %err, "client went away");
    }
}

/// Maps a URL path to a file inside `root`, or `None` if it's missing or
/// tries to escape the folder (`..`, absolute paths, drive letters).
fn resolve(root: &Path, url: &str) -> Option<PathBuf> {
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
    file.is_file().then_some(file)
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

/// Builds a header. `None` only for non-ASCII input, which our static
/// names and values never contain.
fn header(name: &str, value: &str) -> Option<Header> {
    Header::from_bytes(name.as_bytes(), value.as_bytes()).ok()
}

fn text(code: u16, body: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_data(body.as_bytes().to_vec()).with_status_code(StatusCode(code))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpStream;

    fn get(base: &str, path: &str) -> (u16, String) {
        let addr = base.trim_start_matches("http://");
        let mut stream = TcpStream::connect(addr).unwrap();
        write!(
            stream,
            "GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"
        )
        .unwrap();
        let mut out = String::new();
        stream.read_to_string(&mut out).unwrap();
        let code = out.split_whitespace().nth(1).unwrap().parse().unwrap();
        (code, out)
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
}
