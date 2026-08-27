//! The warp mesh's CPU-side contracts (Plan 0100 Phases 1 and 4). GPU-free
//! throughout: everything asserted here is arithmetic, a roster, a clamp, or the
//! geometry the draw layer builds — the pixels are the golden guard's business.
//!
//! Phase 4's draw layer is tested **as geometry rather than as pixels**, and
//! deliberately. "Each `wave_mode` renders distinguishably from the others" is a
//! statement about the figures the modes build, and comparing eight point sets
//! answers it exactly; comparing eight captures answers a weaker version of it
//! through a rasterizer, a blend and a tonemap, at a hundred times the cost. The
//! one thing geometry cannot see — that the new draw seam does not eat a lit
//! backdrop — is a capture, and it lives in `core/tests/warp_mesh.rs` with the
//! other GPU work.

// Test asserts panic on failure; allowed here over the module's pragma.
#![allow(clippy::panic, clippy::indexing_slicing, clippy::unwrap_used)]

use super::*;
use crate::render::TierConfig;

/// The roster and its defaults are the same length and describe the same thing,
/// and the identity really is the identity — the affine this idiom generalizes
/// (ADR-0048) at every one of its nine outputs.
#[test]
fn the_per_vertex_roster_is_the_identity_at_rest() {
    assert_eq!(PER_VERTEX_PARAMS.len(), PER_VERTEX_DEFAULTS.len());
    for (name, value) in PER_VERTEX_PARAMS.iter().zip(PER_VERTEX_DEFAULTS) {
        let expected = match *name {
            // A factor: unit scale.
            "zoom" | "sx" | "sy" => 1.0,
            // The fixed point: the middle of the frame, in uv.
            "cx" | "cy" => 0.5,
            // A rate: nothing moves.
            _ => 0.0,
        };
        assert_eq!(
            value, expected,
            "`{name}`'s default must be the identity, so a preset with no \
             [per_vertex] table and no scalar bindings leaves the past still"
        );
    }
}

/// Every per-vertex name is also an ordinary scalar param, which is what makes a
/// `[per_vertex]` binding an *override* of a whole-mesh value rather than a
/// second unrelated vocabulary.
#[test]
fn every_per_vertex_output_is_also_a_scalar_param() {
    for name in PER_VERTEX_PARAMS {
        assert!(
            PARAMS.contains(name),
            "`{name}` is a per-vertex output but not a scalar param, so a preset \
             could not set it for the whole mesh"
        );
    }
}

/// **The clamp is one function, and both consumers get the same answer**
/// (the contract `set_per_vertex` relies on): the renderer sizes its scratch from
/// it and the scene sizes its vertex buffer from it, so a disagreement would send
/// a series of the wrong length every frame.
#[test]
fn the_tier_clamp_is_shared_and_bounded() {
    let floor = TierConfig::FLOOR;
    let rich = TierConfig::RICH;

    // A request inside the floor's ceiling survives both tiers unchanged.
    assert_eq!(clamp_grid((16, 12), &floor), (16, 12));
    assert_eq!(clamp_grid((16, 12), &rich), (16, 12));

    // A request over the floor's ceiling is cut to it, and the rich tier carries
    // more of it — the whole point of the capacity.
    let big = (128, 96);
    assert_eq!(clamp_grid(big, &floor), floor.mesh_grid);
    assert_eq!(clamp_grid(big, &rich), rich.mesh_grid);
    assert!(
        clamp_grid(big, &rich).0 > clamp_grid(big, &floor).0,
        "the rich tier must carry a finer mesh than the floor, or the capacity \
         buys nothing"
    );

    // Never below the minimum, whatever a caller asks for or a tier declares.
    assert_eq!(clamp_grid((0, 0), &floor), (MIN_MESH, MIN_MESH));
    let daft = TierConfig {
        mesh_grid: (0, 100_000),
        ..floor
    };
    let clamped = clamp_grid((64, 64), &daft);
    assert_eq!(
        clamped.0, MIN_MESH,
        "a degenerate tier ceiling still floors"
    );
    assert_eq!(
        clamped.1, 64,
        "a tier ceiling past the format maximum does not let a request exceed \
         the format maximum either"
    );
    assert_eq!(
        clamp_grid((999, 999), &daft),
        (MIN_MESH, MAX_MESH.1),
        "the format maximum is the hard ceiling on both axes"
    );
}

/// The vertex count is the fencepost, not the cell count — the single most
/// likely off-by-one in this whole path, since the renderer's scratch, the
/// scene's arrays and the index buffer are all sized by it.
#[test]
fn the_vertex_count_is_one_more_than_the_cells() {
    assert_eq!(vertex_count((1, 1)), 4);
    assert_eq!(vertex_count((2, 3)), 3 * 4);
    assert_eq!(vertex_count(MAX_MESH), 129 * 97);
    // Two triangles per cell, three indices each.
    assert_eq!(build_indices((4, 3)).len(), 4 * 3 * 6);
    // Every index addresses a real vertex.
    let n = vertex_count((4, 3)) as u32;
    assert!(build_indices((4, 3)).iter().all(|&i| i < n));
}

/// **`rad` and `ang` take their aspect from the render target, never from the
/// mesh** (ADR-0037) — the bug this family has shipped three times, and the most
/// likely place for a fourth because the grid here is user-visible.
///
/// The property asserted is the one an author relies on: the `rad` at a given
/// *screen* position does not depend on how many cells the mesh has, and it does
/// depend on the target's shape. A `rad` derived from the grid would satisfy the
/// first and fail the second.
#[test]
fn rad_and_ang_come_from_the_target_aspect_and_not_from_the_grid() {
    // The middle of the right edge, on three different grids, at one aspect.
    let a = vertex_position(8, 4, (8, 8), 1.0);
    let b = vertex_position(64, 32, (64, 64), 1.0);
    let c = vertex_position(2, 1, (2, 2), 1.0);
    assert!((a.2 - b.2).abs() < 1e-6, "{a:?} vs {b:?}");
    assert!((a.2 - c.2).abs() < 1e-6, "{a:?} vs {c:?}");
    assert_eq!(
        a.2, 1.0,
        "at aspect 1 the edge midpoint is one half-height out"
    );

    // The same screen position on a 16:9 target is further out, because a
    // half-width is 1.78 half-heights there.
    let wide = vertex_position(8, 4, (8, 8), 16.0 / 9.0);
    assert!(
        (wide.2 - 16.0 / 9.0).abs() < 1e-5,
        "the aspect must scale x, got {}",
        wide.2
    );
    // ...while the middle of the BOTTOM edge is one half-height out on both, which
    // is what makes a `rad`-driven figure round rather than stretched.
    assert!((vertex_position(4, 8, (8, 8), 16.0 / 9.0).2 - 1.0).abs() < 1e-6);

    // The centre is the origin, and the four cardinals are where they look.
    let centre = vertex_position(4, 4, (8, 8), 1.0);
    assert_eq!((centre.0, centre.1), (0.5, 0.5));
    assert_eq!(centre.2, 0.0);
    let right = vertex_position(8, 4, (8, 8), 1.0);
    assert!(right.3.abs() < 1e-6, "+x is angle 0, got {}", right.3);
    let top = vertex_position(4, 0, (8, 8), 1.0);
    assert!(
        (top.3 - std::f32::consts::FRAC_PI_2).abs() < 1e-5,
        "the TOP of the screen is a quarter turn counter-clockwise (y is flipped \
         on the way in), got {}",
        top.3
    );
    assert!(
        (0.0..std::f32::consts::TAU).contains(&vertex_position(4, 8, (8, 8), 1.0).3),
        "`ang` is wrapped into 0..tau"
    );

    // `x`/`y` are the vertex's uv with y = 0 at the top — texture space, which is
    // what every sampler in the scene addresses.
    assert_eq!(vertex_position(0, 0, (8, 8), 1.0).0, 0.0);
    assert_eq!(vertex_position(0, 0, (8, 8), 1.0).1, 0.0);
    assert_eq!(vertex_position(8, 8, (8, 8), 1.0).1, 1.0);

    // A degenerate aspect degrades to square rather than poisoning every vertex.
    for bad in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        let v = vertex_position(8, 4, (8, 8), bad);
        assert!(v.2.is_finite(), "aspect {bad} produced rad {}", v.2);
    }
}

