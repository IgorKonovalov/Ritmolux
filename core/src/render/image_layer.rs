//! One shell-supplied picture drawn over the frame, behind the non-default
//! `text` feature beside the text layer it composites with.
//!
//! The shell hands the renderer an RGBA8 image with
//! [`Renderer::set_overlay_image`](super::Renderer::set_overlay_image), which is
//! the **only** upload: the bytes go into a texture the layer keeps, and every
//! later frame that wants the picture queues a rectangle with
//! [`Renderer::queue_image`](super::Renderer::queue_image), the way it queues
//! [`TextRun`](super::TextRun)s. The draw rides the pass that draws the text,
//! before the text, so a label can sit on top of the picture.
//!
//! **A renderer that never sets an image holds no GPU object for this layer.**
//! The pipeline, sampler, rectangle uniform, texture and bind group are all
//! built on the first `set`, so the cost of the layer while unused is one
//! `Option` test per frame. A frame that sets an image and queues nothing
//! encodes no draw; a frame that only queues writes no texture and, while the
//! rectangle is unchanged, writes nothing at all.
//!
//! **The texture is `Rgba8UnormSrgb`, the format a headless capture reads back
//! from**, so bytes a capture wrote are sRGB-encoded exactly as this texture
//! expects: sampling decodes them to linear light and the sRGB surface encodes
//! them again on the way out, and a picture drawn at its own size reads back as
//! the bytes it was set from. An `Rgba8Unorm` texture here would apply the
//! surface's encode to bytes that already carry it, and the picture would come
//! out washed pale.

// Hot-path panic-denial pragma (Plan 0002 Phase 2; `render/` scan set). The
// prepare and draw run every displayed frame while an image is queued; a panic
// here is a visible crash.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use super::gpu;

/// The picture's texture format. See the module docs: it has to be the format
/// the capture path's bytes are encoded in, or the colour step runs twice.
pub(crate) const IMAGE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// An image the shell hands the renderer: `width * height` pixels of RGBA8,
/// rows top first, no row padding — a
/// [`CaptureImage`](super::CaptureImage)'s own layout. The bytes are copied into
/// a texture; the borrow ends with the call.
pub struct OverlayImage<'a> {
    /// `width * height * 4` bytes.
    pub rgba: &'a [u8],
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

/// Where the set image is drawn this frame, in device pixels from the target's
/// top-left — the coordinate space [`TextRun`](super::TextRun) uses. The image
/// is stretched to fill it; the caller chooses a rectangle of the image's own
/// aspect if it wants none.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImageRect {
    /// Left edge.
    pub x: f32,
    /// Top edge.
    pub y: f32,
    /// Width.
    pub w: f32,
    /// Height.
    pub h: f32,
}

/// Why [`Renderer::set_overlay_image`](super::Renderer::set_overlay_image)
/// refused an image. Nothing changes on a refusal: an image already set stays
/// set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayImageError {
    /// A dimension is zero, or larger than the adapter's 2D texture limit.
    Size {
        /// The width asked for.
        width: u32,
        /// The height asked for.
        height: u32,
    },
    /// The byte count is not `width * height * 4`.
    Length {
        /// `width * height * 4`.
        expected: usize,
        /// The bytes handed over.
        got: usize,
    },
}

impl std::fmt::Display for OverlayImageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Size { width, height } => {
                write!(f, "an overlay image cannot be {width}x{height}")
            }
            Self::Length { expected, got } => {
                write!(f, "an overlay image needs {expected} bytes, got {got}")
            }
        }
    }
}

impl std::error::Error for OverlayImageError {}

/// The quad's shader. `rect` is `(x0, y0, x1, y1)` in NDC with `y0` the top
/// edge, and `uv` runs `0..1` across the quad with `v = 0` at the top — which is
/// also the texture's first row, so no flip is applied anywhere.
const IMAGE_WGSL: &str = r#"
@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;
struct Rect { ndc: vec4<f32> };
@group(0) @binding(2) var<uniform> rect: Rect;

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

