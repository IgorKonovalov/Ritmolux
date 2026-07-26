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
pub mod capture;
pub mod context;
pub mod feedback;
pub(crate) mod gpu;
pub(crate) mod ink;
pub(crate) mod kaleidoscope;
pub mod metrics;
pub mod overlay;
mod overlay_font;
pub mod palette;
mod post;
pub mod scenes;
#[cfg(feature = "text")]
pub mod text;
pub(crate) mod trails;
mod transition;

use crate::audio::AudioFormat;
use crate::diag::{Diag, Metrics};
use crate::dsp::AnalysisFrame;
use crate::preset::{Easing, Preset, SystemKind, Variables};
use background::Background;
pub use capture::CaptureImage;
pub use context::{RenderContext, RenderError};
use ink::Ink;
use overlay::Overlay;
use palette::Palette;
use post::PostChain;
use scenes::Scene;
pub use scenes::lines::CapOverflow;
#[cfg(feature = "text")]
use text::TextLayer;
#[cfg(feature = "text")]
pub use text::TextRun;
use transition::{Blend, DEFAULT_DURATION_SECS, Mode, Transition, TransitionKind};

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
    /// The terminal engine-wide ink pass (`ink_*` / `paper_*`), outside the chain
    /// (ADR-0032).
    Ink,
    /// The active scene's named-parameter surface.
    Scene,
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
/// The order is the composite order (ADR-0032) and matches the first-owner-wins
/// fallthrough it replaces: the backdrop pre-pass, then the post chain, then the
/// terminal ink pass, then the scene. The four namespaces are disjoint, so the
/// order is a formality rather than a tie-break — but it is the documented one.
fn resolve_route(name: &str, system: SystemKind) -> ParamRoute {
    if background::PARAMS.contains(&name) {
        return ParamRoute::Background;
    }
    if let Some(stage) = post::stage_for(name) {
        return ParamRoute::Stage(stage);
    }
    if ink::PARAMS.contains(&name) {
        return ParamRoute::Ink;
    }
    if system.param_names().contains(&name) {
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
        // The direction test is against the **held** value, not the raw signal's
        // own derivative (ADR-0035): a parameter already above its new target
        // releases toward it even while the raw input is still rising. That is
        // the envelope-follower convention, and it is what keeps the behavior
        // stable under a noisy input.
        let tau = if raw > *slot { tau.attack } else { tau.release };
        if tau <= 0.0 || !tau.is_finite() || dt <= 0.0 {
            *slot = raw;
            return raw;
        }
        // alpha = 1 - exp(-dt/tau): the fraction of the gap closed this frame,
        // frame-rate-independent because `dt` is real elapsed time (ADR-0019).
        let alpha = 1.0 - (-dt / tau).exp();
        *slot += alpha * (raw - *slot);
        *slot
    }
}

/// Evaluate one preset's bindings into a composite side, an optional ink pass,
/// and its scene. **Routing only** — nothing is encoded here, because the frame's
/// destination is not known until ink's activity is (ADR-0032).
///
/// `ink` is `None` for a dual-live dissolve's outgoing side: there is one
/// engine-wide ink pass and it belongs to the active preset, whose crossfade the
/// caller applies afterwards. An `ink_*` binding on that side is therefore
/// dropped — the same no-op it was when it fell through to a scene that ignores
/// unknown params.
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
    ink: Option<&mut Ink>,
    smoother: &mut ParamSmoother,
    vars: &Variables,
    frame: &AnalysisFrame,
    time: f32,
    dt: f32,
) {
    scene.set_time(time);
    scene.advance(dt);
    side.reset_params();
    scene.reset_params();
    let mut ink = ink;
    for (index, (binding, route)) in preset.params.iter().zip(routes).enumerate() {
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
            ParamRoute::Ink => {
                if let Some(ink) = ink.as_mut() {
                    ink.set_param(&binding.name, value);
                }
            }
            ParamRoute::Scene => {
                scene.set_param(&binding.name, value);
            }
            // Nothing consumes it. Surfaced at load, silent here (ADR-0020).
            ParamRoute::Unclaimed => {}
        }
    }
    scene.update(frame);
}

/// Encode one preset's composite into `destination`: the backdrop pre-pass (which
/// owns the clear), then the scene, then the chain folded down. Returns the draw
/// calls. Call after [`evaluate_preset`] has routed this side's params.
fn composite_into(
    ctx: &RenderContext,
    scene: &mut Box<dyn Scene>,
    side: &mut CompositeSide,
    encoder: &mut wgpu::CommandEncoder,
    destination: &wgpu::TextureView,
    surface: (u32, u32),
) -> u32 {
    let target = side.chain.begin(encoder, destination, surface);
    side.background.render(&ctx.queue, encoder, &target.view);
    // Hand the scene its target size before it renders: a scene with an internal
    // accumulation field (the attractor's trails) sizes that field from here rather
    // than a fixed grid (Plan 0027 Phase 2). A no-op for every other scene, and a
    // cheap unchanged-compare for the attractor.
    scene.set_target_size(target.size.0, target.size.1);
    scene.render(&ctx.queue, encoder, &target.view, target.aspect);
    // The backdrop and the scene, plus whatever the active chain stages encode on
    // their way down.
    2 + side
        .chain
        .resolve(&ctx.queue, encoder, target.routing, destination, surface)
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
}

impl CompositeSide {
    fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        Self {
            background: Background::new(device, format),
            chain: PostChain::new(device, format),
        }
    }

    /// Reset every stage's params to their defaults (once per frame, before this
    /// side's preset bindings are routed).
    fn reset_params(&mut self) {
        self.background.reset_params();
        self.chain.reset_params();
    }

    /// Drop the lazily-built GPU resources (capture rebuild — keeps a headless
    /// capture a pure function of its inputs, NFR §6).
    fn reset_resources(&mut self) {
        self.background.reset_resources();
        self.chain.reset_resources();
    }
}

