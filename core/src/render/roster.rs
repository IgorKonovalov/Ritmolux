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

    /// Whether `presets` is this roster **rebound** rather than replaced: the
    /// same presets, in the same order, each still driving the same system, with
    /// only their expressions and easing constants free to differ.
    ///
    /// This is the seam a live editor saves through (ADR-0176). The two things
    /// it tests are the two that make the carried frame state meaningful — a
    /// preset that changed system drives a different scene, and a roster that
    /// changed shape re-points every index — and everything else a save can
    /// touch is re-handed by [`Renderer::set_presets`] either way.
    ///
    /// An empty replacement is never a rebind: [`set_presets`](Self::set_presets)
    /// ignores it entirely, so the caller must take the path that ignores it.
    pub(super) fn is_rebind_of(&self, presets: &[Preset]) -> bool {
        !presets.is_empty()
            && self.presets.len() == presets.len()
            && self
                .presets
                .iter()
                .zip(presets)
                .all(|(held, next)| held.name == next.name && held.system == next.system)
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

/// Why a live parameter override was refused.
///
/// Both arms are *load-time* answers to a question asked over the control
/// surface, so the sender learns immediately that a name will never move —
/// rather than sending at slider rate into a value nothing reads. OSC itself has
/// no reply channel (ADR-0164), which is why this is a `Result` here and a
/// counter at the listener.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParamError {
    /// The roster is empty, so there is no system whose vocabulary could claim
    /// any name at all.
    NoActivePreset,
    /// No backdrop, post stage, terminal pass or scene on the active preset's
    /// system answers to this name — the same verdict
    /// [`ParamRoute::Unclaimed`] records for a binding, taken at the moment the
    /// override is set instead of silently at apply time.
    UnknownParam(String),
}

impl std::fmt::Display for ParamError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoActivePreset => f.write_str("no preset is active"),
            Self::UnknownParam(name) => {
                write!(
                    f,
                    "no parameter named `{name}` on the active preset's system"
                )
            }
        }
    }
}

impl std::error::Error for ParamError {}

/// Live per-name parameter overrides on the active preset (ADR-0176).
///
/// One entry shadows whatever that parameter's binding evaluated to, from the
/// next frame until it is cleared. This is the seam a control surface drags a
/// slider through: the alternative — rewriting the preset file — is around
/// 150 ms late and takes the reload path, which the same ADR is why the reload
/// path learned to keep its state.
///
/// **Keyed by name and carrying its own resolved [`ParamRoute`]**, which is what
/// lets an override reach a parameter the preset does not bind at all: the value
/// is applied after the binding walk, against the route
/// [`resolve_route`] gives the name on the active system. Resolving once at
/// `set` is sound because the active system cannot change under a live
/// override — every path that changes it clears the bank.
///
/// A `Vec` rather than a map, for [`Roster`]'s reason: a surface drags one or
/// two parameters at a time, so a linear walk over what is actually held costs
/// less than hashing a name per frame.
#[derive(Default)]
pub(super) struct ParamOverrides {
    entries: Vec<(String, ParamRoute, f32)>,
}

impl ParamOverrides {
    /// Hold `value` on `name`, replacing whatever this name held before.
    pub(super) fn set(&mut self, name: &str, route: ParamRoute, value: f32) {
        match self.entries.iter_mut().find(|(held, ..)| held == name) {
            Some(entry) => {
                entry.1 = route;
                entry.2 = value;
            }
            None => self.entries.push((name.to_owned(), route, value)),
        }
    }

    /// Drop `name`'s override, so its binding resumes on the next frame. Unknown
    /// names are a no-op — a surface releasing a slider it never held is not an
    /// error.
    pub(super) fn clear(&mut self, name: &str) {
        self.entries.retain(|(held, ..)| held != name);
    }

    /// Drop every override.
    pub(super) fn clear_all(&mut self) {
        self.entries.clear();
    }

    /// The held overrides, in the order they were first set.
    pub(super) fn entries(&self) -> &[(String, ParamRoute, f32)] {
        &self.entries
    }
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
    ///
    /// `None` is **no history yet**, which is what the first frame after a reset
    /// sees and also what [`remap`](Self::remap) leaves in a slot whose parameter
    /// had no counterpart in the outgoing preset. A sentinel float would have to
    /// be one `Easing::step` cannot produce, and there is no such float.
    last: Vec<Option<f32>>,
}

impl ParamSmoother {
    /// Forget all state so the next frame snaps to the incoming values.
    pub(super) fn reset(&mut self) {
        self.last.clear();
    }

