//! `shape_collage`'s own contract (Plan 0113 Phase 1).
//!
//! Three of these are rendered rather than arithmetic, because the three claims
//! that matter are claims about what reaches the frame:
//!
//! - **Occlusion is order.** Two overlapping elements, drawn in both array
//!   orders, must give the later one's colour in the overlap each time — and the
//!   two frames must differ there. Order is the whole mechanism (ADR-0123), so
//!   this is the assertion that proves the scene works at all.
//! - **Flat colour is exact.** An element whose brightest channel is at or under
//!   the tonemap's [`KNEE`](crate::render::tonemap::KNEE) reaches the capture at
//!   the value it was authored at. That is a *property* of ADR-0046's curve
//!   below its knee — the identity — so the tolerance is the display write's own
//!   quantization and nothing more.
//! - **The aspect comes from the render target** (ADR-0037). Measured at
//!   **1280x800**: 1920x1080 and this box's 2048x1152 are both exactly 16:9,
//!   where no test can tell a target-derived aspect from a grid-derived one.
//!
//! The bounding-box check is arithmetic, and it is here rather than in an
//! integration test because a loose box costs frames and shows nothing.

#![allow(clippy::indexing_slicing, clippy::panic, clippy::expect_used)]

use super::layout::{self, Grammar, Recipe};
use super::{
    AUTHORED_COUNT, DEFAULT_ANGLE_BIAS, DEFAULT_EDGE_SOFTNESS, DEFAULT_SCALE, DEFAULT_SEED,
    Element, KIND_CIRCLE, KIND_QUAD, KIND_TRIANGLE, MAX_EDGE_SOFTNESS, MAX_SCALE, MAX_SEED,
    MIN_SCALE, PARAMS, SUPREMATIST, ShapeCollageScene, Spec, applied_angle_bias, applied_count,
    applied_edge_softness, applied_scale, applied_seed, triangle_vertices,
};
use crate::dsp::AnalysisFrame;
use crate::preset::Preset;
use crate::render::context::{RenderContext, RenderError as CtxError};
use crate::render::scenes::Scene;
use crate::render::tonemap::KNEE;
use crate::render::{CaptureImage, HeadlessOptions, RenderError, Renderer, capture};

/// The tier cap these tests build against — the floor's, since a headless
/// capture is [`Tier::Floor`](crate::render::Tier::Floor) by construction.
const CAP: usize = crate::render::TierConfig::FLOOR.collage_elements;

// ---------------------------------------------------------------------------
// Arithmetic
// ---------------------------------------------------------------------------

/// `scale` is held inside the range the canvas transform needs, and a broken
/// binding lands on the default rather than on a bound.
#[test]
fn the_scale_is_clamped_and_falls_back() {
    assert_eq!(applied_scale(DEFAULT_SCALE), DEFAULT_SCALE);
    assert_eq!(applied_scale(0.0), MIN_SCALE);
    assert_eq!(applied_scale(-4.0), MIN_SCALE);
    assert_eq!(applied_scale(1e9), MAX_SCALE);
    assert_eq!(applied_scale(f32::NAN), DEFAULT_SCALE);
    assert_eq!(applied_scale(f32::INFINITY), DEFAULT_SCALE);
}

/// `edge_softness` is held at or above the hard edge, and its default is
/// **exactly** zero on the way to the uniform — the one-pixel ramp is the look.
#[test]
fn the_edge_softness_is_clamped_and_falls_back() {
    assert_eq!(applied_edge_softness(DEFAULT_EDGE_SOFTNESS), 0.0);
    assert_eq!(applied_edge_softness(-1.0), 0.0);
    assert_eq!(applied_edge_softness(1e9), MAX_EDGE_SOFTNESS);
    assert_eq!(applied_edge_softness(f32::NAN), DEFAULT_EDGE_SOFTNESS);
}

/// `count` is quantized **CPU-side**, because an eased binding is continuous
/// even where the arithmetic needs an integer, and it is bounded by the tier cap
/// so no build can walk off the end of the storage buffer.
#[test]
fn the_count_is_quantized_and_capped() {
    assert_eq!(applied_count(0.0, CAP), 0);
    assert_eq!(applied_count(-3.0, CAP), 0);
    // `floor`, not `round`: the fifteenth element arrives when it has arrived.
    assert_eq!(applied_count(14.9, CAP), 14);
    assert_eq!(applied_count(1e9, CAP), CAP);
    // A broken binding falls back to the authored canvas — a blank frame is the
    // worse failure.
    assert_eq!(applied_count(f32::NAN, CAP), AUTHORED_COUNT);
}

/// The declared vocabulary covers what a Phase 1 preset binds, so those
/// bindings are not warned at as typos (ADR-0020).
#[test]
fn the_vocabulary_carries_the_canvas_knobs() {
    for name in ["count", "scale", "paper", "opacity", "edge_softness"] {
        assert!(PARAMS.contains(&name), "`{name}` is missing from PARAMS");
    }
}

