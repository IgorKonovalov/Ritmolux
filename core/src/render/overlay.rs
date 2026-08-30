//! The diagnostics debug overlay (Plan 0011): a final compositing pass that,
//! when enabled, paints a translucent panel over the scene with a frame-time
//! sparkline, a GPU-footprint bar, a numeric fps / frame-ms / MB readout, and
//! the analysis block (Plan 0049): the four normalized levels and the downbeat
//! estimator's lock state.
//!
//! **The analysis levels are meters, not just numbers, and that is the point.**
//! Plan 0048 Phase 6 asks whether the levels "ride the music without pumping or
//! going numb" — a judgement about how a value *moves* against music you are
//! hearing, made at a glance while it plays. Four digits re-rendered sixty times
//! a second do not answer that; four bars do. The numbers stay beside them for
//! the moments when a magnitude is what you want.
//!
//! Everything is drawn as solid-color quads through one instanced pipeline —
//! the same instanced-quad pattern the scenes use — so there is no new
//! dependency and no texture: even the digits are quads, one per lit font pixel
//! (see `overlay_font`). The pass loads (does not clear) the scene, so
//! it truly composites on top; when the overlay flag is off the renderer skips
//! this pass entirely (no transparent draw), so a live show pays nothing.

// Hot-path panic-denial pragma (Plan 0002 Phase 2; `render/` scan set). Runs
// every displayed frame while the overlay is enabled.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use std::fmt::Write as _;

use crate::diag::{AnalysisMetrics, Metrics};

use super::overlay_font::{GLYPH_H, GLYPH_W, glyph};
use super::tier::Tier;
use crate::render::gpu;

/// Instance buffer capacity in quads. Comfortably covers the panel, ~240
/// sparkline bars, the bars, and every lit font pixel of the readout — the
/// frame-time line plus the five analysis rows come to roughly 1500 between
/// them.
const MAX_QUADS: usize = 4096;

// Layout, in device pixels from the top-left corner.
const MARGIN: f32 = 12.0;
const PAD: f32 = 8.0;
const FONT_PX: f32 = 2.0; // device pixels per font pixel
const CHAR_ADVANCE: f32 = (GLYPH_W as f32 + 1.0) * FONT_PX;
const TEXT_H: f32 = GLYPH_H as f32 * FONT_PX;
const SPARK_W: f32 = 240.0; // minimum graph width; grows to fit the readout
const SPARK_H: f32 = 72.0; // tall enough to read the frame-time trace + spikes
const BAR_H: f32 = 12.0;

/// Frame time (ms) that fills the sparkline to the top — two 60 fps frames.
const SPARK_MAX_MS: f32 = 33.3;
/// A comfortable 60 fps budget; frames under this read green.
const BUDGET_MS: f32 = 16.7;
/// Suffix on a tier the governor demoted, rather than one that was asked for.
const DEMOTED_MARK: &str = "*";
/// GPU bytes that fill the footprint bar (512 MiB).
const GPU_BAR_MAX_BYTES: f32 = 512.0 * 1024.0 * 1024.0;

/// Vertical pitch between the stacked analysis rows — tighter than [`PAD`], so
/// the five read as one block instead of five separate things.
const ROW_GAP: f32 = 5.0;
/// Height of one analysis meter.
const METER_H: f32 = 9.0;
/// Characters reserved for a row's `LABEL value` column, ahead of its meter.
/// One wider than the longest of them (`ONSET 0.18`), so every meter starts on
/// the same x — the four levels read as one stack — with a character of gap
/// rather than the bar butting against the last digit.
const ROW_TEXT_CHARS: f32 = 11.0;

