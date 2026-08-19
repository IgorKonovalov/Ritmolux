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
/// gate ([ADR-0071](../../../../docs/adrs/0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md):
/// a cost in milliseconds is a property of one machine at one moment, so it is
/// recorded with the machine named and never asserted).
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
// which is Phase 5, and an ADR. Phase 4 stops here, which is the branch the plan
// wrote for it.

/// Field-probe capture size. Small: the recursion under test is per-texel, so
/// resolution buys nothing but readback time.
const FIELD_SIZE: u32 = 64;

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
) -> Option<Vec<FieldLevel>> {
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
    let dt = 1.0 / 60.0;
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
    Some(out)
}

/// A still field with a centred deposit: the recursion is per-texel, so what it
/// does has an arithmetic answer to compare against.
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
    let Some(off) = field_trace(0.0, &params, 300, usize::MAX) else {
        return;
    };
    let on = field_trace(255.0, &params, 300, usize::MAX).expect("the second trace runs too");
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
    let Some(on) = field_trace(255.0, &params, 300, usize::MAX) else {
        return;
    };
    let off = field_trace(0.0, &params, 300, usize::MAX).expect("the second trace runs too");
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