/// **The bounding box is tight for every kind.**
///
/// A loose box does not draw anything wrong — it costs a distance evaluation at
/// every pixel it wrongly admits, which is a silent regression against the
/// Phase 2 cost measurement. So it is asserted rather than inspected: for each
/// kind, at each of several rotations, the box's own half extents are compared
/// against the extents recovered from the shape's actual geometry.
#[test]
fn every_kind_gets_a_tight_bounding_box() {
    /// The largest relative slack a box may have over the true extent. Not zero:
    /// the recovered extents below go through the same `sin_cos` the builder
    /// does, so this is float agreement, not a tolerance on tightness.
    const SLACK: f32 = 1e-4;

    for &(kind, label) in &[
        (KIND_QUAD, "quad"),
        (KIND_CIRCLE, "circle"),
        (KIND_TRIANGLE, "triangle"),
    ] {
        for angle_deg in [0.0f32, 17.0, 45.0, 90.0, 133.0, -62.0] {
            let (hx, hy) = (0.31f32, 0.17f32);
            let e = Element::build(Spec {
                kind,
                center: [0.2, -0.4],
                half: [hx, hy],
                angle_deg,
                coord: 0.5,
                alpha: 1.0,
            });
            let (bx, by) = ((e.aabb[2] - e.aabb[0]) * 0.5, (e.aabb[3] - e.aabb[1]) * 0.5);

            // The true extent, recovered from the shape's own geometry rather
            // than from the builder's formula, so the two can disagree.
            let (sa, ca) = angle_deg.to_radians().sin_cos();
            let support = |px: f32, py: f32| ((ca * px - sa * py).abs(), (sa * px + ca * py).abs());
            let (tx, ty) = if kind == KIND_TRIANGLE {
                triangle_vertices(hx, hy)
                    .iter()
                    .fold((0.0f32, 0.0f32), |acc, v| {
                        let (x, y) = support(v[0], v[1]);
                        (acc.0.max(x), acc.1.max(y))
                    })
            } else {
                // Sampled around the boundary: the four corners for a quad, the
                // parametric circle for an ellipse. 4096 samples resolve the
                // extent far finer than SLACK.
                let mut tx = 0.0f32;
                let mut ty = 0.0f32;
                if kind == KIND_QUAD {
                    for (px, py) in [(hx, hy), (-hx, hy), (hx, -hy), (-hx, -hy)] {
                        let (x, y) = support(px, py);
                        tx = tx.max(x);
                        ty = ty.max(y);
                    }
                } else {
                    for i in 0..4096 {
                        let t = i as f32 / 4096.0 * std::f32::consts::TAU;
                        let (x, y) = support(hx * t.cos(), hy * t.sin());
                        tx = tx.max(x);
                        ty = ty.max(y);
                    }
                }
                (tx, ty)
            };

            let slack_x = bx / tx - 1.0;
            let slack_y = by / ty - 1.0;
            assert!(
                slack_x.abs() <= SLACK && slack_y.abs() <= SLACK,
                "{label} at {angle_deg} deg: box half-extents ({bx:.6}, {by:.6}) against the \
                 shape's own ({tx:.6}, {ty:.6}) — slack ({slack_x:+.2e}, {slack_y:+.2e}). \
                 A box that is too large is a silent cost regression; one that is too \
                 small clips the element."
            );
        }
    }
}

// ---------------------------------------------------------------------------
// The layout grammar (Plan 0113 Phase 4)
// ---------------------------------------------------------------------------

/// A recipe with everything but the fields a test varies held fixed.
fn recipe(grammar: Grammar, seed: u64, recompose: u64, count: usize) -> Recipe {
    Recipe {
        grammar,
        count,
        seed,
        recompose,
        size_hierarchy: 0.5,
        angle_bias: -0.384,
    }
}

/// The three grammars and the control, for tests that sweep all four.
const GRAMMARS: [Grammar; 4] = [
    Grammar::Authored,
    Grammar::AnchorSatellites,
    Grammar::DiagonalAxis,
    Grammar::SizeHierarchy,
];

/// `layout` selects a grammar by number, quantized CPU-side, and anything the
/// roster does not name falls back to the **control** rather than to a grammar
/// nobody asked for. This mapping is what `presets/README.md` documents.
#[test]
fn the_layout_selector_is_quantized_and_falls_back() {
    assert_eq!(Grammar::from_param(0.0), Grammar::Authored);
    assert_eq!(Grammar::from_param(1.0), Grammar::AnchorSatellites);
    assert_eq!(Grammar::from_param(2.0), Grammar::DiagonalAxis);
    assert_eq!(Grammar::from_param(3.0), Grammar::SizeHierarchy);
    // An eased binding passes through everything in between.
    assert_eq!(Grammar::from_param(1.4), Grammar::AnchorSatellites);
    assert_eq!(Grammar::from_param(1.6), Grammar::DiagonalAxis);
    // Off the roster, and broken.
    assert_eq!(Grammar::from_param(9.0), Grammar::Authored);
    assert_eq!(Grammar::from_param(-1.0), Grammar::Authored);
    assert_eq!(Grammar::from_param(f32::NAN), Grammar::Authored);
}

