//! Spectrum scene (Plan 0034 / ADR-0036): the analysis frame's log-spaced band
//! array, drawn as N elements.
//!
//! This is a **fourth consumer of the existing line idiom**, not a fifth render
//! idiom: N bars, an N-point polyline and a radial ring of N spokes are all
//! segment lists, and they go out through the same shared
//! [`LineRenderer`](super::LineRenderer) the three other line scenes draw
//! through (ADR-0007). Nothing new is uploaded, no new pipeline is built, and
//! the `Scene` trait is untouched — `update` already receives the whole
//! [`AnalysisFrame`], bands included.
//!
//! The per-frame work is a chain of small pure steps — [`downsample`], the
//! per-element ease, then one of the three [`SpectrumLayout`] builders — all
//! free functions over preallocated buffers. They are separate from the scene so
//! the claims that matter (low elements track low frequencies; the 64 → N
//! reduction loses nothing) are testable without a GPU.
//!
//! **The composite vocabulary this scene honors**, since a silent no-op is the
//! failure mode the shared-surface work exists to avoid:
//!
//! - `zoom` / `pan_x` / `pan_y` — the shared view transform (ADR-0018), applied
//!   by the renderer exactly as for every other line scene.
//! - `mirror_order` / `mirror_reflect` — the geometry mirror (Plan 0018 Phase
//!   4). It replicates real geometry *before* rasterization and costs this scene
//!   nothing, so refusing it would be a no-op the author could not see. On the
//!   radial ring it is nearly the identity (the ring is already rotationally
//!   symmetric, the same near-no-op the Hankin star has); on bars and polyline
//!   it is genuinely transformative, because those figures are not centred.
//! - `[palette]` / `[palette_b]` / `palette_mix` / `hue` / `hue_spread` /
//!   `saturation` — the colour surface (ADR-0021), sampled on the CPU. This is
//!   the **first line scene to honor the palette**; the others still colour from
//!   the built-in cosine. It matters here because colouring elements along their
//!   own axis is what turns a frequency readout into a look. The default
//!   `spectrum` palette is that same cosine, so an author who sets no `[palette]`
//!   sees the engine's usual colour language.
//! - `thickness` / `brightness` / `scale` / `base` — ordinary stroke styling.
//! - `curve` — the level-shaping exponent (ADR-0040, Plan 0038 Phase 3), applied
//!   to the downsampled level **before** the per-element smoother so the easing
//!   operates in the displayed domain. `1.0` is exactly linear. It is the third
//!   per-element lever on an element's length, beside `base` and `scale`.
//! - `glow` — the line renderer's per-segment falloff multiplier (Plan 0038),
//!   whole-figure like on the other three line scenes. Not a post bloom.
//!
//! Three parameters are **layout-specific**, and each is a no-op on the layouts
//! it does not describe — stated in `presets/README.md` and `docs/presets.md`
//! rather than left for an author to discover:
//!
//! - `radius` is the ring's inner radius; no meaning for bars or the polyline.
//! - `span` and `baseline` place the bars/polyline figure in **world** space; no
//!   meaning for the ring, which `radius` sizes instead (Plan 0038 Phase 2).

// Hot-path panic-denial pragma (Plan 0002 Phase 2, extended to scenes by Plan
// 0003 Phase 0). `update`/`render` run every displayed frame.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use std::cell::RefCell;
use std::rc::Rc;

use super::super::Scene;
use super::renderer::{JOINED_A, JOINED_B, LineRenderer, SegmentInstance};
use super::{
    CapOverflow, GeneratorConfig, MirrorSpec, OverflowContext, ViewTransform, replicate_mirror,
};
use crate::dsp::AnalysisFrame;
use crate::preset::Easing;
use crate::render::palette::{Palette, desaturate};

/// Maps `thickness` to an NDC-y half-width — the same scale the other line
/// scenes use, so a `thickness` that reads well on the rose reads the same here.
const WIDTH_SCALE: f32 = 0.003;

/// Largest element count a `[spectrum]` table may ask for — the band count
/// itself, because above it the 64 → N reduction stops being a partition of the
/// array. The loader validates against this, and the render layer sizes its
/// per-element scratch to it (Plan 0034 Phase 4), so the two cannot disagree.
pub const MAX_ELEMENTS: usize = crate::dsp::SPECTRUM_BINS;

// Parameter defaults — a legible, calm readout when a preset binds nothing.
const DEFAULT_THICKNESS: f32 = 6.0;
const DEFAULT_HUE: f32 = 0.55;
const DEFAULT_HUE_SPREAD: f32 = 0.0;
const DEFAULT_SATURATION: f32 = 1.0;
const DEFAULT_PALETTE_MIX: f32 = 0.0;
const DEFAULT_BRIGHTNESS: f32 = 1.0;
/// The line renderer's **per-segment falloff** multiplier (Plan 0038 Phase 1) —
/// not a post-process bloom. `1.0` is the value this scene passed as a literal
/// before it was bound, so the default is exactly today's look.
const DEFAULT_GLOW: f32 = 1.0;
const DEFAULT_SCALE: f32 = 1.2;
/// Minimum element length, in world units. Non-zero on purpose: a spectrum
/// readout at rest is a comb, not an empty frame, so the figure stays on screen
/// (and legible) through a silence instead of vanishing.
const DEFAULT_BASE: f32 = 0.06;
/// Inner radius of the radial ring (ignored by the other two layouts).
const DEFAULT_RADIUS: f32 = 0.35;
/// World-space **half-width** the readout spans, so the figure is `2 * span`
/// wide — `1.0` is the constant this scene was pinned to before Plan 0038
/// Phase 2 bound it.
///
/// It is a **world** quantity, not a screen one. The renderer divides x by the
/// target aspect on the GPU, so this scene never sees an aspect and cannot take
/// one from the wrong place ([ADR-0037](../../../../../docs/adrs/0037-internal-grid-is-a-resolution-not-a-shape.md)).
/// The honest consequence: *"fill the width"* is aspect-dependent — `span ≈
/// 1.78` fills a 16:9 frame and leaves an ultrawide short. There is
/// deliberately no `fit` mode.
///
/// Applies to [`SpectrumLayout::Bars`] and [`SpectrumLayout::Polyline`]; a
/// **no-op on [`SpectrumLayout::RadialRing`]**, which is sized by `radius`
/// instead — the mirror image of `radius` already being a no-op on the
/// other two.
const DEFAULT_SPAN: f32 = 1.0;
/// World-space y the bars and the polyline rest on — the constant this scene
/// was pinned to before Plan 0038 Phase 2 bound it. Also a **no-op on
/// [`SpectrumLayout::RadialRing`]**, whose spokes start on the ring.
///
/// `baseline = 0` is what makes `mirror_reflect` mean what it means everywhere
/// else: the mirror reflects across the **x-axis**, so a figure standing on the
/// axis reflects into a symmetric "landscape and its reflection" about the
/// frame centre, while one standing at `-0.85` throws its copy against the top
/// edge (design-backlog 0018).
const DEFAULT_BASELINE: f32 = -0.85;
/// Level-shaping exponent ([ADR-0040](../../../../../docs/adrs/0040-spectrum-level-curve-applies-before-the-easing.md)).
/// `1.0` is exactly linear — `powf(x, 1.0) == x`, the map this scene had before
/// Plan 0038 Phase 3 — `0.5` is a square root, and lower values compress harder.
///
/// It applies to the **downsampled level, before the per-element smoother**, so
/// `[spectrum] smoothing` eases the displayed quantity the way meter ballistics
/// do. That ordering is the ADR's whole content; see [`curve_level`].
const DEFAULT_CURVE: f32 = 1.0;
/// The range [`curve_level`] clamps the exponent into before the `powf`.
///
/// **Totality is part of ADR-0040's decision, not an implementation detail.**
/// This runs per element per frame on the render path, where a `NaN` or an
/// infinite length must not reach the geometry. A floor strictly above zero is
/// what rules out `pow(0, 0)` and `pow(0, -1)` for every author expression.
const CURVE_MIN: f32 = 0.05;
const CURVE_MAX: f32 = 4.0;
const DEFAULT_ROTATION: f32 = 0.0;
// Shared view transform (ADR-0018): identity by default.
const DEFAULT_ZOOM: f32 = 1.0;
const DEFAULT_PAN: f32 = 0.0;
// Geometry mirror (Plan 0018 Phase 4): identity by default.
const DEFAULT_MIRROR_ORDER: f32 = 1.0;
const DEFAULT_MIRROR_REFLECT: f32 = 0.0;

