//! The program preview's intermediate target and its letterbox geometry
//! (ADR-0143).
//!
//! While a secondary surface is attached, the frame is drawn **once** into the
//! intermediate here and reaches its real destination by
//! `copy_texture_to_texture` — exact, no shader, no sampling. The same
//! intermediate is then sampled, scaled and letterboxed onto the console. One
//! render, one frame, two destinations.
//!
//! **Not behind the `text` feature, unlike [`super::aux_target`].** The console
//! that consumes a preview is text-gated, but the intermediate is a render path
//! and the property that matters about it — that a frame routed through it is
//! byte-identical to one drawn straight at the target — is asserted on the
//! headless capture path, which compiles with glyphon out.

// Hot-path panic-denial pragma (Plan 0002 Phase 2; `render/` scan set). The
// copy runs once per displayed frame while a preview is open.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use super::gpu;
use std::sync::atomic::{AtomicU64, Ordering};

/// Hands out an identity for each intermediate ever built, so a consumer that
/// caches GPU state against one can tell it has been handed a different
/// texture. A resize destroys and rebuilds the intermediate at the same size in
/// principle, and a pointer comparison on `wgpu::Texture` is not available.
static NEXT_GENERATION: AtomicU64 = AtomicU64::new(1);

/// The offscreen a frame is drawn into while a preview is open, plus the view
/// and identity its two consumers need.
///
/// Sized to the output's configured target and **never resized in place**: the
/// copy's extent is fixed against `width`x`height`, so a renderer resize
/// discards this and builds another. [`super::Renderer::open_preview`] and
/// `resize` are the only things that construct one.
pub struct PreviewTarget {
    /// `RENDER_ATTACHMENT | COPY_SRC | TEXTURE_BINDING` — drawn into, copied
    /// out of, and sampled by the console blit.
    pub(crate) texture: wgpu::Texture,
    /// A view of `texture`, held rather than recreated per frame.
    pub(crate) view: wgpu::TextureView,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
    generation: u64,
}

impl PreviewTarget {
    /// Build the intermediate at `format` — which must be the destination's
    /// format, since `copy_texture_to_texture` refuses a mismatch and that
    /// refusal is the whole guarantee of exactness.
    pub(crate) fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let (width, height) = (width.max(1), height.max(1));
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("rlx-preview-intermediate"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            texture,
            view,
            width,
            height,
            format,
            generation: NEXT_GENERATION.fetch_add(1, Ordering::Relaxed),
        }
    }

    /// The pixel size this intermediate was built against.
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// The texture format this intermediate was built at — the destination's,
    /// since the copy out of it refuses a mismatch.
    pub(crate) fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    /// This intermediate's identity, unique across every one ever built in this
    /// process. A consumer caching a bind group against it compares this rather
    /// than the texture.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Record the exact copy from this intermediate into `dst`.
    ///
    /// Both must carry the same format and the same size; the caller builds
    /// them that way and wgpu rejects the pair if it did not.
    pub(crate) fn record_copy_to(&self, encoder: &mut wgpu::CommandEncoder, dst: &wgpu::Texture) {
        encoder.copy_texture_to_texture(
            self.texture.as_image_copy(),
            dst.as_image_copy(),
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );
    }
}

/// The **fixed-size** mirror the preview readback copies out of (ADR-0187).
///
/// [`PreviewTarget`] above is the show's own size and is rebuilt whenever the
/// window is, so a readback taken straight off it changes size mid-run. The
/// reader on the other end of a byte pipe cannot survive that: the frames carry
/// no header, a raw frame can hold any byte pattern so no sentinel finds the
/// boundary, and bytes already in the OS pipe are still the old geometry. This
/// texture is built once, at the size the caller asked for, and a **sampling
/// blit** fills it from the intermediate every frame — so the window may be
/// resized, maximized or thrown fullscreen and the bytes leaving here keep one
/// shape for the life of the run.
///
/// Built at the intermediate's own format, so the blit preserves the channel
/// order rather than converting it — whoever announces the pipe can then name
/// what it actually carries.
///
/// The show's aspect is **letterboxed** into that fixed shape rather than
/// stretched to it (see [`fit_rect`]).
pub(crate) struct PreviewTap {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    width: u32,
    height: u32,
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    /// The bind group for the intermediate, keyed on that intermediate's
    /// [`generation`](PreviewTarget::generation): a renderer resize builds
    /// another intermediate, and a group still bound to the old one samples a
    /// texture nothing owns.
    bound: Option<(u64, wgpu::BindGroup)>,
}

