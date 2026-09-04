//! The motif roster: the closed set of shapes a `[generator] rings` entry may
//! name, and the geometry each one is (ADR-0079).
//!
//! Pure shape arithmetic in a motif's own local frame -- an outline, an exact
//! arc, or a fitted G1 chain of arcs -- with no ring, no placement and no
//! renderer. [`rings`](super::rings) is what places these; the two do not talk.

// Hot-path panic-denial pragma (Plan 0002 Phase 2; render/ is scanned by the
// hygiene guard).
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

// A continuation of one module split across three files, so it needs the names
// `star/mod.rs` has in scope.
use super::*;

/// The **closed, curated** motif roster (ADR-0079): the shapes a `[generator]
/// rings` entry may repeat around a ring.
///
/// Closed on purpose, and this is the boundary the decision draws. Each motif is
/// a parametric outline sampled to segments — the same thing `parametric_curve`
/// already does, placed rather than drawn once — so making the set authorable
/// would be a drawing language rather than a parameter, with no natural stopping
/// point (ADR-0079 Alternative C). A look outside the roster routes back through
/// `architect` + `dev`.
///
/// **Local convention, and every outline below obeys it:** a motif is authored
/// about its own centre, spanning roughly one unit, with **outward** (away from
/// the frame centre) along `+x`. Placement is then one rotation for both the
/// orientation and the position — see `build_rings`.
///
/// **The roster closed at seven on 2026-08-06** (Plan 0065 Phase 3), picked from
/// the rendered sample sheets rather than from names. Two of the nine provisional
/// members were **cut**, and the property they were cut on is worth keeping
/// because the next candidate meets it too: *does it hold its identity across the
/// whole 8-to-32 count range*.
///
/// - **`star`** is an ornament at x8 and dissolves into texture by x32.
/// - **`triangle`** duplicates [`Chevron`](Motif::Chevron)'s sawtooth role at
///   roughly twelve times the segment cost — `chevron` is 2 segments, the
///   cheapest member in the set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motif {
    /// A closed circle — the plainest bead, and the ring that reads as a dotted
    /// orbit.
    Circle,
    /// A pointed oval (a vesica), pointed at **both** ends along the radius.
    Petal,
    /// Round at the outer end, cusped at the inner one — the classic paisley
    /// drop, and the one motif with an unambiguous "which way is out".
    Teardrop,
    /// A four-vertex rhombus, long along the radius.
    Diamond,
    /// An **open** circular arc bulging outward, chord tangential. One bead
    /// among the others, and **not** the roster's answer to a scalloped
    /// boundary.
    ///
    /// A ring of these approximates one, and at Plan 0065 Phase 2 the user was
    /// shown that side by side with a genuine boundary *curve primitive* and
    /// picked the primitive (design-backlog 0071). That is
    /// [`Scallop`](Motif::Scallop), a single closed chain rather than a ring of
    /// copies faking continuity. Reach for that when you want a boundary, and
    /// for this when you want an open arc.
    Arc,
    /// A three-lobed rose, `r = |cos(3*theta/2)|` — the densest member, and the
    /// one that reads as ornament rather than as a bead.
    Trefoil,
    /// An **open** two-segment chevron, apex outward. The cheapest motif in the
    /// roster at two segments a copy.
    Chevron,
    /// The **scalloped boundary** — one closed chain of outward-bulging arcs
    /// meeting at cusps, and the only roster member that is a *figure* rather
    /// than a bead repeated around a ring.
    ///
    /// **This is the primitive the user chose over an approximation of it**
    /// (Plan 0065 Phase 2, design-backlog 0071). Shown a ring of overlapping
    /// [`Arc`](Motif::Arc) motifs faking a continuous boundary side by side with
    /// the real thing, they picked the real thing — and building one needs the
    /// renderer's arc instance (Plan 0087).
    ///
    /// **It reads the ring's own fields, and each keeps its spirit** — see
    /// `build_rings`. `count` is the **lobe count** rather than a copy count,
    /// because the lobes are one chain and there is nothing to repeat; `radius`
    /// is the base circle they bulge from, which is exactly the ring a boundary
    /// sits on; `scale` is the **depth** of the bulge, which is the only size a
    /// lobe has once `count` has fixed its width; and `phase` turns the whole
    /// boundary, as it turns every other ring.
    Scallop,
}

