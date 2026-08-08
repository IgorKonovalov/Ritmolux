// The pure roster contract, tested without a GPU surface (a live `Renderer`
// can't be built headlessly). The `Renderer::preset_names`/`select_preset`
// wrappers delegate to `Roster` 1:1, so this covers the addressing contract
// Plan 0008 Phase 2 names. Test asserts use `expect`/`panic!`, allowed here
// over the file's hot-path panic-denial pragma — test code is not the render
// path (`headless_or_skip` panics on an unexpected build error).
#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use super::{
    AnalysisMetrics, CaptureImage, HeadlessOptions, Mode, ParamRoute, ParamSmoother, RenderError,
    Renderer, Roster, element_prefix, evaluate_series, resolve_route,
};
use crate::dsp::AnalysisFrame;
use crate::preset::{Easing, Preset, SystemKind, Variables, compile};
use crate::render::metrics::frame_diff;
use crate::render::post::{KALEIDOSCOPE, TRAILS};

/// A minimal valid preset: a known system + explicit name, no params.
fn preset(name: &str) -> Preset {
    Preset::from_toml_str(&format!("system = \"swarm\"\nname = \"{name}\""))
        .expect("hand-written test preset is valid")
}

fn roster(names: &[&str]) -> Roster {
    Roster::new(names.iter().map(|n| preset(n)).collect())
}

/// Plan 0031 Phase 3 — the routing contract, GPU-free: every global namespace
/// resolves to its owner, the system's own names to the scene, and anything
/// else to `Unclaimed` (dropped at apply time, already warned about at load).
///
/// This is the answer the per-frame `set_param` fallthrough chain used to
/// re-derive on every bound param of every frame.
#[test]
fn each_namespace_resolves_to_its_owner() {
    let swarm = SystemKind::Swarm;
    // The backdrop pre-pass, outside the chain (ADR-0031).
    for name in crate::render::background::PARAMS {
        assert_eq!(
            resolve_route(name, swarm),
            ParamRoute::Background,
            "`{name}` belongs to the backdrop"
        );
    }
    // The two chain stages, each to its own fixed position — not merely "some
    // stage", so a swapped `STAGE_PARAMS` order would fail here.
    for name in crate::render::trails::PARAMS {
        assert_eq!(resolve_route(name, swarm), ParamRoute::Stage(TRAILS));
    }
    for name in crate::render::kaleidoscope::PARAMS {
        assert_eq!(resolve_route(name, swarm), ParamRoute::Stage(KALEIDOSCOPE));
    }
    // The terminal engine-wide ink pass (ADR-0032) — `ink_*` and `paper_*`.
    for name in crate::render::ink::PARAMS {
        assert_eq!(
            resolve_route(name, swarm),
            ParamRoute::Ink,
            "`{name}` belongs to the ink pass"
        );
    }
    assert!(
        crate::render::ink::PARAMS.contains(&"ink_amount")
            && crate::render::ink::PARAMS
                .iter()
                .any(|n| n.starts_with("paper_")),
        "the ink vocabulary covers both the ink_* and paper_* halves"
    );

    // Everything a system declares goes to its scene — checked for **every**
    // system, so a family whose names happened to collide with a global
    // namespace could not slip through.
    for system in SystemKind::ALL {
        for name in system.param_names() {
            assert_eq!(
                resolve_route(name, system),
                ParamRoute::Scene,
                "`{name}` is {}'s own param",
                system.as_str()
            );
        }
    }

    // An unknown name is ignored, not an error and not mis-routed. Includes a
    // near-miss typo, which is the case the load-time warning names.
    for name in ["nope", "trail", "bg_", "kaleido", "ink", "warp_"] {
        assert_eq!(
            resolve_route(name, swarm),
            ParamRoute::Unclaimed,
            "`{name}` is claimed by nobody"
        );
    }
    // A param real on one system is not silently accepted on another: `warp`
    // is fragment-only, so on the swarm it is unclaimed.
    assert_eq!(
        resolve_route("warp", SystemKind::FragmentField),
        ParamRoute::Scene
    );
    assert_eq!(resolve_route("warp", swarm), ParamRoute::Unclaimed);
}

/// The routes a roster hands the frame loop line up positionally with the
/// preset's bindings — the property `evaluate_preset`'s `zip` rests on.
#[test]
fn roster_routes_pair_with_each_presets_bindings() {
    let mixed = Preset::from_toml_str(
        "system = \"swarm\"\nname = \"Mixed\"\n[params]\n\
         bg_bright = \"0.5\"\ntrails = \"0.4\"\nkaleido_order = \"6\"\n\
         ink_amount = \"1\"\nforce = \"bass\"\nnot_a_param = \"1\"\n",
    )
    .expect("valid preset with one binding per route");
    let roster = Roster::new(vec![mixed]);
    let preset = roster.active_preset().expect("one preset");
    let routes = roster.active_routes();
    assert_eq!(routes.len(), preset.params.len(), "one route per binding");

    // Bindings are name-sorted at load, so read the pairing by name rather
    // than by position.
    let by_name: Vec<(&str, ParamRoute)> = preset
        .params
        .iter()
        .zip(routes)
        .map(|(binding, route)| (binding.name.as_str(), *route))
        .collect();
    assert!(by_name.contains(&("bg_bright", ParamRoute::Background)));
    assert!(by_name.contains(&("trails", ParamRoute::Stage(TRAILS))));
    assert!(by_name.contains(&("kaleido_order", ParamRoute::Stage(KALEIDOSCOPE))));
    assert!(by_name.contains(&("ink_amount", ParamRoute::Ink)));
    assert!(by_name.contains(&("force", ParamRoute::Scene)));
    assert!(by_name.contains(&("not_a_param", ParamRoute::Unclaimed)));
    // The unknown name loaded with a warning rather than failing (ADR-0020).
    assert_eq!(preset.warnings.len(), 1, "{:?}", preset.warnings);

    // Out of range is empty, pairing with `presets.get` returning `None`.
    assert!(roster.routes_for(9).is_empty());
}

/// `tau` is read off the `[smoothing]` table once at load, so the frame loop
/// does no map lookup. An unlisted param is instant (ADR-0019), and a scalar
/// entry means the same constant in both directions (ADR-0035).
#[test]
fn smoothing_taus_are_resolved_onto_the_bindings() {
    let p = Preset::from_toml_str(
        "system = \"swarm\"\n[params]\nforce = \"bass\"\nhue = \"time\"\n\
         size = \"treb\"\n\
         [smoothing]\nforce = 0.25\nsize = { attack = 0.02, release = 0.7 }\n",
    )
    .expect("valid preset with both [smoothing] forms");
    let tau_of = |name: &str| {
        p.params
            .iter()
            .find(|b| b.name == name)
            .map(|b| b.tau)
            .expect("bound param")
    };
    assert_eq!(
        tau_of("force"),
        Easing::symmetric(0.25),
        "a scalar is both directions"
    );
    assert_eq!(tau_of("hue"), Easing::INSTANT, "unlisted means instant");
    assert_eq!(
        tau_of("size"),
        Easing {
            attack: 0.02,
            release: 0.7
        },
        "the pair form resolves at the same boundary"
    );
}

