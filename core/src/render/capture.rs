//! Headless offscreen capture: draw into a texture with no window and read the
//! pixels back as tight RGBA (Plan 0013). Dev/agent tooling over the native
//! Rust API — no dependency, no present.
//!
//! **Not the hot path.** The readback blocks (`map_async` + `poll(Wait)`); it is
//! only ever driven by capture/QA tooling, never wired into the live `render`
//! loop (see CLAUDE.md real-time rules). The panic-denial pragma below is kept
//! anyway so every file under `render/` satisfies the hygiene guard.

#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use super::RenderError;

/// Bytes per pixel of [`HEADLESS_FORMAT`](super::context::HEADLESS_FORMAT).
const BYTES_PER_PIXEL: u32 = 4;

/// Bytes per pixel of [`COMPOSITE_FORMAT`](super::COMPOSITE_FORMAT) — four
/// 16-bit halves. Only the linear readback below reads it, and that is test-only.
#[cfg(test)]
const LINEAR_BYTES_PER_PIXEL: u32 = 8;

/// A captured frame: tight (row-unpadded) `Rgba8UnormSrgb` pixels, row-major
/// top-to-bottom. `rgba.len() == width * height * 4`.
#[derive(Clone)]
pub struct CaptureImage {
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
    /// `width * height * 4` bytes, RGBA8, no row padding.
    pub rgba: Vec<u8>,
}

/// A `RENDER_ATTACHMENT | COPY_SRC` texture sized `width`×`height` plus a view,
/// the offscreen draw target for one capture.
pub(crate) fn create_target(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("lmv-capture-target"),
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
    (texture, view)
}

/// A `COPY_DST | MAP_READ` readback buffer sized for `height` rows padded to the
/// 256-byte row alignment `copy_texture_to_buffer` requires; returns it with the
/// padded bytes-per-row so [`read_back`] can strip the padding.
pub(crate) fn create_readback(
    device: &wgpu::Device,
    width: u32,
    height: u32,
) -> (wgpu::Buffer, u32) {
    let padded_bpr = padded_row_bytes(width);
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("lmv-capture-readback"),
        size: padded_bpr as u64 * height as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    (buffer, padded_bpr)
}

/// Clear the capture target to opaque black before the scene draws, so an empty
/// or `Load`-op scene still yields defined, non-transparent pixels.
pub(crate) fn record_clear(encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("lmv-capture-clear"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
}

/// Record the texture→buffer copy honoring the padded row stride.
pub(crate) fn record_copy(
    encoder: &mut wgpu::CommandEncoder,
    texture: &wgpu::Texture,
    buffer: &wgpu::Buffer,
    padded_bpr: u32,
    width: u32,
    height: u32,
) {
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_bpr),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
}

/// Map the readback buffer (blocking on `poll(Wait)`), strip the row padding,
/// and return a tight [`CaptureImage`]. The caller must have already submitted
/// the copy. Off the hot path by construction.
pub(crate) fn read_back(
    device: &wgpu::Device,
    buffer: &wgpu::Buffer,
    width: u32,
    height: u32,
    padded_bpr: u32,
) -> Result<CaptureImage, RenderError> {
    let slice = buffer.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |res| {
        let _ = tx.send(res);
    });
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|_| RenderError::CaptureReadback)?;
    rx.recv()
        .map_err(|_| RenderError::CaptureReadback)?
        .map_err(|_| RenderError::CaptureReadback)?;

    let rgba = {
        let mapped = slice
            .get_mapped_range()
            .map_err(|_| RenderError::CaptureReadback)?;
        unpad_rows(&mapped, width, height, padded_bpr)
    };
    buffer.unmap();

    Ok(CaptureImage {
        width,
        height,
        rgba,
    })
}

/// `width * 4` rounded up to the 256-byte row alignment.
fn padded_row_bytes(width: u32) -> u32 {
    row_bytes(width, BYTES_PER_PIXEL)
}

/// `width * bytes_per_pixel` rounded up to the 256-byte row alignment
/// `copy_texture_to_buffer` requires.
fn row_bytes(width: u32, bytes_per_pixel: u32) -> u32 {
    let unpadded = width * bytes_per_pixel;
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    unpadded.div_ceil(align) * align
}