/// The blit's fragment stage. Alpha is forced to 1: the pipe's consumer reads
/// four channels per pixel, and an intermediate carrying anything but opaque
/// there would paint the preview translucent over whatever is behind it.
const TAP_WGSL: &str = r#"
@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return vec4<f32>(textureSample(src, samp, in.uv).rgb, 1.0);
}
"#;

impl PreviewTap {
    /// Build the tap at `format` — the intermediate's — and the requested size.
    pub(crate) fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let (width, height) = (width.max(1), height.max(1));
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("rlx-preview-tap"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        // `[Texture, Sampler]` — a shape no other layout in `core/src` holds,
        // which `no_two_layouts_share_a_shape_without_recorded_evidence`
        // enforces (ADR-0058).
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("rlx-preview-tap-layout"),
            entries: &[gpu::texture(0, true), gpu::sampler(1)],
        });
        let shader = gpu::fullscreen_shader(
            device,
            "rlx-preview-tap",
            gpu::FULLSCREEN_VS_UV_FLIPPED,
            TAP_WGSL,
        );
        let pipeline = gpu::fullscreen_pipeline(
            device,
            &shader,
            &[&layout],
            format,
            wgpu::BlendState::REPLACE,
            "rlx-preview-tap",
        );
        Self {
            texture,
            view,
            width,
            height,
            pipeline,
            layout,
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("rlx-preview-tap-sampler"),
                // Linear: the tap is a heavy minification of the show in every
                // ordinary case, and a nearest sample of it aliases into noise
                // a viewer reads as detail that is not in the picture.
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
            bound: None,
        }
    }

    /// The pixel size this was built at, and the size of every frame copied out
    /// of it for as long as it lives.
    pub(crate) fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// The texture a readback copies out of.
    pub(crate) fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }

    /// Record the scaling, letterboxing blit from `preview` into this target.
    ///
    /// Returns whether it recorded. A caller copies out of this texture only
    /// when it did, so a frame with no bind group is skipped rather than
    /// published as whatever the tap happened to hold before.
    pub(crate) fn record_fill_from(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        preview: &PreviewTarget,
    ) -> bool {
        let generation = preview.generation();
        if self.bound.as_ref().is_none_or(|(g, _)| *g != generation) {
            self.bound = Some((
                generation,
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("rlx-preview-tap-group"),
                    layout: &self.layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&preview.view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&self.sampler),
                        },
                    ],
                }),
            ));
        }
        let Some((_, group)) = self.bound.as_ref() else {
            return false;
        };
        let (x, y, width, height) = fit_rect(preview.size(), (self.width, self.height));
        // Cleared rather than loaded: the letterbox bars are the part of the
        // target the blit never writes, and a loaded target would keep showing
        // the previous frame's picture in them.
        let mut pass = gpu::color_pass(
            encoder,
            "rlx-preview-tap-blit",
            &self.view,
            wgpu::LoadOp::Clear(wgpu::Color::BLACK),
        );
        pass.set_viewport(x as f32, y as f32, width as f32, height as f32, 0.0, 1.0);
        // The viewport transforms; it does not clip. Without the scissor the
        // oversized fullscreen triangle rasterizes over the bars too, and the
        // letterbox is a stretch again.
        pass.set_scissor_rect(x, y, width, height);
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, group, &[]);
        pass.draw(0..3, 0..1);
        true
    }
}

