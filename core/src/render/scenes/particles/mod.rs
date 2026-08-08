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
    // ADR-0087's two channels. `age` counts steps since this particle last
    // respawned; `map` is the index of the map applied on the most recent step.
    // Both are written by the IFS arm alone and stay at their seeded 0.0
    // everywhere else.
    age: f32,
    map: f32,
    // ADR-0088's third, at offset 40: the distance from this point to the
    // nearest of the drawn maps' fixed points, normalised by the skeleton's own
    // diameter. Written by the IFS arm alone, like the two above.
    root: f32,
    // The LAST spare word. Named, not implicit: WGSL's 16-byte round-up bought
    // it, and it is the next channel's budget rather than slack.
    spare1: f32,
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
    // The reciprocal of the fixed-point set's floored diameter (ADR-0088). It
    // spends the FIRST of the three padding words below, which the vec4
    // alignment had already paid for - so the struct stays 192 bytes and the
    // bind-group layout gains no binding. Zero (and unread) on every other
    // family and on the jitter dispatch.
    root_recip: f32,
    // The rest of the vec4 alignment the affine table below requires. SCALARS,
    // not a `vec3<u32>`: a WGSL vec3 aligns to 16, which would push the table to
    // offset 64 and the struct to 176 while the Rust side laid it out at 48 and
    // 160.
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
    // The four respawn targets (ADR-0087), two (x, y) per row exactly as the
    // translations above are. Every slot is a DRAWN map's fixed point - the CPU
    // duplicates into the pads - so a pick of one of four needs no branch and no
    // knowledge of the probability table.
    fixed01: vec4<f32>,
    fixed23: vec4<f32>,
}
@group(0) @binding(1) var<uniform> step: Step;

// ADR-0087's churn constants. MIRRORED FROM THE RUST SIDE, which is the source:
// `CHURN_LIFETIME`, `CHURN_LIFETIME_SPREAD` and `LIFETIME_SALT`, held to these
// literals by `the_churn_constants_agree_between_rust_and_wgsl`. The CPU needs
// the same numbers because `seed()` places each particle at a point in a life
// this shader measures.
const LIFETIME_STEPS: f32 = 180.0;
const LIFETIME_LO: f32 = 0.5;
const LIFETIME_HI: f32 = 1.5;
const LIFETIME_SALT: u32 = 0x9E3779B1u;
// A separate salt for the respawn target, so which point a particle restarts at
// does not correlate with how long it lived.
const RESPAWN_SALT: u32 = 0x85EBCA6Bu;

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
// This particle's lifetime in steps (ADR-0087). A pure function of its own fixed
// seed, so the phases are spread across the buffer once and stay spread: a small
// flat fraction of the population restarts each step rather than the whole of it
// restarting together every three seconds.
fn ifs_lifetime(seed: f32) -> f32 {
    let u = unit01(mix32(bitcast<u32>(seed) ^ LIFETIME_SALT));
    return LIFETIME_STEPS * (LIFETIME_LO + u * (LIFETIME_HI - LIFETIME_LO));
}

// Which of the four targets this particle restarts at. Salted by the step index
// as well as the seed, so a particle does not return to the same point every
// time it recycles - the same reason ADR-0066's kick is salted by the reseed
// counter rather than by the seed alone.
fn ifs_respawn_slot(seed: f32, step_index: u32) -> u32 {
    let u = unit01(mix32(bitcast<u32>(seed) ^ (step_index * RESPAWN_SALT)));
    return min(u32(u * 4.0), 3u);
}

// Unrolled for the reason the map choice above is: WGSL will not dynamically
// index a uniform, and the backends disagree about the rest.
fn ifs_fixed_point(slot: u32) -> vec2<f32> {
    if (slot == 0u) {
        return step.fixed01.xy;
    } else if (slot == 1u) {
        return step.fixed01.zw;
    } else if (slot == 2u) {
        return step.fixed23.xy;
    }
    return step.fixed23.zw;
}

