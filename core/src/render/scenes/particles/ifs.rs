//! Iterated function systems: the `attractor` scene's fifth family (ADR-0075,
//! Plan 0062).
//!
//! An IFS is the same GPU chaos game the strange-attractor families already run,
//! with a different step: instead of one map iterated by every particle, there
//! are **four** affine maps and each particle draws one at random every step. The
//! orbit converges onto the system's attractor — a Barnsley fern, a bare tree, a
//! dragon curve, a Sierpinski triangle, a spiral — rather than onto a strange
//! attractor's filigree.
//!
//! **What this module owns is the safety property**, and it is why the IFS lives
//! in its own file rather than as four more lines of `mod.rs`. De Jong and
//! Clifford are bounded for *any* coefficients, so a preset can drive them
//! anywhere; an IFS converges only while every map contracts, and one map past
//! unit operator norm sends every position to infinity and then to `NaN`, killing
//! the particle buffer for the rest of the session. Everything here is arranged
//! so that cliff is **unreachable** rather than guarded against — see
//! [ADR-0075](../../../../../docs/adrs/0075-ifs-family-morphs-in-singular-value-space.md).
//!
//! Nothing in this module touches the GPU or a clock. It resolves a figure (and,
//! from Phase 3 on, a morph and four levers) to a plain 2x3 affine table plus a
//! cumulative probability table, which is what the compute step receives.

// Hot-path panic-denial pragma (Plan 0002 Phase 2, extended to scenes by Plan
// 0003 Phase 0). Resolved once per frame on the render path.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

/// How many maps every curated table carries.
///
/// **Exactly four, always** — a figure with fewer duplicates one at probability
/// `0`. The shader's map choice is an unrolled four-way branch (the reason
/// [`Basis::masks`](super::Basis::masks) uses one-hot selectors rather than
/// indices: WGSL will not dynamically index outside addressable storage and the
/// backends disagree about the rest), so the count is structural rather than a
/// convenience.
pub const MAPS: usize = 4;

/// Which curated figure the IFS draws.
///
/// A small closed set on purpose (ADR-0075). Twenty-four free affine
/// coefficients with a contractivity cliff is close to unauthorable — most
/// random tables are a blob or a diverging cloud — so the preset surface gets
/// five hand-authored figures and a continuous path between them instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IfsFigure {
    /// Barnsley's fern — the canonical organic fractal.
    Fern,
}

/// One map's raw affine coefficients: `x' = a·x + b·y + e`, `y' = c·x + d·y + f`.
///
/// **Phase 1's literal form.** Phase 2 replaces the stored table with the
/// singular-value decomposition of the same matrices; this stays as the shape
/// everything downstream (the packing below, the shader) consumes, because the
/// GPU never sees anything but a resolved 2x3.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Affine {
    /// Linear part, row 0 column 0.
    pub a: f32,
    /// Linear part, row 0 column 1.
    pub b: f32,
    /// Linear part, row 1 column 0.
    pub c: f32,
    /// Linear part, row 1 column 1.
    pub d: f32,
    /// Translation in `x` — never enters contractivity (ADR-0075).
    pub e: f32,
    /// Translation in `y` — never enters contractivity (ADR-0075).
    pub f: f32,
}

/// A curated figure, fully resolved: four maps and their selection
/// probabilities, in **canonical order** — index 0 the trunk or dominant map, 1
/// the main body, 2 the left branch, 3 the right branch.
///
/// **The order is load-bearing and nothing enforces it.** Phase 3 pairs maps by
/// index when it morphs one figure into another, so a table authored with its
/// trunk at index 2 morphs that trunk into its partner's left branch and the
/// intermediate figures are ugly. This is a comment-and-review property.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IfsTable {
    /// The four maps, in canonical order.
    pub maps: [Affine; MAPS],
    /// Selection probabilities, summing to 1.
    pub p: [f32; MAPS],
}

