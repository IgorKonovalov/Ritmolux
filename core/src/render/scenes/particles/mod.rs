//! GPU compute-particle scenes: strange attractors (ADR-0015, Plan 0016). The
//! engine's **first compute pipeline** — a storage buffer of particles stepped
//! through an attractor map each frame by a compute shader, then drawn as
//! additive point-sprites with fading trails. This is idiom B of the four render
//! idioms; the CPU [`swarm`](super::swarm) is idiom B's ~10k CPU precursor,
//! replaced here by GPU-resident state that scales to 100k+ points with no CPU
//! round-trip.
//!
//! Trails reuse Plan 0014's [`PingPongField`](crate::render::feedback) rather
//! than a second feedback mechanism: each frame the previous accumulation texture
//! is drawn back faded (decay pass), the fresh points are added on top
//! (additive), and the result is composited to the surface (present pass). Trail
//! persistence is the named `fade` parameter; `fade = 0` clears the accumulation
//! each frame, reproducing the trail-free look.
//!
//! Every knob is an ADR-0002 layer-2 named parameter — the attractor
//! coefficients (`a`,`b`,`c`,`d`), look scalars (`size`,`hue`,`fade`), and a
//! beat-driven `reseed` — so a preset steers the cloud's shape and a beat
//! disturbs it. All randomness is the seeded initial scatter plus the reseed's
//! deterministic per-particle kick (NFR 6): the
//! point cloud is a pure function of the seed and the fixed-`dt` step sequence,
//! so a capture reproduces bit-for-bit on one adapter.
//!
//! **GPU resources are built lazily, on first render** — the same discipline the
//! reaction-diffusion scene uses (see its module docs). `create_all` builds every
//! scene up front, but the compute pipeline + storage buffer + trail field are
//! constructed only when this scene is first drawn, so a capture that never
//! activates it never builds them (keeping the other scenes' WARP captures
//! unperturbed).
//!
//! The accumulation field is sized to the render target and capped (Plan 0027
//! Phase 2, now the tier's `attractor_trail_cap`) rather than fixed at 640x360, so the
//! present is close to 1:1 up to the cap instead of a soft upscale on a 1080p+
//! display. That size is quantized to `TRAIL_GRID_STEP`, so a live window drag
//! re-allocates the field a handful of times rather than once per frame.
//!
//! **The field's own aspect is not the projection's** (Plan 0029 Phase 5). The
//! present is a plain stretch (aspect ignored, as the reaction-diffusion present
//! does), so a point at field NDC `x` lands at target NDC `x` — the field's aspect
//! cancels out and the projection must use the **target's**. Quantization makes
//! the two genuinely differ (a 1920x1080 target takes a 2048x1280 grid), so
//! [`trail_grid_size`] scaling both axes by one factor at the cap is about keeping
//! the field's *sampling* near-isotropic, not about the shape on screen.

// Hot-path panic-denial pragma (Plan 0002 Phase 2, extended to scenes by Plan
// 0003 Phase 0). Steps + draws every displayed frame.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

// The four concerns this directory always was (Plan 0061 Phase 6). `family` is
// the GPU-free ODE/basis math, `shaders` the four WGSL programs, `resources` the
// wgpu buffers/pipelines/bind groups; what stays here is the scene, its `Scene`
// impl, the param surface and the `encode_*` passes.
mod encode;
pub mod family;
pub mod ifs;
pub mod resources;
mod shaders;

// `AttractorFamily` and `Basis` were `pub` here before the split and are named
// from outside `particles`, so they keep their old path rather than gaining a
// `family::` segment. Everything else is `pub(super)` in its new file: exactly
// the visibility it had as a private member of this module, not `pub(crate)`,
// which would widen it.
pub use family::{AttractorFamily, Basis};

use encode::*;
use family::*;
use resources::*;
use shaders::*;

use crate::render::gpu;

use ifs::{FitLut, IfsFigure, IfsPacked, IfsTable, Levers};

use super::{Scene, SeededRng};
use crate::dsp::AnalysisFrame;
use crate::render::feedback::PingPongField;
use crate::render::palette::{self, Palette};

/// Compute workgroup size (1D). 64 is a safe, portable default across DX12/Metal.
const WORKGROUP: u32 = 64;
const SEED: u64 = 0x4C4D_5641_5454_5231; // "LMVATTR1"

/// Grid size before the first
/// [`Scene::set_target_size`](crate::render::scenes::Scene::set_target_size) —
/// only reached if a scene renders without one, which the renderer never does.
const TRAIL_FALLBACK_W: u32 = 1280;
const TRAIL_FALLBACK_H: u32 = 720;
/// Quantization step for each axis of the trail grid (Plan 0029 Phase 2).
///
/// A grid change costs a texture-pair reallocation, four bind groups, and a trail
/// restart, and the standalone forwards **every** `WindowEvent::Resized` — so at
/// pixel granularity a live drag pays that hundreds of times across a screen. At
/// 256 px per axis a full-screen-width drag crosses a handful of grids and every
/// other frame of it costs a compare. Coarser wastes fill (a 1920-wide window
/// already takes a 2048-wide grid); finer defeats the point. Purely a constant —
/// no wall clock, so a fixed-size headless capture stays byte-reproducible.
const TRAIL_GRID_STEP: u32 = 256;

/// The trail accumulation grid for a render target of `width` x `height` — this
/// scene's **cap and step** over the one shared policy
/// ([`grid::grid_size`](crate::render::grid::grid_size)).
///
/// A thin wrapper on purpose (Plan 0035 Phase 3). The arithmetic used to live
/// here, and `post.rs` held a line-for-line copy of it; that duplication is how
/// the aspect lesson this scene already paid for failed to reach the post stages
/// and shipped as a defect a second time (ADR-0037). The **numbers** stay here,
/// because they are genuinely this call site's — see
/// [`TierConfig::attractor_trail_cap`](crate::render::TierConfig::attractor_trail_cap)
/// for why the attractor may take a larger grid than a post stage.
///
/// **Still `pub`, deliberately.** Plan 0029's close logged this as a nit (public
/// API widened for a test's benefit), and Plan 0035 re-examined it while touching
/// the function: `core/tests/attractor.rs` is an integration test and can only
/// reach a `pub` item, so narrowing to `pub(crate)` means moving that test set
/// into the crate — a change to a file outside this plan's scope, for no
/// behavioral gain. `core/` is not a published API surface; the cost of the
/// widening is a doc-comment's worth of noise, and the cost of the churn is a
/// silent scope expansion. Kept.
pub fn trail_grid_size(width: u32, height: u32, cap: (u32, u32)) -> (u32, u32) {
    crate::render::grid::grid_size((width, height), cap, TRAIL_GRID_STEP)
}

/// Wall-clock duration of one attractor iteration (Plan 0014 injected `dt`). The
/// fixed-timestep accumulator runs one compute step per `FIXED_STEP` of injected
/// real `dt`, so the cloud evolves at the same rate on any refresh — at the
/// live/capture `dt` of 1/60 s this is exactly one step per frame. Continuous
/// (ODE) families added later integrate by this fixed sub-step, so the map is
/// frame-rate-independent without the shader reading a clock.
const FIXED_STEP: f32 = 1.0 / 60.0;
/// Max steps encoded in one frame — a long stall drops its backlog rather than
/// queueing unbounded compute work (accumulator spiral-of-death guard, as the
/// reaction-diffusion scene does). One step per frame is the norm at 60 fps.
const MAX_SUBSTEPS: u32 = 6;

/// Dynamically-offset slots in the step uniform buffer: one per possible
/// sub-step, plus the jitter dispatch's.
///
/// **One slot per sub-step and not one slot reused**, because the IFS's map
/// choice reads a per-step counter off this uniform (ADR-0075). A frame encodes
/// its `pending_steps` dispatches into one command buffer, and a
/// `queue.write_buffer` between two `encoder` calls does not interleave with them
/// — it lands before the whole submission — so a single slot would hand every
/// sub-step of a stalled frame the *same* step index, and a particle would apply
/// the same map two or three times running. Harmless for the four map families,
/// which do not read it; a quality loss precisely when the frame budget is
/// already blown. Only `pending_steps` slots are written per frame, so the
/// steady-state 60 fps cost is the one write it always was.
const STEP_SLOTS: u32 = MAX_SUBSTEPS + 1;
/// The jitter dispatch's slot — one past the sub-step slots.
const JITTER_SLOT: u32 = MAX_SUBSTEPS;

