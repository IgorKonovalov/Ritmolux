//! What MilkDrop draws between the warp and the composite (Plan 0100 Phase 4):
//! the waveform, the custom waves and shapes, the two borders, and the
//! motion-vector grid.
//!
//! # It is CPU geometry, deliberately
//!
//! Every figure here is a handful of hundreds of points produced by a program the
//! preset wrote, so it is built on the render thread into two reused buffers and
//! handed to the GPU as one line batch and one triangle batch. **Nothing here
//! allocates per frame**: both buffers are sized once at preset load from the
//! bundle's own counts, which are bounded by
//! [`MAX_WAVE_POINTS`](crate::milk::MAX_WAVE_POINTS) and its siblings.
//!
//! # Two blend modes, and why the geometry is ordered
//!
//! MilkDrop chooses per element: `bAdditiveWaves` and a custom element's own
//! `additive` pick between `dst = src + dst` and `dst = src*a + dst*(1-a)`. Both
//! are honoured here, by **partitioning each buffer** — additive producers first,
//! over-blended ones after — and handing the split index to the two-pipeline
//! draw ([`LineRenderer::draw_split`](crate::render::scenes::lines::LineRenderer::draw_split)).
//!
//! Reading both as additive is what saturated the frame to flat colour inside
//! half a second, and it is not a small error: an additive seam **sums** where
//! alpha-over **replaces**, so N overlapping producers land at N rather than at
//! ≤ 1. That bites hardest on the **28.5 % of the corpus that sets
//! `fDecay >= 1.0`** (2 949 of 10 347, measured 2026-08-16), where the field is a
//! perfect integrator and nothing brings the sum back down.
//!
//! The order within each half is the order MilkDrop draws in — waveform, custom
//! waves, custom shapes, borders, motion vectors — because an over-blended stroke
//! must land on top of what it covers.
//!
//! # The coordinate space, once
//!
//! MilkDrop places everything in **uv**: `0..1` across the frame with `y = 0` at
//! the *top*. The shared line renderer takes **world** space: `y` in `-1..1`
//! bottom-to-top, `x` in `-aspect..aspect` (its shader divides x by the aspect).
//! [`uv_to_world`] is the one conversion, and every producer below goes through
//! it — which is what makes a figure round on any display and is the ADR-0037
//! rule applied to geometry rather than to a grid.
//!
//! # What is approximated, stated
//!
//! - **A dot is a very short segment with both caps extended.** `wave_usedots`
//!   draws points; the line renderer draws quads between endpoints, and its
//!   falloff runs across the stroke only — so a short segment is round *because*
//!   both endpoint extensions push the quad past it by the half-width
//!   (ADR-0158), not because it is short. Without them it is a sub-pixel dash;
//!   see `dots`.
//! - **`wave_mystery` means something different in every mode**, which is the
//!   reference's own design rather than a simplification here.
//! - **Mode 6 and 7's line does not drift.** Its angle is `wave_mystery` alone;
//!   a `time * 0.05` term rotates it a full turn every ~126 s, which a Plan 0109
//!   Phase 2 look gate rejected against *Blur Mix 3*'s horizontal reference
//!   traces. Listed here because the reference's own mode 6 is documented only
//!   as "a line", and "which line" is a reading of it — see the arm's comment.

// Hot-path panic-denial pragma (Plan 0002 Phase 2, extended to scenes by Plan
// 0003 Phase 0). This builds geometry every displayed frame.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use crate::dsp::WAVE_SAMPLES;
use crate::milk::outputs::FrameOutputs;
use crate::milk::{MAX_SHAPE_SIDES, MilkRuntime};
use crate::render::scenes::lines::SegmentInstance;

/// One vertex of a filled custom shape.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ShapeVertex {
    /// World-space position — see the module docs.
    pub pos: [f32; 2],
    /// Premultiplied RGB, already scaled by the instance's alpha.
    pub color: [f32; 3],
    /// Coverage, which the additive seam needs to equal the light's own
    /// footprint (ADR-0056).
    pub alpha: f32,
}

/// The two buffers a frame's draw layer fills, each **partitioned by blend
/// mode**. Sized once at preset load.
///
/// Additive geometry occupies `..n_additive` and over-blended geometry the rest,
/// which is what lets one buffer and one render pass serve two pipelines — see
/// the module docs. Producers are appended through `push_segment`
/// and `push_triangle` rather than to the vectors
/// directly, so the invariant is maintained in one place.
#[derive(Default)]
pub struct DrawGeometry {
    /// Every line: the waveform, custom waves, shape outlines, borders, motion
    /// vectors.
    pub segments: Vec<SegmentInstance>,
    /// How many leading entries of [`segments`](Self::segments) blend additively.
    pub segments_additive: usize,
    /// Every filled shape's triangles, as a plain list.
    pub triangles: Vec<ShapeVertex>,
    /// How many leading entries of [`triangles`](Self::triangles) blend
    /// additively. Always a multiple of 3 — a triangle's three vertices share one
    /// blend mode.
    pub triangles_additive: usize,
}

impl DrawGeometry {
    /// Discard last frame's geometry without giving back its capacity — the
    /// reuse that keeps the per-frame path allocation-free.
    pub fn clear(&mut self) {
        self.segments.clear();
        self.segments_additive = 0;
        self.triangles.clear();
        self.triangles_additive = 0;
    }

