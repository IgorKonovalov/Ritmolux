//! The post-composite chain (ADR-0031, membership revised by ADR-0032): the
//! **per-preset look** stages that run *after* the scene has drawn, folded down to
//! a caller-supplied destination in one fixed order.
//!
//! # The order, and the skip rule
//!
//! The chain is `trails -> kaleidoscope -> destination`, built as a compile-time
//! constant array in [`PostChain::new`]. That order is ADR-0018's product decision
//! (feedback before the screen-space fold). This is **not** a render graph and
//! **not** a registration point: nothing reorders the array at runtime, and the
//! only way to add a stage is to add an array element and a [`PostStage`] impl.
//!
//! Every stage is individually **skippable**: a stage whose amount param is off
//! reports [`active`](PostStage::active) `false` and is dropped from the frame
//! entirely — no offscreen, no pipeline, nothing encoded. So the routing is a
//! function of the active flags alone:
//!
//! - the background + scene render into the **first active** stage's input, or
//!   straight into the destination when no stage is active;
//! - each active stage folds into the **next active** stage's input;
//! - the **last active** stage folds into the destination.
//!
//! Skipped stages are simply not in that walk, so trails folds directly into the
//! destination when the kaleidoscope is off. That adjacency is [`route`] — a
//! **pure function** over the flags, with no GPU and no `self`, so the contract is
//! unit-testable (the tests at the bottom of this file are the composite's first
//! real coverage).
//!
//! # What is *not* in the chain
//!
//! ADR-0032's rule: **a pass a preset composes belongs in the chain; a pass that
//! applies to the finished frame belongs outside it.** So the renderer drives three
//! passes directly, around this chain:
//!
//! - [`Background`](super::background::Background) — the pre-pass that owns the
//!   frame clear and never folds a rendered frame down;
//! - the transition blend (Plan 0023) — two inputs, which a one-input
//!   [`PostStage::begin`] cannot express, and live only while a dissolve runs;
//! - [`Ink`](super::ink::Ink) — the engine-wide tone remap, which reads the one
//!   finished frame (ADR-0028) and so must run *after* the blend of two per-preset
//!   composites.
//!
//! That is why `begin`/`resolve` take their **destination** as an argument rather
//! than assuming the surface: it is the blend's input while a transition runs,
//! ink's input when ink is active, and the surface otherwise.
//!
//! # Why a value, not a set of fields
//!
//! [`PostChain`] owns its stages, so a **second chain with fully independent GPU
//! state is constructible** against the same device — each stage builds its own
//! offscreens (and, for trails, its own [`PingPongField`](super::feedback::PingPongField))
//! lazily, on first use. That is what Plan 0023's dual-live transition path needs:
//! two fully-composited frames in one frame, each side with its own feedback
//! history.

// Hot-path panic-denial pragma (Plan 0002 Phase 2; render/ is scanned by the
// hygiene guard). The chain routes and encodes every displayed frame.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use super::kaleidoscope::Kaleidoscope;
use super::trails::Trails;

/// How many stages the chain holds. A compile-time constant, not a capacity:
/// [`PostChain::new`] fills the array exactly, and [`Routing`] is sized from it so
/// a frame's routing costs no allocation.
pub(crate) const STAGE_COUNT: usize = 2;

/// Composite positions, in chain order. Named so the routing tests and the
/// ADR-0018 ordering claim read as assertions rather than magic indices.
pub(crate) const TRAILS: usize = 0;
pub(crate) const KALEIDOSCOPE: usize = 1;

/// Each stage's declared parameter vocabulary, **in chain order** — the same
/// consts the stages themselves match on, so there is no second copy to drift.
/// Pinned to the live array by a `debug_assert` in [`PostChain::new`].
///
/// This exists so a binding's owning stage can be resolved **once, at load**
/// ([`stage_for`]) instead of by walking the stages per binding per frame: the
/// answer is fixed the moment a preset is parsed, and a chained
/// `set_param(&str, ..)` fallthrough inside the hot loop is a link every new
/// stage would lengthen (Plan 0031 Phase 3).
pub(crate) const STAGE_PARAMS: [&[&str]; STAGE_COUNT] =
    [super::trails::PARAMS, super::kaleidoscope::PARAMS];

