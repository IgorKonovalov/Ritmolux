//! On-canvas text via glyphon (ADR-0009), behind the non-default `text` feature.
//!
//! A small, reusable seam: the frontend queues a list of positioned [`TextRun`]s
//! each frame; [`TextLayer`] shapes them and draws them in a second render pass
//! that loads (does not clear) the scene, so text composites over the visual in
//! the same frame. It lives in `core` — not the standalone — because that is
//! where the wgpu device/queue/surface live (ADR-0001: the frontend never sees a
//! backend); the `text` **feature**, not a crate boundary, keeps it out of the
//! plugin/default build. First consumer is Plan 0008's browse overlay; Plan
//! 0009's HUD reuses the same seam rather than a throwaway.

// Hot-path panic-denial pragma (Plan 0002 Phase 2; `render/` scan set). Runs
// every displayed frame while text is queued; a panic here is a visible crash.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use std::borrow::Cow;
use std::collections::HashMap;

use glyphon::{
    Attrs, Buffer, Cache, Color, Family, FontSystem, Metrics, Resolution, Shaping, SwashCache,
    TextArea, TextAtlas, TextBounds, TextRenderer, Viewport,
};

use super::panel::{Panel, PanelPass};

/// A single positioned run of text the frontend queues for the current frame.
/// Coordinates are top-left device pixels (matching the diagnostics overlay);
/// `color` is linear RGBA in `0.0..=1.0`. The public seam the overlay and a
/// later HUD both fill.
pub struct TextRun<'a> {
    /// The text to draw (a single line; no wrapping is applied).
    pub text: &'a str,
    /// Left edge, device pixels from the surface's top-left.
    pub x: f32,
    /// Top edge, device pixels from the surface's top-left.
    pub y: f32,
    /// Font size in device pixels.
    pub size: f32,
    /// Linear RGBA in `0.0..=1.0`.
    pub color: [f32; 4],
}

/// An owned copy of a queued run, held from [`TextLayer::queue`] until the flush
/// in `render()` — the caller's borrowed `&str` need not outlive its own frame.
struct OwnedRun {
    text: String,
    x: f32,
    y: f32,
    size: f32,
    color: [f32; 4],
}

/// Line height as a multiple of the font size. Runs are single-line, so this
/// only sets vertical extent, never wrapping.
pub const LINE_HEIGHT_RATIO: f32 = 1.25;

/// Measured widths kept before the cache is emptied and starts again. A frame
/// measures a few dozen strings, so this is many frames of browsing.
const MEASURE_CACHE_CAP: usize = 4096;

/// The width a run of text lays out to, shaped exactly as [`TextLayer`] draws
/// it: the system sans-serif family, advanced shaping, one line.
///
/// GPU-free — it owns a font system and nothing else — so layout that depends
/// on a measurement can be tested without a device. The fonts are the
/// machine's, so a width is only ever compared against another width from the
/// same measurer, never against a written-down number.
pub struct TextMeasure {
    font_system: FontSystem,
    buffer: Buffer,
    /// `(text, size bits) -> width`, so a list redrawn every frame shapes each
    /// name once rather than once per frame.
    cache: HashMap<(String, u32), f32>,
}

impl Default for TextMeasure {
    fn default() -> Self {
        Self::new()
    }
}

impl TextMeasure {
    /// A measurer over the system's fonts. Loads the font set, which is slow;
    /// build one and keep it.
    pub fn new() -> Self {
        let mut font_system = FontSystem::new();
        let buffer = Buffer::new(
            &mut font_system,
            Metrics::new(16.0, 16.0 * LINE_HEIGHT_RATIO),
        );
        Self {
            font_system,
            buffer,
            cache: HashMap::new(),
        }
    }

    /// The laid-out width of `text` at `size` device pixels. `0.0` for an empty
    /// string or a size that is not positive and finite.
    pub fn width(&mut self, text: &str, size: f32) -> f32 {
        if text.is_empty() || !size.is_finite() || size <= 0.0 {
            return 0.0;
        }
        let key = (text.to_owned(), size.to_bits());
        if let Some(&w) = self.cache.get(&key) {
            return w;
        }
        let Self {
            font_system,
            buffer,
            cache,
        } = self;
        buffer.set_metrics(Metrics::new(size, size * LINE_HEIGHT_RATIO));
        buffer.set_size(None, None);
        buffer.set_text(
            text,
            &Attrs::new().family(Family::SansSerif),
            Shaping::Advanced,
            None,
        );
        buffer.shape_until_scroll(font_system, false);
        let w = buffer
            .layout_runs()
            .map(|run| run.line_w)
            .fold(0.0_f32, f32::max);
        if cache.len() >= MEASURE_CACHE_CAP {
            cache.clear();
        }
        cache.insert(key, w);
        w
    }

