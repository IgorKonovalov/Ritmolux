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

// ---------------------------------------------------------------------------
// The waveform draw layer (Plan 0108 Phase 4, design-backlog 0107)
// ---------------------------------------------------------------------------

/// A waveform with structure in it, so no mode collapses to a straight line.
fn trace() -> [f32; WAVE_SAMPLES] {
    std::array::from_fn(|i| {
        (i as f32 / WAVE_SAMPLES as f32 * std::f32::consts::TAU * 3.0).sin() * 0.7
    })
}

/// The waveform alone: every other producer silenced, so the geometry under test
/// is the trace's and nothing else.
fn waveform_only(mode: f32, use_dots: f32) -> FrameOutputs {
    FrameOutputs {
        wave_a: 1.0,
        wave_mode: mode,
        wave_usedots: use_dots,
        mv_a: 0.0,
        ob_a: 0.0,
        ib_a: 0.0,
        ..Default::default()
    }
}

/// Build the waveform layer for one `(mode, use_dots)` pair.
fn waveform_geometry(mode: f32, use_dots: f32) -> draw::DrawGeometry {
    let mut runtime = MilkRuntime::new(
        MilkBundle::from_assembly(None, None, None).expect("the empty bundle decodes"),
        0,
    );
    let mut geometry = draw::DrawGeometry::default();
    draw::build(
        &mut geometry,
        Some(&mut runtime),
        &waveform_only(mode, use_dots),
        &trace(),
        0.0,
        1.0 / 30.0,
        16.0 / 9.0,
    );
    geometry
}

/// Every `wave_mode` the reference has.
const WAVE_MODES: [f32; 8] = [0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0];

fn segment_length(s: &lmv_core::render::scenes::lines::SegmentInstance) -> f32 {
    (s.b[0] - s.a[0]).hypot(s.b[1] - s.a[1])
}

/// **`wave_usedots = 1` puts separated marks along the trace where `= 0` puts a
/// continuous stroke, in every mode** — Plan 0108 Phase 4's behavioural claim
/// for the symptom the plan names cheapest to convict, because it is binary:
/// the beads appear or they do not.
///
/// The reported symptom is *Cosmic Dust 2*'s `wave_usedots` beads never
/// appearing (design-backlog 0107). This asks the geometry stage first, which
/// separates two very different causes — a dots path that is never reached
/// (nothing in the buffer) from one that is reached and draws something too
/// small to see (marks in the buffer, and the render is the thing to look at).
///
/// The answer was **both, in different modes.** The path is reached everywhere,
/// but `wave_mode 5` draws in two passes and its first pass emitted a polyline
/// unconditionally, so a preset asking for dots got a stroke above `wave_y` and
/// beads below it. That is what the `all marks are one length` arm below pins:
/// before the repair, mode 5's dotted geometry held a segment **13.6x longer
/// than `DOT_LENGTH`** and every other mode's held none.
///
/// The `joined` arm pins the second half of the same phase's finding, and it is
/// not cosmetic: the line renderer's falloff runs *across* the stroke only, so a
/// mark is round because both caps are pushed out by the half-width (ADR-0041),
/// not because the segment is short. Unflagged, it is a hard-edged sub-pixel
/// dash — see `draw::dots` and the render test below.
#[test]
fn wave_usedots_puts_separated_marks_where_a_line_puts_a_stroke() {
    use lmv_core::render::scenes::lines::{JOINED_A, JOINED_B};

    for mode in WAVE_MODES {
        let line = waveform_geometry(mode, 0.0);
        let dotted = waveform_geometry(mode, 1.0);
        assert!(
            !dotted.segments.is_empty(),
            "wave_mode {mode} emitted no dotted geometry at all"
        );

        let lengths: Vec<f32> = dotted.segments.iter().map(segment_length).collect();
        let longest = lengths.iter().copied().fold(0.0, f32::max);
        let shortest = lengths.iter().copied().fold(f32::INFINITY, f32::min);
        let longest_stroke = line
            .segments
            .iter()
            .map(segment_length)
            .fold(0.0f32, f32::max);
        println!(
            "[draw_layer] wave_mode {mode}: line {} segs (longest {longest_stroke:.5}), \
             dots {} segs (lengths {shortest:.5}..{longest:.5})",
            line.segments.len(),
            dotted.segments.len()
        );

        // Every mark is the same length, which is what "all of them are dots"
        // means when the alternative is a mode that emits some of each.
        assert!(
            (longest - shortest).abs() < 1e-6,
            "wave_mode {mode} mixes marks and strokes under `wave_usedots`: its \
             dotted geometry runs {shortest:.5}..{longest:.5}. A pass that emits \
             a polyline regardless of the flag is the shape of this — every \
             emit in `waveform_figure` must go through `emit_trace`"
        );
        // ...and a mark is far shorter than the stroke it replaces, so the trace
        // reads as beads rather than as a line drawn twice.
        assert!(
            longest * 4.0 < longest_stroke,
            "wave_mode {mode}: a dotted mark is {longest:.5} against a stroke's \
             {longest_stroke:.5}, which is not a bead"
        );
        // ...and every mark carries both caps, which is what gives it a round
        // footprint instead of a sub-pixel dash across the trace.
        assert!(
            dotted
                .segments
                .iter()
                .all(|s| s.joined == JOINED_A | JOINED_B),
            "wave_mode {mode}: a dotted mark must flag BOTH ends joined so the \
             quad extends past each cap by the half-width (ADR-0041). Without \
             that the falloff is clipped to a dash and the beads vanish at any \
             resolution where the mark is under a pixel long"
        );
    }
}

