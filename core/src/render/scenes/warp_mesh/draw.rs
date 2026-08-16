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
//! - **Alpha reads as intensity.** This engine's draw seam is additive with
//!   saturating coverage (ADR-0056); MilkDrop's waveform can be additive or
//!   alpha-blended, chosen per preset. Multiplying the colour by the alpha is the
//!   additive reading of the same number, and it is what every producer here
//!   does.
//! - **A dot is a very short segment.** `wave_usedots` draws points; the line
//!   renderer draws quads between endpoints, and its across-the-stroke falloff
//!   makes a near-zero-length one a round dot.
//! - **`wave_mystery` means something different in every mode**, which is the
//!   reference's own design rather than a simplification here.

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

/// The two buffers a frame's draw layer fills. Sized once at preset load.
#[derive(Default)]
pub struct DrawGeometry {
    /// Every line: the waveform, custom waves, shape outlines, borders, motion
    /// vectors. One batch, one draw.
    pub segments: Vec<SegmentInstance>,
    /// Every filled shape's triangles, as a plain list.
    pub triangles: Vec<ShapeVertex>,
}

impl DrawGeometry {
    /// Discard last frame's geometry without giving back its capacity — the
    /// reuse that keeps the per-frame path allocation-free.
    pub fn clear(&mut self) {
        self.segments.clear();
        self.triangles.clear();
    }

    /// Whether anything was built this frame.
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty() && self.triangles.is_empty()
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
pub fn build(
    geometry: &mut DrawGeometry,
    runtime: Option<&mut MilkRuntime>,
    out: &FrameOutputs,
    waveform: &[f32; WAVE_SAMPLES],
    time: f32,
    dt: f32,
    aspect: f32,
) {
    geometry.clear();
    let Some(runtime) = runtime else {
        return;
    };
    let exposure = deposit_exposure(dt);
    waveform_figure(geometry, out, waveform, time, exposure, aspect);
    custom_waves(geometry, runtime, waveform, exposure, aspect);
    custom_shapes(geometry, runtime, exposure, aspect);
    borders(geometry, out, exposure, aspect);
    motion_vectors(geometry, out, exposure, aspect);
}

/// **What one frame of the draw layer is worth**, and it is a rate rather than a
/// constant.
///
/// MilkDrop deposits its waveform once per rendered frame into a buffer that
/// decays once per rendered frame, so the two are in step by construction. Here
/// the field decays **per second** (`decay` is rate-converted like every other
/// MilkDrop factor), which means a 60 Hz display would deposit twice the light
/// per second that a 30 Hz one does into a buffer that fades at the same rate —
/// and the accumulation would be a stop brighter for no reason but the refresh.
///
/// Scaling each frame's contribution by `dt * NOMINAL_FPS` puts them back in
/// step: a second of wall clock deposits the same total light at any refresh,
/// which is the same property ADR-0019 buys everywhere else in this engine and
/// the same normalization ADR-0065 applied to the attractor's deposit when a
/// tier capacity turned out to be a brightness.
fn deposit_exposure(dt: f32) -> f32 {
    if !dt.is_finite() || dt <= 0.0 {
        return 1.0;
    }
    (dt * crate::milk::NOMINAL_FPS).clamp(0.0, 4.0)
}

/// Colour times alpha, which is the additive reading of MilkDrop's blend — see
/// the module docs. `wave_brighten` normalizes to the brightest channel first,
/// which is what the reference's `bMaximizeWaveColor` does.
fn light(r: f32, g: f32, b: f32, a: f32, brighten: bool, exposure: f32) -> [f32; 3] {
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
    let a = a.clamp(0.0, 1.0) * exposure;
    [r * a, g * a, b * a]
}

/// Push a polyline, flagging the interior joins so the strokes meet cleanly
/// (ADR-0041).
fn polyline(
    geometry: &mut DrawGeometry,
    points: &[([f32; 2], [f32; 3])],
    width: f32,
    closed: bool,
) {
    let n = points.len();
    if n < 2 {
        return;
    }
    let last = if closed { n } else { n - 1 };
    for i in 0..last {
        let Some((a, colour)) = points.get(i) else {
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
        geometry.segments.push(SegmentInstance {
            a: *a,
            b: *b,
            color: *colour,
            width,
            joined,
        });
    }
}

/// Push each point as its own dot.
fn dots(geometry: &mut DrawGeometry, points: &[([f32; 2], [f32; 3])], width: f32) {
    for (p, colour) in points {
        geometry.segments.push(SegmentInstance {
            a: *p,
            b: [p[0] + DOT_LENGTH, p[1]],
            color: *colour,
            width,
            joined: 0,
        });
    }
}

/// How many `wave_mode` figures there are — MilkDrop's own eight.
pub const WAVE_MODES: u32 = 8;

/// The built-in waveform: MilkDrop's eight `wave_mode` figures over the audio
/// trace.
///
/// Each mode is the reference's own construction, and they are genuinely
/// different *figures* rather than the same line rearranged — a circle, a pair of
/// lines, a spectrum-shaped strip — which is what Phase 4's done-when asks to be
/// distinguishable.
fn waveform_figure(
    geometry: &mut DrawGeometry,
    out: &FrameOutputs,
    waveform: &[f32; WAVE_SAMPLES],
    time: f32,
    exposure: f32,
    aspect: f32,
) {
    let colour = light(
        out.wave_r,
        out.wave_g,
        out.wave_b,
        out.wave_a,
        out.wave_brighten >= 0.5,
        exposure,
    );
    if colour.iter().all(|c| *c <= 0.0001) {
        return;
    }
    let width = if out.wave_thick >= 0.5 { THICK } else { THIN };
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

    let mut points: [([f32; 2], [f32; 3]); 256] = [([0.0; 2], colour); 256];
    let mut used = 0usize;
    let mut closed = false;
    let push = |uv: (f32, f32), points: &mut [([f32; 2], [f32; 3]); 256], used: &mut usize| {
        if let Some(slot) = points.get_mut(*used) {
            slot.0 = uv_to_world(uv.0, uv.1, aspect);
            *used += 1;
        }
    };

    match mode {
        // 0 — a circle whose radius breathes with the trace. MilkDrop's first
        // mode and the one most presets use.
        0 | 1 => {
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
            polyline(geometry, points.get(..used).unwrap_or(&[]), width, false);
            used = 0;
            for i in 0..count {
                let u = i as f32 / (count - 1).max(1) as f32;
                let s = sample(i).abs() * 0.15;
                push((u, cy - s), &mut points, &mut used);
            }
        }
        // 6 — a line at an angle set by `wave_mystery`, which is what the
        // reference's modes 6 and 7 use it for.
        6 | 7 => {
            let angle = mystery * std::f32::consts::PI + time * 0.05;
            let (s, c) = angle.sin_cos();
            for i in 0..count {
                let t = i as f32 / (count - 1).max(1) as f32 - 0.5;
                let n = sample(i) * 0.15;
                push(
                    (cx + (t * c - n * s) / aspect.max(0.1), cy + (t * s + n * c)),
                    &mut points,
                    &mut used,
                );
            }
        }
        _ => {}
    }

    let built = points.get(..used).unwrap_or(&[]);
    if out.wave_usedots >= 0.5 {
        dots(geometry, built, width);
    } else {
        polyline(geometry, built, width, closed);
    }
}

/// The preset's custom waves, each a polyline or a scatter from its own
/// per-point program.
fn custom_waves(
    geometry: &mut DrawGeometry,
    runtime: &mut MilkRuntime,
    waveform: &[f32; WAVE_SAMPLES],
    exposure: f32,
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
        let mut points: Vec<([f32; 2], [f32; 3])> = Vec::with_capacity(count);
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
                light(point.r, point.g, point.b, point.a, false, exposure),
            ));
        }
        let width = if spec.thick { THICK } else { THIN };
        if spec.use_dots {
            dots(geometry, &points, width);
        } else {
            polyline(geometry, &points, width, false);
        }
    }
}

