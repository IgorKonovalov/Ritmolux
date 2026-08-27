//! **The seeded layout grammar** — where a canvas's composition comes from
//! (Plan 0113 Phase 4).
//!
//! A pure function of `(grammar, seed, recomposition index, count, size
//! hierarchy, angle bias)` onto an element list, in painter order.
//!
//! # Four options, and one of them is a control
//!
//! Plan 0113 Phase 5 is a human gate that picks the composition from rendered
//! samples, so this module's job is to make candidates **comparable**, not to
//! make one of them good. Three grammars are offered:
//!
//! - [`Grammar::AnchorSatellites`] — one or two dominant elements carry the
//!   canvas and the rest cluster around them, so the picture has a subject.
//! - [`Grammar::DiagonalAxis`] — a dominant angle, with elements distributed
//!   along it and scattered narrowly across it, so the picture has a direction.
//! - [`Grammar::SizeHierarchy`] — sizes drawn from a power law (a few large,
//!   many small) with **position independent of size**, so the picture has a
//!   range but no centre.
//!
//! The fourth, [`Grammar::Authored`], is not a grammar at all: it is the
//! fourteen-element canvas Phase 1 shipped, kept as the **control** the three
//! are judged against. It is the default, which is what keeps the golden
//! baseline and the shipped preset from moving underneath Phase 5's decision —
//! and it is the one composition a human has already approved, so throwing it
//! away before that gate would have discarded the only fixed point the
//! comparison has.
//!
//! # Determinism, and what it is a function of
//!
//! No wall clock and no unseeded randomness (the cross-cutting rule): every
//! draw comes off [`SeededRng`](super::super::SeededRng), splitmix64, seeded
//! from the recipe alone. **The aspect is deliberately not an input.** A canvas
//! is generated in its own fixed domain and the frame shows it; making layout
//! depend on the render target would mean the same preset composed differently
//! on two monitors, and would make "the same seed gives the same list" a claim
//! about a window size. It also means this module never touches ADR-0037's
//! hazard: nothing here computes screen-destined geometry.
//!
//! # It allocates nothing
//!
//! [`generate`] takes `&mut Vec<Element>` sized to the tier cap at scene
//! construction and refills it with `clear` + `push`, so a recomposition — which
//! runs on the render thread, off the audio callback but inside the frame — never
//! reaches the allocator.

// Hot-path panic-denial pragma, as everywhere under `scenes/`. This module runs
// at recomposition rather than every frame, but it runs on the render thread and
// inside the frame — a panic here is a crash mid-show exactly as it would be one
// pass further down.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use super::super::SeededRng;
use super::{
    KIND_ARC, KIND_BAR, KIND_CHECKER, KIND_CIRCLE, KIND_QUAD, KIND_RING, KIND_SEGMENT,
    KIND_TRIANGLE, SUPREMATIST, Spec,
};

/// The canvas's own domain, half-extents. Roughly 3:2 — between a 16:10 frame
/// and a square one, so a wide window shows a small margin and a tall one crops
/// a little rather than either being badly served.
///
/// Not the render target's aspect, and that is the point: see the module docs.
pub(crate) const CANVAS_X: f32 = 1.5;
pub(crate) const CANVAS_Y: f32 = 1.0;

/// How many evenly-spaced palette coordinates the generator draws element
/// colours from, **including the one it reserves for the paper**.
///
/// Eight, and drawn at band *centres* (`k/8 + 1/16`), because the palette this
/// look wants is a set of flat plateaus rather than a gradient — `presets/README.md`
/// says so under `shape_collage`. On such a palette every generated element lands
/// exactly inside a band and takes that colour exactly; on a smooth gradient they
/// are simply eight evenly spaced samples, which is a sensible thing to be. The
/// **last** slot is left to the paper by convention, so an element never draws
/// the ground's own colour and vanishes.
pub(crate) const PALETTE_SLOTS: u32 = 8;

