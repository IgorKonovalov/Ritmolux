//! One frame's binding evaluation: the latch bank, the scratch surfaces a
//! per-element or per-vertex binding writes into, and the functions that walk a
//! preset's bindings into a scene.
//!
//! Everything here is a pure function of its inputs plus the smoother's own
//! carried state -- no wall clock, no GPU. The clock and the analysis frame
//! arrive in [`FrameInputs`], which both sides of a dissolve share.

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

/// What a latch expression counts as true — the engine's edge-trigger level,
/// the same `0.5` `shape_collage`'s `recompose` rises past.
///
/// One number for both `arm` and `fire` so an author writes one kind of
/// condition. Every comparison in the grammar already yields exactly `0.0` or
/// `1.0`, so the threshold only matters for an expression that hands over a
/// continuous value, and there `0.5` is the midpoint.
pub(super) const LATCH_TRUE: f32 = 0.5;

/// One latch's state between frames (ADR-0137).
#[derive(Clone, Copy, Default)]
pub(super) struct LatchState {
    /// Whether this latch may still fire in its current arming window. Cleared
    /// by a fire; set again only by a **rising** edge of `arm`, which is what
    /// makes "at most one rise per window" a property rather than a hope.
    pub(super) armed: bool,
    /// Remaining hold, in seconds. Counted down on the injected real `dt`.
    pub(super) hold_left: f32,
    /// Last frame's `arm` truth, for the window edge.
    pub(super) arm_last: bool,
    /// Last frame's `fire` truth, for the trigger edge.
    pub(super) fire_last: bool,
}

/// Per-preset `[latch]` state (ADR-0137) — the one part of the expression path
/// whose value depends on frame history.
///
/// **Held here, beside [`ParamSmoother`], for the reason easing is.** One
/// compiled expression is evaluated many times per frame in this engine — once
/// per mesh vertex for a `[per_vertex]` binding, once per element for one naming
/// `index` — so state inside the compiled tree would need one copy *per
/// evaluation* and there is no correct single answer for what the 400th vertex
/// of a frame should see. The evaluator stays a pure function of its
/// [`Variables`] and stays re-entrant; this writes the reserved variable slots
/// once per frame, before the params that read them.
///
/// Fixed arrays, not a `Vec`: the reserved block is [`LATCH_CAP`] wide by
/// construction and the loader rejects a preset asking for more, so a latch
/// index is in range by that boundary check and nothing here allocates on the
/// frame path. Reset on a preset switch alongside the smoothers, so no armed
/// state crosses from the outgoing preset into the incoming one.
#[derive(Default)]
pub(super) struct LatchBank {
    pub(super) state: [LatchState; LATCH_CAP],
    /// This frame's outputs, contiguous so
    /// [`Variables::with_latches`] takes a slice without a copy into scratch.
    pub(super) values: [f32; LATCH_CAP],
}

impl LatchBank {
    /// Forget every window and hold, so the next frame starts disarmed and at
    /// rest.
    pub(super) fn reset(&mut self) {
        *self = Self::default();
    }

    /// Advance every latch of `latches` by `dt` against `vars`, and return
    /// `vars` with the reserved slots bound to this frame's outputs.
    ///
    /// **Call this exactly once per preset per frame**, before anything that
    /// evaluates that preset's bindings: it consumes the `fire` edge, so a
    /// second call in the same frame would see `fire_last` already true and the
    /// rise would be silently swallowed. A preset's main bindings, its
    /// `[per_vertex]` table and its `[layer]` all read the returned bundle.
    ///
    /// A preset with no latches returns `vars` untouched and does no work, which
    /// is every preset that declares no table.
    pub(super) fn advance<'v>(
        &mut self,
        latches: &[Latch],
        vars: Variables<'v>,
        dt: f32,
    ) -> Variables<'v> {
        if latches.is_empty() {
            return vars;
        }
        for ((latch, state), value) in latches
            .iter()
            .zip(self.state.iter_mut())
            .zip(self.values.iter_mut())
        {
            // The elapsed time is consumed **before** the edges are read, so a
            // hold that runs out does so at the top of the frame it runs out in
            // and a fire in that same frame re-arms it to its full length rather
            // than to what was left. `dt` is the injected real frame time
            // (ADR-0014), which is the whole of `hold` being a duration rather
            // than a frame count.
            state.hold_left = (state.hold_left - dt.max(0.0)).max(0.0);

            let arm_now = latch.arm.eval(&vars) > LATCH_TRUE;
            let fire_now = latch.fire.eval(&vars) > LATCH_TRUE;
            if arm_now && !state.arm_last {
                state.armed = true;
            }
            let fired = state.armed && arm_now && fire_now && !state.fire_last;
            if fired {
                state.armed = false;
                state.hold_left = latch.hold;
            }
            state.arm_last = arm_now;
            state.fire_last = fire_now;
            // The firing frame reads `1.0` whatever the hold is, so `hold = 0`
            // is a one-frame pulse rather than a rise no binding could see.
            *value = f32::from(fired || state.hold_left > 0.0);
        }
        vars.with_latches(self.values.get(..latches.len()).unwrap_or(&self.values))
    }
}

