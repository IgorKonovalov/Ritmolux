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
//!   `saturation` — the colour surface (ADR-0021), sampled on the CPU. This was
//!   the **first line scene to honor the palette**, and since Plan 0054
//!   (ADR-0059) it is the pattern the other three follow: each walks
//!   `hue_spread` along the axis its own generator makes meaningful — path
//!   position on [`parametric`](super::parametric), generation depth on
//!   [`lsystem`](super::lsystem), radius on [`star`](super::star) — and this
//!   scene's is **band index**, unchanged. It mattered here first because
//!   colouring elements along their own axis is what turns a frequency readout
//!   into a look. The default `spectrum` palette is the engine cosine, so an
//!   author who sets no `[palette]` sees the usual colour language.
//! - `thickness` / `brightness` / `scale` / `base` — ordinary stroke styling.
//! - `curve` — the level-shaping exponent (ADR-0040, Plan 0038 Phase 3), applied
//!   to the downsampled level **before** the per-element smoother so the easing
//!   operates in the displayed domain. `1.0` is exactly linear. It is the third
//!   per-element lever on an element's length, beside `base` and `scale`.
//! - `glow` — the line renderer's per-segment falloff multiplier (Plan 0038),
//!   whole-figure like on the other three line scenes. Not a post bloom.
//! - `softness` — the across-the-stroke profile (ADR-0124), whole-figure and
//!   shared with the other three line scenes. Default `0.25`: a solid bar with
//!   a short shoulder. `1.0` is the pure quadratic falloff; `0` is solid with a
//!   one-pixel edge. A different quantity from
//!   `glow`, which scales the light and never the coverage.
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
use crate::render::palette::{self, Palette, desaturate};

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
/// one from the wrong place (ADR-0037). The honest consequence: *"fill the
/// width"* is aspect-dependent — `span ≈ 1.78` fills a 16:9 frame and leaves an
/// ultrawide short. There is deliberately no `fit` mode.
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
/// Level-shaping exponent (ADR-0040). `1.0` is exactly linear — `powf(x, 1.0) ==
/// x`, the map this scene had before Plan 0038 Phase 3 — `0.5` is a square root,
/// and lower values compress harder.
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
    "palette_steps",
    "palette_contour",
    "brightness",
    "glow",
    "softness",
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
                    alpha: 1.0,
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
                    alpha: 1.0,
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
                    alpha: 1.0,
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
    /// Hard palette bands and their contour (ADR-0078), raw as the preset
    /// bound them -- `palette::band_steps` / `band_contour` condition them on
    /// the way to the sample site.
    palette_steps: f32,
    palette_contour: f32,
    brightness: f32,
    glow: f32,
    softness: f32,
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
            palette_steps: palette::DEFAULT_PALETTE_STEPS,
            palette_contour: palette::DEFAULT_PALETTE_CONTOUR,
            brightness: DEFAULT_BRIGHTNESS,
            glow: DEFAULT_GLOW,
            softness: super::DEFAULT_SOFTNESS,
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
        self.palette_steps = palette::DEFAULT_PALETTE_STEPS;
        self.palette_contour = palette::DEFAULT_PALETTE_CONTOUR;
        self.brightness = DEFAULT_BRIGHTNESS;
        self.glow = DEFAULT_GLOW;
        self.softness = super::DEFAULT_SOFTNESS;
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
            "palette_steps" => self.palette_steps = value,
            "palette_contour" => self.palette_contour = value,
            "brightness" => self.brightness = value,
            "glow" => self.glow = value,
            "softness" => self.softness = value,
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
            | GeneratorConfig::Particles { .. }
            | GeneratorConfig::WarpMesh { .. } => {}
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
            // Hard bands on the palette coordinate (ADR-0078), the canonical
            // `palette::band_coord` called rather than copied. `palette_steps <= 1`
            // returns it untouched, so an unbound preset is byte-unchanged.
            let banded = palette::band_coord(hue, self.palette_steps);
            let rgb = desaturate(
                self.palette.sample(banded, self.palette_mix),
                self.saturation,
            );

            if let Some(slot) = self.lengths.get_mut(i) {
                *slot = element_length(level, base, scale);
            }
            if let Some(slot) = self.widths.get_mut(i) {
                *slot = super::half_width(thickness);
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
            self.softness,
            xform,
            &self.segments,
        );
    }
}

#[cfg(test)]
mod tests;
