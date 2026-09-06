//! The secondary present target: a second surface on the renderer's *existing*
//! device (ADR-0143).
//!
//! The core learns nothing about what the second window means. It is handed a
//! window handle and a list of [`TextRun`]s, and it presents them. Every
//! question about *which* rows, *what* they say and *when* they change stays in
//! the shell, where the modal state machines already live.
//!
//! Behind the `text` feature, because a secondary target that carries no text
//! and no picture has no consumer: the only frontend that opens one is the
//! standalone, which enables the feature. The plugin's `cdylib`, the default
//! `cargo build` and the core test suite compile this module out entirely,
//! exactly as they do the text layer it is built on.

// Hot-path panic-denial pragma (Plan 0002 Phase 2; `render/` scan set). Runs
// once per displayed frame while the target is attached; a panic here crashes
// the app the operator is driving.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use super::RenderError;
use super::context::RenderContext;
use super::gpu;
use super::preview::{PreviewTarget, preview_rect};
use super::text::{TextLayer, TextRun};

/// The program preview's blit: one positioned quad sampling the intermediate.
///
/// A quad and not a fullscreen triangle, because the preview is letterboxed
/// into a corner of the console rather than filling it — the rectangle arrives
/// as a uniform in NDC and the vertex shader interpolates the corners across it.
///
/// **Only the console samples the intermediate.** The show's own copy out of it
/// is a `copy_texture_to_texture` with no shader in the path, which is what
/// keeps the output exact; this side is a monitor and a resample is what it is
/// for.
struct Blit {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    rect: wgpu::Buffer,
    /// The bound intermediate's identity and the group built against it. Rebuilt
    /// only when the renderer hands over a different intermediate — a resize or
    /// a close/reopen — so the per-frame path creates no GPU resource.
    bound: Option<(u64, wgpu::BindGroup)>,
}

/// The blit's shader. `rect` is `(x0, y0, x1, y1)` in NDC, with `y0` the top
/// edge; `uv` runs `0..1` across the quad, which is already the texture's
/// top-left-origin convention, so no flip is applied anywhere.
const BLIT_WGSL: &str = r#"
struct Rect { ndc: vec4<f32> };
@group(0) @binding(0) var<uniform> rect: Rect;
@group(0) @binding(1) var src: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0), vec2<f32>(0.0, 1.0),
    );
    let c = corners[vi];
    var out: VsOut;
    out.pos = vec4<f32>(
        mix(rect.ndc.x, rect.ndc.z, c.x),
        mix(rect.ndc.y, rect.ndc.w, c.y),
        0.0,
        1.0,
    );
    out.uv = c;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return vec4<f32>(textureSample(src, samp, in.uv).rgb, 1.0);
}
"#;

impl Blit {
    fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("rlx-console-blit-layout"),
            entries: &[
                gpu::uniform(0, wgpu::ShaderStages::VERTEX),
                gpu::texture(1, true),
                gpu::sampler(2),
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("rlx-console-blit"),
            source: wgpu::ShaderSource::Wgsl(BLIT_WGSL.into()),
        });
        let pipeline = gpu::fullscreen_pipeline(
            device,
            &shader,
            &[&layout],
            format,
            wgpu::BlendState::REPLACE,
            "rlx-console-blit",
        );
        Self {
            pipeline,
            layout,
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("rlx-console-blit-sampler"),
                // Linear: the preview is a heavy minification of the show and a
                // nearest sample of it aliases into unreadable noise. Exactness
                // is the output copy's job, not this one's.
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
            rect: gpu::uniform_buffer(
                device,
                "rlx-console-blit-rect",
                std::mem::size_of::<[f32; 4]>(),
            ),
            bound: None,
        }
    }

    /// The bind group for `preview`, rebuilt only when the intermediate's
    /// identity has changed.
    ///
    /// `Option` rather than an infallible reference so the caller skips the
    /// preview on the one path that cannot produce a group; this file denies
    /// panics, and a console frame is worth nothing next to the show.
    fn bind(&mut self, device: &wgpu::Device, preview: &PreviewTarget) -> Option<&wgpu::BindGroup> {
        let generation = preview.generation();
        if self.bound.as_ref().is_none_or(|(g, _)| *g != generation) {
            let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("rlx-console-blit-group"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.rect.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&preview.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            });
            self.bound = Some((generation, group));
        }
        self.bound.as_ref().map(|(_, group)| group)
    }
}