/// How much of the per-element scratch `preset` uses this frame: its
/// `[spectrum] elements`, or `0` for a system with no per-element surface
/// (Plan 0034 Phase 4). Bounded by `capacity` so a config can never index past
/// the scratch, whatever the loader admitted.
pub(super) fn element_prefix(preset: &Preset, capacity: usize) -> usize {
    config_element_prefix(preset.config.as_ref(), capacity)
}

/// [`element_prefix`] over a bare config — shared with the layer's own
/// structural config (Plan 0076 Phase 1), which is preset data of exactly the
/// same shape but not a whole [`Preset`].
pub(super) fn config_element_prefix(
    config: Option<&scenes::GeneratorConfig>,
    capacity: usize,
) -> usize {
    config
        .map_or(0, scenes::GeneratorConfig::element_count)
        .min(capacity)
}

/// The **clamped** warp-mesh grid `config` asks for, and how much of the
/// per-vertex scratch that needs — or `None` for a config with no per-vertex
/// surface (Plan 0100 Phase 1).
///
/// The clamp is [`warp_mesh::clamp_grid`](scenes::warp_mesh::clamp_grid), the
/// same function the scene calls on the same request and the same tier. That
/// shared call is the whole contract: the series this renderer sends and the
/// vertex buffer that scene assembles are the same length because one function
/// decides both.
pub(super) fn vertex_grid(
    config: Option<&scenes::GeneratorConfig>,
    tier: &TierConfig,
    capacity: usize,
) -> Option<((u32, u32), usize)> {
    let scenes::GeneratorConfig::WarpMesh { mesh, .. } = config? else {
        return None;
    };
    let mesh = scenes::warp_mesh::clamp_grid(*mesh, tier);
    let count = scenes::warp_mesh::vertex_count(mesh);
    (count <= capacity).then_some((mesh, count))
}

/// Evaluate `expr` once **per element**, binding each element's normalized
/// `0..1` position to `index`, into `out` (Plan 0034 Phase 4).
///
/// Normalized rather than an integer count so an expression composes without
/// knowing how many elements there are: `bin(index)` reads the whole spectrum
/// across the figure at any count. The first element is `0` and the last `1`; a
/// single element is `0` (there is no span to normalize over).
///
/// Pure and allocation-free — `out` is the renderer's scratch, sized at preset
/// load. Split out of [`evaluate_preset`] so the per-element contract is
/// testable without a GPU.
pub(super) fn evaluate_series(expr: &Expr, vars: &Variables<'_>, out: &mut [f32]) {
    let last = out.len().saturating_sub(1);
    for (i, slot) in out.iter_mut().enumerate() {
        let t = if last == 0 {
            0.0
        } else {
            i as f32 / last as f32
        };
        *slot = expr.eval(&vars.with_index(t));
    }
}

