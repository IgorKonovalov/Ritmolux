//! The frame, step by step (Plan 0061 Phase 6, continuing Plan 0031 Phase 5).
//!
//! The uniform upload and the four `encode_*` passes: what actually happens
//! between `Scene::render` being called and the command buffer being submitted.
//!
//! Free functions rather than methods, unchanged from when Plan 0031 lifted them
//! out of a 228-line `render`: the caller destructures `self` to borrow the
//! resources and the params at once, which a method taking `&mut self` forbids.
//! They were already marked off by their own section banner inside `mod.rs`;
//! this file is that banner made structural.

// Hot-path panic-denial pragma (Plan 0002 Phase 2; render/ is scanned by the
// hygiene guard).
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

// A continuation of one module split across four files, so it needs the names
// `particles/mod.rs` has in scope.
use super::*;

/// One frame's uniform inputs, gathered so [`upload_uniforms`] takes one argument
/// rather than fourteen. Plain values read off the scene's params.
pub(super) struct UniformInputs {
    /// The **render target's** aspect, not the accumulation grid's — see the note
    /// in [`upload_uniforms`].
    pub(super) aspect: f32,
    pub(super) coeffs: [f32; 4],
    pub(super) family: AttractorFamily,
    /// The active roster entry's framing (ADR-0093) — where this tuple's figure
    /// is and how big, which is what makes a distant tuple reachable at all.
    /// Read instead of `family.projection()`, which is entry 0's.
    pub(super) framing: Framing,
    /// The integrated spin in spin-scaled seconds, not a wall clock — read from
    /// the scene's [`Phase`](crate::render::scenes::Phase).
    pub(super) spin_time: f32,
    pub(super) dt: f32,
    /// How many fixed steps this frame will encode — one uniform slot each.
    pub(super) pending_steps: u32,
    /// The index of the first of them (ADR-0075's map-choice salt).
    pub(super) step_index: u32,
    /// This frame's resolved IFS affine table, or [`IfsPacked::ZERO`] on a map
    /// family. Resolved by the caller because it depends on the cached morph
    /// ends rather than on the family alone.
    pub(super) ifs: IfsPacked,
    /// This frame's IFS framing as `(centre, half-extent)`, sampled from the fit
    /// LUT at the current `morph`. `None` on a map family, which keeps the
    /// single world scale [`AttractorFamily::projection`] hands out.
    pub(super) ifs_frame: Option<([f32; 2], [f32; 2])>,
    pub(super) size: f32,
    pub(super) hue: f32,
    /// The raw bound `brightness`; sanitized by [`brightness_factor`] where it is
    /// packed, not here, so the guard has exactly one site.
    pub(super) brightness: f32,
    pub(super) fade: f32,
    /// This frame's `fb_*` transform and the preset's `[feedback]` table
    /// (ADR-0048) — the shared vocabulary, applied to this scene's own trail
    /// field. See `upload_uniforms` for why the aspect above is the one it uses.
    pub(super) feedback_transform: feedback::Transform,
    pub(super) feedback: feedback::FeedbackConfig,
    /// How much of the cloud's coverage the backdrop resolves against (ADR-0085),
    /// carried to the present pass through the decay uniform's second component.
    /// Not a named param — the renderer sets it on the scene every frame.
    pub(super) occlude: f32,
    pub(super) hue_spread: f32,
    pub(super) hue_center: f32,
    pub(super) saturation: f32,
    pub(super) palette_mix: f32,
    /// Hard palette bands (ADR-0078), already quantized to an integer. No
    /// `palette_contour` counterpart: this scene's LUT read is in the VERTEX
    /// stage, which has no derivatives, and a point sprite has one palette
    /// coordinate — so there is no gradient across it to contour.
    pub(super) palette_steps: f32,
    pub(super) zoom: f32,
    pub(super) pan: [f32; 2],
    pub(super) perspective: f32,
    pub(super) depth_fade: f32,
    pub(super) depth_hue: f32,
    /// ADR-0087's last-map channel, at its two routes. Both reach the draw
    /// uniform unclamped: the palette LUT sampler repeats, so any coordinate
    /// shift is legitimate, and `shift_hue` takes `fract` of its argument.
    pub(super) map_tint: f32,
    pub(super) map_hue: f32,
    /// ADR-0088's root channel, at both routes. Unclamped here for the same
    /// reason: the LUT sampler repeats. The *particle's* value is what gets
    /// clamped, in the shader, and that is a different number.
    pub(super) root_tint: f32,
    pub(super) root_hue: f32,
    /// The emergence ramp's length in **steps**, raw. Sanitized by
    /// [emergence_rate] where it is packed, not here, so the guard has exactly
    /// one site — the discipline `brightness` follows.
    pub(super) emergence: f32,
}

