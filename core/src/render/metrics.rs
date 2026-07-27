//! Pure image metrics over [`CaptureImage`]s (Plan 0013): pixel and shape
//! difference plus coverage/spread, shared by the differential visual-QA tests
//! and the `shot` CLI report.
//!
//! Everything here is a pure function of its input pixels — no GPU, no clock, no
//! allocation beyond the small working buffers. Not a per-frame hot path, but it
//! lives under `render/` so it carries the panic-denial pragma (and the hygiene
//! guard needs it): written index- and panic-free throughout.

#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use super::CaptureImage;

/// Grid the shape metric downscales to before edge detection (~32×32).
const STRUCT_GRID: usize = 32;

/// Mean absolute per-channel (RGB) difference between two images, normalized to
/// `0.0..=1.0` (0 = identical, 1 = every channel maximally different). Mismatched
/// dimensions read as fully different (`1.0`). Alpha is ignored — the capture
/// background is opaque, so alpha carries no signal.
pub fn frame_diff(a: &CaptureImage, b: &CaptureImage) -> f32 {
    if a.width != b.width || a.height != b.height || a.rgba.len() != b.rgba.len() {
        return 1.0;
    }
    let mut sum: u64 = 0;
    let mut count: u64 = 0;
    for (pa, pb) in a.rgba.chunks_exact(4).zip(b.rgba.chunks_exact(4)) {
        for c in 0..3 {
            if let (Some(&x), Some(&y)) = (pa.get(c), pb.get(c)) {
                sum += x.abs_diff(y) as u64;
                count += 1;
            }
        }
    }
    if count == 0 {
        return 0.0;
    }
    sum as f32 / (count as f32 * 255.0)
}

/// Shape-aware difference in `0.0..=1.0`: downscale each image to a small
/// grayscale grid, take the Sobel edge magnitude, normalize each edge map by its
/// own peak, and mean-abs-diff them. Normalizing per-image cancels overall
/// contrast, so a **recolor of the same shape** scores low while a **different
/// shape** scores high — the near-duplicate probe (an approximation of SSIM).
pub fn struct_diff(a: &CaptureImage, b: &CaptureImage) -> f32 {
    let ea = normalize_max(&sobel(&downscale_gray(a)));
    let eb = normalize_max(&sobel(&downscale_gray(b)));
    let mut sum = 0.0f32;
    let mut count = 0.0f32;
    for (x, y) in ea.iter().zip(eb.iter()) {
        sum += (x - y).abs();
        count += 1.0;
    }
    if count == 0.0 {
        return 0.0;
    }
    (sum / count).clamp(0.0, 1.0)
}

/// Fraction of pixels whose RGB differs from `bg` by more than `eps` on any
/// channel — "how much of the frame is lit" (`0.0..=1.0`). Alpha is ignored.
pub fn coverage(img: &CaptureImage, bg: [u8; 4], eps: u8) -> f32 {
    let mut lit: u64 = 0;
    let mut total: u64 = 0;
    for px in img.rgba.chunks_exact(4) {
        total += 1;
        if is_lit(px, bg, eps) {
            lit += 1;
        }
    }
    if total == 0 {
        return 0.0;
    }
    lit as f32 / total as f32
}

/// How many of the four image quadrants contain at least one lit pixel
/// (`0..=4`) — a cheap "not just a dot in one corner" spread check.
pub fn quadrant_spread(img: &CaptureImage, bg: [u8; 4], eps: u8) -> u8 {
    let w = img.width as usize;
    let h = img.height as usize;
    if w == 0 || h == 0 {
        return 0;
    }
    let mut hit = [false; 4];
    for (i, px) in img.rgba.chunks_exact(4).enumerate() {
        if !is_lit(px, bg, eps) {
            continue;
        }
        let x = i % w;
        let y = i / w;
        let qx = usize::from(x >= w / 2);
        let qy = usize::from(y >= h / 2);
        if let Some(slot) = hit.get_mut(qy * 2 + qx) {
            *slot = true;
        }
    }
    hit.iter().filter(|&&b| b).count() as u8
}

// ---------------------------------------------------------------------------
// Step response — how fast the frame reaches its new steady state (Plan 0037)
// ---------------------------------------------------------------------------