/// The analysis rows, in draw order, each with the label the panel prints. The
/// labels are the **only** place these words appear, so the readout-alphabet test
/// sweeps this table rather than a copy of the strings.
const LEVEL_LABELS: [&str; 4] = ["BASS", "MID", "TREB", "ONSET"];
/// What the lock row says when the downbeat estimator's confidence cleared its
/// gate, and when it did not (ADR-0050). **Two words, not a colour** — the state
/// has to survive a screenshot and a colour-blind reader, and this is the one
/// value Plan 0048 Phase 6 records rather than watches.
const LOCKED_LABEL: &str = "LOCK";
const FREE_LABEL: &str = "FREE";

type Rgba = [f32; 4];

/// Viewport size in device pixels, threaded through the layout helpers so a
/// pixel rect can be converted to NDC.
#[derive(Clone, Copy)]
struct Vp {
    w: f32,
    h: f32,
}

const PANEL_COLOR: Rgba = [0.02, 0.02, 0.03, 0.66];
const TEXT_COLOR: Rgba = [0.90, 0.95, 1.00, 1.0];
const SPARK_GOOD: Rgba = [0.30, 0.90, 0.45, 1.0];
const SPARK_WARN: Rgba = [0.95, 0.75, 0.20, 1.0];
const SPARK_BAD: Rgba = [0.95, 0.32, 0.32, 1.0];
const BAR_BG_COLOR: Rgba = [0.14, 0.14, 0.18, 0.85];
const BAR_FILL_COLOR: Rgba = [0.35, 0.60, 1.00, 1.0];
// Dim reference line drawn across the sparkline at the 60 fps budget, so the
// trace reads against a known mark instead of floating.
const BUDGET_LINE_COLOR: Rgba = [0.55, 0.55, 0.62, 0.5];
// The three band levels share a fill; `onset` gets its own because it is a
// different kind of quantity — an event envelope, not a standing level — and
// reading them as one stack of four identical bars invites comparing them.
const LEVEL_FILL_COLOR: Rgba = [0.35, 0.80, 0.95, 1.0];
const ONSET_FILL_COLOR: Rgba = [0.95, 0.70, 0.30, 1.0];
// The lock row, reinforcing its word rather than replacing it.
const LOCKED_COLOR: Rgba = [0.35, 0.95, 0.55, 1.0];
const FREE_COLOR: Rgba = [0.62, 0.62, 0.70, 1.0];

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Quad {
    /// NDC minimum corner (x right, y up).
    min: [f32; 2],
    /// NDC size (both positive).
    size: [f32; 2],
    color: Rgba,
}

const SHADER: &str = r#"
struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn vs_main(
    @builtin(vertex_index) vi: u32,
    @location(0) min: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(2) color: vec4<f32>,
) -> VsOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
    );
    let p = min + corners[vi] * size;
    var out: VsOut;
    out.pos = vec4<f32>(p, 0.0, 1.0);
    out.color = color;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return in.color;
}
"#;

/// The overlay's instanced-quad pipeline plus reusable CPU scratch (rebuilt each
/// frame with no steady-state allocation).
pub struct Overlay {
    pipeline: wgpu::RenderPipeline,
    instances: wgpu::Buffer,
    quads: Vec<Quad>,
    samples: Vec<f32>,
    text: String,
}

