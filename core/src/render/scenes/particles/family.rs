//! The attractor families and their projection bases — the ODE/map math,
//! **GPU-free** (Plan 0061 Phase 6).
//!
//! Nothing here imports `wgpu`. That is the point of the split rather than a
//! coincidence: which figure a family draws, and which plane it is viewed on, is
//! arithmetic that a unit test can exercise without a device.

// Hot-path panic-denial pragma (Plan 0002 Phase 2; render/ is scanned by the
// hygiene guard).
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

// A continuation of one module split across four files, so it needs the names
// `particles/mod.rs` has in scope.
use super::*;

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
    pub(super) fn figure(self) -> Option<IfsFigure> {
        match self {
            AttractorFamily::Ifs(figure) => Some(figure),
            _ => None,
        }
    }

    /// The compute shader's family selector.
    pub(super) fn shader_id(self) -> u32 {
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
    pub(super) fn default_coeffs(self) -> [f32; 4] {
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
    pub(super) fn projection(self) -> (f32, f32, [f32; 3]) {
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
    pub(super) fn basis(self) -> Basis {
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
    pub(super) fn is_continuous(self) -> bool {
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
    pub(super) fn seed_box(self) -> ([f32; 3], [f32; 3]) {
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
    pub(super) fn jitter_extent(self) -> [f32; 3] {
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
    pub(super) fn inv_depth_extent(self) -> f32 {
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
    pub(super) fn masks(self) -> ([f32; 3], [f32; 3]) {
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
pub(super) const SPIN_RATE: f32 = 0.18;

/// `spin`'s default: exactly today's rate.
pub(super) const DEFAULT_SPIN: f32 = 1.0;

/// One frame's contribution to the integrated spin, in **spin-scaled seconds**.
///
/// **The phase is integrated, and that is not a preference.** Computing it as
/// `time · spin · SPIN_RATE` would let a `spin` bound to audio retroactively
/// rescale *all* elapsed time on every frame: the figure would snap to a new
/// angle whenever the binding moved, jerking rather than accelerating. A rate
/// multiplier has to be integrated to be a rate at all.
pub(super) fn advance_spin(spin_time: f32, spin: f32, dt: f32) -> f32 {
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
pub(super) fn spin_phase(spin_time: f32) -> f32 {
    spin_time * SPIN_RATE
}
