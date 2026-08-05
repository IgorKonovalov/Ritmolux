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
//! The mechanism is the parameterization. Every map is carried as the **singular
//! value decomposition** of its linear part, `M = R(θ)·diag(sx, sy)·R(φ)` with
//! `sy` signed, because `R` is an isometry and so contractivity is exactly
//! `max(|sx|, |sy|) < 1` — a comparison on two numbers rather than a property of
//! a matrix. Morphing interpolates there (angles cannot affect contractivity, and
//! an interpolated singular value is below 1 because both endpoints are), and the
//! levers are built the same way.
//!
//! Nothing in this module touches the GPU or a clock. It resolves a figure to a
//! plain 2x3 affine table plus a cumulative probability table, which is what the
//! compute step receives.

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
    /// Barnsley's bare tree: a trunk that forks at ±45° all the way down.
    Tree,
    /// The Heighway dragon — two maps, and nothing organic about it.
    Dragon,
    /// The Sierpinski triangle. **Here as a correctness fixture as much as a
    /// look**: its exact self-similarity makes a wrong implementation obvious at
    /// a glance, and it is the least organic thing in a plan whose brief was
    /// "organic" (ADR-0075).
    Sierpinski,
    /// A logarithmic spiral arm with two satellite maps.
    Spiral,
}

/// One map's raw affine coefficients: `x' = a·x + b·y + e`, `y' = c·x + d·y + f`.
///
/// The **output** form: what [`recompose`] produces and what the GPU is handed.
/// It is also the form the curated tables are *authored* in, because published
/// IFS coefficients are quoted this way and a reviewer can check them against a
/// source. Nothing morphs or levers here — that all happens in [`IfsMap`].
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

/// One map, **decomposed**: `M = R(θ)·diag(sx, sy)·R(φ)`, a translation, and a
/// selection probability.
///
/// This is the space every morph and every lever acts in, and the reason the
/// whole family is safe by construction rather than by a guard.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IfsMap {
    /// The rotation applied **after** the scale.
    pub theta: f32,
    /// The rotation applied **before** the scale.
    pub phi: f32,
    /// The larger singular value — non-negative, and `>= |sy|` by construction.
    pub sx: f32,
    /// The smaller singular value, **signed** so a reflection is representable.
    /// The fern's `f₄` has determinant `−0.109`; a parameterization that forces
    /// this non-negative reproduces the fern with its right-hand frond wrong,
    /// and silently.
    pub sy: f32,
    /// The translation `(e, f)`. Does not enter contractivity at all, which is
    /// what makes `lean` an unconditionally safe lever.
    pub t: [f32; 2],
    /// Selection probability. Changes **where** points land, never whether the
    /// orbit converges — which is what makes `bias` safe too.
    pub p: f32,
}

impl IfsMap {
    /// This map's operator norm — the number the whole safety argument is stated
    /// against. Contractive exactly when this is below 1.
    ///
    /// Written as `max(|sx|, |sy|)` rather than as `sx`, even though `sx` is the
    /// larger by construction and stays so under every operation here (a lerp of
    /// two orderings preserves the ordering; a uniform scale preserves it). The
    /// property is about the operator norm, and spelling it as the property is
    /// what keeps a later edit from quietly making `sx` the wrong answer.
    pub fn sigma_max(&self) -> f32 {
        self.sx.abs().max(self.sy.abs())
    }

    /// The affine this map recomposes to.
    pub fn to_affine(&self) -> Affine {
        let (a, b, c, d) = recompose(self.theta, self.phi, self.sx, self.sy);
        let [e, f] = self.t;
        Affine { a, b, c, d, e, f }
    }
}

/// A curated figure, fully resolved: four maps in **canonical order** — index 0
/// the trunk or dominant map, 1 the main body, 2 the left branch, 3 the right
/// branch.
///
/// **The order is load-bearing and nothing enforces it.** Phase 3 pairs maps by
/// index when it morphs one figure into another, so a table authored with its
/// trunk at index 2 morphs that trunk into its partner's left branch and every
/// intermediate figure is ugly. This is a comment-and-review property; the
/// `authored` tables below each name their four roles.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IfsTable {
    /// The four maps, in canonical order.
    pub maps: [IfsMap; MAPS],
}

impl IfsTable {
    /// The largest operator norm in the table — below 1 exactly when the whole
    /// system converges.
    pub fn sigma_max(&self) -> f32 {
        self.maps
            .iter()
            .fold(0.0f32, |acc, m| acc.max(m.sigma_max()))
    }
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

/// The singular value decomposition of a 2x2, as `(θ, φ, sx, sy)` with
/// `M = R(θ)·diag(sx, sy)·R(φ)` and `sy` **signed**.
///
/// Closed form, not an iteration. Writing out `R(θ)·diag·R(φ)` and collecting
/// terms gives four combinations that separate cleanly:
///
/// ```text
/// (a+d)/2 = (sx+sy)/2 · cos(θ+φ)      (c-b)/2 = (sx+sy)/2 · sin(θ+φ)
/// (a-d)/2 = (sx-sy)/2 · cos(θ-φ)      (c+b)/2 = (sx-sy)/2 · sin(θ-φ)
/// ```
///
/// so the two magnitudes come from two hypotenuses and the two angle sums from
/// two `atan2`s. `sx = Q + R` and `sy = Q - R` — and because `Q` and `R` are
/// both non-negative, **`sy` carries the sign of the determinant for free**,
/// which is the reflection the fern's `f₄` needs. `sx ≥ |sy|` falls out the same
/// way, so `sx` is always the larger singular value.
///
/// `σ₁σ₂ = det M` checks every row; the round-trip test asserts it.
pub fn decompose(a: f32, b: f32, c: f32, d: f32) -> (f32, f32, f32, f32) {
    let e = (a + d) * 0.5;
    let f = (a - d) * 0.5;
    let g = (c + b) * 0.5;
    let h = (c - b) * 0.5;
    let q = e.hypot(h);
    let r = f.hypot(g);
    // `atan2(0, 0)` is 0 rather than a `NaN` in Rust, so a zero map — which is
    // reachable, since a padded slot may be anything — decomposes to all zeros
    // instead of poisoning the table.
    let sum = h.atan2(e);
    let diff = g.atan2(f);
    ((sum + diff) * 0.5, (sum - diff) * 0.5, q + r, q - r)
}

/// The inverse of [`decompose`] — `R(θ)·diag(sx, sy)·R(φ)`, multiplied out.
///
/// **The one place the SVD becomes a matrix again**, and the only arithmetic on
/// the path from a preset's levers to the GPU. Everything upstream of it works
/// on `(θ, φ, sx, sy)`, where safety is a comparison.
pub fn recompose(theta: f32, phi: f32, sx: f32, sy: f32) -> (f32, f32, f32, f32) {
    let (st, ct) = theta.sin_cos();
    let (sp, cp) = phi.sin_cos();
    (
        sx * ct * cp - sy * st * sp,
        -sx * ct * sp - sy * st * cp,
        sx * st * cp + sy * ct * sp,
        -sx * st * sp + sy * ct * cp,
    )
}

/// The factor Barnsley's published spiral coefficients are scaled by.
///
/// **Not cosmetic.** The published table's dominant map has `σ_max = 0.9865`,
/// which is contractive — so the figure is correct — but sits *above* the `0.97`
/// ceiling `vigor` clamps to, so the clamp would fire at neutral levers and
/// silently shrink the figure the moment a preset touched the lever. At `0.94`
/// the arm's `σ_max` is `0.9273`, leaving the same order of headroom the fern's
/// `0.851` has. The visible effect is a spiral whose arm decays faster, i.e.
/// fewer visible turns.
///
/// Applied to the linear part only. Scaling the translations too would move the
/// figure's fixed points and change what it is; this changes only the pitch.
const SPIRAL_ARM: f32 = 0.94;

impl IfsFigure {
    /// Every curated figure, for the sweeps that must cover the whole roster.
    ///
    /// A named constant rather than a literal at each call site: the safety
    /// argument is a *sweep* property (`max σ < 1` for every figure, every pair,
    /// every lever extreme), and a test that iterates a hand-written list is one
    /// forgotten entry away from proving it about four of five figures.
    pub const ALL: [Self; 5] = [
        IfsFigure::Fern,
        IfsFigure::Tree,
        IfsFigure::Dragon,
        IfsFigure::Sierpinski,
        IfsFigure::Spiral,
    ];

