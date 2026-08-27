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

use std::path::{Path, PathBuf};

use lmv_core::dsp::AnalysisFrame;
use lmv_core::preset::Preset;
use lmv_core::render::{CaptureImage, HeadlessOptions, RenderError, Renderer, metrics::frame_diff};

const WIDTH: u32 = 160;
const HEIGHT: u32 = 100;
/// Enough warm-up for the swarm layer's particles to spread into a visible
/// cloud rather than the seed cluster.
const FRAMES: u32 = 40;

const LAYERED: &str = include_str!("fixtures/layer_under.toml");
const CONTROL: &str = include_str!("fixtures/layer_under_control.toml");
const OVER: &str = include_str!("fixtures/layer_over.toml");
/// Plan 0091 Phase 1's two-tone fixture — a dark figure on a light ground,
/// which is the shape design-backlog 0069 said was unreachable.
const MULTIPLY: &str = include_str!("fixtures/layer_multiply.toml");

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

/// The canonical fixture — two fragment fields at different zooms — is
/// legal and deterministic, and the layer instance's configuration is
/// the live one.
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

/// Rec. 601 luma of a **display-referred** capture, in `0..255` — ADR-0106's
/// units, so the numbers this file prints are directly comparable with the table
/// that ADR records. Returns `(min, mean, max)`.
///
/// Display bytes rather than linear light on purpose: the question these probes
/// answer is "how dark does the frame a viewer sees actually get", and the
/// tonemap sits between the composite and that frame.
fn luma_stats(img: &CaptureImage) -> (f32, f32, f32) {
    let mut min = f32::INFINITY;
    let mut max = f32::NEG_INFINITY;
    let mut sum = 0.0f64;
    let mut n = 0u64;
    for px in img.rgba.chunks_exact(4) {
        let (r, g, b) = (f32::from(px[0]), f32::from(px[1]), f32::from(px[2]));
        let luma = 0.299 * r + 0.587 * g + 0.114 * b;
        min = min.min(luma);
        max = max.max(luma);
        sum += f64::from(luma);
        n += 1;
    }
    (min, sum as f32 / n.max(1) as f32, max)
}