/// How to build a headless [`Renderer`] for capture (Plan 0013).
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
    /// The two-input cross-preset blend pass (Plan 0023 / ADR-0032), between the
    /// chain and ink. Holds no GPU resources between dissolves.
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
    /// Segment-cap truncation from the active preset's last `configure`, if any
    /// (ADR-0007: the cap is never a silent cut). Refreshed whenever the active
    /// preset changes; the frontend surfaces it. `None` when geometry fit.
    cap_overflow: Option<CapOverflow>,
    /// Per-parameter easing state (ADR-0019 / Phase 5), reset on every
    /// active-preset change and capture rebuild.
    param_smoother: ParamSmoother,
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
    fn from_context(ctx: RenderContext) -> Self {
        let scenes = crate::render::scenes::create_all(&ctx.device, ctx.surface_format());
        let side = CompositeSide::new(&ctx.device, ctx.surface_format());
        let ink = Ink::new(&ctx.device, ctx.surface_format());
        let blend = Blend::new(&ctx.device, ctx.surface_format());
        let overlay = Overlay::new(&ctx.device, ctx.surface_format());
        #[cfg(feature = "text")]
        let text_layer = TextLayer::new(&ctx.device, &ctx.queue, ctx.surface_format());
        let mut renderer = Self {
            ctx,
            scenes,
            side,
            incoming_side: None,
            ink,
            blend,
            transition: None,
            transitions_started: 0,
            roster: Roster::new(crate::preset::default_presets()),
            time: 0.0,
            diag: Diag::new(),
            overlay,
            #[cfg(feature = "text")]
            text_layer,
            cap_overflow: None,
            param_smoother: ParamSmoother::default(),
            outgoing_smoother: ParamSmoother::default(),
        };
        // Apply the initial preset's structural config (ADR-0007) so a line
        // scene at roster index 0 renders with its geometry built.
        renderer.configure_active_scene();
        renderer
    }

    /// Build a renderer drawing into `target` (a safe window handle — the
    /// standalone path). Starts with the embedded default presets.
    pub fn new(
        target: impl Into<wgpu::SurfaceTarget<'static>>,
        width: u32,
        height: u32,
    ) -> Result<Self, RenderError> {
        Ok(Self::from_context(RenderContext::new(
            target, width, height,
        )?))
    }

    /// Build a **headless** renderer that draws into offscreen textures instead
    /// of a window (Plan 0013 capture tooling). Same scenes, presets, and
    /// per-frame evaluation as the on-surface path — only the target differs.
    /// Starts with the embedded default presets.
    pub fn new_headless(opts: HeadlessOptions) -> Result<Self, RenderError> {
        Ok(Self::from_context(RenderContext::new_headless(
            opts.width,
            opts.height,
            opts.prefer_software,
        )?))
    }

    /// Renderer targeting a native Win32 window the host owns — the C ABI
    /// path (foobar2000 shim). Starts with the embedded default presets (no
    /// ABI surface for preset selection yet).
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
        Ok(Self::from_context(ctx))
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

    /// Start a dissolve to `to`, or cut instantly when a dissolve would be
    /// meaningless (a switch to the already-active preset). An out-of-range `to` is
    /// a no-op that does not disturb a dissolve already running.
    ///
    /// The outgoing index is read from the roster **after** settling any dissolve in
    /// flight, never passed in: a caller that resolved it earlier would name a
    /// preset the snapshot is no longer going to hold.
    ///
    /// The roster is deliberately **not** flipped here: the dissolve's opening
    /// frame composites the still-active outgoing preset into the snapshot, and
    /// [`Transition::advance`] hands back the index to flip to once that frame has
    /// been encoded.
    ///
    /// **This is where a dissolve's GPU cost lands.** Every scene is built at
    /// startup, but the blend's two surface-sized targets (~16 MB at 1080p) and the
    /// incoming side's chain stages build lazily on the frames that first need them
    /// — the dissolve's opening frames. A one-time hitch there is a known
    /// limitation, not a bug; pre-warming here was considered and declined, since
    /// the chain's stages cannot know which of them a preset will even activate
    /// until its params are evaluated.
    fn begin_transition(&mut self, to: usize) {
        if to >= self.roster.presets.len() {
            // Out of range (a stale index from a shrunk hot-reloaded roster): a
            // no-op. Deliberately checked before settling, so an invalid request
            // cannot cut short a dissolve that is running correctly.
            return;
        }
        // A switch arriving mid-dissolve snap-finishes the one in flight to its own
        // target before starting the new one, so the roster is never left on an
        // index nobody asked for (Plan 0023 Phase 5 re-entrancy rule).
        self.snap_finish_transition();
        let from = self.roster.active;
        if to == from {
            self.select_preset_instantly(to);
            return;
        }
        let kind =
            TRANSITION_KIND.unwrap_or_else(|| TransitionKind::rotating(self.transitions_started));
        self.transitions_started = self.transitions_started.wrapping_add(1);
        self.transition = Some(Transition::new(
            from,
            to,
            TRANSITION_DURATION_SECS,
            kind,
            self.dissolve_mode(from, to),
        ));
        // The incoming preset gets its own backdrop + chain for the dissolve, so
        // neither side can see the other's uniforms or feedback history. It is
        // promoted to *the* side at finalize; until then `side` stays on the
        // outgoing preset (see the field docs).
        self.incoming_side = Some(CompositeSide::new(
            &self.ctx.device,
            self.ctx.surface_format(),
        ));
    }

    /// The fidelity a dissolve from `from` to `to` may run at — ADR-0024's
    /// adaptive governor, resolved once at the switch site.
    ///
    /// Dual-live needs both halves: two presets whose scenes hold **independent**
    /// GPU state (not the same `SystemKind`, and not two of the three line scenes,
    /// which share one renderer), and measured frame-time headroom. The decision
    /// itself is the pure [`transition::dual_live_eligible`]; this only gathers its
    /// two inputs.
    fn dissolve_mode(&self, from: usize, to: usize) -> Mode {
        let systems = (
            self.roster.presets.get(from).map(|p| p.system),
            self.roster.presets.get(to).map(|p| p.system),
        );
        let shares = match systems {
            (Some(a), Some(b)) => scenes::shares_resources(a, b),
            // A preset we cannot even resolve is not a pair we will render twice.
            _ => true,
        };
        if transition::dual_live_eligible(
            shares,
            self.diag.stats().frame_ms_avg(),
            DUAL_LIVE_BUDGET_MS,
        ) {
            Mode::DualLive
        } else {
            Mode::Freeze
        }
    }

    /// Start a dissolve to `to` at a forced fidelity, bypassing the governor.
    /// **Test-only** — see [`Transition::set_mode`] for why the GPU dual-live path
    /// is otherwise unreachable from a headless test. No frontend calls this.
    #[cfg(test)]
    fn begin_transition_forced(&mut self, to: usize, mode: Mode) {
        self.begin_transition(to);
        if let Some(tr) = self.transition.as_mut() {
            tr.set_mode(mode);
        }
    }

    /// Land any dissolve in flight on its own target immediately, as if it had run
    /// to `t = 1`, leaving nothing running. The re-entrancy rule's first half: a
    /// switch arriving mid-dissolve finishes the current one before starting the
    /// new one, so the roster is never left on an index nobody asked for.
    ///
    /// Idempotent, and a no-op when nothing is running — callers invoke it to
    /// *settle* the roster before reading it, not only before replacing a dissolve.
    fn snap_finish_transition(&mut self) {
        let Some(running) = self.transition.take() else {
            return;
        };
        let target = running.incoming_index();
        // The roster is already on the target unless the dissolve was interrupted
        // before its capture frame ran — that frame is what flips it. Only then has
        // the incoming preset never been configured, and only then may the eased
        // params be reset; doing it unconditionally would snap a smoothed preset on
        // every mid-dissolve switch.
        let settled_early = self.roster.active != target;
        self.roster.select(target);
        // Its incoming side is promoted whether or not the roster moved: that preset
        // is the active one now, so its composite is the one the next dissolve's
        // outgoing side must use.
        self.promote_incoming_side();
        if settled_early {
            self.configure_active_scene();
        }
    }

    /// Make the dissolve's incoming composite *the* composite, dropping the
    /// outgoing one. Called at finalize (and when a switch snap-finishes another),
    /// so there is never a frame with both live or neither.
    fn promote_incoming_side(&mut self) {
        if let Some(incoming) = self.incoming_side.take() {
            self.side = incoming;
        }
    }

    /// Jump straight to `index` with no dissolve — the escape hatch for paths
    /// where a blend would be wrong (a capture, which must stay a pure function of
    /// its inputs) or meaningless (a switch to the already-active preset). Cancels
    /// any dissolve in flight and releases its GPU targets.
    fn select_preset_instantly(&mut self, index: usize) {
        if index >= self.roster.presets.len() {
            return; // out of range: a no-op, never a panic and never a wrap
        }
        self.cancel_transition();
        self.reset_transition_rotation();
        self.roster.select(index);
        self.configure_active_scene();
    }

    /// Drop any dissolve in flight and release its full-frame GPU targets, leaving
    /// the roster wherever it currently points. The caller decides the resolved
    /// index; this only tears down the blend.
    fn cancel_transition(&mut self) {
        // The incoming side has been rendering the preset the roster now points at,
        // so it is the one to keep — dropping it instead would restart that
        // preset's feedback history mid-show.
        self.promote_incoming_side();
        self.transition = None;
        self.blend.release_targets();
    }

    /// Restart the kind rotation. A **cut** — a roster hot-reload, a capture, or
    /// the [`select_preset_now`](Self::select_preset_now) escape — is the start of
    /// a new stretch of show, so the next dissolve begins the library again rather
    /// than resuming wherever the last one left off, and a scripted sequence of
    /// switches reproduces the same kinds from a known starting point.
    ///
    /// A *dissolving* switch does not reset it: the rotation is what gives a live
    /// show its variety, and restarting it on every browse-select would pin every
    /// hand-picked switch to the library's first kind.
    fn reset_transition_rotation(&mut self) {
        self.transitions_started = 0;
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
        let Self {
            scenes,
            roster,
            cap_overflow,
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
        // the line scenes ignore it. `spectrum` reproduces the prior cosine, so a
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
        // Structural config (ADR-0007), if any: capture segment-cap truncation so
        // the frontend can surface it (never a silent cut). `None` for the
        // fit/no-config case.
        if let Some(cfg) = preset.config.as_ref() {
            *cap_overflow = scene.configure(cfg);
        }
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

        let (width, height) = (self.ctx.config.width, self.ctx.config.height);
        let draw_calls = self.draw_frame(frame, &mut encoder, &view, width, height, dt);

        self.ctx.queue.submit(std::iter::once(encoder.finish()));
        self.ctx.queue.present(surface_tex);

        // Free atlas glyphs unused this frame and clear the queue for the next.
        #[cfg(feature = "text")]
        self.text_layer.end_frame();

        self.diag.set_draw_calls(draw_calls);
        self.diag.record_frame();
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
    /// Returns the draw-call count.
    fn draw_frame(
        &mut self,
        frame: &AnalysisFrame,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        width: u32,
        height: u32,
        dt: f32,
    ) -> u32 {
        let Self {
            ctx,
            scenes,
            side,
            incoming_side,
            ink,
            blend,
            transition,
            roster,
            time,
            diag,
            overlay,
            #[cfg(feature = "text")]
            text_layer,
            // Set at preset load, surfaced by the frontend — not a per-frame concern.
            cap_overflow: _,
            param_smoother,
            outgoing_smoother,
            // Switch-site policy state (the kind rotation) — not a per-frame concern.
            transitions_started: _,
        } = self;

        let Some(preset) = roster.active_preset() else {
            return 0; // no presets loaded — nothing to draw
        };
        let routes = roster.active_routes();

        // Evaluate against the shared clock and this frame's analysis. Both sides
        // of a dissolve read the same variables — they differ in what they *bind*.
        let vars = Variables::new(
            frame.bass,
            frame.mid,
            frame.treb,
            frame.onset,
            if frame.beat { 1.0 } else { 0.0 },
            frame.bar,
            *time,
            frame.bpm,
            frame.novelty,
        );

        // Fixed-order composite (ADR-0018/0028/0032): background (owns the clear)
        // -> scene -> the per-preset post chain -> [blend] -> ink -> present. Where
        // the scene draws and which chain stage folds into which is the chain's
        // business, not the renderer's — see `post.rs` for the order and the skip
        // rule. The blend and ink are engine-wide passes the renderer drives.
        let surface = (width, height);
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
                // No ink: the outgoing side's `ink_*` were held at the capture
                // frame and are crossfaded by the single engine-wide pass below.
                evaluate_preset(
                    outgoing,
                    out_routes,
                    out_scene,
                    side,
                    None,
                    outgoing_smoother,
                    &vars,
                    frame,
                    *time,
                    dt,
                );
                draw_calls += composite_into(ctx, out_scene, side, encoder, &out_view, surface);
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
        ink.reset_params();
        evaluate_preset(
            preset,
            routes,
            scene,
            live_side,
            Some(ink),
            param_smoother,
            &vars,
            frame,
            *time,
            dt,
        );

        // Ink is one engine-wide pass over the *blended* frame (ADR-0028), but a
        // dissolve has two presets each binding their own `ink_*`/`paper_*`. Lerp
        // the params — not two remapped frames, which is non-linear — so `t = 0` is
        // exactly the outgoing look and `t = 1` exactly the incoming one, with no
        // snap at either end. On the capture frame the outgoing preset is still the
        // active one, so its values are already correct and nothing is held yet.
        if let Some((from, t)) = transition
            .as_ref()
            .and_then(|tr| Some((tr.outgoing_ink()?, tr.progress())))
        {
            ink.crossfade_from(from, t);
        }

        // Ink is the terminal pass and lives outside the chain, so its input is
        // resolved *first*: everything upstream targets that view, and ink then
        // folds it into the surface. With ink off, the chain (or the blend) targets
        // the surface directly.
        let ink_input = if ink.active() {
            ink.begin(surface)
        } else {
            None
        };
        let terminal = ink_input.as_ref().unwrap_or(view);

        // Where the active side resolves. While a dissolve runs the blend sits
        // between the chain and ink, so it feeds one of the blend's two inputs: the
        // outgoing target on the opening frame, the live target on every frame
        // after. If the blend cannot build its targets, fall through to the terminal
        // view — a cut, never a blend of undefined pixels.
        let blend_input = match transition.as_ref() {
            Some(tr) if tr.needs_snapshot() => blend.snapshot_view(surface),
            Some(_) => blend.live_view(surface),
            None => None,
        };
        let destination = blend_input.as_ref().unwrap_or(terminal);

        draw_calls += composite_into(ctx, scene, live_side, encoder, destination, surface);
        if let (Some(tr), true) = (transition.as_ref(), blend_input.is_some()) {
            // Mix the outgoing side with the live incoming one into ink's input (or
            // the surface). At t = 0 this is the outgoing frame exactly, which is
            // what lets the opening frame present through the same pass before the
            // live side has ever been rendered into.
            draw_calls += blend.resolve(&ctx.queue, encoder, terminal, tr.progress(), tr.kind());
        }
        if ink_input.is_some() {
            draw_calls += ink.resolve(&ctx.queue, encoder, view);
        }

        // Hold the outgoing preset's evaluated ink params off the capture frame,
        // where the roster still points at it — the one frame they exist.
        let captured_ink = ink.params();

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
        let advanced = transition
            .as_mut()
            .map(|tr| (tr.advance(dt, captured_ink), tr.finished()));
        if let Some((flip_to, finished)) = advanced {
            if let Some(index) = flip_to {
                // Hand the outgoing preset's easing state to the outgoing side
                // before `configure_active_scene` resets the active one, so a
                // heavily-smoothed preset keeps easing through a dual-live dissolve
                // instead of snapping to raw values the frame it stops being active.
                self.outgoing_smoother = std::mem::take(&mut self.param_smoother);
                self.roster.select(index);
                self.configure_active_scene();
            }
            if finished {
                self.cancel_transition();
            }
        }

        draw_calls
    }

    /// Advance the scene clock one step and capture that single frame into an
    /// offscreen texture, returning tight RGBA (Plan 0013). Off the hot path —
    /// blocks on GPU readback; never call it from a live loop.
    pub fn capture_frame(&mut self, frame: &AnalysisFrame) -> Result<CaptureImage, RenderError> {
        self.time += scenes::FALLBACK_DT;
        self.capture_at_clock(frame)
    }

    /// Draw the active preset for `frame` at the **current** clock into a fresh
    /// offscreen texture and read it back. Does not advance the clock, so
    /// callers that already stepped it share this. The whole path (clear → draw
    /// → copy → map) is deterministic for a given `(preset, frame, clock)`.
    fn capture_at_clock(&mut self, frame: &AnalysisFrame) -> Result<CaptureImage, RenderError> {
        let (width, height) = (self.ctx.config.width, self.ctx.config.height);
        let format = self.ctx.surface_format();
        let (texture, view) = capture::create_target(&self.ctx.device, format, width, height);
        let (buffer, padded_bpr) = capture::create_readback(&self.ctx.device, width, height);
        let mut encoder = self
            .ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("lmv-capture-frame"),
            });
        capture::record_clear(&mut encoder, &view);
        let _ = self.draw_frame(
            frame,
            &mut encoder,
            &view,
            width,
            height,
            scenes::FALLBACK_DT,
        );
        capture::record_copy(&mut encoder, &texture, &buffer, padded_bpr, width, height);
        self.ctx.queue.submit(std::iter::once(encoder.finish()));

        #[cfg(feature = "text")]
        self.text_layer.end_frame();

        capture::read_back(&self.ctx.device, &buffer, width, height, padded_bpr)
    }

    /// Capture preset `name` after advancing it `frames` steps from a fixed
    /// initial state, driven by a single constant `frame` (Plan 0013). A **pure
    /// function** of `(name, frame, frames)`: the scenes are rebuilt so any
    /// stateful system (e.g. the seeded swarm particles) starts from its
    /// deterministic seed, and the scene clock resets to `0.0`, so the result is
    /// independent of any earlier capture. Errors if `name` is not in the
    /// roster. `frames` is treated as at least 1.
    pub fn capture_preset(
        &mut self,
        name: &str,
        frame: &AnalysisFrame,
        frames: u32,
    ) -> Result<CaptureImage, RenderError> {
        if !self.select_preset_by_name_now(name) {
            return Err(RenderError::UnknownPreset(name.to_string()));
        }
        // Reset simulation state to the deterministic seed and the clock to 0,
        // so the same (name, frame, frames) always yields identical pixels and
        // differential probes (Phase 3) isolate the stimulus, not history.
        self.scenes = scenes::create_all(&self.ctx.device, self.ctx.surface_format());
        self.cancel_transition();
        self.side.reset_resources();
        self.ink.reset_resources();
        self.blend.reset_resources();
        self.time = 0.0;
        // The rebuilt scenes are fresh — re-apply the active preset's structural
        // config (ADR-0007) so a line scene captures with its geometry built.
        self.configure_active_scene();

        let (width, height) = (self.ctx.config.width, self.ctx.config.height);
        let format = self.ctx.surface_format();
        let (texture, view) = capture::create_target(&self.ctx.device, format, width, height);

        // Warm the scene through the first frames-1 steps (state advances, pixels
        // discarded); then capture the final frame.
        let n = frames.max(1);
        for _ in 1..n {
            self.time += scenes::FALLBACK_DT;
            self.step_offscreen(frame, &view, width, height, scenes::FALLBACK_DT);
        }
        self.time += scenes::FALLBACK_DT;

        let (buffer, padded_bpr) = capture::create_readback(&self.ctx.device, width, height);
        let mut encoder = self
            .ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("lmv-capture-preset"),
            });
        capture::record_clear(&mut encoder, &view);
        let _ = self.draw_frame(
            frame,
            &mut encoder,
            &view,
            width,
            height,
            scenes::FALLBACK_DT,
        );
        capture::record_copy(&mut encoder, &texture, &buffer, padded_bpr, width, height);
        self.ctx.queue.submit(std::iter::once(encoder.finish()));

        #[cfg(feature = "text")]
        self.text_layer.end_frame();

        capture::read_back(&self.ctx.device, &buffer, width, height, padded_bpr)
    }

    /// Drive preset `name` with **real audio through the real analyzer** and
    /// capture the frames at `at_frames` (Plan 0013). The PCM is fed hop-by-hop
    /// into a fresh [`Analyzer`](crate::dsp::Analyzer) (format validated at the
    /// intake boundary — the source-agnostic rule); each produced
    /// [`AnalysisFrame`] drives one rendered frame, so `at_frames` indexes the
    /// hop sequence (frame 0 is the first hop). Deterministic: scenes are rebuilt
    /// to their seed and the clock resets to 0, exactly like
    /// [`capture_preset`](Self::capture_preset).
    ///
    /// This is in-memory PCM only — no file, decoder, or OS audio-source code,
    /// just like a frontend pushing samples. Returned images are in `at_frames`
    /// order; an index past the audio length is an error.
    pub fn capture_audio(
        &mut self,
        name: &str,
        pcm: &[f32],
        format: AudioFormat,
        at_frames: &[u32],
    ) -> Result<Vec<CaptureImage>, RenderError> {
        if !self.select_preset_by_name_now(name) {
            return Err(RenderError::UnknownPreset(name.to_string()));
        }
        let mut analyzer = crate::dsp::Analyzer::new(format).map_err(RenderError::AudioFormat)?;

        self.scenes = scenes::create_all(&self.ctx.device, self.ctx.surface_format());
        self.cancel_transition();
        self.side.reset_resources();
        self.ink.reset_resources();
        self.blend.reset_resources();
        self.time = 0.0;
        self.configure_active_scene();

        let (width, height) = (self.ctx.config.width, self.ctx.config.height);
        let target_format = self.ctx.surface_format();
        let (texture, view) =
            capture::create_target(&self.ctx.device, target_format, width, height);

        let hop_samples = crate::dsp::HOP_SIZE * format.channels as usize;
        let mut captured: Vec<(u32, CaptureImage)> = Vec::with_capacity(at_frames.len());

        for (index, hop) in pcm.chunks(hop_samples).enumerate() {
            let frame_index = index as u32;
            analyzer.push_interleaved(hop);
            let analysis = analyzer.take_frame();
            self.time += scenes::FALLBACK_DT;

            let wanted = at_frames.contains(&frame_index)
                && !captured.iter().any(|(i, _)| *i == frame_index);
            if wanted {
                let (buffer, padded_bpr) =
                    capture::create_readback(&self.ctx.device, width, height);
                let mut encoder =
                    self.ctx
                        .device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("lmv-capture-audio"),
                        });
                capture::record_clear(&mut encoder, &view);
                let _ = self.draw_frame(
                    &analysis,
                    &mut encoder,
                    &view,
                    width,
                    height,
                    scenes::FALLBACK_DT,
                );
                capture::record_copy(&mut encoder, &texture, &buffer, padded_bpr, width, height);
                self.ctx.queue.submit(std::iter::once(encoder.finish()));
                #[cfg(feature = "text")]
                self.text_layer.end_frame();
                let img = capture::read_back(&self.ctx.device, &buffer, width, height, padded_bpr)?;
                captured.push((frame_index, img));
            } else {
                self.step_offscreen(&analysis, &view, width, height, scenes::FALLBACK_DT);
            }
        }

        at_frames
            .iter()
            .map(|idx| {
                captured
                    .iter()
                    .find(|(i, _)| i == idx)
                    .map(|(_, img)| img.clone())
                    .ok_or(RenderError::CaptureReadback)
            })
            .collect()
    }

    /// Draw one frame into `view` and submit it — advancing scene state without
    /// reading anything back. The warm-up step [`capture_preset`] uses to reach
    /// frame `N`.
    fn step_offscreen(
        &mut self,
        frame: &AnalysisFrame,
        view: &wgpu::TextureView,
        width: u32,
        height: u32,
        dt: f32,
    ) {
        let mut encoder = self
            .ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("lmv-capture-step"),
            });
        capture::record_clear(&mut encoder, view);
        let _ = self.draw_frame(frame, &mut encoder, view, width, height, dt);
        self.ctx.queue.submit(std::iter::once(encoder.finish()));

        #[cfg(feature = "text")]
        self.text_layer.end_frame();
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
mod tests {
    // The pure roster contract, tested without a GPU surface (a live `Renderer`
    // can't be built headlessly). The `Renderer::preset_names`/`select_preset`
    // wrappers delegate to `Roster` 1:1, so this covers the addressing contract
    // Plan 0008 Phase 2 names. Test asserts use `expect`/`panic!`, allowed here
    // over the file's hot-path panic-denial pragma — test code is not the render
    // path (`headless_or_skip` panics on an unexpected build error).
    #![allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