/// **The generator is deterministic**, and it is a function of the recipe and
/// nothing else — no wall clock, no unseeded randomness (the cross-cutting
/// rule). Asserted on the element list directly rather than on a rendered
/// frame, so a difference cannot hide under a rasterizer's tolerance.
///
/// The second half is the one that makes the first half worth anything: a
/// generator that ignored its seed would satisfy "same seed, same list"
/// perfectly.
#[test]
fn the_generator_is_a_pure_function_of_its_recipe() {
    for grammar in GRAMMARS {
        let mut a = Vec::with_capacity(CAP);
        let mut b = Vec::with_capacity(CAP);
        layout::generate(&mut a, &recipe(grammar, 7, 3, 24));
        layout::generate(&mut b, &recipe(grammar, 7, 3, 24));
        assert_eq!(
            a, b,
            "{grammar:?}: the same recipe produced two different canvases"
        );
        assert_eq!(a.len(), 24, "{grammar:?}: wrong element count");

        if grammar == Grammar::Authored {
            // The control's **geometry** is a fixed list, so it ignores the seed
            // — asserting that is what keeps the sweep below honest about which
            // arm it is testing. Its *motion* is seeded like every other
            // grammar's, deliberately: a control that could not drift would make
            // `drift` and `spin` silently inert on the one layout a preset gets
            // by default. So the comparison is on the specs, not on the whole.
            let mut c = Vec::with_capacity(CAP);
            layout::generate(&mut c, &recipe(grammar, 8, 4, 24));
            let geometry = |v: &[layout::Placed]| v.iter().map(|p| p.spec).collect::<Vec<_>>();
            assert_eq!(
                geometry(&a),
                geometry(&c),
                "the authored control's geometry must not vary with the seed"
            );
            assert_ne!(a, c, "the control's motion must still be seeded");
            continue;
        }

        // A different seed, and a different recomposition, each produce a
        // different canvas — and the two axes do not collide into one stream.
        let mut seeded = Vec::with_capacity(CAP);
        layout::generate(&mut seeded, &recipe(grammar, 8, 3, 24));
        assert_ne!(a, seeded, "{grammar:?}: the seed does not reach the canvas");

        let mut recomposed = Vec::with_capacity(CAP);
        layout::generate(&mut recomposed, &recipe(grammar, 7, 4, 24));
        assert_ne!(
            a, recomposed,
            "{grammar:?}: the recomposition index does not reach the canvas"
        );
        assert_ne!(
            seeded, recomposed,
            "{grammar:?}: seed and recomposition index collided into the same \
             stream — `seed + 1` and `recompose + 1` must not be the same canvas"
        );
    }
}

/// **It allocates once.** The element vector is sized to the tier cap at scene
/// construction and reused by `clear` + `push`; a thousand recompositions across
/// every grammar and every count must not move its capacity.
///
/// This is the plan's no-allocation duty. The generator runs on the render
/// thread rather than in the audio callback, so the real-time rule is not at
/// stake — but a reallocation mid-frame is still a spike.
#[test]
fn a_thousand_recompositions_never_reallocate() {
    let mut v = Vec::with_capacity(CAP);
    let cap = v.capacity();
    for i in 0..1_000u64 {
        let grammar = GRAMMARS[(i % GRAMMARS.len() as u64) as usize];
        // Counts either side of the cap, including zero and past it.
        let count = (i as usize * 7) % (CAP * 2 + 1);
        layout::generate(&mut v, &recipe(grammar, i, i * 3, count));
        assert_eq!(
            v.capacity(),
            cap,
            "recomposition {i} ({grammar:?}, count {count}) reallocated"
        );
        assert!(
            v.len() <= cap,
            "recomposition {i} produced {} elements over a cap of {cap}",
            v.len()
        );
    }
}

/// The three grammars compose **different** canvases from the same seed. Without
/// this the sample sheet Phase 5 judges could be three renders of one strategy.
#[test]
fn the_three_grammars_are_distinct() {
    let mut lists = Vec::new();
    for grammar in [
        Grammar::AnchorSatellites,
        Grammar::DiagonalAxis,
        Grammar::SizeHierarchy,
    ] {
        let mut v = Vec::with_capacity(CAP);
        layout::generate(&mut v, &recipe(grammar, 11, 0, 20));
        lists.push((grammar, v));
    }
    for (i, (ga, a)) in lists.iter().enumerate() {
        for (gb, b) in lists.iter().skip(i + 1) {
            assert_ne!(a, b, "{ga:?} and {gb:?} composed the same canvas");
        }
    }
}