    /// Whether anything was built this frame.
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty() && self.triangles.is_empty()
    }

    /// Append one segment into its blend mode's half.
    ///
    /// An additive one is **inserted** at the partition rather than pushed, which
    /// is `O(n)` in the over-blended tail. That is deliberate and it is cheap:
    /// the whole layer is a few hundred segments of CPU geometry (see the module
    /// docs), and the alternative — two vectors concatenated per frame — would
    /// either allocate or need a third buffer. Neither is worth it at this size,
    /// and this keeps the draw order MilkDrop's within each half.
    fn push_segment(&mut self, segment: SegmentInstance, additive: bool) {
        if additive {
            self.segments.insert(self.segments_additive, segment);
            self.segments_additive += 1;
        } else {
            self.segments.push(segment);
        }
    }

    /// Append one triangle — three vertices, one blend mode. See
    /// [`push_segment`](Self::push_segment) for why the additive case inserts.
    fn push_triangle(&mut self, vertices: [ShapeVertex; 3], additive: bool) {
        if additive {
            for (offset, vertex) in vertices.into_iter().enumerate() {
                self.triangles
                    .insert(self.triangles_additive + offset, vertex);
            }
            self.triangles_additive += 3;
        } else {
            self.triangles.extend_from_slice(&vertices);
        }
    }
}

/// uv (`y` down, `0..1`) to the line renderer's world space (`y` up, `x` scaled
/// by the aspect).
///
/// **The one conversion**, and the only place the aspect enters the draw layer.
/// `aspect` is the **render target's** (ADR-0037).
pub fn uv_to_world(x: f32, y: f32, aspect: f32) -> [f32; 2] {
    [(x * 2.0 - 1.0) * aspect, 1.0 - y * 2.0]
}

/// The stroke half-width a `thick` flag selects, in NDC-y units.
///
/// MilkDrop draws a thick line as two or four passes offset by a pixel; here it
/// is one stroke of twice the width, which is the same gesture through this
/// engine's soft-falloff primitive.
const THIN: f32 = 0.0025;
/// See [`THIN`].
const THICK: f32 = 0.006;

/// How long a `wave_usedots` dot's segment is, in world units. Short enough that
/// the falloff reads as round and long enough that the quad is not degenerate.
const DOT_LENGTH: f32 = 0.0015;

/// Build the whole draw layer for this frame.
///
/// `runtime` is `None` for a hand-authored `warp_mesh` preset, which draws no
/// MilkDrop layer at all — so this whole file costs a native preset one branch.
/// **`time` is read by exactly three of the eight `wave_mode` figures**, and by
/// nothing else in this file: mode 0 turns at `0.2` rad/s (`milkdropfs.cpp`
/// l.2886-2925), mode 1 at `2.3` (l.2942) and mode 5 at `0.3` (l.3085-3086).
/// Every other figure is a pure function of the trace and the frame outputs, and
/// that narrowed claim is what `draw_layer.rs` tests — by calling this twice at
/// well-separated times for a mode with no time term, and by asserting each
/// turning mode comes back after exactly one turn at its own rate.
#[allow(clippy::too_many_arguments)]
pub fn build(
    geometry: &mut DrawGeometry,
    runtime: Option<&mut MilkRuntime>,
    out: &FrameOutputs,
    pair: &[[f32; WAVE_SAMPLES]; 2],
    time: f32,
    dt: f32,
    aspect: f32,
    target_width: u32,
) {
    geometry.clear();
    let Some(runtime) = runtime else {
        return;
    };
    let exposure = Exposure::new(dt);
    // The two traces, smoothed and scaled **once**, the way the source builds
    // `fWaveL`/`fWaveR` before any figure reads them (`milkdropfs.cpp`
    // l.933-942): a one-pole run along the trace at `wave_smoothing`, then
    // `wave_scale`. Every mode and every custom wave indexes these.
    let smooth = out.wave_smoothing.clamp(0.0, 0.99);
    let scale = out.wave_scale;
    let mut traces = [[0.0f32; WAVE_SAMPLES]; 2];
    for (dst, src) in traces.iter_mut().zip(pair.iter()) {
        let mut held = 0.0f32;
        for (slot, raw) in dst.iter_mut().zip(src.iter()) {
            held = held * smooth + raw * (1.0 - smooth);
            *slot = held * scale;
        }
    }
    let [left, right] = &traces;
    waveform_figure(
        geometry,
        out,
        left,
        right,
        time,
        exposure,
        aspect,
        target_width,
    );
    custom_waves(geometry, runtime, left, right, exposure, aspect);
    custom_shapes(geometry, runtime, exposure, aspect);
    // The two borders and the motion-vector grid are **always** alpha-blended in
    // the reference — neither has an additive flag to read — so they go to the
    // over half unconditionally.
    borders(geometry, out, exposure, aspect);
    motion_vectors(geometry, out, exposure, aspect);
}

/// **What one frame of the draw layer is worth**, which depends on how the
/// producer that drew it blends. Both cases are a rate rather than a constant,
/// and for the same reason.
///
/// MilkDrop deposits once per rendered frame into a buffer that decays once per
/// rendered frame, so the two are in step by construction. Here the field decays
/// **per second** (`decay` is rate-converted like every other MilkDrop factor),
/// which means a 60 Hz display would deposit twice the light per second that a
/// 30 Hz one does into a buffer that fades at the same rate — and the picture
/// would differ with the refresh, which ADR-0019 exists to prevent. `rate` is how
/// many nominal frames of wall clock this frame is, and both branches of
/// `scale` convert through it.
#[derive(Clone, Copy)]
pub struct Exposure {
    /// `dt * NOMINAL_FPS`: how many nominal frames of wall clock this frame is.
    rate: f32,
}

