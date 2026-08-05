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

pub mod ifs;

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

/// Which strange-attractor map the compute step iterates. Selected data-driven
/// via the optional `[particles]` config table (ADR-0007 `configure` hook); the
/// default is De Jong. Extend as follow-up plans add maps; unknown names are
/// rejected at load.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttractorFamily {
    /// De Jong — a 2D discrete map, bounded in ~[-2, 2].
    DeJong,
    /// Clifford — a 2D discrete map, bounded in ~[-2, 2].
    Clifford,
    /// Thomas — a 3D cyclically-symmetric continuous flow.
    Thomas,
    /// Lorenz — the 3D convection flow (the butterfly), projected to 2D.
    Lorenz,
    /// An iterated function system (ADR-0075) — **not** a strange attractor: four
    /// affine maps, one drawn at random per particle per step. The figure it
    /// converges onto is the carried [`IfsFigure`]; see [`ifs`] for why the
    /// parameterization is the interesting half.
    Ifs(IfsFigure),
}

impl AttractorFamily {
    /// Parse a `[particles] family` name, or `None` if unknown.
    ///
    /// The IFS figures sit in the **same** namespace as the map families rather
    /// than behind a `family = "ifs"` + `figure = "fern"` pair: a preset picks
    /// one figure, the way it picks one map today, and `morph_to` names the other
    /// end out of the identical vocabulary.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "de_jong" => AttractorFamily::DeJong,
            "clifford" => AttractorFamily::Clifford,
            "thomas" => AttractorFamily::Thomas,
            "lorenz" => AttractorFamily::Lorenz,
            _ => AttractorFamily::Ifs(IfsFigure::from_name(name)?),
        })
    }

    /// The IFS figure this family draws, or `None` for the four map families.
    ///
    /// The `if let` every IFS-only code path funnels through, so "is this an
    /// IFS" is asked in one spelling.
    fn figure(self) -> Option<IfsFigure> {
        match self {
            AttractorFamily::Ifs(figure) => Some(figure),
            _ => None,
        }
    }

    /// The compute shader's family selector.
    fn shader_id(self) -> u32 {
        match self {
            AttractorFamily::DeJong => 0,
            AttractorFamily::Clifford => 1,
            AttractorFamily::Thomas => 2,
            AttractorFamily::Lorenz => 3,
            // Every figure is the same shader arm — the figure is data in the
            // uniform's affine table, not a branch.
            AttractorFamily::Ifs(_) => 4,
        }
    }

    /// Default coefficients for the family — the meaning is family-specific
    /// (discrete a,b,c,d; Lorenz sigma,rho,beta; Thomas dissipation in `a`). A
    /// preset's coefficient params modulate around these; unbound falls back here.
    fn default_coeffs(self) -> [f32; 4] {
        match self {
            AttractorFamily::DeJong => [1.641, 1.902, 0.316, 1.525],
            AttractorFamily::Clifford => [-1.4, 1.6, 1.0, 0.7],
            AttractorFamily::Thomas => [0.19, 0.0, 0.0, 0.0],
            AttractorFamily::Lorenz => [10.0, 28.0, 2.6667, 0.0],
            // An IFS's shape lives in its affine table, not in four scalars, so
            // `a`..`d` are inert here — the family's own levers are Phase 5's.
            AttractorFamily::Ifs(_) => [0.0, 0.0, 0.0, 0.0],
        }
    }

    /// Projection: (world scale, dim 2/3, **world centre** to subtract). The
    /// scale fits each attractor's native extent into the frame; the centre is
    /// what the projection pivots and frames on.
    ///
    /// **The centre is three components, not a z-centre** (Plan 0062). It was
    /// scalar while every family that needed one was a 3D flow centred on the
    /// origin in `x` and `y`; the fern spans `y ∈ [0, 10]` and is not
    /// origin-centred, so a 2D family needs the other two. The four map families
    /// pass exactly the values they passed before — `[0,0,0]` and `[0,0,25]` —
    /// and subtracting a zero is exact, so no capture moves.
    fn projection(self) -> (f32, f32, [f32; 3]) {
        match self {
            AttractorFamily::DeJong => (0.42, 2.0, [0.0, 0.0, 0.0]),
            AttractorFamily::Clifford => (0.42, 2.0, [0.0, 0.0, 0.0]),
            AttractorFamily::Thomas => (0.14, 3.0, [0.0, 0.0, 0.0]),
            AttractorFamily::Lorenz => (0.022, 3.0, [0.0, 0.0, 25.0]),
            AttractorFamily::Ifs(figure) => {
                let (scale, centre) = figure.frame();
                (scale, 2.0, centre)
            }
        }
    }

    /// Which plane a 3D family is viewed in (ADR-0068).
    ///
    /// **Named outright per family, deliberately.** It is not derived from
    /// [`projection`](Self::projection)'s `dim`, even though `dim == 3.0` selects
    /// exactly the same two families today: `dim` and "wants a non-default basis"
    /// agree on this roster of four and are not the same property, so keying one
    /// off the other is ADR-0037's trap in another costume. A 2D family with a
    /// preferred orientation, or a 3D family happy with x–y, would break the
    /// coincidence silently. The match is exhaustive, so a fifth family has to
    /// answer the question.
    ///
    /// Only the 3D branch of the draw shader reads it; the 2D families' value is
    /// the default and is never consulted.
    fn basis(self) -> Basis {
        match self {
            AttractorFamily::DeJong
            | AttractorFamily::Clifford
            | AttractorFamily::Thomas
            // The IFS family is `dim = 2` and takes the default. A 3-D IFS is a
            // real thing and a separate decision (ADR-0075).
            | AttractorFamily::Ifs(_) => Basis::XY,
            // The butterfly lives in x–z. Seen x–y it is the two lobes edge-on —
            // a hard X, which is the "dense core inside a diffuse cloud" this
            // preset shipped as (ADR-0068).
            AttractorFamily::Lorenz => Basis::XZ,
        }
    }

    /// Whether this family's successive positions lie on **one trajectory**
    /// (ADR-0069), so a segment drawn between them is a piece of that trajectory
    /// rather than an invented chord.
    ///
    /// **Named per family, like [`basis`](Self::basis), and for the same reason.**
    /// It agrees with `projection().1 == 3.0` on today's roster of four, and that
    /// agreement is a **coincidence** — "is an ODE flow" and "is three
    /// dimensional" are different properties. A 2-D flow would be continuous at
    /// `dim == 2.0`, and a 3-D discrete map would be a 3-D family that must not
    /// take the branch. Keying off `dim` is ADR-0037's trap in the costume
    /// ADR-0068 Alternative C already declined once.
    ///
    /// The distinction is not cosmetic: a discrete map *replaces* its state each
    /// iteration, so successive points are scattered across the whole figure and
    /// a segment between them is a bright chord over the picture — meaningless
    /// geometry, drawn brightly.
    fn is_continuous(self) -> bool {
        match self {
            // An IFS is the extreme case of the discrete argument below: a
            // particle applies a *randomly chosen* map each step, so successive
            // points jump right across the figure and a segment between them is
            // a bright chord over the whole fern.
            AttractorFamily::DeJong | AttractorFamily::Clifford | AttractorFamily::Ifs(_) => false,
            AttractorFamily::Thomas | AttractorFamily::Lorenz => true,
        }
    }

    /// The seeded initial-scatter box: `(half-spread, centre)` per axis. Sized to
    /// the attractor's native extent so particles start spread **across** it —
    /// a box too small for a chaotic flow leaves every particle on nearly the same
    /// trajectory, so the cloud clumps instead of filling the shape. The discrete
    /// 2D maps converge from any small box, so theirs is the historical ~[-1.5,1.5]
    /// (kept identical so their seeded look is unchanged; `z` is unused there).
    ///
    /// **Used for the initial fill and a family change only** (ADR-0066). A
    /// `reseed` no longer re-fills it — see [`Self::jitter_extent`] — because
    /// re-filling replaced the cloud with a uniform axis-aligned rectangle, which
    /// is what a reseed visibly was.
    fn seed_box(self) -> ([f32; 3], [f32; 3]) {
        match self {
            AttractorFamily::DeJong | AttractorFamily::Clifford => {
                ([1.5, 1.5, 1.5], [0.0, 0.0, 0.0])
            }
            AttractorFamily::Thomas => ([4.5, 4.5, 4.5], [0.0, 0.0, 0.0]),
            AttractorFamily::Lorenz => ([20.0, 26.0, 24.0], [0.0, 0.0, 25.0]),
            // The figure's own bounding box, so the fill lands *over* the
            // attractor and contracts onto it — see [`IfsFigure::seed_box`].
            AttractorFamily::Ifs(figure) => figure.seed_box(),
        }
    }

    /// Half-extent of the per-axis kick a `reseed` applies to each particle
    /// **where it already is** (ADR-0066), in the family's own world units.
    ///
    /// Family-relative by construction: it is [`JITTER_FRACTION`] of the family's
    /// own [`seed_box`](Self::seed_box) spread, which is itself sized to the
    /// attractor's native extent. So one constant serves a map bounded in
    /// `[-2, 2]` and a flow spanning `±26` without a per-family number to keep in
    /// step.
    ///
    /// **The magnitude is a look constant with no principled value.** It is large
    /// enough that the disturbance reads and small enough that the points stay on
    /// the figure — a chaotic flow separates jittered neighbours within a few
    /// iterations anyway, which is what makes a small kick sufficient. Plan 0057
    /// Phase 6 is where it is judged in motion, at both tiers; ADR-0066 records
    /// that if the disturbance reads too subtle, *this* is the lever and returning
    /// to the box is not.
    fn jitter_extent(self) -> [f32; 3] {
        // Destructured rather than indexed: this file denies `indexing_slicing`.
        let ([sx, sy, sz], _) = self.seed_box();
        [
            sx * JITTER_FRACTION,
            sy * JITTER_FRACTION,
            sz * JITTER_FRACTION,
        ]
    }

    /// Reciprocal of the family's half-extent along the **view depth axis**, in
    /// its own world units — and **exactly `0.0` for a 2D family** (ADR-0076).
    ///
    /// That zero is the whole mechanism by which the flat families opt out: it
    /// makes `d_n` identically zero for every one of their particles, so the
    /// perspective magnification is `1`, the haze multiplier is `1` and the hue
    /// offset is `0`, with **no shader branch, no division and no way to reach a
    /// `NaN`**. De Jong, Clifford and every IFS figure have no third coordinate
    /// to project, and ADR-0076 Alternative B records why inventing one for them
    /// is worse than leaving them alone.
    ///
    /// **Derived from [`seed_box`](Self::seed_box), not hand-written per family**
    /// — the discipline [`jitter_extent`](Self::jitter_extent) already uses, so
    /// there is no second table of magnitudes to keep in step. The depth is the
    /// rotation's third output, and the rotation acts in the plane spanned by
    /// `x` and the basis's horizontal axis ([`Basis::masks`]'s first selector),
    /// so the depth swings through *those two* half-extents and the larger is
    /// what normalizes it. That is **26** for Lorenz (basis XZ, so the plane is
    /// `x`–`y`, half-extents 20 and 26) and **4.5** for Thomas (basis XY, plane
    /// `x`–`z`).
    ///
    /// The match is exhaustive with no wildcard arm, so a fifth family has to
    /// answer the question rather than inherit an answer.
    fn inv_depth_extent(self) -> f32 {
        // Destructured rather than indexed: this file denies `indexing_slicing`.
        let ([sx, sy, sz], _) = self.seed_box();
        let half = match self {
            // Every IFS figure is `dim = 2` — it has no third coordinate to
            // project, which is exactly the case this doc comment anticipated.
            AttractorFamily::DeJong | AttractorFamily::Clifford | AttractorFamily::Ifs(_) => {
                return 0.0;
            }
            AttractorFamily::Thomas | AttractorFamily::Lorenz => {
                // Read off `basis()` rather than restated per family, so the two
                // cannot disagree about which plane the spin turns in.
                let partner = match self.basis() {
                    Basis::XY => sz,
                    Basis::XZ => sy,
                };
                sx.max(partner)
            }
        };
        // A degenerate box would otherwise send an infinity to the shader. It
        // cannot happen with the boxes above; it costs one compare not to rely
        // on that.
        if half > 0.0 { 1.0 / half } else { 0.0 }
    }
}

/// The plane a 3D attractor family is projected into (ADR-0068), as chosen by
/// [`AttractorFamily::basis`].
///
/// The spin always rotates `x` against the *other* horizontal axis and leaves the
/// vertical alone, so a basis is fully described by naming that pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Basis {
    /// `x` horizontal, `y` vertical; the spin rotates `x` against `z`. The shared
    /// convention every 3D family used before ADR-0068, and still every family's
    /// answer but Lorenz's.
    XY,
    /// `x` horizontal, `z` vertical; the spin rotates `x` against `y`. Lorenz's
    /// butterfly lies in this plane.
    XZ,
}

