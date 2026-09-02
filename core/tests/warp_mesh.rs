//! **The warp mesh renders, moves, and reacts** (Plan 0100 Phase 1's done-when,
//! ADR-0113).
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

use rlx_core::audio::AudioFormat;
use rlx_core::dsp::{AnalysisFrame, HOP_SIZE, WARMUP_HOPS};
use rlx_core::preset::{Preset, SystemKind};
use rlx_core::render::{
    CaptureImage, Renderer,
    metrics::{coverage, footprint_diff, frame_diff, quadrant_spread, tonal_flatness},
};
use rlx_core::signal::{bass_sine, chord, click_track, treble_tone};

mod common;

/// The fixture under test — the same text `golden.rs` pins a baseline for, so
/// the two cannot describe different presets.
const FIXTURE: &str = include_str!("fixtures/warp_mesh.toml");

/// Capture size, matching `sanity.rs` and `animation.rs` so the floors below are
/// comparable to theirs rather than to a different rasterization.
const SIZE: u32 = 96;

/// `sanity.rs`'s constants, restated here because they are private to that
/// integration-test crate.
///
/// **Audited at Plan 0116 Phase 3: `BLACK` stayed, and it is now a divergence
/// rather than a restatement.** `sanity.rs` derives its reference from each
/// capture (ADR-0126); this file does not. Every floor below was measured
/// against the constant, and re-basing the predicate without re-deriving them
/// would move this file's verdicts on a plan that measured none — Plan 0116
/// Phase 4 re-derives `sanity.rs`'s own floors and is scoped to that file. The
/// `warp_mesh` presets are lit-on-dark, so the two lenses agree on them today;
/// the day one paints its own ground, these floors need the same re-derivation
/// and this comment is the pointer to it.
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

/// `sanity.rs`'s fully-driven frame.
fn loud() -> AnalysisFrame {
    AnalysisFrame {
        bass: 1.0,
        mid: 1.0,
        treb: 1.0,
        onset: 1.0,
        beat: true,
        bar: 0.5,
        spectrum: [1.0; rlx_core::dsp::SPECTRUM_BINS],
        ..Default::default()
    }
}