/// The deferred one-shot uploads: the palette LUTs on a preset switch or fresh
/// build, the field clear after a (re)build, and the seeded particle scatter on
/// first build or a `reseed` rising edge. Each clears its own flag, so none
/// repeats per frame.
#[allow(clippy::too_many_arguments)]
pub(super) fn flush_deferred_uploads(
    queue: &wgpu::Queue,
    encoder: &mut wgpu::CommandEncoder,
    pipelines: &PipelineResources,
    grid: &FieldResources,
    seed_particles: &[Particle],
    palette: &Palette,
    palette_dirty: &mut bool,
    needs_clear: &mut bool,
    needs_upload: &mut bool,
) {
    // Upload the active palette LUTs (A + B) on a preset switch or a fresh
    // build — off the hot path, once per change.
    if *palette_dirty {
        palette::write_lut(queue, &pipelines.lut_texture_a, &palette.lut_a_bytes());
        palette::write_lut(queue, &pipelines.lut_texture_b, &palette.lut_b_bytes());
        *palette_dirty = false;
    }

    // Clear the trail field once after a (re)build so the first decay reads
    // black rather than garbage.
    if *needs_clear {
        grid.clear_field(encoder);
        *needs_clear = false;
    }
    // (Re)upload the seeded scatter — on first build and on a family change. A
    // A `reseed` does **not** come through here: it disturbs the live cloud on
    // the GPU instead (ADR-0066), because re-uploading this array *replaces*
    // the cloud with a uniform box rather than scattering it.
    if *needs_upload {
        queue.write_buffer(
            &pipelines.particles,
            0,
            bytemuck::cast_slice(seed_particles),
        );
        *needs_upload = false;
    }
}