/// The chain position that owns `name`, or `None` when no stage does. **Pure** —
/// a lookup over the static vocabularies above, with no GPU and no chain
/// instance, so it is callable from load-time resolution and testable directly.
///
/// The stage namespaces are disjoint (`trails` vs `kaleido_*`), so the first
/// match is the only match — the same first-owner-wins rule the per-frame walk
/// this replaced applied.
pub(crate) fn stage_for(name: &str) -> Option<usize> {
    STAGE_PARAMS
        .iter()
        .position(|params| params.contains(&name))
}

/// One skippable post-composite stage (ADR-0031). Crate-internal on purpose: the
/// composite order is fixed in [`PostChain::new`], not registered, and no preset
/// or C-ABI caller can reach this seam.
pub(crate) trait PostStage {
    /// A short stable label for this stage — diagnostics and test assertions.
    fn name(&self) -> &'static str;

    /// Reset this stage's named params to their defaults. Called once per frame,
    /// before the active preset's bindings are routed, so an unbound param never
    /// leaks from the previous preset.
    fn reset_params(&mut self);

    /// Apply one named param, returning whether this stage owns the name. The
    /// caller routes by resolved index (see [`stage_for`]), so a `false` here
    /// means the resolution and this match disagree — which
    /// [`STAGE_PARAMS`]'s `debug_assert` in [`PostChain::new`] exists to catch.
    fn set_param(&mut self, name: &str, value: f32) -> bool;

    /// This stage's declared parameter vocabulary — the names its
    /// [`set_param`](Self::set_param) claims. Read only to pin [`STAGE_PARAMS`]
    /// to the live array at construction; never on the hot path.
    fn params(&self) -> &'static [&'static str];

    /// Whether this stage runs this frame. `false` skips it entirely — the
    /// passthrough that keeps an unbound preset paying nothing.
    fn active(&self) -> bool;

    /// This stage's fixed internal resolution, or `None` to size from the surface.
    /// The trails and kaleidoscope stages both run at a fixed 16:9 grid; the
    /// `None` arm is what a surface-sized stage would use.
    fn internal_size(&self) -> Option<(u32, u32)>;

    /// Lazily build this stage's resources and return the view its **input**
    /// renders into. `surface` is the current surface size, for a surface-sized
    /// stage. `None` means the stage could not produce a target and the caller
    /// should fall back to the surface.
    ///
    /// The view is **owned, not borrowed**: the caller renders a whole frame
    /// between `begin` and [`resolve`](Self::resolve), which a `&self` borrow
    /// could not survive. `wgpu::TextureView` is `Clone`/Arc-backed, so this is an
    /// atomic increment, not a resource copy (ADR-0031).
    fn begin(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        surface: (u32, u32),
    ) -> Option<wgpu::TextureView>;

    /// Fold this stage's input into `out`, returning the draw calls encoded — so
    /// the frame's total is a sum of what the stages report, not hand arithmetic.
    fn resolve(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        out: &wgpu::TextureView,
    ) -> u32;

    /// Drop the lazily-built resources, so the next active frame starts from a
    /// fresh (cleared) state. Used on the capture scene-rebuild to keep a headless
    /// capture a pure function of its inputs (NFR §6).
    fn reset_resources(&mut self);
}

/// One frame's composite routing: the active stages, in chain order.
///
/// Produced by [`route`] and read back as the scene's target plus the fold-down
/// [`edges`](Routing::edges). Fixed-size, so deciding a frame's routing allocates
/// nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Routing {
    steps: [usize; STAGE_COUNT],
    len: usize,
}

