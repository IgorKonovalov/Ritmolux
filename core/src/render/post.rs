//! The post-composite chain (ADR-0031): the stages that run *after* the scene has
//! drawn, folded down to the surface in one fixed order.
//!
//! # The order, and the skip rule
//!
//! The chain is `trails -> kaleidoscope -> ink -> surface`, built as a
//! compile-time constant array in [`PostChain::new`]. That order is ADR-0018's
//! product decision (feedback before the screen-space fold) plus ADR-0028's
//! ink-is-last rule (the tone remap reads the *fully* composited frame). This is
//! **not** a render graph and **not** a registration point: nothing reorders the
//! array at runtime, and the only way to add a stage is to add an array element
//! and a [`PostStage`] impl.
//!
//! Every stage is individually **skippable**: a stage whose amount param is off
//! reports [`active`](PostStage::active) `false` and is dropped from the frame
//! entirely — no offscreen, no pipeline, nothing encoded. So the routing is a
//! function of the active flags alone:
//!
//! - the background + scene render into the **first active** stage's input, or
//!   straight into the surface when no stage is active;
//! - each active stage folds into the **next active** stage's input;
//! - the **last active** stage folds into the surface.
//!
//! Skipped stages are simply not in that walk, so trails folds directly into ink
//! when the kaleidoscope is off. That adjacency is [`route`] — a **pure function**
//! over the flags, with no GPU and no `self`, so the contract is unit-testable
//! (the tests at the bottom of this file are the composite's first real coverage).
//!
//! # Why a value, not a set of fields
//!
//! [`PostChain`] owns its stages, so a **second chain with fully independent GPU
//! state is constructible** against the same device — each stage builds its own
//! offscreens (and, for trails, its own [`PingPongField`](super::feedback::PingPongField))
//! lazily, on first use. That is what Plan 0023's dual-live transition path needs:
//! two fully-composited frames in one frame, each side with its own feedback
//! history.
//!
//! [`Background`](super::background::Background) is deliberately **not** a
//! `PostStage`: it is a pre-pass that owns the frame clear and never folds a
//! rendered frame down, so the renderer drives it directly, ahead of the chain.

// Hot-path panic-denial pragma (Plan 0002 Phase 2; render/ is scanned by the
// hygiene guard). The chain routes and encodes every displayed frame.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use super::ink::Ink;
use super::kaleidoscope::Kaleidoscope;
use super::trails::Trails;

/// How many stages the chain holds. A compile-time constant, not a capacity:
/// [`PostChain::new`] fills the array exactly, and [`Routing`] is sized from it so
/// a frame's routing costs no allocation.
pub(crate) const STAGE_COUNT: usize = 3;

/// Composite positions, in chain order. Named so the routing tests and the
/// ADR-0018/0028 ordering claims read as assertions rather than magic indices.
pub(crate) const TRAILS: usize = 0;
pub(crate) const KALEIDOSCOPE: usize = 1;
pub(crate) const INK: usize = 2;

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
    /// chain stops at the first owner, so the stage namespaces must stay disjoint.
    fn set_param(&mut self, name: &str, value: f32) -> bool;

    /// Whether this stage runs this frame. `false` skips it entirely — the
    /// passthrough that keeps an unbound preset paying nothing.
    fn active(&self) -> bool;

    /// This stage's fixed internal resolution, or `None` to size from the surface.
    /// The trails and kaleidoscope stages run at a fixed 16:9 grid; the ink remap
    /// is 1:1 per-pixel, so it must match the surface.
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

    /// The stage the background + scene render into, or `None` for the surface
    /// (no stage is active this frame).
    pub(crate) fn scene_stage(&self) -> Option<usize> {
        self.active_stages().first().copied()
    }

    /// Each active stage paired with what it folds into: the next active stage,
    /// or `None` for the surface. The last active stage always yields `None`.
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
/// input, or the surface when the chain is entirely skipped.
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

