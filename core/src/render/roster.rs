//! The loaded presets, the active index, and the per-binding smoother.
//!
//! GPU-free on purpose: [`Roster`] is the addressing contract (names in roster
//! order, in-range select, out-of-range no-op) as a pure type, so it is testable
//! without a surface, and [`Renderer`]'s preset methods delegate to it 1:1.
//! [`ParamSmoother`] sits here because it is keyed by the same binding index the
//! roster's routes are.

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

/// The loaded presets plus the active index — the pure, GPU-free part of
/// selection. Split out of [`Renderer`] so the addressing contract (names in
/// roster order, in-range select, out-of-range no-op) is unit-testable without a
/// surface, mirroring how the diagnostics stats are a pure type behind the GPU
/// [`Renderer`]. [`Renderer`]'s preset methods delegate here 1:1.
pub(super) struct Roster {
    pub(super) presets: Vec<Preset>,
    /// Resolved [`ParamRoute`]s, one inner `Vec` per preset and one entry per that
    /// preset's bindings, in `Preset::params` order.
    ///
    /// Kept here rather than on the active preset alone because a dissolve
    /// composites **two** presets in one frame (Plan 0023) and both sides want
    /// their routes; indexing by preset means a side's routes cannot drift out of
    /// step with the preset it is showing. Resolution is a render-layer concern
    /// (it names chain positions), which is why it lives on this render-layer type
    /// and not in `preset/`.
    pub(super) routes: Vec<Vec<ParamRoute>>,
    pub(super) active: usize,
}

impl Roster {
    pub(super) fn new(presets: Vec<Preset>) -> Self {
        Self {
            routes: resolve_routes(&presets),
            presets,
            active: 0,
        }
    }

    /// Replace the roster; reset `active` to the start if it now points past the
    /// end. An empty set is ignored — a directory that briefly reads empty or
    /// all-malformed leaves the last good roster rendering (NFR 10).
    pub(super) fn set_presets(&mut self, presets: Vec<Preset>) {
        if presets.is_empty() {
            return;
        }
        self.routes = resolve_routes(&presets);
        self.presets = presets;
        if self.active >= self.presets.len() {
            self.active = 0;
        }
    }

    /// The resolved routes for the preset at `index`, positionally matching its
    /// `params`. Empty for an out-of-range index, which pairs with
    /// `presets.get(index)` returning `None`.
    pub(super) fn routes_for(&self, index: usize) -> &[ParamRoute] {
        self.routes.get(index).map_or(&[], Vec::as_slice)
    }

    /// The active preset's resolved routes.
    pub(super) fn active_routes(&self) -> &[ParamRoute] {
        self.routes_for(self.active)
    }

    /// The index cycling would land on (wrapping), **without** moving there — the
    /// dissolve controller needs the target before the roster flips, because the
    /// dissolve's opening frame still composites the outgoing preset. Returns the
    /// current index on an empty or single-preset roster, which the caller reads as
    /// "nothing to dissolve to".
    pub(super) fn next_index(&self) -> usize {
        if self.presets.is_empty() {
            return self.active;
        }
        (self.active + 1) % self.presets.len()
    }

    /// Set the active preset **iff** `index` is in range; an out-of-range index
    /// is a no-op — never a panic, never a wrap.
    pub(super) fn select(&mut self, index: usize) {
        if index < self.presets.len() {
            self.active = index;
        }
    }

    /// The active preset, or `None` on an empty roster.
    pub(super) fn active_preset(&self) -> Option<&Preset> {
        self.presets.get(self.active)
    }

    /// The active preset's name, or a placeholder on an empty roster.
    pub(super) fn name(&self) -> &str {
        self.active_preset()
            .map(|p| p.name.as_str())
            .unwrap_or("no presets")
    }

    /// The loaded preset names in roster order.
    pub(super) fn names(&self) -> impl Iterator<Item = &str> {
        self.presets.iter().map(|p| p.name.as_str())
    }
}

/// Resolve every preset's bindings to their destinations, off the hot path — once
/// per roster load, not once per binding per frame.
pub(super) fn resolve_routes(presets: &[Preset]) -> Vec<Vec<ParamRoute>> {
    presets
        .iter()
        .map(|preset| {
            preset
                .params
                .iter()
                .map(|binding| resolve_route(&binding.name, preset.system))
                .collect()
        })
        .collect()
}

