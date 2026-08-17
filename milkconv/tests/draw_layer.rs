//! **A custom shape honours its per-point program** (Plan 0100 Phase 4's second
//! done-when), end to end across the ADR-0113 seam.
//!
//! # Why this file is here and not in `core`
//!
//! Same reason as `conformance.rs`: the question is what a *program* means, and
//! only this crate can turn EEL2 into the bytecode `core` runs. A custom shape's
//! whole contract is that the numbers its per-frame code computes — position,
//! radius, colour, alpha, side count, blend mode — are the numbers the draw layer
//! builds geometry from. Asserted from `core` alone it would need hand-written
//! bytecode, and a test whose fixture is hand-assembled pins the assembler rather
//! than the semantics.
//!
//! # What it walks
//!
//! A `.milk` file's text, through `milk::parse` and `convert::convert`, through
//! the emitted bundle's own TOML, back through `Preset::from_toml_str`, into
//! `MilkRuntime`, into `warp_mesh::draw::build`, out as triangles. **Every stage
//! Phase 4 added is on that path** — including the `[[milk.shapes]]` round trip,
//! which is the stage that was missing entirely until this test's sibling
//! assertions went looking for it.

use lmv_core::dsp::WAVE_SAMPLES;
use lmv_core::milk::outputs::FrameOutputs;
use lmv_core::milk::{MilkBundle, MilkRuntime};
use lmv_core::preset::Preset;
use lmv_core::render::scenes::GeneratorConfig;
use lmv_core::render::scenes::warp_mesh::draw;

/// A minimal `.milk` file with one enabled custom shape whose per-frame program
/// computes every number the draw layer reads.
///
/// The values are deliberately **not** the format's defaults and not each other:
/// a regression that dropped the program and fell back to the initial conditions,
/// or that read one register where it meant another, changes at least one of the
/// assertions below.
const SHAPE_PRESET: &str = "\
[preset00]
fRating=3.000
nMotionVectorsX=0.000
nMotionVectorsY=0.000
fWaveAlpha=0.000
shapecode_0_enabled=1
shapecode_0_sides=4
shapecode_0_num_inst=3
shapecode_0_additive=0
shapecode_0_x=0.5
shapecode_0_y=0.5
shapecode_0_rad=0.1
shapecode_0_a=1.0
shape_0_per_frame1=sides = 7;
shape_0_per_frame2=x = 0.25 + instance*0.25;
shape_0_per_frame3=y = 0.75;
shape_0_per_frame4=rad = 0.2;
shape_0_per_frame5=r = 1; g = 0; b = 0; a = 1;
shape_0_per_frame6=r2 = 0; g2 = 0; b2 = 1; a2 = 1;
shape_0_per_frame7=border_a = 0;
";

/// How many sides `SHAPE_PRESET`'s per-frame program asks for, against the four
/// its initial condition declares.
const PROGRAM_SIDES: u32 = 7;
/// How many copies it draws.
const INSTANCES: u32 = 3;

/// Convert the text and load the emitted bundle back, exactly as `shot` would.
fn bundle() -> MilkBundle {
    let file = milkconv::milk::parse(SHAPE_PRESET).expect("the fixture parses as a .milk file");
    let converted = milkconv::convert::convert(&file, "shape_fixture").expect("it converts");
    let preset = Preset::from_toml_str(&converted.toml).unwrap_or_else(|e| {
        panic!(
            "the emitted bundle must load back: {e}\n---\n{}",
            converted.toml
        )
    });
    match preset.config {
        Some(GeneratorConfig::WarpMesh {
            milk: Some(milk), ..
        }) => *milk,
        other => panic!("the converted preset must carry a bundle, got {other:?}"),
    }
}

/// **The `[[milk.shapes]]` round trip carries the element at all.**
///
/// The non-vacuity check for everything below, and it is not hypothetical: the
/// converter compiled custom shapes into the bundle and the emitter dropped them
/// on the floor, so `MilkBundle::from_assembly` handed the scene an empty
/// `shapes` and 63 % of the corpus drew none of its own geometry. Nothing failed;
/// the presets just rendered without their shapes.
#[test]
fn a_converted_shape_survives_the_bundle_round_trip() {
    let bundle = bundle();
    assert_eq!(
        bundle.shapes.len(),
        1,
        "the one enabled shape must reach the loaded bundle"
    );
    assert!(
        bundle.waves.is_empty(),
        "the fixture enables no custom wave, so none may appear"
    );
}

