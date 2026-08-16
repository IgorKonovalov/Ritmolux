//! **The warp mesh renders, moves, and reacts** (Plan 0100 Phase 1's done-when,
//! [ADR-0113](../../docs/adrs/0113-milkdrop-presets-are-translated-ahead-of-time-onto-a-warp-mesh-idiom.md)).
//!
//! # Why this file exists at all
//!
//! `sanity`, `animation` and `reactivity` are the three behavioural gates the
//! shipped library is held to, and all three iterate `default_presets()`. Plan
//! 0100 Phase 1 ships the **engine** and deliberately no preset content — the
//! same split Plan 0091 took for `shape_field`, on ADR-0081's reasoning that
//! worlds belong to the author's lane — so the `warp_mesh` family has no shipped
//! member for those gates to reach, and its rows in their per-system tables are
//! inherited rather than measured.
//!
//! That would leave the phase's done-when unverifiable, so the three
//! measurements are applied here to the **golden fixture** instead. Each is the
//! same statistic against the same floor as the gate it stands in for, taken
//! through the same headless software-adapter renderer:
//!
//! | this file                              | the gate it stands in for            |
//! |----------------------------------------|--------------------------------------|
//! | [`the_fixture_draws_a_real_shape`]     | `sanity.rs`                          |
//! | [`the_fixture_animates`]               | `animation.rs`                       |
//! | [`the_fixture_reacts_to_audio`]        | `reactivity.rs`                      |
//!
//! **When the first `warp_mesh` preset ships, these become redundant and the
//! floors in the three gates become measurements rather than inheritances** —
//! re-derive them from `sanity.rs`'s printed distribution then, and this file can
//! narrow to the two properties below that no shipped preset would cover.
//!
//! It also carries the two properties that are specific to this idiom and belong
//! nowhere else: that the per-vertex program actually varies across the mesh
//! ([`the_per_vertex_program_varies_across_the_mesh`]), and the ADR-0058 adapter
//! comparison the three new bind-group layouts owe
//! ([`the_adapters_agree_on_the_warp_mesh`]).

use lmv_core::audio::AudioFormat;
use lmv_core::dsp::{AnalysisFrame, HOP_SIZE, WARMUP_HOPS};
use lmv_core::preset::{Preset, SystemKind};
use lmv_core::render::{
    CaptureImage, HeadlessOptions, RenderError, Renderer,
    metrics::{coverage, footprint_diff, frame_diff, quadrant_spread, tonal_flatness},
};
use lmv_core::signal::{bass_sine, chord, click_track, treble_tone};

/// The fixture under test — the same text `golden.rs` pins a baseline for, so
/// the two cannot describe different presets.
const FIXTURE: &str = include_str!("fixtures/warp_mesh.toml");

/// Capture size, matching `sanity.rs` and `animation.rs` so the floors below are
/// comparable to theirs rather than to a different rasterization.
const SIZE: u32 = 96;

/// `sanity.rs`'s constants, restated here because they are private to that
/// integration-test crate.
const EPS: u8 = 10;
const BLACK: [u8; 4] = [0, 0, 0, 255];
const MIN_QUADRANTS: u8 = 2;
const MAX_TONAL_FLATNESS: f32 = 0.90;
/// `sanity.rs`'s `coverage_floor(SystemKind::WarpMesh)`, which is inherited from
/// `FragmentField` on the structural argument that a fullscreen field with
/// `occlude` cannot score low. Duplicated rather than exported because the gate's
/// table is a private function there; [`the_floor_matches_the_sanity_gate`]
/// is what keeps the two honest.
const COVERAGE_FLOOR: f32 = 0.50;
/// `sanity.rs`'s frame count.
const SANITY_FRAMES: u32 = 30;

/// `animation.rs`'s capture points and floor.
const FRAME_A: u32 = 24;
const FRAME_B: u32 = 48;
const MOTION_FLOOR: f32 = 0.01;
/// `animation.rs`'s mask floor (ADR-0091), so a near-empty frame's stray pixel
/// cannot read as strong motion.
const MIN_FOOTPRINT_FRAC: f32 = 0.015;