/// Which composition strategy builds the element list. Quantized from the
/// `layout` param CPU-side, like every other selector in this engine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Grammar {
    /// The hand-authored fourteen-element canvas — the control, and the default.
    Authored,
    /// One or two dominant elements with the rest clustered around them.
    AnchorSatellites,
    /// A dominant angle, elements along it and narrowly across it.
    DiagonalAxis,
    /// Power-law sizes, positions independent of size.
    SizeHierarchy,
}

impl Grammar {
    /// The grammar a bound `layout` selects.
    ///
    /// **This mapping is the content lane's contract** — `presets/README.md`
    /// carries the names against these numbers, and it is the only place they
    /// are written down, deliberately: a second roster in here would be one to
    /// keep in step.
    ///
    /// **Quantized here rather than in the shader** — an eased binding sweeps
    /// continuously through values a selector needs to be integral, which is the
    /// hazard the kaleidoscope seam was fixed for. A non-finite or
    /// out-of-range value falls back to the control rather than to a grammar
    /// nobody asked for.
    pub(crate) fn from_param(layout: f32) -> Grammar {
        if !layout.is_finite() {
            return Grammar::Authored;
        }
        match layout.round() as i64 {
            1 => Grammar::AnchorSatellites,
            2 => Grammar::DiagonalAxis,
            3 => Grammar::SizeHierarchy,
            _ => Grammar::Authored,
        }
    }
}

/// One generated element, with the per-element motion the music drives
/// (Plan 0113 Phase 6).
///
/// **The motion is drawn at generation, from the same seeded stream as the
/// geometry**, so a canvas's drift and spin are as reproducible as its layout —
/// two renders of one seed move identically. Nothing here is a position: these
/// are the *rates*, and the scene integrates them against the injected real
/// `dt`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Placed {
    /// The element as composed, before any time has passed.
    pub(crate) spec: Spec,
    /// Canvas units per second at `drift = 1`.
    pub(crate) vel: [f32; 2],
    /// Radians per second at `spin = 1`.
    pub(crate) spin: f32,
    /// This element's pump phase, `0..1`. **Offset per element** so the canvas
    /// does not breathe in unison — a field of oscillators sharing one phase
    /// pulses as one sheet, which is the swarm's `twinkle` finding applied here.
    pub(crate) phase: f32,
    /// How live this element is, `0..1`. **Not generated** — the scene eases it
    /// toward the `density` gate every frame, and snaps it on a fresh canvas.
    pub(crate) fade: f32,
}

/// Everything a canvas is a function of. **`generate` reads nothing else**, so
/// two equal recipes produce bit-identical element lists — the determinism the
/// plan asks be asserted directly.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Recipe {
    /// Which strategy composes the canvas.
    pub(crate) grammar: Grammar,
    /// How many elements, already held inside the tier cap.
    pub(crate) count: usize,
    /// The preset's seed.
    pub(crate) seed: u64,
    /// Which recomposition this is. Phase 6 advances it on a rising edge; until
    /// then it is always `0`.
    pub(crate) recompose: u64,
    /// How steeply sizes fall from largest to smallest, `0..=1`. `0` is a nearly
    /// uniform canvas; `1` is a few dominant forms over many small ones.
    pub(crate) size_hierarchy: f32,
    /// The canvas's dominant angle, in **radians**. Conditioned CPU-side from
    /// the `angle_bias` param.
    pub(crate) angle_bias: f32,
    /// Which kinds the canvas draws from (Plan 0113 Phase 7).
    pub(crate) roster: Roster,
}

/// The generator's own seeding, so `seed` and `recompose` cannot collide into
/// the same stream by walking past each other (`seed = 3, recompose = 0` and
/// `seed = 2, recompose = 1` must be different canvases, not the same one).
fn stream(recipe: &Recipe) -> SeededRng {
    SeededRng::new(
        recipe
            .seed
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
            .wrapping_add(recipe.recompose.wrapping_mul(0xD1B5_4A32_D192_ED03)),
    )
}