// ---------------------------------------------------------------------------
// Linear-light readback (Plan 0045 Phase 3)
// ---------------------------------------------------------------------------

/// A `COPY_DST | MAP_READ` buffer sized for a `Rgba16Float` texture of
/// `width`×`height`; returns it with the padded bytes-per-row.
#[cfg(test)]
pub(crate) fn create_linear_readback(
    device: &wgpu::Device,
    width: u32,
    height: u32,
) -> (wgpu::Buffer, u32) {
    let padded_bpr = row_bytes(width, LINEAR_BYTES_PER_PIXEL);
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("lmv-capture-linear-readback"),
        size: padded_bpr as u64 * height as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    (buffer, padded_bpr)
}

/// [`read_back`], for a `Rgba16Float` source: strips the row padding and decodes
/// each half to `f32`, returning `width * height * 4` tight linear values
/// row-major top-to-bottom.
///
/// The point is that these are **not clamped to 1.0** — this is the only way to
/// observe the composite as light rather than as a picture, which is what Plan
/// 0045 Phase 3's first done-when asks for. Test-only: nothing in the frame path
/// reads a texture back (see the module docs on why the readback blocks).
#[cfg(test)]
pub(crate) fn read_back_linear(
    device: &wgpu::Device,
    buffer: &wgpu::Buffer,
    width: u32,
    height: u32,
    padded_bpr: u32,
) -> Result<Vec<f32>, RenderError> {
    let slice = buffer.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |res| {
        let _ = tx.send(res);
    });
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|_| RenderError::CaptureReadback)?;
    rx.recv()
        .map_err(|_| RenderError::CaptureReadback)?
        .map_err(|_| RenderError::CaptureReadback)?;

    let rgba = {
        let mapped = slice
            .get_mapped_range()
            .map_err(|_| RenderError::CaptureReadback)?;
        let tight_bpr = (width * LINEAR_BYTES_PER_PIXEL) as usize;
        let mut out = Vec::with_capacity(width as usize * height as usize * 4);
        for row in mapped.chunks_exact(padded_bpr as usize) {
            let Some(tight) = row.get(..tight_bpr) else {
                continue; // a short final row — never expected
            };
            for half in tight.chunks_exact(2) {
                let bits = u16::from_le_bytes([
                    half.first().copied().unwrap_or(0),
                    half.get(1).copied().unwrap_or(0),
                ]);
                out.push(f16_to_f32(bits));
            }
        }
        out
    };
    buffer.unmap();

    Ok(rgba)
}

/// Decode one IEEE-754 binary16 to `f32`. Twelve lines rather than a `half`
/// dependency: it is test-only, and "every new crate is a cost" (CLAUDE.md).
///
/// Subnormals are handled by arithmetic (`mantissa * 2^-24`, exact in `f32`)
/// rather than by renormalizing bit surgery, so there is no loop to bound.
#[cfg(test)]
fn f16_to_f32(bits: u16) -> f32 {
    let sign = if bits & 0x8000 != 0 { -1.0f32 } else { 1.0 };
    let exponent = u32::from((bits >> 10) & 0x1f);
    let mantissa = u32::from(bits & 0x03ff);
    match exponent {
        // Zero or subnormal.
        0 => sign * (mantissa as f32) * (1.0 / 16_777_216.0),
        // Infinity or NaN.
        0x1f => f32::from_bits(((bits as u32 & 0x8000) << 16) | 0x7f80_0000 | (mantissa << 13)),
        // Normal: rebias the exponent from 15 to 127 and left-align the mantissa.
        _ => f32::from_bits(
            ((bits as u32 & 0x8000) << 16) | ((exponent + 127 - 15) << 23) | (mantissa << 13),
        ),
    }
}

/// Copy the tight `width*4` bytes out of each padded row into a contiguous
/// buffer. A short final row (never expected) is skipped rather than panicking.
fn unpad_rows(padded: &[u8], width: u32, height: u32, padded_bpr: u32) -> Vec<u8> {
    let tight_bpr = (width * BYTES_PER_PIXEL) as usize;
    let mut out = Vec::with_capacity(tight_bpr * height as usize);
    for row in padded.chunks_exact(padded_bpr as usize) {
        if let Some(tight) = row.get(..tight_bpr) {
            out.extend_from_slice(tight);
        }
    }
    out
}
