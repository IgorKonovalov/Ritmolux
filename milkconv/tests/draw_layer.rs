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

// ---------------------------------------------------------------------------
// A wave's per-point state carries along the trace (Plan 0108 Phase 5)
// ---------------------------------------------------------------------------

/// The `flip` alternation, in the shape **612 corpus files** write it, on a
/// single custom wave whose points are otherwise identical.
///
/// Every point runs the same three lines with the same `sample`-independent
/// arithmetic, so the only thing that can make consecutive points differ is
/// `flip` surviving from one to the next. `y` is `0.7` on odd points and `0.3`
/// on even ones — an alternation with no other possible source.
const FLIP_PRESET: &str = "\
[preset00]
fRating=3.000
nMotionVectorsX=0.000
nMotionVectorsY=0.000
fWaveAlpha=0.000
wavecode_0_enabled=1
wavecode_0_samples=8
wavecode_0_bUseDots=0
wavecode_0_bDrawThick=0
wavecode_0_bAdditive=1
wavecode_0_a=1.000
wave_0_per_point1=flip = flip + 1;
wave_0_per_point2=flip = flip * below(flip, 2);
wave_0_per_point3=x = 0.5;
wave_0_per_point4=y = 0.3 + flip * 0.4;
";

/// **A custom wave's per-point program carries its state from one point to the
/// next** — Plan 0108 Phase 5, and the reason *chasers 19 Portal*'s mirror
/// symmetry converted cleanly and rendered inert (design-backlog 0107).
///
/// The fold that preset is named for is not in a warp shader and not in a
/// per-vertex program — it has neither. It is three lines of per-point code in
/// each of three custom waves, alternating `yp` about the trace to draw a
/// mirrored pair. `ElementRuntime::run_point` restored a register snapshot
/// before every point, exactly as the mesh's per-vertex path does, so `flip`
/// was handed the same value each time, the two lines computed a constant, and
/// the pair collapsed to one trace. Nothing failed and the conversion was clean
/// — which is what made it hard to see.
///
/// The claim is asserted as an alternation rather than as a pair of literals:
/// `y` must take **two** distinct values across a run of points whose only
/// varying input is one the program never reads.
#[test]
fn a_waves_per_point_state_carries_to_the_next_point() {
    let file = milkconv::milk::parse(FLIP_PRESET).expect("the fixture parses as a .milk file");
    let converted = milkconv::convert::convert(&file, "flip_fixture").expect("it converts");
    let preset = Preset::from_toml_str(&converted.toml)
        .unwrap_or_else(|e| panic!("the emitted bundle must load back: {e}"));
    let bundle = match preset.config {
        Some(GeneratorConfig::WarpMesh {
            milk: Some(milk), ..
        }) => *milk,
        other => panic!("the converted preset must carry a bundle, got {other:?}"),
    };
    assert_eq!(bundle.waves.len(), 1, "the one enabled wave must survive");

    let mut runtime = MilkRuntime::new(bundle, 0);
    runtime.run_frame(
        &lmv_core::dsp::AnalysisFrame::default(),
        0.0,
        1.0 / 30.0,
        (32, 24),
        16.0 / 9.0,
    );
    runtime
        .run_wave_frame(0)
        .expect("the wave's per-frame program runs");

    let ys: Vec<f32> = (0..8)
        .filter_map(|i| runtime.run_wave_point(0, i as f32 / 7.0, 0.0))
        .map(|p| p.y)
        .collect();
    println!("[draw_layer] flip alternation down the trace: {ys:?}");
    assert_eq!(ys.len(), 8, "every point must produce a position");

    let mut distinct: Vec<f32> = ys.clone();
    distinct.sort_by(f32::total_cmp);
    distinct.dedup_by(|a, b| (*a - *b).abs() < 1e-5);
    assert_eq!(
        distinct.len(),
        2,
        "`flip` must alternate down the trace, giving `y` exactly two values; \
         got {distinct:?} from {ys:?}. One value means the per-point register \
         file is being restored between points, so a preset's alternation, \
         accumulation or integration along the wave silently does nothing — \
         3 368 of the corpus's 6 347 custom-wave presets depend on it"
    );
    // ...and it alternates rather than merely holding two values in some order,
    // which a program that reset every other point would also satisfy.
    assert!(
        ys.windows(2).all(|w| (w[0] - w[1]).abs() > 1e-5),
        "consecutive points must differ — `flip` toggles every point; got {ys:?}"
    );
}