/// Both constants are validated at the load boundary, and the error names the
/// parameter — and, for a pair, which side of it. A bad value is a surfaced
/// load error the caller degrades on, never a panic (ADR-0002 / NFR 10).
#[test]
fn a_bad_smoothing_constant_is_a_load_error_naming_the_parameter() {
    let load = |table: &str| {
        Preset::from_toml_str(&format!(
            "system = \"swarm\"\n[params]\nforce = \"bass\"\n[smoothing]\n{table}\n"
        ))
    };
    let err = load("force = -1.0")
        .expect_err("negative scalar")
        .to_string();
    assert!(
        err.contains("force") && err.contains("non-negative"),
        "{err}"
    );
    assert!(load("force = nan").is_err(), "non-finite scalar");
    assert!(load("force = inf").is_err(), "non-finite scalar");

    // Each side of a pair is checked, and the message says which one.
    let err = load("force = { attack = -1.0, release = 0.7 }")
        .expect_err("negative attack")
        .to_string();
    assert!(
        err.contains("force") && err.contains("attack"),
        "the error must name the failing side: {err}"
    );
    let err = load("force = { attack = 0.02, release = nan }")
        .expect_err("non-finite release")
        .to_string();
    assert!(err.contains("release"), "{err}");

    // A malformed table is as clear as a malformed float: both expected keys
    // are named, half a pair is rejected rather than silently defaulted, and
    // a wrong type says what was wanted.
    let err = load("force = { atack = 0.02, release = 0.7 }")
        .expect_err("misspelled key")
        .to_string();
    assert!(
        err.contains("attack") && err.contains("release"),
        "an unknown key must name the expected ones: {err}"
    );
    assert!(
        load("force = { attack = 0.02 }").is_err(),
        "half a pair is a mistake, not a shorthand"
    );
    assert!(
        load("force = { release = 0.7 }").is_err(),
        "half a pair is a mistake, not a shorthand"
    );
    let err = load("force = \"fast\"").expect_err("a string").to_string();
    assert!(
        err.contains("attack") && err.contains("release"),
        "a wrong type must state both accepted forms: {err}"
    );
}

/// Build a headless `Renderer`, or return `None` (a logged skip) when the
/// runner exposes no usable GPU adapter (ADR-0016). A missing adapter is an
/// environmental property of the CI runner — macOS has no software Metal
/// fallback — not a code failure, so the GPU-capture tests skip on it rather
/// than panic; any *other* build error still panics loudly. On Windows WARP
/// an adapter is always present, so the callers' assertions run in full.
fn headless_or_skip(opts: HeadlessOptions) -> Option<Renderer> {
    match Renderer::new_headless(opts) {
        Ok(r) => Some(r),
        Err(RenderError::RequestAdapter(_)) => {
            eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
            None
        }
        Err(e) => panic!("headless renderer build failed: {e}"),
    }
}

#[test]
fn names_are_yielded_in_roster_order() {
    let r = roster(&["alpha", "bravo", "charlie"]);
    let got: Vec<&str> = r.names().collect();
    assert_eq!(got, ["alpha", "bravo", "charlie"]);
}

#[test]
fn select_addresses_by_absolute_index() {
    let mut r = roster(&["alpha", "bravo", "charlie"]);
    assert_eq!(r.name(), "alpha"); // a fresh roster starts at index 0
    r.select(2);
    assert_eq!(r.name(), "charlie"); // the third entry
}

#[test]
fn out_of_range_select_is_a_no_op() {
    let mut r = roster(&["alpha", "bravo", "charlie"]);
    r.select(1);
    r.select(999); // past the end: unchanged — no panic, no wrap
    assert_eq!(r.name(), "bravo");
}

#[test]
fn set_presets_clamps_active_when_the_roster_shrinks() {
    let mut r = roster(&["alpha", "bravo", "charlie"]);
    r.select(2);
    r.set_presets(vec![preset("solo")]); // index 2 now out of range
    assert_eq!(r.name(), "solo");
}

/// Phase 1 (Plan 0013): a surface-less renderer captures the active preset
/// into an offscreen texture. `prefer_software` (WARP on DX12) keeps it
/// reproducible on any adapter. Asserts a full tight RGBA buffer with at
/// least one non-black pixel — the preset actually drew.
#[test]
fn headless_captures_a_non_black_frame() {
    let Some(mut renderer) = headless_or_skip(HeadlessOptions {
        width: 256,
        height: 256,
        prefer_software: true,
    }) else {
        return;
    };

    let img = renderer
        .capture_frame(&AnalysisFrame::default())
        .expect("capture succeeds");

    assert_eq!(img.width, 256);
    assert_eq!(img.height, 256);
    assert_eq!(img.rgba.len(), 256 * 256 * 4, "tight RGBA, no row padding");
    let non_black = img
        .rgba
        .chunks_exact(4)
        .any(|px| px[0] > 0 || px[1] > 0 || px[2] > 0);
    assert!(non_black, "the active preset drew at least one lit pixel");
}

/// Plan 0049 Phase 2: the analysis snapshot survives the trip from the frame
/// the render seam is handed to the accessor the overlay and the 1 Hz logger
/// read. The `diag` unit tests cover the conversion; this covers the plumbing,
/// which is the half a pure test cannot see.
#[test]
fn analysis_metrics_follow_the_drawn_frame() {
    let Some(mut renderer) = headless_or_skip(HeadlessOptions {
        width: 64,
        height: 64,
        prefer_software: true,
    }) else {
        return;
    };
    assert_eq!(
        renderer.analysis_metrics(),
        AnalysisMetrics::default(),
        "nothing drawn yet — zeros, not stale state"
    );

    let frame = AnalysisFrame {
        bass: 0.3,
        mid: 0.6,
        treb: 0.9,
        onset: 0.45,
        downbeat_confidence: 0.72,
        downbeat_locked: true,
        ..Default::default()
    };
    renderer
        .capture_frame(&frame)
        .expect("capture the analysis frame");

    assert_eq!(
        renderer.analysis_metrics(),
        AnalysisMetrics {
            bass: 0.3,
            mid: 0.6,
            treb: 0.9,
            onset: 0.45,
            downbeat_confidence: 0.72,
            downbeat_locked: true,
        },
    );
}