/// Parameter vocabulary — see [`fragment_field::PARAMS`](crate::render::scenes::fragment_field::PARAMS).
/// **Keep in sync with `set_param` below.**
pub const PARAMS: &[&str] = &[
    "base",
    "scale",
    "curve",
    "radius",
    "span",
    "baseline",
    "rotation",
    "thickness",
    "hue",
    "hue_spread",
    "saturation",
    "palette_mix",
    "brightness",
    "glow",
    "zoom",
    "pan_x",
    "pan_y",
    "mirror_order",
    "mirror_reflect",
];

/// Which figure the elements form. Selected once at preset load through the
/// `[spectrum]` table; an unknown name is a surfaced load error (ADR-0007).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpectrumLayout {
    /// Upright bars standing on a common baseline — the classic readout.
    #[default]
    Bars,
    /// A single continuous line through one point per element: the same data as
    /// a contour rather than a comb.
    Polyline,
    /// Spokes radiating outward from a ring, one per element, with the frequency
    /// axis wrapped around the circle.
    RadialRing,
}

impl SpectrumLayout {
    /// The accepted `[spectrum] layout` names, in the order the error message
    /// lists them. The single source for both parsing and the message.
    pub const NAMES: [&'static str; 3] = ["bars", "polyline", "radial_ring"];

    /// Parse a `[spectrum] layout` name, or `None` if unknown.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "bars" => SpectrumLayout::Bars,
            "polyline" => SpectrumLayout::Polyline,
            "radial_ring" => SpectrumLayout::RadialRing,
            _ => return None,
        })
    }
}

/// Where the figure sits — the scalars that belong to the **whole** readout
/// rather than to an element, so they stay off the per-element arrays.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Placement {
    /// Inner radius — [`SpectrumLayout::RadialRing`] only.
    pub radius: f32,
    /// World-space half-width — [`SpectrumLayout::Bars`] and
    /// [`SpectrumLayout::Polyline`] only. See [`DEFAULT_SPAN`].
    pub span: f32,
    /// World-space y the figure rests on — bars and polyline only. See
    /// [`DEFAULT_BASELINE`].
    pub baseline: f32,
    /// Whole-figure rotation in radians, about the world origin.
    pub rotation: f32,
}

/// The scene's own defaults rather than zeroes: a `Placement` with `span = 0`
/// would collapse the figure to a point, which is never a sensible fallback.
impl Default for Placement {
    fn default() -> Self {
        Self {
            radius: 0.0,
            span: DEFAULT_SPAN,
            baseline: DEFAULT_BASELINE,
            rotation: 0.0,
        }
    }
}

/// The parameters this scene accepts as a **per-element series** (Plan 0034
/// Phase 4) — the ones whose effect is genuinely per element. Everything else in
/// [`PARAMS`] describes the whole figure (`radius`, `rotation`, the view
/// transform, the mirror, `hue_spread`, `palette_mix`, `saturation`), so a
/// series aimed at one of those degrades to its `index = 0` value, which is what
/// the trait default does.
///
/// **Index order matches the `SERIES_*` constants below** — the two are read
/// together by `set_param_series` and `update`.
const SERIES_PARAMS: [&str; 6] = ["base", "scale", "curve", "thickness", "brightness", "hue"];
const SERIES_BASE: usize = 0;
const SERIES_SCALE: usize = 1;
/// `curve` sits with `base` and `scale` because it is the third lever on an
/// element's *length*, and per-element is a shape ADR-0040 names explicitly
/// ("walk it per element with `index`") — a series aimed at it has to reach the
/// elements rather than degrade to its `index = 0` value.
const SERIES_CURVE: usize = 2;
const SERIES_THICKNESS: usize = 3;
const SERIES_BRIGHTNESS: usize = 4;
const SERIES_HUE: usize = 5;

/// Reduce the engine's band array to `levels.len()` elements by **averaging each
/// element's own contiguous slice of bands**.
///
/// Element `i` covers `[i * bands / n, (i + 1) * bands / n)`. That is a genuine
/// partition — contiguous, non-overlapping, and complete — so no band is dropped
/// or double-counted at any element count up to the band count (which is why the
/// loader caps the count there). It is also deterministic: integer arithmetic on
/// lengths, no clock and no rounding mode to disagree about.
pub(crate) fn downsample(spectrum: &[f32], levels: &mut [f32]) {
    let bands = spectrum.len();
    let n = levels.len();
    if bands == 0 || n == 0 {
        levels.fill(0.0);
        return;
    }
    for (i, level) in levels.iter_mut().enumerate() {
        let lo = i * bands / n;
        // `hi` is the next element's `lo`, which makes the ranges abut exactly.
        // The `max` only bites when n > bands, where a strict partition is not
        // available; the loader caps the count so that never happens.
        let hi = (((i + 1) * bands / n).min(bands)).max(lo + 1);
        let slice = spectrum.get(lo..hi).unwrap_or(&[]);
        *level = if slice.is_empty() {
            0.0
        } else {
            slice.iter().sum::<f32>() / slice.len() as f32
        };
    }
}

/// Shape a raw downsampled level by the exponent `curve` — ADR-0040's decision,
/// and the step that runs **before** the per-element smoother.
///
/// Audio level is perceptually logarithmic, so the linear map this scene had
/// spent most of its range on the loudest element. `curve = 1.0` is exactly the
/// identity, which is what lets the default leave every existing preset and
/// every golden baseline unchanged.
///
/// **Total by construction**, because it runs per element per frame on the
/// render path and no author guard stands between an expression and this call:
///
/// - the level is floored at `0` — `f32::max` returns the non-`NaN` operand, so
///   a `NaN` level floors to `0` too, and a negative one can never become a
///   fractional power of a negative base;
/// - the exponent is clamped into `[CURVE_MIN, CURVE_MAX]`, a range that
///   excludes `0`, so neither `pow(0, 0)` nor `pow(0, -1)` is reachable. `NaN`
///   has no clamped image (`f32::clamp` propagates it), so it is mapped to the
///   linear default rather than allowed through.
pub(crate) fn curve_level(level: f32, curve: f32) -> f32 {
    let exponent = if curve.is_nan() {
        DEFAULT_CURVE
    } else {
        curve.clamp(CURVE_MIN, CURVE_MAX)
    };
    level.max(0.0).powf(exponent)
}

/// The world-space length an element reaches, `base + scale * level`, floored at
/// zero so a degenerate level can never invert the element.
pub(crate) fn element_length(level: f32, base: f32, scale: f32) -> f32 {
    (base + scale * level).max(0.0)
}