impl Overlay {
    /// Build the overlay pipeline and buffers on `device`.
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("overlay-shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let instances = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("overlay-instances"),
            size: (MAX_QUADS * std::mem::size_of::<Quad>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("overlay-pipeline-layout"),
            bind_group_layouts: &[],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("overlay-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Quad>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x2,
                        1 => Float32x2,
                        2 => Float32x4,
                    ],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    // Alpha OVER so the translucent panel shows the scene through it.
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        Self {
            pipeline,
            instances,
            quads: Vec::with_capacity(MAX_QUADS),
            samples: Vec::with_capacity(256),
            text: String::with_capacity(48),
        }
    }

    /// Composite the overlay over `view`. `frame_ms_samples` is the rolling
    /// frame-time history (oldest first, milliseconds) for the sparkline, and
    /// `tier` is the active quality tier, named in the readout (ADR-0045) — the
    /// same preset looks different on different machines now, so which tier a run
    /// resolved is diagnostics, not trivia. `demoted` marks a tier the frame-time
    /// governor took back rather than one that was asked for.
    #[allow(
        clippy::too_many_arguments,
        reason = "the frame's overlay inputs, each read once; bundling them would name a struct after this call site"
    )]
    pub fn render(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        size: (u32, u32),
        metrics: Metrics,
        analysis: AnalysisMetrics,
        tier: Tier,
        demoted: bool,
        frame_ms_samples: impl Iterator<Item = f32>,
    ) {
        let (width, height) = size;
        let vp = Vp {
            w: width.max(1) as f32,
            h: height.max(1) as f32,
        };
        self.samples.clear();
        self.samples.extend(frame_ms_samples);
        self.build(vp, metrics, analysis, tier, demoted);

        let n = self.quads.len().min(MAX_QUADS);
        let Some(slice) = self.quads.get(..n) else {
            return;
        };
        if slice.is_empty() {
            return;
        }
        queue.write_buffer(&self.instances, 0, bytemuck::cast_slice(slice));

        // Load: composite over the scene already in the surface.
        let mut pass = gpu::color_pass(encoder, "overlay-pass", view, wgpu::LoadOp::Load);
        pass.set_pipeline(&self.pipeline);
        pass.set_vertex_buffer(0, self.instances.slice(..));
        pass.draw(0..6, 0..n as u32);
    }

    /// Rebuild the quad list for this frame from the metrics + samples.
    ///
    /// Splitting the readout out of this method is what lets the panel's one line
    /// of prose be tested without a GPU — see [`write_readout`].
    fn build(
        &mut self,
        vp: Vp,
        metrics: Metrics,
        analysis: AnalysisMetrics,
        tier: Tier,
        demoted: bool,
    ) {
        self.quads.clear();

        // Build the readout first so the panel sizes to whichever is wider — the
        // text row or the graph — and everything shares one content width.
        write_readout(&mut self.text, metrics, tier, demoted);
        // Text width, excluding the last glyph's trailing gap.
        let text_w = (self.text.chars().count() as f32 * CHAR_ADVANCE - FONT_PX).max(0.0);
        let content_w = text_w.max(SPARK_W);

        let content_x = MARGIN + PAD;
        let text_y = MARGIN + PAD;
        let spark_y = text_y + TEXT_H + PAD;
        let bar_y = spark_y + SPARK_H + PAD;
        // The analysis block sits below the renderer's own figures: read the
        // frame-time group as one thing, the audio group as another.
        let analysis_y = bar_y + BAR_H + PAD;
        let row_pitch = TEXT_H.max(METER_H) + ROW_GAP;
        // Five rows: the four levels, then the lock state.
        let panel_w = content_w + PAD * 2.0;
        let panel_h = (analysis_y + row_pitch * 5.0 - ROW_GAP + PAD) - MARGIN;
        push_rect(
            &mut self.quads,
            vp,
            MARGIN,
            MARGIN,
            panel_w,
            panel_h,
            PANEL_COLOR,
        );

        draw_text(
            &mut self.quads,
            vp,
            content_x,
            text_y,
            &self.text,
            TEXT_COLOR,
        );

        // Frame-time sparkline: one vertical bar per retained sample, newest at
        // the right, colored by how close each frame ran to the 60 fps budget.
        let count = self.samples.len();
        if count > 0 {
            let step = content_w / count as f32;
            let bw = step.max(1.0);
            for (i, &ms) in self.samples.iter().enumerate() {
                let frac = (ms / SPARK_MAX_MS).clamp(0.0, 1.0);
                let h = (frac * SPARK_H).max(1.0);
                let x = content_x + i as f32 * step;
                let color = if ms <= BUDGET_MS * 1.1 {
                    SPARK_GOOD
                } else if ms <= SPARK_MAX_MS {
                    SPARK_WARN
                } else {
                    SPARK_BAD
                };
                // Bars grow up from the baseline (bottom of the sparkline band).
                push_rect(&mut self.quads, vp, x, spark_y + SPARK_H - h, bw, h, color);
            }
        }
        // Budget reference line across the band at the 60 fps mark, so the trace
        // reads against a known threshold instead of floating.
        let budget_h = (BUDGET_MS / SPARK_MAX_MS).clamp(0.0, 1.0) * SPARK_H;
        push_rect(
            &mut self.quads,
            vp,
            content_x,
            spark_y + SPARK_H - budget_h,
            content_w,
            1.0,
            BUDGET_LINE_COLOR,
        );

        // GPU-footprint bar: dark track with a colored fill.
        push_rect(
            &mut self.quads,
            vp,
            content_x,
            bar_y,
            content_w,
            BAR_H,
            BAR_BG_COLOR,
        );
        let fill = (metrics.gpu_bytes as f32 / GPU_BAR_MAX_BYTES).clamp(0.0, 1.0);
        if fill > 0.0 {
            push_rect(
                &mut self.quads,
                vp,
                content_x,
                bar_y,
                content_w * fill,
                BAR_H,
                BAR_FILL_COLOR,
            );
        }

        // --- the analysis block (Plan 0049 / ADR-0052) ---
        let meter_x = content_x + ROW_TEXT_CHARS * CHAR_ADVANCE;
        let meter_w = (content_x + content_w - meter_x).max(0.0);
        let levels = [
            (analysis.bass, LEVEL_FILL_COLOR),
            (analysis.mid, LEVEL_FILL_COLOR),
            (analysis.treb, LEVEL_FILL_COLOR),
            (analysis.onset, ONSET_FILL_COLOR),
        ];
        for (row, (&label, (value, fill_color))) in LEVEL_LABELS.iter().zip(levels).enumerate() {
            let y = analysis_y + row as f32 * row_pitch;
            write_value_row(&mut self.text, label, value);
            draw_text(&mut self.quads, vp, content_x, y, &self.text, TEXT_COLOR);
            draw_meter(&mut self.quads, vp, meter_x, y, meter_w, value, fill_color);
        }

        // The lock row. Its word carries the state and its meter carries the
        // confidence, so a screenshot says which one it was without a legend.
        let lock_y = analysis_y + 4.0 * row_pitch;
        let (label, color) = if analysis.downbeat_locked {
            (LOCKED_LABEL, LOCKED_COLOR)
        } else {
            (FREE_LABEL, FREE_COLOR)
        };
        write_value_row(&mut self.text, label, analysis.downbeat_confidence);
        draw_text(&mut self.quads, vp, content_x, lock_y, &self.text, color);
        draw_meter(
            &mut self.quads,
            vp,
            meter_x,
            lock_y,
            meter_w,
            analysis.downbeat_confidence,
            color,
        );
    }
}