/// Render-layer one-pole envelope over evaluated parameter values (ADR-0019 /
/// Plan 0018 Phase 5, widened by ADR-0035). Each active-preset binding gets
/// optional exponential smoothing with a per-param [`Easing`] (seconds), applied
/// on the injected real `dt` **between** `expr.eval` and `set_param`, so band-
/// and beat-driven motion eases instead of snapping. The evaluator stays pure
/// and allocation-free — the smoothing state lives here, beside the other
/// per-frame state the expression path has, [`LatchBank`].
///
/// With `attack != release` this is deliberately **not** a linear filter: a
/// direction-dependent time constant rectifies, so a fast-attack parameter rides
/// above its input's mean under sustained material. That is the envelope-follower
/// behavior ADR-0035 exists to provide, not a defect.
///
/// State is keyed by binding **index** (the active preset's `params` are a stable
/// name-sorted `Vec`) and is **reset on every active-preset change** (a switch
/// snaps to the incoming preset's first value — no cross-preset bleed) and on the
/// capture scene-rebuild (so a headless capture stays a pure function of its
/// inputs, NFR 6).
#[derive(Default)]
pub(super) struct ParamSmoother {
    /// Last smoothed value per binding index; grown lazily and seeded with the
    /// first frame's raw value, so the first frame after a reset snaps rather than
    /// drifting up from a stale zero. Cleared on reset.
    pub(super) last: Vec<f32>,
}

impl ParamSmoother {
    /// Forget all state so the next frame snaps to the incoming values.
    pub(super) fn reset(&mut self) {
        self.last.clear();
    }

    /// Smooth `raw` for binding `index` toward its previous value over `dt`
    /// seconds, using whichever of `tau`'s two constants the direction of travel
    /// selects (ADR-0035). A selected constant of `<= 0` (the default) or
    /// non-finite, or a non-positive `dt`, passes `raw` through unchanged. The
    /// first frame after a reset seeds the state with `raw` (a snap).
    pub(super) fn smooth(&mut self, index: usize, raw: f32, tau: Easing, dt: f32) -> f32 {
        if self.last.len() <= index {
            self.last.resize(index + 1, raw);
        }
        let Some(slot) = self.last.get_mut(index) else {
            return raw; // unreachable after the resize; never panics on the hot path
        };
        // The arithmetic itself lives on `Easing` (Plan 0034 Phase 3), so the
        // spectrum scene's per-element smoother eases by the same rule rather
        // than growing a second easing vocabulary beside this one.
        *slot = tau.step(*slot, raw, dt);
        *slot
    }
}

/// The roster-facing half of [`Renderer`]: replacing the preset set, moving the
/// active index, and applying the incoming preset's structural config to its
/// scene. An `impl Renderer` continuation for the same reason `tier_governor` is
/// one -- these read and write `Roster` and nothing else about the device.
impl Renderer {
    /// Replace the preset roster (the standalone's hot-reload path). An empty
    /// set is ignored so a preset directory that briefly reads empty — or whose
    /// files are all malformed — leaves the last good roster rendering (NFR 10).
    pub fn set_presets(&mut self, presets: Vec<Preset>) {
        // A dissolve in flight is targeting an index in the *old* roster, which the
        // replacement may not even have. Cancel it cleanly — the snapshot goes with
        // it — and land on whatever `set_presets` resolves the active index to.
        self.cancel_transition();
        self.reset_transition_rotation();
        self.roster.set_presets(presets);
        self.configure_active_scene();
    }

    /// Switch to the next preset; returns its name. **Dissolves** rather than cuts
    /// (Plan 0023): the outgoing preset's composite is captured on the next frame
    /// and blended out over `DEFAULT_DURATION_SECS` while the incoming one
    /// renders live. Every system is built at startup, so no *scene* is
    /// constructed here; the dissolve's opening frames do allocate its own
    /// resources lazily — see `begin_transition`.
    ///
    /// The returned name is the **incoming** preset's, immediately — the frontend's
    /// HUD should name where the show is going, not where it has been.
    pub fn cycle_preset(&mut self) -> &str {
        // Settle any dissolve in flight *before* reading the roster: "next" must be
        // one past where the show is actually going, not one past where it started.
        // Two switches arriving between two rendered frames therefore advance two
        // presets, as two switches either side of a frame already did.
        self.snap_finish_transition();
        let to = self.roster.next_index();
        self.begin_transition(to);
        self.roster.presets.get(to).map_or("no presets", |p| {
            // Borrowck: the roster is not flipped yet (the capture frame needs the
            // outgoing preset active), so read the incoming name by index.
            p.name.as_str()
        })
    }