impl Exposure {
    /// From this frame's `dt`, capped at **four** nominal frames: a long frame
    /// deposits proportionally more light, but a stall cannot deposit an
    /// unbounded amount in one go.
    pub fn new(dt: f32) -> Self {
        Self {
            rate: (dt * crate::milk::NOMINAL_FPS).clamp(0.0, 4.0),
        }
    }

    /// The producer's effective alpha this frame — its colour is premultiplied
    /// by this, and it is the coverage the fragment writes.
    ///
    /// # The two conversions
    ///
    /// **Additive** light composes by *addition* across frames, so `n` frames
    /// deposit `n * a` and the rate conversion is the plain product `a * rate`.
    ///
    /// **Alpha-over** composes by *repeated interpolation*: `n` frames of
    /// `dst = src*a + dst*(1-a)` leave `1 - (1-a)^n` of the way travelled, so the
    /// conversion is `1 - (1-a)^rate`. At `rate = 1` it is exactly `a`, which is
    /// what makes 60 Hz the reference's own cadence rather than an approximation
    /// of it; at `rate = 2` a 30 Hz frame travels as far as two 60 Hz ones, which
    /// is the property ADR-0019 asks for.
    ///
    /// Note that the alpha-over branch is **bounded by 1** for every `rate`,
    /// which is the whole reason this is worth two pipelines: the sum of N
    /// over-blended producers is still ≤ 1, where N additive ones is N.
    fn scale(self, alpha: f32, additive: bool) -> f32 {
        let a = alpha.clamp(0.0, 1.0);
        if additive {
            a * self.rate
        } else {
            1.0 - (1.0 - a).powf(self.rate)
        }
    }
}

/// A producer's premultiplied colour and its coverage, both already scaled by
/// [`Exposure::scale`].
///
/// The two are returned together because both seams need coverage to equal the
/// light's own footprint: additively so a dim deposit does not occlude a lit
/// backdrop (ADR-0056), and over-blended because the coverage *is* the blend's
/// alpha.
#[derive(Clone, Copy)]
struct Light {
    /// Premultiplied colour: the producer's RGB times its effective alpha.
    rgb: [f32; 3],
    /// **The alpha the fragment writes**, which is not the same number in the two
    /// seams and that difference is ADR-0056's rule rather than an inconsistency.
    ///
    /// Over-blended, coverage *is* the blend's alpha — a `wave_a = 0.1` stroke
    /// must replace a tenth of what is under it, so it is the effective alpha.
    ///
    /// Additive, it is **`1.0`**: "a dimmed stroke still covers its own
    /// footprint", so brightness lives in [`rgb`](Self::rgb) and the geometry's
    /// own falloff is the whole footprint. Passing the effective alpha here
    /// instead would make a dim additive stroke *narrower* rather than darker,
    /// and could exceed 1 at a long `dt`.
    coverage: f32,
}

impl Light {
    /// Whether this producer writes anything worth a draw call.
    fn is_dark(&self) -> bool {
        self.rgb.iter().all(|c| *c <= 0.0001)
    }
}

/// One point of a stroked figure: where it is, and what it deposits there.
type Point = ([f32; 2], Light);

/// Colour and coverage for one producer. `wave_brighten` normalizes to the
/// brightest channel first, which is what the reference's `bMaximizeWaveColor`
/// does.
fn light(
    r: f32,
    g: f32,
    b: f32,
    a: f32,
    brighten: bool,
    exposure: Exposure,
    additive: bool,
) -> Light {
    let (mut r, mut g, mut b) = (r, g, b);
    if brighten {
        let peak = r.max(g).max(b);
        if peak > 0.0001 {
            let k = 1.0 / peak;
            r *= k;
            g *= k;
            b *= k;
        }
    }
    let a = exposure.scale(a, additive);
    Light {
        rgb: [r * a, g * a, b * a],
        coverage: if additive { 1.0 } else { a },
    }
}

/// Push a polyline, flagging the interior joins so the strokes meet cleanly
/// (ADR-0041).
fn polyline(
    geometry: &mut DrawGeometry,
    points: &[Point],
    width: f32,
    closed: bool,
    additive: bool,
) {
    let n = points.len();
    if n < 2 {
        return;
    }
    let last = if closed { n } else { n - 1 };
    for i in 0..last {
        let Some((a, light)) = points.get(i) else {
            continue;
        };
        let Some((b, _)) = points.get((i + 1) % n) else {
            continue;
        };
        let ext_a = if closed || i > 0 { width } else { 0.0 };
        let ext_b = if closed || i + 2 < n { width } else { 0.0 };
        geometry.push_segment(
            SegmentInstance {
                a: *a,
                b: *b,
                color: light.rgb,
                width,
                alpha: light.coverage,
                ext_a,
                ext_b,
            },
            additive,
        );
    }
}

