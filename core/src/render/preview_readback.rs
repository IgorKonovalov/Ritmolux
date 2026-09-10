//! A non-blocking readback of the preview intermediate, consumed one frame late.
//!
//! The operator console already draws the show into an intermediate and copies
//! it out (ADR-0143). A studio wants those same pixels on the CPU — and the one
//! thing it must not cost is a **stall in the display loop**, which answers to a
//! present deadline the headless tap does not have.
//!
//! ## One frame in flight, and nothing waits
//!
//! Each cycle is three steps across two frames:
//!
//! 1. **Record** — frame *N*'s encoder gets a scaling blit from the intermediate
//!    into the fixed-size tap and a `copy_texture_to_buffer` out of that tap into
//!    this buffer, both riding the submission the frame makes anyway.
//! 2. **Arm** — after that submission, `map_async` is asked for the buffer.
//! 3. **Consume** — frame *N+1* polls **without waiting**
//!    ([`wgpu::PollType::Poll`]) and takes the mapping if it has landed. If it
//!    has not, this frame yields nothing and the buffer stays in flight; the
//!    display loop does not notice.
//!
//! So a consumer sees frame *N* while frame *N+1* is being drawn. That is the
//! whole latency, and it is the price of never blocking.
//!
//! **The buffer cannot be re-recorded while it is mapped**, which is why the
//! consume step precedes the record step within a frame and why a map that has
//! not landed skips the record: a second copy into a mapped buffer is a
//! validation error, not a dropped frame.
//!
//! ## Why not the frame tap's readback
//!
//! `capture.rs`'s `read_back` waits indefinitely, deliberately: a headless loop
//! outruns the GPU and the wait is what paces it. That is the correct policy
//! there and the wrong one here, and the two are not merged for exactly that
//! reason — `core/tests/console_preview.rs` holds `render/` to no indefinite
//! wait outside the two capture files.

// Hot-path panic-denial pragma (Plan 0002 Phase 2; render/ is scanned by the
// hygiene guard).
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

// A continuation of one module split across several files, so it needs the
// names `render/mod.rs` has in scope.
use super::*;

use std::sync::mpsc::{Receiver, TryRecvError};

/// The fixed-size tap, the staging buffer, and the state of the map in flight.
pub(super) struct PreviewReadback {
    /// The tap the buffer copies out of. Owned here rather than beside the
    /// intermediate because its whole purpose is to give this readback one
    /// geometry for the life of the run (ADR-0187).
    pub(super) tap: preview::PreviewTap,
    buffer: wgpu::Buffer,
    width: u32,
    height: u32,
    /// `width * 4` rounded up to wgpu's 256-byte row alignment. The mapped
    /// range carries this stride and the frame handed out does not.
    padded_bpr: u32,
    /// The armed map's result channel, `None` when the buffer is free to record
    /// into.
    armed: Option<Receiver<Result<(), wgpu::BufferAsyncError>>>,
}

impl PreviewReadback {
    /// Build a readback that yields `width`x`height` frames at `format`.
    ///
    /// Neither follows the intermediate: the tap is built here and stays, so a
    /// renderer resize changes what the blit reads and nothing about what this
    /// hands out.
    pub(super) fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let tap = preview::PreviewTap::new(device, format, width, height);
        let (width, height) = tap.size();
        let (buffer, padded_bpr) = capture::create_readback(device, width, height);
        Self {
            tap,
            buffer,
            width,
            height,
            padded_bpr,
            armed: None,
        }
    }

    /// The size of the frames this yields.
    pub(super) fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Take the frame the **previous** submission's map produced, if it has
    /// landed.
    ///
    /// Polls without waiting. A map still in flight yields `None` and leaves the
    /// buffer armed; the next frame asks again. Call this before
    /// [`record`](Self::record) within a frame — the buffer cannot be copied
    /// into while it is mapped.
    pub(super) fn consume(&mut self, device: &wgpu::Device) -> Option<CaptureImage> {
        let armed = self.armed.as_ref()?;
        // The one poll on this path, and it is the non-blocking kind. A `Wait`
        // here is the defect the whole module exists to avoid.
        let _ = device.poll(wgpu::PollType::Poll);
        match armed.try_recv() {
            Ok(Ok(())) => {}
            // Still in flight. Not an error and not a dropped frame — the buffer
            // stays armed and the next frame asks again.
            Err(TryRecvError::Empty) => return None,
            // Mapping failed, or the callback was dropped without firing. Either
            // way this cycle is over: disarm so the next frame records afresh.
            Ok(Err(_)) | Err(TryRecvError::Disconnected) => {
                self.armed = None;
                return None;
            }
        }
        self.armed = None;
        let slice = self.buffer.slice(..);
        let image = slice.get_mapped_range().ok().map(|mapped| CaptureImage {
            width: self.width,
            height: self.height,
            rgba: capture::unpad_rows(&mapped, self.width, self.height, self.padded_bpr),
        });
        // Unmapped whether or not the range was readable: a buffer left mapped
        // is one this readback can never record into again.
        self.buffer.unmap();
        image
    }

    /// Fill the tap from `preview` and record the copy out of it, if the buffer
    /// is free.
    ///
    /// Two recorded steps rather than one: the blit scales and letterboxes the
    /// show into the tap's fixed shape, and the copy that follows reads a
    /// texture whose extent has not moved since this readback was opened.
    ///
    /// Returns whether it recorded, which the caller needs: [`arm`](Self::arm)
    /// must be called after the submission if and only if this did.
    pub(super) fn record(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        preview: &preview::PreviewTarget,
    ) -> bool {
        if self.armed.is_some() || !self.tap.record_fill_from(device, encoder, preview) {
            return false;
        }
        capture::record_copy(
            encoder,
            self.tap.texture(),
            &self.buffer,
            self.padded_bpr,
            self.width,
            self.height,
        );
        true
    }

    /// Ask for the mapping, after the submission carrying the recorded copy.
    ///
    /// The callback only sends; every decision is taken on the display thread
    /// when it next polls, so nothing wgpu calls back into does work.
    pub(super) fn arm(&mut self) {
        let (tx, rx) = std::sync::mpsc::channel();
        self.buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |res| {
                let _ = tx.send(res);
            });
        self.armed = Some(rx);
    }
}