/// What the compute step receives: the four linear parts, the four translations
/// packed two per `vec4`, and the **cumulative** probabilities the shader
/// compares a unit draw against.
///
/// Cumulative rather than raw because the shader's job is then three compares
/// against a rising table instead of a running sum it would have to recompute
/// per particle per step. The last entry is `1.0` by construction and is never
/// read — the fourth map is the `else` arm, which is also what makes a draw of
/// exactly `1.0` land somewhere legal.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IfsPacked {
    /// Per map: `a, b, c, d`.
    pub linear: [[f32; 4]; MAPS],
    /// Four `(e, f)` translation pairs, two per row.
    pub translate: [[f32; 4]; 2],
    /// `c0, c1, c2, 1.0`.
    pub cumulative_p: [f32; MAPS],
}

impl IfsPacked {
    /// The all-zero payload the four map families upload.
    ///
    /// They never read it — the step shader's IFS arm is the only consumer — but
    /// the uniform is one struct for every family, so it has to carry
    /// *something*. Zeros rather than a stray fern: a family that reached this
    /// data by mistake then draws nothing rather than a second figure.
    pub const ZERO: Self = Self {
        linear: [[0.0; 4]; MAPS],
        translate: [[0.0; 4]; 2],
        cumulative_p: [0.0; MAPS],
    };
}

impl IfsFigure {
    /// Every curated figure, for the sweeps that must cover the whole roster.
    ///
    /// A named constant rather than a literal at each call site: the safety
    /// argument is a *sweep* property (`max σ < 1` for every figure, every pair,
    /// every lever extreme), and a test that iterates a hand-written list is one
    /// forgotten entry away from proving it about four of five figures.
    pub const ALL: [Self; 1] = [IfsFigure::Fern];

    /// Parse a `[particles] family` name, or `None` if unknown.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "fern" => IfsFigure::Fern,
            _ => return None,
        })
    }

    /// The curated table.
    ///
    /// Barnsley's canonical coefficients and probabilities. Three properties of
    /// this table are load-bearing enough that Phase 2's round-trip test names
    /// them: `f₁` is rank 1 (`det = 0`, the stem is a line), `f₂` carries the
    /// largest singular value at `0.851` (so a global scale lever has ~17 % of
    /// headroom before it touches 1.0), and **`f₄`'s determinant is `−0.109`** —
    /// it is orientation-reversing, and any parameterization that cannot
    /// represent a reflection reproduces the fern with its right-hand frond
    /// wrong and silently.
    pub fn table(self) -> IfsTable {
        match self {
            // Canonical order, and here it is also Barnsley's own: f1 stem,
            // f2 body, f3 left frond, f4 right frond.
            IfsFigure::Fern => IfsTable {
                maps: [
                    Affine {
                        a: 0.0,
                        b: 0.0,
                        c: 0.0,
                        d: 0.16,
                        e: 0.0,
                        f: 0.0,
                    },
                    Affine {
                        a: 0.85,
                        b: 0.04,
                        c: -0.04,
                        d: 0.85,
                        e: 0.0,
                        f: 1.6,
                    },
                    Affine {
                        a: 0.2,
                        b: -0.26,
                        c: 0.23,
                        d: 0.22,
                        e: 0.0,
                        f: 1.6,
                    },
                    Affine {
                        a: -0.15,
                        b: 0.28,
                        c: 0.26,
                        d: 0.24,
                        e: 0.0,
                        f: 0.44,
                    },
                ],
                p: [0.01, 0.85, 0.07, 0.07],
            },
        }
    }

    /// `(world scale, centre)` — the projection's framing for this figure.
    ///
    /// **Hardcoded per figure in Phase 1**; Phase 4 replaces it with a lookup
    /// over `morph`, fitted from the resolved table with every lever at neutral.
    ///
    /// The fern is the reason [`projection`](super::AttractorFamily::projection)
    /// carries a full three-component centre rather than a z-centre: it spans
    /// `y ∈ [0, 10]` and is not origin-centred, so a projection that subtracts
    /// nothing puts its root on the bottom edge and its canopy off the top.
    pub fn frame(self) -> (f32, [f32; 3]) {
        match self {
            // Sampled bounding box: x ∈ [-2.182, 2.656], y ∈ [0, 9.998].
            // The scale fits the larger half-extent (5.0, in y) to ~0.84 of the
            // half-height, which is what De Jong's shipped `0.42 × 2.0` occupies.
            IfsFigure::Fern => (0.168, [0.237, 4.999, 0.0]),
        }
    }

    /// The seeded initial-scatter box, `(half-spread, centre)` per axis.
    ///
    /// The figure's own bounding box, so the initial fill lands *over* the
    /// attractor and converges onto it rather than travelling to it. The
    /// probability-weighted per-step contraction is `0.742` for the fern, so a
    /// displacement shrinks a thousandfold in ~23 steps — 0.39 s at the fixed
    /// step, which is the startup haze ADR-0075 records and the successor plan's
    /// staggered respawn removes.
    ///
    /// `z` is zero: the family is two-dimensional and takes the default
    /// [`Basis::XY`](super::Basis::XY).
    pub fn seed_box(self) -> ([f32; 3], [f32; 3]) {
        let (_, centre) = self.frame();
        match self {
            IfsFigure::Fern => ([2.5, 5.0, 0.0], centre),
        }
    }

    /// Resolve this figure to the compute step's payload.
    ///
    /// Phase 1: the curated table, unmodified. Phases 3–5 grow this into
    /// `resolve(a, b, morph, levers)`, which is where the whole safety argument
    /// lives — and it stays a pure function with no GPU and no clock, so a sweep
    /// asserting `max σ < 1` over every figure pair and every lever extreme is an
    /// ordinary unit test.
    pub fn packed(self) -> IfsPacked {
        pack(&self.table())
    }
}