/// The per-vertex evaluation surface one frame hands a preset (Plan 0100
/// Phase 1): the grid to walk, the aspect to compute `rad`/`ang` in, and the
/// renderer's scratch to write into.
///
/// A bundle rather than three more arguments on [`evaluate_preset`], which is
/// already at the argument-count lint — and the three genuinely travel together:
/// none of them means anything without the other two.
pub(super) struct VertexSurface<'a> {
    /// The **clamped** grid, in cells. Both this and the scene's own grid come
    /// out of [`warp_mesh::clamp_grid`](scenes::warp_mesh::clamp_grid) on the same
    /// request and the same tier, so the series is exactly as long as the scene
    /// expects.
    pub(super) mesh: (u32, u32),
    /// The **render target's** aspect (ADR-0037), never the mesh grid's. The
    /// target's aspect is the surface's whatever internal grid the chain routes
    /// through (`PostChain::begin`), so the renderer can compute it here from the
    /// frame's own size.
    pub(super) aspect: f32,
    /// The renderer's scratch, sliced to `vertex_count(mesh)`. Empty for every
    /// preset with no per-vertex surface, which is what makes their path
    /// unchanged.
    pub(super) buf: &'a mut [f32],
}

/// Evaluate `expr` once **per mesh vertex**, binding that vertex's `x`, `y`,
/// `rad` and `ang`, into `surface.buf` (Plan 0100 Phase 1).
///
/// Row-major from the top-left, which is the order
/// [`Scene::set_per_vertex`](scenes::Scene::set_per_vertex) documents and the
/// warp mesh assembles its vertex buffer in.
///
/// Pure and allocation-free — the buffer is the renderer's scratch, sized at
/// construction. Split out for [`evaluate_series`]'s reason: the per-vertex
/// contract is then testable without a GPU.
pub(super) fn evaluate_vertex_series(
    expr: &Expr,
    vars: &Variables<'_>,
    surface: &mut VertexSurface<'_>,
) {
    let (mx, my) = surface.mesh;
    let mut v = 0usize;
    for row in 0..=my {
        for col in 0..=mx {
            let (x, y, rad, ang) =
                scenes::warp_mesh::vertex_position(col, row, (mx, my), surface.aspect);
            let Some(slot) = surface.buf.get_mut(v) else {
                return;
            };
            *slot = expr.eval(&vars.with_vertex(x, y, rad, ang));
            v += 1;
        }
    }
}

/// Which of a preset's two salts this frame's `hash()`/`noise()` calls mix in
/// (ADR-0051).
///
/// A **parameter**, not renderer state, and deliberately: it is threaded from
/// each entry point down to the evaluation, so the compiler — not a reviewer —
/// is what guarantees every capture path pins. A flag on `Renderer` could be
/// forgotten at one of the five capture call sites and nothing would say so; the
/// frames would just quietly stop reproducing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SaltMode {
    /// The live app: a preset declaring `seed = "random"` renders under the salt
    /// it drew at load, so it looks different every launch.
    Live,
    /// Every capture path — `capture_preset`, `capture_preset_over`,
    /// `capture_frame`, `capture_audio`, and the warm-up steps under them. Forces
    /// the declared numeric seed so the harness stays a pure function of its
    /// inputs (the same discipline ADR-0045 applies to quality tiers).
    Pinned,
}

impl SaltMode {
    /// The salt `preset` evaluates under in this mode.
    pub(super) fn of(self, preset: &Preset) -> u32 {
        match self {
            SaltMode::Live => preset.salt,
            SaltMode::Pinned => preset.pinned_salt,
        }
    }
}