/// `morph`'s default: the configured figure, unmixed (ADR-0075). An IFS preset
/// that never binds it draws exactly the figure it named.
const DEFAULT_MORPH: f32 = 0.0;

/// Parameter defaults — a calm idle look when nothing is bound.
const DEFAULT_SIZE: f32 = 1.0;
const DEFAULT_HUE: f32 = 0.0;
/// `brightness`'s default (ADR-0080): a multiply by **literal `1.0`**, which is
/// the identity in IEEE-754 — so an unbound preset renders byte-identically to
/// the build before this param existed, and no golden baseline moves.
///
/// The name matches [`swarm`](super::swarm::PARAMS) and
/// [`emitter`](super::emitter::PARAMS) exactly. Three scenes draw additive
/// particle marks and this is the one lever that says how bright; a fourth name
/// for it would be a vocabulary an author has to re-learn per scene.
const DEFAULT_BRIGHTNESS: f32 = 1.0;
/// Depth-cue defaults (ADR-0076): **exactly the pre-ADR-0076 behaviour**. At
/// `perspective = 0` the magnification is `1 / (1 - 0 * d_n)` = `1`, and a
/// multiply by `1.0` is exact — so an unbound preset is byte-identical and no
/// golden baseline moves.
const DEFAULT_PERSPECTIVE: f32 = 0.0;
/// The two atmospheric cues, likewise inert at their defaults: `depth_fade = 0`
/// leaves the brightness multiplier exactly `1`, and `depth_hue = 0` adds an
/// exact `0` to the palette coordinate.
///
/// **They come as a pair on purpose.** Distance washing out contrast is the
/// oldest depth cue there is (ADR-0044), but dimness alone is ambiguous with a
/// thing simply *being dimmer*; a hue shift is what makes it read as **distance**,
/// because real atmospheric perspective moves colour as well as contrast. They
/// are also the substitute for the occlusion ADR-0076 declines to do: far
/// material is attenuated until it stops competing with near material, which is
/// what reads as depth for a diffuse cloud that cannot hide anything.
const DEFAULT_DEPTH_FADE: f32 = 0.0;
const DEFAULT_DEPTH_HUE: f32 = 0.0;
/// ADR-0087's colour channels, all four inert at their default. `*_tint` adds an
/// exact `0` to the palette coordinate; `*_hue` compares equal to literal `0.0`
/// and takes `shift_hue`'s early return, so no capture moves through a
/// round trip that is not bit-exact.
///
/// One constant for all four rather than four spellings of `0.0`: they are the
/// same claim — *the default is the identity* — and it is the claim, not the
/// number, that has to hold.
const DEFAULT_CHANNEL_COLOUR: f32 = 0.0;
/// Ceiling on `perspective`, applied silently where the uniform is packed.
///
/// `perspective` means **the figure's depth half-extent as a fraction of the
/// camera distance**, so the near-to-far magnification ratio is
/// `(1 + p) / (1 - p)`: `0.5` gives 3:1 and this value gives 9:1 (the far end at
/// 0.556, the near end at 5.0). The singularity — a point reaching the camera
/// plane — sits at exactly `1`, and this is well short of it. The arithmetic
/// holds because `d_n` is clamped to `[-1, 1]` before it is used — see
/// `depth_norm` in the draw shader for why that clamp is not decoration.
const MAX_PERSPECTIVE: f32 = 0.8;
// Shared palette color knobs (ADR-0021 / Plan 0020 Phase 5). The per-particle
// seed jitter occupies `hue_center + (seed - 0.5)*hue_spread`; the defaults
// (`spread = 0.15`, `center = 0.075`) reduce to `seed*0.15` — the prior hardcoded
// jitter — so an unbound attractor is unchanged (`saturation = 1`, `mix = 0`).
const DEFAULT_HUE_SPREAD: f32 = 0.15;
const DEFAULT_HUE_CENTER: f32 = 0.075;
const DEFAULT_SATURATION: f32 = 1.0;
const DEFAULT_PALETTE_MIX: f32 = 0.0;
/// View transform defaults (ADR-0018): identity — `zoom` = 1 unscaled, `pan` = 0
/// unshifted, so an unbound preset is byte-unchanged.
const DEFAULT_ZOOM: f32 = 1.0;
const DEFAULT_PAN: f32 = 0.0;
/// Trail persistence: the fraction of the accumulation retained per 1/60 s frame.
/// ~0.94 gives glowing trails that fade over ~1 s; `fade = 0` clears each frame
/// (trail-free). Applied frame-rate-independently (raised to the `dt`-relative
/// power), so the trail length is the same wall-clock duration on any refresh.
const DEFAULT_FADE: f32 = 0.94;
/// Base point half-size in world units (before the `size` multiplier), matching
/// the swarm's small-glowing-point scale.
const POINT_BASE: f32 = 0.006;
/// `reseed` rises past this to disturb the cloud once (edge-triggered, so a
/// sustained beat flag doesn't disturb it every frame).
const RESEED_THRESHOLD: f32 = 0.5;

/// Fraction of a family's own seed-box spread that one `reseed` kick spans
/// (ADR-0066). See [`AttractorFamily::jitter_extent`] for why this is one
/// constant rather than a per-family number, and why its value is provisional.
const JITTER_FRACTION: f32 = 0.06;

/// The compute shader's family selector value meaning **jitter, do not step**.
/// One past the real families, so adding a family is still a matter of extending
/// [`AttractorFamily`] and its `shader_id`.
///
/// Moved from 4 to 5 by Plan 0062's IFS family, which is what this comment
/// anticipated a fifth family would do.
const JITTER_MODE: u32 = 5;

/// Per-particle weight of the additive deposit, so the **total** light laid into
/// the accumulation each frame is invariant to the particle count (ADR-0065).
///
/// The draw blends `One, One` into a linear accumulation and everything
/// downstream to the tonemap is linear, so without this the figure moves up by
/// exactly the count ratio: `attractor_particles` is 50 000 at `Floor` and
/// 150 000 at `Rich`, and `Rich` therefore rendered the same preset **three stops
/// hot**. ADR-0045 and `presets/README.md` both promise a tier changes capacity
/// and not behavior; for an accumulating additive scene that was false, because
/// capacity *is* the picture.
///
/// So a tier now buys what a capacity tier should buy — the same figure sampled
/// three times as densely at a third the weight each, i.e. **less shot noise in
/// the same picture** rather than more light.
///
/// At `Floor` the factor is exactly `1.0` by construction, which is why no golden
/// baseline moves and why that is assertable on the value rather than inferred
/// from pixels. A future tier with a count *below* `Floor` would put this above
/// `1.0` and amplify shot noise instead of reducing it — bounded and predictable,
/// but worth knowing before a third tier is added.
pub fn deposit_scale(active_count: u32) -> f32 {
    // `max(1)` rather than a branch: a zero-particle scene draws nothing, so the
    // value is unobservable, and a division by zero here would reach the shader.
    crate::render::TierConfig::FLOOR.attractor_particles as f32 / active_count.max(1) as f32
}

/// The sanitized `brightness` multiplier on that deposit (ADR-0080).
///
/// Two guards, both for the same reason — the value arrives from an eased
/// expression and lands in an **accumulation** the trail carries across frames,
/// so a bad frame's value is not a bad frame, it is a permanently poisoned field:
///
/// - **Negative is floored to zero.** The draw blends `One, One`, so negative
///   light would *subtract* from whatever the trail already holds — the same trap
///   `depth_fade`'s clamp exists for.
/// - **Non-finite falls back to the default.** An infinite deposit writes `inf`
///   into the field and every later decay multiplies it back to `inf`; nothing
///   downstream recovers.
///
/// At the default this returns exactly `1.0`, so the multiply at the packing site
/// is the identity.
fn brightness_factor(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        DEFAULT_BRIGHTNESS
    }
}

/// Whether a **reseed's** kick is drawn as a streak (ADR-0069).
///
/// **Provisional, and Plan 0059 Phase 4 decides it — not this phase.** A jitter
/// displaces a particle by far more than a step does (ADR-0069 measures roughly
/// 15x a frame's travel), so drawing the segment renders a long stroke along a
/// path the particle never traversed: arguably a legitimate "whip" on the beat,
/// arguably a bright artifact laid over the figure. It cannot be settled by
/// argument, only by watching a beat land.
///
/// Shipped `false` — the kick moves the particle and the *next* step's segment is
/// the first one drawn. That is the conservative default: it is what the scene
/// did before segments existed, so nothing about the reseed's look changes in
/// this phase.
///
/// Flipping it is a one-constant edit and no shader change: it rides the jitter
/// dispatch's otherwise-unused `coeffs.w`, so an A/B for the content pass is a
/// rebuild rather than a shader rewrite.
const RESEED_DRAWS_STREAK: bool = false;