/// **Plan 0091 Phase 1 — the one thing ADR-0106 left unmeasured.**
///
/// That ADR measured a `multiply` layer over the *chain* and found it reaches
/// display luma 18.5 where the additive control cannot go below 181.6. It ran at
/// `bg_bright = 0`, so it establishes the layer-over-chain path only, and
/// `post.rs`'s module docs say the backdrop is a genuinely separate path: **it is
/// not in the chain's input.** It is painted into the chain's *destination*, and
/// the chain composites premultiplied-over it.
///
/// So this probe varies `blend` over a **lit** backdrop, in that ADR's shape, and
/// reports the luma minimum for both. It runs on the hardware adapter for
/// `headless_hardware`'s reason: a fragment-on-fragment pair duplicates
/// byte-identical bind-group layouts, which WARP aliases (ADR-0058).
#[test]
fn a_multiply_layer_meets_a_lit_backdrop() {
    let Some(mut renderer) = headless_hardware() else {
        return;
    };
    let frame = fixed_frame();

    // ADR-0106's construction, with `bg_bright` and `occlude` promoted to
    // variables: a fullscreen field pinned flat by `color_span = 0` as the
    // chain, the same system sampling the whole gradient at `palette_steps = 5`
    // as an `over` layer at `mix = 1`.
    let probe = |name: &str, bg: &str, occlude: &str, blend: Option<&str>| {
        let layer = match blend {
            Some(mode) => format!(
                "[layer]\nsystem = \"fragment_field\"\njoin = \"over\"\n\
                 blend = \"{mode}\"\nmix = \"1\"\n\
                 [layer.params]\ncolor_span = \"1\"\npalette_steps = \"5\"\n"
            ),
            None => String::new(),
        };
        format!(
            "name = \"{name}\"\nsystem = \"fragment_field\"\n\
             [params]\ncolor_span = \"0\"\nwarp = \"0.35\"\nzoom = \"0.6\"\n\
             trails = \"0.25\"\n\
             bg_bright = \"{bg}\"\nbg_vignette = \"0\"\nbg_hue = \"0.55\"\n\
             occlude = \"{occlude}\"\n{layer}"
        )
    };
    // Every arm binds a post stage, and that is not decoration: with an empty
    // chain a fullscreen field draws `REPLACE` straight into the destination and
    // covers the backdrop whatever `occlude` says (`fragment_field.rs`'s own
    // note at its `LoadOp::Load`). The backdrop path this phase is about only
    // exists when the chain folds. Same precedent as the three lit-backdrop
    // draw-seam guards, which all bind `trails` for the same structural reason.

    // Three configurations x {no layer, multiply, add}. The dark-backdrop row
    // reproduces ADR-0106; the two lit rows are what this phase is for.
    const LIT: &str = "0.85";
    let arms: Vec<(String, String)> = [
        ("dark_bg", "0", "1"),
        ("lit_bg_occluding", LIT, "1"),
        ("lit_bg_visible", LIT, "0"),
    ]
    .iter()
    .flat_map(|(row, bg, occ)| {
        [None, Some("multiply"), Some("add")]
            .into_iter()
            .map(move |blend| {
                let name = match blend {
                    Some(mode) => format!("{row}_{mode}"),
                    None => format!("{row}_plain"),
                };
                (name.clone(), probe(&name, bg, occ, blend))
            })
    })
    .collect();

    renderer.set_presets(arms.iter().map(|(_, toml)| load(toml)).collect());

    let mut captures = Vec::new();
    println!("{:<26} {:>8} {:>8} {:>8}", "arm", "min", "mean", "max");
    for (name, _) in &arms {
        let img = renderer
            .capture_preset(name, &frame, FRAMES)
            .unwrap_or_else(|e| panic!("capture {name}: {e}"));
        let (min, mean, max) = luma_stats(&img);
        println!("{name:<26} {min:>8.1} {mean:>8.1} {max:>8.1}");
        captures.push((name.clone(), img, min));
    }

    let arm = |name: &str| -> &(String, CaptureImage, f32) {
        captures
            .iter()
            .find(|(n, _, _)| n == name)
            .unwrap_or_else(|| panic!("missing arm {name}"))
    };

    // 1. The dark-backdrop row reproduces ADR-0106's separation: the multiply
    //    goes far below the chain it darkens, where the additive control only
    //    goes up. This is the control on the harness, not the finding.
    let dark_plain = arm("dark_bg_plain").2;
    let dark_multiply = arm("dark_bg_multiply").2;
    let dark_add = arm("dark_bg_add").2;
    assert!(
        dark_multiply < 0.5 * dark_plain && dark_add > dark_plain,
        "over a black backdrop the multiply layer must darken the chain and the \
         additive control must not (plain {dark_plain:.1}, multiply \
         {dark_multiply:.1}, add {dark_add:.1}) — ADR-0106's measurement"
    );

    // 2. At the default `occlude = 1` the frame is byte-identical to the same
    //    preset over a *black* backdrop, layer or no layer. The backdrop is not
    //    darkened by the multiply; it is **absent**, held out by coverage
    //    (`layer_blend.rs`'s union `alpha = a.a + cov * (1 - a.a)`, and the
    //    chain's own opaque fold before that).
    for row in ["plain", "multiply", "add"] {
        let lit = &arm(&format!("lit_bg_occluding_{row}")).1;
        let dark = &arm(&format!("dark_bg_{row}")).1;
        assert_eq!(
            lit.rgba, dark.rgba,
            "at occlude = 1 the `{row}` arm must render identically over a lit and \
             a black backdrop — an opaque composite removes the backdrop rather \
             than blending with it"
        );
    }
    // ...and with `occlude = 0` the backdrop emphatically does reach the frame,
    // so the identity above is a statement about coverage rather than about an
    // inert `bg_bright`.
    assert_ne!(
        arm("lit_bg_visible_plain").1.rgba,
        arm("dark_bg_plain").1.rgba,
        "at occlude = 0 the lit backdrop must reach the frame"
    );

    // 3. **The finding.** With the backdrop visible the composite is
    //    `blended_chain + backdrop * (1 - alpha)` — the backdrop is added
    //    *after* the junction, so no blend mode can reach it. The multiply still
    //    darkens the chain's own contribution, but the backdrop's light is a
    //    floor underneath it: 18.9 over black against 171.3 over a lit sky on
    //    this box, where the backdrop alone reads 196.9.
    let visible_plain = arm("lit_bg_visible_plain").2;
    let visible_multiply = arm("lit_bg_visible_multiply").2;
    println!(
        "lit + visible: plain {visible_plain:.1}, multiply {visible_multiply:.1}; \
         the same multiply over black reaches {dark_multiply:.1}"
    );
    assert!(
        visible_multiply > 0.75 * visible_plain,
        "a multiply layer must not be able to darken a *visible* backdrop \
         (backdrop-only {visible_plain:.1}, with the multiply layer \
         {visible_multiply:.1}) — if this ever fails, the backdrop has entered \
         the junction's input and `docs/preset-palettes.md`'s two-tone route is \
         wrong about where the light ground has to come from"
    );
    assert!(
        visible_multiply > 4.0 * dark_multiply,
        "the same multiply layer reaches {dark_multiply:.1} over black and only \
         {visible_multiply:.1} over the lit backdrop — the gap *is* the backdrop's \
         own light, arriving underneath the blend"
    );
}