    use super::{
        CaptureImage, HeadlessOptions, Mode, ParamRoute, ParamSmoother, RenderError, Renderer,
        Roster, resolve_route,
    };
    use crate::dsp::AnalysisFrame;
    use crate::preset::{Easing, Preset, SystemKind};
    use crate::render::metrics::frame_diff;
    use crate::render::post::{KALEIDOSCOPE, TRAILS};

    /// A minimal valid preset: a known system + explicit name, no params.
    fn preset(name: &str) -> Preset {
        Preset::from_toml_str(&format!("system = \"swarm\"\nname = \"{name}\""))
            .expect("hand-written test preset is valid")
    }

    fn roster(names: &[&str]) -> Roster {
        Roster::new(names.iter().map(|n| preset(n)).collect())
    }

    /// Plan 0031 Phase 3 — the routing contract, GPU-free: every global namespace
    /// resolves to its owner, the system's own names to the scene, and anything
    /// else to `Unclaimed` (dropped at apply time, already warned about at load).
    ///
    /// This is the answer the per-frame `set_param` fallthrough chain used to
    /// re-derive on every bound param of every frame.
    #[test]
    fn each_namespace_resolves_to_its_owner() {
        let swarm = SystemKind::Swarm;
        // The backdrop pre-pass, outside the chain (ADR-0031).
        for name in crate::render::background::PARAMS {
            assert_eq!(
                resolve_route(name, swarm),
                ParamRoute::Background,
                "`{name}` belongs to the backdrop"
            );
        }
        // The two chain stages, each to its own fixed position — not merely "some
        // stage", so a swapped `STAGE_PARAMS` order would fail here.
        for name in crate::render::trails::PARAMS {
            assert_eq!(resolve_route(name, swarm), ParamRoute::Stage(TRAILS));
        }
        for name in crate::render::kaleidoscope::PARAMS {
            assert_eq!(resolve_route(name, swarm), ParamRoute::Stage(KALEIDOSCOPE));
        }
        // The terminal engine-wide ink pass (ADR-0032) — `ink_*` and `paper_*`.
        for name in crate::render::ink::PARAMS {
            assert_eq!(
                resolve_route(name, swarm),
                ParamRoute::Ink,
                "`{name}` belongs to the ink pass"
            );
        }
        assert!(
            crate::render::ink::PARAMS.contains(&"ink_amount")
                && crate::render::ink::PARAMS
                    .iter()
                    .any(|n| n.starts_with("paper_")),
            "the ink vocabulary covers both the ink_* and paper_* halves"
        );

        // Everything a system declares goes to its scene — checked for **every**
        // system, so a family whose names happened to collide with a global
        // namespace could not slip through.
        for system in SystemKind::ALL {
            for name in system.param_names() {
                assert_eq!(
                    resolve_route(name, system),
                    ParamRoute::Scene,
                    "`{name}` is {}'s own param",
                    system.as_str()
                );
            }
        }

        // An unknown name is ignored, not an error and not mis-routed. Includes a
        // near-miss typo, which is the case the load-time warning names.
        for name in ["nope", "trail", "bg_", "kaleido", "ink", "warp_"] {
            assert_eq!(
                resolve_route(name, swarm),
                ParamRoute::Unclaimed,
                "`{name}` is claimed by nobody"
            );
        }
        // A param real on one system is not silently accepted on another: `warp`
        // is fragment-only, so on the swarm it is unclaimed.
        assert_eq!(
            resolve_route("warp", SystemKind::FragmentField),
            ParamRoute::Scene
        );
        assert_eq!(resolve_route("warp", swarm), ParamRoute::Unclaimed);
    }

