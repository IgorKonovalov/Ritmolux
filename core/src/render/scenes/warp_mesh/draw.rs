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
//!   [`JOINED_A`]`|`[`JOINED_B`] push the quad past both ends by the half-width
//!   (ADR-0041), not because it is short. Without the flags it is a sub-pixel
//!   dash; see [`dots`].
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
use crate::render::scenes::lines::{JOINED_A, JOINED_B, SegmentInstance};

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
/// the module docs. Producers are appended through [`push_segment`](Self::push_segment)
/// and [`push_triangle`](Self::push_triangle) rather than to the vectors
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
/// **`_time` is read by nothing here, and that is the contract** (Plan 0109
/// Phase 2): every figure this layer builds is a pure function of the trace and
/// the frame outputs. The parameter stays in the signature because the scene has
/// the value and because time-independence is a claim worth being able to *test*
/// — `draw_layer.rs` calls this twice at well-separated times and compares the
/// geometry. A future mode that legitimately animates would rename it back, and
/// would owe that test a reason.
pub fn build(
    geometry: &mut DrawGeometry,
    runtime: Option<&mut MilkRuntime>,
    out: &FrameOutputs,
    waveform: &[f32; WAVE_SAMPLES],
    _time: f32,
    dt: f32,
    aspect: f32,
) {
    geometry.clear();
    let Some(runtime) = runtime else {
        return;
    };
    let exposure = Exposure::new(dt);
    waveform_figure(geometry, out, waveform, exposure, aspect);
    custom_waves(geometry, runtime, waveform, exposure, aspect);
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
/// [`scale`](Self::scale) convert through it.
#[derive(Clone, Copy)]
pub struct Exposure {
    /// `dt * NOMINAL_FPS`: how many nominal frames of wall clock this frame is.
    rate: f32,
}

impl Exposure {
    /// From this frame's `dt`. A degenerate frame is worth one nominal frame
    /// rather than nothing.
    pub fn new(dt: f32) -> Self {
        let rate = if dt.is_finite() && dt > 0.0 {
            (dt * crate::milk::NOMINAL_FPS).clamp(0.0, 4.0)
        } else {
            1.0
        };
        Self { rate }
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
        let mut joined = 0;
        if closed || i > 0 {
            joined |= JOINED_A;
        }
        if closed || i + 2 < n {
            joined |= JOINED_B;
        }
        geometry.push_segment(
            SegmentInstance {
                a: *a,
                b: *b,
                color: light.rgb,
                width,
                alpha: light.coverage,
                joined,
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
/// **Both ends are flagged joined, and that is what makes a dot a dot** (Plan
/// 0108 Phase 4). The line renderer's falloff runs *across* the stroke only; the
/// quad simply ends at each endpoint unless [`JOINED_A`]/[`JOINED_B`] push it
/// past by the half-width (ADR-0041). Without those flags a mark is a hard-edged
/// [`DOT_LENGTH`] x `2 * width` rectangle — **3.3x wider than it is long**, a
/// sub-pixel dash lying across the trace rather than the round dot this module's
/// header describes. Measured at 1080p on one drawn frame, that cost 300 pixels
/// above half brightness against a continuous stroke's 5 008, and at 320x180 it
/// left **2**, which is design-backlog 0107's "the `wave_usedots` beads never
/// appear".
///
/// With both ends flagged the quad extends by the half-width at each cap, so the
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
                joined: JOINED_A | JOINED_B,
            },
            additive,
        );
    }
}

/// How many `wave_mode` figures there are — MilkDrop's own eight.
pub const WAVE_MODES: u32 = 8;

/// The built-in waveform: MilkDrop's eight `wave_mode` figures over the audio
/// trace.
///
/// Each mode is the reference's own construction, and the eight are **pairwise
/// distinct figures** — a circle, a pair of rings, a scope, a Lissajous, a
/// mirrored pair, an angled line and its double — which is what Phase 4's
/// done-when asks and what
/// [`every_wave_mode_builds_a_different_figure`](super::tests) holds them to.
///
/// Two pairs had to be separated to get there, and both for the same reason: the
/// reference tells 0 from 1, and 6 from 7, using the **second audio channel**,
/// and this engine's analysis is mono by construction. Where the reference would
/// draw two diverging traces, this draws the one trace at the separation the
/// reference's own parameters name — the same figure with the channel difference
/// removed rather than an invented eighth mode. See each arm.
fn waveform_figure(
    geometry: &mut DrawGeometry,
    out: &FrameOutputs,
    waveform: &[f32; WAVE_SAMPLES],
    exposure: Exposure,
    aspect: f32,
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
    let width = if out.wave_thick >= 0.5 { THICK } else { THIN };
    // Read once and passed to every [`emit_trace`] below: the modes that draw in
    // two passes have to make the same choice in both, and reading the flag at
    // each site is how one of them came to be a stroke where the other was beads.
    let use_dots = out.wave_usedots >= 0.5;
    let scale = out.wave_scale;
    let mystery = out.wave_mystery;
    let (cx, cy) = (out.wave_x, out.wave_y);
    // `wave_smoothing` is a running average along the trace, exactly as the
    // reference's `fWaveSmoothing` is: 0 is the raw samples and 1 is a straight
    // line.
    let smooth = out.wave_smoothing.clamp(0.0, 0.99);

    // How many points this mode draws, and the sampled trace it draws them from.
    let mode = (out.wave_mode.max(0.0) as u32) % WAVE_MODES;
    let count: usize = match mode {
        // The two "spectrum"-ish modes draw a coarser figure, as the reference
        // does — a 512-point circle at 480 px is denser than the frame.
        3 | 4 => 128,
        _ => 256,
    };

    let mut trace = [0.0f32; 256];
    let mut held = 0.0f32;
    for (i, slot) in trace.iter_mut().enumerate().take(count) {
        let source = waveform
            .get(i * WAVE_SAMPLES / count.max(1))
            .copied()
            .unwrap_or(0.0);
        held = held * smooth + source * (1.0 - smooth);
        *slot = held * scale;
    }
    let sample = |i: usize| trace.get(i).copied().unwrap_or(0.0);

    let mut points: [Point; 256] = [([0.0; 2], colour); 256];
    let mut used = 0usize;
    let mut closed = false;
    let push = |uv: (f32, f32), points: &mut [Point; 256], used: &mut usize| {
        if let Some(slot) = points.get_mut(*used) {
            slot.0 = uv_to_world(uv.0, uv.1, aspect);
            *used += 1;
        }
    };

    match mode {
        // 0 — a circle whose radius breathes with the trace. MilkDrop's first
        // mode and the one most presets use.
        0 => {
            closed = true;
            let base = 0.2 + 0.1 * mystery;
            for i in 0..count {
                let t = i as f32 / count as f32 * std::f32::consts::TAU;
                let r = base + sample(i) * 0.1;
                push(
                    (cx + r * t.cos() / aspect.max(0.1), cy + r * t.sin()),
                    &mut points,
                    &mut used,
                );
            }
        }
        // 1 — the reference's **second** circular mode, which draws the left and
        // right channels as two rings whose separation is `wave_mystery`. This
        // engine's analysis is mono (see `MilkRuntime::run_wave_point`), so the
        // two rings carry the same trace and only the separation tells them
        // apart — which is the reference's own figure with the channel
        // difference removed, and is what keeps mode 1 from being mode 0.
        1 => {
            closed = true;
            let base = 0.2 + 0.1 * mystery;
            let separation = 0.04 + 0.06 * mystery.abs();
            for ring in [separation, -separation] {
                used = 0;
                for i in 0..count {
                    let t = i as f32 / count as f32 * std::f32::consts::TAU;
                    let r = base + ring + sample(i) * 0.1;
                    push(
                        (cx + r * t.cos() / aspect.max(0.1), cy + r * t.sin()),
                        &mut points,
                        &mut used,
                    );
                }
                // The outer ring closes here; the inner one falls through to the
                // shared emit below, so both are stroked exactly once.
                if ring > 0.0 {
                    let built = points.get(..used).unwrap_or(&[]);
                    emit_trace(geometry, built, width, true, additive, use_dots);
                }
            }
        }
        // 2 — a horizontal line across the frame, the classic scope.
        2 => {
            for i in 0..count {
                let u = i as f32 / (count - 1).max(1) as f32;
                push((u, cy + sample(i) * 0.15), &mut points, &mut used);
            }
        }
        // 3 — the same line, vertical.
        3 => {
            for i in 0..count {
                let v = i as f32 / (count - 1).max(1) as f32;
                push((cx + sample(i) * 0.15, v), &mut points, &mut used);
            }
        }
        // 4 — a Lissajous-style figure: the trace against itself, offset. In
        // MilkDrop this is the left channel against the right; this engine's
        // analysis is mono, so the offset stands in for the channel difference
        // and the figure is a leaning loop rather than a blob (see
        // `MilkRuntime::run_wave_point`).
        4 => {
            let lag = 8usize;
            for i in 0..count {
                push(
                    (
                        cx + sample(i) * 0.2 / aspect.max(0.1),
                        cy + sample((i + lag) % count) * 0.2,
                    ),
                    &mut points,
                    &mut used,
                );
            }
        }
        // 5 — a double horizontal line, mirrored about `wave_y`.
        5 => {
            for i in 0..count {
                let u = i as f32 / (count - 1).max(1) as f32;
                let s = sample(i).abs() * 0.15;
                push((u, cy + s), &mut points, &mut used);
            }
            emit_trace(
                geometry,
                points.get(..used).unwrap_or(&[]),
                width,
                false,
                additive,
                use_dots,
            );
            used = 0;
            for i in 0..count {
                let u = i as f32 / (count - 1).max(1) as f32;
                let s = sample(i).abs() * 0.15;
                push((u, cy - s), &mut points, &mut used);
            }
        }
        // 6 — a line at an angle set by `wave_mystery`, which is what the
        // reference uses it for here, and 7 — the reference's **double** line, the
        // same figure offset to both sides along its own normal. That is exactly
        // the relationship mode 5 has to mode 2, so the pair is consistent with
        // the pair above it rather than being two names for one figure.
        6 | 7 => {
            // **`time * 0.05` was here until Plan 0109 Phase 2** (design-backlog
            // 0115). It rotated the figure a full turn every ~126 s, so a trace
            // authored horizontal was horizontal only at the instants
            // `mystery * PI + time * 0.05` happened to be a multiple of `pi`.
            // Plan 0108 Phase 4 named it a suspect and deliberately left it in,
            // because removing it moves every mode-6 and mode-7 preset and
            // because whether the reference's line drifts is a question about
            // the reference. Plan 0108 Phase 6 asked it: *Blur Mix 3*'s traces
            // stay horizontal in `foo_vis_milk2` and drew one steep diagonal
            // here. So the angle is what the sentence above always said it was —
            // `wave_mystery` alone — and this file is now a pure function of the
            // trace and the outputs, with no use of `time` anywhere in it.
            let angle = mystery * std::f32::consts::PI;
            let (s, c) = angle.sin_cos();
            let offsets: &[f32] = if mode == 7 { &[0.03, -0.03] } else { &[0.0] };
            for (index, offset) in offsets.iter().enumerate() {
                used = 0;
                for i in 0..count {
                    let t = i as f32 / (count - 1).max(1) as f32 - 0.5;
                    let n = sample(i) * 0.15 + offset;
                    push(
                        (cx + (t * c - n * s) / aspect.max(0.1), cy + (t * s + n * c)),
                        &mut points,
                        &mut used,
                    );
                }
                // All but the last pass emit here; the last falls through to the
                // shared emit below.
                if index + 1 < offsets.len() {
                    let built = points.get(..used).unwrap_or(&[]);
                    emit_trace(geometry, built, width, false, additive, use_dots);
                }
            }
        }
        _ => {}
    }

    let built = points.get(..used).unwrap_or(&[]);
    emit_trace(geometry, built, width, closed, additive, use_dots);
}

/// The preset's custom waves, each a polyline or a scatter from its own
/// per-point program.
fn custom_waves(
    geometry: &mut DrawGeometry,
    runtime: &mut MilkRuntime,
    waveform: &[f32; WAVE_SAMPLES],
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
            // The audio at this point along the wave — MilkDrop's `value1`.
            let value = waveform
                .get(((t * (WAVE_SAMPLES - 1) as f32) as usize).min(WAVE_SAMPLES - 1))
                .copied()
                .unwrap_or(0.0);
            let Some(point) = runtime.run_wave_point(index, t, value) else {
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
                    joined: 0,
                },
                false,
            );
        }
    }
}