/// The parts that do not depend on the image: built on the first `set` and kept
/// for the renderer's life.
struct Pipe {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    rect: wgpu::Buffer,
}

impl Pipe {
    fn new(device: &wgpu::Device, target: wgpu::TextureFormat) -> Self {
        // Texture, sampler, then a vertex-visible uniform: a shape no other
        // layout in `core/src` has (ADR-0058), which the tonemap tests'
        // enumeration holds.
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("rlx-overlay-image-layout"),
            entries: &[
                gpu::texture(0, true),
                gpu::sampler(1),
                gpu::uniform(2, wgpu::ShaderStages::VERTEX),
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("rlx-overlay-image"),
            source: wgpu::ShaderSource::Wgsl(IMAGE_WGSL.into()),
        });
        let pipeline = gpu::fullscreen_pipeline(
            device,
            &shader,
            &[&layout],
            target,
            wgpu::BlendState::REPLACE,
            "rlx-overlay-image",
        );
        Self {
            pipeline,
            layout,
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("rlx-overlay-image-sampler"),
                // Linear, clamped: a picture scaled to a pane is resampled,
                // and at its own size every sample lands on a texel centre, so
                // the filter changes nothing there.
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
            rect: gpu::uniform_buffer(
                device,
                "rlx-overlay-image-rect",
                std::mem::size_of::<[f32; 4]>(),
            ),
        }
    }
}

/// The uploaded picture: its texture, and the bind group built against it.
struct Uploaded {
    texture: wgpu::Texture,
    group: wgpu::BindGroup,
    size: (u32, u32),
}

/// What the layer has done since it was built — read by the tests that hold the
/// "setting is the only upload" claim.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ImageLayerCounts {
    /// Textures created. Moves only when the image's dimensions change.
    pub(crate) textures: u32,
    /// `write_texture` calls. Moves only on a `set`.
    pub(crate) uploads: u32,
    /// Writes of the rectangle uniform. Moves only when the rectangle, in the
    /// target's NDC, differs from the last one written.
    pub(crate) rect_writes: u32,
}

/// The layer: nothing until the first image is set, then a pipeline and one
/// texture. One instance per [`super::Renderer`].
pub(crate) struct ImageLayer {
    /// The format the draw writes — the surface's, like the text layer's.
    target: wgpu::TextureFormat,
    pipe: Option<Pipe>,
    image: Option<Uploaded>,
    /// This frame's rectangle, taken by [`prepare`](Self::prepare).
    queued: Option<ImageRect>,
    /// The NDC rectangle last written to the uniform, so an unchanged one is
    /// not written again.
    written: Option<[f32; 4]>,
    /// Whether the last `prepare` left something for [`render`](Self::render).
    ready: bool,
    counts: ImageLayerCounts,
}

impl ImageLayer {
    /// An empty layer drawing into `target`. Creates no GPU object.
    pub(crate) fn new(target: wgpu::TextureFormat) -> Self {
        Self {
            target,
            pipe: None,
            image: None,
            queued: None,
            written: None,
            ready: false,
            counts: ImageLayerCounts::default(),
        }
    }

    /// Whether any GPU object of this layer exists.
    #[cfg(test)]
    pub(crate) fn built(&self) -> bool {
        self.pipe.is_some() || self.image.is_some()
    }

    #[cfg(test)]
    pub(crate) fn counts(&self) -> ImageLayerCounts {
        self.counts
    }

    /// The set image's size, or `None` when none is set.
    pub(crate) fn size(&self) -> Option<(u32, u32)> {
        self.image.as_ref().map(|image| image.size)
    }

