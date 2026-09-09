// MJPEG (multipart/x-mixed-replace) wire framing - the browser-fallback video
// transport from AGENTS.md section 4. Pure string/byte building so the exact
// wire format is unit-testable without a socket or a browser.

/// Part boundary. Must not appear in JPEG payloads (it cannot: it is ASCII text
/// and parts are length-delimited by Content-Length).
pub const BOUNDARY: &str = "extendoframe";

/// Response headers for the streaming endpoint.
///
/// `Cache-Control`/`Pragma` stop proxies and Android WebViews from buffering the
/// stream, which otherwise shows up as seconds of apparent latency.
#[must_use]
pub fn stream_headers() -> String {
    format!(
        "HTTP/1.1 200 OK\r\n\
         Content-Type: multipart/x-mixed-replace; boundary={BOUNDARY}\r\n\
         Cache-Control: no-store, no-cache, must-revalidate, private\r\n\
         Pragma: no-cache\r\n\
         Connection: close\r\n\
         Access-Control-Allow-Origin: *\r\n\
         \r\n"
    )
}

/// Per-frame part header. Sent immediately before the raw JPEG bytes.
#[must_use]
pub fn part_header(jpeg_len: usize) -> String {
    format!(
        "--{BOUNDARY}\r\n\
         Content-Type: image/jpeg\r\n\
         Content-Length: {jpeg_len}\r\n\
         \r\n"
    )
}

/// Trailing CRLF after a part's payload, before the next boundary.
pub const PART_TRAILER: &[u8] = b"\r\n";

/// A complete single-image response (`GET /frame.jpg`), used by smoke tests and
/// by clients that cannot hold a streaming connection open.
#[must_use]
pub fn single_image_headers(jpeg_len: usize) -> String {
    format!(
        "HTTP/1.1 200 OK\r\n\
         Content-Type: image/jpeg\r\n\
         Content-Length: {jpeg_len}\r\n\
         Cache-Control: no-store\r\n\
         Access-Control-Allow-Origin: *\r\n\
         Connection: close\r\n\
         \r\n"
    )
}

/// Minimal JSON/text response used for `/health` and error replies.
#[must_use]
pub fn text_response(status: u16, reason: &str, content_type: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status} {reason}\r\n\
         Content-Type: {content_type}\r\n\
         Content-Length: {}\r\n\
         Cache-Control: no-store\r\n\
         Access-Control-Allow-Origin: *\r\n\
         Connection: close\r\n\
         \r\n{body}",
        body.len()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_headers_declare_multipart_with_boundary() {
        let h = stream_headers();
        assert!(h.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(h.contains("Content-Type: multipart/x-mixed-replace; boundary=extendoframe\r\n"));
        // Must terminate the header block exactly once.
        assert!(h.ends_with("\r\n\r\n"));
    }

    #[test]
    fn stream_headers_disable_caching() {
        // Without these, WebView/proxy buffering masquerades as latency.
        let h = stream_headers();
        assert!(h.contains("Cache-Control: no-store"));
        assert!(h.contains("Pragma: no-cache"));
    }

    #[test]
    fn part_header_carries_boundary_and_length() {
        let p = part_header(1234);
        assert!(p.starts_with("--extendoframe\r\n"));
        assert!(p.contains("Content-Type: image/jpeg\r\n"));
        assert!(p.contains("Content-Length: 1234\r\n"));
        assert!(p.ends_with("\r\n\r\n"));
    }

    #[test]
    fn single_image_headers_report_exact_length() {
        let h = single_image_headers(42);
        assert!(h.contains("Content-Length: 42\r\n"));
        assert!(h.contains("Content-Type: image/jpeg\r\n"));
    }

    #[test]
    fn text_response_length_matches_body_bytes() {
        let body = "{\"ok\":true}";
        let r = text_response(200, "OK", "application/json", body);
        assert!(r.contains(&format!("Content-Length: {}\r\n", body.len())));
        assert!(r.ends_with(body));
    }

    #[test]
    fn text_response_supports_error_statuses() {
        let r = text_response(401, "Unauthorized", "application/json", "{}");
        assert!(r.starts_with("HTTP/1.1 401 Unauthorized\r\n"));
    }
}
