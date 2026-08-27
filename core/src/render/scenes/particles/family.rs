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

    /// Which plane a 3D family is viewed in (ADR-0068).
    ///
    /// **Named outright per family, deliberately.** It is not derived from
    /// [`canonical_framing`](Self::canonical_framing)'s `dim`, even though `dim == 3.0` selects
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
    /// It agrees with `canonical_framing().projection.1 == 3.0` on today's roster of
    /// four, and that
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

    /// The framing of roster entry 0 — the coefficients in
    /// [`default_coeffs`](Self::default_coeffs) — as literal constants.
    ///
    /// **These are the numbers this scene shipped with**, and they are here as
    /// literals rather than derived from anything: entry 0 *is* today's framing
    /// by construction, which is what makes "an unbound `tuple` is byte-identical
    /// to the build before the roster existed" structural rather than a claim
    /// about two tables agreeing (ADR-0093). Every other roster entry is measured
    /// — see [`measured_framing`].
    ///
    /// **`projection` is (world scale, dim 2/3, world centre to subtract).** The
    /// scale fits the attractor's native extent into the frame; the centre is
    /// what the projection pivots and frames on, and it is three components
    /// rather than a z-centre (Plan 0062) — it was scalar while every family that
    /// needed one was a 3D flow centred on the origin in `x` and `y`, and the
    /// fern spans `y ∈ [0, 10]`. The four map families carry exactly the values
    /// they carried before, `[0,0,0]` and `[0,0,25]`, and subtracting a zero is
    /// exact, so no capture moves.
    ///
    /// **`seed_box` is the seeded initial-scatter box**, `(half-spread, centre)`
    /// per axis, sized to the attractor's native extent so particles start spread
    /// **across** it — a box too small for a chaotic flow leaves every particle
    /// on nearly the same trajectory, so the cloud clumps instead of filling the
    /// shape. The discrete 2D maps converge from any small box, so theirs is the
    /// historical ~[-1.5, 1.5] (kept identical so their seeded look is unchanged;
    /// `z` is unused there). It feeds the initial fill and a family change only
    /// (ADR-0066) — a `reseed` does **not** re-fill it, see
    /// [`Framing::jitter_extent`], because re-filling replaces the cloud with a
    /// uniform axis-aligned rectangle, which reads as a wipe rather than a kick.
    pub(super) fn canonical_framing(self) -> Framing {
        match self {
            AttractorFamily::DeJong | AttractorFamily::Clifford => Framing {
                projection: (0.42, 2.0, [0.0, 0.0, 0.0]),
                seed_box: ([1.5, 1.5, 1.5], [0.0, 0.0, 0.0]),
            },
            AttractorFamily::Thomas => Framing {
                projection: (0.14, 3.0, [0.0, 0.0, 0.0]),
                seed_box: ([4.5, 4.5, 4.5], [0.0, 0.0, 0.0]),
            },
            AttractorFamily::Lorenz => Framing {
                projection: (0.022, 3.0, [0.0, 0.0, 25.0]),
                seed_box: ([20.0, 26.0, 24.0], [0.0, 0.0, 25.0]),
            },
            AttractorFamily::Ifs(figure) => {
                let (scale, centre) = figure.frame();
                Framing {
                    projection: (scale, 2.0, centre),
                    // The figure's own bounding box, so the fill lands *over* the
                    // attractor and contracts onto it — see [`IfsFigure::seed_box`].
                    seed_box: figure.seed_box(),
                }
            }
        }
    }

    /// The curated tuples **past entry 0** (ADR-0093), in roster order.
    ///
    /// Coefficients only: their framing is measured rather than written down, so
    /// a curator adds a figure by adding the four numbers that define it and
    /// nothing else. Entry 0 is deliberately absent from this table — it is
    /// [`default_coeffs`](Self::default_coeffs) with
    /// [`canonical_framing`](Self::canonical_framing), which is what keeps an
    /// unbound `tuple` byte-identical to the build before this table existed.
    ///
    /// **Curated, and the curation kept everything** (Plan 0079 Phase 3,
    /// 2026-08-13). This table was drafted as a candidate menu for the contact
    /// sheets; the user judged all 50 entries *in motion in the app* — a sheet
    /// freezes one instant of a rotating figure and several of these read
    /// differently once they move — and kept every one. A four-per-family
    /// shortlist was drafted and rejected, so the length is a verdict rather
    /// than a default.
    ///
    /// The consequence for anyone editing this table: **an entry's index is a
    /// preset-visible name.** The shipped `attractor_*gallery` presets step
    /// through these by index, and a preset may pin one (`attractor_torusknot`
    /// pins Lorenz entry 1), so inserting or reordering renames figures out from
    /// under them. Append; do not insert.
    ///
    /// The map families' tuples are the gallery sets backlog 0055 cites; Thomas
    /// is a sweep of its single dissipation coefficient across the chaotic band
    /// and out the far side into its periodic windows; Lorenz walks `rho`
    /// through the regimes above and below the canonical butterfly, plus three
    /// tuples that move `sigma`/`beta` instead. Entry 1 on Lorenz is the
    /// rho ≈ 100 torus knot Phase 1 shipped as the walking skeleton — the regime
    /// Plan 0075 cohort 5 measured as physically unreachable, because the figure
    /// is centred on `z ≈ 102` against the canonical framing's `25` and spans
    /// twice its extent.
    ///
    /// **The IFS is empty and stays empty.** Its shape lives in an affine table
    /// rather than in four scalars, and its figure-to-figure travel is ADR-0075's
    /// `morph`, which already carries its own measured framing.
    pub(super) fn extra_tuples(self) -> &'static [[f32; 4]] {
        match self {
            AttractorFamily::DeJong => &[
                [-2.0, -2.0, -1.2, 2.0],
                [-2.7, -0.09, -0.86, -2.2],
                [1.4, -2.3, 2.4, -2.1],
                [2.01, -2.53, 1.61, -0.33],
                [1.5, -1.8, 1.6, 0.9],
                [-0.827, -1.637, 1.659, -0.943],
                [0.97, -1.899, 1.381, -1.506],
                [-1.24, -1.25, -1.81, -1.9],
                [-0.709, 1.638, 0.452, 1.74],
                [-1.9, 1.7, 1.7, -1.4],
                [1.7, 1.7, 0.6, 1.2],
                [2.1, -1.9, 1.4, 1.1],
            ],
            AttractorFamily::Clifford => &[
                [-1.7, 1.3, -0.1, -1.21],
                [1.5, -1.8, 1.6, 0.9],
                [-1.8, -2.0, -0.5, -0.9],
                [1.7, 1.7, 0.6, 1.2],
                [-1.7, 1.8, -1.9, -0.4],
                [1.6, -0.6, -1.2, 1.6],
                [1.1, -1.0, 1.0, 1.5],
                [-1.9, 1.4, 1.9, 0.4],
                [-1.3, -1.3, -1.8, -1.9],
                [1.9, -1.9, -1.4, 1.2],
                [-1.2, -1.9, 1.5, -0.8],
                [-1.4, -1.5, 1.1, 1.4],
            ],
            // Thomas reads `a` alone, so its roster is a one-dimensional sweep:
            // 0.05 is the space-filling end of the chaotic band, 0.208 is where
            // the chaos gives way, and past it the flow closes into successively
            // tighter periodic loops.
            AttractorFamily::Thomas => &[
                [0.03, 0.0, 0.0, 0.0],
                [0.05, 0.0, 0.0, 0.0],
                [0.07, 0.0, 0.0, 0.0],
                [0.09, 0.0, 0.0, 0.0],
                [0.11, 0.0, 0.0, 0.0],
                [0.13, 0.0, 0.0, 0.0],
                [0.15, 0.0, 0.0, 0.0],
                [0.17, 0.0, 0.0, 0.0],
                [0.20, 0.0, 0.0, 0.0],
                [0.205, 0.0, 0.0, 0.0],
                [0.208, 0.0, 0.0, 0.0],
                [0.22, 0.0, 0.0, 0.0],
            ],
            AttractorFamily::Lorenz => &[
                [10.0, 100.0, 2.6667, 0.0],
                [10.0, 92.0, 2.6667, 0.0],
                [10.0, 126.52, 2.6667, 0.0],
                [10.0, 35.0, 2.6667, 0.0],
                [10.0, 60.0, 2.6667, 0.0],
                [10.0, 70.0, 2.6667, 0.0],
                [10.0, 24.4, 2.6667, 0.0],
                [16.0, 45.92, 4.0, 0.0],
                [10.0, 28.0, 1.0, 0.0],
                [14.0, 28.0, 2.6667, 0.0],
                [10.0, 28.0, 4.0, 0.0],
            ],
            AttractorFamily::Ifs(_) => &[],
        }
    }
}

