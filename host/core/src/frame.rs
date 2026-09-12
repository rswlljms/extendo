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

/// Byte size of an NV12 frame: full-size Y plane + half-size interleaved UV.
#[must_use]
pub fn nv12_frame_size(w: u32, h: u32) -> usize {
    (w as usize) * (h as usize) * 3 / 2
}

/// Nearest-neighbour downscale from a padded RGBA source straight into NV12.
///
/// This is the H.264 input path (the MF encoder takes NV12, not RGB): resize,
/// alpha strip and BT.601 RGB→YCbCr happen in one pass so we stay inside the
/// AGENTS.md section 7.3 encode budget with no intermediate RGB buffer.
///
/// Uses full-range (JPEG) BT.601: `Y=(77R+150G+29B)>>8`,
/// `U=((-43R-84G+127B)>>8)+128`, `V=((127R-106G-21B)>>8)+128`.
/// Chroma is 2x2-box-averaged. NV12 needs even dimensions, so odd targets are
/// clamped down by one (a 405-wide fit becomes 404); minimum 2x2.
#[must_use]
pub fn downscale_rgba_to_nv12(
    src: &[u8],
    src_w: u32,
    src_h: u32,
    src_stride: usize,
    dst_w: u32,
    dst_h: u32,
) -> Vec<u8> {
    let w = (dst_w & !1).max(2) as usize;
    let h = (dst_h & !1).max(2) as usize;
    let mut out = vec![0u8; w * h * 3 / 2];
    if src_w == 0 || src_h == 0 {
        return out;
    }
    let (y_plane, uv_plane) = out.split_at_mut(w * h);

    // Luma: one Y per pixel.
    for y in 0..h {
        let sy = y * src_h as usize / h;
        let row = sy * src_stride;
        let dst_row = y * w;
        for x in 0..w {
            let sx = x * src_w as usize / w;
            let si = row + sx * 4;
            let (r, g, b) = if si + 2 < src.len() {
                (src[si] as i32, src[si + 1] as i32, src[si + 2] as i32)
            } else {
                (0, 0, 0) // torn/partial frame: black, never a panic
            };
            y_plane[dst_row + x] = ((77 * r + 150 * g + 29 * b) >> 8) as u8;
        }
    }

    // Chroma: one interleaved U,V pair per 2x2 block, averaged.
    for y in (0..h).step_by(2) {
        for x in (0..w).step_by(2) {
            let mut su = 0i32;
            let mut sv = 0i32;
            for dy in 0..2 {
                let sy = (y + dy) * src_h as usize / h;
                let row = sy * src_stride;
                for dx in 0..2 {
                    let sx = (x + dx) * src_w as usize / w;
                    let si = row + sx * 4;
                    let (r, g, b) = if si + 2 < src.len() {
                        (src[si] as i32, src[si + 1] as i32, src[si + 2] as i32)
                    } else {
                        (0, 0, 0)
                    };
                    su += (-43 * r - 84 * g + 127 * b) >> 8;
                    sv += (127 * r - 106 * g - 21 * b) >> 8;
                }
            }
            let di = (y / 2) * w + x;
            uv_plane[di] = (su / 4 + 128).clamp(0, 255) as u8;
            uv_plane[di + 1] = (sv / 4 + 128).clamp(0, 255) as u8;
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

    fn rgba_frame(w: usize, h: usize, px: [u8; 4]) -> Vec<u8> {
        let mut src = vec![0u8; w * h * 4];
        for chunk in src.chunks_exact_mut(4) {
            chunk.copy_from_slice(&px);
        }
        src
    }

    #[test]
    fn nv12_size_is_y_plus_half_uv() {
        assert_eq!(nv12_frame_size(1280, 720), 1280 * 720 * 3 / 2);
        assert_eq!(nv12_frame_size(1920, 1080), 1920 * 1080 * 3 / 2);
    }

    #[test]
    fn nv12_primary_colors_match_bt601() {
        // 2x2 uniform frames: Y plane uniform, UV plane a single pair.
        for (px, y, u, v) in [
            ([0u8, 0, 0, 255], 0u8, 128u8, 128u8), // black
            ([255, 255, 255, 255], 255, 128, 128), // white
            ([255, 0, 0, 255], 76, 85, 254),       // red
            ([0, 255, 0, 255], 149, 44, 22),       // green
            ([0, 0, 255, 255], 28, 254, 107),      // blue
        ] {
            let nv12 = downscale_rgba_to_nv12(&rgba_frame(2, 2, px), 2, 2, 2 * 4, 2, 2);
            assert_eq!(nv12.len(), 6, "2x2 NV12 is 4 Y + 2 UV");
            assert!(nv12[0..4].iter().all(|&p| p == y), "{px:?}: Y={:?}, want {y}", &nv12[0..4]);
            assert_eq!((nv12[4], nv12[5]), (u, v), "{px:?}: UV mismatch");
        }
    }

    #[test]
    fn nv12_honours_stride_and_downscales() {
        // 4x4 red source with padded rows -> 2x2 stays red.
        let (w, h) = (4usize, 4usize);
        let stride = w * 4 + 16;
        let mut src = vec![0u8; stride * h];
        for y in 0..h {
            for x in 0..w {
                let i = y * stride + x * 4;
                src[i..i + 4].copy_from_slice(&[255, 0, 0, 255]);
            }
        }
        let nv12 = downscale_rgba_to_nv12(&src, 4, 4, stride, 2, 2);
        assert_eq!(nv12.len(), 6);
        assert!(nv12[0..4].iter().all(|&p| p == 76));
        assert_eq!((nv12[4], nv12[5]), (85, 254));
    }

    #[test]
    fn nv12_clamps_odd_dimensions_to_even() {
        // A 405-wide fit (odd) must come out 404 wide: NV12 needs even dims.
        let nv12 = downscale_rgba_to_nv12(&rgba_frame(8, 8, [9, 9, 9, 255]), 8, 8, 8 * 4, 5, 7);
        assert_eq!(nv12.len(), nv12_frame_size(4, 6));
    }

    #[test]
    fn nv12_truncated_source_yields_black_not_panic() {
        let nv12 = downscale_rgba_to_nv12(&[1, 2, 3], 64, 64, 64 * 4, 8, 8);
        assert_eq!(nv12.len(), nv12_frame_size(8, 8));
    }
}