/// The background the secondary surface clears to before its text is
/// composited. Near-black rather than black so an operator can tell a live
/// console from a dead one across a dim room, and dark enough that it throws no
/// usable light onto a stage.
const CLEAR: wgpu::Color = wgpu::Color {
    r: 0.02,
    g: 0.02,
    b: 0.025,
    a: 1.0,
};

/// The present mode a secondary surface ended up with, so the shell can record
/// which arm ran (ADR-0071 reporting: a frame-time measurement that does not
/// name its present mode cannot be compared with another machine's).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuxPresentMode {
    /// A non-blocking mode was offered and taken: the console's present does
    /// not block on its own display's vblank.
    ///
    /// That is a property of this surface's present, **not** a guarantee about
    /// the output's cadence — the two presents still run on one thread, and
    /// what the second costs the first is a measurement rather than a
    /// deduction. Measured at Plan 0147 Phase 4 on an integrated Radeon: at the
    /// 165 Hz vsync cap, 14,797 console presents cost the output 0.0 fps.
    NonBlocking(&'static str),
    /// Only `Fifo` was offered. The console presents in lockstep with its own
    /// display, which is the configuration where a slower second monitor can be
    /// felt on the output.
    Fifo,
}

impl AuxPresentMode {
    /// The mode's name, for the diagnostic log line.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NonBlocking(name) => name,
            Self::Fifo => "Fifo",
        }
    }
}

/// What [`AuxTarget::present`] did with the calls it was given, since attach.
///
/// The witness a cost measurement of this surface needs (ADR-0172). Every arm
/// of the present path that returns without reaching `queue.present` returns
/// the same `Ok(())` a successful present does, so a surface that was occluded
/// for a whole run and one that presented every frame produce the same log and
/// the same frame rate. `presented` is what separates them; a measurement
/// reading zero cost against a zero present count has measured nothing.
///
/// `presented + skipped` is the number of calls the target received, which is
/// what lets a caller reconcile its own totals: whatever it decimated, plus
/// these two, is the frames it ran with this target attached.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AuxCounts {
    /// Calls that reached this surface's own `queue.present`.
    pub presented: u64,
    /// Calls that returned without presenting — the surface had no texture to
    /// give, or refused validation.
    pub skipped: u64,
}

/// A second swapchain plus its own text layer.
///
/// Its own layer, not the renderer's: glyphon's atlas and viewport are built
/// against one surface format and one resolution, and the console's differ from
/// the output's. Sharing one would make the console's size the output's, which
/// is the bug ADR-0037 describes in its other clothes.
pub struct AuxTarget {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    text: TextLayer,
    mode: AuxPresentMode,
    blit: Blit,
    counts: AuxCounts,
}

/// The range a secondary surface's `desired_maximum_frame_latency` is held to.
///
/// A depth of 0 configures no images and is rejected by the backend; past 3 the
/// queue is deeper than any presentation engine here will run ahead, so the
/// extra images cost memory and buy latency. The caller's value is clamped into
/// this range at the boundary rather than validated and refused: it is a pacing
/// hint, and a surface that will not attach is a worse answer than one that
/// attaches at the nearest depth.
const AUX_FRAME_LATENCY: std::ops::RangeInclusive<u32> = 1..=3;