impl Routing {
    /// The active stage indices, in chain order.
    pub(crate) fn active_stages(&self) -> &[usize] {
        self.steps.get(..self.len).unwrap_or(&[])
    }

    /// The stage the background + scene render into, or `None` for the caller's
    /// destination (no stage is active this frame).
    pub(crate) fn scene_stage(&self) -> Option<usize> {
        self.active_stages().first().copied()
    }

    /// Each active stage paired with what it folds into: the next active stage, or
    /// `None` for the caller's destination. The last active stage always yields
    /// `None`.
    pub(crate) fn edges(&self) -> impl Iterator<Item = (usize, Option<usize>)> + '_ {
        let steps = self.active_stages();
        steps
            .iter()
            .enumerate()
            .map(move |(position, &stage)| (stage, steps.get(position + 1).copied()))
    }
}

/// **Pure**: which active stage folds into which, from the per-stage active flags
/// alone. No GPU, no `self` — this is the composite's routing contract, and it is
/// what the tests at the bottom of this file pin down.
///
/// `active[i]` is stage `i`'s [`PostStage::active`]. Flags past [`STAGE_COUNT`]
/// are ignored (the array is fixed at construction, so this cannot happen in
/// practice).
pub(crate) fn route(active: &[bool]) -> Routing {
    let mut steps = [0usize; STAGE_COUNT];
    let mut len = 0;
    for (index, &on) in active.iter().enumerate() {
        if !on {
            continue;
        }
        let Some(slot) = steps.get_mut(len) else {
            break; // more flags than stages — ignore the overflow rather than panic
        };
        *slot = index;
        len += 1;
    }
    Routing { steps, len }
}

/// Where the background + the scene render this frame — the first active stage's
/// input, or the caller's destination when the chain is entirely skipped.
pub(crate) struct SceneTarget {
    /// The view to draw into. Owned (an Arc bump) — see [`PostStage::begin`].
    pub view: wgpu::TextureView,
    /// The aspect that target implies, derived from [`size`](Self::size).
    pub aspect: f32,
    /// The pixel size a scene sees through
    /// [`Scene::set_target_size`](super::scenes::Scene::set_target_size) (ADR-0030),
    /// so a scene with an internal accumulation field matches its actual target
    /// instead of a fixed grid.
    pub size: (u32, u32),
}

/// The per-preset post-composite stages in ADR-0018 order — see the module docs.
///
/// An owned value rather than a set of `Renderer` fields, so a second instance
/// with fully independent GPU state is constructible (Plan 0023 dual-live).
pub(crate) struct PostChain {
    /// Built once in fixed order; never reordered at runtime.
    stages: [Box<dyn PostStage>; STAGE_COUNT],
}