/// The **trap** the two-tone route has to name, measured rather than asserted
/// from the shader (Plan 0091 Phase 1).
///
/// ADR-0106 states that a particle scene in a `multiply` slot "cannot darken,
/// because its alpha *is* its brightness" — so a `swarm` heart and a field heart
/// would have different colour capabilities. `swarm.rs`'s fragment stage says
/// something narrower: it emits `vec4(color * g, g)`, where `g` is the mark's
/// **geometric** falloff and `color` is its palette colour. Those are independent,
/// and `layer_blend.rs` un-premultiplies (`straight = b.rgb / max(b.a, 1e-4)`)
/// before the mode runs — so a *dark* particle has full coverage and a dark
/// operand, which is the darkening condition met.
///
/// This probe settles which description holds, because the docs are about to
/// teach one of them.
#[test]
fn a_dark_particle_layer_in_a_multiply_slot() {
    let Some(mut renderer) = headless() else {
        return;
    };
    let frame = fixed_frame();

    // A flat, bright chain to darken, and a sparse frozen swarm of large marks
    // as the layer. `brightness` is the only variable between the two layered
    // arms: it scales the particle's colour and *not* its falloff.
    let probe = |name: &str, layer: Option<&str>| {
        let layer = match layer {
            Some(brightness) => format!(
                "[layer]\nsystem = \"swarm\"\njoin = \"over\"\n\
                 blend = \"multiply\"\nmix = \"1\"\n\
                 [layer.params]\nforce = \"0\"\nspin = \"0\"\nburst = \"0\"\n\
                 size = \"6.0\"\nbrightness = \"{brightness}\"\n"
            ),
            None => String::new(),
        };
        format!(
            "name = \"{name}\"\nsystem = \"fragment_field\"\n\
             [params]\ncolor_span = \"0\"\ncolor_center = \"0.75\"\nglow = \"1.2\"\n\
             warp = \"0\"\n{layer}"
        )
    };
    renderer.set_presets(vec![
        load(&probe("chain_only", None)),
        load(&probe("dark_marks", Some("0.0"))),
        load(&probe("bright_marks", Some("1.6"))),
    ]);

    let mut mins = Vec::new();
    for name in ["chain_only", "dark_marks", "bright_marks"] {
        let img = renderer
            .capture_preset(name, &frame, FRAMES)
            .unwrap_or_else(|e| panic!("capture {name}: {e}"));
        let (min, mean, max) = luma_stats(&img);
        println!("{name:<14} min {min:>7.1} mean {mean:>7.1} max {max:>7.1}");
        mins.push(min);
    }
    let (chain_only, dark_marks) = (mins[0], mins[1]);

    assert!(
        dark_marks < 0.5 * chain_only,
        "a swarm layer at brightness = 0 must darken the chain it multiplies \
         (chain alone {chain_only:.1}, with the dark marks {dark_marks:.1}) — a \
         particle's alpha is its falloff, which is geometry, and the blend \
         un-premultiplies before it takes the mode, so the mark's *colour* is the \
         darkening operand"
    );
}

