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
use crate::frame::{downscale_to_rgb, encode_jpeg, fit_within};
use crate::{JpegFrame, Stats};

/// Everything the capture callback needs, handed over via `Settings` flags.
pub struct CaptureFlags {
    pub cfg: Args,
    pub tx: watch::Sender<Option<JpegFrame>>,
    pub stats: Arc<Stats>,
}

pub struct CaptureHandler {
    cfg: Args,
    tx: watch::Sender<Option<JpegFrame>>,
    stats: Arc<Stats>,
    seq: u64,
    last_sent: Option<Instant>,
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
        // Use the padded GPU buffer as-is: row_pitch is the stride and
        // downscale_to_rgb handles it, which avoids a full-frame de-padding
        // copy per frame (that copy costs more than the JPEG encode at 1080p).
        let stride = buffer.row_pitch() as usize;
        let (dst_w, dst_h) = fit_within(src_w, src_h, self.cfg.width, self.cfg.height);
        let raw = buffer.as_raw_buffer();
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
        let _ = self.tx.send(Some(JpegFrame {
            bytes: Arc::new(jpeg),
            seq: self.seq,
            width: dst_w,
            height: dst_h,
            encode_ms: encode_us as f32 / 1000.0,
        }));
        Ok(())
    }

    fn on_closed(&mut self) -> Result<(), Self::Error> {
        eprintln!("capture: session closed by the system");
        Ok(())
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
    tx: watch::Sender<Option<JpegFrame>>,
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