/// Fraction of a step's total change the response must reach to count as
/// settled. 0.9 is the textbook rise-time convention, and it is the one the
/// one-pole arithmetic in ADR-0019 is quoted against: a smoother with time
/// constant `tau` reaches it at `t = tau * ln(10) = 2.303 * tau`.
pub const SETTLE_FRAC: f32 = 0.9;

/// How many frames a captured response took to settle after a step up and after
/// the matching step down (Plan 0037, ADR-0039).
///
/// The whole point of ADR-0035's `{ attack, release }` pair is that these two
/// differ; a scalar `[smoothing]` entry makes them equal by construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StepResponse {
    /// Frames from the step up until the frame settled at [`SETTLE_FRAC`].
    pub rise_frames: u32,
    /// Frames from the step down until the frame settled at [`SETTLE_FRAC`].
    pub fall_frames: u32,
}

impl StepResponse {
    /// `fall / rise` — the asymmetry, which is the number that reads.
    ///
    /// **This is a pixel-domain ratio, not a parameter-domain one.** A scene's
    /// response to its own parameter is rarely linear, so the value differs from
    /// the ratio of the `[smoothing]` constants themselves (ADR-0039); only its
    /// distance from 1.0 is meaningful. A frame that never moved reports
    /// `0.0` rather than dividing by zero.
    pub fn ratio(self) -> f32 {
        self.fall_frames as f32 / self.rise_frames.max(1) as f32
    }
}

/// Measure a step response from two captured segments: `rise` starting at the
/// last frame *before* the step up, `fall` starting at the last frame before the
/// step down. Each segment's own last frame is taken as its settled state.
///
/// Both segments should be the **same length**, because each is normalized
/// against its own final frame: a segment that has not fully settled
/// underestimates the total change and so settles early, and only equal windows
/// make that bias cancel in [`StepResponse::ratio`].
pub fn step_response(rise: &[CaptureImage], fall: &[CaptureImage]) -> StepResponse {
    StepResponse {
        rise_frames: frames_to_settle(rise, SETTLE_FRAC),
        fall_frames: frames_to_settle(fall, SETTLE_FRAC),
    }
}

/// Index of the first frame in `segment` whose distance from `segment[0]` has
/// reached `settle_frac` of the distance between the first and last frames.
///
/// `segment[0]` is the state at the step and the last entry is the settled
/// state, so the answer is in frames-since-the-step. A segment that never moves
/// (total change at or below the float epsilon) reports `0` — the honest answer
/// for a preset the stimulus does not reach, and the one that keeps
/// [`StepResponse::ratio`] finite.
pub fn frames_to_settle(segment: &[CaptureImage], settle_frac: f32) -> u32 {
    let (Some(start), Some(end)) = (segment.first(), segment.last()) else {
        return 0;
    };
    let total = linear_diff(start, end);
    if total <= f32::EPSILON {
        return 0;
    }
    let target = total * settle_frac.clamp(0.0, 1.0);
    for (i, img) in segment.iter().enumerate() {
        if linear_diff(start, img) >= target {
            return i as u32;
        }
    }
    segment.len().saturating_sub(1) as u32
}

/// Mean absolute per-channel difference between two images in **linear light**,
/// normalized to `0.0..=1.0`. Mismatched dimensions read as fully different.
///
/// [`frame_diff`] works on the stored sRGB bytes, which is right for "how
/// different do these two look". It is *wrong* for a step response: sRGB's
/// transfer curve is concave, so a parameter easing linearly toward its target
/// crosses 90 % of its pixel change early on the way up and late on the way
/// down, and a symmetric `[smoothing]` entry would measure asymmetric. Decoding
/// first makes the probe's response proportional to the parameter for a scene
/// whose shader is, which is exactly what the purpose-built easing fixtures are.
fn linear_diff(a: &CaptureImage, b: &CaptureImage) -> f32 {
    if a.width != b.width || a.height != b.height || a.rgba.len() != b.rgba.len() {
        return 1.0;
    }
    let lut = srgb_decode_lut();
    let mut sum = 0.0f64;
    let mut count: u64 = 0;
    for (pa, pb) in a.rgba.chunks_exact(4).zip(b.rgba.chunks_exact(4)) {
        for c in 0..3 {
            let (Some(&x), Some(&y)) = (pa.get(c), pb.get(c)) else {
                continue;
            };
            let lx = lut.get(x as usize).copied().unwrap_or(0.0);
            let ly = lut.get(y as usize).copied().unwrap_or(0.0);
            sum += f64::from((lx - ly).abs());
            count += 1;
        }
    }
    if count == 0 {
        return 0.0;
    }
    (sum / count as f64) as f32
}