/// Lay a resolved table out for the uniform, accumulating the probabilities.
///
/// The accumulation is deliberately **not** renormalized here: a table whose
/// probabilities do not sum to 1 would leave the last cumulative entry short of
/// it, and the shader's `else` arm would then absorb the shortfall into the
/// fourth map. Forcing the final entry to `1.0` states that outright rather than
/// letting a rounding residue decide.
pub fn pack(table: &IfsTable) -> IfsPacked {
    let mut linear = [[0.0f32; 4]; MAPS];
    let mut translate = [[0.0f32; 4]; 2];
    let mut cumulative_p = [0.0f32; MAPS];
    let mut running = 0.0f32;
    for (i, (map, p)) in table.maps.iter().zip(table.p.iter()).enumerate() {
        // `get_mut` rather than an index: this file denies `indexing_slicing`,
        // and `enumerate` over a fixed-size array cannot exceed it anyway.
        if let Some(row) = linear.get_mut(i) {
            *row = [map.a, map.b, map.c, map.d];
        }
        if let Some(row) = translate.get_mut(i / 2) {
            let half = (i % 2) * 2;
            if let Some(slot) = row.get_mut(half) {
                *slot = map.e;
            }
            if let Some(slot) = row.get_mut(half + 1) {
                *slot = map.f;
            }
        }
        running += p;
        if let Some(slot) = cumulative_p.get_mut(i) {
            *slot = running;
        }
    }
    // The fourth map is the shader's `else`, so this entry is never compared
    // against — pinned at 1.0 so nothing downstream has to reason about the sum.
    if let Some(last) = cumulative_p.get_mut(MAPS - 1) {
        *last = 1.0;
    }
    IfsPacked {
        linear,
        translate,
        cumulative_p,
    }
}

#[cfg(test)]
mod tests {
    // Tests panic on failure; allowed over the file's hot-path pragma.
    #![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

    use super::{IfsFigure, MAPS};

    /// Every figure name a preset can write parses, and nothing else does.
    #[test]
    fn a_figure_name_parses_or_is_rejected() {
        assert_eq!(IfsFigure::from_name("fern"), Some(IfsFigure::Fern));
        for unknown in ["", "Fern", "barnsley", "de_jong", "frond"] {
            assert_eq!(
                IfsFigure::from_name(unknown),
                None,
                "'{unknown}' must not parse as a figure"
            );
        }
    }