    /// The routes a roster hands the frame loop line up positionally with the
    /// preset's bindings — the property `evaluate_preset`'s `zip` rests on.
    #[test]
    fn roster_routes_pair_with_each_presets_bindings() {
        let mixed = Preset::from_toml_str(
            "system = \"swarm\"\nname = \"Mixed\"\n[params]\n\
             bg_bright = \"0.5\"\ntrails = \"0.4\"\nkaleido_order = \"6\"\n\
             ink_amount = \"1\"\nforce = \"bass\"\nnot_a_param = \"1\"\n",
        )
        .expect("valid preset with one binding per route");
        let roster = Roster::new(vec![mixed]);
        let preset = roster.active_preset().expect("one preset");
        let routes = roster.active_routes();
        assert_eq!(routes.len(), preset.params.len(), "one route per binding");

        // Bindings are name-sorted at load, so read the pairing by name rather
        // than by position.
        let by_name: Vec<(&str, ParamRoute)> = preset
            .params
            .iter()
            .zip(routes)
            .map(|(binding, route)| (binding.name.as_str(), *route))
            .collect();
        assert!(by_name.contains(&("bg_bright", ParamRoute::Background)));
        assert!(by_name.contains(&("trails", ParamRoute::Stage(TRAILS))));
        assert!(by_name.contains(&("kaleido_order", ParamRoute::Stage(KALEIDOSCOPE))));
        assert!(by_name.contains(&("ink_amount", ParamRoute::Ink)));
        assert!(by_name.contains(&("force", ParamRoute::Scene)));
        assert!(by_name.contains(&("not_a_param", ParamRoute::Unclaimed)));
        // The unknown name loaded with a warning rather than failing (ADR-0020).
        assert_eq!(preset.warnings.len(), 1, "{:?}", preset.warnings);

        // Out of range is empty, pairing with `presets.get` returning `None`.
        assert!(roster.routes_for(9).is_empty());
    }