/// This frame's three uniform buffers: the compute step's coefficients, the
/// point draw's projection and colour, and the trail decay factor.
///
/// The step uniform is written **once per sub-step this frame owes**, into its
/// own slot, so each dispatch carries its own `step_index` — see [`STEP_SLOTS`].
/// At the steady 60 fps that is one write, exactly as before.
pub(super) fn upload_uniforms(
    queue: &wgpu::Queue,
    pipelines: &PipelineResources,
    active: u32,
    inputs: &UniformInputs,
) {
    let packed = inputs.ifs;
    for slot in 0..inputs.pending_steps.min(MAX_SUBSTEPS) {
        queue.write_buffer(
            &pipelines.step_uniform,
            u64::from(pipelines.step_stride * slot),
            bytemuck::bytes_of(&StepUniform::new(
                inputs.coeffs,
                inputs.family.shader_id(),
                // The **active** count, not the allocated one: this is the bound
                // the compute step early-returns against, so it is what leaves
                // the tail beyond `density` untouched (ADR-0069).
                active,
                0,
                inputs.step_index.wrapping_add(slot),
                packed,
            )),
        );
    }
    let (scale, dim, centre) = inputs.framing.projection;
    // An IFS takes its framing from the fit instead — measured over `morph` at
    // load, so the figure stays in the frame as it crosses from one figure to
    // another, and **aspect-aware**, so a wide figure fits a portrait window
    // rather than hanging out of it.
    //
    // The aspect handed in is the render **target's**, not the trail grid's
    // (ADR-0037): the present stretches the grid over the whole target, so the
    // grid's own aspect cancels and using it would draw the figure the wrong
    // width.
    let (scale, centre) = match inputs.ifs_frame {
        Some((c, half)) => (ifs::fit_scale(half, inputs.aspect), [c[0], c[1], 0.0]),
        None => (scale, centre),
    };
    let ([hx, hy, hz], [vx, vy, vz]) = inputs.family.basis().masks();
    // Off the count actually drawn, not off the tier and not off the buffer that
    // was allocated: the draw below issues `active` instances, and normalizing
    // against anything else would be a claim about a different draw call. This is
    // what carries ADR-0065's invariance across `density` (ADR-0069).
    //
    // `brightness` (ADR-0080) rides the same slot rather than a second uniform
    // field: both are scalars on the very same additive weight, and the shader
    // has no reason to tell "how many particles are sharing this light" apart
    // from "how bright the figure is". At the default the factor is exactly
    // `1.0`, so this line is the identity and no existing capture moves.
    let deposit = deposit_scale(active) * brightness_factor(inputs.brightness);
    queue.write_buffer(
        &pipelines.draw_uniform,
        0,
        bytemuck::bytes_of(&DrawUniform {
            v: [
                // The **target's** aspect, not the accumulation field's (Plan
                // 0029 Phase 5). The points are projected into the field, but
                // the present stretches the field over the whole target with
                // aspect ignored, so field NDC `x` becomes target NDC `x` and
                // the field's own aspect cancels out. Using the grid ratio was
                // harmless only while quantization was absent and the grid
                // equalled the target; with a 256 px step a 1920x1080 target
                // takes a 2048x1280 grid and drew the cloud 11% too wide.
                inputs.aspect,
                POINT_BASE * inputs.size,
                inputs.hue,
                spin_phase(inputs.spin_time),
            ],
            // `w.z` is unused since Plan 0062 — the centre grew to three
            // components and moved to `ctr` below.
            w: [scale, dim, 0.0, deposit],
            u: [
                inputs.hue_spread,
                inputs.hue_center,
                inputs.palette_mix,
                inputs.saturation,
            ],
            x: [
                inputs.zoom,
                inputs.pan[0],
                inputs.pan[1],
                if inputs.family.is_continuous() {
                    1.0
                } else {
                    0.0
                },
            ],
            bh: [hx, hy, hz, 0.0],
            bv: [vx, vy, vz, 0.0],
            d: [
                // Clamped here, silently, and not in the shader: this is the one
                // place the value crosses into the GPU, so a preset asking for
                // more gets the ceiling rather than a divisor approaching zero
                // (ADR-0076). `presets/README.md` documents that it is silent.
                inputs.perspective.clamp(0.0, MAX_PERSPECTIVE),
                // Clamped for a harder reason than a ceiling: past `1` the haze
                // multiplier goes negative, and negative light in an additive
                // accumulation *subtracts* from whatever the trail already holds.
                inputs.depth_fade.clamp(0.0, 1.0),
                // Not clamped: the palette LUT sampler repeats, so any shift is
                // a legitimate coordinate.
                inputs.depth_hue,
                inputs.framing.inv_depth_extent(inputs.family),
            ],
            ctr: [centre[0], centre[1], centre[2], 0.0],
            // The four colour channels, unclamped for the same reason
            // `depth_hue` above is: the LUT sampler repeats, so any
            // palette-coordinate shift is legitimate, and the hue route takes
            // `fract`. `map_*` is ADR-0087's, `root_*` ADR-0088's — the latter
            // took the `age_*` slots at Plan 0074 Phase 3 rather than adding to
            // them.
            //
            // **Zeroed wholesale off the IFS**, which is what makes all four
            // exactly inert on the four map families rather than merely
            // defaulted. `map` and `root` are identically `0.0` there, and
            // `channel_shift` is *centred* — so a bound `map_tint` would land a
            // uniform `-map_tint/2` on the palette coordinate of a family the
            // channel means nothing on. Inertness has to be the engine declining
            // to upload the param, the way `d.w` makes the depth cues inert on a
            // 2D family; a default of zero only covers presets that never bind it.
            //
            // The `root_*` half would survive without this branch — anchored at
            // zero, `root_tint * 0` is `0` whatever the binding — so the zeroing
            // is load-bearing for x/y and belt-and-braces for z/w. Kept whole
            // because a row that is conditionally zeroed in halves is worse to
            // reason about than one that is zeroed outright.
            ch: if inputs.family.figure().is_some() {
                [
                    inputs.map_tint,
                    inputs.map_hue,
                    inputs.root_tint,
                    inputs.root_hue,
                ]
            } else {
                [0.0; 4]
            },
            // The IFS ramps a respawned particle in; nothing else respawns, so
            // nothing else has an age at all — hence the flat floor of exactly
            // 1.0 rather than a rate they would read as zero and black
            // themselves out with. `z`/`w` are unused since the age colour
            // channel retired.
            // `z` carries `palette_steps` (ADR-0078). It has been FREE since
            // Plan 0074 Phase 3 retired the age colour channel, and it is
            // family-independent — banding is a property of the palette, not of
            // the attractor — so it is written the same in both arms rather than
            // joining the `figure()` branch above.
            em: if inputs.family.figure().is_some() {
                [
                    emergence_rate(inputs.emergence),
                    0.0,
                    palette::band_steps(inputs.palette_steps),
                    0.0,
                ]
            } else {
                [0.0, 1.0, palette::band_steps(inputs.palette_steps), 0.0]
            },
        }),
    );
    // Frame-rate-independent trail decay: retain `fade` per 1/60 s, raised to
    // the `dt`-relative power so the trail length is the same wall-clock
    // duration on any refresh. `fade = 0` -> factor 0 -> trail-free.
    let decay = inputs
        .fade
        .clamp(0.0, 1.0)
        .powf((inputs.dt * 60.0).max(0.0));
    // The ADR-0048 transform this frame's decay resamples the field through — the
    // engine's SECOND accumulation sink, packed by the same code the trails stage
    // packs with, so the two cannot disagree about what `fb_rotate` means.
    //
    // `inputs.aspect` is the RENDER TARGET's, never the trail grid's (ADR-0037):
    // the grid is quantized to a 256 px step, the present is a plain stretch, and
    // a rotation computed in grid-uv would shear.
    let moved = !inputs.feedback_transform.is_identity(inputs.feedback.warp);
    let [xf, tr, wp] =
        inputs
            .feedback_transform
            .pack(inputs.dt, inputs.aspect, inputs.feedback.warp);
    queue.write_buffer(
        &pipelines.decay_uniform,
        0,
        bytemuck::bytes_of(&DecayUniform {
            // y: `occlude` — the present pass reads it out of this same buffer
            // (ADR-0085), so one write feeds the decay and the composite seam.
            // z: whether the transform above moves anything at all; the decay
            // shader `select`s on it so an untransformed preset samples the
            // literal uv and every attractor golden is byte-identical.
            k: [decay, inputs.occlude, f32::from(u8::from(moved)), 0.0],
            xf,
            tr,
            wp,
        }),
    );
}

