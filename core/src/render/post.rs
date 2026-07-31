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
//! - the scene renders into the **first active** stage's input, or straight into
//!   the destination when no stage is active;
//! - each active stage folds into the **next active** stage's input;
//! - the **last active** stage folds into the destination.
//!
//! Skipped stages are simply not in that walk, so trails folds directly into the
//! destination when the kaleidoscope is off. That adjacency is [`route`] — a
//! **pure function** over the flags, with no GPU and no `self`, so the contract is
//! unit-testable (the tests at the bottom of this file are the composite's first
//! real coverage).
//!
//! # The chain carries premultiplied alpha (ADR-0055)
//!
//! The **backdrop is not in the chain's input.** It is painted into the chain's
//! *destination*, and the chain composites over it — so `bg_*` is never folded,
//! blurred or accumulated, and a stage's alpha is what lets it show through.
//!
//! That makes two things load-bearing that were free before. A stage's input is
//! cleared **transparent** rather than opaque (an opaque clear holds the backdrop
//! out of every pixel the scene did not cover), and every stage must *propagate*
//! alpha rather than write `1.0`. [`Fold`] is how a stage learns which of the two
//! situations its `out` is in; both use one premultiplied-OVER pipeline, because
//! over a transparent-cleared target that blend reduces exactly to `REPLACE`.
//!
//! This generalizes the convention ADR-0026 already established at the scene seam,
//! where the fullscreen scenes present premultiplied over the backdrop and the
//! emissive ones draw additive in colour with `OVER` alpha — scene alpha was always
//! meaningful; the chain simply used to discard it.
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

/// Quantization step for each axis of a post stage's internal grid.
///
/// Same 256 px as the attractor's trail grid, for the same reason: a grid change
/// costs texture reallocation, bind-group rebuilds and — for trails — a cleared
/// accumulation, and the standalone forwards **every** `WindowEvent::Resized`. At
/// pixel granularity a live window drag would pay that hundreds of times and blink
/// the afterglow away continuously; at 256 px it crosses a handful of grids.
/// Purely a constant — no wall clock, so a fixed-size headless capture stays
/// byte-reproducible (NFR §6).
const POST_GRID_STEP: u32 = 256;

/// The grid both post stages run at for a given render target (ADR-0034) — this
/// call site's **cap and step** over the one shared policy
/// ([`grid::grid_size`](super::grid::grid_size)).
///
/// A thin wrapper on purpose. The arithmetic used to live here as a line-for-line
/// copy of the attractor's, which is how the two ended up with different aspect
/// behavior and how ADR-0037's defect shipped a second time; the numbers are
/// what is genuinely this call site's, and they stay here with their reasoning
/// (Plan 0035 Phase 3).
///
/// `cap` is the active tier's [`post_cap`](super::TierConfig::post_cap) — the one
/// number this call site used to hold as a constant (Plan 0044 Phase 1). It is
/// passed rather than read from a global so the function stays pure: a stage
/// resolves it once at construction and the chain's rebuild comparison keeps
/// answering a pure function of `surface`.
pub(crate) fn internal_grid_size(surface: (u32, u32), cap: (u32, u32)) -> (u32, u32) {
    super::grid::grid_size(surface, cap, POST_GRID_STEP)
}

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

    /// The internal resolution this stage runs at for the given render target
    /// (ADR-0034). Both shipped stages follow the target through
    /// [`internal_grid_size`], so line geometry composited through them is sharp at
    /// the display's own resolution rather than being rasterized full-size and then
    /// thrown away through a fixed 720p grid.
    ///
    /// A stage is free to answer with something else — a constant, or a fraction of
    /// the target — but it must be a **pure function of `surface`**, because the
    /// chain compares this against the built size to decide whether to rebuild.
    fn internal_size(&self, surface: (u32, u32)) -> (u32, u32);

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
    ///
    /// `surface` is the **render target's** pixel size — the same value
    /// [`begin`](Self::begin) took, and the shape the finished frame is seen at.
    /// A stage computing screen-destined geometry (the kaleidoscope's fold) takes
    /// its aspect from here and **never** from the grid it happens to be
    /// rasterizing into, nor from `out`'s own size: every present down the chain
    /// is a plain normalized stretch, so an intermediate stage's grid is a
    /// resolution too and its aspect cancels out (ADR-0037). The chain's
    /// destination is surface-sized in every case (the blend's input, ink's input,
    /// or the surface), so this *is* the destination's aspect today — passing
    /// `surface` rather than `out`'s size is what keeps that true if a stage is
    /// ever added after the fold.
    ///
    /// `fold` says whether `out` is this stage's to own or something already
    /// painted to blend over — see [`Fold`].
    fn resolve(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        out: &wgpu::TextureView,
        surface: (u32, u32),
        fold: Fold,
    ) -> u32;

    /// Drop the lazily-built resources, so the next active frame starts from a
    /// fresh (cleared) state. Used on the capture scene-rebuild to keep a headless
    /// capture a pure function of its inputs (NFR §6).
    fn reset_resources(&mut self);
}