impl Motif {
    /// Every motif, in roster order — **the closed set**, and the list a load
    /// error names when a preset asks for something outside it.
    pub const ALL: &'static [Motif] = &[
        Motif::Circle,
        Motif::Petal,
        Motif::Teardrop,
        Motif::Diamond,
        Motif::Arc,
        Motif::Trefoil,
        Motif::Chevron,
        Motif::Scallop,
    ];

    /// The `motif = "..."` name a preset writes. `None` for anything outside the
    /// roster — the loader turns that into a surfaced error, never a fallback.
    pub fn from_name(name: &str) -> Option<Motif> {
        Some(match name.trim() {
            "circle" => Motif::Circle,
            "petal" => Motif::Petal,
            "teardrop" => Motif::Teardrop,
            "diamond" => Motif::Diamond,
            "arc" => Motif::Arc,
            "trefoil" => Motif::Trefoil,
            "chevron" => Motif::Chevron,
            "scallop" => Motif::Scallop,
            _ => return None,
        })
    }

    /// The roster name, for error messages and the sample index.
    pub fn name(self) -> &'static str {
        match self {
            Motif::Circle => "circle",
            Motif::Petal => "petal",
            Motif::Teardrop => "teardrop",
            Motif::Diamond => "diamond",
            Motif::Arc => "arc",
            Motif::Trefoil => "trefoil",
            Motif::Chevron => "chevron",
            Motif::Scallop => "scallop",
        }
    }

    /// Whether the outline closes back onto its first vertex. Open motifs
    /// ([`Arc`](Motif::Arc), [`Chevron`](Motif::Chevron)) emit one segment fewer
    /// than they have vertices and leave their two free ends unjoined.
    pub(super) fn is_closed(self) -> bool {
        !matches!(self, Motif::Arc | Motif::Chevron)
    }

    /// Whether this motif is the closed [`Scallop`](Motif::Scallop) boundary,
    /// whose ring is **one chain of `count` lobes** rather than `count` copies
    /// of anything.
    pub(crate) fn is_scallop(self) -> bool {
        matches!(self, Motif::Scallop)
    }

    /// Vertices in one copy of this motif.
    pub(super) fn vertex_count(self) -> usize {
        match self {
            Motif::Circle | Motif::Petal | Motif::Teardrop => SMOOTH_SAMPLES,
            Motif::Diamond => 4,
            Motif::Arc => ARC_SAMPLES + 1,
            Motif::Trefoil => TREFOIL_SAMPLES,
            Motif::Chevron => 3,
            // The base circle its lobes bulge from — see `outline`.
            Motif::Scallop => SMOOTH_SAMPLES,
        }
    }

    /// This motif as **one exact circular arc**, when it is one (ADR-0098):
    /// a centre, a radius, and a signed angular span in the local convention.
    ///
    /// `None` for every member whose outline is not a circle — those stay
    /// sampled polylines, which is the right primitive for them: a distance
    /// field is strictly more expensive for a straight line, and a diamond and
    /// a chevron are nothing but straight lines.
    ///
    /// [`outline`](Self::outline) still returns the sampled polyline for the two
    /// that have one, and it is **not** what `build_rings` draws them with. It
    /// is kept because it is the *reference* the arc is checked against —
    /// `renderer/tests.rs` compares the primitive to a densely sampled polyline
    /// of the same arc, and this is where that polyline's shape is defined.
    pub(super) fn arc_shape(self) -> Option<ArcShape> {
        match self {
            Motif::Circle => Some(ArcShape {
                centre: [0.0, 0.0],
                radius: 0.5,
                start: 0.0,
                sweep: TAU,
            }),
            // The same circle `outline` samples: centred so the arc sits on the
            // origin like every other motif, spanning `[-H, H]` about `+x`.
            Motif::Arc => Some(ArcShape {
                centre: [-(ARC_RADIUS * ARC_HALF_ANGLE.cos() + arc_bulge()), 0.0],
                radius: ARC_RADIUS,
                start: -ARC_HALF_ANGLE,
                sweep: 2.0 * ARC_HALF_ANGLE,
            }),
            _ => None,
        }
    }

    /// This motif as a **G1 chain of circular arcs**, when its outline is a
    /// curve that no single arc carries (ADR-0098, Plan 0087 Phase 5).
    ///
    /// `None` for the two circular members, which are exact single arcs already
    /// ([`arc_shape`](Self::arc_shape)), and for the two polygonal ones, whose
    /// outlines are nothing but straight lines and corners — a distance field
    /// is strictly more expensive for a line, and there is no faceting to
    /// remove from a shape whose facets are the figure.
    ///
    /// **Fitted once for the life of the process, not per rebuild.** A chain is
    /// a pure function of its motif: the fit's budget is stated in the motif's
    /// own local frame, so neither a ring's `scale` nor its `phase` nor the
    /// frame can change one. `build_rings` runs on most frames of an animated
    /// mandala, and re-deriving a constant there would put a build-time
    /// algorithm on the hot path.
    pub(super) fn chain(self) -> Option<&'static [Piece]> {
        let index = self.fitted_index()?;
        CHAINS
            .get_or_init(build_chains)
            .get(index)
            .map(Vec::as_slice)
    }

    /// This motif's slot in [`CHAINS`], and the roster's answer to *is this one
    /// fitted?* — kept as one function so the two cannot disagree.
    pub(super) fn fitted_index(self) -> Option<usize> {
        match self {
            Motif::Petal => Some(0),
            Motif::Teardrop => Some(1),
            Motif::Trefoil => Some(2),
            _ => None,
        }
    }

    /// **Segments** one copy of this motif contributes.
    ///
    /// **Zero for the two circular members**: they are drawn as one
    /// [`ArcInstance`] each and contribute no segments at all, which is where
    /// ADR-0098's order of magnitude of tier headroom comes from — sampling a
    /// `circle` costs `SMOOTH_SAMPLES` of this budget, one arc costs one of
    /// [`arcs`](Self::arcs). **A fitted member contributes whatever straight
    /// runs its chain contains**, which for all three of them is none.
    pub fn segments(self) -> usize {
        if self.arc_shape().is_some() || self.is_scallop() {
            return 0;
        }
        if let Some(chain) = self.chain() {
            return chain
                .iter()
                .filter(|piece| matches!(piece, Piece::Line { .. }))
                .count();
        }
        let n = self.vertex_count();
        if self.is_closed() { n } else { n - 1 }
    }

    /// **Arcs** one copy contributes — one for each circular member, its whole
    /// chain for each fitted one, zero for the rest.
    pub fn arcs(self) -> usize {
        // One arc per lobe for a scallop, and its ring's `count` is its lobe
        // count — so `count * instances()` is the whole chain, exactly as it is
        // the whole ring for every other member. The budget arithmetic never
        // learns that this one is a chain.
        if self.arc_shape().is_some() || self.is_scallop() {
            return 1;
        }
        self.chain().map_or(0, |chain| {
            chain
                .iter()
                .filter(|piece| matches!(piece, Piece::Arc { .. }))
                .count()
        })
    }

    /// Instances of **either kind** one copy costs: the number the budget
    /// arithmetic multiplies by `count` against `max_segments`.
    ///
    /// One budget over both kinds rather than two budgets, because the cap is a
    /// statement about how much geometry a tier will draw and an arc is a draw
    /// like any other. It also keeps the overflow message meaning one thing.
    pub fn instances(self) -> usize {
        self.segments() + self.arcs()
    }

    /// Write this motif's outline vertices into `out` (cleared first), in the
    /// local convention: centred on the origin, spanning roughly one unit,
    /// outward along `+x`.
    ///
    /// A pure function of the variant — no clock, no randomness (the determinism
    /// rule), so a mandala is the same figure on every device and in every
    /// capture.
    pub(super) fn outline(self, out: &mut Vec<[f32; 2]>) {
        self.outline_at(self.vertex_count(), out);
    }

    /// [`outline`](Self::outline) at an arbitrary sample count.
    ///
    /// The count exists for the **fit**, not for the draw: a chain is fitted to
    /// samples and can only put a piece boundary on one, so
    /// [`vertex_count`](Self::vertex_count)'s 24 would quantize every boundary
    /// to a 15-degree grid. The three polygonal members ignore it — a diamond
    /// has four vertices at any sample count anyone asks for.
    pub(super) fn outline_at(self, samples: usize, out: &mut Vec<[f32; 2]>) {
        out.clear();
        let smooth = samples.max(3);
        match self {
            Motif::Circle => {
                for k in 0..smooth {
                    let t = TAU * k as f32 / smooth as f32;
                    out.push([0.5 * t.cos(), 0.5 * t.sin()]);
                }
            }
            // A pointed oval: the `1.6` exponent is what makes the two ends cusp
            // instead of round, and it is the whole difference from `Circle`.
            Motif::Petal => {
                for k in 0..smooth {
                    let t = TAU * k as f32 / smooth as f32;
                    let s = t.sin();
                    out.push([0.5 * t.cos(), 0.30 * s.signum() * s.abs().powf(1.6)]);
                }
            }
            // The `(1 + cos t) / 2` taper collapses the width at `t = pi`, i.e.
            // at the *inner* end, so the cusp points at the frame centre.
            Motif::Teardrop => {
                for k in 0..smooth {
                    let t = TAU * k as f32 / smooth as f32;
                    let c = t.cos();
                    out.push([0.5 * c, 0.32 * t.sin() * 0.5 * (1.0 + c)]);
                }
            }
            Motif::Diamond => {
                out.push([0.5, 0.0]);
                out.push([0.0, 0.3]);
                out.push([-0.5, 0.0]);
                out.push([0.0, -0.3]);
            }
            // Chord along `y`, bulge along `+x`, then shifted so the arc is
            // centred on the origin like every other motif — otherwise `radius`
            // would mean the chord for this one member and the centre for the
            // rest.
            Motif::Arc => {
                let bulge = arc_bulge();
                for k in 0..=ARC_SAMPLES {
                    let psi = ARC_HALF_ANGLE * (2.0 * k as f32 / ARC_SAMPLES as f32 - 1.0);
                    out.push([
                        ARC_RADIUS * (psi.cos() - ARC_HALF_ANGLE.cos()) - bulge,
                        ARC_RADIUS * psi.sin(),
                    ]);
                }
            }
            // `|cos(1.5 t)|` has three lobes over a full turn, and the sample
            // count is a multiple of six so every cusp lands exactly on a vertex
            // rather than being rounded off by the sampling.
            Motif::Trefoil => {
                // A multiple of six so every cusp lands exactly on a sample —
                // the fit reads a cusp as a corner and breaks its chain there,
                // and a cusp rounded off by the sampling would be smoothed
                // into the figure instead.
                for k in 0..(smooth / 6).max(1) * 6 {
                    let n = (smooth / 6).max(1) * 6;
                    let t = TAU * k as f32 / n as f32;
                    let r = 0.5 * (1.5 * t).cos().abs();
                    out.push([r * t.cos(), r * t.sin()]);
                }
            }
            Motif::Chevron => {
                out.push([-0.25, 0.42]);
                out.push([0.5, 0.0]);
                out.push([-0.25, -0.42]);
            }
            // The base circle the boundary's lobes bulge from. A scallop has no
            // outline of its own in this frame: its shape needs a lobe count
            // and a depth, and both live on the ring rather than on the motif.
            // `build_rings` builds the chain directly and never asks for this.
            Motif::Scallop => {
                for k in 0..smooth {
                    let t = TAU * k as f32 / smooth as f32;
                    out.push([0.5 * t.cos(), 0.5 * t.sin()]);
                }
            }
        }
    }
}

