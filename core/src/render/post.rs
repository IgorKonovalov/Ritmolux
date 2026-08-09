//! The post-composite chain (ADR-0031, membership revised by ADR-0032): the
//! **per-preset look** stages that run *after* the scene has drawn, folded down to
//! a caller-supplied destination in one fixed order.
//!
//! # The order, and the skip rule
//!
//! The chain is `trails -> kaleidoscope -> bloom -> destination`, built as a
//! compile-time constant array in [`PostChain::new`]. That order is ADR-0018's
//! product decision (feedback before the screen-space fold) extended by ADR-0046
//! (bloom last, so its bright-pass reads the finished HDR frame). This is **not**
//! a render graph and
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
//! # The chain runs in linear light (ADR-0046)
//!
//! Every target in here is [`COMPOSITE_FORMAT`](super::COMPOSITE_FORMAT), not the
//! surface's — the `surface_format` a stage is constructed with is the
//! *composite's* format, and the name is now a historical one. So an additive
//! accumulation is free to exceed 1.0 and no hand-off clips: the frame becomes
//! display-referred once, at the tonemap, downstream of everything here. The
//! memory that costs, and the cap that relieves it, are in
//! [`TierConfig::post_cap`](super::TierConfig::post_cap)'s docs.
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
//! - [`Tonemap`](super::tonemap::Tonemap) — the exposure + tonemap pass, the
//!   frame's linear/display boundary, which reads the one finished *composite*;
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

use super::bloom::Bloom;
use super::kaleidoscope::Kaleidoscope;
use super::trails::Trails;

/// How many stages the chain holds. A compile-time constant, not a capacity:
/// [`PostChain::new`] fills the array exactly, and [`Routing`] is sized from it so
/// a frame's routing costs no allocation.
pub(crate) const STAGE_COUNT: usize = 3;

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
pub(crate) const BLOOM: usize = 2;

/// Each stage's declared parameter vocabulary, **in chain order** — the same
/// consts the stages themselves match on, so there is no second copy to drift.
/// Pinned to the live array by a `debug_assert` in [`PostChain::new`].
///
/// This exists so a binding's owning stage can be resolved **once, at load**
/// ([`stage_for`]) instead of by walking the stages per binding per frame: the
/// answer is fixed the moment a preset is parsed, and a chained
/// `set_param(&str, ..)` fallthrough inside the hot loop is a link every new
/// stage would lengthen (Plan 0031 Phase 3).
pub(crate) const STAGE_PARAMS: [&[&str]; STAGE_COUNT] = [
    super::trails::PARAMS,
    super::kaleidoscope::PARAMS,
    super::bloom::PARAMS,
];

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

/// The chain's own parameter vocabulary — names owned by the **composite seam**
/// rather than by any one stage (ADR-0085).
///
/// Disjoint from [`STAGE_PARAMS`] and resolved ahead of it, because a chain-level
/// name has no stage index to route to: it reaches every stage's fold *and* the
/// scene's own present, which is the one place a backdrop can be occluded when no
/// stage is active at all.
pub const CHAIN_PARAMS: &[&str] = &["occlude"];

/// How much of the scene's coverage the backdrop resolves against, by default:
/// **all of it**, which is the arithmetic every frame ran before `occlude`
/// existed (ADR-0085). A literal `1.0` — the multiply it produces is exact, so
/// an unbound preset renders byte-identically rather than approximately so.
pub const DEFAULT_OCCLUDE: f32 = 1.0;

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

    /// Hand this stage the frame's evaluated `exposure` (ADR-0080).
    ///
    /// **Not a param, and deliberately not routed like one.** `exposure` belongs
    /// to the tonemap: the tonemap owns the name, takes the binding, and is the
    /// only pass that applies it. This is a one-way read for the one stage that
    /// needs to know what the display will be shown — the bloom bright-pass, whose
    /// `bloom_threshold` is meaningless in pre-exposure units once a preset moves
    /// off 1.0. A stage cannot change the value, so the composite's fixed order
    /// (ADR-0018) is unaffected.
    ///
    /// Defaulted to a no-op because two of the three stages have no business with
    /// it, and because a stage added later should have to opt in rather than
    /// remember to ignore it.
    fn set_exposure(&mut self, _exposure: f32) {}

    /// Hand this stage the frame's **real elapsed seconds** (ADR-0019/ADR-0048).
    ///
    /// Travels the same one-way route as [`set_exposure`](Self::set_exposure), and
    /// for the same reason: it is not a preset param — no author binds it, no
    /// expression produces it, and no stage may change it. It is the frontend's
    /// measurement, and the composite's mirror of
    /// [`Scene::advance`](super::scenes::Scene::advance).
    ///
    /// Only the trails stage reads it, because only the trails stage carries state
    /// *between* frames: its `fade` decay and every `fb_*` rate are per-second, so
    /// without this the same preset would decay three times faster at 144 Hz than
    /// at 48. The kaleidoscope and bloom are pure functions of one frame and have
    /// nothing to advance. Defaulted to a no-op so a stage added later opts in
    /// rather than remembers to ignore it.
    fn set_dt(&mut self, _dt: f32) {}

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
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Fold {
    /// `out` is the next stage's input: this stage owns it. Clear it to
    /// transparent first — it holds the **previous frame's** content otherwise,
    /// since stage offscreens are persistent textures, not transient ones.
    Own,
    /// `out` is the chain's destination, where the backdrop is already painted.
    /// Load and blend over it, so `bg_*` survives underneath wherever this stage's
    /// alpha is below 1.
    ///
    /// `occlude` is **how much of that alpha the backdrop resolves against**
    /// (ADR-0085): the blend computes `src.rgb + bg * (1 - src.a * occlude)`, so
    /// `1.0` is coverage-as-occlusion exactly as it has always been and `0.0` is
    /// additive light that never covers. It rides on this variant rather than on
    /// the stage because it is only meaningful where a backdrop is underneath —
    /// there is nothing to occlude in a scratch offscreen.
    Over { occlude: f32 },
}