/// Build the segment list for `layout` into `out` (cleared first).
/// Allocation-free into a preallocated buffer; the per-frame half of the scene.
///
/// `lengths`, `widths` and `colors` are read **positionally, per element**, which
/// is what lets a per-element binding vary any of them across the figure (Plan
/// 0034 Phase 4). A short list falls back to a sane constant rather than
/// panicking — which cannot happen, since all three are sized together at load,
/// but keeps the hot path total.
pub(crate) fn build(
    layout: SpectrumLayout,
    lengths: &[f32],
    widths: &[f32],
    colors: &[[f32; 3]],
    place: Placement,
    out: &mut Vec<SegmentInstance>,
) {
    out.clear();
    if lengths.is_empty() {
        return;
    }
    let (sin, cos) = place.rotation.sin_cos();
    // The whole-figure rotation is applied here, at the point where world-space
    // endpoints are emitted, so it composes with every layout identically.
    let turn = |p: [f32; 2]| -> [f32; 2] { [p[0] * cos - p[1] * sin, p[0] * sin + p[1] * cos] };
    let color_of = |i: usize| colors.get(i).copied().unwrap_or([1.0, 1.0, 1.0]);
    let width_of = |i: usize| widths.get(i).copied().unwrap_or(0.01);

    match layout {
        SpectrumLayout::Bars => {
            let step = 2.0 * place.span / lengths.len() as f32;
            for (i, &length) in lengths.iter().enumerate() {
                let x = -place.span + step * (i as f32 + 0.5);
                out.push(SegmentInstance {
                    a: turn([x, place.baseline]),
                    b: turn([x, place.baseline + length]),
                    color: color_of(i),
                    width: width_of(i),
                    // Isolated: one segment per element, both ends free. Bars
                    // must keep exactly their previous geometry, or a bar would
                    // hang below `baseline` and break the centre-mirror.
                    joined: 0,
                });
            }
        }
        SpectrumLayout::Polyline => {
            // One point per element, spanning edge to edge, joined by n-1
            // segments. A single element has no segment to draw, which is why
            // the loader's minimum count is 2.
            let gaps = lengths.len().saturating_sub(1);
            if gaps == 0 {
                return;
            }
            let step = 2.0 * place.span / gaps as f32;
            let point = |i: usize, length: f32| -> [f32; 2] {
                turn([-place.span + step * i as f32, place.baseline + length])
            };
            let mut prev = point(0, lengths.first().copied().unwrap_or(0.0));
            for (i, &length) in lengths.iter().enumerate().skip(1) {
                let next = point(i, length);
                // Chained (ADR-0041): consecutive segments share a point, so
                // every interior endpoint is a joint. Only the two ends of the
                // whole figure are free — segment `i` runs from point `i - 1` to
                // point `i`, so its `a` is joined for every segment but the
                // first and its `b` for every segment but the last.
                let mut joined = 0;
                if i > 1 {
                    joined |= JOINED_A;
                }
                if i < gaps {
                    joined |= JOINED_B;
                }
                out.push(SegmentInstance {
                    a: prev,
                    b: next,
                    color: color_of(i),
                    width: width_of(i),
                    joined,
                });
                prev = next;
            }
        }
        SpectrumLayout::RadialRing => {
            // The frequency axis wrapped around the circle: element 0 points
            // along +x and the rest follow counter-clockwise, each spoke running
            // outward from the ring.
            let n = lengths.len() as f32;
            let inner = place.radius.max(0.0);
            for (i, &length) in lengths.iter().enumerate() {
                let angle = place.rotation + std::f32::consts::TAU * i as f32 / n;
                let (s, c) = angle.sin_cos();
                let outer = inner + length;
                out.push(SegmentInstance {
                    a: [c * inner, s * inner],
                    b: [c * outer, s * outer],
                    color: color_of(i),
                    width: width_of(i),
                    // Isolated, like the bars: a spoke that extended inward
                    // would grow through `radius` and fill the inner circle.
                    joined: 0,
                });
            }
        }
    }
}

/// The spectrum readout: N elements driven by the analysis frame's band array,
/// drawn through the shared line renderer.
pub struct SpectrumScene {
    /// The single line renderer, shared with the other line scenes (ADR-0007:
    /// "one line renderer"). Only the active scene draws in a frame.
    renderer: Rc<RefCell<LineRenderer>>,
    /// The drawn geometry, after the mirror. Preallocated to the cap.
    segments: Vec<SegmentInstance>,
    /// The single (pre-mirror) figure, replicated into
    /// [`segments`](Self::segments). Preallocated to the cap.
    single_buf: Vec<SegmentInstance>,
    /// The active tier's segment ceiling
    /// ([`TierConfig::max_segments`](crate::render::TierConfig::max_segments)),
    /// resolved once at construction (Plan 0044). A field rather than a constant
    /// so the tier can raise it; the buffers above are preallocated to it, which
    /// is what keeps the per-frame replication allocation-free.
    max_segments: usize,
    /// Set when this frame's mirror replication overflowed the cap.
    mirror_overflow: Option<CapOverflow>,
    /// This frame's downsampled band levels, before easing.
    raw_levels: Vec<f32>,
    /// The **held** (eased) levels actually drawn — the per-element envelope
    /// state. Sized at load beside [`raw_levels`](Self::raw_levels).
    levels: Vec<f32>,
    /// Per-element stroke colour, rebuilt each frame into a buffer sized at load.
    colors: Vec<[f32; 3]>,
    /// Per-element world-space length, rebuilt each frame. Sized at load.
    lengths: Vec<f32>,
    /// Per-element stroke half-width, rebuilt each frame. Sized at load.
    widths: Vec<f32>,
    /// Per-element binding overrides (Plan 0034 Phase 4), one row per
    /// [`SERIES_PARAMS`] entry, each sized at load.
    series: [Vec<f32>; SERIES_PARAMS.len()],
    /// Which rows this frame's bindings actually wrote. Cleared in
    /// `reset_params` alongside the scalars, so a series never outlives the frame
    /// that produced it — the same lifetime rule every other param follows.
    series_active: [bool; SERIES_PARAMS.len()],
    /// The figure, from `[spectrum] layout`.
    layout: SpectrumLayout,
    /// Per-element easing, from `[spectrum] smoothing`.
    easing: Easing,
    /// Real elapsed seconds for this frame, injected through
    /// [`advance`](Scene::advance) — what makes the easing frame-rate
    /// independent (ADR-0019).
    dt: f32,
    /// The preset's baked colour LUT (ADR-0021), sampled on the CPU per element.
    palette: Palette,
    thickness: f32,
    hue: f32,
    hue_spread: f32,
    saturation: f32,
    palette_mix: f32,
    brightness: f32,
    glow: f32,
    scale: f32,
    base: f32,
    curve: f32,
    radius: f32,
    span: f32,
    baseline: f32,
    rotation: f32,
    zoom: f32,
    pan_x: f32,
    pan_y: f32,
    mirror_order: f32,
    mirror_reflect: f32,
}

impl SpectrumScene {
    /// Build the scene over the shared line renderer, preallocating its segment
    /// buffers to the cap. The element buffers are sized by `configure`, which
    /// the renderer runs on every preset switch.
    pub fn new(renderer: Rc<RefCell<LineRenderer>>, max_segments: usize) -> Self {
        Self {
            renderer,
            segments: Vec::with_capacity(max_segments),
            single_buf: Vec::with_capacity(max_segments),
            max_segments,
            mirror_overflow: None,
            raw_levels: Vec::new(),
            levels: Vec::new(),
            colors: Vec::new(),
            lengths: Vec::new(),
            widths: Vec::new(),
            series: Default::default(),
            series_active: [false; SERIES_PARAMS.len()],
            layout: SpectrumLayout::default(),
            easing: Easing::INSTANT,
            dt: 0.0,
            // Replaced by the preset's palette on the next switch; the default
            // is the engine cosine, so an unconfigured scene still colours.
            palette: Palette::default_spectrum(),
            thickness: DEFAULT_THICKNESS,
            hue: DEFAULT_HUE,
            hue_spread: DEFAULT_HUE_SPREAD,
            saturation: DEFAULT_SATURATION,
            palette_mix: DEFAULT_PALETTE_MIX,
            brightness: DEFAULT_BRIGHTNESS,
            glow: DEFAULT_GLOW,
            scale: DEFAULT_SCALE,
            base: DEFAULT_BASE,
            curve: DEFAULT_CURVE,
            radius: DEFAULT_RADIUS,
            span: DEFAULT_SPAN,
            baseline: DEFAULT_BASELINE,
            rotation: DEFAULT_ROTATION,
            zoom: DEFAULT_ZOOM,
            pan_x: DEFAULT_PAN,
            pan_y: DEFAULT_PAN,
            mirror_order: DEFAULT_MIRROR_ORDER,
            mirror_reflect: DEFAULT_MIRROR_REFLECT,
        }
    }