/// Encode a streak choice into the `f32` slot a uniform carries it in.
///
/// Trivial, and named anyway: both the draw uniform's `x.w` and the jitter
/// dispatch's `coeffs.w` mean "non-zero draws a segment", and a shader comparing
/// `!= 0.0` will read *any* stray value as yes. One helper is what keeps the two
/// call sites agreeing, and gives the encoding somewhere to be tested.
fn streak_flag(on: bool) -> f32 {
    if on { 1.0 } else { 0.0 }
}

/// The smallest `[particles] density` a preset may ask for (ADR-0069).
///
/// At this fraction the scene draws **25** particles at `Floor` (50 000) and
/// **75** at `Rich` (150 000) — and that is deliberately far sparser than it
/// first looks like it should be, because **the sparse end is the point of the
/// key rather than its degenerate edge**.
///
/// **This value is set from rendered captures, and the first arithmetic argument
/// for it was wrong.** The reasoning that picked `0.01` (500 particles) ran:
/// [ADR-0065] holds total light invariant by weighting each particle
/// `50 000 / active`, so a hundredth of the budget already concentrates a hundred
/// particles' worth of light into every point, and an order of magnitude below
/// that must clip to white before it reads as a curve. Rendered at `fade = 0.95`,
/// it does not. The banding first appears around `0.01`, and at `0.002` (100
/// particles) and `0.0005` (25) the Lorenz lobes resolve into visibly *cleaner*
/// spiral traces. The prediction missed that the trail spreads each particle's
/// deposit along its whole path rather than piling it on one texel, so
/// concentrating light into fewer particles buys contrast against the background
/// instead of clipping.
///
/// So the floor is not protecting a look — it is rejecting a mis-typed magnitude.
/// `0.0005` is the sparsest fraction that has actually been captured rendering
/// as the attractor; below it a preset is asking for single-digit trajectories,
/// which is a few orbits rather than a figure. `active_particles` separately
/// guarantees at least one particle, so nothing here can produce an empty draw.
///
/// [ADR-0065]: ../../../../docs/adrs/0065-the-attractor-deposit-is-normalized-by-particle-count.md
pub const MIN_PARTICLE_DENSITY: f32 = 0.0005;

/// Resolve a validated `density` against a tier's particle budget.
///
/// Rounds rather than truncates, and floors at one particle: `density` is already
/// range-checked at load, so this cannot be handed a zero, but a scene that drew
/// zero instances would silently render nothing rather than fail.
fn active_particles(budget: u32, density: f32) -> u32 {
    ((budget as f32 * density).round() as u32).clamp(1, budget)
}

/// The CPU transcription of [`DRAW_SHADER`]'s depth projection (ADR-0076).
///
/// **The WGSL above is the source and this is the mirror** — the same discipline
/// `apply_saturation` follows against `palette.rs::desaturate`, and `project()`
/// up there names this module outright. If you edit one, edit the other.
///
/// It exists because the property this whole change rests on is *dimensionless
/// algebra*, not a picture: under orthography the projection at rotation `π` is
/// the exact `x`-mirror of the projection at `0`, and under perspective it is
/// not, because `m(h) ≠ m(−h)` for any `h ≠ 0`. That holds on every machine,
/// every adapter and every resolution. A capture-level check could only say the
/// picture changed; this says *what* changed and why it matters.
///
/// Test-only: nothing on the render path projects on the CPU, and the point of
/// the module is to be the thing the assertions run.
#[cfg(test)]
mod projection_mirror;

/// One particle, GPU storage-buffer layout (std430). **48 bytes**: the current
/// 3D attractor position (2D families keep `z = 0`), a per-particle seed jitter
/// set once at init, the position this particle held *before* the current step
/// (which the continuous families draw a segment back to, ADR-0069), and the two
/// channels ADR-0087 added — how old the particle is and which map last moved it.
///
/// Each of the first two `f32`s packs into the preceding `vec3`'s trailing slot
/// (offsets 12 and 28), so std430 lays the first 32 bytes out as two tight
/// 16-byte halves. **It used to be a tight 16**, then a tight 32, and that note
/// is kept rather than deleted because the packing argument is the same one
/// applied twice; `_pad` is the explicit name for the slot `seed` occupies in
/// the first half.
///
/// **Why 48 and not 40.** WGSL rounds a struct to a multiple of its alignment
/// and `vec3<f32>` aligns to 16, so `age` and `map` land at offsets 32 and 36
/// and the whole rounds to 48. The two words that follow are **not slack to be
/// reclaimed** — they are the budget for the next per-particle channel, which is
/// why they are named rather than left implicit (and why `bytemuck::Pod`, which
/// forbids implicit padding, agrees).
///
/// The price is one more 16 bytes per particle: at the tier budgets **2.4 MB**
/// at `Floor` (50 000) and **7.2 MB** at `Rich` (150 000), up from 1.6 and 4.8.
/// Paid by all five families including the four that never read the new fields
/// (ADR-0087 Consequences). It is GPU storage, allocated once at build and never
/// resized — `[particles] density` narrows what is *drawn*, not what is allocated
/// (ADR-0069), so a sparse preset pays the full figure.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Particle {
    pos: [f32; 3],
    seed: f32,
    prev: [f32; 3],
    _pad: f32,
    /// Steps since this particle last respawned (ADR-0087). Written by the IFS
    /// arm of the step shader; left at its seeded `0.0` by every other family.
    age: f32,
    /// Index of the map applied on the **most recent** step — a property of
    /// *position*, not of history: it names which sub-copy `fₖ(A)` the particle
    /// currently sits in (the fern's stem, body, left frond, right frond), which
    /// is what makes a one-value channel refreshed every step partition the
    /// figure into its parts rather than read as noise (ADR-0087).
    ///
    /// `0.0` on every non-IFS family, where nothing writes it.
    map: f32,
    /// Distance from this point to the nearest of the **drawn** maps' fixed
    /// points, normalised by the skeleton's own diameter (ADR-0088). **Offset
    /// 40**, which [`PARTICLE_ATTRIBUTES`] spells out by hand.
    ///
    /// A pure function of *position*, recomputed every step rather than
    /// accumulated — which is the whole difference from [`age`](Self::age), and
    /// the reason this gradient is permanent where that one decayed: an old
    /// particle near a fixed point reads the same near-zero a fresh one does.
    ///
    /// **May exceed `1`, and that is not an error.** The fixed-point set's
    /// diameter is not an upper bound on how far the attractor reaches, so the
    /// stored value is a faithful measurement and the *draw* clamps.
    ///
    /// `0.0` on every non-IFS family, where nothing writes it.
    root: f32,
    /// The **last** spare word — the next per-particle channel after this one is
    /// a struct change to a type four families share (ADR-0088 Consequences).
    /// Explicit so it reads as budget rather than as slack.
    _spare: f32,
}

/// Mean particle lifetime, in fixed steps (ADR-0087) — 3 s at [`FIXED_STEP`].
///
/// **A look constant with no principled value**, in the same position ADR-0075's
/// `0.97` occupies, and the acceptance is the look rather than the number. At the
/// 150 000-particle ceiling this restarts ~0.56 % of the buffer per step. Plan
/// 0073 Phase 6 judges it live; if the churn reads as *twinkle*, **this constant
/// and [`DEFAULT_EMERGENCE`] are the lever, and making the rate bindable is not**.
const CHURN_LIFETIME: f32 = 180.0;

/// The per-particle spread on [`CHURN_LIFETIME`], as multipliers of it.
///
/// **The spread is what makes the churn continuous rather than a pulse.** With
/// one shared lifetime every particle seeded together would restart together —
/// a bulk respawn, which is the artifact this whole plan exists to remove, just
/// on a 3-second period. Drawn from the particle's own fixed `seed`, so the
/// phases stay spread for the life of the session and the restart rate is flat.
const CHURN_LIFETIME_SPREAD: [f32; 2] = [0.5, 1.5];