impl PostChain {
    /// Build the chain in its fixed composite order. Every stage stores the device
    /// handle and builds its GPU resources lazily on its first active frame, so a
    /// chain that is never used costs nothing — and two chains against the same
    /// device share no resources.
    pub(crate) fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let chain = Self {
            // ADR-0018's feedback-then-fold order. Ink is deliberately absent:
            // it is an engine-wide pass on the finished frame, so it runs after
            // the chain (and after the transition blend) — ADR-0032.
            stages: [
                Box::new(Trails::new(device, surface_format)),
                Box::new(Kaleidoscope::new(device, surface_format)),
            ],
        };
        // The array above *is* the composite order, and the routing contract is
        // written against the positions below (module docs, ADR-0018). Assert they
        // still hold, so reordering the literal trips here in a debug/test build
        // instead of silently re-composing every preset.
        debug_assert_eq!(
            chain.stage_names().get(TRAILS).copied(),
            Some("trails"),
            "the feedback stage must come first (ADR-0018)"
        );
        debug_assert_eq!(
            chain.stage_names().get(KALEIDOSCOPE).copied(),
            Some("kaleidoscope"),
            "the screen-space fold runs after the feedback (ADR-0018)"
        );
        // `STAGE_PARAMS` is what load-time route resolution reads, so its order
        // has to be this array's order — otherwise a resolved `Stage(i)` would
        // hand `kaleido_*` to trails. Swapping the literal above trips here.
        for (index, stage) in chain.stages.iter().enumerate() {
            debug_assert_eq!(
                STAGE_PARAMS.get(index).copied(),
                Some(stage.params()),
                "STAGE_PARAMS[{index}] must be `{}`'s own vocabulary",
                stage.name()
            );
        }
        chain
    }

    /// Reset every stage's params to their defaults (once per frame, before the
    /// active preset's bindings are routed).
    pub(crate) fn reset_params(&mut self) {
        for stage in self.stages.iter_mut() {
            stage.reset_params();
        }
    }

    /// Apply one named param to the stage at `index` — the position load-time
    /// resolution already decided ([`stage_for`]). Returns whether that stage
    /// owned the name, so a resolution/`set_param` disagreement is visible rather
    /// than silent.
    ///
    /// Indexed rather than searched on purpose: the by-name walk this replaced ran
    /// once per bound param per frame and grew a link with every stage added
    /// (Plan 0031 Phase 3).
    pub(crate) fn set_stage_param(&mut self, index: usize, name: &str, value: f32) -> bool {
        self.stages
            .get_mut(index)
            .is_some_and(|stage| stage.set_param(name, value))
    }

    /// Drop every stage's lazily-built resources (capture rebuild — keeps a
    /// headless capture a pure function of its inputs, NFR §6).
    pub(crate) fn reset_resources(&mut self) {
        for stage in self.stages.iter_mut() {
            stage.reset_resources();
        }
    }

    /// This frame's routing, from the stages' current active flags.
    fn routing(&self) -> Routing {
        let mut active = [false; STAGE_COUNT];
        for (flag, stage) in active.iter_mut().zip(self.stages.iter()) {
            *flag = stage.active();
        }
        route(&active)
    }

    /// The target the background + scene render into this frame. Builds the first
    /// active stage's resources if needed; falls back to `destination` when the
    /// chain is skipped entirely.
    ///
    /// `destination` is **whatever runs next downstream**, not the surface
    /// specifically (ADR-0032): the transition blend's input while a dissolve runs,
    /// ink's input when ink is active, the surface otherwise.
    pub(crate) fn begin(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        destination: &wgpu::TextureView,
        surface: (u32, u32),
    ) -> SceneTarget {
        let routing = self.routing();
        let first = routing.scene_stage().and_then(|index| {
            let stage = self.stages.get_mut(index)?;
            let size = stage.internal_size().unwrap_or(surface);
            let view = stage.begin(encoder, surface)?;
            Some((view, size))
        });
        let (view, size) = first.unwrap_or_else(|| (destination.clone(), surface));
        SceneTarget {
            view,
            aspect: size.0 as f32 / size.1.max(1) as f32,
            size,
        }
    }

    /// Fold the chain down: each active stage resolves into the next active
    /// stage's input, the last into `destination` (the same caller-supplied view
    /// [`begin`](Self::begin) took). Returns the total draw calls the stages
    /// encoded. Call once, after the scene has rendered into `begin`'s target.
    pub(crate) fn resolve(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        destination: &wgpu::TextureView,
        surface: (u32, u32),
    ) -> u32 {
        let routing = self.routing();
        let mut draw_calls = 0;
        for (stage, next_stage) in routing.edges() {
            // The next active stage's input, built lazily right before anything
            // renders into it; `destination` when this is the last active stage.
            let out = next_stage
                .and_then(|next| self.stages.get_mut(next)?.begin(encoder, surface))
                .unwrap_or_else(|| destination.clone());
            if let Some(stage) = self.stages.get_mut(stage) {
                draw_calls += stage.resolve(queue, encoder, &out);
            }
        }
        draw_calls
    }

    /// The stage labels in chain order — the composite order, readable without a
    /// GPU. Used by the routing tests and available to diagnostics.
    pub(crate) fn stage_names(&self) -> [&'static str; STAGE_COUNT] {
        let mut names = [""; STAGE_COUNT];
        for (slot, stage) in names.iter_mut().zip(self.stages.iter()) {
            *slot = stage.name();
        }
        names
    }
}