    /// Resize the per-element buffers and clear the envelope state. Off the hot
    /// path (preset load only) — the scratch every frame writes into is sized
    /// here, never per frame.
    fn resize(&mut self, elements: usize) {
        self.raw_levels.clear();
        self.raw_levels.resize(elements, 0.0);
        // Cleared rather than resized in place: a preset switch must not show
        // the previous preset's envelope decaying under the new one.
        self.levels.clear();
        self.levels.resize(elements, 0.0);
        self.colors.clear();
        self.colors.resize(elements, [0.0; 3]);
        self.lengths.clear();
        self.lengths.resize(elements, 0.0);
        self.widths.clear();
        self.widths.resize(elements, 0.0);
        for row in &mut self.series {
            row.clear();
            row.resize(elements, 0.0);
        }
        self.series_active = [false; SERIES_PARAMS.len()];
    }

    /// Element `i`'s value for series row `row`, or `fallback` when no binding
    /// drove that row this frame. Total — an out-of-range row or element reads
    /// the fallback rather than panicking on the hot path.
    fn series_value(&self, row: usize, i: usize, fallback: f32) -> f32 {
        if !self.series_active.get(row).copied().unwrap_or(false) {
            return fallback;
        }
        self.series
            .get(row)
            .and_then(|values| values.get(i))
            .copied()
            .unwrap_or(fallback)
    }
}

impl Scene for SpectrumScene {
    fn name(&self) -> &'static str {
        "spectrum"
    }

    fn advance(&mut self, dt: f32) {
        self.dt = dt;
    }

    fn reset_params(&mut self) {
        self.thickness = DEFAULT_THICKNESS;
        self.hue = DEFAULT_HUE;
        self.hue_spread = DEFAULT_HUE_SPREAD;
        self.saturation = DEFAULT_SATURATION;
        self.palette_mix = DEFAULT_PALETTE_MIX;
        self.brightness = DEFAULT_BRIGHTNESS;
        self.glow = DEFAULT_GLOW;
        self.scale = DEFAULT_SCALE;
        self.base = DEFAULT_BASE;
        self.curve = DEFAULT_CURVE;
        self.radius = DEFAULT_RADIUS;
        self.span = DEFAULT_SPAN;
        self.baseline = DEFAULT_BASELINE;
        self.rotation = DEFAULT_ROTATION;
        self.zoom = DEFAULT_ZOOM;
        self.pan_x = DEFAULT_PAN;
        self.pan_y = DEFAULT_PAN;
        self.mirror_order = DEFAULT_MIRROR_ORDER;
        self.mirror_reflect = DEFAULT_MIRROR_REFLECT;
        // A per-element series lives exactly one frame, like every scalar above:
        // the rows keep their storage (sized at load) but stop being read until
        // a binding writes them again.
        self.series_active = [false; SERIES_PARAMS.len()];
    }

    fn set_param(&mut self, name: &str, value: f32) {
        match name {
            "base" => self.base = value,
            "scale" => self.scale = value,
            "curve" => self.curve = value,
            "radius" => self.radius = value,
            "span" => self.span = value,
            "baseline" => self.baseline = value,
            "rotation" => self.rotation = value,
            "thickness" => self.thickness = value,
            "hue" => self.hue = value,
            "hue_spread" => self.hue_spread = value,
            "saturation" => self.saturation = value,
            "palette_mix" => self.palette_mix = value,
            "brightness" => self.brightness = value,
            "glow" => self.glow = value,
            "zoom" => self.zoom = value,
            "pan_x" => self.pan_x = value,
            "pan_y" => self.pan_y = value,
            "mirror_order" => self.mirror_order = value,
            "mirror_reflect" => self.mirror_reflect = value,
            _ => {}
        }
    }

    fn set_param_series(&mut self, name: &str, values: &[f32]) {
        let Some(row) = SERIES_PARAMS.iter().position(|&p| p == name) else {
            // Not a per-element parameter on this scene (the whole-figure ones:
            // radius, rotation, the view transform, the mirror, hue_spread,
            // palette_mix, saturation). Fall back to the trait's rule — the
            // element-0 value — rather than dropping the binding.
            if let Some(&first) = values.first() {
                self.set_param(name, first);
            }
            return;
        };
        let (Some(dst), Some(active)) = (self.series.get_mut(row), self.series_active.get_mut(row))
        else {
            return; // unreachable: `position` returned an in-range row
        };
        // Copy rather than borrow: the caller's slice is the renderer's scratch,
        // reused by the next binding. `n` is the overlap, so a scratch sized for
        // a different element count can neither overrun nor leave stale values
        // beyond it (the rest of the row keeps whatever it held; only the first
        // `n` are read, because `update` walks `lengths`, which is `n` long).
        let n = dst.len().min(values.len());
        if let (Some(dst), Some(src)) = (dst.get_mut(..n), values.get(..n)) {
            dst.copy_from_slice(src);
        }
        *active = n > 0;
    }

    fn set_palette(&mut self, palette: &Palette) {
        self.palette = palette.clone();
    }

    fn configure(&mut self, cfg: &GeneratorConfig) -> Option<CapOverflow> {
        // Exhaustive with no wildcard, like the sibling line scenes: a new config
        // variant has to be acknowledged here rather than silently ignored.
        match cfg {
            GeneratorConfig::Spectrum {
                elements,
                layout,
                easing,
            } => {
                self.layout = *layout;
                self.easing = *easing;
                self.resize(*elements);
            }
            // Other scenes' configs (curve, L-system, star, particle attractor).
            GeneratorConfig::Curve { .. }
            | GeneratorConfig::LSystem { .. }
            | GeneratorConfig::Star { .. }
            | GeneratorConfig::Particles { .. } => {}
        }
        // Nothing is built here — the element count is validated at load and is
        // orders of magnitude under the segment cap — so nothing truncates.
        None
    }

    fn mirror_overflow(&self) -> Option<&CapOverflow> {
        self.mirror_overflow.as_ref()
    }

    fn update(&mut self, frame: &AnalysisFrame) {
        downsample(&frame.spectrum, &mut self.raw_levels);
        // `downsample -> curve -> ease`, in that order, which is ADR-0040's
        // decision and not an incidental arrangement of two lines: the smoother's
        // state **is** the displayed quantity, so a fall's time constant is
        // exactly the `release` the preset wrote, at every value of `curve`.
        // Easing first would have made the effective release `release / curve` —
        // engaging `curve = 0.5` would silently double every fall time, and a
        // fall to a non-zero floor would stop being exponential at all, so
        // `release` would name no duration.
        //
        // ADR-0040 originally argued this ordering bought a perceptually *even*
        // fall. Plan 0038 Phase 3 measured that and it is false — both orderings
        // are exponentials of identical shape and differ only in speed. See the
        // ADR's Outcome section; the ordering survives on the reason above.
        //
        // The easing itself is per element on the injected real `dt`, through the
        // same `Easing` the `[smoothing]` table uses (ADR-0035) — so "0.2
        // seconds" means the same thing here as on a binding, at any frame rate.
        // The default is `INSTANT`, which passes the curved level straight
        // through, and the default `curve` is `1.0`, which is the identity.
        let (easing, dt) = (self.easing, self.dt);
        for i in 0..self.levels.len() {
            let raw = self.raw_levels.get(i).copied().unwrap_or(0.0);
            let shaped = curve_level(raw, self.series_value(SERIES_CURVE, i, self.curve));
            if let Some(held) = self.levels.get_mut(i) {
                *held = easing.step(*held, shaped, dt);
            }
        }

        // Per-element geometry and colour. Each scalar below reads its own series
        // row when a binding drove one this frame (Plan 0034 Phase 4) and the
        // whole-figure param otherwise — so `thickness = "0.01 + bin(index) * 5"`
        // varies the stroke across the figure while `thickness = "2"` does not,
        // with no branch in the preset and no second code path here.
        let count = self.levels.len();
        let span = count.max(1) as f32;
        for i in 0..count {
            let level = self.levels.get(i).copied().unwrap_or(0.0);
            let base = self.series_value(SERIES_BASE, i, self.base);
            let scale = self.series_value(SERIES_SCALE, i, self.scale);
            let thickness = self.series_value(SERIES_THICKNESS, i, self.thickness);
            let brightness = self.series_value(SERIES_BRIGHTNESS, i, self.brightness);
            // `hue_spread` walks the palette along the axis on top of whatever
            // `hue` is — at the default spread of 0 the figure is one hue, at 1
            // it spans the palette from the lowest element to the highest.
            let hue =
                self.series_value(SERIES_HUE, i, self.hue) + self.hue_spread * (i as f32 / span);
            let rgb = desaturate(self.palette.sample(hue, self.palette_mix), self.saturation);

            if let Some(slot) = self.lengths.get_mut(i) {
                *slot = element_length(level, base, scale);
            }
            if let Some(slot) = self.widths.get_mut(i) {
                *slot = (thickness * WIDTH_SCALE).max(0.0005);
            }
            if let Some(slot) = self.colors.get_mut(i) {
                *slot = [
                    rgb[0] * brightness,
                    rgb[1] * brightness,
                    rgb[2] * brightness,
                ];
            }
        }

        let place = Placement {
            radius: self.radius,
            span: self.span,
            baseline: self.baseline,
            rotation: self.rotation,
        };
        build(
            self.layout,
            &self.lengths,
            &self.widths,
            &self.colors,
            place,
            &mut self.single_buf,
        );

        let mirror = MirrorSpec::from_params(self.mirror_order, self.mirror_reflect);
        if mirror.is_identity() {
            // Identity replication would copy the whole set to produce exactly
            // what it was given; swap instead (Plan 0031 Phase 4). Both buffers
            // are preallocated to the cap, so neither can grow later.
            std::mem::swap(&mut self.single_buf, &mut self.segments);
            self.mirror_overflow = None;
            return;
        }
        let dropped = replicate_mirror(
            &self.single_buf,
            mirror,
            self.max_segments,
            &mut self.segments,
        );
        self.mirror_overflow = (dropped > 0).then_some(CapOverflow {
            dropped,
            context: OverflowContext::Mirror(mirror.order),
            cap: self.max_segments,
        });
    }

    fn render(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        aspect: f32,
    ) {
        let xform = ViewTransform {
            zoom: self.zoom,
            pan: [self.pan_x, self.pan_y],
            _pad: 0.0,
        };
        self.renderer.borrow_mut().draw(
            queue,
            encoder,
            view,
            aspect,
            self.glow,
            xform,
            &self.segments,
        );
    }
}