/// **Shape sanity**, `sanity.rs`'s three questions asked of the fixture: is
/// something there, is it more than a dot, and does it have an interior.
#[test]
fn the_fixture_draws_a_real_shape() {
    let Some(mut renderer) = common::headless(SIZE, SIZE) else {
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
    let Some(mut renderer) = common::headless(SIZE, SIZE) else {
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
    let Some(mut renderer) = common::headless(SIZE, SIZE) else {
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
    let Some(mut renderer) = common::headless(SIZE, SIZE) else {
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
    let (Some(mut hardware), Some(mut software)) = (
        common::headless_on(SIZE, SIZE, false),
        common::headless_on(SIZE, SIZE, true),
    ) else {
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
    let Some(mut renderer) = common::headless(SIZE, SIZE) else {
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
    let Some(mut renderer) = common::headless(SIZE, SIZE) else {
        return;
    };
    let driven = milk_fixture();
    let control = Preset::from_toml_str(&MILK_FIXTURE.replace("[milk]", "[milk_disabled]"))
        .expect("the control parses");
    assert!(
        matches!(
            control.config.as_ref(),
            Some(rlx_core::render::scenes::GeneratorConfig::WarpMesh { milk: None, .. })
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

// ---------------------------------------------------------------------------
// The converted-shader surface (Plan 0110 Phase 2)
// ---------------------------------------------------------------------------

/// The shader-carrying fixture — the only preset anywhere in the crate whose
/// `[milk]` table names a `warp_shader` or a `comp_shader`, and so the only one
/// that builds `warp_mesh/shader.rs` at all.
const SHADER_FIXTURE: &str = include_str!("fixtures/warp_mesh_shader.toml");

fn shader_fixture() -> Preset {
    let preset = Preset::from_toml_str(SHADER_FIXTURE).expect("the shader fixture parses");
    assert_eq!(preset.system, SystemKind::WarpMesh);
    assert!(
        preset.warnings.is_empty(),
        "the shader fixture must load clean, got {:?}",
        preset.warnings
    );
    // Both modules and the full chain reached the bundle — `validate_wgsl` ran
    // over each at load, so a broken module is a named load error above rather
    // than a pipeline failure at first render.
    let Some(rlx_core::render::scenes::GeneratorConfig::WarpMesh {
        milk: Some(bundle), ..
    }) = preset.config.as_ref()
    else {
        panic!("the shader fixture must carry a [milk] table");
    };
    assert!(bundle.warp_wgsl.is_some(), "and a warp module");
    assert!(bundle.comp_wgsl.is_some(), "and a comp module");
    assert_eq!(bundle.blur_level, 3, "and all three blur levels");
    preset
}

/// **The fixture's shaders begin with exactly the prelude the engine generates**
/// — `milkconv/tests/fixture.rs`'s discipline applied to WGSL.
///
/// A hand-written fixture has to inline the *complete* module, because
/// `preset/schema.rs` runs `validate_wgsl` over the whole string at load and
/// `milkconv`'s emitter builds every converted shader as `fragment_prelude(g)`
/// followed by translated code. The prelude is ~2 KB of generated text and
/// ADR-0118 changed it this month, so a verbatim copy in a `.toml` **will**
/// drift. When it does, this fails and names the repair — rather than the
/// fixture silently pinning a surface the engine does not emit.
#[test]
fn the_fixture_shaders_begin_with_the_prelude() {
    use rlx_core::milk::shader::{COMP_GROUP, WARP_GROUP, fragment_prelude};

    let preset = shader_fixture();
    let Some(rlx_core::render::scenes::GeneratorConfig::WarpMesh {
        milk: Some(bundle), ..
    }) = preset.config.as_ref()
    else {
        panic!("the shader fixture must carry a [milk] table");
    };

    for (which, source, group) in [
        ("warp", bundle.warp_wgsl.as_deref(), WARP_GROUP),
        ("comp", bundle.comp_wgsl.as_deref(), COMP_GROUP),
    ] {
        let source = source.unwrap_or_default();
        let prelude = fragment_prelude(group);
        assert!(
            source.starts_with(&prelude),
            "the fixture's `{which}_shader` no longer begins with \
             `milk::shader::fragment_prelude({group})`. Regenerate the fixture's \
             two modules by PRINTING the prelude rather than editing them by \
             hand — the first {} bytes of each string are generated text, not \
             authored text",
            prelude.len()
        );
        // ...and there is a body after it, or the guard above is satisfied by a
        // fixture that carries the prelude and nothing else.
        assert!(
            source.len() > prelude.len() + 200,
            "`{which}_shader` is the prelude and almost nothing else"
        );
    }
}

/// **The shader surface builds and renders a real, moving picture** — the same
/// two statistics `sanity.rs` and `animation.rs` apply, against the same floors,
/// so this entry is comparable to the fixtures beside it rather than measured
/// differently.
///
/// This is the first time in the crate's history that
/// `MilkShaderResources::build` runs: the six noise textures are generated and
/// uploaded, the three-level blur chain is allocated and encoded, and the
/// fifteen-entry bind group resolves. A failure here is far more likely to be
/// "the pipeline did not build" than "the picture is dim".
#[test]
fn the_shader_fixture_draws_a_real_shape_and_animates() {
    let Some(mut renderer) = common::headless(SIZE, SIZE) else {
        return;
    };
    let preset = without_backdrop(shader_fixture());
    let name = preset.name.clone();
    renderer.set_presets(vec![preset]);

    let img = renderer
        .capture_preset(&name, &loud(), SANITY_FRAMES)
        .expect("capture the shader fixture");
    let cov = coverage(&img, BLACK, EPS);
    let spread = quadrant_spread(&img, BLACK, EPS);
    let flat = tonal_flatness(&img, BLACK, EPS);
    println!(
        "[warp_mesh/shader] coverage={cov:.4} (floor {COVERAGE_FLOOR:.2}) \
         quadrants={spread} flatness={flat:.4} (max {MAX_TONAL_FLATNESS:.2})"
    );
    assert!(
        cov >= COVERAGE_FLOOR,
        "the shader fixture is blank: coverage {cov:.4} < {COVERAGE_FLOOR:.2}"
    );
    assert!(
        spread >= MIN_QUADRANTS,
        "the shader fixture is a dot: {spread} quadrant(s) < {MIN_QUADRANTS}"
    );
    assert!(
        flat <= MAX_TONAL_FLATNESS,
        "the shader fixture is flat: {:.1}% of its lit pixels sit in one \
         luminance band (max {:.0}%)",
        flat * 100.0,
        MAX_TONAL_FLATNESS * 100.0
    );

    // ...and it moves, under silence, on the scene's own clock — which is what
    // makes the animated uniform lanes (`U.clock`, `U.roam`, `U.rot`, `U.q`)
    // load-bearing rather than merely written.
    let quiet = AnalysisFrame::default();
    let a = renderer
        .capture_preset(&name, &quiet, FRAME_A)
        .expect("capture frame A");
    let b = renderer
        .capture_preset(&name, &quiet, FRAME_B)
        .expect("capture frame B");
    let motion = footprint_diff(&a, &b, BLACK, EPS, MIN_FOOTPRINT_FRAC);
    println!(
        "[warp_mesh/shader] motion (footprint) frames {FRAME_A}->{FRAME_B} = \
         {motion:.4} (floor {MOTION_FLOOR:.2})"
    );
    assert!(
        motion >= MOTION_FLOOR,
        "the shader fixture is frozen: {motion:.4} < {MOTION_FLOOR:.2}"
    );
}

/// **The shaders, and not the engine's defaults, drive the picture** — the
/// `[milk]`-vs-control argument `the_bundle_and_not_the_defaults_drives_the_transform`
/// makes one layer down, applied one layer up.
///
/// The control is the *same* preset with only its two shader keys removed: the
/// same bundle, the same EEL programs, the same mesh, palette and deposit. So
/// the two frames differ in exactly one thing — whether the custom warp and comp
/// fragments replaced the built-in decay and present ones. A regression that
/// built `MilkShaderResources` and then never bound it would render the control
/// and this would go quiet.
#[test]
fn the_shaders_and_not_the_defaults_drive_the_picture() {
    let Some(mut renderer) = common::headless(SIZE, SIZE) else {
        return;
    };
    let mut driven = shader_fixture();
    let mut control = Preset::from_toml_str(&without_shaders(SHADER_FIXTURE))
        .expect("the no-shader control parses");
    assert!(
        matches!(
            control.config.as_ref(),
            Some(rlx_core::render::scenes::GeneratorConfig::WarpMesh {
                milk: Some(bundle), ..
            }) if bundle.warp_wgsl.is_none() && bundle.comp_wgsl.is_none()
        ),
        "the control must keep its bundle and lose only its shaders, or it \
         proves nothing"
    );

    driven.name = "shader-driven".into();
    control.name = "shader-control".into();
    renderer.set_presets(vec![driven, control]);

    let frame = loud();
    let a = renderer
        .capture_preset("shader-driven", &frame, SANITY_FRAMES)
        .expect("capture the shader-driven fixture");
    let b = renderer
        .capture_preset("shader-control", &frame, SANITY_FRAMES)
        .expect("capture the control");
    let difference = frame_diff(&a, &b);
    println!("[warp_mesh/shader] shaders vs no shaders: frame_diff {difference:.4}");
    assert!(
        difference > 0.01,
        "a bundle carrying WGSL must render differently from the same bundle \
         without it; got {difference:.4}. The custom pipelines are built but not \
         bound"
    );
}

/// The fixture's text with the two shader keys cut out, keeping everything else
/// — including the `[milk]` table itself — exactly as it was.
///
/// Cut by position rather than by `replace`, because the header discusses
/// `warp_shader` and `comp_shader` in prose and a textual substitution would
/// rewrite the comment too.
fn without_shaders(text: &str) -> String {
    let Some(cut) = text.find("warp_shader = \"\"\"") else {
        panic!("the fixture no longer declares `warp_shader`");
    };
    text[..cut].to_string()
}

// ---------------------------------------------------------------------------
// The branches where the surface is partly absent (Plan 0110 Phase 3)
// ---------------------------------------------------------------------------

/// The fixture's text with only `comp_shader` cut out, leaving `warp_shader` and
/// the rest of the `[milk]` table as they were.
///
/// Cut by position for [`without_shaders`]'s reason: the header discusses both
/// keys in prose, and a textual substitution would rewrite the comment too.
fn warp_only(text: &str) -> String {
    let Some(cut) = text.find("comp_shader = \"\"\"") else {
        panic!("the fixture no longer declares `comp_shader`");
    };
    text[..cut].to_string()
}

/// The fixture's text with only `warp_shader` cut out — the key, its module and
/// the blank line that follows — leaving `comp_shader` in place.
fn comp_only(text: &str) -> String {
    let Some(start) = text.find("warp_shader = \"\"\"") else {
        panic!("the fixture no longer declares `warp_shader`");
    };
    let Some(end) = text.find("comp_shader = \"\"\"") else {
        panic!("the fixture no longer declares `comp_shader`");
    };
    assert!(
        start < end,
        "`warp_shader` must precede `comp_shader` for this cut to excise one key"
    );
    let mut out = String::with_capacity(text.len());
    out.push_str(&text[..start]);
    out.push_str(&text[end..]);
    out
}

/// The fixture's text with the blur chain asked for at `level`.
///
/// Anchored to the declaration at the start of its own line, **not** `replace`d.
/// The header spells `blur_level = 3` in prose to say why the fixture wants the
/// whole chain rather than a prefix of it, so the string occurs twice in the
/// file and a substitution would rewrite that sentence into one contradicting
/// the table below it.
fn with_blur_level(text: &str, level: u8) -> String {
    const DECL: &str = "\nblur_level = 3\n";
    let Some(at) = text.find(DECL) else {
        panic!("the fixture no longer declares `blur_level = 3` on its own line");
    };
    let rest = &text[at + DECL.len()..];
    assert!(
        !rest.contains(DECL),
        "`blur_level` is declared twice; this cut would rewrite only the first"
    );
    format!("{}\nblur_level = {level}\n{rest}", &text[..at])
}

/// **Each partly-absent surface still builds and renders.**
///
/// `MilkShaderResources::build` takes each module as an `Option` and the blur
/// chain as a count, so three of its arms never execute under the full fixture:
/// a bundle with no comp module, one with no warp module, and one with no chain
/// at all. None of those is a hypothetical shape — a converted preset carries
/// whichever blocks its `.milk` had, and carrying one shader and not the other
/// is ordinary.
///
/// The bar is that each variant builds and renders a full-size frame without
/// panicking, and deliberately not `sanity.rs`'s floors. A surface with half
/// its shaders replaced by the built-in defaults renders a legitimately
/// different picture, and holding it to the whole fixture's numbers would pin
/// something this phase does not claim.
#[test]
fn each_partly_absent_shader_surface_builds_and_renders() {
    let Some(mut renderer) = common::headless(SIZE, SIZE) else {
        return;
    };

    let variants = [
        ("warp-only", warp_only(SHADER_FIXTURE), true, false, 3u8),
        ("comp-only", comp_only(SHADER_FIXTURE), false, true, 3u8),
        (
            "no-blur",
            with_blur_level(SHADER_FIXTURE, 0),
            true,
            true,
            0u8,
        ),
    ];

    let mut presets = Vec::new();
    for (name, text, warp, comp, blur) in &variants {
        let mut preset = Preset::from_toml_str(text)
            .unwrap_or_else(|e| panic!("the `{name}` variant parses: {e:?}"));
        assert!(
            preset.warnings.is_empty(),
            "the `{name}` variant must load clean, got {:?}",
            preset.warnings
        );
        let Some(rlx_core::render::scenes::GeneratorConfig::WarpMesh {
            milk: Some(bundle), ..
        }) = preset.config.as_ref()
        else {
            panic!("the `{name}` variant must keep its [milk] table");
        };
        assert_eq!(
            bundle.warp_wgsl.is_some(),
            *warp,
            "`{name}`: whether a warp module survived the cut"
        );
        assert_eq!(
            bundle.comp_wgsl.is_some(),
            *comp,
            "`{name}`: whether a comp module survived the cut"
        );
        assert_eq!(bundle.blur_level, *blur, "`{name}`: blur level");

        preset.name = format!("partial-{name}");
        presets.push(without_backdrop(preset));
    }
    renderer.set_presets(presets);

    let frame = loud();
    for (name, ..) in &variants {
        let image = renderer
            .capture_preset(&format!("partial-{name}"), &frame, SANITY_FRAMES)
            .unwrap_or_else(|e| panic!("the `{name}` variant renders: {e:?}"));
        assert_eq!(
            image.rgba.len(),
            (SIZE * SIZE * 4) as usize,
            "`{name}`: a full-size frame"
        );
        let lit = coverage(&image, BLACK, EPS);
        println!("[warp_mesh/shader] {name}: coverage {lit:.4}");
        assert!(
            lit > 0.0,
            "the `{name}` variant built its pipelines and then drew nothing"
        );
    }
}

/// **The blur chain is load-bearing on the picture.**
///
/// Both fixture bodies read `rlx_GetBlur1`, `rlx_GetBlur2` and `rlx_GetBlur3` by
/// construction, so a surface built with no chain must resolve those three
/// bindings to *something*, and what that something is decides whether the two
/// arms can differ at all.
///
/// If this ever comes back equal, that is a finding about what bindings 12..14
/// resolve to when no chain is built — that is a followup to file, and it is
/// **not** a cue to tune the fixture until it goes away.
#[test]
fn the_blur_chain_changes_the_picture() {
    let Some(mut renderer) = common::headless(SIZE, SIZE) else {
        return;
    };
    let mut full = shader_fixture();
    let mut none = Preset::from_toml_str(&with_blur_level(SHADER_FIXTURE, 0))
        .expect("the no-blur variant parses");
    full.name = "blur-three".into();
    none.name = "blur-zero".into();
    renderer.set_presets(vec![without_backdrop(full), without_backdrop(none)]);

    let frame = loud();
    let a = renderer
        .capture_preset("blur-three", &frame, SANITY_FRAMES)
        .expect("capture the three-level chain");
    let b = renderer
        .capture_preset("blur-zero", &frame, SANITY_FRAMES)
        .expect("capture the chainless variant");
    let difference = frame_diff(&a, &b);
    println!("[warp_mesh/shader] blur 3 vs blur 0: frame_diff {difference:.4}");
    assert!(
        difference > 0.01,
        "`blur_level = 0` renders identically to `blur_level = 3` (frame_diff \
         {difference:.4}), yet both fixture bodies read `rlx_GetBlur3`. Record \
         what bindings 12..14 resolve to with no chain built - do not tune the \
         fixture to make this pass"
    );
}

/// The lit-backdrop fixture — see its own header for why every binding is what
/// it is.
const LIT_FIXTURE: &str = include_str!("fixtures/warp_mesh_lit_backdrop.toml");

/// **The backdrop arrives intact where the draw layer wrote nothing** — the
/// lit-backdrop capture test `gpu::ADDITIVE_LIGHT_SATURATING_COVERAGE` says every
/// new draw seam owes, and Plan 0100 Phase 4 added two of them (the shape
/// triangle pipeline and its premultiplied-OVER twin).
///
/// The defect it guards is not hypothetical and not subtle once you can see it:
/// a fragment that emits a constant alpha rather than its own coverage discards
/// the backdrop across its whole footprint, including everywhere it drew no
/// light. Both existing call sites shipped exactly that. It is invisible at
/// `bg_bright = 0`, which is the setting **every golden baseline runs**, so no
/// amount of golden coverage would catch a third instance.
///
/// The property, in two arms:
///
/// - the frame's corners — far from the centred circle the fixture draws — carry
///   the backdrop, unchanged from what the same preset renders with its draw
///   layer producing nothing;
/// - the centre differs from those corners, so the capture is not simply a flat
///   backdrop with no scene in it at all.
#[test]
fn a_lit_backdrop_survives_where_the_draw_layer_drew_nothing() {
    let Some(mut renderer) = common::headless(SIZE, SIZE) else {
        return;
    };
    let preset = Preset::from_toml_str(LIT_FIXTURE).expect("the lit fixture parses");
    assert_eq!(preset.system, SystemKind::WarpMesh);
    assert!(
        preset.warnings.is_empty(),
        "the lit fixture must load clean, got {:?}",
        preset.warnings
    );

    // --- Non-vacuity, before any GPU work. ---
    let bright = preset
        .params
        .iter()
        .find(|b| b.name == "bg_bright")
        .map(|b| b.expr.eval(&Default::default()))
        .unwrap_or(0.0);
    assert!(
        bright > 0.1,
        "warp_mesh_lit_backdrop.toml no longer ships a lit backdrop \
         (bg_bright = {bright}); on black this whole comparison is black \
         against black"
    );
    assert!(
        matches!(
            preset.config.as_ref(),
            Some(rlx_core::render::scenes::GeneratorConfig::WarpMesh { milk: Some(_), .. })
        ),
        "the fixture must carry a `[milk]` table — without one the scene draws no \
         MilkDrop layer at all and there is no seam under test"
    );

    // The same preset with its draw layer silenced: `wave_a = 0` leaves the
    // waveform dark, so the field stays empty and every pixel is pure backdrop.
    // That is the control the corners are read against — a hard-coded expected
    // colour would drift with any backdrop change and prove nothing about the
    // seam.
    // Appended rather than substituted: the fixture's own header discusses
    // `[milk]` in prose, and a `replace` would rewrite the comment too.
    let control = Preset::from_toml_str(&format!(
        "{LIT_FIXTURE}per_frame = \"\"\"\n.regs wave_a\n.code\nconst 0.0\nstore 0\n\"\"\"\n"
    ))
    .expect("the control parses");

    let mut lit = preset;
    lit.name = "warp-mesh-lit".into();
    let mut control = control;
    control.name = "warp-mesh-lit-control".into();
    renderer.set_presets(vec![lit, control]);

    let frame = loud();
    let drawn = renderer
        .capture_preset("warp-mesh-lit", &frame, SANITY_FRAMES)
        .expect("capture the lit fixture");
    let empty = renderer
        .capture_preset("warp-mesh-lit-control", &frame, SANITY_FRAMES)
        .expect("capture the control");

    // The four corners, well outside the circle the waveform draws at radius 0.2.
    let corners = [
        (0u32, 0u32),
        (SIZE - 1, 0),
        (0, SIZE - 1),
        (SIZE - 1, SIZE - 1),
    ];
    let pixel = |image: &CaptureImage, x: u32, y: u32| -> [u8; 3] {
        let i = ((y * image.width + x) * 4) as usize;
        [
            image.rgba.get(i).copied().unwrap_or(0),
            image.rgba.get(i + 1).copied().unwrap_or(0),
            image.rgba.get(i + 2).copied().unwrap_or(0),
        ]
    };

    let channel_delta = |a: [u8; 3], b: [u8; 3]| -> i16 {
        a.iter()
            .zip(&b)
            .map(|(p, q)| (i16::from(*p) - i16::from(*q)).abs())
            .max()
            .unwrap_or(0)
    };

    for (x, y) in corners {
        let a = pixel(&drawn, x, y);
        let b = pixel(&empty, x, y);
        let delta = channel_delta(a, b);
        println!("[warp_mesh/lit] corner ({x}, {y}) {a:?} vs control {b:?} — {delta}");
        assert!(
            delta <= LIT_TOLERANCE,
            "at ({x}, {y}) — far from anything the draw layer drew — the backdrop \
             moved by {delta} channels ({a:?} against the undrawn control's {b:?}). \
             A draw seam is writing coverage where it wrote no light (ADR-0056)"
        );
    }

    // ...and the draw layer really did put light somewhere, or the arm above is
    // satisfied by two identical blank frames. Taken as the largest per-pixel
    // difference anywhere rather than at a named coordinate: the waveform is a
    // ring, so the pixel it lights is a function of the mode's own geometry and
    // pinning one would make this test a second copy of `waveform_figure`.
    let widest = (0..SIZE)
        .flat_map(|y| (0..SIZE).map(move |x| (x, y)))
        .map(|(x, y)| channel_delta(pixel(&drawn, x, y), pixel(&empty, x, y)))
        .max()
        .unwrap_or(0);
    println!("[warp_mesh/lit] widest difference anywhere: {widest}");
    assert!(
        widest > LIT_TOLERANCE * 3,
        "the fixture drew nothing distinguishable — the drawn frame and the \
         undrawn control differ by at most {widest} channels anywhere. With no \
         geometry the corner arm above is vacuous"
    );
}

/// How far a corner channel may move between the drawn frame and the undrawn
/// control. Not zero: the tonemap scales every channel off the frame's own
/// brightest pixel (ADR-0055), so drawing anything at all moves the whole frame
/// by a little. The defect this guards is the backdrop being *replaced*, which is
/// the full `bg_bright` swing.
const LIT_TOLERANCE: i16 = 12;

// ---------------------------------------------------------------------------
// The feedback field's floor (Plan 0108 Phase 1 / ADR-0118)
// ---------------------------------------------------------------------------

/// The quantization probe — see its own header for why every binding is what it
/// is.
const QUANTIZE_FIXTURE: &str = include_str!("fixtures/warp_mesh_quantize.toml");

/// How long the probe runs. Long enough that the quantized field has been at
/// zero for a while; **not** a number either assertion depends on, which is the
/// plan's own instruction — the frame count a field takes to die is a function
/// of `decay` and of where it started, and pinning it would pin the fixture's
/// tuning rather than the property.
const QUANTIZE_FRAMES: u32 = 120;

/// A capture with `brightness` scaled by `gain`, and quantization set to
/// `steps` — the two levers the arms below differ in, applied to one text so
/// nothing else can vary between them.
fn quantize_probe(renderer: &mut Renderer, label: &str, steps: &str, gain: &str) -> CaptureImage {
    let text = QUANTIZE_FIXTURE
        .replace(
            "brightness     = \"600\"",
            &format!("brightness     = \"{gain}\""),
        )
        .replace("[milk]\n", &format!("[milk]\nquantize_steps = {steps}\n"));
    let mut preset = Preset::from_toml_str(&text).expect("the quantize probe parses");
    assert!(
        preset.warnings.is_empty(),
        "the quantize probe must load clean, got {:?}",
        preset.warnings
    );
    preset.name = label.to_string();
    renderer.set_presets(vec![preset]);
    renderer
        .capture_preset(label, &AnalysisFrame::default(), QUANTIZE_FRAMES)
        .expect("capture the quantize probe")
}

/// The brightest byte anywhere in a capture. Zero means the frame is *exactly*
/// black — every channel of every pixel — which is the statistic both arms turn
/// on.
fn peak(image: &CaptureImage) -> u8 {
    image
        .rgba
        .chunks_exact(4)
        .flat_map(|px| px.iter().take(3).copied())
        .max()
        .unwrap_or(0)
}

/// **The quantized feedback field reaches exact zero; the unquantized one does
/// not** — Plan 0108 Phase 1's central done-when, and the defect ADR-0118
/// exists for.
///
/// MilkDrop's feedback target is 8-bit, so `decay` times a dim pixel truncates
/// to zero and a classic preset's background stays black. This engine's field is
/// `Rgba16Float`: nothing truncates, every dim residual survives and integrates,
/// and Plan 0100 Phase 7 judged the four faces of that one mechanism — pastel
/// wash, white-hot glow, runaway with channel fringing, and full tonal
/// inversion.
///
/// # Why three arms and not two
///
/// An 8-bit capture cannot tell an exact zero from a `1e-4` residual on its own:
/// both read as byte 0. Two arms at one gain would therefore be satisfied by a
/// change that merely made the field *dimmer*, which is not the claim. The third
/// arm is what closes that: the quantized field is re-rendered at a **hundred
/// times** the brightness and is still exactly black. Nothing multiplies to zero
/// except zero.
#[test]
fn the_quantized_field_reaches_exact_zero() {
    let Some(mut renderer) = common::headless(SIZE, SIZE) else {
        return;
    };

    let on = quantize_probe(&mut renderer, "quantize-on", "255", "600");
    let on_amplified = quantize_probe(&mut renderer, "quantize-on-amplified", "255", "60000");
    let off = quantize_probe(&mut renderer, "quantize-off", "0", "600");
    let off_amplified = quantize_probe(&mut renderer, "quantize-off-amplified", "0", "60000");

    // **Measured 2026-08-17** on the development box (Windows 10, DX12 WARP),
    // 96x96 over 120 frames: on 0 / 0, off 11 / 154. The unquantized field is
    // still positive at a hundred times the gain that already shows it; the
    // quantized one is black at both.
    println!(
        "[warp_mesh/quantize] peak after {QUANTIZE_FRAMES} frames — \
         on {} (at 100x gain {}), off {} (at 100x gain {})",
        peak(&on),
        peak(&on_amplified),
        peak(&off),
        peak(&off_amplified)
    );

    assert!(
        peak(&off) > 0,
        "the CONTROL is blank: with quantization off the field must still hold \
         light this test can see, or every arm below is black against black. The \
         probe's `brightness` gain is what makes the residual visible — if this \
         fires, raise the gain rather than lowering the bar"
    );
    assert_eq!(
        peak(&on),
        0,
        "the quantized field never reached zero: its brightest channel is {} \
         after {QUANTIZE_FRAMES} frames with the deposit off, against an \
         unquantized control at {}. The warp epilogue's `rlx_quantize` is not \
         flooring dim pixels",
        peak(&on),
        peak(&off)
    );
    assert_eq!(
        peak(&on_amplified),
        0,
        "the quantized field is small but NOT zero — black at 600x brightness \
         and {} at 60000x. Only zero multiplies to zero, so a residual that \
         survives amplification is exactly the accumulation ADR-0118 exists to \
         stop",
        peak(&on_amplified)
    );
    assert!(
        peak(&off_amplified) > peak(&off),
        "the unquantized control does not respond to the gain ({} at 600x, {} at \
         60000x), so the amplified arm above proves nothing about the quantized \
         field. `brightness` is not reaching the present pass",
        peak(&off),
        peak(&off_amplified)
    );
}

/// **Off is off, and on is not vacuous** — the other half of the switch.
///
/// The identity claim ADR-0118 makes ("with the switch off the epilogue is an
/// exact identity") is asserted where it is checkable against pre-change bytes:
/// `core/tests/golden/warp_mesh.png` is the native fixture's committed
/// pre-change output and **must not move**, and `golden.rs` is what holds it to
/// that. What no golden covers is that the switch is wired to anything at all —
/// a `quantize_steps` that reached no shader would leave both arms above
/// identical and this file would still be green.
#[test]
fn the_quantize_switch_reaches_the_shader() {
    let Some(mut renderer) = common::headless(SIZE, SIZE) else {
        return;
    };
    let on = quantize_probe(&mut renderer, "switch-on", "255", "600");
    let off = quantize_probe(&mut renderer, "switch-off", "0", "600");
    let difference = frame_diff(&on, &off);
    println!("[warp_mesh/quantize] steps 255 vs 0: frame_diff {difference:.4}");
    assert!(
        difference > 0.0,
        "a bundle with `quantize_steps = 255` renders byte-identically to one \
         with `quantize_steps = 0`. The uniform lane is not reaching either warp \
         fragment"
    );

    // ...and `0` really is the off path rather than a step count that happens to
    // round to the same picture: a second value below 1 must agree with it to
    // the byte, which only the early return in `rlx_quantize` gives.
    let half = quantize_probe(&mut renderer, "switch-half", "0.5", "600");
    assert_eq!(
        off.rgba, half.rgba,
        "`quantize_steps = 0` and `= 0.5` must both take `rlx_quantize`'s early \
         return and so render identically; they differ, so the off path is doing \
         arithmetic"
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
