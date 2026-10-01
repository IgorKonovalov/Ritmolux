//! Panels: the translucent backdrops the interface's text sits on, so a list
//! stays readable over a bright scene.
//!
//! A panel is a rounded rectangle in the theme's `panel` fill, with a 1 px lit
//! edge in `panel_edge` and, optionally, a faint scanline modulation every
//! `scanline_pitch`th row — every value from [`THEME`](super::theme::THEME), so a
//! panel carries no colour of its own. The same pass draws the row highlight
//! ([`PanelKind::Highlight`]), which is why the selection can glide under text
//! without a second pipeline.
//!
//! **All of a frame's panels are one instanced draw**, recorded by the text
//! layer before its glyphs in the same pass. The rounded corners, the edge and
//! the scanlines are computed per fragment from the panel's rectangle, so there
//! is no texture and no bind group.

// Hot-path panic-denial pragma (`render/` scan set): the pass is prepared every
// frame a panel is queued.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

/// One panel, in device pixels from the target's top-left — the coordinate
/// space a [`TextRun`](super::TextRun) uses.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Panel {
    /// Left edge.
    pub x: f32,
    /// Top edge.
    pub y: f32,
    /// Width.
    pub w: f32,
    /// Height.
    pub h: f32,
    /// Opacity multiplier over the theme's own, `0.0..=1.0`: an envelope fading
    /// the panel with the text it holds.
    pub alpha: f32,
    /// Whether the scanline modulation is drawn. A highlight never carries one.
    pub scanlines: bool,
    /// What the rectangle is.
    pub kind: PanelKind,
}

/// The two things a panel rectangle can be.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PanelKind {
    /// A backdrop: the theme's `panel` fill, a 1 px `panel_edge`, rounded
    /// corners, and the scanline when asked for.
    #[default]
    Backdrop,
    /// The selection marker on a row: the theme's `highlight` fill with a 4 px
    /// `accent` bar at its left edge, square-cornered, no edge.
    Highlight,
}

/// Width of a highlight's accent bar, device pixels.
pub const HIGHLIGHT_BAR_W: f32 = 4.0;

impl Panel {
    /// A fully opaque (as the theme declares it) backdrop with scanlines.
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self {
            x,
            y,
            w,
            h,
            alpha: 1.0,
            scanlines: true,
            kind: PanelKind::Backdrop,
        }
    }

    /// A row highlight at `(x, y)`, `w` x `h`.
    pub fn highlight(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self {
            scanlines: false,
            kind: PanelKind::Highlight,
            ..Self::new(x, y, w, h)
        }
    }

    /// The panel that holds a block of text spanning `x0..x1` horizontally and
    /// `y0..y1` vertically, with `pad` pixels around it on every side.
    pub fn around(x0: f32, y0: f32, x1: f32, y1: f32, pad: f32) -> Self {
        Self::new(
            x0 - pad,
            y0 - pad,
            (x1 - x0) + 2.0 * pad,
            (y1 - y0) + 2.0 * pad,
        )
    }

    /// The same panel with every coordinate multiplied by `s` — for a surface
    /// that scales its text by the same factor.
    pub fn scaled(self, s: f32) -> Self {
        Self {
            x: self.x * s,
            y: self.y * s,
            w: self.w * s,
            h: self.h * s,
            ..self
        }
    }
}

#[cfg(feature = "text")]
pub(crate) use pass::PanelPass;

#[cfg(feature = "text")]
mod pass {
    use super::{HIGHLIGHT_BAR_W, Panel, PanelKind};
    use crate::render::theme::THEME;

    /// Panels one frame draws; more are dropped. The interface draws a handful.
    const MAX_PANELS: usize = 32;