    /// `text` shortened to fit `max_width` at `size`, with an ASCII ellipsis —
    /// [`fit_width`] over this measurer.
    pub fn fit<'a>(&mut self, text: &'a str, size: f32, max_width: f32) -> Cow<'a, str> {
        fit_width(text, max_width, |s| self.width(s, size))
    }
}

/// `text` shortened to fit `max_width` as `width` measures it, ending in `...`
/// when it had to cut.
///
/// **The property this holds, whatever `width` is:** the result's measured
/// width is at most `max_width`, and a `text` that already fits comes back
/// unchanged and borrowed. When not even `...` fits, the result is empty.
///
/// Cut on character boundaries, so a multi-byte name is never split inside a
/// code point. The longest fitting prefix is found by bisection and then
/// checked, stepping down while it does not fit, so a measurer that is not
/// monotone in the prefix length (kerning can make it so) still cannot return
/// a result wider than asked.
pub fn fit_width<'a>(
    text: &'a str,
    max_width: f32,
    mut width: impl FnMut(&str) -> f32,
) -> Cow<'a, str> {
    if width(text) <= max_width {
        return Cow::Borrowed(text);
    }
    let bounds: Vec<usize> = text.char_indices().map(|(i, _)| i).collect();
    let cut = |keep: usize| -> String {
        let end = bounds.get(keep).copied().unwrap_or(text.len());
        let mut s = String::with_capacity(end + 3);
        s.push_str(text.get(..end).unwrap_or(""));
        s.push_str("...");
        s
    };
    // Bisect on the number of characters kept: `lo` always fits (or is zero).
    let (mut lo, mut hi) = (0usize, bounds.len());
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        if width(&cut(mid)) <= max_width {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    loop {
        let candidate = cut(lo);
        if width(&candidate) <= max_width {
            return Cow::Owned(candidate);
        }
        if lo == 0 {
            return Cow::Owned(String::new());
        }
        lo -= 1;
    }
}

/// Owns glyphon's font/atlas/renderer state plus a reusable buffer pool, and the
/// per-frame queue of runs and panels. One instance per [`super::Renderer`], and
/// one per secondary surface.
pub struct TextLayer {
    /// The measurer, whose font system is also the one every run is shaped
    /// with — so a measured width is the drawn width.
    measure: TextMeasure,
    swash_cache: SwashCache,
    viewport: Viewport,
    atlas: TextAtlas,
    renderer: TextRenderer,
    /// Runs queued for the current frame (cleared each frame at `end_frame`).
    runs: Vec<OwnedRun>,
    /// Panels queued for the current frame, drawn under every run.
    panels: Vec<Panel>,
    panel_pass: PanelPass,
    /// One reusable cosmic-text buffer per run, grown on demand and reshaped in
    /// place each frame — no per-frame `Buffer` allocation in steady state.
    buffers: Vec<Buffer>,
    /// Whether the last `prepare` produced text for `render`.
    ready: bool,
    /// Whether the last `prepare` produced panels for `render`.
    panels_ready: bool,
}