/// One analysis meter: a dark track with a fill proportional to `value` in 0..1.
fn draw_meter(out: &mut Vec<Quad>, vp: Vp, x: f32, y: f32, w: f32, value: f32, fill: Rgba) {
    // Centred against the text row so the label and its bar sit on one line.
    let y = y + (TEXT_H - METER_H) * 0.5;
    push_rect(out, vp, x, y, w, METER_H, BAR_BG_COLOR);
    let frac = if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    };
    push_rect(out, vp, x, y, w * frac, METER_H, fill);
}

/// Push one axis-aligned rectangle, given in top-left device-pixel coordinates,
/// as an NDC quad. Off-screen or degenerate rects are dropped.
fn push_rect(out: &mut Vec<Quad>, vp: Vp, x: f32, y: f32, w: f32, h: f32, color: Rgba) {
    if w <= 0.0 || h <= 0.0 || out.len() >= MAX_QUADS {
        return;
    }
    // Pixel space (y down) -> NDC (y up).
    let x0 = x / vp.w * 2.0 - 1.0;
    let x1 = (x + w) / vp.w * 2.0 - 1.0;
    let y_top = 1.0 - y / vp.h * 2.0;
    let y_bot = 1.0 - (y + h) / vp.h * 2.0;
    out.push(Quad {
        min: [x0, y_bot],
        size: [x1 - x0, y_top - y_bot],
        color,
    });
}

