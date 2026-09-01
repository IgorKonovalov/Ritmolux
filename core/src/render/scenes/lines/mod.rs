//! Line-geometry scenes (ADR-0007): a line-art category built on one shared
//! [`LineRenderer`] (segments -> thick glowing instanced quads) and two build
//! models over it — a cheap **parametric** system sampled every frame (the
//! Maurer rose) and, from Phase 3, an expensive **generator** system built and
//! cached at preset load. Ported in spirit from the user's Maurer rose,
//! L-system, and Islamic-star sketches; none of that JavaScript is reused, only
//! the math.
//!
//! The renderer and the per-frame scene halves are hot-path; the generators
//! (grammar/turtle/Hankin, from later phases) run only at load. All files here
//! live under `render/` and so carry the panic pragma the hygiene guard scans
//! for recursively — the build-time files are written panic-free too.

// Hot-path panic-denial pragma (Plan 0002 Phase 2, extended to scenes by Plan
// 0003 Phase 0). `palette` may be called per frame.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

pub mod biarc;
pub mod curves;
pub mod grammar;
pub mod hankin;
pub mod lsystem;
pub mod parametric;
pub mod renderer;
pub mod spectrum;
pub mod star;
pub mod turtle;

pub use lsystem::LSystemScene;
pub use parametric::ParametricCurveScene;
pub use renderer::{ArcInstance, LineRenderer, SegmentInstance};
pub use spectrum::{SpectrumLayout, SpectrumScene};
pub use star::StarPatternScene;

/// The shared structural-config and cap-overflow types now live one level up, in
/// [`scenes`](super) — every scene family can see them there without the line
/// module having to reach sideways into `particles` for an attractor variant
/// (Plan 0031 Phase 6, closing Plan 0016's close-review minor 2). Re-exported
/// here because the sibling line scenes and the preset schema name them through
/// this path.
pub use super::{CapOverflow, GeneratorConfig, OverflowContext};

/// Maps the `thickness` parameter (a small integer-ish stroke weight) to an
/// NDC-y half-width; `thickness = 2` gives a comfortably thick projector line.
///
/// One constant for all four line scenes, so a `thickness` that reads well on
/// the rose reads the same on the mandala.
pub const WIDTH_SCALE: f32 = 0.003;

/// The smallest half-width a stroke is drawn at, whatever `thickness` asks for.
///
/// It exists to stop a zero or negative `thickness` degenerating the quad into
/// a line of zero area, and it stays: the defect design-backlog 0098 records
/// is the **silence** around it, not the clamp.
pub const MIN_HALF_WIDTH: f32 = 0.0005;

/// The `thickness` at which [`MIN_HALF_WIDTH`] stops binding — about `0.167`.
///
/// **Below this every value renders identically**, because they all clamp to the
/// same floor: a dead zone about 0.27 px wide at 1080p, which rasterizes as a
/// broken dotted line rather than as a stroke. Derived from the two constants
/// above rather than written out, so the load-time warning that quotes it
/// (`preset::schema`) cannot drift from the floor it describes.
pub const MIN_USEFUL_THICKNESS: f32 = MIN_HALF_WIDTH / WIDTH_SCALE;

/// The NDC-y half-width a line scene strokes `thickness` at — the one place the
/// scale and the floor are applied, shared by all four line scenes.
pub fn half_width(thickness: f32) -> f32 {
    (thickness * WIDTH_SCALE).max(MIN_HALF_WIDTH)
}

/// The across-the-stroke profile the **four line families** draw at unless a
/// preset binds `softness` itself (ADR-0124).
///
/// **`0.25` — a solid stroke with a short shoulder — set by Plan 0114 Phase
/// 4's look gate**, which judged the shipped presets side by side at 1920x1080
/// and 1280x800 and then in the running app on real audio. `1.0` — the pure
/// quadratic falloff every line scene drew from Plan 0010 — puts a 4 px spine
/// inside a 10 px gradient, which is the *blurred* verdict that opened Plan
/// 0114.
///
/// **`1.0` remains reachable and is not dead surface.** The same gate returned
/// `1.0` for the Maurer roses, `0` for `curve_ionwake` and `0.25` for
/// `lsystem_vellum` — which is why this is an authorable parameter with a
/// default rather than a constant. A preset that wants the luminous smear
/// binds one number and gets the pre-Plan-0114 fragment back, term for term.
///
/// It is deliberately *not* the value
/// [`warp_mesh`](crate::render::scenes::warp_mesh::MILKDROP_SOFTNESS) passes:
/// that surface is judged against `foo_vis_milk2` rather than against this
/// plan's gate and is pinned at `1.0`, so a reader of either call site can see
/// which judge it serves without leaving the file.
pub const DEFAULT_SOFTNESS: f32 = 0.25;