/// **The beads survive to the screen, at low resolution as well as high** — the
/// render half of the claim above, and the arm that actually convicts
/// design-backlog 0107's "never appear".
///
/// The geometry test cannot see this defect: the marks were in the buffer all
/// along. What was wrong was their *footprint*. A mark's extent is in world
/// units, so it shrinks with the target, and an unflagged one is only
/// `DOT_LENGTH` long against `2 * width` across — a rectangle **3.3x wider than
/// it is long**, whose along-axis extent is under a pixel at any size a person
/// would run. A continuous stroke of the same width does not care: it
/// accumulates coverage along its whole length.
///
/// Measured on one drawn frame, pixels above half brightness, dots against the
/// same preset with `bWaveDots=0`:
///
/// | target     | before      | after        |
/// |------------|-------------|--------------|
/// | 320 x 180  | 2 / 137     | 46 / 137     |
/// | 960 x 540  | 59 / 1 255  | 474 / 1 255  |
/// | 1920 x 1080| 300 / 5 008 | 2 045 / 5 008|
///
/// **Two of 512 marks at 320x180** is the defect as a number. The floor below is
/// a fifth of the stroke's footprint — comfortably under the ~0.4 the repair
/// reaches and far above the ~0.02 it replaced — and it is checked at both ends
/// of a 9x span in area, because resolution independence is the property that
/// broke.
#[test]
fn the_dotted_trace_reaches_the_screen_at_every_resolution() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("milkconv has a workspace-root parent")
        .join("core/tests/fixtures/scratch-0108/wave-dots.milk");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("the Phase 4 fixture must be readable at {path:?}: {e}"));
    let frame = lmv_core::dsp::AnalysisFrame {
        waveform: trace(),
        ..Default::default()
    };

    for (w, h) in [(320u32, 180u32), (960u32, 540u32)] {
        let mut renderer =
            match lmv_core::render::Renderer::new_headless(lmv_core::render::HeadlessOptions {
                width: w,
                height: h,
                prefer_software: true,
            }) {
                Ok(r) => r,
                Err(lmv_core::render::RenderError::RequestAdapter(_)) => {
                    eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
                    return;
                }
                Err(e) => panic!("headless renderer build failed: {e}"),
            };

        // One frame, so what is measured is the light this draw layer laid down
        // rather than several frames of it integrated by the feedback field.
        let bright = |renderer: &mut lmv_core::render::Renderer, label: &str, src: &str| -> usize {
            let file = milkconv::milk::parse(src).expect("the fixture parses as a .milk file");
            let converted = milkconv::convert::convert(&file, label).expect("it converts");
            let mut preset = Preset::from_toml_str(&converted.toml)
                .unwrap_or_else(|e| panic!("the emitted bundle must load back: {e}"));
            preset.name = label.to_string();
            renderer.set_presets(vec![preset]);
            renderer
                .capture_preset(label, &frame, 1)
                .expect("capture one drawn frame")
                .rgba
                .chunks_exact(4)
                .filter(|px| px.iter().take(3).any(|b| *b > 96))
                .count()
        };

        let dotted = bright(&mut renderer, "dots", &text);
        let stroked = bright(
            &mut renderer,
            "line",
            &text.replace("bWaveDots=1", "bWaveDots=0"),
        );
        let share = dotted as f32 / stroked.max(1) as f32;
        println!(
            "[draw_layer] {w}x{h}: dotted trace lights {dotted} px above half \
             brightness against a stroke's {stroked} — {share:.3}"
        );

        assert!(
            stroked > 50,
            "the CONTROL drew almost nothing at {w}x{h} ({stroked} px), so the \
             ratio below means nothing. The fixture's waveform is what lights \
             this frame — check it is reaching the draw layer at all"
        );
        assert!(
            share > 0.2,
            "the dotted trace lights {dotted} px at {w}x{h} against a stroke's \
             {stroked} ({share:.3}). The beads are being drawn under a pixel \
             wide — see `draw::dots`, whose caps are what give a mark its \
             footprint"
        );
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
