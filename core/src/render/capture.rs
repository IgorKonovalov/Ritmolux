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
use crate::render::gpu;

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
        label: Some("rlx-capture-target"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        // `COPY_DST` alongside the two it is drawn and read through, so this
        // target can stand in for a swapchain image on the preview path — where
        // the frame is drawn into an intermediate and reaches its destination by
        // `copy_texture_to_texture`. Without it the capture paths could not
        // exercise that copy at all, and the only instrument left for it would
        // need a real window.
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST,
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
        label: Some("rlx-capture-readback"),
        size: padded_bpr as u64 * height as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    (buffer, padded_bpr)
}

/// Clear the capture target to opaque black before the scene draws, so an empty
/// or `Load`-op scene still yields defined, non-transparent pixels.
pub(crate) fn record_clear(encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
    gpu::color_pass(
        encoder,
        "rlx-capture-clear",
        view,
        wgpu::LoadOp::Clear(wgpu::Color::BLACK),
    );
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
        label: Some("rlx-capture-linear-readback"),
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
pub(super) fn unpad_rows(padded: &[u8], width: u32, height: u32, padded_bpr: u32) -> Vec<u8> {
    let tight_bpr = (width * BYTES_PER_PIXEL) as usize;
    let mut out = Vec::with_capacity(tight_bpr * height as usize);
    for row in padded.chunks_exact(padded_bpr as usize) {
        if let Some(tight) = row.get(..tight_bpr) {
            out.extend_from_slice(tight);
        }
    }
    out
}

// ---------------------------------------------------------------------------
// The sustained frame tap (Plan 0115 Phase 2)
// ---------------------------------------------------------------------------

/// A persistent offscreen target plus readback buffer, built once and reused for
/// every frame of a sustained tap.
///
/// The difference from the rest of this file is lifetime, not stage. Every other
/// capture entry point builds its target and buffer per call — right for QA
/// tooling that takes one frame, and wrong for a source that takes 864,000 of
/// them, where per-frame texture and buffer creation is GPU allocation inside
/// the loop. `capture_stream` already reuses its pair, but only for the length
/// of one fixed-`dt`, one-preset run it drives itself; this type hands that
/// reuse to a caller who owns the loop.
///
/// # One frame in flight, and a wait only when the GPU is behind
///
/// The cycle is the preview readback's, three steps across two frames:
/// **take** the previous submission's map, **record** this frame's copy into the
/// freed buffer, and **arm** the map after the submission. So a caller sees
/// frame *N* while frame *N+1* is being drawn, and the very first call yields
/// nothing at all.
///
/// **The take waits when the previous map has not landed**
/// (`take_previous`). That wait is the only backpressure
/// a windowless loop has: the window is held to the GPU's pace by the
/// swapchain, and nothing holds a tap to it but this. Without it, an adapter
/// whose frame costs more GPU time than the caller's loop costs CPU time queues
/// submissions without bound, the one map in flight lands only behind all of
/// them, and nearly every frame drawn meanwhile is never copied out. On an
/// adapter that keeps up, the map has landed by the next call and nothing
/// waits.
///
/// **The buffer cannot be re-recorded while it is mapped**, which is why the
/// take comes before the record: a second copy into a mapped buffer is a
/// validation error, not a dropped frame.
///
/// **Sized at construction and never resized.** `record_copy`'s extent, the
/// buffer's length and `padded_bpr` are all fixed against `width`×`height`, so a
/// renderer that resizes underneath a live tap needs a new one — [`open_tap`]
/// is the only thing that sets these.
///
/// [`open_tap`]: super::Renderer::open_tap
pub struct FrameTap {
    /// `RENDER_ATTACHMENT | COPY_SRC`, the offscreen the frame draws into.
    pub(crate) texture: wgpu::Texture,
    /// A view of `texture`, held rather than recreated per frame.
    pub(crate) view: wgpu::TextureView,
    /// `COPY_DST | MAP_READ`, sized `padded_bpr * height`.
    pub(crate) buffer: wgpu::Buffer,
    /// Row stride of `buffer`, padded to the 256-byte copy alignment.
    pub(crate) padded_bpr: u32,
    /// Pixel width the three resources above are sized against.
    pub(crate) width: u32,
    /// Pixel height the three resources above are sized against.
    pub(crate) height: u32,
    /// The per-pass GPU timer, `None` on an adapter with no timestamp queries.
    ///
    /// **This is the only thing in the engine that builds a query set**, which
    /// is what makes "the window path encodes no timestamp writes" structural:
    /// a window has no tap.
    pub(crate) timer: Option<gpu::PassTimer>,
    /// What the timer has measured since the last [`reset_pass_costs`].
    ///
    /// [`reset_pass_costs`]: FrameTap::reset_pass_costs
    pub(crate) costs: PassCosts,
    /// The armed map's result channel, `None` when [`buffer`](Self::buffer) is
    /// free to record into.
    armed: Option<std::sync::mpsc::Receiver<Result<(), wgpu::BufferAsyncError>>>,
}

impl FrameTap {
    /// Build the target, its view, the readback buffer and — where the device
    /// offers timestamp queries — the pass timer, in one step: the whole of the
    /// tap's GPU allocation, paid here so the per-frame path pays none.
    pub(crate) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let (texture, view) = create_target(device, format, width, height);
        let (buffer, padded_bpr) = create_readback(device, width, height);
        Self {
            texture,
            view,
            buffer,
            padded_bpr,
            width,
            height,
            timer: gpu::PassTimer::new(device, queue),
            costs: PassCosts::default(),
            armed: None,
        }
    }

    /// The pixel size every frame this tap yields will carry.
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Take the frame the **previous** submission's map produced, if it has
    /// landed, and fold that frame's pass timings in with it.
    ///
    /// Polls without waiting. A map still in flight yields `None` and leaves the
    /// buffer armed; the next call asks again. Call this before
    /// [`record`](Self::record) within a frame — the buffer cannot be copied
    /// into while it is mapped.
    ///
    /// The timings are collected **here**, before the timer is re-armed for the
    /// frame about to be encoded, because the labels and the claim count it
    /// still holds are the ones the landed timestamps belong to.
    pub(crate) fn consume(&mut self, device: &wgpu::Device) -> Option<CaptureImage> {
        use std::sync::mpsc::TryRecvError;

        let armed = self.armed.as_ref()?;
        // Non-blocking: a map that has landed is taken without paying for
        // whatever else the GPU has queued. The wait, where one is owed, is
        // `take_previous`'s.
        let _ = device.poll(wgpu::PollType::Poll);
        match armed.try_recv() {
            Ok(Ok(())) => {}
            // Still in flight: not an error and not a dropped frame. The buffer
            // stays armed and the next call asks again.
            Err(TryRecvError::Empty) => return None,
            // Mapping failed, or the callback was dropped without firing. Either
            // way this cycle is over: disarm so the next frame records afresh.
            Ok(Err(_)) | Err(TryRecvError::Disconnected) => {
                self.armed = None;
                if let Some(timer) = self.timer.as_mut() {
                    timer.discard();
                }
                return None;
            }
        }
        self.armed = None;
        if let Some(timer) = self.timer.as_mut() {
            timer.collect(&mut self.costs);
        }
        let slice = self.buffer.slice(..);
        let image = slice.get_mapped_range().ok().map(|mapped| CaptureImage {
            width: self.width,
            height: self.height,
            rgba: unpad_rows(&mapped, self.width, self.height, self.padded_bpr),
        });
        // Unmapped whether or not the range was readable: a buffer left mapped
        // is one this tap can never record into again.
        self.buffer.unmap();
        image
    }

    /// Take the frame the previous submission carried, **waiting for it only if
    /// its map has not landed**, so that on return the buffer is always free to
    /// record into.
    ///
    /// This is what bounds the GPU queue to the frame about to be drawn: every
    /// call after the first returns a frame, on any adapter, and a caller that
    /// outruns the GPU is held to its rate instead of queueing draws it will
    /// never read. `None` on the first call and after a map that failed.
    pub(crate) fn take_previous(&mut self, device: &wgpu::Device) -> Option<CaptureImage> {
        let image = self.consume(device);
        if image.is_some() || self.armed.is_none() {
            return image;
        }
        self.drain(device)
    }

    /// **Wait** for the frame still in flight and take it, drawing nothing.
    ///
    /// The blocking counterpart of [`consume`](Self::consume): a bounded run's
    /// last frame, and the half of [`take_previous`](Self::take_previous) that
    /// runs when the GPU is behind. `None` when nothing is in flight. This file
    /// is one of the two the indefinite-wait allowlist admits, for this wait.
    pub(crate) fn drain(&mut self, device: &wgpu::Device) -> Option<CaptureImage> {
        self.armed.as_ref()?;
        // The map was asked for on a submission that has already been made, so
        // this returns as soon as the GPU retires it.
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
        self.consume(device)
    }

    /// Ask for the mapping, after the submission that carried the copy.
    ///
    /// The callback only sends; every decision is taken on the caller's thread
    /// when it next polls, so nothing wgpu calls back into does work.
    pub(crate) fn arm(&mut self) {
        let (tx, rx) = std::sync::mpsc::channel();
        self.buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |res| {
                let _ = tx.send(res);
            });
        self.armed = Some(rx);
    }

    /// Whether this tap can report per-pass GPU costs at all. `false` on an
    /// adapter without `TIMESTAMP_QUERY` — the software rasterizers — where a
    /// caller says so once rather than printing an empty table every window.
    pub fn times_passes(&self) -> bool {
        self.timer.is_some()
    }

    /// What every labelled pass has cost since the last reset.
    pub fn pass_costs(&self) -> &PassCosts {
        &self.costs
    }

    /// Start a fresh measurement window, so each report covers the interval
    /// since the last one rather than the whole run to date.
    pub fn reset_pass_costs(&mut self) {
        self.costs.reset();
    }
}

