//! Where one side of the frame is encoded: the backdrop pre-pass, the scene, the
//! post chain folded down over it, and the two owners of that sequence --
//! [`CompositeSide`], which holds one side's chain and its layer scene, and
//! [`Terminal`], the display-referred pair only the active preset routes.

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

/// Encode one preset's composite into `destination`: the backdrop pre-pass (which
/// owns `destination`'s clear), then the scene, then the chain folded down over
/// it. Returns the draw calls. Call after [`evaluate_preset`] has routed this
/// side's params.
///
/// The backdrop paints **`destination`, not the chain's input** (ADR-0055), so the
/// chain never folds `bg_*` and its last stage composites over the backdrop with
/// premultiplied alpha. When no stage is active `target.view` *is* `destination`,
/// which makes that path bit-for-bit what it always was.
///
/// The side's `under`-join layer scene, when its preset declares one (ADR-0090
/// / Plan 0076), draws into the **same** scene target after the main scene,
/// through the same `ViewTransform` aspect, so the two layers share every
/// downstream stage and fuse into one substance. One extra draw — no new pass,
/// no new target — and a layerless side encodes exactly what this function
/// always encoded. The scene lives on `side`, one per preset, so
/// whichever side is drawn brings the right layer with it.
pub(super) fn composite_into(
    ctx: &RenderContext,
    scene: &mut Box<dyn Scene>,
    side: &mut CompositeSide,
    encoder: &mut wgpu::CommandEncoder,
    destination: &wgpu::TextureView,
    surface: (u32, u32),
    exposure: f32,
) -> u32 {
    // The stop this side's light will be shown at, handed to the chain before it
    // folds (ADR-0080). Only the bloom bright-pass reads it, and only to decide
    // what counts as over-range; nothing here applies it — the tonemap still does,
    // downstream.
    side.chain.set_exposure(exposure);
    let target = side.chain.begin(encoder, destination, surface);
    // `surface`, never `target.size` on the line below: the backdrop paints
    // `destination`, which is surface-sized, while `target.size` is the chain's
    // quantized capped internal grid (ADR-0037). The ramp's angle is only true in
    // screen pixels if it takes the shape it is actually seen at.
    side.background
        .render(&ctx.queue, encoder, destination, surface);
    // Hand the scene its target size before it renders: a scene with an internal
    // accumulation field (the attractor's trails) sizes that field from here rather
    // than a fixed grid (Plan 0027 Phase 2). A no-op for every other scene, and a
    // cheap unchanged-compare for the attractor.
    scene.set_target_size(target.size.0, target.size.1);
    // `occlude` reaches whichever pass lands on the backdrop, and that is the
    // chain's last stage whenever one is active (ADR-0085). With an empty chain
    // the scene draws straight onto `destination` and owns the seam itself, so the
    // factor goes to the scene *instead* — never to both, which would apply it
    // twice. A scene that presents premultiplied (reaction-diffusion, attractor,
    // fragment field) consumes it; the additive families ignore it, their colour
    // blend having no occlusion to scale.
    // With the `over` junction live the scene renders into the blend's chain
    // input — a scratch — so the seam belongs to the junction's final fold,
    // never to the scene (Plan 0076 Phase 3).
    let scene_in_scratch = target.routing.scene_stage().is_some() || side.chain.layer_over_active();
    scene.set_occlude(if scene_in_scratch {
        post::DEFAULT_OCCLUDE
    } else {
        side.chain.occlude()
    });
    scene.render(&ctx.queue, encoder, &target.view, target.aspect);
    // The layer scene. `under`: into the same target as the main scene, after
    // it, under the same seam rules — the target's own size and aspect (never
    // a grid's, ADR-0037) and the same `occlude` answer, because the two land
    // on the same backdrop through the same last fold (ADR-0085). `over`: into
    // the layer's own surface-sized offscreen, crisp, with the junction owning
    // the backdrop seam — so the scene gets the scratch answer (no occlusion
    // to scale in a transparent offscreen).
    let mut layer_draws = 0;
    if let Some(layer_scene) = side.layer.as_mut() {
        match side.chain.layer_input(encoder, surface) {
            Some(layer_view) => {
                layer_scene.set_target_size(surface.0, surface.1);
                layer_scene.set_occlude(post::DEFAULT_OCCLUDE);
                layer_scene.render(&ctx.queue, encoder, &layer_view, target.aspect);
            }
            None => {
                layer_scene.set_target_size(target.size.0, target.size.1);
                layer_scene.set_occlude(if scene_in_scratch {
                    post::DEFAULT_OCCLUDE
                } else {
                    side.chain.occlude()
                });
                layer_scene.render(&ctx.queue, encoder, &target.view, target.aspect);
            }
        }
        layer_draws = 1;
    }
    // The backdrop and the scene(s), plus whatever the active chain stages
    // encode on their way down.
    2 + layer_draws
        + side
            .chain
            .resolve(&ctx.queue, encoder, target.routing, destination, surface)
}