    /// The loaded preset names in roster order — the browse overlay's list
    /// source (Plan 0008). Selection addresses these by absolute index.
    pub fn preset_names(&self) -> impl Iterator<Item = &str> {
        self.roster.names()
    }

    /// Switch to the preset at `index` (its absolute position in
    /// [`preset_names`](Self::preset_names)); returns the incoming name. Like
    /// [`cycle_preset`](Self::cycle_preset) this **dissolves** rather than cuts
    /// (Plan 0023 Phase 5) — the browse overlay's select is a switch the operator
    /// watches, so it gets the same treatment as Space. An out-of-range `index` is
    /// a no-op (never a panic, never a wrap), so a stale index from a shrunk
    /// hot-reloaded roster is harmless.
    ///
    /// Use [`select_preset_now`](Self::select_preset_now) where a blend would be
    /// wrong rather than merely unwanted.
    pub fn select_preset(&mut self, index: usize) -> &str {
        self.begin_transition(index);
        // A dissolve has not flipped the roster yet — the opening frame still
        // composites the outgoing preset — so name the incoming one by index, as
        // `cycle_preset` does. `begin_transition` cuts instantly when the index is
        // already active, and no-ops when it is out of range; either way the roster
        // *is* the answer then.
        match self.transition.as_ref().map(Transition::incoming_index) {
            Some(to) => self
                .roster
                .presets
                .get(to)
                .map_or("no presets", |p| &p.name),
            None => self.preset_name(),
        }
    }

    /// Jump to the preset at `index` with **no dissolve** — the instant-cut escape
    /// for paths where a blend is wrong rather than unwanted: a capture, which must
    /// stay a pure function of its inputs (NFR §6), or a test placing the roster on
    /// a known preset before measuring. Returns the now-active name; an
    /// out-of-range `index` is a no-op.
    pub fn select_preset_now(&mut self, index: usize) -> &str {
        self.select_preset_instantly(index);
        self.preset_name()
    }

    /// Make the preset named `name` active, returning whether it was found — the
    /// by-name form of [`select_preset`](Self::select_preset), and like it a
    /// **dissolve**. An unknown name leaves the active preset unchanged.
    pub fn select_preset_by_name(&mut self, name: &str) -> bool {
        let Some(index) = self.preset_names().position(|n| n == name) else {
            return false;
        };
        self.select_preset(index);
        true
    }

