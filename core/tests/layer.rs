//! The `[layer]` capability's Phase 1 contract (Plan 0076 / ADR-0090): a
//! preset composes a second scene `under` the post chain, through the roster's
//! existing one-instance-per-system scenes.
//!
//! Four claims, matching the phase's done-when:
//!
//! 1. the layered fixture renders **both** scenes, visibly (it differs from its
//!    layerless control) and **deterministically** (two captures are
//!    byte-identical);
//! 2. a **layer binding reacts to the analysis frame like a top-level one** —
//!    proven differentially: the fixture binding `zoom = "bass"` renders
//!    pixel-identical to a probe hard-coding the same value, and different from
//!    a probe hard-coding another, all under one frame, so the only degree of
//!    freedom is the binding's evaluation;
//! 3. a pair whose scenes share GPU state — the same system twice, or two
//!    line-family systems borrowing the shared `LineRenderer` — **fails at
//!    load** with the Phase 1 error (deleted when Phase 2's per-layer instances
//!    land);
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

/// Done-when 3: the Phase 1 shared-state error. Deleted by Phase 2, which
/// constructs layer scenes per preset.
#[test]
fn a_shared_state_pair_fails_at_load_until_phase_2() {
    // The same system twice — the same `Box<dyn Scene>` cannot render twice in
    // one frame while layers resolve from the roster.
    let same = Preset::from_toml_str(
        "system = \"fragment_field\"\n[layer]\nsystem = \"fragment_field\"\n",
    );
    let err = same.err().map(|e| e.to_string()).unwrap_or_default();
    assert!(
        err.contains("Plan 0076 Phase 2"),
        "a same-system pair names the Phase 1 restriction, got: {err}"
    );

    // Two *different* line-family systems share the one `LineRenderer`
    // (`scenes::create_all`), so they are just as unrenderable together.
    let lines =
        Preset::from_toml_str("system = \"parametric_curve\"\n[layer]\nsystem = \"spectrum\"\n");
    let err = lines.err().map(|e| e.to_string()).unwrap_or_default();
    assert!(
        err.contains("shares GPU state"),
        "a line-family pair shares the LineRenderer and must be rejected, got: {err}"
    );

    // An independent-state pair loads cleanly.
    assert!(
        Preset::from_toml_str("system = \"fragment_field\"\n[layer]\nsystem = \"swarm\"\n").is_ok(),
        "an independent-state pair is exactly what Phase 1 supports"
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

    // ...and `over` is a Phase 1 load error (deleted by Phase 3, which builds
    // the junction).
    let err = Preset::from_toml_str(
        "system = \"fragment_field\"\n[layer]\nsystem = \"swarm\"\njoin = \"over\"\n",
    )
    .err()
    .map(|e| e.to_string())
    .unwrap_or_default();
    assert!(err.contains("Plan 0076 Phase 3"), "got: {err}");

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