/// The authored control is exactly the authored canvas at its own count, and
/// cycles rather than inventing past it — so the golden fixture and the shipped
/// preset pin a fixed picture, and Phase 5 has a fixed point to judge against.
#[test]
fn the_control_is_the_authored_canvas() {
    let mut v = Vec::with_capacity(CAP);
    layout::generate(&mut v, &recipe(Grammar::Authored, 0, 0, AUTHORED_COUNT));
    assert_eq!(v.len(), AUTHORED_COUNT);
    let specs: Vec<Spec> = v.iter().map(|p| p.spec).collect();
    assert_eq!(
        specs,
        SUPREMATIST.to_vec(),
        "the control drifted from the authored roster"
    );

    let mut wrapped = Vec::with_capacity(CAP);
    layout::generate(
        &mut wrapped,
        &recipe(Grammar::Authored, 0, 0, AUTHORED_COUNT + 3),
    );
    assert_eq!(
        &wrapped[..AUTHORED_COUNT],
        v.as_slice(),
        "the authored prefix must not move when the control is asked for more"
    );
    let wrapped_specs: Vec<Spec> = wrapped[AUTHORED_COUNT..].iter().map(|p| p.spec).collect();
    assert_eq!(wrapped_specs, SUPREMATIST[..3].to_vec(), "it must cycle");
}

/// A seed is quantized and bounded, and the bound is where `f32` stops being
/// able to tell two seeds apart rather than an arbitrary ceiling.
#[test]
fn the_seed_is_quantized_and_bounded() {
    assert_eq!(applied_seed(0.0), 0);
    assert_eq!(applied_seed(41.9), 41);
    assert_eq!(applied_seed(-5.0), 0);
    assert_eq!(applied_seed(1e30), MAX_SEED as u64);
    assert_eq!(applied_seed(f32::NAN), DEFAULT_SEED as u64);
}

/// The dominant angle **wraps** rather than clamping — an angle has no ends, and
/// a param walking past 360 must not stick at a bound.
#[test]
fn the_angle_bias_wraps() {
    let at = |d: f32| applied_angle_bias(d);
    assert!((at(0.0) - 0.0).abs() < 1e-6);
    assert!((at(360.0) - 0.0).abs() < 1e-5, "360 must be 0, not a bound");
    assert!(
        (at(-330.0) - at(30.0)).abs() < 1e-5,
        "negative angles wrap into the same circle"
    );
    assert!((at(f32::NAN) - DEFAULT_ANGLE_BIAS.to_radians()).abs() < 1e-6);
}

// ---------------------------------------------------------------------------
// The music moves the canvas (Plan 0113 Phase 6)
// ---------------------------------------------------------------------------

/// A GPU-free scene, for the motion assertions. Building one needs a device, so
/// these go through a headless context and then never touch it again — every
/// claim below is about the CPU-side element array.
fn scene(ctx: &RenderContext) -> ShapeCollageScene {
    ShapeCollageScene::new(&ctx.device, ctx.surface_format(), CAP)
}

/// Drive `frames` steps of `dt` and return the composed element centres.
fn walk(scene: &mut ShapeCollageScene, frames: u32, dt: f32) -> Vec<[f32; 2]> {
    for _ in 0..frames {
        scene.advance(dt);
    }
    scene
        .composed()
        .iter()
        .map(|e| [e.center_size[0], e.center_size[1]])
        .collect()
}

/// **The canvas moves with real time, not with frames** (ADR-0012).
///
/// One second stepped as 60 frames and as 30 must land every element in the same
/// place. A drift integrated by a fixed per-frame constant — the defect this
/// guards — would put the 60-frame run twice as far.
///
/// Non-vacuity is the second half and it is not decoration: the elements have to
/// have *moved*, or two identical still canvases would pass this perfectly.
#[test]
fn a_second_of_drift_is_a_second_at_any_frame_rate() {
    let Some(ctx) = context(64, 64) else {
        return;
    };
    let params = [
        ("layout", 2.0),
        ("seed", 5.0),
        ("drift", 1.0),
        ("spin", 1.0),
    ];

    let mut still = scene(&ctx);
    for (name, value) in params {
        still.set_param(name, value);
    }
    let start = walk(&mut still, 1, 0.0);

    let mut fast = scene(&ctx);
    let mut slow = scene(&ctx);
    for (name, value) in params {
        fast.set_param(name, value);
        slow.set_param(name, value);
    }
    let at_60 = walk(&mut fast, 60, 1.0 / 60.0);
    let at_30 = walk(&mut slow, 30, 1.0 / 30.0);

    assert_eq!(at_60.len(), at_30.len());
    assert!(!at_60.is_empty(), "the probe canvas is empty");

    let mut moved = 0.0f32;
    for ((a, b), s) in at_60.iter().zip(at_30.iter()).zip(start.iter()) {
        let gap = ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt();
        assert!(
            gap < 1e-4,
            "an element landed at {a:?} after 60 frames and {b:?} after 30 — one \
             second of drift must be one second of drift at any frame rate, so \
             this is a per-frame constant somewhere instead of the injected dt"
        );
        moved = moved.max(((a[0] - s[0]).powi(2) + (a[1] - s[1]).powi(2)).sqrt());
    }
    println!("one second of drift moved the furthest element {moved:.5} canvas units");
    assert!(
        moved > 1e-3,
        "nothing moved in a second, so this compared two still canvases: {moved}"
    );
}

