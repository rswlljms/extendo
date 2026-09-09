// Minimal HTTP/1.1 server for the video path.
//
// Hand-rolled rather than pulling hyper: this serves exactly three routes on a
// LAN, and MJPEG needs raw control of the response body anyway. The request
// parser is intentionally strict and the framing lives in mjpeg.rs so both are
// testable without a socket.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;

use crate::cli::Args;
use crate::mjpeg;
use crate::{JpegFrame, Stats};

/// Cap on the request head we will buffer; a client sending more than this is
/// not a viewer we want to serve.
const MAX_HEAD: usize = 8 * 1024;

/// Parsed request line: method, path, and the raw query string.
#[derive(Debug, PartialEq, Eq)]
pub struct Request {
    pub method: String,
    pub path: String,
    pub query: String,
}

/// Parse the request line out of a raw HTTP head.
///
/// # Errors
/// Returns a message when the head is not a well-formed request line.
pub fn parse_request(head: &str) -> Result<Request, String> {
    let line = head.lines().next().ok_or("empty request")?;
    let mut parts = line.split_whitespace();
    let method = parts.next().ok_or("no method")?.to_string();
    let target = parts.next().ok_or("no target")?;
    if parts.next().is_none() {
        return Err("no HTTP version".to_string());
    }
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    Ok(Request { method, path: path.to_string(), query: query.to_string() })
}

/// Look up a key in a `a=b&c=d` query string, percent-decoding minimally.
#[must_use]
pub fn query_param(query: &str, key: &str) -> Option<String> {
    query.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=')?;
        (k == key).then(|| percent_decode(v))
    })
}

/// Decode `%XX` escapes and `+`. Enough for tokens in a query string.
#[must_use]
pub fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' if i + 2 < b.len() => {
                let hex = std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or("");
                if let Ok(v) = u8::from_str_radix(hex, 16) {
                    out.push(v);
                    i += 3;
                } else {
                    out.push(b[i]);
                    i += 1;
                }
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Constant-time-ish token comparison. Tokens are short and this is a LAN
/// service, but avoid leaking length/prefix through early return anyway.
#[must_use]
pub fn token_ok(expected: &str, provided: Option<&str>) -> bool {
    if expected.is_empty() {
        return true; // loopback/dev mode; cli::validate already enforced this
    }
    let Some(got) = provided else { return false };
    if got.len() != expected.len() {
        return false;
    }
    got.bytes().zip(expected.bytes()).fold(0u8, |acc, (a, b)| acc | (a ^ b)) == 0
}

/// Serve until the process is killed.
///
/// # Errors
/// Returns the bind error if the address is unavailable.
pub async fn serve(
    cfg: Args,
    rx: watch::Receiver<Option<JpegFrame>>,
    stats: Arc<Stats>,
    source: String,
) -> std::io::Result<()> {
    let listener = TcpListener::bind((cfg.bind.as_str(), cfg.port)).await?;
    println!(
        "extendo-core: http://{}:{}  source={}  {}x{}@{} q{}",
        cfg.bind, cfg.port, source, cfg.width, cfg.height, cfg.fps, cfg.quality
    );
    println!("  GET /video.mjpg  (stream)   GET /frame.jpg  (single)   GET /health");
    loop {
        let (stream, _peer) = listener.accept().await?;
        let cfg = cfg.clone();
        let rx = rx.clone();
        let stats = Arc::clone(&stats);
        let source = source.clone();
        tokio::spawn(async move {
            if let Err(e) = handle(stream, cfg, rx, stats, source).await {
                // Client disconnects are normal for streaming; log at debug volume.
                let msg = e.to_string();
                if !msg.contains("aborted") && !msg.contains("forcibly closed") {
                    eprintln!("http: {msg}");
                }
            }
        });
    }
}