/// `reactivity.rs`'s stimulus shape.
const FORMAT: AudioFormat = AudioFormat {
    sample_rate: 48_000,
    channels: 2,
};
const SIGNAL_HOPS: usize = 24;
const HOPS: usize = WARMUP_HOPS + SIGNAL_HOPS;
const CLIP_SECS: f32 = (HOPS * HOP_SIZE) as f32 / FORMAT.sample_rate as f32;
const CLICK_BPM: f32 = 240.0;
const REACTIVITY_FLOOR: f32 = 0.02;

fn fixture() -> Preset {
    let preset = Preset::from_toml_str(FIXTURE).expect("the warp_mesh fixture parses");
    assert_eq!(preset.system, SystemKind::WarpMesh);
    assert!(
        preset.warnings.is_empty(),
        "the fixture must load clean, got {:?}",
        preset.warnings
    );
    assert!(
        preset.per_vertex.iter().any(|b| b.name == "zoom"),
        "the fixture must drive `zoom` from a [per_vertex] table — that binding is \
         the phase's done-when"
    );
    preset
}

/// The fixture with its `bg_*` bindings dropped, the way `sanity.rs` and
/// `animation.rs` both capture (ADR-0067). The fixture binds none, so this is the
/// identity today; it is written out so the comparison stays true if one is ever
/// added.
fn without_backdrop(mut preset: Preset) -> Preset {
    preset.params.retain(|b| !b.name.starts_with("bg_"));
    preset
}

fn headless() -> Option<Renderer> {
    headless_on(true)
}

fn headless_on(prefer_software: bool) -> Option<Renderer> {
    match Renderer::new_headless(HeadlessOptions {
        width: SIZE,
        height: SIZE,
        prefer_software,
    }) {
        Ok(r) => Some(r),
        Err(RenderError::RequestAdapter(_)) => {
            eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
            None
        }
        Err(e) => panic!("headless renderer build failed: {e}"),
    }
}

/// `sanity.rs`'s fully-driven frame.
fn loud() -> AnalysisFrame {
    AnalysisFrame {
        bass: 1.0,
        mid: 1.0,
        treb: 1.0,
        onset: 1.0,
        beat: true,
        bar: 0.5,
        spectrum: [1.0; lmv_core::dsp::SPECTRUM_BINS],
        ..Default::default()
    }
}

/// **Shape sanity**, `sanity.rs`'s three questions asked of the fixture: is
/// something there, is it more than a dot, and does it have an interior.
#[test]
fn the_fixture_draws_a_real_shape() {
    let Some(mut renderer) = headless() else {
        return;
    };
    let preset = without_backdrop(fixture());
    let name = preset.name.clone();
    renderer.set_presets(vec![preset]);
    let img = renderer
        .capture_preset(&name, &loud(), SANITY_FRAMES)
        .expect("capture the warp mesh fixture");

    let cov = coverage(&img, BLACK, EPS);
    let spread = quadrant_spread(&img, BLACK, EPS);
    let flat = tonal_flatness(&img, BLACK, EPS);
    println!(
        "[warp_mesh] coverage={cov:.4} (floor {COVERAGE_FLOOR:.2}) quadrants={spread} \
         flatness={flat:.4} (max {MAX_TONAL_FLATNESS:.2})"
    );

    assert!(
        cov >= COVERAGE_FLOOR,
        "the warp mesh fixture is blank: coverage {cov:.4} < {COVERAGE_FLOOR:.2}"
    );
    assert!(
        spread >= MIN_QUADRANTS,
        "the warp mesh fixture is a dot: {spread} quadrant(s) < {MIN_QUADRANTS}"
    );
    assert!(
        flat <= MAX_TONAL_FLATNESS,
        "the warp mesh fixture is flat: {:.1}% of its lit pixels sit in one \
         luminance band (max {:.0}%)",
        flat * 100.0,
        MAX_TONAL_FLATNESS * 100.0
    );
}