/// The drift speed an element is given at `drift = 1`, in canvas units a
/// second, before its own `0.3..1` share of it.
///
/// A canvas is 2 units tall, so at the top of the range an element crosses it in
/// about a minute — the pace of a composition that is *alive* rather than one
/// that is travelling. `drift` above 1 is legal and is how a preset asks for
/// more.
const DRIFT_SPEED: f32 = 0.035;
/// The angular speed at `spin = 1`, radians a second, before the element's own
/// signed `-1..1` share. A full turn in about a minute and a half at the top of
/// the range, for the same reason.
const SPIN_SPEED: f32 = 0.07;

/// Draw one element's motion and wrap it into a [`Placed`].
///
/// Called by every grammar at generation, off the same stream as the geometry,
/// so `drift` and `spin` are reproducible per seed rather than per run.
fn motion(rng: &mut SeededRng, spec: Spec) -> Placed {
    let theta = rng.range(0.0, std::f32::consts::TAU);
    let speed = DRIFT_SPEED * rng.range(0.3, 1.0);
    Placed {
        spec,
        vel: [theta.cos() * speed, theta.sin() * speed],
        spin: SPIN_SPEED * rng.range(-1.0, 1.0),
        phase: rng.next_f32(),
        // Snapped by the scene against the density gate before the first frame;
        // a fresh canvas does not fade itself in, because the recomposition
        // blend is what covers a canvas *changing*.
        fade: 1.0,
    }
}

/// One element's palette coordinate, drawn from the plateau centres. Never the
/// last slot — that one belongs to the paper (see [`PALETTE_SLOTS`]).
fn draw_coord(rng: &mut SeededRng) -> f32 {
    let slot = (rng.next_f32() * (PALETTE_SLOTS - 1) as f32) as u32;
    (slot.min(PALETTE_SLOTS - 2) as f32 + 0.5) / PALETTE_SLOTS as f32
}

/// One element's kind.
///
/// Weighted toward the quad — about two in three — because a suprematist canvas
/// is mostly bars and planes, and a roster drawn uniformly reads as a shape
/// sampler rather than as a painting.
fn draw_kind(rng: &mut SeededRng, roster: Roster) -> f32 {
    let r = rng.next_f32();
    match roster {
        Roster::Suprematist => {
            if r < 0.66 {
                KIND_QUAD
            } else if r < 0.85 {
                KIND_CIRCLE
            } else {
                KIND_TRIANGLE
            }
        }
        // **Still quad-weighted, and deliberately.** *On White II* adds lines,
        // arcs, rings and checker patches to a canvas that is still mostly
        // planes; a roster drawn flat across eight kinds reads as a shape
        // sampler rather than as a painting, which is the same finding the
        // three-kind weighting above came from.
        Roster::Kandinsky => {
            if r < 0.34 {
                KIND_QUAD
            } else if r < 0.50 {
                KIND_BAR
            } else if r < 0.62 {
                KIND_CIRCLE
            } else if r < 0.72 {
                KIND_TRIANGLE
            } else if r < 0.83 {
                KIND_RING
            } else if r < 0.90 {
                KIND_ARC
            } else if r < 0.96 {
                KIND_SEGMENT
            } else {
                KIND_CHECKER
            }
        }
    }
}

/// Which kinds a canvas draws from — the `roster` param, quantized CPU-side.
///
/// Two, not a bitmask: a mask would be the more flexible surface and it is the
/// wrong one here, because an **eased** binding sweeping through a bitmask is
/// meaningless in a way an eased selector is merely blunt. A preset that wants
/// one specific kind authors it; a preset that wants a *world* picks which of
/// the two the references describe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Roster {
    /// `quad`, `circle`, `triangle` — Malevich's vocabulary, and the default, so
    /// a preset that says nothing draws the canvas Phase 5 settled on.
    Suprematist,
    /// All eight — Kandinsky's *On White II* adds lines, rings, arcs, sectors
    /// and checker patches on top.
    Kandinsky,
}