impl Basis {
    /// The two axis selectors the draw shader dots the centred position against:
    /// `(the axis the spin rotates x against, the vertical axis)`.
    ///
    /// Masks rather than indices because WGSL will not dynamically index a
    /// `vec3` outside addressable storage, and a `dot` against a one-hot vector
    /// is branch-free — so the basis stays one pipeline and one draw call.
    fn masks(self) -> ([f32; 3], [f32; 3]) {
        match self {
            Basis::XY => ([0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
            Basis::XZ => ([0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
        }
    }
}

/// Base display rotation (rad/s), so the cloud visibly turns even when the point
/// set saturates its footprint — the animation liveness the differential tests
/// require, independent of audio.
///
/// It is `2π / 0.18` = **one revolution per 34.9 seconds**, and until ADR-0076
/// that was the *only* rate any attractor could turn at: no preset could reach
/// the rotation of a 3D figure at all. The slowness is part of why the 3D
/// families read as flat — a viewer never accumulates enough motion evidence to
/// resolve which way the thing is turning. `spin` is a **multiplier** on this,
/// so `1` is unchanged, `0` holds the figure still and negative reverses it.
const SPIN_RATE: f32 = 0.18;

/// `spin`'s default: exactly today's rate.
const DEFAULT_SPIN: f32 = 1.0;

/// One frame's contribution to the integrated spin, in **spin-scaled seconds**.
///
/// **The phase is integrated, and that is not a preference.** Computing it as
/// `time · spin · SPIN_RATE` would let a `spin` bound to audio retroactively
/// rescale *all* elapsed time on every frame: the figure would snap to a new
/// angle whenever the binding moved, jerking rather than accelerating. A rate
/// multiplier has to be integrated to be a rate at all.
fn advance_spin(spin_time: f32, spin: f32, dt: f32) -> f32 {
    spin_time + spin * dt
}

/// The rotation angle, in radians, for an accumulated spin-time.
///
/// **The multiply by [`SPIN_RATE`] is deferred to here rather than folded into
/// [`advance_spin`], and the reason is arithmetic rather than taste.** At the
/// default `spin = 1` the accumulator is then `Σ dt` term for term — bit-for-bit
/// the same summation the renderer performs for its own clock — so `spin = 1`
/// reproduces the pre-ADR-0076 `time * SPIN_RATE` *exactly*, and no golden
/// baseline moves. Folding the rate in would sum `0.18 · dt` instead and drift
/// in the last bits of every capture.
fn spin_phase(spin_time: f32) -> f32 {
    spin_time * SPIN_RATE
}

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

/// Compute step: iterate every particle through the selected attractor map once.
/// Discrete maps (De Jong, Clifford) iterate directly; continuous flows (Thomas,
/// Lorenz) Euler-integrate a few sub-steps of the fixed frame `dt`. Writes the
/// storage buffer in place; the draw pass then reads it as a vertex buffer.
const STEP_SHADER: &str = r#"
struct Particle {
    pos: vec3<f32>,
    seed: f32,
    prev: vec3<f32>,
    pad: f32,
}
@group(0) @binding(0) var<storage, read_write> particles: array<Particle>;

struct Step {
    coeffs: vec4<f32>, // discrete: a,b,c,d; Lorenz: sigma,rho,beta; Thomas: b
                       //   family 5 (jitter): xyz half-extent of the kick,
                       //   w != 0 draws the kick as a streak (ADR-0069)
    dt: f32,           // fixed sub-step seconds (for continuous families)
    family: u32,       // 0 De Jong, 1 Clifford, 2 Thomas, 3 Lorenz, 4 IFS, 5 jitter
    count: u32,        // active particle count
    salt: u32,         // jitter only: which reseed this is
    // Monotonic fixed-step counter, incremented once per compute step (ADR-0075).
    // The IFS's map choice is drawn from it and the particle's own fixed seed, so
    // the draw stays a pure function of the seed and the step sequence — and the
    // step sequence is a pure function of accumulated injected dt, which captures
    // pin at 1/60 s. Zero and unread on every other family.
    step_index: u32,
    // The vec4 alignment the affine table below requires. THREE SCALARS, not a
    // `vec3<u32>`: a WGSL vec3 aligns to 16, which would push the table to offset
    // 64 and the struct to 176 while the Rust side laid it out at 48 and 160.
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
    // The IFS's resolved affine table (family 4), CPU-side output of
    // `ifs::resolve`. Four linear parts (a,b,c,d), four (e,f) translations packed
    // two per row, and the cumulative probabilities a unit draw is compared
    // against. Four named rows rather than an array: the map choice is an
    // unrolled branch, for the reason `Basis::masks` uses one-hot selectors.
    m0: vec4<f32>,
    m1: vec4<f32>,
    m2: vec4<f32>,
    m3: vec4<f32>,
    t01: vec4<f32>,
    t23: vec4<f32>,
    cumulative_p: vec4<f32>,
}
@group(0) @binding(1) var<uniform> step: Step;

// Euler sub-steps per frame for the continuous (ODE) families, so a stiff flow
// (Lorenz) stays stable at the frame dt without a per-family clock.
const ODE_SUBSTEPS: i32 = 4;

// A reseed's per-particle offset (ADR-0066). Deterministic: a pure function of
// the particle's own fixed seed and the reseed counter, so the cloud stays a pure
// function of its seed and step sequence, and two runs from the same seed produce
// identical positions after a reseed.
//
// Salted by the counter rather than by the seed alone, so successive reseeds kick
// a given particle in different directions. With the seed alone every reseed
// would apply the same displacement field, which over a session is a rigid
// pattern rather than a disturbance.
// One round of a bit-mixer (the lowbias32 constants), so a small change in the
// input decorrelates the output.
fn mix32(v: u32) -> u32 {
    var h = v;
    h = h ^ (h >> 16u);
    h = h * 0x7FEB352Du;
    h = h ^ (h >> 15u);
    h = h * 0x846CA68Bu;
    h = h ^ (h >> 16u);
    return h;
}

// The top 24 bits as a signed unit fraction in [-1, 1).
fn unit(h: u32) -> f32 {
    return f32(h >> 8u) / 16777216.0 * 2.0 - 1.0;
}

// The same 24 bits as an UNSIGNED fraction in [0, 1) — the IFS's map draw, which
// is compared against a cumulative probability table. It cannot reach 1.0, and
// the fourth map is the `else` arm regardless, so no draw lands nowhere.
fn unit01(h: u32) -> f32 {
    return f32(h >> 8u) / 16777216.0;
}

// Unrolled rather than looped over a dynamically-indexed vector: WGSL permits
// that only for addressable storage and the backends disagree about the rest, so
// three named rounds is the portable spelling.
fn hash3(seed: f32, salt: u32) -> vec3<f32> {
    let h0 = mix32(bitcast<u32>(seed) ^ (salt * 0x9E3779B9u));
    let h1 = mix32(h0);
    let h2 = mix32(h1);
    return vec3<f32>(unit(h0), unit(h1), unit(h2));
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= step.count) {
        return;
    }
    let a = step.coeffs.x;
    let b = step.coeffs.y;
    let c = step.coeffs.z;
    let d = step.coeffs.w;
    var p = particles[i].pos;
    // Captured before any branch mutates `p`. The storage slot still holds this
    // value until the write at the end, so reading it there would work too —
    // but only by knowing that, which is exactly the kind of thing that breaks
    // when someone reorders the writes.
    let origin = p;

    if (step.family == 5u) {
        // A reseed: disturb the cloud where it is, rather than replacing it with
        // a uniform fill of the seed box (ADR-0066). The points stay on the
        // attractor, so no axis-aligned rectangle exists at any moment, and the
        // map's own mixing spreads the kick within a few iterations.
        let kicked = p + hash3(particles[i].seed, step.salt) * step.coeffs.xyz;
        // Whether the kick is *drawn* as a streak is Phase 4's call, not this
        // phase's: a jitter displaces a particle by far more than a step does
        // (ADR-0069 measures ~15x a frame's travel), so the segment would be a
        // long stroke along a path the particle never traversed. `w` selects it,
        // so an A/B is a constant flip and no shader edit.
        //   w != 0 -> prev stays pre-kick, the streak is drawn
        //   w == 0 -> prev follows the kick, so the segment has zero length
        particles[i].prev = select(kicked, origin, step.coeffs.w != 0.0);
        particles[i].pos = kicked;
        return;
    }

    if (step.family == 0u) {
        // De Jong: x' = sin(a*y) - cos(b*x), y' = sin(c*x) - cos(d*y).
        p = vec3<f32>(sin(a * p.y) - cos(b * p.x), sin(c * p.x) - cos(d * p.y), 0.0);
    } else if (step.family == 1u) {
        // Clifford: x' = sin(a*y) + c*cos(a*x), y' = sin(b*x) + d*cos(b*y).
        p = vec3<f32>(sin(a * p.y) + c * cos(a * p.x), sin(b * p.x) + d * cos(b * p.y), 0.0);
    } else if (step.family == 4u) {
        // Iterated function system (ADR-0075): draw one of four affine maps and
        // apply it. The draw is salted by the step counter rather than by the
        // reseed counter, so a particle picks a different map each step while
        // staying a pure function of its own fixed seed and the step index.
        let r = unit01(mix32(bitcast<u32>(particles[i].seed) ^ (step.step_index * 0x9E3779B9u)));
        // Unrolled rather than a dynamically-indexed uniform array — WGSL permits
        // that only for addressable storage and the backends disagree about the
        // rest, the same reason `hash3` above is three named rounds.
        var m = step.m3;
        var t = step.t23.zw;
        if (r < step.cumulative_p.x) {
            m = step.m0;
            t = step.t01.xy;
        } else if (r < step.cumulative_p.y) {
            m = step.m1;
            t = step.t01.zw;
        } else if (r < step.cumulative_p.z) {
            m = step.m2;
            t = step.t23.xy;
        }
        // x' = a*x + b*y + e,  y' = c*x + d*y + f. Two dimensional: z stays 0.
        p = vec3<f32>(m.x * p.x + m.y * p.y + t.x, m.z * p.x + m.w * p.y + t.y, 0.0);
    } else if (step.family == 2u) {
        // Thomas cyclically-symmetric flow (b = dissipation). Lively speed-up so
        // the slow flow visibly moves each frame.
        let h = step.dt * 3.0 / f32(ODE_SUBSTEPS);
        for (var s = 0; s < ODE_SUBSTEPS; s = s + 1) {
            let dp = vec3<f32>(sin(p.y) - a * p.x, sin(p.z) - a * p.y, sin(p.x) - a * p.z);
            p = p + dp * h;
        }
    } else {
        // Lorenz (sigma, rho, beta). Euler-integrated in sub-steps for stability.
        let h = step.dt / f32(ODE_SUBSTEPS);
        for (var s = 0; s < ODE_SUBSTEPS; s = s + 1) {
            let dp = vec3<f32>(a * (p.y - p.x), p.x * (b - p.z) - p.y, p.x * p.y - c * p.z);
            p = p + dp * h;
        }
    }

    // The position this particle came from, for the continuous families' segment
    // (ADR-0069). Written for every family — the *draw* decides whether to use
    // it, so the buffer's contents stay one shape and a discrete map that took
    // the branch by mistake is a visible chord rather than stale data.
    //
    // This is the position before the whole step, not before the last Euler
    // sub-step: ADR-0069 rejected the sub-step polyline by measurement, and the
    // segment is meant to span the frame's travel.
    particles[i].prev = origin;
    particles[i].pos = p;
}
"#;

/// Draw pass: one additive glowing point-sprite per particle, into the trail
/// field. The particle storage buffer is bound as an instance vertex buffer; the
/// shader expands each into a screen-facing quad, projects the (2D or 3D)
/// attractor state to the screen with a slow spin, and tints it from the seeded
/// per-particle offset.
const DRAW_SHADER: &str = r#"
struct Draw {
    // v: x aspect, y point half-size (world), z hue offset, w spin (radians)
    // w: x world scale, y dim (2 or 3), z unused (was the z-center; Plan 0062
    //    made the centre three components and moved it to `ctr`),
    //    w deposit scale (ADR-0065: FLOOR_PARTICLES / active_count)
    // u: x hue_spread, y hue_center, z palette_mix, w saturation
    // x: x zoom, yz pan (view transform, ADR-0018), w streak (ADR-0069:
    //    non-zero on a continuous family, so the quad spans prev -> pos)
    // bh: xyz the axis the spin rotates x against (ADR-0068), w unused
    // bv: xyz the vertical axis (ADR-0068), w unused
    // d: x perspective, y depth_fade, z depth_hue, w the family's INVERSE depth
    //    half-extent (ADR-0076) - exactly 0 for a 2D family, which is what
    //    collapses every depth cue below to the identity with no branch
    // ctr: xyz the world centre subtracted before projection, w unused. The four
    //    map families pass [0,0,0] or [0,0,25] - exactly what they passed when
    //    this was the scalar `w.z` - and subtracting a zero is exact.
    v: vec4<f32>,
    w: vec4<f32>,
    u: vec4<f32>,
    x: vec4<f32>,
    bh: vec4<f32>,
    bv: vec4<f32>,
    d: vec4<f32>,
    ctr: vec4<f32>,
}
@group(0) @binding(0) var<uniform> draw: Draw;
// Shared gradient LUTs (ADR-0021): sampled per-particle in the vertex shader
// (VERTEX visibility). A/B for the `palette_mix` crossfade, one repeat sampler.
@group(0) @binding(1) var lut_a: texture_2d<f32>;
@group(0) @binding(2) var lut_b: texture_2d<f32>;
@group(0) @binding(3) var lut_samp: sampler;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    // Position within the sprite, in units of the point radius. For a point this
    // is the corner itself; for a segment the quad is stretched along its axis
    // and this is the coordinate in the segment's own frame.
    @location(0) local: vec2<f32>,
    @location(1) color: vec3<f32>,
    // Half-length of the segment in the same units, so the fragment measures
    // distance to a *capsule* rather than to a disc. Exactly 0 for a point,
    // which makes the two cases one expression (ADR-0069).
    @location(2) @interpolate(flat) half_len: f32,
}

// Shared `saturation` (mirrors core/src/render/palette.rs::desaturate verbatim).
fn apply_saturation(c: vec3<f32>, s: f32) -> vec3<f32> {
    let luma = dot(c, vec3<f32>(0.299, 0.587, 0.114));
    return vec3<f32>(luma) + (c - vec3<f32>(luma)) * s;
}

// Project one attractor position to the pre-aspect "world" plane, **keeping the
// depth** the rotation produces: `xy` is the screen position, `z` is the view
// depth (ADR-0076). Factored out of the vertex body so a segment can project
// **both** its endpoints through the identical path — two call sites that must
// not be allowed to drift apart.
//
// The depth used to be computed and thrown away, which is exactly why the 3D
// families rendered flat: an orthographic projection of a rotating transparent
// structure carries no information about the direction of rotation, because the
// image at rotation pi is the exact x-mirror of the image at 0.
//
// **This function and the two below are the SOURCE**; `projection_mirror` in the
// Rust body transcribes them for the property test, the same discipline
// `apply_saturation` follows against `palette.rs::desaturate`. Edit here, then
// edit there.
fn project(q: vec3<f32>, dim: f32, ctr: vec3<f32>, cs: f32, sn: f32) -> vec3<f32> {
    if (dim < 2.5) {
        // 2D map: centre, then in-plane rotation. There is no third coordinate,
        // so the depth is zero here as well as via `draw.d.w` - belt and braces,
        // and neither is load-bearing alone.
        //
        // The centre is what lets a 2D figure sit off the origin (the fern spans
        // y in [0, 10]); De Jong and Clifford pass [0,0,0], so this subtraction
        // is exact and their captures are unchanged.
        let c = q - ctr;
        return vec3<f32>(c.x * cs - c.y * sn, c.x * sn + c.y * cs, 0.0);
    }
    // 3D flow: centre, pick the viewing plane, rotate around the vertical
    // axis, project. The plane is the family's own (ADR-0068), arriving as two
    // one-hot axis selectors rather than as a second pipeline: `bh` is the axis
    // the spin rotates `x` against, `bv` is the vertical. `bh = z, bv = y`
    // reproduces the shared convention this replaced exactly; Lorenz ships
    // `bh = y, bv = z`.
    let p = q - ctr;
    let h = dot(p, draw.bh.xyz);
    // The third term is the rotation's OTHER output - the exact partner of the
    // horizontal one - so it costs a multiply-add rather than a second rotation.
    return vec3<f32>(p.x * cs + h * sn, dot(p, draw.bv.xyz), -p.x * sn + h * cs);
}

// Depth in units of the family's own half-extent, clamped to [-1, 1].
//
// `draw.d.w` is an INVERSE extent and is exactly 0 for a 2D family, so this is
// identically 0 there - no branch, no division, no NaN.
//
// **The clamp is not decoration.** A family's converged figure overruns its
// `seed_box` (Lorenz reaches y = 25.4 against a 26 half-extent while its x
// reaches 19.2, so the rotated depth reaches ~1.22), and an unclamped value at
// the `perspective` ceiling would magnify by ~50x rather than the 5x ADR-0076
// documents. Clamping is what makes the stated (1 + p) / (1 - p) ratio true and
// keeps the divisor below bounded away from zero.
fn depth_norm(depth: f32) -> f32 {
    return clamp(depth * draw.d.w, -1.0, 1.0);
}

// The perspective magnification: near material grows, far material shrinks.
// `perspective` is the figure's depth half-extent as a fraction of the camera
// distance, clamped CPU-side to [0, 0.8], so the divisor stays in [0.2, 1.8].
// At `perspective = 0` this is exactly 1.0 and every use of it is a no-op.
fn magnify(dn: f32) -> f32 {
    return 1.0 / (1.0 - draw.d.x * dn);
}

// Depth remapped to [0, 1] with **1 nearest**, which is the sense both
// atmospheric cues below are written in. `dn` is already clamped, so this needs
// no clamp of its own.
fn depth01(dn: f32) -> f32 {
    return (dn + 1.0) * 0.5;
}

// Distance haze: brightness attenuated with distance, so `depth_fade = 1` takes
// the far end to black and `0` is exactly 1.0 everywhere (ADR-0076).
// `depth_fade` is clamped CPU-side to [0, 1] - past 1 this would go NEGATIVE,
// and a negative deposit in an additive accumulation subtracts light.
fn haze(dn: f32) -> f32 {
    return 1.0 - draw.d.y * (1.0 - depth01(dn));
}

// Distance tint: a shift of +/- depth_hue/2 in the palette coordinate across the
// depth range, centred so the mid-depth colour is the one the preset asked for.
// Rides the existing LUT sample and needs no new machinery.
fn depth_tint(dn: f32) -> f32 {
    return draw.d.z * (depth01(dn) - 0.5);
}

@vertex
fn vs_main(
    @builtin(vertex_index) vi: u32,
    @location(0) center: vec3<f32>,
    @location(1) seed: f32,
    @location(2) previous: vec3<f32>,
) -> VsOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
    );
    let corner = corners[vi] * 2.0 - vec2<f32>(1.0, 1.0);
    let aspect = draw.v.x;
    let psize = draw.v.y;
    let hue = draw.v.z;
    let rot = draw.v.w;
    let scl = draw.w.x;
    let dim = draw.w.y;
    let ctr = draw.ctr.xyz;
    let hue_spread = draw.u.x;
    let hue_center = draw.u.y;
    let palette_mix = draw.u.z;
    let saturation = draw.u.w;

    let cs = cos(rot);
    let sn = sin(rot);
    let streak = draw.x.w;
    let projected = project(center, dim, ctr, cs, sn);
    let screen = projected.xy;
    // This particle's normalized depth, and the magnification it earns
    // (ADR-0076). Both are exactly 0 and exactly 1 for a 2D family.
    let dn = depth_norm(projected.z);
    let mag = magnify(dn);
    // Position AND sprite size take the same magnification, which is what makes
    // size grading and parallax one mutually-consistent term rather than two
    // hand-tuned constants (the swarm needed two; ADR-0076 Alternative B).
    let sprite = psize * mag;

    // The sprite. A point is a `sprite` square about the projected position; a
    // segment is that square swept from `prev` to `pos` — a capsule (ADR-0069).
    //
    // Both are built in **world** space, before the single aspect division below.
    // That is deliberate and it is what keeps the stroke an even width: world `x`
    // is what becomes NDC `x / aspect`, so equal world distances are equal
    // *pixels* on both axes, and a capsule built here is round-ended on screen
    // rather than sheared by the target's aspect (ADR-0037).
    var world: vec2<f32>;
    var local: vec2<f32>;
    var half_len = 0.0;
    if (streak != 0.0) {
        // **Both endpoints are magnified independently**, so a trace receding
        // into the distance is drawn genuinely shorter - the strongest depth cue
        // a curve has, and free, because the capsule already projects both ends.
        let pp = project(previous, dim, ctr, cs, sn);
        let dn_prev = depth_norm(pp.z);
        let a = pp.xy * scl * magnify(dn_prev);
        let b = screen * scl * mag;
        let mid = (a + b) * 0.5;
        let axis = (b - a) * 0.5;
        let len = length(axis);
        // A stationary particle has no direction to orient by, and `normalize`
        // of a zero vector is undefined — so fall back to the point's own frame,
        // which is what a zero-length capsule is anyway.
        var dir = vec2<f32>(1.0, 0.0);
        if (len > 1e-9) {
            dir = axis / len;
        }
        let nrm = vec2<f32>(-dir.y, dir.x);
        // The capsule's WIDTH is uniform and takes the midpoint's magnification.
        // A tapered stroke would mean interpolating a radius in the fragment's
        // distance function, which reworks ADR-0069's one-expression
        // point/segment unification - deliberately out of scope (ADR-0076).
        let wid = psize * magnify((dn + dn_prev) * 0.5);
        half_len = len / wid;
        // Extended by `wid` past each end so the round caps have room.
        world = mid + dir * (corner.x * (len + wid)) + nrm * (corner.y * wid);
        local = vec2<f32>(corner.x * (half_len + 1.0), corner.y);
    } else {
        world = screen * scl * mag + corner * sprite;
        local = corner;
    }