/// **`ang`'s branch cut and its handedness, pinned** — Plan 0111 Phase 4, carrying
/// design-backlog 0119.
///
/// # What this settles
///
/// Two facts about *this engine*, both derived from
/// [`vertex_position`]'s arithmetic and neither read off a picture:
///
/// - the cut is on **+x**, where `atan2`'s own range boundary is lifted into
///   `0..tau`, and it is a genuine discontinuity of size `tau` rather than a
///   steep gradient;
/// - `ang` increases **counter-clockwise as seen on screen**, because
///   `vertex_position` flips y (`py = (0.5 - y) * 2`) before the `atan2`. Texture
///   space is y-down and the polar pair is taken y-**up**.
///
/// Pinned so neither can move silently. `milkconv/tests/warp_geometry.rs` asserts
/// the engine carries the same y-down/y-up asymmetry in the emitted WGSL warp
/// epilogue and in the draw layer, so these three agree by test rather than by
/// coincidence.
///
/// # What this does **not** settle, and why the phase stopped there
///
/// **Whether the reference's handedness matches.** That is the actual question
/// behind backlog 0119: MilkDrop's `atan2` has the same cut, so presets are
/// *authored against* a discontinuity at +x and smoothing the wrap would break
/// every preset that uses it deliberately — but if the **handedness** differs,
/// every angle-driven per-vertex program in the corpus runs mirrored and the
/// visible seam is a symptom rather than the defect.
///
/// Plan 0111 Phase 4 required that comparison be derived from the source format's
/// convention or from the reference implementation, with the source named, and
/// **never from a picture**. Neither is available here: the corpus is 10 347
/// `.milk` files with no MilkDrop source and no authoring documentation beside
/// them, and a corpus-wide search for a preset that states a rotation direction
/// returns two files, both building their own Kardan rotation from `q` variables
/// rather than reading the per-vertex `ang`. A `.milk` preset does not record the
/// convention it was authored against.
///
/// So the phase changes no behaviour and does not claim the seam is
/// authored-against either — that claim needs the half that is missing. What
/// would settle it, in order of directness: MilkDrop 2's `milkdropfs.cpp` mesh
/// setup, where the sign of the `y` handed to `atan2f` is one line; or the
/// authoring documentation that shipped with MilkDrop; or one reference capture
/// of a preset built to be handedness-revealing, which is a look-gate artifact
/// and not a test.
///
/// Per ADR-0071's prose rule, nothing here attributes a convention to "MilkDrop"
/// at all. It states this engine's, and names the question.
#[test]
fn ang_cuts_on_plus_x_and_turns_counter_clockwise_on_screen() {
    use std::f32::consts::{FRAC_PI_2, PI, TAU};

    let mesh = (64, 64);
    let ang = |col: u32, row: u32| vertex_position(col, row, mesh, 1.0).3;

    // Handedness: walking counter-clockwise on screen — right, top, left, bottom
    // — `ang` increases. The TOP of the screen is a QUARTER turn, which is the
    // y-flip's whole visible consequence: without it the top would read 3/4.
    let right = ang(64, 32);
    let top = ang(32, 0);
    let left = ang(0, 32);
    let bottom = ang(32, 64);
    assert!(right.abs() < 1e-6, "+x is angle 0, got {right}");
    assert!(
        (top - FRAC_PI_2).abs() < 1e-5,
        "the top is tau/4, got {top}"
    );
    assert!(
        (left - PI).abs() < 1e-5,
        "the left edge is tau/2, got {left}"
    );
    assert!(
        (bottom - 3.0 * FRAC_PI_2).abs() < 1e-5,
        "the bottom is 3*tau/4, got {bottom}"
    );

    // The cut is ON +x and it is a real discontinuity. One row above and one row
    // below the midline, at the right edge: the two are adjacent vertices whose
    // `ang` differ by very nearly a full turn.
    let just_above = ang(64, 31);
    let just_below = ang(64, 33);
    assert!(
        just_above < FRAC_PI_2 && just_below > 3.0 * FRAC_PI_2,
        "the +x neighbours must straddle the wrap, got {just_above} and {just_below}"
    );
    let jump = just_below - just_above;
    assert!(
        jump > TAU * 0.9,
        "the cut on +x must be a discontinuity of nearly a full turn, got {jump}"
    );

    // ...and nowhere else. Sweeping the ring away from +x, no adjacent pair jumps
    // by even a quarter turn, so a program continuous in `ang` meets exactly one
    // seam rather than several.
    let ring: Vec<f32> = (1..64).map(|row| ang(64, row)).collect();
    for pair in ring.windows(2) {
        let step = (pair[1] - pair[0]).abs();
        assert!(
            !(FRAC_PI_2..=TAU * 0.9).contains(&step),
            "off the +x axis `ang` must be continuous; a step of {step} is neither \
             smooth nor the wrap"
        );
    }
}

/// A `[per_vertex]` binding **overrides** the scalar of the same name, and only
/// that name — the composition rule the module docs state, asserted on the
/// assembled vertex buffer rather than on prose.
#[test]
fn a_per_vertex_binding_overrides_only_its_own_scalar() {
    let mesh = (2, 2);
    let mut state = MeshState::new(mesh);
    // Every scalar off its default, so an override is distinguishable from a
    // fallback in either direction.
    let scalars: [f32; OUTPUTS] = [2.0, 0.5, 0.25, 0.75, 0.1, 0.2, 3.0, 4.0, 5.0];

    state.assemble(&scalars);
    for v in &state.vertices {
        assert_eq!(
            v.t0,
            [2.0, 0.5, 0.25, 0.75],
            "unbound outputs take the scalar"
        );
        assert_eq!(v.t1, [0.1, 0.2, 3.0, 4.0]);
        assert_eq!(v.t2[0], 5.0);
    }

    // Bind `rot` (index 1) per vertex.
    let n = vertex_count(mesh);
    let series: Vec<f32> = (0..n).map(|i| i as f32).collect();
    state.values[1].copy_from_slice(&series);
    state.bound[1] = true;
    state.assemble(&scalars);
    for (i, v) in state.vertices.iter().enumerate() {
        assert_eq!(v.t0[1], i as f32, "vertex {i} takes its own `rot`");
        assert_eq!(v.t0[0], 2.0, "`zoom` still takes the scalar");
        assert_eq!(v.t1[2], 3.0, "`sx` still takes the scalar");
    }
}

/// The assembled clip positions span the whole target exactly once, so the warp
/// pass covers every pixel — the property that lets it write `REPLACE`.
#[test]
fn the_mesh_covers_the_target_exactly() {
    let mut state = MeshState::new((4, 3));
    state.assemble(&PER_VERTEX_DEFAULTS);
    let xs: Vec<f32> = state.vertices.iter().map(|v| v.clip[0]).collect();
    let ys: Vec<f32> = state.vertices.iter().map(|v| v.clip[1]).collect();
    assert_eq!(xs.iter().cloned().fold(f32::MAX, f32::min), -1.0);
    assert_eq!(xs.iter().cloned().fold(f32::MIN, f32::max), 1.0);
    assert_eq!(ys.iter().cloned().fold(f32::MAX, f32::min), -1.0);
    assert_eq!(ys.iter().cloned().fold(f32::MIN, f32::max), 1.0);
    // Vertex 0 is the top-left, which is the order `set_per_vertex` documents.
    assert_eq!(state.vertices[0].clip, [-1.0, 1.0]);
}