/// Phase 2 (Plan 0013): `capture_preset` is a pure function of
/// `(name, frame, frames)`. Uses the stateful swarm preset "Drift" — the
/// case where a missing state reset would leak history — to prove two
/// captures are byte-identical, that N=1 differs from N=120 (the scene
/// animates), and that an unknown name is a clean error.
#[test]
fn capture_preset_is_deterministic_and_animates() {
    let Some(mut renderer) = headless_or_skip(HeadlessOptions {
        width: 128,
        height: 128,
        prefer_software: true,
    }) else {
        return;
    };
    let frame = AnalysisFrame::default();

    let a = renderer
        .capture_preset("Drift", &frame, 120)
        .expect("capture Drift @120");
    let b = renderer
        .capture_preset("Drift", &frame, 120)
        .expect("recapture Drift @120");
    assert_eq!(
        a.rgba, b.rgba,
        "same (preset, frame, N) is byte-identical across calls"
    );

    let one = renderer
        .capture_preset("Drift", &frame, 1)
        .expect("capture Drift @1");
    assert_ne!(
        one.rgba, a.rgba,
        "N=1 differs from N=120 — the scene advances over time"
    );

    assert!(
        renderer
            .capture_preset("no-such-preset", &frame, 1)
            .is_err(),
        "an unknown preset name is a clean error, not a panic"
    );
}

/// Plan 0010 review finding #1: a line generator that hits the segment cap
/// must **surface** the truncation, never cut silently (ADR-0007). An
/// L-system whose depth blows past the cap reports a `CapOverflow` through
/// `configure`, read back via `cap_overflow()`; a grammar that fits reports
/// `None`. This is the surfacing half of the cap contract the mechanism
/// tracked but nothing exercised.
#[test]
fn oversized_lsystem_surfaces_a_cap_overflow() {
    let Some(mut renderer) = headless_or_skip(HeadlessOptions {
        width: 64,
        height: 64,
        prefer_software: true,
    }) else {
        return;
    };

    // F -> ten F's per iteration: depth 5 is 100k draw steps, far past the
    // 20k cap, so the build truncates and must report the drop.
    let huge = Preset::from_toml_str(
        "system = \"lsystem\"\nname = \"Huge\"\n\
         [generator]\naxiom = \"F\"\nrules = { F = \"FFFFFFFFFF\" }\n\
         angle_deg = 20\nmax_depth = 5\n",
    )
    .expect("valid lsystem preset");
    renderer.set_presets(vec![huge]);
    let overflow = renderer
        .cap_overflow()
        .expect("an oversized L-system surfaces its cap truncation");
    assert!(
        overflow.dropped > 0,
        "the dropped-segment count is reported"
    );

    // A modest grammar (F -> FF, depth 5 = 32 segments) fits — no overflow.
    let small = Preset::from_toml_str(
        "system = \"lsystem\"\nname = \"Small\"\n\
         [generator]\naxiom = \"F\"\nrules = { F = \"FF\" }\n\
         angle_deg = 20\nmax_depth = 5\n",
    )
    .expect("valid lsystem preset");
    renderer.set_presets(vec![small]);
    assert!(
        renderer.cap_overflow().is_none(),
        "a grammar that fits within the cap reports no overflow"
    );
}

/// Plan 0018 Phase 4: the per-frame geometry mirror must also surface a cap
/// truncation through `cap_overflow()`, reusing the ADR-0007 `CapOverflow`
/// path — never a silent cut. A dense rose replicated six-fold blows past the
/// 20k cap; a modest one fits. Unlike the L-system's load-time overflow, this
/// one is computed per frame, so it surfaces only after a frame has rendered.
#[test]
fn oversized_mirror_surfaces_a_cap_overflow() {
    let Some(mut renderer) = headless_or_skip(HeadlessOptions {
        width: 64,
        height: 64,
        prefer_software: true,
    }) else {
        return;
    };
    let frame = AnalysisFrame::default();

    // ~5000 chords replicated six-fold = ~30k segments, far past the 20k cap.
    let huge = Preset::from_toml_str(
        "system = \"parametric_curve\"\nname = \"MirrorHuge\"\n\
         [curve]\nfamily = \"maurer_rose\"\n\
         [params]\nsamples = \"5000\"\nmirror_order = \"6\"\n",
    )
    .expect("valid parametric preset");
    renderer.set_presets(vec![huge]);
    // Render frames so the per-frame mirror replication runs and records the drop.
    renderer
        .capture_preset("MirrorHuge", &frame, 2)
        .expect("capture MirrorHuge");
    let overflow = renderer
        .cap_overflow()
        .expect("an oversized mirror surfaces its cap truncation");
    assert!(
        overflow.dropped > 0,
        "the dropped-segment count is reported"
    );

    // A modest rose at order 3 stays well under the cap — no overflow.
    let small = Preset::from_toml_str(
        "system = \"parametric_curve\"\nname = \"MirrorSmall\"\n\
         [curve]\nfamily = \"maurer_rose\"\n\
         [params]\nsamples = \"200\"\nmirror_order = \"3\"\n",
    )
    .expect("valid parametric preset");
    renderer.set_presets(vec![small]);
    renderer
        .capture_preset("MirrorSmall", &frame, 2)
        .expect("capture MirrorSmall");
    assert!(
        renderer.cap_overflow().is_none(),
        "a mirror that fits within the cap reports no overflow"
    );
}

/// Phase 5 (ADR-0019): a step change eases toward the target over several
/// frames instead of snapping, and converges. The one-pole is the whole point.
/// Plan 0034 Phase 4 done-when 1 and 4. A binding that names `index` really
/// does vary **per element** — a monotonic ramp, not N copies of one value —
/// and the positions it is evaluated at are normalized `0..1` rather than
/// element counts, so an expression composes without knowing how many there
/// are. A binding that does **not** name `index` yields one constant across
/// the whole series, which is what makes the per-element path opt-in.
#[test]
fn a_per_element_binding_varies_and_a_plain_one_does_not() {
    let vars = Variables::default();
    let mut out = [0.0f32; 6];

    // `index` itself: the first element is 0, the last is 1, and the steps
    // in between are even — the normalization the whole feature rests on.
    let ramp = compile("index").expect("compiles");
    assert!(
        ramp.uses_index(),
        "naming index marks the binding per-element"
    );
    evaluate_series(&ramp, &vars, &mut out);
    assert_eq!(out.first().copied(), Some(0.0), "the first element is 0");
    assert_eq!(out.last().copied(), Some(1.0), "the last element is 1");
    for i in 1..out.len() {
        assert!(
            out[i] > out[i - 1],
            "the series must be monotonically varying, got {out:?}"
        );
        let step = out[i] - out[i - 1];
        assert!(
            (step - 0.2).abs() < 1e-6,
            "steps are even at 1/(n-1), got {step}"
        );
    }

    // A composed expression varies with it rather than being clamped flat.
    let shaped = compile("0.01 + index * 0.05").expect("compiles");
    evaluate_series(&shaped, &vars, &mut out);
    assert!((out[0] - 0.01).abs() < 1e-6 && (out[5] - 0.06).abs() < 1e-6);

    // No `index`: one constant across every element, and the flag says so.
    let flat = compile("0.4 + 0.1").expect("compiles");
    assert!(
        !flat.uses_index(),
        "a binding without index is not per-element"
    );
    evaluate_series(&flat, &vars, &mut out);
    assert!(
        out.iter().all(|&v| (v - 0.5).abs() < 1e-6),
        "a binding not using index is constant across elements, got {out:?}"
    );

    // A single element has no span to normalize over, so it reads 0 — the
    // same value `index` takes outside a per-element evaluation.
    let mut one = [9.0f32; 1];
    evaluate_series(&ramp, &vars, &mut one);
    assert_eq!(one[0], 0.0);
    // And an empty series is simply not evaluated.
    evaluate_series(&ramp, &vars, &mut []);
}