/// One roster entry's framing (ADR-0093): where the figure is and how big, as
/// the two constants the render path needs.
///
/// **The unit that travels with a tuple.** Two per-family constants instead
/// are exactly what makes a distant tuple unreachable: the coefficients are
/// bindable and a per-family framing is not. Both derived quantities below
/// hang off it, so `reseed` and the depth cues follow a tuple without a second
/// table to keep in step — the Plan 0062 coupling, preserved by construction
/// rather than by discipline.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Framing {
    /// (world scale, dim 2/3, world centre) — see
    /// [`AttractorFamily::projection`].
    pub(super) projection: (f32, f32, [f32; 3]),
    /// (half-spread, centre) per axis — see [`AttractorFamily::seed_box`].
    pub(super) seed_box: ([f32; 3], [f32; 3]),
}

impl Framing {
    /// Half-extent of the per-axis kick a `reseed` applies to each particle
    /// **where it already is** (ADR-0066), in the family's own world units.
    ///
    /// Extent-relative by construction: it is [`JITTER_FRACTION`] of this
    /// entry's own [`seed_box`](Self::seed_box), which is itself sized to the
    /// figure's native extent. So one constant serves a map bounded in
    /// `[-2, 2]`, a flow spanning `±26`, and a roster entry twice that — with no
    /// per-entry number to keep in step. **This is the Plan 0062 coupling**, and
    /// it is why the roster carries framing rather than coefficients alone: a
    /// tuple whose framing did not travel with it would leave `reseed` kicking
    /// by the canonical figure's fraction, which on a larger figure is a kick
    /// too small to read and on a smaller one is a kick that throws the cloud
    /// off the attractor.
    ///
    /// **The magnitude is a look constant with no principled value.** It is large
    /// enough that the disturbance reads and small enough that the points stay on
    /// the figure — a chaotic flow separates jittered neighbours within a few
    /// iterations anyway, which is what makes a small kick sufficient. Plan 0057
    /// Phase 6 is where it is judged in motion, at both tiers; ADR-0066 records
    /// that if the disturbance reads too subtle, *this* is the lever and returning
    /// to the box is not.
    pub(super) fn jitter_extent(&self) -> [f32; 3] {
        // Destructured rather than indexed: this file denies `indexing_slicing`.
        let ([sx, sy, sz], _) = self.seed_box;
        [
            sx * JITTER_FRACTION,
            sy * JITTER_FRACTION,
            sz * JITTER_FRACTION,
        ]
    }