/// Evaluate one preset's bindings into a composite side, an optional ink pass,
/// and its scene. **Routing only** — nothing is encoded here, because the frame's
/// destination is not known until ink's activity is (ADR-0032).
///
/// `terminal` is `None` for a dual-live dissolve's outgoing side: the tonemap and
/// ink are each **one** engine-wide pass over the blended frame, and both belong
/// to the active preset, whose crossfade the caller applies afterwards. An
/// `exposure` or `ink_*` binding on that side is therefore dropped — the same
/// no-op it was when it fell through to a scene that ignores unknown params.
///
/// `routes` carries each binding's destination, resolved once at roster load
/// ([`ParamRoute`]); it is positionally paired with `preset.params`, so a binding
/// with no route entry is skipped rather than mis-routed.
pub(super) fn evaluate_preset(
    active: Active<'_>,
    scene: &mut Box<dyn Scene>,
    side: &mut CompositeSide,
    terminal: Option<Terminal<'_>>,
    smoother: &mut ParamSmoother,
    inputs: &FrameInputs<'_>,
    scratch: Scratch<'_>,
) {
    let Active { preset, routes } = active;
    let &FrameInputs {
        vars,
        frame,
        time,
        dt,
    } = inputs;
    let Scratch { series, vertex } = scratch;
    scene.set_time(time);
    scene.advance(dt);
    // The composite advances on the same measured `dt` the scene does (ADR-0048):
    // the trails accumulation is the one post stage with state between frames, and
    // its decay and `fb_*` rates are per-second.
    side.chain.set_dt(dt);
    side.reset_params();
    scene.reset_params();
    let mut terminal = terminal;
    for (index, (binding, route)) in preset.params.iter().zip(routes).enumerate() {
        // A binding that names `index` is asking to be evaluated once per
        // element (Plan 0034 Phase 4). The test is a `bool` read decided at
        // compile, and `series` is empty for every system without a per-element
        // surface — so for the other seven systems this branch is never taken
        // and the path below is the one that ran before this existed.
        //
        // Tested *before* the scalar evaluation, not inside the routing match:
        // the scalar `eval` would be the same expression a second time (at
        // `index = 0`) and its result is not read, so evaluating it would make a
        // per-element binding cost N + 1 evaluations instead of N. The smoother
        // is skipped with it — a per-element binding's `tau` is always
        // `INSTANT` (the loader forces it and warns), so its slot was a
        // passthrough nothing read.
        if matches!(*route, ParamRoute::Scene | ParamRoute::SceneAndBackdrop)
            && !series.is_empty()
            && binding.expr.uses_index()
        {
            // The scene eases the element levels itself through
            // `[spectrum] smoothing`; a series has no single value for the
            // per-binding smoother to hold.
            evaluate_series(&binding.expr, vars, series);
            scene.set_param_series(&binding.name, series);
            // One backdrop has no elements to vary across, so it takes element
            // 0 — the same fallback `set_param_series` already gives a
            // whole-figure param, which is what `saturation` and `palette_mix`
            // are on every scene that has a per-element surface.
            if matches!(*route, ParamRoute::SceneAndBackdrop)
                && let Some(&first) = series.first()
            {
                side.background
                    .set_shared_colour_param(&binding.name, first);
            }
            continue;
        }
        let raw = binding.expr.eval(vars);
        // Ease the evaluated value on the injected real `dt` before applying it
        // (ADR-0019). `tau` came off the preset's `[smoothing]` table at load;
        // `0` (the default for an unlisted param) passes through instantly. The
        // evaluation above is a pure function of `vars` and allocates nothing —
        // including a latch, whose history was folded into `vars` before this
        // loop opened (ADR-0137).
        let value = smoother.smooth(index, raw, binding.tau, dt);
        // Dispatch on the resolved destination — no map lookup, no walk over the
        // stages, no chained fallthrough. The owner was decided at load.
        match *route {
            ParamRoute::Background => {
                side.background.set_param(&binding.name, value);
            }
            ParamRoute::Stage(stage) => {
                side.chain.set_stage_param(stage, &binding.name, value);
            }
            ParamRoute::Composite => {
                side.chain.set_chain_param(&binding.name, value);
            }
            ParamRoute::Tonemap => {
                if let Some(terminal) = terminal.as_mut() {
                    terminal.tonemap.set_param(&binding.name, value);
                }
            }
            ParamRoute::Ink => {
                if let Some(terminal) = terminal.as_mut() {
                    terminal.ink.set_param(&binding.name, value);
                }
            }
            ParamRoute::Scene => {
                scene.set_param(&binding.name, value);
            }
            ParamRoute::StageAndScene(stage) => {
                side.chain.set_stage_param(stage, &binding.name, value);
                scene.set_param(&binding.name, value);
            }
            ParamRoute::SceneAndBackdrop => {
                scene.set_param(&binding.name, value);
                side.background
                    .set_shared_colour_param(&binding.name, value);
            }
            // Nothing consumes it. Surfaced at load, silent here (ADR-0020).
            ParamRoute::Unclaimed => {}
        }
    }
    // The `[per_vertex]` table (Plan 0100 Phase 1), after the scalars — so a
    // per-vertex binding overrides the scalar of the same name for this frame
    // rather than racing it. `None` for every preset with no per-vertex surface,
    // and `per_vertex` is empty for every preset that declares no table, so the
    // other ten systems take exactly the path they took before this existed.
    if let Some(mut surface) = vertex {
        for binding in &preset.per_vertex {
            evaluate_vertex_series(&binding.expr, vars, &mut surface);
            scene.set_per_vertex(&binding.name, surface.buf);
        }
    }
    scene.update(frame);
}