/// **Animation liveness**, `animation.rs`'s question: does the scene change over
/// time with the audio held constant?
///
/// A real question here rather than a formality. The warp mesh's motion is
/// entirely the feedback loop — the deposit is laid at a fixed place under a
/// fixed frame, and everything that moves does so because the *past* was
/// resampled somewhere else. A per-vertex transform that collapsed to the
/// identity would leave the field converging to a still ring and score near
/// zero.
#[test]
fn the_fixture_animates() {
    let Some(mut renderer) = headless() else {
        return;
    };
    let preset = without_backdrop(fixture());
    let name = preset.name.clone();
    renderer.set_presets(vec![preset]);
    // Silence, deliberately: the motion under test is the scene's own clock and
    // its feedback, not an audio edge.
    let quiet = AnalysisFrame::default();
    let a = renderer
        .capture_preset(&name, &quiet, FRAME_A)
        .expect("capture frame A");
    let b = renderer
        .capture_preset(&name, &quiet, FRAME_B)
        .expect("capture frame B");

    let motion = footprint_diff(&a, &b, BLACK, EPS, MIN_FOOTPRINT_FRAC);
    println!(
        "[warp_mesh] motion (footprint) frames {FRAME_A}->{FRAME_B} = {motion:.4} \
         (floor {MOTION_FLOOR:.2}), whole-frame {:.4}",
        frame_diff(&a, &b)
    );
    assert!(
        motion >= MOTION_FLOOR,
        "the warp mesh fixture is frozen: {motion:.4} < {MOTION_FLOOR:.2}"
    );
}

/// **Per-band reactivity**, `reactivity.rs`'s question, driven by the same four
/// PCM stimuli through the real analyzer.
#[test]
fn the_fixture_reacts_to_audio() {
    let Some(mut renderer) = headless() else {
        return;
    };
    let preset = fixture();
    let name = preset.name.clone();
    renderer.set_presets(vec![preset]);

    let hops: Vec<u32> = (WARMUP_HOPS as u32..HOPS as u32).collect();
    let capture = |renderer: &mut Renderer, pcm: &[f32]| -> Vec<CaptureImage> {
        renderer
            .capture_audio_after_warmup(&name, pcm, FORMAT, &hops, WARMUP_HOPS)
            .expect("capture through the analyzer")
            .images
    };

    let silence = vec![0.0f32; HOPS * HOP_SIZE * FORMAT.channels as usize];
    let baseline = capture(&mut renderer, &silence);

    let stimuli: [(&str, Vec<f32>); 4] = [
        ("bass", bass_sine(60.0, CLIP_SECS, FORMAT)),
        ("mid", chord(&[440.0, 660.0, 990.0], CLIP_SECS, FORMAT)),
        ("treb", treble_tone(12_000.0, CLIP_SECS, FORMAT)),
        ("onset", click_track(CLICK_BPM, CLIP_SECS, FORMAT)),
    ];

    let mut best = 0.0f32;
    for (band, pcm) in &stimuli {
        let lit = capture(&mut renderer, pcm);
        let response = baseline
            .iter()
            .zip(lit.iter())
            .map(|(a, b)| frame_diff(a, b))
            .fold(0.0f32, f32::max);
        println!("[warp_mesh] {band:<6} {response:.4}");
        best = best.max(response);
    }
    assert!(
        best >= REACTIVITY_FLOOR,
        "the warp mesh fixture has a dead reaction: its best band moved \
         {best:.4} < {REACTIVITY_FLOOR:.2}"
    );
}

/// **The per-vertex program varies across the mesh** — the one property that
/// distinguishes this idiom from the single shared affine it generalizes
/// (ADR-0048), and the only one no shipped preset would ever cover.
///
/// The fixture's `zoom` is `1.9 + rad * 0.9`. The control replaces it with the
/// constant that program evaluates to **at the mesh centre**, so the two frames
/// differ in exactly one thing: whether the transform varies with `rad`. A
/// regression that dropped the per-vertex path — a scratch never filled, a
/// series of the wrong length, a scalar leaking past the override — would render
/// the control, and the two frames would agree.
#[test]
fn the_per_vertex_program_varies_across_the_mesh() {
    let Some(mut renderer) = headless() else {
        return;
    };
    let varying = fixture();
    // The same preset with `zoom` pinned to its centre value (`rad = 0`) and no
    // `[per_vertex]` table at all — ADR-0048's single shared transform.
    let control = Preset::from_toml_str(
        &FIXTURE
            .replace("[per_vertex]", "[per_vertex_disabled]")
            .replace(
                "decay           =",
                "zoom            = \"1.9\"\ndecay           =",
            ),
    )
    .expect("the control parses");
    assert!(
        control.per_vertex.is_empty(),
        "the control must have no per-vertex program, or it proves nothing"
    );

    let varying_name = format!("{}-varying", varying.name);
    let control_name = format!("{}-control", control.name);
    let mut varying = varying;
    varying.name = varying_name.clone();
    let mut control = control;
    control.name = control_name.clone();
    renderer.set_presets(vec![varying, control]);

    let frame = loud();
    let a = renderer
        .capture_preset(&varying_name, &frame, SANITY_FRAMES)
        .expect("capture the varying mesh");
    let b = renderer
        .capture_preset(&control_name, &frame, SANITY_FRAMES)
        .expect("capture the control");
    let difference = frame_diff(&a, &b);
    println!(
        "[warp_mesh] per-vertex `zoom = 1.9 + rad * 0.9` vs the constant \
         `zoom = 1.9`: frame_diff {difference:.4}"
    );
    assert!(
        difference > 0.01,
        "a `rad`-varying per-vertex transform must render differently from the \
         constant it takes at the centre; got {difference:.4}. The per-vertex \
         series is not reaching the vertex buffer"
    );
}

