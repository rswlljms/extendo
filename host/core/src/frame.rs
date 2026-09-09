// Pure pixel + JPEG logic. No Windows APIs here on purpose: this is the part
// `cargo test` can actually verify (AGENTS.md section 7.8), while capture.rs
// holds the untestable WGC glue.

use jpeg_encoder::{ColorType, Encoder};

/// Largest w/h that fits inside `max_w` x `max_h` preserving aspect ratio.
/// Never upscales: a 800x600 source stays 800x600 under a 1280x720 cap, so we
/// spend bandwidth on real pixels rather than interpolation.
#[must_use]
pub fn fit_within(src_w: u32, src_h: u32, max_w: u32, max_h: u32) -> (u32, u32) {
    if src_w == 0 || src_h == 0 || max_w == 0 || max_h == 0 {
        return (1, 1);
    }
    if src_w <= max_w && src_h <= max_h {
        return (src_w, src_h);
    }
    // Integer math throughout; pick the tighter of the two ratios.
    let by_w = (src_h as u64 * max_w as u64) / src_w as u64;
    if by_w <= max_h as u64 {
        (max_w, (by_w as u32).max(1))
    } else {
        let by_h = (src_w as u64 * max_h as u64) / src_h as u64;
        ((by_h as u32).max(1), max_h)
    }
}

/// Nearest-neighbour downscale from a padded RGBA/BGRA source into tight RGB.
///
/// `src_stride` is the row pitch in bytes; WGC buffers are padded to a multiple
/// of the GPU pitch, so it is usually larger than `src_w * 4`. Doing the
/// resize, the alpha strip and the channel swap in one pass keeps us inside the
/// AGENTS.md section 7.3 encode budget - a separate convert pass at 1080p costs
/// more than the JPEG encode itself.
#[must_use]
pub fn downscale_to_rgb(
    src: &[u8],
    src_w: u32,
    src_h: u32,
    src_stride: usize,
    dst_w: u32,
    dst_h: u32,
    swap_rb: bool,
) -> Vec<u8> {
    let mut out = vec![0u8; (dst_w as usize) * (dst_h as usize) * 3];
    if src_w == 0 || src_h == 0 || dst_w == 0 || dst_h == 0 {
        return out;
    }
    let (r_off, b_off) = if swap_rb { (2usize, 0usize) } else { (0usize, 2usize) };

    for y in 0..dst_h as usize {
        // Map destination row to source row (nearest neighbour).
        let sy = y * src_h as usize / dst_h as usize;
        let row = sy * src_stride;
        let dst_row = y * dst_w as usize * 3;
        for x in 0..dst_w as usize {
            let sx = x * src_w as usize / dst_w as usize;
            let si = row + sx * 4;
            let di = dst_row + x * 3;
            // Bounds-checked once per pixel; a short/truncated frame yields
            // black rather than a panic in the capture callback.
            if si + 2 < src.len() {
                out[di] = src[si + r_off];
                out[di + 1] = src[si + 1];
                out[di + 2] = src[si + b_off];
            }
        }
    }
    out
}