/// Vertices in the three smooth closed motifs. Twenty-four is the number
/// ADR-0079's budget arithmetic quotes, and it is smooth enough that a bead at a
/// shipped `scale` shows no facets.
pub(super) const SMOOTH_SAMPLES: usize = 24;
/// Vertices in [`Motif::Trefoil`] — a multiple of six, so the three lobe cusps
/// fall on samples.
pub(super) const TREFOIL_SAMPLES: usize = 36;
/// Segments in one [`Motif::Arc`].
pub(super) const ARC_SAMPLES: usize = 12;
/// Half the angle [`Motif::Arc`] subtends at its own centre of curvature. Sixty
/// degrees gives a chord of `2 * 0.5 * sin(60 deg) = 0.866` against a `0.25`
/// bulge — a shallow scallop rather than a hook.
pub(super) const ARC_HALF_ANGLE: f32 = std::f32::consts::FRAC_PI_3;
/// [`Motif::Arc`]'s radius of curvature.
pub(super) const ARC_RADIUS: f32 = 0.5;

/// How far [`Motif::Arc`] is shifted along `-x` so it sits centred on the origin
/// like every other motif — otherwise `radius` would mean the chord for that one
/// member and the centre for the rest.
///
/// A function rather than a `const` because `cos` is not a const fn; it is
/// called twice at build time and never per frame.
pub(super) fn arc_bulge() -> f32 {
    0.5 * ARC_RADIUS * (1.0 - ARC_HALF_ANGLE.cos())
}