#[cfg(test)]
mod tests {
    // Test asserts index the produced Vec; allowed here over the file's
    // hot-path pragma since test code is not the render path.
    #![allow(clippy::indexing_slicing)]

    use super::*;
    use crate::dsp::SPECTRUM_BINS;

    /// The cap these tests run at — the floor tier's, which is the value they were
    /// written against and the one every shipped preset is authored and gated on
    /// (Plan 0044).
    const FLOOR_CAP: usize = crate::render::TierConfig::FLOOR.max_segments;

    fn white(n: usize) -> Vec<[f32; 3]> {
        vec![[1.0, 1.0, 1.0]; n]
    }

    fn even_widths(n: usize) -> Vec<f32> {
        vec![0.01; n]
    }

    /// Build one figure from raw per-element lengths at an explicit placement.
    fn placed(layout: SpectrumLayout, lengths: &[f32], place: Placement) -> Vec<SegmentInstance> {
        let mut out = Vec::new();
        build(
            layout,
            lengths,
            &even_widths(lengths.len()),
            &white(lengths.len()),
            place,
            &mut out,
        );
        out
    }

    /// Build one figure from raw per-element lengths, at the default placement,
    /// unturned.
    fn figure(layout: SpectrumLayout, lengths: &[f32]) -> Vec<SegmentInstance> {
        placed(layout, lengths, Placement::default())
    }

    /// The element lengths a stimulus produces, low index first — the whole
    /// chain from band array to drawn geometry, at `base = 0`, `scale = 1`.
    fn bar_lengths(spectrum: &[f32; SPECTRUM_BINS], elements: usize) -> Vec<f32> {
        let mut levels = vec![0.0; elements];
        downsample(spectrum, &mut levels);
        let lengths: Vec<f32> = levels
            .iter()
            .map(|&level| element_length(level, 0.0, 1.0))
            .collect();
        figure(SpectrumLayout::Bars, &lengths)
            .iter()
            .map(|s| s.b[1] - s.a[1])
            .collect()
    }

    /// A band array with energy `1.0` in `lit` bands starting at `from`.
    fn banded(from: usize, lit: usize) -> [f32; SPECTRUM_BINS] {
        let mut spectrum = [0.0; SPECTRUM_BINS];
        for band in spectrum.iter_mut().skip(from).take(lit) {
            *band = 1.0;
        }
        spectrum
    }

    /// Plan 0034 Phase 2 done-when 3, and the claim that separates a working
    /// spectrum from N bars of noise: **energy lands on the elements that own
    /// its frequencies.** Bass raises the low-index bars and leaves the high
    /// ones flat; treble does exactly the reverse.
    #[test]
    fn energy_raises_the_elements_that_own_its_frequencies() {
        let n = 24;
        let quarter = n / 4;

        let low = bar_lengths(&banded(0, SPECTRUM_BINS / 4), n);
        let high = bar_lengths(&banded(3 * SPECTRUM_BINS / 4, SPECTRUM_BINS / 4), n);

        for i in 0..quarter {
            assert!(
                low[i] > 0.5,
                "bass stimulus must raise low element {i}, got {}",
                low[i]
            );
            assert_eq!(
                high[i], 0.0,
                "treble stimulus must leave low element {i} flat"
            );
        }
        for i in (n - quarter)..n {
            assert!(
                high[i] > 0.5,
                "treble stimulus must raise high element {i}, got {}",
                high[i]
            );
            assert_eq!(
                low[i], 0.0,
                "bass stimulus must leave high element {i} flat"
            );
        }
    }

    /// Plan 0034 Phase 3 done-when 2. The downsample is a genuine **partition**
    /// of the band array at every element count the loader admits: contiguous,
    /// non-overlapping, complete. Proved by conservation — the element means,
    /// weighted by how many bands each covers, must sum to the band total, which
    /// is only possible if every band was counted exactly once.
    #[test]
    fn every_band_is_counted_exactly_once_at_every_element_count() {
        // A distinct value per band, so a dropped or double-counted band shifts
        // the total rather than cancelling out.
        let mut spectrum = [0.0f32; SPECTRUM_BINS];
        for (i, band) in spectrum.iter_mut().enumerate() {
            *band = (i + 1) as f32;
        }
        let total: f32 = spectrum.iter().sum();

        // Every count the loader admits, walked end to end at the small sizes
        // and sampled at the awkward ones (7, 24, 30, 31 do not divide 64).
        for n in 2..=SPECTRUM_BINS {
            let mut levels = vec![0.0; n];
            downsample(&spectrum, &mut levels);

            let mut covered = 0usize;
            let mut weighted = 0.0f32;
            for (i, &level) in levels.iter().enumerate() {
                let lo = i * SPECTRUM_BINS / n;
                let hi = (i + 1) * SPECTRUM_BINS / n;
                assert_eq!(
                    lo,
                    covered,
                    "element {i} of {n} must start where element {} ended",
                    i.saturating_sub(1)
                );
                assert!(hi > lo, "element {i} of {n} must cover at least one band");
                covered = hi;
                weighted += level * (hi - lo) as f32;
            }
            assert_eq!(covered, SPECTRUM_BINS, "{n} elements must cover every band");
            assert!(
                (weighted - total).abs() < 1e-2,
                "{n} elements lose or duplicate energy: {weighted} vs {total}"
            );
        }
    }