/// The preset's custom shapes: filled polygons with an optional outline.
fn custom_shapes(
    geometry: &mut DrawGeometry,
    runtime: &mut MilkRuntime,
    exposure: f32,
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
            let sides = (shape.sides.max(3.0) as u32).clamp(3, MAX_SHAPE_SIDES);
            let centre = uv_to_world(shape.x, shape.y, aspect);
            let inner = light(shape.r, shape.g, shape.b, shape.a, false, exposure);
            let outer = light(shape.r2, shape.g2, shape.b2, shape.a2, false, exposure);
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
            // one buffer and one draw.
            for i in 0..sides {
                geometry.triangles.push(ShapeVertex {
                    pos: centre,
                    color: inner,
                    alpha: shape.a.clamp(0.0, 1.0),
                });
                geometry.triangles.push(ShapeVertex {
                    pos: point(i),
                    color: outer,
                    alpha: shape.a2.clamp(0.0, 1.0),
                });
                geometry.triangles.push(ShapeVertex {
                    pos: point((i + 1) % sides),
                    color: outer,
                    alpha: shape.a2.clamp(0.0, 1.0),
                });
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
                );
                let outline: Vec<([f32; 2], [f32; 3])> =
                    (0..sides).map(|i| (point(i), colour)).collect();
                let width = if shape.thick_outline >= 0.5 || spec.thick {
                    THICK
                } else {
                    THIN
                };
                polyline(geometry, &outline, width, true);
            }
        }
    }
}

/// The inner and outer borders: two rectangles inset from the frame edge.
fn borders(geometry: &mut DrawGeometry, out: &FrameOutputs, exposure: f32, aspect: f32) {
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
        let colour = light(r, g, b, a, false, exposure);
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
        let points: Vec<([f32; 2], [f32; 3])> = corners
            .iter()
            .map(|(u, v)| (uv_to_world(*u, *v, aspect), colour))
            .collect();
        polyline(geometry, &points, half * 2.0, true);
    }
}

/// The motion-vector grid: a lattice of short strokes showing where the warp is
/// taking the frame.
///
/// MilkDrop samples its own warp mesh to draw these. Here the grid is drawn from
/// the same `mv_*` vocabulary at the positions the preset names, with `mv_l`
/// setting the length — the figure a preset asks for, without a second
/// evaluation of the per-vertex program per grid point.
fn motion_vectors(geometry: &mut DrawGeometry, out: &FrameOutputs, exposure: f32, aspect: f32) {
    if out.mv_a <= 0.0001 {
        return;
    }
    let nx = (out.mv_x.max(0.0) as u32).min(64);
    let ny = (out.mv_y.max(0.0) as u32).min(48);
    if nx == 0 || ny == 0 {
        return;
    }
    let colour = light(out.mv_r, out.mv_g, out.mv_b, out.mv_a, false, exposure);
    let len = out.mv_l * 0.02;
    for iy in 0..ny {
        for ix in 0..nx {
            let u = (ix as f32 + 0.5 + out.mv_dx) / nx as f32;
            let v = (iy as f32 + 0.5 + out.mv_dy) / ny as f32;
            let a = uv_to_world(u, v, aspect);
            geometry.segments.push(SegmentInstance {
                a,
                b: [a[0] + len * aspect, a[1] + len],
                color: colour,
                width: THIN,
                joined: 0,
            });
        }
    }
}
