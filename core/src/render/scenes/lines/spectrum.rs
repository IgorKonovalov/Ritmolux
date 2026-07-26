//! Spectrum scene (Plan 0034 / ADR-0036): the analysis frame's log-spaced band
//! array, drawn as N elements.
//!
//! This is a **fourth consumer of the existing line idiom**, not a fifth render
//! idiom: N bars are a segment list, and they go out through the same shared
//! [`LineRenderer`](super::LineRenderer) the three other line scenes draw
//! through (ADR-0007). Nothing new is uploaded, no new pipeline is built, and
//! the `Scene` trait is untouched — `update` already receives the whole
//! [`AnalysisFrame`], bands included.
//!
//! The per-frame work is two small pure steps, [`downsample`] and
//! [`build_bars`], both free functions over preallocated buffers. They are
//! separate from the scene so the claim that matters — that low elements track
//! low frequencies — is testable without a GPU.
//!
//! Phase 2 is deliberately one layout (upright bars) at a fixed element count;
//! the `[spectrum]` table (count, layout, per-element easing) lands in Phase 3.

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
use super::renderer::{LineRenderer, SegmentInstance};
use super::{CapOverflow, GeneratorConfig, MAX_SEGMENTS, ViewTransform, palette};
use crate::dsp::AnalysisFrame;

/// Maps `thickness` to an NDC-y half-width — the same scale the other line
/// scenes use, so a `thickness` that reads well on the rose reads the same here.
const WIDTH_SCALE: f32 = 0.003;

/// How many elements the readout draws. Fixed for Phase 2; Phase 3's
/// `[spectrum]` table makes it the author's choice (and the plan's "20-30
/// points" is why the fixed value sits in that range).
pub const DEFAULT_ELEMENTS: usize = 24;

/// World-space half-width the readout spans. The renderer divides x by the
/// target aspect, so this is a **world** extent, not a screen one — the scene
/// never sees an aspect and so cannot take one from the wrong place (ADR-0037).
const SPAN_X: f32 = 1.0;

/// World-space y the bars stand on.
const BASELINE_Y: f32 = -0.85;

// Parameter defaults — a legible, calm readout when a preset binds nothing.
const DEFAULT_THICKNESS: f32 = 6.0;
const DEFAULT_HUE: f32 = 0.55;
const DEFAULT_BRIGHTNESS: f32 = 1.0;
const DEFAULT_SCALE: f32 = 1.2;
/// Minimum element length, in world units. Non-zero on purpose: a spectrum
/// readout at rest is a comb, not an empty frame, so the figure stays on screen
/// (and legible) through a silence instead of vanishing.
const DEFAULT_BASE: f32 = 0.06;
// Shared view transform (ADR-0018): identity by default.
const DEFAULT_ZOOM: f32 = 1.0;
const DEFAULT_PAN: f32 = 0.0;

/// Parameter vocabulary — see [`fragment_field::PARAMS`](crate::render::scenes::fragment_field::PARAMS).
/// **Keep in sync with `set_param` below.**
pub const PARAMS: &[&str] = &[
    "thickness",
    "hue",
    "brightness",
    "scale",
    "base",
    "zoom",
    "pan_x",
    "pan_y",
];

/// The per-frame style scalars [`build_bars`] needs, gathered so the geometry
/// step is a pure function of `(levels, style)`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct BarStyle {
    /// Minimum element length in world units (the `base` param).
    pub base: f32,
    /// Multiplier on the element's band level (the `scale` param).
    pub scale: f32,
    /// Stroke colour, brightness already folded in.
    pub color: [f32; 3],
    /// Half-width in NDC-y units.
    pub width: f32,
}

/// Reduce the engine's band array to `levels.len()` elements by **averaging each
/// element's own contiguous slice of bands**.
///
/// Element `i` covers `[i * bands / n, (i + 1) * bands / n)`. That is a genuine
/// partition — contiguous, non-overlapping, and complete — so no band is dropped
/// or double-counted at any element count up to the band count. It is also
/// deterministic: integer arithmetic on lengths, no clock and no rounding mode
/// to disagree about.
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
        // available; the loader caps the count so that never happens in practice.
        let hi = (((i + 1) * bands / n).min(bands)).max(lo + 1);
        let slice = spectrum.get(lo..hi).unwrap_or(&[]);
        *level = if slice.is_empty() {
            0.0
        } else {
            slice.iter().sum::<f32>() / slice.len() as f32
        };
    }
}