/// A grid change reallocates off the hot path and leaves a consistent state — the
/// per-vertex arrays, the flags and the vertex buffer all move together.
#[test]
fn resizing_the_grid_keeps_every_array_in_step() {
    let mut state = MeshState::new((4, 4));
    state.bound[0] = true;
    state.resize((4, 4));
    assert!(state.bound[0], "a same-size resize is a no-op");
    state.resize((8, 6));
    assert_eq!(state.mesh, (8, 6));
    assert_eq!(state.vertices.len(), vertex_count((8, 6)));
    for series in &state.values {
        assert_eq!(series.len(), vertex_count((8, 6)));
    }
    assert!(
        !state.bound[0],
        "a resize drops the previous grid's bindings rather than reading a \
         stale series of the wrong length"
    );
}

// ---------------------------------------------------------------------------
// The tier measurement (Plan 0100 Phase 1's done-when)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// The draw layer (Plan 0100 Phase 4)
// ---------------------------------------------------------------------------

/// An empty bundle's runtime: no custom waves, no custom shapes, no programs.
///
/// Enough for every producer the [`FrameOutputs`](crate::milk::outputs::FrameOutputs)
/// roster alone drives — the waveform, the two borders, the motion grid — which
/// is what the three tests below need. The custom elements need EEL2 to be
/// compiled and so are tested from `milkconv/tests/draw_layer.rs`, on the far
/// side of the seam that owns the compiler.
fn bare_runtime() -> crate::milk::MilkRuntime {
    let bundle =
        crate::milk::MilkBundle::from_assembly(None, None, None).expect("the empty bundle decodes");
    crate::milk::MilkRuntime::new(bundle, 0)
}

/// A waveform trace with structure in it — a sine, so every `wave_mode` has
/// something to shape and none of them collapses to a straight line.
fn trace() -> [f32; crate::dsp::WAVE_SAMPLES] {
    std::array::from_fn(|i| {
        (i as f32 / crate::dsp::WAVE_SAMPLES as f32 * std::f32::consts::TAU * 3.0).sin()
    })
}

/// The outputs a preset that draws only its waveform leaves — everything else
/// off, so each test isolates the producer it is about.
fn waveform_only() -> crate::milk::outputs::FrameOutputs {
    crate::milk::outputs::FrameOutputs {
        wave_a: 1.0,
        wave_r: 1.0,
        wave_g: 1.0,
        wave_b: 1.0,
        // Off by default already, and restated because these three tests turn
        // them on one at a time and a reader should see the baseline.
        ob_a: 0.0,
        ib_a: 0.0,
        mv_a: 0.0,
        ..Default::default()
    }
}

/// **Each of the eight `wave_mode` figures is a different figure** — Phase 4's
/// first done-when.
///
/// Compared as geometry: two modes are distinguishable when the point sets they
/// build differ, and `WAVE_MODES` of them must be pairwise distinct. Asserting
/// *pairwise* rather than "all eight differ from mode 0" is the whole content —
/// the reference's modes 6 and 7 share a construction and modes 0 and 1 do too,
/// so a lazier check passes while two modes render identically.
///
/// The comparison is on the endpoints alone. Colour and width are the same for
/// every mode under one set of outputs, so including them would let a mode pass
/// by being differently *coloured*, which is not what the done-when asks.
#[test]
fn every_wave_mode_builds_a_different_figure() {
    let out = waveform_only();
    let waveform = trace();
    let mut runtime = bare_runtime();

    let mut figures: Vec<(u32, Vec<[f32; 2]>)> = Vec::new();
    for mode in 0..draw::WAVE_MODES {
        let mut geometry = draw::DrawGeometry::default();
        let out = crate::milk::outputs::FrameOutputs {
            wave_mode: mode as f32,
            ..out
        };
        draw::build(
            &mut geometry,
            Some(&mut runtime),
            &out,
            &waveform,
            0.0,
            1.0 / 60.0,
            16.0 / 9.0,
        );
        assert!(
            !geometry.segments.is_empty(),
            "wave_mode {mode} drew nothing at all"
        );
        let points: Vec<[f32; 2]> = geometry.segments.iter().map(|s| s.a).collect();
        figures.push((mode, points));
    }

    for (i, (mode_a, a)) in figures.iter().enumerate() {
        for (mode_b, b) in figures.iter().skip(i + 1) {
            assert_ne!(
                a, b,
                "wave_mode {mode_a} and wave_mode {mode_b} build the same figure; \
                 the eight modes must be distinguishable (Plan 0100 Phase 4)"
            );
        }
    }
}

/// **A mode-6 trace covers `1/aspect` of the frame's width** — Plan 0111 Phase 5,
/// and the arithmetic behind design-backlog 0120's *second* symptom.
///
/// The entry reported two things together: an oversized waveform figure, and
/// *Blur Mix 3*'s crisp trace spanning "roughly the middle 57 %" of the frame
/// where the reference draws full-width traces. **They are separate defects**,
/// and this one is provable without a capture.
///
/// Modes 6 and 7 lay their points on `t = i/(count-1) - 0.5` and divide the x
/// component by `aspect` — but [`draw::uv_to_world`] multiplies x *by* aspect on
/// the way out, so the two cancel exactly and the trace's world-space length is
/// `2t = 2.0` **whatever the target's shape**. The frame is `2 * aspect` wide in
/// those units, so the trace covers `1/aspect` of the width: `1/(16/9) = 0.5625`,
/// the reported 57 %.
///
/// Stated the other way round, which is the useful way: **the trace is normalized
/// to the frame's height, not its width.** On a square target that is full width,
/// which is why nothing caught it at aspect 1 — the same coincidence ADR-0037
/// exists for, one level down.
///
/// It is independent of `wave_scale`, which scales only the amplitude term, so a
/// corrected scale constant cannot fix it and this test keeps passing if one
/// lands. Asserted as a **property over three aspects** rather than as one frozen
/// number, so it states the relationship and not this box's reading of it
/// (ADR-0071).
#[test]
fn a_straight_wave_trace_spans_one_over_aspect_of_the_width() {
    let waveform = trace();
    let mut runtime = bare_runtime();
    for aspect in [1.0f32, 4.0 / 3.0, 16.0 / 9.0] {
        let mut geometry = draw::DrawGeometry::default();
        let out = crate::milk::outputs::FrameOutputs {
            wave_mode: 6.0,
            // `mystery = 0` is the horizontal line: the angle is `mystery * PI`.
            wave_mystery: 0.0,
            // Amplitude out of the way, so what is measured is the `t` term alone.
            wave_scale: 0.0,
            ..waveform_only()
        };
        draw::build(
            &mut geometry,
            Some(&mut runtime),
            &out,
            &waveform,
            0.0,
            1.0 / 60.0,
            aspect,
        );
        // Both endpoints of every segment: `a` alone misses the trace's final
        // vertex and shortens the span by one fencepost.
        let xs: Vec<f32> = geometry
            .segments
            .iter()
            .flat_map(|s| [s.a[0], s.b[0]])
            .collect();
        assert!(!xs.is_empty(), "mode 6 drew nothing at aspect {aspect}");
        let lo = xs.iter().copied().fold(f32::INFINITY, f32::min);
        let hi = xs.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let span = hi - lo;
        // The defect, stated directly: the length does not depend on the target's
        // shape, because the `/aspect` at the point and the `*aspect` in
        // `uv_to_world` cancel.
        assert!(
            (span - 2.0).abs() < 0.02,
            "at aspect {aspect} a horizontal mode-6 trace has world length \
             {span:.4}; it must be 2.0 at every aspect, because that constancy \
             IS design-backlog 0120's second symptom"
        );
        // ...and the consequence, in the units the defect was reported in. The
        // frame is `2 * aspect` wide in world units.
        let fraction = span / (2.0 * aspect);
        let expected = 1.0 / aspect;
        assert!(
            (fraction - expected).abs() < 0.01,
            "at aspect {aspect} the trace covers {fraction:.4} of the frame width \
             where `1/aspect` predicts {expected:.4}; the reference draws these \
             full-width at every aspect"
        );
    }
}

