//! The frame, stage by stage: what happens between `Scene::render` being called
//! and the command buffer being submitted.
//!
//! Five encode stages -- warp, deposit, the draw layer, the blur chain and
//! present -- plus the two that feed them: `ensure_resources` and the per-frame
//! uniform upload. **The call order is the contract, not a reading order**: the
//! deposit lands on the warped past, the draw layer lands on the deposit, and
//! the swap that makes the result "the past" happens after all three. Reordering
//! two stages is a picture change.
//!
//! Free functions rather than methods, for the reason
//! [`particles::encode`](crate::render::scenes::particles) records: a stage that
//! needs the resources and the scene's own params at once has the caller
//! destructure `self`, which a method taking `&mut self` forbids.

// Hot-path panic-denial pragma (Plan 0002 Phase 2; render/ is scanned by the
// hygiene guard). Encodes every displayed frame.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

// A continuation of one module split across five files, so it needs the names
// `warp_mesh/mod.rs` has in scope.
use super::*;

/// Build or rebuild the GPU resources if they are stale, clear a fresh field and
/// flush the palette LUTs.
///
/// Returns whether the scene has resources to render with; `false` means the
/// caller returns without encoding anything.
pub(super) fn ensure_resources(
    scene: &mut WarpMeshScene,
    queue: &wgpu::Queue,
    encoder: &mut wgpu::CommandEncoder,
    size: (u32, u32),
) -> bool {
    // Build or rebuild: a fresh scene, a resized target, a preset whose
    // grid differs from the one the buffers were built for, or one whose
    // translated shaders differ (Plan 0100 Phase 6).
    let shader_key = scene
        .shader_spec
        .as_ref()
        .map_or(0, shader::ShaderSpec::key);
    let stale = match scene.res.as_ref() {
        None => true,
        Some(res) => {
            res.size != size || res.mesh != scene.state.mesh || res.shader_key != shader_key
        }
    };
    if stale {
        let mut built = Resources::build(
            &scene.device,
            queue,
            scene.surface_format,
            size,
            scene.state.mesh,
            scene.max_segments,
            scene.shader_spec.as_ref(),
            shader_key,
        );
        // A fresh pair's textures are empty and hold the default palette;
        // hand it the one the scene is actually carrying.
        built.luts.set(&scene.palette);
        scene.res = Some(built);
    }
    let Some(res) = scene.res.as_mut() else {
        return false;
    };

    if res.needs_clear {
        res.encode_clear(encoder);
        // The blur chain's textures start undefined too, and a custom warp
        // shader may sample them on the very first frame.
        if let Some(milk_shaders) = res.milk_shaders.as_ref() {
            milk_shaders.encode_clear(encoder);
        }
        queue.write_buffer(
            &res.indices,
            0,
            bytemuck::cast_slice(&build_indices(res.mesh)),
        );
        res.needs_clear = false;
    }
    res.luts.flush(queue);
    true
}

/// Evaluate a converted preset's per-vertex program over the grid, then assemble
/// this frame's vertex buffer from the result and the scalar fallbacks.
pub(super) fn prepare_mesh(scene: &mut WarpMeshScene, aspect: f32) {
    // A converted preset's per-vertex program, evaluated over the whole grid
    // here — after the ordinary `[per_vertex]` bindings, which it replaces.
    // Allocation-free: the series live in the scene's own arrays, sized when
    // the grid was.
    if scene
        .milk
        .as_ref()
        .is_some_and(crate::milk::MilkRuntime::has_per_vertex)
    {
        let (mx, my) = scene.state.mesh;
        let mut v = 0usize;
        for row in 0..=my {
            for col in 0..=mx {
                let (x, y, _, _) = vertex_position(col, row, (mx, my), aspect);
                // Only the uv goes over: the runtime computes MilkDrop's own
                // `rad`/`ang` from it, which are normalized differently from
                // the native pair `vertex_position` returns. See
                // `MilkRuntime::run_vertex` for the factor and why it matters.
                let outputs = match scene.milk.as_mut() {
                    Some(runtime) => runtime.run_vertex(x, y),
                    None => break,
                };
                for (index, value) in outputs.iter().enumerate() {
                    if let Some(slot) = scene.state.values.get_mut(index).and_then(|s| s.get_mut(v))
                    {
                        *slot = *value;
                    }
                }
                v += 1;
            }
        }
        scene.state.bound = [true; OUTPUTS];
    }

    // Assemble and upload this frame's mesh.
    scene.state.assemble(&scene.scalars);
}

