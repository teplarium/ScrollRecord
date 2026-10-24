//! Minimal Chrome DevTools Protocol (CDP) client.
//!
//! Why our own small client instead of a big library: ScrollRecord needs about
//! a dozen CDP commands. A ~150-line synchronous client is easy to read, debug
//! and fix for anyone, compiles fast, and has no async runtime.
//! How it works: JSON messages over one WebSocket. Each command gets an `id`;
//! we read messages until the reply with that id arrives. Events that arrive
//! meanwhile are skipped (nothing in Phase 1 needs them).

use std::net::TcpStream;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use sr_core::{AppError, AppResult, ErrorCode};
use tungstenite::protocol::WebSocketConfig;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};

/// A connection to the browser's DevTools endpoint.
pub struct Cdp {
    socket: WebSocket<MaybeTlsStream<TcpStream>>,
    next_id: u64,
}

impl std::fmt::Debug for Cdp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cdp")
            .field("next_id", &self.next_id)
            .finish_non_exhaustive()
    }
}

fn protocol_error(msg: impl Into<String>) -> AppError {
    AppError::new(ErrorCode::BrowserProtocolError, msg)
}

impl Cdp {
    /// Connects to `ws://127.0.0.1:<port>/devtools/browser/<id>`.
    pub fn connect(ws_url: &str) -> AppResult<Cdp> {
        // Screenshots of 4K pages are large base64 strings; allow big messages.
        let config = WebSocketConfig::default()
            .max_message_size(Some(512 << 20))
            .max_frame_size(Some(512 << 20));
        let (socket, _) = tungstenite::client::connect_with_config(ws_url, Some(config), 0)
            .map_err(|err| protocol_error(format!("connect {ws_url}: {err}")))?;
        Ok(Cdp { socket, next_id: 1 })
    }

    /// Sends a command and waits for its result.
    /// `session` targets a page (flattened sessions); `None` = browser itself.
    pub fn call(
        &mut self,
        session: Option<&str>,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> AppResult<Value> {
        let id = self.next_id;
        self.next_id += 1;
        let mut msg = json!({ "id": id, "method": method, "params": params });
        if let Some(s) = session {
            msg["sessionId"] = json!(s);
        }
        self.socket
            .send(Message::text(msg.to_string()))
            .map_err(|err| protocol_error(format!("{method}: send failed: {err}")))?;

        let deadline = Instant::now() + timeout;
        loop {
            let reply = self.read_until(deadline, method)?;
            if reply.get("id").and_then(Value::as_u64) == Some(id) {
                if let Some(err) = reply.get("error") {
                    let text = err
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown error");
                    return Err(protocol_error(format!("{method}: {text}"))
                        .with_param("method", method)
                        .with_param("detail", text));
                }
                return Ok(reply.get("result").cloned().unwrap_or(Value::Null));
            }
            // Anything else is an event we did not subscribe to on purpose: skip it.
        }
    }

    /// Reads one JSON message, waiting at most until `deadline`.
    fn read_until(&mut self, deadline: Instant, method: &str) -> AppResult<Value> {
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(protocol_error(format!(
                    "{method}: no answer from the browser (timeout)"
                ))
                .with_param("method", method));
            }
            if let MaybeTlsStream::Plain(stream) = self.socket.get_mut() {
                let _ = stream.set_read_timeout(Some(left.max(Duration::from_millis(1))));
            }
            match self.socket.read() {
                Ok(Message::Text(text)) => {
                    return serde_json::from_str(text.as_str())
                        .map_err(|err| protocol_error(format!("bad JSON from browser: {err}")));
                }
                Ok(Message::Binary(bytes)) => {
                    return serde_json::from_slice(&bytes)
                        .map_err(|err| protocol_error(format!("bad JSON from browser: {err}")));
                }
                Ok(Message::Close(_)) => {
                    return Err(protocol_error("browser closed the connection"));
                }
                Ok(_) => {}
                Err(tungstenite::Error::Io(err))
                    if matches!(
                        err.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) => {}
                Err(err) => {
                    return Err(protocol_error(format!("{method}: connection error: {err}")));
                }
            }
        }
    }
}