    /// Reciprocal of this entry's half-extent along the **view depth axis**, in
    /// the family's own world units — and **exactly `0.0` for a 2D family**
    /// (ADR-0076).
    ///
    /// That zero is the whole mechanism by which the flat families opt out: it
    /// makes `d_n` identically zero for every one of their particles, so the
    /// perspective magnification is `1`, the haze multiplier is `1` and the hue
    /// offset is `0`, with **no shader branch, no division and no way to reach a
    /// `NaN`**. De Jong, Clifford and every IFS figure have no third coordinate
    /// to project, and ADR-0076 Alternative B records why inventing one for them
    /// is worse than leaving them alone.
    ///
    /// **Derived from [`seed_box`](Self::seed_box), not hand-written** — the
    /// discipline [`jitter_extent`](Self::jitter_extent) already uses, so there
    /// is no second table of magnitudes to keep in step. The depth is the
    /// rotation's third output, and the rotation acts in the plane spanned by
    /// `x` and the basis's horizontal axis ([`Basis::masks`]'s first selector),
    /// so the depth swings through *those two* half-extents and the larger is
    /// what normalizes it. That is **26** for the canonical Lorenz (basis XZ, so
    /// the plane is `x`–`y`, half-extents 20 and 26) and **4.5** for Thomas
    /// (basis XY, plane `x`–`z`).
    ///
    /// **The family is a parameter and not a field**, because flatness is a
    /// property of the family rather than of the framing: the match below is
    /// exhaustive with no wildcard arm, so a fifth family has to answer the
    /// question rather than inherit an answer — and deriving it from
    /// `projection.1 == 3.0` would be ADR-0037's trap in the costume
    /// [`AttractorFamily::basis`] already declined once.
    pub(super) fn inv_depth_extent(&self, family: AttractorFamily) -> f32 {
        // Destructured rather than indexed: this file denies `indexing_slicing`.
        let ([sx, sy, sz], _) = self.seed_box;
        let half = match family {
            // Every IFS figure is `dim = 2` — it has no third coordinate to
            // project, which is exactly the case this doc comment anticipated.
            AttractorFamily::DeJong | AttractorFamily::Clifford | AttractorFamily::Ifs(_) => {
                return 0.0;
            }
            AttractorFamily::Thomas | AttractorFamily::Lorenz => {
                // Read off `basis()` rather than restated per family, so the two
                // cannot disagree about which plane the spin turns in.
                let partner = match family.basis() {
                    Basis::XY => sz,
                    Basis::XZ => sy,
                };
                sx.max(partner)
            }
        };
        // A degenerate box would otherwise send an infinity to the shader. It
        // cannot happen with the canonical boxes; a measured entry makes it a
        // live possibility rather than a theoretical one, and it costs one
        // compare either way.
        if half > 0.0 { 1.0 / half } else { 0.0 }
    }
}

