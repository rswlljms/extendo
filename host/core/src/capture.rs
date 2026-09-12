// Windows Graphics Capture glue. Deliberately thin: everything that can be
// unit-tested lives in frame.rs, because this file needs a real GPU, a real
// desktop and a real monitor to run at all (AGENTS.md section 7.8 - the
// verifiable work is in the pure modules; this path is smoke-tested live).

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::watch;
use windows_capture::capture::{Context, GraphicsCaptureApiHandler};
use windows_capture::frame::Frame;
use windows_capture::graphics_capture_api::InternalCaptureControl;
use windows_capture::monitor::Monitor;
use windows_capture::settings::{
    ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
    MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
};

use crate::cli::Args;
use crate::frame::{downscale_rgba_to_nv12, downscale_to_rgb, encode_jpeg, fit_within};
use crate::h264::H264Config;
use crate::mf::MfH264Encoder;
use crate::{H264Frame, JpegFrame, Stats, VideoFrame};
use std::cell::RefCell;

// The MF encoder lives here - not in `CaptureHandler` - because the
// `IMFTransform` COM pointer is not `Send` and the handler crosses threads
// into `Capture::start`.
thread_local! {
    static H264_ENCODER: RefCell<Option<MfH264Encoder>> = const { RefCell::new(None) };
}

/// Everything the capture callback needs, handed over via `Settings` flags.
pub struct CaptureFlags {
    pub cfg: Args,
    pub tx: watch::Sender<Option<VideoFrame>>,
    pub stats: Arc<Stats>,
}

pub struct CaptureHandler {
    cfg: Args,
    tx: watch::Sender<Option<VideoFrame>>,
    stats: Arc<Stats>,
    seq: u64,
    last_sent: Option<Instant>,
    /// H.264 init backoff: a missing/broken MFT must not log once per frame.
    /// `None` = encoder healthy (or not yet attempted).
    retry_after: Option<Instant>,
}

impl GraphicsCaptureApiHandler for CaptureHandler {
    type Flags = CaptureFlags;
    type Error = Box<dyn std::error::Error + Send + Sync>;

    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        Ok(Self {
            cfg: ctx.flags.cfg,
            tx: ctx.flags.tx,
            stats: ctx.flags.stats,
            seq: 0,
            last_sent: None,
            retry_after: None,
        })
    }

    fn on_frame_arrived(
        &mut self,
        frame: &mut Frame,
        _capture_control: InternalCaptureControl,
    ) -> Result<(), Self::Error> {
        let t_arrive = Instant::now();
        self.stats.frames_captured.fetch_add(1, Ordering::Relaxed);

        // WGC delivers on change, which can exceed the negotiated fps. Drop
        // early (before the expensive copy+encode) to hold the rate.
        if let Some(last) = self.last_sent {
            if t_arrive.duration_since(last) < self.cfg.frame_interval() {
                self.stats.frames_dropped.fetch_add(1, Ordering::Relaxed);
                return Ok(());
            }
        }
        // Nobody is watching: skip the encode entirely rather than burn CPU.
        if self.stats.clients.load(Ordering::Relaxed) == 0 {
            return Ok(());
        }

        let mut buffer = frame.buffer()?;
        let src_w = buffer.width();
        let src_h = buffer.height();
        // Use the padded GPU buffer as-is: row_pitch is the stride and the
        // downscale helpers handle it, which avoids a full-frame de-padding
        // copy per frame (that copy costs more than the encode at 1080p).
        let stride = buffer.row_pitch() as usize;
        let (dst_w, dst_h) = fit_within(src_w, src_h, self.cfg.width, self.cfg.height);
        let raw = buffer.as_raw_buffer();
        if self.cfg.codec == "h264" {
            self.encode_h264(H264Input { raw, src_w, src_h, stride, dst_w, dst_h, t_arrive });
            return Ok(());
        }
        // ColorFormat::Rgba8 => channels already R,G,B,A; no swap needed.
        let rgb = downscale_to_rgb(raw, src_w, src_h, stride, dst_w, dst_h, false);
        self.stats
            .last_capture_us
            .store(t_arrive.elapsed().as_micros() as u64, Ordering::Relaxed);

        let t_enc = Instant::now();
        let jpeg = encode_jpeg(&rgb, dst_w, dst_h, self.cfg.quality)?;
        let encode_us = t_enc.elapsed().as_micros() as u64;
        self.stats.last_encode_us.store(encode_us, Ordering::Relaxed);

        self.seq += 1;
        self.last_sent = Some(t_arrive);
        self.stats.frames_encoded.fetch_add(1, Ordering::Relaxed);

        // watch = "latest value wins": slow clients skip stale frames instead of
        // building a queue, which is what we want for live video.
        let _ = self.tx.send(Some(VideoFrame::Mjpeg(JpegFrame {
            bytes: Arc::new(jpeg),
            seq: self.seq,
            width: dst_w,
            height: dst_h,
            encode_ms: encode_us as f32 / 1000.0,
        })));
        Ok(())
    }

    fn on_closed(&mut self) -> Result<(), Self::Error> {
        eprintln!("capture: session closed by the system");
        Ok(())
    }
}

/// Inputs to one H.264 frame encode (bundled so clippy's arg-count lint
/// stays quiet and call sites stay readable).
struct H264Input<'a> {
    raw: &'a [u8],
    src_w: u32,
    src_h: u32,
    stride: usize,
    dst_w: u32,
    dst_h: u32,
    t_arrive: Instant,
}