/// Evaluate a preset's `[layer]` bindings into the layer's scene (Plan 0076
/// Phase 1). The layer's counterpart of [`evaluate_preset`], and deliberately
/// narrower: layer params are **namespaced to the layer's scene** (ADR-0090) —
/// no route dispatch, because there is nowhere else a layer binding can go. The
/// compositing stages, the terminal passes and the backdrop all belong to the
/// preset as a whole and take their values from the top-level bindings.
///
/// `series` is the renderer's per-element scratch, sliced to the **layer's own**
/// config (a layer spectrum reads its `[layer.spectrum] elements`); empty for
/// every layer system without a per-element surface, exactly as at the top
/// level.
///
/// `chain` receives the layer's bindable `mix` (ADR-0090 / Plan 0076 Phase 3):
/// it is the one layer binding that drives the composite rather than the
/// scene — the `over` junction's amount — eased through its own
/// `[layer.smoothing] mix` entry at the smoother slot one past the params
/// (which is stable: the layer's `params` are fixed at load). Inert on an
/// `under` join, whose target has no junction.
pub(super) fn evaluate_layer(
    layer: &Layer,
    scene: &mut Box<dyn Scene>,
    chain: &mut PostChain,
    smoother: &mut ParamSmoother,
    inputs: &FrameInputs<'_>,
    scratch: Scratch<'_>,
) {
    let &FrameInputs {
        vars,
        frame,
        time,
        dt,
    } = inputs;
    let Scratch { series, vertex } = scratch;
    scene.set_time(time);
    scene.advance(dt);
    scene.reset_params();
    for (index, binding) in layer.params.iter().enumerate() {
        if !series.is_empty() && binding.expr.uses_index() {
            evaluate_series(&binding.expr, vars, series);
            scene.set_param_series(&binding.name, series);
            continue;
        }
        let raw = binding.expr.eval(vars);
        let value = smoother.smooth(index, raw, binding.tau, dt);
        scene.set_param(&binding.name, value);
    }
    if let Some(mut surface) = vertex {
        for binding in &layer.per_vertex {
            evaluate_vertex_series(&binding.expr, vars, &mut surface);
            scene.set_per_vertex(&binding.name, surface.buf);
        }
    }
    if let Some(mix) = layer.mix.as_ref() {
        let raw = mix.expr.eval(vars);
        let value = smoother.smooth(layer.params.len(), raw, mix.tau, dt);
        chain.set_layer_mix(value);
    }
    scene.update(frame);
}

/// The active preset and the routes resolved for it.
///
/// One struct because they are read out of the roster at the **same index**, and
/// a route list paired with the wrong preset mis-routes every binding silently.
#[derive(Clone, Copy)]
pub(super) struct Active<'a> {
    pub(super) preset: &'a Preset,
    pub(super) routes: &'a [ParamRoute],
}

/// One frame's shared evaluation inputs: the variable bundle a binding is
/// evaluated against, the analysis frame the scene is updated with, and the
/// clock and elapsed time every rate is scaled by.
///
/// `vars` rides **by borrow** (ADR-0036): the bundle is built once per frame and
/// read once per binding, so a by-value spectrum would put a 256-byte copy on
/// the per-binding path.
#[derive(Clone, Copy)]
pub(super) struct FrameInputs<'a> {
    pub(super) vars: &'a Variables<'a>,
    pub(super) frame: &'a AnalysisFrame,
    pub(super) time: f32,
    pub(super) dt: f32,
}

/// The renderer's two per-frame scratch buffers, each already sliced to what
/// this preset uses. The series is empty and the surface `None` for a preset
/// with no per-element and no per-vertex binding, which is most of them.
pub(super) struct Scratch<'a> {
    pub(super) series: &'a mut [f32],
    pub(super) vertex: Option<VertexSurface<'a>>,
}