/// Emit one built trace the way the mode asked for it: separated marks when
/// `wave_usedots` is set, a continuous stroke otherwise.
///
/// **This exists because there were four call sites and one of them forgot**
/// (Plan 0108 Phase 4). `wave_mode 5` draws its figure in two passes and its
/// first pass called [`polyline`] unconditionally, so a preset asking for dots
/// got a continuous stroke above `wave_y` and beads below it. Nothing failed and
/// nothing warned; the trace was simply half wrong. The defect was found by
/// measurement — mode 5's dotted geometry held segments **13.6x longer than
/// [`DOT_LENGTH`]** where every other mode's held none — and the repair is to
/// leave one place where the choice is made.
fn emit_trace(
    geometry: &mut DrawGeometry,
    points: &[Point],
    width: f32,
    closed: bool,
    additive: bool,
    use_dots: bool,
) {
    if use_dots {
        dots(geometry, points, width, additive);
    } else {
        polyline(geometry, points, width, closed, additive);
    }
}

/// Push each point as its own dot.
///
/// **Both ends are extended by a half-width, and that is what makes a dot a
/// dot** (Plan 0108 Phase 4). The line renderer's falloff runs *across* the
/// stroke only; the quad simply ends at each endpoint unless `ext_a`/`ext_b`
/// push it past (ADR-0158). Without that extension a mark is a hard-edged
/// [`DOT_LENGTH`] x `2 * width` rectangle — **3.3x wider than it is long**, a
/// sub-pixel dash lying across the trace rather than the round dot this module's
/// header describes. Measured at 1080p on one drawn frame, that cost 300 pixels
/// above half brightness against a continuous stroke's 5 008, and at 320x180 it
/// left **2**, which is design-backlog 0107's "the `wave_usedots` beads never
/// appear".
///
/// With both ends extended the quad grows by the half-width at each cap, so the
/// mark is `DOT_LENGTH + 2 * width` long against `2 * width` across — round
/// enough that the falloff reads as a bead at any resolution, through the
/// mechanism that already exists rather than a new constant.
///
/// The mark is also **centred** on its point rather than growing forward from
/// it. Half of [`DOT_LENGTH`] is well under a pixel, so this moves nothing
/// visible; it is here because a dot that is offset from the sample it stands
/// for is wrong in a way nobody would ever see and everybody would inherit.
fn dots(geometry: &mut DrawGeometry, points: &[Point], width: f32, additive: bool) {
    for (p, light) in points {
        geometry.push_segment(
            SegmentInstance {
                a: [p[0] - DOT_LENGTH * 0.5, p[1]],
                b: [p[0] + DOT_LENGTH * 0.5, p[1]],
                color: light.rgb,
                width,
                alpha: light.coverage,
                // A cap, not a miter: this is the half-width that rounds the
                // mark, and a zero-length segment has no interior angle to
                // compute one from.
                ext_a: width,
                ext_b: width,
            },
            additive,
        );
    }
}

/// How many `wave_mode` figures there are — MilkDrop's own eight.
pub const WAVE_MODES: u32 = 8;

/// The factor between the source's sample term and what the host people run
/// actually draws.
///
/// `foo_vis_milk2` 0.2.0.0, `wave_mode = 6`, a full-scale 200 Hz sine at
/// `fWaveScale = 1`, captured by Plan 0127: the trace measured **0.316 frame
/// heights peak to peak**, over a stroke whose own width read `0.0019`. The
/// source's mode-6 coefficient is `0.25` clip along the line's normal, which is
/// `0.125` frame heights per unit sample, so the factor is
/// `((0.316 - 0.0019) / 2) / 0.125`.
///
/// **It multiplies the sample term and nothing else** — never a base radius, a
/// separation or an extent (ADR-0199). It is measured on one mode and inferred
/// for the other seven, on the argument that the host's gap is in the sample path
/// every mode shares; the confirmation is a unit-scale mode-0 capture that
/// Plan 0142's rig session owes.
const HOST_SAMPLE_FACTOR: f32 = 1.256;

/// A whole turn, **as the source writes it** (`milkdropfs.cpp` l.2886-2948):
/// `6.28`, not `TAU`.
///
/// The difference is 0.05 % of a turn over the 239 steps modes 0 and 1 lay down,
/// so the last point falls an eighth of a step short of the first. Mode 0's
/// cosine blend is what closes that join, and writing `TAU` here would move every
/// converted circle by that eighth of a step for no reason but tidiness.
#[allow(
    clippy::approx_constant,
    reason = "the source writes 6.28 and the truncation is the behaviour, not a typo for TAU"
)]
const SOURCE_TURN: f32 = 6.28;

/// The largest figure the source builds, in constructed points: modes 2, 3, 4
/// and 5 at 480. [`smooth_wave`] turns `n` of them into `2n - 1`.
const MAX_WAVE_POINTS: usize = 480;
/// See [`MAX_WAVE_POINTS`].
const MAX_SMOOTHED_POINTS: usize = 2 * MAX_WAVE_POINTS - 1;