    /// Parse a `[particles] family` name, or `None` if unknown.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "fern" => IfsFigure::Fern,
            "tree" => IfsFigure::Tree,
            "dragon" => IfsFigure::Dragon,
            "sierpinski" => IfsFigure::Sierpinski,
            "spiral" => IfsFigure::Spiral,
            _ => return None,
        })
    }

    /// The `[particles] family` name this figure parses from — the inverse of
    /// [`from_name`](Self::from_name), for diagnostics and for the round-trip
    /// test that keeps the two in step.
    pub fn name(self) -> &'static str {
        match self {
            IfsFigure::Fern => "fern",
            IfsFigure::Tree => "tree",
            IfsFigure::Dragon => "dragon",
            IfsFigure::Sierpinski => "sierpinski",
            IfsFigure::Spiral => "spiral",
        }
    }

    /// The curated table **as authored** — raw affine coefficients and
    /// probabilities, in canonical order, each row named by its role.
    ///
    /// **The tables live in this form and are decomposed on the way out**
    /// ([`table`](Self::table)), rather than being stored as twenty
    /// hand-transcribed `(θ, φ, sx, sy)` quadruples. The published coefficients
    /// are checkable against a source by eye and the SVD literals would not be,
    /// so a transcription slip in the decomposed form would be a wrong figure
    /// nobody could review — and it would buy nothing, because the decomposition
    /// is four hypotenuses and four `atan2`s per map, computed once per preset
    /// switch, off the hot path.
    ///
    /// Every table is padded to exactly [`MAPS`] by duplicating a map at
    /// probability `0`.
    fn authored(self) -> [(Affine, f32); MAPS] {
        // Named rather than positional, so a row and its role cannot drift apart
        // in a diff.
        let map = |a, b, c, d, e, f, p| (Affine { a, b, c, d, e, f }, p);
        match self {
            // Barnsley's canonical fern. Three rows carry properties later
            // phases lean on: `f₁` is rank 1 (`det = 0` — the stem is a line),
            // `f₂` has the table's largest singular value at `0.851`, and `f₄`
            // is orientation-reversing (`det = −0.1088`).
            IfsFigure::Fern => [
                //  a      b      c      d     e     f      p        role
                map(0.00, 0.00, 0.00, 0.16, 0.0, 0.00, 0.01), // stem
                map(0.85, 0.04, -0.04, 0.85, 0.0, 1.60, 0.85), // body
                map(0.20, -0.26, 0.23, 0.22, 0.0, 1.60, 0.07), // left frond
                map(-0.15, 0.28, 0.26, 0.24, 0.0, 0.44, 0.07), // right frond
            ],
            // Barnsley's bare tree. The two branch maps are `0.594·R(±45°)`;
            // `+45°` turns the upward growth to the left, so it is index 2.
            IfsFigure::Tree => [
                map(0.00, 0.00, 0.00, 0.50, 0.0, 0.00, 0.05),  // trunk
                map(0.10, 0.00, 0.00, 0.10, 0.0, 0.20, 0.15),  // body
                map(0.42, -0.42, 0.42, 0.42, 0.0, 0.20, 0.40), // left branch
                map(0.42, 0.42, -0.42, 0.42, 0.0, 0.20, 0.40), // right branch
            ],
            // The Heighway dragon: two maps, each `0.7071·R(45°)` / `R(135°)`.
            // They *are* the figure's left and right halves, so they take the
            // branch slots and the two dominant slots duplicate them at zero —
            // which is what a padded table means.
            IfsFigure::Dragon => [
                map(0.50, -0.50, 0.50, 0.50, 0.0, 0.00, 0.00), // (pad of 2)
                map(-0.50, -0.50, 0.50, -0.50, 1.0, 0.00, 0.00), // (pad of 3)
                map(0.50, -0.50, 0.50, 0.50, 0.0, 0.00, 0.50), // left half
                map(-0.50, -0.50, 0.50, -0.50, 1.0, 0.00, 0.50), // right half
            ],
            // The Sierpinski triangle: three half-scale copies at the corners.
            // The apex map takes the trunk slot (it is the one that grows
            // upward) and the body slot duplicates it at zero.
            IfsFigure::Sierpinski => [
                map(0.50, 0.00, 0.00, 0.50, 0.25, 0.433, 1.0 / 3.0), // apex
                map(0.50, 0.00, 0.00, 0.50, 0.25, 0.433, 0.0),       // (pad of 0)
                map(0.50, 0.00, 0.00, 0.50, 0.00, 0.000, 1.0 / 3.0), // lower left
                map(0.50, 0.00, 0.00, 0.50, 0.50, 0.000, 1.0 / 3.0), // lower right
            ],
            // Barnsley's spiral: one dominant arm map (scaled by [`SPIRAL_ARM`])
            // plus two small satellites that seed the arm's substructure. The
            // arm is the dominant map, the satellites are the branches, and the
            // body slot duplicates the left satellite at zero.
            IfsFigure::Spiral => [
                map(
                    0.787879 * SPIRAL_ARM,
                    -0.424242 * SPIRAL_ARM,
                    0.242424 * SPIRAL_ARM,
                    0.859848 * SPIRAL_ARM,
                    1.758647,
                    1.408065,
                    0.90,
                ), // arm
                map(
                    0.181818, -0.136364, 0.090909, 0.181818, 6.086107, 1.568035, 0.0,
                ), // (pad of 3)
                map(
                    -0.121212, 0.257576, 0.151515, 0.053030, -6.721654, 1.377236, 0.05,
                ), // left satellite
                map(
                    0.181818, -0.136364, 0.090909, 0.181818, 6.086107, 1.568035, 0.05,
                ), // right satellite
            ],
        }
    }

    /// The curated table, decomposed — the form everything downstream works in.
    pub fn table(self) -> IfsTable {
        let mut maps = [IfsMap {
            theta: 0.0,
            phi: 0.0,
            sx: 0.0,
            sy: 0.0,
            t: [0.0, 0.0],
            p: 0.0,
        }; MAPS];
        for (slot, (affine, p)) in maps.iter_mut().zip(self.authored()) {
            let (theta, phi, sx, sy) = decompose(affine.a, affine.b, affine.c, affine.d);
            *slot = IfsMap {
                theta,
                phi,
                sx,
                sy,
                t: [affine.e, affine.f],
                p,
            };
        }
        IfsTable { maps }
    }

    /// `(world scale, centre)` — the projection's framing for this figure at the
    /// reference aspect, **as a fallback**.
    ///
    /// Since Phase 4 the render path takes its framing from [`FitLut`] instead,
    /// which follows the morph and knows the target's aspect. This survives for
    /// the two callers that have neither: the seeded scatter, and the CPU
    /// transcription of the draw shader that the projection tests run.
    ///
    /// The fern is the reason
    /// [`projection`](super::AttractorFamily::projection) carries a full
    /// three-component centre rather than a z-centre: it spans `y ∈ [0, 10]` and
    /// is not origin-centred, so a projection that subtracts nothing puts its
    /// root on the bottom edge and its canopy off the top.
    pub fn frame(self) -> (f32, [f32; 3]) {
        let (centre, half) = self.extent();
        let [cx, cy] = centre;
        (fit_scale(half, REFERENCE_ASPECT), [cx, cy, 0.0])
    }

    /// The figure's sampled bounding box as `(centre, half-extent)`, in its own
    /// world units.
    ///
    /// Measured literals rather than a call to [`chaos_extent`], because
    /// [`frame`](Self::frame) reaches this through
    /// [`projection`](super::AttractorFamily::projection), which the uniform
    /// packing calls **every frame** — a few thousand iterations there would be
    /// a chaos game per frame to answer a question whose answer never changes.
    /// The 400 000-iteration run they come from is reproduced by
    /// `the_chaos_reference_is_deterministic_and_measures_the_figure`, so the
    /// two cannot drift.
    fn extent(self) -> ([f32; 2], [f32; 2]) {
        match self {
            IfsFigure::Fern => ([0.237, 4.999], [2.419, 4.968]),
            IfsFigure::Tree => ([0.000, 0.226], [0.239, 0.213]),
            IfsFigure::Dragon => ([0.417, 0.167], [0.750, 0.500]),
            IfsFigure::Sierpinski => ([0.500, 0.433], [0.500, 0.433]),
            IfsFigure::Spiral => ([-0.008, 4.352], [7.024, 3.916]),
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
        let (centre, half) = self.extent();
        let ([cx, cy], [hx, hy]) = (centre, half);
        ([hx, hy, 0.0], [cx, cy, 0.0])
    }

    /// Resolve this figure to the compute step's payload.
    ///
    /// Phases 3–5 grow this into `resolve(a, b, morph, levers)`, which is where
    /// the whole safety argument lives — and it stays a pure function with no GPU
    /// and no clock, so a sweep asserting `max σ < 1` over every figure pair and
    /// every lever extreme is an ordinary unit test.
    pub fn packed(self) -> IfsPacked {
        pack(&self.table())
    }
}