impl Roster {
    /// The roster a bound `roster` selects. Anything off the list falls back to
    /// the suprematist three rather than to a vocabulary nobody asked for.
    pub(crate) fn from_param(roster: f32) -> Roster {
        if roster.is_finite() && roster.round() as i64 == 1 {
            Roster::Kandinsky
        } else {
            Roster::Suprematist
        }
    }
}

/// The two kind-specific parameters, drawn per element off the same stream.
///
/// `p0` is the sector half-aperture and `p1` the checker's cells per axis, and
/// **both are drawn for every element whatever its kind** — deliberately. Drawing
/// them conditionally would make the RNG stream's shape depend on which kind came
/// out of it, so a roster change would re-roll every element after the first one
/// that differed. Two draws an element is nothing; a layout that shifts when an
/// unrelated param moves is a real defect.
fn draw_kind_params(rng: &mut SeededRng, roster: Roster) -> (f32, f32, f32) {
    // A quarter turn to most of a full one: narrower reads as a splinter, and
    // past this a sector is a disc with a notch.
    let aperture = rng.range(0.45, 2.6);
    // 2 to 8, forced even downstream by `checker_cells`.
    let cells = (rng.range(2.0, 8.99)).floor();
    // **Translucent crossings, and only on the Kandinsky roster.** *On White II*
    // overlaps its forms and lets both show; a suprematist canvas does not, and
    // an opaque plane is what that half of the reference set is made of. About
    // one element in four, so a crossing is an event rather than the condition
    // of the whole canvas.
    let roll = rng.next_f32();
    let alpha = if roster == Roster::Kandinsky && roll < 0.26 {
        // Not lower: below about 0.4 a crossing stops reading as two elements
        // and starts reading as a third, paler one.
        0.45 + roll
    } else {
        1.0
    };
    (aperture, cells, alpha)
}

/// A size drawn from a power law whose steepness is `hierarchy`, as a `0..1`
/// fraction of the largest an element may be.
///
/// `rank` is the element's position in the list as a `0..1` fraction, so the
/// distribution is applied *by construction* rather than sampled and sorted —
/// which is what makes "a few large, many small" true for any count rather than
/// true on average. At `hierarchy = 0` the exponent is 1 and every element is
/// the same size; at 1 it is 4, and the last element is a sixteenth of the
/// first at the same `rank` spread.
fn power_size(rank: f32, hierarchy: f32) -> f32 {
    let exponent = 1.0 + 3.0 * hierarchy.clamp(0.0, 1.0);
    (1.0 - rank).powf(exponent)
}

/// **A big element is a bar; only a small one is blocky.** The half extents for
/// an element of the given kind at `unit` of its kind's size range.
///
/// This is the single rule that decides whether a generated canvas reads as
/// suprematist or as a pile of slabs, and it is taken from the authored canvas
/// rather than invented: its dominant forms are `[0.72, 0.075]` and
/// `[0.62, 0.115]` — long bars — while everything square is small
/// (`[0.16, 0.16]`, `[0.09, 0.09]`). So the elongation *ceiling falls as the
/// element grows*, and a large near-square element is unreachable by
/// construction rather than merely unlikely.
///
/// The two compact kinds are excluded from that rule and given their own,
/// narrower size range: a disc drawn at a bar's elongation is a sliver, and the
/// references have no such thing.
fn draw_extents(rng: &mut SeededRng, kind: f32, unit: f32) -> [f32; 2] {
    if kind == KIND_CIRCLE {
        let size = 0.04 + 0.14 * unit;
        return [size, size * rng.range(0.75, 1.0)];
    }
    if kind == KIND_TRIANGLE {
        let size = 0.06 + 0.14 * unit;
        return [size, size * rng.range(0.8, 1.25)];
    }
    // The circle family reads its half extents as **radius, then thickness**
    // (`sdf.rs`'s table), so its "elongation" is a stroke weight rather than a
    // shape. A ring at a bar's proportions would be a hairline circle, which is
    // the one thing *On White II*'s rings are not.
    if kind == KIND_RING || kind == KIND_ARC {
        let radius = 0.06 + 0.30 * unit;
        return [radius, radius * rng.range(0.08, 0.30)];
    }
    if kind == KIND_SEGMENT {
        // A sector's `hy` is unused as a shape; keeping it equal to the radius
        // means nothing downstream reads a stale thickness off it.
        let radius = 0.05 + 0.26 * unit;
        return [radius, radius];
    }
    if kind == KIND_CHECKER {
        // Blockier than a quad and smaller: a checker patch is a *texture*, and
        // one drawn at a dominant plane's size stops reading as an element and
        // starts reading as a background.
        let size = 0.07 + 0.22 * unit;
        return [size, size * rng.range(0.45, 1.0)];
    }
    let size = 0.055 + 0.665 * unit;
    // `0.13 / size` is "the short axis stays about a small element's width
    // however long the element gets", floored so a small quad can still be a
    // square and capped so nothing is thinner than a hairline.
    let hi = (0.13 / size.max(1e-3)).clamp(0.12, 1.0);
    [size, size * rng.range(0.10, hi)]
}