/// `CPlugin::DrawWave`'s `SmoothWave` (`milkdropfs.cpp` l.2549-2577), applied to
/// every built-in figure once it is constructed (l.3319-3335) and to each side of
/// mode 7's break separately.
///
/// It inserts one point between each consecutive pair, at
/// `(-0.15 v[i-1] + 1.15 v[i] + 1.15 v[i+1] - 0.15 v[i+2]) / 2` with the indices
/// clamped at the ends, so `n` points become `2n - 1` and **every original point
/// keeps its position, at an even index**. That last property is what lets a test
/// read the figure the mode built rather than the curve through it.
///
/// The interpolation is affine in the points, so running it in uv rather than in
/// the line renderer's world space is the same arithmetic — `uv_to_world` is
/// itself affine.
fn smooth_wave(src: &[[f32; 2]], dst: &mut [[f32; 2]; MAX_SMOOTHED_POINTS]) -> usize {
    if src.len() < 2 {
        for (slot, point) in dst.iter_mut().zip(src) {
            *slot = *point;
        }
        return src.len();
    }
    let at = |i: isize| -> [f32; 2] {
        let clamped = i.clamp(0, src.len() as isize - 1) as usize;
        src.get(clamped).copied().unwrap_or([0.0; 2])
    };
    let mut used = 0usize;
    for i in 0..src.len() {
        if let Some(slot) = dst.get_mut(used) {
            *slot = at(i as isize);
        }
        used += 1;
        if i + 1 == src.len() {
            break;
        }
        let (a, b, c, d) = (
            at(i as isize - 1),
            at(i as isize),
            at(i as isize + 1),
            at(i as isize + 2),
        );
        if let Some(slot) = dst.get_mut(used) {
            *slot = [
                (-0.15 * a[0] + 1.15 * b[0] + 1.15 * c[0] - 0.15 * d[0]) * 0.5,
                (-0.15 * a[1] + 1.15 * b[1] + 1.15 * c[1] - 0.15 * d[1]) * 0.5,
            ];
        }
        used += 1;
    }
    used.min(MAX_SMOOTHED_POINTS)
}

/// The source's `wave_mystery` fold, `milkdropfs.cpp` l.2869-2877 — **modes 0, 1
/// and 4 only**, which is why it is a function rather than applied to the output
/// on the way in. Modes 6 and 7 read the value unfolded, and the rest ignore it.
fn fold_mystery(m: f32) -> f32 {
    if !m.is_finite() {
        return 0.0;
    }
    m - 2.0 * ((m + 1.0) * 0.5).floor()
}