    /// The packing the shader reads: cumulative probabilities that rise, and a
    /// final entry of exactly 1.0.
    #[test]
    fn the_packed_probabilities_are_cumulative_and_end_at_one() {
        for figure in IfsFigure::ALL {
            let table = figure.table();
            let packed = figure.packed();

            let sum: f32 = table.p.iter().sum();
            assert!(
                (sum - 1.0).abs() < 1e-6,
                "{figure:?}'s probabilities must sum to 1, got {sum}"
            );

            let mut running = 0.0;
            for i in 0..MAPS {
                running += table.p[i];
                let expected = if i == MAPS - 1 { 1.0 } else { running };
                assert!(
                    (packed.cumulative_p[i] - expected).abs() < 1e-6,
                    "{figure:?} cumulative[{i}] = {}, expected {expected}",
                    packed.cumulative_p[i]
                );
            }
            assert_eq!(
                packed.cumulative_p[MAPS - 1],
                1.0,
                "the last entry is the shader's `else` arm and must be exactly 1.0"
            );
            // Non-decreasing, or the shader's three compares select the wrong map.
            for i in 1..MAPS {
                assert!(packed.cumulative_p[i] >= packed.cumulative_p[i - 1]);
            }
        }
    }

    /// The linear parts and translations land where the shader looks for them —
    /// `(e, f)` packed two maps per row is the one place an off-by-one is
    /// invisible in a render (it would draw a different, plausible figure).
    #[test]
    fn the_packing_puts_each_map_where_the_shader_reads_it() {
        let figure = IfsFigure::Fern;
        let table = figure.table();
        let packed = figure.packed();
        for (i, map) in table.maps.iter().enumerate() {
            assert_eq!(packed.linear[i], [map.a, map.b, map.c, map.d]);
            assert_eq!(packed.translate[i / 2][(i % 2) * 2], map.e);
            assert_eq!(packed.translate[i / 2][(i % 2) * 2 + 1], map.f);
        }
    }

    /// **The fern's fourth map is orientation-reversing**, and every later phase
    /// rests on that being representable. Asserted on the shipped table so a
    /// well-meant sign fix fails here rather than in the right-hand frond.
    #[test]
    fn the_ferns_right_frond_reflects() {
        let maps = IfsFigure::Fern.table().maps;
        let det = |m: &super::Affine| m.a * m.d - m.b * m.c;

        assert!(
            (det(&maps[3]) + 0.1088).abs() < 1e-5,
            "f4's determinant is -0.1088 (ADR-0075's table), got {}",
            det(&maps[3])
        );
        assert!(det(&maps[3]) < 0.0, "f4 must reverse orientation");
        // The stem is rank 1 — the other degenerate case the parameterization
        // has to survive.
        assert_eq!(det(&maps[0]), 0.0, "f1 is the rank-1 stem");
        // ...and the other two do not reflect, so the assertion above is about
        // f4 rather than about the whole table.
        assert!(det(&maps[1]) > 0.0);
        assert!(det(&maps[2]) > 0.0);
    }

    /// The figure is framed by its own bounding box: the scale must fit the
    /// larger half-extent inside the frame at **both** aspects.
    ///
    /// The vertical is the binding constraint (NDC `y` is undivided), and the
    /// horizontal is checked at a portrait aspect, where `world.x / aspect`
    /// magnifies rather than shrinks it.
    #[test]
    fn every_figure_is_framed_inside_the_ndc_box() {
        // The narrowest aspect the standalone realistically presents: a 9:16
        // portrait window. Anything narrower magnifies x further.
        const PORTRAIT: f32 = 9.0 / 16.0;
        for figure in IfsFigure::ALL {
            let (scale, _) = figure.frame();
            let ([hx, hy, _], _) = figure.seed_box();
            assert!(
                hy * scale < 1.0,
                "{figure:?} overflows the frame vertically: {}",
                hy * scale
            );
            assert!(
                hx * scale / PORTRAIT < 1.0,
                "{figure:?} overflows a 9:16 frame horizontally: {}",
                hx * scale / PORTRAIT
            );
            // Non-vacuity: it must also *fill* the frame rather than sit as a
            // speck in the middle — a scale of 0.001 would pass the two above.
            assert!(
                hy * scale > 0.5,
                "{figure:?} occupies only {} of the half-height",
                hy * scale
            );
        }
    }
}