async fn handle(
    mut stream: TcpStream,
    cfg: Args,
    mut rx: watch::Receiver<Option<JpegFrame>>,
    stats: Arc<Stats>,
    source: String,
) -> std::io::Result<()> {
    // Read just the head; none of our routes have a body.
    let mut buf = Vec::with_capacity(1024);
    let mut chunk = [0u8; 1024];
    loop {
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            return Ok(());
        }
        buf.extend_from_slice(&chunk[..n]);
        if buf.windows(4).any(|w| w == b"\r\n\r\n") || buf.len() > MAX_HEAD {
            break;
        }
    }
    let head = String::from_utf8_lossy(&buf).into_owned();
    let Ok(req) = parse_request(&head) else {
        let r = mjpeg::text_response(400, "Bad Request", "application/json", "{\"error\":\"bad request\"}");
        stream.write_all(r.as_bytes()).await?;
        return Ok(());
    };

    if req.method != "GET" {
        let r = mjpeg::text_response(405, "Method Not Allowed", "application/json", "{\"error\":\"GET only\"}");
        stream.write_all(r.as_bytes()).await?;
        return Ok(());
    }

    // /health is unauthenticated on purpose: it exposes no pixels, and the Node
    // host polls it to tell whether the core is alive.
    if req.path == "/health" {
        let body = stats.snapshot_json(&cfg, &source);
        let r = mjpeg::text_response(200, "OK", "application/json", &body);
        stream.write_all(r.as_bytes()).await?;
        return Ok(());
    }

    // Everything below streams desktop pixels and therefore needs the token.
    let provided = query_param(&req.query, "token");
    if !token_ok(&cfg.token, provided.as_deref()) {
        let r = mjpeg::text_response(401, "Unauthorized", "application/json", "{\"error\":\"pairing token required\"}");
        stream.write_all(r.as_bytes()).await?;
        return Ok(());
    }

    match req.path.as_str() {
        "/frame.jpg" => {
            stats.clients.fetch_add(1, Ordering::Relaxed);
            // Wait for the first encoded frame (capture skips work with 0 clients).
            let frame = wait_for_frame(&mut rx).await;
            stats.clients.fetch_sub(1, Ordering::Relaxed);
            match frame {
                Some(f) => {
                    stream.write_all(mjpeg::single_image_headers(f.bytes.len()).as_bytes()).await?;
                    stream.write_all(&f.bytes).await?;
                }
                None => {
                    let r = mjpeg::text_response(503, "Service Unavailable", "application/json", "{\"error\":\"no frame yet\"}");
                    stream.write_all(r.as_bytes()).await?;
                }
            }
            Ok(())
        }
        "/video.mjpg" => {
            stats.clients.fetch_add(1, Ordering::Relaxed);
            let result = stream_mjpeg(&mut stream, &mut rx).await;
            stats.clients.fetch_sub(1, Ordering::Relaxed);
            result
        }
        _ => {
            let r = mjpeg::text_response(404, "Not Found", "application/json", "{\"error\":\"not found\"}");
            stream.write_all(r.as_bytes()).await?;
            Ok(())
        }
    }
}

/// Wait up to ~2s for the first frame so a client that connects before capture
/// warms up gets a picture instead of an error.
async fn wait_for_frame(rx: &mut watch::Receiver<Option<JpegFrame>>) -> Option<JpegFrame> {
    if let Some(f) = rx.borrow_and_update().clone() {
        return Some(f);
    }
    for _ in 0..20 {
        if tokio::time::timeout(std::time::Duration::from_millis(100), rx.changed())
            .await
            .is_ok()
        {
            if let Some(f) = rx.borrow_and_update().clone() {
                return Some(f);
            }
        }
    }
    None
}

async fn stream_mjpeg(
    stream: &mut TcpStream,
    rx: &mut watch::Receiver<Option<JpegFrame>>,
) -> std::io::Result<()> {
    stream.write_all(mjpeg::stream_headers().as_bytes()).await?;
    stream.flush().await?;
    let mut last_seq = 0u64;
    loop {
        // `changed()` resolving means a newer frame landed; stale frames are
        // skipped by design (watch keeps only the latest).
        if rx.changed().await.is_err() {
            return Ok(()); // capture ended
        }
        let frame = rx.borrow_and_update().clone();
        let Some(f) = frame else { continue };
        if f.seq == last_seq {
            continue;
        }
        last_seq = f.seq;
        stream.write_all(mjpeg::part_header(f.bytes.len()).as_bytes()).await?;
        stream.write_all(&f.bytes).await?;
        stream.write_all(mjpeg::PART_TRAILER).await?;
        stream.flush().await?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_normal_request_line() {
        let r = parse_request("GET /video.mjpg?token=abc HTTP/1.1\r\nHost: x\r\n\r\n").expect("parse");
        assert_eq!(r.method, "GET");
        assert_eq!(r.path, "/video.mjpg");
        assert_eq!(r.query, "token=abc");
    }

    #[test]
    fn parses_request_without_query() {
        let r = parse_request("GET /health HTTP/1.1\r\n\r\n").expect("parse");
        assert_eq!(r.path, "/health");
        assert_eq!(r.query, "");
    }

    #[test]
    fn rejects_malformed_request_lines() {
        assert!(parse_request("").is_err());
        assert!(parse_request("GET\r\n").is_err());
        assert!(parse_request("GET /only-two-parts\r\n").is_err());
    }

    #[test]
    fn reads_query_params() {
        assert_eq!(query_param("token=abc&x=1", "token").as_deref(), Some("abc"));
        assert_eq!(query_param("x=1&token=abc", "token").as_deref(), Some("abc"));
        assert_eq!(query_param("x=1", "token"), None);
        assert_eq!(query_param("", "token"), None);
    }

    #[test]
    fn percent_decodes_tokens() {
        // Tokens are URL-encoded by the viewer (encodeURIComponent).
        assert_eq!(percent_decode("a%2Bb"), "a+b");
        assert_eq!(percent_decode("a+b"), "a b");
        assert_eq!(percent_decode("plain"), "plain");
        assert_eq!(percent_decode("%zz"), "%zz"); // malformed stays literal
    }

    #[test]
    fn token_check_accepts_only_exact_match() {
        assert!(token_ok("secret", Some("secret")));
        assert!(!token_ok("secret", Some("secrez")));
        assert!(!token_ok("secret", Some("secre")));
        assert!(!token_ok("secret", Some("secretx")));
        assert!(!token_ok("secret", None));
    }

    #[test]
    fn empty_expected_token_is_dev_loopback_mode() {
        // cli::validate guarantees this only happens on a loopback bind.
        assert!(token_ok("", None));
        assert!(token_ok("", Some("anything")));
    }
}