impl CaptureHandler {
    /// H.264 branch: NV12 convert (one pass) -> MF baseline encode -> one
    /// watch update per access unit. Never returns Err: a failed frame is a
    /// drop, not a dead capture session; the cause stays visible in /health.
    fn encode_h264(&mut self, input: H264Input<'_>) {
        // NV12 needs even dimensions; fit_within can hand us odd ones.
        let w = (input.dst_w & !1).max(2);
        let h = (input.dst_h & !1).max(2);
        let nv12 = downscale_rgba_to_nv12(input.raw, input.src_w, input.src_h, input.stride, w, h);
        self.stats
            .last_capture_us
            .store(input.t_arrive.elapsed().as_micros() as u64, Ordering::Relaxed);

        // (Re)create the encoder when missing or when the resolution changed.
        let mut init_err: Option<String> = None;
        let t_enc = Instant::now();
        let units = H264_ENCODER.with(|cell| {
            let mut slot = cell.borrow_mut();
            let dims_ok = slot.as_ref().is_some_and(|e| e.dims() == (w, h));
            if !dims_ok && self.retry_after.is_none_or(|t| Instant::now() >= t) {
                let enc_cfg = H264Config {
                    width: w,
                    height: h,
                    fps: self.cfg.fps,
                    bitrate_kbps: self.cfg.bitrate_kbps,
                    ..H264Config::default()
                };
                match MfH264Encoder::new(&enc_cfg) {
                    Ok(enc) => {
                        *slot = Some(enc);
                        self.retry_after = None;
                        self.stats.clear_error();
                        eprintln!("capture: MF H.264 encoder ready ({w}x{h}@{} {}kbps)", self.cfg.fps, self.cfg.bitrate_kbps);
                    }
                    Err(e) => {
                        *slot = None;
                        // Cool down: a missing/broken MFT must not log per frame.
                        self.retry_after = Some(Instant::now() + std::time::Duration::from_secs(5));
                        self.stats.set_error(&e);
                        init_err = Some(e);
                    }
                }
            }
            let enc = slot.as_mut()?;
            match enc.encode(&nv12, w, h) {
                Ok(u) => Some(Ok(u)),
                Err(e) => {
                    // Resolution switches surface as dim mismatches: drop the
                    // encoder so the next frame recreates it at the new size.
                    *slot = None;
                    self.retry_after = None;
                    Some(Err(e))
                }
            }
        });
        if let Some(e) = init_err {
            eprintln!("capture: H.264 init failed ({e}); retrying in 5s");
            return;
        }
        let units = match units {
            Some(Ok(u)) => u,
            Some(Err(e)) => {
                self.stats.frames_dropped.fetch_add(1, Ordering::Relaxed);
                self.stats.set_error(&e);
                // Genuine failures are rare (init path is throttled already);
                // log them — /health alone loses them once recovery clears it.
                eprintln!("capture: H.264 encode failed ({e}); encoder recreated next frame");
                return;
            }
            None => return, // encoder missing and cooling down
        };
        let encode_us = t_enc.elapsed().as_micros() as u64;
        self.stats.last_encode_us.store(encode_us, Ordering::Relaxed);
        if units.is_empty() {
            return; // encoder buffered (stream start); still count the capture
        }

        self.last_sent = Some(input.t_arrive);
        for u in units {
            self.seq += 1;
            self.stats.frames_encoded.fetch_add(1, Ordering::Relaxed);
            let _ = self.tx.send(Some(VideoFrame::H264(H264Frame {
                bytes: Arc::new(u.bytes),
                seq: self.seq,
                width: w,
                height: h,
                keyframe: u.keyframe,
                encode_ms: encode_us as f32 / 1000.0,
            })));
        }
        self.stats.clear_error();
    }
}

/// Resolve the monitor to capture. `0` means primary; otherwise a 1-based index
/// matching `Monitor::enumerate` order.
///
/// # Errors
/// Returns a message when the monitor does not exist.
pub fn resolve_monitor(index: usize) -> Result<Monitor, String> {
    if index == 0 {
        Monitor::primary().map_err(|e| format!("no primary monitor: {e}"))
    } else {
        Monitor::from_index(index).map_err(|e| format!("no monitor at index {index}: {e}"))
    }
}

/// Start capture on a dedicated OS thread.
///
/// `Capture::start` takes over its thread, so it gets its own; the tokio
/// runtime keeps the main thread for the HTTP server.
pub fn spawn(
    monitor: Monitor,
    cfg: Args,
    tx: watch::Sender<Option<VideoFrame>>,
    stats: Arc<Stats>,
) -> std::io::Result<std::thread::JoinHandle<()>> {
    std::thread::Builder::new()
        .name("extendo-capture".to_string())
        .spawn(move || {
            let settings = Settings::new(
                monitor,
                CursorCaptureSettings::Default,
                DrawBorderSettings::Default,
                SecondaryWindowSettings::Default,
                MinimumUpdateIntervalSettings::Default,
                DirtyRegionSettings::Default,
                ColorFormat::Rgba8,
                CaptureFlags { cfg, tx, stats },
            );
            if let Err(e) = CaptureHandler::start(settings) {
                eprintln!("capture: fatal: {e}");
            }
        })
}