#[cfg(test)]
mod tests {
    //! The routing contract, GPU-free (ADR-0031): these are the cases the old
    //! hand-written branch ladder in `draw_frame` encoded by enumeration and that
    //! nothing tested — they were only ever exercised indirectly, through WARP
    //! captures of presets that happened to bind the right params.

    // Test asserts index, expect and panic freely; this is not the render path.
    #![allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

    use super::{KALEIDOSCOPE, PostChain, Routing, STAGE_COUNT, TRAILS, route};
    use crate::render::background::Background;
    use crate::render::capture::{self, CaptureImage};
    use crate::render::context::{RenderContext, RenderError};

    /// `(stage, destination)` pairs, for readable assertions.
    fn edges(routing: &Routing) -> Vec<(usize, Option<usize>)> {
        routing.edges().collect()
    }

    /// Every combination of active flags, as `[bool; STAGE_COUNT]`.
    fn all_combinations() -> Vec<[bool; STAGE_COUNT]> {
        (0..(1 << STAGE_COUNT))
            .map(|mask: usize| {
                let mut flags = [false; STAGE_COUNT];
                for (bit, flag) in flags.iter_mut().enumerate() {
                    *flag = mask & (1 << bit) != 0;
                }
                flags
            })
            .collect()
    }

    /// No stage active: the scene renders straight to the destination and nothing
    /// folds. The passthrough every shipped preset takes today.
    #[test]
    fn no_active_stage_renders_the_scene_to_the_surface() {
        let routing = route(&[false, false]);
        assert_eq!(
            routing.scene_stage(),
            None,
            "with no stage active the scene targets the destination"
        );
        assert!(edges(&routing).is_empty(), "nothing to fold down");
    }

    /// One stage active: the scene renders into it and it resolves to the
    /// destination. Asserted for each position, since the ladder this replaced had
    /// a separate branch per stage.
    #[test]
    fn a_single_active_stage_resolves_to_the_surface() {
        for stage in [TRAILS, KALEIDOSCOPE] {
            let mut active = [false; STAGE_COUNT];
            active[stage] = true;
            let routing = route(&active);
            assert_eq!(
                routing.scene_stage(),
                Some(stage),
                "the scene renders into the only active stage"
            );
            assert_eq!(
                edges(&routing),
                vec![(stage, None)],
                "the only active stage folds into the destination"
            );
        }
    }

    /// Both active: trails folds into the kaleidoscope's input and the
    /// kaleidoscope into the destination — ADR-0018's feedback-then-fold order.
    #[test]
    fn all_active_stages_fold_in_composite_order() {
        let routing = route(&[true, true]);
        assert_eq!(routing.scene_stage(), Some(TRAILS));
        assert_eq!(
            edges(&routing),
            vec![(TRAILS, Some(KALEIDOSCOPE)), (KALEIDOSCOPE, None)]
        );
    }

    /// A skipped stage leaves no hole in the walk: with trails off, the scene
    /// renders **directly** into the kaleidoscope's input and the walk starts
    /// there, rather than the array position surviving as an empty slot. This is
    /// [`route`]'s compaction — the mechanism the old ladder answered with a nested
    /// `else if`, and the one a third stage would exercise at more positions.
    #[test]
    fn a_skipped_stage_compacts_the_walk() {
        let routing = route(&[false, true]);
        assert_eq!(routing.scene_stage(), Some(KALEIDOSCOPE));
        assert_eq!(routing.active_stages(), &[KALEIDOSCOPE]);
        assert_eq!(edges(&routing), vec![(KALEIDOSCOPE, None)]);
    }

