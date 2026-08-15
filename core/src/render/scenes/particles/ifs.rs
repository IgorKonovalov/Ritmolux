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
    /// The four respawn targets (ADR-0087), packed two `(x, y)` per row exactly
    /// as [`translate`](Self::translate) is. Straight from [`fixed_points`], so
    /// the padded slots already duplicate a drawn map and the shader picks one of
    /// four with no branch and no knowledge of the probability table.
    pub fixed: [[f32; 4]; 2],
    /// The reciprocal of [`skeleton_scale`] (ADR-0088) — what the step shader
    /// multiplies a raw nearest-fixed-point distance by to get the `[0, 1]`-ish
    /// colour coordinate it stores on the particle.
    ///
    /// Shipped as a reciprocal rather than as the diameter because the shader
    /// then multiplies where it would otherwise divide, per particle per step.
    pub root_recip: f32,
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
        fixed: [[0.0; 4]; 2],
        // Zero rather than the floor's reciprocal, for the same reason the table
        // above is zeroed: a family that reached this data by mistake reads a
        // distance of exactly 0 everywhere, which is an inert channel rather than
        // a gradient measured against somebody else's figure.
        root_recip: 0.0,
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

/// The fraction of the frame a fitted figure occupies along its binding axis
/// **at zero rotation** (ADR-0103).
///
/// The remaining `0.12` is margin against what the fit under-measures, not
/// against what turns: a figure of aspect `a = hx / hy` reaches
/// `FRAME_FILL · sqrt(1 + a²)` of the frame at its worst spin angle, so only a
/// figure at or under `sqrt(1/FRAME_FILL² − 1)` — about `1.85x` taller than
/// wide — stays inside at every angle. Of the shipped roster only the fern
/// does. See `the_fit_frames_a_figure_that_does_not_turn`.
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
/// **Rotation is a separate axis, and this does not cover it** (ADR-0103). The
/// box being fitted is axis-aligned, and `project`'s 2D branch rotates it by
/// the spin phase afterwards — `spin` defaults to on — so what this guarantees
/// is "inside the frame at neutral levers **and zero rotation**". A rotated
/// figure reaches `hypot(hx, hy)` on both axes at its worst angle, which for
/// every shipped figure but the fern is outside the frame. `zoom` is the
/// recourse, the same one the levers have.
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

/// The ceiling every map's operator norm is held under (ADR-0075).
///
/// **A look constant with no principled value**, and it is worth being honest
/// about that. It is far enough below `1.0` that floating-point error cannot
/// cross it, and it leaves the fern's largest singular value — `0.851` — about
/// 17 % to grow, which is what `vigor` has to work with. Whether that is enough
/// to feel like a surge is a question for the content pass; if it is too tight,
/// **this constant is the lever and widening the parameterization is not**.
///
/// A preset asking for more `vigor` than this allows gets silence rather than an
/// error — the same undiscoverable-ceiling shape `presets/README.md` already
/// documents for `bloom_threshold` and `perspective`.
pub const SIGMA_CEILING: f32 = 0.97;

/// How far [`Levers::bias`] may shift the sampling weight, as a fraction.
///
/// At `1.0` a full-scale `bias` would take one group's probability to exactly
/// zero, which stops drawing part of the figure rather than re-weighting it —
/// the orbit would then converge onto the *sub*-system of the maps that remain,
/// a much smaller attractor that the neutral-lever fit does not frame. `0.6`
/// keeps every map drawn at both extremes.
const BIAS_DEPTH: f32 = 0.6;

/// The four audio-driven shape levers (ADR-0075), applied in SVD space.
///
/// **Built to be safe rather than checked**, which is the whole point of the
/// parameterization: `curl` and `lean` are rotations and cannot affect
/// contractivity at all, `bias` moves probabilities and changes only where
/// points land, and `vigor` is the one that touches the singular values — so it
/// is the one, and the only one, behind a clamp.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Levers {
    /// Radians added to **every** map's `θ`. Fronds curl and uncurl.
    /// Unconditionally safe: `R` is an isometry.
    pub curl: f32,
    /// Multiplier on every singular value, under [`SIGMA_CEILING`]. A bushier,
    /// deeper, denser figure — and the only lever that can reach the cliff.
    pub vigor: f32,
    /// Radians every translation vector is rotated about the origin by, bending
    /// the plant. Translations do not enter contractivity, so this is
    /// unconditionally safe.
    pub lean: f32,
    /// Shifts sampling weight between the **body** maps (canonical indices 0
    /// and 1) and the **branch** maps (2 and 3), renormalizing. The shape is
    /// untouched and only the density distribution moves — the cheapest
    /// genuinely organic response in the set.
    ///
    /// Inert on the dragon, whose two real maps both live in the branch slots
    /// (its body slots are the padding), so there is nothing to shift weight
    /// away from. Worth a note in the content pass rather than a special case.
    pub bias: f32,
}

