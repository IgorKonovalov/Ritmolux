//! The render seam: take an [`AnalysisFrame`], drive the active preset's system,
//! draw one frame.
//!
//! The render loop is driven by the frontend at display cadence and is fully
//! decoupled from audio delivery — the ring buffer is the seam (CLAUDE.md).
//! Cycling moves between loaded presets (ADR-0002); each preset names a built-in
//! system and binds its parameters to expressions the renderer evaluates from
//! the analysis frame plus the shared scene clock.

// Hot-path panic-denial pragma (Plan 0002 Phase 2). Runs every displayed
// frame; a panic here is a visible crash mid-show.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

// The four compositing stages stay crate-private to the outside world, but are
// `pub(crate)` so `preset::schema` can read their global `PARAMS` vocabularies
// for the load-time typo check (ADR-0020). `post` holds the two **per-preset**
// stages that run after the scene, behind one trait in one fixed-order chain
// (ADR-0031); the engine-wide passes stay outside it (ADR-0032) — `background`
// is the pre-pass that owns the clear, `ink` the terminal tone remap.
pub(crate) mod background;
pub(crate) mod bloom;
pub mod capture;
// The `capture_*` entry points themselves — a continuation of `impl Renderer`
// (Plan 0061 Phase 3). Private, because it adds no path of its own: every method
// in it is reached as `Renderer::capture_*`, exactly as before the split.
mod capture_api;
pub mod context;
pub mod feedback;
pub(crate) mod gpu;
pub(crate) mod grid;
pub(crate) mod ink;
pub(crate) mod kaleidoscope;
// The `over`-join blend pass (ADR-0090 / Plan 0076 Phase 3) — driven by the
// `PostChain`, whose walk knows the junction; nothing else reaches it.
pub(crate) mod layer_blend;
pub mod metrics;
// The now-playing banner (ADR-0110). Deliberately **not** behind the `text`
// feature: a build without it keeps the state and never asks for a layout, so
// the plugin build turns the feature on without touching this module.
pub mod now_playing;
pub mod overlay;
mod overlay_font;
pub mod palette;
// `pub(crate)` for the same reason the stage modules are: the preset loader's
// typo check unions every global vocabulary, and since ADR-0085 one of them —
// `occlude` — belongs to the chain rather than to a stage inside it.
pub(crate) mod post;
pub mod scenes;
#[cfg(feature = "text")]
pub mod text;
pub mod tier;
pub(crate) mod tonemap;
pub(crate) mod trails;
mod transition;

use crate::audio::AudioFormat;
use crate::diag::{AnalysisMetrics, Diag, Metrics};
use crate::dsp::AnalysisFrame;
use crate::preset::{Easing, Expr, Layer, LayerJoin, Preset, SystemKind, Variables};
use background::Background;
pub use capture::CaptureImage;
pub use capture_api::AudioCapture;
pub use context::{RenderContext, RenderError};
use ink::Ink;
use now_playing::NowPlaying;
use overlay::Overlay;
use palette::Palette;
use post::PostChain;
use scenes::Scene;
pub use scenes::lines::CapOverflow;
#[cfg(feature = "text")]
use text::TextLayer;
#[cfg(feature = "text")]
pub use text::TextRun;
pub use tier::{Tier, TierConfig};
use tonemap::Tonemap;
use transition::{Blend, DEFAULT_DURATION_SECS, Transition, TransitionKind};

/// The format **every intermediate upstream of the tonemap** carries: linear
/// light, unbounded above 1.0 (ADR-0046, Plan 0045 Phase 3).
///
/// The scene targets, both post stages, the transition blend's two sides and the
/// tonemap's own input are all this — the surface format stops at the tonemap,
/// which is where the frame becomes display-referred. Before this plan every one
/// of those intermediates ran at the surface's 8 bits, so an additive
/// accumulation clipped per channel at each hand-off: the "additive ceiling"
/// ADR-0046's Context catalogues, and the reason a bright-pass had nothing
/// correct to bloom from.
///
/// `Rgba16Float` rather than 32-bit because it is the format
/// [`PingPongField`](feedback::PingPongField) has shipped on since Plan 0014 —
/// already proven blendable and filterable on both backends — and because half
/// the bandwidth matters on the floor tier (`tier::TierConfig::post_cap`).
///
/// Note the arithmetic did **not** change: an 8-bit *sRGB* target already blends
/// in linear space, so what this buys is headroom above 1.0 and precision, not a
/// different colour model.
pub(crate) const COMPOSITE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// Assumed bytes-per-pixel for the swapchain GPU-byte estimate (the common
/// 8-bit RGBA/BGRA surface formats). An approximation, per ADR-0008.
const SWAPCHAIN_BYTES_PER_PIXEL: u64 = 4;
/// Fixed 2-image approximation for the swapchain GPU-byte estimate. wgpu exposes
/// no real image count, so this stays a constant decoupled from the context's
/// `desired_maximum_frame_latency` (also 2); the figure is a trend indicator,
/// not an exact footprint (ADR-0008).
const SWAPCHAIN_IMAGE_COUNT: u64 = 2;

/// **The engine's whole transition policy** (ADR-0024: policy lives in code, not
/// in a preset's `[transition]` table and not in an operator UI — both are
/// deliberate follow-ups). Two constants, in one place, so tuning the show is a
/// one-line edit that changes *every* switch path at once.
///
/// `None` rotates deterministically over [`TransitionKind::LIBRARY`], so a live
/// show sees the whole library; `Some(kind)` pins every dissolve to one. The
/// rotation counter is engine state, never a clock or an RNG, so a captured
/// sequence of switches reproduces exactly (NFR §6).
const TRANSITION_KIND: Option<TransitionKind> = None;
/// How long every dissolve runs, in seconds. See [`TRANSITION_KIND`].
const TRANSITION_DURATION_SECS: f32 = DEFAULT_DURATION_SECS;
/// Smoothed frame-time ceiling, in milliseconds, under which a dissolve may run
/// its outgoing side **live** as well (ADR-0024's adaptive governor). A named
/// constant on purpose — this is the number to calibrate on a low-end rig.
///
/// Sized against 60 fps @ 1080p (NFR §1 = 16.7 ms) with slack, not under it: with
/// vsync on, a machine keeping up reports the refresh interval no matter how much
/// GPU headroom it has, so a stricter threshold would simply never upgrade on a
/// 60 Hz display. ~55 fps is the "we are already struggling, do not double the
/// composite" line. The real protection against *starting* an unaffordable
/// dissolve is the latch: dual-live begins, the frame time rises past this within
/// a few frames, and the rest of the dissolve falls back to the frozen side.
const DUAL_LIVE_BUDGET_MS: f32 = 18.0;

/// The built scenes, each paired with the [`SystemKind`] it drives — the roster
/// [`scenes::create_all`] returns. Addressed by kind, never by position, so a
/// scene cannot silently render in another system's slot.
type SceneRoster = Vec<(SystemKind, Box<dyn Scene>)>;

/// The scene a preset's `system` drives, or `None` if the roster somehow lacks it
/// (impossible: the roster is built from [`SystemKind::ALL`] by an exhaustive
/// factory). A linear scan over seven `Copy`-enum keys, once per frame — the same
/// cost as the `match` it replaces, so no map.
fn scene_for(scenes: &SceneRoster, system: SystemKind) -> Option<&dyn Scene> {
    scenes
        .iter()
        .find(|(kind, _)| *kind == system)
        .map(|(_, scene)| scene.as_ref())
}