/// The renderer-facing half: opening and closing the readback, and the two steps
/// a frame drawn through the intermediate performs.
impl Renderer {
    /// Open a non-blocking readback of the preview, yielding `width`x`height`
    /// frames.
    ///
    /// Requires an open preview — the blit that fills the readback's tap samples
    /// that intermediate and has nothing to read without one. The **size is the
    /// caller's** and is answered for the life of the readback: a renderer
    /// resize rebuilds the intermediate under the blit and moves nothing here
    /// (ADR-0187).
    ///
    /// Calling it again replaces the readback, which is how a caller changes the
    /// size it asked for.
    ///
    /// Refused when the frames would come out at a format no consumer can be
    /// told the order of, so the announcement that follows can always be true
    /// (ADR-0187).
    pub fn open_preview_readback(&mut self, width: u32, height: u32) -> Result<(), RenderError> {
        let Some(preview) = self.preview.as_ref() else {
            return Err(RenderError::CaptureReadback);
        };
        let format = preview.format();
        if PixelOrder::of(format).is_none() {
            return Err(RenderError::UnnameablePixelOrder(format));
        }
        self.preview_readback = Some(PreviewReadback::new(
            &self.ctx.device,
            format,
            width,
            height,
        ));
        Ok(())
    }

    /// Close the readback and free its staging buffer. A closed readback yields
    /// nothing and costs the frame one `Option` test.
    pub fn close_preview_readback(&mut self) {
        self.preview_readback = None;
    }

    /// The size of the frames the readback yields, or `None` when it is closed.
    ///
    /// **The size the caller asked for, and not the output's.** The frames are a
    /// scaled, letterboxed copy of the intermediate rather than an exact one, so
    /// this is a fixed property of the open readback: it answers the same pair
    /// across every resize, and a consumer told it once never has to be told
    /// again.
    pub fn preview_readback_size(&self) -> Option<(u32, u32)> {
        self.preview_readback.as_ref().map(PreviewReadback::size)
    }

    /// Take the frame the readback produced, if one has landed.
    ///
    /// A frame is available at most every other call in the steady state, and
    /// `None` means "not yet" rather than "never": the caller sends what it gets
    /// and does not wait.
    pub fn take_preview_frame(&mut self) -> Option<CaptureImage> {
        self.preview_frame.take()
    }

    /// The consume-then-record half, before the frame's submission.
    ///
    /// Returns whether a copy was recorded, which decides whether
    /// [`arm_preview_readback`](Self::arm_preview_readback) runs after it.
    pub(super) fn step_preview_readback(&mut self, encoder: &mut wgpu::CommandEncoder) -> bool {
        let Self {
            ctx,
            preview,
            preview_readback,
            preview_frame,
            ..
        } = self;
        let (Some(readback), Some(preview)) = (preview_readback.as_mut(), preview.as_ref()) else {
            return false;
        };
        // Consumed first: the buffer cannot be recorded into while it is mapped,
        // so this frame's copy is only possible once the previous one has been
        // taken. A frame the caller never collected is replaced rather than
        // queued — the newest picture is the one a preview wants.
        if let Some(image) = readback.consume(&ctx.device) {
            *preview_frame = Some(image);
        }
        readback.record(&ctx.device, encoder, preview)
    }

    /// Ask for the mapping, after the submission that carried the copy.
    pub(super) fn arm_preview_readback(&mut self) {
        if let Some(readback) = self.preview_readback.as_mut() {
            readback.arm();
        }
    }
}