/// Load [`FLIP_PRESET`] (or a variant of it) and run its per-frame program once,
/// leaving the runtime positioned at the first point of a frame.
fn flip_runtime(text: &str) -> MilkRuntime {
    let file = milkconv::milk::parse(text).expect("the fixture parses as a .milk file");
    let converted = milkconv::convert::convert(&file, "flip_fixture").expect("it converts");
    let preset = Preset::from_toml_str(&converted.toml)
        .unwrap_or_else(|e| panic!("the emitted bundle must load back: {e}"));
    let bundle = match preset.config {
        Some(GeneratorConfig::WarpMesh {
            milk: Some(milk), ..
        }) => *milk,
        other => panic!("the converted preset must carry a bundle, got {other:?}"),
    };
    MilkRuntime::new(bundle, 0)
}

/// One frame of `count` points: the per-frame program, then the walk.
fn flip_frame(runtime: &mut MilkRuntime, count: usize) -> Vec<f32> {
    runtime.run_frame(
        &lmv_core::dsp::AnalysisFrame::default(),
        0.0,
        1.0 / 30.0,
        (32, 24),
        16.0 / 9.0,
    );
    runtime
        .run_wave_frame(0)
        .expect("the wave's per-frame program runs");
    (0..count)
        .filter_map(|i| runtime.run_wave_point(0, i as f32 / count.max(2) as f32, 0.0))
        .map(|p| p.y)
        .collect()
}

/// **The carry also crosses the frame boundary, and on an odd-length trace that
/// inverts the whole figure every frame** — Plan 0108's Mode 4 review,
/// 2026-08-17.
///
/// Phase 5 removed the per-point snapshot restore. What it did not state is that
/// nothing else reseeds a working register either: `ElementRuntime::run_frame`
/// seeds only the named wave-point **outputs** (`milk::outputs`'s
/// `WavePointSlots` — `x`, `y`, `r`, `g`, `b`, `a`), and a wave's `snapshot_of`
/// is now empty, so `flip` survives from the last point of one frame into the
/// first point of the next.
///
/// With an **even** sample count the two-state counter returns to where it
/// started and the frame boundary is invisible — which is exactly what the
/// eight-point fixture above cannot see, and why this arm exists. With an odd
/// one the trace comes out inverted on every other frame, so a mirrored pair
/// alternates at the **display's** refresh rate: twice as fast on a 120 Hz panel
/// as on a 60 Hz one.
///
/// **This pins the behaviour rather than blessing it.** It is most likely
/// faithful — MilkDrop allocates a custom element's variable space once and only
/// its `init` code reseeds it — but that is a claim about the reference, and
/// Plan 0108's Phase 6 is where it is answered with `foo_vis_milk2` on screen.
/// What must not happen is the behaviour moving silently while `run_point`'s doc
/// comment still describes the carry as reaching only "the next point".
#[test]
fn a_waves_per_point_state_also_carries_across_the_frame_boundary() {
    // The same three lines on a seven-point trace: an odd count, so `flip` does
    // NOT come back to its starting value at the end of a frame.
    let odd = FLIP_PRESET.replace("wavecode_0_samples=8", "wavecode_0_samples=7");
    assert_ne!(
        odd, FLIP_PRESET,
        "the sample-count substitution found nothing"
    );

    let mut runtime = flip_runtime(&odd);
    let first = flip_frame(&mut runtime, 7);
    let second = flip_frame(&mut runtime, 7);
    let third = flip_frame(&mut runtime, 7);
    println!("[draw_layer] flip across frames, 7 points — 1: {first:?} 2: {second:?} 3: {third:?}");

    assert!(
        first.iter().zip(&second).all(|(a, b)| (a - b).abs() > 1e-5),
        "frame 2 repeats frame 1 ({first:?} then {second:?}), so something \
         reseeds `flip` at the frame boundary. That is a defensible behaviour — \
         but it is the OPPOSITE of what ships today, and `run_point`'s doc \
         comment has to move with it"
    );
    assert!(
        first.iter().zip(&third).all(|(a, b)| (a - b).abs() < 1e-5),
        "frame 3 does not return to frame 1 ({first:?} then {third:?}). The \
         two-state counter has a period of two frames on an odd-length trace; \
         anything else means the carry is not a clean walk"
    );

    // ...and the even-length case is genuinely the blind spot this arm covers:
    // there the frame boundary changes nothing at all.
    let mut even = flip_runtime(FLIP_PRESET);
    let a = flip_frame(&mut even, 8);
    let b = flip_frame(&mut even, 8);
    assert!(
        a.iter().zip(&b).all(|(x, y)| (x - y).abs() < 1e-5),
        "an eight-point trace must be frame-invariant ({a:?} then {b:?}) — if it \
         is not, the even case is no longer the quiet one and the arm above is \
         measuring something other than the counter's parity"
    );
}

/// Every `wave_mode` the reference has.
const WAVE_MODES: [f32; 8] = [0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0];

fn segment_length(s: &lmv_core::render::scenes::lines::SegmentInstance) -> f32 {
    (s.b[0] - s.a[0]).hypot(s.b[1] - s.a[1])
}