/// The built-in waveform: MilkDrop's eight `wave_mode` figures, each built the
/// way `CPlugin::DrawWave` builds it (`milkdropfs.cpp` l.2765-3259).
///
/// # The two coordinate conventions, and which modes use which
///
/// The source works in a clip space where `-1..1` is the whole frame on **both**
/// axes. Modes 0, 1, 2, 3 and 5 multiply their x offset by `m_fAspectY` and their
/// y offset by `m_fAspectX` before placing it, which puts those figures in
/// shorter-axis units and makes a circle round; modes 4, 6 and 7 apply no aspect
/// term at all, so their extents are the frame's own. Both are reproduced here,
/// and [`uv_to_world`] supplies the one conversion out (ADR-0037).
///
/// # What the eight figures are
///
/// Modes 2 and 3 are **the same geometry, line for line** (l.2950-2976 against
/// l.2977-3004) and differ only in the alpha they draw at (l.2982-2991), which
/// this file does not carry. That is the source's own design and not a gap here.
///
/// Modes 1, 2, 3, 5 and 7 read **both** channels of the analyzer's pair; modes 0
/// and 6 read one.
#[allow(clippy::too_many_arguments)]
fn waveform_figure(
    geometry: &mut DrawGeometry,
    out: &FrameOutputs,
    left: &[f32; WAVE_SAMPLES],
    right: &[f32; WAVE_SAMPLES],
    time: f32,
    exposure: Exposure,
    aspect: f32,
    target_width: u32,
) {
    let additive = out.wave_additive >= 0.5;
    let colour = light(
        out.wave_r,
        out.wave_g,
        out.wave_b,
        out.wave_a,
        out.wave_brighten >= 0.5,
        exposure,
        additive,
    );
    if colour.is_dark() {
        return;
    }
    let stroke = if out.wave_thick >= 0.5 { THICK } else { THIN };
    // Read once and passed to every emit below: the mode that draws in two passes
    // has to make the same choice in both, and reading the flag at each site is
    // how one of them came to be a stroke where the other was beads.
    let use_dots = out.wave_usedots >= 0.5;
    let (cx, cy) = (out.wave_x, out.wave_y);
    let mystery = out.wave_mystery;
    let folded = fold_mystery(mystery);
    let mode = (out.wave_mode.max(0.0) as u32) % WAVE_MODES;

    // The sample term, with the host factor on it and nothing else (ADR-0199).
    let l = |i: usize| left.get(i).copied().unwrap_or(0.0) * HOST_SAMPLE_FACTOR;
    let r = |i: usize| right.get(i).copied().unwrap_or(0.0) * HOST_SAMPLE_FACTOR;

    // Clip to uv. `centred` carries the aspect pair and the preset's centre;
    // `plain` is the frame's own clip space, which modes 4, 6 and 7 place in.
    let (ax, ay) = if aspect >= 1.0 {
        (1.0, 1.0 / aspect)
    } else {
        (aspect, 1.0)
    };
    let centred = |px: f32, py: f32| [cx + 0.5 * px * ay, cy - 0.5 * py * ax];
    let plain = |px: f32, py: f32| [0.5 + 0.5 * px, 0.5 - 0.5 * py];

    let mut pts = [[0.0f32; 2]; MAX_WAVE_POINTS];
    let mut n = 0usize;
    let push = |p: [f32; 2], pts: &mut [[f32; 2]; MAX_WAVE_POINTS], n: &mut usize| {
        if let Some(slot) = pts.get_mut(*n) {
            *slot = p;
            *n += 1;
        }
    };
    // `min(480, texW/3)` and `min(240, texW/3)`: the source refuses to lay more
    // points than a third of the frame's pixels across (l.3005-3065, l.3100-3244).
    let thirds = (target_width / 3).max(2) as usize;

    match mode {
        // 0 — a circle of radius 0.5 clip, breathing with the right channel, and
        // turning at 0.2 rad/s (l.2886-2925). The first 24 points cosine-blend to
        // the sample 240 further along, which is what closes the loop without a
        // step across the join.
        0 => {
            let count = 240usize;
            for i in 0..count {
                let ang = i as f32 / (count - 1) as f32 * SOURCE_TURN + time * 0.2;
                let blend = 24usize;
                let s = if i < blend {
                    let mix = 0.5 - 0.5 * (i as f32 / blend as f32 * std::f32::consts::PI).cos();
                    r(i + 360) * (1.0 - mix) + r(i + 120) * mix
                } else {
                    r(i + 120)
                };
                let rad = 0.5 + 0.4 * s + folded;
                push(centred(rad * ang.cos(), rad * ang.sin()), &mut pts, &mut n);
            }
        }
        // 1 — a polar figure: the right channel drives the radius and the left
        // the angle, turning at 2.3 rad/s (l.2927-2948). Open, unlike mode 0.
        1 => {
            let count = 240usize;
            for i in 0..count {
                let rad = 0.53 + 0.43 * r(i) + folded;
                let ang =
                    i as f32 / (count - 1) as f32 * SOURCE_TURN + 1.57 * l(i + 32) + time * 2.3;
                push(centred(rad * ang.cos(), rad * ang.sin()), &mut pts, &mut n);
            }
        }
        // 2 and 3 — the x-y scope, right against left at a 32-sample offset
        // (l.2950-3004). One construction: the two modes differ in alpha alone.
        2 | 3 => {
            let count = 480usize;
            for i in 0..count {
                push(centred(r(i), l(i + 32)), &mut pts, &mut n);
            }
        }
        // 4 — a horizontal sweep whose y comes from the left channel and whose x
        // is nudged by the right, then run through the source's momentum filter
        // (l.3005-3065). `wave_mystery` sets how much of each point's step
        // carries into the next.
        4 => {
            let count = 480usize.min(thirds);
            let offset = (480 - count) / 2;
            let w1 = 0.45 + 0.5 * (folded * 0.5 + 0.5);
            let w2 = 1.0 - w1;
            let mut dx = [0.0f32; MAX_WAVE_POINTS];
            let mut dy = [0.0f32; MAX_WAVE_POINTS];
            for i in 0..count {
                if let (Some(sx), Some(sy)) = (dx.get_mut(i), dy.get_mut(i)) {
                    *sx = 0.44 * r(i + 25 + offset);
                    *sy = 0.47 * l(i + offset);
                }
            }
            for series in [&mut dx, &mut dy] {
                for i in 2..count {
                    let (a, b) = (
                        series.get(i - 1).copied().unwrap_or(0.0),
                        series.get(i - 2).copied().unwrap_or(0.0),
                    );
                    if let Some(slot) = series.get_mut(i) {
                        *slot = *slot * w2 + w1 * (2.0 * a - b);
                    }
                }
            }
            for i in 0..count {
                let along = -1.0 + 2.0 * i as f32 / count.max(2) as f32;
                let px = along + (cx * 2.0 - 1.0) + dx.get(i).copied().unwrap_or(0.0);
                let py = (cy * 2.0 - 1.0) + dy.get(i).copied().unwrap_or(0.0);
                push(plain(px, py), &mut pts, &mut n);
            }
        }
        // 5 — the source's product figure, the two channels multiplied into each
        // other and the whole thing turned at 0.3 rad/s (l.3067-3098).
        5 => {
            let count = 480usize;
            let (s, c) = (time * 0.3).sin_cos();
            for i in 0..count {
                let x0 = r(i) * l(i + 32) + l(i) * r(i + 32);
                let y0 = r(i) * r(i) - l(i + 32) * l(i + 32);
                push(centred(x0 * c - y0 * s, x0 * s + y0 * c), &mut pts, &mut n);
            }
        }
        // 6 and 7 — a line at an angle `wave_mystery` sets, run across the frame
        // and clipped to just outside it, with the left channel displacing it
        // along its own normal (l.3100-3244). Mode 7 draws the same line twice,
        // one channel each, pushed apart by `wave_y` squared; the two sides are a
        // break in the figure rather than one polyline, so they are emitted and
        // smoothed separately.
        //
        // **`wave_mystery` is read unfolded here** and `wave_x` slides the line
        // along its NORMAL rather than along itself, which is the source's own
        // use of the two and not a transcription slip.
        6 | 7 => {
            let count = 240usize.min(thirds);
            let ang = 1.57 * mystery;
            let (sa, ca) = ang.sin_cos();
            let dir = [ca, sa];
            let nrm = [-sa, ca];
            let slide = cx * 2.0 - 1.0;
            let (t0, t1) = clip_to_frame(dir, [nrm[0] * slide, nrm[1] * slide]);
            let sep = cy * cy;
            let sides: &[(f32, bool)] = if mode == 7 {
                &[(sep, true), (-sep, false)]
            } else {
                &[(0.0, true)]
            };
            let mut smoothed = [[0.0f32; 2]; MAX_SMOOTHED_POINTS];
            for (offset, from_left) in sides {
                n = 0;
                for i in 0..count {
                    let t = t0 + (t1 - t0) * i as f32 / (count - 1).max(1) as f32;
                    let sample = if *from_left { l(i + 120) } else { r(i + 120) };
                    let d = 0.25 * sample + offset;
                    let px = dir[0] * t + nrm[0] * (slide + d);
                    let py = dir[1] * t + nrm[1] * (slide + d);
                    push(plain(px, py), &mut pts, &mut n);
                }
                let built = smooth_wave(pts.get(..n).unwrap_or(&[]), &mut smoothed);
                emit_uv(
                    geometry,
                    smoothed.get(..built).unwrap_or(&[]),
                    colour,
                    stroke,
                    false,
                    additive,
                    use_dots,
                    aspect,
                );
            }
            return;
        }
        _ => {}
    }

    let mut smoothed = [[0.0f32; 2]; MAX_SMOOTHED_POINTS];
    let built = smooth_wave(pts.get(..n).unwrap_or(&[]), &mut smoothed);
    emit_uv(
        geometry,
        smoothed.get(..built).unwrap_or(&[]),
        colour,
        stroke,
        false,
        additive,
        use_dots,
        aspect,
    );
}