/// Lay `levels` out as upright bars: one segment per element, evenly spaced
/// across `-SPAN_X..SPAN_X`, standing on [`BASELINE_Y`] and reaching
/// `base + scale * level` upward.
///
/// Allocation-free into a preallocated `out` (cleared first) — the per-frame
/// half of the scene.
pub(crate) fn build_bars(levels: &[f32], style: BarStyle, out: &mut Vec<SegmentInstance>) {
    out.clear();
    let n = levels.len();
    if n == 0 {
        return;
    }
    let step = 2.0 * SPAN_X / n as f32;
    for (i, &level) in levels.iter().enumerate() {
        let x = -SPAN_X + step * (i as f32 + 0.5);
        // A negative or non-finite level (impossible from the DSP, cheap to
        // exclude) must not produce a bar hanging below the baseline.
        let length = (style.base + style.scale * level).max(0.0);
        out.push(SegmentInstance {
            a: [x, BASELINE_Y],
            b: [x, BASELINE_Y + length],
            color: style.color,
            width: style.width,
        });
    }
}

/// The spectrum readout: N elements driven by the analysis frame's band array,
/// drawn through the shared line renderer.
pub struct SpectrumScene {
    /// The single line renderer, shared with the other line scenes (ADR-0007:
    /// "one line renderer"). Only the active scene draws in a frame.
    renderer: Rc<RefCell<LineRenderer>>,
    /// The drawn geometry. Preallocated so a frame never allocates.
    segments: Vec<SegmentInstance>,
    /// Per-element band levels, **sized once** (here) rather than per frame.
    levels: Vec<f32>,
    thickness: f32,
    hue: f32,
    brightness: f32,
    scale: f32,
    base: f32,
    zoom: f32,
    pan_x: f32,
    pan_y: f32,
}

impl SpectrumScene {
    /// Build the scene over the shared line renderer, preallocating both its
    /// element buffer and its segment buffer.
    pub fn new(renderer: Rc<RefCell<LineRenderer>>) -> Self {
        Self {
            renderer,
            segments: Vec::with_capacity(MAX_SEGMENTS),
            levels: vec![0.0; DEFAULT_ELEMENTS],
            thickness: DEFAULT_THICKNESS,
            hue: DEFAULT_HUE,
            brightness: DEFAULT_BRIGHTNESS,
            scale: DEFAULT_SCALE,
            base: DEFAULT_BASE,
            zoom: DEFAULT_ZOOM,
            pan_x: DEFAULT_PAN,
            pan_y: DEFAULT_PAN,
        }
    }
}

impl Scene for SpectrumScene {
    fn name(&self) -> &'static str {
        "spectrum"
    }

    fn reset_params(&mut self) {
        self.thickness = DEFAULT_THICKNESS;
        self.hue = DEFAULT_HUE;
        self.brightness = DEFAULT_BRIGHTNESS;
        self.scale = DEFAULT_SCALE;
        self.base = DEFAULT_BASE;
        self.zoom = DEFAULT_ZOOM;
        self.pan_x = DEFAULT_PAN;
        self.pan_y = DEFAULT_PAN;
    }

    fn set_param(&mut self, name: &str, value: f32) {
        match name {
            "thickness" => self.thickness = value,
            "hue" => self.hue = value,
            "brightness" => self.brightness = value,
            "scale" => self.scale = value,
            "base" => self.base = value,
            "zoom" => self.zoom = value,
            "pan_x" => self.pan_x = value,
            "pan_y" => self.pan_y = value,
            _ => {}
        }
    }

    fn configure(&mut self, _cfg: &GeneratorConfig) -> Option<CapOverflow> {
        // Phase 2 has no `[spectrum]` table yet, so every structural config
        // belongs to another scene. Nothing is built here, so nothing truncates.
        None
    }

    fn update(&mut self, frame: &AnalysisFrame) {
        downsample(&frame.spectrum, &mut self.levels);
        let base = palette(self.hue);
        let style = BarStyle {
            base: self.base,
            scale: self.scale,
            color: [
                base[0] * self.brightness,
                base[1] * self.brightness,
                base[2] * self.brightness,
            ],
            width: (self.thickness * WIDTH_SCALE).max(0.0005),
        };
        build_bars(&self.levels, style, &mut self.segments);
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
        self.renderer
            .borrow_mut()
            .draw(queue, encoder, view, aspect, 1.0, xform, &self.segments);
    }
}