/// **A preset that asks for borders and motion vectors gets both** — Phase 4's
/// third done-when.
///
/// Each is switched on alone against the waveform-only baseline, so the count it
/// adds is its own. Both together are then asserted to add the sum, which is what
/// rules out one producer's geometry being counted twice.
#[test]
fn borders_and_motion_vectors_each_draw_their_own_figure() {
    let waveform = trace();
    let mut runtime = bare_runtime();
    let build = |out: &crate::milk::outputs::FrameOutputs,
                 runtime: &mut crate::milk::MilkRuntime|
     -> usize {
        let mut geometry = draw::DrawGeometry::default();
        draw::build(
            &mut geometry,
            Some(runtime),
            out,
            &waveform,
            0.0,
            1.0 / 60.0,
            16.0 / 9.0,
        );
        geometry.segments.len()
    };

    let base = waveform_only();
    let n_base = build(&base, &mut runtime);

    // Two borders, both visible. Four segments each — a closed rectangle.
    let bordered = crate::milk::outputs::FrameOutputs {
        ob_a: 1.0,
        ob_size: 0.02,
        ib_a: 1.0,
        ib_size: 0.02,
        ..base
    };
    let n_borders = build(&bordered, &mut runtime);
    assert_eq!(
        n_borders - n_base,
        8,
        "two visible borders are two closed rectangles, so eight segments"
    );

    // A 12x9 motion grid, one stroke per cell.
    let vectors = crate::milk::outputs::FrameOutputs {
        mv_a: 1.0,
        mv_x: 12.0,
        mv_y: 9.0,
        mv_l: 1.0,
        ..base
    };
    let n_vectors = build(&vectors, &mut runtime);
    assert_eq!(
        n_vectors - n_base,
        12 * 9,
        "a 12x9 motion grid is one stroke per cell"
    );

    // ...and together, each contributing exactly what it did alone.
    let both = crate::milk::outputs::FrameOutputs {
        ob_a: 1.0,
        ob_size: 0.02,
        ib_a: 1.0,
        ib_size: 0.02,
        mv_a: 1.0,
        mv_x: 12.0,
        mv_y: 9.0,
        mv_l: 1.0,
        ..base
    };
    assert_eq!(
        build(&both, &mut runtime) - n_base,
        8 + 12 * 9,
        "borders and motion vectors must both show, and neither may swallow the other"
    );
}

/// **The blend partition is what the two-pipeline draw reads**, so it has to hold
/// exactly: every entry below the split is additive, every entry above it is not,
/// and a triangle never straddles.
///
/// This is the invariant `WarpMeshScene::render` and
/// [`LineRenderer::draw_split`](crate::render::scenes::lines::LineRenderer::draw_split)
/// both assume without being able to check. Getting it wrong does not fail — it
/// blends some producers with the wrong pipeline, which reads as a preset that is
/// too bright rather than as a bug.
#[test]
fn the_blend_partition_separates_the_two_seams() {
    let waveform = trace();
    let mut runtime = bare_runtime();

    // `wave_additive` off puts the waveform in the OVER half; the borders and the
    // motion grid are always there. So nothing is additive and the split is 0.
    let over = crate::milk::outputs::FrameOutputs {
        ob_a: 1.0,
        ob_size: 0.02,
        mv_a: 1.0,
        mv_x: 4.0,
        mv_y: 3.0,
        wave_additive: 0.0,
        ..waveform_only()
    };
    let mut geometry = draw::DrawGeometry::default();
    draw::build(
        &mut geometry,
        Some(&mut runtime),
        &over,
        &waveform,
        0.0,
        1.0 / 60.0,
        16.0 / 9.0,
    );
    assert_eq!(
        geometry.segments_additive, 0,
        "no producer asked to add, so the additive half is empty"
    );
    assert!(!geometry.segments.is_empty());

    // Turn the waveform additive: exactly its own segments move below the split,
    // and the borders and grid stay above it.
    let mixed = crate::milk::outputs::FrameOutputs {
        wave_additive: 1.0,
        ..over
    };
    let mut mixed_geometry = draw::DrawGeometry::default();
    draw::build(
        &mut mixed_geometry,
        Some(&mut runtime),
        &mixed,
        &waveform,
        0.0,
        1.0 / 60.0,
        16.0 / 9.0,
    );
    let border_and_grid = 4 + 4 * 3;
    assert_eq!(
        mixed_geometry.segments.len() - mixed_geometry.segments_additive,
        border_and_grid,
        "the borders and the motion grid always blend OVER, whatever the waveform does"
    );
    assert_eq!(
        mixed_geometry.segments_additive,
        mixed_geometry.segments.len() - border_and_grid,
        "an additive waveform puts all of its own segments below the split"
    );

    // The additive half's coverage is ADR-0056's footprint rule (always 1.0);
    // the OVER half's is the producer's real alpha, which must be in range or the
    // premultiplied blend's `1 - src.a` goes negative.
    for (i, segment) in mixed_geometry.segments.iter().enumerate() {
        if i < mixed_geometry.segments_additive {
            assert_eq!(
                segment.alpha, 1.0,
                "segment {i} is additive, so its coverage is its whole footprint"
            );
        } else {
            assert!(
                (0.0..=1.0).contains(&segment.alpha),
                "segment {i} blends OVER with alpha {} outside 0..=1",
                segment.alpha
            );
        }
    }
    assert_eq!(
        mixed_geometry.triangles_additive % 3,
        0,
        "a triangle's three vertices share one blend mode"
    );
}

/// **A frame-rate-independent alpha.** At MilkDrop's own nominal cadence
/// ([`NOMINAL_FPS`](crate::milk::NOMINAL_FPS), 30 fps — the rate a `.milk`
/// author tuned by eye against) the effective alpha is the one the preset asked
/// for; at half that cadence one frame travels as far as two would. That is
/// ADR-0019 applied to a blend rather than to a rate.
///
/// The property, stated as the composition it comes from: applying the slow
/// frame's alpha once leaves the same fraction of the way travelled as applying
/// the nominal one's twice.
#[test]
fn the_over_blend_alpha_is_frame_rate_independent() {
    let waveform = trace();
    let mut runtime = bare_runtime();
    let alpha_at = |dt: f32, runtime: &mut crate::milk::MilkRuntime| -> f32 {
        let out = crate::milk::outputs::FrameOutputs {
            wave_a: 0.25,
            wave_additive: 0.0,
            ..waveform_only()
        };
        let mut geometry = draw::DrawGeometry::default();
        draw::build(
            &mut geometry,
            Some(runtime),
            &out,
            &waveform,
            0.0,
            dt,
            16.0 / 9.0,
        );
        geometry.segments.first().map(|s| s.alpha).unwrap_or(0.0)
    };

    let nominal = 1.0 / crate::milk::NOMINAL_FPS;
    let at_nominal = alpha_at(nominal, &mut runtime);
    let at_half_rate = alpha_at(nominal * 2.0, &mut runtime);
    assert!(
        (at_nominal - 0.25).abs() < 1e-5,
        "at the nominal cadence the effective alpha is the preset's own, got {at_nominal}"
    );
    // Two nominal frames leave `1 - (1-a)^2` of the way travelled; one frame of
    // twice the length must leave the same.
    let two_frames = 1.0 - (1.0 - at_nominal) * (1.0 - at_nominal);
    assert!(
        (at_half_rate - two_frames).abs() < 1e-5,
        "one double-length frame ({at_half_rate}) must travel as far as two \
         nominal ones ({two_frames})"
    );
}