/// A roster entry with its framing resolved (ADR-0093) — what the render path
/// actually reads once `tuple` has selected an entry.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct ResolvedTuple {
    /// The entry's coefficients, family-interpreted exactly as
    /// [`AttractorFamily::default_coeffs`]'s are. They become the fallback the
    /// `a`..`d` params modulate around, which is why selecting an entry changes
    /// the figure at all.
    pub(super) coeffs: [f32; 4],
    pub(super) framing: Framing,
}

/// A roster entry: the resolved tuple, plus the on-attractor fill a **measured**
/// entry carries.
pub(super) struct RosterEntry {
    pub(super) tuple: ResolvedTuple,
    /// Points **on** this entry's own attractor, banked by the measurement while
    /// it framed the figure — see [`MEASURE_BANK`].
    ///
    /// **Empty on the canonical entry**, which keeps the box fill it shipped
    /// with. That is not an omission: entry 0's seeded scatter is what sixteen
    /// golden baselines were blessed against, and a fill from anywhere else
    /// would move every one of them.
    pub(super) fill: Vec<[f32; 3]>,
}

/// A family's roster, framing and all — **built once at preset load**, off the
/// hot path (ADR-0093).
///
/// Entry 0 is the canonical tuple with its pinned constants; every other entry
/// is [`measured_framing`]. Resolved here rather than per frame for
/// [`FitLut::build`](super::ifs::FitLut::build)'s reason: the framing is a pure
/// function of the family and the tuple, so a frame that pays for it is paying
/// repeatedly for an answer that cannot have changed — and a `tuple` cut would
/// spend a measurement *inside the frame loop*, which is a visible hitch on the
/// exact frame the figure is already changing.
///
/// **A measurement that fails falls back to the canonical framing rather than
/// dropping the entry**, because an index is a preset-visible name: dropping
/// entry 2 would silently renumber every entry after it. A diverged tuple is
/// then visibly wrong on its contact sheet, which is where it gets rejected.
pub(super) fn resolve_roster(family: AttractorFamily) -> Vec<RosterEntry> {
    let canonical = ResolvedTuple {
        coeffs: family.default_coeffs(),
        framing: family.canonical_framing(),
    };
    let extras = family.extra_tuples();
    if extras.is_empty() {
        // Nothing to measure, so nothing is measured — which is what keeps the
        // four one-entry families (and every IFS figure) paying literally zero
        // for the roster at load.
        return vec![RosterEntry {
            tuple: canonical,
            fill: Vec::new(),
        }];
    }
    // The reference extent, measured **once per family** rather than once per
    // entry: every entry is scaled against the canonical figure's own extent, and
    // that number does not depend on which entry is being framed.
    let reference = family_reference(family);
    std::iter::once(RosterEntry {
        tuple: canonical,
        fill: Vec::new(),
    })
    .chain(extras.iter().map(|&coeffs| {
        let measured = measure_figure(family, coeffs)
            .zip(reference)
            .and_then(|(m, reference)| {
                measured_framing(family, &m.extent, reference).map(|framing| (framing, m.fill))
            });
        match measured {
            Some((framing, fill)) => RosterEntry {
                tuple: ResolvedTuple { coeffs, framing },
                fill,
            },
            None => RosterEntry {
                tuple: ResolvedTuple {
                    coeffs,
                    framing: canonical.framing,
                },
                fill: Vec::new(),
            },
        }
    }))
    .collect()
}

/// Framing samples taken across a tuple walk (ADR-0093).
///
/// [`FIT_STEPS`](super::ifs::FIT_STEPS)' reason, on this family's arithmetic: the
/// walk's framing is **measured at each sample rather than interpolated between
/// the endpoints'**, because the figure halfway between two tuples is a figure
/// in its own right and its extent is not the average of theirs. Nine is enough
/// that the sampled scale moves smoothly at a walk slow enough to read; the cost
/// is nine measurements at preset load, which is the same order the roster
/// itself already pays.
const WALK_STEPS: usize = 9;

/// A measured path between two roster entries — the mechanism ADR-0093 gates on
/// evidence.
///
/// **It walks a named pair and nothing else.** There is deliberately no way to
/// reach an arbitrary pair of coefficients through this type: it is constructed
/// from two roster indices, and the only thing a frame can move is *where along
/// that pair* it sits. That is the whole difference between this and the free
/// interpolation `tuple`'s quantization exists to forbid — a curator measured
/// this pair and decided the walk holds; nobody measured the others.
pub(super) struct TupleWalk {
    from: [f32; 4],
    to: [f32; 4],
    /// The framing at each of [`WALK_STEPS`] evenly-spaced positions.
    frames: [Framing; WALK_STEPS],
}