/// How much of an element is allowed to hang off the canvas, as a fraction of
/// its own reach.
///
/// **Not zero, and not one.** A form crossing the edge is a suprematist device —
/// the authored canvas has three — and forbidding it would push every large bar
/// toward the middle and produce a wreath. Letting the *centre* go to the edge,
/// which the first draft did, produces the opposite failure: a 0.7-unit bar
/// centred on the boundary is a crop of a painting rather than a painting.
const OVERHANG: f32 = 0.45;

/// Hold a centre so the element mostly stays on the canvas — see [`OVERHANG`].
///
/// Uses the element's larger half extent as its reach rather than its rotated
/// bounding box, which is conservative on the short axis and costs nothing: this
/// runs once per element at generation, not per pixel.
fn place(center: [f32; 2], half: [f32; 2]) -> [f32; 2] {
    let reach = half[0].max(half[1]) * (1.0 - OVERHANG);
    let lx = (CANVAS_X - reach).max(0.0);
    let ly = (CANVAS_Y - reach).max(0.0);
    [center[0].clamp(-lx, lx), center[1].clamp(-ly, ly)]
}

/// Build the canvas `recipe` describes into `out`, in painter order.
///
/// **Allocation-free**: `out` keeps its capacity, and `count` is held inside it
/// by the caller. Pure — see [`Recipe`].
pub(crate) fn generate(out: &mut Vec<Placed>, recipe: &Recipe) {
    let count = recipe.count.min(out.capacity());
    out.clear();
    if count == 0 {
        return;
    }
    let mut control = stream(recipe);
    if recipe.grammar == Grammar::Authored {
        // The control. Cycles the authored roster if asked for more than it
        // holds, which keeps the element count honest without inventing a
        // fifteenth authored element.
        for i in 0..count {
            if let Some(&spec) = SUPREMATIST.get(i % SUPREMATIST.len()) {
                // The control is a fixed composition, so its motion is drawn
                // from a stream of its own rather than left undrawn: a static
                // canvas that cannot drift would make `drift` silently inert on
                // the one layout a preset gets by default.
                out.push(motion(&mut control, spec));
            }
        }
        return;
    }

    let mut rng = stream(recipe);
    match recipe.grammar {
        Grammar::AnchorSatellites => anchor_satellites(out, recipe, &mut rng, count),
        Grammar::DiagonalAxis => diagonal_axis(out, recipe, &mut rng, count),
        Grammar::SizeHierarchy => size_hierarchy(out, recipe, &mut rng, count),
        Grammar::Authored => {}
    }
}

