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

    /// Advance preset `name` under a single constant `frame` and read back only
    /// the frames named in `at_frames` (Plan 0085 Phase 1) — the **long-run**
    /// primitive, and the one a horizon needs.
    ///
    /// Its two siblings cannot serve a run of tens of thousands of frames:
    /// [`capture_preset`](Self::capture_preset) reseeds from scratch on every
    /// call, so sampling *k* points costs `O(k·N)` renders, and
    /// [`capture_preset_over`](Self::capture_preset_over) reads back *every*
    /// frame, so a ten-minute run at 720p would materialize ~36,000 images. This
    /// renders `N` frames once and holds `at_frames.len()` of them.
    ///
    /// Frame numbering matches [`capture_audio`](Self::capture_audio): frame 0
    /// is the first advanced frame, so `at_frames = [n - 1]` returns exactly what
    /// `capture_preset(name, frame, n)` returns — asserted in
    /// `core/tests/capture_advance.rs` rather than argued, because it is the
    /// property that lets a horizon's rows be compared with every other capture
    /// this repo takes.
    ///
    /// Deterministic on the same terms as its siblings: scenes are rebuilt to
    /// their seed, the clock resets to `0.0`, and the step is a fixed
    /// [`FALLBACK_DT`](scenes::FALLBACK_DT) — so a row at index *k* does not
    /// depend on how far the run was asked to go. Images come back in
    /// `at_frames` order; a repeated index yields the same frame twice rather
    /// than rendering it twice. An empty `at_frames` renders nothing.
    ///
    /// **Off the hot path** — it blocks on a GPU readback per requested frame.
    /// The readback buffer is built **once, at the first requested frame**, and
    /// reused for every later one. Both halves of that matter on the DX12
    /// software adapter, where building GPU resources mid-sequence perturbs what
    /// the feedback stages resolve to (the hazard
    /// [`capture_preset_over`](Self::capture_preset_over) documents, and a
    /// horizon is precisely a long feedback sequence): reusing it means the
    /// perturbation happens once rather than per sample, and doing it at the
    /// first sample rather than up front is what puts the allocation at the same
    /// point in the sequence [`capture_preset`](Self::capture_preset) puts it —
    /// which is what makes the two agree pixel-for-pixel on WARP as well as on
    /// hardware. It also stays independent of the horizon requested, since the
    /// first sample sits at the same frame index however long the run is.
    pub fn capture_preset_at(
        &mut self,
        name: &str,
        frame: &AnalysisFrame,
        at_frames: &[u32],
    ) -> Result<Vec<CaptureImage>, RenderError> {
        self.reset_for_capture(name)?;
        let Some(&last) = at_frames.iter().max() else {
            return Ok(Vec::new());
        };

        let (width, height) = (self.ctx.config.width, self.ctx.config.height);
        let format = self.ctx.surface_format();
        let (texture, view) = capture::create_target(&self.ctx.device, format, width, height);
        let mut readback: Option<(wgpu::Buffer, u32)> = None;

        let mut captured: Vec<(u32, CaptureImage)> = Vec::with_capacity(at_frames.len());
        for index in 0..=last {
            self.time += scenes::FALLBACK_DT;
            if !at_frames.contains(&index) {
                self.step_offscreen(frame, &view, width, height, scenes::FALLBACK_DT);
                continue;
            }
            let slot = readback
                .get_or_insert_with(|| capture::create_readback(&self.ctx.device, width, height));
            let (buffer, padded_bpr) = (&slot.0, slot.1);
            let mut encoder =
                self.ctx
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("lmv-capture-at"),
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
            capture::record_copy(&mut encoder, &texture, buffer, padded_bpr, width, height);
            self.ctx.queue.submit(std::iter::once(encoder.finish()));

            #[cfg(feature = "text")]
            self.text_layer.end_frame();

            let img = capture::read_back(&self.ctx.device, buffer, width, height, padded_bpr)?;
            captured.push((index, img));
        }

        at_frames
            .iter()
            .map(|idx| {
                captured
                    .iter()
                    .find(|(i, _)| i == idx)
                    .map(|(_, img)| img.clone())
                    .ok_or(RenderError::CaptureReadback)
            })
            .collect()
    }

    /// Render preset `name` for `frames` frames at an **injected** `dt`, handing
    /// each frame to `sink` the moment it is read back (Plan 0101 / ADR-0114) —
    /// the **streaming** primitive, and the one an offline video render needs.
    ///
    /// Its three siblings all return a `Vec<CaptureImage>`, which is exactly what
    /// a video render cannot afford: a 1080p frame is 8.29 MB, so a four-minute
    /// track at 60 fps is 119 GB of retained images. Nothing is retained here —
    /// the frame is handed to `sink` and dropped, so the resident set of a
    /// 14,400-frame render is the same as a 100-frame one.
    ///
    /// It is also the only capture entry point whose step is **not** the fixed
    /// [`FALLBACK_DT`](scenes::FALLBACK_DT). A render at `--fps 30` advances the
    /// scene by 1/30 s a frame, or the visuals would run at half speed against
    /// their own soundtrack; that `dt` is the caller's, exactly as it is for the
    /// live frontend (ADR-0013). At 60 fps `dt` *is* `FALLBACK_DT`, which is what
    /// makes a rendered frame comparable with every other capture this repo takes.
    ///
    /// `analysis` supplies the [`AnalysisFrame`] for each frame index. The audio
    /// hop clock and the frame clock are different clocks and only the caller
    /// knows the mapping between them, so this deliberately does not walk PCM —
    /// unlike [`capture_audio`](Self::capture_audio), which welds one rendered
    /// frame to one analysis hop.
    ///
    /// Deterministic on the same terms as its siblings: scenes rebuilt to their
    /// seed, the clock reset to `0.0`, the salt pinned. Given a deterministic
    /// `analysis` the whole run is a pure function of `(name, frames, dt)`.
    ///
    /// **Off the hot path** — it blocks on a GPU readback every frame, which is
    /// also what bounds its memory: `read_back` polls, so each frame's submission
    /// is retired before the next is encoded (the retention Plan 0099 measured).
    /// The target and the readback buffer are built **once** and reused, so a
    /// long run allocates no GPU resources mid-sequence.
    ///
    /// A `sink` error stops the run and comes back as
    /// [`RenderError::Sink`](RenderError::Sink) carrying the consumer's own
    /// message.
    pub fn capture_stream(
        &mut self,
        name: &str,
        frames: u32,
        dt: f32,
        analysis: &mut dyn FnMut(u32) -> AnalysisFrame,
        sink: &mut dyn FnMut(u32, &CaptureImage) -> Result<(), String>,
    ) -> Result<(), RenderError> {
        self.reset_for_capture(name)?;

        let (width, height) = (self.ctx.config.width, self.ctx.config.height);
        let format = self.ctx.surface_format();
        let (texture, view) = capture::create_target(&self.ctx.device, format, width, height);
        let (buffer, padded_bpr) = capture::create_readback(&self.ctx.device, width, height);

        for index in 0..frames {
            let frame = analysis(index);
            self.time += dt;
            let mut encoder =
                self.ctx
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("lmv-capture-stream"),
                    });
            capture::record_clear(&mut encoder, &view);
            let _ = self.draw_frame(
                &frame,
                &mut encoder,
                &view,
                width,
                height,
                dt,
                SaltMode::Pinned,
            );
            capture::record_copy(&mut encoder, &texture, &buffer, padded_bpr, width, height);
            self.ctx.queue.submit(std::iter::once(encoder.finish()));

            #[cfg(feature = "text")]
            self.text_layer.end_frame();

            let img = capture::read_back(&self.ctx.device, &buffer, width, height, padded_bpr)?;
            sink(index, &img).map_err(RenderError::Sink)?;
        }
        Ok(())
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
    ///
    /// **It polls, and that is what bounds the memory of a long run** (Plan
    /// 0099). Nothing else here reads anything back, so before this line the
    /// only `device.poll` in the whole capture path was
    /// [`capture::read_back`]'s — meaning wgpu got no opportunity to retire a
    /// completed submission between two *sampled* frames. A horizon at the
    /// default 30 s interval is 1,800 consecutive unpolled submits, and the
    /// retention is per **pass**, not per pixel: measured over one such stretch
    /// on the Windows dev box (hardware adapter, debug, 96x96), a
    /// reaction-diffusion world — 12 simulation sub-steps plus a present, 13
    /// passes a frame — retained **950 KB per frame** against a captured frame
    /// of 36 KB, while single-pass worlds retained ~30 KB. That is what made
    /// the ceiling look like a property of the RD family: every world grew, RD
    /// grew ~32x faster and hit the allocator first, at ~4.4 GB.
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

        // Retire this submission's resources. `wait_indefinitely` rather than a
        // non-blocking `Poll`, and that was **measured, not assumed**: a
        // `PollType::Poll` here took the same 3,600-frame stretch from
        // 3,668 MB to 3,188 MB and no further, because a headless loop submits
        // far faster than the GPU drains and a non-blocking poll finds almost
        // nothing complete to retire. Waiting is what makes the retention
        // per-frame instead of per-run.
        //
        // It is the same `poll(Wait)` `capture::read_back` already performs at
        // every sampled frame, so this path pays what the sampled path always
        // paid — and this whole file is off the hot path by construction (see
        // the module docs); nothing in the frame loop or behind the C ABI
        // reaches it.
        //
        // The result is discarded for the same reason `draw_frame`'s is above —
        // this returns nothing, and a poll that fails means the device is gone,
        // which the next `read_back` reports as `CaptureReadback` rather than
        // letting the run pass silently.
        let _ = self.ctx.device.poll(wgpu::PollType::wait_indefinitely());

        #[cfg(feature = "text")]
        self.text_layer.end_frame();
    }
}