    /// The instant-cut form of [`select_preset_by_name`](Self::select_preset_by_name),
    /// used by the capture entry points below.
    pub(super) fn select_preset_by_name_now(&mut self, name: &str) -> bool {
        let Some(index) = self.preset_names().position(|n| n == name) else {
            return false;
        };
        self.select_preset_instantly(index);
        true
    }
    /// Apply the active preset's declarative structural config to its scene, if
    /// it has one (ADR-0007). Called once whenever the active preset changes —
    /// on select/cycle/hot-reload and after a capture rebuilds the scenes — so a
    /// generator builds and caches its geometry exactly once, off the hot path.
    /// A `None` config (fragment/swarm, or a curve on the family default) is a
    /// no-op via the trait's default `configure`.
    pub(super) fn configure_active_scene(&mut self) {
        // Snap the eased params to the incoming preset's first values — no
        // cross-preset bleed, and determinism across capture rebuilds (ADR-0019).
        // The latch bank resets on the same beat and for the same two reasons:
        // an armed window must not cross a preset switch, and a capture has to
        // stay a pure function of its inputs (NFR 6).
        self.param_smoother.reset();
        self.layer_smoother.reset();
        self.latches.reset();
        let Self {
            ctx,
            scenes,
            roster,
            cap_overflow,
            side,
            incoming_side,
            tier,
            budget,
            ..
        } = self;
        *cap_overflow = None;
        let Some(preset) = roster.active_preset() else {
            return;
        };
        let Some(scene) = scene_for_mut(scenes, preset.system) else {
            return;
        };
        // Bake the preset's color palette (default `spectrum` if it declares no
        // `[palette]`) and hand it to the active scene (ADR-0021), off the hot
        // path. A shader-colored scene stores the LUT and uploads it next frame;
        // the spectrum readout samples it on the CPU per element (Plan 0034); the
        // other line scenes ignore it. `spectrum` reproduces the prior cosine, so a
        // palette-less preset is visually unchanged. A `[palette_b]` bakes an A/B
        // pair for the bindable `palette_mix` crossfade.
        let baked = match (preset.palette.as_ref(), preset.palette_b.as_ref()) {
            (Some(a), Some(b)) => Palette::bake_pair(a, b),
            (Some(a), None) => Palette::bake(a),
            (None, Some(b)) => Palette::bake_pair(
                &crate::render::palette::PaletteConfig::default_spectrum(),
                b,
            ),
            (None, None) => Palette::default_spectrum(),
        };
        scene.set_palette(&baked);
        // The backdrop colours through the same bake (ADR-0086) — one gradient,
        // two consumers, no second bake and no drift.
        //
        // It goes to the side that will actually **draw** this preset. During a
        // dissolve that is `incoming_side`, which this call precedes by one frame
        // (the roster flips at the end of the capture frame); `side` is still
        // painting the outgoing preset's backdrop and keeps the gradient it was
        // given, until `promote_incoming_side` makes the incoming one *the* side.
        let live = incoming_side.as_mut().unwrap_or(side);
        live.background.set_palette(&baked);
        // The `[feedback]` table (ADR-0048), to the same side and for the same
        // reason: it is this preset's structural choice, and the outgoing side
        // keeps the one it is still painting with. Handed over unconditionally —
        // a preset with no table hands the default, which is what stops the
        // previous preset's warp surviving a switch.
        live.chain.set_feedback(preset.feedback);
        // ...and to the scene, which is the SECOND sink of the same table
        // (ADR-0048): the attractor's internal trail. Unconditional for the same
        // reason, and a no-op for every other scene.
        scene.set_feedback(preset.feedback);
        // Structural config (ADR-0007), if any: capture segment-cap truncation so
        // the frontend can surface it (never a silent cut). `None` for the
        // fit/no-config case.
        if let Some(cfg) = preset.config.as_ref() {
            *cap_overflow = scene.configure(cfg);
        }
        // The layer's scene is **constructed for the preset** (ADR-0090 point
        // 4, Plan 0076 Phase 2), never resolved from the one-instance-per-
        // system roster — same-system pairs are legal, and two dissolving
        // sides' layers share nothing. It goes to the side that will draw this
        // preset (`live`, exactly like the palette and feedback hand-offs
        // above), constructed fresh at every preset change: a switch is off
        // the hot path, and a fresh deterministic seed is the same contract
        // the roster scenes get from the capture rebuild. Its load-time
        // hand-offs mirror the main scene's — the **shared** palette bake (one
        // gradient, two layers, one world), the default `[feedback]` table (a
        // layer declares none), and its own structural config, whose cap
        // overflow surfaces through the same channel when the main scene
        // produced none (never a silent cut).
        live.layer = preset.layer.as_ref().map(|layer| {
            let mut layer_scene =
                // The **same** ceiling the main scene was built against: a
                // `[layer]` may itself be an attractor, and a layer resolving a
                // different budget than the preset beside it would be two
                // densities in one frame.
                scenes::create_layer_scene(
                    layer.system,
                    &ctx.device,
                    COMPOSITE_FORMAT,
                    tier,
                    *budget,
                );
            layer_scene.set_palette(&baked);
            layer_scene.set_feedback(crate::render::feedback::FeedbackConfig::default());
            if let Some(cfg) = layer.config.as_ref() {
                let overflow = layer_scene.configure(cfg);
                if cap_overflow.is_none() {
                    *cap_overflow = overflow;
                }
            }
            layer_scene
        });
        // The `over` junction's presence and blend mode (ADR-0090 / Plan 0076
        // Phase 3), handed over unconditionally like the `[feedback]` table
        // above: a preset with an `under` (or no) layer hands `None`, which
        // also frees the junction's two full-frame inputs.
        live.chain.set_layer_join(
            preset
                .layer
                .as_ref()
                .and_then(|layer| (layer.join == LayerJoin::Over).then_some(layer.blend)),
        );
    }
}