/// The largest rectangle carrying `src`'s aspect that fits inside `dst`,
/// centred — origin top-left, in `dst`'s own pixels.
///
/// The tap is a fixed **resolution** and the show's window is any shape, so the
/// two aspects disagree the moment anyone drags a window edge. Fitting rather
/// than stretching is ADR-0037's rule at the one place a scaled copy of the show
/// leaves the engine: bars are honest about the shape, a squash is not.
///
/// Every returned dimension is at least 1 — a zero-extent viewport is a wgpu
/// validation error, and a degenerate `src` is a window mid-minimize rather than
/// a caller's mistake.
pub(crate) fn fit_rect(src: (u32, u32), dst: (u32, u32)) -> (u32, u32, u32, u32) {
    let (dst_w, dst_h) = (dst.0.max(1), dst.1.max(1));
    let (src_w, src_h) = (f64::from(src.0.max(1)), f64::from(src.1.max(1)));
    let scale = (f64::from(dst_w) / src_w).min(f64::from(dst_h) / src_h);
    let width = ((src_w * scale).round() as u32).clamp(1, dst_w);
    let height = ((src_h * scale).round() as u32).clamp(1, dst_h);
    ((dst_w - width) / 2, (dst_h - height) / 2, width, height)
}

/// A rectangle in a console surface's device pixels, origin top-left.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    /// Distance from the surface's left edge to the rectangle's.
    pub x: f32,
    /// Distance from the surface's top edge to the rectangle's.
    pub y: f32,
    /// Rectangle width in device pixels.
    pub width: f32,
    /// Rectangle height in device pixels.
    pub height: f32,
}

impl Rect {
    /// Width over height. Undefined for a zero-height rectangle, which
    /// [`preview_rect`] never returns.
    pub fn aspect(&self) -> f32 {
        self.width / self.height
    }
}

/// The preview slot's longer side, as a fraction of the console surface.
///
/// A monitor, not a second show: large enough to read a cut from across a desk,
/// small enough that the modal list it sits beside keeps most of the window.
const SLOT_FRACTION: f32 = 0.32;

/// Gap between the slot and the console's bottom and right edges, in device
/// pixels at any console size — a fixed inset reads as a margin where a
/// proportional one reads as an error at small sizes.
const SLOT_MARGIN: f32 = 16.0;

/// Below this, on either side, the preview is not a picture of anything and is
/// better absent than misleading.
const MIN_SIDE: f32 = 32.0;