/// The `stroke_blend` level at or above which a line scene draws through the
/// opacity-preserving seam (ADR-0138). Below it the batch is additive light.
///
/// A midpoint threshold on a continuous param, for the reason every other
/// quantized param in this engine carries one: `[smoothing]` eases a value
/// through everything between its endpoints, so a binding that steps `0 -> 1`
/// is `0.37` for a frame or two on the way. The seam has no state to interpolate
/// — a draw call has one blend mode — so the decision is taken CPU-side, once
/// per frame, at the midpoint.
pub const OPAQUE_BLEND: f32 = 0.5;

/// The `stroke_blend` a line scene draws at when its preset binds nothing —
/// additive light, ADR-0056's seam, and what every line scene drew before the
/// selector existed.
pub const ADDITIVE_BLEND: f32 = 0.0;

/// Hard clamp on L-system iteration depth, enforced at preset load. A branching
/// rule expands exponentially, so an unbounded `max_depth` would stall a preset
/// switch and blow the segment cap (ADR-0007 Risks). Curated presets stay well
/// under this; the turtle's own segment cap is the second backstop.
pub const MAX_LSYSTEM_DEPTH: u32 = 7;

/// Hard clamp on the geometry-mirror rotational order (Plan 0018 Phase 4). Beyond
/// a couple dozen the fold is visually indistinguishable and only multiplies
/// segment count toward the cap; a sane ceiling keeps a runaway `mirror_order`
/// expression from doing useless work before the segment cap bites.
pub const MAX_MIRROR_ORDER: u32 = 24;

/// The shared camera transform every scene family applies (ADR-0018): a uniform
/// **zoom** about the frame centre, then a **pan**, in world space before the
/// aspect divide. Identity (`zoom = 1`, `pan = 0`) leaves geometry exactly where
/// a scene placed it, so a preset that binds none of `zoom`/`pan_x`/`pan_y` is
/// unchanged. `#[repr(C)]` + `Pod` so it uploads straight into a line-renderer
/// uniform slot. Rotate is reserved for a follow-up (ADR-0018 reserves it).
///
/// Defined here for Phase 1 (the line scenes are the walking skeleton); Phase 2
/// threads the same transform through the fragment and swarm scenes.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ViewTransform {
    /// Uniform scale about the frame centre (`1.0` = no zoom).
    pub zoom: f32,
    /// Pan offset in world units `(x, y)`, applied after the zoom.
    pub pan: [f32; 2],
    /// Padding to fill a 16-byte uniform slot (unused).
    pub _pad: f32,
}

impl Default for ViewTransform {
    /// The identity view: no zoom, no pan.
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan: [0.0, 0.0],
            _pad: 0.0,
        }
    }
}

/// Which parametric curve family a `[curve]` preset draws. Extend as Plan 0010's
/// follow-ups add curve families (epicycloids, Lissajous, ...); unknown names
/// are rejected at load.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurveFamily {
    /// The Maurer rose — `sin(n * theta)` walked at a fixed angular step.
    MaurerRose,
}

impl CurveFamily {
    /// Parse a `[curve] family` name, or `None` if unknown.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "maurer_rose" => CurveFamily::MaurerRose,
            _ => return None,
        })
    }
}