    /// `tau` is read off the `[smoothing]` table once at load, so the frame loop
    /// does no map lookup. An unlisted param is instant (ADR-0019), and a scalar
    /// entry means the same constant in both directions (ADR-0035).
    #[test]
    fn smoothing_taus_are_resolved_onto_the_bindings() {
        let p = Preset::from_toml_str(
            "system = \"swarm\"\n[params]\nforce = \"bass\"\nhue = \"time\"\n\
             size = \"treb\"\n\
             [smoothing]\nforce = 0.25\nsize = { attack = 0.02, release = 0.7 }\n",
        )
        .expect("valid preset with both [smoothing] forms");
        let tau_of = |name: &str| {
            p.params
                .iter()
                .find(|b| b.name == name)
                .map(|b| b.tau)
                .expect("bound param")
        };
        assert_eq!(
            tau_of("force"),
            Easing::symmetric(0.25),
            "a scalar is both directions"
        );
        assert_eq!(tau_of("hue"), Easing::INSTANT, "unlisted means instant");
        assert_eq!(
            tau_of("size"),
            Easing {
                attack: 0.02,
                release: 0.7
            },
            "the pair form resolves at the same boundary"
        );
    }

    /// Both constants are validated at the load boundary, and the error names the
    /// parameter — and, for a pair, which side of it. A bad value is a surfaced
    /// load error the caller degrades on, never a panic (ADR-0002 / NFR 10).
    #[test]
    fn a_bad_smoothing_constant_is_a_load_error_naming_the_parameter() {
        let load = |table: &str| {
            Preset::from_toml_str(&format!(
                "system = \"swarm\"\n[params]\nforce = \"bass\"\n[smoothing]\n{table}\n"
            ))
        };
        let err = load("force = -1.0")
            .expect_err("negative scalar")
            .to_string();
        assert!(
            err.contains("force") && err.contains("non-negative"),
            "{err}"
        );
        assert!(load("force = nan").is_err(), "non-finite scalar");
        assert!(load("force = inf").is_err(), "non-finite scalar");

        // Each side of a pair is checked, and the message says which one.
        let err = load("force = { attack = -1.0, release = 0.7 }")
            .expect_err("negative attack")
            .to_string();
        assert!(
            err.contains("force") && err.contains("attack"),
            "the error must name the failing side: {err}"
        );
        let err = load("force = { attack = 0.02, release = nan }")
            .expect_err("non-finite release")
            .to_string();
        assert!(err.contains("release"), "{err}");

        // A malformed table is as clear as a malformed float: both expected keys
        // are named, half a pair is rejected rather than silently defaulted, and
        // a wrong type says what was wanted.
        let err = load("force = { atack = 0.02, release = 0.7 }")
            .expect_err("misspelled key")
            .to_string();
        assert!(
            err.contains("attack") && err.contains("release"),
            "an unknown key must name the expected ones: {err}"
        );
        assert!(
            load("force = { attack = 0.02 }").is_err(),
            "half a pair is a mistake, not a shorthand"
        );
        assert!(
            load("force = { release = 0.7 }").is_err(),
            "half a pair is a mistake, not a shorthand"
        );
        let err = load("force = \"fast\"").expect_err("a string").to_string();
        assert!(
            err.contains("attack") && err.contains("release"),
            "a wrong type must state both accepted forms: {err}"
        );
    }

    /// Build a headless `Renderer`, or return `None` (a logged skip) when the
    /// runner exposes no usable GPU adapter (ADR-0016). A missing adapter is an
    /// environmental property of the CI runner — macOS has no software Metal
    /// fallback — not a code failure, so the GPU-capture tests skip on it rather
    /// than panic; any *other* build error still panics loudly. On Windows WARP
    /// an adapter is always present, so the callers' assertions run in full.
    fn headless_or_skip(opts: HeadlessOptions) -> Option<Renderer> {
        match Renderer::new_headless(opts) {
            Ok(r) => Some(r),
            Err(RenderError::RequestAdapter(_)) => {
                eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
                None
            }
            Err(e) => panic!("headless renderer build failed: {e}"),
        }
    }

    #[test]
    fn names_are_yielded_in_roster_order() {
        let r = roster(&["alpha", "bravo", "charlie"]);
        let got: Vec<&str> = r.names().collect();
        assert_eq!(got, ["alpha", "bravo", "charlie"]);
    }

    #[test]
    fn select_addresses_by_absolute_index() {
        let mut r = roster(&["alpha", "bravo", "charlie"]);
        assert_eq!(r.name(), "alpha"); // a fresh roster starts at index 0
        r.select(2);
        assert_eq!(r.name(), "charlie"); // the third entry
    }

    #[test]
    fn out_of_range_select_is_a_no_op() {
        let mut r = roster(&["alpha", "bravo", "charlie"]);
        r.select(1);
        r.select(999); // past the end: unchanged — no panic, no wrap
        assert_eq!(r.name(), "bravo");
    }

    #[test]
    fn set_presets_clamps_active_when_the_roster_shrinks() {
        let mut r = roster(&["alpha", "bravo", "charlie"]);
        r.select(2);
        r.set_presets(vec![preset("solo")]); // index 2 now out of range
        assert_eq!(r.name(), "solo");
    }

    /// Phase 1 (Plan 0013): a surface-less renderer captures the active preset
    /// into an offscreen texture. `prefer_software` (WARP on DX12) keeps it
    /// reproducible on any adapter. Asserts a full tight RGBA buffer with at
    /// least one non-black pixel — the preset actually drew.
    #[test]
    fn headless_captures_a_non_black_frame() {
        let Some(mut renderer) = headless_or_skip(HeadlessOptions {
            width: 256,
            height: 256,
            prefer_software: true,
        }) else {
            return;
        };

        let img = renderer
            .capture_frame(&AnalysisFrame::default())
            .expect("capture succeeds");

        assert_eq!(img.width, 256);
        assert_eq!(img.height, 256);
        assert_eq!(img.rgba.len(), 256 * 256 * 4, "tight RGBA, no row padding");
        let non_black = img
            .rgba
            .chunks_exact(4)
            .any(|px| px[0] > 0 || px[1] > 0 || px[2] > 0);
        assert!(non_black, "the active preset drew at least one lit pixel");
    }

    /// Phase 2 (Plan 0013): `capture_preset` is a pure function of
    /// `(name, frame, frames)`. Uses the stateful swarm preset "Drift" — the
    /// case where a missing state reset would leak history — to prove two
    /// captures are byte-identical, that N=1 differs from N=120 (the scene
    /// animates), and that an unknown name is a clean error.
    #[test]
    fn capture_preset_is_deterministic_and_animates() {
        let Some(mut renderer) = headless_or_skip(HeadlessOptions {
            width: 128,
            height: 128,
            prefer_software: true,
        }) else {
            return;
        };
        let frame = AnalysisFrame::default();

        let a = renderer
            .capture_preset("Drift", &frame, 120)
            .expect("capture Drift @120");
        let b = renderer
            .capture_preset("Drift", &frame, 120)
            .expect("recapture Drift @120");
        assert_eq!(
            a.rgba, b.rgba,
            "same (preset, frame, N) is byte-identical across calls"
        );

        let one = renderer
            .capture_preset("Drift", &frame, 1)
            .expect("capture Drift @1");
        assert_ne!(
            one.rgba, a.rgba,
            "N=1 differs from N=120 — the scene advances over time"
        );

        assert!(
            renderer
                .capture_preset("no-such-preset", &frame, 1)
                .is_err(),
            "an unknown preset name is a clean error, not a panic"
        );
    }