/// The element count a preset asks the render layer to evaluate for: its
/// `[spectrum] elements`, zero for every other system — which is what makes
/// the per-element branch unreachable for them — and always bounded by the
/// scratch, so a config can never index past it.
#[test]
fn only_a_spectrum_preset_claims_a_per_element_prefix() {
    let spectrum = Preset::from_toml_str(
        "system = \"spectrum\"\n[spectrum]\nelements = 30\n[params]\nbase = \"0.2\"\n",
    )
    .expect("valid spectrum preset");
    assert_eq!(element_prefix(&spectrum, 64), 30);
    assert_eq!(
        element_prefix(&spectrum, 8),
        8,
        "the scratch bounds the prefix"
    );

    for src in [
        "system = \"swarm\"\n[params]\nforce = \"1.0\"\n",
        "system = \"parametric_curve\"\n[params]\nn = \"6\"\n",
        "system = \"attractor\"\n[params]\nsize = \"1.0\"\n",
    ] {
        let preset = Preset::from_toml_str(src).expect("valid preset");
        assert_eq!(
            element_prefix(&preset, 64),
            0,
            "{} has no per-element surface",
            preset.system.as_str()
        );
    }
}

#[test]
fn smoothing_eases_a_step_instead_of_snapping() {
    let mut s = ParamSmoother::default();
    let dt = 1.0 / 60.0;
    let tau = Easing::symmetric(0.1);
    // The first value after a reset snaps (it seeds the state).
    assert_eq!(s.smooth(0, 0.0, tau, dt), 0.0);
    // A step to 1.0 closes only a fraction of the gap — eased, not snapped.
    let f1 = s.smooth(0, 1.0, tau, dt);
    assert!(f1 > 0.0 && f1 < 1.0, "eased, not snapped: {f1}");
    let f2 = s.smooth(0, 1.0, tau, dt);
    assert!(f2 > f1 && f2 < 1.0, "monotonic approach: {f1} -> {f2}");
    // Many frames of the held target converge to it.
    for _ in 0..600 {
        s.smooth(0, 1.0, tau, dt);
    }
    assert!(
        (s.smooth(0, 1.0, tau, dt) - 1.0).abs() < 1e-3,
        "converges to the held target"
    );
}

/// ADR-0035: the same step, up and then down, under
/// `{ attack = 0.02, release = 0.7 }`. The property is the **asymmetry** —
/// a snap up and a glide down — which no single `tau` reaches at any value.
///
/// The absolute figures are what this filter actually does at 60 Hz:
/// `alpha = 1 - exp(-dt/tau)` closes 56.5 % of the gap per frame at
/// `tau = 0.02`, so two frames reach 81 % and three reach 92 %. (Plan 0033's
/// done-when says "90 % within two frames"; that is one frame optimistic for
/// this constant — the assertion below pins the arithmetic, not the prose.)
#[test]
fn asymmetric_easing_snaps_up_and_glides_down() {
    let mut s = ParamSmoother::default();
    let dt = 1.0 / 60.0;
    let e = Easing {
        attack: 0.02,
        release: 0.7,
    };

    // Seed at 0, then step to 1.0 and watch the rise.
    assert_eq!(s.smooth(0, 0.0, e, dt), 0.0);
    let after_two = {
        s.smooth(0, 1.0, e, dt);
        s.smooth(0, 1.0, e, dt)
    };
    assert!(
        after_two >= 0.80,
        "attack = 0.02 must cover most of the step in two 60 Hz frames, got {after_two}"
    );
    let after_three = s.smooth(0, 1.0, e, dt);
    assert!(
        after_three >= 0.90,
        "three frames reach 90 % of the target, got {after_three}"
    );

    // Settle, then step back to 0 and watch the fall over 0.4 s.
    for _ in 0..300 {
        s.smooth(0, 1.0, e, dt);
    }
    let mut falling = 0.0;
    for _ in 0..(0.4 / dt) as usize {
        falling = s.smooth(0, 0.0, e, dt);
    }
    assert!(
        falling > 0.50,
        "release = 0.7 must still be above half a second's worth of glide after \
         0.4 s, got {falling}"
    );

    // The asymmetry itself, stated as a comparison rather than two constants:
    // the rise covers far more of its gap in two frames than the fall does.
    let mut sym = ParamSmoother::default();
    let slow = Easing::symmetric(0.7);
    sym.smooth(0, 0.0, slow, dt);
    sym.smooth(0, 1.0, slow, dt);
    let symmetric_two = sym.smooth(0, 1.0, slow, dt);
    assert!(
        after_two > symmetric_two * 10.0,
        "a 0.02 s attack must be dramatically faster than the 0.7 s release used \
         symmetrically ({after_two} vs {symmetric_two}) — otherwise one constant \
         would have done"
    );
}

/// ADR-0035's compatibility claim, checked rather than asserted in prose: a
/// scalar `[smoothing]` entry and an explicit `{ attack = t, release = t }`
/// table are **bit-identical** through the whole load-and-smooth path. This is
/// why no shipped preset moved and no golden was re-blessed.
#[test]
fn a_scalar_smoothing_entry_is_bit_identical_to_an_equal_pair() {
    let load = |table: &str| {
        Preset::from_toml_str(&format!(
            "system = \"swarm\"\n[params]\nforce = \"bass\"\n[smoothing]\nforce = {table}\n"
        ))
        .expect("valid preset")
        .params
        .first()
        .expect("one binding")
        .tau
    };
    let scalar = load("0.31");
    let pair = load("{ attack = 0.31, release = 0.31 }");
    assert_eq!(scalar, pair, "the two forms resolve to the same constants");

    // Drive both through the smoother with a signal that rises *and* falls, so
    // the direction branch is exercised in both directions, and compare raw
    // bits — an epsilon compare would hide exactly the drift this rules out.
    let dt = 1.0 / 60.0;
    let (mut a, mut b) = (ParamSmoother::default(), ParamSmoother::default());
    for i in 0..240 {
        let raw = ((i as f32) * 0.11).sin() * 0.5 + 0.5;
        let va = a.smooth(0, raw, scalar, dt);
        let vb = b.smooth(0, raw, pair, dt);
        assert_eq!(
            va.to_bits(),
            vb.to_bits(),
            "frame {i}: scalar {va} != pair {vb}"
        );
    }
}