/// **Raising `density` never reorders or pops an already-live element.**
///
/// Birth order is the array's own order and the gate is a prefix, so growing it
/// extends the live set and touches nothing already in it. Asserted on the
/// *colours and kinds* in painter order rather than on positions, because those
/// are what an element's identity is — a reorder would shuffle them.
#[test]
fn raising_density_only_ever_adds() {
    let Some(ctx) = context(64, 64) else {
        return;
    };
    let mut s = scene(&ctx);
    s.set_param("layout", 2.0);
    s.set_param("seed", 12.0);
    s.set_param("count", 20.0);

    let mut previous: Vec<[f32; 2]> = Vec::new();
    let mut counts = Vec::new();
    for step in 0..=10 {
        s.set_param("density", step as f32 / 10.0);
        // Long enough for every fade to finish, so "live" means visible.
        s.advance(1.0);
        let live: Vec<[f32; 2]> = s
            .composed()
            .iter()
            .filter(|e| e.tint[1] > 0.5)
            .map(|e| [e.tint[0], e.shape[2]])
            .collect();
        assert!(
            live.len() >= previous.len(),
            "density {} produced {} live elements, down from {}",
            step as f32 / 10.0,
            live.len(),
            previous.len()
        );
        assert_eq!(
            &live[..previous.len()],
            previous.as_slice(),
            "raising density to {} reordered or replaced an element that was \
             already live — the gate must be a PREFIX of the birth order",
            step as f32 / 10.0
        );
        counts.push(live.len());
        previous = live;
    }
    println!("live elements as density rises 0 -> 1: {counts:?}");
    assert!(
        counts.first() < counts.last(),
        "density did nothing across its whole range: {counts:?}"
    );
}

/// **The canvas does not breathe in unison.**
///
/// `pump_size` and `pump_alpha` are phase-offset per element, so at any instant
/// the modulation across live elements differs. The plan asks for exactly this
/// property and no threshold on the spread — a number there would be inventing
/// one — so the assertion is that the values are *not all equal*, plus the
/// non-vacuity that the pump moved anything at all.
#[test]
fn the_pump_is_phase_offset_across_elements() {
    let Some(ctx) = context(64, 64) else {
        return;
    };
    let mut s = scene(&ctx);
    s.set_param("layout", 2.0);
    s.set_param("seed", 3.0);
    s.set_param("count", 16.0);
    s.set_param("pump_size", 0.4);
    s.set_param("pump_alpha", 0.5);
    // A quarter of a pump period in, so no element sits at a node by accident.
    s.advance(0.45);

    let sizes: Vec<f32> = s.composed().iter().map(|e| e.center_size[2]).collect();
    let alphas: Vec<f32> = s.composed().iter().map(|e| e.tint[1]).collect();
    assert!(
        sizes.len() > 4,
        "the probe canvas is too small to say anything"
    );

    // Against the same canvas with the pump off: the ratio is the modulation,
    // which is what has to differ across elements rather than the raw size.
    let mut flat = scene(&ctx);
    flat.set_param("layout", 2.0);
    flat.set_param("seed", 3.0);
    flat.set_param("count", 16.0);
    flat.advance(0.45);
    let base: Vec<f32> = flat.composed().iter().map(|e| e.center_size[2]).collect();

    let ratios: Vec<f32> = sizes
        .iter()
        .zip(base.iter())
        .map(|(a, b)| a / b.max(1e-6))
        .collect();
    let lo = ratios.iter().copied().fold(f32::INFINITY, f32::min);
    let hi = ratios.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    println!(
        "size modulation across {} elements: {lo:.4}..{hi:.4}",
        ratios.len()
    );
    assert!(
        hi > lo,
        "every element is pumping by exactly {lo} — the phase offset is not \
         reaching them, so the canvas breathes as one sheet"
    );
    assert!(
        (hi - 1.0).abs() > 1e-4 || (lo - 1.0).abs() > 1e-4,
        "the pump modulated nothing at all"
    );

    let a_lo = alphas.iter().copied().fold(f32::INFINITY, f32::min);
    let a_hi = alphas.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    println!("alpha across elements: {a_lo:.4}..{a_hi:.4}");
    assert!(a_hi > a_lo, "every element carries the same alpha");
}