/// Steps a respawned particle takes to reach full brightness (ADR-0087) — the
/// default the `emergence` param falls back to.
///
/// **Load-bearing rather than polish.** A fixed rate at the particle ceiling
/// lands on the order of a thousand particles per frame onto exactly **four
/// points**, and the trail field integrates that into four bright dots. Ramping
/// from zero means those points deposit almost nothing, and by the time a
/// particle is bright it has been iterated enough to have spread across the
/// figure. Without it the churn is four blobs; with it the churn is invisible,
/// which is the whole intent.
///
/// Bindable since Plan 0074 Phase 4, and **for ADR-0087's reason rather than the
/// colour one**: a ramp sufficient at `fade = 0.86` may not be at `0.94`,
/// because a longer trail integrates the four restart points over more frames.
/// The *other* motivation for exposing it — letting the age gradient show — died
/// with the age channel and is not why this shipped.
const DEFAULT_EMERGENCE: f32 = 8.0;

/// The shortest ramp that is still a ramp.
///
/// **The guard here is arithmetic, not taste.** `em.x` is `1 / emergence`, so a
/// zero binding divides by zero and a negative one *inverts* the ramp — a
/// just-respawned particle would start at full brightness and dim, which is the
/// four-blob artifact the ramp exists to remove, brought back through the front
/// door. Below `1` nothing further changes: a particle's `age` is a whole number
/// of steps, so a rate at or above `1.0` already means the first step after a
/// respawn is fully bright.
const MIN_EMERGENCE: f32 = 1.0;

/// The per-step brightness increment the draw uniform carries, from a bound
/// `emergence` in **steps**.
///
/// Clamped here rather than in the shader for the reason `perspective` is
/// (ADR-0076): this is the one place the value crosses into the GPU, so a preset
/// asking for something the maths does not accept gets the floor rather than a
/// divisor approaching zero. **A smoothing curve makes this necessary rather
/// than defensive** — an eased param is continuous even when its own maths is
/// not, so a binding easing from `8` toward `0` sweeps *through* the invalid
/// range whatever its endpoints are.
///
/// **There is deliberately no upper clamp.** Past the longest lifetime a
/// particle can draw, no particle completes the ramp and the figure simply gets
/// dimmer — that is a look an author can ask for, not an arithmetic hazard, and
/// capping it would be taste. Non-finite is the one case a clamp cannot handle:
/// `f32::clamp` propagates `NaN`, which would reach the division and black the
/// figure out, so it falls back to the default instead.
fn emergence_rate(steps: f32) -> f32 {
    if !steps.is_finite() {
        return 1.0 / DEFAULT_EMERGENCE;
    }
    1.0 / steps.max(MIN_EMERGENCE)
}

/// This particle's lifetime in steps, from its own fixed `seed`.
///
/// **The CPU mirror of `ifs_lifetime` in [`STEP_SHADER`]**, and the two must
/// agree exactly or a particle's seeded age would not sit inside the life the
/// GPU measures it against. `the_churn_constants_agree_between_rust_and_wgsl`
/// holds the shader's literals to these constants;
/// [`hash_unit`] is the transcription of the hash itself.
fn churn_lifetime(seed: f32) -> f32 {
    let [lo, hi] = CHURN_LIFETIME_SPREAD;
    CHURN_LIFETIME * (lo + hash_unit(seed.to_bits() ^ LIFETIME_SALT) * (hi - lo))
}

// `churn_max_lifetime()` lived here until Plan 0074 Phase 3. It was the longest
// lifetime any particle could draw, and the ONLY thing that read it was the age
// colour channel's normalizer (`em.z`). Retiring `age_tint`/`age_hue` left it
// with no caller, so it went with them rather than sitting as dead code that
// reads like a live invariant.

/// Salt separating the lifetime draw from every other hash on the particle's
/// seed — the map choice, the reseed kick, and the respawn slot each use their
/// own, so two of them cannot correlate.
const LIFETIME_SALT: u32 = 0x9E37_79B1;

/// **The CPU mirror of `mix32` + `unit01` in [`STEP_SHADER`]** — one round of the
/// lowbias32 bit-mixer, then the top 24 bits as a fraction in `[0, 1)`.
///
/// The same discipline `projection_mirror` follows: **the WGSL is the source and
/// this is the mirror**. It exists because `seed()` has to place a particle at a
/// point in a life the *shader* computes, and a CPU that disagreed about the
/// lifetime would seed ages outside it.
fn hash_unit(v: u32) -> f32 {
    let mut h = v;
    h ^= h >> 16;
    h = h.wrapping_mul(0x7FEB_352D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846C_A68B);
    h ^= h >> 16;
    (h >> 8) as f32 / 16_777_216.0
}