impl Levers {
    /// Every lever at rest. The fit is built here, and `resolve` at these values
    /// is **bit-identical** to `resolve` with no levers at all — every operation
    /// below is guarded so that neutrality is exact rather than approximate.
    pub const NEUTRAL: Self = Self {
        curl: 0.0,
        vigor: 1.0,
        lean: 0.0,
        bias: 0.0,
    };

    /// The documented extremes, for the sweep that has to cover them.
    ///
    /// `curl` and `lean` are angles, so their "extreme" is a look choice rather
    /// than a limit — a full turn is the same figure. `vigor` is quoted well
    /// past what [`SIGMA_CEILING`] will grant, precisely so the sweep exercises
    /// the clamp; `bias` is quoted at the ends of its own range.
    pub const EXTREMES: [Self; 2] = [
        Self {
            curl: -1.5,
            vigor: 0.4,
            lean: -1.5,
            bias: -1.0,
        },
        Self {
            curl: 1.5,
            vigor: 2.5,
            lean: 1.5,
            bias: 1.0,
        },
    ];
}

/// **The pure function everything safety-critical lives in** — no GPU, no clock,
/// no randomness (ADR-0075).
///
/// Interpolates two figures **in SVD space**, map by map, paired by index:
/// singular values and translations lerped, angles taken along the shortest arc,
/// probabilities lerped. Then applies the four levers, in the same space.
///
/// The morph is contractive by construction and there is no clamp making it so.
/// A lerp of two values below 1 is below 1, and the angles do not enter
/// contractivity at all — so if both endpoints converge, so does every point on
/// the path between them. Of the levers, only `vigor` can reach the cliff, and
/// it is held under [`SIGMA_CEILING`] by one comparison on one number.
///
/// Degenerate maps stay legal rather than being special-cased: the fern's stem
/// is rank 1 (`sx = 0`), and morphing a reflection into a non-reflection passes
/// through `sy = 0`. Both are contractions — the branch momentarily collapses to
/// a line and recovers.
pub fn resolve(a: &IfsTable, b: &IfsTable, morph: f32, levers: Levers) -> IfsTable {
    apply_levers(morph_tables(a, b, morph), levers)
}