// ---------------------------------------------------------------------------
// What the passes cost
// ---------------------------------------------------------------------------

/// GPU time per labelled render/compute pass, accumulated over a window of
/// frames (ADR-0245).
///
/// **Summed per label, not per pass instance.** A frame encodes `bloom-blur-h`
/// once per pyramid level, and what a reader wants to know is what the blur
/// costs the frame — so the rows are what each *label* costs per frame, and a
/// label that appears `N` times carries all `N`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PassCosts {
    /// `(label, total milliseconds)`, in first-seen order. A `Vec` rather than
    /// a map because the roster is a few dozen entries walked once a frame: a
    /// linear scan over that is cheaper than hashing, and it allocates nothing
    /// once the labels have all been seen.
    rows: Vec<(String, f64)>,
    /// Frames these totals cover, so a row can be reported as a mean.
    frames: u64,
}

impl PassCosts {
    /// Add one pass's milliseconds to its label's running total.
    pub(crate) fn add(&mut self, label: &str, ms: f64) {
        if !ms.is_finite() {
            return;
        }
        if let Some(row) = self.rows.iter_mut().find(|(name, _)| name == label) {
            row.1 += ms;
            return;
        }
        self.rows.push((label.to_owned(), ms));
    }

    /// Note that a frame's worth of passes has been added.
    pub(crate) fn close_frame(&mut self) {
        self.frames += 1;
    }

    /// Frames the accumulated totals cover.
    pub fn frames(&self) -> u64 {
        self.frames
    }

    /// Every label's **mean milliseconds per frame**, costliest first. Empty
    /// while no frame has been measured.
    ///
    /// Sorted here rather than by the caller so every report orders the rows
    /// the same way; ties keep first-seen order, which is roughly composite
    /// order and reads as the frame's own sequence.
    pub fn rows(&self) -> Vec<(&str, f64)> {
        if self.frames == 0 {
            return Vec::new();
        }
        let frames = self.frames as f64;
        let mut rows: Vec<(&str, f64)> = self
            .rows
            .iter()
            .map(|(label, total)| (label.as_str(), total / frames))
            .collect();
        rows.sort_by(|a, b| b.1.total_cmp(&a.1));
        rows
    }

    /// Drop the accumulated totals and the frame count.
    pub fn reset(&mut self) {
        self.rows.clear();
        self.frames = 0;
    }
}
