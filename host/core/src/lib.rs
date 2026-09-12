// extendo capture core (Rust) - AGENTS.md section 4 host core.
//
// Pipeline: Windows Graphics Capture -> downscale/RGB -> JPEG -> MJPEG over HTTP,
// or WGC -> NV12 -> Media Foundation H.264 -> Annex B over HTTP (`--codec h264`).
// MJPEG is the sanctioned browser-fallback transport (AGENTS.md section 4:
// "WebRTC/MJPEG only for browser fallback"); native Android uses the H.264
// bytestream (UDP/RTP packetizing reuses `capture`, `frame` and `h264`).
//
// Entitlement note (AGENTS.md section 10.1): this crate contains NO paywall
// logic. Caps arrive as CLI flags from the Node host, which owns the single
// `Entitlement` source of truth in host/src/license.js.

pub mod capture;
pub mod cli;
pub mod frame;
pub mod h264;
pub mod mf;
pub mod mjpeg;
pub mod server;

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// One encoded JPEG frame handed from the capture thread to HTTP clients.
#[derive(Debug, Clone)]
pub struct JpegFrame {
    pub bytes: Arc<Vec<u8>>,
    pub seq: u64,
    pub width: u32,
    pub height: u32,
    /// Encode cost, to check against the AGENTS.md section 7.3 budget (<12ms).
    pub encode_ms: f32,
}

/// One encoded H.264 access unit (Annex B, start codes intact, SPS/PPS
/// prefixed to keyframes so mid-stream join works).
#[derive(Debug, Clone)]
pub struct H264Frame {
    pub bytes: Arc<Vec<u8>>,
    pub seq: u64,
    pub width: u32,
    pub height: u32,
    pub keyframe: bool,
    /// Encode cost, to check against the AGENTS.md section 7.3 budget (<12ms).
    pub encode_ms: f32,
}

/// Whatever the capture thread most recently produced. Slow clients skip
/// stale values by design (watch keeps only the latest).
#[derive(Debug, Clone)]
pub enum VideoFrame {
    Mjpeg(JpegFrame),
    H264(H264Frame),
}

/// Process-wide counters surfaced by `GET /health` for the RTT/fps overlay and
/// for `tools/diag-collect.ps1`.
#[derive(Debug, Default)]
pub struct Stats {
    pub frames_captured: AtomicU64,
    pub frames_encoded: AtomicU64,
    pub frames_dropped: AtomicU64,
    pub clients: AtomicUsize,
    /// Last encode duration in microseconds (avoids float atomics).
    pub last_encode_us: AtomicU64,
    pub last_capture_us: AtomicU64,
    /// Last encoder error (empty = healthy). Surfaced in `GET /health` so the
    /// Node host and `tools/diag-collect.ps1` can show it without log access.
    pub last_error: Mutex<String>,
}

impl Stats {
    /// Record an encoder failure (capture keeps running; clients see it via
    /// `/health` and get 503 on frame endpoints until frames flow again).
    pub fn set_error(&self, msg: &str) {
        if let Ok(mut slot) = self.last_error.lock() {
            *slot = msg.to_string();
        }
    }

    /// Clear a previous failure once frames flow again.
    pub fn clear_error(&self) {
        if let Ok(mut slot) = self.last_error.lock() {
            slot.clear();
        }
    }

    fn error_json(&self) -> String {
        let msg = self.last_error.lock().map(|s| s.clone()).unwrap_or_default();
        format!("\"{}\"", msg.replace('\\', "\\\\").replace('"', "'").replace('\n', " "))
    }
    pub fn snapshot_json(&self, cfg: &cli::Args, source: &str) -> String {
        let enc_ms = self.last_encode_us.load(Ordering::Relaxed) as f64 / 1000.0;
        let cap_ms = self.last_capture_us.load(Ordering::Relaxed) as f64 / 1000.0;
        format!(
            concat!(
                "{{\"ok\":true,\"source\":\"{}\",\"monitor\":{},\"width\":{},\"height\":{},",
                "\"fps\":{},\"quality\":{},\"codec\":\"{}\",\"bitrate_kbps\":{},",
                "\"captured\":{},\"encoded\":{},\"dropped\":{},",
                "\"clients\":{},\"encode_ms\":{:.2},\"capture_ms\":{:.2},\"error\":{}}}",
            ),
            source,
            cfg.monitor,
            cfg.width,
            cfg.height,
            cfg.fps,
            cfg.quality,
            cfg.codec,
            cfg.bitrate_kbps,
            self.frames_captured.load(Ordering::Relaxed),
            self.frames_encoded.load(Ordering::Relaxed),
            self.frames_dropped.load(Ordering::Relaxed),
            self.clients.load(Ordering::Relaxed),
            enc_ms,
            cap_ms,
            self.error_json(),
        )
    }
}