/// `tau = 0` (the default for an unlisted param) is today's instant behaviour,
/// and ADR-0035 keeps `0` meaning instant **per side**.
#[test]
fn zero_tau_passes_through_instantly() {
    let mut s = ParamSmoother::default();
    let dt = 1.0 / 60.0;
    assert_eq!(s.smooth(0, 0.5, Easing::INSTANT, dt), 0.5);
    assert_eq!(
        s.smooth(0, 0.9, Easing::INSTANT, dt),
        0.9,
        "tau=0 snaps every frame"
    );

    // A zero on one side only: that direction snaps while the other still
    // eases. `{ attack = 0, release = 0.5 }` is the "instant hit, slow decay"
    // an author reaches for on a percussive accent.
    let half = Easing {
        attack: 0.0,
        release: 0.5,
    };
    let mut s = ParamSmoother::default();
    assert_eq!(s.smooth(1, 0.0, half, dt), 0.0, "seeds");
    assert_eq!(s.smooth(1, 1.0, half, dt), 1.0, "attack = 0 snaps up");
    let falling = s.smooth(1, 0.0, half, dt);
    assert!(
        falling > 0.0 && falling < 1.0,
        "release = 0.5 still eases down: {falling}"
    );
}

/// A reset makes the next frame snap to the incoming value — the mechanism
/// behind a preset switch snapping to the new preset (no cross-preset bleed).
#[test]
fn reset_snaps_to_the_next_value() {
    let mut s = ParamSmoother::default();
    let dt = 1.0 / 60.0;
    let tau = Easing::symmetric(0.2);
    s.smooth(0, 0.0, tau, dt);
    for _ in 0..10 {
        s.smooth(0, 1.0, tau, dt); // partway toward 1.0
    }
    s.reset();
    assert_eq!(
        s.smooth(0, 5.0, tau, dt),
        5.0,
        "after a reset the next value seeds fresh — a snap, no stale bleed"
    );
}

/// Phase 5 determinism (NFR 6): a preset with a `[smoothing]` table, captured
/// twice, is byte-identical — the smoother state resets on the capture
/// scene-rebuild, so a capture stays a pure function of its inputs.
#[test]
fn smoothed_preset_capture_is_deterministic() {
    let Some(mut renderer) = headless_or_skip(HeadlessOptions {
        width: 96,
        height: 96,
        prefer_software: true,
    }) else {
        return;
    };
    let smoothed = Preset::from_toml_str(
        "system = \"fragment_field\"\nname = \"Smoothed\"\n\
         [params]\nwarp = \"0.3 + bass * 0.4\"\nhue = \"0.2\"\nglow = \"0.8\"\n\
         [smoothing]\nwarp = 0.25\n",
    )
    .expect("valid smoothed preset");
    renderer.set_presets(vec![smoothed]);
    let frame = AnalysisFrame {
        bass: 0.8,
        ..Default::default()
    };
    let a = renderer
        .capture_preset("Smoothed", &frame, 30)
        .expect("capture Smoothed a");
    let b = renderer
        .capture_preset("Smoothed", &frame, 30)
        .expect("capture Smoothed b");
    assert_eq!(
        a.rgba, b.rgba,
        "smoothing state resets on rebuild -> identical recaptures"
    );
}

/// Phase 6 determinism (NFR 6): a preset with `trails` (the feedback stage),
/// captured twice, is byte-identical — the accumulation resets on the capture
/// scene-rebuild, so a capture stays a pure function of its inputs even though
/// the trail is stateful across frames.
#[test]
fn trailed_preset_capture_is_deterministic() {
    let Some(mut renderer) = headless_or_skip(HeadlessOptions {
        width: 96,
        height: 96,
        prefer_software: true,
    }) else {
        return;
    };
    // A spinning rose with a long trail: the accumulation carries state across
    // the warm-up frames, so a missing reset would leak between captures.
    let trailed = Preset::from_toml_str(
        "system = \"parametric_curve\"\nname = \"Trailed\"\n\
         [curve]\nfamily = \"maurer_rose\"\n\
         [params]\nn = \"3\"\nspin = \"0.9\"\nsamples = \"120\"\ntrails = \"0.8\"\n",
    )
    .expect("valid trailed preset");
    renderer.set_presets(vec![trailed]);
    let frame = AnalysisFrame::default();
    let a = renderer
        .capture_preset("Trailed", &frame, 20)
        .expect("capture Trailed a");
    let b = renderer
        .capture_preset("Trailed", &frame, 20)
        .expect("capture Trailed b");
    assert_eq!(
        a.rgba, b.rgba,
        "trails accumulation resets on rebuild -> identical recaptures"
    );
}

/// Plan 0028: the two new shape params (`radial_offset`, `phase`) are
/// preset-bindable and actually reach the sampler. A rose that binds both to
/// a `bass` expression, driven by a bass stimulus, must render differently
/// from an identical rose with both unbound (default `0.0`) — proof the
/// evaluated values thread through `set_param` into the geometry, not just
/// that the preset parses.
#[test]
fn shape_params_reach_the_parametric_scene() {
    let Some(mut renderer) = headless_or_skip(HeadlessOptions {
        width: 96,
        height: 96,
        prefer_software: true,
    }) else {
        return;
    };
    let frame = AnalysisFrame {
        bass: 1.0,
        ..Default::default()
    };

    // Baseline: shape params unbound, so radial_offset = phase = 0.0 even
    // under the bass stimulus — the plain rose.
    let baseline = Preset::from_toml_str(
        "system = \"parametric_curve\"\nname = \"ShapeBaseline\"\n\
         [curve]\nfamily = \"maurer_rose\"\n\
         [params]\nn = \"6\"\nd = \"71\"\nsamples = \"200\"\nscale = \"0.9\"\n",
    )
    .expect("valid baseline parametric preset");
    renderer.set_presets(vec![baseline]);
    let base = renderer
        .capture_preset("ShapeBaseline", &frame, 4)
        .expect("capture ShapeBaseline");

    // Same rose, but radial_offset and phase are bound to the bass stimulus.
    let bound = Preset::from_toml_str(
        "system = \"parametric_curve\"\nname = \"ShapeBound\"\n\
         [curve]\nfamily = \"maurer_rose\"\n\
         [params]\nn = \"6\"\nd = \"71\"\nsamples = \"200\"\nscale = \"0.9\"\n\
         radial_offset = \"bass * 0.6\"\nphase = \"bass * 2.0\"\n",
    )
    .expect("valid shape-bound parametric preset");
    renderer.set_presets(vec![bound]);
    let lit = renderer
        .capture_preset("ShapeBound", &frame, 4)
        .expect("capture ShapeBound");

    assert_ne!(
        base.rgba, lit.rgba,
        "bound radial_offset/phase must change the rendered geometry"
    );
}