    /// One panel as the shader reads it.
    #[repr(C)]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    struct Instance {
        /// `(x, y, w, h)` in device pixels.
        rect: [f32; 4],
        /// The fill, linear, with the panel's alpha applied.
        fill: [f32; 4],
        /// The lit edge, linear, with the panel's alpha applied.
        edge: [f32; 4],
        /// The accent bar at the left edge, linear, with the panel's alpha
        /// applied; transparent on a backdrop.
        bar: [f32; 4],
        /// `(radius px, scanline alpha, scanline pitch px, target height px)`.
        params: [f32; 4],
        /// `(target width px, bar width px, unused, unused)`.
        extra: [f32; 4],
    }

    /// The rectangle becomes NDC in the vertex stage, from the target size in
    /// `params.w` and `extra.x`; the fragment stage works in framebuffer pixels
    /// (`@builtin(position)`, top-left origin, pixel centres at `.5`), where the
    /// rounded-box distance is exact.
    const SHADER: &str = r#"
struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) rect: vec4<f32>,
    @location(1) fill: vec4<f32>,
    @location(2) edge: vec4<f32>,
    @location(3) bar: vec4<f32>,
    @location(4) params: vec4<f32>,
    @location(5) extra: vec4<f32>,
};

@vertex
fn vs_main(
    @builtin(vertex_index) vi: u32,
    @location(0) rect: vec4<f32>,
    @location(1) fill: vec4<f32>,
    @location(2) edge: vec4<f32>,
    @location(3) bar: vec4<f32>,
    @location(4) params: vec4<f32>,
    @location(5) extra: vec4<f32>,
) -> VsOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
    );
    let px = rect.xy + corners[vi] * rect.zw;
    let size = vec2<f32>(extra.x, params.w);
    var out: VsOut;
    out.pos = vec4<f32>(px.x / size.x * 2.0 - 1.0, 1.0 - px.y / size.y * 2.0, 0.0, 1.0);
    out.rect = rect;
    out.fill = fill;
    out.edge = edge;
    out.bar = bar;
    out.params = params;
    out.extra = extra;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = in.pos.xy;
    let half = in.rect.zw * 0.5;
    let r = min(in.params.x, min(half.x, half.y));
    let q = abs(p - (in.rect.xy + half)) - half + vec2<f32>(r, r);
    // Signed distance to the rounded box: negative inside.
    let d = length(max(q, vec2<f32>(0.0, 0.0))) + min(max(q.x, q.y), 0.0) - r;
    let coverage = clamp(0.5 - d, 0.0, 1.0);
    if (coverage <= 0.0) {
        discard;
    }

    var fill = in.fill;
    // Every pitch-th row from the panel's top carries the scanline.
    let pitch = max(in.params.z, 1.0);
    let row = floor(p.y - in.rect.y);
    if (in.params.y > 0.0 && row - pitch * floor(row / pitch) < 0.5) {
        fill = vec4<f32>(fill.rgb * (1.0 - in.params.y), fill.a + in.params.y * (1.0 - fill.a));
    }

    // The 1 px lit edge: full for `d > -0.5`, gone by `d = -1.5`, so a one-pixel
    // band inside the outline, antialiased on its inner side (its outer side is
    // `coverage`'s).
    var edge_amt = clamp(d + 1.5, 0.0, 1.0) * in.edge.a;
    var edge_rgb = in.edge.rgb;
    // The accent bar replaces the edge along the left `extra.y` pixels.
    if (p.x - in.rect.x < in.extra.y) {
        edge_amt = in.bar.a;
        edge_rgb = in.bar.rgb;
    }
    let rgb = edge_rgb * edge_amt + fill.rgb * fill.a * (1.0 - edge_amt);
    let a = edge_amt + fill.a * (1.0 - edge_amt);
    return vec4<f32>(rgb, a) * coverage;
}
"#;

    /// The panels' pipeline and instance buffer. Built with the text layer that
    /// records it, against the same target format.
    pub(crate) struct PanelPass {
        pipeline: wgpu::RenderPipeline,
        instances: wgpu::Buffer,
        scratch: Vec<Instance>,
        /// Panels written by the last [`prepare`](Self::prepare).
        count: u32,
    }

    impl PanelPass {
        pub(crate) fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("rlx-panel-shader"),
                source: wgpu::ShaderSource::Wgsl(SHADER.into()),
            });
            let instances = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("rlx-panel-instances"),
                size: (MAX_PANELS * std::mem::size_of::<Instance>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("rlx-panel-pipeline-layout"),
                bind_group_layouts: &[],
                immediate_size: 0,
            });
            let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("rlx-panel-pipeline"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<Instance>() as u64,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &wgpu::vertex_attr_array![
                            0 => Float32x4,
                            1 => Float32x4,
                            2 => Float32x4,
                            3 => Float32x4,
                            4 => Float32x4,
                            5 => Float32x4,
                        ],
                    })],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        // The fragment stage outputs premultiplied colour.
                        blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
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
                scratch: Vec::with_capacity(MAX_PANELS),
                count: 0,
            }
        }

        /// Write `panels` for a `width`x`height` target. Returns whether
        /// [`render`](Self::render) will draw. A panel with no area, or with a
        /// coordinate that is not finite, is skipped.
        pub(crate) fn prepare(
            &mut self,
            queue: &wgpu::Queue,
            panels: &[Panel],
            width: u32,
            height: u32,
        ) -> bool {
            self.scratch.clear();
            // Resolved per prepare: the sRGB decode is not a `const fn`.
            let backdrop = (THEME.panel.linear(), THEME.panel_edge.linear());
            let highlight = THEME.highlight.linear();
            let accent = THEME.accent.linear();
            let scan = THEME.scanline.a;
            let with_alpha = |[r, g, b, a]: [f32; 4], k: f32| [r, g, b, a * k];
            for panel in panels.iter().take(MAX_PANELS) {
                let finite = [panel.x, panel.y, panel.w, panel.h, panel.alpha]
                    .iter()
                    .all(|v| v.is_finite());
                if !finite || panel.w <= 0.0 || panel.h <= 0.0 || panel.alpha <= 0.0 {
                    continue;
                }
                let alpha = panel.alpha.min(1.0);
                let (fill, edge, bar, radius, bar_w) = match panel.kind {
                    PanelKind::Backdrop => (backdrop.0, backdrop.1, [0.0; 4], THEME.radius, 0.0),
                    PanelKind::Highlight => (highlight, [0.0; 4], accent, 0.0, HIGHLIGHT_BAR_W),
                };
                let scanlines = panel.scanlines && panel.kind == PanelKind::Backdrop;
                self.scratch.push(Instance {
                    rect: [panel.x, panel.y, panel.w, panel.h],
                    fill: with_alpha(fill, alpha),
                    edge: with_alpha(edge, alpha),
                    bar: with_alpha(bar, alpha),
                    params: [
                        radius,
                        if scanlines { scan * alpha } else { 0.0 },
                        THEME.scanline_pitch as f32,
                        height.max(1) as f32,
                    ],
                    extra: [width.max(1) as f32, bar_w, 0.0, 0.0],
                });
            }
            self.count = self.scratch.len() as u32;
            if self.count == 0 {
                return false;
            }
            queue.write_buffer(&self.instances, 0, bytemuck::cast_slice(&self.scratch));
            true
        }

        /// Draw the prepared panels into `pass`, as one instanced draw.
        pub(crate) fn render(&self, pass: &mut wgpu::RenderPass<'_>) {
            if self.count == 0 {
                return;
            }
            pass.set_pipeline(&self.pipeline);
            pass.set_vertex_buffer(0, self.instances.slice(..));
            pass.draw(0..6, 0..self.count);
        }

        /// Forget the last prepare, so a frame with no panels draws none.
        pub(crate) fn end_frame(&mut self) {
            self.count = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_panel_around_a_block_pads_every_side() {
        let p = Panel::around(10.0, 20.0, 110.0, 70.0, 8.0);
        assert_eq!((p.x, p.y, p.w, p.h), (2.0, 12.0, 116.0, 66.0));
        assert_eq!(p.alpha, 1.0);
    }

    #[test]
    fn scaling_a_panel_scales_its_rectangle_and_nothing_else() {
        let p = Panel {
            alpha: 0.5,
            scanlines: false,
            ..Panel::new(10.0, 20.0, 30.0, 40.0)
        }
        .scaled(0.5);
        assert_eq!((p.x, p.y, p.w, p.h), (5.0, 10.0, 15.0, 20.0));
        assert_eq!(p.alpha, 0.5);
        assert!(!p.scanlines);
    }
}