impl TextLayer {
    /// Build the text layer on `device`, targeting `format` (the surface format).
    /// Loads the system font set once here, not per frame.
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let measure = TextMeasure::new();
        let swash_cache = SwashCache::new();
        let cache = Cache::new(device);
        let viewport = Viewport::new(device, &cache);
        let mut atlas = TextAtlas::new(device, queue, &cache, format);
        let renderer =
            TextRenderer::new(&mut atlas, device, wgpu::MultisampleState::default(), None);
        Self {
            measure,
            swash_cache,
            viewport,
            atlas,
            renderer,
            runs: Vec::new(),
            panels: Vec::new(),
            panel_pass: PanelPass::new(device, format),
            buffers: Vec::new(),
            ready: false,
            panels_ready: false,
        }
    }

    /// The laid-out width of `text` at `size` device pixels, as this layer
    /// would draw it.
    pub fn measure(&mut self, text: &str, size: f32) -> f32 {
        self.measure.width(text, size)
    }

    /// Replace this frame's queued panels with `panels`.
    pub fn queue_panels(&mut self, panels: &[Panel]) {
        self.panels.clear();
        self.panels.extend_from_slice(panels);
    }

    /// Append one panel to this frame's queue — the core's own furniture, for
    /// the reason [`push`](Self::push) exists.
    pub fn push_panel(&mut self, panel: Panel) {
        self.panels.push(panel);
    }

    /// Replace this frame's queued runs with owning copies of `runs`.
    pub fn queue(&mut self, runs: &[TextRun<'_>]) {
        self.runs.clear();
        self.runs.extend(runs.iter().map(|r| OwnedRun {
            text: r.text.to_owned(),
            x: r.x,
            y: r.y,
            size: r.size,
            color: r.color,
        }));
    }

    /// Append one run to this frame's queue, leaving what is already there
    /// alone. The core's own furniture — the now-playing banner (ADR-0110) —
    /// goes in through this rather than [`queue`](Self::queue), which replaces:
    /// a frontend that queues nothing this frame must not erase it.
    pub fn push(&mut self, run: TextRun<'_>) {
        self.runs.push(OwnedRun {
            text: run.text.to_owned(),
            x: run.x,
            y: run.y,
            size: run.size,
            color: run.color,
        });
    }

    /// Write the queued panels, shape the queued runs and upload their glyphs to
    /// the atlas. Returns whether there is anything to draw; an atlas-full or
    /// shaping failure degrades to "no text drawn" rather than panicking on the
    /// render path.
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
    ) -> bool {
        self.ready = false;
        self.panels_ready = self.panel_pass.prepare(queue, &self.panels, width, height);
        if self.runs.is_empty() {
            return self.panels_ready;
        }

        // Split-borrow the fields so the buffer pool and the font system can be
        // mutated disjointly (glyphon's shaping needs both).
        let Self {
            measure,
            swash_cache,
            viewport,
            atlas,
            renderer,
            runs,
            buffers,
            ready,
            panels_ready,
            ..
        } = self;
        let font_system = &mut measure.font_system;

        // Grow the reusable pool to cover this frame's run count.
        while buffers.len() < runs.len() {
            buffers.push(Buffer::new(
                font_system,
                Metrics::new(16.0, 16.0 * LINE_HEIGHT_RATIO),
            ));
        }

        // Reshape one buffer per run with its size and text (single line).
        for (buf, run) in buffers.iter_mut().zip(runs.iter()) {
            buf.set_metrics(Metrics::new(run.size, run.size * LINE_HEIGHT_RATIO));
            buf.set_size(None, None); // no wrap; TextArea bounds clip to screen
            buf.set_text(
                run.text.as_str(),
                &Attrs::new().family(Family::SansSerif),
                Shaping::Advanced,
                None,
            );
            buf.shape_until_scroll(font_system, false);
        }

        viewport.update(
            queue,
            Resolution {
                width: width.max(1),
                height: height.max(1),
            },
        );

        let clip_w = width.max(1) as i32;
        let clip_h = height.max(1) as i32;
        let areas = buffers.iter().zip(runs.iter()).map(|(buf, run)| TextArea {
            buffer: buf,
            left: run.x,
            top: run.y,
            scale: 1.0,
            bounds: TextBounds {
                left: 0,
                top: 0,
                right: clip_w,
                bottom: clip_h,
            },
            default_color: color_of(run.color),
            custom_glyphs: &[],
        });

        if renderer
            .prepare(
                device,
                queue,
                font_system,
                atlas,
                viewport,
                areas,
                swash_cache,
            )
            .is_err()
        {
            // Atlas full / shaping error — skip text this frame, keep the panels.
            return *panels_ready;
        }
        *ready = true;
        true
    }

    /// Draw the prepared panels into `pass`, as one draw. Separate from
    /// [`render`](Self::render) so a caller can draw between the two — the
    /// shell's picture sits on its panel and under its caption. Call this first.
    pub fn render_panels(&self, pass: &mut wgpu::RenderPass<'_>) {
        if self.panels_ready {
            self.panel_pass.render(pass);
        }
    }

    /// Whether the last [`prepare`](Self::prepare) left panels to draw.
    pub fn has_panels(&self) -> bool {
        self.panels_ready
    }

    /// Draw the prepared runs into `pass` (a load pass over the scene). No-op if
    /// `prepare` produced no text. Panels are [`render_panels`](Self::render_panels)'.
    pub fn render<'pass>(&'pass self, pass: &mut wgpu::RenderPass<'pass>) {
        if !self.ready {
            return;
        }
        // Best-effort: a render error can't recover mid-frame, so drop it rather
        // than panic on the hot path (the text simply won't appear).
        let _ = self.renderer.render(&self.atlas, &self.viewport, pass);
    }

    /// End-of-frame housekeeping: free atlas space unused this frame and clear
    /// the queue for the next one.
    pub fn end_frame(&mut self) {
        self.atlas.trim();
        self.runs.clear();
        self.panels.clear();
        self.panel_pass.end_frame();
        self.ready = false;
        self.panels_ready = false;
    }
}