    /// Re-key the carried state from binding order `from` to binding order `to`,
    /// **by name**, keeping every eased value whose parameter is still bound.
    ///
    /// State is keyed by binding *index* and a preset's `params` are name-sorted,
    /// so one binding added or removed shifts every index past it. Carrying the
    /// raw `Vec` across such an edit would hand a parameter its neighbour's eased
    /// value — visible as a jump on exactly the save that was meant to change
    /// nothing else. A name with no counterpart lands on `None` and so snaps to
    /// its own first value, which is what a reset would have given it anyway.
    ///
    /// Quadratic in the binding count, and deliberately: this runs once per
    /// hot-reload over the twenty-odd bindings a preset carries, never per frame.
    pub(super) fn remap(&mut self, from: &[&str], to: &[&str]) {
        let carried: Vec<Option<f32>> = to
            .iter()
            .map(|name| {
                from.iter()
                    .position(|prev| prev == name)
                    .and_then(|index| self.last.get(index).copied().flatten())
            })
            .collect();
        self.last = carried;
    }

    /// The value carried in `index`'s slot, or `None` where nothing has been
    /// eased into it yet. The state [`remap`](Self::remap) moves, read back so a
    /// test can assert the move rather than infer it from a rendered frame.
    #[cfg(test)]
    pub(super) fn carried(&self, index: usize) -> Option<f32> {
        self.last.get(index).copied().flatten()
    }

    /// [`remap`](Self::remap) for a **layer's** smoother, whose slots are the
    /// layer's params followed by one more for the bindable `mix` (ADR-0090).
    ///
    /// That extra slot has no entry in `params`, so it cannot ride the same call:
    /// it is carried by hand, and only when both sides declare a `mix` — a layer
    /// that gained or lost one has nothing to carry.
    pub(super) fn remap_layer(&mut self, from: &Layer, to: &Layer) {
        let from_names: Vec<&str> = from.params.iter().map(|b| b.name.as_str()).collect();
        let to_names: Vec<&str> = to.params.iter().map(|b| b.name.as_str()).collect();
        let mix = self.last.get(from_names.len()).copied().flatten();
        self.remap(&from_names, &to_names);
        if from.mix.is_some() && to.mix.is_some() {
            self.last.resize(to_names.len() + 1, None);
            if let Some(slot) = self.last.get_mut(to_names.len()) {
                *slot = mix;
            }
        }
    }

    /// Smooth `raw` for binding `index` toward its previous value over `dt`
    /// seconds, using whichever of `tau`'s two constants the direction of travel
    /// selects (ADR-0035). A selected constant of `<= 0` (the default) or
    /// non-finite, or a non-positive `dt`, passes `raw` through unchanged. The
    /// first frame after a reset seeds the state with `raw` (a snap).
    pub(super) fn smooth(&mut self, index: usize, raw: f32, tau: Easing, dt: f32) -> f32 {
        if self.last.len() <= index {
            self.last.resize(index + 1, None);
        }
        let Some(slot) = self.last.get_mut(index) else {
            return raw; // unreachable after the resize; never panics on the hot path
        };
        // The arithmetic itself lives on `Easing` (Plan 0034 Phase 3), so the
        // spectrum scene's per-element smoother eases by the same rule rather
        // than growing a second easing vocabulary beside this one.
        //
        // No history in the slot means this parameter has never been eased under
        // this preset -- the first frame after a reset, or a binding a rebind
        // brought in -- and it snaps.
        let next = match *slot {
            Some(prev) => tau.step(prev, raw, dt),
            None => raw,
        };
        *slot = Some(next);
        next
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
        // The file is the durable channel and the control socket the live one
        // (ADR-0176). A save is the durable one speaking, so whatever the file
        // now says wins and the live overrides go with it — including on the
        // rebind path below, which is the path a live editor's own save takes.
        self.overrides.clear_all();
        // A save that only rewrote expressions keeps the show running: the eased
        // values, the armed latches and the accumulated fields all carry across,
        // because nothing the scene was built from moved. See `rebind_roster`.
        if self.roster.is_rebind_of(&presets) {
            self.rebind_roster(presets);
            return;
        }
        // A dissolve in flight is targeting an index in the *old* roster, which the
        // replacement may not even have. Cancel it cleanly — the snapshot goes with
        // it — and land on whatever `set_presets` resolves the active index to.
        self.cancel_transition();
        self.reset_transition_rotation();
        self.roster.set_presets(presets);
        self.configure_active_scene();
    }