/// One side of the frame: the preset it draws, the scene and composite side it
/// draws through, its two smoothers, and the latch bank that has followed it
/// since before the switch.
pub(super) struct Side<'a> {
    pub(super) active: Active<'a>,
    pub(super) scene: &'a mut Box<dyn Scene>,
    pub(super) composite: &'a mut CompositeSide,
    /// The display-referred pair, routed for the **active** preset only. The
    /// outgoing side's `exposure`/`ink_*` were held at the capture frame and are
    /// crossfaded by the single engine-wide pass of each (ADR-0080).
    pub(super) terminal: Option<Terminal<'a>>,
    pub(super) smoother: &'a mut ParamSmoother,
    pub(super) layer_smoother: &'a mut ParamSmoother,
    pub(super) latches: &'a mut LatchBank,
}

/// What both sides of a dissolve share. `vars` is the **unsalted** bundle: the
/// salt is a fact about a preset rather than about the audio, so each side
/// re-salts it with its own (ADR-0051).
pub(super) struct SideInputs<'a> {
    pub(super) tier: &'a TierConfig,
    pub(super) series: &'a mut [f32],
    pub(super) vertex: &'a mut [f32],
    /// The **render target's** aspect, never an internal grid's (ADR-0037).
    pub(super) aspect: f32,
    pub(super) vars: Variables<'a>,
    pub(super) frame: &'a AnalysisFrame,
    pub(super) time: f32,
    pub(super) dt: f32,
    pub(super) salt: SaltMode,
}

/// The per-vertex scratch as a surface for `config`'s grid, or `None` when the
/// preset has no per-vertex surface or the scratch cannot cover its grid.
///
/// One function rather than the same `and_then` closure written at each call
/// site: the closure carries the aspect, and an aspect taken from anything but
/// the render target is ADR-0037's bug.
pub(super) fn vertex_surface<'a>(
    config: Option<&scenes::GeneratorConfig>,
    tier: &TierConfig,
    scratch: &'a mut [f32],
    aspect: f32,
) -> Option<VertexSurface<'a>> {
    let (mesh, count) = vertex_grid(config, tier, scratch.len())?;
    Some(VertexSurface {
        mesh,
        aspect,
        buf: scratch.get_mut(..count)?,
    })
}

/// Evaluate one side's preset and, if it declares one, its layer.
///
/// The latches advance **once** here, so the `fire` edge is consumed once and
/// the preset's own bindings and its layer's read the same bundle. Both sides of
/// a dual-live dissolve run through this; they differ in which preset, which
/// smoothers, which latch bank and whether a terminal is routed, which is what
/// the two argument structs carry.
pub(super) fn evaluate_side(side: Side<'_>, shared: &mut SideInputs<'_>) {
    let Side {
        active,
        scene,
        composite,
        terminal,
        smoother,
        layer_smoother,
        latches,
    } = side;
    let vars = latches.advance(
        &active.preset.latches,
        shared.vars.with_salt(shared.salt.of(active.preset)),
        shared.dt,
    );
    let inputs = FrameInputs {
        vars: &vars,
        frame: shared.frame,
        time: shared.time,
        dt: shared.dt,
    };
    let elements = element_prefix(active.preset, shared.series.len());
    let vertex = vertex_surface(
        active.preset.config.as_ref(),
        shared.tier,
        &mut *shared.vertex,
        shared.aspect,
    );
    evaluate_preset(
        active,
        scene,
        &mut *composite,
        terminal,
        smoother,
        &inputs,
        Scratch {
            series: shared.series.get_mut(..elements).unwrap_or(&mut []),
            vertex,
        },
    );
    // The `[layer]` bindings, into the layer's own scene and nowhere else
    // (ADR-0090 / Plan 0076): evaluated under the same salt, clock and analysis
    // frame as the top level, eased by their own smoother. The scene is this
    // side's own per-preset instance.
    let (Some(layer), Some(layer_scene)) = (active.preset.layer.as_ref(), composite.layer.as_mut())
    else {
        return;
    };
    let count = config_element_prefix(layer.config.as_ref(), shared.series.len());
    let layer_vertex = vertex_surface(
        layer.config.as_ref(),
        shared.tier,
        &mut *shared.vertex,
        shared.aspect,
    );
    evaluate_layer(
        layer,
        layer_scene,
        &mut composite.chain,
        layer_smoother,
        &inputs,
        Scratch {
            series: shared.series.get_mut(..count).unwrap_or(&mut []),
            vertex: layer_vertex,
        },
    );
}