/// What a stage's [`resolve`](PostStage::resolve) finds in `out` — and therefore
/// how it must treat what is already there (ADR-0055).
///
/// The chain carries **premultiplied alpha**, and the backdrop is no longer inside
/// its input: it is painted into the chain's destination and the chain composites
/// over it. So the last active stage must *blend*, while every earlier one is
/// writing into a scratch offscreen it owns outright.
///
/// Both cases use the same premultiplied-OVER pipeline. Over a target just cleared
/// to transparent, `src + dst * (1 - src.a)` reduces to `src` in every channel, so
/// [`Own`](Self::Own) is bit-identical to a `REPLACE` write — which is why this
/// distinction costs a load-op and not a second pipeline per stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Fold {
    /// `out` is the next stage's input: this stage owns it. Clear it to
    /// transparent first — it holds the **previous frame's** content otherwise,
    /// since stage offscreens are persistent textures, not transient ones.
    Own,
    /// `out` is the chain's destination, where the backdrop is already painted.
    /// Load and blend over it, so `bg_*` survives underneath wherever this stage's
    /// alpha is below 1.
    Over,
}

impl Fold {
    /// The colour attachment load op this fold implies.
    pub(crate) fn load_op(self) -> wgpu::LoadOp<wgpu::Color> {
        match self {
            // TRANSPARENT, not BLACK: an opaque clear would hold the backdrop out
            // of every pixel the scene did not cover.
            Self::Own => wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
            Self::Over => wgpu::LoadOp::Load,
        }
    }
}