/// The two engine-wide passes a preset's bindings reach **outside** the chain:
/// the exposure/tonemap boundary (ADR-0046) and the terminal ink remap
/// (ADR-0032).
///
/// Bundled because they are routed together and withheld together — a dual-live
/// dissolve's outgoing side gets neither, each being one pass over the *blended*
/// frame rather than one per side. A borrow pair rather than two more arguments
/// on [`evaluate_preset`], which is already at the lint's limit.
pub(super) struct Terminal<'a> {
    pub(super) tonemap: &'a mut Tonemap,
    pub(super) ink: &'a mut Ink,
}

/// One preset's private composite: the background pre-pass it owns plus the post
/// chain its look folds through.
///
/// Bundled because a dual-live dissolve needs **two**, with fully independent GPU
/// state. That independence is not a nicety: `Queue::write_buffer` writes are
/// applied before the submission's commands run, so two passes in one frame
/// sharing one uniform buffer would *both* read the second write — the first
/// side's backdrop would silently take the second side's `bg_*`. Two sides, two
/// buffers, no ordering hazard. (Plan 0030 already proved the chain half of this
/// with `post.rs::two_chains_against_one_device_accumulate_independently`.)
///
/// `Background` stays outside the [`PostChain`](post::PostChain) per ADR-0031 —
/// it is a pre-pass that owns the frame clear — so the pairing lives here rather
/// than in the chain.
pub(super) struct CompositeSide {
    pub(super) background: Background,
    pub(super) chain: PostChain,
    /// This side's `[layer]` scene, **constructed for the preset it draws**
    /// (ADR-0090 point 4, Plan 0076 Phase 2) — never a roster instance, so
    /// same-system pairs are legal and two dissolving sides' layers share
    /// nothing. `Some` exactly when that preset declares a `[layer]`;
    /// [`Renderer::configure_active_scene`] maintains the invariant on every
    /// preset change. Living here rather than on the renderer gives it the
    /// side's own lifetime for free: the outgoing side keeps its layer alive
    /// through a dissolve, and promotion at finalize carries it over.
    pub(super) layer: Option<Box<dyn Scene>>,
}

impl CompositeSide {
    /// `format` is [`COMPOSITE_FORMAT`], never the surface's: everything a side
    /// paints lands upstream of the tonemap, in linear light (ADR-0046).
    pub(super) fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        tier: &TierConfig,
    ) -> Self {
        Self {
            background: Background::new(device, format),
            chain: PostChain::new(device, format, tier),
            layer: None,
        }
    }

    /// Reset every stage's params to their defaults (once per frame, before this
    /// side's preset bindings are routed).
    pub(super) fn reset_params(&mut self) {
        self.background.reset_params();
        self.chain.reset_params();
    }

    /// Drop the lazily-built GPU resources (capture rebuild — keeps a headless
    /// capture a pure function of its inputs, NFR §6). The layer scene goes
    /// with them: `configure_active_scene` reconstructs it from its
    /// deterministic seed, which is exactly the rebuild the roster scenes get
    /// from `scenes::create_all` on the same path.
    pub(super) fn reset_resources(&mut self) {
        self.background.reset_resources();
        self.chain.reset_resources();
        self.layer = None;
    }
}

/// The outgoing preset's own live frame, for a dual-live dissolve only.
///
/// It draws through the side that has followed it since **before** the switch,
/// so its trail keeps accumulating rather than restarting, and into the same
/// texture the snapshot lives in -- dual-live simply overwrites it each frame,
/// so a latch to freeze holds the last live picture instead of jumping back to
/// the one the dissolve opened on.
pub(super) struct Outgoing<'a> {
    pub(super) transition: Option<&'a Transition>,
    pub(super) roster: &'a Roster,
    pub(super) blend: &'a mut Blend,
    pub(super) scenes: &'a mut SceneRoster,
    pub(super) side: &'a mut CompositeSide,
    pub(super) smoother: &'a mut ParamSmoother,
    pub(super) layer_smoother: &'a mut ParamSmoother,
    pub(super) latches: &'a mut LatchBank,
}