/// **The governor's wiring**, not its arithmetic (Plan 0023 close review, minor
/// 4). `dual_live_eligible` and `shares_resources` are each covered where they
/// live; what nothing exercised is `dissolve_mode` composing them — including
/// the arm that decides what an *unresolvable* preset index means.
///
/// GPU-free: `Roster` and the preset list are enough, so this runs everywhere.
#[test]
fn dissolve_mode_freezes_a_shared_scene_pair_and_an_unresolvable_one() {
    let Some(mut renderer) = headless_or_skip(HeadlessOptions {
        width: 64,
        height: 64,
        prefer_software: true,
    }) else {
        return;
    };
    let of = |name: &str, body: &str| {
        Preset::from_toml_str(&format!("name = \"{name}\"\n{body}"))
            .expect("hand-written test preset is valid")
    };
    // 0 and 1 are the same system (one scene object); 2 is a different line
    // system (a *different* scene that still shares the one `LineRenderer`); 3
    // holds genuinely independent GPU state.
    renderer.set_presets(vec![
        of("SameA", "system = \"parametric_curve\"\n"),
        of("SameB", "system = \"parametric_curve\"\n"),
        of(
            "OtherLine",
            "system = \"star_pattern\"\n[generator]\ntiling = \"8\"\n",
        ),
        of("Field", "system = \"fragment_field\"\n"),
    ]);

    assert_eq!(
        renderer.dissolve_mode(0, 1),
        Mode::Freeze,
        "two presets on one `SystemKind` are one mutable scene object"
    );
    assert_eq!(
        renderer.dissolve_mode(0, 2),
        Mode::Freeze,
        "two line systems share one `LineRenderer`, so neither may render twice"
    );
    // An index the roster cannot resolve must read as *shared*, not as
    // independent: the safe answer to "can we render both?" is no.
    assert_eq!(
        renderer.dissolve_mode(0, 99),
        Mode::Freeze,
        "an unresolvable preset index must not be read as an independent pair"
    );
    assert_eq!(
        renderer.dissolve_mode(99, 0),
        Mode::Freeze,
        "...from either side"
    );

    // The independent pair is the only one that *could* upgrade — and it still
    // freezes here, because a headless renderer collects no frame times and the
    // governor upgrades only on positive evidence of headroom. Asserting that
    // pins the second half of the composition: passing the veto is necessary,
    // not sufficient.
    assert_eq!(
        renderer.dissolve_mode(2, 3),
        Mode::Freeze,
        "independent scenes still need frame-time evidence, which a headless \
         capture never has"
    );
}

// --- Plan 0023 Phase 4: the adaptive dual-live upgrade -------------------
//
// These live inside the crate rather than in `tests/transition.rs` because a
// headless capture cannot reach the dual-live path from outside: diagnostics
// are off, so the governor has no frame-time evidence and correctly answers
// `Freeze` every time. `begin_transition_forced` is the crate-private,
// `#[cfg(test)]` way in — the shipped API grows nothing.
//
// Both tests are **differential**: they run the *same* dissolve twice, with
// only the mode changed, so any difference is the live outgoing side and
// nothing else. That is stronger than a threshold on one run, which could pass
// on scene animation alone.

/// The outgoing preset — a rose that both **spins** and leaves a long trail, so
/// it has motion to show and cross-frame state to preserve.
///
/// Spun **fast** and faded **slowly** on purpose: the accumulated smear has to
/// cover a lot more of the frame than one stroke does, or "the trail survived"
/// and "the trail restarted" differ by too little to assert. At this rate the
/// warm-up sweeps the rose through its whole symmetry period.
fn spinning_trailed_rose() -> Preset {
    Preset::from_toml_str(
        "system = \"parametric_curve\"\nname = \"DualA\"\n\
         [curve]\nfamily = \"maurer_rose\"\n\
         [params]\nn = \"3\"\nd = \"71\"\nsamples = \"300\"\nscale = \"0.85\"\n\
         spin = \"6.0\"\ntrails = \"0.98\"\n",
    )
    .expect("valid spinning trailed rose")
}

/// The incoming preset — a fragment field, so the pair resolves to **different
/// scene objects** with independent GPU state (not two of the three line scenes
/// sharing one renderer), which is what dual-live requires.
fn moving_field() -> Preset {
    Preset::from_toml_str(
        "system = \"fragment_field\"\nname = \"DualB\"\n\
         [params]\nwarp = \"0.5\"\nhue = \"0.2\"\nglow = \"0.9\"\n",
    )
    .expect("valid fragment field preset")
}

/// How many frames the outgoing preset renders before the switch — the length
/// of trail history the dissolve inherits. [`WARMED`] is well past the point
/// where the accumulation dominates the picture; [`COLD`] is the counterfactual
/// a restarted chain would look like.
const WARMED: usize = 60;
const COLD: usize = 1;

/// Capture one dissolve at a forced fidelity, after `warmup` frames of the
/// outgoing preset. Returns the dissolve window, opening frame first.
///
/// `software` picks the adapter. **A trail's survival across the switch can
/// only be seen on real hardware**: on the DX12 WARP rasterizer, allocating the
/// dissolve's GPU resources mid-run (the blend's targets, the incoming side's
/// chain) resets what the trails feedback resolves to, so the outgoing side
/// comes back at a single stroke's brightness whether it has one frame of
/// history or thirty. That is the same coexisting-pipeline quirk
/// `trails.rs` documents and `tests/background_composite.rs` skips for; on
/// hardware the dissolve's opening frame is byte-identical to the ordinary
/// frame it replaces. Checks that only compare two dissolves against each other
/// stay on WARP, where they run in CI.
fn dissolve_at(
    mode: Mode,
    frames: usize,
    warmup: usize,
    software: bool,
) -> Option<Vec<CaptureImage>> {
    let mut renderer = headless_or_skip(HeadlessOptions {
        width: 96,
        height: 96,
        prefer_software: software,
    })?;
    if !software && renderer.adapter_is_software() {
        eprintln!(
            "skipped: only a software rasterizer is available (WARP drops the \
             trails accumulation when the dissolve allocates; see dissolve_at)"
        );
        return None;
    }
    let stimulus = AnalysisFrame::default();
    renderer.set_presets(vec![spinning_trailed_rose(), moving_field()]);
    for _ in 0..warmup.max(1) {
        renderer.capture_frame(&stimulus).expect("warm-up frame");
    }
    renderer.begin_transition_forced(1, mode);
    Some(
        (0..frames)
            .map(|i| {
                renderer
                    .capture_frame(&stimulus)
                    .unwrap_or_else(|e| panic!("dissolve frame {i}: {e}"))
            })
            .collect(),
    )
}

/// The mean per-channel difference `core/tests/golden.rs` calls cross-rasterizer
/// drift rather than signal (its `MEAN_TOL`, per [ADR-0023]). Reused here only as
/// a **lower bound on the control**: a denominator that sits inside the declared
/// noise band cannot calibrate anything above it. Mechanism-derived rather than
/// measured — the control peak reads `0.4078` on the local WARP, 20x this.
///
/// [ADR-0023]: ../../docs/adrs/0023-golden-drift-guard-uses-frozen-fixtures.md
const NOISE_FLOOR: f32 = 0.02;