impl TupleWalk {
    /// Measure a path between two resolved entries, or `None` if any point
    /// along it cannot be framed — which is itself a finding: a pair whose
    /// middle diverges has no walk, whatever its endpoints look like.
    pub(super) fn build(
        family: AttractorFamily,
        from: ResolvedTuple,
        to: ResolvedTuple,
        reference: f32,
    ) -> Option<Self> {
        let mut frames = [from.framing; WALK_STEPS];
        for (i, slot) in frames.iter_mut().enumerate() {
            let t = i as f32 / (WALK_STEPS - 1) as f32;
            let coeffs = lerp4(from.coeffs, to.coeffs, t);
            // The endpoints keep the framing the roster already measured for
            // them, so a walk parked at either end renders exactly the entry it
            // names rather than a re-measurement of it.
            *slot = if i == 0 {
                from.framing
            } else if i == WALK_STEPS - 1 {
                to.framing
            } else {
                let figure = measure_figure(family, coeffs)?;
                measured_framing(family, &figure.extent, reference)?
            };
        }
        Some(Self {
            from: from.coeffs,
            to: to.coeffs,
            frames,
        })
    }

    /// The coefficients at `t`, clamped into the path.
    ///
    /// **This is the one interpolation of coefficients the scene performs**, and
    /// it is why the type exists rather than the arithmetic being inlined
    /// somewhere a stray value could reach it.
    pub(super) fn coeffs_at(&self, t: f32) -> [f32; 4] {
        lerp4(self.from, self.to, walk_position(t))
    }

    /// The framing at `t`, interpolated between the two nearest measured
    /// samples — [`FitLut::sample`](super::ifs::FitLut::sample)'s shape.
    pub(super) fn framing_at(&self, t: f32) -> Framing {
        let last = WALK_STEPS - 1;
        let pos = walk_position(t) * last as f32;
        let i = (pos.floor() as usize).min(last - 1);
        let frac = pos - i as f32;
        let (Some(a), Some(b)) = (self.frames.get(i), self.frames.get(i + 1)) else {
            return self.frames.first().copied().unwrap_or(Framing {
                projection: (1.0, 2.0, [0.0; 3]),
                seed_box: ([1.0; 3], [0.0; 3]),
            });
        };
        let (sa, dim, ca) = a.projection;
        let (sb, _, cb) = b.projection;
        let (ha, boxca) = a.seed_box;
        let (hb, boxcb) = b.seed_box;
        Framing {
            projection: (sa + (sb - sa) * frac, dim, lerp3(ca, cb, frac)),
            seed_box: (lerp3(ha, hb, frac), lerp3(boxca, boxcb, frac)),
        }
    }
}