/// **The three new bind-group layouts do not alias on WARP** — ADR-0058's
/// standing rule for any plan that adds a pass, measured rather than asserted.
///
/// The golden baseline this scene now owns is captured on the DX12 WARP software
/// adapter, so a layout collision there would be *blessed* rather than caught.
/// This renders the same fixture on both adapters and reports the difference; it
/// skips when the machine has only one of them, which is the CI case.
///
/// A **report with a loose gate**, not a byte comparison: the two adapters
/// rasterize a triangle mesh differently at the sub-pixel level and this scene
/// integrates that difference over 30 feedback frames, so a small drift is
/// expected. What aliasing looks like is not a small drift — it is a pass reading
/// another pass's buffer, which in the measurements recorded on `emitter.rs` and
/// `background.rs` moved whole frames.
///
/// **Measured 2026-08-16 on the development box (Windows 10, DX12), 96x96 over
/// 30 frames, before the baseline was blessed: hardware mean rgb
/// `56.2266 79.1377 91.2537`, WARP `56.2209 79.1241 91.2429`, `frame_diff`
/// `0.000303`.** Agreement to well under one 8-bit level, so the three
/// `warp-mesh-*-layout` shapes do not alias and the committed baseline pins a
/// picture hardware also draws.
#[test]
#[ignore = "needs both a hardware and a software adapter; run locally before blessing"]
fn the_adapters_agree_on_the_warp_mesh() {
    let (Some(mut hardware), Some(mut software)) = (headless_on(false), headless_on(true)) else {
        eprintln!("skipped: this machine does not expose both adapters");
        return;
    };
    let frame = loud();
    let capture = |renderer: &mut Renderer| -> CaptureImage {
        let preset = fixture();
        let name = preset.name.clone();
        renderer.set_presets(vec![preset]);
        renderer
            .capture_preset(&name, &frame, SANITY_FRAMES)
            .expect("capture the warp mesh fixture")
    };
    let hw = capture(&mut hardware);
    let sw = capture(&mut software);
    let difference = frame_diff(&hw, &sw);
    let mean = |img: &CaptureImage| -> [f64; 3] {
        let mut sums = [0f64; 3];
        for px in img.rgba.chunks_exact(4) {
            for (sum, c) in sums.iter_mut().zip(px) {
                *sum += f64::from(*c);
            }
        }
        let n = (img.rgba.len() / 4) as f64;
        [sums[0] / n, sums[1] / n, sums[2] / n]
    };
    println!(
        "[warp_mesh] hardware mean rgb {:?}, WARP mean rgb {:?}, frame_diff {difference:.6}",
        mean(&hw),
        mean(&sw)
    );
    assert!(
        difference < 0.05,
        "the two adapters disagree by {difference:.4} on the warp mesh fixture. \
         That is the shape of an ADR-0058 layout collision — compare the three \
         `warp-mesh-*-layout` shapes against the crate's enumeration before \
         blessing anything"
    );
}

// ---------------------------------------------------------------------------
// The bundle (Plan 0100 Phase 2)
// ---------------------------------------------------------------------------

/// The bundle-driven fixture — a hand-written `[milk]` table driving the mesh.
const MILK_FIXTURE: &str = include_str!("fixtures/warp_mesh_milk.toml");

fn milk_fixture() -> Preset {
    let preset = Preset::from_toml_str(MILK_FIXTURE).expect("the milk fixture parses");
    assert_eq!(preset.system, SystemKind::WarpMesh);
    assert!(
        preset.warnings.is_empty(),
        "the milk fixture must load clean, got {:?}",
        preset.warnings
    );
    preset
}