/// The morph half of [`resolve`], separated so the fit — which must never see a
/// lever — can call it and be structurally unable to pass one.
fn morph_tables(a: &IfsTable, b: &IfsTable, morph: f32) -> IfsTable {
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

/// Apply the four levers to a resolved table, in SVD space.
///
/// **Every step is guarded so that neutrality is exact.** That is not tidiness:
/// the fit LUT is built at neutral, so a lever that perturbed the table by an
/// ulp at its rest value would make the framing disagree with the figure, and
/// would move a golden baseline for a preset that binds nothing.
fn apply_levers(mut table: IfsTable, levers: Levers) -> IfsTable {
    let Levers {
        curl,
        vigor,
        lean,
        bias,
    } = levers;

    // `curl` — a shared rotation added after the scale. Cannot affect
    // contractivity: `R` is an isometry, so this needs no bound of any kind.
    if curl != 0.0 && curl.is_finite() {
        for map in &mut table.maps {
            map.theta += curl;
        }
    }

    // `lean` — the translations rotated about the origin. Translations do not
    // enter contractivity either, so this is unconditionally safe.
    if lean != 0.0 && lean.is_finite() {
        let (sn, cs) = lean.sin_cos();
        for map in &mut table.maps {
            let [x, y] = map.t;
            map.t = [x * cs - y * sn, x * sn + y * cs];
        }
    }

    // `vigor` — **the one lever that can reach the cliff**, and the only one
    // behind a clamp. A non-finite or non-positive value is treated as neutral
    // rather than propagated: a preset expression can produce either, and a zero
    // scale would collapse the figure to its fixed points.
    if vigor != 1.0 && vigor.is_finite() && vigor > 0.0 {
        for map in &mut table.maps {
            map.sx *= vigor;
            map.sy *= vigor;
        }
    }
    // Run unconditionally, so the ceiling holds whatever route the table took
    // here — the compare is the whole cost, and it removes a dependency on
    // reasoning about what the morph can produce.
    //
    // The **whole table** is scaled by one factor rather than each map clamped
    // separately: clamping per map would change the figure's proportions, which
    // is a different shape rather than a smaller one.
    let sigma = table.sigma_max();
    if sigma > SIGMA_CEILING {
        let shrink = SIGMA_CEILING / sigma;
        for map in &mut table.maps {
            map.sx *= shrink;
            map.sy *= shrink;
        }
    }

    // `bias` — weight moved between the body maps (canonical 0 and 1) and the
    // branch maps (2 and 3). Multiplicative and bounded by [`BIAS_DEPTH`], so no
    // probability can go negative and none can reach zero.
    if bias != 0.0 && bias.is_finite() {
        let b = bias.clamp(-1.0, 1.0) * BIAS_DEPTH;
        let mut total = 0.0;
        for (i, map) in table.maps.iter_mut().enumerate() {
            map.p *= if i < 2 { 1.0 - b } else { 1.0 + b };
            total += map.p;
        }
        // A table whose weight is entirely in one group (the dragon's is) can
        // still renormalize; a table with no weight at all cannot, and dividing
        // by it would put a NaN in the cumulative table.
        if total > 0.0 {
            for map in &mut table.maps {
                map.p /= total;
            }
        }
    }

    table
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
///
/// **`spin` is the second unmodelled input, and unlike `vigor` it defaults to
/// on** (ADR-0103). The table holds axis-aligned half-extents; the projection
/// rotates them. So the framing this buys is "at neutral levers and zero
/// rotation", `zoom` is the recourse for both, and the three shipped 2D worlds
/// each carry a sub-1 base `zoom` for exactly this reason.
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
            let extent = chaos_extent(&morph_tables(a, b, morph), FIT_ITERATIONS);
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

/// One map's fixed point `(I − M)⁻¹ t` — **a point that is on the attractor**.
///
/// That it lies on `A` is a consequence of the parameterization rather than of
/// anything this plan added: `A = ⋃ fᵢ(A)` with `A` closed makes each `fᵢ`'s
/// fixed point the limit of `fᵢⁿ(x)` for any `x ∈ A` (ADR-0075's Notes,
/// ADR-0087). **It does not exist for De Jong, Clifford, Thomas or Lorenz**, and
/// that is why the respawn ADR-0087 builds on this is IFS-only structurally
/// rather than by default.
///
/// **Closed form, and unguarded on purpose.** For `M = [[a, b], [c, d]]`,
/// `(I − M)⁻¹` is `1/Δ · [[1 − d, b], [c, 1 − a]]` with
/// `Δ = (1 − a)(1 − d) − bc`. `Δ ≠ 0` **follows from contractivity rather than
/// being checked**: `Δ` is `det(I − M)`, which vanishes only if `M` has an
/// eigenvalue of `1`, which `σ_max < 1` forbids — and [`SIGMA_CEILING`] is
/// enforced on every reachable table. The magnitude is bounded the same way,
/// `‖(I − M)⁻¹‖ ≤ 1/(1 − σ_max)`, at most `33.3` under the `0.97` ceiling. So
/// there is nothing here for a caller to fall back to, which is the same shape
/// as the rest of this module: safety by construction, not by a guard.
fn fixed_point(map: &IfsMap) -> [f32; 2] {
    let Affine { a, b, c, d, e, f } = map.to_affine();
    let delta = (1.0 - a) * (1.0 - d) - b * c;
    [
        ((1.0 - d) * e + b * f) / delta,
        (c * e + (1.0 - a) * f) / delta,
    ]
}

/// Every map's fixed point, **with the padded slots filled by duplication**.
///
/// A table always carries [`MAPS`] maps, but a figure with fewer duplicates one
/// at probability `0` — and a padded slot's fixed point is on the attractor only
/// when the pad happens to duplicate a drawn map. That is true of all five
/// curated tables today and is exactly the sort of thing that stops being true
/// when a sixth figure is added, so this returns only the `p > 0` maps' points,
/// repeated around all four slots. Every consumer then picks one of four
/// unconditionally: no branch, and no knowledge of the probability table.
///
/// The `p > 0` test survives the levers: `bias` is multiplicative and bounded by
/// [`BIAS_DEPTH`], so it can neither zero a drawn map nor revive a pad.
pub fn fixed_points(table: &IfsTable) -> [[f32; 2]; MAPS] {
    let mut drawn = [[0.0f32; 2]; MAPS];
    let mut count = 0usize;
    for map in table.maps.iter().filter(|m| m.p > 0.0) {
        if let Some(slot) = drawn.get_mut(count) {
            *slot = fixed_point(map);
        }
        count += 1;
    }
    let mut out = [[0.0f32; 2]; MAPS];
    for i in 0..MAPS {
        // A table with no drawn map at all is not reachable — every curated
        // figure has at least two — and `count == 0` would be a modulo by zero
        // here rather than a wrong picture, so it is spelled out.
        let source = if count == 0 {
            None
        } else {
            drawn.get(i % count)
        };
        if let (Some(slot), Some(point)) = (out.get_mut(i), source) {
            *slot = *point;
        }
    }
    out
}

/// Floor on the fixed-point set's diameter (ADR-0088).
///
/// **Required rather than defensive.** Two drawn maps' fixed points genuinely
/// approach each other as a morph interpolates between two tables, and a
/// diameter of zero makes [`skeleton_scale`]'s reciprocal diverge — every
/// particle's stored distance would come back `inf` or `NaN` and the whole
/// figure would sample one end of the palette.
///
/// It is **a constant somebody picked**, in the same position ADR-0075's `0.97`
/// and ADR-0087's `180` occupy, but it is bounded by measurement rather than by
/// taste: `the_skeleton_never_collapses_across_the_morph` sweeps every ordered
/// figure pair, every position of the 33-point morph sweep and all three lever
/// settings, and asserts the observed minimum diameter sits above this while
/// printing the margin and where it occurred. A thin margin there does not mean
/// "tune the floor" — it means the channel degenerates somewhere in the morph
/// and the figure pair the assertion names is where to look.
pub const SKELETON_FLOOR: f32 = 0.05;

/// The **diameter of the fixed-point set**: `max over j, k of ‖pⱼ − pₖ‖`.
///
/// At most six pairwise distances over [`fixed_points`]' four slots. The padded
/// slots duplicate drawn maps, so a duplicate contributes a zero distance and
/// this is the *drawn* set's diameter exactly — the same property that lets the
/// respawn pick one of four with no branch.
///
/// **Closed form, and deliberately not [`chaos_extent`].** A bounding box is a
/// supremum statistic fixed by the single rarest point an orbit reached: two
/// runs of `chaos_extent` on the same table disagree by `0.046` at 20 000
/// iterations and by `0.143` at 100 000, and the disagreement *grows* (ADR-0087
/// Notes, ADR-0088 Alternative D). Normalising a colour coordinate by that would
/// make the gradient's scale wobble across a morph by an amount nobody chose,
/// and cost a Monte Carlo per preset switch. This is exact, deterministic, and a
/// continuous function of the table, so it moves across a morph exactly as
/// smoothly as the points themselves do.
///
/// It is also the *meaningful* scale — the figure's own skeleton rather than a
/// box drawn around its excursions — which is why a particle can legitimately
/// measure past `1`. That is clamped at the read, not here.
pub fn skeleton_diameter(table: &IfsTable) -> f32 {
    let points = fixed_points(table);
    let mut max = 0.0f32;
    for (i, [ax, ay]) in points.into_iter().enumerate() {
        for [bx, by] in points.into_iter().skip(i + 1) {
            max = max.max((bx - ax).hypot(by - ay));
        }
    }
    max
}

/// [`skeleton_diameter`] held above [`SKELETON_FLOOR`] — the scale the GPU
/// normalises a particle's distance-from-the-skeleton against.
///
/// The floored value rather than the raw one is what ships; the raw one is what
/// the sweep measures, which is the only way to find out whether the floor is
/// doing nothing (good) or is load-bearing (a finding).
pub fn skeleton_scale(table: &IfsTable) -> f32 {
    skeleton_diameter(table).max(SKELETON_FLOOR)
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
    // The respawn targets ride the same packing (ADR-0087), so the one function
    // that lays a table out for the GPU lays out all of it — a caller cannot
    // upload a table and forget its fixed points.
    let points = fixed_points(table);
    let mut fixed = [[0.0f32; 4]; 2];
    for (i, [x, y]) in points.into_iter().enumerate() {
        if let Some(row) = fixed.get_mut(i / 2) {
            let half = (i % 2) * 2;
            if let Some(slot) = row.get_mut(half) {
                *slot = x;
            }
            if let Some(slot) = row.get_mut(half + 1) {
                *slot = y;
            }
        }
    }
    IfsPacked {
        linear,
        translate,
        cumulative_p,
        fixed,
        // ...and so does the scale those points are measured against (ADR-0088),
        // for the same reason: one function lays a table out for the GPU, so a
        // caller cannot upload the skeleton and forget its size.
        root_recip: 1.0 / skeleton_scale(table),
    }
}

#[cfg(test)]
mod tests;
