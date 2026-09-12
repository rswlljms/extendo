// Media Foundation H.264 encoder (AGENTS.md section 4 MF fallback path).
//
// Thin wrapper around the inbox `CMSH264EncoderMFT`: NV12 in, Annex B H.264
// out (baseline, no B-frames, CBR, low-latency mode). All COM/MF calls live
// here; config validation and Annex B parsing stay in the pure, unit-tested
// `h264` module. Like `capture.rs`, this file needs a real desktop + the
// inbox encoder, so it is smoke-tested live (`GET /health` counters plus NAL
// inspection of `/video.h264`).
//
// Threading: the encoder is created AND driven on ONE thread (the capture
// thread, lazily on the first frame — the `IMFTransform` COM pointer is not
// `Send`, so it lives in a thread-local in capture.rs, never in a struct
// that crosses threads). COM is initialized MTA on first use.
//
// windows-crate note (0.62): codecapi GUIDs carry a `CODECAPI_` prefix and
// the `CLSID_` prefix is dropped (`CMSH264EncoderMFT`); factory functions
// return `Result<T>` directly; `ICodecAPI::SetValue` needs the
// `Win32_System_Ole` feature.

use crate::h264::H264Config;

/// One encoded access unit (usually one picture; Annex B, start codes intact).
#[derive(Debug, Clone)]
pub struct EncodedUnit {
    pub bytes: Vec<u8>,
    pub keyframe: bool,
}

#[cfg(windows)]
mod imp {
    use super::*;
    use crate::h264::is_keyframe_access_unit;
    use std::sync::Once;
    use windows::Win32::Foundation::{VARIANT_FALSE, VARIANT_TRUE};
    use windows::Win32::Media::MediaFoundation::*;
    use windows::Win32::System::Com::*;
    use windows::Win32::System::Variant::*;
    // NOTE: no `windows::core::*` glob — it shadows std `Result` with
    // `windows::core::Result`, which breaks every `-> Result<_, String>`.
    use windows::core::{GUID, Interface};

    static MF_START: Once = Once::new();

    fn mf_startup() -> Result<(), String> {
        let mut err: Option<String> = None;
        MF_START.call_once(|| {
            // MTA: the capture thread has no message pump for STA anyway.
            // Someone (WGC) may have initialized COM already; that is fine.
            unsafe {
                let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
                if let Err(e) = MFStartup(MF_VERSION, MFSTARTUP_FULL) {
                    err = Some(format!("MFStartup failed: {}", describe(e)));
                }
            }
        });
        err.map_or(Ok(()), Err)
    }

    pub(super) fn describe(e: windows::core::Error) -> String {
        format!("{e} (0x{:08X})", e.code().0 as u32)
    }

    unsafe fn uint_var(v: u32) -> VARIANT {
        let mut var = std::mem::zeroed::<VARIANT>();
        // Union fields need explicit derefs: no auto-DerefMut through
        // `ManuallyDrop` on a union member.
        let inner = &mut *var.Anonymous.Anonymous;
        inner.vt = VT_UI4;
        inner.Anonymous.ulVal = v;
        var
    }

    unsafe fn bool_var(v: bool) -> VARIANT {
        let mut var = std::mem::zeroed::<VARIANT>();
        let inner = &mut *var.Anonymous.Anonymous;
        inner.vt = VT_BOOL;
        inner.Anonymous.boolVal = if v { VARIANT_TRUE } else { VARIANT_FALSE };
        var
    }

    fn set_codec_opt(api: &ICodecAPI, guid: &GUID, var: &VARIANT, what: &str) -> Result<(), String> {
        unsafe { api.SetValue(guid, var) }
            .map_err(|e| format!("ICodecAPI {what} failed: {}", describe(e)))
    }