/// **A dual-live dissolve runs, and its picture is not freeze's.** Same
/// presets, same `dt` sequence, same blend kind — only the fidelity differs.
///
/// The opening frame must be *identical* in both modes: it is the outgoing
/// preset's own composite either way, before dual-live has anything extra to
/// do. That pins the assertion to the dissolve rather than to a warm-up drift.
///
/// **What is asserted is exact, not a threshold** ([ADR-0071]). This project's
/// determinism contract makes two runs of this code byte-identical — the
/// opening-frame `assert_eq!` above is the demonstration — so "the two modes
/// differ somewhere in the window" needs no number, where the floor it replaces
/// (`frame_diff > 0.01`) was half of [`NOISE_FLOOR`] and so was made inside the
/// band this project already calls noise.
///
/// **On a software adapter that is a smoke check, not a guard on the defect**
/// ([ADR-0074]). What it proves is that dual-live runs, that it produces a
/// different picture from freeze somewhere in the window, and that the dissolve
/// is dissolving. What it cannot prove is that the outgoing side is *live*:
/// `dissolve_at`'s own docstring records that on WARP, allocating the dissolve's
/// GPU resources mid-run resets what the trails feedback resolves to, and
/// dual-live allocates more than freeze does — so the two modes differ for that
/// reason alone, and an outgoing side that was genuinely held would still pass
/// here.
///
/// The **magnitude** half is deliberately not calibrated from this statistic.
/// Plan 0060 Phase 2 read the printed ratio off both machines and it did not
/// travel:
///
/// | statistic | local WARP 10.0.19041 | CI WARP 10.0.26100 | spread |
/// |---|---|---|---|
/// | peak signal | 0.109573 | 0.009683 | 11.3x |
/// | peak control | 0.407826 | 0.264177 | 1.54x |
/// | ratio | 0.268675 | 0.036654 | 7.3x |
///
/// Signal and control are not the same kind of quantity — the control is the
/// dissolve's own progression, the signal is the outgoing side rendering
/// *through* trails accumulation — so the rasterizer does not cancel out of
/// their ratio, and the CI signal lands under [`NOISE_FLOOR`] besides. A floor
/// taken from either reading would be a measurement asserted universally, the
/// shape [ADR-0071] exists to forbid. The claim goes to hardware instead —
/// [`a_dual_live_dissolve_moves_the_picture_against_its_own_progression`], which
/// takes these same two series on a non-software adapter, where the allocation
/// quirk does not exist and the ratio therefore does carry a floor. (That
/// deferral originally named Plan 0053 Phase 3, on the premise that no hardware
/// adapter was reachable from this suite; the premise was wrong — the gate is
/// `device_type == Cpu`, not "a discrete GPU" — so the measurement was taken here
/// instead.) The series below stay printed so the next time these numbers move,
/// the log says so.
///
/// [ADR-0071]: ../../docs/adrs/0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md
/// [ADR-0074]: ../../docs/adrs/0074-a-ratio-against-an-in-run-control-is-not-automatically-portable.md
#[test]
fn dual_live_keeps_the_outgoing_side_animating() {
    const FRAMES: usize = 40;
    let (Some(frozen), Some(live)) = (
        dissolve_at(Mode::Freeze, FRAMES, WARMED, true),
        dissolve_at(Mode::DualLive, FRAMES, WARMED, true),
    ) else {
        return; // no GPU adapter (ADR-0016)
    };

    assert_eq!(
        frozen[0].rgba, live[0].rgba,
        "the opening frame is the outgoing composite in either mode"
    );

    // Signal: what the fidelity changed. Control: what the dissolve does anyway,
    // read on the same adapter in the same run. Both are reported and neither is
    // thresholded — ADR-0074 is why their ratio is not the portable quantity it
    // was taken for.
    let signal: Vec<f32> = frozen
        .iter()
        .zip(live.iter())
        .map(|(f, l)| frame_diff(f, l))
        .collect();
    let control: Vec<f32> = frozen.iter().map(|f| frame_diff(&frozen[0], f)).collect();
    let peak = |series: &[f32]| series.iter().copied().fold(0.0f32, f32::max);
    let peak_signal = peak(&signal);
    let peak_control = peak(&control);
    let mean_signal = signal.iter().sum::<f32>() / signal.len() as f32;

    let series = |s: &[f32]| {
        s.iter()
            .map(|v| format!("{v:.4}"))
            .collect::<Vec<_>>()
            .join(" ")
    };
    eprintln!("dual-live vs freeze, {FRAMES} frames on this adapter (ADR-0071 report):");
    eprintln!(
        "  signal  frame_diff(frozen[i], live[i]):  {}",
        series(&signal)
    );
    eprintln!("  signal  peak {peak_signal:.6}  mean {mean_signal:.6}");
    eprintln!(
        "  control frame_diff(frozen[0], frozen[i]): {}",
        series(&control)
    );
    eprintln!("  control peak {peak_control:.6}");
    eprintln!(
        "  ratio   peak signal / peak control = {:.6}",
        peak_signal / peak_control
    );

    assert!(
        peak_signal > 0.0,
        "over {FRAMES} frames dual-live never differed from freeze by a single \
         byte, which is what holding the outgoing side would produce"
    );
    assert!(
        peak_control > NOISE_FLOOR,
        "the control is trivial (peak {peak_control} at or under the {NOISE_FLOOR} \
         noise floor) — this dissolve is not dissolving, so it cannot calibrate \
         the signal"
    );
}

/// Half the ratio measured on this box (`0.036542`), rounded down. A held
/// outgoing side collapses the numerator to zero, so the floor only has to
/// separate the reading from nothing; half of it is the same margin
/// [`CARRIES`] takes against its own counterfactual.
///
/// [`CARRIES`]: a_dual_live_dissolve_carries_the_outgoing_trail
const HW_RATIO_FLOOR: f32 = 0.018;