/// Encode the outgoing side. Returns its draw calls, or `0` when the dissolve
/// has no outgoing preset, no snapshot target, or no scene for that system.
pub(super) fn encode_outgoing_side(
    ctx: &RenderContext,
    encoder: &mut wgpu::CommandEncoder,
    surface: (u32, u32),
    out: Outgoing<'_>,
    shared: &mut SideInputs<'_>,
) -> u32 {
    let Outgoing {
        transition,
        roster,
        blend,
        scenes,
        side,
        smoother,
        layer_smoother,
        latches,
    } = out;
    // The outgoing preset and the routes resolved for it come from the same
    // index, so the two cannot drift apart.
    let outgoing = transition
        .map(Transition::outgoing_index)
        .and_then(|index| Some((roster.presets.get(index)?, roster.routes_for(index))));
    let (Some((outgoing, out_routes)), Some(out_view)) = (outgoing, blend.snapshot_view(surface))
    else {
        return 0;
    };
    let Some(out_scene) = scene_for_mut(scenes, outgoing.system) else {
        return 0;
    };
    // No terminal: the outgoing side's `exposure`/`ink_*` were held at the
    // capture frame and are crossfaded by the single engine-wide pass of each.
    // Its own layer keeps animating through the dissolve exactly as its main
    // scene does, which is the second half `evaluate_side` covers.
    evaluate_side(
        Side {
            active: Active {
                preset: outgoing,
                routes: out_routes,
            },
            scene: out_scene,
            composite: side,
            terminal: None,
            smoother,
            layer_smoother,
            latches,
        },
        shared,
    );
    // **The outgoing preset's OWN held stop, not the crossfaded one**
    // (ADR-0080). Two reasons, and the first is structural: the crossfade in
    // `encode_active_side` cannot have run yet, because it interpolates towards
    // the incoming preset's `exposure` and that is not routed until then. The
    // second is that this is the right answer anyway -- the outgoing side's
    // bright-pass should keep selecting what its author aimed it at for as long
    // as the side is drawn; fading it out is the blend's job, not the
    // threshold's.
    let out_exposure = transition
        .and_then(Transition::outgoing_exposure)
        .unwrap_or(tonemap::DEFAULT_EXPOSURE);
    composite_into(
        ctx,
        out_scene,
        side,
        encoder,
        &out_view,
        surface,
        out_exposure,
    )
}

/// The three engine-wide passes between the composite and the surface, plus the
/// dissolve blend that sits between the chain and the tonemap.
pub(super) struct DisplayTail<'a> {
    pub(super) blend: &'a mut Blend,
    pub(super) tonemap: &'a mut Tonemap,
    pub(super) ink: &'a mut Ink,
    pub(super) transition: Option<&'a Transition>,
}

/// The active preset's side: the incoming one during a dissolve, the only one
/// otherwise. Unlike [`Outgoing`] it carries no terminal of its own -- the
/// terminal is built here, from [`DisplayTail`]'s own tonemap and ink, because
/// the same two objects are then resolved over the frame.
pub(super) struct ActiveSide<'a> {
    pub(super) active: Active<'a>,
    pub(super) scene: &'a mut Box<dyn Scene>,
    pub(super) composite: &'a mut CompositeSide,
    pub(super) smoother: &'a mut ParamSmoother,
    pub(super) layer_smoother: &'a mut ParamSmoother,
    pub(super) latches: &'a mut LatchBank,
}