    /// The invariant, over every combination: whatever runs, the last active stage
    /// targets the caller's destination — the composite always terminates there
    /// exactly once. (The destination is the surface only when neither the
    /// transition blend nor ink is downstream; ADR-0032 made it an argument.)
    #[test]
    fn the_last_active_stage_always_targets_the_surface() {
        for active in all_combinations() {
            let routing = route(&active);
            let edges = edges(&routing);
            let to_surface = edges.iter().filter(|(_, dest)| dest.is_none()).count();
            if active.iter().any(|&on| on) {
                assert_eq!(
                    edges.last().map(|(_, dest)| *dest),
                    Some(None),
                    "the last active stage folds into the destination for {active:?}"
                );
            }
            assert!(
                to_surface <= 1,
                "at most one stage may target the destination for {active:?}"
            );
        }
    }

    /// ADR-0032: ink is **not** in the chain. The ordering the retired
    /// `ink_when_active_is_always_last` asserted is structural now — ink is not in
    /// the thing that composes, so no flag combination can schedule it before a
    /// per-preset stage. This pins the membership itself instead.
    #[test]
    fn the_chain_holds_only_the_per_preset_look_stages() {
        let Some(ctx) = headless_context_or_skip() else {
            return;
        };
        let chain = PostChain::new(&ctx.device, ctx.surface_format());
        assert_eq!(
            chain.stage_names(),
            ["trails", "kaleidoscope"],
            "the chain is exactly the per-preset look; the engine-wide passes \
             (background, blend, ink) are driven outside it (ADR-0032)"
        );
    }

    /// Offscreen size for the GPU independence test — small enough to read back
    /// cheaply, large enough that a trail covers many pixels.
    const CHAIN_TEST_SIZE: u32 = 64;

    /// A headless device, or `None` (a logged skip) when the runner exposes no
    /// GPU adapter — macOS has no software Metal fallback (ADR-0016). Any other
    /// build error still panics loudly.
    fn headless_context_or_skip() -> Option<RenderContext> {
        match RenderContext::new_headless(CHAIN_TEST_SIZE, CHAIN_TEST_SIZE, true) {
            Ok(ctx) => Some(ctx),
            Err(RenderError::RequestAdapter(_)) => {
                eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
                None
            }
            Err(e) => panic!("headless context build failed: {e}"),
        }
    }