    /// Plan 0010 review finding #1: a line generator that hits the segment cap
    /// must **surface** the truncation, never cut silently (ADR-0007). An
    /// L-system whose depth blows past the cap reports a `CapOverflow` through
    /// `configure`, read back via `cap_overflow()`; a grammar that fits reports
    /// `None`. This is the surfacing half of the cap contract the mechanism
    /// tracked but nothing exercised.
    #[test]
    fn oversized_lsystem_surfaces_a_cap_overflow() {
        let Some(mut renderer) = headless_or_skip(HeadlessOptions {
            width: 64,
            height: 64,
            prefer_software: true,
        }) else {
            return;
        };

        // F -> ten F's per iteration: depth 5 is 100k draw steps, far past the
        // 20k cap, so the build truncates and must report the drop.
        let huge = Preset::from_toml_str(
            "system = \"lsystem\"\nname = \"Huge\"\n\
             [generator]\naxiom = \"F\"\nrules = { F = \"FFFFFFFFFF\" }\n\
             angle_deg = 20\nmax_depth = 5\n",
        )
        .expect("valid lsystem preset");
        renderer.set_presets(vec![huge]);
        let overflow = renderer
            .cap_overflow()
            .expect("an oversized L-system surfaces its cap truncation");
        assert!(
            overflow.dropped > 0,
            "the dropped-segment count is reported"
        );

        // A modest grammar (F -> FF, depth 5 = 32 segments) fits — no overflow.
        let small = Preset::from_toml_str(
            "system = \"lsystem\"\nname = \"Small\"\n\
             [generator]\naxiom = \"F\"\nrules = { F = \"FF\" }\n\
             angle_deg = 20\nmax_depth = 5\n",
        )
        .expect("valid lsystem preset");
        renderer.set_presets(vec![small]);
        assert!(
            renderer.cap_overflow().is_none(),
            "a grammar that fits within the cap reports no overflow"
        );
    }

    /// Plan 0018 Phase 4: the per-frame geometry mirror must also surface a cap
    /// truncation through `cap_overflow()`, reusing the ADR-0007 `CapOverflow`
    /// path — never a silent cut. A dense rose replicated six-fold blows past the
    /// 20k cap; a modest one fits. Unlike the L-system's load-time overflow, this
    /// one is computed per frame, so it surfaces only after a frame has rendered.
    #[test]
    fn oversized_mirror_surfaces_a_cap_overflow() {
        let Some(mut renderer) = headless_or_skip(HeadlessOptions {
            width: 64,
            height: 64,
            prefer_software: true,
        }) else {
            return;
        };
        let frame = AnalysisFrame::default();

        // ~5000 chords replicated six-fold = ~30k segments, far past the 20k cap.
        let huge = Preset::from_toml_str(
            "system = \"parametric_curve\"\nname = \"MirrorHuge\"\n\
             [curve]\nfamily = \"maurer_rose\"\n\
             [params]\nsamples = \"5000\"\nmirror_order = \"6\"\n",
        )
        .expect("valid parametric preset");
        renderer.set_presets(vec![huge]);
        // Render frames so the per-frame mirror replication runs and records the drop.
        renderer
            .capture_preset("MirrorHuge", &frame, 2)
            .expect("capture MirrorHuge");
        let overflow = renderer
            .cap_overflow()
            .expect("an oversized mirror surfaces its cap truncation");
        assert!(
            overflow.dropped > 0,
            "the dropped-segment count is reported"
        );

        // A modest rose at order 3 stays well under the cap — no overflow.
        let small = Preset::from_toml_str(
            "system = \"parametric_curve\"\nname = \"MirrorSmall\"\n\
             [curve]\nfamily = \"maurer_rose\"\n\
             [params]\nsamples = \"200\"\nmirror_order = \"3\"\n",
        )
        .expect("valid parametric preset");
        renderer.set_presets(vec![small]);
        renderer
            .capture_preset("MirrorSmall", &frame, 2)
            .expect("capture MirrorSmall");
        assert!(
            renderer.cap_overflow().is_none(),
            "a mirror that fits within the cap reports no overflow"
        );
    }

    /// Phase 5 (ADR-0019): a step change eases toward the target over several
    /// frames instead of snapping, and converges. The one-pole is the whole point.
    #[test]
    fn smoothing_eases_a_step_instead_of_snapping() {
        let mut s = ParamSmoother::default();
        let dt = 1.0 / 60.0;
        let tau = Easing::symmetric(0.1);
        // The first value after a reset snaps (it seeds the state).
        assert_eq!(s.smooth(0, 0.0, tau, dt), 0.0);
        // A step to 1.0 closes only a fraction of the gap — eased, not snapped.
        let f1 = s.smooth(0, 1.0, tau, dt);
        assert!(f1 > 0.0 && f1 < 1.0, "eased, not snapped: {f1}");
        let f2 = s.smooth(0, 1.0, tau, dt);
        assert!(f2 > f1 && f2 < 1.0, "monotonic approach: {f1} -> {f2}");
        // Many frames of the held target converge to it.
        for _ in 0..600 {
            s.smooth(0, 1.0, tau, dt);
        }
        assert!(
            (s.smooth(0, 1.0, tau, dt) - 1.0).abs() < 1e-3,
            "converges to the held target"
        );
    }

    /// ADR-0035: the same step, up and then down, under
    /// `{ attack = 0.02, release = 0.7 }`. The property is the **asymmetry** —
    /// a snap up and a glide down — which no single `tau` reaches at any value.
    ///
    /// The absolute figures are what this filter actually does at 60 Hz:
    /// `alpha = 1 - exp(-dt/tau)` closes 56.5 % of the gap per frame at
    /// `tau = 0.02`, so two frames reach 81 % and three reach 92 %. (Plan 0033's
    /// done-when says "90 % within two frames"; that is one frame optimistic for
    /// this constant — the assertion below pins the arithmetic, not the prose.)
    #[test]
    fn asymmetric_easing_snaps_up_and_glides_down() {
        let mut s = ParamSmoother::default();
        let dt = 1.0 / 60.0;
        let e = Easing {
            attack: 0.02,
            release: 0.7,
        };

        // Seed at 0, then step to 1.0 and watch the rise.
        assert_eq!(s.smooth(0, 0.0, e, dt), 0.0);
        let after_two = {
            s.smooth(0, 1.0, e, dt);
            s.smooth(0, 1.0, e, dt)
        };
        assert!(
            after_two >= 0.80,
            "attack = 0.02 must cover most of the step in two 60 Hz frames, got {after_two}"
        );
        let after_three = s.smooth(0, 1.0, e, dt);
        assert!(
            after_three >= 0.90,
            "three frames reach 90 % of the target, got {after_three}"
        );

        // Settle, then step back to 0 and watch the fall over 0.4 s.
        for _ in 0..300 {
            s.smooth(0, 1.0, e, dt);
        }
        let mut falling = 0.0;
        for _ in 0..(0.4 / dt) as usize {
            falling = s.smooth(0, 0.0, e, dt);
        }
        assert!(
            falling > 0.50,
            "release = 0.7 must still be above half a second's worth of glide after \
             0.4 s, got {falling}"
        );

        // The asymmetry itself, stated as a comparison rather than two constants:
        // the rise covers far more of its gap in two frames than the fall does.
        let mut sym = ParamSmoother::default();
        let slow = Easing::symmetric(0.7);
        sym.smooth(0, 0.0, slow, dt);
        sym.smooth(0, 1.0, slow, dt);
        let symmetric_two = sym.smooth(0, 1.0, slow, dt);
        assert!(
            after_two > symmetric_two * 10.0,
            "a 0.02 s attack must be dramatically faster than the 0.7 s release used \
             symmetrically ({after_two} vs {symmetric_two}) — otherwise one constant \
             would have done"
        );
    }

    /// ADR-0035's compatibility claim, checked rather than asserted in prose: a
    /// scalar `[smoothing]` entry and an explicit `{ attack = t, release = t }`
    /// table are **bit-identical** through the whole load-and-smooth path. This is
    /// why no shipped preset moved and no golden was re-blessed.
    #[test]
    fn a_scalar_smoothing_entry_is_bit_identical_to_an_equal_pair() {
        let load = |table: &str| {
            Preset::from_toml_str(&format!(
                "system = \"swarm\"\n[params]\nforce = \"bass\"\n[smoothing]\nforce = {table}\n"
            ))
            .expect("valid preset")
            .params
            .first()
            .expect("one binding")
            .tau
        };
        let scalar = load("0.31");
        let pair = load("{ attack = 0.31, release = 0.31 }");
        assert_eq!(scalar, pair, "the two forms resolve to the same constants");

        // Drive both through the smoother with a signal that rises *and* falls, so
        // the direction branch is exercised in both directions, and compare raw
        // bits — an epsilon compare would hide exactly the drift this rules out.
        let dt = 1.0 / 60.0;
        let (mut a, mut b) = (ParamSmoother::default(), ParamSmoother::default());
        for i in 0..240 {
            let raw = ((i as f32) * 0.11).sin() * 0.5 + 0.5;
            let va = a.smooth(0, raw, scalar, dt);
            let vb = b.smooth(0, raw, pair, dt);
            assert_eq!(
                va.to_bits(),
                vb.to_bits(),
                "frame {i}: scalar {va} != pair {vb}"
            );
        }
    }