/// **`recompose` is edge-triggered, and `recompose_blend` decides cut or fade.**
///
/// A held gate must recompose once, not every frame — the swarm's `reseed`
/// contract, and the reason `prev_recompose` survives `reset_params`. At blend
/// zero the new canvas is complete on the next frame; above zero both canvases
/// are on screen for that many seconds.
#[test]
fn a_recomposition_fires_once_per_edge() {
    let Some(ctx) = context(64, 64) else {
        return;
    };
    let mut s = scene(&ctx);
    s.set_param("layout", 2.0);
    s.set_param("seed", 9.0);
    s.set_param("count", 12.0);
    s.advance(1.0 / 60.0);
    let first: Vec<f32> = s.composed().iter().map(|e| e.tint[0]).collect();
    assert_eq!(s.composed().len(), 12, "the hard-cut canvas is one canvas");

    // Hold the gate high across several frames: exactly one recomposition.
    for _ in 0..5 {
        s.set_param("recompose", 1.0);
        s.advance(1.0 / 60.0);
    }
    assert_eq!(
        s.recompositions(),
        1,
        "a HELD gate recomposed more than once — it must be edge-triggered"
    );
    let second: Vec<f32> = s.composed().iter().map(|e| e.tint[0]).collect();
    assert_ne!(first, second, "the recomposition drew the same canvas");
    assert_eq!(
        s.composed().len(),
        12,
        "at recompose_blend = 0 the cut is hard, so only one canvas is drawn"
    );

    // Drop and raise again: a second edge, a second recomposition.
    s.set_param("recompose", 0.0);
    s.advance(1.0 / 60.0);
    s.set_param("recompose", 1.0);
    s.advance(1.0 / 60.0);
    assert_eq!(s.recompositions(), 2, "a fresh edge must fire again");

    // With a blend, both canvases are live for its duration and neither after.
    let mut b = scene(&ctx);
    b.set_param("layout", 2.0);
    b.set_param("seed", 9.0);
    b.set_param("count", 12.0);
    b.set_param("recompose_blend", 0.5);
    b.advance(1.0 / 60.0);
    b.set_param("recompose", 1.0);
    b.advance(1.0 / 60.0);
    assert_eq!(
        b.composed().len(),
        24,
        "mid-blend both canvases are on screen"
    );
    b.set_param("recompose", 0.0);
    b.advance(0.6);
    assert_eq!(
        b.composed().len(),
        12,
        "past its duration the blend is finished and the outgoing canvas is gone"
    );
}

// ---------------------------------------------------------------------------
// Rendered
// ---------------------------------------------------------------------------

/// A raw headless context, or `None` (a logged skip) on a runner with no GPU
/// adapter — macOS has no software Metal fallback (ADR-0016).
fn context(width: u32, height: u32) -> Option<RenderContext> {
    match RenderContext::new_headless(width, height, true) {
        Ok(ctx) => Some(ctx),
        Err(CtxError::RequestAdapter(_)) => {
            eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
            None
        }
        Err(e) => panic!("headless context build failed: {e}"),
    }
}

/// Draw one canvas of `specs`, in painter order, straight through this scene's
/// pipeline and read the surface back.
///
/// Deliberately **not** through `Renderer`: the element list is what is under
/// test and no preset can name one. The palette is the engine default, so the
/// only variable between two calls is the array.
fn paint(ctx: &RenderContext, width: u32, height: u32, specs: &[Spec]) -> CaptureImage {
    let mut scene = ShapeCollageScene::new(&ctx.device, ctx.surface_format(), CAP);
    scene.set_specs(specs);
    let (target, view) = capture::create_target(&ctx.device, ctx.surface_format(), width, height);
    let (buffer, padded_bpr) = capture::create_readback(&ctx.device, width, height);
    let mut encoder = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("shape-collage-probe"),
        });
    capture::record_clear(&mut encoder, &view);
    scene.render(
        &ctx.queue,
        &mut encoder,
        &view,
        width as f32 / height as f32,
    );
    capture::record_copy(&mut encoder, &target, &buffer, padded_bpr, width, height);
    ctx.queue.submit(std::iter::once(encoder.finish()));
    capture::read_back(&ctx.device, &buffer, width, height, padded_bpr)
        .expect("read back the painted canvas")
}

/// One pixel's RGB.
fn rgb(img: &CaptureImage, x: u32, y: u32) -> [u8; 3] {
    let i = ((y * img.width + x) * 4) as usize;
    [img.rgba[i], img.rgba[i + 1], img.rgba[i + 2]]
}

/// The pixel a canvas point lands on, at the default `scale` and no pan.
fn at(width: u32, height: u32, cx: f32, cy: f32) -> (u32, u32) {
    let aspect = width as f32 / height as f32;
    // The inverse of the shader's `uv.x = ndc.x * aspect`, then NDC to pixels.
    // NDC y is up and the capture is top-down, so y flips.
    let px = ((cx / aspect * 0.5 + 0.5) * width as f32) as u32;
    let py = ((0.5 - cy * 0.5) * height as f32) as u32;
    (px.min(width - 1), py.min(height - 1))
}