/// GPU compute-particle strange-attractor scene (ADR-0015). A storage buffer of
/// particles is stepped through the De Jong map by a compute shader each frame,
/// drawn as additive point-sprites into a fading trail field, and composited to
/// the surface. Every knob is an ADR-0002 layer-2 named parameter — the attractor
/// coefficients (`a`,`b`,`c`,`d`), the look scalars (`size`,`hue`,`fade`), and a
/// beat-driven `reseed` — so a preset binds them to the audio bands and beat.
pub struct AttractorScene {
    /// Cloned device handle (an `Arc` inside wgpu) used to build [`Resources`]
    /// lazily on first render — see the module docs for why.
    device: wgpu::Device,
    surface_format: wgpu::TextureFormat,
    res: Option<Resources>,
    /// The accumulation grid the next build should use — the render target's size
    /// through [`trail_grid_size`], updated by
    /// [`Scene::set_target_size`](crate::render::scenes::Scene::set_target_size).
    /// Held separately from [`FieldResources::trail_w`]/`trail_h` so a size change
    /// is a compare here and a field re-allocation on the next render, never GPU
    /// work inside the hook (ADR-0030 condition 2).
    trail_w: u32,
    trail_h: u32,
    /// The active tier's cap on the trail grid
    /// ([`TierConfig::attractor_trail_cap`](crate::render::TierConfig::attractor_trail_cap)),
    /// resolved once at construction. Read only in `set_target_size`, so the grid
    /// stays a pure function of the target and the cap.
    trail_cap: (u32, u32),
    /// The active tier's particle count
    /// ([`TierConfig::attractor_particles`](crate::render::TierConfig::attractor_particles)).
    /// Fixed for the life of the scene: the storage buffer and the seeded scatter
    /// are both sized to it, and a tier change rebuilds the scene.
    particle_count: u32,
    /// How many of [`particle_count`](Self::particle_count) are actually stepped
    /// and drawn — `round(particle_count * density)` (ADR-0069).
    ///
    /// **Nothing is reallocated when this moves.** The storage buffer, the seeded
    /// scatter and every bind group stay sized to `particle_count`; this only
    /// narrows the dispatch, the draw's instance count, and the `count` the step
    /// shader early-returns against. Particles beyond it keep their seeded
    /// positions untouched for the life of the preset — asserted directly, since
    /// "rebuilds nothing" is otherwise a claim about code that is easy to break.
    active_count: u32,
    /// Fraction of the tier budget the loaded preset asked for, from
    /// `[particles] density`. Structural: set in `configure`, never per frame.
    density: f32,
    /// The deterministic seeded scatter, uploaded on the first frame after a
    /// (re)build so a rebuilt scene restarts identically (capture determinism).
    seed_particles: Vec<Particle>,
    /// Re-upload the seed scatter next render. Set on first build and on a family
    /// change — the two places there is no existing cloud to disturb. A `reseed`
    /// no longer sets it (ADR-0066); see [`Self::pending_jitter`].
    needs_upload: bool,
    /// A `reseed` rising edge is pending: the next render encodes **one** jitter
    /// dispatch before its steps, kicking each particle where it already is.
    /// A bool rather than a count, because two reseeds inside one frame are one
    /// disturbance — the parameter is edge-triggered, not integrated.
    pending_jitter: bool,
    /// How many reseeds have fired. Salts the jitter hash so successive reseeds
    /// kick a particle in different directions, and advances only on the edge —
    /// so it is a function of the input sequence and the cloud stays reproducible.
    reseed_count: u32,
    /// Clear the trail field to black next render. Set only on first build (not on
    /// reseed, so a beat's disturbance blooms over the existing trails).
    needs_clear: bool,
    /// Fixed-timestep accumulator: unspent injected `dt`, drained one
    /// [`FIXED_STEP`] at a time into compute steps.
    fixed_step: gpu::FixedStep,
    /// Steps `advance` scheduled for the next `render` to encode.
    pending_steps: u32,
    /// The index of the **next** fixed step to run — the IFS's map-choice salt
    /// (ADR-0075), advanced by the number of steps actually encoded.
    ///
    /// Determinism is preserved exactly: it is a pure function of the injected
    /// `dt` sequence, which captures pin at 1/60 s, and it starts at zero on
    /// every rebuild. It wraps rather than saturating — at 60 steps per second a
    /// `u32` takes 2.3 years to go round, and the value's only job is to
    /// decorrelate successive draws.
    step_index: u32,
    /// Real elapsed seconds for this frame, injected via `advance`, used to make
    /// the trail decay frame-rate-independent.
    dt: f32,
    /// The integrated spin, in **spin-scaled seconds** — advanced once per frame
    /// in [`update`](Scene::update), where this frame's `spin` is already
    /// resolved, and turned into radians by [`spin_phase`] at the uniform.
    ///
    /// **This scene no longer reads the shared clock at all**, which is why
    /// `set_time` is gone: the display rotation was the only thing that used it,
    /// and a rate multiplier has to be integrated rather than multiplied against
    /// elapsed time (see [`advance_spin`]). Determinism is unaffected — the phase
    /// is a pure function of the injected `dt` sequence, which captures pin at
    /// 1/60 s, and it starts at zero on every rebuild.
    spin_time: f32,
    /// The active attractor map, selected data-driven via `[particles]`
    /// (ADR-0007 `configure`); its default coefficients seed `a`..`d`.
    family: AttractorFamily,
    /// The two ends of the IFS morph, **decomposed once at `configure`**
    /// (ADR-0075). `None` on the four map families.
    ///
    /// Cached rather than derived per frame because the decomposition is the
    /// expensive half — four hypotenuses and four `atan2`s per map — and it is a
    /// function of the figure pair alone. What the frame pays is the lerp and
    /// the recompose, which is what has to be per-frame because `morph` is
    /// bindable. When `[particles] morph_to` is absent both ends are the same
    /// table, so `morph` is exactly inert rather than conditionally applied.
    ifs_ends: Option<(IfsTable, IfsTable)>,
    /// The figure pair's framing over `morph`, measured once at `configure`
    /// with every lever at neutral (ADR-0075). `None` on the four map families,
    /// which keep their single hand-fitted world scale.
    ifs_fit: Option<FitLut>,
    /// Position along the morph from the configured figure to `morph_to`
    /// (ADR-0075). Bindable; clamped to `[0, 1]` inside
    /// [`ifs::resolve`](ifs::resolve), where extrapolation would be the one
    /// operation that can leave the contractive ball.
    morph: f32,
    /// The four IFS shape levers (ADR-0075), applied in SVD space by
    /// [`ifs::resolve`]. Inert on the four map families, which have no table for
    /// them to act on.
    ///
    /// Held as the lever struct rather than four loose floats so the one thing
    /// that must not happen — a lever reaching [`FitLut::build`] — is a type
    /// error rather than a discipline.
    levers: Levers,
    /// Attractor coefficients — named params, so a preset can steer the cloud's
    /// shape with the bands. Their meaning is family-specific.
    a: f32,
    b: f32,
    c: f32,
    d: f32,
    size: f32,
    hue: f32,
    /// Scene-local level (ADR-0080): a multiplier on the per-particle additive
    /// deposit, which [`deposit_scale`] has already normalized by the particle
    /// count. So it composes with `density` instead of fighting it, and — being a
    /// property of the pixels this scene lays down — it blends across a dissolve
    /// the way the picture does, which the engine-wide `exposure` presets used to
    /// reach for does not.
    brightness: f32,
    fade: f32,
    /// Shared palette color knobs (ADR-0021 / Plan 0020 Phase 5): the per-particle
    /// seed jitter band + shared desaturation + A/B crossfade.
    hue_spread: f32,
    hue_center: f32,
    saturation: f32,
    palette_mix: f32,
    /// Shared view transform (ADR-0018 / Plan 0025 Phase 4): `zoom` scales the
    /// projected cloud about the screen centre, `pan_*` offsets it.
    zoom: f32,
    pan_x: f32,
    pan_y: f32,
    /// Perspective strength (ADR-0076): the figure's depth half-extent as a
    /// fraction of the camera distance, clamped to [`MAX_PERSPECTIVE`] where the
    /// uniform is packed. `0` is the orthographic projection this scene shipped
    /// with, and it is inert on the 2D families whatever it is set to.
    perspective: f32,
    /// Atmospheric depth cues (ADR-0076), the substitute for occlusion:
    /// `depth_fade` attenuates a particle's brightness with distance (clamped to
    /// `[0, 1]` where the uniform is packed — past `1` the multiplier would go
    /// negative and *subtract* light from the additive accumulation), and
    /// `depth_hue` shifts its palette coordinate by `±depth_hue/2` across the
    /// depth range. Both inert on the 2D families, like `perspective`.
    depth_fade: f32,
    depth_hue: f32,
    /// The last-map colour channel's two routes (ADR-0087), **IFS-only** — every
    /// other family leaves `Particle::map` at `0.0`, so both are exactly inert
    /// there without a branch, the way `perspective` is inert on a 2D family.
    ///
    /// `map_tint` shifts the particle's palette coordinate by `±map_tint/2`
    /// across the four maps, so the colour comes from the preset's own
    /// `[palette]` ramp and `palette_mix`/`saturation` reach it for free.
    /// `map_hue` instead rotates the hue of the colour that ramp produced,
    /// leaving the coordinate alone — the route for a preset that wants its
    /// fronds nudged off its body without editing its gradient.
    map_tint: f32,
    map_hue: f32,
    /// The root channel's two routes (ADR-0088), IFS-only for the same structural
    /// reason the two above are: only the IFS arm writes [`Particle::root`].
    ///
    /// `root_tint` shifts the palette coordinate by `root_tint · root01` across
    /// the figure's own skeleton — dark at the stem base and the frond origins,
    /// bright at the tips. `root_hue` rotates the hue of the colour that ramp
    /// produced instead, leaving the coordinate untouched.
    ///
    /// **Both anchored at `0`, not centred like `map_*`** (ADR-0088's Anchoring
    /// section): the fixed points keep the preset's chosen colour exactly and
    /// the figure ramps away from them. Centring assumes the channel spans
    /// `[0, 1]`, and this one does not — its ceiling is a property of each
    /// figure's own invariant measure, from `0.41` on the spiral to `1.05` on
    /// the dragon, so the **same binding is not the same look across figures**.
    ///
    /// **`root_hue` is the escape from a full palette coordinate.** Three params
    /// write that coordinate and it is a fixed budget — Plan 0074's gate measured
    /// `attractor_fern` needing `map_tint` cut `0.46 -> 0.22` before `root_tint`
    /// improved on stock. This route costs it nothing.
    ///
    /// These replaced `age_tint`/`age_hue`, which read the decaying age proxy and
    /// never produced a gradient.
    root_tint: f32,
    root_hue: f32,
    /// Length of the emergence ramp in **steps** - how long a just-respawned
    /// particle takes to reach full brightness (ADR-0087), IFS-only because
    /// nothing else respawns.
    ///
    /// At [`FIXED_STEP`] the default 8 steps is ~0.13 s. Raise it when a longer
    /// `fade` lets the four restart points accumulate: a trail integrates them
    /// over more frames, so a ramp sufficient at `fade = 0.86` may not be at
    /// `0.94`. Clamped silently at the pack site by [`emergence_rate`].
    emergence: f32,
    /// Rate multiplier on [`SPIN_RATE`] (ADR-0076). Unlike the depth cues this is
    /// **not** inert on the 2D families: the discrete maps rotate in-plane
    /// through the same angle, so `spin` reaches all four families where
    /// `perspective`, `depth_fade` and `depth_hue` reach two. That asymmetry is
    /// deliberate — an in-plane spin is a real look on De Jong today.
    spin: f32,
    /// The active baked palette pair; uploaded to the draw LUT textures when
    /// `palette_dirty` (a preset switch or a resource rebuild), off the hot path.
    palette: Palette,
    palette_dirty: bool,
    /// This frame's `reseed` level (bound to a beat/onset expression); its rising
    /// edge disturbs the cloud in place (ADR-0066).
    reseed: f32,
    /// Previous frame's `reseed`, for rising-edge detection.
    prev_reseed: f32,
}

