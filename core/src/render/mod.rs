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
// The secondary present target (ADR-0143): a second surface on this same
// device, carrying text the shell queues for it. Feature-gated with the text
// layer it draws through — see the module docs.
//
// NOT `aux.rs`: `AUX` is a reserved DOS device name, so on Windows that path
// cannot be opened by the compiler even though the file creates fine.
#[cfg(feature = "text")]
pub mod aux_target;
pub(crate) mod background;
pub(crate) mod bloom;
pub mod capture;
// The `capture_*` entry points themselves — a continuation of `impl Renderer`
// (Plan 0061 Phase 3). Private, because it adds no path of its own: every method
// in it is reached as `Renderer::capture_*`, exactly as before the split.
mod capture_api;
pub mod context;
pub mod feedback;
// The program preview's intermediate and its letterbox geometry (ADR-0143).
// NOT feature-gated with `aux_target`: the console that consumes a preview
// draws text, but the intermediate is a render path, and the property that
// matters about it is asserted on the headless capture path — which compiles
// glyphon out.
pub(crate) mod gpu;
pub(crate) mod grid;
pub(crate) mod ink;
pub(crate) mod kaleidoscope;
pub mod preview;
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
use crate::preset::{
    Easing, Expr, LATCH_CAP, Latch, Layer, LayerJoin, Preset, SystemKind, Variables,
};
#[cfg(feature = "text")]
pub use aux_target::AuxPresentMode;
#[cfg(feature = "text")]
use aux_target::AuxTarget;
use background::Background;
pub use capture::{CaptureImage, FrameTap};
pub use capture_api::AudioCapture;
pub use context::{AdapterChoice, AdapterDescription, RenderContext, RenderError, list_adapters};
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
pub use tier::{REFERENCE_PX, Tier, TierConfig, attractor_budget};
use tonemap::Tonemap;
use transition::{Blend, DEFAULT_DURATION_SECS, Transition, TransitionKind};

/// The format **every intermediate upstream of the tonemap** carries: linear
/// light, unbounded above 1.0 (ADR-0046, Plan 0045 Phase 3).
///
/// The scene targets, both post stages, the transition blend's two sides and the
/// tonemap's own input are all this — the surface format stops at the tonemap,
/// which is where the frame becomes display-referred. Running those
/// intermediates at the surface's 8 bits instead clips an additive accumulation
/// per channel at each hand-off: the "additive ceiling" ADR-0046's Context
/// catalogues, and the reason a bright-pass had nothing correct to bloom from.
///
/// `Rgba16Float` rather than 32-bit because it is the format
/// [`PingPongField`](feedback::PingPongField) ships on (Plan 0014) — proven
/// blendable and filterable on both backends — and because half
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

// The five concerns this file keeps out of the `Renderer` (Plan 0126 Phase 2).
// `routing` answers where a name goes and which scene a system has, `roster` the
// loaded presets and their smoothers, `evaluate` one frame's bindings,
// `composite` one side's encode, `tier_governor` the demotion path as an
// `impl Renderer` continuation. What stays here is the `Renderer` itself.
mod composite;
mod evaluate;
mod roster;
mod routing;
mod tier_governor;

use composite::*;
use evaluate::*;
use roster::*;
use routing::*;

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
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RendererOptions {
    /// An explicit tier pin, or `None` for auto — which resolves [`Tier::Rich`]
    /// and leaves the frame-time governor free to demote it once (ADR-0045). A
    /// pin is honoured in both directions and never demotes.
    pub tier: Option<Tier>,
    /// Which graphics adapter the window's context asks for.
    ///
    /// [`AdapterChoice::Default`] is what the surface path asked for before this
    /// field existed and stays the default here: an operator's `--gpu` is a
    /// lever, not a new preference. Changing what an *unflagged* window selects
    /// would re-base every frame-time figure this project has published, which
    /// is a measurement question and not an argument-parsing one (ADR-0155).
    ///
    /// Carrying a [`AdapterChoice::Named`] `String` is why this struct is
    /// `Clone` rather than `Copy`.
    pub adapter: AdapterChoice,
    /// Which of the tier's two attractor sample ceilings to resolve against
    /// (ADR-0140). [`SampleBudget::Live`] by default, so every caller that does
    /// not ask resolves exactly what it resolved before the choice existed.
    pub budget: SampleBudget,
}

impl RendererOptions {
    /// Options pinning `tier` explicitly, on the default adapter.
    pub fn pinned(tier: Tier) -> Self {
        Self {
            tier: Some(tier),
            ..Self::default()
        }
    }