/// **`wave_usedots = 1` puts separated marks along the trace where `= 0` puts a
/// continuous stroke, in every mode** — Plan 0108 Phase 4's behavioural claim
/// for the symptom cheapest to convict, because it is binary: the beads appear
/// or they do not.
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

/// A trace with structure in it, so "the same geometry" below is a claim about
/// something rather than about a flat line. Asymmetric on purpose: a symmetric
/// trace would survive a half-turn rotation and could not tell one apart from
/// the identity.
fn ramp_waveform() -> [f32; WAVE_SAMPLES] {
    std::array::from_fn(|i| {
        let t = i as f32 / (WAVE_SAMPLES - 1) as f32;
        t * t - 0.2
    })
}

/// The frame outputs for a mode-6 line, with everything but the waveform
/// silenced so every segment built belongs to it.
fn wave_outputs(mystery: f32) -> FrameOutputs {
    FrameOutputs {
        wave_mode: 6.0,
        wave_mystery: mystery,
        wave_a: 1.0,
        mv_a: 0.0,
        ob_a: 0.0,
        ib_a: 0.0,
        ..Default::default()
    }
}

/// Build the draw layer at `time` and hand back the segments it emitted.
fn segments_at(time: f32, mystery: f32, waveform: &[f32; WAVE_SAMPLES]) -> Vec<[f32; 4]> {
    let mut runtime = MilkRuntime::new(bundle(), 0);
    let mut geometry = draw::DrawGeometry::default();
    draw::build(
        &mut geometry,
        Some(&mut runtime),
        &wave_outputs(mystery),
        waveform,
        time,
        1.0 / 30.0,
        16.0 / 9.0,
    );
    geometry
        .segments
        .iter()
        .map(|s| [s.a[0], s.a[1], s.b[0], s.b[1]])
        .collect()
}

/// **A mode-6 figure's orientation is a pure function of `wave_mystery`** —
/// Plan 0109 Phase 2, design-backlog 0116.
///
/// # The defect
///
/// The arm computed its angle as `mystery * PI + time * 0.05`, a full turn every
/// ~126 s. `time` was the only reading of the clock in the whole draw layer, it
/// contradicted the arm's own comment, and it meant *a trace authored horizontal
/// was horizontal only at instants*. Plan 0108 Phase 4 named it and left it in
/// because the question — does the reference's line drift? — is about the
/// reference; Phase 6 of that plan put the two side by side and answered no.
///
/// # The three claims
///
/// **Pure**, so the same trace builds the same geometry at two well-separated
/// times: 61 s was half a turn under the old term, the furthest apart two frames
/// could be. **Horizontal** in the strict sense — a trace authored horizontal
/// (`wave_mystery = 0`, a flat trace) has every endpoint at one height. And
/// **still steered by `wave_mystery`**, which is the non-vacuity: a figure that
/// ignored its angle entirely would pass the first two.
#[test]
fn a_mode_six_figure_is_oriented_by_mystery_alone() {
    let waveform = ramp_waveform();
    let early = segments_at(0.0, 0.0, &waveform);
    let late = segments_at(61.0, 0.0, &waveform);
    assert!(
        !early.is_empty(),
        "the fixture drew no waveform segments, so nothing below is a test of one"
    );
    assert_eq!(
        early, late,
        "the mode-6 figure moved between t = 0 and t = 61 s. Its angle is \
         `wave_mystery` alone — if the clock is being read again, a trace \
         authored horizontal is horizontal only at instants"
    );

    // Horizontal means horizontal: a flat trace at `mystery = 0` lies on one
    // line. The endpoints carry the waveform's own excursion, so the trace is
    // flattened for this arm rather than the tolerance being widened.
    let flat = segments_at(0.0, 0.0, &[0.0f32; WAVE_SAMPLES]);
    let height = flat.first().map_or(0.0, |s| s[1]);
    for (i, s) in flat.iter().enumerate() {
        assert!(
            (s[1] - height).abs() < 1e-6 && (s[3] - height).abs() < 1e-6,
            "segment {i} of a flat `mystery = 0` trace runs from y {} to y {}, \
             off the trace's own height {height} — the figure is at an angle its \
             preset never asked for",
            s[1],
            s[3]
        );
    }
    let span = flat.iter().fold(f32::MIN, |m, s| m.max(s[2].abs()));
    assert!(
        span > 0.5,
        "the flat trace spans only {span} in x, so its horizontality is the \
         horizontality of a dot"
    );

    // Non-vacuity: the angle still responds to the parameter that owns it.
    let turned = segments_at(0.0, 0.5, &waveform);
    assert_ne!(
        early, turned,
        "`wave_mystery = 0.5` built the same geometry as `0`, so the figure is \
         not oriented by mystery either — the term was not removed, the whole \
         angle was"
    );
}