    fn media_type_nv12(cfg: &H264Config) -> Result<IMFMediaType, String> {
        unsafe {
            let ty = MFCreateMediaType().map_err(|e| format!("MFCreateMediaType: {}", describe(e)))?;
            ty.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)
                .map_err(|e| format!("input major type: {}", describe(e)))?;
            ty.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_NV12)
                .map_err(|e| format!("input subtype NV12: {}", describe(e)))?;
            ty.SetUINT64(&MF_MT_FRAME_SIZE, ((cfg.width as u64) << 32) | cfg.height as u64)
                .map_err(|e| format!("input frame size: {}", describe(e)))?;
            ty.SetUINT64(&MF_MT_FRAME_RATE, ((cfg.fps as u64) << 32) | 1)
                .map_err(|e| format!("input frame rate: {}", describe(e)))?;
            // 2 = progressive (MFVideoInterlace_Progressive).
            ty.SetUINT32(&MF_MT_INTERLACE_MODE, 2)
                .map_err(|e| format!("input interlace mode: {}", describe(e)))?;
            Ok(ty)
        }
    }

    fn media_type_h264(cfg: &H264Config) -> Result<IMFMediaType, String> {
        unsafe {
            let ty = MFCreateMediaType().map_err(|e| format!("MFCreateMediaType: {}", describe(e)))?;
            ty.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)
                .map_err(|e| format!("output major type: {}", describe(e)))?;
            ty.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_H264)
                .map_err(|e| format!("output subtype H264: {}", describe(e)))?;
            ty.SetUINT64(&MF_MT_FRAME_SIZE, ((cfg.width as u64) << 32) | cfg.height as u64)
                .map_err(|e| format!("output frame size: {}", describe(e)))?;
            ty.SetUINT64(&MF_MT_FRAME_RATE, ((cfg.fps as u64) << 32) | 1)
                .map_err(|e| format!("output frame rate: {}", describe(e)))?;
            ty.SetUINT32(&MF_MT_AVG_BITRATE, cfg.bitrate_kbps * 1000)
                .map_err(|e| format!("output bitrate: {}", describe(e)))?;
            ty.SetUINT32(&MF_MT_INTERLACE_MODE, 2)
                .map_err(|e| format!("output interlace mode: {}", describe(e)))?;
            // 66 = Baseline (eAVEncH264VProfile_Base); Main would be 77.
            // This is the documented profile mechanism for the H264 MFT, so
            // no separate CODECAPI profile call is needed.
            ty.SetUINT32(&MF_MT_MPEG2_PROFILE, 66)
                .map_err(|e| format!("output profile: {}", describe(e)))?;
            Ok(ty)
        }
    }

    pub struct MfH264Encoder {
        enc: IMFTransform,
        /// SPS/PPS blob (MF_MT_MPEG_SEQUENCE_HEADER), prepended to every
        /// keyframe so a client joining mid-stream can start decoding.
        seq_header: Vec<u8>,
        width: u32,
        height: u32,
        next_time_100ns: i64,
        dur_100ns: i64,
        out_buf_size: u32,
        warned_raw: bool,
    }

    impl MfH264Encoder {
        pub fn new(cfg: &H264Config) -> Result<Self, String> {
            cfg.validate()?;
            mf_startup()?;
            unsafe {
                let enc: IMFTransform =
                    CoCreateInstance(&CMSH264EncoderMFT, None, CLSCTX_INPROC_SERVER)
                        .map_err(|e| format!("H264 encoder MFT not available: {}", describe(e)))?;

                // Encoder MFTs resolve input requirements FROM the output type,
                // so the output MUST be set first (MF_E_INVALIDTYPE otherwise).
                enc.SetOutputType(0, &media_type_h264(cfg)?, 0)
                    .map_err(|e| format!("SetOutputType H264: {}", describe(e)))?;
                enc.SetInputType(0, &media_type_nv12(cfg)?, 0)
                    .map_err(|e| format!("SetInputType NV12: {}", describe(e)))?;

                // Latency/quality contract (AGENTS.md 7.3: no B-frames, <12ms
                // encode, CBR for a predictable LAN bitrate).
                let api: ICodecAPI = enc
                    .cast()
                    .map_err(|e| format!("ICodecAPI unavailable: {}", describe(e)))?;
                set_codec_opt(
                    &api,
                    &CODECAPI_AVEncCommonRateControlMode,
                    &uint_var(eAVEncCommonRateControlMode_CBR.0 as u32),
                    "rate control (CBR)",
                )?;
                set_codec_opt(
                    &api,
                    &CODECAPI_AVEncCommonMeanBitRate,
                    &uint_var(cfg.bitrate_kbps * 1000),
                    "mean bitrate",
                )?;
                set_codec_opt(
                    &api,
                    &CODECAPI_AVEncMPVDefaultBPictureCount,
                    &uint_var(0),
                    "B-frames=0",
                )?;
                set_codec_opt(&api, &CODECAPI_AVEncMPVGOPSize, &uint_var(cfg.gop_len), "GOP")?;
                // Best-effort: older inbox encoders may not expose it.
                let _ = api.SetValue(&CODECAPI_AVLowLatencyMode, &bool_var(true));

                enc.ProcessMessage(MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, 0)
                    .map_err(|e| format!("begin streaming: {}", describe(e)))?;

                // Stash the sequence header for keyframe prefixing.
                let mut seq_header = Vec::new();
                if let Ok(cur) = enc.GetOutputCurrentType(0) {
                    let mut blob = vec![0u8; 4096];
                    let mut len = 0u32;
                    if cur
                        .GetBlob(&MF_MT_MPEG_SEQUENCE_HEADER, &mut blob, Some(&mut len))
                        .is_ok()
                    {
                        blob.truncate(len as usize);
                        seq_header = blob;
                    }
                }

                Ok(Self {
                    enc,
                    seq_header,
                    width: cfg.width,
                    height: cfg.height,
                    next_time_100ns: 0,
                    dur_100ns: 10_000_000 / cfg.fps.max(1) as i64,
                    // First IDR (SPS/PPS + full picture) is the biggest unit;
                    // start at 2x frame size so stream start needs no regrow.
                    // BUFFERTOOSMALL below is the backstop, not the plan.
                    out_buf_size: cfg.width * cfg.height * 3 + 65536,
                    warned_raw: false,
                })
            }
        }

        pub fn dims(&self) -> (u32, u32) {
            (self.width, self.height)
        }

        pub fn encode(&mut self, nv12: &[u8], w: u32, h: u32) -> Result<Vec<EncodedUnit>, String> {
            if (w, h) != (self.width, self.height) {
                return Err(format!(
                    "frame {w}x{h} != encoder {}x{} (recreate on resolution change)",
                    self.width, self.height
                ));
            }
            let need = (w as usize) * (h as usize) * 3 / 2;
            if nv12.len() < need {
                return Err(format!("NV12 buffer {} bytes, need {need}", nv12.len()));
            }
            unsafe {
                // Feed one input frame...
                let in_buf =
                    MFCreateMemoryBuffer(need as u32).map_err(|e| format!("input buffer: {}", describe(e)))?;
                {
                    let mut ptr = std::ptr::null_mut::<u8>();
                    in_buf
                        .Lock(&mut ptr, None, None)
                        .map_err(|e| format!("input lock: {}", describe(e)))?;
                    std::ptr::copy_nonoverlapping(nv12.as_ptr(), ptr, need);
                    in_buf
                        .SetCurrentLength(need as u32)
                        .map_err(|e| format!("input length: {}", describe(e)))?;
                    in_buf.Unlock().map_err(|e| format!("input unlock: {}", describe(e)))?;
                }
                let sample = MFCreateSample().map_err(|e| format!("input sample: {}", describe(e)))?;
                sample
                    .AddBuffer(&in_buf)
                    .map_err(|e| format!("input add buffer: {}", describe(e)))?;
                sample
                    .SetSampleTime(self.next_time_100ns)
                    .map_err(|e| format!("input timestamp: {}", describe(e)))?;
                sample
                    .SetSampleDuration(self.dur_100ns)
                    .map_err(|e| format!("input duration: {}", describe(e)))?;
                self.next_time_100ns += self.dur_100ns;

                match self.enc.ProcessInput(0, &sample, 0) {
                    Ok(()) => {}
                    // Output queue full: drain it, then retry once.
                    Err(e)
                        if e.code() == MF_E_TRANSFORM_NEED_MORE_INPUT
                            || e.code() == MF_E_NOTACCEPTING =>
                    {
                        self.drain()?;
                        match self.enc.ProcessInput(0, &sample, 0) {
                            Ok(()) => {}
                            // Still warming up (rate-control lookahead): keep
                            // the encoder, skip this frame, feed the next.
                            // Treating this as fatal recreates the MFT and it
                            // never accumulates enough frames to emit.
                            Err(e)
                                if e.code() == MF_E_TRANSFORM_NEED_MORE_INPUT
                                    || e.code() == MF_E_NOTACCEPTING =>
                            {
                                return Ok(Vec::new());
                            }
                            Err(e) => {
                                return Err(format!("ProcessInput (retry): {}", describe(e)));
                            }
                        }
                    }
                    Err(e) => return Err(format!("ProcessInput: {}", describe(e))),
                }
                let mut units = self.drain()?;
                // Mid-stream join: every keyframe carries SPS/PPS up front.
                // (Duplicated sets are legal; decoders apply the latest.)
                for u in &mut units {
                    if u.keyframe && !self.seq_header.is_empty() {
                        let mut prefixed = Vec::with_capacity(self.seq_header.len() + u.bytes.len());
                        prefixed.extend_from_slice(&self.seq_header);
                        prefixed.append(&mut u.bytes);
                        u.bytes = prefixed;
                    }
                    if !starts_with_annexb(&u.bytes) {
                        if !self.warned_raw {
                            self.warned_raw = true;
                            eprintln!("mf: encoder emitted a sample without Annex B start codes; prefixing");
                        }
                        let mut fixed = vec![0, 0, 0, 1];
                        fixed.append(&mut u.bytes);
                        u.bytes = fixed;
                    }
                }
                Ok(units)
            }
        }

        /// Pull all pending output. `MF_E_TRANSFORM_NEED_MORE_INPUT` is the
        /// normal drain terminator, not an error.
        fn drain(&mut self) -> Result<Vec<EncodedUnit>, String> {
            let mut units = Vec::new();
            unsafe {
                loop {
                    let out_sample =
                        MFCreateSample().map_err(|e| format!("output sample: {}", describe(e)))?;
                    let out_buf = MFCreateMemoryBuffer(self.out_buf_size)
                        .map_err(|e| format!("output buffer: {}", describe(e)))?;
                    out_sample
                        .AddBuffer(&out_buf)
                        .map_err(|e| format!("output add buffer: {}", describe(e)))?;

                    let output = MFT_OUTPUT_DATA_BUFFER {
                        dwStreamID: 0,
                        pSample: std::mem::ManuallyDrop::new(Some(out_sample)),
                        dwStatus: 0,
                        pEvents: std::mem::ManuallyDrop::new(None),
                    };
                    let mut status = 0u32;
                    let mut outputs = [output];
                    match self.enc.ProcessOutput(0, &mut outputs, &mut status) {
                        Ok(()) => {}
                        Err(e) if e.code() == MF_E_TRANSFORM_NEED_MORE_INPUT => break,
                        Err(e) if e.code() == MF_E_BUFFERTOOSMALL => {
                            // Documented handling: grow and retry. Cap at 32MB;
                            // beyond that something structural is wrong.
                            const MAX_OUT: u32 = 32 * 1024 * 1024;
                            if self.out_buf_size >= MAX_OUT {
                                return Err(format!(
                                    "ProcessOutput still BUFFERTOOSMALL at {}MB",
                                    MAX_OUT / (1024 * 1024)
                                ));
                            }
                            self.out_buf_size = (self.out_buf_size * 2).min(MAX_OUT);
                            eprintln!("mf: output buffer too small, grew to {}KB", self.out_buf_size / 1024);
                            continue;
                        }
                        Err(e) if e.code() == MF_E_TRANSFORM_STREAM_CHANGE => {
                            // Encoder renegotiated (e.g. first frame): adopt
                            // the new output type and keep draining.
                            let avail = self
                                .enc
                                .GetOutputAvailableType(0, 0)
                                .map_err(|e| format!("available output type: {}", describe(e)))?;
                            self.enc
                                .SetOutputType(0, &avail, 0)
                                .map_err(|e| format!("renegotiate output: {}", describe(e)))?;
                            continue;
                        }
                        Err(e) => return Err(format!("ProcessOutput: {}", describe(e))),
                    }
                    let Some(sample) = outputs[0].pSample.take() else {
                        break;
                    };
                    let bytes = contiguous_bytes(&sample)?;
                    if bytes.is_empty() {
                        continue;
                    }
                    let keyframe = is_keyframe_access_unit(&bytes);
                    units.push(EncodedUnit { bytes, keyframe });
                    // Synchronous MFTs usually return one AU per call, but
                    // keep draining until the MFT says it needs more input.
                }
            }
            Ok(units)
        }
    }

    impl Drop for MfH264Encoder {
        fn drop(&mut self) {
            unsafe {
                let _ = self.enc.ProcessMessage(MFT_MESSAGE_NOTIFY_END_OF_STREAM, 0);
                let _ = self.enc.ProcessMessage(MFT_MESSAGE_NOTIFY_END_STREAMING, 0);
            }
        }
    }

    unsafe fn contiguous_bytes(sample: &IMFSample) -> Result<Vec<u8>, String> {
        let contig = sample
            .ConvertToContiguousBuffer()
            .map_err(|e| format!("contiguous buffer: {}", describe(e)))?;
        let mut ptr = std::ptr::null_mut::<u8>();
        let mut len = 0u32;
        contig
            .Lock(&mut ptr, None, Some(&mut len))
            .map_err(|e| format!("output lock: {}", describe(e)))?;
        let bytes = std::slice::from_raw_parts(ptr, len as usize).to_vec();
        contig
            .Unlock()
            .map_err(|e| format!("output unlock: {}", describe(e)))?;
        Ok(bytes)
    }

    fn starts_with_annexb(b: &[u8]) -> bool {
        (b.len() >= 3 && b[0] == 0 && b[1] == 0 && b[2] == 1)
            || (b.len() >= 4 && b[0] == 0 && b[1] == 0 && b[2] == 0 && b[3] == 1)
    }
}

#[cfg(windows)]
pub use imp::MfH264Encoder;

#[cfg(not(windows))]
pub struct MfH264Encoder;

#[cfg(not(windows))]
impl MfH264Encoder {
    pub fn new(_cfg: &H264Config) -> Result<Self, String> {
        Err("h264 needs Windows (Media Foundation)".to_string())
    }

    pub fn dims(&self) -> (u32, u32) {
        (0, 0)
    }

    pub fn encode(&mut self, _nv12: &[u8], _w: u32, _h: u32) -> Result<Vec<EncodedUnit>, String> {
        Err("h264 needs Windows (Media Foundation)".to_string())
    }
}