/// **The ladder [`TierConfig::mesh_grid`] is set from** — a measurement, not a
/// gate (ADR-0071: a cost in milliseconds is a property of one machine at one
/// moment, so it is recorded with the machine named and never asserted).
///
/// `#[ignore]`d and run explicitly:
///
/// ```text
/// cargo test -p lmv-core --release --lib mesh_cost_by_grid -- --ignored --nocapture
/// ```
///
/// It times exactly what the capacity bounds: one frame of per-vertex evaluation
/// for a four-binding `[per_vertex]` program of the shape a real preset writes,
/// over the whole grid, on the render thread. Nothing GPU-side is in it — the
/// mesh draw itself is a few thousand triangles and is not the cost this
/// capacity is about.
///
/// The numbers it printed on 2026-08-16, and what they decided, are recorded on
/// [`TierConfig::mesh_grid`] rather than here, because that is where someone
/// reading the constant will look.
#[test]
#[ignore = "a timing measurement, not a gate — see TierConfig::mesh_grid"]
#[allow(
    clippy::disallowed_methods,
    reason = "a capacity measurement deliberately times execution; the per-vertex \
              evaluation under test stays clock-free"
)]
fn mesh_cost_by_grid() {
    use crate::preset::expr::{self, Variables};

    // A program of the shape a real preset writes: every output a function of
    // the vertex's own position and the audio, with a transcendental in it.
    let sources = [
        "1 + rad * 0.12 + bass * 0.08",
        "sin(ang * 3 + time * 0.4) * 0.5 + mid * 0.2",
        "0.5 + sin(time * 0.2) * 0.1 * x",
        "clamp(treb * 0.3, 0, 0.25) * cos(rad * 6)",
    ];
    let programs: Vec<_> = sources
        .iter()
        .map(|src| expr::compile(src).expect("the measurement program compiles"))
        .collect();
    let vars = Variables::new(0.6, 0.4, 0.3, 0.2, 1.0, 0.5, 12.0, 128.0, 0.1);

    println!(" grid      vertices    per frame");
    // Finer than a doubling ladder on purpose: the 1 ms bar falls between 64x48
    // and 96x72, and the tier values are read off where it actually lands rather
    // than off the nearest power of two.
    for mesh in [
        (16u32, 12u32),
        (32, 24),
        (48, 36),
        (64, 48),
        (72, 54),
        (80, 60),
        (88, 66),
        (96, 72),
        (112, 84),
        (128, 96),
    ] {
        let n = vertex_count(mesh);
        let mut sink = 0.0f32;
        // Enough repeats that the clock resolution is not the measurement.
        let repeats = 200;
        let start = std::time::Instant::now();
        for _ in 0..repeats {
            for row in 0..=mesh.1 {
                for col in 0..=mesh.0 {
                    let (x, y, rad, ang) = vertex_position(col, row, mesh, 16.0 / 9.0);
                    let v = vars.with_vertex(x, y, rad, ang);
                    for program in &programs {
                        sink += program.eval(&v);
                    }
                }
            }
        }
        let per_frame = start.elapsed().as_secs_f64() * 1000.0 / f64::from(repeats);
        println!(
            "{:>4}x{:<4} {n:>8}    {per_frame:>7.3} ms   (sink {sink:.1})",
            mesh.0, mesh.1
        );
    }
    println!(
        "the budget is 1 ms — 6 % of the 16.67 ms NFR §1 commits to at 1080p; \
         FLOOR is {:?}, RICH is {:?}",
        TierConfig::FLOOR.mesh_grid,
        TierConfig::RICH.mesh_grid
    );
}

/// **The echo orientation is four states, and a continuous value picks one** —
/// Plan 0109 Phase 3.
///
/// The source format stores an integer, but the value reaches the scene as an
/// `f32` that a per-frame program computes and that a preset's own `[smoothing]`
/// can sweep through the gaps between states. Quantizing here rather than in the
/// shader means one decision instead of one per flip, and it is the same rule
/// the kaleidoscope's seam took: an eased param is continuous even when its
/// meaning is not.
#[test]
fn the_echo_orientation_quantizes_to_four_states() {
    // The four states, and the halves that round into them.
    for (input, want) in [
        (0.0, 0),
        (0.4, 0),
        (1.0, 1),
        (0.6, 1),
        (1.49, 1),
        (2.0, 2),
        (3.0, 3),
    ] {
        assert_eq!(
            echo_orientation(input),
            want,
            "{input} must read as orientation {want}"
        );
    }
    // Out of range wraps, so counting animates rather than sticking.
    assert_eq!(echo_orientation(4.0), 0, "4 is 0 again");
    assert_eq!(echo_orientation(5.0), 1);
    assert_eq!(echo_orientation(-1.0), 3, "counting down wraps too");
    // Total: a non-finite orientation loses the flip, never the echo.
    assert_eq!(echo_orientation(f32::NAN), 0);
    assert_eq!(echo_orientation(f32::INFINITY), 0);
}

// ---------------------------------------------------------------------------
// The field instrument (Plan 0109 Phase 4)
// ---------------------------------------------------------------------------
//
// **Every observation of the wash before this was of the final picture**, where
// `gamma`, `brightness`, the four remaps, the post chain and the tonemap all sit
// downstream of the feedback field and any of them could be the whole story.
// Two plans failed to find the cause with that instrument. What follows reads
// the `Rgba16Float` field itself, frame by frame, in its own linear units:
// `PingPongField::read_texture` is copied to a readback buffer after each frame
// and decoded with `capture::read_back_linear`, which does not clamp at 1.
//
// It is a GPU test, so it skips where there is no adapter (ADR-0016).
//
// # What it found, including what it ruled out
//
// **The built-in warp path is clean.** With the quantizer on the field converges
// and its background sits at black; with it off the field integrates without
// bound. Both are asserted below. ADR-0118's mechanism is therefore confirmed at
// the field rather than inferred from a picture, which is what it was.
//
// **Dead hypothesis: the decay multiply's domain.** This engine multiplies the
// field by `decay` in LINEAR light; the reference multiplies its 8-bit target in
// the GAMMA-ENCODED domain, and a factor `a` encoded is `a^2.2` linear, so the
// arithmetic predicts our trails outliving the reference's by roughly that much.
// Measured, they do not: with the quantizer on, an undeposited field fading from
// frame 40 to frame 200 reads an encoded-domain ratio of **0.2303** against the
// reference's arithmetic **0.1986** — 16 % slower, not the 2.4x the domain
// difference alone predicts (0.4797). **The truncation absorbs most of the
// domain error**, and this is not the wash. That is the third hypothesis this
// defect has killed, after frame-rate accumulation and `bAdditiveWaves`.
//
// **The live hypothesis, and it is not in this path.** A converted warp shader
// applies `decay` only if the preset's own HLSL names it: `emit.rs` exposes
// `decay` as a readable `U.misc.x` and nothing multiplies by it, so a preset
// whose shader merely samples never fades at all — its field is bounded by the
// epilogue's clamp and the quantizer's floor, which is a saturated plateau
// rather than a fade. **Of the corpus's 8 162 files carrying a warp shader,
// only 1 253 name `decay` in it** — so 6 909 presets, two thirds of the whole
// corpus, land on that path. It predicts the look gate's own pattern: the five
// washed presets are shader presets, and the one clean control, *Blur Mix 3*, is
// the one whose blur chain darkens by a second mechanism.
//
// Whether the engine should apply `decay` there — and what to do about the 15 %
// that would then apply it twice — is a design call about matching the
// reference, not an implementation detail. It needs the reference on screen,
// which is Phase 5, and an ADR. Phase 4 stops here.