/// The one-shot reseed disturbance (ADR-0066): **one** dispatch of the compute
/// pipeline in [`JITTER_MODE`], kicking every particle by a bounded family-relative
/// offset derived from its own seed and the reseed counter.
///
/// Exactly one dispatch, whatever the frame's `pending_steps` is, which is why it
/// carries its own uniform: the disturbance a preset asked for must not be a
/// function of how many fixed steps this frame happened to owe.
///
/// Nothing at all is encoded when no edge is pending — a `reseed` that never fires
/// costs one boolean test per frame.
pub(super) fn encode_jitter(
    queue: &wgpu::Queue,
    encoder: &mut wgpu::CommandEncoder,
    pipelines: &PipelineResources,
    active: u32,
    framing: Framing,
    reseed_count: &u32,
    pending_jitter: &mut bool,
) {
    if !*pending_jitter {
        return;
    }
    *pending_jitter = false;

    // The **active entry's** extent, not the family's (ADR-0093): the kick is a
    // fraction of the figure's own size, and on a roster entry twice the
    // canonical figure's extent the canonical fraction would be a disturbance
    // half as strong as the one the preset asked for. This is the Plan 0062
    // coupling, and it survives the roster because framing travels with the tuple.
    let [jx, jy, jz] = framing.jitter_extent();
    let jitter_offset = pipelines.step_stride * JITTER_SLOT;
    queue.write_buffer(
        &pipelines.step_uniform,
        u64::from(jitter_offset),
        bytemuck::bytes_of(&StepUniform::new(
            [jx, jy, jz, streak_flag(RESEED_DRAWS_STREAK)],
            JITTER_MODE,
            // Active, like the step above — a reseed that kicked the inert tail
            // would move particles nothing draws, and would break the very
            // property that proves the tail is inert.
            active,
            *reseed_count,
            // The jitter is not a fixed step and draws no map — it keeps its own
            // `salt` and leaves the step counter alone.
            0,
            StepUniform::NO_IFS,
        )),
    );

    let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
        label: Some("attractor-jitter-pass"),
        timestamp_writes: None,
    });
    pass.set_pipeline(&pipelines.compute_pipeline);
    pass.set_bind_group(0, &pipelines.compute_bg, &[jitter_offset]);
    pass.dispatch_workgroups(active.div_ceil(WORKGROUP), 1, 1);
}