/// The colour surface every line scene shares (ADR-0021 / ADR-0059): `hue`
/// places the whole figure in the baked palette and `hue_spread` says how far
/// the palette travels **across** it.
///
/// The axis `u` walks is the one thing that differs per scene — generation depth
/// on the L-system, path position on the parametric curve, radius on the star,
/// band index on the spectrum readout — so the *colour* half lives here once
/// rather than as four similar-but-not-identical loops that drift
/// (ADR-0059's own stated risk). Each generator computes its `u` and asks.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ColorRamp {
    /// Where the figure sits in the palette.
    pub hue: f32,
    /// How far the palette travels from `u = 0` to `u = 1`. `0` — every scene's
    /// default — is the single flat `hue` the line scenes drew before the
    /// palette reached them, which is what makes the surface a strict superset.
    pub hue_spread: f32,
    /// A/B crossfade position (`0` = palette A alone).
    pub palette_mix: f32,
    /// Hard palette bands (ADR-0078), already quantized to an integer.
    ///
    /// **No `palette_contour` counterpart, and that is the honest scoping rather
    /// than an omission.** A contour is drawn from `fwidth` across a *fragment's*
    /// gradient; a stroke takes one palette sample for a whole segment, so there
    /// is no gradient here for a contour to sit in. Banding reaches every scene,
    /// contours reach the continuous-field scenes — see `palette.rs`'s module
    /// docs.
    pub palette_steps: f32,
    /// Shared saturation modulation, applied to the sampled colour.
    pub saturation: f32,
    /// Stroke brightness, folded in here because these scenes carry it in the
    /// segment colour rather than as a separate uniform.
    pub brightness: f32,
}

impl ColorRamp {
    /// The stroke colour at normalized position `u` along the scene's own axis.
    /// Allocation-free; runs per segment (or per generation) on the hot path.
    pub(crate) fn at(self, pal: &crate::render::palette::Palette, u: f32) -> [f32; 3] {
        // `band_coord` is the canonical banding definition (ADR-0078) — called,
        // not copied, because this site is Rust. `palette_steps <= 1` returns the
        // coordinate untouched, so an unbound preset is byte-unchanged.
        let coord =
            crate::render::palette::band_coord(self.hue + self.hue_spread * u, self.palette_steps);
        let rgb = crate::render::palette::desaturate(
            pal.sample(coord, self.palette_mix),
            self.saturation,
        );
        [
            rgb[0] * self.brightness,
            rgb[1] * self.brightness,
            rgb[2] * self.brightness,
        ]
    }
}

/// iq-style cosine palette (RGB phase-shifted), matching the swarm/fragment
/// scenes so line art shares the engine's colour language.
pub fn palette(t: f32) -> [f32; 3] {
    let tau = std::f32::consts::TAU;
    [
        0.5 + 0.5 * (tau * (t + 0.10)).cos(),
        0.5 + 0.5 * (tau * (t + 0.42)).cos(),
        0.5 + 0.5 * (tau * (t + 0.62)).cos(),
    ]
}

/// A thing [`LineRenderer`] draws, under the two transforms every generator
/// line scene applies to its cached geometry: the per-frame rotate/scale/style
/// ([`transform_cached`]) and the geometry mirror ([`replicate_mirror`]).
///
/// It exists so those two run **once** over both instance kinds rather than
/// twice in parallel. Two copies of a rotation would be two places for a
/// segment figure and an arc figure to drift apart under the same `rotation`
/// binding — and the failure would render as a mandala whose circles lag its
/// interlace, which is close to unreadable in a capture.
/// The half-width the two **cached** producers — the L-system walk and the star
/// pattern's outlines — fill their instances with at build time.
///
/// Both figures are built once at `configure` and restyled every frame by
/// [`transform_cached`], which overwrites this with the frame's real half-width.
/// No preset ever sees the value and it is not a default.
///
/// **It is load-bearing exactly once**: a joined end's extension is stored in
/// these units, so [`LineInstance::styled`] can carry it to this frame's width by
/// the ratio between the two. A producer that wrote a different placeholder into
/// `width` than into the extensions would rescale them wrongly.
pub(crate) const PLACEHOLDER_WIDTH: f32 = 0.01;