impl AttractorScene {
    /// Build the CPU-side seeded scatter. GPU resources are deferred to the first
    /// render (module docs).
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        particle_count: u32,
        trail_cap: (u32, u32),
    ) -> Self {
        let family = AttractorFamily::DeJong;
        let seed_particles = Self::seed(family, particle_count);
        let [a, b, c, d] = family.default_coeffs();
        Self {
            device: device.clone(),
            surface_format,
            res: None,
            trail_cap,
            particle_count,
            // The whole budget until a `[particles] density` says otherwise, so a
            // preset that never mentions the key is byte-identical to before it.
            active_count: particle_count,
            density: 1.0,
            trail_w: TRAIL_FALLBACK_W,
            trail_h: TRAIL_FALLBACK_H,
            seed_particles,
            needs_upload: true,
            pending_jitter: false,
            reseed_count: 0,
            needs_clear: true,
            fixed_step: gpu::FixedStep::new(FIXED_STEP, MAX_SUBSTEPS),
            pending_steps: 0,
            step_index: 0,
            dt: FIXED_STEP,
            spin_time: 0.0,
            family,
            ifs_ends: None,
            ifs_fit: None,
            morph: DEFAULT_MORPH,
            levers: Levers::NEUTRAL,
            a,
            b,
            c,
            d,
            size: DEFAULT_SIZE,
            hue: DEFAULT_HUE,
            brightness: DEFAULT_BRIGHTNESS,
            fade: DEFAULT_FADE,
            hue_spread: DEFAULT_HUE_SPREAD,
            hue_center: DEFAULT_HUE_CENTER,
            saturation: DEFAULT_SATURATION,
            palette_mix: DEFAULT_PALETTE_MIX,
            zoom: DEFAULT_ZOOM,
            pan_x: DEFAULT_PAN,
            pan_y: DEFAULT_PAN,
            perspective: DEFAULT_PERSPECTIVE,
            depth_fade: DEFAULT_DEPTH_FADE,
            depth_hue: DEFAULT_DEPTH_HUE,
            map_tint: DEFAULT_CHANNEL_COLOUR,
            map_hue: DEFAULT_CHANNEL_COLOUR,
            root_tint: DEFAULT_CHANNEL_COLOUR,
            root_hue: DEFAULT_CHANNEL_COLOUR,
            emergence: DEFAULT_EMERGENCE,
            spin: DEFAULT_SPIN,
            palette: Palette::default_spectrum(),
            palette_dirty: true,
            reseed: 0.0,
            prev_reseed: 0.0,
        }
    }

    /// The deterministic initial particle set: a seeded scatter in a small box,
    /// each with a per-particle hue jitter. Points converge onto the attractor
    /// within a few iterations, so the starting positions only need to differ.
    ///
    /// The `x`/`y`/`seed` draws come first (their order matches the earlier 2D
    /// scatter, so De Jong/Clifford stay byte-identical across the 3D upgrade),
    /// then `z` is drawn in a second pass for the 3D families.
    #[allow(
        clippy::indexing_slicing,
        reason = "spread/centre/pos index fixed [f32; 3] at constant offsets, always in-bounds"
    )]
    /// Build the GPU resources on the first frame, and re-allocate the
    /// grid-dependent half when `set_target_size` asked for a different grid than
    /// the live one (Plan 0027 Phase 2). In the steady state — every frame of a
    /// static window — this is the two integer compares below and nothing else
    /// (ADR-0030 condition 2).
    ///
    /// A grid change rebuilds **only** `FieldResources` (Plan 0029 Phase 1): the
    /// shaders, pipelines, particle buffer and LUT textures do not depend on the
    /// grid and survive, so a resize costs a texture pair plus four bind groups
    /// instead of four shader compilations. The rebuilt field is undefined, so the
    /// **clear** is re-flagged — a resize restarts the trail rather than carrying a
    /// differently-sized accumulation across. The palette is not re-flagged: the
    /// LUT textures survive.
    ///
    /// Neither is the particle buffer (Plan 0031 Phase 4, closing Plan 0029's
    /// close-review minor 1). It survives the split, so re-uploading the seed
    /// scatter on a grid change is no longer *necessary* — and it was the surviving
    /// half of "a fullscreen toggle pops the cloud back to its seed scatter": the
    /// points kept iterating across a resize, then jumped back. Determinism does
    /// not need it either, since a headless capture holds one target size for its
    /// whole run.
    fn rebuild_if_stale(&mut self) {
        let grid_stale = self
            .res
            .as_ref()
            .is_none_or(|res| res.grid.trail_w != self.trail_w || res.grid.trail_h != self.trail_h);
        if !grid_stale {
            return;
        }
        let res = match self.res.take() {
            Some(mut res) => {
                res.rebuild_grid(&self.device, self.trail_w, self.trail_h);
                res
            }
            None => {
                // First build: the LUT textures are fresh, so the palette needs its
                // one upload, and the particle buffer has never been written — this
                // is the arm the seed upload belongs to.
                self.palette_dirty = true;
                self.needs_upload = true;
                Resources::build(
                    &self.device,
                    self.surface_format,
                    self.trail_w,
                    self.trail_h,
                    self.particle_count,
                )
            }
        };
        self.res = Some(res);
        self.needs_clear = true;
    }

    /// Read every live particle position back off the GPU.
    ///
    /// **A test instrument** (Plan 0057 Phase 3), not a render path: it blocks on a
    /// buffer map, which the frame loop must never do. It exists because the
    /// property ADR-0066 changes is a property of *the cloud*, and a pixel
    /// differential cannot state it — "the reseed no longer puts particles outside
    /// the attractor's extent" is a claim about positions, and a frame diff would
    /// only say the picture moved, which the wipe also did.
    ///
    /// `None` before the first render, when there are no GPU resources yet.
    ///
    /// Returns whole [`Particle`]s rather than the `(pos, prev)` pair it used to:
    /// ADR-0087's `age` and `map` are the same kind of claim — a property of the
    /// buffer that a capture can only report indirectly — so the readback hands
    /// back the struct and each caller takes the fields its own assertion is
    /// about.
    #[cfg(test)]
    fn read_particles(&self, queue: &wgpu::Queue) -> Option<Vec<Particle>> {
        let res = self.res.as_ref()?;
        let size = (self.particle_count as usize * std::mem::size_of::<Particle>()) as u64;
        let staging = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("attractor-particle-readback"),
            size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("attractor-particle-readback"),
            });
        encoder.copy_buffer_to_buffer(&res.pipelines.particles, 0, &staging, 0, size);
        queue.submit(std::iter::once(encoder.finish()));

        // The same idiom as `capture::read_back`, which is the readback this
        // project already trusts.
        let slice = staging.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |res| {
            let _ = tx.send(res);
        });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("particle readback poll");
        rx.recv()
            .expect("particle readback callback")
            .expect("particle readback map");

        let out = {
            let mapped = slice.get_mapped_range().expect("particle readback range");
            let particles: &[Particle] = bytemuck::cast_slice(&mapped);
            particles.to_vec()
        };
        staging.unmap();
        Some(out)
    }

    /// The CPU-side initial fill.
    ///
    /// **The IFS does not fill a box, and that is where backlog 0064 dies**
    /// (ADR-0087). Every other family scatters uniformly over
    /// [`AttractorFamily::seed_box`] and contracts onto its attractor over the
    /// following second, which showed as a legible, hard-edged, axis-aligned
    /// rectangle for roughly two thirds of a second on every switch into the
    /// family — the same artifact class ADR-0066 removed from `reseed`, back on
    /// a different path. An IFS has somewhere legal to start instead: its maps'
    /// fixed points are **on** the attractor by construction
    /// ([`ifs::fixed_points`]), so its fill is on the figure at step zero and
    /// there is no rectangle to fade out at any frame.
    ///
    /// **`seed_box` is deliberately untouched.**
    /// [`AttractorFamily::jitter_extent`] is derived from it as a fraction of its
    /// spread, so collapsing that spread would make `reseed` silently inert on
    /// the whole family. What changes is what this function *writes*.
    ///
    /// The figure's **base** table, not the resolved one: `configure` runs on a
    /// preset switch, before that preset's `morph` and levers have been routed,
    /// so there is no resolved table to ask. Phase 3's continuous respawn targets
    /// the live resolved table, which is what carries the fill to wherever a
    /// bound `morph` has taken the figure — within one particle lifetime.
    fn seed(family: AttractorFamily, count: u32) -> Vec<Particle> {
        let (spread, center) = family.seed_box();
        let fixed = family.figure().map(|f| ifs::fixed_points(&f.table()));
        let mut rng = SeededRng::new(SEED);
        let mut particles: Vec<Particle> = (0..count)
            .map(|_| {
                let (x, y, seed, age) = match fixed {
                    // Drawn from the particle's own fixed seed, so which point a
                    // given particle starts at is a pure function of the seeded
                    // scatter — the same property every other determinism claim
                    // in this scene rests on.
                    Some(points) => {
                        let seed = rng.next_f32();
                        let slot = ((seed * ifs::MAPS as f32) as usize).min(ifs::MAPS - 1);
                        let [x, y] = points.get(slot).copied().unwrap_or([0.0, 0.0]);
                        // **Age starts spread, and that is not a refinement of
                        // the churn — it is the churn's first frame.** Seeded at
                        // zero, every particle would reach the end of its life
                        // within one lifetime-spread of every other, and the
                        // population would hold a single age for the first
                        // ~1.5 s of every preset. The colour gradient Phase 4
                        // builds on this would be flat for exactly as long.
                        //
                        // Strictly below the particle's own life (`next_f32` is
                        // `[0, 1)`), so nothing respawns on its first step and
                        // there is no bulk restart at startup either.
                        let age = rng.next_f32() * churn_lifetime(seed);
                        (x, y, seed, age)
                    }
                    None => {
                        let x = center[0] + rng.range(-spread[0], spread[0]);
                        let y = center[1] + rng.range(-spread[1], spread[1]);
                        // Nothing ages a map family: no respawn, and the draw's
                        // emergence ramp is a flat 1.0 there.
                        (x, y, rng.next_f32(), 0.0)
                    }
                };
                Particle {
                    pos: [x, y, 0.0],
                    seed,
                    // Seeded equal to `pos`, so a particle that has not stepped
                    // yet spans a zero-length segment and draws as the point it
                    // would have drawn before ADR-0069. A zeroed `prev` would
                    // instead streak every particle from the origin on the first
                    // frame — a starburst that the trail would then keep.
                    prev: [x, y, 0.0],
                    _pad: 0.0,
                    // Staggered on the IFS (see above), flat zero everywhere
                    // else. `map` stays 0.0 forever on the four map families,
                    // which never write it.
                    age,
                    map: 0.0,
                    // Exact, not a placeholder: an IFS seeds every particle AT a
                    // fixed point, so its distance from the nearest one really
                    // is zero, and the first step overwrites it anyway. On a map
                    // family nothing ever writes it (ADR-0088).
                    root: 0.0,
                    _spare: 0.0,
                }
            })
            .collect();
        for p in &mut particles {
            p.pos[2] = center[2] + rng.range(-spread[2], spread[2]);
            p.prev[2] = p.pos[2];
        }
        particles
    }
}