/// **The outgoing side keeps moving the picture, by a measured fraction of the
/// dissolve's own progression.** The magnitude half of
/// [`dual_live_keeps_the_outgoing_side_animating`], taken where it means
/// something: a **non-software** adapter, where `dissolve_at`'s allocation quirk
/// is absent and the opening frame is byte-identical to the ordinary frame it
/// replaces. So the numerator is the outgoing side animating and nothing else,
/// which is what the WARP sibling cannot say ([ADR-0074]).
///
/// **This is a measurement, not a property** ([ADR-0071]). Taken 2026-08-04 on:
///
/// | | |
/// |---|---|
/// | adapter | `AMD Radeon(TM) Graphics` (integrated, `0x1002:0x1638`) |
/// | driver | `30.0.13002.1001` |
/// | backend | DX12 |
///
/// | statistic | this adapter | CI WARP 10.0.26100 | local WARP 10.0.19041 |
/// |---|---|---|---|
/// | peak signal | 0.009653 | 0.009683 | 0.109573 |
/// | peak control | 0.264172 | 0.264177 | 0.407826 |
/// | ratio | **0.036542** | 0.036654 | 0.268675 |
///
/// **It did not land near the local WARP `0.268675`** — it landed on the CI WARP
/// reading, to three figures on the ratio and five on the control. That reframes
/// the 7.3x spread ADR-0074 recorded between two WARP builds: the newer build
/// agrees with hardware and the older local one is the outlier, rather than the
/// quantity being unstable in both directions. It does not reopen ADR-0074's
/// decision — a number that reproduces on two configurations is still a
/// measurement, not a portable floor — but it is the first evidence about *which*
/// WARP reading was anomalous, and it belongs to the open question that ADR-0074
/// left for Plan [0053].
///
/// **The signal sits under [`NOISE_FLOOR`], and that is not the objection it
/// looks like.** That band is cross-*rasterizer* drift; this comparison is two
/// runs of the same code on the same adapter in the same process, which this
/// project's determinism contract makes byte-identical. The series is the proof:
/// it opens at exactly `0.0000` and climbs monotonically, which is an animation
/// accumulating, not noise.
///
/// **CI never enforces this.** It skips on both runners — `windows-latest` offers
/// only WARP, `macos-latest` has no software Metal at all (ADR-0016) — so the
/// local gate and `.githooks/pre-push` are what run it.
///
/// [ADR-0071]: ../../docs/adrs/0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md
/// [ADR-0074]: ../../docs/adrs/0074-a-ratio-against-an-in-run-control-is-not-automatically-portable.md
/// [0053]: ../../docs/plans/0053-the-suite-stops-blessing-what-warp-gets-wrong.md
#[test]
fn a_dual_live_dissolve_moves_the_picture_against_its_own_progression() {
    const FRAMES: usize = 40;
    let (Some(frozen), Some(live)) = (
        dissolve_at(Mode::Freeze, FRAMES, WARMED, false),
        dissolve_at(Mode::DualLive, FRAMES, WARMED, false),
    ) else {
        return; // no adapter, or only a software one
    };

    assert_eq!(
        frozen[0].rgba, live[0].rgba,
        "the opening frame is the outgoing composite in either mode"
    );

    let signal: Vec<f32> = frozen
        .iter()
        .zip(live.iter())
        .map(|(f, l)| frame_diff(f, l))
        .collect();
    let control: Vec<f32> = frozen.iter().map(|f| frame_diff(&frozen[0], f)).collect();
    let peak = |series: &[f32]| series.iter().copied().fold(0.0f32, f32::max);
    let (peak_signal, peak_control) = (peak(&signal), peak(&control));
    let mean_signal = signal.iter().sum::<f32>() / signal.len() as f32;
    let ratio = peak_signal / peak_control;

    let series = |s: &[f32]| {
        s.iter()
            .map(|v| format!("{v:.4}"))
            .collect::<Vec<_>>()
            .join(" ")
    };
    eprintln!("dual-live vs freeze on hardware, {FRAMES} frames (ADR-0071 report):");
    eprintln!(
        "  signal  frame_diff(frozen[i], live[i]):  {}",
        series(&signal)
    );
    eprintln!("  signal  peak {peak_signal:.6}  mean {mean_signal:.6}");
    eprintln!(
        "  control frame_diff(frozen[0], frozen[i]): {}",
        series(&control)
    );
    eprintln!("  control peak {peak_control:.6}");
    eprintln!("  ratio   peak signal / peak control = {ratio:.6}");

    assert!(
        peak_control > NOISE_FLOOR,
        "the control is trivial (peak {peak_control} at or under the {NOISE_FLOOR} \
         noise floor) — this dissolve is not dissolving, so it cannot calibrate \
         the signal"
    );
    assert!(
        ratio > HW_RATIO_FLOOR,
        "the outgoing side moved the picture by {ratio} of the dissolve's own \
         progression, under the {HW_RATIO_FLOOR} this adapter was measured at — \
         a held outgoing side is what produces a collapsing ratio"
    );
}

/// Mean Rec. 709 luminance in bytes — how much light a frame carries, which is
/// what a feedback trail adds and a restarted one would not.
fn mean_luma(img: &CaptureImage) -> f32 {
    let sum: f32 = img
        .rgba
        .chunks_exact(4)
        .map(|px| 0.2126 * px[0] as f32 + 0.7152 * px[1] as f32 + 0.0722 * px[2] as f32)
        .sum();
    sum / (img.rgba.len() / 4) as f32
}

/// **A dual-live dissolve out of a trails-on preset keeps that trail.** The
/// outgoing side re-renders through the composite it has been using all along,
/// so its accumulation carries into the dissolve instead of restarting.
///
/// Measured as **brightness against the same dissolve run cold** — the
/// counterfactual of the bug. A restarted chain would enter the dissolve with a
/// single stroke's worth of light, which is exactly what the cold run has, so
/// the two would read alike; carrying thirty frames of decay-0.9 history makes
/// the warmed run several times brighter.
///
/// The reference cannot be the frozen run at the same warm-up: freeze and
/// dual-live take the same opening frame through the same composite, so a bug
/// that restarted the chain at the switch would move both together and the
/// comparison would still pass. The cold run moves only with the trail history,
/// which is the claim.
///
/// **Real hardware only** — WARP cannot show a trail surviving the dissolve's
/// allocations at all (see [`dissolve_at`]).
#[test]
fn a_dual_live_dissolve_carries_the_outgoing_trail() {
    const FRAMES: usize = 4;
    // Halfway between the two outcomes rather than close to either: a restarted
    // chain reads 1.0x the cold run by construction, and the carried trail
    // measures ~1.9x it on the dev box. The floor cannot go lower — the cold run
    // still draws the same stroke this one does; only the swept history differs.
    const CARRIES: f32 = 1.5;
    let (Some(warmed), Some(cold), Some(frozen)) = (
        dissolve_at(Mode::DualLive, FRAMES, WARMED, false),
        dissolve_at(Mode::DualLive, FRAMES, COLD, false),
        dissolve_at(Mode::Freeze, FRAMES, WARMED, false),
    ) else {
        return; // no adapter, or only a software one
    };

    let carries = |warm: &CaptureImage, restarted: &CaptureImage, what: &str| {
        let (got, floor) = (mean_luma(warm), mean_luma(restarted));
        assert!(
            floor > 0.0 && got > CARRIES * floor,
            "{what} must carry the outgoing preset's accumulated trail, not \
             restart from a fresh accumulation ({got} against {floor} for the \
             same dissolve run cold)"
        );
    };
    // The opening frame is the outgoing preset's own composite...
    carries(&warmed[0], &cold[0], "the dissolve's opening frame");
    // ...and the first dual-live re-render — the frame the outgoing side is
    // drawn a second time, at ~98% outgoing weight — must still carry it.
    carries(&warmed[1], &cold[1], "the first dual-live frame");

    // And it is genuinely re-rendering rather than reusing the held texture:
    // the spin has moved the geometry even though the light is preserved.
    assert!(
        frame_diff(&frozen[1], &warmed[1]) > 0.0,
        "the outgoing side re-renders; it does not reuse the snapshot"
    );
}