/// Upload the mesh and the three per-pass uniform blocks, plus the
/// converted-shader block when the preset carries one.
pub(super) fn upload_uniforms(
    scene: &WarpMeshScene,
    res: &Resources,
    queue: &wgpu::Queue,
    aspect: f32,
    size: (u32, u32),
    dt: f32,
) {
    queue.write_buffer(
        &res.vertices,
        0,
        bytemuck::cast_slice(&scene.state.vertices),
    );
    // `decay` is a factor per second, like `fb_zoom`, and is clamped below 1
    // so the field cannot integrate without bound.
    let decay = scene.decay.clamp(0.0, MAX_DECAY).powf(dt);
    queue.write_buffer(
        &res.warp_uniform,
        0,
        bytemuck::bytes_of(&WarpUniform {
            misc: [aspect, dt, scene.time, scene.warp_scale],
            misc2: [
                decay,
                scene.warp_phase.get(),
                f32::from(scene.wrap >= 0.5),
                scene.darken_center.clamp(0.0, 1.0) * DARKEN_CENTER_STRENGTH,
            ],
            misc3: [scene.quantize_steps, 0.0, 0.0, 0.0],
        }),
    );
    queue.write_buffer(
        &res.deposit_uniform,
        0,
        bytemuck::bytes_of(&DepositUniform {
            a: [
                aspect,
                scene.deposit * dt,
                scene.deposit_radius,
                scene.deposit_width,
            ],
            b: [
                scene.deposit_x,
                scene.deposit_y,
                scene.deposit_arms,
                scene.deposit_twist,
            ],
            c: [
                scene.deposit_phase.get(),
                scene.colour.hue + scene.color_center,
                scene.color_span,
                scene.colour.saturation,
            ],
            d: [
                scene.colour.mix,
                palette::band_steps(scene.colour.steps),
                palette::band_contour(scene.colour.contour),
                0.0,
            ],
        }),
    );
    queue.write_buffer(
        &res.present_uniform,
        0,
        bytemuck::bytes_of(&PresentUniform {
            a: [scene.colour.brightness, scene.occlude, scene.gamma, 0.0],
            b: [
                f32::from(scene.brighten >= 0.5),
                f32::from(scene.darken >= 0.5),
                f32::from(scene.solarize >= 0.5),
                f32::from(scene.invert >= 0.5),
            ],
            // **The orientation is quantized here, on the CPU.** It is one of
            // four states in the source format, but it arrives as an `f32`
            // that a per-frame program — or a preset's own smoothing — can
            // sweep continuously through the values between them. Rounding
            // to the nearest state on this side keeps the shader from having
            // to decide what 1.5 means.
            c: {
                let orient = echo_orientation(scene.echo_orient);
                [
                    scene.echo_alpha.clamp(0.0, 1.0),
                    scene.echo_zoom,
                    f32::from(orient & 1 != 0),
                    f32::from(orient & 2 != 0),
                ]
            },
        }),
    );
    // The converted-shader uniform, filled from the same frame the EEL
    // programs saw (Plan 0100 Phase 6). One buffer serves both the warp
    // pass (pre-swap) and the comp pass (post-swap).
    if let (Some(milk_shaders), Some(runtime)) = (res.milk_shaders.as_ref(), scene.milk.as_ref()) {
        // The *unclamped* per-second decay: a custom warp shader applies
        // decay itself, and the reference's bound is its 8-bit target — both
        // ends of which the shader epilogue now reproduces. The clamp is the
        // ceiling; `rlx_quantize`, driven by the step count passed here, is
        // the floor that makes a `decay`-scaled dim pixel reach zero instead
        // of integrating forever (ADR-0118).
        queue.write_buffer(
            &milk_shaders.uniform,
            0,
            bytemuck::bytes_of(&shader::fill_uniform(
                runtime,
                scene.time,
                dt,
                size,
                aspect,
                scene.decay,
                scene.colour.brightness,
                scene.occlude,
                scene.quantize_steps,
            )),
        );
    }
}