#[cfg(test)]
mod tests {
    // Test asserts index the produced Vec; allowed here over the file's
    // hot-path pragma since test code is not the render path.
    #![allow(clippy::indexing_slicing)]

    use super::*;
    use crate::dsp::SPECTRUM_BINS;

    fn style() -> BarStyle {
        BarStyle {
            base: 0.0,
            scale: 1.0,
            color: [1.0, 1.0, 1.0],
            width: 0.01,
        }
    }

    /// The element lengths a stimulus produces, low index first.
    fn bar_lengths(spectrum: &[f32; SPECTRUM_BINS], elements: usize) -> Vec<f32> {
        let mut levels = vec![0.0; elements];
        downsample(spectrum, &mut levels);
        let mut out = Vec::new();
        build_bars(&levels, style(), &mut out);
        out.iter().map(|s| s.b[1] - s.a[1]).collect()
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
        let n = DEFAULT_ELEMENTS;
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

    /// The downsample is a genuine **partition** of the band array at every
    /// element count the loader admits: contiguous, non-overlapping, complete.
    /// Proved by conservation — the element means, weighted by how many bands
    /// each covers, must sum to the band total, which is only possible if every
    /// band was counted exactly once.
    #[test]
    fn every_band_is_counted_exactly_once_at_every_element_count() {
        // A distinct value per band, so a dropped or double-counted band shifts
        // the total rather than cancelling out.
        let mut spectrum = [0.0f32; SPECTRUM_BINS];
        for (i, band) in spectrum.iter_mut().enumerate() {
            *band = (i + 1) as f32;
        }
        let total: f32 = spectrum.iter().sum();

        for n in [2usize, 3, 7, 16, 24, 30, 31, SPECTRUM_BINS] {
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
                    "element {i} of {n} must start where {} ended",
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

    /// The geometry is what a spectrum readout should be: one segment per
    /// element, evenly spaced left to right, standing on a common baseline.
    #[test]
    fn bars_are_evenly_spaced_upright_segments_on_one_baseline() {
        let levels = [0.0f32, 0.5, 1.0, 0.25];
        let mut out = Vec::new();
        build_bars(&levels, style(), &mut out);
        assert_eq!(out.len(), levels.len(), "one segment per element");

        let spacing = out[1].a[0] - out[0].a[0];
        for (i, seg) in out.iter().enumerate() {
            assert_eq!(seg.a[0], seg.b[0], "element {i} is upright");
            assert_eq!(seg.a[1], BASELINE_Y, "element {i} stands on the baseline");
            assert!(
                seg.a[0].abs() <= SPAN_X,
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

        let mut lifted = Vec::new();
        build_bars(
            &levels,
            BarStyle {
                base: 0.1,
                ..style()
            },
            &mut lifted,
        );
        assert!(
            (length(&lifted[0]) - 0.1).abs() < 1e-6,
            "base is the resting length, so silence still draws a comb"
        );
    }

    /// A degenerate element count must not panic or produce garbage geometry —
    /// this runs per frame.
    #[test]
    fn a_degenerate_element_count_is_inert() {
        let mut none: Vec<f32> = Vec::new();
        downsample(&[0.5; SPECTRUM_BINS], &mut none);
        let mut out = vec![SegmentInstance {
            a: [9.0, 9.0],
            b: [9.0, 9.0],
            color: [1.0, 1.0, 1.0],
            width: 1.0,
        }];
        build_bars(&none, style(), &mut out);
        assert!(out.is_empty(), "no elements, no segments");

        // An empty spectrum leaves every element at zero rather than reading
        // stale values.
        let mut levels = vec![7.0; 4];
        downsample(&[], &mut levels);
        assert_eq!(levels, vec![0.0; 4]);
    }
}