/// Where a line through `offset` in direction `dir` enters and leaves the source's
/// `+/-1.1` clip box (`milkdropfs.cpp` l.3100-3244) — slightly outside the frame,
/// so a rotated trace runs past the corners rather than stopping short of them.
///
/// Returns the parameter range along `dir`, clamped to the source's own
/// `-3..3` extent when the line is axis-parallel and one axis never bounds it.
fn clip_to_frame(dir: [f32; 2], offset: [f32; 2]) -> (f32, f32) {
    const EDGE: f32 = 1.1;
    const REACH: f32 = 3.0;
    let (mut lo, mut hi) = (-REACH, REACH);
    for axis in 0..2 {
        let (d, o) = (
            dir.get(axis).copied().unwrap_or(0.0),
            offset.get(axis).copied().unwrap_or(0.0),
        );
        if d.abs() < 1e-6 {
            continue;
        }
        let (a, b) = ((-EDGE - o) / d, (EDGE - o) / d);
        lo = lo.max(a.min(b));
        hi = hi.min(a.max(b));
    }
    if hi <= lo { (-REACH, REACH) } else { (lo, hi) }
}

/// Convert a figure built in uv into the line renderer's world space and emit it.
#[allow(clippy::too_many_arguments)]
fn emit_uv(
    geometry: &mut DrawGeometry,
    uv: &[[f32; 2]],
    colour: Light,
    width: f32,
    closed: bool,
    additive: bool,
    use_dots: bool,
    aspect: f32,
) {
    let mut points: Vec<Point> = Vec::with_capacity(uv.len());
    for p in uv {
        points.push((uv_to_world(p[0], p[1], aspect), colour));
    }
    emit_trace(geometry, &points, width, closed, additive, use_dots);
}

/// The preset's custom waves, each a polyline or a scatter from its own
/// per-point program.
fn custom_waves(
    geometry: &mut DrawGeometry,
    runtime: &mut MilkRuntime,
    left: &[f32; WAVE_SAMPLES],
    right: &[f32; WAVE_SAMPLES],
    exposure: Exposure,
    aspect: f32,
) {
    for index in 0..runtime.wave_count() {
        let Some(spec) = runtime.wave_spec(index) else {
            continue;
        };
        if runtime.run_wave_frame(index).is_none() {
            continue;
        }
        let count = spec.count.max(2) as usize;
        let mut points: Vec<Point> = Vec::with_capacity(count);
        for i in 0..count {
            let t = i as f32 / (count - 1).max(1) as f32;
            // The audio at this point along the wave — MilkDrop's `value1` and
            // `value2`, which are its left and right channels and are two
            // different numbers on a stereo stream (ADR-0199). A per-point
            // program that plots one against the other draws the figure its
            // author saw rather than a diagonal line.
            let at = ((t * (WAVE_SAMPLES - 1) as f32) as usize).min(WAVE_SAMPLES - 1);
            let value1 = left.get(at).copied().unwrap_or(0.0);
            let value2 = right.get(at).copied().unwrap_or(0.0);
            let Some(point) = runtime.run_wave_point(index, t, value1, value2) else {
                break;
            };
            points.push((
                uv_to_world(point.x, point.y, aspect),
                light(
                    point.r,
                    point.g,
                    point.b,
                    point.a,
                    false,
                    exposure,
                    spec.additive,
                ),
            ));
        }
        let width = if spec.thick { THICK } else { THIN };
        if spec.use_dots {
            dots(geometry, &points, width, spec.additive);
        } else {
            polyline(geometry, &points, width, false, spec.additive);
        }
    }
}