/// One motif expressed as a circular arc — see [`Motif::arc_shape`]. In the
/// local convention: about the motif's own centre, spanning roughly one unit,
/// outward along `+x`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct ArcShape {
    pub(super) centre: [f32; 2],
    pub(super) radius: f32,
    pub(super) start: f32,
    pub(super) sweep: f32,
}

/// The fewest lobes a [`Motif::Scallop`] boundary is built with.
///
/// Below three there is no boundary to speak of: at one lobe the two ends of
/// the chain's only arc coincide and its sweep degenerates to zero, and at two
/// the "scallop" is a lens. `build_rings` raises a smaller `count` to this
/// rather than declining it, and charges the raised count against the cap, so
/// the budget arithmetic and the geometry agree.
pub const MIN_SCALLOP_LOBES: u32 = 3;

/// One lobe of a scalloped boundary, in the ring's own frame: the arc that
/// leaves the base circle at `-half_span`, bulges out to `depth` past it on the
/// axis, and returns to the base circle at `+half_span`.
///
/// **Constructed exactly rather than fitted.** A scallop *is* a chain of
/// circular arcs — that is what makes it a scallop and not a sine wave — so
/// there is nothing here for [`biarc`] to approximate. The circle through the
/// two ends and the apex has its centre on the axis by symmetry, and equating
/// its distance to an end and to the apex gives the centre in one line:
///
/// ```text
///   c = ((R + d)^2 - R^2) / (2 * ((R + d) - R * cos(half_span)))
/// ```
///
/// At `d = 0` that is `c = 0` and the lobe is an arc of the base circle itself,
/// so a zero depth draws the plain ring rather than anything degenerate — the
/// property that makes `ring_scale` a continuous lever on this member as it is
/// on every other.
pub(super) fn scallop_lobe(base: f32, depth: f32, half_span: f32) -> ArcShape {
    let apex = base + depth;
    let denom = 2.0 * (apex - base * half_span.cos());
    // `apex - base * cos` is positive for every depth a preset can reach, so
    // this only guards the exactly-flat case. What makes that true is the
    // load-time refusal of a negative structural ring `scale` on a scallop
    // (`preset::schema`) — **not** the `ring_scale` clamp, which is the bindable
    // per-frame multiplier and a different quantity.
    let centre = if denom.abs() > f32::EPSILON {
        (apex * apex - base * base) / denom
    } else {
        0.0
    };
    let radius = (apex - centre).abs();
    let (sin, cos) = half_span.sin_cos();
    let start = (-base * sin).atan2(base * cos - centre);
    let end = (base * sin).atan2(base * cos - centre);
    ArcShape {
        centre: [centre, 0.0],
        radius,
        start,
        // The lobe runs the short way from one end to the other, which is
        // outward past the apex: the two ends straddle the axis and the sweep
        // between them is under half a turn for **every depth this can be
        // called with**, which is what the load-time refusal above buys. At a
        // negative depth past `-R * (cos(s) + sin(s) - 1)` the ends cross over
        // and this sweep runs the long way instead.
        sweep: (end - start).rem_euclid(TAU),
    }
}