/// Parameter vocabulary — see [`fragment_field::PARAMS`](super::fragment_field::PARAMS).
/// **Keep in sync with `set_param` below.**
pub const PARAMS: &[&str] = &[
    "a",
    "b",
    "c",
    "d",
    "size",
    "hue",
    // The scene-local level (ADR-0080) — spelled exactly as `swarm` and
    // `emitter` spell it.
    "brightness",
    "fade",
    "hue_spread",
    "hue_center",
    "saturation",
    "palette_mix",
    "zoom",
    "pan_x",
    "pan_y",
    "reseed",
    "perspective",
    "depth_fade",
    "depth_hue",
    "spin",
    // IFS-only (ADR-0075). Inert on the four map families, the same way `a`..`d`
    // already carry family-specific meanings.
    "morph",
    "curl",
    "vigor",
    "lean",
    "bias",
    // Also IFS-only (ADR-0087), and for a structural reason rather than a
    // default: `Particle::map` is written by the IFS arm of the step shader and
    // by nothing else, so on the four map families it is identically `0.0` and
    // both of these are exactly the identity.
    "map_tint",
    "map_hue",
    // ...and ADR-0088's root channel, IFS-only for exactly the same structural
    // reason: `Particle::root` is written by the IFS arm and by nothing else.
    // These two took `age_tint`/`age_hue`'s place at Plan 0074 Phase 3 rather
    // than joining them, so the roster did not grow.
    "root_tint",
    "root_hue",
    // The emergence ramp's length in steps (Plan 0074 Phase 4). IFS-only for the
    // structural reason the channels above are: nothing else respawns, so
    // nothing else has a ramp - em is a flat 1.0 on the four map families.
    "emergence",
];