    // View transform (ADR-0018): project to NDC, then scale about the screen centre
    // by `zoom` and offset by `pan`. Default zoom = 1, pan = 0 is the identity, so an
    // unbound preset is byte-unchanged. Applied post-projection so it moves the whole
    // attractor (position and apparent point size) as one.
    let zoom = draw.x.x;
    let pan = draw.x.yz;
    let ndc = vec2<f32>(world.x / aspect, world.y) * zoom + pan;

    // Per-particle colour through the shared LUT: the seeded jitter occupies the
    // band `hue_center + (seed - 0.5)*hue_spread` (was a hardcoded `seed*0.15`),
    // plus the shared `hue`; both LUTs crossfade by `palette_mix` before
    // `saturation`. `textureSampleLevel` (LOD 0) — vertex stage has no derivatives.
    //
    // The depth tint rides the same coordinate (ADR-0076). Taken from the
    // particle's own `dn` - for a segment that is the head's, not the midpoint's:
    // the colour follows where the particle IS, and the trail behind it already
    // carries the shade it had when it was there.
    let coord = hue + hue_center + (seed - 0.5) * hue_spread + depth_tint(dn);
    let ca = textureSampleLevel(lut_a, lut_samp, vec2<f32>(coord, 0.5), 0.0).rgb;
    let cb = textureSampleLevel(lut_b, lut_samp, vec2<f32>(coord, 0.5), 0.0).rgb;
    let col = apply_saturation(mix(ca, cb, clamp(palette_mix, 0.0, 1.0)), saturation);

    // Normalize the additive deposit by the particle count (ADR-0065), so total
    // light per frame is invariant to the tier, times the preset's `brightness`
    // (ADR-0080). Applied here rather than in the fragment shader because the draw
    // uniform is bound VERTEX-only; the fragment multiplies this by its own radial
    // falloff, and both are linear, so the result is identical to scaling the
    // emitted fragment.
    let deposit = draw.w.w;

    var out: VsOut;
    out.pos = vec4<f32>(ndc, 0.0, 1.0);
    out.local = local;
    out.half_len = half_len;
    // ...and the distance haze, which is the stand-in for the occlusion this
    // scene deliberately does not do (ADR-0076). Applied to the emitted light,
    // so the trail inherits the grading: a particle that was far and is now near
    // leaves a dim streak behind a bright head.
    out.color = col * deposit * haze(dn);
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // Distance to the segment from (-half_len, 0) to (+half_len, 0), in units of
    // the point radius. At `half_len = 0` this is `length(in.local)` exactly —
    // the point's own radial falloff, unchanged, which is what lets the discrete
    // families keep byte-identical captures through one shader.
    let d = length(vec2<f32>(max(abs(in.local.x) - in.half_len, 0.0), in.local.y));
    let falloff = max(0.0, 1.0 - d);
    let g = falloff * falloff;
    return vec4<f32>(in.color * g, 1.0);
}
"#;

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
mod projection_mirror {
    use super::AttractorFamily;

    /// One projected particle: the pre-aspect "world" plane position, and the
    /// view depth the rotation produces alongside it.
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub(super) struct Projected {
        pub(super) screen: [f32; 2],
        pub(super) depth: f32,
    }

    /// Mirrors `project()` in [`DRAW_SHADER`](super::DRAW_SHADER).
    ///
    /// Takes `cs`/`sn` rather than an angle, exactly as the WGSL does — which is
    /// also what lets a test state the mirror identity *exactly*: `cos` of an
    /// `f32` `π` is not `−1` to the last bit, and the property ADR-0076 names is
    /// about `cs = −1, sn = 0`.
    pub(super) fn project(q: [f32; 3], family: AttractorFamily, cs: f32, sn: f32) -> Projected {
        let (_, dim, [cx, cy, cz]) = family.projection();
        let ([hx, hy, hz], [vx, vy, vz]) = family.basis().masks();
        let [qx, qy, qz] = q;
        let [px, py, pz] = [qx - cx, qy - cy, qz - cz];
        if dim < 2.5 {
            return Projected {
                screen: [px * cs - py * sn, px * sn + py * cs],
                depth: 0.0,
            };
        }
        let h = px * hx + py * hy + pz * hz;
        let v = px * vx + py * vy + pz * vz;
        Projected {
            screen: [px * cs + h * sn, v],
            depth: -px * sn + h * cs,
        }
    }

    /// Mirrors `depth_norm()` in [`DRAW_SHADER`](super::DRAW_SHADER).
    pub(super) fn depth_norm(depth: f32, inv_extent: f32) -> f32 {
        (depth * inv_extent).clamp(-1.0, 1.0)
    }

    /// Mirrors `magnify()` in [`DRAW_SHADER`](super::DRAW_SHADER).
    pub(super) fn magnify(dn: f32, perspective: f32) -> f32 {
        1.0 / (1.0 - perspective * dn)
    }

    /// Mirrors `depth01()` in [`DRAW_SHADER`](super::DRAW_SHADER).
    pub(super) fn depth01(dn: f32) -> f32 {
        (dn + 1.0) * 0.5
    }

    /// Mirrors `haze()` in [`DRAW_SHADER`](super::DRAW_SHADER) — the per-particle
    /// brightness multiplier.
    ///
    /// This is where the fade is measurable at all. Which *screen region* holds
    /// the far material depends on the spin phase, so a pixel-side assertion
    /// would be measuring the clock; the multiplier is the thing the decision is
    /// about.
    pub(super) fn haze(dn: f32, depth_fade: f32) -> f32 {
        1.0 - depth_fade * (1.0 - depth01(dn))
    }

    /// Mirrors `depth_tint()` in [`DRAW_SHADER`](super::DRAW_SHADER) — the shift
    /// added to the per-particle palette coordinate.
    pub(super) fn depth_tint(dn: f32, depth_hue: f32) -> f32 {
        depth_hue * (depth01(dn) - 0.5)
    }

    /// The magnified world-space position of one particle — `project` composed
    /// with the two above and the family's world scale, which is the composition
    /// the vertex shader performs before the aspect division and the view
    /// transform.
    ///
    /// The sprite's own corner offset is left off: it is a fixed square about
    /// this point, so it cannot affect whether two projections are mirror images.
    pub(super) fn world(
        q: [f32; 3],
        family: AttractorFamily,
        cs: f32,
        sn: f32,
        perspective: f32,
    ) -> [f32; 2] {
        let (scl, _, _) = family.projection();
        let p = project(q, family, cs, sn);
        let m = magnify(depth_norm(p.depth, family.inv_depth_extent()), perspective);
        let [sx, sy] = p.screen;
        [sx * scl * m, sy * scl * m]
    }
}

/// Decay pass: draw the previous accumulation back into the fresh target scaled
/// by the per-frame retention factor `k`, laying down the faded trail before the
/// new points are added on top.
const DECAY_SHADER: &str = r#"
struct Decay { k: vec4<f32> } // x: per-frame retention factor
@group(0) @binding(0) var prev: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var<uniform> decay: Decay;

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let c = textureSampleLevel(prev, samp, in.uv, 0.0).rgb * decay.k.x;
    return vec4<f32>(c, 1.0);
}
"#;

/// Present pass: composite the accumulation field to the surface (linear sample,
/// stretched to fill; aspect ignored as in the reaction-diffusion present).
const PRESENT_SHADER: &str = r#"
@group(0) @binding(0) var field: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let c = textureSampleLevel(field, samp, in.uv, 0.0).rgb;
    // Alpha from the accumulated luminance so empty space (no points, no trail) is
    // transparent and reveals the bg_* backdrop (ADR-0026), while bright cloud cores
    // (luma -> 1) occlude it. The present pipeline blends premultiplied-OVER: `c` is
    // emitted as-is (added over the backdrop), so over the default black backdrop
    // this is byte-identical to the prior opaque present.
    let a = clamp(dot(c, vec3<f32>(0.299, 0.587, 0.114)), 0.0, 1.0);
    return vec4<f32>(c, a);
}
"#;

/// One particle, GPU storage-buffer layout (std430). **32 bytes**: the current
/// 3D attractor position (2D families keep `z = 0`), a per-particle seed jitter
/// set once at init, and the position this particle held *before* the current
/// step, which the continuous families draw a segment back to (ADR-0069).
///
/// Each `f32` packs into the preceding `vec3`'s trailing slot (offsets 12 and
/// 28), so std430 lays this out as two tight 16-byte halves and the stride is 32
/// — matching this `repr(C)` layout. **It used to be a tight 16**, and that note
/// is corrected here rather than deleted because the packing argument is the same
/// one, applied twice; `_pad` is the explicit name for the slot `seed` occupies
/// in the first half.
///
/// The price of `prev` is one extra 16 bytes per particle, which at the tier
/// budgets is **1.6 MB** at `Floor` (50 000) and **4.8 MB** at `Rich` (150 000),
/// up from 0.8 MB and 2.4 MB. It is GPU storage, allocated once at build and
/// never resized — `[particles] density` narrows what is *drawn*, not what is
/// allocated (ADR-0069), so a sparse preset pays the full figure.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Particle {
    pos: [f32; 3],
    seed: f32,
    prev: [f32; 3],
    _pad: f32,
}

/// Compute step uniform (per frame): the attractor coefficients, the fixed
/// sub-step `dt`, the selected family, and the active particle count.
///
/// The same layout drives the one-shot **jitter** dispatch (ADR-0066), where
/// `family` is [`JITTER_MODE`], `coeffs.xyz` is the kick's half-extent and `salt`
/// is the reseed counter. One struct and one pipeline rather than a second of
/// each: the jitter reads and writes the same storage buffer through the same
/// bind-group layout, so only the uniform's contents differ.
///
/// **160 bytes since Plan 0062**, up from 32, for every family including the four
/// that ignore the new fields — negligible in bandwidth, and noted because it is
/// a struct four families share. ADR-0075 predicted 144; the extra 16 is the
/// alignment padding [`step_index`](Self::step_index) forces, because the scalar
/// block ahead of the `vec4` table has to round up to a multiple of 16 and it was
/// already exactly full.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct StepUniform {
    coeffs: [f32; 4],
    dt: f32,
    family: u32,
    count: u32,
    /// Which reseed this is, for the jitter dispatch. Zero (and unread) on a
    /// stepping dispatch — it was the struct's explicit padding word.
    salt: u32,
    /// The monotonic fixed-step counter the IFS draws its map choice from
    /// (ADR-0075). Zero (and unread) on every other family, and on the jitter
    /// dispatch — which keeps its own `salt` rather than sharing this.
    step_index: u32,
    /// Explicit, because the `vec4` table below is 16-byte aligned and the
    /// scalars above are five words. `bytemuck::Pod` requires no implicit
    /// padding, so this word must be named.
    _pad: [u32; 3],
    /// The IFS's resolved affine table — [`IfsPacked`] laid out flat. Zeroed for
    /// the four map families, which never read it.
    linear: [[f32; 4]; ifs::MAPS],
    translate: [[f32; 4]; 2],
    cumulative_p: [f32; 4],
}

impl StepUniform {
    /// The IFS half of the uniform, as the four map families and the jitter
    /// dispatch write it: all zeros, and unread.
    const NO_IFS: IfsPacked = IfsPacked::ZERO;

    /// Assemble one slot. The IFS payload is spread across three fields, so a
    /// constructor is what keeps the three call sites from disagreeing about it.
    fn new(
        coeffs: [f32; 4],
        family: u32,
        count: u32,
        salt: u32,
        step_index: u32,
        packed: IfsPacked,
    ) -> Self {
        Self {
            coeffs,
            dt: FIXED_STEP,
            family,
            count,
            salt,
            step_index,
            _pad: [0; 3],
            linear: packed.linear,
            translate: packed.translate,
            cumulative_p: packed.cumulative_p,
        }
    }
}

/// Draw uniform (per frame). `v`: x aspect, y point half-size, z hue offset, w
/// spin. `w`: x world scale, y projection dim (2 or 3), z z-centre (3D),
/// w [`deposit_scale`] (ADR-0065) times [`brightness_factor`] (ADR-0080).
/// `u`: x hue_spread, y hue_center, z palette_mix, w saturation (ADR-0021).
/// `x`: x zoom, yz pan (view transform, ADR-0018), w the streak flag
/// (ADR-0069) — non-zero exactly when [`AttractorFamily::is_continuous`].
/// `bh`/`bv`: the 3D projection basis's two axis selectors (ADR-0068) — the axis
/// the spin rotates `x` against, and the vertical. Read only on the 3D branch.
/// `d`: x `perspective`, y `depth_fade`, z `depth_hue`, w the family's
/// **inverse** depth half-extent (ADR-0076) — `0` for a 2D family, which is what
/// makes every depth cue the identity there without a shader branch.
/// `ctr`: xyz the world centre subtracted before projection (Plan 0062), w unused.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct DrawUniform {
    v: [f32; 4],
    w: [f32; 4],
    u: [f32; 4],
    x: [f32; 4],
    bh: [f32; 4],
    bv: [f32; 4],
    d: [f32; 4],
    ctr: [f32; 4],
}

/// Decay uniform (per frame): x is the per-frame trail retention factor.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct DecayUniform {
    k: [f32; 4],
}

/// The GPU-side state, built lazily on first render (see the module docs), split
/// along the axis that actually varies (Plan 0029 Phase 1): everything in
/// [`PipelineResources`] is built once and survives every size change; only
/// [`FieldResources`] is rebuilt when the accumulation grid changes.
struct Resources {
    pipelines: PipelineResources,
    grid: FieldResources,
}

/// The grid-**independent** GPU state: the four shader modules, every pipeline,
/// the particle storage buffer, the uniform buffers, and the LUT textures. None
/// of it references the accumulation field, so a size change must not touch it —
/// recompiling four WGSL modules and rebuilding four pipelines inside `render` is
/// a multi-hundred-millisecond stall, and the standalone forwards every
/// `WindowEvent::Resized`, so a live drag paid it per frame (Plan 0029 Phase 1).
struct PipelineResources {
    compute_pipeline: wgpu::ComputePipeline,
    draw_pipeline: wgpu::RenderPipeline,
    decay_pipeline: wgpu::RenderPipeline,
    present_pipeline: wgpu::RenderPipeline,
    particles: wgpu::Buffer,
    step_uniform: wgpu::Buffer,
    /// Byte stride between two slots of `step_uniform`, rounded up to the
    /// adapter's dynamic-offset alignment.
    ///
    /// Separate slots rather than one written repeatedly, because a frame encodes
    /// `pending_steps` step dispatches against one binding: folding the jitter into
    /// the step slot would apply it once per sub-step, making the disturbance a
    /// function of the frame's timing and breaking determinism. [`STEP_SLOTS`] has
    /// the same argument for the sub-steps themselves.
    step_stride: u32,
    draw_uniform: wgpu::Buffer,
    decay_uniform: wgpu::Buffer,
    compute_bg: wgpu::BindGroup,
    draw_bg: wgpu::BindGroup,
    /// The shared gradient LUT textures (A/B) the draw vertex shader samples +
    /// crossfades (ADR-0021); uploaded from the scene's baked palette on the first
    /// frame after a build and on a preset switch. They outlive a grid change, so
    /// a resize no longer re-uploads the palette.
    lut_texture_a: wgpu::Texture,
    lut_texture_b: wgpu::Texture,
    /// Kept so a grid change can rebuild [`FieldResources`]' four bind groups
    /// without recreating a layout, a sampler, or any pipeline.
    decay_layout: wgpu::BindGroupLayout,
    present_layout: wgpu::BindGroupLayout,
    field_sampler: wgpu::Sampler,
    /// How many particles the buffer above holds — the active tier's
    /// [`attractor_particles`](crate::render::TierConfig::attractor_particles),
    /// fixed for the life of these resources.
    ///
    /// Since ADR-0069 the dispatch, the instance draw and the step uniform take
    /// the **active** count instead (`round(budget * density)`), which is a
    /// different and smaller number. This one survives as the allocation bound:
    /// the draw clamps its instance range to it, so no arithmetic on `density`
    /// can fetch a vertex past the end of the buffer.
    count: u32,
}

/// The grid-**dependent** GPU state: the accumulation field and the four bind
/// groups that reference its two texture views. The only block a size change
/// rebuilds — a texture pair plus four bind groups, none of which compiles a
/// shader (Plan 0029 Phase 1).
struct FieldResources {
    /// Two-texture accumulation the trails ping-pong between (ADR-0012 reuse).
    field: PingPongField,
    /// Decay/present bind groups reading texture A / texture B — selected by the
    /// field's read side each frame so nothing is rebuilt on the hot path.
    decay_bg_a: wgpu::BindGroup,
    decay_bg_b: wgpu::BindGroup,
    present_bg_a: wgpu::BindGroup,
    present_bg_b: wgpu::BindGroup,
    /// The accumulation grid this block was built for; `render` compares the
    /// requested grid against it and rebuilds only this block on a difference.
    trail_w: u32,
    trail_h: u32,
}