    /// Drive `chain` through one frame per entry in `lit` — a lit frame paints a
    /// full-brightness backdrop, a dark one is a plain black clear — folding each
    /// down into a **fresh offscreen of this call's own**, and read the last one
    /// back. The chain keeps whatever cross-frame state it has built between
    /// calls, so consecutive calls continue that chain's history.
    fn drive(
        ctx: &RenderContext,
        chain: &mut PostChain,
        background: &mut Background,
        lit: &[bool],
    ) -> CaptureImage {
        let size = (CHAIN_TEST_SIZE, CHAIN_TEST_SIZE);
        let (texture, view) =
            capture::create_target(&ctx.device, ctx.surface_format(), size.0, size.1);

        for &is_lit in lit {
            background.reset_params();
            if is_lit {
                // A flat, full-brightness backdrop: bright everywhere, so the
                // trail it leaves is unmistakable at 8-bit precision.
                assert!(background.set_param("bg_bright", 1.0));
                assert!(background.set_param("bg_vignette", 0.0));
            } else {
                assert!(background.set_param("bg_bright", 0.0));
            }

            let mut encoder = ctx
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("post-chain-independence"),
                });
            capture::record_clear(&mut encoder, &view);
            let target = chain.begin(&mut encoder, &view, size);
            background.render(&ctx.queue, &mut encoder, &target.view);
            chain.resolve(&ctx.queue, &mut encoder, &view, size);
            ctx.queue.submit(std::iter::once(encoder.finish()));
        }

        let (buffer, padded_bpr) = capture::create_readback(&ctx.device, size.0, size.1);
        let mut encoder = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("post-chain-readback"),
            });
        capture::record_copy(&mut encoder, &texture, &buffer, padded_bpr, size.0, size.1);
        ctx.queue.submit(std::iter::once(encoder.finish()));
        capture::read_back(&ctx.device, &buffer, size.0, size.1, padded_bpr)
            .expect("read back the folded chain output")
    }

    fn any_lit_pixel(image: &CaptureImage) -> bool {
        image
            .rgba
            .chunks_exact(4)
            .any(|px| px[0] > 0 || px[1] > 0 || px[2] > 0)
    }

    /// Plan 0030 Phase 2 — **the Plan 0023 unblock**: two `PostChain`s built
    /// against one device hold fully independent GPU state, so a dual-live
    /// transition can run two composites in one frame.
    ///
    /// Trails is the stage that matters: it is the one owning cross-frame state (a
    /// [`PingPongField`](crate::render::feedback::PingPongField)), which is exactly
    /// what Plan 0023's "in dual-live, each side needs its own feedback field" risk
    /// bullet names. Both chains are driven past the point where their lazily-built
    /// resources exist, then:
    ///
    /// - chain A accumulates a lit frame and fades it on a dark one — its own trail;
    /// - chain B, driven only through a dark frame, comes back **black**: none of
    ///   A's accumulation bled across;
    /// - chain B driven through the same history as A yields the **same pixels**,
    ///   so B's field is a real, working, separate one — not merely empty.
    ///
    /// Needs a GPU adapter, so it skips on runners without one (ADR-0016).
    #[test]
    fn two_chains_against_one_device_accumulate_independently() {
        let Some(ctx) = headless_context_or_skip() else {
            return;
        };
        let format = ctx.surface_format();
        let mut background = Background::new(&ctx.device, format);

        let mut chain_a = PostChain::new(&ctx.device, format);
        let mut chain_b = PostChain::new(&ctx.device, format);
        // A long trail on both, so a leaked accumulation would be glaring.
        assert!(
            chain_a.set_stage_param(TRAILS, "trails", 0.9),
            "the TRAILS position owns `trails`"
        );
        assert!(
            chain_b.set_stage_param(TRAILS, "trails", 0.9),
            "the TRAILS position owns `trails`"
        );

        // A: lit, then dark — the dark frame shows A's own fading trail.
        let a_lit_then_dark = drive(&ctx, &mut chain_a, &mut background, &[true, false]);
        assert!(
            any_lit_pixel(&a_lit_then_dark),
            "chain A's own accumulation survives into its dark frame"
        );

        // B, never driven before: one dark frame. A shared field would show A's
        // trail here; an independent one starts cleared.
        let b_dark_only = drive(&ctx, &mut chain_b, &mut background, &[false]);
        assert!(
            !any_lit_pixel(&b_dark_only),
            "chain B starts from its own cleared accumulation — none of A's \
             history leaked across"
        );

        // B through A's history: its own field must now imply the same pixels.
        let b_lit_then_dark = drive(&ctx, &mut chain_b, &mut background, &[true, false]);
        assert_eq!(
            b_lit_then_dark.rgba, a_lit_then_dark.rgba,
            "each chain folds to the pixels its own history implies"
        );
    }

    /// The active stages always come out in chain order, never reordered — the
    /// order is a compile-time constant (ADR-0018, reaffirmed by ADR-0031).
    #[test]
    fn active_stages_stay_in_chain_order() {
        for active in all_combinations() {
            let stages = route(&active).active_stages().to_vec();
            assert!(
                stages.windows(2).all(|w| w[0] < w[1]),
                "chain order is strictly increasing for {active:?}: {stages:?}"
            );
            let expected: Vec<usize> = active
                .iter()
                .enumerate()
                .filter(|&(_, &on)| on)
                .map(|(index, _)| index)
                .collect();
            assert_eq!(stages, expected, "exactly the active stages run");
        }
    }
}