    /// The bars layout is what a spectrum readout should be: one segment per
    /// element, evenly spaced left to right, standing on a common baseline.
    #[test]
    fn bars_are_evenly_spaced_upright_segments_on_one_baseline() {
        let levels = [0.0f32, 0.5, 1.0, 0.25];
        let plain: Vec<f32> = levels
            .iter()
            .map(|&level| element_length(level, 0.0, 1.0))
            .collect();
        let out = figure(SpectrumLayout::Bars, &plain);
        assert_eq!(out.len(), levels.len(), "one segment per element");

        let spacing = out[1].a[0] - out[0].a[0];
        for (i, seg) in out.iter().enumerate() {
            assert_eq!(seg.a[0], seg.b[0], "element {i} is upright");
            assert_eq!(
                seg.a[1], DEFAULT_BASELINE,
                "element {i} stands on the baseline"
            );
            assert!(
                seg.a[0].abs() <= DEFAULT_SPAN,
                "element {i} stays inside the span"
            );
            if i > 0 {
                assert!(
                    ((out[i].a[0] - out[i - 1].a[0]) - spacing).abs() < 1e-6,
                    "elements are evenly spaced"
                );
            }
        }
        // Length tracks the level, and `base` lifts every element off the floor.
        // Compared approximately: a length is a difference of two world-space
        // f32 coordinates, so it carries the baseline's rounding.
        let length = |seg: &SegmentInstance| seg.b[1] - seg.a[1];
        assert!(length(&out[0]).abs() < 1e-6, "a silent element is flat");
        assert!(
            (length(&out[2]) - 1.0).abs() < 1e-6,
            "a full element is full"
        );

        let lifted_lengths: Vec<f32> = levels
            .iter()
            .map(|&level| element_length(level, 0.1, 1.0))
            .collect();
        let lifted = figure(SpectrumLayout::Bars, &lifted_lengths);
        assert!(
            (length(&lifted[0]) - 0.1).abs() < 1e-6,
            "base is the resting length, so silence still draws a comb"
        );
        // And a level can never pull an element below its baseline, whatever a
        // degenerate `scale` does.
        assert_eq!(element_length(1.0, 0.0, -5.0), 0.0, "length floors at zero");
    }

    /// Plan 0038 Phase 3 done-when 2, ADR-0040's totality clause. `curve_level`
    /// runs per element per frame on the render path with no author guard in
    /// front of it, so **every** degenerate combination has to land on a finite,
    /// non-negative number — not merely the ones an author is likely to write.
    #[test]
    fn the_level_curve_is_total_over_degenerate_input() {
        // `1.0` is exactly the identity, which is the property the whole
        // byte-identical-goldens claim rests on. Asserted bit-for-bit.
        for level in [0.0f32, 0.003, 0.03, 0.5, 1.0, 7.5] {
            assert_eq!(
                curve_level(level, 1.0),
                level,
                "curve = 1.0 must be exactly `powf(x, 1.0) == x` for {level}"
            );
        }

        // The exponent is the author-reachable half: `curve` is an expression, so
        // every one of these is something a preset can actually produce.
        let exponents = [
            0.0f32,
            -0.0,
            -1.0,
            -1e30,
            1e30,
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::MIN_POSITIVE,
            0.5,
            CURVE_MIN,
            CURVE_MAX,
        ];
        // The level is the engine-side half — it comes from `downsample` over the
        // band array, never from an expression. These span far past anything the
        // DSP produces (bands sit around 0.02-0.05).
        let levels = [0.0f32, -0.0, -1.0, -1e3, f32::MIN_POSITIVE, 0.03, 1.0, 1e3];

        for &curve in &exponents {
            // Whatever the exponent, a level the DSP can produce stays drawable
            // all the way through to the geometry.
            for &level in &levels {
                let out = curve_level(level, curve);
                assert!(
                    out.is_finite() && out >= 0.0,
                    "curve_level({level}, {curve}) = {out} is not a drawable level"
                );
                let len = element_length(out, 0.06, 1.1);
                assert!(
                    len.is_finite() && len >= 0.0,
                    "element_length after curve_level({level}, {curve}) = {len}"
                );
            }
            // And the weaker claim that holds for *any* level at all, including
            // the non-finite ones no band array should ever contain: the curve
            // never manufactures a `NaN` and never inverts an element. An
            // infinite level still maps to an infinite length, exactly as it did
            // through `element_length` before this step existed — that is the
            // DSP's boundary to hold, not this function's.
            for &level in &[f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
                let out = curve_level(level, curve);
                assert!(
                    !out.is_nan() && out >= 0.0,
                    "curve_level({level}, {curve}) = {out} introduced a NaN or a \
                     negative where the input had neither"
                );
            }
        }
        // A non-finite level that is not `+inf` is neutralised outright, because
        // the floor runs before the `powf`.
        assert_eq!(curve_level(f32::NAN, 0.5), 0.0);
        assert_eq!(curve_level(f32::NEG_INFINITY, 0.5), 0.0);

        // The two clauses that make `pow(0, 0)` and `pow(0, -1)` unreachable: the
        // exponent floor is strictly positive, so a silent element stays at zero
        // rather than jumping to one or to infinity.
        assert_eq!(curve_level(0.0, 0.0), 0.0, "pow(0, 0) is not reachable");
        assert_eq!(curve_level(0.0, -1.0), 0.0, "pow(0, -1) is not reachable");
        assert_eq!(
            curve_level(0.0, f32::NAN),
            0.0,
            "a NaN exponent falls back to the linear default, not through clamp"
        );
        // A negative level can never become a fractional power of a negative
        // base, because it is floored before the `powf` rather than after.
        assert_eq!(curve_level(-4.0, 0.5), 0.0, "a negative level floors first");
        // The exponent clamp bites at both ends rather than passing through.
        assert_eq!(curve_level(0.25, 10.0), curve_level(0.25, CURVE_MAX));
        assert_eq!(curve_level(0.25, 0.0001), curve_level(0.25, CURVE_MIN));
    }

    /// Plan 0038 Phase 3 done-when 3, and the reason ADR-0040 exists. The
    /// ordering is invisible in a still frame — it only shows in *motion* — so
    /// nothing but a test stops a later refactor from swapping two lines and
    /// silently inverting the decision.
    ///
    /// Asserted as a **property**, not against a tuned constant: for a
    /// compressive `curve` and a non-instant smoother, curving-then-easing and
    /// easing-then-curving disagree, and they disagree in a *stated direction* —
    /// during a fall the curve-first value sits **below** the ease-first one,
    /// because curving after the smoother stretches the decay by `1 / curve` and
    /// a slower decay is a higher value at every instant. **Swap the two steps in
    /// `update` and this fails.**
    ///
    /// The direction is all this asserts. It is *not* evidence for the "even
    /// fall" ADR-0040 originally claimed — Plan 0038 Phase 3 measured that and
    /// found both orderings to be exponentials of identical shape, differing only
    /// in speed (see the ADR's Outcome). The ordering is still worth pinning,
    /// because it is invisible in a still frame and a refactor can swap two lines.
    #[test]
    fn the_curve_runs_before_the_easing_and_the_two_orders_differ() {
        const CURVE: f32 = 0.5;
        const DT: f32 = 1.0 / 60.0;
        let easing = Easing {
            attack: 0.02,
            release: 0.5,
        };

        // A step down from a settled loud element to silence — the fall ADR-0040
        // reasons about. Both orders start from the same settled state, so the
        // only thing that differs downstream is where the curve sits.
        let settled = curve_level(1.0, CURVE);
        let (mut curve_first, mut ease_first) = (settled, 1.0f32);
        let mut saw_difference = false;

        for frame in 0..90 {
            // Curve then ease: the smoother sees the compressed value. This is
            // exactly what `update` does.
            curve_first = easing.step(curve_first, curve_level(0.0, CURVE), DT);
            // Ease then curve: the smoother sees the linear value and the curve
            // is applied to the eased result. The rejected alternative.
            ease_first = easing.step(ease_first, 0.0, DT);
            let ease_first_shown = curve_level(ease_first, CURVE);

            if (curve_first - ease_first_shown).abs() > 1e-6 {
                saw_difference = true;
                assert!(
                    curve_first < ease_first_shown,
                    "frame {frame}: curve-then-ease ({curve_first}) should sit \
                     below ease-then-curve ({ease_first_shown}) during a fall — \
                     applying the curve after the smoother stretches the decay \
                     by 1 / curve, and the slower fall is the higher value"
                );
            }
        }
        assert!(
            saw_difference,
            "the two orders never diverged, so this test would pass with the \
             steps swapped — the fixture is not exercising ADR-0040 at all"
        );

        // ...and the ordering is a no-op at the default, which is the other half
        // of the claim: `curve = 1.0` makes the two orders identical, so the
        // decision costs nothing until a preset opts in.
        let (mut a, mut b) = (1.0f32, 1.0f32);
        for _ in 0..30 {
            a = easing.step(a, curve_level(0.0, 1.0), DT);
            b = curve_level(easing.step(b, 0.0, DT), 1.0);
            assert_eq!(a, b, "at curve = 1.0 the two orders must coincide");
        }
    }