    /// `tau = 0` (the default for an unlisted param) is today's instant behaviour,
    /// and ADR-0035 keeps `0` meaning instant **per side**.
    #[test]
    fn zero_tau_passes_through_instantly() {
        let mut s = ParamSmoother::default();
        let dt = 1.0 / 60.0;
        assert_eq!(s.smooth(0, 0.5, Easing::INSTANT, dt), 0.5);
        assert_eq!(
            s.smooth(0, 0.9, Easing::INSTANT, dt),
            0.9,
            "tau=0 snaps every frame"
        );

        // A zero on one side only: that direction snaps while the other still
        // eases. `{ attack = 0, release = 0.5 }` is the "instant hit, slow decay"
        // an author reaches for on a percussive accent.
        let half = Easing {
            attack: 0.0,
            release: 0.5,
        };
        let mut s = ParamSmoother::default();
        assert_eq!(s.smooth(1, 0.0, half, dt), 0.0, "seeds");
        assert_eq!(s.smooth(1, 1.0, half, dt), 1.0, "attack = 0 snaps up");
        let falling = s.smooth(1, 0.0, half, dt);
        assert!(
            falling > 0.0 && falling < 1.0,
            "release = 0.5 still eases down: {falling}"
        );
    }

    /// A reset makes the next frame snap to the incoming value — the mechanism
    /// behind a preset switch snapping to the new preset (no cross-preset bleed).
    #[test]
    fn reset_snaps_to_the_next_value() {
        let mut s = ParamSmoother::default();
        let dt = 1.0 / 60.0;
        let tau = Easing::symmetric(0.2);
        s.smooth(0, 0.0, tau, dt);
        for _ in 0..10 {
            s.smooth(0, 1.0, tau, dt); // partway toward 1.0
        }
        s.reset();
        assert_eq!(
            s.smooth(0, 5.0, tau, dt),
            5.0,
            "after a reset the next value seeds fresh — a snap, no stale bleed"
        );
    }

    /// Phase 5 determinism (NFR 6): a preset with a `[smoothing]` table, captured
    /// twice, is byte-identical — the smoother state resets on the capture
    /// scene-rebuild, so a capture stays a pure function of its inputs.
    #[test]
    fn smoothed_preset_capture_is_deterministic() {
        let Some(mut renderer) = headless_or_skip(HeadlessOptions {
            width: 96,
            height: 96,
            prefer_software: true,
        }) else {
            return;
        };
        let smoothed = Preset::from_toml_str(
            "system = \"fragment_field\"\nname = \"Smoothed\"\n\
             [params]\nwarp = \"0.3 + bass * 0.4\"\nhue = \"0.2\"\nglow = \"0.8\"\n\
             [smoothing]\nwarp = 0.25\n",
        )
        .expect("valid smoothed preset");
        renderer.set_presets(vec![smoothed]);
        let frame = AnalysisFrame {
            bass: 0.8,
            ..Default::default()
        };
        let a = renderer
            .capture_preset("Smoothed", &frame, 30)
            .expect("capture Smoothed a");
        let b = renderer
            .capture_preset("Smoothed", &frame, 30)
            .expect("capture Smoothed b");
        assert_eq!(
            a.rgba, b.rgba,
            "smoothing state resets on rebuild -> identical recaptures"
        );
    }

    /// Phase 6 determinism (NFR 6): a preset with `trails` (the feedback stage),
    /// captured twice, is byte-identical — the accumulation resets on the capture
    /// scene-rebuild, so a capture stays a pure function of its inputs even though
    /// the trail is stateful across frames.
    #[test]
    fn trailed_preset_capture_is_deterministic() {
        let Some(mut renderer) = headless_or_skip(HeadlessOptions {
            width: 96,
            height: 96,
            prefer_software: true,
        }) else {
            return;
        };
        // A spinning rose with a long trail: the accumulation carries state across
        // the warm-up frames, so a missing reset would leak between captures.
        let trailed = Preset::from_toml_str(
            "system = \"parametric_curve\"\nname = \"Trailed\"\n\
             [curve]\nfamily = \"maurer_rose\"\n\
             [params]\nn = \"3\"\nspin = \"0.9\"\nsamples = \"120\"\ntrails = \"0.8\"\n",
        )
        .expect("valid trailed preset");
        renderer.set_presets(vec![trailed]);
        let frame = AnalysisFrame::default();
        let a = renderer
            .capture_preset("Trailed", &frame, 20)
            .expect("capture Trailed a");
        let b = renderer
            .capture_preset("Trailed", &frame, 20)
            .expect("capture Trailed b");
        assert_eq!(
            a.rgba, b.rgba,
            "trails accumulation resets on rebuild -> identical recaptures"
        );
    }

    /// Plan 0028: the two new shape params (`radial_offset`, `phase`) are
    /// preset-bindable and actually reach the sampler. A rose that binds both to
    /// a `bass` expression, driven by a bass stimulus, must render differently
    /// from an identical rose with both unbound (default `0.0`) — proof the
    /// evaluated values thread through `set_param` into the geometry, not just
    /// that the preset parses.
    #[test]
    fn shape_params_reach_the_parametric_scene() {
        let Some(mut renderer) = headless_or_skip(HeadlessOptions {
            width: 96,
            height: 96,
            prefer_software: true,
        }) else {
            return;
        };
        let frame = AnalysisFrame {
            bass: 1.0,
            ..Default::default()
        };

        // Baseline: shape params unbound, so radial_offset = phase = 0.0 even
        // under the bass stimulus — the plain rose.
        let baseline = Preset::from_toml_str(
            "system = \"parametric_curve\"\nname = \"ShapeBaseline\"\n\
             [curve]\nfamily = \"maurer_rose\"\n\
             [params]\nn = \"6\"\nd = \"71\"\nsamples = \"200\"\nscale = \"0.9\"\n",
        )
        .expect("valid baseline parametric preset");
        renderer.set_presets(vec![baseline]);
        let base = renderer
            .capture_preset("ShapeBaseline", &frame, 4)
            .expect("capture ShapeBaseline");

        // Same rose, but radial_offset and phase are bound to the bass stimulus.
        let bound = Preset::from_toml_str(
            "system = \"parametric_curve\"\nname = \"ShapeBound\"\n\
             [curve]\nfamily = \"maurer_rose\"\n\
             [params]\nn = \"6\"\nd = \"71\"\nsamples = \"200\"\nscale = \"0.9\"\n\
             radial_offset = \"bass * 0.6\"\nphase = \"bass * 2.0\"\n",
        )
        .expect("valid shape-bound parametric preset");
        renderer.set_presets(vec![bound]);
        let lit = renderer
            .capture_preset("ShapeBound", &frame, 4)
            .expect("capture ShapeBound");

        assert_ne!(
            base.rgba, lit.rgba,
            "bound radial_offset/phase must change the rendered geometry"
        );
    }

    /// **The governor's wiring**, not its arithmetic (Plan 0023 close review, minor
    /// 4). `dual_live_eligible` and `shares_resources` are each covered where they
    /// live; what nothing exercised is `dissolve_mode` composing them — including
    /// the arm that decides what an *unresolvable* preset index means.
    ///
    /// GPU-free: `Roster` and the preset list are enough, so this runs everywhere.
    #[test]
    fn dissolve_mode_freezes_a_shared_scene_pair_and_an_unresolvable_one() {
        let Some(mut renderer) = headless_or_skip(HeadlessOptions {
            width: 64,
            height: 64,
            prefer_software: true,
        }) else {
            return;
        };
        let of = |name: &str, body: &str| {
            Preset::from_toml_str(&format!("name = \"{name}\"\n{body}"))
                .expect("hand-written test preset is valid")
        };
        // 0 and 1 are the same system (one scene object); 2 is a different line
        // system (a *different* scene that still shares the one `LineRenderer`); 3
        // holds genuinely independent GPU state.
        renderer.set_presets(vec![
            of("SameA", "system = \"parametric_curve\"\n"),
            of("SameB", "system = \"parametric_curve\"\n"),
            of(
                "OtherLine",
                "system = \"star_pattern\"\n[generator]\ntiling = \"8\"\n",
            ),
            of("Field", "system = \"fragment_field\"\n"),
        ]);

        assert_eq!(
            renderer.dissolve_mode(0, 1),
            Mode::Freeze,
            "two presets on one `SystemKind` are one mutable scene object"
        );
        assert_eq!(
            renderer.dissolve_mode(0, 2),
            Mode::Freeze,
            "two line systems share one `LineRenderer`, so neither may render twice"
        );
        // An index the roster cannot resolve must read as *shared*, not as
        // independent: the safe answer to "can we render both?" is no.
        assert_eq!(
            renderer.dissolve_mode(0, 99),
            Mode::Freeze,
            "an unresolvable preset index must not be read as an independent pair"
        );
        assert_eq!(
            renderer.dissolve_mode(99, 0),
            Mode::Freeze,
            "...from either side"
        );

        // The independent pair is the only one that *could* upgrade — and it still
        // freezes here, because a headless renderer collects no frame times and the
        // governor upgrades only on positive evidence of headroom. Asserting that
        // pins the second half of the composition: passing the veto is necessary,
        // not sufficient.
        assert_eq!(
            renderer.dissolve_mode(2, 3),
            Mode::Freeze,
            "independent scenes still need frame-time evidence, which a headless \
             capture never has"
        );
    }