    /// [`pinned`](Self::pinned), against the **offline** sample ceiling — for a
    /// headless render, which has no present deadline to answer to.
    pub fn pinned_offline(tier: Tier) -> Self {
        Self {
            tier: Some(tier),
            budget: SampleBudget::Offline,
            ..Self::default()
        }
    }
}

/// Which of a tier's two sample ceilings this renderer resolves against
/// (ADR-0140).
///
/// The **law is the same either way** — a budget is
/// `clamp(round(anchor * target_px / REFERENCE_PX), anchor, ceiling)` — and this
/// picks the `ceiling`. It is a construction choice and never a per-frame one:
/// the ceiling is also the **allocation**, so changing it means rebuilding the
/// scene.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SampleBudget {
    /// Frame-time bound: a window, the plugin's host surface, and every capture
    /// path — a still, a filmstrip, a report, the golden and sanity suites.
    ///
    /// **A capture takes this one even though it has no deadline either**, and
    /// that is deliberate rather than an oversight: the offline ceiling is a
    /// larger *allocation*, and `AttractorScene::seed`'s scatter is a function of
    /// how many particles were asked for, so handing a capture path the offline
    /// ceiling would move every committed baseline without moving a single
    /// resolved count.
    #[default]
    Live,
    /// Memory bound: `shot --render`, which walks a clip end to end with `dt`
    /// injected and answers to no display.
    Offline,
}