impl AuxTarget {
    /// Attach a secondary surface for `target` to `ctx`'s device.
    ///
    /// `frame_latency` is the swapchain's `desired_maximum_frame_latency`,
    /// clamped to [`AUX_FRAME_LATENCY`]. It is a **pacing** control and not a
    /// picture one: at 1 the surface holds a single in-flight image, so
    /// `get_current_texture` waits for this surface's own previous present to
    /// retire before it returns — one vblank, spent on whichever thread calls
    /// it. A caller presenting this surface from the same thread as another one
    /// pays that wait inside that thread's frame.
    ///
    /// Fails — rather than panicking or degrading silently — when the surface
    /// cannot be configured on the adapter this device was created on. That is
    /// the dual-GPU path: a window on a monitor driven by the *other* GPU may
    /// present no format this adapter can write. The caller degrades; the core
    /// only reports.
    pub fn new(
        ctx: &RenderContext,
        target: impl Into<wgpu::SurfaceTarget<'static>>,
        width: u32,
        height: u32,
        frame_latency: u32,
    ) -> Result<Self, RenderError> {
        let surface = ctx
            .instance
            .create_surface(target)
            .map_err(RenderError::CreateSurface)?;

        let mut config = surface
            .get_default_config(&ctx.gpu, width.max(1), height.max(1))
            .ok_or(RenderError::UnsupportedSurface)?;

        // A non-blocking mode where the surface offers one. The console must not
        // become a second pacing source for the output: under `Fifo` on a slower
        // display, `get_current_texture` blocks on *that* display's vblank, and
        // the show's frame loop waits behind it. Mailbox first (tear-free),
        // Immediate second, `Fifo` only when neither is offered — and the caps
        // query is what decides, not an assumption about the backend.
        let caps = surface.get_capabilities(&ctx.gpu);
        let mode = if caps.present_modes.contains(&wgpu::PresentMode::Mailbox) {
            config.present_mode = wgpu::PresentMode::Mailbox;
            AuxPresentMode::NonBlocking("Mailbox")
        } else if caps.present_modes.contains(&wgpu::PresentMode::Immediate) {
            config.present_mode = wgpu::PresentMode::Immediate;
            AuxPresentMode::NonBlocking("Immediate")
        } else {
            config.present_mode = wgpu::PresentMode::Fifo;
            AuxPresentMode::Fifo
        };
        config.desired_maximum_frame_latency =
            frame_latency.clamp(*AUX_FRAME_LATENCY.start(), *AUX_FRAME_LATENCY.end());
        surface.configure(&ctx.device, &config);

        let text = TextLayer::new(&ctx.device, &ctx.queue, config.format);
        let blit = Blit::new(&ctx.device, config.format);
        Ok(Self {
            surface,
            config,
            text,
            mode,
            blit,
            counts: AuxCounts::default(),
        })
    }

    /// The present mode this surface was configured with.
    pub fn present_mode(&self) -> AuxPresentMode {
        self.mode
    }

    /// What the present path has done since attach. Reset with the target: the
    /// counts describe one open session, not the process.
    pub fn counts(&self) -> AuxCounts {
        self.counts
    }

    /// The frame latency this surface was configured with, **after clamping** —
    /// so a caller reporting which arm ran quotes the depth the swapchain got
    /// rather than the one it asked for.
    pub fn frame_latency(&self) -> u32 {
        self.config.desired_maximum_frame_latency
    }

