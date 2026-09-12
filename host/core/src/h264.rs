// H.264 wire helpers for the native-Android video path (AGENTS.md section 4).
//
// The actual encoder (Media Foundation, baseline/main, NO B-frames) lands as
// the next milestone; this module holds everything around it that `cargo test`
// can verify without a GPU or the MF SDK:
//
// - `H264Config` + `validate`: v1 caps (1080p60 SDR, 500-12000 kbps) and the
//   no-B-frames invariant, so misconfiguration fails fast at spawn time.
// - Annex B bytestream framing: start-code scan, NAL split, NAL unit type,
//   keyframe (IDR/SPS/PPS) detection for the future RTP packetizer and for
//   the Android `MediaCodec` `BUFFER_FLAG_KEY_FRAME` path.
// - `bitrate_for_mode`: the adaptive ladder shared by the Node host tick and
//   the encoder, so both sides agree without a second source of truth.
//
// Entitlement note (AGENTS.md 10.1): no tier logic here. Caps arrive from the
// Node host already ceiling-clamped; `validate` only enforces physical v1
// limits.

/// Encoder profile. v1 allows baseline (widest HW decode) and main.
/// High/HEVC are out of scope for v1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum H264Profile {
    Baseline,
    Main,
}

impl H264Profile {
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "baseline" => Some(Self::Baseline),
            "main" => Some(Self::Main),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Baseline => "baseline",
            Self::Main => "main",
        }
    }
}

/// Encoder configuration. `bframes` must stay 0: B-frames break browsers,
/// add a frame of latency, and are banned by AGENTS.md section 7.3.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct H264Config {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub bitrate_kbps: u32,
    pub profile: H264Profile,
    pub bframes: u32,
    /// IDR interval in frames (keyframe cadence for seek/reconnect).
    pub gop_len: u32,
}

impl Default for H264Config {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
            fps: 30,
            bitrate_kbps: 4000,
            profile: H264Profile::Baseline,
            bframes: 0,
            gop_len: 60,
        }
    }
}

impl H264Config {
    /// Check v1 limits.
    ///
    /// # Errors
    /// Returns a message describing the violated constraint.
    pub fn validate(&self) -> Result<(), String> {
        if self.width == 0 || self.height == 0 || self.width > 1920 || self.height > 1080 {
            return Err("h264 dimensions must fit within 1920x1080 (v1 SDR cap)".to_string());
        }
        if self.fps == 0 || self.fps > 60 {
            return Err("h264 fps must be 1..=60 (v1 caps at 60)".to_string());
        }
        if self.bitrate_kbps < 500 || self.bitrate_kbps > 12000 {
            return Err("h264 bitrate must be 500..=12000 kbps".to_string());
        }
        if self.bframes != 0 {
            return Err("h264 bframes must be 0 (no B-frames in v1: latency)".to_string());
        }
        if self.gop_len == 0 || self.gop_len > 600 {
            return Err("h264 gop_len must be 1..=600".to_string());
        }
        Ok(())
    }
}

/// Adaptive bitrate ladder (kbps) shared with the Node host tick.
/// 720p30 floor keeps USB-tethering usable; 1080p60 ceiling is the v1 cap.
#[must_use]
pub fn bitrate_for_mode(width: u32, height: u32, fps: u32) -> u32 {
    let pixels = width as u64 * height as u64;
    let base = if pixels <= 1280 * 720 { 4000 } else { 8000 };
    if fps > 30 { base + base / 2 } else { base }.clamp(500, 12000)
}

/// Find Annex B start codes (`0x000001` or `0x00000001`) in a bytestream.
/// Returns the byte offset of each start code.
#[must_use]
pub fn find_start_codes(buf: &[u8]) -> Vec<usize> {
    let mut out = Vec::new();
    let mut i = 0;
    while i + 3 < buf.len() {
        if buf[i] == 0 && buf[i + 1] == 0 {
            if buf[i + 2] == 1 {
                out.push(i);
                i += 3;
                continue;
            }
            if i + 4 < buf.len() && buf[i + 2] == 0 && buf[i + 3] == 1 {
                out.push(i);
                i += 4;
                continue;
            }
        }
        i += 1;
    }
    out
}