impl Resources {
    fn build(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        trail_w: u32,
        trail_h: u32,
        count: u32,
    ) -> Self {
        let pipelines = PipelineResources::build(device, surface_format, count);
        let grid = FieldResources::build(device, &pipelines, trail_w, trail_h);
        Self { pipelines, grid }
    }

    /// Re-allocate the accumulation field at a new grid, reusing every pipeline,
    /// buffer, and texture that does not depend on it. The rebuilt field is
    /// undefined, so the caller re-flags the clear (and the seed upload, which
    /// keeps a capture reproducible from the same starting scatter).
    fn rebuild_grid(&mut self, device: &wgpu::Device, trail_w: u32, trail_h: u32) {
        self.grid = FieldResources::build(device, &self.pipelines, trail_w, trail_h);
    }
}

impl PipelineResources {
    fn build(device: &wgpu::Device, surface_format: wgpu::TextureFormat, count: u32) -> Self {
        let step_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("attractor-step-shader"),
            source: wgpu::ShaderSource::Wgsl(STEP_SHADER.into()),
        });
        let draw_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("attractor-draw-shader"),
            source: wgpu::ShaderSource::Wgsl(DRAW_SHADER.into()),
        });
        let decay_shader = gpu::fullscreen_shader(
            device,
            "attractor-decay-shader",
            gpu::FULLSCREEN_VS_UV_FLIPPED,
            DECAY_SHADER,
        );
        let present_shader = gpu::fullscreen_shader(
            device,
            "attractor-present-shader",
            gpu::FULLSCREEN_VS_UV_FLIPPED,
            PRESENT_SHADER,
        );

        // Particle storage buffer: written by the compute step (STORAGE), read by
        // the draw pass as an instance vertex buffer (VERTEX), seeded once from
        // the CPU (COPY_DST). One buffer, two roles — no CPU round-trip.
        //
        // `COPY_SRC` is there for [`read_particles`], the reseed test's readback
        // (Plan 0057 Phase 3). Carried unconditionally rather than behind
        // `cfg(test)` so the test exercises the buffer the app actually allocates;
        // a usage flag costs nothing that is not used, and a test running against a
        // differently-configured resource is a test of something else.
        let particles = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("attractor-particles"),
            size: (count as usize * std::mem::size_of::<Particle>()) as u64,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::VERTEX
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        // [`STEP_SLOTS`] slots in ONE buffer, selected per dispatch by a dynamic
        // offset.
        //
        // **Not two buffers behind two bind groups**, which is what this was first
        // written as and which does not survive the software adapter: a second bind
        // group sharing a live pipeline's layout gets aliased on WARP, so the step
        // dispatch read the *jitter* slot — all zeros, so `count = 0`, so every
        // invocation returned and the cloud never moved. It rendered a plausible
        // static box, moved the golden baseline, and dropped three presets to
        // ~0.000 in `animation`. One layout and one bind group has no aliasing
        // surface to get wrong.
        let step_stride = uniform_stride(device);
        let step_uniform = uniform_buffer(
            device,
            "attractor-step-uniform",
            (step_stride * STEP_SLOTS) as usize,
        );
        let draw_uniform =
            uniform_buffer(device, "attractor-draw-uniform", size_of::<DrawUniform>());
        let decay_uniform =
            uniform_buffer(device, "attractor-decay-uniform", size_of::<DecayUniform>());

        // --- compute: read_write storage + step uniform ---
        let compute_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("attractor-compute-layout"),
            entries: &[
                storage_entry(0),
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        // The sub-step slots and the jitter slot, one dispatch each.
                        has_dynamic_offset: true,
                        min_binding_size: wgpu::BufferSize::new(size_of::<StepUniform>() as u64),
                    },
                    count: None,
                },
            ],
        });
        let compute_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("attractor-compute-bg"),
            layout: &compute_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: particles.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    // A window the size of one `StepUniform`, not the whole
                    // buffer: the dynamic offset slides it between the two slots.
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &step_uniform,
                        offset: 0,
                        size: wgpu::BufferSize::new(size_of::<StepUniform>() as u64),
                    }),
                },
            ],
        });
        let compute_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("attractor-compute-pipeline-layout"),
                bind_group_layouts: &[Some(&compute_layout)],
                immediate_size: 0,
            });
        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("attractor-compute-pipeline"),
            layout: Some(&compute_pipeline_layout),
            module: &step_shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        // Shared gradient LUTs (ADR-0021): two 256×1 textures (A/B) + a repeat
        // sampler, bound to the draw pass and sampled per-particle in the vertex
        // shader (so VERTEX visibility).
        let lut_texture_a = palette::lut_texture(device, "attractor-lut-a");
        let lut_texture_b = palette::lut_texture(device, "attractor-lut-b");
        let lut_view_a = lut_texture_a.create_view(&wgpu::TextureViewDescriptor::default());
        let lut_view_b = lut_texture_b.create_view(&wgpu::TextureViewDescriptor::default());
        let lut_sampler = palette::lut_sampler(device);

        // --- draw: the particle buffer as an instance vertex buffer, additively
        // into the trail field (float target so the accumulation has headroom) ---
        let lut_vertex_texture = |binding: u32| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let draw_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("attractor-draw-layout"),
            entries: &[
                gpu::uniform(0, wgpu::ShaderStages::VERTEX),
                lut_vertex_texture(1),
                lut_vertex_texture(2),
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let draw_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("attractor-draw-bg"),
            layout: &draw_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: draw_uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&lut_view_a),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&lut_view_b),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&lut_sampler),
                },
            ],
        });
        let draw_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("attractor-draw-pipeline-layout"),
            bind_group_layouts: &[Some(&draw_layout)],
            immediate_size: 0,
        });
        let draw_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("attractor-draw-pipeline"),
            layout: Some(&draw_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &draw_shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Particle>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x3, // pos (z = 0 for 2D families)
                        1 => Float32,   // seed
                        2 => Float32x3, // prev (ADR-0069; offset 16, see `Particle`)
                    ],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &draw_shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: PingPongField::FORMAT,
                    // Additive: overlapping points bloom brighter (the dense look).
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent::OVER,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        // --- decay + present: fullscreen samples of the accumulation field ---
        // The layouts, the sampler and both pipelines are grid-independent; only
        // the bind groups that name the field's views are not, and those live in
        // `FieldResources`.
        let field_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("attractor-sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let decay_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("attractor-decay-layout"),
            entries: &[
                gpu::texture(0, true),
                gpu::sampler(1),
                gpu::uniform(2, wgpu::ShaderStages::FRAGMENT),
            ],
        });
        let decay_pipeline = gpu::fullscreen_pipeline(
            device,
            &decay_shader,
            &[&decay_layout],
            PingPongField::FORMAT,
            // The decay pass overwrites the trail field with the faded previous frame.
            wgpu::BlendState::REPLACE,
            "attractor-decay",
        );

        let present_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("attractor-present-layout"),
            entries: &[gpu::texture(0, true), gpu::sampler(1)],
        });
        let present_pipeline = gpu::fullscreen_pipeline(
            device,
            &present_shader,
            &[&present_layout],
            surface_format,
            // Premultiplied-alpha OVER the backdrop (ADR-0026): the accumulation is
            // emissive, so `c` adds over the atmosphere and the present's alpha
            // (accumulated luminance) reveals bg_* in the cloud's empty space. Over
            // the default black backdrop this equals the prior opaque present.
            wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING,
            "attractor-present",
        );

        Self {
            compute_pipeline,
            draw_pipeline,
            decay_pipeline,
            present_pipeline,
            particles,
            step_uniform,
            step_stride,
            draw_uniform,
            decay_uniform,
            compute_bg,
            draw_bg,
            lut_texture_a,
            lut_texture_b,
            decay_layout,
            present_layout,
            field_sampler,
            count,
        }
    }
}

impl FieldResources {
    /// Allocate the accumulation field at `trail_w`x`trail_h` and bind its two
    /// views into the four decay/present groups, reusing `pipelines`' layouts,
    /// sampler and decay uniform. No shader, pipeline, particle or LUT resource
    /// is created here — that is the whole point of the split.
    fn build(
        device: &wgpu::Device,
        pipelines: &PipelineResources,
        trail_w: u32,
        trail_h: u32,
    ) -> Self {
        let field = PingPongField::new(device, trail_w, trail_h);
        let decay_bg_a = blit_bind_group(
            device,
            &pipelines.decay_layout,
            "attractor-decay-bg-a",
            field.view_a(),
            &pipelines.field_sampler,
            Some(&pipelines.decay_uniform),
        );
        let decay_bg_b = blit_bind_group(
            device,
            &pipelines.decay_layout,
            "attractor-decay-bg-b",
            field.view_b(),
            &pipelines.field_sampler,
            Some(&pipelines.decay_uniform),
        );
        let present_bg_a = blit_bind_group(
            device,
            &pipelines.present_layout,
            "attractor-present-bg-a",
            field.view_a(),
            &pipelines.field_sampler,
            None,
        );
        let present_bg_b = blit_bind_group(
            device,
            &pipelines.present_layout,
            "attractor-present-bg-b",
            field.view_b(),
            &pipelines.field_sampler,
            None,
        );
        Self {
            field,
            decay_bg_a,
            decay_bg_b,
            present_bg_a,
            present_bg_b,
            trail_w,
            trail_h,
        }
    }

