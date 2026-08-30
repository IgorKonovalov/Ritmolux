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
use super::text::{TextLayer, TextRun};

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
    /// A non-blocking mode was offered and taken: the console's present cannot
    /// block on its own display's vblank, so it cannot pace the output.
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
}

impl AuxTarget {
    /// Attach a secondary surface for `target` to `ctx`'s device.
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
        // One in-flight image: the console is a monitor, so a deep queue only
        // buys it latency behind the output it is reporting on.
        config.desired_maximum_frame_latency = 1;
        surface.configure(&ctx.device, &config);

        let text = TextLayer::new(&ctx.device, &ctx.queue, config.format);
        Ok(Self {
            surface,
            config,
            text,
            mode,
        })
    }

    /// The present mode this surface was configured with.
    pub fn present_mode(&self) -> AuxPresentMode {
        self.mode
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
    /// cannot alter what the show displays.
    pub fn present(
        &mut self,
        ctx: &RenderContext,
        runs: &[TextRun<'_>],
    ) -> Result<(), RenderError> {
        use wgpu::CurrentSurfaceTexture as C;
        let frame = match self.surface.get_current_texture() {
            C::Success(frame) | C::Suboptimal(frame) => frame,
            // Transient: the window is resizing, occluded or hidden. Skipping
            // this console frame is correct, and the output is unaffected —
            // which is the whole reason the console presents on its own encoder.
            C::Timeout | C::Occluded => return Ok(()),
            // Reconfigure and skip. Unlike the output path this does not retry
            // in the same frame: a console frame is worth nothing and the next
            // one is 16 ms away, so the retry would only add a stall the show
            // could feel.
            C::Outdated | C::Lost => {
                self.surface.configure(&ctx.device, &self.config);
                return Ok(());
            }
            C::Validation => return Err(RenderError::SurfaceValidation),
        };

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("lmv-console-frame"),
            });

        self.text.queue(runs);
        let (width, height) = (self.config.width, self.config.height);
        let drew = self.text.prepare(&ctx.device, &ctx.queue, width, height);

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("lmv-console-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(CLEAR),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            if drew {
                self.text.render(&mut pass);
            }
        }

        ctx.queue.submit(std::iter::once(encoder.finish()));
        ctx.queue.present(frame);
        self.text.end_frame();
        Ok(())
    }
}