/// The three frozen golden fixtures (Plan 0076 Phase 5 and Plan 0091 Phase 1,
/// ADR-0023), each pinned to a committed baseline PNG within `composite.rs`'s
/// tolerances. `layer_under` is the Phase 1 walking-skeleton fixture;
/// `layer_over` runs the junction's richest path (layer offscreen -> `screen`
/// blend -> bloom's grid -> fold); `layer_multiply` pins the **two-tone**
/// capability — a dark figure on a light ground, which is the one thing in this
/// suite that would notice the darkening modes regressing into additive ones.
///
/// `LMV_BLESS=1 cargo test -p lmv-core --test layer` rewrites these two — and,
/// run against the whole suite instead of this one binary, **every other
/// baseline as well**. Bless by `--test layer` and check `git status`. Both
/// baselines were adapter-compared before blessing (the ADR-0058 standing
/// rule): the WARP capture agrees with the hardware adapter's within the
/// cross-rasterizer tolerance, so the baseline pins a picture hardware also
/// draws, not a WARP artifact.
#[test]
fn layered_fixtures_match_golden_baselines() {
    const MEAN_TOL: f32 = 0.02;
    const MAX_OUTLIER: u8 = 48;
    let golden_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden");

    let Some(mut renderer) = headless() else {
        return;
    };
    let frame = fixed_frame();
    let bless = std::env::var_os("LMV_BLESS").is_some();
    std::fs::create_dir_all(&golden_dir).expect("create tests/golden");

    let decode = |path: &PathBuf| -> CaptureImage {
        let img = image::open(path)
            .unwrap_or_else(|e| panic!("decode baseline {}: {e}", path.display()))
            .to_rgba8();
        CaptureImage {
            width: img.width(),
            height: img.height(),
            rgba: img.into_raw(),
        }
    };
    let max_outlier = |a: &CaptureImage, b: &CaptureImage| -> u8 {
        a.rgba
            .chunks_exact(4)
            .zip(b.rgba.chunks_exact(4))
            .flat_map(|(pa, pb)| {
                pa.iter()
                    .zip(pb.iter())
                    .take(3)
                    .map(|(x, y)| x.abs_diff(*y))
            })
            .max()
            .unwrap_or(0)
    };

    let mut failures = Vec::new();
    // `layer_multiply` is **appended**, never inserted: building GPU resources
    // mid-run changes what a later capture resolves to on WARP (Plan 0053's
    // standing rule), so a new entry at the end leaves the two older baselines
    // rendered from the device state they were blessed under.
    for (stem, toml) in [
        ("layer_under", LAYERED),
        ("layer_over", OVER),
        ("layer_multiply", MULTIPLY),
    ] {
        let preset = load(toml);
        let name = preset.name.clone();
        renderer.set_presets(vec![preset]);
        let fresh = renderer
            .capture_preset(&name, &frame, FRAMES)
            .expect("capture layered golden fixture");
        let path = golden_dir.join(format!("{stem}.png"));

        if bless {
            let buf = image::RgbaImage::from_raw(fresh.width, fresh.height, fresh.rgba.clone())
                .expect("capture buffer matches its declared dimensions");
            buf.save(&path)
                .unwrap_or_else(|e| panic!("write baseline {}: {e}", path.display()));
            println!("blessed {}", path.display());
            continue;
        }

        assert!(
            path.exists(),
            "missing baseline {} — run `LMV_BLESS=1 cargo test -p lmv-core --test layer`",
            path.display()
        );
        let baseline = decode(&path);
        let mean = frame_diff(&baseline, &fresh);
        let outlier = max_outlier(&baseline, &fresh);
        println!(
            "{stem:<12} mean {mean:.4} (tol {MEAN_TOL}) max_outlier {outlier} (tol {MAX_OUTLIER})"
        );
        if mean > MEAN_TOL || outlier > MAX_OUTLIER {
            failures.push(format!(
                "{stem}: mean {mean:.4} / outlier {outlier} exceeds tolerance"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "layered golden drift beyond tolerance — the join's routing, the blend, or a \
         layer scene's rendering has changed. Bless with LMV_BLESS=1 only if intended: \
         {failures:#?}"
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