/// The fraction of the frame a fitted figure occupies along its binding axis.
const FRAME_FILL: f32 = 0.88;
/// The aspect [`IfsFigure::frame`]'s fallback fits against.
const REFERENCE_ASPECT: f32 = 16.0 / 9.0;

/// The world scale that fits a figure of this half-extent inside the frame.
///
/// **Aspect-aware, and it has to be** (ADR-0037's lesson in its own costume).
/// The vertex shader divides world `x` by the target's aspect and leaves `y`
/// alone, so the horizontal budget is `aspect` world units and the vertical is
/// `1`. A single scale fitted at 16:9 leaves the dragon and the spiral — both
/// about twice as wide as they are tall — hanging out of a portrait window.
/// Taking the smaller of the two fits is what makes "inside the frame" true at
/// every aspect rather than at one.
///
/// The aspect comes from the **render target**, never from the trail grid: the
/// grid is a resolution, not a shape (ADR-0037), and the present is a plain
/// stretch, so the grid's own aspect cancels out.
pub fn fit_scale(half: [f32; 2], aspect: f32) -> f32 {
    let [hx, hy] = half;
    // `half()` floors both axes above zero, so neither division can blow up;
    // a non-positive aspect would, and is not reachable from a real target.
    let vertical = FRAME_FILL / hy;
    let horizontal = FRAME_FILL * aspect.max(1e-3) / hx;
    vertical.min(horizontal)
}

/// Interpolate two angles along the **shortest arc**.
///
/// Not a plain lerp, and the difference is visible rather than pedantic: two
/// maps whose rotations are `+3.0` and `−3.0` rad are a tenth of a turn apart,
/// and a plain lerp would walk the long way round — the branch would sweep
/// almost all the way through the figure and back instead of nudging across the
/// discontinuity.
///
/// Contractivity is untouched whatever this returns: `R` is an isometry, so an
/// angle cannot make a map expand. That is why the morph needs no guard.
fn lerp_angle(a: f32, b: f32, t: f32) -> f32 {
    use std::f32::consts::{PI, TAU};
    let mut delta = (b - a) % TAU;
    if delta > PI {
        delta -= TAU;
    } else if delta < -PI {
        delta += TAU;
    }
    a + delta * t
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// **The pure function everything safety-critical lives in** — no GPU, no clock,
/// no randomness (ADR-0075).
///
/// Interpolates two figures **in SVD space**, map by map, paired by index:
/// singular values and translations lerped, angles taken along the shortest arc,
/// probabilities lerped.
///
/// The result is contractive by construction and there is no clamp here to make
/// it so. A lerp of two values below 1 is below 1, and the angles do not enter
/// contractivity at all — so if both endpoints converge, so does every point on
/// the path between them. Phase 5's levers are applied on top, in this same
/// space, and only one of them needs a clamp.
///
/// Degenerate maps stay legal rather than being special-cased: the fern's stem
/// is rank 1 (`sx = 0`), and morphing a reflection into a non-reflection passes
/// through `sy = 0`. Both are contractions — the branch momentarily collapses to
/// a line and recovers.
pub fn resolve(a: &IfsTable, b: &IfsTable, morph: f32) -> IfsTable {
    // Clamped rather than trusted: `morph` is a bindable param, so a preset
    // expression can hand this anything. Outside [0, 1] the lerp would
    // extrapolate, and an extrapolated singular value is exactly the one number
    // that can leave the contractive ball.
    //
    // A `NaN` is reachable too — `0/0` is a legal preset expression — and
    // `f32::clamp` *propagates* it rather than clamping it, which would put a
    // `NaN` in the affine table and kill the buffer as surely as a divergence.
    // It resolves to the start figure.
    let t = if morph.is_nan() {
        0.0
    } else {
        morph.clamp(0.0, 1.0)
    };
    // **The endpoints are returned exactly, not lerped to.** `x + (y - x)·1` is
    // not `y` in floating point, and the difference is not academic: an absent
    // `morph_to` makes both ends the same table, and an unbound `morph` must
    // draw precisely the figure the preset named rather than one a few ulps
    // away from it. It is also what keeps a golden fixture stable.
    if t == 0.0 {
        return *a;
    }
    if t == 1.0 {
        return *b;
    }
    let mut maps = a.maps;
    for (slot, (x, y)) in maps.iter_mut().zip(a.maps.iter().zip(b.maps.iter())) {
        *slot = IfsMap {
            theta: lerp_angle(x.theta, y.theta, t),
            phi: lerp_angle(x.phi, y.phi, t),
            sx: lerp(x.sx, y.sx, t),
            sy: lerp(x.sy, y.sy, t),
            t: [lerp(x.t[0], y.t[0], t), lerp(x.t[1], y.t[1], t)],
            p: lerp(x.p, y.p, t),
        };
    }
    IfsTable { maps }
}

/// A sampled bounding box in the figure's own world units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Extent {
    /// Lower corner.
    pub lo: [f32; 2],
    /// Upper corner.
    pub hi: [f32; 2],
}

impl Extent {
    /// The box's midpoint.
    pub fn centre(&self) -> [f32; 2] {
        [
            (self.lo[0] + self.hi[0]) * 0.5,
            (self.lo[1] + self.hi[1]) * 0.5,
        ]
    }

    /// Half the box's span on each axis, floored just above zero so a degenerate
    /// figure (every point identical — reachable at a fully collapsed morph)
    /// cannot produce a division by zero downstream where a scale is fitted.
    pub fn half(&self) -> [f32; 2] {
        [
            ((self.hi[0] - self.lo[0]) * 0.5).max(1e-6),
            ((self.hi[1] - self.lo[1]) * 0.5).max(1e-6),
        ]
    }

    /// Whether every corner is a real number — the property a diverged table
    /// fails, and the one the sweep asserts.
    pub fn is_finite(&self) -> bool {
        self.lo.iter().chain(self.hi.iter()).all(|v| v.is_finite())
    }
}

/// Samples in the framing lookup, spanning `morph` from 0 to 1 inclusive.
///
/// A figure's extent moves smoothly along the morph — it is a bounded
/// continuous function of a table that is itself a lerp — so 33 samples put the
/// interpolation error far below the `0.12` of frame the fit leaves as margin.
pub const FIT_STEPS: usize = 33;