/// Field-probe capture size. Small: the recursion under test is per-texel, so
/// resolution buys nothing but readback time.
const FIELD_SIZE: u32 = 64;

/// The exponent between this engine's linear field and the reference's
/// gamma-encoded 8-bit one. sRGB's transfer function is a piecewise curve, but
/// the domain question below is about a *ratio over a decade*, where the
/// straight `2.2` power it approximates is what the arithmetic is worth stating
/// to. Naming it as a constant keeps the two predictions in
/// [`the_decay_domain_is_not_the_wash`] derived from one number rather than two
/// literals that could drift apart.
const ENCODE_GAMMA: f64 = 2.2;

/// The frame step every probe below runs at. A constant rather than a literal
/// because [`the_decay_domain_is_not_the_wash`]'s arithmetic is in *seconds*,
/// and a step that drifted from the one the frames were rendered with would
/// falsify the comparison silently.
const FIELD_DT: f32 = 1.0 / 60.0;

/// One run of [`field_trace`]: what the field did, and the decay it did it with.
///
/// The decay is carried out rather than assumed because **the scene does not
/// keep the one a caller sets**: a bundle's per-frame outputs overwrite the
/// whole composite roster on every `update`, and `decay` is a roster member (the
/// deposit is not, which is why *that* one sticks). Any arithmetic about the
/// fade has to be built on the value the frames were actually rendered with, so
/// the probe reads it back off the scene instead of restating it.
struct FieldTrace {
    /// One entry per frame, in order.
    levels: Vec<FieldLevel>,
    /// The per-second `decay` in force on the last frame rendered.
    decay: f32,
}

/// One frame's reading of the field.
#[derive(Debug, Clone, Copy, Default)]
struct FieldLevel {
    /// Mean of the three colour channels over every texel.
    mean: f32,
    /// The brightest single channel anywhere in the field.
    peak: f32,
    /// Mean over the outermost ring of texels — the **background**, where a
    /// centred deposit puts nothing and only the warp's resampling reaches.
    edge: f32,
}

/// Drive a `warp_mesh` scene for `frames` frames and read the field back after
/// each one.
///
/// The scene is driven directly rather than through `Renderer`, which is what
/// makes the field reachable at all — and it keeps the probe honest in a second
/// way: no preset loader, no post chain, no backdrop. What the numbers describe
/// is the warp pass, the deposit pass and the draw layer, and nothing else.
fn field_trace(
    quantize_steps: f32,
    params: &[(&str, f32)],
    frames: usize,
    deposit_off_at: usize,
    decay_per_second: Option<f32>,
) -> Option<FieldTrace> {
    use crate::render::capture;
    use crate::render::context::{RenderContext, RenderError};

    let ctx = match RenderContext::new_headless(FIELD_SIZE, FIELD_SIZE, true) {
        Ok(ctx) => ctx,
        Err(RenderError::RequestAdapter(_)) => {
            eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
            return None;
        }
        Err(e) => panic!("headless context build failed: {e}"),
    };

    // A bundle carries the quantizer's step count, so an empty one is how the
    // probe reaches ADR-0118's switch from a test. Its programs are empty: the
    // field is driven by the scene's own params below, which is what makes the
    // recursion a scalar with an arithmetic answer to compare against.
    let mut bundle = crate::milk::MilkBundle::from_assembly(None, None, None)
        .expect("an empty bundle assembles");
    bundle.quantize_steps = quantize_steps;

    // **`COMPOSITE_FORMAT`, not the surface format** — that is what the renderer
    // hands every scene (`scenes::create_all`), because a scene draws into the
    // composite chain's linear target rather than onto the swapchain.
    let mut scene = WarpMeshScene::new(
        &ctx.device,
        crate::render::COMPOSITE_FORMAT,
        TierConfig::FLOOR.mesh_grid,
        TierConfig::FLOOR.max_segments,
    );
    scene.configure(&super::super::GeneratorConfig::WarpMesh {
        mesh: (32, 24),
        milk: Some(Box::new(bundle)),
        salt: 0,
    });
    for (name, value) in params {
        assert!(
            PARAMS.contains(name),
            "the probe set an unknown param `{name}`"
        );
        scene.set_param(name, *value);
    }

    let (target, view) = capture::create_target(
        &ctx.device,
        crate::render::COMPOSITE_FORMAT,
        FIELD_SIZE,
        FIELD_SIZE,
    );
    let _ = &target;
    let (readback, padded_bpr) =
        capture::create_linear_readback(&ctx.device, FIELD_SIZE, FIELD_SIZE);

    let frame = AnalysisFrame::default();
    let dt = FIELD_DT;
    let mut out = Vec::with_capacity(frames);
    for i in 0..frames {
        if i == deposit_off_at {
            // The decay half of the experiment: stop feeding the field and watch
            // what it does with what it has. A bundle's per-frame outputs
            // overwrite the composite roster every frame but not `deposit`,
            // which is a native param, so this sticks.
            scene.set_param("deposit", 0.0);
        }
        scene.set_target_size(FIELD_SIZE, FIELD_SIZE);
        scene.set_time(i as f32 * dt);
        scene.advance(dt);
        scene.update(&frame);
        // **After `update`, not with the other params.** `update` has just
        // overwritten the entire composite roster from the bundle's per-frame
        // outputs, and `decay` is a roster member — a value set before the loop
        // would be gone by the first frame. (`deposit` is a native param, not a
        // roster member, which is why stopping it above sticks.)
        //
        // An empty bundle leaves `decay` at `FrameOutputs`' fallback, which is
        // MilkDrop's `0.98` read as **per second** rather than per frame — a
        // near-unity factor, and not a fade any experiment about a fade can use.
        if let Some(d) = decay_per_second {
            scene.set_param("decay", d);
        }
        let mut encoder = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("field-probe"),
            });
        capture::record_clear(&mut encoder, &view);
        scene.render(&ctx.queue, &mut encoder, &view, 1.0);
        let field = scene
            .res
            .as_ref()
            .expect("the first render builds the resources")
            .field
            .read_texture()
            .clone();
        // `record_copy` is format-agnostic — it honours whatever stride it is
        // handed, and `create_linear_readback` sized this one for four halves.
        capture::record_copy(
            &mut encoder,
            &field,
            &readback,
            padded_bpr,
            FIELD_SIZE,
            FIELD_SIZE,
        );
        ctx.queue.submit(std::iter::once(encoder.finish()));

        let texels =
            capture::read_back_linear(&ctx.device, &readback, FIELD_SIZE, FIELD_SIZE, padded_bpr)
                .expect("the field reads back");
        let mut sum = 0f64;
        let mut n = 0u64;
        let mut peak = 0f32;
        let mut edge_sum = 0f64;
        let mut edge_n = 0u64;
        for (i, rgba) in texels.chunks_exact(4).enumerate() {
            let (x, y) = (i as u32 % FIELD_SIZE, i as u32 / FIELD_SIZE);
            let border = x < 2 || y < 2 || x >= FIELD_SIZE - 2 || y >= FIELD_SIZE - 2;
            for c in rgba.iter().take(3) {
                sum += f64::from(*c);
                n += 1;
                peak = peak.max(*c);
                if border {
                    edge_sum += f64::from(*c);
                    edge_n += 1;
                }
            }
        }
        out.push(FieldLevel {
            mean: if n == 0 { 0.0 } else { (sum / n as f64) as f32 },
            peak,
            edge: if edge_n == 0 {
                0.0
            } else {
                (edge_sum / edge_n as f64) as f32
            },
        });
    }
    Some(FieldTrace {
        levels: out,
        decay: scene.decay,
    })
}