    /// Plan 0038 Phase 2. `span` and `baseline` place the figure in **world**
    /// space — no aspect and no target size is read anywhere in this scene to
    /// compute them (ADR-0037), which is why doubling `span` is exactly doubling
    /// an x coordinate and nothing else.
    #[test]
    fn span_and_baseline_place_the_figure_in_world_space() {
        let lengths = [0.2f32, 0.5, 0.9, 0.4];

        for layout in [SpectrumLayout::Bars, SpectrumLayout::Polyline] {
            let narrow = placed(layout, &lengths, Placement::default());
            let wide = placed(
                layout,
                &lengths,
                Placement {
                    span: 2.0 * DEFAULT_SPAN,
                    ..Placement::default()
                },
            );
            for (i, (n, w)) in narrow.iter().zip(&wide).enumerate() {
                assert!(
                    (w.a[0] - 2.0 * n.a[0]).abs() < 1e-6 && (w.b[0] - 2.0 * n.b[0]).abs() < 1e-6,
                    "{layout:?} segment {i}: doubling span must double x, got {} from {}",
                    w.a[0],
                    n.a[0]
                );
                assert_eq!(
                    (w.a[1], w.b[1]),
                    (n.a[1], n.b[1]),
                    "{layout:?} segment {i}: span must not move y"
                );
            }

            // `baseline` is a pure y offset, again with no x effect.
            let lifted = placed(
                layout,
                &lengths,
                Placement {
                    baseline: DEFAULT_BASELINE + 0.5,
                    ..Placement::default()
                },
            );
            for (i, (n, l)) in narrow.iter().zip(&lifted).enumerate() {
                assert!(
                    (l.a[1] - n.a[1] - 0.5).abs() < 1e-6,
                    "{layout:?} segment {i}: baseline must offset y by exactly its change"
                );
                assert_eq!(
                    l.a[0], n.a[0],
                    "{layout:?} segment {i}: baseline moves no x"
                );
            }
        }

        // Both are no-ops on the ring, which `radius` sizes instead.
        let ring = placed(SpectrumLayout::RadialRing, &lengths, Placement::default());
        let ring_moved = placed(
            SpectrumLayout::RadialRing,
            &lengths,
            Placement {
                span: 4.0,
                baseline: 0.7,
                ..Placement::default()
            },
        );
        assert_eq!(
            ring.iter().map(|s| (s.a, s.b)).collect::<Vec<_>>(),
            ring_moved.iter().map(|s| (s.a, s.b)).collect::<Vec<_>>(),
            "span and baseline are no-ops on the radial ring"
        );
    }

    /// Plan 0038 Phase 2 done-when 3, the design-backlog 0018 fix. The geometry
    /// mirror reflects across the **x-axis** (`lines/mod.rs`) on every line
    /// scene alike; nothing about that changes here. What changes is where the
    /// figure stands. At `baseline = 0` the two copies share one foot line — the
    /// symmetric "landscape and its reflection". At the default `-0.85` they
    /// have two feet lines 1.7 apart, so the copy hangs from the top edge
    /// instead of mirroring about a shared centre.
    #[test]
    fn baseline_zero_mirrors_the_readout_about_the_frame_centre() {
        let lengths = [0.2f32, 0.5, 0.9, 0.4];
        let mirror = MirrorSpec::from_params(1.0, 1.0);
        assert!(!mirror.is_identity(), "the probe must actually replicate");

        // The distinct y values the bars' feet (their `a` endpoints) rest on,
        // rounded so float noise cannot invent a second line.
        let feet = |place: Placement| -> Vec<i32> {
            let single = placed(SpectrumLayout::Bars, &lengths, place);
            let mut out = Vec::new();
            replicate_mirror(&single, mirror, FLOOR_CAP, &mut out);
            let mut ys: Vec<i32> = out.iter().map(|s| (s.a[1] * 1e4) as i32).collect();
            ys.sort_unstable();
            ys.dedup();
            ys
        };

        assert_eq!(
            feet(Placement {
                baseline: 0.0,
                ..Placement::default()
            }),
            vec![0],
            "baseline = 0: both copies stand on the one centre line"
        );
        let default_feet = feet(Placement::default());
        assert_eq!(
            default_feet.len(),
            2,
            "the default baseline gives the pair two separate foot lines"
        );
        assert_eq!(
            default_feet,
            vec![-8500, 8500],
            "and they sit at -0.85 and +0.85, 1.7 apart — the copy against the \
             top edge that design-backlog 0018 reported"
        );
    }

    /// Plan 0034 Phase 3 done-when 1: each layout draws the *same data* as a
    /// different figure, and each is structurally what its name claims.
    #[test]
    fn each_layout_draws_the_same_levels_as_its_own_figure() {
        let levels = [0.1f32, 0.4, 0.9, 0.3, 0.6];

        // Polyline: a connected chain, so consecutive segments share an endpoint
        // and there is one fewer segment than elements.
        let out = figure(SpectrumLayout::Polyline, &levels);
        assert_eq!(out.len(), levels.len() - 1, "n-1 segments join n points");
        for i in 1..out.len() {
            assert_eq!(
                out[i - 1].b,
                out[i].a,
                "segment {i} continues from the previous one"
            );
        }
        // The chain spans the full width, edge to edge.
        assert!(
            (out[0].a[0] + DEFAULT_SPAN).abs() < 1e-6,
            "starts at the left edge"
        );
        assert!(
            (out[out.len() - 1].b[0] - DEFAULT_SPAN).abs() < 1e-6,
            "ends at the right edge"
        );

        // Radial ring: one spoke per element, each pointing away from the origin
        // and starting on a circle of the configured inner radius.
        let mut out = Vec::new();
        build(
            SpectrumLayout::RadialRing,
            &levels,
            &even_widths(levels.len()),
            &white(levels.len()),
            Placement {
                radius: 0.4,
                ..Placement::default()
            },
            &mut out,
        );
        assert_eq!(out.len(), levels.len(), "one spoke per element");
        for (i, seg) in out.iter().enumerate() {
            let inner = (seg.a[0] * seg.a[0] + seg.a[1] * seg.a[1]).sqrt();
            let outer = (seg.b[0] * seg.b[0] + seg.b[1] * seg.b[1]).sqrt();
            assert!(
                (inner - 0.4).abs() < 1e-5,
                "spoke {i} starts on the ring, got {inner}"
            );
            assert!(
                (outer - (0.4 + levels[i])).abs() < 1e-5,
                "spoke {i} reaches its own level outward"
            );
        }
        // The spokes are distinct directions covering the circle once.
        let angle = |seg: &SegmentInstance| seg.a[1].atan2(seg.a[0]);
        let expected = std::f32::consts::TAU / levels.len() as f32;
        for i in 1..out.len() {
            // Wrapped, because `atan2` returns -pi..pi and the walk crosses it.
            let step = (angle(&out[i]) - angle(&out[i - 1])).rem_euclid(std::f32::consts::TAU);
            assert!(
                (step - expected).abs() < 1e-5,
                "spoke {i} sits one even step around the circle, got {step}"
            );
        }
    }