/// Write the panel's single readout line into `out`, replacing its contents.
///
/// Unit labels and the tier name are uppercase because that is what is legible at
/// 5x7 — and, more to the point, because the [`glyph`] table only *has* uppercase.
/// A character with no glyph renders as a blank cell rather than failing, so the
/// only thing standing between "the overlay names the tier" and a silent gap in
/// the panel is the test below.
///
/// Takes the buffer by `&mut` rather than returning a `String`: the overlay reuses
/// one allocation across frames, and this runs on every frame the panel is up.
fn write_readout(out: &mut String, metrics: Metrics, tier: Tier, demoted: bool) {
    out.clear();
    let _ = write!(
        out,
        "{:.0} FPS  {:.1} MS  {:.0} MB  {}{}",
        metrics.fps,
        metrics.frame_ms_p99,
        metrics.gpu_bytes as f32 / (1024.0 * 1024.0),
        tier.label(),
        // A demoted floor and a pinned floor are the same tier and very different
        // facts, so the marker is what keeps the demotion from being silent
        // (ADR-0045). One glyph, because the panel is already the width of its
        // sparkline and this is the only place with room.
        if demoted { DEMOTED_MARK } else { "" },
    );
}

/// Write one analysis row — `LABEL value` — into `out`, replacing its contents.
///
/// The value is left-padded to a fixed width so the four level rows line up as a
/// column, which is most of what makes them readable as a stack.
///
/// **The value is clamped and non-finite is printed as zero.** This is a readout,
/// not a validator: `{:.2}` of a `NaN` is the string `NaN`, whose lowercase `a`
/// has no glyph and would paint a blank cell (see `overlay_font`), and a level
/// far outside 0..1 would run its number into the meter. Neither should be
/// possible from the analyzer — that is why this clamps rather than reports.
fn write_value_row(out: &mut String, label: &str, value: f32) {
    out.clear();
    let v = if value.is_finite() {
        value.clamp(0.0, 9.99)
    } else {
        0.0
    };
    // Label padded to the widest of them so every meter starts on one x.
    let _ = write!(out, "{label:<5} {v:.2}");
}