/// [`scene_for`], mutably.
fn scene_for_mut(scenes: &mut SceneRoster, system: SystemKind) -> Option<&mut Box<dyn Scene>> {
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
enum ParamRoute {
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
fn resolve_route(name: &str, system: SystemKind) -> ParamRoute {
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

/// The loaded presets plus the active index — the pure, GPU-free part of
/// selection. Split out of [`Renderer`] so the addressing contract (names in
/// roster order, in-range select, out-of-range no-op) is unit-testable without a
/// surface, mirroring how the diagnostics stats are a pure type behind the GPU
/// [`Renderer`]. [`Renderer`]'s preset methods delegate here 1:1.
struct Roster {
    presets: Vec<Preset>,
    /// Resolved [`ParamRoute`]s, one inner `Vec` per preset and one entry per that
    /// preset's bindings, in `Preset::params` order.
    ///
    /// Kept here rather than on the active preset alone because a dissolve
    /// composites **two** presets in one frame (Plan 0023) and both sides want
    /// their routes; indexing by preset means a side's routes cannot drift out of
    /// step with the preset it is showing. Resolution is a render-layer concern
    /// (it names chain positions), which is why it lives on this render-layer type
    /// and not in `preset/`.
    routes: Vec<Vec<ParamRoute>>,
    active: usize,
}

impl Roster {
    fn new(presets: Vec<Preset>) -> Self {
        Self {
            routes: resolve_routes(&presets),
            presets,
            active: 0,
        }
    }

    /// Replace the roster; reset `active` to the start if it now points past the
    /// end. An empty set is ignored — a directory that briefly reads empty or
    /// all-malformed leaves the last good roster rendering (NFR 10).
    fn set_presets(&mut self, presets: Vec<Preset>) {
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
    fn routes_for(&self, index: usize) -> &[ParamRoute] {
        self.routes.get(index).map_or(&[], Vec::as_slice)
    }

    /// The active preset's resolved routes.
    fn active_routes(&self) -> &[ParamRoute] {
        self.routes_for(self.active)
    }

    /// The index cycling would land on (wrapping), **without** moving there — the
    /// dissolve controller needs the target before the roster flips, because the
    /// dissolve's opening frame still composites the outgoing preset. Returns the
    /// current index on an empty or single-preset roster, which the caller reads as
    /// "nothing to dissolve to".
    fn next_index(&self) -> usize {
        if self.presets.is_empty() {
            return self.active;
        }
        (self.active + 1) % self.presets.len()
    }

    /// Set the active preset **iff** `index` is in range; an out-of-range index
    /// is a no-op — never a panic, never a wrap.
    fn select(&mut self, index: usize) {
        if index < self.presets.len() {
            self.active = index;
        }
    }

    /// The active preset, or `None` on an empty roster.
    fn active_preset(&self) -> Option<&Preset> {
        self.presets.get(self.active)
    }

    /// The active preset's name, or a placeholder on an empty roster.
    fn name(&self) -> &str {
        self.active_preset()
            .map(|p| p.name.as_str())
            .unwrap_or("no presets")
    }

    /// The loaded preset names in roster order.
    fn names(&self) -> impl Iterator<Item = &str> {
        self.presets.iter().map(|p| p.name.as_str())
    }
}

/// Resolve every preset's bindings to their destinations, off the hot path — once
/// per roster load, not once per binding per frame.
fn resolve_routes(presets: &[Preset]) -> Vec<Vec<ParamRoute>> {
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
/// and beat-driven motion eases instead of snapping. The expression layer stays
/// pure and allocation-free — the smoothing state lives only here.
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
struct ParamSmoother {
    /// Last smoothed value per binding index; grown lazily and seeded with the
    /// first frame's raw value, so the first frame after a reset snaps rather than
    /// drifting up from a stale zero. Cleared on reset.
    last: Vec<f32>,
}

impl ParamSmoother {
    /// Forget all state so the next frame snaps to the incoming values.
    fn reset(&mut self) {
        self.last.clear();
    }

    /// Smooth `raw` for binding `index` toward its previous value over `dt`
    /// seconds, using whichever of `tau`'s two constants the direction of travel
    /// selects (ADR-0035). A selected constant of `<= 0` (the default) or
    /// non-finite, or a non-positive `dt`, passes `raw` through unchanged. The
    /// first frame after a reset seeds the state with `raw` (a snap).
    fn smooth(&mut self, index: usize, raw: f32, tau: Easing, dt: f32) -> f32 {
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

/// How much of the per-element scratch `preset` uses this frame: its
/// `[spectrum] elements`, or `0` for a system with no per-element surface
/// (Plan 0034 Phase 4). Bounded by `capacity` so a config can never index past
/// the scratch, whatever the loader admitted.
fn element_prefix(preset: &Preset, capacity: usize) -> usize {
    config_element_prefix(preset.config.as_ref(), capacity)
}

/// [`element_prefix`] over a bare config — shared with the layer's own
/// structural config (Plan 0076 Phase 1), which is preset data of exactly the
/// same shape but not a whole [`Preset`].
fn config_element_prefix(config: Option<&scenes::GeneratorConfig>, capacity: usize) -> usize {
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
fn vertex_grid(
    config: Option<&scenes::GeneratorConfig>,
    tier: &TierConfig,
    capacity: usize,
) -> Option<((u32, u32), usize)> {
    let scenes::GeneratorConfig::WarpMesh { mesh } = config? else {
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
fn evaluate_series(expr: &Expr, vars: &Variables<'_>, out: &mut [f32]) {
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
struct VertexSurface<'a> {
    /// The **clamped** grid, in cells. Both this and the scene's own grid come
    /// out of [`warp_mesh::clamp_grid`](scenes::warp_mesh::clamp_grid) on the same
    /// request and the same tier, so the series is exactly as long as the scene
    /// expects.
    mesh: (u32, u32),
    /// The **render target's** aspect (ADR-0037), never the mesh grid's. The
    /// target's aspect is the surface's whatever internal grid the chain routes
    /// through (`PostChain::begin`), so the renderer can compute it here from the
    /// frame's own size.
    aspect: f32,
    /// The renderer's scratch, sliced to `vertex_count(mesh)`. Empty for every
    /// preset with no per-vertex surface, which is what makes their path
    /// unchanged.
    buf: &'a mut [f32],
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
fn evaluate_vertex_series(expr: &Expr, vars: &Variables<'_>, surface: &mut VertexSurface<'_>) {
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
enum SaltMode {
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
    fn of(self, preset: &Preset) -> u32 {
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
#[allow(clippy::too_many_arguments)]
fn evaluate_preset(
    preset: &Preset,
    routes: &[ParamRoute],
    scene: &mut Box<dyn Scene>,
    side: &mut CompositeSide,
    terminal: Option<Terminal<'_>>,
    smoother: &mut ParamSmoother,
    vars: &Variables<'_>,
    frame: &AnalysisFrame,
    time: f32,
    dt: f32,
    series: &mut [f32],
    vertex: Option<VertexSurface<'_>>,
) {
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
        // expression layer above stays pure and allocation-free.
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
#[allow(clippy::too_many_arguments)]
fn evaluate_layer(
    layer: &Layer,
    scene: &mut Box<dyn Scene>,
    chain: &mut PostChain,
    smoother: &mut ParamSmoother,
    vars: &Variables<'_>,
    frame: &AnalysisFrame,
    time: f32,
    dt: f32,
    series: &mut [f32],
    vertex: Option<VertexSurface<'_>>,
) {
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
/// always encoded. The scene lives on `side` (per-preset since Phase 2), so
/// whichever side is drawn brings the right layer with it.
fn composite_into(
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
struct Terminal<'a> {
    tonemap: &'a mut Tonemap,
    ink: &'a mut Ink,
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
struct CompositeSide {
    background: Background,
    chain: PostChain,
    /// This side's `[layer]` scene, **constructed for the preset it draws**
    /// (ADR-0090 point 4, Plan 0076 Phase 2) — never a roster instance, so
    /// same-system pairs are legal and two dissolving sides' layers share
    /// nothing. `Some` exactly when that preset declares a `[layer]`;
    /// [`Renderer::configure_active_scene`] maintains the invariant on every
    /// preset change. Living here rather than on the renderer gives it the
    /// side's own lifetime for free: the outgoing side keeps its layer alive
    /// through a dissolve, and promotion at finalize carries it over.
    layer: Option<Box<dyn Scene>>,
}

impl CompositeSide {
    /// `format` is [`COMPOSITE_FORMAT`], never the surface's: everything a side
    /// paints lands upstream of the tonemap, in linear light (ADR-0046).
    fn new(device: &wgpu::Device, format: wgpu::TextureFormat, tier: &TierConfig) -> Self {
        Self {
            background: Background::new(device, format),
            chain: PostChain::new(device, format, tier),
            layer: None,
        }
    }

    /// Reset every stage's params to their defaults (once per frame, before this
    /// side's preset bindings are routed).
    fn reset_params(&mut self) {
        self.background.reset_params();
        self.chain.reset_params();
    }

    /// Drop the lazily-built GPU resources (capture rebuild — keeps a headless
    /// capture a pure function of its inputs, NFR §6). The layer scene goes
    /// with them: `configure_active_scene` reconstructs it from its
    /// deterministic seed, which is exactly the rebuild the roster scenes get
    /// from `scenes::create_all` on the same path.
    fn reset_resources(&mut self) {
        self.background.reset_resources();
        self.chain.reset_resources();
        self.layer = None;
    }
}

/// How to build a headless [`Renderer`] for capture (Plan 0013).
///
/// Deliberately carries **no tier**: [`Renderer::new_headless`] is
/// [`Tier::Floor`] by construction, which is what keeps every golden baseline
/// byte-reproducible (ADR-0045). A capture at another tier goes through
/// [`Renderer::new_headless_tiered`], where the choice is written at the call
/// site and cannot be reached by forgetting a field.
#[derive(Debug, Clone, Copy)]
pub struct HeadlessOptions {
    /// Offscreen render width in pixels.
    pub width: u32,
    /// Offscreen render height in pixels.
    pub height: u32,
    /// Force a fallback (software) adapter — WARP on DX12 — so captures
    /// rasterize identically across machines. Tests want this on.
    pub prefer_software: bool,
}

/// Construction-time options for an on-surface [`Renderer`] (Plan 0044).
///
/// One field today and a struct anyway, because the tier pin is the first of a
/// family: a renderer's *construction* choices (quality, later backend or format
/// preferences) are decided once and never per frame, and threading them as
/// positional arguments is how the three constructors drift apart.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RendererOptions {
    /// An explicit tier pin, or `None` for auto — which resolves [`Tier::Rich`]
    /// and leaves the frame-time governor free to demote it once (ADR-0045). A
    /// pin is honoured in both directions and never demotes.
    pub tier: Option<Tier>,
}

impl RendererOptions {
    /// Options pinning `tier` explicitly.
    pub fn pinned(tier: Tier) -> Self {
        Self { tier: Some(tier) }
    }
}

/// Owns the GPU context, the built-in systems, and the loaded presets; renders
/// one frame per call by evaluating the active preset into the active system.
pub struct Renderer {
    ctx: RenderContext,
    /// Every built-in scene, keyed by the system it drives (see [`SceneRoster`]).
    scenes: SceneRoster,
    /// The active preset's composite — its `bg_*` backdrop pre-pass (ADR-0018,
    /// which owns the frame clear now that scenes `Load` instead of `Clear`) and
    /// the per-preset [`PostChain`] its trails and kaleidoscope fold through
    /// (ADR-0018 order, ADR-0031 seam). Each stage is individually skippable, so an
    /// unbound preset renders straight to the chain's destination.
    ///
    /// While a dissolve runs this side stays on the **outgoing** preset — which is
    /// what keeps its trail accumulating across the dissolve instead of restarting
    /// — and [`incoming_side`](Self::incoming_side) carries the new one.
    side: CompositeSide,
    /// The incoming preset's composite while a dissolve runs, `None` otherwise.
    /// Created at the switch site and promoted to [`side`](Self::side) at finalize,
    /// so there is no frame where both or neither is the live one, and no GPU state
    /// is ever shared between the two presets.
    incoming_side: Option<CompositeSide>,
    /// The terminal engine tone-remap (ADR-0028), outside the chain per ADR-0032:
    /// it remaps the **one** finished frame, so it must run after the transition
    /// blend of two per-preset composites. Skipped entirely at `ink_amount <= 0`,
    /// which is every preset that does not opt in.
    ink: Ink,
    /// The exposure + tonemap pass (ADR-0046) — the frame's **linear/display
    /// boundary**, between the transition blend and ink. Unlike every other pass
    /// it never skips: it is the format seam, not a look, so an unbound preset
    /// still runs it at `exposure = 1.0`.
    tonemap: Tonemap,
    /// The two-input cross-preset blend pass (Plan 0023 / ADR-0032), between the
    /// chain and the tonemap. Holds no GPU resources between dissolves.
    blend: Blend,
    /// The in-flight dissolve, if any. `None` is the ordinary frame path — chain
    /// straight into ink's input (or the surface), no blend encoded at all.
    transition: Option<Transition>,
    /// Dissolves started since the last explicit jump — the rotation position for
    /// [`TRANSITION_KIND`]. A counter, not a clock or an RNG, so the sequence of
    /// kinds a run produces is reproducible.
    transitions_started: u32,
    /// Loaded presets + the active index (pure selection state — see [`Roster`]).
    roster: Roster,
    /// Shared scene clock (seconds), advanced one fixed step per rendered frame.
    /// The single source for both an expression's `time` and system animation.
    time: f32,
    /// Runtime diagnostics: rolling frame-time stats + overlay flags (Plan 0011).
    diag: Diag,
    /// The debug overlay pass, painted only while `diag.overlay_enabled()`.
    overlay: Overlay,
    /// On-canvas text seam (browse overlay / HUD), standalone-only via the
    /// `text` feature (ADR-0009); absent from the plugin/default build.
    #[cfg(feature = "text")]
    text_layer: TextLayer,
    /// The now-playing banner (ADR-0110): a string a shell pushes in, plus the
    /// `dt`-driven envelope that fades it. Present in every build — a plugin
    /// build without the `text` feature holds the state and draws nothing.
    now_playing: NowPlaying,
    /// Segment-cap truncation from the active preset's last `configure`, if any
    /// (ADR-0007: the cap is never a silent cut). Refreshed whenever the active
    /// preset changes; the frontend surfaces it. `None` when geometry fit.
    cap_overflow: Option<CapOverflow>,
    /// Per-parameter easing state (ADR-0019 / Phase 5), reset on every
    /// active-preset change and capture rebuild.
    /// Scratch for per-element binding evaluation (Plan 0034 Phase 4). Sized
    /// **once, here at construction**, to the largest element count the loader
    /// admits — so the per-frame path slices it and never allocates. A frame uses
    /// the prefix its preset's `[spectrum] elements` asks for; every other system
    /// uses an empty prefix, which is what makes their path unchanged.
    series_scratch: Vec<f32>,
    /// Scratch for per-vertex binding evaluation (Plan 0100 Phase 1). Sized
    /// **once, here at construction**, to the largest mesh any tier may name
    /// ([`MAX_MESH`](scenes::warp_mesh::MAX_MESH)), so the per-frame path slices
    /// it and never allocates. Every system but the warp mesh uses an empty
    /// prefix.
    vertex_scratch: Vec<f32>,
    param_smoother: ParamSmoother,
    /// The active preset's **layer** easing state (Plan 0076 Phase 1) — its own
    /// smoother because layer bindings are indexed within the layer's `params`,
    /// which would collide with the main preset's indices in
    /// [`param_smoother`](Self::param_smoother). Reset wherever that one is.
    layer_smoother: ParamSmoother,
    /// The outgoing preset's layer easing state during a dual-live dissolve —
    /// the layer counterpart of [`outgoing_smoother`](Self::outgoing_smoother),
    /// handed over at the same roster flip.
    outgoing_layer_smoother: ParamSmoother,
    /// The active quality tier's capacity values, resolved **once** here at
    /// construction (ADR-0045). Read at construction and reconfigure time only —
    /// never branched on per frame.
    tier: TierConfig,
    /// True when the tier was pinned explicitly rather than auto-resolved. A pin
    /// is honoured in both directions (ADR-0045), so the governor never touches
    /// one — which is also the escape hatch for a machine whose transient stall
    /// cost it the rich tier.
    tier_pinned: bool,
    /// **The governor's one-way latch.** Set the single time the frame-time
    /// governor demotes `Rich -> Floor`, and never cleared: there is no
    /// auto-promotion by design, so a demoted session stays demoted and the
    /// decision cannot oscillate. The same shape as the dual-live freeze latch
    /// below, for the same reason.
    ///
    /// Also what the frontend reads to report the demotion, so a pinned floor and
    /// a demoted floor are distinguishable — otherwise the demotion would be
    /// silent, which ADR-0045 rules out.
    tier_demoted: bool,
    /// The display's frame budget in seconds, set by the frontend from its
    /// monitor's refresh rate ([`set_display_hz`](Self::set_display_hz)). Not read
    /// from the platform here — a refresh rate is a shell concern, and `core`
    /// stays source- and platform-agnostic.
    frame_budget_secs: f32,
    /// The **outgoing** preset's easing state during a dual-live dissolve. Moved
    /// out of [`param_smoother`](Self::param_smoother) when the roster flips, so a
    /// heavily-smoothed preset keeps easing through the dissolve instead of
    /// snapping to raw values the moment it stops being active.
    outgoing_smoother: ParamSmoother,
}

impl Renderer {
    /// Everything a renderer is beyond its [`RenderContext`]: the scene roster,
    /// the composite side, the engine-wide post passes, the overlay, and the
    /// embedded default presets. **The one construction path** — the three public
    /// constructors differ only in how they obtain the context, so a new field is
    /// a one-place edit here rather than three.
    fn from_context(ctx: RenderContext, opts: RendererOptions) -> Self {
        // The one tier resolution in the engine (ADR-0045): a pin wins, and
        // unpinned is `Rich` — the governor's job is to take that back, not to
        // hedge it here.
        let tier = TierConfig::for_tier(opts.tier.unwrap_or(Tier::Rich));
        // Everything upstream of the tonemap is built against COMPOSITE_FORMAT,
        // not the surface's (ADR-0046): the scenes, both composite sides and the
        // blend all paint in linear light. Only the tonemap, ink, the overlay and
        // the text layer write display-referred pixels.
        let scenes = crate::render::scenes::create_all(&ctx.device, COMPOSITE_FORMAT, &tier);
        let side = CompositeSide::new(&ctx.device, COMPOSITE_FORMAT, &tier);
        let blend = Blend::new(&ctx.device, COMPOSITE_FORMAT);
        let tonemap = Tonemap::new(&ctx.device, ctx.surface_format());
        let ink = Ink::new(&ctx.device, ctx.surface_format());
        let overlay = Overlay::new(&ctx.device, ctx.surface_format());
        #[cfg(feature = "text")]
        let text_layer = TextLayer::new(&ctx.device, &ctx.queue, ctx.surface_format());
        let mut renderer = Self {
            ctx,
            scenes,
            side,
            incoming_side: None,
            ink,
            tonemap,
            blend,
            transition: None,
            transitions_started: 0,
            roster: Roster::new(crate::preset::default_presets()),
            time: 0.0,
            diag: Diag::new(),
            overlay,
            #[cfg(feature = "text")]
            text_layer,
            now_playing: NowPlaying::default(),
            cap_overflow: None,
            series_scratch: vec![0.0; scenes::lines::spectrum::MAX_ELEMENTS],
            vertex_scratch: vec![0.0; scenes::warp_mesh::vertex_count(scenes::warp_mesh::MAX_MESH)],
            param_smoother: ParamSmoother::default(),
            layer_smoother: ParamSmoother::default(),
            outgoing_layer_smoother: ParamSmoother::default(),
            tier,
            tier_pinned: opts.tier.is_some(),
            tier_demoted: false,
            frame_budget_secs: tier::budget_secs(tier::DEFAULT_DISPLAY_HZ),
            outgoing_smoother: ParamSmoother::default(),
        };
        // Apply the initial preset's structural config (ADR-0007) so a line
        // scene at roster index 0 renders with its geometry built.
        renderer.configure_active_scene();
        renderer
    }

    /// Build a renderer drawing into `target` (a safe window handle — the
    /// standalone path). Starts with the embedded default presets.
    ///
    /// `opts` carries the quality-tier pin;
    /// [`RendererOptions::default()`](RendererOptions) is auto (rich, governed).
    pub fn new(
        target: impl Into<wgpu::SurfaceTarget<'static>>,
        width: u32,
        height: u32,
        opts: RendererOptions,
    ) -> Result<Self, RenderError> {
        Ok(Self::from_context(
            RenderContext::new(target, width, height)?,
            opts,
        ))
    }

    /// Build a **headless** renderer that draws into offscreen textures instead
    /// of a window (Plan 0013 capture tooling). Same scenes, presets, and
    /// per-frame evaluation as the on-surface path — only the target differs.
    /// Starts with the embedded default presets.
    ///
    /// **Pinned [`Tier::Floor`], and there is no argument to say otherwise**
    /// (ADR-0045). A capture is a pure function of its inputs (NFR §6), and a
    /// baseline that moved because the machine that blessed it was fast is not a
    /// baseline — so the floor is the default by construction here rather than by
    /// every call site remembering to ask for it. Use
    /// [`new_headless_tiered`](Self::new_headless_tiered) for a deliberate
    /// rich-tier capture.
    pub fn new_headless(opts: HeadlessOptions) -> Result<Self, RenderError> {
        Self::new_headless_tiered(opts, Tier::Floor)
    }

    /// A headless renderer pinned to `tier` — the opt-in behind the `shot` CLI's
    /// `--tier`, for spot-checking that the rich tier's raised budgets actually
    /// render (Plan 0044 Phase 3). Always **pinned**, so a capture never demotes
    /// mid-run and stays reproducible.
    pub fn new_headless_tiered(opts: HeadlessOptions, tier: Tier) -> Result<Self, RenderError> {
        Ok(Self::from_context(
            RenderContext::new_headless(opts.width, opts.height, opts.prefer_software)?,
            RendererOptions::pinned(tier),
        ))
    }

    /// Renderer targeting a native Win32 window the host owns — the C ABI
    /// path (foobar2000 shim). Starts with the embedded default presets (no
    /// ABI surface for preset selection yet).
    ///
    /// Auto tier: the plugin gets rich-with-governor, because the C ABI stays v4
    /// and a plugin-side tier picker is a future ABI question rather than part of
    /// ADR-0045.
    ///
    /// # Safety
    /// `hwnd` must be a valid window handle that outlives this renderer.
    #[cfg(windows)]
    pub unsafe fn new_from_win32_hwnd(
        hwnd: std::num::NonZeroIsize,
        width: u32,
        height: u32,
    ) -> Result<Self, RenderError> {
        let target = wgpu::SurfaceTargetUnsafe::RawHandle {
            raw_display_handle: Some(wgpu::rwh::RawDisplayHandle::Windows(
                wgpu::rwh::WindowsDisplayHandle::new(),
            )),
            raw_window_handle: wgpu::rwh::RawWindowHandle::Win32(
                wgpu::rwh::Win32WindowHandle::new(hwnd),
            ),
        };
        // The `unsafe` is exactly the surface-from-raw-handle call: the caller's
        // promise about `hwnd`'s validity and lifetime. Construction past that
        // point is the same safe code the other two paths run.
        let ctx = unsafe { RenderContext::new_unsafe(target, width, height) }?;
        Ok(Self::from_context(ctx, RendererOptions::default()))
    }

    /// The active quality tier (ADR-0045) — what the diagnostics overlay and the
    /// `shot` report header name.
    pub fn tier(&self) -> Tier {
        self.tier.tier
    }

    /// **Change the quality tier on the running renderer** (ADR-0054).
    ///
    /// Rebuilds the tier-dependent GPU resources — the scene roster and the
    /// composite side — against the new [`TierConfig`], on the existing
    /// [`RenderContext`]. The device, queue, surface, preset roster, active
    /// preset, engine clock, text layer and diagnostics all survive, so the
    /// operator stays on the preset they were watching. A dissolve in flight is
    /// dropped: its two sides are GPU state built at the outgoing tier.
    ///
    /// The visible cost is one re-accumulation of everything that accumulates —
    /// trails, reaction-diffusion state, the attractor's deposit. That is the
    /// correct affordance rather than a defect: the operator asked for this and
    /// can see that it happened.
    ///
    /// **A no-op on a surface-less (headless) context**, so ADR-0045's
    /// by-construction guarantee that a capture is `Tier::Floor` survives a
    /// public mutator existing on this type at all. The condition is
    /// [`tier::tier_change_permitted`], which is a value rather than a comment.
    ///
    /// An explicit call **pins** the tier and **clears the governor's demotion
    /// latch**. The latch means "the governor took a decision the operator did
    /// not ask for, and must be told about it"; once the operator has asked for
    /// something, that history is spent. ADR-0045 says the latch is never
    /// cleared — ADR-0054 narrows that to "never cleared *by the governor*", and
    /// is the correction of record.
    pub fn set_tier(&mut self, tier: Tier) {
        if !tier::tier_change_permitted(self.ctx.surface.is_some()) {
            return;
        }
        self.tier_pinned = true;
        self.tier_demoted = false;
        // The same rebuild the governor's demotion runs, reused rather than
        // open-coded: a tier-sized resource added to a new scene is then covered
        // by construction instead of by remembering two call sites.
        self.apply_tier(TierConfig::for_tier(tier));
    }

    /// The active preset's index in the roster — what the browse overlay opens
    /// on, and what a caller checks a tier rebuild against.
    pub fn active_index(&self) -> usize {
        self.roster.active
    }

    /// Whether the frame-time governor demoted this session's tier.
    ///
    /// The frontend reports the **transition**, so a demotion is announced once
    /// rather than shouted every frame — the same pattern
    /// [`cap_overflow`](Self::cap_overflow) is surfaced through. A pinned floor
    /// answers `false`; only a governed demotion sets this.
    pub fn tier_demoted(&self) -> bool {
        self.tier_demoted
    }

    /// Tell the renderer the display's refresh rate, which sets the frame budget
    /// the governor measures against (ADR-0045). Defaults to
    /// [`DEFAULT_DISPLAY_HZ`](tier::DEFAULT_DISPLAY_HZ); a rate that is not usable
    /// falls back to it rather than producing a degenerate budget.
    ///
    /// Off the hot path — call it at startup and on a monitor change. The core
    /// does not read this from the platform itself: a refresh rate is a shell
    /// concern, and the whole point of the split is that `core` knows nothing
    /// about windows.
    pub fn set_display_hz(&mut self, hz: f32) {
        self.frame_budget_secs = tier::budget_secs(hz);
    }

    /// Run the governor over the rolling frame-time history and demote if it says
    /// so, returning whether this call was the demotion.
    ///
    /// Nothing happens when the tier is pinned, when it is already the floor, or
    /// when the latch has already fired — so the decision is made **once per
    /// session** and the expensive part below cannot run twice.
    fn govern_tier(&mut self) -> bool {
        if !tier::should_demote(
            self.tier.tier,
            self.tier_pinned,
            self.tier_demoted,
            self.diag.stats().samples(),
            self.frame_budget_secs,
        ) {
            return false;
        }
        self.tier_demoted = true;
        self.apply_tier(TierConfig::FLOOR);
        true
    }

    /// Rebuild the tier-dependent GPU state for `tier`.
    ///
    /// Allocation at a **reconfigure**, not on the hot path: this runs at most
    /// once in a session (the governor's latch is what guarantees that). The
    /// visible cost is one blink of the trails accumulation as the field pair is
    /// rebuilt at the smaller grid — the same blink a window resize across a grid
    /// step produces, and ADR-0045 accepts it as the price of a rare, deliberately
    /// visible event.
    ///
    /// A dissolve in flight is cancelled rather than migrated: its two sides are
    /// GPU state built at the outgoing tier, and finishing a crossfade across a
    /// tier change is a worse artifact than landing on the incoming preset.
    fn apply_tier(&mut self, tier: TierConfig) {
        self.tier = tier;
        self.cancel_transition();
        self.incoming_side = None;
        self.side = CompositeSide::new(&self.ctx.device, COMPOSITE_FORMAT, &self.tier);
        // The scenes carry tier capacities too — particle counts, the segment
        // buffer, the trail-grid cap — and those are sized at construction, so a
        // tier change means rebuilding them. Rebuilding also resets their
        // simulation state to its seed, which is the visible half of a demotion:
        // the attractor's cloud and the swarm restart rather than losing two
        // thirds of their points mid-flight.
        self.scenes = scenes::create_all(&self.ctx.device, COMPOSITE_FORMAT, &self.tier);
        self.configure_active_scene();
    }

    /// Reconfigure the surface for a new window size.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.ctx.resize(width, height);
    }

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
    fn select_preset_by_name_now(&mut self, name: &str) -> bool {
        let Some(index) = self.preset_names().position(|n| n == name) else {
            return false;
        };
        self.select_preset_instantly(index);
        true
    }

    /// Queue text runs to composite over the next rendered frame; the queue is
    /// cleared after each `render`. The standalone fills it each frame with the
    /// active preset name and, while the browse overlay is open, its rows. A
    /// `text`-feature (standalone) path — the plugin/default build has no text.
    #[cfg(feature = "text")]
    pub fn queue_text(&mut self, runs: &[TextRun<'_>]) {
        self.text_layer.queue(runs);
    }

    /// Announce the currently playing track (ADR-0110). The banner fades in,
    /// holds, and fades out on its own; the caller only says *what*, never
    /// *when to stop*.
    ///
    /// The string is `artist - title`, split on the first ` - `. Setting the
    /// string that is already set does nothing, so a metadata source may push on
    /// every update it receives. An empty string clears the banner.
    ///
    /// **Source-agnostic by construction** (ADR-0001): the argument carries no
    /// evidence of whether it came from Windows SMTC or foobar's `titleformat`.
    /// Callers must not call this from an audio callback — the copy allocates.
    pub fn set_now_playing(&mut self, text: &str) {
        self.now_playing.set(text);
    }

    /// Append the banner's lines to this frame's text queue, after whatever the
    /// frontend queued — [`queue_text`](Self::queue_text) *replaces* the queue,
    /// so the core's own furniture has to go in afterwards or a shell that draws
    /// nothing would erase it.
    #[cfg(feature = "text")]
    fn queue_now_playing(&mut self) {
        // Split-borrowed: the layout reads `now_playing` and `ctx` while the
        // queue is mutated, which a `&mut self` method call would forbid.
        let Self {
            now_playing,
            text_layer,
            ctx,
            ..
        } = self;
        let (width, height) = (ctx.config.width as f32, ctx.config.height as f32);
        for line in now_playing.layout(width, height).into_iter().flatten() {
            text_layer.push(TextRun {
                text: &line.text,
                x: line.x,
                y: line.y,
                size: line.size,
                color: line.color,
            });
        }
    }

    /// Enable or disable rolling frame-time collection — the gated diagnostics
    /// clock read (Plan 0011). The standalone leaves this on so the title always
    /// shows live fps/p99; turning it off keeps the core fully clock-free.
    pub fn enable_diagnostics(&mut self, on: bool) {
        self.diag.set_collecting(on);
    }

    /// Turn the on-screen debug overlay on or off (off by default). Independent
    /// of collection, so the plugin can log metrics without painting the overlay.
    pub fn set_overlay(&mut self, on: bool) {
        self.diag.set_overlay(on);
    }

    /// Whether the debug overlay is currently painted.
    pub fn overlay_enabled(&self) -> bool {
        self.diag.overlay_enabled()
    }

    /// The current diagnostics snapshot (fps, p99, GPU bytes, …).
    pub fn metrics(&self) -> Metrics {
        self.diag.metrics()
    }

    /// The last drawn frame's analysis snapshot — the levels and the downbeat
    /// lock state. **Native-only**: deliberately absent from the C ABI, so the
    /// foobar plugin has no counterpart (ADR-0052).
    pub fn analysis_metrics(&self) -> AnalysisMetrics {
        self.diag.analysis()
    }

    /// Name of the currently active preset.
    pub fn preset_name(&self) -> &str {
        self.roster.name()
    }

    /// Whether the active GPU adapter is a CPU/software rasterizer (WARP on DX12).
    /// Visual-QA tests read this to skip differential checks the software
    /// rasterizer can't render faithfully — notably the fullscreen-scene +
    /// background-pipeline coexistence, which WARP mis-renders while real hardware
    /// renders it correctly (Plan 0025 / ADR-0026).
    pub fn adapter_is_software(&self) -> bool {
        self.ctx.is_software()
    }

    /// Name of the built-in system the active preset drives (e.g. the frontend
    /// shows it next to the preset name).
    pub fn active_system_name(&self) -> &'static str {
        self.roster
            .active_preset()
            .and_then(|p| scene_for(&self.scenes, p.system))
            .map(|scene| scene.name())
            .unwrap_or("")
    }

    /// Apply the active preset's declarative structural config to its scene, if
    /// it has one (ADR-0007). Called once whenever the active preset changes —
    /// on select/cycle/hot-reload and after a capture rebuilds the scenes — so a
    /// generator builds and caches its geometry exactly once, off the hot path.
    /// A `None` config (fragment/swarm, or a curve on the family default) is a
    /// no-op via the trait's default `configure`.
    fn configure_active_scene(&mut self) {
        // Snap the eased params to the incoming preset's first values — no
        // cross-preset bleed, and determinism across capture rebuilds (ADR-0019).
        self.param_smoother.reset();
        self.layer_smoother.reset();
        let Self {
            ctx,
            scenes,
            roster,
            cap_overflow,
            side,
            incoming_side,
            tier,
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
                scenes::create_layer_scene(layer.system, &ctx.device, COMPOSITE_FORMAT, tier);
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

    /// The segment-cap truncation from the active preset's last `configure`, if
    /// its geometry hit the fixed cap (ADR-0007: the cap is never a silent cut).
    /// Refreshed on every active-preset change (select / cycle / hot-reload); the
    /// standalone surfaces it at load. `None` in the normal case where geometry
    /// fit — which is every shipped preset.
    pub fn cap_overflow(&self) -> Option<&CapOverflow> {
        // The configure-time overflow (an oversized L-system depth) takes
        // precedence; otherwise the active scene's per-frame geometry-mirror
        // overflow (Plan 0018 Phase 4), set once a frame has replicated. Both
        // reuse the same `CapOverflow` type so the frontend surfaces either.
        if let Some(overflow) = self.cap_overflow.as_ref() {
            return Some(overflow);
        }
        self.roster
            .active_preset()
            .and_then(|preset| scene_for(&self.scenes, preset.system))
            .and_then(|scene| scene.mirror_overflow())
    }

    /// Draw the current preset for this analysis frame, advancing all animation
    /// by `dt` real seconds (Plan 0014 Phase 2). The frontend measures and
    /// injects elapsed wall-clock time so the visuals run at the same speed on
    /// any refresh rate; `core` never reads a clock. Lost/outdated surfaces
    /// self-heal by reconfiguring; timeouts/occlusion skip the frame; only a
    /// validation failure (a bug) bubbles up.
    pub fn render(&mut self, frame: &AnalysisFrame, dt: f32) -> Result<(), RenderError> {
        self.time += dt;
        // The banner rides the same injected `dt` the scene does, so it lasts the
        // same number of seconds on any refresh rate (ADR-0110 / Plan 0014).
        self.now_playing.advance(dt);

        // Core-tracked GPU footprint: the swapchain dominates what the core
        // allocates. An approximation (ADR-0008), refreshed each frame so it
        // tracks resizes and Phase 6's swapchain trim.
        self.diag.set_gpu_bytes(
            self.ctx.config.width as u64
                * self.ctx.config.height as u64
                * SWAPCHAIN_BYTES_PER_PIXEL
                * SWAPCHAIN_IMAGE_COUNT,
        );

        let Some(surface_tex) = Self::acquire(&self.ctx)? else {
            self.diag.record_dropped(); // transient (timeout/occluded) — skip
            return Ok(());
        };
        let view = surface_tex
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("lmv-frame"),
            });

        // After the acquire, so a dropped frame does not leave a second copy of
        // the banner queued behind the one the next frame pushes.
        #[cfg(feature = "text")]
        self.queue_now_playing();

        let (width, height) = (self.ctx.config.width, self.ctx.config.height);
        // The one live call site: a preset that asked for `seed = "random"` gets
        // the salt it drew at load (ADR-0051). Every other caller of `draw_frame`
        // is a capture and pins.
        let draw_calls = self.draw_frame(
            frame,
            &mut encoder,
            &view,
            width,
            height,
            dt,
            SaltMode::Live,
        );

        self.ctx.queue.submit(std::iter::once(encoder.finish()));
        self.ctx.queue.present(surface_tex);

        // Free atlas glyphs unused this frame and clear the queue for the next.
        #[cfg(feature = "text")]
        self.text_layer.end_frame();

        self.diag.set_draw_calls(draw_calls);
        self.diag.record_frame();
        // The quality governor, after this frame is recorded: on sustained
        // evidence that the rich tier does not fit the display budget it latches
        // to the floor for the remainder of the session (ADR-0045). Cheap in the
        // steady state — a pin, an already-floor tier, or a fired latch all return
        // before the series is read — and it can rebuild GPU state at most once.
        self.govern_tier();
        Ok(())
    }

    /// Record this frame's scene pass — plus the optional text and overlay
    /// passes — into `encoder`, drawing into `view` at `width`×`height`. Shared
    /// by the on-surface present path and headless capture; the caller owns
    /// acquire/submit/present (or the offscreen copy-back). Evaluates the active
    /// preset into the active system using the current scene clock
    /// (`self.time`, advanced by the caller — this does not touch it) and
    /// injects `dt` real seconds into the scene's [`advance`](scenes::Scene::advance)
    /// so its simulation steps at the same wall-clock rate on any refresh.
    /// `salt` says which of the active preset's two salts its `hash()`/`noise()`
    /// calls mix in (ADR-0051) — [`SaltMode::Live`] from the one on-surface
    /// caller, [`SaltMode::Pinned`] from every capture path.
    ///
    /// Returns the draw-call count.
    // Eight arguments, one past the lint: they are the frame's inputs and each is
    // read once. The same allowance `evaluate_preset` above carries, and for the
    // same reason — bundling them would name a struct after a call site rather
    // than after anything in the design.
    #[allow(clippy::too_many_arguments)]
    fn draw_frame(
        &mut self,
        frame: &AnalysisFrame,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        width: u32,
        height: u32,
        dt: f32,
        salt: SaltMode,
    ) -> u32 {
        let Self {
            ctx,
            scenes,
            side,
            incoming_side,
            ink,
            tonemap,
            blend,
            transition,
            roster,
            time,
            diag,
            overlay,
            #[cfg(feature = "text")]
            text_layer,
            // Advanced and queued in `render`, before this is called — the banner
            // is a live-surface concern, so a headless capture never draws one.
            now_playing: _,
            // Set at preset load, surfaced by the frontend — not a per-frame concern.
            cap_overflow: _,
            series_scratch,
            vertex_scratch,
            param_smoother,
            layer_smoother,
            outgoing_smoother,
            outgoing_layer_smoother,
            // Resolved once at construction; the overlay names it (ADR-0045). The
            // capacity values themselves were consumed at construction time — the
            // frame path reads the tier only to print it.
            tier,
            // Whether that tier was the governor's doing, so the overlay can tell
            // a demoted floor from a pinned one.
            tier_demoted,
            // Governor inputs, read after the frame is encoded (see `govern_tier`)
            // rather than while encoding it — not a per-frame drawing concern.
            tier_pinned: _,
            frame_budget_secs: _,
            // Switch-site policy state (the kind rotation) — not a per-frame concern.
            transitions_started: _,
        } = self;

        // The analysis snapshot for the overlay and the 1 Hz log (ADR-0052).
        // Taken here rather than in `render` so a capture records it too, and
        // before the early return below so it is the frame's own values even when
        // there is no preset to draw them under.
        diag.set_analysis(frame);

        let Some(preset) = roster.active_preset() else {
            return 0; // no presets loaded — nothing to draw
        };
        let routes = roster.active_routes();

        // Evaluate against the shared clock and this frame's analysis. Both sides
        // of a dissolve read the same variables — they differ in what they *bind*.
        //
        // The band array rides along **by borrow** (ADR-0036): this bundle is
        // built once per frame here but read once per binding below, so a
        // by-value spectrum would put a 256-byte copy on the per-binding path.
        //
        // Through `from_frame` rather than the nine positional arguments, so the
        // harness probe that reads the same frame cannot bind it differently.
        //
        // The one thing that is *not* shared is the salt (ADR-0051): it is a fact
        // about a preset, not about the audio, so each side re-salts this bundle
        // below with its own. Sharing it would put the incoming preset's seed on
        // the outgoing preset's `hash()` for the second a dissolve lasts.
        let vars = Variables::from_frame(frame, *time);

        // Fixed-order composite (ADR-0018/0028/0032/0046): background (owns the
        // clear) -> scene -> the per-preset post chain -> [blend] -> tonemap ->
        // ink -> present. Everything left of the tonemap is linear light at
        // `COMPOSITE_FORMAT`; everything right of it is display-referred at the
        // surface's. Where the scene draws and which chain stage folds into which
        // is the chain's business, not the renderer's — see `post.rs` for the
        // order and the skip rule. The blend, the tonemap and ink are engine-wide
        // passes the renderer drives.
        let surface = (width, height);
        // The **render target's** aspect, which `PostChain::begin` reports as the
        // surface's whatever internal grid the chain routes through (ADR-0037).
        // Computed once here because the per-vertex evaluation below happens
        // before the chain opens, and `rad`/`ang` must be aspect-corrected.
        let surface_aspect = width as f32 / height.max(1) as f32;
        let mut draw_calls = 0;

        // --- the outgoing side, while it is still animating (dual-live only) ---
        //
        // Encoded first because it feeds the blend, and driven through `side` —
        // which has followed the outgoing preset since before the switch, so its
        // trail keeps accumulating rather than restarting. Its target is the same
        // texture the snapshot lives in: dual-live simply overwrites it each frame,
        // so a latch to freeze holds the last live picture instead of jumping back
        // to the one the dissolve opened on.
        let dual_live = transition.as_ref().is_some_and(Transition::is_dual_live);
        if dual_live {
            let outgoing_index = transition.as_ref().map(Transition::outgoing_index);
            // The outgoing preset and the routes resolved for it come from the same
            // index, so the two cannot drift apart.
            let outgoing = outgoing_index
                .and_then(|index| Some((roster.presets.get(index)?, roster.routes_for(index))));
            if let (Some((outgoing, out_routes)), Some(out_view)) =
                (outgoing, blend.snapshot_view(surface))
                && let Some(out_scene) = scene_for_mut(scenes, outgoing.system)
            {
                // No terminal: the outgoing side's `exposure`/`ink_*` were held at
                // the capture frame and are crossfaded by the single engine-wide
                // pass of each below.
                let out_elements = element_prefix(outgoing, series_scratch.len());
                let out_vertex = vertex_grid(outgoing.config.as_ref(), tier, vertex_scratch.len())
                    .and_then(|(mesh, n)| {
                        Some(VertexSurface {
                            mesh,
                            aspect: surface_aspect,
                            buf: vertex_scratch.get_mut(..n)?,
                        })
                    });
                evaluate_preset(
                    outgoing,
                    out_routes,
                    out_scene,
                    side,
                    None,
                    outgoing_smoother,
                    &vars.with_salt(salt.of(outgoing)),
                    frame,
                    *time,
                    dt,
                    series_scratch.get_mut(..out_elements).unwrap_or(&mut []),
                    out_vertex,
                );
                // The outgoing preset's own layer keeps animating through a
                // dual-live dissolve exactly as its main scene does — its scene
                // is the outgoing side's own instance (Plan 0076 Phase 2).
                if let (Some(layer), Some(layer_scene)) =
                    (outgoing.layer.as_ref(), side.layer.as_mut())
                {
                    let n = config_element_prefix(layer.config.as_ref(), series_scratch.len());
                    let lv = vertex_grid(layer.config.as_ref(), tier, vertex_scratch.len())
                        .and_then(|(mesh, count)| {
                            Some(VertexSurface {
                                mesh,
                                aspect: surface_aspect,
                                buf: vertex_scratch.get_mut(..count)?,
                            })
                        });
                    evaluate_layer(
                        layer,
                        layer_scene,
                        &mut side.chain,
                        outgoing_layer_smoother,
                        &vars.with_salt(salt.of(outgoing)),
                        frame,
                        *time,
                        dt,
                        series_scratch.get_mut(..n).unwrap_or(&mut []),
                        lv,
                    );
                }
                // **The outgoing preset's OWN held stop, not the crossfaded one**
                // (ADR-0080). Two reasons, and the first is structural: the
                // crossfade below cannot have run yet, because it interpolates
                // towards the incoming preset's `exposure` and that is not routed
                // until further down. The second is that this is the right answer
                // anyway — the outgoing side's bright-pass should keep selecting
                // what its author aimed it at for as long as the side is drawn;
                // fading it out is the blend's job, not the threshold's.
                let out_exposure = transition
                    .as_ref()
                    .and_then(Transition::outgoing_exposure)
                    .unwrap_or(tonemap::DEFAULT_EXPOSURE);
                draw_calls += composite_into(
                    ctx,
                    out_scene,
                    side,
                    encoder,
                    &out_view,
                    surface,
                    out_exposure,
                );
            }
        }

        // --- the active preset: the incoming side during a dissolve, the only
        // side otherwise ---
        //
        // The opening frame is the exception that makes the whole scheme cheap: the
        // roster still points at the *outgoing* preset there, so this one ordinary
        // composite is the snapshot, and `side` is the right chain for it.
        let live_side = match incoming_side.as_mut() {
            Some(incoming) if !transition.as_ref().is_some_and(Transition::needs_snapshot) => {
                incoming
            }
            _ => side,
        };
        let Some(scene) = scene_for_mut(scenes, preset.system) else {
            return draw_calls;
        };
        tonemap.reset_params();
        ink.reset_params();
        let elements = element_prefix(preset, series_scratch.len());
        let vertex = vertex_grid(preset.config.as_ref(), tier, vertex_scratch.len()).and_then(
            |(mesh, n)| {
                Some(VertexSurface {
                    mesh,
                    aspect: surface_aspect,
                    buf: vertex_scratch.get_mut(..n)?,
                })
            },
        );
        evaluate_preset(
            preset,
            routes,
            scene,
            live_side,
            Some(Terminal { tonemap, ink }),
            param_smoother,
            &vars.with_salt(salt.of(preset)),
            frame,
            *time,
            dt,
            series_scratch.get_mut(..elements).unwrap_or(&mut []),
            vertex,
        );
        // The `[layer]` bindings, into the layer's own scene and nowhere else
        // (ADR-0090 / Plan 0076): evaluated under the same salt, clock and
        // analysis frame as the top level, eased by their own smoother. The
        // scene is this side's own per-preset instance (Phase 2).
        if let (Some(layer), Some(layer_scene)) = (preset.layer.as_ref(), live_side.layer.as_mut())
        {
            let n = config_element_prefix(layer.config.as_ref(), series_scratch.len());
            let lv = vertex_grid(layer.config.as_ref(), tier, vertex_scratch.len()).and_then(
                |(mesh, count)| {
                    Some(VertexSurface {
                        mesh,
                        aspect: surface_aspect,
                        buf: vertex_scratch.get_mut(..count)?,
                    })
                },
            );
            evaluate_layer(
                layer,
                layer_scene,
                &mut live_side.chain,
                layer_smoother,
                &vars.with_salt(salt.of(preset)),
                frame,
                *time,
                dt,
                series_scratch.get_mut(..n).unwrap_or(&mut []),
                lv,
            );
        }

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

        // Hold the outgoing preset's evaluated terminal params off the capture
        // frame, where the roster still points at it — the one frame they exist.
        let captured_ink = ink.params();
        let captured_exposure = tonemap.exposure();

        // On-canvas text (browse overlay / HUD): a second pass that loads the
        // scene and composites the queued runs on top, in the same frame
        // (ADR-0009). Standalone-only via the `text` feature; when both this and
        // the diagnostics overlay are on, the overlay draws last so it sits on
        // top of the text.
        #[cfg(feature = "text")]
        {
            if text_layer.prepare(&ctx.device, &ctx.queue, width, height) {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("lmv-text-pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            // Load: composite over the scene already in the view.
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
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
                tier.tier,
                *tier_demoted,
                diag.stats().samples().map(|s| s * 1000.0),
            );
            draw_calls += 1;
        }

        // The budget governor, re-checked every frame a dual-live dissolve runs: on
        // evidence of overload it latches to the frozen side for the remainder, so
        // the mode can never flicker frame to frame (ADR-0024).
        if let Some(tr) = transition.as_mut()
            && dual_live
            && transition::budget_blown(diag.stats().frame_ms_avg(), DUAL_LIVE_BUDGET_MS)
        {
            tr.latch_freeze();
        }

        // Advance the dissolve now that the frame at the current `t` is encoded.
        // The capture frame hands back the index to flip the roster to, so the next
        // frame composites the incoming preset through its own side; a dissolve that
        // has reached `t = 1` promotes that side and releases the blend's targets.
        // The borrows above all end here, so `self` is free again (NLL).
        let advanced = transition.as_mut().map(|tr| {
            (
                tr.advance(dt, captured_ink, captured_exposure),
                tr.finished(),
            )
        });
        if let Some((flip_to, finished)) = advanced {
            if let Some(index) = flip_to {
                // Hand the outgoing preset's easing state to the outgoing side
                // before `configure_active_scene` resets the active one, so a
                // heavily-smoothed preset keeps easing through a dual-live dissolve
                // instead of snapping to raw values the frame it stops being active.
                self.outgoing_smoother = std::mem::take(&mut self.param_smoother);
                self.outgoing_layer_smoother = std::mem::take(&mut self.layer_smoother);
                self.roster.select(index);
                self.configure_active_scene();
            }
            if finished {
                self.cancel_transition();
            }
        }

        draw_calls
    }

    fn acquire(ctx: &RenderContext) -> Result<Option<wgpu::SurfaceTexture>, RenderError> {
        use wgpu::CurrentSurfaceTexture as C;
        let Some(surface) = ctx.surface.as_ref() else {
            return Ok(None); // headless context — no swapchain to present into
        };
        match surface.get_current_texture() {
            C::Success(t) | C::Suboptimal(t) => Ok(Some(t)),
            C::Timeout | C::Occluded => Ok(None),
            C::Outdated | C::Lost => {
                ctx.reconfigure();
                match surface.get_current_texture() {
                    C::Success(t) | C::Suboptimal(t) => Ok(Some(t)),
                    C::Validation => Err(RenderError::SurfaceValidation),
                    _ => Ok(None),
                }
            }
            C::Validation => Err(RenderError::SurfaceValidation),
        }
    }
}

#[cfg(test)]
mod tests;