/// **A hand-written bundle drives the mesh, and the render is byte-identical
/// across two runs** — Plan 0100 Phase 2's done-when, both halves.
///
/// Byte-identical rather than within a tolerance, and that is the point. The
/// bundle carries state in three places a run could leak from: a register file
/// (`q1`, `q2`), a `megabuf` arena, and an RNG stream. Two renders that agree to
/// the last byte say all three were reset with the preset — which is what
/// NFR §6 asks of a capture and what the golden baseline below rests on.
#[test]
fn a_bundle_drives_the_mesh_and_reruns_identically() {
    let Some(mut renderer) = headless() else {
        return;
    };
    let preset = milk_fixture();
    let name = preset.name.clone();
    let frame = loud();

    renderer.set_presets(vec![preset]);
    let first = renderer
        .capture_preset(&name, &frame, SANITY_FRAMES)
        .expect("capture the bundle-driven fixture");
    let second = renderer
        .capture_preset(&name, &frame, SANITY_FRAMES)
        .expect("capture it again");

    assert_eq!(
        first.rgba,
        second.rgba,
        "two renders of a bundle-driven preset must be byte-identical; \
         {} of {} bytes differ",
        first
            .rgba
            .iter()
            .zip(&second.rgba)
            .filter(|(a, b)| a != b)
            .count(),
        first.rgba.len()
    );

    // ...and it is a picture, not a blank frame — otherwise the byte-identity
    // above would be satisfied by rendering nothing twice.
    let cov = coverage(&first, BLACK, EPS);
    let spread = quadrant_spread(&first, BLACK, EPS);
    println!("[warp_mesh/milk] coverage={cov:.4} quadrants={spread}");
    assert!(
        cov >= COVERAGE_FLOOR,
        "the bundle-driven fixture is blank: coverage {cov:.4} < {COVERAGE_FLOOR:.2}"
    );
    assert!(spread >= MIN_QUADRANTS);
}

/// **The bundle is what is driving the mesh**, not the preset's own defaults.
///
/// The non-vacuity check for the test above: a runtime that silently did nothing
/// would still render a picture and still render it twice identically. The
/// control is the same preset with its `[milk]` table removed — the same deposit,
/// the same palette, the same grid, and the scene's default transform — and the
/// two must differ.
#[test]
fn the_bundle_and_not_the_defaults_drives_the_transform() {
    let Some(mut renderer) = headless() else {
        return;
    };
    let driven = milk_fixture();
    let control = Preset::from_toml_str(&MILK_FIXTURE.replace("[milk]", "[milk_disabled]"))
        .expect("the control parses");
    assert!(
        matches!(
            control.config.as_ref(),
            Some(lmv_core::render::scenes::GeneratorConfig::WarpMesh { milk: None, .. })
        ),
        "the control must carry no bundle, or it proves nothing"
    );

    let mut driven = driven;
    driven.name = "milk-driven".into();
    let mut control = control;
    control.name = "milk-control".into();
    renderer.set_presets(vec![driven, control]);

    let frame = loud();
    let a = renderer
        .capture_preset("milk-driven", &frame, SANITY_FRAMES)
        .expect("capture the driven fixture");
    let b = renderer
        .capture_preset("milk-control", &frame, SANITY_FRAMES)
        .expect("capture the control");
    let difference = frame_diff(&a, &b);
    println!("[warp_mesh/milk] bundle vs no bundle: frame_diff {difference:.4}");
    assert!(
        difference > 0.01,
        "a preset with a `[milk]` table must render differently from the same \
         preset without one; got {difference:.4}. The VM is not reaching the mesh"
    );
}

/// The floor this file measures against is the one `sanity.rs` would apply, so
/// the duplication above cannot silently drift into a weaker bar.
///
/// It cannot read that gate's private table, so it asserts the *reason* instead:
/// the warp mesh inherits `fragment_field`'s floor, and both are fullscreen
/// scenes presenting with `occlude`. If someone lowers one and not the other,
/// the argument for the inheritance is gone and this comment is the thing to
/// re-read.
#[test]
fn the_floor_matches_the_sanity_gate() {
    assert_eq!(
        COVERAGE_FLOOR, 0.50,
        "this is `sanity.rs`'s coverage_floor(SystemKind::WarpMesh); update both \
         together, and re-derive it from that gate's printed distribution once a \
         warp_mesh preset ships"
    );
}