pub(super) fn encode_warp(res: &Resources, encoder: &mut wgpu::CommandEncoder) {
    // --- warp: the past, resampled through the mesh, into the write half ---
    let warp_bg = if res.field.reading_a() {
        &res.warp_bg_a
    } else {
        &res.warp_bg_b
    };
    {
        // The mesh covers the whole target, but clearing first makes the
        // pass independent of what the buffer held two frames ago — which
        // is what keeps a capture a pure function of its inputs.
        let mut pass = gpu::color_pass(
            encoder,
            "warp-mesh-warp-pass",
            res.field.write_view(),
            wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
        );
        // A converted warp shader replaces the built-in decay fragment; the
        // vertex stage — the mesh transform — is shared, so `uv` reaching
        // the custom fragment is exactly the warped source uv (Phase 6).
        let custom_warp = res.milk_shaders.as_ref().and_then(|milk_shaders| {
            milk_shaders.warp_pipeline.as_ref().map(|pipeline| {
                let bg = if res.field.reading_a() {
                    &milk_shaders.bind_a
                } else {
                    &milk_shaders.bind_b
                };
                (pipeline, bg)
            })
        });
        match custom_warp {
            Some((pipeline, milk_bg)) => {
                pass.set_pipeline(pipeline);
                pass.set_bind_group(0, warp_bg, &[]);
                pass.set_bind_group(1, milk_bg, &[]);
            }
            None => {
                pass.set_pipeline(&res.warp_pipeline);
                pass.set_bind_group(0, warp_bg, &[]);
            }
        }
        pass.set_vertex_buffer(0, res.vertices.slice(..));
        pass.set_index_buffer(res.indices.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..res.index_count, 0, 0..1);
    }
}

pub(super) fn encode_deposit(res: &Resources, encoder: &mut wgpu::CommandEncoder) {
    // --- deposit: this frame's light, onto the warped past ---
    {
        let mut pass = gpu::color_pass(
            encoder,
            "warp-mesh-deposit-pass",
            res.field.write_view(),
            wgpu::LoadOp::Load,
        );
        pass.set_pipeline(&res.deposit_pipeline);
        pass.set_bind_group(0, &res.deposit_bg, &[]);
        pass.draw(0..3, 0..1);
    }
}