/// The 256-entry sRGB→linear decode table, built once. A table rather than a
/// `powf` per channel because [`frames_to_settle`] runs a full-frame difference
/// per captured frame, and the probe is a whole sequence of them.
fn srgb_decode_lut() -> &'static [f32; 256] {
    static LUT: std::sync::OnceLock<[f32; 256]> = std::sync::OnceLock::new();
    LUT.get_or_init(|| {
        let mut table = [0.0f32; 256];
        for (i, slot) in table.iter_mut().enumerate() {
            let c = i as f32 / 255.0;
            *slot = if c <= 0.040_45 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            };
        }
        table
    })
}

/// Whether a pixel's RGB differs from `bg` by more than `eps` on any channel.
fn is_lit(px: &[u8], bg: [u8; 4], eps: u8) -> bool {
    px.iter()
        .zip(bg.iter())
        .take(3)
        .any(|(&c, &b)| c.abs_diff(b) > eps)
}

/// Box-average an image down to a `STRUCT_GRID`×`STRUCT_GRID` grid of grayscale
/// luma in `0.0..=1.0`.
fn downscale_gray(img: &CaptureImage) -> Vec<f32> {
    let g = STRUCT_GRID;
    let mut cells = vec![0.0f32; g * g];
    let mut counts = vec![0u32; g * g];
    let w = img.width as usize;
    let h = img.height as usize;
    if w == 0 || h == 0 {
        return cells;
    }
    for (i, px) in img.rgba.chunks_exact(4).enumerate() {
        let x = i % w;
        let y = i / w;
        let cx = (x * g / w).min(g - 1);
        let cy = (y * g / h).min(g - 1);
        let idx = cy * g + cx;
        let luma = 0.299 * px.first().copied().unwrap_or(0) as f32
            + 0.587 * px.get(1).copied().unwrap_or(0) as f32
            + 0.114 * px.get(2).copied().unwrap_or(0) as f32;
        if let (Some(cell), Some(cnt)) = (cells.get_mut(idx), counts.get_mut(idx)) {
            *cell += luma;
            *cnt += 1;
        }
    }
    for (cell, cnt) in cells.iter_mut().zip(counts.iter()) {
        if *cnt > 0 {
            *cell /= *cnt as f32 * 255.0;
        }
    }
    cells
}

/// Sobel gradient magnitude over a `STRUCT_GRID`×`STRUCT_GRID` grayscale grid.
/// Border cells stay zero (no wrap).
fn sobel(gray: &[f32]) -> Vec<f32> {
    let g = STRUCT_GRID;
    let mut edges = vec![0.0f32; g * g];
    let at = |x: usize, y: usize| -> f32 { gray.get(y * g + x).copied().unwrap_or(0.0) };
    for y in 1..g.saturating_sub(1) {
        for x in 1..g.saturating_sub(1) {
            let gx = at(x + 1, y - 1) + 2.0 * at(x + 1, y) + at(x + 1, y + 1)
                - at(x - 1, y - 1)
                - 2.0 * at(x - 1, y)
                - at(x - 1, y + 1);
            let gy = at(x - 1, y + 1) + 2.0 * at(x, y + 1) + at(x + 1, y + 1)
                - at(x - 1, y - 1)
                - 2.0 * at(x, y - 1)
                - at(x + 1, y - 1);
            if let Some(e) = edges.get_mut(y * g + x) {
                *e = (gx * gx + gy * gy).sqrt();
            }
        }
    }
    edges
}

/// Scale a map so its peak is 1.0; an all-zero map is returned unchanged.
fn normalize_max(v: &[f32]) -> Vec<f32> {
    let max = v.iter().copied().fold(0.0f32, f32::max);
    if max <= f32::EPSILON {
        return v.to_vec();
    }
    v.iter().map(|x| x / max).collect()
}

#[cfg(test)]
mod tests {
    // Test bodies index and unwrap freely — not the hot path.
    #![allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    const BLACK: [u8; 4] = [0, 0, 0, 255];