/// Owns the GPU context, the built-in systems, and the loaded presets; renders
/// one frame per call by evaluating the active preset into the active system.
pub struct Renderer {
    ctx: RenderContext,
    /// Which sample ceiling this renderer's scenes were built against
    /// (ADR-0140). Read wherever the scenes are rebuilt — a tier change, a
    /// capture reset — so a rebuild cannot silently swap the ceiling under a
    /// run that is already producing frames.
    budget: SampleBudget,
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
    /// The secondary present target (ADR-0143), `None` until a shell attaches
    /// one and again the moment it detaches. Holds a swapchain and a text atlas,
    /// so the `None` case is the whole cost of the feature while unused.
    #[cfg(feature = "text")]
    aux: Option<AuxTarget>,
    /// The program preview's intermediate (ADR-0143), `None` unless a shell has
    /// opened one. While it is `Some` the frame is drawn into it and reaches the
    /// real destination by an exact copy; while it is `None` nothing is
    /// allocated, no copy is encoded and the frame path is what it was — which
    /// is what makes the console free when it is closed.
    preview: Option<preview::PreviewTarget>,
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
    /// The active preset's `[latch]` state (ADR-0137). Reset wherever
    /// [`param_smoother`](Self::param_smoother) is, and handed to the outgoing
    /// bank at the same roster flip — a latch mid-hold keeps reading through a
    /// dual-live dissolve exactly as an eased param keeps easing.
    ///
    /// One bank per preset, not per surface: a latch is preset-level state and
    /// its `[layer]` bindings read the same event the main scene does, so unlike
    /// [`layer_smoother`](Self::layer_smoother) there is no second one.
    latches: LatchBank,
    /// The outgoing preset's `[latch]` state during a dual-live dissolve.
    outgoing_latches: LatchBank,
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
        let budget = opts.budget;
        let scenes =
            crate::render::scenes::create_all(&ctx.device, COMPOSITE_FORMAT, &tier, budget);
        let side = CompositeSide::new(&ctx.device, COMPOSITE_FORMAT, &tier);
        let blend = Blend::new(&ctx.device, COMPOSITE_FORMAT);
        let tonemap = Tonemap::new(&ctx.device, ctx.surface_format());
        let ink = Ink::new(&ctx.device, ctx.surface_format());
        let overlay = Overlay::new(&ctx.device, ctx.surface_format());
        #[cfg(feature = "text")]
        let text_layer = TextLayer::new(&ctx.device, &ctx.queue, ctx.surface_format());
        let mut renderer = Self {
            ctx,
            budget,
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
            #[cfg(feature = "text")]
            aux: None,
            preview: None,
            now_playing: NowPlaying::default(),
            cap_overflow: None,
            series_scratch: vec![0.0; scenes::lines::spectrum::MAX_ELEMENTS],
            vertex_scratch: vec![0.0; scenes::warp_mesh::vertex_count(scenes::warp_mesh::MAX_MESH)],
            param_smoother: ParamSmoother::default(),
            latches: LatchBank::default(),
            outgoing_latches: LatchBank::default(),
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
    /// `opts` carries the quality-tier pin and the adapter choice;
    /// [`RendererOptions::default()`](RendererOptions) is auto (rich, governed)
    /// on whatever adapter wgpu picks for the surface.
    pub fn new(
        target: impl Into<wgpu::SurfaceTarget<'static>>,
        width: u32,
        height: u32,
        opts: RendererOptions,
    ) -> Result<Self, RenderError> {
        let ctx = RenderContext::new(target, width, height, &opts.adapter)?;
        Ok(Self::from_context(ctx, opts))
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
        Self::new_headless_on(opts, tier, &AdapterChoice::from(opts.prefer_software))
    }

    /// A headless renderer pinned to `tier`, on a **named** adapter (ADR-0146).
    ///
    /// The one real headless constructor; the two above delegate here with the
    /// choice their `prefer_software` flag already implies, so every capture
    /// path resolves exactly the adapter it resolved before.
    ///
    /// A live video-out needs this because the adapter is not a performance
    /// preference there: a Spout receiver can only open a sender that lives on
    /// the GPU it renders with, and on a hybrid machine a console process is
    /// handed the power-saving one. The **sender's** adapter is what that
    /// constrains; this one is the renderer's, and the two are matched by name
    /// on each side rather than by a shared index.
    pub fn new_headless_on(
        opts: HeadlessOptions,
        tier: Tier,
        adapter: &AdapterChoice,
    ) -> Result<Self, RenderError> {
        Ok(Self::from_context(
            RenderContext::new_headless_on(opts.width, opts.height, adapter)?,
            RendererOptions::pinned(tier),
        ))
    }

    /// A headless renderer pinned to `tier` and resolving the **offline** sample
    /// ceiling (ADR-0140) — the one constructor `shot --render` reaches, and the
    /// only one in the engine that does.
    ///
    /// Separate from [`new_headless_tiered`](Self::new_headless_tiered) rather
    /// than a flag on it, because the two answer to different bounds and only one
    /// of them may ever produce a baseline: a render walks a clip with `dt`
    /// injected and no display to miss, so its ceiling is memory; every other
    /// headless path is a capture, and a capture takes the live ceiling so the
    /// allocation — and with it the seeded scatter — is the one every committed
    /// baseline was blessed against.
    pub fn new_headless_offline(opts: HeadlessOptions, tier: Tier) -> Result<Self, RenderError> {
        Ok(Self::from_context(
            RenderContext::new_headless_on(
                opts.width,
                opts.height,
                &AdapterChoice::from(opts.prefer_software),
            )?,
            RendererOptions::pinned_offline(tier),
        ))
    }

    /// Renderer targeting a surface the host owns and built the handle for —
    /// the C ABI path, and the only constructor that does not create its own
    /// surface. Starts with the embedded default presets (no ABI surface for
    /// preset selection yet).
    ///
    /// **The platform lives on the caller's side of this seam, not here**
    /// (ADR-0001, ADR-0072): the host knows what kind of window it has and
    /// builds the [`wgpu::SurfaceTargetUnsafe`] for it, so `core` stays
    /// source-agnostic and platform-free. `core-cabi` is the one that knows
    /// about `HWND`.
    ///
    /// Auto tier: the plugin gets rich-with-governor, because the C ABI stays v4
    /// and a plugin-side tier picker is a future ABI question rather than part of
    /// ADR-0045.
    ///
    /// # Safety
    /// `target`'s handles must be valid and must outlive this renderer.
    pub unsafe fn new_from_surface_target(
        target: wgpu::SurfaceTargetUnsafe,
        width: u32,
        height: u32,
    ) -> Result<Self, RenderError> {
        // The `unsafe` is exactly the surface-from-raw-handle call: the caller's
        // promise about the handles' validity and lifetime. Construction past
        // that point is the same safe code the other two paths run.
        // `RendererOptions::default()` carries `AdapterChoice::Default`, which
        // is the request this path made before the choice was a parameter — the
        // shim has no flag surface to select an adapter with, and the C ABI
        // does not move for this (ADR-0155).
        let opts = RendererOptions::default();
        let ctx = unsafe { RenderContext::new_unsafe(target, width, height, &opts.adapter) }?;
        Ok(Self::from_context(ctx, opts))
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

    /// The index the show is **going** to: the dissolve's target while a
    /// transition is in flight, and [`active_index`](Self::active_index)
    /// otherwise. This is the "where the show is going, not where it has been"
    /// convention [`cycle_preset`](Self::cycle_preset) already returns a name by,
    /// expressed as an index — a host checkmarking a menu wants the user's most
    /// recent choice to be ticked immediately, not a quarter-second later.
    ///
    /// Meaningless on an empty roster (`0`, like `active_index`); a caller that
    /// distinguishes that case reads [`preset_names`](Self::preset_names) first.
    pub fn target_preset_index(&self) -> usize {
        self.transition
            .as_ref()
            .map_or(self.roster.active, Transition::incoming_index)
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

    /// Reconfigure the surface for a new window size.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.ctx.resize(width, height);
        // The intermediate's copy extent is fixed at construction, so a live
        // preview is rebuilt at the new size rather than left disagreeing with
        // the destination it copies into.
        if self.preview.is_some() {
            self.preview = Some(preview::PreviewTarget::new(
                &self.ctx.device,
                self.ctx.surface_format(),
                self.ctx.config.width,
                self.ctx.config.height,
            ));
        }
    }

    /// Open the program preview: the frame starts being drawn into an
    /// intermediate and copied to its destination, so a second consumer can
    /// sample the same pixels the show is getting.
    ///
    /// Fails when the destination surface does not accept `COPY_DST`, which is
    /// the one thing that makes the copy exact. Reported rather than degraded
    /// to a sampling blit: a preview is worth less than a show whose encoded
    /// values silently changed.
    ///
    /// Idempotent in effect — an already-open preview is rebuilt at the current
    /// size, which is also how a caller follows a resize it did not see.
    pub fn open_preview(&mut self) -> Result<(), RenderError> {
        if !self.ctx.can_copy_to_target() {
            return Err(RenderError::UnsupportedSurface);
        }
        self.preview = Some(preview::PreviewTarget::new(
            &self.ctx.device,
            self.ctx.surface_format(),
            self.ctx.config.width,
            self.ctx.config.height,
        ));
        Ok(())
    }

    /// Release the intermediate. Idempotent, and the frame path returns to
    /// drawing straight at its destination on the very next frame.
    pub fn close_preview(&mut self) {
        self.preview = None;
    }

    /// The open preview's size and identity, or `None` when closed.
    pub fn preview_state(&self) -> Option<((u32, u32), u64)> {
        self.preview.as_ref().map(|p| (p.size(), p.generation()))
    }

    /// Attach a **secondary present target** — a second window's surface, on
    /// this renderer's existing device (ADR-0143).
    ///
    /// The core learns nothing about what that window is for. It presents the
    /// runs it is handed and nothing else; the shell decides their meaning.
    /// Returns the present mode the surface negotiated, so the caller can log
    /// which arm ran.
    ///
    /// An already-attached target is replaced. An `Err` means this adapter
    /// cannot drive that surface — the dual-GPU case — and the caller is
    /// expected to degrade rather than treat it as fatal: the show is on the
    /// primary surface, which is unaffected.
    #[cfg(feature = "text")]
    pub fn attach_aux(
        &mut self,
        target: impl Into<wgpu::SurfaceTarget<'static>>,
        width: u32,
        height: u32,
    ) -> Result<AuxPresentMode, RenderError> {
        let aux = AuxTarget::new(&self.ctx, target, width, height)?;
        let mode = aux.present_mode();
        self.aux = Some(aux);
        Ok(mode)
    }

    /// Release the secondary target, its swapchain and its text atlas. Idempotent.
    #[cfg(feature = "text")]
    pub fn detach_aux(&mut self) {
        self.aux = None;
    }

    /// Whether a secondary target is currently attached.
    #[cfg(feature = "text")]
    pub fn aux_attached(&self) -> bool {
        self.aux.is_some()
    }

    /// Resize the secondary target's swapchain. No-op with none attached.
    #[cfg(feature = "text")]
    pub fn resize_aux(&mut self, width: u32, height: u32) {
        if let Some(aux) = self.aux.as_mut() {
            aux.resize(&self.ctx.device, width, height);
        }
    }

    /// The secondary target's size in physical pixels, or `None` when detached.
    #[cfg(feature = "text")]
    pub fn aux_size(&self) -> Option<(u32, u32)> {
        self.aux.as_ref().map(AuxTarget::size)
    }

    /// Draw `runs` on the secondary target and present it. No-op with none
    /// attached.
    ///
    /// Deliberately **not** called from [`render`](Self::render): the two
    /// surfaces present independently, so a console that stalls cannot pace the
    /// show, and a frame the output drops does not have to cost the console one.
    #[cfg(feature = "text")]
    pub fn present_aux(&mut self, runs: &[TextRun<'_>]) -> Result<(), RenderError> {
        match self.aux.as_mut() {
            Some(aux) => aux.present(&self.ctx, runs, self.preview.as_ref()),
            None => Ok(()),
        }
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

    /// The active adapter's description — name, backend, device type and driver.
    ///
    /// **For reports that have to name the machine they were taken on**
    /// (ADR-0071): a frame time is a fact about a GPU and a driver rather
    /// than about the code, so a cost instrument that prints one has to say
    /// which. Read by `core/tests/collage_cost.rs`; nothing on a render path
    /// consults it.
    pub fn adapter_description(&self) -> &str {
        self.ctx.adapter()
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
                label: Some("rlx-frame"),
            });

        // After the acquire, so a dropped frame does not leave a second copy of
        // the banner queued behind the one the next frame pushes.
        #[cfg(feature = "text")]
        self.queue_now_playing();

        let (width, height) = (self.ctx.config.width, self.ctx.config.height);
        // Moved out of `self` for the draw, which takes `&mut self`; put back
        // below. With no preview open this is `None` and the frame is drawn
        // straight at the swapchain view, exactly as it was.
        let preview = self.preview.take();
        // The one live call site: a preset that asked for `seed = "random"` gets
        // the salt it drew at load (ADR-0051). Every other caller of `draw_frame`
        // is a capture and pins.
        let draw_calls = self.draw_frame(
            frame,
            &mut encoder,
            preview.as_ref().map_or(&view, |p| &p.view),
            (width, height),
            dt,
            SaltMode::Live,
        );
        if let Some(p) = preview.as_ref() {
            p.record_copy_to(&mut encoder, &surface_tex.texture);
        }
        self.preview = preview;

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
    fn draw_frame(
        &mut self,
        frame: &AnalysisFrame,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        surface: (u32, u32),
        dt: f32,
        salt: SaltMode,
    ) -> u32 {
        let Self {
            ctx,
            // Read where the scenes are BUILT, not where they are drawn - a
            // frame never consults it.
            budget: _,
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
            // The caller decided which view this frame draws into and owns the
            // copy out of it; from in here the intermediate is just the target.
            preview: _,
            series_scratch,
            vertex_scratch,
            param_smoother,
            layer_smoother,
            outgoing_smoother,
            outgoing_layer_smoother,
            latches,
            outgoing_latches,
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
            // The secondary surface presents on its own encoder in `present_aux`,
            // never from inside a frame encode: the output's pixels must not
            // depend on whether a console is attached (ADR-0143).
            #[cfg(feature = "text")]
                aux: _,
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
        // The **render target's** aspect, which `PostChain::begin` reports as the
        // surface's whatever internal grid the chain routes through (ADR-0037).
        // Computed once here because the per-vertex evaluation below happens
        // before the chain opens, and `rad`/`ang` must be aspect-corrected.
        let surface_aspect = surface.0 as f32 / surface.1.max(1) as f32;
        let mut draw_calls = 0;
        // What both sides evaluate through. Held across the two `evaluate_side`
        // calls below rather than rebuilt, so the scratch buffers are sliced from
        // one owner and the salt cannot be taken from two different bundles.
        let mut shared = SideInputs {
            tier,
            series: series_scratch,
            vertex: vertex_scratch,
            aspect: surface_aspect,
            vars,
            frame,
            time: *time,
            dt,
            salt,
        };

        // The outgoing side first, because it feeds the blend.
        let dual_live = transition.as_ref().is_some_and(Transition::is_dual_live);
        if dual_live {
            draw_calls += encode_outgoing_side(
                ctx,
                encoder,
                surface,
                Outgoing {
                    transition: transition.as_ref(),
                    roster,
                    blend,
                    scenes,
                    side,
                    smoother: outgoing_smoother,
                    layer_smoother: outgoing_layer_smoother,
                    latches: outgoing_latches,
                },
                &mut shared,
            );
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
        draw_calls += encode_active_side(
            ctx,
            encoder,
            view,
            surface,
            ActiveSide {
                active: Active { preset, routes },
                scene,
                composite: live_side,
                smoother: param_smoother,
                layer_smoother,
                latches,
            },
            DisplayTail {
                blend,
                tonemap,
                ink,
                transition: transition.as_ref(),
            },
            &mut shared,
        );

        // Hold the outgoing preset's evaluated terminal params off the capture
        // frame, where the roster still points at it — the one frame they exist.
        let captured_ink = ink.params();
        let captured_exposure = tonemap.exposure();

        draw_calls += encode_on_canvas(
            ctx,
            encoder,
            view,
            surface,
            OnCanvas {
                #[cfg(feature = "text")]
                text_layer,
                diag,
                overlay,
                tier: tier.tier,
                tier_demoted: *tier_demoted,
            },
        );

        // The borrows above all end here, so `self` is free again (NLL).
        self.advance_transition(dt, dual_live, captured_ink, captured_exposure);

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

#[cfg(test)]
mod milk_wash;
