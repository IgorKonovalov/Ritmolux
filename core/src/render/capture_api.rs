//! The capture API — `Renderer`'s headless, off-hot-path entry points
//! (Plan 0013), carved out of `render/mod.rs` by Plan 0061 Phase 3.
//!
//! **This is dev-tooling API, not app API.** Nothing in the standalone's frame
//! loop and nothing behind the C ABI calls into this file: its callers are the
//! `shot` example and `core/tests/`. Every entry point here blocks on a GPU
//! readback, so calling one from a live loop is a stutter by construction.
//!
//! It is a second `impl Renderer` block rather than a separate type, because the
//! methods are public API whose paths must not move — `Renderer::capture_preset`
//! is spelled the same before and after this split.

// Hot-path panic-denial pragma (Plan 0002 Phase 2; render/ is scanned by the
// hygiene guard). These paths are off the frame loop, but they share
// `Renderer`'s state and the pragma travels with the code, not with the file.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

// A continuation of one `impl` block that was split across two files, so it
// needs the same names `render/mod.rs` has in scope. Enumerating them would be a
// list to keep in sync with a file whose whole purpose is to be the other half
// of this one.
use super::*;

/// What one [`Renderer::capture_audio_after_warmup`] run produced.
///
/// The two fields beside the images exist so a caller can check *what the run
/// did* rather than infer it from a stopwatch (Plan 0084 Phase 3): `analysis`
/// is the analyzer state the run walked through, and `rendered` is how much of
/// it reached a rasterizer.
pub struct AudioCapture {
    /// The requested frames, in `at_frames` order.
    pub images: Vec<CaptureImage>,
    /// One published [`AnalysisFrame`] per hop, in hop order. Independent of
    /// whether the hop was rendered — which is the property that makes feeding
    /// warm-up hops without pixels safe, and is asserted rather than argued
    /// (`core/tests/capture_advance.rs`).
    pub analysis: Vec<AnalysisFrame>,
    /// How many frames were rasterized. Zero when every hop is a warm-up hop.
    pub rendered: usize,
}

impl Renderer {
    /// Advance the scene clock one step and capture that single frame into an
    /// offscreen texture, returning tight RGBA (Plan 0013). Off the hot path —
    /// blocks on GPU readback; never call it from a live loop.
    pub fn capture_frame(&mut self, frame: &AnalysisFrame) -> Result<CaptureImage, RenderError> {
        self.time += scenes::FALLBACK_DT;
        self.capture_at_clock(frame)
    }