/// Encode a clear-only pass that blanks `view` to transparent.
///
/// No pipeline and no draw — just the load op — so this costs a render-pass
/// begin/end and nothing else.
fn clear_transparent(encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView, label: &str) {
    let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
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
    /// The aspect the scene projects at — the **render target's**, never
    /// [`size`](Self::size)'s (ADR-0037).
    ///
    /// An internal grid is a *resolution, not a shape*: it is quantized to a
    /// 256 px step and capped, so its aspect is only approximately the target's,
    /// and every stage presents with a plain normalized blit that ignores aspect
    /// entirely. The two stretches therefore **cancel**. A scene told the target's
    /// aspect draws itself pre-squashed into a grid of a different shape, and the
    /// present's stretch is exactly the inverse: a unit circle at aspect 1.6
    /// rendered into a 1280x1024 grid occupies 400x512 texels, and the blit down
    /// to a 1280x800 surface returns it to 400x400 — round.
    ///
    /// Deriving this from the grid instead is what made turning `trails` or
    /// `kaleido_*` on change the **shape** of the picture — 1.28x too wide at
    /// 1280x800, 1.07x at 1280x720 — and re-broke Plan 0029's attractor fix
    /// whenever a stage was active, since the attractor reads this value. It was
    /// invisible at 1920x1080 and 2048x1152, which the policy returns exactly
    /// 16:9. Any `aspect` computed from a grid size is a bug.
    pub aspect: f32,
    /// The pixel size a scene sees through
    /// [`Scene::set_target_size`](super::scenes::Scene::set_target_size) (ADR-0030),
    /// so a scene with an internal accumulation field matches its actual target
    /// instead of a fixed grid.
    pub size: (u32, u32),
    /// **This frame's routing decision**, made once in [`PostChain::begin`] and
    /// handed back so [`PostChain::resolve`] consumes it rather than recomputing
    /// it (Plan 0031 Phase 6, closing Plan 0030's close-review minor 1).
    ///
    /// The two used to call `routing()` independently. That was correct only
    /// because no stage's [`active`](PostStage::active) changes between them —
    /// a whole frame's correctness resting on an incidental property. Threading
    /// the value through makes "one routing decision per frame" structural: there
    /// is no way to call `resolve` without the `Routing` `begin` produced.
    /// `Routing` is `Copy` and fixed-size, so this costs nothing.
    pub routing: Routing,
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
    pub(crate) fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        tier: &super::TierConfig,
    ) -> Self {
        let chain = Self {
            // ADR-0018's feedback-then-fold order. Ink is deliberately absent:
            // it is an engine-wide pass on the finished frame, so it runs after
            // the chain (and after the transition blend) — ADR-0032.
            stages: [
                Box::new(Trails::new(device, surface_format, tier.post_cap)),
                Box::new(Kaleidoscope::new(device, surface_format, tier.post_cap)),
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

    /// The target the **scene** renders into this frame. Builds the first active
    /// stage's resources if needed; falls back to `destination` when the chain is
    /// skipped entirely.
    ///
    /// The backdrop is *not* in this target (ADR-0055) — it is painted into
    /// `destination`, underneath everything the chain will fold down. So when a
    /// stage is active this clears that stage's input to **transparent** and the
    /// scene draws onto nothing; the chain's alpha is what lets the backdrop show
    /// through at the end.
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
            let size = stage.internal_size(surface);
            let view = stage.begin(encoder, surface)?;
            // The scene loads rather than clears (the backdrop pass used to own
            // this clear, and now owns `destination`'s instead), and a stage
            // offscreen persists between frames — so without this the scene would
            // accumulate onto the previous frame.
            clear_transparent(encoder, &view, "post-chain-input-clear");
            Some((view, size))
        });
        let (view, size) = first.unwrap_or_else(|| (destination.clone(), surface));
        SceneTarget {
            view,
            // `surface`, not `size` — see the field's docs and ADR-0037. The grid
            // is a texel count; the shape is the render target's.
            aspect: surface.0 as f32 / surface.1.max(1) as f32,
            size,
            routing,
        }
    }

    /// Fold the chain down: each active stage resolves into the next active
    /// stage's input, the last into `destination` (the same caller-supplied view
    /// [`begin`](Self::begin) took). Returns the total draw calls the stages
    /// encoded. Call once, after the scene has rendered into `begin`'s target.
    ///
    /// `routing` is the [`SceneTarget::routing`] `begin` produced — passed in, not
    /// recomputed, so the frame folds down exactly the stages it opened.
    pub(crate) fn resolve(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        routing: Routing,
        destination: &wgpu::TextureView,
        surface: (u32, u32),
    ) -> u32 {
        let mut draw_calls = 0;
        for (stage, next_stage) in routing.edges() {
            // The next active stage's input, built lazily right before anything
            // renders into it; `destination` when this is the last active stage.
            let out = next_stage
                .and_then(|next| self.stages.get_mut(next)?.begin(encoder, surface))
                .unwrap_or_else(|| destination.clone());
            // The last active stage lands on the backdrop and must blend; every
            // earlier one owns the scratch offscreen it writes (ADR-0055).
            let fold = if next_stage.is_some() {
                Fold::Own
            } else {
                Fold::Over
            };
            if let Some(stage) = self.stages.get_mut(stage) {
                draw_calls += stage.resolve(queue, encoder, &out, surface, fold);
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

    use super::{
        Fold, KALEIDOSCOPE, POST_GRID_STEP, PostChain, PostStage, Routing, STAGE_COUNT, TRAILS,
        internal_grid_size, route,
    };
    use crate::render::TierConfig;
    use crate::render::background::Background;
    use crate::render::capture::{self, CaptureImage};
    use crate::render::context::{RenderContext, RenderError};
    use crate::render::gpu;

    /// The tier every test in this module runs at, and the one every golden
    /// baseline is blessed at (ADR-0045). These tests pin the **policy** — the
    /// quantization, the single scale factor, the purity — not the tier, so they
    /// read the floor cap the constants here used to be.
    const FLOOR: TierConfig = TierConfig::FLOOR;
    const POST_MAX_W: u32 = FLOOR.post_cap.0;
    const POST_MAX_H: u32 = FLOOR.post_cap.1;

    /// [`internal_grid_size`] at the floor cap.
    fn floor_grid(surface: (u32, u32)) -> (u32, u32) {
        internal_grid_size(surface, FLOOR.post_cap)
    }

    // -----------------------------------------------------------------------
    // The internal-grid policy (ADR-0034) — GPU-free
    // -----------------------------------------------------------------------

    /// The headline property: a stage's grid **follows the render target** instead
    /// of being pinned to a fixed 1280x720. This is the whole point of Plan 0033 —
    /// line geometry composited through trails or the fold was rasterized at full
    /// resolution and then thrown away through a 720p grid, which on the 2048x1152
    /// display the preset lane worked at was a 1.6x upscale of the entire frame.
    #[test]
    fn the_internal_grid_follows_the_target_instead_of_a_fixed_720p() {
        // Common desktop sizes, all under the cap: the grid is the target rounded
        // up to the step, never the old constant.
        for target in [(1280, 720), (1600, 900), (1920, 1080)] {
            let grid = floor_grid(target);
            assert!(
                grid.0 >= target.0 && grid.1 >= target.1,
                "{target:?} must not be downsampled below the target: got {grid:?}"
            );
            assert!(grid.0 <= POST_MAX_W && grid.1 <= POST_MAX_H, "{grid:?}");
        }
        // The display this plan exists for. 2048x1152 is 16:9 and above the width
        // cap, so it comes back capped *with its aspect exactly preserved* — a
        // 1.07x downscale, and emphatically not 1280x720 (ADR-0034).
        assert_eq!(floor_grid((2048, 1152)), (1920, 1080));
        assert_ne!(floor_grid((2048, 1152)), (1280, 720));
    }

    /// Every axis lands on the quantization step (or the cap), never on 0.
    ///
    /// The step is what keeps a live window drag from reallocating a
    /// `Rgba16Float` texture pair — and, for trails, clearing the accumulated
    /// history — on every one of the hundreds of `Resized` events a drag emits.
    #[test]
    fn every_grid_axis_is_quantized_and_non_degenerate() {
        for target in [
            (1, 1),
            (17, 3),
            (640, 480),
            (1281, 721),
            (1920, 1080),
            (3840, 2160),
            (100, 4000),
        ] {
            let (w, h) = floor_grid(target);
            assert!(w > 0 && h > 0, "{target:?} produced a degenerate grid");
            assert!(
                w <= POST_MAX_W && h <= POST_MAX_H,
                "{target:?} -> ({w}, {h})"
            );
            for (axis, cap) in [(w, POST_MAX_W), (h, POST_MAX_H)] {
                assert!(
                    axis % POST_GRID_STEP == 0 || axis == cap,
                    "{target:?}: axis {axis} is neither a {POST_GRID_STEP} multiple nor the cap"
                );
            }
        }
    }

    /// Plan 0029's lesson, re-paid here: when the cap binds, **one** scale factor
    /// applies to both axes. Clamping each axis independently turned a 3440x1440
    /// ultrawide into a 16:9 grid, which the aspect-ignoring present then stretched
    /// back — so the picture changed shape as the window crossed the cap.
    ///
    /// Note what "aspect intact" can and cannot mean under a 256 px step: the
    /// derived axis is rounded up to the step, so an ultrawide's grid aspect is
    /// coarser than its target's (3440x1440 is 2.39; the grid is 1920x1024 = 1.88).
    /// What the single factor buys is that it is **not** squashed to the cap's own
    /// 16:9, which is the regression.
    #[test]
    fn a_capped_target_keeps_its_proportions_rather_than_the_caps() {
        let ultrawide = floor_grid((3440, 1440));
        let squashed_to_the_cap = (POST_MAX_W, POST_MAX_H);
        assert_ne!(
            ultrawide, squashed_to_the_cap,
            "a 3440x1440 ultrawide must not come back as the cap's own 16:9"
        );
        let target_aspect = 3440.0 / 1440.0;
        let grid_aspect = ultrawide.0 as f32 / ultrawide.1 as f32;
        let cap_aspect = POST_MAX_W as f32 / POST_MAX_H as f32;
        assert!(
            grid_aspect > cap_aspect,
            "the ultrawide grid ({grid_aspect:.3}) is no wider than 16:9 ({cap_aspect:.3}) \
             — the per-axis clamp regression is back"
        );
        assert!(
            (grid_aspect - target_aspect).abs() < target_aspect * 0.25,
            "grid aspect {grid_aspect:.3} is far from the target's {target_aspect:.3}"
        );

        // The same in portrait, where the height binds instead.
        let portrait = floor_grid((1440, 3440));
        assert!(
            (portrait.1 as f32 / portrait.0 as f32) > cap_aspect,
            "a portrait target must keep its proportions too: {portrait:?}"
        );
    }

    /// The policy is a **pure function** — the same target always yields the same
    /// grid, with no wall clock anywhere in it, so a fixed-size headless capture
    /// stays byte-reproducible (NFR §6).
    #[test]
    fn the_grid_policy_is_a_pure_function_of_the_target() {
        for target in [(800, 600), (2048, 1152), (3440, 1440)] {
            assert_eq!(floor_grid(target), floor_grid(target));
        }
    }

    /// **The tier's first visible effect** (Plan 0044 Phase 1): where the floor
    /// cap binds, the rich tier resolves a genuinely larger grid; where it does
    /// not, the two tiers agree exactly.
    ///
    /// The second half is what makes the first mean something. A tier raises a
    /// **ceiling**, not the grid itself, so a preset diverges only where the floor
    /// was actually costing it resolution — which is also why every golden
    /// baseline is untouched by this plan (they capture at 1280x720 and smaller,
    /// squarely in the agreeing set, and `new_headless` pins the floor regardless).
    ///
    /// **1920x1080 belongs in the *binding* set, not the agreeing one**, and that
    /// is worth stating because it is the display size the floor was written for.
    /// The policy quantizes *then* clamps, so a 1080p target rounds up to
    /// 2048x1280 and the floor cap cuts it back to exactly 1920x1080 — meaning on
    /// the rich tier a 1080p window supersamples its post stages by ~1.07x rather
    /// than matching them to the surface.
    #[test]
    fn the_rich_tier_raises_the_grid_only_where_the_floor_cap_binds() {
        let rich = TierConfig::RICH;
        for target in [(1920, 1080), (2560, 1440), (3440, 1440), (3840, 2160)] {
            let (fw, fh) = floor_grid(target);
            let (rw, rh) = internal_grid_size(target, rich.post_cap);
            assert!(
                rw > fw && rh > fh,
                "the floor cap binds at {target:?}, so rich must resolve a larger \
                 grid: floor {fw}x{fh}, rich {rw}x{rh}"
            );
        }
        // Targets whose *quantized* grid still fits under the floor cap, so
        // neither cap binds and the tier cannot change the answer.
        for target in [(640, 480), (1280, 720), (1600, 900)] {
            assert_eq!(
                floor_grid(target),
                internal_grid_size(target, rich.post_cap),
                "neither cap binds at {target:?}, so the tier must not change the \
                 grid — a tier raises a ceiling, not the resolution"
            );
        }
    }

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
        let chain = PostChain::new(&ctx.device, ctx.surface_format(), &FLOOR);
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
            chain.resolve(&ctx.queue, &mut encoder, target.routing, &view, size);
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

        let mut chain_a = PostChain::new(&ctx.device, format, &FLOOR);
        let mut chain_b = PostChain::new(&ctx.device, format, &FLOOR);
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

    /// A chain's stages, addressed concretely so their build counters are readable.
    struct Stages {
        trails: crate::render::trails::Trails,
        kaleido: crate::render::kaleidoscope::Kaleidoscope,
    }

    /// Drive one `begin`/`resolve` frame through `stage` at `surface`, discarding
    /// the pixels — this is about what the stage *allocates*, not what it draws.
    fn pump(ctx: &RenderContext, stage: &mut dyn PostStage, surface: (u32, u32)) {
        let (_texture, view) =
            capture::create_target(&ctx.device, ctx.surface_format(), surface.0, surface.1);
        let mut encoder = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("post-resize"),
            });
        stage.begin(&mut encoder, surface);
        stage.resolve(&ctx.queue, &mut encoder, &view, surface, Fold::Over);
        ctx.queue.submit(std::iter::once(encoder.finish()));
    }

    /// **ADR-0030's compare-first obligation, counted rather than assumed**: a
    /// stage rebuilds its resources on a size change and *only* on a size change.
    ///
    /// This is worth a counter rather than a pixel assertion because rebuilding
    /// every frame would look almost right: the picture would be correct, but the
    /// trails accumulation would be cleared every frame (so trails would silently
    /// stop working) and a `Rgba16Float` texture pair would be reallocated at frame
    /// rate. Neither is visible in a single captured frame.
    #[test]
    fn stages_rebuild_on_a_size_change_and_only_on_a_size_change() {
        let Some(ctx) = headless_context_or_skip() else {
            return;
        };
        let format = ctx.surface_format();
        let mut stages = Stages {
            trails: crate::render::trails::Trails::new(&ctx.device, format, FLOOR.post_cap),
            kaleido: crate::render::kaleidoscope::Kaleidoscope::new(
                &ctx.device,
                format,
                FLOOR.post_cap,
            ),
        };
        // Both stages have to be active or `begin` is never reached.
        assert!(stages.trails.set_param("trails", 0.9));
        assert!(stages.kaleido.set_param("kaleido_order", 6.0));

        // Two sizes that the policy maps to *different* grids, so the compare has
        // something to see. (Sizes inside one 256 px step deliberately do not
        // rebuild — that is the point of quantizing.)
        let small = (512, 512);
        let large = (1024, 768);
        assert_ne!(
            floor_grid(small),
            floor_grid(large),
            "the two probe sizes must land on different grids for this test to mean anything"
        );

        for _ in 0..5 {
            pump(&ctx, &mut stages.trails, small);
            pump(&ctx, &mut stages.kaleido, small);
        }
        assert_eq!(stages.trails.build_count(), 1, "five frames, one build");
        assert_eq!(stages.kaleido.build_count(), 1, "five frames, one build");

        for _ in 0..3 {
            pump(&ctx, &mut stages.trails, large);
            pump(&ctx, &mut stages.kaleido, large);
        }
        assert_eq!(
            stages.trails.build_count(),
            2,
            "a size change builds once more"
        );
        assert_eq!(
            stages.kaleido.build_count(),
            2,
            "a size change builds once more"
        );

        // Back to the first size: it must build *again* rather than resurrect the
        // stale grid — the resources for `small` were dropped when `large` replaced
        // them, so reusing them is not merely wasteful, it is impossible.
        for _ in 0..3 {
            pump(&ctx, &mut stages.trails, small);
            pump(&ctx, &mut stages.kaleido, small);
        }
        assert_eq!(
            stages.trails.build_count(),
            3,
            "returning to a size rebuilds"
        );
        assert_eq!(
            stages.kaleido.build_count(),
            3,
            "returning to a size rebuilds"
        );

        // A size within the same 256 px step is free — the quantization is what
        // makes a live window drag survivable. (512 sits exactly *on* a step, so
        // 513 is already the next grid up; the probe has to be inside the step.)
        let same_grid = (400, 400);
        assert_eq!(floor_grid(small), floor_grid(same_grid));
        pump(&ctx, &mut stages.trails, same_grid);
        assert_eq!(
            stages.trails.build_count(),
            3,
            "a resize inside one quantization step must not reallocate"
        );
    }

    /// The chain reports the **target's** grid, not a fixed 720p — checked through
    /// the real `begin`, so it covers the wiring and not just the policy function.
    ///
    /// Plan 0033's done-when for this reads "a `PostChain` driven at 2048x1152
    /// reports an internal size of 2048x1152". That is not reachable alongside the
    /// same plan's 1920x1080 cap, which 2048x1152 exceeds on width; ADR-0034 says
    /// as much when it calls the cap "a 1.07x downscale at the display in
    /// question". So the assertion is the capped grid — with the aspect exactly
    /// preserved, 2048x1152 and 1920x1080 both being 16:9 — and, per the done-when's
    /// actual point, emphatically not 1280x720.
    #[test]
    fn the_chain_reports_the_targets_grid_not_a_fixed_720p() {
        let Some(ctx) = headless_context_or_skip() else {
            return;
        };
        let format = ctx.surface_format();
        let mut chain = PostChain::new(&ctx.device, format, &FLOOR);
        assert!(chain.set_stage_param(TRAILS, "trails", 0.9));

        let surface = (2048, 1152);
        let (_texture, view) =
            capture::create_target(&ctx.device, format, CHAIN_TEST_SIZE, CHAIN_TEST_SIZE);
        let mut encoder = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("post-target-size"),
            });
        let target = chain.begin(&mut encoder, &view, surface);
        ctx.queue.submit(std::iter::once(encoder.finish()));

        assert_eq!(
            target.size,
            (1920, 1080),
            "the composite must follow the render target under the cap, not sit at 1280x720"
        );
        assert_ne!(target.size, (1280, 720), "the fixed 720p grid is retired");
        assert!(
            (target.aspect - 2048.0 / 1152.0).abs() < 1e-3,
            "the capped grid must keep the target's aspect, got {}",
            target.aspect
        );
    }

    /// Fold order used by the symmetry probes.
    const FOLD_ORDER: usize = 6;

    /// Fold a vignetted backdrop at `surface` and report
    /// [`fold_mirror_error`] over the result.
    fn fold_error_at(ctx: &RenderContext, surface: (u32, u32)) -> f32 {
        let format = ctx.surface_format();
        let mut background = Background::new(&ctx.device, format);
        let mut kaleido =
            crate::render::kaleidoscope::Kaleidoscope::new(&ctx.device, format, FLOOR.post_cap);
        assert!(kaleido.set_param("kaleido_order", FOLD_ORDER as f32));

        let (texture, view) = capture::create_target(&ctx.device, format, surface.0, surface.1);
        let mut encoder = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("kaleido-aspect"),
            });
        background.reset_params();
        assert!(background.set_param("bg_bright", 1.0));
        assert!(background.set_param("bg_vignette", 0.6));
        let src = kaleido
            .begin(&mut encoder, surface)
            .expect("the fold builds its input");
        // The backdrop is the fold's *test pattern* here, not the chain's backdrop
        // — this probe wants radially-structured content to measure symmetry on,
        // and paints it straight into the fold's input. `Fold::Own` because `view`
        // is a fresh capture target with nothing underneath to blend with.
        background.render(&ctx.queue, &mut encoder, &src);
        kaleido.resolve(&ctx.queue, &mut encoder, &view, surface, Fold::Own);
        let (buffer, padded_bpr) = capture::create_readback(&ctx.device, surface.0, surface.1);
        capture::record_copy(
            &mut encoder,
            &texture,
            &buffer,
            padded_bpr,
            surface.0,
            surface.1,
        );
        ctx.queue.submit(std::iter::once(encoder.finish()));
        let img = capture::read_back(&ctx.device, &buffer, surface.0, surface.1, padded_bpr)
            .expect("read back the folded frame");
        fold_mirror_error(&img, FOLD_ORDER)
    }

    /// The kaleidoscope's aspect correction is the **render target's**, not a
    /// compile-time 16:9 (ADR-0034) and not its own internal grid's (ADR-0037).
    ///
    /// Asserted as the fold's own symmetry: an order-N dihedral fold makes the
    /// output identical in each of its N wedges *in aspect-corrected space*, so
    /// sampling those wedges with the **target's** aspect must give matching means.
    /// If the shader folded about a different axis, the wedges would land somewhere
    /// else and the means would diverge. Restricted to an inscribed disc so the
    /// rectangular frame's corners do not give some wedges more area.
    ///
    /// Two probes, and the second is the one that matters:
    ///
    /// - **(512, 256)** — 2:1, which `internal_grid_size` returns unchanged. This
    ///   was the whole test through Plan 0033, and it *cannot* distinguish grid
    ///   aspect from surface aspect, which is why it passed for the entire life of
    ///   the defect.
    /// - **(320, 256)** — grid (512, 256). The target is 1.25:1 and the grid is
    ///   2:1, so folding about the grid is a 1.6x wrong axis and only the target's
    ///   aspect scores clean.
    #[test]
    fn the_fold_stays_symmetric_on_a_non_16_9_target() {
        let Some(ctx) = headless_context_or_skip() else {
            return;
        };

        // The grid-agnostic probe (retained): grid and surface agree here.
        let agreeing = (512, 256);
        assert_eq!(floor_grid(agreeing), (512, 256), "a 2:1 grid");
        // Measured: 0.0099 with the target's aspect, 0.1860 with a baked 1280x720
        // — a 19x separation, and the threshold sits between them. Verified
        // non-vacuous by re-baking the old constant and watching this fail.
        let error = fold_error_at(&ctx, agreeing);
        eprintln!("fold mirror error at {agreeing:?} (grid == surface): {error:.4}");
        assert!(
            error < 0.05,
            "the fold's wedge mirror is broken by {error:.4} of the frame's contrast on a \
             2:1 target — it is being aspect-corrected to something other than the target"
        );

        // The discriminating probe: the grid's shape is not the target's here, so
        // only one of the two can be the axis the fold uses.
        let disagreeing = (320, 256);
        let grid = floor_grid(disagreeing);
        assert_eq!(grid, (512, 256), "the probe's grid must not be its surface");
        assert!(
            (grid.0 as f32 / grid.1 as f32 - disagreeing.0 as f32 / disagreeing.1 as f32).abs()
                > 0.5,
            "grid {grid:?} and surface {disagreeing:?} must disagree enough to separate them"
        );
        let error = fold_error_at(&ctx, disagreeing);
        eprintln!("fold mirror error at {disagreeing:?} (grid {grid:?}): {error:.4}");
        assert!(
            error < 0.05,
            "the fold's wedge mirror is broken by {error:.4} of the frame's contrast on a \
             target whose grid has a different shape — it is folding about the grid's axis \
             rather than the target's (ADR-0037)"
        );
    }

    /// How badly the fold's own mirror symmetry is broken, measured in the frame's
    /// **true** aspect-corrected space and normalized by the frame's contrast.
    ///
    /// A dihedral fold mirrors within each wedge, so the output must satisfy
    /// `L(r, θ) == L(r, 2c - θ)` where `c` is that wedge's centre line. The
    /// **screen** direction of those centre lines depends on the aspect the shader
    /// corrected by: a line at angle `θ` in aspect-`A` space is a different set of
    /// pixels than the same angle in aspect-`T` space. So measuring the symmetry
    /// with the true grid aspect detects a shader folding about the wrong one —
    /// which comparing whole-wedge *means* cannot, because the fold is periodic and
    /// a periodic function's mean over any full period is the same wherever the
    /// period starts.
    fn fold_mirror_error(img: &CaptureImage, order: usize) -> f32 {
        let (w, h) = (img.width as usize, img.height as usize);
        let aspect = img.width as f32 / img.height.max(1) as f32;
        let seg = std::f32::consts::TAU / order as f32;
        let luma_at = |r: f32, theta: f32| -> Option<f32> {
            // Back to pixels: undo the aspect correction on x only.
            let x = ((r * theta.cos()) / aspect + 0.5) * w as f32 - 0.5;
            let y = (r * theta.sin() + 0.5) * h as f32 - 0.5;
            let (xi, yi) = (x.round() as i32, y.round() as i32);
            if xi < 0 || yi < 0 || xi >= w as i32 || yi >= h as i32 {
                return None;
            }
            let i = (yi as usize * w + xi as usize) * 4;
            Some(
                (0.299 * img.rgba[i] as f32
                    + 0.587 * img.rgba[i + 1] as f32
                    + 0.114 * img.rgba[i + 2] as f32)
                    / 255.0,
            )
        };

        let mut diff = 0.0f32;
        let mut pairs = 0usize;
        let mut values: Vec<f32> = Vec::new();
        for ri in 1..=24 {
            // Inside the inscribed disc, so no sample leaves the frame.
            let r = 0.45 * ri as f32 / 24.0;
            for ti in 0..(order * 12) {
                let theta = std::f32::consts::TAU * ti as f32 / (order * 12) as f32;
                let centre = seg * (theta / seg).floor() + seg * 0.5;
                let mirrored = 2.0 * centre - theta;
                if let (Some(a), Some(b)) = (luma_at(r, theta), luma_at(r, mirrored)) {
                    diff += (a - b).abs();
                    pairs += 1;
                    values.push(a);
                }
            }
        }
        // Normalize by the frame's own contrast, so the number is scale-free and a
        // flat frame cannot score a deceptively good zero.
        let mean = values.iter().sum::<f32>() / values.len().max(1) as f32;
        let contrast =
            values.iter().map(|v| (v - mean).abs()).sum::<f32>() / values.len().max(1) as f32;
        (diff / pairs.max(1) as f32) / contrast.max(1e-6)
    }

    // -----------------------------------------------------------------------
    // ADR-0037: composing a stage must not change the picture's *shape*
    // -----------------------------------------------------------------------

    /// A test-only stand-in for a scene: a hard-edged disc drawn at whatever
    /// `aspect` the composite hands it.
    ///
    /// `world.x = ndc.x * aspect` is the exact inverse of the `ndc = world.x /
    /// aspect` projection every real scene applies (`particles/mod.rs:414`,
    /// `lines/renderer.rs:76`), so this models the seam under test — a scene's only
    /// shape input is that one float — without dragging a whole scene's params,
    /// palette and audio bindings into a geometry assertion.
    const DISC_SHADER: &str = r#"