/// **Occlusion is order, and this is the assertion that proves it.**
///
/// Two overlapping elements are painted in both array orders. In each frame the
/// overlap must take the colour of the **later** element — matched against that
/// element's own uncovered interior in the same frame, so the comparison is not
/// against a computed colour — and the two frames must differ there, because a
/// painter that ignored order would draw both the same.
#[test]
fn the_later_element_wins_the_overlap() {
    const W: u32 = 192;
    const H: u32 = 120;
    let Some(ctx) = context(W, H) else {
        return;
    };

    // Two big quads crossing at the origin, far enough apart in palette
    // coordinate that the default gradient gives them clearly different colours.
    let horizontal = Spec {
        kind: KIND_QUAD,
        center: [0.0, 0.0],
        half: [0.7, 0.15],
        angle_deg: 0.0,
        coord: 0.15,
        alpha: 1.0,
    };
    let vertical = Spec {
        kind: KIND_QUAD,
        center: [0.0, 0.0],
        half: [0.15, 0.7],
        angle_deg: 0.0,
        coord: 0.65,
        alpha: 1.0,
    };

    let h_only = at(W, H, 0.5, 0.0); // inside the horizontal bar alone
    let v_only = at(W, H, 0.0, 0.5); // inside the vertical bar alone
    let cross = at(W, H, 0.0, 0.0); // the overlap

    let first = paint(&ctx, W, H, &[horizontal, vertical]);
    let second = paint(&ctx, W, H, &[vertical, horizontal]);

    // Non-vacuity: the two elements really are different colours, or nothing
    // below could tell an ordered painter from an unordered one.
    let (h_col, v_col) = (
        rgb(&first, h_only.0, h_only.1),
        rgb(&first, v_only.0, v_only.1),
    );
    println!(
        "horizontal {h_col:?} vertical {v_col:?}; crossing {:?} / {:?}",
        rgb(&first, cross.0, cross.1),
        rgb(&second, cross.0, cross.1)
    );
    assert_ne!(
        h_col, v_col,
        "the two probe elements resolved to the same colour, so this test can \
         see nothing — move their palette coordinates apart"
    );

    assert_eq!(
        rgb(&first, cross.0, cross.1),
        v_col,
        "with the vertical bar LAST the crossing must be the vertical bar's colour"
    );
    assert_eq!(
        rgb(&second, cross.0, cross.1),
        h_col,
        "with the horizontal bar LAST the crossing must be the horizontal bar's colour"
    );
    assert_ne!(
        rgb(&first, cross.0, cross.1),
        rgb(&second, cross.0, cross.1),
        "the two array orders drew the same crossing, so array order is not the \
         depth — which is the whole of this scene's occlusion mechanism"
    );
}

/// **Flat colour is exact.**
///
/// A palette of two flat plateaus is authored; a large quad takes the darker
/// one, whose brightest channel sits under the tonemap's knee. Below the knee
/// ADR-0046's curve is the **identity**, so the element's interior must arrive
/// at the display encoding of exactly the linear value that was authored — this
/// is a property, not a measurement, and a wider tolerance would be hiding
/// something.
///
/// The two bytes admitted are named rather than fitted: one for the 8-bit
/// write's own rounding, one for the tonemap's `+-1` encoded-level dither
/// (ADR-0096), which is part of the display write and not part of the scene. The
/// interior's *spread* is asserted separately and at the same width, which is
/// the "flat" half of the claim.
#[test]
fn an_element_under_the_knee_arrives_at_the_value_it_was_authored_at() {
    const W: u32 = 160;
    const H: u32 = 100;
    /// The authored element colour. Brightest channel `0x88/255 = 0.533`, under
    /// `KNEE`, so the whole tonemap is the identity for it.
    const HEX: [u8; 3] = [0x88, 0x22, 0x44];
    /// Rounding (1) plus the dither's one encoded level (1). See the doc.
    const TOL: i32 = 2;

    assert!(
        f32::from(HEX[0]) / 255.0 <= KNEE,
        "the probe colour must sit under the knee or this test asserts nothing"
    );

    let Some(mut renderer) = headless(W, H) else {
        return;
    };
    // A palette whose lower half is the probe colour flat and whose upper half is
    // the paper, with a hard transition, so a coordinate's colour is exact rather
    // than interpolated.
    let toml = format!(
        "name = \"collage_flat\"\nsystem = \"shape_collage\"\n\
         [palette]\nstops = [\n\
         {{ at = 0.0, color = \"#{0:02x}{1:02x}{2:02x}\" }},\n\
         {{ at = 0.4999, color = \"#{0:02x}{1:02x}{2:02x}\" }},\n\
         {{ at = 0.5001, color = \"#f0ece0\" }},\n\
         {{ at = 1.0, color = \"#f0ece0\" }},\n\
         ]\n\
         [params]\ncount = \"1\"\nscale = \"1\"\n",
        HEX[0], HEX[1], HEX[2]
    );
    let preset = Preset::from_toml_str(&toml).expect("the flat-colour probe parses");
    renderer.set_presets(vec![preset]);
    let img = renderer
        .capture_preset("collage_flat", &AnalysisFrame::default(), 2)
        .expect("capture the flat-colour probe");

    // Element 0 of the authored canvas is the broad plane centred at
    // (-0.15, 0.10) at coordinate 0.4375 — the lower plateau. With `count = 1`
    // nothing is drawn over it.
    let (px, py) = at(W, H, -0.15, 0.10);
    let expected = HEX.map(|c| encoded(f32::from(c) / 255.0));

    let mut lo = [255i32; 3];
    let mut hi = [0i32; 3];
    for dy in 0..3u32 {
        for dx in 0..3u32 {
            let px = rgb(&img, px + dx - 1, py + dy - 1);
            for c in 0..3 {
                lo[c] = lo[c].min(i32::from(px[c]));
                hi[c] = hi[c].max(i32::from(px[c]));
            }
        }
    }
    println!("authored {HEX:?} -> expected {expected:?}, read {lo:?}..{hi:?}");

    for c in 0..3 {
        assert!(
            (lo[c] - i32::from(expected[c])).abs() <= TOL
                && (hi[c] - i32::from(expected[c])).abs() <= TOL,
            "channel {c}: authored {} (linear {:.4}) encodes to {}, but the element's \
             interior reads {}..{}. Below KNEE = {KNEE} the tonemap is the identity, so \
             the only admitted difference is the display write's own rounding and dither.",
            HEX[c],
            f32::from(HEX[c]) / 255.0,
            expected[c],
            lo[c],
            hi[c],
        );
        assert!(
            hi[c] - lo[c] <= TOL,
            "channel {c}: the element's interior spans {}..{} — it is not flat",
            lo[c],
            hi[c],
        );
    }
}