/// A still field with a centred deposit: the recursion is per-texel, so what it
/// does has an arithmetic answer to compare against.
/// The decay the two quantizer probes below run at: near-unity, so **the
/// quantizer is the only thing bounding the field** and the experiment isolates
/// it. Over their 300 frames (5 s at the probe's `dt`) this fades to `0.98^5 =
/// 0.904` — present, and negligible against what the deposit adds.
///
/// **It is stated here rather than defaulted.** A probe passing `None` got a
/// near-unity factor for free while `FrameSlots::read` returned the `decay`
/// default *unconverted* — MilkDrop's per-frame `0.98` landing in a field that
/// means per-second. Plan 0111 Phase 1 fixed that (design-backlog 0121), so
/// `None` now yields the converted `0.5455`/s, which is a real fade and a
/// different experiment: under it the unquantized field settles by frame 450
/// rather than climbing. `0.98` reproduces what these two always measured, and
/// naming it means the next fix to the fallback cannot silently re-aim them.
const NEUTRALIZED_DECAY: Option<f32> = Some(0.98);

fn still_field_params() -> Vec<(&'static str, f32)> {
    vec![
        ("zoom", 1.0),
        ("sx", 1.0),
        ("sy", 1.0),
        ("rot", 0.0),
        ("dx", 0.0),
        ("dy", 0.0),
        ("warp", 0.0),
        ("deposit", 1.6),
        ("deposit_radius", 0.0),
        ("deposit_width", 0.2),
    ]
}

/// **The field is bounded by the quantizer and by nothing else** — ADR-0118's
/// mechanism, measured in the field rather than inferred from a picture.
///
/// Plan 0108 diagnosed the wash from seven side-by-side captures: an
/// `Rgba16Float` field keeps every dim residual, so it integrates where the
/// reference's 8-bit target truncates. That reasoning was right and it was never
/// *observed* — the evidence was always the final picture, where `gamma`,
/// `brightness`, the four remaps and the whole post chain sit downstream. This
/// reads the field itself.
///
/// Measured 2026-08-19 on WARP at 64x64, 300 frames, deposit at the centre, no
/// warp motion:
///
/// ```text
///   steps    frame  mean      peak
///   0 (off)     30  0.1011
///   0 (off)    120  0.3690
///   0 (off)    300  0.8331    6.680   <- still climbing, no equilibrium
///   255 (on)    30  0.0851
///   255 (on)   120  0.2039
///   255 (on)   300  0.2081    1.017   <- converged by ~120
/// ```
#[test]
fn the_field_equilibrates_only_when_the_quantizer_runs() {
    let params = still_field_params();
    let Some(off) = field_trace(0.0, &params, 300, usize::MAX, NEUTRALIZED_DECAY) else {
        return;
    };
    let on =
        field_trace(255.0, &params, 300, usize::MAX, NEUTRALIZED_DECAY).expect("the second runs");
    let (off, on) = (off.levels, on.levels);
    let read = |t: &[FieldLevel], n: usize| t.get(n).copied().unwrap_or_default();
    println!(
        "[field] quantizer OFF  f30 {:.4} f120 {:.4} f300 {:.4} (peak {:.3})",
        read(&off, 29).mean,
        read(&off, 119).mean,
        read(&off, 299).mean,
        read(&off, 299).peak
    );
    println!(
        "[field] quantizer ON   f30 {:.4} f120 {:.4} f300 {:.4} (peak {:.3})",
        read(&on, 29).mean,
        read(&on, 119).mean,
        read(&on, 299).mean,
        read(&on, 299).peak
    );

    // Off: no equilibrium. The last stretch still climbs measurably, which is
    // the integrator ADR-0118 named.
    assert!(
        read(&off, 299).mean > read(&off, 119).mean * 1.5,
        "with the quantizer off the field must still be climbing at frame 300 — \
         it read {:.4} against frame 120's {:.4}",
        read(&off, 299).mean,
        read(&off, 119).mean
    );
    // On: converged. The last 180 frames move it by under a percent.
    let (mid, end) = (read(&on, 119).mean, read(&on, 299).mean);
    assert!(
        (end - mid).abs() < mid * 0.05,
        "with the quantizer on the field must have converged by frame 120 — it \
         read {mid:.4} there and {end:.4} at frame 300"
    );
    assert!(
        end < read(&off, 299).mean * 0.5,
        "the quantized field must settle far below the unquantized one"
    );
}

/// **The background goes black, and stays black, once the quantizer runs** —
/// Plan 0109 Phase 4's redirection.
///
/// The phase was written to find why "the background equilibrates far brighter
/// than the reference's" and told to look in the field rather than in the
/// composite. It looked. **With the quantizer on, this field's background is
/// not bright: it sits at `1e-6` linear and does not move**, while the same
/// fixture with the quantizer off climbs monotonically. So for the built-in warp
/// path the field is not where the wash lives, and the next hunt belongs
/// downstream — or, far more likely, in the *other* warp path. See the module
/// note below.
///
/// Measured on WARP at 64x64 with a zooming warp dragging a centred deposit
/// outward, which is how a MilkDrop tunnel fills its background — edge is the
/// mean over the outer two-texel ring:
///
/// ```text
///   steps    frame  mean      edge
///   0 (off)     30  0.0409    0.000037
///   0 (off)    120  0.1396    0.000140
///   0 (off)    300  0.3104    0.000319   <- climbing
///   255 (on)    30  0.0345    0.000001
///   255 (on)   120  0.0772    0.000001
///   255 (on)   300  0.0787    0.000001   <- pinned at black
/// ```
///
/// (Measured 2026-08-19 on the development box.)
#[test]
fn the_quantized_background_stays_black() {
    let mut params = still_field_params();
    // A zoom drags the deposit outward; `wrap` off so the edge is fed only by
    // what the warp actually carries there.
    for (name, value) in &mut params {
        if *name == "zoom" {
            *value = 1.6;
        }
        if *name == "deposit_width" {
            *value = 0.12;
        }
    }
    let Some(on) = field_trace(255.0, &params, 300, usize::MAX, NEUTRALIZED_DECAY) else {
        return;
    };
    let off =
        field_trace(0.0, &params, 300, usize::MAX, NEUTRALIZED_DECAY).expect("the second runs");
    let (on, off) = (on.levels, off.levels);
    let read = |t: &[FieldLevel], n: usize| t.get(n).copied().unwrap_or_default();
    println!(
        "[field] edge ON  f30 {:.6} f120 {:.6} f300 {:.6}",
        read(&on, 29).edge,
        read(&on, 119).edge,
        read(&on, 299).edge
    );
    println!(
        "[field] edge OFF f30 {:.6} f120 {:.6} f300 {:.6}",
        read(&off, 29).edge,
        read(&off, 119).edge,
        read(&off, 299).edge
    );

    assert!(
        read(&on, 299).edge < 1e-4,
        "the quantized field's background must stay black; it read {:.6}",
        read(&on, 299).edge
    );
    assert!(
        read(&off, 299).edge > read(&off, 29).edge * 4.0,
        "the unquantized control must show the background filling — it read \
         {:.6} at frame 300 against {:.6} at frame 30, and if it does not, this \
         fixture no longer drags light outward and the assertion above is \
         measuring nothing",
        read(&off, 299).edge,
        read(&off, 29).edge
    );
}