/// The post-composite stages in ADR-0018/0028 order — see the module docs.
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
            // ADR-0018's feedback-then-fold order, with ADR-0028's ink last.
            stages: [
                Box::new(Trails::new(device, surface_format)),
                Box::new(Kaleidoscope::new(device, surface_format)),
                Box::new(Ink::new(device, surface_format)),
            ],
        };
        // The array above *is* the composite order, and the routing contract is
        // written against the positions below (module docs, ADR-0018/0028). Assert
        // they still hold, so reordering the literal trips here in a debug/test
        // build instead of silently re-composing every preset.
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
            chain.stage_names().get(INK).copied(),
            Some("ink"),
            "the tone remap reads the fully composited frame, so it is last (ADR-0028)"
        );
        chain
    }

    /// Reset every stage's params to their defaults (once per frame, before the
    /// active preset's bindings are routed).
    pub(crate) fn reset_params(&mut self) {
        for stage in self.stages.iter_mut() {
            stage.reset_params();
        }
    }

    /// Offer one named param to the chain, returning whether a stage owned it.
    /// **First owner wins**, in chain order; the renderer falls through to the
    /// scene when this returns `false`.
    pub(crate) fn set_param(&mut self, name: &str, value: f32) -> bool {
        for stage in self.stages.iter_mut() {
            if stage.set_param(name, value) {
                return true;
            }
        }
        false
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
    /// active stage's resources if needed; falls back to the surface when the
    /// chain is skipped entirely.
    pub(crate) fn begin(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        surface_view: &wgpu::TextureView,
        surface: (u32, u32),
    ) -> SceneTarget {
        let routing = self.routing();
        let first = routing.scene_stage().and_then(|index| {
            let stage = self.stages.get_mut(index)?;
            let size = stage.internal_size().unwrap_or(surface);
            let view = stage.begin(encoder, surface)?;
            Some((view, size))
        });
        let (view, size) = first.unwrap_or_else(|| (surface_view.clone(), surface));
        SceneTarget {
            view,
            aspect: size.0 as f32 / size.1.max(1) as f32,
            size,
        }
    }

    /// Fold the chain down to the surface: each active stage resolves into the
    /// next active stage's input, the last into `surface_view`. Returns the total
    /// draw calls the stages encoded. Call once, after the scene has rendered into
    /// [`begin`](Self::begin)'s target.
    pub(crate) fn resolve(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        surface_view: &wgpu::TextureView,
        surface: (u32, u32),
    ) -> u32 {
        let routing = self.routing();
        let mut draw_calls = 0;
        for (stage, destination) in routing.edges() {
            // The next active stage's input, built lazily right before anything
            // renders into it; the surface when this is the last active stage.
            let out = destination
                .and_then(|next| self.stages.get_mut(next)?.begin(encoder, surface))
                .unwrap_or_else(|| surface_view.clone());
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

    // Test asserts index and panic freely; this is not the render path.
    #![allow(clippy::indexing_slicing, clippy::panic)]

    use super::{INK, KALEIDOSCOPE, Routing, STAGE_COUNT, TRAILS, route};

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

    /// No stage active: the scene renders straight to the surface and nothing
    /// folds. The passthrough every shipped preset takes today.
    #[test]
    fn no_active_stage_renders_the_scene_to_the_surface() {
        let routing = route(&[false, false, false]);
        assert_eq!(
            routing.scene_stage(),
            None,
            "with no stage active the scene targets the surface"
        );
        assert!(edges(&routing).is_empty(), "nothing to fold down");
    }

    /// One stage active: the scene renders into it and it resolves to the surface.
    /// Asserted for each position, since the ladder had a separate branch per stage.
    #[test]
    fn a_single_active_stage_resolves_to_the_surface() {
        for stage in [TRAILS, KALEIDOSCOPE, INK] {
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
                "the only active stage folds into the surface"
            );
        }
    }

    /// All three active: trails folds into the kaleidoscope's input, the
    /// kaleidoscope into ink's, ink into the surface — ADR-0018's order with
    /// ADR-0028's ink last.
    #[test]
    fn all_three_active_fold_in_composite_order() {
        let routing = route(&[true, true, true]);
        assert_eq!(routing.scene_stage(), Some(TRAILS));
        assert_eq!(
            edges(&routing),
            vec![
                (TRAILS, Some(KALEIDOSCOPE)),
                (KALEIDOSCOPE, Some(INK)),
                (INK, None),
            ]
        );
    }

    /// A skipped middle stage is not a gap: trails folds **directly** into ink's
    /// input. This is the case the ladder answered with a nested `else if`.
    #[test]
    fn a_skipped_middle_stage_folds_straight_past_it() {
        let routing = route(&[true, false, true]);
        assert_eq!(routing.scene_stage(), Some(TRAILS));
        assert_eq!(edges(&routing), vec![(TRAILS, Some(INK)), (INK, None)]);
    }

    /// The invariant, over every combination: whatever runs, the last active stage
    /// targets the surface — the composite always terminates there exactly once.
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
                    "the last active stage folds into the surface for {active:?}"
                );
            }
            assert!(
                to_surface <= 1,
                "at most one stage may target the surface for {active:?}"
            );
        }
    }

    /// The invariant ADR-0028 names: ink, when active, is always the final stage —
    /// it reads the *fully* composited frame, so nothing may fold after it.
    #[test]
    fn ink_when_active_is_always_last() {
        for active in all_combinations() {
            let routing = route(&active);
            if !active[INK] {
                assert!(
                    !routing.active_stages().contains(&INK),
                    "an inactive ink stage never runs for {active:?}"
                );
                continue;
            }
            assert_eq!(
                routing.active_stages().last().copied(),
                Some(INK),
                "ink is the last active stage for {active:?}"
            );
            assert_eq!(
                edges(&routing).last().map(|(stage, dest)| (*stage, *dest)),
                Some((INK, None)),
                "ink folds into the surface for {active:?}"
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