/// Step the particles: one compute dispatch per scheduled sub-step, each against
/// **its own** uniform slot so it carries its own `step_index` ([`STEP_SLOTS`]).
/// wgpu inserts the storage-to-vertex barrier before the draw pass that follows.
pub(super) fn encode_steps(
    encoder: &mut wgpu::CommandEncoder,
    pipelines: &PipelineResources,
    active: u32,
    pending_steps: u32,
) {
    let groups = active.div_ceil(WORKGROUP);
    // `FixedStep` already clamps to `MAX_SUBSTEPS`; clamped again because a
    // dispatch past the slots written above would read an undefined slot.
    for slot in 0..pending_steps.min(MAX_SUBSTEPS) {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("attractor-step-pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipelines.compute_pipeline);
        pass.set_bind_group(0, &pipelines.compute_bg, &[pipelines.step_stride * slot]);
        pass.dispatch_workgroups(groups, 1, 1);
    }
}

/// Trail pass: draw the faded previous accumulation into the fresh target, then
/// add this frame's points on top. **One** pass, so the decay lays the bed and the
/// additive points bloom over it — splitting it in two would clear the bed away.
/// Reads the field's current read side; the caller swaps afterwards.
pub(super) fn encode_trail_pass(
    encoder: &mut wgpu::CommandEncoder,
    pipelines: &PipelineResources,
    active: u32,
    grid: &FieldResources,
) {
    let decay_bg = if grid.field.reading_a() {
        &grid.decay_bg_a
    } else {
        &grid.decay_bg_b
    };
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("attractor-trail-pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: grid.field.write_view(),
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
    pass.set_pipeline(&pipelines.decay_pipeline);
    pass.set_bind_group(0, decay_bg, &[]);
    pass.draw(0..3, 0..1);

    pass.set_pipeline(&pipelines.draw_pipeline);
    pass.set_bind_group(0, &pipelines.draw_bg, &[]);
    pass.set_vertex_buffer(0, pipelines.particles.slice(..));
    // Clamped to what was allocated rather than trusted: `active` is derived from
    // a preset-supplied `density`, and an instance range past the buffer is an
    // out-of-bounds vertex fetch. `active_particles` already bounds it, so this
    // costs nothing and removes the possibility rather than documenting it.
    pass.draw(0..6, 0..active.min(pipelines.count));
}

/// Present the freshly-written accumulation to the target, loading over whatever
/// the engine backdrop painted (ADR-0018). Call **after** the swap.
pub(super) fn encode_present(
    encoder: &mut wgpu::CommandEncoder,
    pipelines: &PipelineResources,
    grid: &FieldResources,
    view: &wgpu::TextureView,
) {
    let present_bg = if grid.field.reading_a() {
        &grid.present_bg_a
    } else {
        &grid.present_bg_b
    };
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("attractor-present-pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                // Load over the engine backdrop (ADR-0018): the additive
                // point cloud blooms over whatever the background pass painted.
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    pass.set_pipeline(&pipelines.present_pipeline);
    pass.set_bind_group(0, present_bg, &[]);
    pass.draw(0..3, 0..1);
}
