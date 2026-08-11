//! The `[layer]` capability's contract (Plan 0076 / ADR-0090): a preset
//! composes a second scene `under` the post chain, through a scene
//! **constructed for the preset** (Phase 2 — never a roster instance).
//!
//! The claims, matching the Phase 1 and Phase 2 done-whens:
//!
//! 1. the layered fixture renders **both** scenes, visibly (it differs from its
//!    layerless control) and **deterministically** (two captures are
//!    byte-identical);
//! 2. a **layer binding reacts to the analysis frame like a top-level one** —
//!    proven differentially: the fixture binding `zoom = "bass"` renders
//!    pixel-identical to a probe hard-coding the same value, and different from
//!    a probe hard-coding another, all under one frame, so the only degree of
//!    freedom is the binding's evaluation;
//! 3. **same-system pairs are legal and independent** (Phase 2): two fragment
//!    fields hold two live parameter states, two swarms hold two seeded
//!    simulations, and a line-on-line pair draws through two `LineRenderer`s;
//! 4. the `[layer]` grammar's load-time surface holds: `join` and `blend` are
//!    closed rosters, `blend` on an `under` join warns as ignored, and an
//!    unknown layer param warns like a top-level one (ADR-0020).
//!
//! A separate test binary in `background_composite.rs`'s posture: one file, one
//! process, so the fragment-field + swarm coexistence these captures build
//! never shares a device with the golden baselines (building GPU resources
//! mid-run is documented to shift what WARP resolves — Plan 0053's standing
//! rule).

use lmv_core::dsp::AnalysisFrame;
use lmv_core::preset::Preset;
use lmv_core::render::{HeadlessOptions, RenderError, Renderer, metrics::frame_diff};

const WIDTH: u32 = 160;
const HEIGHT: u32 = 100;
/// Enough warm-up for the swarm layer's particles to spread into a visible
/// cloud rather than the seed cluster.
const FRAMES: u32 = 40;

const LAYERED: &str = include_str!("fixtures/layer_under.toml");
const CONTROL: &str = include_str!("fixtures/layer_under_control.toml");

/// The fixed frame the captures run under. `bass = 0.9` on purpose: the layered
/// fixture binds the layer's `zoom` to `bass`, and the reactivity probe below
/// hard-codes `0.9` to meet it.
fn fixed_frame() -> AnalysisFrame {
    AnalysisFrame {
        bass: 0.9,
        mid: 0.5,
        treb: 0.6,
        onset: 0.4,
        bar: 0.25,
        ..Default::default()
    }
}