    /// Clear both accumulation textures to black — run once after a (re)build so
    /// the first decay pass reads a defined (empty) trail rather than garbage.
    fn clear_field(&self, encoder: &mut wgpu::CommandEncoder) {
        for view in [self.field.view_a(), self.field.view_b()] {
            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("attractor-clear-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        }
    }
}

/// Byte stride between two dynamically-offset slots of a `StepUniform`, rounded
/// up to the adapter's `min_uniform_buffer_offset_alignment` (256 on the default
/// limits). Read from the device rather than hardcoded: a dynamic offset that is
/// not a multiple of it is a validation error, and the limit is the adapter's to
/// state.
fn uniform_stride(device: &wgpu::Device) -> u32 {
    let align = device.limits().min_uniform_buffer_offset_alignment.max(1);
    size_of::<StepUniform>().next_multiple_of(align as usize) as u32
}

fn uniform_buffer(device: &wgpu::Device, label: &str, size: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: size as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn storage_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: false },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

/// A texture(+sampler)[+uniform] bind group for the decay/present fullscreen
/// passes. `uniform` is `Some` for decay (the retention factor) and `None` for
/// present (no scaling).
fn blit_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    label: &str,
    input: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
    uniform: Option<&wgpu::Buffer>,
) -> wgpu::BindGroup {
    let mut entries = vec![
        wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::TextureView(input),
        },
        wgpu::BindGroupEntry {
            binding: 1,
            resource: wgpu::BindingResource::Sampler(sampler),
        },
    ];
    if let Some(buf) = uniform {
        entries.push(wgpu::BindGroupEntry {
            binding: 2,
            resource: buf.as_entire_binding(),
        });
    }
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout,
        entries: &entries,
    })
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
    #[cfg(test)]
    fn read_particles(&self, queue: &wgpu::Queue) -> Option<Vec<([f32; 3], [f32; 3])>> {
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
            particles.iter().map(|p| (p.pos, p.prev)).collect()
        };
        staging.unmap();
        Some(out)
    }

    fn seed(family: AttractorFamily, count: u32) -> Vec<Particle> {
        let (spread, center) = family.seed_box();
        let mut rng = SeededRng::new(SEED);
        let mut particles: Vec<Particle> = (0..count)
            .map(|_| {
                let x = center[0] + rng.range(-spread[0], spread[0]);
                let y = center[1] + rng.range(-spread[1], spread[1]);
                let seed = rng.next_f32();
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

/// One frame's uniform inputs, gathered so [`upload_uniforms`] takes one argument
/// rather than fourteen. Plain values read off the scene's params.
struct UniformInputs {
    /// The **render target's** aspect, not the accumulation grid's — see the note
    /// in [`upload_uniforms`].
    aspect: f32,
    coeffs: [f32; 4],
    family: AttractorFamily,
    /// The integrated spin in spin-scaled seconds, not a wall clock — see
    /// [`advance_spin`].
    spin_time: f32,
    dt: f32,
    /// How many fixed steps this frame will encode — one uniform slot each.
    pending_steps: u32,
    /// The index of the first of them (ADR-0075's map-choice salt).
    step_index: u32,
    /// This frame's resolved IFS affine table, or [`IfsPacked::ZERO`] on a map
    /// family. Resolved by the caller because it depends on the cached morph
    /// ends rather than on the family alone.
    ifs: IfsPacked,
    /// This frame's IFS framing as `(centre, half-extent)`, sampled from the fit
    /// LUT at the current `morph`. `None` on a map family, which keeps the
    /// single world scale [`AttractorFamily::projection`] hands out.
    ifs_frame: Option<([f32; 2], [f32; 2])>,
    size: f32,
    hue: f32,
    /// The raw bound `brightness`; sanitized by [`brightness_factor`] where it is
    /// packed, not here, so the guard has exactly one site.
    brightness: f32,
    fade: f32,
    hue_spread: f32,
    hue_center: f32,
    saturation: f32,
    palette_mix: f32,
    zoom: f32,
    pan: [f32; 2],
    perspective: f32,
    depth_fade: f32,
    depth_hue: f32,
}

/// The deferred one-shot uploads: the palette LUTs on a preset switch or fresh
/// build, the field clear after a (re)build, and the seeded particle scatter on
/// first build or a `reseed` rising edge. Each clears its own flag, so none
/// repeats per frame.
#[allow(clippy::too_many_arguments)]
fn flush_deferred_uploads(
    queue: &wgpu::Queue,
    encoder: &mut wgpu::CommandEncoder,
    pipelines: &PipelineResources,
    grid: &FieldResources,
    seed_particles: &[Particle],
    palette: &Palette,
    palette_dirty: &mut bool,
    needs_clear: &mut bool,
    needs_upload: &mut bool,
) {
    // Upload the active palette LUTs (A + B) on a preset switch or a fresh
    // build — off the hot path, once per change.
    if *palette_dirty {
        palette::write_lut(queue, &pipelines.lut_texture_a, &palette.lut_a_bytes());
        palette::write_lut(queue, &pipelines.lut_texture_b, &palette.lut_b_bytes());
        *palette_dirty = false;
    }

    // Clear the trail field once after a (re)build so the first decay reads
    // black rather than garbage.
    if *needs_clear {
        grid.clear_field(encoder);
        *needs_clear = false;
    }
    // (Re)upload the seeded scatter — on first build and on a family change. A
    // `reseed` no longer comes through here: it disturbs the live cloud on the GPU
    // instead (ADR-0066), because re-uploading this array *replaced* the cloud with
    // a uniform box rather than scattering it.
    if *needs_upload {
        queue.write_buffer(
            &pipelines.particles,
            0,
            bytemuck::cast_slice(seed_particles),
        );
        *needs_upload = false;
    }
}

/// This frame's three uniform buffers: the compute step's coefficients, the
/// point draw's projection and colour, and the trail decay factor.
///
/// The step uniform is written **once per sub-step this frame owes**, into its
/// own slot, so each dispatch carries its own `step_index` — see [`STEP_SLOTS`].
/// At the steady 60 fps that is one write, exactly as before.
fn upload_uniforms(
    queue: &wgpu::Queue,
    pipelines: &PipelineResources,
    active: u32,
    inputs: &UniformInputs,
) {
    let packed = inputs.ifs;
    for slot in 0..inputs.pending_steps.min(MAX_SUBSTEPS) {
        queue.write_buffer(
            &pipelines.step_uniform,
            u64::from(pipelines.step_stride * slot),
            bytemuck::bytes_of(&StepUniform::new(
                inputs.coeffs,
                inputs.family.shader_id(),
                // The **active** count, not the allocated one: this is the bound
                // the compute step early-returns against, so it is what leaves
                // the tail beyond `density` untouched (ADR-0069).
                active,
                0,
                inputs.step_index.wrapping_add(slot),
                packed,
            )),
        );
    }
    let (scale, dim, centre) = inputs.family.projection();
    // An IFS takes its framing from the fit instead — measured over `morph` at
    // load, so the figure stays in the frame as it crosses from one figure to
    // another, and **aspect-aware**, so a wide figure fits a portrait window
    // rather than hanging out of it.
    //
    // The aspect handed in is the render **target's**, not the trail grid's
    // (ADR-0037): the present stretches the grid over the whole target, so the
    // grid's own aspect cancels and using it would draw the figure the wrong
    // width.
    let (scale, centre) = match inputs.ifs_frame {
        Some((c, half)) => (ifs::fit_scale(half, inputs.aspect), [c[0], c[1], 0.0]),
        None => (scale, centre),
    };
    let ([hx, hy, hz], [vx, vy, vz]) = inputs.family.basis().masks();
    // Off the count actually drawn, not off the tier and not off the buffer that
    // was allocated: the draw below issues `active` instances, and normalizing
    // against anything else would be a claim about a different draw call. This is
    // what carries ADR-0065's invariance across `density` (ADR-0069).
    //
    // `brightness` (ADR-0080) rides the same slot rather than a second uniform
    // field: both are scalars on the very same additive weight, and the shader
    // has no reason to tell "how many particles are sharing this light" apart
    // from "how bright the figure is". At the default the factor is exactly
    // `1.0`, so this line is the identity and no existing capture moves.
    let deposit = deposit_scale(active) * brightness_factor(inputs.brightness);
    queue.write_buffer(
        &pipelines.draw_uniform,
        0,
        bytemuck::bytes_of(&DrawUniform {
            v: [
                // The **target's** aspect, not the accumulation field's (Plan
                // 0029 Phase 5). The points are projected into the field, but
                // the present stretches the field over the whole target with
                // aspect ignored, so field NDC `x` becomes target NDC `x` and
                // the field's own aspect cancels out. Using the grid ratio was
                // harmless only while quantization was absent and the grid
                // equalled the target; with a 256 px step a 1920x1080 target
                // takes a 2048x1280 grid and drew the cloud 11% too wide.
                inputs.aspect,
                POINT_BASE * inputs.size,
                inputs.hue,
                spin_phase(inputs.spin_time),
            ],
            // `w.z` is unused since Plan 0062 — the centre grew to three
            // components and moved to `ctr` below.
            w: [scale, dim, 0.0, deposit],
            u: [
                inputs.hue_spread,
                inputs.hue_center,
                inputs.palette_mix,
                inputs.saturation,
            ],
            x: [
                inputs.zoom,
                inputs.pan[0],
                inputs.pan[1],
                if inputs.family.is_continuous() {
                    1.0
                } else {
                    0.0
                },
            ],
            bh: [hx, hy, hz, 0.0],
            bv: [vx, vy, vz, 0.0],
            d: [
                // Clamped here, silently, and not in the shader: this is the one
                // place the value crosses into the GPU, so a preset asking for
                // more gets the ceiling rather than a divisor approaching zero
                // (ADR-0076). `presets/README.md` documents that it is silent.
                inputs.perspective.clamp(0.0, MAX_PERSPECTIVE),
                // Clamped for a harder reason than a ceiling: past `1` the haze
                // multiplier goes negative, and negative light in an additive
                // accumulation *subtracts* from whatever the trail already holds.
                inputs.depth_fade.clamp(0.0, 1.0),
                // Not clamped: the palette LUT sampler repeats, so any shift is
                // a legitimate coordinate.
                inputs.depth_hue,
                inputs.family.inv_depth_extent(),
            ],
            ctr: [centre[0], centre[1], centre[2], 0.0],
        }),
    );
    // Frame-rate-independent trail decay: retain `fade` per 1/60 s, raised to
    // the `dt`-relative power so the trail length is the same wall-clock
    // duration on any refresh. `fade = 0` -> factor 0 -> trail-free.
    let decay = inputs
        .fade
        .clamp(0.0, 1.0)
        .powf((inputs.dt * 60.0).max(0.0));
    queue.write_buffer(
        &pipelines.decay_uniform,
        0,
        bytemuck::bytes_of(&DecayUniform {
            k: [decay, 0.0, 0.0, 0.0],
        }),
    );
}

/// The one-shot reseed disturbance (ADR-0066): **one** dispatch of the compute
/// pipeline in [`JITTER_MODE`], kicking every particle by a bounded family-relative
/// offset derived from its own seed and the reseed counter.
///
/// Exactly one dispatch, whatever the frame's `pending_steps` is, which is why it
/// carries its own uniform: the disturbance a preset asked for must not be a
/// function of how many fixed steps this frame happened to owe.
///
/// Nothing at all is encoded when no edge is pending — a `reseed` that never fires
/// costs one boolean test per frame.
fn encode_jitter(
    queue: &wgpu::Queue,
    encoder: &mut wgpu::CommandEncoder,
    pipelines: &PipelineResources,
    active: u32,
    family: AttractorFamily,
    reseed_count: &u32,
    pending_jitter: &mut bool,
) {
    if !*pending_jitter {
        return;
    }
    *pending_jitter = false;

    let [jx, jy, jz] = family.jitter_extent();
    let jitter_offset = pipelines.step_stride * JITTER_SLOT;
    queue.write_buffer(
        &pipelines.step_uniform,
        u64::from(jitter_offset),
        bytemuck::bytes_of(&StepUniform::new(
            [jx, jy, jz, streak_flag(RESEED_DRAWS_STREAK)],
            JITTER_MODE,
            // Active, like the step above — a reseed that kicked the inert tail
            // would move particles nothing draws, and would break the very
            // property that proves the tail is inert.
            active,
            *reseed_count,
            // The jitter is not a fixed step and draws no map — it keeps its own
            // `salt` and leaves the step counter alone.
            0,
            StepUniform::NO_IFS,
        )),
    );

    let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
        label: Some("attractor-jitter-pass"),
        timestamp_writes: None,
    });
    pass.set_pipeline(&pipelines.compute_pipeline);
    pass.set_bind_group(0, &pipelines.compute_bg, &[jitter_offset]);
    pass.dispatch_workgroups(active.div_ceil(WORKGROUP), 1, 1);
}

/// Step the particles: one compute dispatch per scheduled sub-step, each against
/// **its own** uniform slot so it carries its own `step_index` ([`STEP_SLOTS`]).
/// wgpu inserts the storage-to-vertex barrier before the draw pass that follows.
fn encode_steps(
    encoder: &mut wgpu::CommandEncoder,
    pipelines: &PipelineResources,
    active: u32,
    pending_steps: u32,
) {
    let groups = active.div_ceil(WORKGROUP);
    // `FixedStep` already clamps to `MAX_SUBSTEPS`; clamped again because a
    // dispatch past the slots written above would read an undefined slot.
    for slot in 0..pending_steps.min(MAX_SUBSTEPS) {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("attractor-step-pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipelines.compute_pipeline);
        pass.set_bind_group(0, &pipelines.compute_bg, &[pipelines.step_stride * slot]);
        pass.dispatch_workgroups(groups, 1, 1);
    }
}

/// Trail pass: draw the faded previous accumulation into the fresh target, then
/// add this frame's points on top. **One** pass, so the decay lays the bed and the
/// additive points bloom over it — splitting it in two would clear the bed away.
/// Reads the field's current read side; the caller swaps afterwards.
fn encode_trail_pass(
    encoder: &mut wgpu::CommandEncoder,
    pipelines: &PipelineResources,
    active: u32,
    grid: &FieldResources,
) {
    let decay_bg = if grid.field.reading_a() {
        &grid.decay_bg_a
    } else {
        &grid.decay_bg_b
    };
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("attractor-trail-pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: grid.field.write_view(),
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    pass.set_pipeline(&pipelines.decay_pipeline);
    pass.set_bind_group(0, decay_bg, &[]);
    pass.draw(0..3, 0..1);

    pass.set_pipeline(&pipelines.draw_pipeline);
    pass.set_bind_group(0, &pipelines.draw_bg, &[]);
    pass.set_vertex_buffer(0, pipelines.particles.slice(..));
    // Clamped to what was allocated rather than trusted: `active` is derived from
    // a preset-supplied `density`, and an instance range past the buffer is an
    // out-of-bounds vertex fetch. `active_particles` already bounds it, so this
    // costs nothing and removes the possibility rather than documenting it.
    pass.draw(0..6, 0..active.min(pipelines.count));
}

/// Present the freshly-written accumulation to the target, loading over whatever
/// the engine backdrop painted (ADR-0018). Call **after** the swap.
fn encode_present(
    encoder: &mut wgpu::CommandEncoder,
    pipelines: &PipelineResources,
    grid: &FieldResources,
    view: &wgpu::TextureView,
) {
    let present_bg = if grid.field.reading_a() {
        &grid.present_bg_a
    } else {
        &grid.present_bg_b
    };
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("attractor-present-pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                // Load over the engine backdrop (ADR-0018): the additive
                // point cloud blooms over whatever the background pass painted.
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    pass.set_pipeline(&pipelines.present_pipeline);
    pass.set_bind_group(0, present_bg, &[]);
    pass.draw(0..3, 0..1);
}

#[cfg(test)]
mod tests {
    // Tests panic on failure; allowed over the file's hot-path pragma — this is
    // not the render path.
    #![allow(clippy::panic, clippy::expect_used)]

    use super::{
        AttractorFamily, AttractorScene, Basis, DEFAULT_BRIGHTNESS, DEFAULT_DEPTH_FADE,
        DEFAULT_DEPTH_HUE, DEFAULT_SPIN, FIXED_STEP, JITTER_MODE, MAX_PERSPECTIVE,
        MIN_PARTICLE_DENSITY, Particle, RESEED_DRAWS_STREAK, SPIN_RATE, STEP_SLOTS, Scene,
        StepUniform, active_particles, advance_spin, brightness_factor, deposit_scale, ifs,
        projection_mirror, spin_phase, streak_flag,
    };
    use crate::dsp::AnalysisFrame;
    use crate::render::context::RenderContext;
    use crate::render::{Tier, TierConfig};

    // -----------------------------------------------------------------------
    // The IFS family (Plan 0062 Phase 1 / ADR-0075)
    // -----------------------------------------------------------------------

    /// Every curated figure reaches the enum through the **same** `[particles]
    /// family` key the map families use, and an unknown name is still rejected —
    /// which is what makes it a load error rather than a silent De Jong.
    #[test]
    fn a_figure_name_selects_the_ifs_family() {
        for figure in ifs::IfsFigure::ALL {
            let name = figure.name();
            assert_eq!(
                AttractorFamily::from_name(name),
                Some(AttractorFamily::Ifs(figure)),
                "'{name}' must select {figure:?}"
            );
        }
        // The map families are untouched, and nothing else parses.
        assert_eq!(
            AttractorFamily::from_name("de_jong"),
            Some(AttractorFamily::DeJong)
        );
        for unknown in ["ifs", "barnsley", "fern_", ""] {
            assert_eq!(AttractorFamily::from_name(unknown), None);
        }
    }

    /// **The jitter selector must not collide with a real family's**, and the IFS
    /// is what moved it (from 4 to 5).
    ///
    /// This is the failure that would be invisible in a unit test and obvious
    /// only in a render: a family sharing the jitter's id takes the jitter arm,
    /// so the cloud is kicked every step and never iterates its map at all.
    #[test]
    fn the_jitter_selector_sits_past_every_family() {
        let families = [
            AttractorFamily::DeJong,
            AttractorFamily::Clifford,
            AttractorFamily::Thomas,
            AttractorFamily::Lorenz,
            AttractorFamily::Ifs(ifs::IfsFigure::Fern),
        ];
        for family in families {
            assert!(
                family.shader_id() < JITTER_MODE,
                "{family:?} shares or exceeds the jitter selector {JITTER_MODE}"
            );
        }
        // Every family is a distinct arm, or two of them draw the same figure.
        let mut ids: Vec<u32> = families.iter().map(|f| f.shader_id()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), families.len(), "two families share a shader id");
        // ...and every figure is the *same* arm, because the figure is data.
        for figure in ifs::IfsFigure::ALL {
            assert_eq!(AttractorFamily::Ifs(figure).shader_id(), 4);
        }
    }

    /// The step uniform's shape, pinned where ADR-0075 quotes it.
    ///
    /// A Rust/WGSL layout disagreement fails loudly at pipeline creation (wgpu
    /// compares the struct size against `min_binding_size`), so this is not the
    /// safety net — it is the record of *which* number the two agree on, and of
    /// the one place the ADR's arithmetic was off: 160 rather than 144, because
    /// `step_index` forced the scalar block up to the next multiple of 16.
    #[test]
    fn the_step_uniform_carries_the_ifs_table_in_one_binding() {
        assert_eq!(size_of::<StepUniform>(), 160);
        // A slot per possible sub-step plus the jitter's — the property
        // `encode_steps` relies on when it offsets by the sub-step index.
        assert_eq!(STEP_SLOTS, super::MAX_SUBSTEPS + 1);
        assert_eq!(super::JITTER_SLOT, super::MAX_SUBSTEPS);
    }

    /// The IFS payload is inert for the four map families — asserted on the value
    /// rather than left to the shader's family branch, because "they never read
    /// it" is a claim about code that is easy to break.
    #[test]
    fn the_map_families_upload_no_affine_table() {
        for family in [
            AttractorFamily::DeJong,
            AttractorFamily::Clifford,
            AttractorFamily::Thomas,
            AttractorFamily::Lorenz,
        ] {
            assert_eq!(family.figure(), None);
        }
        assert_eq!(
            AttractorFamily::Ifs(ifs::IfsFigure::Fern).figure(),
            Some(ifs::IfsFigure::Fern)
        );
        assert_eq!(StepUniform::NO_IFS, ifs::IfsPacked::ZERO);
        // Zeroed means *every* cumulative entry is zero, so a stray dispatch on
        // the IFS arm would pick the fourth map and apply an all-zero affine —
        // a single point at the origin, not a second figure.
        assert!(StepUniform::NO_IFS.cumulative_p.iter().all(|c| *c == 0.0));
    }

    /// ADR-0065's invariance, asserted **on the scalar** rather than inferred from
    /// pixels — which is the whole reason the decision is expressible as one.
    ///
    /// The `Floor` half is why no golden baseline moves: `1.0` is not a tolerance
    /// or a near-miss, it is exact, so every baseline blessed at the floor renders
    /// the same arithmetic it always did.
    #[test]
    fn the_deposit_scalar_is_exactly_one_at_the_floor_and_a_third_at_rich() {
        let floor = TierConfig::for_tier(Tier::Floor).attractor_particles;
        let rich = TierConfig::for_tier(Tier::Rich).attractor_particles;

        assert_eq!(
            deposit_scale(floor),
            1.0,
            "the floor factor must be exactly 1.0 — every golden is blessed there"
        );
        // 50 000 / 150 000 at the shipped counts. Asserted against the ratio the
        // tier table actually holds rather than a literal 1/3, so a re-calibrated
        // `Rich` (Plan 0044 Phase 4 has never run) changes this test's expectation
        // with it instead of failing for the wrong reason.
        let expected = floor as f32 / rich as f32;
        assert_eq!(deposit_scale(rich), expected);
        assert!(
            (deposit_scale(rich) - 1.0 / 3.0).abs() < 1e-6,
            "at the counts shipped today that ratio is 1/3, got {}",
            deposit_scale(rich)
        );

        // Non-vacuity: the two tiers must actually differ, or the assertions above
        // hold for a table where nothing is normalized at all.
        assert!(
            rich > floor,
            "the rich tier is meant to raise the count ({rich} vs {floor})"
        );

        // Total deposited light is what is invariant, and that is the product.
        // Stated directly because it is the property, and the scalar is only the
        // means: three times the samples at a third the weight is the same light.
        let total = |count: u32| count as f32 * deposit_scale(count);
        assert_eq!(total(floor), total(rich));
    }

    /// ADR-0080's identity claim, asserted on the scalar for the same reason
    /// ADR-0065's is: it is what makes every existing golden baseline
    /// **byte-identical** rather than approximately unchanged. A multiply by
    /// literal `1.0` is exact in IEEE-754, and this is where that literal is.
    #[test]
    fn the_brightness_factor_is_exactly_one_by_default_and_scales_linearly() {
        assert_eq!(
            brightness_factor(DEFAULT_BRIGHTNESS),
            1.0,
            "the default must be an exact 1.0 — no golden baseline may move"
        );
        assert_eq!(brightness_factor(0.5), 0.5);
        assert_eq!(brightness_factor(2.0), 2.0);
        // The whole product, at the floor tier where every baseline is blessed:
        // half the brightness is exactly half the deposit.
        let floor = TierConfig::for_tier(Tier::Floor).attractor_particles;
        assert_eq!(
            deposit_scale(floor) * brightness_factor(0.5),
            deposit_scale(floor) * 0.5
        );
    }

    /// The two guards on that factor. Both matter more here than for an ordinary
    /// param because the value lands in an accumulation the trail carries forward:
    /// a single poisoned frame is a permanently poisoned field.
    #[test]
    fn the_brightness_factor_refuses_negative_and_non_finite_bindings() {
        // Negative light would *subtract* from the additive accumulation.
        assert_eq!(brightness_factor(-1.0), 0.0);
        assert_eq!(brightness_factor(-0.0), 0.0);
        // NaN and the infinities fall back rather than reaching the uniform: an
        // `inf` deposit survives every later decay multiply.
        assert_eq!(brightness_factor(f32::NAN), DEFAULT_BRIGHTNESS);
        assert_eq!(brightness_factor(f32::INFINITY), DEFAULT_BRIGHTNESS);
        assert_eq!(brightness_factor(f32::NEG_INFINITY), DEFAULT_BRIGHTNESS);
    }

    /// A zero-particle scene draws nothing, so the factor is unobservable — but it
    /// must not be an infinity on its way to a shader uniform.
    #[test]
    fn the_deposit_scalar_survives_a_degenerate_count() {
        assert!(deposit_scale(0).is_finite());
        // And it is still monotone the right way round: fewer particles, more
        // weight each, which is what keeps the total constant.
        assert!(deposit_scale(25_000) > deposit_scale(50_000));
        assert!(deposit_scale(50_000) > deposit_scale(100_000));
    }

    // -----------------------------------------------------------------------
    // The projection basis (Plan 0059 Phase 1 / ADR-0068)
    // -----------------------------------------------------------------------

    /// The basis, pinned per family against an explicit table.
    ///
    /// The **compiler** is what stops a fifth family being added without choosing
    /// one — `basis()` matches exhaustively with no wildcard arm. This test pins
    /// the values that match resolves to, so a silent flip of Lorenz back to the
    /// shared convention fails here rather than in someone's eyes.
    #[test]
    fn the_projection_basis_is_pinned_per_family() {
        let table = [
            (AttractorFamily::DeJong, Basis::XY),
            (AttractorFamily::Clifford, Basis::XY),
            (AttractorFamily::Thomas, Basis::XY),
            (AttractorFamily::Lorenz, Basis::XZ),
        ];
        for (family, expected) in table {
            assert_eq!(
                family.basis(),
                expected,
                "{family:?} must be viewed in {expected:?}"
            );
        }

        // Non-vacuity: exactly one family departs from the shared convention, so
        // the table above is not four copies of one answer.
        let departures = table.iter().filter(|(_, b)| *b != Basis::XY).count();
        assert_eq!(
            departures, 1,
            "ADR-0068 moves exactly one family's basis; {departures} differ from XY"
        );

        // And the departure is not derived from `dim`. Thomas is 3D and keeps
        // x–y — which is the whole reason ADR-0068 Alternative C was declined,
        // so it is asserted rather than left to the doc comment.
        assert_eq!(AttractorFamily::Thomas.projection().1, 3.0);
        assert_eq!(AttractorFamily::Thomas.basis(), Basis::XY);
        assert_eq!(AttractorFamily::Lorenz.projection().1, 3.0);
        assert_ne!(
            AttractorFamily::Thomas.basis(),
            AttractorFamily::Lorenz.basis(),
            "two 3D families disagree about the basis, so `dim == 3.0` cannot decide it"
        );
    }

    /// The masks the shader dots against, asserted as the geometry they encode.
    ///
    /// `XY` must reproduce the pre-ADR-0068 expression exactly — `cx*cs + cz*sn`
    /// vertical `y` — because three families' captures have to stay
    /// byte-identical, and that claim rests on these six numbers.
    #[test]
    fn the_basis_masks_select_the_axes_they_name() {
        // A position with distinguishable components, so a mask that picks the
        // wrong axis cannot agree by coincidence. Destructured rather than
        // indexed — this file denies `indexing_slicing`.
        let [px, py, pz] = [2.0f32, 3.0, 5.0];
        let dot = |[mx, my, mz]: [f32; 3]| px * mx + py * my + pz * mz;

        let (h, v) = Basis::XY.masks();
        assert_eq!(dot(h), 5.0, "XY rotates x against z");
        assert_eq!(dot(v), 3.0, "XY is vertical in y");

        let (h, v) = Basis::XZ.masks();
        assert_eq!(dot(h), 3.0, "XZ rotates x against y");
        assert_eq!(dot(v), 5.0, "XZ is vertical in z");

        // Each selector is one-hot and never picks x — the spin's first
        // horizontal term is always `x`, so a mask with an x component would
        // double-count it.
        for basis in [Basis::XY, Basis::XZ] {
            let (h, v) = basis.masks();
            for m in [h, v] {
                let [mx, ..] = m;
                assert_eq!(mx, 0.0, "{basis:?} selector must not pick x: {m:?}");
                assert_eq!(m.iter().filter(|c| **c != 0.0).count(), 1);
                assert_eq!(m.iter().sum::<f32>(), 1.0);
            }
            // ...and the two selectors are different axes, or the projection
            // collapses onto a line.
            assert_ne!(
                h, v,
                "{basis:?} put the horizontal and vertical on one axis"
            );
        }
    }

    // -----------------------------------------------------------------------
    // The view depth (Plan 0063 Phase 1 / ADR-0076)
    // -----------------------------------------------------------------------

    /// The inverse depth extent, pinned per family against an explicit table.
    ///
    /// The compiler forces a fifth family to answer — the match is exhaustive.
    /// This pins *what* it answers, and above all that the 2D families answer
    /// **exactly zero**: every depth cue in the shader is the identity there by
    /// arithmetic rather than by a branch, so a non-zero value here would give
    /// De Jong and Clifford a depth they do not have.
    #[test]
    fn the_depth_extent_is_zero_for_every_flat_family() {
        // The half-extents ADR-0076 quotes, as reciprocals — derived from
        // `seed_box`, so this fails if that table moves without the ADR.
        let table = [
            (AttractorFamily::DeJong, 0.0),
            (AttractorFamily::Clifford, 0.0),
            (AttractorFamily::Thomas, 1.0 / 4.5),
            (AttractorFamily::Lorenz, 1.0 / 26.0),
        ];
        for (family, expected) in table {
            assert_eq!(
                family.inv_depth_extent(),
                expected,
                "{family:?}'s inverse depth extent must be exactly {expected}"
            );
        }

        // The flat families' zero is an *exact* zero, not a small number: the
        // shader multiplies by it and clamps, so anything else is a depth.
        for flat in [AttractorFamily::DeJong, AttractorFamily::Clifford] {
            assert_eq!(flat.inv_depth_extent(), 0.0);
            assert!(flat.inv_depth_extent().is_finite());
        }
        // Non-vacuity: the 3D families genuinely carry one, and the two differ —
        // so this is a per-family derivation and not one shared constant.
        assert!(AttractorFamily::Thomas.inv_depth_extent() > 0.0);
        assert!(AttractorFamily::Lorenz.inv_depth_extent() > 0.0);
        assert_ne!(
            AttractorFamily::Thomas.inv_depth_extent(),
            AttractorFamily::Lorenz.inv_depth_extent()
        );
    }

    /// The identity every one of these assertions is stated against: the rest
    /// rotation (`cs = 1, sn = 0`) and the half turn (`cs = -1, sn = 0`).
    ///
    /// Written as exact components rather than as `cos(0.0)` and `cos(PI)`,
    /// because `f32::consts::PI` is not `π` and `cos` of it is `-1.0` with a
    /// `sin` of `-8.7e-8` — near enough to look right and far enough to make an
    /// *exact* equality fail for a reason that has nothing to do with the claim.
    const REST: (f32, f32) = (1.0, 0.0);
    const HALF_TURN: (f32, f32) = (-1.0, 0.0);

    /// Positions on the two 3D figures, each with a genuinely non-zero depth at
    /// rest (the depth at `cs = 1, sn = 0` is `dot(p, bh)`, so the component the
    /// family's basis selects must not be zero).
    fn depth_samples(family: AttractorFamily) -> [[f32; 3]; 4] {
        match family {
            // Lorenz: basis XZ, so `bh` is y and the depth at rest is `y`.
            AttractorFamily::Lorenz => [
                [10.0, 12.0, 30.0],
                [-14.0, -20.0, 40.0],
                [3.0, 25.0, 10.0],
                [19.0, -5.0, 45.0],
            ],
            // Thomas: basis XY, so `bh` is z and the depth at rest is `z`.
            _ => [
                [2.0, -3.0, 1.5],
                [-4.0, 1.0, -2.5],
                [0.5, 3.5, 4.0],
                [-1.5, -0.5, -3.5],
            ],
        }
    }

    /// **ADR-0076's diagnosis and its fix, as one dimensionless property.**
    ///
    /// Under orthography the projection at rotation `π` is the exact `x`-mirror
    /// of the projection at `0` — at `cs = -1, sn = 0` the horizontal term
    /// becomes `-p.x` and the vertical term is untouched. That is why a rotating
    /// transparent structure carries no information about *which way* it is
    /// turning, and with additive blending there is no occlusion to break the
    /// tie either: the percept flips and settles on "flat".
    ///
    /// Under perspective it is not a mirror, because `m(h) != m(-h)` for any
    /// `h != 0`. Both halves are asserted here, exactly, on the formula — a
    /// capture could only report that the picture changed.
    #[test]
    fn perspective_breaks_the_orthographic_mirror() {
        for family in [AttractorFamily::Lorenz, AttractorFamily::Thomas] {
            for q in depth_samples(family) {
                // The premise: this sample must actually have depth, or every
                // assertion below is about `m(0) = m(0)`.
                let rest = projection_mirror::project(q, family, REST.0, REST.1);
                let dn = projection_mirror::depth_norm(rest.depth, family.inv_depth_extent());
                assert!(
                    dn.abs() > 0.05,
                    "{family:?} sample {q:?} sits at depth {dn} — too near the view plane to \
                     distinguish a perspective divide from an orthographic one"
                );

                // --- the flatness, pinned ---
                let flat_rest = projection_mirror::world(q, family, REST.0, REST.1, 0.0);
                let flat_half = projection_mirror::world(q, family, HALF_TURN.0, HALF_TURN.1, 0.0);
                let ([rx, ry], [hx, hy]) = (flat_rest, flat_half);
                assert_eq!(
                    hx, -rx,
                    "{family:?} sample {q:?}: at perspective 0 the half turn must be the exact \
                     x-mirror of the rest pose"
                );
                assert_eq!(
                    hy, ry,
                    "{family:?} sample {q:?}: the vertical must not move"
                );

                // --- and the fix, pinned ---
                const P: f32 = 0.5;
                let deep_rest = projection_mirror::world(q, family, REST.0, REST.1, P);
                let deep_half = projection_mirror::world(q, family, HALF_TURN.0, HALF_TURN.1, P);
                let ([drx, dry], [dhx, dhy]) = (deep_rest, deep_half);
                assert_ne!(
                    dhx, -drx,
                    "{family:?} sample {q:?}: at perspective {P} the half turn is STILL the \
                     x-mirror of the rest pose — the depth is not reaching the magnification, so \
                     the rotation is as ambiguous as it was"
                );
                // The vertical breaks too, and that is worth stating separately:
                // the magnification scales the whole projected position, so a
                // half turn moves the figure toward or away from the camera
                // rather than merely flipping it.
                assert_ne!(
                    dhy, dry,
                    "{family:?} sample {q:?}: the vertical is unchanged by the half turn under \
                     perspective — the magnification is not being applied to it"
                );
            }
        }
    }

    /// **The flat families are untouched at every `perspective`.**
    ///
    /// Stated as invariance rather than as the mirror identity above, and the
    /// difference is not pedantry: a 2D map's projection is a full *in-plane*
    /// rotation, so its half turn is a point reflection (both axes negated), not
    /// an `x`-mirror — that identity was never true for them and is not what
    /// this change is about. What must hold is that the depth machinery is
    /// **exactly the identity** here, which is what `inv_depth_extent() == 0`
    /// buys: same bits at every perspective, including the ceiling.
    #[test]
    fn perspective_is_exactly_inert_on_a_flat_family() {
        for family in [AttractorFamily::DeJong, AttractorFamily::Clifford] {
            for q in [
                [1.2f32, -0.7, 0.0],
                [-1.5, 1.4, 0.0],
                [0.3, 0.9, 0.0],
                // A stray `z`, which a 2D particle never has — but if one did,
                // it must still not become a depth.
                [0.8, -1.1, 5.0],
            ] {
                for (cs, sn) in [REST, HALF_TURN, (0.6, 0.8)] {
                    let base = projection_mirror::world(q, family, cs, sn, 0.0);
                    for p in [0.25, 0.5, MAX_PERSPECTIVE] {
                        assert_eq!(
                            projection_mirror::world(q, family, cs, sn, p),
                            base,
                            "{family:?} sample {q:?} moved at perspective {p} — a flat family \
                             must have no depth to spend"
                        );
                    }
                }
                // ...and the in-plane rotation is what it always was: a half turn
                // negates both axes exactly.
                let ([rx, ry], [hx, hy]) = (
                    projection_mirror::world(q, family, REST.0, REST.1, 0.0),
                    projection_mirror::world(q, family, HALF_TURN.0, HALF_TURN.1, 0.0),
                );
                assert_eq!((hx, hy), (-rx, -ry));
            }
        }
    }

    /// The magnification's arithmetic, which is what makes `perspective` legible
    /// rather than magic: it means the figure's depth half-extent as a fraction
    /// of the camera distance, so the near-to-far ratio is `(1 + p) / (1 - p)`.
    #[test]
    fn the_magnification_matches_the_documented_ratio() {
        for (p, expected) in [(0.0, 1.0), (0.5, 3.0), (MAX_PERSPECTIVE, 9.0)] {
            let near = projection_mirror::magnify(1.0, p);
            let far = projection_mirror::magnify(-1.0, p);
            assert!(
                (near / far - expected).abs() < 1e-5,
                "perspective {p} gives a near/far ratio of {:.4}, not the documented {expected}",
                near / far
            );
        }
        // The two ends ADR-0076 quotes at the ceiling.
        assert!((projection_mirror::magnify(1.0, MAX_PERSPECTIVE) - 5.0).abs() < 1e-5);
        assert!((projection_mirror::magnify(-1.0, MAX_PERSPECTIVE) - 0.5556).abs() < 1e-3);

        // At `perspective = 0` it is **exactly** 1.0 — not nearly. A multiply by
        // exactly 1.0 is an identity in IEEE arithmetic, which is what makes the
        // default byte-identical rather than merely close.
        for dn in [-1.0f32, -0.37, 0.0, 0.42, 1.0] {
            assert_eq!(projection_mirror::magnify(dn, 0.0), 1.0);
        }

        // The clamp keeps the divisor away from the singularity at `p = 1`: a
        // converged figure overruns its seed box, so `d_n` before clamping
        // reaches past 1 and an unclamped magnification would blow up.
        assert_eq!(projection_mirror::depth_norm(100.0, 1.0 / 26.0), 1.0);
        assert_eq!(projection_mirror::depth_norm(-100.0, 1.0 / 26.0), -1.0);
        // A flat family's zero extent survives even an absurd depth.
        assert_eq!(projection_mirror::depth_norm(1e30, 0.0), 0.0);
    }

    // -----------------------------------------------------------------------
    // Atmosphere (Plan 0063 Phase 2 / ADR-0076)
    // -----------------------------------------------------------------------

    /// **Far material is dimmer than near material**, measured on the
    /// per-particle multiplier across a sampled depth range.
    ///
    /// Not on pixels, and not because pixels are inconvenient: which screen
    /// region holds the far material depends on the spin phase, so a frame's
    /// "far half" is not a fixed set of columns and an assertion about one would
    /// be reading the clock. The multiplier is where the decision lives.
    #[test]
    fn distance_dims_the_far_material() {
        const FADE: f32 = 0.8;
        const SAMPLES: usize = 33;

        // Sample the depth range end to end. `dn = -1` is farthest, `+1` nearest.
        let dn_at = |i: usize| i as f32 / (SAMPLES - 1) as f32 * 2.0 - 1.0;
        let mean = |range: std::ops::Range<usize>| -> f32 {
            let n = range.len() as f32;
            range
                .map(|i| projection_mirror::haze(dn_at(i), FADE))
                .sum::<f32>()
                / n
        };
        let far = mean(0..SAMPLES / 2);
        let near = mean(SAMPLES / 2 + 1..SAMPLES);
        println!("mean haze at depth_fade {FADE}: far half {far:.4}, near half {near:.4}");
        assert!(
            far < near * 0.8,
            "the far half is not measurably dimmer than the near half ({far:.4} against \
             {near:.4}) — `depth_fade` is not attenuating with distance"
        );

        // Monotone the whole way, not merely lower on average: a cue that
        // brightened anywhere in the middle would still pass the halves test.
        for i in 1..SAMPLES {
            let (prev, now) = (
                projection_mirror::haze(dn_at(i - 1), FADE),
                projection_mirror::haze(dn_at(i), FADE),
            );
            assert!(
                now > prev,
                "haze is not monotone in depth: {prev:.5} at d_n {:.3}, {now:.5} at {:.3}",
                dn_at(i - 1),
                dn_at(i)
            );
        }

        // The two ends the parameter is documented by: `depth_fade = 1` takes the
        // far end to black and leaves the near end untouched.
        assert_eq!(projection_mirror::haze(-1.0, 1.0), 0.0);
        assert_eq!(projection_mirror::haze(1.0, 1.0), 1.0);
        // Never negative anywhere in the clamped range — a negative deposit would
        // subtract light from the additive accumulation.
        for i in 0..SAMPLES {
            assert!(projection_mirror::haze(dn_at(i), 1.0) >= 0.0);
        }
    }

    /// Both cues are **exactly** the identity at their defaults, which is why no
    /// existing capture moves. Exact, not approximate: `1.0 - 0.0 * x` is `1.0`
    /// and a multiply by it is an IEEE identity, and `0.0 * x` added to a
    /// coordinate leaves its bits alone.
    #[test]
    fn the_atmosphere_is_off_by_default() {
        for dn in [-1.0f32, -0.5, 0.0, 0.25, 1.0] {
            assert_eq!(projection_mirror::haze(dn, DEFAULT_DEPTH_FADE), 1.0);
            assert_eq!(projection_mirror::depth_tint(dn, DEFAULT_DEPTH_HUE), 0.0);
            // And on a flat family the cues are inert whatever they are set to,
            // because `d_n` is identically zero there — so both land on the
            // mid-depth value and no De Jong particle can be tinted or dimmed.
            let flat =
                projection_mirror::depth_norm(1e6, AttractorFamily::DeJong.inv_depth_extent());
            assert_eq!(projection_mirror::haze(flat, 1.0), 0.5);
            assert_eq!(projection_mirror::depth_tint(flat, 1.0), 0.0);
        }
        assert_eq!(DEFAULT_DEPTH_FADE, 0.0);
        assert_eq!(DEFAULT_DEPTH_HUE, 0.0);
    }

    /// The hue shift spans `±depth_hue/2` across the depth range and is centred,
    /// so the mid-depth colour is the one the preset asked for — a cue that
    /// merely *tinted* the whole picture would be a `hue` offset with extra
    /// steps.
    #[test]
    fn the_depth_tint_is_centred_and_spans_the_parameter() {
        const HUE: f32 = 0.3;
        assert!((projection_mirror::depth_tint(1.0, HUE) - HUE / 2.0).abs() < 1e-6);
        assert!((projection_mirror::depth_tint(-1.0, HUE) + HUE / 2.0).abs() < 1e-6);
        assert_eq!(projection_mirror::depth_tint(0.0, HUE), 0.0);
        let span =
            projection_mirror::depth_tint(1.0, HUE) - projection_mirror::depth_tint(-1.0, HUE);
        assert!(
            (span - HUE).abs() < 1e-6,
            "the tint spans {span} across the depth range, not the {HUE} it was asked for"
        );
    }

    // -----------------------------------------------------------------------
    // The spin (Plan 0063 Phase 3 / ADR-0076)
    // -----------------------------------------------------------------------

    /// A `spin` sequence that runs a while, then changes — a second of turning
    /// at the shipped rate, then double, then a reversal, half a second each.
    ///
    /// **The length is the point.** The product form's error is proportional to
    /// *elapsed* time, so a sequence that changes on frame 3 barely shows it; a
    /// sequence that changes after a second of accumulated rotation shows it as
    /// the figure jumping a fifth of a revolution between two frames.
    fn spin_ramp() -> Vec<f32> {
        let mut ramp = vec![1.0f32; 60];
        ramp.extend(std::iter::repeat_n(2.0f32, 30));
        ramp.extend(std::iter::repeat_n(-1.0f32, 30));
        ramp
    }

    /// **The phase is the running sum, and provably not the product.**
    ///
    /// Under `time · spin · SPIN_RATE` a `spin` that changed between frames would
    /// retroactively rescale *all* elapsed time, so the figure would snap to a
    /// new angle rather than accelerate toward one. Both halves are asserted:
    /// the integral matches term for term, and the product does not — computed
    /// here rather than argued about.
    #[test]
    fn the_spin_phase_integrates_rather_than_multiplying() {
        let dt = FIXED_STEP;
        let mut spin_time = 0.0f32;
        let mut running = 0.0f32;
        let mut elapsed = 0.0f32;
        let mut worst_integrated_step = 0.0f32;
        let mut worst_multiplied_step = 0.0f32;
        let mut prev_multiplied = 0.0f32;

        let ramp = spin_ramp();
        for spin in ramp.iter().copied() {
            let before = spin_phase(spin_time);
            spin_time = advance_spin(spin_time, spin, dt);
            running += spin * SPIN_RATE * dt;
            elapsed += dt;

            // The integrated phase moves by at most one frame's worth of angle,
            // whatever the binding did. That is what "accelerates" means.
            worst_integrated_step =
                worst_integrated_step.max((spin_phase(spin_time) - before).abs());

            // The rejected form, evaluated alongside it.
            let multiplied = elapsed * spin * SPIN_RATE;
            worst_multiplied_step = worst_multiplied_step.max((multiplied - prev_multiplied).abs());
            prev_multiplied = multiplied;
        }

        assert!(
            (spin_phase(spin_time) - running).abs() < 1e-6,
            "the phase is {} but the running sum of spin * SPIN_RATE * dt is {running}",
            spin_phase(spin_time)
        );

        // ...and it is NOT the product. Not a near miss: the two disagree by more
        // than the whole integrated phase.
        let multiplied = elapsed * ramp.last().copied().unwrap_or(0.0) * SPIN_RATE;
        println!(
            "after {} frames: integrated {:.6} rad, multiplied {multiplied:.6} rad",
            ramp.len(),
            spin_phase(spin_time)
        );
        assert!(
            (spin_phase(spin_time) - multiplied).abs() > spin_phase(spin_time).abs(),
            "the integrated phase {} and the multiplied one {multiplied} agree — this test \
             cannot tell the two formulations apart",
            spin_phase(spin_time)
        );

        // The snap, stated as the thing a viewer would see. The largest angle one
        // frame can honestly turn through is `max|spin| * SPIN_RATE * dt`; the
        // integrated phase never exceeds it, and the product form leaps a large
        // multiple of it the instant the binding moves — because it rescales a
        // second of already-elapsed rotation.
        let frame_angle = ramp.iter().fold(0.0f32, |m, s| m.max(s.abs())) * SPIN_RATE * dt;
        println!(
            "worst single-frame phase jump: integrated {worst_integrated_step:.6}, \
             multiplied {worst_multiplied_step:.6} (one frame of rotation is {frame_angle:.6})"
        );
        assert!(
            // A relative slack, because this compares two `f32` roundings of the
            // same product, which differ in the last bits.
            worst_integrated_step <= frame_angle * (1.0 + 1e-5),
            "the integrated phase jumped {worst_integrated_step} in one frame, past the \
             {frame_angle} a frame's rotation can be"
        );
        assert!(
            worst_multiplied_step > frame_angle * 20.0,
            "the multiplied form's worst jump is only {worst_multiplied_step} against a frame's \
             {frame_angle} — this sequence does not exercise the retroactive rescale the \
             integration exists to avoid"
        );
    }

    /// `spin = 1` is **exactly** the rate this scene shipped with, and `spin = 0`
    /// holds the figure still — the two ends the parameter is documented by.
    ///
    /// The exactness is the load-bearing half. At `spin = 1` the accumulator is
    /// `Σ dt` term for term, which is bit-for-bit the renderer's own clock, so
    /// the phase equals the `time * SPIN_RATE` it replaced and no golden baseline
    /// moves. Deferring the `SPIN_RATE` multiply to [`spin_phase`] is what buys
    /// that; summing `spin * SPIN_RATE * dt` would drift in the last bits.
    #[test]
    fn the_default_spin_reproduces_the_shipped_rate_exactly() {
        let dt = FIXED_STEP;
        let (mut spin_time, mut clock) = (0.0f32, 0.0f32);
        for _ in 0..600 {
            spin_time = advance_spin(spin_time, DEFAULT_SPIN, dt);
            // The renderer's own accumulation, verbatim (`self.time += dt`).
            clock += dt;
            assert_eq!(
                spin_phase(spin_time),
                clock * SPIN_RATE,
                "the integrated phase has drifted from the clock-multiplied one it replaced"
            );
        }
        assert_eq!(DEFAULT_SPIN, 1.0);

        // `spin = 0` holds the angle fixed, exactly, however long it runs.
        let mut held = 0.0f32;
        for _ in 0..600 {
            held = advance_spin(held, 0.0, dt);
        }
        assert_eq!(spin_phase(held), 0.0, "spin = 0 must hold the figure still");

        // ...and negative reverses it, rather than being clamped away.
        let mut back = 0.0f32;
        for _ in 0..60 {
            back = advance_spin(back, -1.0, dt);
        }
        assert!(
            spin_phase(back) < 0.0,
            "a negative spin must turn the other way"
        );
    }

    /// The scene is actually wired to the integration — asserted on the scene's
    /// own accumulator across rendered frames, since the two tests above prove
    /// the arithmetic and not that anything calls it.
    #[test]
    fn the_scene_integrates_the_spin_it_is_given() {
        let Some(mut h) = Harness::new(AttractorFamily::Lorenz) else {
            return;
        };
        // A held figure, across the 120 frames the plan names.
        h.scene.set_param("spin", 0.0);
        h.run(120);
        assert_eq!(
            spin_phase(h.scene.spin_time),
            0.0,
            "spin = 0 did not hold the projection angle across 120 frames"
        );

        // ...then let it turn, and the phase is exactly the frames it ran for.
        h.scene.set_param("spin", 1.0);
        h.run(60);
        let mut expected = 0.0f32;
        for _ in 0..60 {
            expected = advance_spin(expected, 1.0, FIXED_STEP);
        }
        assert_eq!(
            h.scene.spin_time, expected,
            "the scene's accumulator is not the frame-by-frame integral of its `spin`"
        );
        assert!(spin_phase(h.scene.spin_time) > 0.0);
    }

    // -----------------------------------------------------------------------
    // The continuous-flow segment (Plan 0059 Phase 3 / ADR-0069)
    // -----------------------------------------------------------------------

    /// `is_continuous()` pinned per family against an explicit table.
    ///
    /// The compiler is what forces a fifth family to answer — the match is
    /// exhaustive with no wildcard. This pins what it answers, and, crucially,
    /// **records that its agreement with `dim == 3.0` is a coincidence of this
    /// roster**. That is the same hazard ADR-0068 Alternative C declined for the
    /// projection basis, and here the two properties happen to line up, which is
    /// exactly the condition under which someone "simplifies" one into the other.
    #[test]
    fn continuity_is_pinned_per_family() {
        let table = [
            (AttractorFamily::DeJong, false),
            (AttractorFamily::Clifford, false),
            (AttractorFamily::Thomas, true),
            (AttractorFamily::Lorenz, true),
        ];
        for (family, expected) in table {
            assert_eq!(
                family.is_continuous(),
                expected,
                "{family:?} continuity must be {expected}"
            );
        }

        // Both answers are represented, so the table is not four copies of one.
        assert!(table.iter().any(|(_, c)| *c));
        assert!(table.iter().any(|(_, c)| !*c));

        // On TODAY's roster `is_continuous()` and `dim == 3.0` agree everywhere.
        // That is recorded as a coincidence, not relied on: the moment a 2-D flow
        // or a 3-D map is added this loop stops holding, and the *correct* fix is
        // to leave `is_continuous` alone. If this assertion ever fails, do not
        // re-derive continuity from `dim` to make it pass.
        for (family, continuous) in table {
            assert_eq!(
                family.projection().1 == 3.0,
                continuous,
                "{family:?}: the dim/continuity coincidence has broken, which is                  allowed - update this note, do NOT key `is_continuous` off `dim`"
            );
        }
    }

    /// **The segment's endpoints are this frame's `pos` and the previous frame's
    /// `pos`** — asserted on the shader's inputs rather than on pixels, because
    /// "is the stroke connected" is not measurable and "is there a gap" is.
    ///
    /// Zero gap by construction is what "the beading closes" means: if `prev`
    /// after frame N is bit-identical to `pos` after frame N-1, consecutive
    /// segments share an endpoint exactly and the stroke has no seams.
    ///
    /// One caveat this is honest about: the harness advances exactly one
    /// [`FIXED_STEP`] per frame, so one compute step runs per frame. `prev` is the
    /// position before the *step*, so under a variable `dt` that drains several
    /// steps in a frame it is the last step's origin, not the frame's. The
    /// shader-level contract is per step, and that is what is asserted here.
    #[test]
    fn a_segment_starts_where_the_last_one_ended() {
        let Some(mut h) = Harness::new(AttractorFamily::Lorenz) else {
            return;
        };
        // Past the seed, so the cloud is on the attractor and genuinely moving.
        h.run(CONVERGE_FRAMES);
        let before: Vec<[f32; 3]> = h.particles().into_iter().map(|(pos, _)| pos).collect();
        h.run(1);
        let after = h.particles();

        let mut moved = 0usize;
        for (i, ((pos, prev), was)) in after.iter().zip(before.iter()).enumerate() {
            assert_eq!(
                prev, was,
                "particle {i}: this frame's segment starts at {prev:?}, but last                  frame ended at {was:?} - the stroke has a gap"
            );
            if pos != prev {
                moved += 1;
            }
        }
        // Non-vacuity: the particles actually advanced, so the equality above is
        // a statement about a moving cloud and not about a stalled one.
        assert!(
            moved * 2 > after.len(),
            "only {moved} of {} particles moved in a frame - a stalled cloud would              satisfy the endpoint check trivially",
            after.len()
        );
    }

    /// A discrete map never takes the segment branch, checked where it is decided
    /// rather than in the pixels: the draw uniform's streak slot.
    ///
    /// The pixel-level version of this claim is the golden baseline plus the
    /// byte-identity captures, which is where a chord across a De Jong's
    /// scattered successive points would actually show up.
    #[test]
    fn only_continuous_families_ask_for_a_segment() {
        // The value the draw uniform actually carries, per family — the shader
        // tests `!= 0.0`, so this is the decision point.
        assert_eq!(streak_flag(AttractorFamily::DeJong.is_continuous()), 0.0);
        assert_eq!(streak_flag(AttractorFamily::Clifford.is_continuous()), 0.0);
        assert_eq!(streak_flag(AttractorFamily::Thomas.is_continuous()), 1.0);
        assert_eq!(streak_flag(AttractorFamily::Lorenz.is_continuous()), 1.0);

        // The reseed streak ships suppressed and Plan 0059 Phase 4 decides it
        // (ADR-0069). Pinned so the provisional default is a value someone chose
        // rather than whatever a later edit last left it at.
        #[expect(
            clippy::assertions_on_constants,
            reason = "the constancy is the point: this pins a provisional default                       that Phase 4 owns flipping"
        )]
        {
            assert!(
                !RESEED_DRAWS_STREAK,
                "the reseed streak ships off; Plan 0059 Phase 4 owns flipping it"
            );
        }
    }

    // -----------------------------------------------------------------------
    // Sample density (Plan 0059 Phase 2 / ADR-0069)
    // -----------------------------------------------------------------------

    /// `density` resolves against the budget, rounds, and never leaves the range
    /// a draw can survive.
    #[test]
    fn density_resolves_against_the_tier_budget() {
        let floor = TierConfig::FLOOR.attractor_particles;
        let rich = TierConfig::RICH.attractor_particles;
        assert_eq!(active_particles(floor, 1.0), floor);
        assert_eq!(active_particles(rich, 1.0), rich);
        assert_eq!(active_particles(floor, 0.5), 25_000);

        // The documented floor, at both tiers. These are the numbers
        // `MIN_PARTICLE_DENSITY`'s rationale is written against — and that
        // rationale is a rendered capture, so pinning them here is what keeps the
        // doc comment honest if the constant is ever nudged.
        assert_eq!(active_particles(floor, MIN_PARTICLE_DENSITY), 25);
        assert_eq!(active_particles(rich, MIN_PARTICLE_DENSITY), 75);

        // The captures that set the floor were taken at these fractions, so the
        // counts they correspond to are pinned too.
        assert_eq!(active_particles(floor, 0.01), 500);
        assert_eq!(active_particles(floor, 0.002), 100);

        // Never zero (a scene drawing nothing would look like a hang, not an
        // error) and never above the allocation (an out-of-bounds vertex fetch).
        assert_eq!(active_particles(1, MIN_PARTICLE_DENSITY), 1);
        assert_eq!(active_particles(floor, 1.0), floor);
    }

    /// **Total deposited light is invariant across `density`** — the property that
    /// makes this key structural rather than an exposure control (ADR-0069 on top
    /// of ADR-0065).
    ///
    /// Asserted on the value, like ADR-0065's own scalar: `active * scale(active)`
    /// is the frame's total weight, and it must not depend on `active`. Were it
    /// not so, lowering `density` would dim the picture and every author would
    /// have to re-tune exposure to change sample count — which is precisely the
    /// trap ADR-0065 removed one plan earlier.
    #[test]
    fn total_deposited_light_is_invariant_across_density() {
        let floor = TierConfig::FLOOR.attractor_particles;
        let reference = floor as f64 * f64::from(deposit_scale(floor));
        for density in [1.0, 0.5, 0.25, 0.1, MIN_PARTICLE_DENSITY] {
            let active = active_particles(floor, density);
            let total = f64::from(active) * f64::from(deposit_scale(active));
            assert!(
                (total - reference).abs() < 1e-6 * reference,
                "density {density} draws {active} particles for total light {total},                  against {reference} at full density"
            );
        }
        // Non-vacuity: the counts genuinely differ, so the constancy above is a
        // property of the scalar and not of an unchanging `active`.
        assert_ne!(
            active_particles(floor, 1.0),
            active_particles(floor, MIN_PARTICLE_DENSITY)
        );
    }

    /// **A density change rebuilds no GPU resource**, asserted where it can fail
    /// rather than by reading the code: the particles past `active_count` still
    /// hold their seeded positions after the cloud has run.
    ///
    /// This is the same claim as "the buffer stays allocated at the tier budget
    /// and the compute early-returns" (ADR-0069), stated as an observable. It also
    /// proves the guard is what does the work — a dispatch that ran over the whole
    /// buffer, or a reallocation sized to `active`, both fail here.
    #[test]
    fn the_tail_beyond_the_active_count_never_moves() {
        const DENSITY: f32 = 0.25;
        let Some(mut h) = Harness::with_density(AttractorFamily::DeJong, DENSITY) else {
            return;
        };
        let active = active_particles(TEST_PARTICLES, DENSITY) as usize;
        assert!(active > 0 && active < TEST_PARTICLES as usize);
        let seeded: Vec<[f32; 3]> = h.scene.seed_particles.iter().map(|p| p.pos).collect();

        h.run(CONVERGE_FRAMES);
        let after = h.positions();

        // The tail is untouched, bit for bit. Not an epsilon: these particles were
        // never dispatched, so anything but equality means they were.
        for (i, (now, seed)) in after.iter().zip(seeded.iter()).enumerate().skip(active) {
            assert_eq!(
                now, seed,
                "particle {i} is past the active count {active} but moved from its seed"
            );
        }

        // Non-vacuity: the active head DID move, so the equality above is a real
        // constraint and not a report that nothing ran at all.
        let moved = after
            .iter()
            .zip(seeded.iter())
            .take(active)
            .filter(|(now, seed)| now != seed)
            .count();
        assert!(
            moved * 2 > active,
            "only {moved} of {active} active particles moved - the dispatch did not run,              so the inert tail proves nothing"
        );
    }

    // -----------------------------------------------------------------------
    // The reseed (Plan 0057 Phase 3 / ADR-0066)
    // -----------------------------------------------------------------------

    /// Particles for the reseed tests. Small: the property is about *where* the
    /// points are, not how many, and a WARP dispatch of 50 000 buys nothing here.
    const TEST_PARTICLES: u32 = 4_096;
    /// Frames run before a reseed, so the cloud has converged onto the attractor
    /// and its measured extent is the attractor's rather than the seed box's.
    const CONVERGE_FRAMES: u32 = 120;

    /// A scene driven straight, with no `Renderer`: this is a claim about the
    /// particle buffer, and going through a renderer would only add ways for the
    /// capture path to be what is under test.
    struct Harness {
        ctx: RenderContext,
        scene: AttractorScene,
        target: wgpu::TextureView,
    }

    impl Harness {
        /// `None` on a runner with no GPU adapter at all (ADR-0016). **Software
        /// adapters run this**, like the rest of the differential suite.
        ///
        /// An earlier draft skipped WARP, on the evidence that the compute
        /// dispatch there had no effect — the particle buffer came back
        /// bit-identical to the seeded scatter after 120 frames, with the right
        /// group count, the right uniform, and no validation error. That evidence
        /// was real and the conclusion was wrong: the cause was this scene's own
        /// second bind group aliasing the first on WARP (see
        /// `PipelineResources::build`), so the step dispatch read a zeroed uniform
        /// and returned on `count = 0`. Fixing that fixed the adapter. **A skip
        /// added to route around a symptom would have hidden the defect and left
        /// this test green and vacuous on CI**, which is the shape the skip was
        /// about to take.
        fn new(family: AttractorFamily) -> Option<Self> {
            Self::with_density(family, 1.0)
        }

        /// As [`Self::new`], at a chosen `[particles] density`. The buffer is
        /// still `TEST_PARTICLES` either way — that is the point of the key.
        fn with_density(family: AttractorFamily, density: f32) -> Option<Self> {
            let ctx = match RenderContext::new_headless(64, 64, true) {
                Ok(ctx) => ctx,
                Err(crate::render::RenderError::RequestAdapter(_)) => {
                    eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
                    return None;
                }
                Err(e) => panic!("headless context build failed: {e}"),
            };
            // COMPOSITE_FORMAT, not the surface's: every scene upstream of the
            // tonemap is built against it, and a mismatch here fails render-pass
            // validation on submit, which discards the WHOLE command buffer -
            // compute dispatches included. The first draft of this harness used
            // `surface_format()` and read back a cloud that had never stepped.
            let mut scene = AttractorScene::new(
                &ctx.device,
                crate::render::COMPOSITE_FORMAT,
                TEST_PARTICLES,
                TierConfig::FLOOR.attractor_trail_cap,
            );
            scene.configure(&crate::render::scenes::lines::GeneratorConfig::Particles {
                family,
                density,
                morph_to: None,
            });
            scene.set_target_size(64, 64);
            let target = ctx
                .device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some("attractor-test-target"),
                    size: wgpu::Extent3d {
                        width: 64,
                        height: 64,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: crate::render::COMPOSITE_FORMAT,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[],
                })
                .create_view(&wgpu::TextureViewDescriptor::default());
            Some(Self { ctx, scene, target })
        }

        /// Advance and render `frames` frames at the fixed capture `dt`.
        fn run(&mut self, frames: u32) {
            for _ in 0..frames {
                self.scene.advance(FIXED_STEP);
                self.scene.update(&AnalysisFrame::default());
                let mut encoder = self
                    .ctx
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
                self.scene
                    .render(&self.ctx.queue, &mut encoder, &self.target, 1.0);
                self.ctx.queue.submit(std::iter::once(encoder.finish()));
            }
        }

        /// Drive one `reseed` rising edge and render the frame it lands on.
        fn reseed(&mut self) {
            self.scene.set_param("reseed", 1.0);
            self.run(1);
            self.scene.set_param("reseed", 0.0);
        }

        fn positions(&self) -> Vec<[f32; 3]> {
            self.particles().into_iter().map(|(pos, _)| pos).collect()
        }

        /// `(pos, prev)` per particle — the pair ADR-0069's segment is drawn
        /// between.
        fn particles(&self) -> Vec<([f32; 3], [f32; 3])> {
            self.scene
                .read_particles(&self.ctx.queue)
                .expect("resources exist after a render")
        }
    }

    /// Per-axis min/max over a position set.
    fn extent(points: &[[f32; 3]]) -> ([f32; 3], [f32; 3]) {
        let mut lo = [f32::INFINITY; 3];
        let mut hi = [f32::NEG_INFINITY; 3];
        for p in points {
            for k in 0..3 {
                let (Some(l), Some(h), Some(v)) = (lo.get_mut(k), hi.get_mut(k), p.get(k)) else {
                    continue;
                };
                *l = l.min(*v);
                *h = h.max(*v);
            }
        }
        (lo, hi)
    }

    /// Cells per axis in the occupancy grid below.
    const OCCUPANCY_CELLS: i32 = 24;

    /// The region a converged cloud actually occupies, as a set of voxels over its
    /// own bounding box.
    ///
    /// **The bounding box is the wrong instrument and measuring it is how the first
    /// draft of this test went wrong.** Every family's `seed_box` is sized to the
    /// attractor's native extent, so the two boxes agree to within a percent —
    /// measured, De Jong converges to `±1.499` against a `±1.5` box and Lorenz's x
    /// to `±19.99` against `±20`. What the box cannot see is that an attractor is a
    /// *filigree*: it occupies a small fraction of its own bounding volume, and a
    /// uniform re-fill of that volume is off the figure almost everywhere while
    /// staying entirely inside its extent. Occupancy is the measure that separates
    /// them, and it is what ADR-0066 means by "the points stay on the attractor".
    struct Occupancy {
        cells: std::collections::HashSet<(i32, i32, i32)>,
        lo: [f32; 3],
        scale: [f32; 3],
    }

    impl Occupancy {
        fn of(points: &[[f32; 3]]) -> Self {
            let (lo, hi) = extent(points);
            let mut scale = [1.0f32; 3];
            for k in 0..3 {
                let (Some(s), Some(&l), Some(&h)) = (scale.get_mut(k), lo.get(k), hi.get(k)) else {
                    continue;
                };
                *s = OCCUPANCY_CELLS as f32 / (h - l).max(f32::EPSILON);
            }
            let mut me = Self {
                cells: std::collections::HashSet::new(),
                lo,
                scale,
            };
            for p in points {
                me.cells.insert(me.cell(p));
            }
            me
        }

        fn cell(&self, p: &[f32; 3]) -> (i32, i32, i32) {
            let at = |k: usize| -> i32 {
                let (Some(&v), Some(&l), Some(&s)) = (p.get(k), self.lo.get(k), self.scale.get(k))
                else {
                    return 0;
                };
                ((v - l) * s).floor() as i32
            };
            (at(0), at(1), at(2))
        }

        /// Fraction of `points` landing more than one cell away from anything the
        /// converged cloud occupied.
        ///
        /// **The one-cell dilation is the unit of the measurement, not slack.** The
        /// kick is `0.09` in a figure spanning `3.0` over 24 cells, so it is 0.72
        /// of a cell — a disturbed point routinely lands in the *neighbouring*
        /// cell, and on a figure this sparse a neighbouring cell is usually empty.
        /// Counting that as "off the attractor" would measure the grid's phase
        /// rather than the behaviour. Beyond one cell is a displacement the kick
        /// cannot produce.
        ///
        /// It costs the instrument nothing it needs: the figure occupies ~1.6 % of
        /// its bounding volume, so even dilated it is a small part of the box and a
        /// uniform re-fill still reads overwhelmingly outside — which the test
        /// asserts rather than assumes.
        fn fraction_outside(&self, points: &[[f32; 3]]) -> f32 {
            if points.is_empty() {
                return 0.0;
            }
            let near = |p: &[f32; 3]| {
                let (cx, cy, cz) = self.cell(p);
                for dx in -1..=1 {
                    for dy in -1..=1 {
                        for dz in -1..=1 {
                            if self.cells.contains(&(cx + dx, cy + dy, cz + dz)) {
                                return true;
                            }
                        }
                    }
                }
                false
            };
            let out = points.iter().filter(|p| !near(p)).count();
            out as f32 / points.len() as f32
        }

        /// Fraction of the bounding box's cells the cloud actually occupies — the
        /// number that makes the filigree claim concrete.
        fn filled(&self) -> f32 {
            let total = OCCUPANCY_CELLS.pow(3) as f32;
            self.cells.len() as f32 / total
        }
    }

    /// **The Phase 3 done-when, over the particle buffer rather than the pixels.**
    ///
    /// Two directions, and both are load-bearing. A reseed must not throw particles
    /// outside the attractor's own converged extent — which is exactly what
    /// re-filling `seed_box` did, since the box is sized to the family's *native*
    /// extent and much of its volume is off the figure. And the positions must
    /// actually *change*, because a reseed that quietly did nothing would satisfy
    /// the first half perfectly.
    ///
    /// **The control is the behaviour this replaces.** Rather than argue about a
    /// threshold, the same measurement is taken over the exact population the old
    /// re-fill would have produced — `AttractorScene::seed`, unchanged and still
    /// used for the initial fill — so the test states the two behaviours' readings
    /// side by side and asserts they are far apart in the right direction.
    #[test]
    fn a_reseed_disturbs_the_cloud_without_leaving_the_attractor() {
        // De Jong: the family the artifact was reported on (`attractor_ink`).
        const FAMILY: AttractorFamily = AttractorFamily::DeJong;
        let Some(mut h) = Harness::new(FAMILY) else {
            return;
        };
        h.run(CONVERGE_FRAMES);
        let before = h.positions();
        let occupied = Occupancy::of(&before);

        // The premise, asserted rather than assumed: the attractor must be a
        // filigree inside its own bounding box, or "off the figure" and "outside
        // the box" would name the same region and a re-fill would pass this test.
        let filled = occupied.filled();
        println!(
            "converged cloud fills {:.1}% of its own bounding volume ({} of {} cells)",
            filled * 100.0,
            occupied.cells.len(),
            OCCUPANCY_CELLS.pow(3)
        );
        assert!(
            filled < 0.5,
            "the converged cloud fills {filled:.3} of its bounding box — it is not \
             sparse enough for occupancy to distinguish a re-fill from a jitter"
        );

        h.reseed();
        let after = h.positions();
        assert_eq!(after.len(), before.len(), "the population size is fixed");

        // Direction 1 — the positions actually changed. A reseed that quietly did
        // nothing would satisfy direction 2 perfectly.
        let moved = before
            .iter()
            .zip(after.iter())
            .filter(|(a, b)| a != b)
            .count();
        assert!(
            moved * 10 > before.len() * 9,
            "a reseed must disturb essentially the whole cloud, moved {moved} of {}",
            before.len()
        );

        // Direction 2 — and the cloud is still on the attractor. Measured against
        // the old behaviour under the identical instrument.
        let jittered_outside = occupied.fraction_outside(&after);
        let refilled: Vec<[f32; 3]> = AttractorScene::seed(FAMILY, TEST_PARTICLES)
            .iter()
            .map(|p| p.pos)
            .collect();
        let refill_outside = occupied.fraction_outside(&refilled);
        println!(
            "off the figure after a reseed: jitter {:.1}%, the old seed-box re-fill \
             {:.1}%",
            jittered_outside * 100.0,
            refill_outside * 100.0
        );

        // These read **0.0 % and 100.0 %** as measured, so the bounds below are not
        // thresholds chosen to admit the result: the two behaviours sit at opposite
        // ends of the instrument, and the margins exist for adapter variation
        // rather than for the claim.
        assert!(
            jittered_outside < 0.02,
            "a reseed put {:.1}% of the cloud off the attractor; the kick is ±{:?} \
             in a figure spanning {:?}",
            jittered_outside * 100.0,
            FAMILY.jitter_extent(),
            extent(&before)
        );
        // Non-vacuity, and the half that makes this a test of ADR-0066 rather than
        // of arithmetic: the instrument must be able to see the behaviour that was
        // replaced, or the assertion above is satisfied by any measurement at all.
        assert!(
            refill_outside > 0.9,
            "the seed-box re-fill reads only {:.1}% off the figure — this instrument \
             cannot see the behaviour ADR-0066 replaced, so it proves nothing about \
             the one that replaced it",
            refill_outside * 100.0
        );
    }

    /// Determinism (`particles/mod.rs`'s pure-function-of-seed-and-step-sequence
    /// claim, NFR §6): two runs of the same input sequence produce **identical**
    /// positions after a reseed. The jitter is the one thing here that could have
    /// broken it, since it is the only per-particle randomness applied after init.
    #[test]
    fn a_reseed_is_reproducible_from_the_same_seed() {
        let run = || -> Option<Vec<[f32; 3]>> {
            let mut h = Harness::new(AttractorFamily::DeJong)?;
            h.run(30);
            h.reseed();
            h.run(3);
            Some(h.positions())
        };
        let (Some(a), Some(b)) = (run(), run()) else {
            return;
        };
        assert_eq!(a, b, "the cloud after a reseed is not reproducible");

        // ...and the reseed is genuinely doing something, so the equality above is
        // not two identical no-ops agreeing.
        let Some(mut h) = Harness::new(AttractorFamily::DeJong) else {
            return;
        };
        h.run(30);
        let unreseeded = {
            h.run(4);
            h.positions()
        };
        assert_ne!(
            a, unreseeded,
            "a reseeded run and an unreseeded one are identical — the jitter did nothing"
        );
    }

    /// Successive reseeds must kick a given particle *differently*. Salting the
    /// hash with the particle's seed alone would apply one fixed displacement field
    /// every time, which over a session is a rigid pattern rather than a
    /// disturbance — and it is invisible in a single-reseed test.
    #[test]
    fn successive_reseeds_kick_in_different_directions() {
        let Some(mut h) = Harness::new(AttractorFamily::DeJong) else {
            return;
        };
        h.run(60);
        let base = h.positions();
        h.reseed();
        let first: Vec<[f32; 3]> = h.positions();

        // The displacement the first reseed applied, per particle. Includes the
        // frame's own step, which is common to both reseeds and so cancels in the
        // comparison below.
        let delta = |from: &[[f32; 3]], to: &[[f32; 3]]| -> Vec<[f32; 3]> {
            from.iter()
                .zip(to.iter())
                .map(|(a, b)| {
                    [
                        b.first().unwrap_or(&0.0) - a.first().unwrap_or(&0.0),
                        b.get(1).unwrap_or(&0.0) - a.get(1).unwrap_or(&0.0),
                        b.get(2).unwrap_or(&0.0) - a.get(2).unwrap_or(&0.0),
                    ]
                })
                .collect()
        };
        let d1 = delta(&base, &first);

        let second_base = h.positions();
        h.reseed();
        let d2 = delta(&second_base, &h.positions());

        let identical = d1.iter().zip(d2.iter()).filter(|(a, b)| a == b).count();
        assert!(
            identical * 10 < d1.len(),
            "two reseeds applied the same displacement to {identical} of {} particles — \
             the hash is not salted by the reseed counter",
            d1.len()
        );
    }

    /// The `Particle` layout the storage buffer and the vertex attributes both
    /// assume: **two** tight 16-byte std430 halves, stride 32, each `f32` packed
    /// into the preceding `vec3`'s trailing slot. The readback above casts raw
    /// bytes to this, so a change here silently reinterprets every position.
    ///
    /// It was a tight 16 until ADR-0069 added `prev`. The offsets matter beyond
    /// the size: `vertex_attr_array!` lays its attributes out consecutively, so
    /// `prev` is fetched from offset 16 and a padding change would feed the draw
    /// someone else's bytes rather than fail to compile.
    #[test]
    fn the_particle_layout_is_two_tight_sixteens() {
        assert_eq!(std::mem::size_of::<Particle>(), 32);
        assert_eq!(std::mem::align_of::<Particle>(), 4);

        // The offsets the vertex layout hard-codes, measured rather than assumed.
        let p = Particle {
            pos: [0.0; 3],
            seed: 0.0,
            prev: [0.0; 3],
            _pad: 0.0,
        };
        let base = std::ptr::from_ref(&p) as usize;
        assert_eq!(std::ptr::from_ref(&p.pos) as usize - base, 0);
        assert_eq!(std::ptr::from_ref(&p.seed) as usize - base, 12);
        assert_eq!(std::ptr::from_ref(&p.prev) as usize - base, 16);

        // The memory this costs, at both tier budgets — the number ADR-0069's
        // price is quoted as, so it fails here if the struct grows again.
        let floor = TierConfig::FLOOR.attractor_particles as usize;
        let rich = TierConfig::RICH.attractor_particles as usize;
        assert_eq!(floor * size_of::<Particle>(), 1_600_000);
        assert_eq!(rich * size_of::<Particle>(), 4_800_000);
    }
}
