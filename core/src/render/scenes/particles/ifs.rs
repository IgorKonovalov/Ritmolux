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

    /// `(world scale, centre)` — the projection's framing for this figure.
    ///
    /// Both are **measured**, from a fixed-seed 400 000-iteration run of the
    /// chaos game on the curated table: the centre is the sampled bounding box's
    /// midpoint and the scale fits its larger half-extent to `0.88` of the frame.
    ///
    /// **Fitted at a 16:9 reference**, which is what the four map families do
    /// too — their world scales are single constants. The wide figures (dragon,
    /// spiral) therefore overflow a portrait window horizontally, and that is
    /// what Phase 4 fixes when it replaces this with a lookup over `morph` and
    /// makes the scale aspect-aware. The fern, which is tall, is inside the frame
    /// at both aspects already.
    ///
    /// The fern is also the reason
    /// [`projection`](super::AttractorFamily::projection) carries a full
    /// three-component centre rather than a z-centre: it spans `y ∈ [0, 10]` and
    /// is not origin-centred, so a projection that subtracts nothing puts its
    /// root on the bottom edge and its canopy off the top.
    pub fn frame(self) -> (f32, [f32; 3]) {
        let (centre, half) = self.extent();
        let [cx, cy] = centre;
        let [_, hy] = half;
        let scale = match self {
            // Phase 1's measured value, kept exactly: Phase 2 changes how the
            // table is *represented*, and a re-derived scale here would move the
            // figure for a reason that has nothing to do with the SVD.
            IfsFigure::Fern => 0.168,
            _ => (FRAME_FILL / hy).min(FRAME_FILL * REFERENCE_ASPECT / half[0]),
        };
        (scale, [cx, cy, 0.0])
    }

    /// The figure's sampled bounding box as `(centre, half-extent)`, in its own
    /// world units.
    ///
    /// Measured, and quoted here rather than recomputed, because a fixed-seed
    /// chaos game at load would be the same numbers at a cost. Phase 4 replaces
    /// these five literals with a 33-entry lookup over `morph`, built from a
    /// fixed-seed run of the CPU reference at `configure` time — at which point
    /// the measurement moves into code and these become its endpoints.
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
/// The aspect [`IfsFigure::frame`] fits against until Phase 4 makes it the
/// target's.
const REFERENCE_ASPECT: f32 = 16.0 / 9.0;

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

    use super::{IfsFigure, MAPS, decompose, recompose};

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

    /// Every figure is framed inside the 16:9 reference frame it is fitted
    /// against, and the fern — which is tall — is inside a portrait frame too.
    ///
    /// Phase 4 strengthens this to every figure at every aspect, which is what
    /// the aspect-aware fit buys.
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

        // The fern's Phase 1 claim, kept: it is framed at portrait too, and its
        // scale is the measured value Phase 1 shipped rather than a re-derived
        // one — Phase 2 changes the representation, not the picture.
        let (scale, centre) = IfsFigure::Fern.frame();
        assert_eq!(scale, 0.168);
        assert_eq!(centre, [0.237, 4.999, 0.0]);
        let ([hx, hy, _], _) = IfsFigure::Fern.seed_box();
        assert!(hy * scale < 1.0);
        assert!(hx * scale / (9.0 / 16.0) < 1.0);
    }
}