// ADR-0088's channel: how far this point is from the figure's own skeleton,
// normalised by the skeleton's diameter (the CPU ships the reciprocal).
//
// A `min` over all four slots with no branch and no knowledge of the probability
// table, for the reason `ifs_fixed_point` above needs neither: every slot is a
// DRAWN map's fixed point, because the CPU duplicates into the pads.
//
// **This is the SOURCE**; `root_distance` in the Rust test body transcribes it,
// the discipline `projection_mirror` follows against the draw shader.
//
// Normalised at the write and clamped at the READ, so the stored value stays a
// faithful measurement: the skeleton's diameter is not an upper bound on how far
// the attractor reaches, and a point past 1 is a real point rather than an error.
fn ifs_root_distance(q: vec2<f32>) -> f32 {
    let d0 = distance(q, step.fixed01.xy);
    let d1 = distance(q, step.fixed01.zw);
    let d2 = distance(q, step.fixed23.xy);
    let d3 = distance(q, step.fixed23.zw);
    return min(min(d0, d1), min(d2, d3)) * step.root_recip;
}

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
        // Which of the four was chosen, carried as an f32 because that is what
        // the channel is and because the draw reads it through a vertex
        // attribute, which has no integer path here (ADR-0087).
        var k = 3.0;
        if (r < step.cumulative_p.x) {
            m = step.m0;
            t = step.t01.xy;
            k = 0.0;
        } else if (r < step.cumulative_p.y) {
            m = step.m1;
            t = step.t01.zw;
            k = 1.0;
        } else if (r < step.cumulative_p.z) {
            m = step.m2;
            t = step.t23.xy;
            k = 2.0;
        }
        // x' = a*x + b*y + e,  y' = c*x + d*y + f. Two dimensional: z stays 0.
        p = vec3<f32>(m.x * p.x + m.y * p.y + t.x, m.z * p.x + m.w * p.y + t.y, 0.0);

        // ADR-0087's churn. The particle ages one step, and at the end of its own
        // lifetime restarts at one of the drawn maps' fixed points - which are ON
        // the attractor, so it is drawing the figure again from its first step
        // rather than travelling to it.
        //
        // Continuous rather than a one-time unfurl, and that is the load-bearing
        // half: under a one-time unfurl every age saturates within ~0.4 s and the
        // age channel is a uniform value thereafter. Under churn the population
        // always holds every age, so Phase 4's gradient is permanent.
        var age = particles[i].age + 1.0;
        if (age >= ifs_lifetime(particles[i].seed)) {
            let slot = ifs_respawn_slot(particles[i].seed, step.step_index);
            p = vec3<f32>(ifs_fixed_point(slot), 0.0);
            // It now sits AT map `slot`'s fixed point, which is inside that map's
            // sub-copy - so `map` is the slot, not the map that was applied above
            // and then discarded.
            k = f32(slot);
            age = 0.0;
        }
        particles[i].age = age;
        // Unconditional within this arm: the value names where the particle now
        // IS, so it is only meaningful when written every step. The jitter
        // dispatch returns above without touching it, which is right — a reseed
        // displaces a particle without changing which sub-copy it belongs to.
        particles[i].map = k;
        // ...and so is ADR-0088's distance, but NOT for the same reason, and the
        // difference is worth spelling out because the obvious reading has it
        // backwards. It is a PURE FUNCTION OF POSITION, recomputed from where the
        // particle now sits rather than accumulated — which is the whole
        // difference from `age`, and the reason this gradient does not decay: a
        // particle five hundred steps old sitting near a fixed point reads the
        // same near-zero a freshly restarted one does.
        //
        // That makes the jitter dispatch's early return WEAKER here than it is
        // for `map`, not stronger. A reseed kick leaves sub-copy membership
        // alone, so `map` is still correct after it; it moves the particle, so
        // `root` is not. The kicked particle carries the distance it had before
        // the kick until the next fixed step overwrites it — one step, ~1/60 s,
        // and the emergence ramp is not involved because nothing respawned.
        // Do NOT "fix" it by calling `ifs_root_distance` in the jitter arm. That
        // dispatch is handed `StepUniform::NO_IFS`, so `step.fixed01`/`fixed23`
        // and `step.root_recip` are all zero there — the call would return an
        // exact 0 for every particle and flash the whole figure to the palette's
        // anchor colour on every reseed. Fixing it properly means uploading the
        // table to the jitter slot, which costs more than one stale step is
        // worth.
        //
        // After the respawn branch, so a just-restarted particle reads EXACTLY
        // 0 — it *is* at a fixed point. That is one end of the ramp rather than a
        // special case.
        particles[i].root = ifs_root_distance(p.xy);
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
    // ch: the two per-particle colour channels, two routes each -
    //    x map_tint, y map_hue (ADR-0087), z root_tint, w root_hue (ADR-0088).
    //    The row SWAPPED rather than grew at Plan 0074 Phase 3: `age_tint` and
    //    `age_hue` held z and w and were retired, because `age` proxied
    //    distance-from-the-fixed-points and the proxy decayed. Every one defaults
    //    to 0 and 0 is the arithmetic identity on all four routes, so a preset
    //    that binds none of them renders exactly what it rendered before they
    //    existed. The CPU zeroes the WHOLE ROW on a non-IFS family, where both
    //    channels are identically 0 and `channel_shift` being centred would
    //    otherwise turn a bound value into a uniform tint over a family it means
    //    nothing on.
    //    THE TWO HALVES ARE NOT THE SAME SHAPE. `map_*` is centred, because
    //    `map01` genuinely spans [0, 1]; `root_*` is ANCHORED at 0, because
    //    `root01` does not (ADR-0088's Anchoring section). The zeroing above is
    //    therefore load-bearing for x/y and merely belt-and-braces for z/w.
    // em: ADR-0087's emergence ramp - x the per-step brightness increment, y the
    //    floor. The IFS passes (1/emergence, 0); every other family passes
    //    (0, 1), which makes the ramp EXACTLY 1.0 there rather than exactly 0 -
    //    their `age` is identically zero, so a bare `age * rate` would black them
    //    out. Two numbers rather than a branch, and the multiply by a literal 1.0
    //    is the identity in IEEE-754, so no existing capture moves.
    //    z and w are FREE since Plan 0074 Phase 3: z carried the reciprocal of
    //    the longest reachable lifetime, which only the retired age colour
    //    channel read.
    v: vec4<f32>,
    w: vec4<f32>,
    u: vec4<f32>,
    x: vec4<f32>,
    bh: vec4<f32>,
    bv: vec4<f32>,
    d: vec4<f32>,
    ctr: vec4<f32>,
    ch: vec4<f32>,
    em: vec4<f32>,
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