/// A walk position: clamped into `[0, 1]`, with a non-finite binding parked at
/// the near end rather than propagated.
///
/// Clamped rather than wrapped, and that is ADR-0075's reason on this family's
/// arithmetic: past either end the coefficients leave the measured pair, and an
/// unmeasured tuple is exactly what the walk exists to avoid reaching.
fn walk_position(t: f32) -> f32 {
    if t.is_finite() {
        t.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let ([ax, ay, az], [bx, by, bz]) = (a, b);
    [ax + (bx - ax) * t, ay + (by - ay) * t, az + (bz - az) * t]
}

fn lerp4(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let ([ax, ay, az, aw], [bx, by, bz, bw]) = (a, b);
    [
        ax + (bx - ax) * t,
        ay + (by - ay) * t,
        az + (bz - az) * t,
        aw + (bw - aw) * t,
    ]
}

/// The reference reach a family's measured framings are scaled against — the
/// canonical figure's own [`framed_half`]. `None` if the canonical tuple cannot
/// be measured, which cannot happen for the four shipped families.
pub(super) fn family_reference(family: AttractorFamily) -> Option<f32> {
    measure_figure(family, family.default_coeffs()).map(|m| framed_half(family, m.extent.half))
}

/// `tuple`'s CPU-side quantization: a bound value to a roster index.
///
/// **Quantized here and not in the shader, and that is not an optimization**
/// (ADR-0093). These are chaotic maps: a fractional index would interpolate
/// coefficients *between two different figures*, which is not a halfway figure
/// but a third, unmeasured one — with neither endpoint's framing. `kaleido_order`
/// and `kaleido_edge` round for the same class of reason, and the reason bites
/// harder here: a smoothing curve makes it **necessary rather than defensive**,
/// since an eased param is continuous even when the thing it selects is not, so
/// a binding easing from `0` toward `3` sweeps *through* 1.4 whatever its
/// endpoints are.
///
/// Nearest-integer rather than truncation, so an eased sweep lands on the entry
/// it is closest to; clamped into the roster, so an over-driven binding holds the
/// last figure rather than selecting nothing; and a non-finite binding falls back
/// to the canonical entry, since `f32::clamp` propagates `NaN` and a `NaN as
/// usize` is zero by saturation rather than by decision.
pub(super) fn roster_index(value: f32, len: usize) -> usize {
    if !value.is_finite() {
        return 0;
    }
    let last = len.saturating_sub(1);
    value.round().clamp(0.0, last as f32) as usize
}

/// Euler sub-steps per fixed step for the continuous families.
///
/// **Mirrored from [`STEP_SHADER`](super::STEP_SHADER), which is the source** —
/// `the_ode_substeps_agree_between_rust_and_wgsl` holds the WGSL literal to this
/// constant. The CPU needs the same number because [`measure_extent`] frames a
/// tuple by iterating the very map the GPU will iterate, and an integrator that
/// took different steps would measure a different figure.
pub(super) const ODE_SUBSTEPS: u32 = 4;

/// Trajectories the measurement runs at once.
///
/// **A handful rather than a cloud**, because a chaotic attractor is sampled by
/// one trajectory's *time*, not by how many trajectories are launched — the
/// steps below are what buys coverage. More than one only so a figure with
/// disjoint basins cannot be measured from inside one of them.
const MEASURE_TRAJECTORIES: u32 = 4;
/// Steps discarded before measuring, so the seed box's own extent is not what
/// gets measured.
///
/// **Sized to the slowest transient on the roster, not to the fastest.** The
/// discrete maps converge within tens of steps; the Lorenz torus knot at
/// rho ≈ 100 reaches its periodic orbit through several seconds of transient
/// chaos, and measured at 600 steps its bounding box comes out ~40 % too large
/// — a figure framed off that measurement renders correspondingly small. At
/// 1200 the measurement is stable to three digits against a ten-times-longer
/// warm-up.
const MEASURE_WARMUP: u32 = 1200;
/// Steps the bounding box accumulates over, after the warm-up.
const MEASURE_STEPS: u32 = 3000;
/// A coordinate past this is a divergence rather than a figure.
///
/// **Euler is conditionally stable and the roster can reach past its
/// condition**: the Lorenz flow at rho ≈ 160 blows up at this scene's sub-step,
/// so an uncurated tuple really does produce infinities. The guard is what turns
/// that into a fallback rather than an `inf` scale reaching the GPU.
const MEASURE_DIVERGENCE: f32 = 1.0e6;
/// The measurement's own RNG seed — its own, so the starting points do not
/// correlate with the scene's seeded scatter.
const MEASURE_SEED: u64 = 0x4C4D_5641_5455_5031; // "LMVATUP1"
/// How many on-attractor points a measurement banks for the initial fill
/// ([`Measurement::fill`]).
///
/// **Duplication across the particle buffer is fine and is not what this number
/// is protecting.** At the `Rich` tier 150 000 particles share these, so each
/// bank point starts ~37 particles — and on a chaotic figure those separate
/// within a few steps, while on a periodic one they stay a curve, which is what
/// the figure is. What the count buys is *coverage*: too few points and the fill
/// is a handful of arcs rather than the whole attractor for the first second. At
/// 12 bytes each this is 48 KB per measured entry, held for the life of the
/// preset.
const MEASURE_BANK: u32 = 4096;

/// A measured bounding box, as the projection and the seed box both want it.
pub(super) struct Extent {
    pub(super) half: [f32; 3],
    pub(super) centre: [f32; 3],
}

/// Frame a tuple from its measured [`Extent`] (ADR-0093) — the readback-free
/// form of auto-centering, run once at load instead of once a frame.
///
/// `reference` is the family's canonical figure's own [`framed_half`]; `None`
/// comes back when the measurement is degenerate, which the caller turns into
/// the canonical framing.
///
/// **The scale is a ratio against the canonical tuple rather than a target fill
/// fraction**, and that is the whole trick: each family's shipped scale already
/// encodes a judgement about how much of the frame its figure should occupy —
/// 0.42 against De Jong's extent of ~1.9 fills far more of the frame than 0.022
/// against Lorenz's ~26, because a spinning 3D figure needs slack a flat map does
/// not. Scaling by the ratio of the two extents means a roster entry occupies
/// **the same footprint as its family's canonical figure**, whatever its native
/// size — so a curator judges figures, and no fill constant has to be invented
/// or defended.
pub(super) fn measured_framing(
    family: AttractorFamily,
    extent: &Extent,
    reference: f32,
) -> Option<Framing> {
    let (canonical_scale, dim, _) = family.canonical_framing().projection;
    let measured = framed_half(family, extent.half);
    if !(measured > 0.0 && reference > 0.0) {
        return None;
    }
    let scale = canonical_scale * reference / measured;
    if !scale.is_finite() || scale <= 0.0 {
        return None;
    }
    Some(Framing {
        projection: (scale, dim, extent.centre),
        // The measured box IS the figure's extent, so the initial fill lands
        // across the attractor rather than in a corner of it — and
        // `jitter_extent` inherits the right magnitude for free.
        seed_box: (extent.half, extent.centre),
    })
}

/// What one measurement pass produces: the figure's extent, and a bank of points
/// **on** it.
pub(super) struct Measurement {
    pub(super) extent: Extent,
    /// Positions the measured trajectories actually visited, evenly sampled
    /// across the window — see [`MEASURE_BANK`].
    pub(super) fill: Vec<[f32; 3]>,
}

/// Iterate a tuple: where its figure is, how big, and where it lives.
///
/// **The bank is not a by-product, it is half the point** (ADR-0087's argument,
/// applied to a measured tuple). A uniform fill of a figure's bounding box puts
/// most of its particles *off* the attractor, and getting back on is a transient
/// the viewer watches: at rho ≈ 100 the cloud wanders out to **2.2 times** the
/// figure's own extent for its first several seconds — measured, and visible as
/// a capture clipped on all four edges. The IFS solved exactly this by seeding at
/// its maps' fixed points, which are on the attractor by construction. A measured
/// tuple has no closed-form point set, but the measurement **visits** the
/// attractor thousands of times while it frames it, so banking what it saw costs
/// one push per sampled step and starts the figure on itself: seeded from the
/// bank, the same tuple never exceeds its own extent at all.
pub(super) fn measure_figure(family: AttractorFamily, coeffs: [f32; 4]) -> Option<Measurement> {
    measure_extent(
        family,
        coeffs,
        // Started from the canonical seed box: it is the one box known to be in
        // the right neighbourhood before anything has been measured, and a
        // chaotic map forgets where it started within the warm-up anyway.
        family.canonical_framing().seed_box,
        MEASURE_WARMUP,
        MEASURE_STEPS,
    )
}

/// The half-extent the frame is sized against: the largest the figure gets on
/// screen at any rotation.
///
/// Read off [`Basis::masks`] rather than restated, so it cannot disagree with
/// the shader about which axes are drawn. The spin turns `x` against the basis's
/// partner axis, so the horizontal sweep reaches the larger of those two — the
/// same reduction [`Framing::inv_depth_extent`] normalizes depth by, for the same
/// reason. **A 2D family needs no special case**: its partner axis is `z`, whose
/// extent is exactly zero, so the `max` falls through to `x`.
pub(super) fn framed_half(family: AttractorFamily, half: [f32; 3]) -> f32 {
    let ([hx, hy, hz], [vx, vy, vz]) = family.basis().masks();
    let [x, y, z] = half;
    let partner = x * hx + y * hy + z * hz;
    let vertical = x * vx + y * vy + z * vz;
    x.max(partner).max(vertical)
}

/// The tuple's bounding box and point bank over `steps`, from a few trajectories
/// started in `from` and run `warmup` steps first.
fn measure_extent(
    family: AttractorFamily,
    coeffs: [f32; 4],
    from: ([f32; 3], [f32; 3]),
    warmup: u32,
    steps: u32,
) -> Option<Measurement> {
    let ([sx, sy, sz], [cx, cy, cz]) = from;
    let mut rng = SeededRng::new(MEASURE_SEED);
    let mut points: Vec<[f32; 3]> = (0..MEASURE_TRAJECTORIES)
        .map(|_| {
            [
                cx + rng.range(-sx, sx),
                cy + rng.range(-sy, sy),
                cz + rng.range(-sz, sz),
            ]
        })
        .collect();
    // Every `stride`-th visited point is banked, so the fill samples the whole
    // measured window evenly rather than the last few hundred steps of it —
    // which on a slow flow would be a short arc of the figure rather than the
    // figure.
    let visited = steps.max(1) * MEASURE_TRAJECTORIES.max(1);
    let stride = (visited / MEASURE_BANK.max(1)).max(1) as usize;
    let mut fill: Vec<[f32; 3]> = Vec::with_capacity(MEASURE_BANK as usize + 1);
    let mut seen = 0usize;
    let mut lo = [f32::INFINITY; 3];
    let mut hi = [f32::NEG_INFINITY; 3];
    for step in 0..(warmup + steps) {
        for p in points.iter_mut() {
            *p = step_once(family, coeffs, *p);
            let [x, y, z] = *p;
            let reach = x.abs().max(y.abs()).max(z.abs());
            if !reach.is_finite() || reach > MEASURE_DIVERGENCE {
                return None;
            }
            if step < warmup {
                continue;
            }
            let ([lx, ly, lz], [hx, hy, hz]) = (lo, hi);
            lo = [lx.min(x), ly.min(y), lz.min(z)];
            hi = [hx.max(x), hy.max(y), hz.max(z)];
            if seen.is_multiple_of(stride) {
                fill.push(*p);
            }
            seen += 1;
        }
    }
    let ([lx, ly, lz], [hx, hy, hz]) = (lo, hi);
    if !(lx.is_finite() && hx.is_finite()) || fill.is_empty() {
        return None;
    }
    Some(Measurement {
        extent: Extent {
            half: [(hx - lx) * 0.5, (hy - ly) * 0.5, (hz - lz) * 0.5],
            centre: [(hx + lx) * 0.5, (hy + ly) * 0.5, (hz + lz) * 0.5],
        },
        fill,
    })
}

/// One fixed step of a family's map, on the CPU.
///
/// **The CPU mirror of [`STEP_SHADER`](super::STEP_SHADER)'s four map arms** —
/// the WGSL is the source and this is the mirror, the discipline
/// [`projection_mirror`](super::projection_mirror) and [`hash_unit`](super::hash_unit)
/// already follow. `the_cpu_step_mirrors_the_shader` runs both and compares, so
/// this is held to the shader by a differential rather than by reading.
///
/// It exists because a framing measured off *different* arithmetic would frame a
/// figure the GPU does not draw. The IFS is deliberately absent: its step draws a
/// random map per particle and its framing is ADR-0075's measured fit, so there
/// is nothing here for it to be measured by.
pub(super) fn step_once(family: AttractorFamily, coeffs: [f32; 4], p: [f32; 3]) -> [f32; 3] {
    let [a, b, c, d] = coeffs;
    let [x, y, z] = p;
    match family {
        AttractorFamily::DeJong => [
            (a * y).sin() - (b * x).cos(),
            (c * x).sin() - (d * y).cos(),
            0.0,
        ],
        AttractorFamily::Clifford => [
            (a * y).sin() + c * (a * x).cos(),
            (b * x).sin() + d * (b * y).cos(),
            0.0,
        ],
        AttractorFamily::Thomas => {
            // The shader's "lively speed-up" factor of 3 is part of the map as
            // far as a measurement is concerned: it is what the figure is
            // iterated at, so leaving it out would measure a different flow.
            let h = FIXED_STEP * 3.0 / ODE_SUBSTEPS as f32;
            let (mut x, mut y, mut z) = (x, y, z);
            for _ in 0..ODE_SUBSTEPS {
                let (dx, dy, dz) = (y.sin() - a * x, z.sin() - a * y, x.sin() - a * z);
                x += dx * h;
                y += dy * h;
                z += dz * h;
            }
            [x, y, z]
        }
        AttractorFamily::Lorenz => {
            let h = FIXED_STEP / ODE_SUBSTEPS as f32;
            let (mut x, mut y, mut z) = (x, y, z);
            for _ in 0..ODE_SUBSTEPS {
                let (dx, dy, dz) = (a * (y - x), x * (b - z) - y, x * y - c * z);
                x += dx * h;
                y += dy * h;
                z += dz * h;
            }
            [x, y, z]
        }
        // Nothing to iterate: an IFS has no coefficient tuple to frame (see
        // `extra_tuples`), so the fixed point of this function is the honest
        // answer rather than a placeholder.
        AttractorFamily::Ifs(_) => p,
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

/// The rotation angle, in radians, for an accumulated spin-time.
///
/// The spin itself is accumulated by [`Phase`](crate::render::scenes::Phase),
/// which every bindable rate in this engine advances through; **the multiply by
/// [`SPIN_RATE`] is deferred to here rather than folded into that accumulation**,
/// for the arithmetic reason the type's own header records: at the default
/// `spin = 1` the accumulator is `Σ dt` term for term, so `spin = 1` reproduces
/// the pre-ADR-0076 `time * SPIN_RATE` *exactly* and no golden baseline moves.
pub(super) fn spin_phase(spin_time: f32) -> f32 {
    spin_time * SPIN_RATE
}