/// The draw layer, then the swap that makes the composed frame the next frame's
/// past. The swap closes this stage rather than opening the next one because
/// everything above writes the same half of the field.
pub(super) fn encode_draw_layer(
    scene: &mut WarpMeshScene,
    queue: &wgpu::Queue,
    encoder: &mut wgpu::CommandEncoder,
    aspect: f32,
    dt: f32,
) {
    // The resources and the scene's own geometry at once, which is what the
    // free-function shape buys.
    let WarpMeshScene {
        res,
        geometry,
        milk,
        frame,
        draw: draw_out,
        time,
        ..
    } = scene;
    let (time, draw_out) = (*time, *draw_out);
    let Some(res) = res.as_mut() else {
        return;
    };
    // --- the draw layer: what MilkDrop draws between the warp and the
    // composite (Plan 0100 Phase 4). Onto the SAME target the deposit went
    // to, so this frame's strokes are warped from the next frame onward —
    // which is what makes a waveform leave a trail.
    if let Some(out) = draw_out {
        draw::build(
            geometry,
            milk.as_mut(),
            &out,
            &frame.waveform,
            time,
            dt,
            aspect,
        );
    } else {
        geometry.clear();
    }
    if !geometry.triangles.is_empty() {
        let count = geometry.triangles.len().min(res.shape_capacity);
        if let Some(drawn) = geometry.triangles.get(..count) {
            queue.write_buffer(&res.shape_vertices, 0, bytemuck::cast_slice(drawn));
            queue.write_buffer(
                &res.shape_uniform,
                0,
                bytemuck::bytes_of(&ShapeUniform {
                    v: [aspect, 0.0, 0.0, 0.0],
                }),
            );
            let mut pass = gpu::color_pass(
                encoder,
                "warp-mesh-shape-pass",
                res.field.write_view(),
                wgpu::LoadOp::Load,
            );
            // Two draws over one buffer, split at the partition the draw
            // layer built: additive instances first, then the over-blended
            // ones on top of them. See `draw`'s module docs for why both
            // exist.
            let split = geometry.triangles_additive.min(count) as u32;
            pass.set_bind_group(0, &res.shape_bind_group, &[]);
            pass.set_vertex_buffer(0, res.shape_vertices.slice(..));
            if split > 0 {
                pass.set_pipeline(&res.shape_pipeline);
                pass.draw(0..split, 0..1);
            }
            if count as u32 > split {
                pass.set_pipeline(&res.shape_over_pipeline);
                pass.draw(split..count as u32, 0..1);
            }
        }
    }
    if !geometry.segments.is_empty() {
        // One batch for the whole layer — the waveform, every custom wave,
        // every shape outline, both borders and the motion grid — because
        // colour and width are per segment and only `glow` and `softness`
        // are per draw. The split is the same partition the shapes above use.
        //
        // `softness` is MILKDROP_SOFTNESS and not the line default: this
        // surface is judged against foo_vis_milk2, not against Plan 0114 -
        // see the constant.
        //
        // `StrokeMetric::Clip` is the same argument, one space over, and it
        // is a DATED DEFERRAL rather than a decision (ADR-0160). Every other
        // producer strokes in world space, where a half-width is a thickness
        // on screen at any orientation; this one keeps the clip metric, in
        // which a vertical stroke is `aspect` times thicker than a
        // horizontal one. It stays because Plan 0142 is taking foo_vis_milk2
        // readings on this surface and moving the instrument between the
        // question and the answer is a mistake, not a tradeoff.
        //
        // **Revisit at Plan 0142's close**, and note the open possibility
        // that the clip metric is CORRECT here rather than merely deferred:
        // MilkDrop authors in a square space and its own stroke may be
        // anisotropic on screen too, in which case matching it is
        // compatibility. Nobody has measured that against the reference rig.
        res.lines.draw_split(
            queue,
            encoder,
            res.field.write_view(),
            aspect,
            1.0,
            MILKDROP_SOFTNESS,
            lines::StrokeMetric::Clip,
            lines::ViewTransform::default(),
            &geometry.segments,
            geometry.segments_additive,
        );
    }

    // The fresh state becomes the next frame's past.
    res.field.swap();
}

pub(super) fn encode_blur(res: &Resources, encoder: &mut wgpu::CommandEncoder) {
    // --- the blur chain (Phase 6): from the frame just composed, for the
    // comp pass now and the next frame's warp pass ---
    if let Some(milk_shaders) = res.milk_shaders.as_ref() {
        milk_shaders.encode_blur(encoder, res.field.reading_a());
    }
}

pub(super) fn encode_present(
    res: &Resources,
    encoder: &mut wgpu::CommandEncoder,
    view: &wgpu::TextureView,
) {
    // --- present: the field, over the backdrop ---
    let custom_comp = res.milk_shaders.as_ref().and_then(|milk_shaders| {
        milk_shaders.comp_pipeline.as_ref().map(|pipeline| {
            let bg = if res.field.reading_a() {
                &milk_shaders.bind_a
            } else {
                &milk_shaders.bind_b
            };
            (pipeline, bg)
        })
    });
    let present_bg = if res.field.reading_a() {
        &res.present_bg_a
    } else {
        &res.present_bg_b
    };
    let mut pass = gpu::color_pass(encoder, "warp-mesh-present-pass", view, wgpu::LoadOp::Load);
    // A converted comp shader replaces the built-in remap stack whole — it
    // *is* MilkDrop's composite, gamma and echo and all, in the preset's
    // own arithmetic.
    match custom_comp {
        Some((pipeline, milk_bg)) => {
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, milk_bg, &[]);
        }
        None => {
            pass.set_pipeline(&res.present_pipeline);
            pass.set_bind_group(0, present_bg, &[]);
        }
    }
    pass.draw(0..3, 0..1);
}