/// Split an Annex B bytestream into raw NAL units (start codes stripped).
/// Trailing zero-length units are dropped.
#[must_use]
pub fn split_nals(buf: &[u8]) -> Vec<&[u8]> {
    let starts = find_start_codes(buf);
    if starts.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(starts.len());
    for (idx, &s) in starts.iter().enumerate() {
        let hdr_len = if buf[s + 2] == 1 { 3 } else { 4 };
        let end = starts.get(idx + 1).copied().unwrap_or(buf.len());
        let nal = &buf[s + hdr_len..end];
        if !nal.is_empty() {
            out.push(nal);
        }
    }
    out
}

/// NAL unit type = low 5 bits of the first byte (H.264, not HEVC).
#[must_use]
pub fn nal_unit_type(nal: &[u8]) -> Option<u8> {
    nal.first().map(|b| b & 0x1F)
}

/// True for parameter sets and IDR pictures: a decoder joining mid-stream
/// must wait for one of these before it can output pictures.
#[must_use]
pub fn is_keyframe_nal(nal_type: u8) -> bool {
    matches!(nal_type, 5 | 7 | 8) // IDR, SPS, PPS
}

/// True when an Annex B access unit contains an IDR picture.
#[must_use]
pub fn is_keyframe_access_unit(buf: &[u8]) -> bool {
    split_nals(buf)
        .iter()
        .filter_map(|n| nal_unit_type(n))
        .any(is_keyframe_nal)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_720p30_baseline_no_bframes() {
        let c = H264Config::default();
        assert_eq!(c.profile, H264Profile::Baseline);
        assert_eq!(c.bframes, 0);
        assert!(c.validate().is_ok());
    }

    #[test]
    fn rejects_4k_high_fps_and_bframes() {
        let mut c = H264Config::default();
        c.width = 3840;
        c.height = 2160;
        assert!(c.validate().is_err(), "no 4K in v1");
        c = H264Config::default();
        c.fps = 120;
        assert!(c.validate().is_err(), "v1 caps at 60fps");
        c = H264Config::default();
        c.bframes = 2;
        assert!(c.validate().is_err(), "B-frames banned (latency)");
        c = H264Config::default();
        c.bitrate_kbps = 100;
        assert!(c.validate().is_err());
        c = H264Config::default();
        c.bitrate_kbps = 50000;
        assert!(c.validate().is_err());
    }

    #[test]
    fn profile_parses_baseline_and_main_only() {
        assert_eq!(H264Profile::parse("baseline"), Some(H264Profile::Baseline));
        assert_eq!(H264Profile::parse("Main"), Some(H264Profile::Main));
        assert_eq!(H264Profile::parse("high"), None);
        assert_eq!(H264Profile::parse("hevc"), None);
    }

    #[test]
    fn bitrate_ladder_stays_inside_v1_caps() {
        assert_eq!(bitrate_for_mode(1280, 720, 30), 4000);
        assert_eq!(bitrate_for_mode(1920, 1080, 30), 8000);
        assert_eq!(bitrate_for_mode(1920, 1080, 60), 12000);
        assert_eq!(bitrate_for_mode(640, 480, 30), 4000);
    }

    #[test]
    fn finds_both_start_code_lengths() {
        // 4-byte SPS + 3-byte IDR.
        let buf = [0, 0, 0, 1, 0x67, 0x42, 0, 0, 1, 0x65, 0x88];
        assert_eq!(find_start_codes(&buf), vec![0, 6]);
    }

    #[test]
    fn splits_nals_and_reads_types() {
        let buf = [0, 0, 0, 1, 0x67, 0x42, 0, 0, 1, 0x68, 0xCE, 0, 0, 1, 0x65, 0x88];
        let nals = split_nals(&buf);
        assert_eq!(nals.len(), 3);
        let types: Vec<u8> = nals.iter().filter_map(|n| nal_unit_type(n)).collect();
        assert_eq!(types, vec![7, 8, 5]); // SPS, PPS, IDR
        assert!(is_keyframe_access_unit(&buf));
    }

    #[test]
    fn non_idr_slice_is_not_a_keyframe() {
        // Single non-IDR slice (type 1).
        let buf = [0, 0, 0, 1, 0x41, 0x9A, 0x22];
        assert!(!is_keyframe_access_unit(&buf));
        assert_eq!(nal_unit_type(&split_nals(&buf)[0]), Some(1));
    }

    #[test]
    fn empty_and_startcode_less_inputs_yield_no_nals() {
        assert!(split_nals(&[]).is_empty());
        assert!(split_nals(&[0x41, 0x9A]).is_empty());
    }
}