impl Fold {
    /// The colour attachment load op this fold implies.
    pub(crate) fn load_op(self) -> wgpu::LoadOp<wgpu::Color> {
        match self {
            // TRANSPARENT, not BLACK: an opaque clear would hold the backdrop out
            // of every pixel the scene did not cover.
            Self::Own => wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
            Self::Over { .. } => wgpu::LoadOp::Load,
        }
    }

    /// The factor a stage must multiply its **emitted alpha** by before the blend
    /// sees it (ADR-0085).
    ///
    /// Every stage applies this unconditionally rather than branching on the fold:
    /// [`Own`](Self::Own) is a literal `1.0`, which over a transparent-cleared
    /// target leaves the premultiplied-OVER reduction to `REPLACE` exactly intact.
    /// A stage that read the variant instead would carry a conditional that only
    /// the destination path exercises, which is the shape of bug ADR-0056's
    /// two-seams-one-convention Negative bullet describes.
    pub(crate) fn alpha_scale(self) -> f32 {
        match self {
            Self::Own => 1.0,
            Self::Over { occlude } => occlude,
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
    /// This side's `occlude` (ADR-0085), reset to [`DEFAULT_OCCLUDE`] each frame
    /// and handed to the last active stage through [`Fold::Over`].
    ///
    /// On the chain rather than on a stage because it belongs to the **seam**, not
    /// to whichever stage happens to be last this frame: a preset that turns bloom
    /// off must not thereby change how much its figure covers the sky.
    occlude: f32,
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
                Box::new(Bloom::new(
                    device,
                    surface_format,
                    tier.post_cap,
                    tier.bloom_levels,
                )),
            ],
            occlude: DEFAULT_OCCLUDE,
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
        debug_assert_eq!(
            chain.stage_names().get(BLOOM).copied(),
            Some("bloom"),
            "bloom is last, so its bright-pass sees the folded HDR frame (ADR-0046)"
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
        self.occlude = DEFAULT_OCCLUDE;
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

    /// Hand every stage this frame's evaluated `exposure` (ADR-0080). Only bloom
    /// reads it; see [`PostStage::set_exposure`] for why it travels this way
    /// rather than as a routed param.
    ///
    /// Called once per side per frame, before [`resolve`](Self::resolve) — the
    /// value has to be the crossfaded one a dissolve will actually apply, and that
    /// is not known until the incoming preset's bindings have been routed.
    pub(crate) fn set_exposure(&mut self, exposure: f32) {
        for stage in self.stages.iter_mut() {
            stage.set_exposure(exposure);
        }
    }

    /// Hand every stage this frame's real elapsed seconds (see
    /// [`PostStage::set_dt`]). Only the trails stage reads it.
    ///
    /// Called once per side per frame, beside `Scene::advance` — the chain and the
    /// scene advance on the same measured `dt`, which is the whole basis of
    /// ADR-0019's frame-rate independence.
    pub(crate) fn set_dt(&mut self, dt: f32) {
        for stage in self.stages.iter_mut() {
            stage.set_dt(dt);
        }
    }

    /// Apply a [`CHAIN_PARAMS`] name, returning whether the chain owned it.
    ///
    /// Clamped to `[0, 1]` here rather than in the shaders: past 1 the blend's
    /// `1 - a * occlude` goes negative and *subtracts* the backdrop under the
    /// figure — the Plan 0045 Phase 4b defect, reachable again through a bound
    /// expression. Below 0 it would add the backdrop twice. A `[smoothing]` ease
    /// sweeps this continuously, so the clamp is on the value the frame uses, not
    /// on what the author wrote.
    pub(crate) fn set_chain_param(&mut self, name: &str, value: f32) -> bool {
        if name == "occlude" {
            self.occlude = if value.is_finite() {
                value.clamp(0.0, 1.0)
            } else {
                DEFAULT_OCCLUDE
            };
            return true;
        }
        false
    }

    /// This frame's `occlude` for **this side**. Read by the renderer to hand the
    /// same factor to the scene's own present, which is the composite's other
    /// backdrop-facing seam when no stage is active (ADR-0085).
    pub(crate) fn occlude(&self) -> f32 {
        self.occlude
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
                Fold::Over {
                    occlude: self.occlude,
                }
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
mod tests;