pub(crate) trait LineInstance: Copy {
    /// Rotate about the origin and scale uniformly. `sin`/`cos` are the
    /// rotation's, passed pre-computed because the caller applies it to a whole
    /// buffer; `angle` is the same rotation in radians, which a shape carrying
    /// an *orientation* needs and a pair of endpoints does not.
    fn rotate_scale(self, sin: f32, cos: f32, angle: f32, scale: f32) -> Self;

    /// Reflect across the x-axis, leaving everything but position alone.
    fn reflect_x(self) -> Self;

    /// Take this frame's colour and half-width. Alpha goes to `1.0`: every
    /// generator line scene draws through ADR-0056's additive seam.
    ///
    /// **Anything measured in half-widths is re-resolved here, not passed
    /// through.** A cached figure is walked once at a placeholder width and
    /// restyled every frame, so a field holding a *length* — as
    /// [`SegmentInstance::ext_a`] does under ADR-0158 — goes stale the moment
    /// `thickness` moves unless this carries it along. A field holding a
    /// width-independent property passes through untouched.
    fn styled(self, color: [f32; 3], width: f32) -> Self;
}

impl LineInstance for SegmentInstance {
    fn rotate_scale(self, sin: f32, cos: f32, _angle: f32, scale: f32) -> Self {
        let rot = |p: [f32; 2]| -> [f32; 2] {
            [
                (p[0] * cos - p[1] * sin) * scale,
                (p[0] * sin + p[1] * cos) * scale,
            ]
        };
        Self {
            a: rot(self.a),
            b: rot(self.b),
            // Connectivity is a property of the cached structure, not of this
            // frame's rotation/scale, so it passes straight through — and so do
            // the extensions, which are measured against `width`. `scale` moves
            // endpoints and leaves stroke width alone, so an extension scaled
            // here would stop matching the stroke it belongs to.
            ..self
        }
    }

    fn reflect_x(self) -> Self {
        Self {
            a: [self.a[0], -self.a[1]],
            b: [self.b[0], -self.b[1]],
            ..self
        }
    }

    fn styled(self, color: [f32; 3], width: f32) -> Self {
        // The extensions are world-space lengths resolved against the width the
        // producer held when it filled the instance (ADR-0158), and a cached
        // figure was walked at a placeholder one. Carry them across by the width
        // ratio: a free end is `0.0` and stays exactly `0.0`, and an end
        // extended by its own half-width stays extended by this frame's.
        let k = if self.width > 0.0 {
            width / self.width
        } else {
            0.0
        };
        Self {
            color,
            width,
            alpha: 1.0,
            ext_a: self.ext_a * k,
            ext_b: self.ext_b * k,
            ..self
        }
    }
}

impl LineInstance for ArcInstance {
    fn rotate_scale(self, sin: f32, cos: f32, angle: f32, scale: f32) -> Self {
        Self {
            centre: [
                (self.centre[0] * cos - self.centre[1] * sin) * scale,
                (self.centre[0] * sin + self.centre[1] * cos) * scale,
            ],
            // `abs`, because a negative `scale` reflects an arc through the
            // origin and a circle of radius `-r` is the circle of radius `r`
            // about the reflected centre — which the centre above already is.
            // A negative radius would instead draw nothing.
            radius: (self.radius * scale).abs(),
            // The half a pair of endpoints does not have: the shape carries its
            // own orientation, so the rotation has to reach it as an angle.
            angle_start: self.angle_start + angle,
            ..self
        }
    }

    fn reflect_x(self) -> Self {
        Self {
            centre: [self.centre[0], -self.centre[1]],
            // Reflection maps the angle `t` to `-t`, so the span `[s, s + w]`
            // becomes `[-s, -s - w]` — the same two endpoints and the same set
            // of angles between them, traversed the other way.
            angle_start: -self.angle_start,
            angle_sweep: -self.angle_sweep,
            ..self
        }
    }

    fn styled(self, color: [f32; 3], width: f32) -> Self {
        Self {
            color,
            width,
            ..self
        }
    }
}