struct D { v: vec4<f32> } // x: aspect

@group(0) @binding(0) var<uniform> u: D;

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let aspect = max(u.v.x, 0.001);
    let p = vec2<f32>(in.ndc.x * aspect, in.ndc.y);
    let lit = select(0.0, 1.0, length(p) < 0.5);
    return vec4<f32>(lit, lit, lit, 1.0);
}
"#;

    /// Encode the disc into `target`, projected at `aspect`.
    fn draw_disc(
        ctx: &RenderContext,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        aspect: f32,
    ) {
        let format = ctx.surface_format();
        let uniform = ctx.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("disc-uniform"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        ctx.queue
            .write_buffer(&uniform, 0, bytemuck::bytes_of(&[aspect, 0.0, 0.0, 0.0]));
        let layout = ctx
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("disc-bind-layout"),
                entries: &[gpu::uniform(0, wgpu::ShaderStages::FRAGMENT)],
            });
        let bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("disc-bind-group"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        let shader = gpu::fullscreen_shader(
            &ctx.device,
            "disc-shader",
            gpu::FULLSCREEN_VS_NDC,
            DISC_SHADER,
        );
        let pipeline = gpu::fullscreen_pipeline(
            &ctx.device,
            &shader,
            &[&layout],
            format,
            wgpu::BlendState::REPLACE,
            "disc",
        );
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("disc-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
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
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..3, 0..1);
    }

    /// Width/height of the lit region's bounding box, in destination pixels.
    /// `1.0` is a round disc; the defect made it the grid's aspect over the
    /// target's.
    fn lit_extent_ratio(img: &CaptureImage) -> f32 {
        let (w, h) = (img.width as usize, img.height as usize);
        let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0usize, 0usize);
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) * 4;
                if img.rgba[i] > 128 {
                    x0 = x0.min(x);
                    y0 = y0.min(y);
                    x1 = x1.max(x);
                    y1 = y1.max(y);
                }
            }
        }
        assert!(x0 != usize::MAX, "the disc drew nothing to measure");
        (x1 - x0 + 1) as f32 / (y1 - y0 + 1) as f32
    }

    /// Draw the disc through the composite at `surface` — with the trails stage
    /// active or with the chain entirely skipped — and report the shape it lands
    /// on the destination as.
    ///
    /// Trails is the right stage for this: it computes no geometry of its own, and
    /// on its first frame the max-decay against a cleared accumulation is the
    /// identity, so anything that moves is the aspect and nothing else.
    fn disc_extent_ratio(ctx: &RenderContext, surface: (u32, u32), through_a_stage: bool) -> f32 {
        let format = ctx.surface_format();
        let mut chain = PostChain::new(&ctx.device, format, &FLOOR);
        if through_a_stage {
            assert!(chain.set_stage_param(TRAILS, "trails", 0.5));
        }
        let (texture, view) = capture::create_target(&ctx.device, format, surface.0, surface.1);
        let mut encoder = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("composite-aspect"),
            });
        capture::record_clear(&mut encoder, &view);
        let target = chain.begin(&mut encoder, &view, surface);
        draw_disc(ctx, &mut encoder, &target.view, target.aspect);
        chain.resolve(&ctx.queue, &mut encoder, target.routing, &view, surface);

        let (buffer, padded_bpr) = capture::create_readback(&ctx.device, surface.0, surface.1);
        capture::record_copy(
            &mut encoder,
            &texture,
            &buffer,
            padded_bpr,
            surface.0,
            surface.1,
        );
        ctx.queue.submit(std::iter::once(encoder.finish()));
        let img = capture::read_back(&ctx.device, &buffer, surface.0, surface.1, padded_bpr)
            .expect("read back the composited frame");
        lit_extent_ratio(&img)
    }

    /// **The defect this plan exists for**: turning a post stage on must change the
    /// picture's softness, never its **shape** (ADR-0037).
    ///
    /// A radially symmetric figure is composited twice at the same target size —
    /// once with the chain skipped, once through an active `trails` — and the two
    /// must agree. Before the fix `SceneTarget::aspect` came from the quantized
    /// internal grid, so the scene drew correct-for-the-grid and the stage's
    /// aspect-ignoring present stretched it by grid-aspect-over-target-aspect.
    ///
    /// The sizes are chosen, not incidental. **1280x800** takes a 1280x1024 grid:
    /// 1.25 against the target's 1.6, a 1.28x stretch and the worst ordinary case.
    /// **1920x1080** is the control the policy returns unchanged — it is what the
    /// project develops at, it is why this shipped, and it must move under neither
    /// the defect nor the fix.
    #[test]
    fn composing_a_stage_does_not_change_the_pictures_shape() {
        let Some(ctx) = headless_context_or_skip() else {
            return;
        };

        let skewing = (1280, 800);
        assert_eq!(
            floor_grid(skewing),
            (1280, 1024),
            "the probe size must be one where the grid's shape is not the target's"
        );
        let control = (1920, 1080);
        assert_eq!(
            floor_grid(control),
            control,
            "the control must be a size the policy returns exactly — that is what hid this"
        );

        for surface in [skewing, control] {
            let plain = disc_extent_ratio(&ctx, surface, false);
            let staged = disc_extent_ratio(&ctx, surface, true);
            eprintln!(
                "{surface:?}: disc x/y = {plain:.4} with the chain skipped, \
                 {staged:.4} through trails (grid {:?})",
                floor_grid(surface)
            );
            assert!(
                (plain - 1.0).abs() < 0.03,
                "{surface:?}: the reference disc is not round ({plain:.4}) — the \
                 comparison below would be meaningless"
            );
            assert!(
                (staged / plain - 1.0).abs() < 0.03,
                "{surface:?}: composing `trails` restretched the picture by \
                 {:.3}x ({plain:.4} -> {staged:.4}) — the scene is being handed the \
                 internal grid's aspect instead of the target's (ADR-0037)",
                staged / plain
            );
        }
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