    /// Plan 0039 Phase 2 done-when 1 and 2 (ADR-0041). This scene is the one
    /// producer emitting **both** connectivities from a single `build()`, so it
    /// is what proves the flag is per endpoint rather than per scene.
    ///
    /// The isolated half is the load-bearing one: it is the done-when that would
    /// have caught the rejected unconditional-extend design. A bar whose `a` end
    /// extended would hang below `baseline` — breaking the `baseline = 0`
    /// centre-mirror Plan 0038 shipped — and a spoke whose `a` end extended would
    /// grow inward through `radius` and fill the inner circle.
    #[test]
    fn only_the_polyline_declares_its_endpoints_joined() {
        let levels = [0.1f32, 0.4, 0.9, 0.3, 0.6];

        // Chained: every interior vertex is a joint; the figure's two outer ends
        // stay free, or the stroke would run half a width past each edge.
        let chain = figure(SpectrumLayout::Polyline, &levels);
        assert_eq!(
            chain.iter().map(|s| s.joined).collect::<Vec<_>>(),
            vec![JOINED_B, JOINED_A | JOINED_B, JOINED_A | JOINED_B, JOINED_A,],
            "the interior endpoints of the chain are joined and only those"
        );
        // ...and every flag matches a genuinely shared point, which is the
        // invariant nothing else in the pipeline validates.
        for i in 1..chain.len() {
            assert_eq!(chain[i - 1].b, chain[i].a, "segment {i} shares a point");
            assert_ne!(
                chain[i - 1].joined & JOINED_B,
                0,
                "seen from the segment before"
            );
            assert_ne!(chain[i].joined & JOINED_A, 0, "and from the one after");
        }
        // Two elements make one segment, which is all end and no joint.
        let lone = figure(SpectrumLayout::Polyline, &[0.3, 0.7]);
        assert_eq!(lone.len(), 1);
        assert_eq!(lone[0].joined, 0, "a lone segment has two free ends");

        // Isolated: one segment per element, both ends free, and the endpoints
        // stay exactly where they always were.
        let bars = figure(SpectrumLayout::Bars, &levels);
        for (i, seg) in bars.iter().enumerate() {
            assert_eq!(seg.joined, 0, "bar {i} is isolated");
            assert_eq!(
                seg.a[1], DEFAULT_BASELINE,
                "bar {i} still stands on the baseline"
            );
            assert!(
                (seg.b[1] - (DEFAULT_BASELINE + levels[i])).abs() < 1e-6,
                "bar {i} still ends exactly at baseline + length"
            );
        }

        let ring = placed(
            SpectrumLayout::RadialRing,
            &levels,
            Placement {
                radius: 0.4,
                ..Placement::default()
            },
        );
        for (i, seg) in ring.iter().enumerate() {
            assert_eq!(seg.joined, 0, "spoke {i} is isolated");
            let inner = (seg.a[0] * seg.a[0] + seg.a[1] * seg.a[1]).sqrt();
            assert!(
                (inner - 0.4).abs() < 1e-5,
                "spoke {i} still starts exactly on the ring, got {inner}"
            );
        }
    }

    /// `rotation` turns the whole figure about the origin — the same angle for
    /// every layout, so it is never a silent no-op on one of them.
    #[test]
    fn rotation_turns_every_layout_by_the_same_angle() {
        let levels = [0.2f32, 0.7, 0.4, 0.5];
        let quarter = std::f32::consts::FRAC_PI_2;

        for layout in [
            SpectrumLayout::Bars,
            SpectrumLayout::Polyline,
            SpectrumLayout::RadialRing,
        ] {
            let plain = figure(layout, &levels);
            let mut turned = Vec::new();
            build(
                layout,
                &levels,
                &even_widths(levels.len()),
                &white(levels.len()),
                Placement {
                    rotation: quarter,
                    ..Placement::default()
                },
                &mut turned,
            );
            assert_eq!(plain.len(), turned.len(), "{layout:?} keeps its segments");
            for (a, b) in plain.iter().zip(&turned) {
                // A quarter turn maps (x, y) to (-y, x).
                assert!(
                    (b.a[0] + a.a[1]).abs() < 1e-5 && (b.a[1] - a.a[0]).abs() < 1e-5,
                    "{layout:?} start point is not rotated a quarter turn"
                );
                assert!(
                    (b.b[0] + a.b[1]).abs() < 1e-5 && (b.b[1] - a.b[0]).abs() < 1e-5,
                    "{layout:?} end point is not rotated a quarter turn"
                );
            }
        }
    }

    /// Plan 0034 Phase 3 done-when 3. Per-element easing is expressed in seconds
    /// and is **frame-rate independent**: reaching the same wall-clock time
    /// through many small steps or a few large ones lands in the same place, so a
    /// preset looks the same at 60 and 144 Hz (ADR-0019).
    #[test]
    fn per_element_easing_is_frame_rate_independent() {
        let easing = Easing::symmetric(0.25);
        let settle = |steps: u32, dt: f32| {
            let mut held = 0.0f32;
            for _ in 0..steps {
                held = easing.step(held, 1.0, dt);
            }
            held
        };
        // One second of wall clock at 60, 144 and 30 fps.
        let at_60 = settle(60, 1.0 / 60.0);
        let at_144 = settle(144, 1.0 / 144.0);
        let at_30 = settle(30, 1.0 / 30.0);
        assert!(
            (at_60 - at_144).abs() < 1e-3 && (at_60 - at_30).abs() < 1e-3,
            "one second of easing must land in the same place: 60 {at_60}, 144 {at_144}, 30 {at_30}"
        );
        // And it is genuinely easing rather than snapping or stalling.
        assert!(
            at_60 > 0.9 && at_60 < 1.0,
            "a 0.25 s constant is most of the way there after 1 s, got {at_60}"
        );
        // `INSTANT` (the default) passes the raw value straight through.
        assert_eq!(Easing::INSTANT.step(0.0, 0.6, 1.0 / 60.0), 0.6);
    }

    /// A degenerate element count must not panic or produce garbage geometry —
    /// this runs per frame.
    #[test]
    fn a_degenerate_element_count_is_inert() {
        let mut none: Vec<f32> = Vec::new();
        downsample(&[0.5; SPECTRUM_BINS], &mut none);
        for layout in [
            SpectrumLayout::Bars,
            SpectrumLayout::Polyline,
            SpectrumLayout::RadialRing,
        ] {
            let mut out = vec![SegmentInstance {
                a: [9.0, 9.0],
                b: [9.0, 9.0],
                color: [1.0, 1.0, 1.0],
                width: 1.0,
                joined: 0,
            }];
            build(layout, &none, &[], &[], Placement::default(), &mut out);
            assert!(out.is_empty(), "{layout:?}: no elements, no segments");

            // Short width/colour lists fall back rather than panicking — the hot
            // path must stay total even if the buffers ever disagree.
            build(
                layout,
                &[0.3, 0.4],
                &[],
                &[],
                Placement::default(),
                &mut out,
            );
            assert!(
                out.iter().all(|s| s.width > 0.0),
                "{layout:?}: a missing width falls back to a drawable one"
            );
        }
        // A single element has no polyline to draw and must not underflow.
        assert!(
            figure(SpectrumLayout::Polyline, &[0.5]).is_empty(),
            "one point is not a line"
        );

        // An empty spectrum leaves every element at zero rather than reading
        // stale values.
        let mut levels = vec![7.0; 4];
        downsample(&[], &mut levels);
        assert_eq!(levels, vec![0.0; 4]);
    }
}