/// **The decay multiply's domain is not the wash** — Plan 0109 Phase 4's third
/// dead hypothesis, measured rather than argued.
///
/// # The hypothesis
///
/// This engine multiplies the field by `decay` in **linear** light; the
/// reference multiplies its 8-bit target in the **gamma-encoded** domain, and a
/// factor `a` encoded is `a^2.2` linear. The same nominal decay should therefore
/// leave our trails visibly outliving the reference's, and a trail that outlives
/// the reference's is what a wash looks like.
///
/// # Why this is a property rather than a frozen number
///
/// Both predictions are derived from the decay the frames were **actually**
/// rendered with — read back off the scene by [`FieldTrace`], not assumed — so
/// the claim holds at any decay, any frame step and any adapter:
///
/// - the **reference's** fade, stated in the reference's own encoded domain:
///   `d^T`
/// - the **pure-linear** prediction, which is our multiply expressed in that
///   same domain so the two are comparable: `d^(T/2.2)`
///
/// The measured fade lands far closer to the first than to the second. The
/// quantizer's truncation absorbs most of the domain error, so the domain
/// difference is real arithmetic that does **not** reach the picture — and it is
/// not what makes converted presets wash out. That is the third hypothesis this
/// defect has killed, after frame-rate accumulation and `bAdditiveWaves`.
///
/// **What it does not touch** is the live hypothesis, which is in the other warp
/// path entirely — see the module note above.
///
/// # What it read
///
/// Measured 2026-08-19 on the development box (Windows 10, DX12), 64x64, decay
/// `0.5455`/s over 2.650 s of undeposited field:
///
/// ```text
///   measured                 0.3034
///   reference arithmetic     0.2007   <- 0.103 away
///   pure-linear prediction   0.4819   <- 0.179 away
/// ```
///
/// Those are an observation and not the assertion — nothing above is asserted
/// against them. Plan 0109 Phase 4 read `0.2303` for the same quantity on a
/// window it did not commit, which is nearer the reference still; both readings
/// say the same thing about the hypothesis and neither is load-bearing.
#[test]
fn the_decay_domain_is_not_the_wash() {
    /// The frame the deposit stops on, so everything after it is the field
    /// living on what it already has.
    const DEPOSIT_OFF_AT: usize = 40;
    const FRAMES: usize = 200;

    // MilkDrop's own default `fDecay`, converted the way the converter converts
    // it — `0.98` per frame at the nominal rate. Written as the conversion
    // rather than as `0.5455` so it cannot drift from `NOMINAL_FPS`, and held
    // for every frame because an empty bundle's fallback is a near-unity factor
    // that would leave nothing to measure.
    let decay = 0.98f32.powf(crate::milk::NOMINAL_FPS);
    let Some(trace) = field_trace(
        255.0,
        &still_field_params(),
        FRAMES,
        DEPOSIT_OFF_AT,
        Some(decay),
    ) else {
        return;
    };
    let read = |n: usize| trace.levels.get(n).copied().unwrap_or_default();
    let (first, last) = (read(DEPOSIT_OFF_AT).mean, read(FRAMES - 1).mean);

    // `render` clamps before it converts, so this is the factor the frames were
    // rendered with rather than the one the scene was handed.
    let d = f64::from(trace.decay.clamp(0.0, MAX_DECAY));
    let elapsed = f64::from((FRAMES - 1 - DEPOSIT_OFF_AT) as f32 * FIELD_DT);

    // The fade this engine produced, moved into the reference's domain so the
    // three numbers below are all the same kind of quantity.
    let measured = (f64::from(last) / f64::from(first)).powf(1.0 / ENCODE_GAMMA);
    let reference = d.powf(elapsed);
    let linear_domain = d.powf(elapsed / ENCODE_GAMMA);

    println!(
        "[field] decay {d:.4}/s over {elapsed:.3} s — measured {measured:.4}, \
         reference arithmetic {reference:.4}, pure-linear prediction \
         {linear_domain:.4} (nearer the reference by {:.1}x)",
        (measured - linear_domain).abs() / (measured - reference).abs().max(f64::EPSILON)
    );

    // Non-vacuity, in three parts. The field must have faded at all, or the
    // ratio is a reading of a constant.
    assert!(
        first > 0.0 && last < first * 0.9,
        "the field did not fade between frames {DEPOSIT_OFF_AT} and {FRAMES} \
         ({first:.4} -> {last:.4}), so nothing below measures a decay. Check \
         that the deposit actually stopped"
    );
    // And the two predictions must be far enough apart to tell apart, which is
    // the whole hypothesis: if the domain difference did not predict a large
    // divergence, there would be nothing to rule out.
    assert!(
        linear_domain > reference * 1.5,
        "the domain hypothesis predicts a LARGE divergence and here it predicts \
         almost none ({linear_domain:.4} against {reference:.4}), so this test \
         rules nothing out. The decay in force was {d:.4}/s over {elapsed:.3} s \
         — too little fade in the window to separate the two predictions"
    );

    // The finding. Stated as a comparison of two distances rather than as a
    // threshold: the measured fade is nearer the reference's arithmetic than
    // the pure-linear prediction, which is what "the truncation absorbs the
    // domain error" means.
    let to_reference = (measured - reference).abs();
    let to_linear = (measured - linear_domain).abs();
    assert!(
        to_reference < to_linear,
        "the measured fade {measured:.4} is nearer the PURE-LINEAR prediction \
         {linear_domain:.4} than the reference's {reference:.4}, so the decay \
         multiply's domain IS reaching the picture after all and this \
         hypothesis is alive again — Plan 0109 Phase 4 recorded it as dead"
    );
}

/// ADR-0132's correction, and the whole of its evidence: no shipped preset binds
/// `warp_speed`, so there is nothing here to regress and the constant-rate
/// equivalence is the only claim with anything behind it.
///
/// At a constant rate the integrated phase equals `rate * time`, which is why
/// `DEFAULT_WARP_SPEED = 1.0` renders exactly what `time * wspeed` rendered.
#[test]
fn a_constant_warp_speed_integrates_to_the_multiply_it_replaced() {
    let dt = super::super::FALLBACK_DT;
    for rate in [1.0f32, 0.25, 3.0] {
        let mut phase = 0.0f32;
        let mut time = 0.0f32;
        for _ in 0..600 {
            phase = integrate_phase(phase, rate, dt);
            time += dt;
        }
        assert!(
            (phase - rate * time).abs() < 1e-3,
            "rate {rate}: integrated {phase} against the multiply's {}",
            rate * time
        );
    }
}

/// ...and the property the multiply failed: a rate that MOVES advances the phase
/// by `rate * dt` whatever the elapsed time. Under `time * wspeed`, a swing from
/// `1.0` to `1.5` at t = 100 s moved the phase by fifty seconds in one frame —
/// the picture jumped rather than quickening, which is what an audio-bound rate
/// does on every beat.
#[test]
fn a_warp_speed_change_bends_the_phase_instead_of_teleporting_it() {
    let dt = super::super::FALLBACK_DT;
    let mut phase = 0.0f32;
    let mut time = 0.0f32;
    for _ in 0..6_000 {
        phase = integrate_phase(phase, 1.0, dt);
        time += dt;
    }
    assert!(time > 99.0, "the fixture must be far from t = 0: {time}");

    let before = phase;
    phase = integrate_phase(phase, 1.5, dt);
    let step = phase - before;
    assert!(
        (step - 1.5 * dt).abs() < 1e-4,
        "the phase advanced {step}, not {}",
        1.5 * dt
    );
    // What the multiply would have done, stated so the size of the defect is on
    // the record rather than only described.
    let teleport = (1.5 * time) - (1.0 * time);
    assert!(
        teleport > 40.0,
        "the multiply's one-frame jump at this elapsed time was {teleport} s"
    );
}