    /// Swap in a roster that is the current one **rebound** — same presets, same
    /// order, same systems, new expressions — without taking
    /// [`configure_active_scene`](Self::configure_active_scene)'s reset path.
    ///
    /// The full path exists to stop one preset's state bleeding into another's,
    /// and here there is no other preset: this is the same show with its
    /// arithmetic rewritten. So the smoothers keep their eased values, the latch
    /// bank keeps its armed windows and holds, any dissolve in flight keeps
    /// running, and every accumulation — the trails buffer, the attractor's own
    /// field — is simply never touched. What a live editor sees is the parameter
    /// it changed moving and nothing else moving with it; what the full path gave
    /// it was the whole frame snapping on every keystroke.
    ///
    /// The structural tables are still handed over, because a save may have moved
    /// one: the palette is re-baked, `[feedback]` re-delivered, and `configure`
    /// re-run. Those are idempotent for an unchanged table — the attractor
    /// re-seeds only on a family change, and every other `configure` rebuilds
    /// geometry that is a pure function of the table. The **layer scene** is the
    /// one that is not, since it is constructed rather than configured
    /// (ADR-0090), so it is rebuilt only when the layer's system actually
    /// changed.
    fn rebind_roster(&mut self, presets: Vec<Preset>) {
        let active = self.roster.active;
        let Self {
            roster,
            param_smoother,
            layer_smoother,
            latches,
            ..
        } = self;
        // Read out of the OUTGOING roster, before the swap: the carried state is
        // keyed by the index order that roster had.
        let previous_layer = roster
            .presets
            .get(active)
            .and_then(|preset| preset.layer.as_ref())
            .map(|layer| layer.system);
        if let (Some(prev), Some(next)) = (roster.presets.get(active), presets.get(active)) {
            let from: Vec<&str> = prev.params.iter().map(|b| b.name.as_str()).collect();
            let to: Vec<&str> = next.params.iter().map(|b| b.name.as_str()).collect();
            param_smoother.remap(&from, &to);
            latches.remap(&prev.latches, &next.latches);
            match (prev.layer.as_ref(), next.layer.as_ref()) {
                (Some(from), Some(to)) => layer_smoother.remap_layer(from, to),
                // A layer that arrived or left has no state to carry either way.
                _ => layer_smoother.reset(),
            }
        }
        self.roster.set_presets(presets);
        let incoming_layer = self
            .roster
            .active_preset()
            .and_then(|preset| preset.layer.as_ref())
            .map(|layer| layer.system);
        self.hand_over_active_preset(previous_layer != incoming_layer);
    }

    /// Hold `value` on `name` until it is cleared, shadowing whatever the active
    /// preset's binding for it evaluates to (ADR-0176).
    ///
    /// The name does **not** have to be one the preset binds: anything the active
    /// system's vocabulary claims can be driven, and one it does not is refused
    /// here rather than dropped silently at apply time. The override survives
    /// every frame until [`clear_param_override`](Self::clear_param_override),
    /// and is dropped wholesale by a preset switch and by
    /// [`set_presets`](Self::set_presets).
    ///
    /// Not eased. A sender moving a slider is already producing a continuous
    /// path, and a smoother between the two would make the picture lag the hand.
    pub fn set_param_override(&mut self, name: &str, value: f32) -> Result<(), ParamError> {
        let Some(preset) = self.roster.active_preset() else {
            return Err(ParamError::NoActivePreset);
        };
        match resolve_route(name, preset.system) {
            ParamRoute::Unclaimed => Err(ParamError::UnknownParam(name.to_owned())),
            route => {
                self.overrides.set(name, route, value);
                Ok(())
            }
        }
    }

    /// Drop `name`'s override; its binding resumes on the next frame, easing from
    /// wherever the smoother left it rather than from the held value. A name that
    /// holds no override is a no-op.
    pub fn clear_param_override(&mut self, name: &str) {
        self.overrides.clear(name);
    }

    /// Drop every override at once.
    pub fn clear_param_overrides(&mut self) {
        self.overrides.clear_all();
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
        // A switch also drops whatever a control surface was holding: an override
        // names a parameter on the preset it was set against, and the same name
        // on the incoming preset is a different author's decision (ADR-0176).
        self.overrides.clear_all();
        self.hand_over_active_preset(true);
    }

    /// The hand-over itself, without the resets: the baked palette, the
    /// `[feedback]` table and the structural `configure`, delivered to the side
    /// that will draw this preset.
    ///
    /// `rebuild_layer` constructs a fresh `[layer]` scene, which is what a preset
    /// **change** needs (ADR-0090 point 4: a layer is built for the preset, never
    /// resolved from the roster). A rebind passes `false` where the layer's
    /// system did not move, and the standing instance is re-handed the same three
    /// things the main scene is.
    fn hand_over_active_preset(&mut self, rebuild_layer: bool) {
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
        let build_layer = |layer: &Layer, cap_overflow: &mut Option<CapOverflow>| {
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
        };
        match (rebuild_layer, preset.layer.as_ref(), live.layer.as_mut()) {
            // The rebind case: the standing scene keeps its state and takes the
            // same three hand-offs a fresh one would have been built with.
            (false, Some(layer), Some(layer_scene)) => {
                layer_scene.set_palette(&baked);
                layer_scene.set_feedback(crate::render::feedback::FeedbackConfig::default());
                if let Some(cfg) = layer.config.as_ref() {
                    let overflow = layer_scene.configure(cfg);
                    if cap_overflow.is_none() {
                        *cap_overflow = overflow;
                    }
                }
            }
            _ => {
                live.layer = preset
                    .layer
                    .as_ref()
                    .map(|layer| build_layer(layer, cap_overflow));
            }
        }
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
