// extendo capture core (Rust) - AGENTS.md section 4 host core.
//
// Pipeline: Windows Graphics Capture -> downscale/RGB -> JPEG -> MJPEG over HTTP.
// MJPEG is the sanctioned browser-fallback transport (AGENTS.md section 4:
// "WebRTC/MJPEG only for browser fallback"); the native Android client gets
// H.264 over UDP/RTP later, reusing `capture` and `frame` unchanged.
//
// Entitlement note (AGENTS.md section 10.1): this crate contains NO paywall
// logic. Caps arrive as CLI flags from the Node host, which owns the single
// `Entitlement` source of truth in host/src/license.js.

pub mod capture;
pub mod cli;
pub mod frame;
pub mod mjpeg;
pub mod server;

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;

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
}

impl Stats {
    pub fn snapshot_json(&self, cfg: &cli::Args, source: &str) -> String {
        let enc_ms = self.last_encode_us.load(Ordering::Relaxed) as f64 / 1000.0;
        let cap_ms = self.last_capture_us.load(Ordering::Relaxed) as f64 / 1000.0;
        format!(
            concat!(
                "{{\"ok\":true,\"source\":\"{}\",\"monitor\":{},\"width\":{},\"height\":{},",
                "\"fps\":{},\"quality\":{},\"captured\":{},\"encoded\":{},\"dropped\":{},",
                "\"clients\":{},\"encode_ms\":{:.2},\"capture_ms\":{:.2}}}"
            ),
            source,
            cfg.monitor,
            cfg.width,
            cfg.height,
            cfg.fps,
            cfg.quality,
            self.frames_captured.load(Ordering::Relaxed),
            self.frames_encoded.load(Ordering::Relaxed),
            self.frames_dropped.load(Ordering::Relaxed),
            self.clients.load(Ordering::Relaxed),
            enc_ms,
            cap_ms,
        )
    }
}