/// Evaluate the active preset, then encode its composite and resolve the
/// display-referred tail over it. Returns the draw calls the whole sequence
/// cost.
pub(super) fn encode_active_side(
    ctx: &RenderContext,
    encoder: &mut wgpu::CommandEncoder,
    view: &wgpu::TextureView,
    surface: (u32, u32),
    side: ActiveSide<'_>,
    display_tail: DisplayTail<'_>,
    shared: &mut SideInputs<'_>,
) -> u32 {
    let ActiveSide {
        active,
        scene,
        composite: live_side,
        smoother,
        layer_smoother,
        latches,
    } = side;
    let DisplayTail {
        blend,
        tonemap,
        ink,
        transition,
    } = display_tail;
    let mut draw_calls = 0;

    tonemap.reset_params();
    ink.reset_params();
    // The only side that routes a terminal: `exposure` and the `ink_*` pair are
    // engine-wide, so they follow the active preset and are crossfaded below
    // rather than being bound twice.
    evaluate_side(
        Side {
            active,
            scene,
            composite: live_side,
            terminal: Some(Terminal {
                tonemap: &mut *tonemap,
                ink: &mut *ink,
            }),
            smoother,
            layer_smoother,
            latches,
        },
        shared,
    );
    // Ink is one engine-wide pass over the *blended* frame (ADR-0028), but a
    // dissolve has two presets each binding their own `ink_*`/`paper_*`. Lerp
    // the params — not two remapped frames, which is non-linear — so `t = 0` is
    // exactly the outgoing look and `t = 1` exactly the incoming one, with no
    // snap at either end. On the capture frame the outgoing preset is still the
    // active one, so its values are already correct and nothing is held yet.
    if let Some(tr) = transition.as_ref() {
        let t = tr.progress();
        if let Some(from) = tr.outgoing_ink() {
            ink.crossfade_from(from, t);
        }
        // `exposure` has the same problem for the same reason: one pass over a
        // frame that is a mix of two presets cannot show two stops, so it shows
        // the mix. Without this, a preset binding `exposure` would pop a stop
        // on the single frame the roster flips.
        if let Some(from) = tr.outgoing_exposure() {
            tonemap.crossfade_from(from, t);
        }
    }

    // The display-referred tail is resolved *first*, outermost inwards, because
    // each pass's input is the previous one's output and the outermost is the
    // only one whose destination is known: ink folds into the surface, the
    // tonemap folds into ink's input (or the surface, with ink off), and
    // everything linear targets the tonemap's input.
    let ink_input = if ink.active() {
        ink.begin(surface)
    } else {
        None
    };
    let display = ink_input.as_ref().unwrap_or(view);

    // The linear terminal: where the composite stops. Unlike ink this is never
    // skipped — it is the format boundary (ADR-0046). If it somehow cannot
    // build its target, fall through to `display` and take the old clipped
    // 8-bit composite rather than dropping the frame.
    let tonemap_input = tonemap.begin(surface);
    let terminal = tonemap_input.as_ref().unwrap_or(display);

    // Where the active side resolves. While a dissolve runs the blend sits
    // between the chain and the tonemap, so it feeds one of the blend's two
    // inputs: the outgoing target on the opening frame, the live target on
    // every frame after. If the blend cannot build its targets, fall through to
    // the terminal view — a cut, never a blend of undefined pixels.
    let blend_input = match transition.as_ref() {
        Some(tr) if tr.needs_snapshot() => blend.snapshot_view(surface),
        Some(_) => blend.live_view(surface),
        None => None,
    };
    let destination = blend_input.as_ref().unwrap_or(terminal);

    // After the crossfade above, so a dissolve's bright-pass thresholds against
    // the stop the tonemap will actually apply to this frame rather than the
    // incoming preset's endpoint (ADR-0080).
    draw_calls += composite_into(
        ctx,
        scene,
        live_side,
        encoder,
        destination,
        surface,
        tonemap.applied_exposure(),
    );
    if let (Some(tr), true) = (transition.as_ref(), blend_input.is_some()) {
        // Mix the outgoing side with the live incoming one into the tonemap's
        // input. At t = 0 this is the outgoing frame exactly, which is what lets
        // the opening frame present through the same pass before the live side
        // has ever been rendered into.
        draw_calls += blend.resolve(&ctx.queue, encoder, terminal, tr.progress(), tr.kind());
    }
    if tonemap_input.is_some() {
        draw_calls += tonemap.resolve(&ctx.queue, encoder, display);
    }
    if ink_input.is_some() {
        draw_calls += ink.resolve(&ctx.queue, encoder, view);
    }

    draw_calls
}

/// The on-canvas passes that sit on top of the finished frame: the queued text
/// runs, then the diagnostics overlay. Returns their draw calls.
///
/// The overlay draws **last** so it sits on top of the text when both are on.
pub(super) struct OnCanvas<'a> {
    #[cfg(feature = "text")]
    pub(super) text_layer: &'a mut TextLayer,
    pub(super) diag: &'a Diag,
    pub(super) overlay: &'a mut Overlay,
    /// The tier the overlay prints, and whether the governor put it there --
    /// which is what tells a demoted floor from a pinned one.
    pub(super) tier: Tier,
    pub(super) tier_demoted: bool,
}

pub(super) fn encode_on_canvas(
    ctx: &RenderContext,
    encoder: &mut wgpu::CommandEncoder,
    view: &wgpu::TextureView,
    surface: (u32, u32),
    on_canvas: OnCanvas<'_>,
) -> u32 {
    let OnCanvas {
        #[cfg(feature = "text")]
        text_layer,
        diag,
        overlay,
        tier,
        tier_demoted,
    } = on_canvas;
    let (width, height) = surface;
    let mut draw_calls = 0;
    // On-canvas text (browse overlay / HUD): a second pass that loads the
    // scene and composites the queued runs on top, in the same frame
    // (ADR-0009). Standalone-only via the `text` feature; when both this and
    // the diagnostics overlay are on, the overlay draws last so it sits on
    // top of the text.
    #[cfg(feature = "text")]
    {
        if text_layer.prepare(&ctx.device, &ctx.queue, width, height) {
            // Load: composite over the scene already in the view.
            let mut pass = gpu::color_pass(encoder, "rlx-text-pass", view, wgpu::LoadOp::Load);
            text_layer.render(&mut pass);
            draw_calls += 1;
        }
    }

    if diag.overlay_enabled() {
        let metrics = diag.metrics();
        overlay.render(
            &ctx.queue,
            encoder,
            view,
            (width, height),
            metrics,
            diag.analysis(),
            tier,
            tier_demoted,
            diag.stats().samples().map(|s| s * 1000.0),
        );
        draw_calls += 1;
    }
    draw_calls
}