/// **A custom shape honours its per-point program** — Phase 4's done-when.
///
/// Every assertion is a number the *program* computed rather than one the file
/// declared, which is what makes this a test of the program rather than of the
/// initial conditions:
///
/// - `sides = 7` against an initial condition of `4`, read as the triangle count;
/// - `x = 0.25 + instance*0.25`, read as three copies at three distinct centres,
///   which also pins that `instance` is bound per copy;
/// - `rad = 0.2` against an initial condition of `0.1`, read as the extent;
/// - the centre colour red and the edge colour blue, read per vertex, which pins
///   that the fan's hub and rim take different registers.
#[test]
fn a_custom_shape_honours_its_per_point_program() {
    let mut runtime = MilkRuntime::new(bundle(), 0);
    let waveform = [0.0f32; WAVE_SAMPLES];
    let mut geometry = draw::DrawGeometry::default();
    // `wave_a = 0` and no motion vectors, so every triangle below is the shape's.
    let out = FrameOutputs {
        wave_a: 0.0,
        mv_a: 0.0,
        ob_a: 0.0,
        ib_a: 0.0,
        ..Default::default()
    };
    draw::build(
        &mut geometry,
        Some(&mut runtime),
        &out,
        &waveform,
        0.0,
        1.0 / 30.0,
        16.0 / 9.0,
    );

    // Seven sides, three instances, three vertices per fan triangle.
    assert_eq!(
        geometry.triangles.len(),
        (PROGRAM_SIDES * INSTANCES * 3) as usize,
        "the program's `sides = {PROGRAM_SIDES}` must beat the file's initial \
         condition of 4, across all {INSTANCES} instances"
    );

    // The three centres the program placed, taken from each fan's hub vertex.
    let mut centres: Vec<f32> = geometry
        .triangles
        .chunks_exact(3)
        .map(|t| t[0].pos[0])
        .collect();
    centres.dedup();
    centres.sort_by(f32::total_cmp);
    centres.dedup();
    assert_eq!(
        centres.len(),
        INSTANCES as usize,
        "`x = 0.25 + instance*0.25` must place {INSTANCES} copies at {INSTANCES} \
         distinct centres, so `instance` is bound per copy; got {centres:?}"
    );
    // ...and they are evenly spaced, which no per-instance constant would be.
    let gaps: Vec<f32> = centres.windows(2).map(|w| w[1] - w[0]).collect();
    assert!(
        gaps.windows(2).all(|g| (g[0] - g[1]).abs() < 1e-4),
        "the three centres must be evenly spaced, got gaps {gaps:?}"
    );

    // The hub is red and the rim is blue: two different registers, read per
    // vertex rather than per shape.
    let hub = geometry.triangles[0];
    let rim = geometry.triangles[1];
    assert!(
        hub.color[0] > 0.0 && hub.color[2] <= 1e-6,
        "the fan's hub takes `r`/`g`/`b` (red here), got {:?}",
        hub.color
    );
    assert!(
        rim.color[2] > 0.0 && rim.color[0] <= 1e-6,
        "the fan's rim takes `r2`/`g2`/`b2` (blue here), got {:?}",
        rim.color
    );

    // `rad = 0.2` against an initial condition of 0.1. Measured as the hub-to-rim
    // distance with the aspect divided back out, which is exact for **every**
    // vertex rather than only for one at the pole — a seven-sided polygon has no
    // vertex at the pole, and reading the extent along y alone would land at
    // `rad * sin(102.9 deg)` and look like a 2.5 % error in the program.
    //
    // That it is exact for every vertex is also the ADR-0037 statement: the shape
    // is round on a 16:9 target, not stretched.
    let aspect = 16.0f32 / 9.0;
    for (index, triangle) in geometry.triangles.chunks_exact(3).enumerate() {
        for vertex in &triangle[1..] {
            let dx = (vertex.pos[0] - triangle[0].pos[0]) / aspect;
            let dy = vertex.pos[1] - triangle[0].pos[1];
            let reach = dx.hypot(dy);
            assert!(
                (reach - 0.2).abs() < 1e-4,
                "triangle {index}: the program's `rad = 0.2` must beat the file's \
                 0.1, and the reach must be the same in every direction; got {reach}"
            );
        }
    }
}

/// **`shapecode_N_additive = 0` puts the shape in the OVER half**, which is the
/// half of Phase 4's blend work that only a converted preset exercises.
///
/// The fixture declares the flag off, so every one of its triangles must sit
/// above the partition — and the partition is what `WarpMeshScene::render` splits
/// its two draw calls on.
#[test]
fn a_non_additive_shape_lands_in_the_over_half() {
    let mut runtime = MilkRuntime::new(bundle(), 0);
    let mut geometry = draw::DrawGeometry::default();
    draw::build(
        &mut geometry,
        Some(&mut runtime),
        &FrameOutputs {
            wave_a: 0.0,
            mv_a: 0.0,
            ob_a: 0.0,
            ib_a: 0.0,
            ..Default::default()
        },
        &[0.0f32; WAVE_SAMPLES],
        0.0,
        1.0 / 30.0,
        16.0 / 9.0,
    );
    assert!(!geometry.triangles.is_empty(), "the shape drew nothing");
    assert_eq!(
        geometry.triangles_additive, 0,
        "the fixture's shape declares `additive = 0`, so none of its triangles \
         may blend additively"
    );
    for (i, vertex) in geometry.triangles.iter().enumerate() {
        assert!(
            (0.0..=1.0).contains(&vertex.alpha),
            "vertex {i} blends OVER with alpha {} outside 0..=1, which would make \
             the premultiplied blend's `1 - src.a` negative",
            vertex.alpha
        );
    }
}