/// The per-frame half shared by every **generator** line scene (L-system,
/// star): transform cached base geometry into `out` — rotate by `rotation`
/// (radians), scale, colour, set `width`, and reveal a `progress` prefix
/// (line-draw-on). Allocation-free into a preallocated `out`; expansion /
/// construction lives at load, this is the only per-frame work.
pub(crate) fn transform_cached<T: LineInstance>(
    base: &[T],
    rotation: f32,
    scale: f32,
    color: [f32; 3],
    width: f32,
    progress: f32,
    out: &mut Vec<T>,
) {
    out.clear();
    let (sin, cos) = rotation.sin_cos();
    let keep = ((base.len() as f32) * progress.clamp(0.0, 1.0)).round() as usize;
    for instance in base.iter().take(keep) {
        out.push(
            instance
                .rotate_scale(sin, cos, rotation, scale)
                .styled(color, width),
        );
    }
}

/// N-fold geometry-mirror spec (Plan 0018 Phase 4): replicate a line scene's
/// segment set under rotational (and optionally reflective) symmetry to build a
/// true geometric fractal. Driven by the `mirror_order` / `mirror_reflect` named
/// params. `order = 1, reflect = false` is the identity — the base drawn once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MirrorSpec {
    /// Rotational symmetry order (`>= 1`).
    pub order: u32,
    /// Also emit a reflected copy per sector (dihedral symmetry).
    pub reflect: bool,
}

impl MirrorSpec {
    /// Build a spec from the raw `mirror_order` / `mirror_reflect` param values —
    /// the shared conversion every line scene uses. The order rounds and clamps to
    /// `1..=MAX_MIRROR_ORDER` (a non-finite or `< 1` value is the identity);
    /// `reflect` is a `>= 0.5` threshold so a preset can drive it with a `beat`.
    pub fn from_params(order: f32, reflect: f32) -> Self {
        let order = if order.is_finite() {
            (order.round() as i64).clamp(1, MAX_MIRROR_ORDER as i64) as u32
        } else {
            1
        };
        Self {
            order,
            reflect: reflect >= 0.5,
        }
    }

    /// How many copies of the base a full replication emits.
    fn copies(self) -> usize {
        self.order.max(1) as usize * if self.reflect { 2 } else { 1 }
    }

    /// Whether replication would be a no-op — one sector, no reflection, so the
    /// output is the input. The common case (no shipped preset binds
    /// `mirror_order`), and the one the scenes skip the copy for.
    pub(crate) fn is_identity(self) -> bool {
        self.order <= 1 && !self.reflect
    }
}

/// Replicate `single` (already positioned/coloured segments) about the frame
/// centre under `mirror.order`-fold rotation, plus an optional reflected copy per
/// sector, into `out` (cleared first) — a geometric kaleidoscope whose segment
/// set is invariant under a `2*pi/order` rotation. Truncates at `cap` (the active
/// tier's [`max_segments`](crate::render::TierConfig::max_segments), which the
/// scene resolved at construction) and returns the number of segments dropped, so
/// the caller can surface it — the cap is never a silent cut (ADR-0007 Risks).
///
/// Allocation-free into a preallocated `out`; the per-frame half of every mirrored
/// line scene.
pub(crate) fn replicate_mirror<T: LineInstance>(
    single: &[T],
    mirror: MirrorSpec,
    cap: usize,
    out: &mut Vec<T>,
) -> usize {
    out.clear();
    let n = mirror.order.max(1);
    let wanted = single.len() * mirror.copies();
    for k in 0..n {
        let sector = std::f32::consts::TAU * (k as f32) / (n as f32);
        let (sin, cos) = sector.sin_cos();
        for reflected in [false, true] {
            if reflected && !mirror.reflect {
                continue;
            }
            for instance in single {
                if out.len() >= cap {
                    break;
                }
                // Reflect across the x-axis (optional), then rotate into the
                // sector. A reflected or rotated copy keeps its source's colour,
                // width and connectivity: the geometry moves, the topology does
                // not — so the scale is exactly 1.0, which is an IEEE identity
                // and leaves the pre-Plan-0087 arithmetic byte for byte.
                let placed = if reflected {
                    instance.reflect_x()
                } else {
                    *instance
                };
                out.push(placed.rotate_scale(sin, cos, sector, 1.0));
            }
        }
    }
    wanted.saturating_sub(out.len())
}

#[cfg(test)]
mod tests;
