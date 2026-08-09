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

/// Luminance buckets [`tonal_flatness`] histograms into. 16 over the 0..255
/// range makes each bucket 16 levels wide — narrow enough that a figure with any
/// modelling at all spreads across several, wide enough that dithering and
/// 8-bit quantization do not split one tone in two.
pub const TONE_BANDS: usize = 16;

/// Share of the **lit** figure whose luminance falls inside the single most
/// populated narrow luminance band (`0.0..=1.0`) — "does this picture have any
/// tonal structure".
///
/// `coverage` and `quadrant_spread` answer *is something there* and *is it more
/// than a dot*, and a fully saturated single-tone mass satisfies both: it is a
/// real shape, of the right size, in every quadrant. This asks the question they
/// cannot — whether the shape has any interior. A figure with falloff, depth or
/// modelling spreads across several buckets; one driven past the tonemap knee
/// collapses into one and reads near `1.0`.
///
/// Measured over lit pixels only, against the frame's own sampled background,
/// for the same reason `coverage` is: a sparse figure on a wide ground would
/// otherwise report the *background's* flatness, which is total by construction
/// and says nothing about the scene.
///
/// `0.0` for a frame with no lit pixels at all — an empty picture makes no claim
/// here, and `coverage` is the metric that already convicts it.
pub fn tonal_flatness(img: &CaptureImage, bg: [u8; 4], eps: u8) -> f32 {
    let mut buckets = [0u64; TONE_BANDS];
    let mut lit: u64 = 0;
    for px in img.rgba.chunks_exact(4) {
        if !is_lit(px, bg, eps) {
            continue;
        }
        lit += 1;
        // The same weights `downscale_gray` uses, so "luminance" means one thing
        // across this module.
        let luma = 0.299 * px.first().copied().unwrap_or(0) as f32
            + 0.587 * px.get(1).copied().unwrap_or(0) as f32
            + 0.114 * px.get(2).copied().unwrap_or(0) as f32;
        let bucket = ((luma / 256.0) * TONE_BANDS as f32) as usize;
        if let Some(slot) = buckets.get_mut(bucket.min(TONE_BANDS - 1)) {
            *slot += 1;
        }
    }
    if lit == 0 {
        return 0.0;
    }
    buckets.iter().copied().max().unwrap_or(0) as f32 / lit as f32
}

/// Concentric annuli [`radial_shell_occupancy`] divides the frame's inscribed
/// disc into. Ten equal-radius shells is the granularity the Plan 0065 lane's
/// one-off prototype measured with when it separated the four-ring mandala from
/// the bare rosette (9 shells against 1, design-backlog 0072), and it is kept:
/// coarse enough that a 96×96 capture gives the innermost shell a usable pixel
/// count (~70), fine enough that "occupies most shells" cannot be satisfied by
/// one ring and a halo.
pub const RADIAL_SHELLS: usize = 10;

/// Minimum share of a shell's own pixels that must be lit for the shell to
/// count as occupied in [`radial_shell_occupancy`].
///
/// **Checked against both sides by measurement** (Plan 0075 Phase 1). The
/// failure this guards against is a stray near-threshold pixel marking an
/// empty shell occupied; the content it must not disenfranchise is a hairline
/// stroke crossing a shell. At the sanity suite's 96×96 capture, shell `k`
/// holds ~72·(2k+1) pixels (~72 innermost, ~1370 outermost), so 2 % asks for
/// roughly 2–28 lit pixels per shell — above stray-pixel scale, far below any
/// real stroke. Measured at this threshold: the three honest ring-mandala
/// tunings (backlog 0072's evidence — `glow = 1.0`, no `trails`) read
/// **10 / 10 / 9** occupied shells, every shipped preset reads ≥ 3, and the
/// frozen renders-nothing defect (the pre-repair `spectrum_ridge`, its contour
/// off frame) reads exactly **0** — the threshold separates honest-thin from
/// absent by the measure's whole range.
pub const MIN_SHELL_LIT: f32 = 0.02;