/// Where to draw the program preview inside a console surface, letterboxed.
///
/// **The aspect comes from `output` — the render target — and from nothing
/// else** (ADR-0037). The console window's own aspect and the slot's are both
/// containers: the returned rectangle fits inside the slot and keeps the
/// output's shape, so a 16:9 show in a square slot gets bars above and below
/// rather than a stretch. This project has shipped the other reading twice, and
/// both times the tests were written where the two sources agree.
///
/// `None` when either size is degenerate or the slot comes out too small to be
/// worth drawing.
pub fn preview_rect(output: (u32, u32), console: (u32, u32)) -> Option<Rect> {
    let (out_w, out_h) = (output.0 as f32, output.1 as f32);
    let (con_w, con_h) = (console.0 as f32, console.1 as f32);
    if out_w <= 0.0 || out_h <= 0.0 || con_w <= 0.0 || con_h <= 0.0 {
        return None;
    }
    let aspect = out_w / out_h;
    if !aspect.is_finite() || aspect <= 0.0 {
        return None;
    }

    // The slot is a square fraction of the console; the picture is then fitted
    // inside it. Two steps rather than one so the container's own shape cannot
    // leak into the picture's.
    let slot = (con_w.min(con_h) * SLOT_FRACTION).min(con_w - 2.0 * SLOT_MARGIN);
    if slot < MIN_SIDE {
        return None;
    }

    let (width, height) = if aspect >= 1.0 {
        (slot, slot / aspect)
    } else {
        (slot * aspect, slot)
    };
    if width < MIN_SIDE || height < MIN_SIDE {
        return None;
    }

    Some(Rect {
        x: con_w - SLOT_MARGIN - width,
        y: con_h - SLOT_MARGIN - height,
        width,
        height,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two sizes chosen so no pair of them shares an aspect: a wide output, a
    /// tall console, and a square one. At 16:9 against 16:9 — the shape the
    /// dev box and every golden run at — the target's aspect and the
    /// container's coincide, and no assertion written there can say which one
    /// the code read.
    const WIDE: (u32, u32) = (1920, 1080);
    const TALL: (u32, u32) = (600, 1000);
    const SQUARE: (u32, u32) = (900, 900);

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    #[test]
    fn the_preview_carries_the_output_aspect_and_not_the_consoles() {
        for console in [TALL, SQUARE, (1280, 400)] {
            let rect = preview_rect(WIDE, console).expect("a preview fits in every console here");
            let want = WIDE.0 as f32 / WIDE.1 as f32;
            assert!(
                approx(rect.aspect(), want),
                "console {console:?}: preview aspect {} is not the output's {want} — a \
                 container's shape reached the picture (ADR-0037)",
                rect.aspect()
            );
        }
    }

    #[test]
    fn a_portrait_output_letterboxes_the_other_way() {
        // The control on the test above: if the code read the container rather
        // than the target, this case and that one cannot both pass, because
        // the two aspects cross over.
        let rect = preview_rect(TALL, WIDE).expect("a preview fits");
        let want = TALL.0 as f32 / TALL.1 as f32;
        assert!(
            approx(rect.aspect(), want),
            "portrait output came back at {} rather than {want}",
            rect.aspect()
        );
        assert!(
            rect.height > rect.width,
            "a portrait output must produce a taller-than-wide rectangle, got \
             {}x{}",
            rect.width,
            rect.height
        );
    }

    #[test]
    fn the_rectangle_stays_inside_the_console_with_its_margin() {
        for console in [WIDE, TALL, SQUARE] {
            let rect = preview_rect(WIDE, console).expect("a preview fits");
            assert!(
                rect.x >= 0.0 && rect.y >= 0.0,
                "{rect:?} starts off-surface"
            );
            assert!(
                approx(rect.x + rect.width, console.0 as f32 - SLOT_MARGIN),
                "{rect:?} is not inset from the right edge of {console:?}"
            );
            assert!(
                approx(rect.y + rect.height, console.1 as f32 - SLOT_MARGIN),
                "{rect:?} is not inset from the bottom edge of {console:?}"
            );
        }
    }

    #[test]
    fn the_tap_keeps_the_shows_aspect_and_centres_the_bars() {
        // 16:10 into 16:9: bars left and right, and the picture centred between
        // them. The tap is a resolution and not a shape (ADR-0037), so the
        // show's aspect survives and the tap's does not reach the picture.
        assert_eq!(fit_rect((1280, 800), (640, 360)), (32, 0, 576, 360));
        // The other way: 4:3 into 16:9 is the same rule, and 21:9 into 16:9 puts
        // the bars above and below instead — the case a test written only at
        // 16:9 cannot tell apart from a stretch.
        assert_eq!(fit_rect((1024, 768), (640, 360)), (80, 0, 480, 360));
        assert_eq!(fit_rect((2560, 1080), (640, 360)), (0, 45, 640, 270));
    }

    #[test]
    fn a_matching_aspect_fills_the_tap_with_no_bars() {
        // The control on the test above: where the two aspects agree there is
        // nothing to letterbox, and a fit that still inset the picture would be
        // shrinking the show for no reason.
        assert_eq!(fit_rect((1920, 1080), (640, 360)), (0, 0, 640, 360));
        assert_eq!(fit_rect((640, 360), (640, 360)), (0, 0, 640, 360));
    }

    #[test]
    fn a_degenerate_size_still_produces_a_drawable_rectangle() {
        // A window mid-minimize reports a zero dimension, and a zero-extent
        // viewport is a wgpu validation error rather than an empty frame.
        for (src, dst) in [((0, 0), (640, 360)), ((1920, 0), (640, 360))] {
            let (_, _, width, height) = fit_rect(src, dst);
            assert!(
                width >= 1 && height >= 1,
                "fit_rect({src:?}, {dst:?}) produced a {width}x{height} viewport"
            );
        }
        assert_eq!(fit_rect((1920, 1080), (0, 0)), (0, 0, 1, 1));
    }

    #[test]
    fn a_console_too_small_to_show_anything_gets_no_preview() {
        assert_eq!(preview_rect(WIDE, (120, 90)), None);
        assert_eq!(preview_rect(WIDE, (0, 0)), None);
        assert_eq!(preview_rect((0, 1080), SQUARE), None);
    }
}