/// One or two dominant elements, the rest clustered around them.
///
/// The anchors go in **first**, so everything else paints over them — which is
/// what makes them read as ground rather than as foreground clutter, and is a
/// property of the painter's order rather than of their size.
fn anchor_satellites(out: &mut Vec<Placed>, recipe: &Recipe, rng: &mut SeededRng, count: usize) {
    let anchors = if rng.next_f32() < 0.45 { 2 } else { 1 };
    let anchors = anchors.min(count);
    let mut centres = [[0.0f32; 2]; 2];

    for slot in 0..anchors {
        // Anchors sit off-centre but not at the edge: a dominant form centred
        // exactly is a target, not a composition.
        let cx = rng.range(-0.5, 0.5) * CANVAS_X;
        let cy = rng.range(-0.45, 0.45) * CANVAS_Y;
        if let Some(c) = centres.get_mut(slot) {
            *c = [cx, cy];
        }
        // An anchor is always a quad: it is the canvas's dominant *plane*, and
        // the compact kinds are capped small by `draw_extents` anyway.
        let unit = rng.range(0.78, 1.0);
        let half = draw_extents(rng, KIND_QUAD, unit);
        let (aperture, cells, alpha) = draw_kind_params(rng, recipe.roster);
        let spec = Spec {
            kind: KIND_QUAD,
            center: place([cx, cy], half),
            half,
            angle_deg: (recipe.angle_bias + rng.range(-0.15, 0.15)).to_degrees(),
            coord: draw_coord(rng),
            alpha,
            p0: aperture,
            p1: cells,
        };
        out.push(motion(rng, spec));
    }

    for i in anchors..count {
        let rank = (i - anchors) as f32 / (count - anchors).max(1) as f32;
        let anchor = centres
            .get(if anchors > 1 && rng.next_f32() < 0.5 {
                1
            } else {
                0
            })
            .copied()
            .unwrap_or([0.0, 0.0]);
        // Clustered around the anchor rather than on it: a uniform radius over
        // a band that starts clear of the anchor's own centre. (A uniform radius
        // in two dimensions already concentrates the population inward, which is
        // the clustering; drawing it from a product of uniforms as the first
        // draft did piled every satellite on top of its anchor.)
        let radius = rng.range(0.18, 1.05);
        let theta = rng.range(0.0, std::f32::consts::TAU);
        let kind = draw_kind(rng, recipe.roster);
        // Satellites are the small half of the range whatever the hierarchy
        // says — an anchor grammar in which a satellite can match its anchor has
        // no anchor.
        let half = draw_extents(rng, kind, 0.42 * power_size(rank, recipe.size_hierarchy));
        let (aperture, cells, alpha) = draw_kind_params(rng, recipe.roster);
        let spec = Spec {
            kind,
            center: place(
                [
                    anchor[0] + theta.cos() * radius * CANVAS_X,
                    anchor[1] + theta.sin() * radius * CANVAS_Y,
                ],
                half,
            ),
            half,
            angle_deg: (recipe.angle_bias + rng.range(-0.6, 0.6)).to_degrees(),
            coord: draw_coord(rng),
            alpha,
            p0: aperture,
            p1: cells,
        };
        out.push(motion(rng, spec));
    }
}

/// Half the chord through the canvas centre along direction `(cos_a, sin_a)`.
///
/// **This is what makes the band run at the angle it was asked for.** The
/// canvas is wider than it is tall, so rotating a unit vector and *then*
/// scaling its components by `CANVAS_X` and `CANVAS_Y` shears the direction: a
/// `-22 deg` axis came out at about `-15 deg` on screen while every element sat
/// correctly at `-22 deg`, so the elements and the band they were distributed
/// along were at different angles. Found at Plan 0113's Phase 5 gate, in the
/// samples rather than in a test — a rendered canvas is the only thing that
/// shows it.
///
/// The direction here is already in square canvas units, so it is used as-is and
/// only its *reach* is a function of the canvas shape.
fn reach(cos_a: f32, sin_a: f32) -> f32 {
    let x = if cos_a.abs() > 1e-4 {
        CANVAS_X / cos_a.abs()
    } else {
        f32::INFINITY
    };
    let y = if sin_a.abs() > 1e-4 {
        CANVAS_Y / sin_a.abs()
    } else {
        f32::INFINITY
    };
    x.min(y)
}