/// The sRGB transfer function, to a byte — what the `Rgba8UnormSrgb` surface
/// does to a linear value after the shader has run.
fn encoded(linear: f32) -> u8 {
    let e = if linear <= 0.003_130_8 {
        12.92 * linear
    } else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    };
    (e.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// A headless renderer on the software adapter, or `None` (a logged skip).
fn headless(width: u32, height: u32) -> Option<Renderer> {
    match Renderer::new_headless(HeadlessOptions {
        width,
        height,
        prefer_software: true,
    }) {
        Ok(r) => Some(r),
        Err(RenderError::RequestAdapter(_)) => {
            eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
            None
        }
        Err(e) => panic!("headless renderer build failed: {e}"),
    }
}

/// **The aspect comes from the render target, and this test bites**
/// ([ADR-0037](../../../../../docs/adrs/0037-internal-grid-is-a-resolution-not-a-shape.md)).
///
/// A circle element is painted at **1280x800** and its own extent measured in
/// pixels. It must be round — the same number of pixels across as down — which
/// an aspect taken from anywhere but the render target cannot be.
///
/// The size is the plan's, and it is the point: 1920x1080 and this box's
/// 2048x1152 are both exactly 16:9, where every wrong aspect source is right by
/// accident. 16:10 distorts a dropped aspect by 1.6 and an inverted one by 2.56.
/// The 1:1.6 case is rendered as well, so a hard-coded 1.6 fails too.
///
/// **Confirmed to bite, in the reverted direction**, which is the only way this
/// claim is worth anything. Substituting a literal `1.0` for the target's aspect
/// on the way to the uniform — the ADR-0037 defect, which has shipped three
/// times in this repo — renders the circle **319 px across and 199 px down** at
/// 1280x800 (ratio 1.603), and this test fails on it. As shipped it measures
/// 199 x 199 at both sizes.
#[test]
fn a_circle_element_is_round_at_sixteen_by_ten() {
    let circle = [Spec {
        kind: KIND_CIRCLE,
        center: [0.0, 0.0],
        half: [0.5, 0.5],
        angle_deg: 0.0,
        coord: 0.15,
        alpha: 1.0,
    }];

    for (w, h) in [(1280u32, 800u32), (500, 800)] {
        let Some(ctx) = context(w, h) else {
            return;
        };
        let img = paint(&ctx, w, h, &circle);
        let (cx, cy) = (w / 2, h / 2);
        let centre = rgb(&img, cx, cy);
        let differs =
            |p: [u8; 3]| (0..3).any(|c| i32::from(p[c]).abs_diff(i32::from(centre[c])) > 8);

        let mut half_w = 0u32;
        while cx + half_w + 1 < w && !differs(rgb(&img, cx + half_w + 1, cy)) {
            half_w += 1;
        }
        let mut half_h = 0u32;
        while cy + half_h + 1 < h && !differs(rgb(&img, cx, cy + half_h + 1)) {
            half_h += 1;
        }
        println!("{w}x{h}: circle half-extent {half_w} px across, {half_h} px down");
        assert!(
            half_w > 8 && half_h > 8,
            "{w}x{h}: the circle has no measurable extent ({half_w} x {half_h}) — the \
             probe found no edge, so it is measuring nothing"
        );
        let ratio = half_w as f32 / half_h as f32;
        assert!(
            (ratio - 1.0).abs() < 0.05,
            "at {w}x{h} the circle is {half_w} px across and {half_h} px down (ratio \
             {ratio:.3}) — it must be ROUND. An aspect taken from anywhere but the \
             render target distorts it by exactly the target's own aspect."
        );
    }
}