// The largest map index, so `map / MAP_SPAN` puts the fern's stem at 0 and its
// right frond at 1. Mirrors `ifs::MAPS - 1`; the count is structural (the step
// shader's choice is an unrolled four-way branch), so this is a constant rather
// than a uniform field.
const MAP_SPAN: f32 = 3.0;

// A per-particle channel's contribution, **centred**: a shift of +/- amount/2
// across the channel's [0, 1] range, so the mid-channel colour is the one the
// preset asked for and raising the amount opens a spread rather than sliding the
// whole figure. Exactly `depth_tint`'s shape, for exactly its reason.
//
// At `amount = 0` this is an exact 0 whatever `unit` is, which is what makes
// both palette-coordinate routes the arithmetic identity at their defaults.
fn channel_shift(unit: f32, amount: f32) -> f32 {
    return amount * (unit - 0.5);
}

// Standard RGB->HSV, the inverse of `hsv2rgb` below (both are the iq forms; the
// forward one is transcribed from `render/ink.rs`, which is where this project
// already spells HSV). The `1e-10` guards the two divisions on a greyscale or
// black input, where hue is undefined and any value is as good as another.
fn rgb2hsv(c: vec3<f32>) -> vec3<f32> {
    let k = vec4<f32>(0.0, -1.0 / 3.0, 2.0 / 3.0, -1.0);
    let p = mix(vec4<f32>(c.bg, k.wz), vec4<f32>(c.gb, k.xy), step(c.b, c.g));
    let q = mix(vec4<f32>(p.xyw, c.r), vec4<f32>(c.r, p.yzx), step(p.x, c.r));
    let d = q.x - min(q.w, q.y);
    let e = 1.0e-10;
    return vec3<f32>(abs(q.z + (q.w - q.y) / (6.0 * d + e)), d / (q.x + e), q.x);
}

// Standard HSV->RGB (iq form), transcribed from `render/ink.rs::hsv2rgb`.
// `fract` normalizes an arbitrary hue into [0, 1), so a shift may sweep freely.
fn hsv2rgb(c: vec3<f32>) -> vec3<f32> {
    let h = fract(c.x);
    let rgb = clamp(
        abs(((h * 6.0 + vec3<f32>(0.0, 4.0, 2.0)) % vec3<f32>(6.0)) - vec3<f32>(3.0)) - vec3<f32>(1.0),
        vec3<f32>(0.0),
        vec3<f32>(1.0),
    );
    return c.z * mix(vec3<f32>(1.0), rgb, c.y);
}