/// A dominant angle, with elements distributed along it and spread across it.
///
/// **The winner of Plan 0113's Phase 5 gate, with the runner-up's spread folded
/// in.** The verdict was that this grammar's organising axis is the suprematist
/// diagonal the reference canvases are actually built on — which the other two
/// have to invent — but that `size-hierarchy` used the *frame* better, because
/// this one's population hugged the axis so tightly that a canvas read as one
/// horizontal band with the top and bottom empty. So the across-axis spread and
/// the angle jitter below are `size-hierarchy`'s, and the placement is this
/// grammar's. Both numbers are marked; they are the combination, not tuning.
fn diagonal_axis(out: &mut Vec<Placed>, recipe: &Recipe, rng: &mut SeededRng, count: usize) {
    let (sin_a, cos_a) = recipe.angle_bias.sin_cos();
    // The two axes of the band, each reaching the canvas edge in its own
    // direction — see `reach` for the shear this replaced.
    let along_reach = reach(cos_a, sin_a);
    let across_reach = reach(-sin_a, cos_a);
    for i in 0..count {
        let rank = i as f32 / count.max(1) as f32;
        let along = rng.range(-1.0, 1.0) * along_reach;
        // **The Phase 5 combination.** Still biased toward the axis — that bias
        // is what makes the direction legible — but `sqrt` where the first draft
        // squared, and 0.8 of the reach where it took 0.45. The squared version
        // pinned everything to a stripe.
        let across = rng.range(-1.0, 1.0);
        let across = across * across.abs().sqrt() * 0.8 * across_reach;
        let x = along * cos_a - across * sin_a;
        let y = along * sin_a + across * cos_a;
        let kind = draw_kind(rng, recipe.roster);
        let half = draw_extents(rng, kind, power_size(rank, recipe.size_hierarchy));
        // Most elements lie ALONG the axis; a minority cross it, which is what
        // keeps a diagonal composition from reading as a single striped texture.
        let crossing = rng.next_f32() < 0.22;
        let jitter = if crossing {
            std::f32::consts::FRAC_PI_2 + rng.range(-0.3, 0.3)
        } else {
            // The other half of the combination: +-26 deg rather than +-10, so
            // the forms relate to the axis instead of being welded to it.
            rng.range(-0.45, 0.45)
        };
        let (aperture, cells, alpha) = draw_kind_params(rng, recipe.roster);
        let spec = Spec {
            kind,
            center: place([x, y], half),
            half,
            angle_deg: (recipe.angle_bias + jitter).to_degrees(),
            coord: draw_coord(rng),
            alpha,
            p0: aperture,
            p1: cells,
        };
        out.push(motion(rng, spec));
    }
}

/// Power-law sizes with **position independent of size** — the grammar that
/// deliberately has no centre, so Phase 5 can see whether a canvas needs one.
fn size_hierarchy(out: &mut Vec<Placed>, recipe: &Recipe, rng: &mut SeededRng, count: usize) {
    for i in 0..count {
        let rank = i as f32 / count.max(1) as f32;
        let kind = draw_kind(rng, recipe.roster);
        let half = draw_extents(rng, kind, power_size(rank, recipe.size_hierarchy));
        let (aperture, cells, alpha) = draw_kind_params(rng, recipe.roster);
        let spec = Spec {
            kind,
            // Uniform over the whole canvas, and drawn from the same stream
            // whatever the size is — that independence IS this grammar.
            center: place(
                [
                    rng.range(-0.92, 0.92) * CANVAS_X,
                    rng.range(-0.92, 0.92) * CANVAS_Y,
                ],
                half,
            ),
            half,
            angle_deg: (recipe.angle_bias + rng.range(-1.2, 1.2)).to_degrees(),
            coord: draw_coord(rng),
            alpha,
            p0: aperture,
            p1: cells,
        };
        out.push(motion(rng, spec));
    }
}