/// The lateral budget [`build_chains`] fits the roster's curved motifs to,
/// in the **motif's own local frame**.
///
/// **One pixel at 1080p, at the largest scale the roster is drawn at.** A motif
/// is authored spanning roughly one unit and placed at a ring `scale`; the three
/// retired mandalas it was measured against use `0.13` to `0.46`, so a copy at
/// the top of that range covers `0.46` of the renderer's world y — 248 px at
/// 1080p — and one of those pixels is `1 / 248 = 4.0e-3` of the local frame.
/// Everything smaller is drawn better than the budget promises; a preset
/// reaching past `0.46` (the ceiling is [`MAX_RING_SCALE`]) trades this off
/// linearly and is still bounded by a chain that is G1 whatever its scale.
pub(super) const MOTIF_FIT_BUDGET: f32 = 4.0e-3;

/// Every fitted motif's chain, in [`Motif::fitted_index`] order — see
/// [`Motif::chain`] for why it is built once.
static CHAINS: OnceLock<[Vec<Piece>; 3]> = OnceLock::new();

/// Fit the three curved motifs. Runs at most once per process, on the first
/// `[generator] rings` roster that names one.
pub(super) fn build_chains() -> [Vec<Piece>; 3] {
    let mut points = Vec::with_capacity(biarc::FIT_SAMPLES);
    let mut walk = Vec::new();
    [Motif::Petal, Motif::Teardrop, Motif::Trefoil].map(|motif| {
        motif.outline_at(biarc::FIT_SAMPLES, &mut points);
        let mut chain = Vec::new();
        // `walk` is the colour axis a fitted chain would be read along; the
        // roster has none — `normalized_radii` colours a placed motif by where
        // its copy sits on its ring, not by where a piece sits on its outline.
        biarc::fit(
            &points,
            motif.is_closed(),
            MOTIF_FIT_BUDGET,
            &mut chain,
            &mut walk,
        );
        chain
    })
}
