//! The diagnostics debug overlay (Plan 0011): a final compositing pass that,
//! when enabled, paints a translucent panel over the scene with a frame-time
//! sparkline, a GPU-footprint bar, and a numeric fps / frame-ms / MB readout.
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

use crate::diag::Metrics;

use super::overlay_font::{GLYPH_H, GLYPH_W, glyph};
use super::tier::Tier;

/// Instance buffer capacity in quads. Comfortably covers the panel, ~240
/// sparkline bars, the bars, and every lit digit pixel of the readout.
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
/// GPU bytes that fill the footprint bar (512 MiB).
const GPU_BAR_MAX_BYTES: f32 = 512.0 * 1024.0 * 1024.0;

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
    /// resolved is diagnostics, not trivia.
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
        tier: Tier,
        frame_ms_samples: impl Iterator<Item = f32>,
    ) {
        let (width, height) = size;
        let vp = Vp {
            w: width.max(1) as f32,
            h: height.max(1) as f32,
        };
        self.samples.clear();
        self.samples.extend(frame_ms_samples);
        self.build(vp, metrics, tier);

        let n = self.quads.len().min(MAX_QUADS);
        let Some(slice) = self.quads.get(..n) else {
            return;
        };
        if slice.is_empty() {
            return;
        }
        queue.write_buffer(&self.instances, 0, bytemuck::cast_slice(slice));

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("overlay-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    // Load: composite over the scene already in the surface.
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_vertex_buffer(0, self.instances.slice(..));
        pass.draw(0..6, 0..n as u32);
    }

    /// Rebuild the quad list for this frame from the metrics + samples.
    ///
    /// Splitting the readout out of this method is what lets the panel's one line
    /// of prose be tested without a GPU — see [`write_readout`].
    fn build(&mut self, vp: Vp, metrics: Metrics, tier: Tier) {
        self.quads.clear();

        // Build the readout first so the panel sizes to whichever is wider — the
        // text row or the graph — and everything shares one content width.
        write_readout(&mut self.text, metrics, tier);
        // Text width, excluding the last glyph's trailing gap.
        let text_w = (self.text.chars().count() as f32 * CHAR_ADVANCE - FONT_PX).max(0.0);
        let content_w = text_w.max(SPARK_W);

        let content_x = MARGIN + PAD;
        let text_y = MARGIN + PAD;
        let spark_y = text_y + TEXT_H + PAD;
        let bar_y = spark_y + SPARK_H + PAD;
        let panel_w = content_w + PAD * 2.0;
        let panel_h = (bar_y + BAR_H + PAD) - MARGIN;
        push_rect(
            &mut self.quads,
            vp,
            MARGIN,
            MARGIN,
            panel_w,
            panel_h,
            PANEL_COLOR,
        );

        draw_text(&mut self.quads, vp, content_x, text_y, &self.text);

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
    }
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
fn write_readout(out: &mut String, metrics: Metrics, tier: Tier) {
    out.clear();
    let _ = write!(
        out,
        "{:.0} FPS  {:.1} MS  {:.0} MB  {}",
        metrics.fps,
        metrics.frame_ms_p99,
        metrics.gpu_bytes as f32 / (1024.0 * 1024.0),
        tier.label(),
    );
}

/// Emit the lit font pixels of `text` starting at device-pixel (`x`, `y`).
fn draw_text(out: &mut Vec<Quad>, vp: Vp, x: f32, y: f32, text: &str) {
    for (ci, c) in text.chars().enumerate() {
        let gx = x + ci as f32 * CHAR_ADVANCE;
        for (row, bits) in glyph(c).iter().enumerate() {
            for col in 0..GLYPH_W {
                // Bit (GLYPH_W-1 - col) is column `col` from the left.
                if (bits >> (GLYPH_W - 1 - col)) & 1 == 1 {
                    let px = gx + col as f32 * FONT_PX;
                    let py = y + row as f32 * FONT_PX;
                    push_rect(out, vp, px, py, FONT_PX, FONT_PX, TEXT_COLOR);
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

    use super::{Tier, write_readout};
    use crate::diag::Metrics;
    use crate::render::overlay_font::{GLYPH_H, glyph};

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
        let metrics = Metrics {
            fps: 60.0,
            frame_ms_p99: 16.7,
            gpu_bytes: 340 * 1024 * 1024,
            ..Metrics::default()
        };
        let mut text = String::new();
        for tier in [Tier::Floor, Tier::Rich] {
            write_readout(&mut text, metrics, tier);
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

    /// A space is legitimately blank, so the check above would pass vacuously if
    /// the tier label were ever spaces — and it would also pass if `glyph` had
    /// stopped returning blanks for unknown characters, which is what makes the
    /// missing-glyph assertion a real check. Pin both.
    #[test]
    fn an_unknown_character_is_blank_and_a_known_one_is_not() {
        assert_eq!(glyph('\u{0}').len(), GLYPH_H);
        assert_eq!(glyph('~'), [0x00; GLYPH_H], "unknown must render blank");
        assert_ne!(glyph('F'), [0x00; GLYPH_H], "a covered glyph must be lit");
        for tier in [Tier::Floor, Tier::Rich] {
            assert!(!tier.label().trim().is_empty());
        }
    }
}