/// Map a linear `[r, g, b, a]` in `0.0..=1.0` to glyphon's 8-bit color.
fn color_of([r, g, b, a]: [f32; 4]) -> Color {
    let to_u8 = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
    Color::rgba(to_u8(r), to_u8(g), to_u8(b), to_u8(a))
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )]

    use super::*;

    /// A deterministic spread of strings: ASCII names, wide and multi-byte
    /// characters, spaces, and the empty string.
    fn corpus() -> Vec<String> {
        let alphabet: Vec<char> = "aBcW iIl.MmQ-éßЖ漢字".chars().collect();
        let mut seed = 0x9e37_79b9_u32;
        let mut out = vec![
            String::new(),
            "Star Mandala Bordered".to_owned(),
            "Iris Bloom Kaleidoscope".to_owned(),
        ];
        for len in 1..40 {
            let mut s = String::new();
            for _ in 0..len {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                let i = (seed >> 16) as usize % alphabet.len();
                s.push(alphabet[i]);
            }
            out.push(s);
        }
        out
    }

    fn assert_holds(measure: &mut dyn FnMut(&str) -> f32, label: &str) {
        for text in corpus() {
            let full = measure(&text);
            for max_width in [0.0, 1.0, 12.0, 40.0, 97.5, 180.0, 333.0, 1000.0] {
                let fitted = fit_width(&text, max_width, &mut *measure);
                let w = measure(&fitted);
                assert!(
                    w <= max_width,
                    "{label}: {text:?} fitted to {max_width} measures {w}: {fitted:?}"
                );
                if full <= max_width {
                    assert!(
                        matches!(fitted, Cow::Borrowed(s) if s == text),
                        "{label}: {text:?} fits {max_width} and must come back unchanged"
                    );
                } else {
                    assert!(
                        fitted.is_empty() || fitted.ends_with("..."),
                        "{label}: a cut string must say so: {fitted:?}"
                    );
                }
            }
        }
    }

    /// **The truncation property, held on the machine's own font.** Whatever the
    /// system sans-serif is, the measured width of a fitted string never exceeds
    /// the width asked for, and a string that fits is untouched. A property of
    /// the call, not of a font, so it holds wherever this runs — a machine with
    /// no fonts at all measures everything as zero, and still holds it.
    #[test]
    fn a_fitted_string_never_measures_wider_than_asked_on_the_system_font() {
        let mut m = TextMeasure::new();
        assert_holds(&mut |s| m.width(s, 22.0), "system font");
    }

    /// The same property against a measurer that is not monotone in prefix
    /// length — a wide glyph that shrinks when followed by another, the way a
    /// kerning pair can — which the bisection alone would get wrong.
    #[test]
    fn a_fitted_string_holds_the_property_under_a_non_monotone_measure() {
        let mut odd = |s: &str| {
            let n = s.chars().count() as f32;
            let penalty = if s.chars().count() % 2 == 1 { 9.0 } else { 0.0 };
            n * 7.0 + penalty
        };
        assert_holds(&mut odd, "non-monotone");
    }

    #[test]
    fn a_long_name_keeps_its_longest_fitting_prefix() {
        // Ten px per character: 100 px holds seven characters and the ellipsis.
        let fitted = fit_width("abcdefghijklmnop", 100.0, |s| {
            s.chars().count() as f32 * 10.0
        });
        assert_eq!(fitted, "abcdefg...");
    }

    #[test]
    fn the_measurer_caches_rather_than_reshaping() {
        let mut m = TextMeasure::new();
        let a = m.width("Spectrum Corona", 22.0);
        assert_eq!(m.width("Spectrum Corona", 22.0), a);
        assert_eq!(m.cache.len(), 1);
        assert_eq!(m.width("", 22.0), 0.0);
        assert_eq!(m.width("x", f32::NAN), 0.0);
    }
}