// The **second** route a channel reaches colour by (ADR-0087): rotate the hue of
// the colour the palette already produced, leaving the palette coordinate alone.
// That is the route for a preset that wants its fronds nudged off its body
// without editing its ramp; `*_tint` is the route for one whose colour should be
// the author's gradient.
//
// **The zero early-out is load-bearing, not an optimization.** An RGB -> HSV ->
// RGB round trip is not bit-exact, and sixteen golden baselines assert that a
// preset binding none of these params renders byte-identically. Comparing
// against literal 0.0 is exact, and 0.0 is what every unbound preset carries.
fn shift_hue(c: vec3<f32>, turns: f32) -> vec3<f32> {
    if (turns == 0.0) {
        return c;
    }
    var hsv = rgb2hsv(c);
    hsv.x = fract(hsv.x + turns);
    return hsv2rgb(hsv);
}

@vertex
fn vs_main(
    @builtin(vertex_index) vi: u32,
    @location(0) center: vec3<f32>,
    @location(1) seed: f32,
    @location(2) previous: vec3<f32>,
    // ADR-0087's last-map channel, at byte offset 36 of the particle. The
    // attribute offsets are spelled out in `PARTICLE_ATTRIBUTES` rather than
    // taken from `vertex_attr_array!`, which lays attributes out consecutively
    // and would fetch this from the padding word.
    @location(3) map: f32,
    // ADR-0087's age channel, at byte offset 32. Steps since this particle last
    // respawned; identically 0 on every family but the IFS.
    @location(4) age: f32,
    // ADR-0088's root channel, at byte offset 40. Distance to the nearest of the
    // drawn maps' fixed points, already normalised by the skeleton's diameter;
    // identically 0 on every family but the IFS.
    @location(5) root: f32,
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
    //
    // The last-map channel rides it too (ADR-0087), by the same centred shift:
    // `map` names which sub-copy of the figure this point sits in, so on the
    // fern this is what makes stem, body and the two fronds separate colours.
    // It is identically 0 on every family but the IFS, where nothing writes it.
    //
    // ...and ADR-0088's root channel, which is what the age channel was trying
    // to be. `age` proxied distance-from-the-fixed-points and the proxy decayed
    // after ~10 steps, so `age_tint`/`age_hue` never produced a gradient and were
    // retired here (Plan 0074 Phase 3). This IS that distance, recomputed every
    // step, so it is permanent. Clamped at the READ rather than at the write: the
    // skeleton's diameter is not an upper bound on the attractor's reach, so a
    // stored value past 1 is a faithful measurement, and the palette coordinate
    // is where it has to become a unit.
    //
    // **ANCHORED AT ZERO, and deliberately not `channel_shift`** (ADR-0088's
    // Anchoring section). The other terms are centred because `map01` and
    // `depth01` genuinely span [0, 1], so their midpoint means *typical* and
    // raising the amount opens a spread about the preset's colour. `root01` does
    // NOT span [0, 1] - measured, it tops out at 0.41 on the spiral and 1.05 on
    // the dragon - so a centred shift would be negative almost everywhere and
    // would slide the figure as well as spread it. Zero is the anchor that is
    // both meaningful and exactly reachable here: it is the respawn state, a
    // particle sitting on a fixed point. So the contraction points keep the
    // preset's chosen colour and the figure ramps away from them.
    let map01 = map / MAP_SPAN;
    let root01 = clamp(root, 0.0, 1.0);
    let coord = hue + hue_center + (seed - 0.5) * hue_spread + depth_tint(dn)
        + channel_shift(map01, draw.ch.x)
        + draw.ch.z * root01;
    let ca = textureSampleLevel(lut_a, lut_samp, vec2<f32>(coord, 0.5), 0.0).rgb;
    let cb = textureSampleLevel(lut_b, lut_samp, vec2<f32>(coord, 0.5), 0.0).rgb;
    // ...and each channel's OTHER route, which shifts the hue of whatever colour
    // the palette produced instead of moving where it was sampled. Before
    // `apply_saturation`, so `saturation` stays the last word on colour.
    //
    // `root_hue` is anchored for the same reason `root_tint` is: a particle on a
    // fixed point takes NO rotation, and the figure rotates away from the colour
    // the ramp gave it. Centring would rotate the skeleton itself by
    // `-root_hue/2`, which is the slide the anchoring exists to avoid - and it
    // matters more on this route than on the tint one, because a hue rotation
    // has no `hue_center` to absorb it.
    let shifted = shift_hue(
        mix(ca, cb, clamp(palette_mix, 0.0, 1.0)),
        channel_shift(map01, draw.ch.y) + draw.ch.w * root01,
    );
    let col = apply_saturation(shifted, saturation);

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
    //
    // Times the emergence ramp (ADR-0087), which is what makes the churn
    // invisible: a just-respawned particle sits on one of exactly four points, so
    // a thousand of them per frame would integrate into four bright dots in the
    // trail field. Ramped from zero it deposits almost nothing until it has been
    // iterated enough to have spread. Exactly 1.0 on every non-IFS family.
    let emergence = min(1.0, age * draw.em.x + draw.em.y);
    out.color = col * deposit * haze(dn) * emergence;
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
mod projection_mirror;

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

/// Compute step uniform (per frame): the attractor coefficients, the fixed
/// sub-step `dt`, the selected family, and the active particle count.
///
/// The same layout drives the one-shot **jitter** dispatch (ADR-0066), where
/// `family` is [`JITTER_MODE`], `coeffs.xyz` is the kick's half-extent and `salt`
/// is the reseed counter. One struct and one pipeline rather than a second of
/// each: the jitter reads and writes the same storage buffer through the same
/// bind-group layout, so only the uniform's contents differ.
///
/// **192 bytes since Plan 0073**, 160 since Plan 0062 and 32 before that, for
/// every family including the four that ignore the new fields — negligible in
/// bandwidth, and noted because it is a struct four families share. ADR-0075
/// predicted 144 for the Plan 0062 shape; the extra 16 is the alignment padding
/// [`step_index`](Self::step_index) forces, because the scalar block ahead of the
/// `vec4` table has to round up to a multiple of 16 and it was already exactly
/// full. **The bind-group layout gains no binding at either step**, so the
/// collision surface ADR-0058 reasons about does not change shape.
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
    /// The reciprocal of the fixed-point set's floored diameter
    /// ([`ifs::skeleton_scale`], ADR-0088) — the scale the step shader's IFS arm
    /// normalises a raw nearest-point distance by. Zero (and unread) on the four
    /// map families and on the jitter dispatch, exactly as the affine table is.
    ///
    /// **It costs no bytes.** It takes the first of the three explicit padding
    /// words the `vec4` table's alignment had already paid for, so the struct
    /// stays 192 and the bind-group layout gains no binding.
    root_recip: f32,
    /// The rest of that padding. Explicit, because the `vec4` table below is
    /// 16-byte aligned and the scalars above are five words. `bytemuck::Pod`
    /// requires no implicit padding, so these words must be named.
    _pad: [u32; 2],
    /// The IFS's resolved affine table — [`IfsPacked`] laid out flat. Zeroed for
    /// the four map families, which never read it.
    linear: [[f32; 4]; ifs::MAPS],
    translate: [[f32; 4]; 2],
    cumulative_p: [f32; 4],
    /// The four respawn targets (ADR-0087), two `(x, y)` per row exactly as
    /// `translate` is packed.
    fixed: [[f32; 4]; 2],
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
            root_recip: packed.root_recip,
            _pad: [0; 2],
            linear: packed.linear,
            translate: packed.translate,
            cumulative_p: packed.cumulative_p,
            fixed: packed.fixed,
        }
    }
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
/// `ch`: the two per-particle colour channels at two routes each — x `map_tint`,
/// y `map_hue` (ADR-0087), z `root_tint`, w `root_hue` (ADR-0088). All four
/// default to `0`, which is the arithmetic identity on every route.
///
/// **The row swapped rather than grew** at Plan 0074 Phase 3: `age_tint` and
/// `age_hue` held z and w until the age channel was retired. The two halves are
/// *not* the same shape — `map_*` is centred, `root_*` is anchored at `0` — for
/// the reason in ADR-0088's Anchoring section.
///
/// `em`: the emergence ramp (ADR-0087) — x the per-step brightness increment, y
/// the floor. `(1/emergence, 0)` on the IFS and `(0, 1)` everywhere else,
/// because every other family's `age` is identically zero and a bare `age·rate`
/// would black them out rather than leave them alone. **z and w are free** since
/// the retirement: z carried `1/churn_max_lifetime()`, which only the age colour
/// channel read.
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
    ch: [f32; 4],
    em: [f32; 4],
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