/// Chaos-game iterations behind each [`FIT_STEPS`] entry.
///
/// The measurement is a **maximum** over the sampled orbit, so it converges
/// from below: too few iterations under-measure the figure and the fit draws it
/// slightly too large. What the error has to stay inside is the margin
/// [`FRAME_FILL`] leaves — see
/// `the_fit_leaves_margin_for_what_it_under_measures`, which also records why
/// iterating it to zero is not available.
///
/// **Kept low deliberately, because iterating buys almost nothing here.** The
/// binding figure is the tree, whose `y` is 7.9 % under a long run at this
/// count — and still 6.6 % at 32 000, for eight times the load cost. The margin
/// absorbs both; the extra 3.5 ms buys 1.3 points of an error that never
/// reaches zero.
const FIT_ITERATIONS: u32 = 4_000;

/// The framing of a figure pair, sampled over `morph` once at `configure`.
///
/// **Built with every lever at neutral, and that is the non-obvious half of the
/// design** (ADR-0075 Alternative C). A fit that saw the levers would cancel its
/// own most valuable one: `vigor` exists to make the figure surge on a beat, and
/// a fit that re-framed every frame would shrink it back by exactly as much, for
/// a net zero. So the fit is a function of `morph` and the figure pair **only** —
/// which is also what lets it be a load-time table instead of a per-frame chaos
/// game, and what leaves nothing stochastic to shimmer between frames.
///
/// The accepted cost is that a hard `vigor` push can leave the frame. That is
/// the intended trade — an audible lever that can overshoot beats an inaudible
/// one that cannot — and `zoom` is the recourse.
#[derive(Debug, Clone, PartialEq)]
pub struct FitLut {
    /// `(centre, half-extent)` per sample, evenly spaced over `morph`.
    entries: [([f32; 2], [f32; 2]); FIT_STEPS],
}

impl FitLut {
    /// Measure the figure pair's framing at [`FIT_STEPS`] positions.
    ///
    /// Takes the two **neutral** tables and nothing else, so the lever
    /// independence above is structural rather than a discipline: there is no
    /// parameter here through which a lever could arrive.
    pub fn build(a: &IfsTable, b: &IfsTable) -> Self {
        let mut entries = [([0.0; 2], [1.0; 2]); FIT_STEPS];
        for (i, slot) in entries.iter_mut().enumerate() {
            let morph = i as f32 / (FIT_STEPS - 1) as f32;
            let extent = chaos_extent(&resolve(a, b, morph), FIT_ITERATIONS);
            // A diverged table cannot reach here — the sweep proves no reachable
            // morph produces one — but the fit is what the *projection* reads, so
            // a non-finite box would put a NaN in the uniform rather than fail a
            // test. Falling back to the unit box draws a wrong size, not nothing.
            *slot = if extent.is_finite() {
                (extent.centre(), extent.half())
            } else {
                ([0.0; 2], [1.0; 2])
            };
        }
        Self { entries }
    }

    /// The framing at this `morph`, linearly interpolated between the two
    /// bracketing samples. One lerp per frame; no chaos game, no allocation.
    pub fn sample(&self, morph: f32) -> ([f32; 2], [f32; 2]) {
        let t = if morph.is_nan() {
            0.0
        } else {
            morph.clamp(0.0, 1.0)
        };
        let last = FIT_STEPS - 1;
        let pos = t * last as f32;
        // `min(last - 1)` so `i + 1` is always in range, including at `t == 1`
        // where `pos` lands exactly on the final sample and `frac` is then 1.
        let i = (pos.floor() as usize).min(last - 1);
        let frac = pos - i as f32;
        let (Some((c0, h0)), Some((c1, h1))) = (self.entries.get(i), self.entries.get(i + 1))
        else {
            // Unreachable given the clamp above; this file denies `unreachable`,
            // and a unit box is a visible wrong size rather than a panic.
            return ([0.0; 2], [1.0; 2]);
        };
        (
            [lerp(c0[0], c1[0], frac), lerp(c0[1], c1[1], frac)],
            [lerp(h0[0], h1[0], frac), lerp(h0[1], h1[1], frac)],
        )
    }
}

/// Burn-in iterations discarded before the box is measured.
///
/// The orbit starts at the origin, which need not be on the figure. The
/// probability-weighted per-step contraction is 0.742 for the fern, so a
/// displacement shrinks a thousandfold in ~23 steps; 256 is far past that for
/// any table here and costs nothing at load.
const CHAOS_BURN_IN: u32 = 256;

/// The seed the CPU reference runs from. Fixed, so every measurement this module
/// makes is reproducible — the fit LUT built from it is part of a capture's
/// determinism.
const CHAOS_SEED: u64 = 0x4C4D_5641_4946_5300; // "LMVAIFS\0"

/// **A CPU run of the same step the compute shader runs**, returning the
/// sampled bounding box of the orbit.
///
/// Deliberately CPU-side and deliberately not a render (ADR-0075). The property
/// this family rests on — that no reachable table diverges — is about
/// *positions*, and a capture could only report that the picture looked
/// plausible. Here it is an ordinary assertion over a sweep, provable without a
/// GPU, before a shader ever runs.
///
/// It also feeds the framing: Phase 4's fit is this function at 33 values of
/// `morph`, run once at `configure`.
///
/// Returns a box that may be non-finite. That is the point — the caller asserts
/// finiteness rather than this silently repairing it.
pub fn chaos_extent(table: &IfsTable, iterations: u32) -> Extent {
    let mut rng = super::SeededRng::new(CHAOS_SEED);
    let packed = pack(table);
    let [c0, c1, c2, _] = packed.cumulative_p;
    // Destructured rather than indexed — this file denies `indexing_slicing`,
    // and the unrolled four-way choice below mirrors the shader's for the same
    // reason the shader has one.
    let [m0, m1, m2, m3] = table.maps.map(|m| m.to_affine());
    let (mut x, mut y) = (0.0f32, 0.0f32);
    let mut lo = [f32::INFINITY; 2];
    let mut hi = [f32::NEG_INFINITY; 2];
    for i in 0..(iterations + CHAOS_BURN_IN) {
        // The shader's selection, in Rust: a unit draw against the cumulative
        // table, with the fourth map as the `else` arm.
        let r = rng.next_f32();
        let m = if r < c0 {
            m0
        } else if r < c1 {
            m1
        } else if r < c2 {
            m2
        } else {
            m3
        };
        let (nx, ny) = (m.a * x + m.b * y + m.e, m.c * x + m.d * y + m.f);
        x = nx;
        y = ny;
        if i >= CHAOS_BURN_IN {
            lo[0] = lo[0].min(x);
            lo[1] = lo[1].min(y);
            hi[0] = hi[0].max(x);
            hi[1] = hi[1].max(y);
        }
    }
    Extent { lo, hi }
}