    // --- Plan 0023 Phase 4: the adaptive dual-live upgrade -------------------
    //
    // These live inside the crate rather than in `tests/transition.rs` because a
    // headless capture cannot reach the dual-live path from outside: diagnostics
    // are off, so the governor has no frame-time evidence and correctly answers
    // `Freeze` every time. `begin_transition_forced` is the crate-private,
    // `#[cfg(test)]` way in — the shipped API grows nothing.
    //
    // Both tests are **differential**: they run the *same* dissolve twice, with
    // only the mode changed, so any difference is the live outgoing side and
    // nothing else. That is stronger than a threshold on one run, which could pass
    // on scene animation alone.

    /// The outgoing preset — a rose that both **spins** and leaves a long trail, so
    /// it has motion to show and cross-frame state to preserve.
    ///
    /// Spun **fast** and faded **slowly** on purpose: the accumulated smear has to
    /// cover a lot more of the frame than one stroke does, or "the trail survived"
    /// and "the trail restarted" differ by too little to assert. At this rate the
    /// warm-up sweeps the rose through its whole symmetry period.
    fn spinning_trailed_rose() -> Preset {
        Preset::from_toml_str(
            "system = \"parametric_curve\"\nname = \"DualA\"\n\
             [curve]\nfamily = \"maurer_rose\"\n\
             [params]\nn = \"3\"\nd = \"71\"\nsamples = \"300\"\nscale = \"0.85\"\n\
             spin = \"6.0\"\ntrails = \"0.98\"\n",
        )
        .expect("valid spinning trailed rose")
    }

    /// The incoming preset — a fragment field, so the pair resolves to **different
    /// scene objects** with independent GPU state (not two of the three line scenes
    /// sharing one renderer), which is what dual-live requires.
    fn moving_field() -> Preset {
        Preset::from_toml_str(
            "system = \"fragment_field\"\nname = \"DualB\"\n\
             [params]\nwarp = \"0.5\"\nhue = \"0.2\"\nglow = \"0.9\"\n",
        )
        .expect("valid fragment field preset")
    }

    /// How many frames the outgoing preset renders before the switch — the length
    /// of trail history the dissolve inherits. [`WARMED`] is well past the point
    /// where the accumulation dominates the picture; [`COLD`] is the counterfactual
    /// a restarted chain would look like.
    const WARMED: usize = 60;
    const COLD: usize = 1;

    /// Capture one dissolve at a forced fidelity, after `warmup` frames of the
    /// outgoing preset. Returns the dissolve window, opening frame first.
    ///
    /// `software` picks the adapter. **A trail's survival across the switch can
    /// only be seen on real hardware**: on the DX12 WARP rasterizer, allocating the
    /// dissolve's GPU resources mid-run (the blend's targets, the incoming side's
    /// chain) resets what the trails feedback resolves to, so the outgoing side
    /// comes back at a single stroke's brightness whether it has one frame of
    /// history or thirty. That is the same coexisting-pipeline quirk
    /// `trails.rs` documents and `tests/background_composite.rs` skips for; on
    /// hardware the dissolve's opening frame is byte-identical to the ordinary
    /// frame it replaces. Checks that only compare two dissolves against each other
    /// stay on WARP, where they run in CI.
    fn dissolve_at(
        mode: Mode,
        frames: usize,
        warmup: usize,
        software: bool,
    ) -> Option<Vec<CaptureImage>> {
        let mut renderer = headless_or_skip(HeadlessOptions {
            width: 96,
            height: 96,
            prefer_software: software,
        })?;
        if !software && renderer.adapter_is_software() {
            eprintln!(
                "skipped: only a software rasterizer is available (WARP drops the \
                 trails accumulation when the dissolve allocates; see dissolve_at)"
            );
            return None;
        }
        let stimulus = AnalysisFrame::default();
        renderer.set_presets(vec![spinning_trailed_rose(), moving_field()]);
        for _ in 0..warmup.max(1) {
            renderer.capture_frame(&stimulus).expect("warm-up frame");
        }
        renderer.begin_transition_forced(1, mode);
        Some(
            (0..frames)
                .map(|i| {
                    renderer
                        .capture_frame(&stimulus)
                        .unwrap_or_else(|e| panic!("dissolve frame {i}: {e}"))
                })
                .collect(),
        )
    }

    /// **Both visuals animate through a dual-live dissolve.** Same presets, same
    /// `dt` sequence, same blend kind — only the fidelity differs, so any pixel
    /// difference is the outgoing side still rendering.
    ///
    /// The opening frame must be *identical* in both modes: it is the outgoing
    /// preset's own composite either way, before dual-live has anything extra to
    /// do. That pins the assertion to the dissolve rather than to a warm-up drift.
    #[test]
    fn dual_live_keeps_the_outgoing_side_animating() {
        const FRAMES: usize = 40;
        let (Some(frozen), Some(live)) = (
            dissolve_at(Mode::Freeze, FRAMES, WARMED, true),
            dissolve_at(Mode::DualLive, FRAMES, WARMED, true),
        ) else {
            return; // no GPU adapter (ADR-0016)
        };

        assert_eq!(
            frozen[0].rgba, live[0].rgba,
            "the opening frame is the outgoing composite in either mode"
        );

        let mid = frame_diff(&frozen[FRAMES / 2], &live[FRAMES / 2]);
        assert!(
            mid > 0.01,
            "mid-dissolve the outgoing side must still be moving, not held \
             (freeze vs dual-live differ by only {mid})"
        );
    }

    /// Mean Rec. 709 luminance in bytes — how much light a frame carries, which is
    /// what a feedback trail adds and a restarted one would not.
    fn mean_luma(img: &CaptureImage) -> f32 {
        let sum: f32 = img
            .rgba
            .chunks_exact(4)
            .map(|px| 0.2126 * px[0] as f32 + 0.7152 * px[1] as f32 + 0.0722 * px[2] as f32)
            .sum();
        sum / (img.rgba.len() / 4) as f32
    }

    /// **A dual-live dissolve out of a trails-on preset keeps that trail.** The
    /// outgoing side re-renders through the composite it has been using all along,
    /// so its accumulation carries into the dissolve instead of restarting.
    ///
    /// Measured as **brightness against the same dissolve run cold** — the
    /// counterfactual of the bug. A restarted chain would enter the dissolve with a
    /// single stroke's worth of light, which is exactly what the cold run has, so
    /// the two would read alike; carrying thirty frames of decay-0.9 history makes
    /// the warmed run several times brighter.
    ///
    /// The reference cannot be the frozen run at the same warm-up: freeze and
    /// dual-live take the same opening frame through the same composite, so a bug
    /// that restarted the chain at the switch would move both together and the
    /// comparison would still pass. The cold run moves only with the trail history,
    /// which is the claim.
    ///
    /// **Real hardware only** — WARP cannot show a trail surviving the dissolve's
    /// allocations at all (see [`dissolve_at`]).
    #[test]
    fn a_dual_live_dissolve_carries_the_outgoing_trail() {
        const FRAMES: usize = 4;
        // Halfway between the two outcomes rather than close to either: a restarted
        // chain reads 1.0x the cold run by construction, and the carried trail
        // measures ~1.9x it on the dev box. The floor cannot go lower — the cold run
        // still draws the same stroke this one does; only the swept history differs.
        const CARRIES: f32 = 1.5;
        let (Some(warmed), Some(cold), Some(frozen)) = (
            dissolve_at(Mode::DualLive, FRAMES, WARMED, false),
            dissolve_at(Mode::DualLive, FRAMES, COLD, false),
            dissolve_at(Mode::Freeze, FRAMES, WARMED, false),
        ) else {
            return; // no adapter, or only a software one
        };

        let carries = |warm: &CaptureImage, restarted: &CaptureImage, what: &str| {
            let (got, floor) = (mean_luma(warm), mean_luma(restarted));
            assert!(
                floor > 0.0 && got > CARRIES * floor,
                "{what} must carry the outgoing preset's accumulated trail, not \
                 restart from a fresh accumulation ({got} against {floor} for the \
                 same dissolve run cold)"
            );
        };
        // The opening frame is the outgoing preset's own composite...
        carries(&warmed[0], &cold[0], "the dissolve's opening frame");
        // ...and the first dual-live re-render — the frame the outgoing side is
        // drawn a second time, at ~98% outgoing weight — must still carry it.
        carries(&warmed[1], &cold[1], "the first dual-live frame");

        // And it is genuinely re-rendering rather than reusing the held texture:
        // the spin has moved the geometry even though the light is preserved.
        assert!(
            frame_diff(&frozen[1], &warmed[1]) > 0.0,
            "the outgoing side re-renders; it does not reuse the snapshot"
        );
    }
}