/// The draw pass's instance attributes, with **explicit byte offsets into
/// [`Particle`]**.
///
/// **Spelled out rather than built by `vertex_attr_array!`, and that is the
/// whole point of this constant.** That macro lays its attributes out
/// *consecutively* — which was correct while the struct was `pos`, `seed`,
/// `prev` and one trailing pad, and stopped being correct the moment ADR-0087
/// put `age` and `map` past that pad. A fourth macro entry would have fetched
/// the padding word at offset 28 and fed the draw someone else's bytes, silently
/// and with no compile error. `the_particle_layout_carries_three_channels`
/// measures these offsets against the struct so the two cannot drift.
const PARTICLE_ATTRIBUTES: &[wgpu::VertexAttribute] = &[
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x3,
        offset: 0,
        shader_location: 0, // pos (z = 0 for 2D families)
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32,
        offset: 12,
        shader_location: 1, // seed
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x3,
        offset: 16,
        shader_location: 2, // prev (ADR-0069)
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32,
        offset: 36,
        shader_location: 3, // map (ADR-0087) — 36, NOT 28, which is `_pad`
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32,
        offset: 32,
        shader_location: 4, // age (ADR-0087)
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32,
        offset: 40,
        shader_location: 5, // root (ADR-0088) — 40, the first spare word
    },
];

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
                    attributes: PARTICLE_ATTRIBUTES,
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
    /// ADR-0087's last-map channel, at its two routes. Both reach the draw
    /// uniform unclamped: the palette LUT sampler repeats, so any coordinate
    /// shift is legitimate, and `shift_hue` takes `fract` of its argument.
    map_tint: f32,
    map_hue: f32,
    /// ADR-0088's root channel, at both routes. Unclamped here for the same
    /// reason: the LUT sampler repeats. The *particle's* value is what gets
    /// clamped, in the shader, and that is a different number.
    root_tint: f32,
    root_hue: f32,
    /// The emergence ramp's length in **steps**, raw. Sanitized by
    /// [emergence_rate] where it is packed, not here, so the guard has exactly
    /// one site — the discipline rightness follows.
    emergence: f32,
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
            // The four colour channels, unclamped for the same reason
            // `depth_hue` above is: the LUT sampler repeats, so any
            // palette-coordinate shift is legitimate, and the hue route takes
            // `fract`. `map_*` is ADR-0087's, `root_*` ADR-0088's — the latter
            // took the `age_*` slots at Plan 0074 Phase 3 rather than adding to
            // them.
            //
            // **Zeroed wholesale off the IFS**, which is what makes all four
            // exactly inert on the four map families rather than merely
            // defaulted. `map` and `root` are identically `0.0` there, and
            // `channel_shift` is *centred* — so a bound `map_tint` would land a
            // uniform `-map_tint/2` on the palette coordinate of a family the
            // channel means nothing on. Inertness has to be the engine declining
            // to upload the param, the way `d.w` makes the depth cues inert on a
            // 2D family; a default of zero only covers presets that never bind it.
            //
            // The `root_*` half would survive without this branch — anchored at
            // zero, `root_tint * 0` is `0` whatever the binding — so the zeroing
            // is load-bearing for x/y and belt-and-braces for z/w. Kept whole
            // because a row that is conditionally zeroed in halves is worse to
            // reason about than one that is zeroed outright.
            ch: if inputs.family.figure().is_some() {
                [
                    inputs.map_tint,
                    inputs.map_hue,
                    inputs.root_tint,
                    inputs.root_hue,
                ]
            } else {
                [0.0; 4]
            },
            // The IFS ramps a respawned particle in; nothing else respawns, so
            // nothing else has an age at all — hence the flat floor of exactly
            // 1.0 rather than a rate they would read as zero and black
            // themselves out with. `z`/`w` are unused since the age colour
            // channel retired.
            em: if inputs.family.figure().is_some() {
                [emergence_rate(inputs.emergence), 0.0, 0.0, 0.0]
            } else {
                [0.0, 1.0, 0.0, 0.0]
            },
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
mod tests;