    /// Set the picture, or clear it with `None`.
    ///
    /// The bytes are validated here, once, at the boundary: a refused image
    /// changes nothing. A texture is created only when there is none or the
    /// dimensions differ from the one held; otherwise the bytes are written into
    /// the existing texture, and its bind group stays valid.
    pub(crate) fn set(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        image: Option<OverlayImage<'_>>,
    ) -> Result<(), OverlayImageError> {
        let Some(image) = image else {
            self.image = None;
            return Ok(());
        };
        let (width, height) = (image.width, image.height);
        let limit = device.limits().max_texture_dimension_2d;
        if width == 0 || height == 0 || width > limit || height > limit {
            return Err(OverlayImageError::Size { width, height });
        }
        let expected = width as usize * height as usize * 4;
        if image.rgba.len() != expected {
            return Err(OverlayImageError::Length {
                expected,
                got: image.rgba.len(),
            });
        }

        let target = self.target;
        let pipe = self.pipe.get_or_insert_with(|| Pipe::new(device, target));
        if self
            .image
            .as_ref()
            .is_none_or(|held| held.size != (width, height))
        {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("rlx-overlay-image-texture"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: IMAGE_FORMAT,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("rlx-overlay-image-group"),
                layout: &pipe.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&pipe.sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: pipe.rect.as_entire_binding(),
                    },
                ],
            });
            self.image = Some(Uploaded {
                texture,
                group,
                size: (width, height),
            });
            self.counts.textures = self.counts.textures.saturating_add(1);
        }
        if let Some(held) = self.image.as_ref() {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &held.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                image.rgba,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(width * 4),
                    rows_per_image: Some(height),
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
            self.counts.uploads = self.counts.uploads.saturating_add(1);
        }
        Ok(())
    }

    /// Draw the set image into `rect` on the next frame. Replaces a rectangle
    /// already queued; a frame consumes it, so a picture wanted on every frame
    /// is queued on every frame, as text is.
    pub(crate) fn queue(&mut self, rect: ImageRect) {
        self.queued = Some(rect);
    }

    /// Take this frame's rectangle and, if there is an image to put in it,
    /// write the rectangle for a `width`x`height` target. Returns whether
    /// [`render`](Self::render) will draw.
    ///
    /// A rectangle with no area, or one that is not finite, draws nothing.
    pub(crate) fn prepare(&mut self, queue: &wgpu::Queue, width: u32, height: u32) -> bool {
        self.ready = false;
        let Some(rect) = self.queued.take() else {
            return false;
        };
        let (Some(pipe), Some(_)) = (self.pipe.as_ref(), self.image.as_ref()) else {
            return false;
        };
        let finite = [rect.x, rect.y, rect.w, rect.h]
            .iter()
            .all(|v| v.is_finite());
        if !finite || rect.w <= 0.0 || rect.h <= 0.0 {
            return false;
        }
        let ndc = to_ndc(rect, width, height);
        if self.written != Some(ndc) {
            queue.write_buffer(&pipe.rect, 0, bytemuck::cast_slice(&ndc));
            self.written = Some(ndc);
            self.counts.rect_writes = self.counts.rect_writes.saturating_add(1);
        }
        self.ready = true;
        true
    }

    /// Draw the prepared quad into `pass`. A no-op unless `prepare` said it
    /// would draw.
    pub(crate) fn render(&self, pass: &mut wgpu::RenderPass<'_>) {
        if !self.ready {
            return;
        }
        let (Some(pipe), Some(image)) = (self.pipe.as_ref(), self.image.as_ref()) else {
            return;
        };
        pass.set_pipeline(&pipe.pipeline);
        pass.set_bind_group(0, &image.group, &[]);
        pass.draw(0..6, 0..1);
    }
}

/// `rect`, in device pixels of a `width`x`height` target, as
/// `(x0, y0, x1, y1)` in NDC with `y0` the top edge.
fn to_ndc(rect: ImageRect, width: u32, height: u32) -> [f32; 4] {
    let (w, h) = (width.max(1) as f32, height.max(1) as f32);
    [
        rect.x / w * 2.0 - 1.0,
        1.0 - rect.y / h * 2.0,
        (rect.x + rect.w) / w * 2.0 - 1.0,
        1.0 - (rect.y + rect.h) / h * 2.0,
    ]
}