/// The preset's custom shapes: filled polygons with an optional outline.
fn custom_shapes(
    geometry: &mut DrawGeometry,
    runtime: &mut MilkRuntime,
    exposure: Exposure,
    aspect: f32,
) {
    for index in 0..runtime.shape_count() {
        let Some(spec) = runtime.shape_spec(index) else {
            continue;
        };
        for instance in 0..spec.instances {
            let Some(shape) = runtime.run_shape_instance(index, instance) else {
                break;
            };
            // Per **instance**, not per element: `additive` is one of the
            // registers the shape's own per-frame program may write, so one
            // shape's copies can blend differently from each other.
            let additive = shape.additive >= 0.5;
            let sides = (shape.sides.max(3.0) as u32).clamp(3, MAX_SHAPE_SIDES);
            let centre = uv_to_world(shape.x, shape.y, aspect);
            let inner = light(
                shape.r, shape.g, shape.b, shape.a, false, exposure, additive,
            );
            let outer = light(
                shape.r2, shape.g2, shape.b2, shape.a2, false, exposure, additive,
            );
            // The perimeter, in world space. `rad` is in frame-heights, so the
            // aspect only enters through the centre — which is what keeps a
            // shape round rather than stretched.
            let point = |i: u32| -> [f32; 2] {
                let t = shape.ang + i as f32 / sides as f32 * std::f32::consts::TAU;
                [
                    centre[0] + shape.rad * t.cos() * aspect,
                    centre[1] + shape.rad * t.sin(),
                ]
            };
            // A triangle fan, emitted as a plain list so the whole draw layer is
            // one buffer, in the blend mode this instance asked for.
            for i in 0..sides {
                geometry.push_triangle(
                    [
                        ShapeVertex {
                            pos: centre,
                            color: inner.rgb,
                            alpha: inner.coverage,
                        },
                        ShapeVertex {
                            pos: point(i),
                            color: outer.rgb,
                            alpha: outer.coverage,
                        },
                        ShapeVertex {
                            pos: point((i + 1) % sides),
                            color: outer.rgb,
                            alpha: outer.coverage,
                        },
                    ],
                    additive,
                );
            }
            // ...and the outline, through the same line batch as everything else.
            if shape.border_a > 0.0001 {
                let colour = light(
                    shape.border_r,
                    shape.border_g,
                    shape.border_b,
                    shape.border_a,
                    false,
                    exposure,
                    additive,
                );
                let outline: Vec<Point> = (0..sides).map(|i| (point(i), colour)).collect();
                let width = if shape.thick_outline >= 0.5 || spec.thick {
                    THICK
                } else {
                    THIN
                };
                polyline(geometry, &outline, width, true, additive);
            }
        }
    }
}

/// The inner and outer borders: two rectangles inset from the frame edge.
fn borders(geometry: &mut DrawGeometry, out: &FrameOutputs, exposure: Exposure, aspect: f32) {
    for (size, r, g, b, a, inset) in [
        (out.ob_size, out.ob_r, out.ob_g, out.ob_b, out.ob_a, 0.0),
        (
            out.ib_size,
            out.ib_r,
            out.ib_g,
            out.ib_b,
            out.ib_a,
            out.ob_size,
        ),
    ] {
        if a <= 0.0001 || size <= 0.0 {
            continue;
        }
        let colour = light(r, g, b, a, false, exposure, false);
        // The rectangle sits at the middle of its own band, and the stroke is the
        // band's whole width — which is how a border of `size` reads as a band of
        // `size` rather than as a hairline.
        let half = (size * 0.5).clamp(0.0005, 0.4);
        let edge = inset + half;
        let corners = [
            (edge, edge),
            (1.0 - edge, edge),
            (1.0 - edge, 1.0 - edge),
            (edge, 1.0 - edge),
        ];
        let points: Vec<Point> = corners
            .iter()
            .map(|(u, v)| (uv_to_world(*u, *v, aspect), colour))
            .collect();
        polyline(geometry, &points, half * 2.0, true, false);
    }
}

/// The motion-vector grid: a lattice of short strokes showing where the warp is
/// taking the frame.
///
/// MilkDrop samples its own warp mesh to draw these. Here the grid is drawn from
/// the same `mv_*` vocabulary at the positions the preset names, with `mv_l`
/// setting the length — the figure a preset asks for, without a second
/// evaluation of the per-vertex program per grid point.
fn motion_vectors(
    geometry: &mut DrawGeometry,
    out: &FrameOutputs,
    exposure: Exposure,
    aspect: f32,
) {
    if out.mv_a <= 0.0001 {
        return;
    }
    let nx = (out.mv_x.max(0.0) as u32).min(64);
    let ny = (out.mv_y.max(0.0) as u32).min(48);
    if nx == 0 || ny == 0 {
        return;
    }
    let colour = light(
        out.mv_r, out.mv_g, out.mv_b, out.mv_a, false, exposure, false,
    );
    let len = out.mv_l * 0.02;
    for iy in 0..ny {
        for ix in 0..nx {
            let u = (ix as f32 + 0.5 + out.mv_dx) / nx as f32;
            let v = (iy as f32 + 0.5 + out.mv_dy) / ny as f32;
            let a = uv_to_world(u, v, aspect);
            geometry.push_segment(
                SegmentInstance {
                    a,
                    b: [a[0] + len * aspect, a[1] + len],
                    color: colour.rgb,
                    width: THIN,
                    alpha: colour.coverage,
                    ext_a: 0.0,
                    ext_b: 0.0,
                },
                false,
            );
        }
    }
}