impl Scene for AttractorScene {
    fn name(&self) -> &'static str {
        "attractor"
    }

    fn advance(&mut self, dt: f32) {
        self.dt = dt;
        // Drain the accumulator one fixed step at a time, clamped so a long stall
        // can't queue unbounded compute work (the reaction-diffusion discipline,
        // and now literally the same code). The sub-`FIXED_STEP` remainder
        // carries to the next frame.
        self.pending_steps = self.fixed_step.advance(dt);
    }

    /// Size the trail accumulation grid to the render target, capped and
    /// quantized (Plan 0027 Phase 2, Plan 0029 Phase 2). Called every frame, so
    /// the unchanged case must stay free (ADR-0030 condition 2): this only records
    /// the request — no allocation, no GPU work — and `render` re-allocates the
    /// field when it differs from what the live one was built for.
    fn set_target_size(&mut self, width: u32, height: u32) {
        let (w, h) = trail_grid_size(width, height, self.trail_cap);
        self.trail_w = w;
        self.trail_h = h;
    }

    // No `set_time`. The display rotation was this scene's only reader of the
    // shared clock, and since ADR-0076 it is an integrated phase instead — so
    // the trait's no-op default is the honest implementation.

    fn set_palette(&mut self, palette: &Palette) {
        // Uploaded to the draw LUT textures in `render` (deferred — resources build
        // lazily on first render). Cheap array copy, off the hot path.
        self.palette = palette.clone();
        self.palette_dirty = true;
    }

    fn reset_params(&mut self) {
        // Defaults are the active family's canonical coefficients + the calm look,
        // so an unbound preset (or a param a preset leaves out) falls back here
        // rather than leaking last frame's.
        let [a, b, c, d] = self.family.default_coeffs();
        self.a = a;
        self.b = b;
        self.c = c;
        self.d = d;
        self.size = DEFAULT_SIZE;
        self.hue = DEFAULT_HUE;
        self.brightness = DEFAULT_BRIGHTNESS;
        self.fade = DEFAULT_FADE;
        self.hue_spread = DEFAULT_HUE_SPREAD;
        self.hue_center = DEFAULT_HUE_CENTER;
        self.saturation = DEFAULT_SATURATION;
        self.palette_mix = DEFAULT_PALETTE_MIX;
        self.zoom = DEFAULT_ZOOM;
        self.pan_x = DEFAULT_PAN;
        self.pan_y = DEFAULT_PAN;
        self.perspective = DEFAULT_PERSPECTIVE;
        self.depth_fade = DEFAULT_DEPTH_FADE;
        self.depth_hue = DEFAULT_DEPTH_HUE;
        self.map_tint = DEFAULT_CHANNEL_COLOUR;
        self.map_hue = DEFAULT_CHANNEL_COLOUR;
        self.root_tint = DEFAULT_CHANNEL_COLOUR;
        self.root_hue = DEFAULT_CHANNEL_COLOUR;
        self.emergence = DEFAULT_EMERGENCE;
        self.spin = DEFAULT_SPIN;
        self.morph = DEFAULT_MORPH;
        self.levers = Levers::NEUTRAL;
        self.reseed = 0.0;
    }

    fn set_param(&mut self, name: &str, value: f32) {
        match name {
            "a" => self.a = value,
            "b" => self.b = value,
            "c" => self.c = value,
            "d" => self.d = value,
            "size" => self.size = value,
            "hue" => self.hue = value,
            "brightness" => self.brightness = value,
            "fade" => self.fade = value,
            "hue_spread" => self.hue_spread = value,
            "hue_center" => self.hue_center = value,
            "saturation" => self.saturation = value,
            "palette_mix" => self.palette_mix = value,
            "zoom" => self.zoom = value,
            "pan_x" => self.pan_x = value,
            "pan_y" => self.pan_y = value,
            "perspective" => self.perspective = value,
            "depth_fade" => self.depth_fade = value,
            "depth_hue" => self.depth_hue = value,
            "map_tint" => self.map_tint = value,
            "map_hue" => self.map_hue = value,
            "root_tint" => self.root_tint = value,
            "root_hue" => self.root_hue = value,
            "emergence" => self.emergence = value,
            "spin" => self.spin = value,
            "morph" => self.morph = value,
            "curl" => self.levers.curl = value,
            "vigor" => self.levers.vigor = value,
            "lean" => self.levers.lean = value,
            "bias" => self.levers.bias = value,
            "reseed" => self.reseed = value,
            _ => {}
        }
    }

    fn update(&mut self, _frame: &AnalysisFrame) {
        // Integrate the spin. **Here and not in `advance`**: the renderer calls
        // `advance` before it routes this frame's bindings, so `self.spin` is
        // last frame's value there and this frame's here. `self.dt` is the real
        // elapsed seconds `advance` recorded, so the phase stays a pure function
        // of the injected `dt` sequence.
        self.spin_time = advance_spin(self.spin_time, self.spin, self.dt);

        // Rising-edge detect on `reseed` (a beat/onset expression): **disturb** the
        // cloud once, where it is (ADR-0066). Edge-triggered so a sustained flag
        // doesn't disturb it every frame; deterministic because the kick is a pure
        // function of each particle's fixed seed and the reseed counter. The trail
        // field is kept, so the disturbance blooms through the trails.
        //
        // This used to re-upload the seed scatter, which did not scatter the cloud
        // — it *replaced* it, with a uniform fill of an axis-aligned box that then
        // took a visible number of iterations to converge back onto the attractor.
        // Every shipped preset header describes reseed as a percussive accent; the
        // wipe is what it actually was.
        if self.reseed >= RESEED_THRESHOLD && self.prev_reseed < RESEED_THRESHOLD {
            self.pending_jitter = true;
            self.reseed_count = self.reseed_count.wrapping_add(1);
        }
        self.prev_reseed = self.reseed;
    }

    /// Select the attractor family from the preset's `[particles]` table (ADR-0007
    /// `configure`, off the hot path). Reuses the shared [`GeneratorConfig`] enum
    /// rather than a new trait method. A family change re-seeds and clears the
    /// trail so the new attractor forms cleanly rather than iterating the old
    /// family's points. Never truncates, so it never reports a [`CapOverflow`].
    fn configure(
        &mut self,
        cfg: &super::lines::GeneratorConfig,
    ) -> Option<super::lines::CapOverflow> {
        if let super::lines::GeneratorConfig::Particles {
            family,
            density,
            morph_to,
        } = cfg
        {
            // `density` is resolved unconditionally, not behind the family guard:
            // two presets can share a family and differ only in how much of it
            // they draw, and `configure` runs on every preset switch.
            self.density = *density;
            self.active_count = active_particles(self.particle_count, *density);
            // Likewise the morph ends: two presets can share a figure and morph
            // it towards different partners. **Decomposed here**, off the hot
            // path, so a frame pays only the lerp and the recompose.
            //
            // An absent `morph_to` gives both ends the same table rather than a
            // `None` the render path has to branch on — so `morph` resolves to
            // the identity by arithmetic instead of by a special case.
            self.ifs_ends = family.figure().map(|figure| {
                let start = figure.table();
                let end = morph_to.unwrap_or(figure).table();
                (start, end)
            });
            // ...and the framing that follows the morph, measured here for the
            // same reason: it is a function of the figure pair alone, so a frame
            // pays one lerp rather than 33 chaos games.
            self.ifs_fit = self
                .ifs_ends
                .as_ref()
                .map(|(start, end)| FitLut::build(start, end));
            if *family != self.family {
                self.family = *family;
                let [a, b, c, d] = family.default_coeffs();
                self.a = a;
                self.b = b;
                self.c = c;
                self.d = d;
                // Re-seed with the new family's box (its scale differs) and clear
                // the trail so the new attractor forms cleanly.
                self.seed_particles = Self::seed(*family, self.particle_count);
                self.needs_upload = true;
                self.needs_clear = true;
            }
        }
        None
    }

    /// The frame, in the order the GPU must see it: rebuild if the grid moved,
    /// flush the deferred one-shot uploads, write this frame's uniforms, dispatch
    /// the compute steps, lay the trail, swap, present.
    ///
    /// **The order and the `swap()` placement are load-bearing** — the decay reads
    /// the field's current read side and the present reads the freshly-written one,
    /// so the swap sits exactly between those two passes. The steps are separate
    /// functions for readability only; nothing here may be reordered or merged.
    fn render(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        aspect: f32,
    ) {
        self.rebuild_if_stale();
        let Self {
            res,
            active_count,
            seed_particles,
            needs_upload,
            pending_jitter,
            reseed_count,
            needs_clear,
            pending_steps,
            step_index,
            ifs_ends,
            ifs_fit,
            morph,
            levers,
            dt,
            spin_time,
            family,
            a,
            b,
            c,
            d,
            size,
            hue,
            brightness,
            fade,
            hue_spread,
            hue_center,
            saturation,
            palette_mix,
            zoom,
            pan_x,
            pan_y,
            perspective,
            depth_fade,
            depth_hue,
            map_tint,
            map_hue,
            root_tint,
            root_hue,
            emergence,
            palette,
            palette_dirty,
            ..
        } = self;
        let Some(Resources { pipelines, grid }) = res.as_mut() else {
            return;
        };

        flush_deferred_uploads(
            queue,
            encoder,
            pipelines,
            grid,
            seed_particles,
            palette,
            palette_dirty,
            needs_clear,
            needs_upload,
        );
        upload_uniforms(
            queue,
            pipelines,
            *active_count,
            &UniformInputs {
                aspect,
                coeffs: [*a, *b, *c, *d],
                family: *family,
                spin_time: *spin_time,
                dt: *dt,
                pending_steps: *pending_steps,
                step_index: *step_index,
                // The frame's whole IFS cost: one lerp of two cached
                // decompositions and four recomposes. `map_or` rather than a
                // branch on the family — a map family has no ends cached, and
                // that is the same question asked once.
                ifs: ifs_ends.as_ref().map_or(IfsPacked::ZERO, |(a, b)| {
                    ifs::pack(&ifs::resolve(a, b, *morph, *levers))
                }),
                // **The levers are deliberately absent here** (ADR-0075
                // Alternative C): the fit is a function of `morph` and the
                // figure pair only, so `vigor` surges the figure instead of
                // being re-framed back to a net zero.
                ifs_frame: ifs_fit.as_ref().map(|fit| fit.sample(*morph)),
                size: *size,
                hue: *hue,
                brightness: *brightness,
                fade: *fade,
                hue_spread: *hue_spread,
                hue_center: *hue_center,
                saturation: *saturation,
                palette_mix: *palette_mix,
                zoom: *zoom,
                pan: [*pan_x, *pan_y],
                perspective: *perspective,
                depth_fade: *depth_fade,
                depth_hue: *depth_hue,
                map_tint: *map_tint,
                map_hue: *map_hue,
                root_tint: *root_tint,
                root_hue: *root_hue,
                emergence: *emergence,
            },
        );
        // Before the steps, and only on the frame a `reseed` edge landed: kick each
        // particle where it is. Ahead of the steps so the map immediately begins
        // pulling the disturbed points back onto the attractor within the same
        // frame, which is what makes the disturbance read as the figure being
        // shaken rather than as a separate layer of noise over it.
        encode_jitter(
            queue,
            encoder,
            pipelines,
            *active_count,
            *family,
            reseed_count,
            pending_jitter,
        );
        encode_steps(encoder, pipelines, *active_count, *pending_steps);
        // Advanced by what was actually encoded, and here rather than in
        // `advance`: the uniforms above are written against this frame's base
        // index, so the counter cannot move before they are.
        *step_index = step_index.wrapping_add((*pending_steps).min(MAX_SUBSTEPS));
        encode_trail_pass(encoder, pipelines, *active_count, grid);
        // Between the trail pass and the present, and nowhere else: the trail wrote
        // the write side, so the present must read it.
        grid.field.swap();
        encode_present(encoder, pipelines, grid, view);
    }
}

// ---------------------------------------------------------------------------
// The frame, step by step (Plan 0031 Phase 5)
// ---------------------------------------------------------------------------
//
// `AttractorScene::render` was 228 lines. These are the paragraphs its own
// comments already marked, lifted out verbatim: same calls, same order, same
// `swap()` placement. Free functions rather than methods because `render`
// destructures `self` to borrow the resources and the params at once.

#[cfg(test)]
mod tests;