/// Lay a resolved table out for the uniform, recomposing each map and
/// accumulating the probabilities.
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
    for (i, map) in table.maps.iter().enumerate() {
        let affine = map.to_affine();
        // `get_mut` rather than an index: this file denies `indexing_slicing`,
        // and `enumerate` over a fixed-size array cannot exceed it anyway.
        if let Some(row) = linear.get_mut(i) {
            *row = [affine.a, affine.b, affine.c, affine.d];
        }
        if let Some(row) = translate.get_mut(i / 2) {
            let half = (i % 2) * 2;
            if let Some(slot) = row.get_mut(half) {
                *slot = affine.e;
            }
            if let Some(slot) = row.get_mut(half + 1) {
                *slot = affine.f;
            }
        }
        running += map.p;
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

    use super::{
        FIT_STEPS, FitLut, IfsFigure, MAPS, chaos_extent, decompose, fit_scale, lerp_angle,
        recompose, resolve,
    };

    /// The tolerance the plan states for the round trip: well above `f32`
    /// round-trip error on values of order 1 through five multiplies and two
    /// trig calls, and well below any coefficient difference that would be
    /// visible.
    const ROUND_TRIP_TOL: f32 = 1e-5;

    /// Every figure name a preset can write parses, and nothing else does.
    #[test]
    fn a_figure_name_parses_or_is_rejected() {
        for figure in IfsFigure::ALL {
            assert_eq!(
                IfsFigure::from_name(figure.name()),
                Some(figure),
                "{figure:?} must parse back from its own name '{}'",
                figure.name()
            );
        }
        for unknown in ["", "Fern", "barnsley", "de_jong", "frond", "ifs"] {
            assert_eq!(
                IfsFigure::from_name(unknown),
                None,
                "'{unknown}' must not parse as a figure"
            );
        }
        // The roster and the parser agree on its size — a figure added to the
        // enum but not to `ALL` would silently escape every sweep below.
        assert_eq!(IfsFigure::ALL.len(), 5);
    }

    /// **The round trip, on every curated map.**
    ///
    /// `decompose` then `recompose` must return the authored 2x2 within
    /// [`ROUND_TRIP_TOL`] per entry — the property the whole parameterization
    /// rests on, because a decomposition that cannot represent a map is a figure
    /// drawn wrong with nothing failing.
    #[test]
    fn the_decomposition_round_trips_every_curated_map() {
        for figure in IfsFigure::ALL {
            for (i, (affine, _)) in figure.authored().into_iter().enumerate() {
                let (theta, phi, sx, sy) = decompose(affine.a, affine.b, affine.c, affine.d);
                let (a, b, c, d) = recompose(theta, phi, sx, sy);
                for (got, want, which) in [
                    (a, affine.a, "a"),
                    (b, affine.b, "b"),
                    (c, affine.c, "c"),
                    (d, affine.d, "d"),
                ] {
                    assert!(
                        (got - want).abs() < ROUND_TRIP_TOL,
                        "{figure:?} map {i} entry {which}: round trip gave {got}, authored {want}"
                    );
                }
                // `σ₁σ₂ = det M` — the independent check on each row, and the
                // one that catches a decomposition that got the magnitudes right
                // and the signs wrong.
                let det = affine.a * affine.d - affine.b * affine.c;
                assert!(
                    (sx * sy - det).abs() < ROUND_TRIP_TOL,
                    "{figure:?} map {i}: sx*sy = {} but det = {det}",
                    sx * sy
                );
                // `sx` is the larger singular value and is non-negative.
                assert!(sx >= 0.0, "{figure:?} map {i}: sx = {sx} is negative");
                assert!(
                    sx >= sy.abs() - ROUND_TRIP_TOL,
                    "{figure:?} map {i}: sx = {sx} is not the larger of the two ({sy})"
                );
            }
        }
    }

    /// **The row that matters, named outright: the fern's `f₄` reflects.**
    ///
    /// A parameterization that cannot represent a reflection reproduces the fern
    /// with its right-hand frond wrong and passes every other row — so the
    /// signed `sy` is asserted on that map specifically, not inferred from the
    /// sweep above.
    #[test]
    fn the_ferns_right_frond_survives_the_decomposition_as_a_reflection() {
        let table = IfsFigure::Fern.table();
        let f4 = table.maps[3];

        assert!(
            f4.sy < 0.0,
            "f4 must decompose to a NEGATIVE second singular value, got {}",
            f4.sy
        );
        assert!(
            (f4.sx * f4.sy + 0.1088).abs() < ROUND_TRIP_TOL,
            "f4's determinant is -0.1088 (ADR-0075's table), got {}",
            f4.sx * f4.sy
        );
        // And it survives the trip back: the recomposed map still reverses
        // orientation, which is the property the render depends on.
        let back = f4.to_affine();
        assert!(
            back.a * back.d - back.b * back.c < 0.0,
            "the recomposed f4 no longer reflects"
        );

        // The other three rows do not reflect, so the assertion above is about
        // f4 rather than about the whole table. The stem is the rank-1 case —
        // the other degenerate map the parameterization has to survive.
        assert!(table.maps[0].sy.abs() < ROUND_TRIP_TOL, "f1 is rank 1");
        assert!(table.maps[1].sy > 0.0);
        assert!(table.maps[2].sy > 0.0);
    }

    /// **Every curated table is contractive, with headroom.**
    ///
    /// The precondition for everything Phases 3-5 build: a figure whose own
    /// table already sat at or above 1 could not be made safe by any lever.
    /// The `0.97` bound is asserted too, because that is `vigor`'s ceiling — a
    /// figure above it would have the clamp fire at *neutral* levers and shrink
    /// silently, which is what [`SPIRAL_ARM`](super::SPIRAL_ARM) exists to
    /// prevent.
    #[test]
    fn every_curated_table_contracts_below_the_vigor_ceiling() {
        for figure in IfsFigure::ALL {
            let sigma = figure.table().sigma_max();
            assert!(
                sigma < 1.0,
                "{figure:?} does not contract: sigma_max = {sigma}"
            );
            assert!(
                sigma <= 0.97,
                "{figure:?} sits above vigor's ceiling at neutral: sigma_max = {sigma}"
            );
            // Non-vacuity: a table of near-zero maps would pass the two above
            // and draw a dot. Every figure has a map doing real work.
            assert!(
                sigma > 0.4,
                "{figure:?} contracts so hard it cannot draw a figure: {sigma}"
            );
        }
        // The fern's is ADR-0075's quoted number, which the headroom argument
        // (~17 % to 1.0) is stated against.
        let fern = IfsFigure::Fern.table().sigma_max();
        assert!(
            (fern - 0.851).abs() < 1e-3,
            "the fern's sigma_max is 0.851 in ADR-0075, got {fern}"
        );
    }

    /// Every table is exactly [`MAPS`] maps with probabilities summing to 1, and
    /// the padding is a **duplicate at zero** rather than a zero map.
    ///
    /// The distinction matters for the morph: a zero map at index `i` would drag
    /// its partner's map toward the origin at every intermediate `morph`, so the
    /// padded slot has to be a real map that simply never gets picked.
    #[test]
    fn every_table_is_four_maps_padded_by_duplication() {
        for figure in IfsFigure::ALL {
            let authored = figure.authored();
            assert_eq!(authored.len(), MAPS);

            let sum: f32 = authored.iter().map(|(_, p)| *p).sum();
            assert!(
                (sum - 1.0).abs() < 1e-5,
                "{figure:?}'s probabilities sum to {sum}, not 1"
            );
            for (i, (_, p)) in authored.iter().enumerate() {
                assert!(*p >= 0.0, "{figure:?} map {i} has a negative probability");
            }
            // Each padded slot duplicates a slot that is actually drawn.
            for (i, (affine, p)) in authored.iter().enumerate() {
                if *p > 0.0 {
                    continue;
                }
                assert!(
                    authored
                        .iter()
                        .any(|(other, q)| *q > 0.0 && other == affine),
                    "{figure:?} map {i} has probability 0 but is not a duplicate of a drawn map"
                );
            }
        }
        // Non-vacuity: the fern and the tree have no padding at all, so the loop
        // above is not four figures' worth of vacuous truth.
        for full in [IfsFigure::Fern, IfsFigure::Tree, IfsFigure::Sierpinski] {
            assert!(full.authored().iter().filter(|(_, p)| *p == 0.0).count() <= 1);
        }
        assert_eq!(
            IfsFigure::Dragon
                .authored()
                .iter()
                .filter(|(_, p)| *p == 0.0)
                .count(),
            2,
            "the dragon has two real maps, so two slots are padding"
        );
    }

    /// The packing the shader reads: cumulative probabilities that rise, and a
    /// final entry of exactly 1.0.
    #[test]
    fn the_packed_probabilities_are_cumulative_and_end_at_one() {
        for figure in IfsFigure::ALL {
            let table = figure.table();
            let packed = figure.packed();

            let mut running = 0.0;
            for i in 0..MAPS {
                running += table.maps[i].p;
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
        for figure in IfsFigure::ALL {
            let table = figure.table();
            let packed = figure.packed();
            for (i, map) in table.maps.iter().enumerate() {
                let affine = map.to_affine();
                assert_eq!(
                    packed.linear[i],
                    [affine.a, affine.b, affine.c, affine.d],
                    "{figure:?} map {i}"
                );
                assert_eq!(packed.translate[i / 2][(i % 2) * 2], affine.e);
                assert_eq!(packed.translate[i / 2][(i % 2) * 2 + 1], affine.f);
            }
        }
    }

    /// [`IfsFigure::frame`] — the **fallback** framing, used by the seeded
    /// scatter and by the CPU transcription of the draw shader — frames every
    /// figure at the reference aspect.
    ///
    /// The render path does not go through here since Phase 4; what it uses is
    /// asserted by `every_shipped_pair_stays_framed_at_both_aspects`. This
    /// stays because the fallback is what sizes the seed box, and a figure
    /// seeded outside its own attractor takes visibly longer to converge.
    #[test]
    fn every_figure_is_framed_at_the_reference_aspect() {
        const REFERENCE: f32 = 16.0 / 9.0;
        for figure in IfsFigure::ALL {
            let (scale, _) = figure.frame();
            let ([hx, hy, _], _) = figure.seed_box();
            assert!(
                hy * scale < 1.0,
                "{figure:?} overflows the frame vertically: {}",
                hy * scale
            );
            assert!(
                hx * scale / REFERENCE < 1.0,
                "{figure:?} overflows a 16:9 frame horizontally: {}",
                hx * scale / REFERENCE
            );
            // Non-vacuity: it must also *fill* the frame along its binding axis
            // rather than sit as a speck in the middle — a scale of 0.001 would
            // pass the two above.
            let fill = (hy * scale).max(hx * scale / REFERENCE);
            assert!(
                fill > 0.7,
                "{figure:?} occupies only {fill} of the frame's binding axis"
            );
        }

        // The fern's Phase 1 claim, kept: it is tall, so it is framed at a
        // portrait aspect too without the fit having to narrow it.
        let (scale, centre) = IfsFigure::Fern.frame();
        assert_eq!(centre, [0.237, 4.999, 0.0]);
        let ([hx, hy, _], _) = IfsFigure::Fern.seed_box();
        assert!(hy * scale < 1.0);
        assert!(hx * scale / (9.0 / 16.0) < 1.0);
    }

    // -----------------------------------------------------------------------
    // The morph (Plan 0062 Phase 3 / ADR-0075)
    // -----------------------------------------------------------------------

    /// The sweep resolution the plan names, and the one the fit LUT uses too.
    const MORPH_STEPS: usize = 33;

    /// The 33 sweep positions, `0.0` and `1.0` inclusive and exact at both ends.
    fn morph_sweep() -> impl Iterator<Item = f32> {
        (0..MORPH_STEPS).map(|i| i as f32 / (MORPH_STEPS - 1) as f32)
    }

    /// **The property the whole family rests on, asserted without a GPU.**
    ///
    /// A CPU run of the same step the shader runs, on the resolved table, at
    /// every position of a 33-point sweep, for **every ordered pair** of the five
    /// figures: 25 pairs x 33 positions x 10 000 iterations. Every position must
    /// stay finite and inside a bounded box, and every map's `max σ` must stay
    /// below 1.
    ///
    /// Ordered pairs rather than unordered, because `resolve` is not symmetric —
    /// the shortest-arc angle interpolation and the clamp both read `a` first —
    /// and a preset names a start and a target, not a set.
    ///
    /// Deliberately CPU-side (ADR-0075): the failure this excludes is a
    /// permanently dead particle buffer, and a capture of a preset that only
    /// diverges on a loud passage would pass.
    #[test]
    fn no_reachable_morph_diverges() {
        // Generous, and it is meant to be: the point is to catch a run to
        // infinity, not to pin a figure's size. The largest curated figure spans
        // ~14 world units, so anything past 1000 is a divergence in progress
        // rather than a big fern.
        const BOUND: f32 = 1_000.0;
        let mut pairs = 0;
        for start in IfsFigure::ALL {
            for target in IfsFigure::ALL {
                pairs += 1;
                let (a, b) = (start.table(), target.table());
                for morph in morph_sweep() {
                    let table = resolve(&a, &b, morph);

                    // Contractivity first — it is the *reason* the box below is
                    // bounded, and asserting it separately is what distinguishes
                    // "converges" from "happened not to blow up in 10 000 steps".
                    for (i, map) in table.maps.iter().enumerate() {
                        let sigma = map.sigma_max();
                        assert!(
                            sigma < 1.0,
                            "{start:?} -> {target:?} at morph {morph}: map {i} has \
                             sigma_max = {sigma}, which does not contract"
                        );
                    }

                    let extent = chaos_extent(&table, 10_000);
                    assert!(
                        extent.is_finite(),
                        "{start:?} -> {target:?} at morph {morph}: the orbit left \
                         the reals ({extent:?})"
                    );
                    for v in extent.lo.iter().chain(extent.hi.iter()) {
                        assert!(
                            v.abs() < BOUND,
                            "{start:?} -> {target:?} at morph {morph}: the orbit \
                             reached {v}, past the {BOUND} bound ({extent:?})"
                        );
                    }
                    // Non-vacuity: a table that collapsed to a point would pass
                    // every assertion above and draw nothing. Every intermediate
                    // is a real figure with real extent.
                    let [hx, hy] = extent.half();
                    assert!(
                        hx.max(hy) > 0.01,
                        "{start:?} -> {target:?} at morph {morph}: collapsed to a \
                         point ({extent:?})"
                    );
                }
            }
        }
        assert_eq!(pairs, 25, "every ordered pair of the five figures");
    }

    /// The endpoints are the figures themselves, exactly — `morph = 0` is the
    /// configured figure and `morph = 1` is its target, with no drift from the
    /// lerp at either end.
    ///
    /// This is what makes an unbound `morph` a no-op rather than a slight
    /// distortion, and what lets `morph_to` be absent by giving both ends the
    /// same table.
    #[test]
    fn the_morph_endpoints_are_the_figures_themselves() {
        for start in IfsFigure::ALL {
            for target in IfsFigure::ALL {
                let (a, b) = (start.table(), target.table());
                assert_eq!(
                    resolve(&a, &b, 0.0),
                    a,
                    "{start:?} -> {target:?} at morph 0 must be {start:?}"
                );
                assert_eq!(
                    resolve(&a, &b, 1.0),
                    b,
                    "{start:?} -> {target:?} at morph 1 must be {target:?}"
                );
            }
        }
        // A figure morphing to itself is the identity at *every* position, which
        // is the absent-`morph_to` case: `morph` is inert by arithmetic rather
        // than by a branch on the render path.
        for figure in IfsFigure::ALL {
            let t = figure.table();
            for morph in morph_sweep() {
                assert_eq!(
                    resolve(&t, &t, morph),
                    t,
                    "{figure:?} morphing to itself moved at {morph}"
                );
            }
        }
    }

    /// `morph` is bindable, so a preset expression can hand `resolve` anything.
    /// Out of range it must **clamp**, not extrapolate — an extrapolated singular
    /// value is the one number that can leave the contractive ball.
    #[test]
    fn an_out_of_range_morph_clamps_rather_than_extrapolating() {
        let (a, b) = (IfsFigure::Fern.table(), IfsFigure::Spiral.table());
        for wild in [-5.0, -1.0, -0.001, 1.001, 2.0, 47.0] {
            let table = resolve(&a, &b, wild);
            let expected = if wild < 0.0 { a } else { b };
            assert_eq!(table, expected, "morph {wild} must clamp to an endpoint");
            assert!(table.sigma_max() < 1.0);
        }
        // A NaN — reachable from a preset expression like `0/0` — must not
        // produce a NaN table. `clamp` panics on a NaN bound but propagates a
        // NaN value, so this is asserted rather than assumed.
        let table = resolve(&a, &b, f32::NAN);
        assert!(
            table.maps.iter().all(|m| m.sigma_max().is_finite()),
            "a NaN morph produced a non-finite table: {table:?}"
        );
    }

    /// Angles take the **shortest arc**, which is visible rather than pedantic:
    /// a branch at `+3.0` rad and one at `−3.0` rad are a tenth of a turn apart,
    /// and a plain lerp would sweep it almost all the way round and back.
    #[test]
    fn angles_interpolate_the_short_way_round() {
        use std::f32::consts::PI;

        // Across the +/-pi discontinuity: the midpoint is just past pi, not 0.
        let mid = lerp_angle(3.0, -3.0, 0.5);
        assert!(
            (mid.abs() - PI).abs() < 0.15,
            "the midpoint of 3.0 -> -3.0 should sit near +/-pi, got {mid}"
        );
        // ...and the total travel is the short arc, not the long one.
        let travel = (lerp_angle(3.0, -3.0, 1.0) - 3.0).abs();
        assert!(
            travel < PI,
            "3.0 -> -3.0 travelled {travel} rad, the long way round"
        );

        // Endpoints are exact, and an ordinary in-range pair is the plain lerp.
        assert_eq!(lerp_angle(0.3, 1.1, 0.0), 0.3);
        assert!((lerp_angle(0.3, 1.1, 1.0) - 1.1).abs() < 1e-6);
        assert!((lerp_angle(0.0, 1.0, 0.5) - 0.5).abs() < 1e-6);
    }

    /// The CPU reference is deterministic and actually measures the figure —
    /// otherwise the sweep above is asserting something about noise.
    #[test]
    fn the_chaos_reference_is_deterministic_and_measures_the_figure() {
        let fern = IfsFigure::Fern.table();
        let a = chaos_extent(&fern, 20_000);
        let b = chaos_extent(&fern, 20_000);
        assert_eq!(
            a, b,
            "the CPU reference must be a pure function of its table"
        );

        // It finds the fern's published box: x in ~[-2.2, 2.7], y in ~[0, 10].
        assert!(a.lo[1] > -0.1 && a.lo[1] < 0.3, "fern y floor: {}", a.lo[1]);
        assert!(
            a.hi[1] > 9.0 && a.hi[1] < 10.2,
            "fern y ceiling: {}",
            a.hi[1]
        );
        assert!(
            a.lo[0] > -2.6 && a.lo[0] < -1.8,
            "fern x floor: {}",
            a.lo[0]
        );
        assert!(
            a.hi[0] > 2.2 && a.hi[0] < 3.0,
            "fern x ceiling: {}",
            a.hi[0]
        );

        // And it agrees with the measured literals `frame`/`seed_box` are built
        // from — the two must not drift, since Phase 4 replaces those literals
        // with this function's output.
        for figure in IfsFigure::ALL {
            let measured = chaos_extent(&figure.table(), 200_000);
            let ([hx, hy, _], [cx, cy, _]) = figure.seed_box();
            let [mcx, mcy] = measured.centre();
            let [mhx, mhy] = measured.half();
            for (got, want, axis) in [(mcx, cx, "centre x"), (mcy, cy, "centre y")] {
                assert!(
                    (got - want).abs() < 0.05 * (1.0 + want.abs()),
                    "{figure:?} {axis}: reference says {got}, the literal says {want}"
                );
            }
            for (got, want, axis) in [(mhx, hx, "half x"), (mhy, hy, "half y")] {
                assert!(
                    (got - want).abs() < 0.05 * (1.0 + want.abs()),
                    "{figure:?} {axis}: reference says {got}, the literal says {want}"
                );
            }
        }
    }

    // -----------------------------------------------------------------------
    // The fit (Plan 0062 Phase 4 / ADR-0075)
    // -----------------------------------------------------------------------

    /// The two aspects the framing has to hold at: a 16:9 landscape window and a
    /// 9:16 portrait one. The wide figures are the reason the second is checked —
    /// the dragon and the spiral are about twice as wide as they are tall, and a
    /// scale fitted at 16:9 leaves them hanging out of a portrait frame.
    const FRAMED_ASPECTS: [f32; 2] = [16.0 / 9.0, 9.0 / 16.0];

    /// **The figure stays in the frame, at every morph, at both aspects.**
    ///
    /// Asserted on the fitted extent rather than on pixels: what a capture could
    /// say is that the picture looked plausible at one size, and the claim is
    /// about every position of a continuous parameter at any window shape.
    #[test]
    fn every_shipped_pair_stays_framed_at_both_aspects() {
        for start in IfsFigure::ALL {
            for target in IfsFigure::ALL {
                let (a, b) = (start.table(), target.table());
                let fit = FitLut::build(&a, &b);
                // Sampled between the LUT's own entries too, so the assertion
                // covers the interpolation and not only the measured points.
                for i in 0..=128 {
                    let morph = i as f32 / 128.0;
                    let (centre, half) = fit.sample(morph);
                    assert!(
                        centre.iter().chain(half.iter()).all(|v| v.is_finite()),
                        "{start:?} -> {target:?} at {morph}: non-finite framing"
                    );
                    for aspect in FRAMED_ASPECTS {
                        let scale = fit_scale(half, aspect);
                        // The frame is NDC |y| <= 1 and |x| <= aspect (the
                        // vertex shader divides x by the aspect).
                        let (ndc_x, ndc_y) = (half[0] * scale / aspect, half[1] * scale);
                        assert!(
                            ndc_x <= 1.0 && ndc_y <= 1.0,
                            "{start:?} -> {target:?} at morph {morph}, aspect {aspect}: \
                             the figure reaches ({ndc_x}, {ndc_y}) in NDC"
                        );
                        // Non-vacuity: it fills the frame along its binding axis
                        // rather than shrinking to a dot to stay inside.
                        assert!(
                            ndc_x.max(ndc_y) > 0.7,
                            "{start:?} -> {target:?} at morph {morph}, aspect {aspect}: \
                             occupies only {} of the binding axis",
                            ndc_x.max(ndc_y)
                        );
                    }
                }
            }
        }
    }

    /// The fit's endpoints are each figure's own framing — so a preset that
    /// never binds `morph` is framed exactly as the single figure it named.
    #[test]
    fn the_fit_endpoints_are_the_figures_own_framing() {
        for start in IfsFigure::ALL {
            for target in IfsFigure::ALL {
                let fit = FitLut::build(&start.table(), &target.table());
                for (morph, figure) in [(0.0, start), (1.0, target)] {
                    let (centre, half) = fit.sample(morph);
                    let solo = chaos_extent(&figure.table(), super::FIT_ITERATIONS);
                    assert_eq!(
                        (centre, half),
                        (solo.centre(), solo.half()),
                        "{start:?} -> {target:?} at morph {morph} must frame {figure:?} \
                         exactly as it frames itself"
                    );
                }
            }
        }
    }

    /// The fit is a **function of `morph` and the figure pair only** — the
    /// property the whole design rests on, since a fit that saw `vigor` would
    /// cancel it (ADR-0075 Alternative C).
    ///
    /// Phase 4 can assert the structural half: the builder takes two tables and
    /// nothing else, and the same pair produces a bit-identical table however
    /// many times it is built. Phase 5 asserts the half that needs levers to
    /// exist — that moving all four to their extremes does not move this.
    #[test]
    fn the_fit_is_reproducible_and_sees_nothing_but_the_pair() {
        for start in IfsFigure::ALL {
            for target in IfsFigure::ALL {
                let (a, b) = (start.table(), target.table());
                assert_eq!(
                    FitLut::build(&a, &b),
                    FitLut::build(&a, &b),
                    "{start:?} -> {target:?}: the fit is not reproducible, so a \
                     capture's framing would depend on when it ran"
                );
            }
        }
        // Different pairs give different fits — otherwise the assertion above
        // holds for a builder that ignores its arguments.
        assert_ne!(
            FitLut::build(&IfsFigure::Fern.table(), &IfsFigure::Fern.table()),
            FitLut::build(&IfsFigure::Fern.table(), &IfsFigure::Spiral.table())
        );
    }

    /// `morph` is bindable, so the LUT is sampled with whatever a preset
    /// produces. Out of range and at `NaN` it must stay on the table.
    #[test]
    fn sampling_the_fit_out_of_range_stays_on_the_table() {
        let fit = FitLut::build(&IfsFigure::Fern.table(), &IfsFigure::Dragon.table());
        let at0 = fit.sample(0.0);
        let at1 = fit.sample(1.0);
        for (wild, expected) in [
            (-1.0, at0),
            (-0.001, at0),
            (f32::NAN, at0),
            (1.001, at1),
            (99.0, at1),
        ] {
            assert_eq!(fit.sample(wild), expected, "sampling at {wild}");
        }
        // And the table really has FIT_STEPS entries the sampler walks — an
        // off-by-one at the top end would read past the last.
        assert_eq!(FIT_STEPS, 33);
        assert!(fit.sample(1.0).1.iter().all(|v| v.is_finite()));
    }

    /// The fit is built at `configure`, so it has to sit inside the
    /// preset-switch budget (~150 ms).
    ///
    /// **Asserted on the work rather than on a wall clock**, and not only
    /// because this crate forbids reading one (the determinism rule, enforced by
    /// a clippy `disallowed_methods` entry). An iteration count is the thing the
    /// cost is actually proportional to, it is the same number on every machine,
    /// and it is what an order-of-magnitude mistake would move — a chaos game
    /// per frame, or an iteration count with an extra zero. A timing assertion
    /// would measure the CI runner's load instead and flake.
    ///
    /// The constant is calibrated against a measured run: the 140 448 iterations
    /// this table costs take **0.51 ms** in an optimized build, so a 500 000
    /// ceiling is under 2 ms — about 1 % of the switch budget.
    #[test]
    fn the_fit_is_built_inside_the_preset_switch_budget() {
        const ITERATION_CEILING: u32 = 500_000;
        let total = FIT_STEPS as u32 * (super::FIT_ITERATIONS + super::CHAOS_BURN_IN);
        assert!(
            total <= ITERATION_CEILING,
            "the fit runs {total} chaos-game iterations at every preset switch, \
             past the {ITERATION_CEILING} ceiling"
        );

        // Non-vacuity: not so few iterations that the box is noise. How close
        // it has to be is the next test's question, not this one's.
        const { assert!(super::FIT_ITERATIONS >= 1_000) };
        assert!(
            FitLut::build(&IfsFigure::Fern.table(), &IfsFigure::Spiral.table())
                .sample(0.5)
                .1
                .iter()
                .all(|h| *h > 0.0)
        );
    }

    /// **What the fit under-measures, [`FRAME_FILL`](super::FRAME_FILL)'s margin
    /// has to absorb** — and this is the assertion that makes
    /// `every_shipped_pair_stays_framed_at_both_aspects` a claim about the
    /// figure rather than about the lookup's own numbers.
    ///
    /// A sampled bounding box is a **maximum over a finite orbit**, so it
    /// converges from below: the fit always measures a figure slightly smaller
    /// than it is, and therefore draws it slightly larger than it planned. If the
    /// under-measure is `e`, the true fill is `FRAME_FILL / (1 - e)`, so the
    /// figure stays inside the frame exactly while `e < 1 - FRAME_FILL`.
    ///
    /// **Iterating the error away is not available, and that is worth knowing
    /// before someone tries.** The tree's trunk map is `(0, 0, 0, 0.5)` — it
    /// halves `y` towards zero — so the figure's true infimum in `y` is
    /// approached and never reached, and the sampled box keeps creeping for as
    /// long as anyone is willing to iterate. Measured: **7.9 % under at 4 000
    /// iterations and still 6.6 % at 32 000**, for eight times the load cost.
    /// The margin is the mechanism; the iteration count only has to keep the
    /// error inside it.
    ///
    /// So this asserts the **consequence** — the fraction of the frame the true
    /// figure occupies — rather than the error itself. The error is a proxy with
    /// an arbitrary budget; the fill is the property, and a figure whose fill
    /// crosses 1 is a figure hanging out of the frame.
    #[test]
    fn the_fit_leaves_margin_for_what_it_under_measures() {
        /// How much of the frame the true figure may occupy along its binding
        /// axis. Below `1.0` is the property; this leaves visible daylight so a
        /// pass is not a near-miss.
        const FILL_CEILING: f32 = 0.97;

        let mut cases: Vec<(String, super::IfsTable)> = IfsFigure::ALL
            .iter()
            .map(|f| (format!("{f:?}"), f.table()))
            .collect();
        for (start, target) in [
            (IfsFigure::Fern, IfsFigure::Spiral),
            (IfsFigure::Fern, IfsFigure::Dragon),
        ] {
            let (a, b) = (start.table(), target.table());
            for morph in [0.25, 0.5, 0.75] {
                cases.push((
                    format!("{start:?}->{target:?}@{morph}"),
                    resolve(&a, &b, morph),
                ));
            }
        }

        let mut worst_fill = 0.0f32;
        for (name, table) in &cases {
            let quick = chaos_extent(table, super::FIT_ITERATIONS).half();
            let long = chaos_extent(table, 200_000).half();
            for axis in 0..2 {
                let (Some(q), Some(c)) = (quick.get(axis), long.get(axis)) else {
                    continue;
                };
                // The fit scales so the MEASURED half-extent occupies
                // `FRAME_FILL`; the TRUE half-extent then occupies this.
                let fill = super::FRAME_FILL * c / q;
                worst_fill = worst_fill.max(fill);
                assert!(
                    fill < FILL_CEILING,
                    "{name} axis {axis}: the fit measures {q} against a long-run \
                     {c}, so the true figure fills {:.3} of the frame — past the \
                     {FILL_CEILING} ceiling, and {:.3} is where it leaves it",
                    fill,
                    1.0f32
                );
            }
        }
        // Non-vacuity in both directions: the under-measure is real (so the
        // margin is doing work rather than the two runs agreeing exactly), and
        // it never inverts (a sampled maximum cannot over-measure).
        assert!(
            worst_fill > super::FRAME_FILL + 0.001,
            "no figure under-measured at all ({worst_fill}), so this test is not \
             measuring what it claims"
        );
    }

    /// The aspect-aware fit is what makes a wide figure legal in a narrow
    /// window, so the scale must genuinely differ between the two aspects for a
    /// wide figure and not for a tall one.
    #[test]
    fn the_fit_scale_binds_on_whichever_axis_is_tighter() {
        // The fern is tall: vertically bound at both aspects, same scale.
        let fern_half = chaos_extent(&IfsFigure::Fern.table(), 20_000).half();
        assert!(
            (fit_scale(fern_half, 16.0 / 9.0) - fit_scale(fern_half, 9.0 / 16.0)).abs() < 1e-6,
            "a tall figure is vertically bound at every aspect"
        );

        // The dragon is wide: horizontally bound in portrait, so its scale must
        // drop — that is the whole point of taking the aspect.
        let dragon_half = chaos_extent(&IfsFigure::Dragon.table(), 20_000).half();
        assert!(
            fit_scale(dragon_half, 9.0 / 16.0) < fit_scale(dragon_half, 16.0 / 9.0) * 0.6,
            "a wide figure must shrink in a portrait window"
        );

        // A degenerate half-extent cannot produce an infinity on its way to a
        // uniform — `Extent::half` floors it, and this states the consequence.
        assert!(fit_scale([1e-6, 1e-6], 1.0).is_finite());
        assert!(fit_scale([1.0, 1.0], 0.0).is_finite());
    }
}