/// Encode tight RGB to baseline JPEG. Quality is 1-100.
///
/// # Errors
/// Returns the encoder error if the buffer size does not match `w * h * 3`.
pub fn encode_jpeg(rgb: &[u8], w: u32, h: u32, quality: u8) -> Result<Vec<u8>, String> {
    let expect = (w as usize) * (h as usize) * 3;
    if rgb.len() != expect {
        return Err(format!("rgb buffer {} bytes, expected {}", rgb.len(), expect));
    }
    if w == 0 || h == 0 || w > u16::MAX as u32 || h > u16::MAX as u32 {
        return Err(format!("unsupported dimensions {w}x{h}"));
    }
    let mut buf = Vec::with_capacity(expect / 8);
    let encoder = Encoder::new(&mut buf, quality.clamp(1, 100));
    encoder
        .encode(rgb, w as u16, h as u16, ColorType::Rgb)
        .map_err(|e| format!("jpeg encode failed: {e}"))?;
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_within_caps_1080p_to_720p() {
        assert_eq!(fit_within(1920, 1080, 1280, 720), (1280, 720));
    }

    #[test]
    fn fit_within_preserves_aspect_when_height_bound() {
        // 16:10 source must letterbox by width, not stretch to the 16:9 cap.
        assert_eq!(fit_within(1920, 1200, 1280, 720), (1152, 720));
    }

    #[test]
    fn fit_within_never_upscales() {
        assert_eq!(fit_within(800, 600, 1280, 720), (800, 600));
    }

    #[test]
    fn fit_within_survives_zero_and_tiny_inputs() {
        assert_eq!(fit_within(0, 0, 1280, 720), (1, 1));
        assert_eq!(fit_within(1920, 1080, 0, 0), (1, 1));
        // Extreme ratio must still produce a legal (>=1) dimension.
        let (w, h) = fit_within(10_000, 2, 100, 100);
        assert!(w >= 1 && h >= 1, "got {w}x{h}");
    }

    #[test]
    fn downscale_strips_alpha_and_honours_stride() {
        // 2x1 RGBA source with 4 bytes of row padding after the pixels.
        let src_w = 2;
        let src_h = 1;
        let stride = src_w * 4 + 4;
        let mut src = vec![0u8; stride * src_h];
        src[0..4].copy_from_slice(&[10, 20, 30, 255]);
        src[4..8].copy_from_slice(&[40, 50, 60, 255]);
        let out = downscale_to_rgb(&src, 2, 1, stride, 2, 1, false);
        assert_eq!(out, vec![10, 20, 30, 40, 50, 60]);
    }

    #[test]
    fn downscale_swaps_red_and_blue_when_asked() {
        let mut src = vec![0u8; 8];
        src[0..4].copy_from_slice(&[10, 20, 30, 255]);
        src[4..8].copy_from_slice(&[40, 50, 60, 255]);
        let out = downscale_to_rgb(&src, 2, 1, 8, 2, 1, true);
        assert_eq!(out, vec![30, 20, 10, 60, 50, 40]);
    }

    #[test]
    fn downscale_halves_dimensions_by_nearest_neighbour() {
        // 4x4 source of distinct rows -> 2x2 picks rows 0,2 and cols 0,2.
        let (w, h) = (4usize, 4usize);
        let stride = w * 4;
        let mut src = vec![0u8; stride * h];
        for y in 0..h {
            for x in 0..w {
                let i = y * stride + x * 4;
                src[i] = (x * 10) as u8;
                src[i + 1] = (y * 10) as u8;
                src[i + 2] = 7;
                src[i + 3] = 255;
            }
        }
        let out = downscale_to_rgb(&src, 4, 4, stride, 2, 2, false);
        assert_eq!(out.len(), 2 * 2 * 3);
        assert_eq!(&out[0..3], &[0, 0, 7]); // (0,0)
        assert_eq!(&out[3..6], &[20, 0, 7]); // (2,0)
        assert_eq!(&out[6..9], &[0, 20, 7]); // (0,2)
    }

    #[test]
    fn downscale_truncated_source_yields_black_not_panic() {
        // A short buffer (torn/partial frame) must not take down the capture thread.
        let src = vec![0u8; 4];
        let out = downscale_to_rgb(&src, 64, 64, 64 * 4, 8, 8, false);
        assert_eq!(out.len(), 8 * 8 * 3);
    }

    #[test]
    fn encode_jpeg_emits_valid_soi_eoi_markers() {
        let rgb = vec![128u8; 16 * 16 * 3];
        let jpg = encode_jpeg(&rgb, 16, 16, 70).expect("encode");
        assert!(jpg.len() > 4, "jpeg too small: {}", jpg.len());
        assert_eq!(&jpg[0..2], &[0xFF, 0xD8], "missing SOI");
        assert_eq!(&jpg[jpg.len() - 2..], &[0xFF, 0xD9], "missing EOI");
    }

    #[test]
    fn encode_jpeg_rejects_mismatched_buffer() {
        let rgb = vec![0u8; 10];
        assert!(encode_jpeg(&rgb, 16, 16, 70).is_err());
    }

    #[test]
    fn encode_jpeg_quality_affects_size() {
        // Gradient so quality actually changes the entropy-coded size.
        let (w, h) = (64u32, 64u32);
        let mut rgb = vec![0u8; (w * h * 3) as usize];
        for (i, px) in rgb.as_chunks_mut::<3>().0.iter_mut().enumerate() {
            px[0] = (i % 251) as u8;
            px[1] = (i % 97) as u8;
            px[2] = (i % 37) as u8;
        }
        let low = encode_jpeg(&rgb, w, h, 20).expect("low");
        let high = encode_jpeg(&rgb, w, h, 95).expect("high");
        assert!(high.len() > low.len(), "q95 {} !> q20 {}", high.len(), low.len());
    }
}
