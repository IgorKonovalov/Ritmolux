//! Where a preset's parameter names go, and which scene answers to a system.
//!
//! Two questions with one answer each and no state: the [`SceneRoster`] lookup
//! that pairs a [`SystemKind`] with the scene instance built for it, and
//! [`resolve_route`], which decides at load time whether a binding reaches the
//! scene, the backdrop, both, the post chain or nothing. Resolving once and
//! storing the answer is what keeps the per-frame path a table walk.

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

/// The built scenes, each paired with the [`SystemKind`] it drives — the roster
/// [`scenes::create_all`] returns. Addressed by kind, never by position, so a
/// scene cannot silently render in another system's slot.
pub(super) type SceneRoster = Vec<(SystemKind, Box<dyn Scene>)>;

/// The scene a preset's `system` drives, or `None` if the roster somehow lacks it
/// (impossible: the roster is built from [`SystemKind::ALL`] by an exhaustive
/// factory). A linear scan over seven `Copy`-enum keys, once per frame — the same
/// cost as the `match` it replaces, so no map.
pub(super) fn scene_for(scenes: &SceneRoster, system: SystemKind) -> Option<&dyn Scene> {
    scenes
        .iter()
        .find(|(kind, _)| *kind == system)
        .map(|(_, scene)| scene.as_ref())
}

/// [`scene_for`], mutably.
pub(super) fn scene_for_mut(
    scenes: &mut SceneRoster,
    system: SystemKind,
) -> Option<&mut Box<dyn Scene>> {
    scenes
        .iter_mut()
        .find(|(kind, _)| *kind == system)
        .map(|(_, scene)| scene)
}

/// What one binding's evaluated value drives, resolved **once when the roster is
/// loaded** (Plan 0031 Phase 3) so the frame loop dispatches on an enum instead of
/// walking a chain of `set_param(&str, ..)` string matches to discover the owner.
///
/// The answer is fixed the moment a preset is parsed: it depends only on the
/// param's name, the preset's system, and which composite stages exist. Adding a
/// stage now costs a `ParamRoute` arm here, not another link in a chained `if`
/// inside the hot loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ParamRoute {
    /// The backdrop pre-pass (`bg_*`), which sits outside the chain (ADR-0031).
    Background,
    /// A post-composite stage, by its fixed index in the chain (ADR-0031).
    Stage(usize),
    /// The **composite seam** itself (`occlude`, ADR-0085) — how much of the
    /// scene's coverage the backdrop resolves against.
    ///
    /// Not a [`Stage`](Self::Stage) because it belongs to no stage: it reaches
    /// whichever stage folds onto the backdrop this frame *and*, when none does,
    /// the scene's own present. So a preset that switches bloom off does not
    /// thereby change how much of its sky the figure covers.
    Composite,
    /// The terminal engine-wide ink pass (`ink_*` / `paper_*`), outside the chain
    /// (ADR-0032).
    Ink,
    /// The engine-wide exposure + tonemap pass (`exposure`), which sits between
    /// the transition blend and ink and is the frame's linear/display boundary
    /// (ADR-0046).
    Tonemap,
    /// The active scene's named-parameter surface.
    Scene,
    /// A `fb_*` name (ADR-0048) on a system whose scene owns an accumulation of
    /// its own — today, only the attractor.
    ///
    /// **One vocabulary, two buffers.** The trails stage transforms the
    /// accumulation every scene composites through; the attractor's internal trail
    /// transforms its own field. Both may be live in one preset, and then one
    /// `fb_rotate` turns both — each about its own buffer, neither about the
    /// other's. This is the second fan-out in this enum and the reason it exists
    /// is the reverse of [`SceneAndBackdrop`](Self::SceneAndBackdrop)'s: there the
    /// name belongs to the *system* and reaches the sky as a courtesy; here it
    /// belongs to a *stage* and reaches the scene because the scene declared it.
    StageAndScene(usize),
    /// A shared colour modulation (`saturation`, `palette_mix`) — the scene's
    /// **and** the backdrop's, from one binding (ADR-0086).
    ///
    /// The only fan-out in this enum, and it is one because the backdrop joined
    /// the scenes' two colour levers rather than declaring its own: an author who
    /// crossfades palettes means the whole frame, not the figure alone. The name
    /// still belongs to the system's vocabulary — `background::PARAMS` does not
    /// claim it — so a system that declared neither reaches neither.
    SceneAndBackdrop,
    /// No owner claimed the name — silently dropped at apply time, exactly as
    /// the old fallthrough dropped it on reaching a scene that ignores it. The
    /// author already heard about it: an unknown name is a **load-time warning**
    /// carried on [`Preset::warnings`] (ADR-0020, Plan 0019). This is where a
    /// future *render-time* diagnostic for the same case would hang.
    Unclaimed,
}

/// Resolve one binding's destination. **Pure** — a lookup over the static stage
/// vocabularies plus the system's own, so it is decidable at load and testable
/// without a GPU.
///
/// The order is the composite order (ADR-0032/0046) and matches the
/// first-owner-wins fallthrough it replaces: the backdrop pre-pass, then the post
/// chain, then the tonemap, then the terminal ink pass, then the scene. The five
/// namespaces are disjoint, so the order is a formality rather than a tie-break —
/// but it is the documented one.
pub(super) fn resolve_route(name: &str, system: SystemKind) -> ParamRoute {
    if background::PARAMS.contains(&name) {
        return ParamRoute::Background;
    }
    // Ahead of the stages, and disjoint from them: a chain-level name has no stage
    // index to route to (ADR-0085).
    if post::CHAIN_PARAMS.contains(&name) {
        return ParamRoute::Composite;
    }
    if let Some(stage) = post::stage_for(name) {
        // The one place a stage name may also reach the scene (ADR-0048): a
        // system whose scene owns an accumulation buffer of its own declares the
        // `fb_*` names too, and then the binding drives both. Tested against the
        // system's own vocabulary, exactly as `SceneAndBackdrop` is, so the
        // fan-out can never conjure a route for a name the scene would drop.
        if feedback::PARAMS.contains(&name) && system.param_names().contains(&name) {
            return ParamRoute::StageAndScene(stage);
        }
        return ParamRoute::Stage(stage);
    }
    if tonemap::PARAMS.contains(&name) {
        return ParamRoute::Tonemap;
    }
    if ink::PARAMS.contains(&name) {
        return ParamRoute::Ink;
    }
    if system.param_names().contains(&name) {
        // The backdrop colours through the preset's palette (ADR-0086), so the
        // two shared modulations move it with the figure. Tested *after* the
        // system's own vocabulary, not before it: a system that declares neither
        // gives neither to the sky, and the fan-out can never conjure a route for
        // a name the scene would have dropped.
        if background::SHARED_COLOUR_PARAMS.contains(&name) {
            return ParamRoute::SceneAndBackdrop;
        }
        return ParamRoute::Scene;
    }
    ParamRoute::Unclaimed
}