/// Emit the lit font pixels of `text` starting at device-pixel (`x`, `y`).
fn draw_text(out: &mut Vec<Quad>, vp: Vp, x: f32, y: f32, text: &str, color: Rgba) {
    for (ci, c) in text.chars().enumerate() {
        let gx = x + ci as f32 * CHAR_ADVANCE;
        for (row, bits) in glyph(c).iter().enumerate() {
            for col in 0..GLYPH_W {
                // Bit (GLYPH_W-1 - col) is column `col` from the left.
                if (bits >> (GLYPH_W - 1 - col)) & 1 == 1 {
                    let px = gx + col as f32 * FONT_PX;
                    let py = y + row as f32 * FONT_PX;
                    push_rect(out, vp, px, py, FONT_PX, FONT_PX, color);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    //! The readout line, GPU-free. The panel geometry needs a device; the prose
    //! does not, and the prose is what Plan 0044's done-when is about.

    // Test asserts panic on failure; allowed here over the file's pragma.
    #![allow(clippy::panic)]

    use super::{
        DEMOTED_MARK, FREE_LABEL, LEVEL_LABELS, LOCKED_LABEL, Tier, write_readout, write_value_row,
    };
    use crate::diag::Metrics;
    use crate::render::overlay_font::{GLYPH_H, glyph};

    fn metrics() -> Metrics {
        Metrics {
            fps: 60.0,
            frame_ms_p99: 16.7,
            gpu_bytes: 340 * 1024 * 1024,
            ..Metrics::default()
        }
    }

    /// The overlay **names the active tier**, and every character it names it with
    /// actually has a glyph.
    ///
    /// The second half is the load-bearing one. [`glyph`](super::glyph) returns a
    /// blank cell for an unknown character instead of failing, so adding `FLOOR`
    /// to the readout without adding `L`, `O` and `R` to the 5x7 table would paint
    /// a confident `F` followed by four empty cells — a "named" tier that reads as
    /// nothing on screen. Nothing else in the engine would notice.
    #[test]
    fn the_readout_names_the_tier_in_glyphs_the_font_actually_has() {
        let metrics = metrics();
        let mut text = String::new();
        for tier in [Tier::Floor, Tier::Rich] {
            write_readout(&mut text, metrics, tier, false);
            assert!(
                text.contains(tier.label()),
                "the readout does not name the {} tier: {text:?}",
                tier.as_str()
            );
            // The numbers stay where they were — the tier is appended, not swapped in.
            assert!(text.starts_with("60 FPS  16.7 MS  340 MB  "), "{text:?}");

            for c in tier.label().chars() {
                assert_ne!(
                    glyph(c),
                    [0x00; GLYPH_H],
                    "`{c}` of `{}` has no glyph, so the tier paints as a blank gap",
                    tier.label()
                );
            }
        }
    }

    /// **A demoted floor reads differently from a pinned floor.** They are the
    /// same tier and very different facts — one is what the operator asked for,
    /// the other is the engine telling them their machine could not hold the rich
    /// budget — so if these two strings were equal the demotion would be silent,
    /// which is exactly what ADR-0045 rules out.
    #[test]
    fn a_demoted_tier_is_marked_and_a_pinned_one_is_not() {
        let (mut pinned, mut demoted) = (String::new(), String::new());
        write_readout(&mut pinned, metrics(), Tier::Floor, false);
        write_readout(&mut demoted, metrics(), Tier::Floor, true);
        assert_ne!(pinned, demoted);
        assert!(demoted.ends_with(DEMOTED_MARK), "{demoted:?}");
        assert!(!pinned.ends_with(DEMOTED_MARK), "{pinned:?}");
        // The mark is a suffix, not a replacement: the tier is still named.
        assert!(demoted.contains(Tier::Floor.label()));
    }

    /// **Every character the readout can emit has a glyph.**
    ///
    /// This is the guard the whole analysis block rests on. `glyph` returns a
    /// blank cell for an uncovered character rather than failing, so `BASS` in a
    /// font without `A` paints `B SS` and nothing in the engine notices — no
    /// error, no warning, no failing test. Plan 0044 hit the same trap with the
    /// tier names.
    ///
    /// So this sweeps the readout's **alphabet**, not a fixed expected string: it
    /// drives every writer the panel has over a range of inputs chosen to reach
    /// every digit, both lock words, all four level labels, both tiers and the
    /// demotion mark, and asserts each emitted non-space character is lit. A
    /// changed format string stays covered; a new label does not sneak past.
    #[test]
    fn every_character_the_readout_can_emit_has_a_glyph() {
        let mut text = String::new();
        let mut seen = std::collections::BTreeSet::new();
        let mut sweep = |text: &String| {
            for c in text.chars() {
                seen.insert(c);
                if c == ' ' {
                    continue;
                }
                assert_ne!(
                    glyph(c),
                    [0x00; GLYPH_H],
                    "`{c}` has no glyph, so the readout `{text}` paints a blank cell there"
                );
            }
        };

        // The frame-time line, over values that between them print every digit,
        // both tiers, and the demotion mark.
        for (fps, p99, bytes) in [
            (60.0, 16.7, 340 * 1024 * 1024),
            (23.0, 45.9, 178 * 1024 * 1024),
            (0.0, 0.0, 0),
        ] {
            for tier in [Tier::Floor, Tier::Rich] {
                for demoted in [false, true] {
                    write_readout(
                        &mut text,
                        Metrics {
                            fps,
                            frame_ms_p99: p99,
                            gpu_bytes: bytes,
                            ..Metrics::default()
                        },
                        tier,
                        demoted,
                    );
                    sweep(&text);
                }
            }
        }

        // Every analysis row: all four level labels and both lock words, over
        // values that reach every digit — plus the ones that must never reach the
        // panel as text at all (non-finite, out of range, negative).
        let labels: Vec<&str> = LEVEL_LABELS
            .iter()
            .copied()
            .chain([LOCKED_LABEL, FREE_LABEL])
            .collect();
        for label in labels {
            for value in [
                0.0,
                0.123,
                0.456,
                0.789,
                1.0,
                -0.5,
                42.0,
                f32::NAN,
                f32::INFINITY,
                f32::NEG_INFINITY,
            ] {
                write_value_row(&mut text, label, value);
                sweep(&text);
            }
        }

        // And the sweep must actually have covered letters — a writer that
        // silently produced empty strings would satisfy every assertion above.
        assert!(
            seen.iter().filter(|c| c.is_ascii_uppercase()).count() >= 12,
            "the sweep saw too few letters to be exercising the labels: {seen:?}"
        );
        assert!(
            ('0'..='9').all(|d| seen.contains(&d)),
            "the sweep never printed some digit: {seen:?}"
        );
    }

    /// The lock state survives without colour. `LOCK` and `FREE` are the same
    /// width and differ in every character, so a screenshot — or a colour-blind
    /// reader — sees the estimator's gate rather than inferring it from a hue.
    /// This is the value Plan 0048 Phase 6 records a **rate** from, and ADR-0050's
    /// stopping condition is unfalsifiable without it.
    #[test]
    fn the_lock_row_states_the_gate_in_words() {
        let (mut locked, mut free) = (String::new(), String::new());
        write_value_row(&mut locked, LOCKED_LABEL, 0.83);
        write_value_row(&mut free, FREE_LABEL, 0.21);
        assert_ne!(locked, free);
        assert!(locked.starts_with(LOCKED_LABEL), "{locked:?}");
        assert!(free.starts_with(FREE_LABEL), "{free:?}");
        // The confidence rides along, so the row says how close a free frame was.
        assert!(locked.ends_with("0.83"), "{locked:?}");
        assert!(free.ends_with("0.21"), "{free:?}");
        // Same width, so the two states do not shift the column under them.
        assert_eq!(locked.chars().count(), free.chars().count());
    }

    /// The four level rows are a column: same label field width, so their values
    /// and meters line up. A ragged stack is the difference between reading four
    /// bars at a glance and parsing four lines.
    #[test]
    fn the_level_rows_line_up_as_a_column() {
        let mut text = String::new();
        let mut widths = std::collections::BTreeSet::new();
        for label in LEVEL_LABELS {
            write_value_row(&mut text, label, 0.5);
            widths.insert(text.chars().count());
            assert!(text.starts_with(label), "{text:?}");
        }
        assert_eq!(widths.len(), 1, "rows are ragged: {widths:?}");
    }

    /// A space is legitimately blank, so the check above would pass vacuously if
    /// the tier label were ever spaces — and it would also pass if `glyph` had
    /// stopped returning blanks for unknown characters, which is what makes the
    /// missing-glyph assertion a real check. Pin both.
    #[test]
    fn an_unknown_character_is_blank_and_a_known_one_is_not() {
        assert_eq!(glyph('\u{0}').len(), GLYPH_H);
        assert_eq!(glyph('~'), [0x00; GLYPH_H], "unknown must render blank");
        assert_ne!(glyph('F'), [0x00; GLYPH_H], "a covered glyph must be lit");
        for c in DEMOTED_MARK.chars() {
            assert_ne!(glyph(c), [0x00; GLYPH_H], "the demotion mark must be lit");
        }
        for tier in [Tier::Floor, Tier::Rich] {
            assert!(!tier.label().trim().is_empty());
        }
    }
}