    /// Draw the active preset for `frame` at the **current** clock into a fresh
    /// offscreen texture and read it back. Does not advance the clock, so
    /// callers that already stepped it share this. The whole path (clear → draw
    /// → copy → map) is deterministic for a given `(preset, frame, clock)`.
    fn capture_at_clock(&mut self, frame: &AnalysisFrame) -> Result<CaptureImage, RenderError> {
        let (width, height) = (self.ctx.config.width, self.ctx.config.height);
        let format = self.ctx.surface_format();
        let (texture, view) = capture::create_target(&self.ctx.device, format, width, height);
        let (buffer, padded_bpr) = capture::create_readback(&self.ctx.device, width, height);
        let mut encoder = self
            .ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("lmv-capture-frame"),
            });
        capture::record_clear(&mut encoder, &view);
        let _ = self.draw_frame(
            frame,
            &mut encoder,
            &view,
            width,
            height,
            scenes::FALLBACK_DT,
            SaltMode::Pinned,
        );
        capture::record_copy(&mut encoder, &texture, &buffer, padded_bpr, width, height);
        self.ctx.queue.submit(std::iter::once(encoder.finish()));

        #[cfg(feature = "text")]
        self.text_layer.end_frame();

        capture::read_back(&self.ctx.device, &buffer, width, height, padded_bpr)
    }

    /// Capture preset `name` after advancing it `frames` steps from a fixed
    /// initial state, driven by a single constant `frame` (Plan 0013). A **pure
    /// function** of `(name, frame, frames)`: the scenes are rebuilt so any
    /// stateful system (e.g. the seeded swarm particles) starts from its
    /// deterministic seed, and the scene clock resets to `0.0`, so the result is
    /// independent of any earlier capture. Errors if `name` is not in the
    /// roster. `frames` is treated as at least 1.
    pub fn capture_preset(
        &mut self,
        name: &str,
        frame: &AnalysisFrame,
        frames: u32,
    ) -> Result<CaptureImage, RenderError> {
        self.reset_for_capture(name)?;

        let (width, height) = (self.ctx.config.width, self.ctx.config.height);
        let format = self.ctx.surface_format();
        let (texture, view) = capture::create_target(&self.ctx.device, format, width, height);

        // Warm the scene through the first frames-1 steps (state advances, pixels
        // discarded); then capture the final frame.
        let n = frames.max(1);
        for _ in 1..n {
            self.time += scenes::FALLBACK_DT;
            self.step_offscreen(frame, &view, width, height, scenes::FALLBACK_DT);
        }
        self.time += scenes::FALLBACK_DT;

        let (buffer, padded_bpr) = capture::create_readback(&self.ctx.device, width, height);
        let mut encoder = self
            .ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("lmv-capture-preset"),
            });
        capture::record_clear(&mut encoder, &view);
        let _ = self.draw_frame(
            frame,
            &mut encoder,
            &view,
            width,
            height,
            scenes::FALLBACK_DT,
            SaltMode::Pinned,
        );
        capture::record_copy(&mut encoder, &texture, &buffer, padded_bpr, width, height);
        self.ctx.queue.submit(std::iter::once(encoder.finish()));

        #[cfg(feature = "text")]
        self.text_layer.end_frame();

        capture::read_back(&self.ctx.device, &buffer, width, height, padded_bpr)
    }

    /// Capture preset `name` across a **time-varying** stimulus (Plan 0037):
    /// one rendered frame per entry of `stimulus`, read back in order, so the
    /// returned images are the response *while it changes* rather than after it
    /// settles.
    ///
    /// This is the primitive [`capture_preset`](Self::capture_preset) cannot be:
    /// holding one frame for every step converges every smoother before the
    /// pixels are read, which makes the result identical for any `[smoothing]`
    /// constant (ADR-0039). `capture_preset` is left exactly as it was — four
    /// suites and `--report` consume it — and this is its sibling, sharing the
    /// same [`reset_for_capture`](Self::reset_for_capture) seed so both are pure
    /// functions of their arguments.
    ///
    /// The clock advances one [`FALLBACK_DT`](scenes::FALLBACK_DT) per entry, so
    /// index `i` is second `i * dt` of the response. An empty `stimulus` yields
    /// no images. Errors if `name` is not in the roster.
    ///
    /// **Off the hot path, and more so than its sibling** — it blocks on a GPU
    /// readback *per frame*, not once per call. The target and readback buffer
    /// are allocated up front rather than per frame, because building GPU
    /// resources mid-sequence perturbs what the feedback stages resolve to on the
    /// DX12 software adapter.
    pub fn capture_preset_over(
        &mut self,
        name: &str,
        stimulus: &[AnalysisFrame],
    ) -> Result<Vec<CaptureImage>, RenderError> {
        self.reset_for_capture(name)?;

        let (width, height) = (self.ctx.config.width, self.ctx.config.height);
        let format = self.ctx.surface_format();
        let (texture, view) = capture::create_target(&self.ctx.device, format, width, height);
        let (buffer, padded_bpr) = capture::create_readback(&self.ctx.device, width, height);

        let mut images = Vec::with_capacity(stimulus.len());
        for frame in stimulus {
            self.time += scenes::FALLBACK_DT;
            let mut encoder =
                self.ctx
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("lmv-capture-over"),
                    });
            capture::record_clear(&mut encoder, &view);
            let _ = self.draw_frame(
                frame,
                &mut encoder,
                &view,
                width,
                height,
                scenes::FALLBACK_DT,
                SaltMode::Pinned,
            );
            capture::record_copy(&mut encoder, &texture, &buffer, padded_bpr, width, height);
            self.ctx.queue.submit(std::iter::once(encoder.finish()));

            #[cfg(feature = "text")]
            self.text_layer.end_frame();

            images.push(capture::read_back(
                &self.ctx.device,
                &buffer,
                width,
                height,
                padded_bpr,
            )?);
        }
        Ok(images)
    }

    /// Select `name` and reset every stateful system to its deterministic seed —
    /// the shared preamble of [`capture_preset`](Self::capture_preset) and
    /// [`capture_preset_over`](Self::capture_preset_over), so both are pure
    /// functions of their arguments and neither inherits an earlier capture's
    /// history.
    fn reset_for_capture(&mut self, name: &str) -> Result<(), RenderError> {
        if !self.select_preset_by_name_now(name) {
            return Err(RenderError::UnknownPreset(name.to_string()));
        }
        self.scenes = scenes::create_all(&self.ctx.device, COMPOSITE_FORMAT, &self.tier);
        self.cancel_transition();
        self.side.reset_resources();
        self.tonemap.reset_resources();
        self.ink.reset_resources();
        self.blend.reset_resources();
        self.time = 0.0;
        // The rebuilt scenes are fresh — re-apply the active preset's structural
        // config (ADR-0007) so a line scene captures with its geometry built.
        self.configure_active_scene();
        Ok(())
    }

    /// Drive preset `name` with **real audio through the real analyzer** and
    /// capture the frames at `at_frames` (Plan 0013). The PCM is fed hop-by-hop
    /// into a fresh [`Analyzer`](crate::dsp::Analyzer) (format validated at the
    /// intake boundary — the source-agnostic rule); each produced
    /// [`AnalysisFrame`] drives one rendered frame, so `at_frames` indexes the
    /// hop sequence (frame 0 is the first hop). Deterministic: scenes are rebuilt
    /// to their seed and the clock resets to 0, exactly like
    /// [`capture_preset`](Self::capture_preset).
    ///
    /// This is in-memory PCM only — no file, decoder, or OS audio-source code,
    /// just like a frontend pushing samples. Returned images are in `at_frames`
    /// order; an index past the audio length is an error.
    pub fn capture_audio(
        &mut self,
        name: &str,
        pcm: &[f32],
        format: AudioFormat,
        at_frames: &[u32],
    ) -> Result<Vec<CaptureImage>, RenderError> {
        Ok(self
            .capture_audio_after_warmup(name, pcm, format, at_frames, 0)?
            .images)
    }

    /// [`capture_audio`](Self::capture_audio), with the first `warmup_hops` hops
    /// **advanced but not rasterized** (Plan 0084 Phase 3).
    ///
    /// A warm-up hop still pushes its samples, still publishes its
    /// [`AnalysisFrame`], and still advances the scene clock by one
    /// [`FALLBACK_DT`](scenes::FALLBACK_DT) — the hop happened, it just did not
    /// draw. What it skips is the render pass, which is why a caller that only
    /// needs the analyzer warm (`core/tests/reactivity.rs` drives
    /// `WARMUP_HOPS` of them per capture, at silence, and reads none of them
    /// back) stops paying a full rasterization per hop to reach a DSP state that
    /// needs no pixels.
    ///
    /// **This does not warm GPU-side scene state.** Analysis is a pure function
    /// of its window and the render pass never touches the analyzer, so the
    /// published frames are bit-for-bit what they would have been — but a scene
    /// that *integrates* on the GPU (particles, trails, reaction-diffusion) has
    /// that many fewer steps behind it at the first rendered hop. Time-driven
    /// scenes are unaffected, since the clock advances either way.
    ///
    /// An `at_frames` entry inside the warm-up span was never rendered and is an
    /// error, the same one an index past the audio length gives.
    pub fn capture_audio_after_warmup(
        &mut self,
        name: &str,
        pcm: &[f32],
        format: AudioFormat,
        at_frames: &[u32],
        warmup_hops: usize,
    ) -> Result<AudioCapture, RenderError> {
        if !self.select_preset_by_name_now(name) {
            return Err(RenderError::UnknownPreset(name.to_string()));
        }
        let mut analyzer = crate::dsp::Analyzer::new(format).map_err(RenderError::AudioFormat)?;

        self.scenes = scenes::create_all(&self.ctx.device, COMPOSITE_FORMAT, &self.tier);
        self.cancel_transition();
        self.side.reset_resources();
        self.tonemap.reset_resources();
        self.ink.reset_resources();
        self.blend.reset_resources();
        self.time = 0.0;
        self.configure_active_scene();

        let (width, height) = (self.ctx.config.width, self.ctx.config.height);
        let target_format = self.ctx.surface_format();
        let (texture, view) =
            capture::create_target(&self.ctx.device, target_format, width, height);

        let hop_samples = crate::dsp::HOP_SIZE * format.channels as usize;
        let mut captured: Vec<(u32, CaptureImage)> = Vec::with_capacity(at_frames.len());
        let mut published: Vec<AnalysisFrame> = Vec::new();
        let mut rendered = 0usize;

        for (index, hop) in pcm.chunks(hop_samples).enumerate() {
            let frame_index = index as u32;
            analyzer.push_interleaved(hop);
            let analysis = analyzer.take_frame();
            self.time += scenes::FALLBACK_DT;
            published.push(analysis);

            if index < warmup_hops {
                continue;
            }
            rendered += 1;

            let wanted = at_frames.contains(&frame_index)
                && !captured.iter().any(|(i, _)| *i == frame_index);
            if wanted {
                let (buffer, padded_bpr) =
                    capture::create_readback(&self.ctx.device, width, height);
                let mut encoder =
                    self.ctx
                        .device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("lmv-capture-audio"),
                        });
                capture::record_clear(&mut encoder, &view);
                let _ = self.draw_frame(
                    &analysis,
                    &mut encoder,
                    &view,
                    width,
                    height,
                    scenes::FALLBACK_DT,
                    SaltMode::Pinned,
                );
                capture::record_copy(&mut encoder, &texture, &buffer, padded_bpr, width, height);
                self.ctx.queue.submit(std::iter::once(encoder.finish()));
                #[cfg(feature = "text")]
                self.text_layer.end_frame();
                let img = capture::read_back(&self.ctx.device, &buffer, width, height, padded_bpr)?;
                captured.push((frame_index, img));
            } else {
                self.step_offscreen(&analysis, &view, width, height, scenes::FALLBACK_DT);
            }
        }

        let images = at_frames
            .iter()
            .map(|idx| {
                captured
                    .iter()
                    .find(|(i, _)| i == idx)
                    .map(|(_, img)| img.clone())
                    .ok_or(RenderError::CaptureReadback)
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(AudioCapture {
            images,
            analysis: published,
            rendered,
        })
    }

    /// Draw one frame into `view` and submit it — advancing scene state without
    /// reading anything back. The warm-up step [`capture_preset`] uses to reach
    /// frame `N`.
    fn step_offscreen(
        &mut self,
        frame: &AnalysisFrame,
        view: &wgpu::TextureView,
        width: u32,
        height: u32,
        dt: f32,
    ) {
        let mut encoder = self
            .ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("lmv-capture-step"),
            });
        capture::record_clear(&mut encoder, view);
        let _ = self.draw_frame(
            frame,
            &mut encoder,
            view,
            width,
            height,
            dt,
            SaltMode::Pinned,
        );
        self.ctx.queue.submit(std::iter::once(encoder.finish()));

        #[cfg(feature = "text")]
        self.text_layer.end_frame();
    }
}