/// How many of [`RADIAL_SHELLS`] concentric equal-radius annuli over the
/// frame's inscribed disc contain a meaningful share of lit pixels
/// (`0..=RADIAL_SHELLS`) — a **structural occupancy** measure: *at how many
/// radii does this picture exist?*
///
/// [`coverage`] counts lit pixels, which at capture size measures a thin-stroke
/// figure's halo rather than its geometry: at 96×96 the bare rosette and a
/// 46×-denser four-ring mandala score identically, and 54 % more geometry moves
/// the number 2.6 % (design-backlog 0072). This asks the question that actually
/// separates them — the mandala exists at nine of ten radii, the rosette's
/// interlace band at one to three — and it cannot be bought with `glow` or
/// `trails`, because inflating the halo around a stroke does not move which
/// shells the stroke lives in.
///
/// Pixels outside the inscribed disc (the frame's corners) are ignored: the
/// measure is radial, and the corners exist at radii only a diagonal figure
/// reaches. `0` for a frame with no lit pixels — a scene that renders nothing
/// occupies nothing, which is the one conviction the coverage floor demonstrably
/// gets right and this measure must preserve.
pub fn radial_shell_occupancy(img: &CaptureImage, bg: [u8; 4], eps: u8) -> usize {
    let w = img.width as usize;
    let h = img.height as usize;
    if w == 0 || h == 0 {
        return 0;
    }
    let (cx, cy) = (w as f32 * 0.5, h as f32 * 0.5);
    let radius = w.min(h) as f32 * 0.5;
    let mut lit = [0u32; RADIAL_SHELLS];
    let mut total = [0u32; RADIAL_SHELLS];
    for (i, px) in img.rgba.chunks_exact(4).enumerate() {
        let x = (i % w) as f32 + 0.5;
        let y = (i / w) as f32 + 0.5;
        let r = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt() / radius;
        if r >= 1.0 {
            continue;
        }
        let shell = ((r * RADIAL_SHELLS as f32) as usize).min(RADIAL_SHELLS - 1);
        if let Some(t) = total.get_mut(shell) {
            *t += 1;
        }
        if is_lit(px, bg, eps)
            && let Some(l) = lit.get_mut(shell)
        {
            *l += 1;
        }
    }
    lit.iter()
        .zip(total.iter())
        .filter(|&(&l, &t)| t > 0 && l as f32 / t as f32 >= MIN_SHELL_LIT)
        .count()
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
/// underestimates the total change and so settles early.
///
/// **Equal windows do not make that bias cancel, and this comment used to claim
/// they did** (corrected by Plan 0038 Phase 8). Cancellation would need both
/// directions to be truncated by the same fraction, which is exactly what an
/// `{ attack, release }` pair is built not to do: at `attack = 0.02` against a
/// `release = 0.5` the rise finishes in 80 τ and carries **no** bias at all, so
/// the fall's has nothing to cancel against and passes straight into
/// [`StepResponse::ratio`]. That is not hypothetical — it is how this repo's own
/// asymmetric probe reported a fall of 61 frames where the settled answer is 69.
///
/// Equal windows remain the right default. They are just not a guarantee:
/// **gate on [`segment_settled`] before trusting either number.**
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

/// Whether `segment`'s last frame is close enough to its asymptote for
/// [`frames_to_settle`] to mean anything — the question that function cannot
/// answer about itself (Plan 0038 Phase 7).
///
/// **Why this is needed at all.** [`frames_to_settle`] normalizes against the
/// segment's *own last frame*. When that frame is still travelling, the measured
/// total is short and every threshold is crossed early — and the returned frame
/// count is a plausible-looking number rather than an obvious failure, because
/// normalizing against the last frame *guarantees* the threshold is reached
/// inside the segment. So `frames_to_settle(seg, f) < seg.len()` is a tautology,
/// not a check, and a caller has no way to tell *settled at frame k* from *still
/// moving at frame k*. Plan 0038 Phase 3 read a truncated window as a shape
/// difference between two orderings on exactly this basis; see
/// [ADR-0040](../../../../docs/adrs/0040-spectrum-level-curve-applies-before-the-easing.md)'s
/// Outcome.
///
/// **The rule.** A settling response's change per unit time decays
/// geometrically, so the tail beyond the last frame can be extrapolated without
/// knowing the time constant. Sample three points at equal spacing `h` — the
/// first frame `A`, the midpoint `B`, the last `C` — and for an exponential
/// approach `|C - B| / |B - A|` is `exp(-h/tau)`, whatever `tau` is. The travel
/// still to come after `C` is then `|C - B| * rho / (1 - rho)`. Settled means
/// that estimate is under `tol` of the change measured so far.
///
/// **The three points are spread across the whole segment on purpose, not taken
/// from the end.** Captures are 8-bit, and a response slow enough to outrun its
/// window moves by *less than one code value per frame* near the end — the
/// residual is large but each individual step is sub-quantum, so consecutive
/// frames decode as identical and any estimator reading adjacent deltas concludes
/// "flat, therefore settled" precisely in the case worth catching. Half a segment
/// of travel is always far above the quantum. (Measured while building this: at
/// `tau` = 2 s over a 2 s window the per-frame step is ~0.003 linear against a
/// ~0.004 quantum at that brightness, and the adjacent-frame version of this
/// function reported the response settled with 37 % of its travel left.)
///
/// Assumes a monotone approach, which every one-pole in this engine is — a
/// response that overshoots is outside what this can judge.
///
/// This deliberately does **not** change [`frames_to_settle`] or
/// [`step_response`], whose numbers `shot --report` publishes for the whole
/// shipped library. Use it as the gate *before* trusting one of those numbers.
pub fn segment_settled(segment: &[CaptureImage], tol: f32) -> bool {
    let (Some(start), Some(end)) = (segment.first(), segment.last()) else {
        return true; // Nothing captured: no claim to invalidate.
    };
    let total = linear_diff(start, end);
    if total <= f32::EPSILON {
        return true; // Never moved; `frames_to_settle` reports 0 and says so.
    }
    // Equal spacing is what makes the ratio below a pure function of tau.
    let Some(mid) = segment.get(segment.len() / 2) else {
        return false; // Too short to see a trend — assume nothing.
    };
    let (first_half, second_half) = (linear_diff(start, mid), linear_diff(mid, end));
    if first_half <= f32::EPSILON {
        return false; // No motion in the first half: nothing to extrapolate from.
    }
    let rho = second_half / first_half;
    if !(0.0..1.0).contains(&rho) {
        return false; // Not decaying: still ramping, or accelerating.
    }
    let remaining = second_half * rho / (1.0 - rho);
    remaining <= tol * total
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
mod tests;