    /// The surface's current size in physical pixels.
    pub fn size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }

    /// Reconfigure for a new size. A zero dimension is ignored — the window is
    /// minimized and the old config stays valid for when it returns.
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(device, &self.config);
    }

    /// Draw `runs` onto the secondary surface and present it.
    ///
    /// Wholly independent of the output's frame: its own encoder, its own
    /// submit, its own present. Nothing here touches the primary swapchain, the
    /// scene clock or the dissolve, so a console that stalls or drops a frame
    /// cannot alter the **pixels** the show puts on screen — which the golden
    /// suite asserts byte-exactly.
    ///
    /// **It says nothing about when.** This runs on the display thread, so its
    /// cost is inside the caller's frame whatever this surface's present mode
    /// is; the separation above is of *state*, not of *time*. What that costs
    /// is measured rather than argued — Plan 0147 Phase 4, five arms in three
    /// frame-time regimes on an integrated Radeon, found it inside noise, with
    /// [`AuxCounts`] beside each arm to prove the presents happened.
    ///
    /// **Every exit counts itself** into [`AuxCounts`]: the four surface states
    /// that skip return the same `Ok(())` a present does, so without the
    /// counter a caller cannot tell a console that ran from one that never
    /// acquired a texture. The validation arm counts as a skip too — it is the
    /// only exit that returns `Err`, and leaving it uncounted would break the
    /// caller's reconciliation by one frame on exactly the frame the console
    /// dies.
    pub fn present(
        &mut self,
        ctx: &RenderContext,
        runs: &[TextRun<'_>],
        preview: Option<&PreviewTarget>,
    ) -> Result<(), RenderError> {
        use wgpu::CurrentSurfaceTexture as C;
        let frame = match self.surface.get_current_texture() {
            C::Success(frame) | C::Suboptimal(frame) => frame,
            // Transient: the window is resizing, occluded or hidden. Skipping
            // this console frame is correct, and the output is unaffected —
            // which is the whole reason the console presents on its own encoder.
            C::Timeout | C::Occluded => {
                self.counts.skipped = self.counts.skipped.saturating_add(1);
                return Ok(());
            }
            // Reconfigure and skip. Unlike the output path this does not retry
            // in the same frame: a console frame is worth nothing and the next
            // one is 16 ms away, so the retry would only add a stall the show
            // could feel.
            C::Outdated | C::Lost => {
                self.counts.skipped = self.counts.skipped.saturating_add(1);
                self.surface.configure(&ctx.device, &self.config);
                return Ok(());
            }
            C::Validation => {
                self.counts.skipped = self.counts.skipped.saturating_add(1);
                return Err(RenderError::SurfaceValidation);
            }
        };

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("rlx-console-frame"),
            });

        self.text.queue(runs);
        let (width, height) = (self.config.width, self.config.height);
        let drew = self.text.prepare(&ctx.device, &ctx.queue, width, height);

        // The preview's rectangle, in this surface's NDC. Its aspect comes from
        // the intermediate — which is the *output* render target's size — so the
        // console window's own shape never reaches the picture (ADR-0037).
        // `None` here means no preview is open, or this console is too small to
        // show one; either way the pass below just clears and draws text.
        let quad = preview.and_then(|p| {
            let rect = preview_rect(p.size(), (width, height))?;
            let (w, h) = (width as f32, height as f32);
            let ndc = [
                rect.x / w * 2.0 - 1.0,
                1.0 - rect.y / h * 2.0,
                (rect.x + rect.width) / w * 2.0 - 1.0,
                1.0 - (rect.y + rect.height) / h * 2.0,
            ];
            ctx.queue
                .write_buffer(&self.blit.rect, 0, bytemuck::cast_slice(&ndc));
            self.blit.bind(&ctx.device, p).is_some().then_some(())
        });
        // Re-borrowed immutably below rather than held across the pass: `bind`
        // takes `&mut self.blit` to refresh its cache, and the pass needs the
        // pipeline from the same field.
        let quad = quad.and(self.blit.bound.as_ref().map(|(_, group)| group));

        {
            let mut pass = gpu::color_pass(
                &mut encoder,
                "rlx-console-pass",
                &view,
                wgpu::LoadOp::Clear(CLEAR),
            );
            // The preview first, the text over it: the modal list is what the
            // operator is reading and the monitor must not cover it.
            if let Some(group) = quad {
                pass.set_pipeline(&self.blit.pipeline);
                pass.set_bind_group(0, group, &[]);
                pass.draw(0..6, 0..1);
            }
            if drew {
                self.text.render(&mut pass);
            }
        }

        ctx.queue.submit(std::iter::once(encoder.finish()));
        ctx.queue.present(frame);
        self.counts.presented = self.counts.presented.saturating_add(1);
        self.text.end_frame();
        Ok(())
    }
}