    /// Build a `w`×`h` image by painting each pixel from `f(x, y) -> [r,g,b,a]`.
    fn image(w: u32, h: u32, f: impl Fn(u32, u32) -> [u8; 4]) -> CaptureImage {
        let mut rgba = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            for x in 0..w {
                rgba.extend_from_slice(&f(x, y));
            }
        }
        CaptureImage {
            width: w,
            height: h,
            rgba,
        }
    }

    fn solid(w: u32, h: u32, color: [u8; 4]) -> CaptureImage {
        image(w, h, |_, _| color)
    }

    /// A vertical bar in `color` over black covering `x < w/2`.
    fn left_bar(w: u32, h: u32, color: [u8; 4]) -> CaptureImage {
        image(w, h, |x, _| if x < w / 2 { color } else { BLACK })
    }

    /// A horizontal bar in `color` over black covering `y < h/2`.
    fn top_bar(w: u32, h: u32, color: [u8; 4]) -> CaptureImage {
        image(w, h, |_, y| if y < h / 2 { color } else { BLACK })
    }

    #[test]
    fn frame_diff_bounds() {
        let black = solid(32, 32, BLACK);
        let white = solid(32, 32, [255, 255, 255, 255]);
        assert_eq!(frame_diff(&black, &black), 0.0);
        assert_eq!(frame_diff(&black, &white), 1.0);
        // Mismatched sizes read as fully different.
        assert_eq!(frame_diff(&black, &solid(16, 16, BLACK)), 1.0);
    }

    #[test]
    fn coverage_and_spread_extremes() {
        let black = solid(32, 32, BLACK);
        let white = solid(32, 32, [255, 255, 255, 255]);
        assert_eq!(coverage(&black, BLACK, 8), 0.0);
        assert_eq!(coverage(&white, BLACK, 8), 1.0);
        assert_eq!(quadrant_spread(&black, BLACK, 8), 0);
        assert_eq!(quadrant_spread(&white, BLACK, 8), 4);

        // A single lit pixel in the top-left quadrant hits exactly one quadrant.
        let dot = image(32, 32, |x, y| {
            if x == 2 && y == 2 {
                [255, 255, 255, 255]
            } else {
                BLACK
            }
        });
        assert_eq!(quadrant_spread(&dot, BLACK, 8), 1);
    }

    /// One frame at 60 Hz — the clock `capture_preset_over` advances by.
    const DT: f32 = 1.0 / 60.0;

    /// sRGB-encode a linear value into the byte a capture would hold, the
    /// inverse of [`srgb_decode_lut`].
    fn encode(linear: f32) -> u8 {
        let c = if linear <= 0.003_130_8 {
            linear * 12.92
        } else {
            1.055 * linear.powf(1.0 / 2.4) - 0.055
        };
        (c.clamp(0.0, 1.0) * 255.0).round() as u8
    }

    /// A flat image whose **linear** grey level is `linear`.
    fn solid_linear(linear: f32) -> CaptureImage {
        let v = encode(linear);
        solid(16, 16, [v, v, v, 255])
    }

    /// A one-pole segment: `n` frames easing from `from` to `to` with time
    /// constant `tau`, rendered as linear grey.
    fn one_pole(from: f32, to: f32, tau: f32, n: usize) -> Vec<CaptureImage> {
        (0..n)
            .map(|i| {
                let p = 1.0 - (-(i as f32 * DT) / tau).exp();
                solid_linear(from + (to - from) * p)
            })
            .collect()
    }

    /// The measurement's contract against the arithmetic ADR-0019 is quoted in:
    /// a one-pole reaches 90 % of a step at `t = tau * ln(10)`, and it does so in
    /// the same number of frames going down as going up.
    #[test]
    fn frames_to_settle_matches_the_one_pole_arithmetic_in_both_directions() {
        let tau = 0.25f32;
        // 2.303 * 0.25 s = 0.576 s = 34.5 frames at 60 Hz.
        let expected = (tau * 10.0f32.ln() / DT).round() as i32;
        // Long enough (2 s = 8 tau) that the segment's own last frame is settled,
        // so normalizing against it costs nothing.
        let n = 120;

        let rise = frames_to_settle(&one_pole(0.0, 1.0, tau, n), SETTLE_FRAC) as i32;
        let fall = frames_to_settle(&one_pole(1.0, 0.0, tau, n), SETTLE_FRAC) as i32;
        println!("one-pole tau={tau}: rise {rise}, fall {fall}, expected {expected}");
        assert!(
            (rise - expected).abs() <= 2,
            "rise {rise} should be within 2 frames of {expected}"
        );
        assert!(
            (fall - expected).abs() <= 2,
            "fall {fall} should be within 2 frames of {expected}"
        );
        assert_eq!(rise, fall, "a symmetric ease must measure symmetric");

        // A faster constant settles proportionally sooner — the measure tracks
        // tau rather than reporting a fixed fraction of the window.
        let quick = frames_to_settle(&one_pole(0.0, 1.0, 0.05, n), SETTLE_FRAC) as i32;
        assert!(quick * 3 < rise, "tau 0.05 ({quick}) vs tau 0.25 ({rise})");
    }

    /// Why [`linear_diff`] exists rather than reusing [`frame_diff`]: on the very
    /// same symmetric ramp, measuring in the stored sRGB bytes reports a rise and
    /// a fall that differ by more than 2x, so a scalar `[smoothing]` entry would
    /// read as asymmetric and the probe would be measuring the transfer curve.
    #[test]
    fn measuring_in_srgb_would_fake_an_asymmetry() {
        let tau = 0.25f32;
        let n = 120;
        let srgb_settle = |seg: &[CaptureImage]| -> usize {
            let total = frame_diff(&seg[0], seg.last().unwrap());
            seg.iter()
                .position(|img| frame_diff(&seg[0], img) >= SETTLE_FRAC * total)
                .unwrap_or(0)
        };
        let rise = srgb_settle(&one_pole(0.0, 1.0, tau, n));
        let fall = srgb_settle(&one_pole(1.0, 0.0, tau, n));
        println!("same ramp measured in sRGB: rise {rise}, fall {fall}");
        assert!(
            fall > rise * 2,
            "sRGB rise {rise} / fall {fall} should be badly skewed — if they are \
             now close, linear_diff is no longer earning its keep"
        );
    }

    /// A stimulus a preset does not respond to must not divide by zero or invent
    /// a transient: both directions read 0 and the ratio is 0.
    #[test]
    fn a_frame_that_never_moves_reports_no_transient() {
        let flat: Vec<CaptureImage> = (0..30).map(|_| solid_linear(0.4)).collect();
        let r = step_response(&flat, &flat);
        assert_eq!(
            r,
            StepResponse {
                rise_frames: 0,
                fall_frames: 0
            }
        );
        assert_eq!(r.ratio(), 0.0);
        // ...and an empty capture is total too.
        assert_eq!(frames_to_settle(&[], SETTLE_FRAC), 0);
    }

    #[test]
    fn step_response_ratio_reports_the_asymmetry() {
        // A 3-frame rise against a 69-frame fall is the shape
        // `{ attack = 0.02, release = 0.5 }` produces at 60 Hz.
        let r = StepResponse {
            rise_frames: 3,
            fall_frames: 69,
        };
        assert!((r.ratio() - 23.0).abs() < 1e-6, "got {}", r.ratio());
        let sym = StepResponse {
            rise_frames: 35,
            fall_frames: 35,
        };
        assert_eq!(sym.ratio(), 1.0);
    }

    #[test]
    fn struct_diff_is_recolor_robust_but_shape_sensitive() {
        // Same shape (left bar), different colors: low structural difference.
        let red_bar = left_bar(64, 64, [220, 30, 30, 255]);
        let blue_bar = left_bar(64, 64, [30, 30, 220, 255]);
        let recolor = struct_diff(&red_bar, &blue_bar);

        // Different shape (left bar vs top bar), same color: high difference.
        let red_top = top_bar(64, 64, [220, 30, 30, 255]);
        let reshape = struct_diff(&red_bar, &red_top);

        assert!(
            recolor < reshape,
            "a recolor ({recolor:.3}) must read as more similar than a reshape ({reshape:.3})"
        );
        assert!(
            recolor < 0.05,
            "same-shape recolor is near-zero ({recolor:.3})"
        );
        assert!(
            reshape > 0.10,
            "a genuine shape change is clearly nonzero ({reshape:.3})"
        );
    }
}