/// A headless renderer on the software adapter (reproducible rasterization),
/// or a logged skip where the runner has no adapter at all (ADR-0016).
fn headless() -> Option<Renderer> {
    match Renderer::new_headless(HeadlessOptions {
        width: WIDTH,
        height: HEIGHT,
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

/// The default-adapter twin of [`headless`], for the one assertion WARP cannot
/// host. A same-system pair duplicates pipelines with **byte-identical bind
/// layouts**, and WARP is documented to alias those (ADR-0058 / Plan 0053):
/// measured here, the layer instance's uniform wins and the main's becomes a
/// dead lever — on the software adapter only; hardware renders both. So this
/// takes `background_composite.rs`'s posture: the guard runs on developer
/// machines (hardware adapter) and skips with notice on a software-only
/// runner, rather than asserting on an adapter that mis-renders the shape
/// under test.
fn headless_hardware() -> Option<Renderer> {
    match Renderer::new_headless(HeadlessOptions {
        width: WIDTH,
        height: HEIGHT,
        prefer_software: false,
    }) {
        Ok(r) if r.adapter_is_software() => {
            eprintln!(
                "skipped: only a software adapter available, and WARP aliases the \
                 identical pipeline layouts a same-system pair duplicates (ADR-0058)"
            );
            None
        }
        Ok(r) => Some(r),
        Err(RenderError::RequestAdapter(_)) => {
            eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
            None
        }
        Err(e) => panic!("headless renderer build failed: {e}"),
    }
}

/// The layered fixture with its layer `zoom` binding replaced by a literal —
/// the reactivity probe's control arm.
fn probe(zoom_literal: &str) -> String {
    LAYERED.replace("zoom = \"bass\"", &format!("zoom = \"{zoom_literal}\""))
}

fn load(toml: &str) -> Preset {
    Preset::from_toml_str(toml).unwrap_or_else(|e| panic!("fixture failed to load: {e}"))
}

/// Done-when 1: both scenes render, visibly and deterministically.
#[test]
fn a_layered_preset_renders_both_scenes_deterministically() {
    let Some(mut renderer) = headless() else {
        return;
    };
    let frame = fixed_frame();

    let layered = load(LAYERED);
    assert!(
        layered.warnings.is_empty(),
        "clean fixture: {:?}",
        layered.warnings
    );
    let control = load(CONTROL);
    renderer.set_presets(vec![layered, control]);

    let with_layer = renderer
        .capture_preset("layer_under", &frame, FRAMES)
        .expect("capture layered fixture");
    let again = renderer
        .capture_preset("layer_under", &frame, FRAMES)
        .expect("capture layered fixture again");
    assert_eq!(
        with_layer.rgba, again.rgba,
        "a layered capture is a pure function of its inputs (NFR 6)"
    );

    let without = renderer
        .capture_preset("layer_under_control", &frame, FRAMES)
        .expect("capture layerless control");
    let diff = frame_diff(&without, &with_layer);
    assert!(
        diff > 0.001,
        "the layer must visibly contribute over the layerless control \
         (mean diff {diff:.5} is indistinguishable from none)"
    );
}

/// Done-when 4 (the reactivity half): a layer binding is evaluated from the
/// analysis frame exactly as a top-level binding is.
///
/// Differential, so it holds regardless of how either scene reacts to audio
/// intrinsically: every capture here runs under the **same** frame, so the only
/// thing that can distinguish them is the value the layer's `zoom` binding
/// evaluated to.
#[test]
fn a_layer_binding_reacts_to_the_analysis_frame() {
    let Some(mut renderer) = headless() else {
        return;
    };
    // `bass = 0.9` — what the fixture's `zoom = "bass"` should evaluate to.
    let frame = fixed_frame();

    let mut bound = load(LAYERED);
    bound.name = "bound".into();
    let mut matching = load(&probe("0.9"));
    matching.name = "matching".into();
    let mut differing = load(&probe("0.2"));
    differing.name = "differing".into();
    renderer.set_presets(vec![bound, matching, differing]);

    let bound = renderer
        .capture_preset("bound", &frame, FRAMES)
        .expect("capture bound fixture");
    let matching = renderer
        .capture_preset("matching", &frame, FRAMES)
        .expect("capture matching probe");
    let differing = renderer
        .capture_preset("differing", &frame, FRAMES)
        .expect("capture differing probe");

    assert_eq!(
        bound.rgba, matching.rgba,
        "`zoom = \"bass\"` under bass = 0.9 must render exactly as `zoom = \"0.9\"` — \
         the layer binding is evaluated from the analysis frame"
    );
    let diff = frame_diff(&bound, &differing);
    assert!(
        diff > 0.001,
        "`zoom = \"0.2\"` must render differently under the same frame (mean diff \
         {diff:.5}), or the equality above is vacuous"
    );
}

/// Phase 2's headline: the same system twice is a legal pair whose two
/// instances hold **independent, simultaneously-live states**. Two swarms —
/// an additive family, so both stay visible through the shared target: moving
/// only the layer's zoom moves the picture, and so does moving only the
/// main's. With one shared instance, one of the two would be a dead lever
/// (last write wins). Deterministic throughout, so the duplicated particle
/// simulation is seeded like the roster's.
///
/// On the hardware adapter (`headless_hardware`) because the "main is live
/// too" half is exactly what WARP's layout aliasing breaks.
#[test]
fn a_same_system_pair_renders_two_independent_configurations() {
    let Some(mut renderer) = headless_hardware() else {
        return;
    };
    let frame = fixed_frame();

    let pair = |name: &str, main_zoom: f32, layer_zoom: f32| {
        format!(
            "name = \"{name}\"\nsystem = \"swarm\"\n\
             [params]\nzoom = \"{main_zoom}\"\nsize = \"1.5\"\n\
             [layer]\nsystem = \"swarm\"\n\
             [layer.params]\nzoom = \"{layer_zoom}\"\nsize = \"3.0\"\nbrightness = \"1.6\"\n"
        )
    };
    renderer.set_presets(vec![
        load(&pair("both", 0.4, 1.4)),
        load(&pair("layer_moved", 0.4, 0.8)),
        load(&pair("main_moved", 1.0, 1.4)),
        load(
            "name = \"one_swarm\"\nsystem = \"swarm\"\n\
             [params]\nzoom = \"0.4\"\nsize = \"1.5\"\n",
        ),
    ]);

    let both = renderer
        .capture_preset("both", &frame, FRAMES)
        .expect("capture same-system pair");
    let again = renderer
        .capture_preset("both", &frame, FRAMES)
        .expect("capture same-system pair again");
    assert_eq!(
        both.rgba, again.rgba,
        "two simulations of one system, still a pure function of the inputs"
    );

    let alone = renderer
        .capture_preset("one_swarm", &frame, FRAMES)
        .expect("capture single swarm");
    let visible = frame_diff(&alone, &both);
    assert!(
        visible > 0.001,
        "the second simulation must visibly contribute (diff {visible:.5})"
    );

    let layer_moved = renderer
        .capture_preset("layer_moved", &frame, FRAMES)
        .expect("capture layer-moved variant");
    let main_moved = renderer
        .capture_preset("main_moved", &frame, FRAMES)
        .expect("capture main-moved variant");
    let layer_diff = frame_diff(&both, &layer_moved);
    let main_diff = frame_diff(&both, &main_moved);
    assert!(
        layer_diff > 0.001,
        "moving only the layer's zoom must move the picture (diff {layer_diff:.5})"
    );
    assert!(
        main_diff > 0.001,
        "moving only the main's zoom must move the picture (diff {main_diff:.5})"
    );
}

/// The plan's named fixture — two fragment fields at different zooms — is
/// legal and deterministic, and the layer instance's configuration is the
/// live one.
///
/// **What this deliberately does not assert**: that the *main* field stays
/// visible. A fragment field presents premultiplied with full coverage, so an
/// `under` layer of the same shape occludes the main scene entirely — real
/// composite semantics, not a defect: `under` is the sparse-over-dense idiom,
/// and a fullscreen-over-fullscreen pair is what Phase 3's `over` join and
/// blend modes exist for. Recorded here so the Phase 5 judgement and the
/// authoring docs inherit the finding rather than rediscover it.
#[test]
fn a_fragment_pair_is_legal_and_the_layer_config_is_live() {
    let Some(mut renderer) = headless() else {
        return;
    };
    let frame = fixed_frame();

    let pair = |name: &str, layer_zoom: f32| {
        format!(
            "name = \"{name}\"\nsystem = \"fragment_field\"\n\
             [params]\nwarp = \"0.35\"\nhue = \"0.1\"\nzoom = \"0.4\"\n\
             [layer]\nsystem = \"fragment_field\"\n\
             [layer.params]\nwarp = \"0.6\"\nhue = \"0.7\"\nzoom = \"{layer_zoom}\"\n"
        )
    };
    renderer.set_presets(vec![
        load(&pair("fields", 1.6)),
        load(&pair("fields_layer_moved", 0.9)),
    ]);

    let fields = renderer
        .capture_preset("fields", &frame, FRAMES)
        .expect("capture fragment pair");
    let again = renderer
        .capture_preset("fields", &frame, FRAMES)
        .expect("capture fragment pair again");
    assert_eq!(fields.rgba, again.rgba, "same-system pair is deterministic");

    let moved = renderer
        .capture_preset("fields_layer_moved", &frame, FRAMES)
        .expect("capture layer-moved variant");
    let diff = frame_diff(&fields, &moved);
    assert!(
        diff > 0.001,
        "the layer instance's own zoom must drive the visible field (diff {diff:.5})"
    );
}

/// A line-on-line pair draws through **two** `LineRenderer`s — the roster's
/// shared one for the main scene, the layer's own for the layer (the Phase 2
/// discovery, recorded in `scenes::create_layer_scene`'s docs). The pixel
/// claims here are weak by design: WARP's sensitivity to coexisting identical
/// pipeline layouts (ADR-0058) means this pair's *look* is judged on hardware
/// in Phase 5, not asserted on the software adapter.
#[test]
fn a_line_on_line_pair_draws_through_two_renderers() {
    let Some(mut renderer) = headless() else {
        return;
    };
    // The spectrum layer draws its elements from the frame's band array, and
    // the band levels of `fixed_frame` default to zero — bars of zero height
    // draw nothing. Light the bins so the layer has a figure.
    let mut frame = fixed_frame();
    frame.spectrum = [0.5; lmv_core::dsp::SPECTRUM_BINS];

    let toml = "name = \"curve_over_spectrum\"\nsystem = \"parametric_curve\"\n\
                [params]\nn = \"5\"\nd = \"71\"\n\
                [layer]\nsystem = \"spectrum\"\n\
                [layer.params]\nscale = \"0.7\"\nthickness = \"3\"\nhue = \"0.6\"\n";
    let control = "name = \"curve_alone\"\nsystem = \"parametric_curve\"\n\
                   [params]\nn = \"5\"\nd = \"71\"\n";
    renderer.set_presets(vec![load(toml), load(control)]);

    let pair = renderer
        .capture_preset("curve_over_spectrum", &frame, FRAMES)
        .expect("capture line-on-line pair");
    let again = renderer
        .capture_preset("curve_over_spectrum", &frame, FRAMES)
        .expect("capture line-on-line pair again");
    assert_eq!(
        pair.rgba, again.rgba,
        "two line renderers, still deterministic"
    );

    let alone = renderer
        .capture_preset("curve_alone", &frame, FRAMES)
        .expect("capture curve alone");
    let diff = frame_diff(&alone, &pair);
    assert!(
        diff > 0.001,
        "the layered spectrum must visibly contribute (diff {diff:.5})"
    );
}

/// Phase 3's skippability property: at `mix = 0` the `over` junction is
/// pixel-identical to the layerless preset — with no stage active (the blend's
/// chain input is surface-sized, so the main scene rasterizes exactly as it
/// would straight into the destination) **and** with bloom active (the chain
/// input takes bloom's internal grid, so the main content resamples exactly as
/// a layerless frame would before folding).
#[test]
fn an_over_layer_at_mix_zero_is_pixel_identical_to_layerless() {
    let Some(mut renderer) = headless() else {
        return;
    };
    let frame = fixed_frame();

    let over = |name: &str, extra: &str| {
        format!(
            "name = \"{name}\"\nsystem = \"fragment_field\"\n\
             [params]\nwarp = \"0.35\"\nhue = \"0.1\"\nzoom = \"0.6\"\n{extra}\
             [layer]\nsystem = \"swarm\"\njoin = \"over\"\nmix = \"0\"\n\
             [layer.params]\nsize = \"2.0\"\nbrightness = \"1.4\"\n"
        )
    };
    let control = |name: &str, extra: &str| {
        format!(
            "name = \"{name}\"\nsystem = \"fragment_field\"\n\
             [params]\nwarp = \"0.35\"\nhue = \"0.1\"\nzoom = \"0.6\"\n{extra}"
        )
    };
    renderer.set_presets(vec![
        load(&over("over_zero", "")),
        load(&control("plain", "")),
        load(&over("over_zero_bloom", "bloom_amount = \"0.5\"\n")),
        load(&control("plain_bloom", "bloom_amount = \"0.5\"\n")),
    ]);

    let over_zero = renderer
        .capture_preset("over_zero", &frame, FRAMES)
        .expect("capture over layer at mix 0");
    let plain = renderer
        .capture_preset("plain", &frame, FRAMES)
        .expect("capture layerless control");
    assert_eq!(
        over_zero.rgba, plain.rgba,
        "mix = 0 with no stage active must be byte-identical to the layerless preset"
    );

    let over_bloom = renderer
        .capture_preset("over_zero_bloom", &frame, FRAMES)
        .expect("capture over layer at mix 0 with bloom");
    let plain_bloom = renderer
        .capture_preset("plain_bloom", &frame, FRAMES)
        .expect("capture layerless bloom control");
    assert_eq!(
        over_bloom.rgba, plain_bloom.rgba,
        "mix = 0 through bloom's grid must be byte-identical to the layerless preset"
    );
}

/// Phase 3's four blend modes, pairwise distinct on one fixture pair — the
/// GPU-checkable half of the done-when (the judged filmstrip is Phase 5's).
/// One selector in one pipeline, so no mode adds a pipeline the others do not
/// (the WARP pipeline-count posture, Plan 0046's precedent).
#[test]
fn the_four_blend_modes_render_distinct_results() {
    let Some(mut renderer) = headless() else {
        return;
    };
    let frame = fixed_frame();

    const MODES: [&str; 4] = ["add", "screen", "multiply", "overlay"];
    let fixture = |mode: &str| {
        format!(
            "name = \"{mode}\"\nsystem = \"fragment_field\"\n\
             [params]\nwarp = \"0.35\"\nhue = \"0.1\"\nzoom = \"0.6\"\n\
             [layer]\nsystem = \"swarm\"\njoin = \"over\"\nblend = \"{mode}\"\n\
             [layer.params]\nsize = \"3.0\"\nbrightness = \"1.6\"\n"
        )
    };
    renderer.set_presets(MODES.iter().map(|m| load(&fixture(m))).collect());

    let captures: Vec<_> = MODES
        .iter()
        .map(|mode| {
            renderer
                .capture_preset(mode, &frame, FRAMES)
                .unwrap_or_else(|e| panic!("capture blend mode {mode}: {e}"))
        })
        .collect();

    for (i, a) in captures.iter().enumerate() {
        for (j, b) in captures.iter().enumerate().skip(i + 1) {
            let diff = frame_diff(a, b);
            assert!(
                diff > 0.0005,
                "blend modes '{}' and '{}' render indistinguishably (diff {diff:.5}) — \
                 a selector that ignores its mode slot would pass every per-mode \
                 capture and fail here",
                MODES[i],
                MODES[j]
            );
        }
    }
}

/// The junction's placement — pre-bloom, so a crisp `over` figure still glows
/// (ADR-0090): with bloom active, the layer visibly contributes through it,
/// and the whole path stays deterministic.
#[test]
fn an_over_layer_participates_in_bloom() {
    let Some(mut renderer) = headless() else {
        return;
    };
    let frame = fixed_frame();

    let toml = "name = \"over_bloom\"\nsystem = \"fragment_field\"\n\
                [params]\nwarp = \"0.35\"\nhue = \"0.1\"\nzoom = \"0.6\"\n\
                bloom_amount = \"0.6\"\n\
                [layer]\nsystem = \"swarm\"\njoin = \"over\"\nblend = \"add\"\n\
                [layer.params]\nsize = \"3.0\"\nbrightness = \"1.8\"\n";
    let control = "name = \"bloom_alone\"\nsystem = \"fragment_field\"\n\
                   [params]\nwarp = \"0.35\"\nhue = \"0.1\"\nzoom = \"0.6\"\n\
                   bloom_amount = \"0.6\"\n";
    renderer.set_presets(vec![load(toml), load(control)]);

    let layered = renderer
        .capture_preset("over_bloom", &frame, FRAMES)
        .expect("capture over layer through bloom");
    let again = renderer
        .capture_preset("over_bloom", &frame, FRAMES)
        .expect("capture over layer through bloom again");
    assert_eq!(layered.rgba, again.rgba, "the junction is deterministic");

    let alone = renderer
        .capture_preset("bloom_alone", &frame, FRAMES)
        .expect("capture bloom-only control");
    let diff = frame_diff(&alone, &layered);
    assert!(
        diff > 0.001,
        "the over layer must visibly contribute through bloom (diff {diff:.5})"
    );
}

/// Done-when 4 (the grammar half): the `[layer]` table's load-time validation
/// and warnings.
#[test]
fn the_layer_grammar_validates_at_load() {
    // `join` is a closed roster...
    let err = Preset::from_toml_str(
        "system = \"fragment_field\"\n[layer]\nsystem = \"swarm\"\njoin = \"beside\"\n",
    )
    .err()
    .map(|e| e.to_string())
    .unwrap_or_default();
    assert!(err.contains("unknown [layer] join"), "got: {err}");

    // ...and `over` is a legal join (Phase 3): it loads, with its blend and
    // mix parsed and no warning.
    let over = Preset::from_toml_str(
        "system = \"fragment_field\"\n[layer]\nsystem = \"swarm\"\njoin = \"over\"\n\
         blend = \"multiply\"\nmix = \"0.5 + 0.5 * onset\"\n",
    )
    .expect("an over-join layer loads");
    assert!(
        over.warnings.is_empty(),
        "a clean over-join layer warns nothing: {:?}",
        over.warnings
    );

    // `blend` is a closed roster.
    let err = Preset::from_toml_str(
        "system = \"fragment_field\"\n[layer]\nsystem = \"swarm\"\nblend = \"burn\"\n",
    )
    .err()
    .map(|e| e.to_string())
    .unwrap_or_default();
    assert!(err.contains("unknown [layer] blend"), "got: {err}");

    // `blend` on an `under` join loads but warns as ignored.
    let preset = Preset::from_toml_str(
        "system = \"fragment_field\"\n[layer]\nsystem = \"swarm\"\nblend = \"screen\"\n",
    )
    .expect("blend on under loads");
    assert!(
        preset
            .warnings
            .iter()
            .any(|w| w.contains("ignored on an under join")),
        "warnings: {:?}",
        preset.warnings
    );

    // An unknown layer param warns and keeps the binding (ADR-0020)...
    let preset = Preset::from_toml_str(
        "system = \"fragment_field\"\n[layer]\nsystem = \"swarm\"\n\
         [layer.params]\nwobble = \"1\"\n",
    )
    .expect("unknown layer param still loads");
    assert!(
        preset
            .warnings
            .iter()
            .any(|w| w.contains("unknown [layer] parameter 'wobble'")),
        "warnings: {:?}",
        preset.warnings
    );

    // ...and a compositing name in the layer's params gets the sharper message:
    // layers bind only their own scene's params.
    let preset = Preset::from_toml_str(
        "system = \"fragment_field\"\n[layer]\nsystem = \"swarm\"\n\
         [layer.params]\ntrails = \"0.5\"\n",
    )
    .expect("global name in layer params still loads");
    assert!(
        preset
            .warnings
            .iter()
            .any(|w| w.contains("compositing parameter")),
        "warnings: {:?}",
        preset.warnings
    );
}
