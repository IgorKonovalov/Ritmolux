//! Plan 0003 Phase 4: the pure expression evaluator and TOML preset schema.
//! Values are exact, functions behave, malformed input is rejected without a
//! panic, compiled evaluation allocates nothing, and a sample preset parses
//! with its bindings intact.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use lmv_core::preset::{Preset, SystemKind, Variables, compile};

/// Global allocator that counts allocation calls **per thread**, so a test can
/// assert that a region on the current thread performs no heap allocation,
/// independent of what other tests are doing in parallel. A process-global
/// counter would fold in concurrent tests' allocations and fail under stock
/// multi-threaded `cargo test` (it only passed under nextest's process-per-test
/// isolation); the thread-local counter holds under both runners.
struct Counting;

thread_local! {
    /// Allocations charged to the current thread. `const`-initialized so the
    /// first touch neither allocates nor registers a destructor — the allocator
    /// can read it without re-entering itself.
    static ALLOCS: Cell<usize> = const { Cell::new(0) };
}

/// Allocations counted on the current thread so far.
fn alloc_count() -> usize {
    ALLOCS.with(|c| c.get())
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // `try_with`: a no-op if TLS is unavailable (thread teardown), never a
        // panic or an allocation on the alloc path.
        let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

/// All-zero variables except where overridden per test.
fn vars(bass: f32, mid: f32, treb: f32, onset: f32, beat: f32, bar: f32, time: f32) -> Variables {
    Variables::new(bass, mid, treb, onset, beat, bar, time)
}

#[test]
fn arithmetic_evaluates_exactly() {
    let e = compile("bass * 2 + 0.1").expect("compiles");
    let v = vars(0.25, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    // Same f32 operations as the expression, so the result is bit-exact.
    let expected = 0.25f32 * 2.0 + 0.1f32;
    assert_eq!(e.eval(&v), expected);

    // Precedence and parentheses.
    let e = compile("(bass + mid) * 2").expect("compiles");
    let v = vars(1.0, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0);
    assert_eq!(e.eval(&v), 3.0);
}

#[test]
fn builtin_functions_behave() {
    // sin(pi/2) ~ 1
    let e = compile("sin(time)").expect("compiles");
    let v = vars(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, std::f32::consts::FRAC_PI_2);
    assert!((e.eval(&v) - 1.0).abs() < 1e-6);

    // clamp saturates on both sides (and does not panic if lo>hi never occurs).
    let e = compile("clamp(bass, 0, 1)").expect("compiles");
    assert_eq!(e.eval(&vars(2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0)), 1.0);
    assert_eq!(e.eval(&vars(-3.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0)), 0.0);
    assert_eq!(e.eval(&vars(0.4, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0)), 0.4);

    // lerp(mid, treb, bar): 2 + (10-2)*0.5 = 6.
    let e = compile("lerp(mid, treb, bar)").expect("compiles");
    let v = vars(0.0, 2.0, 10.0, 0.0, 0.0, 0.5, 0.0);
    assert_eq!(e.eval(&v), 6.0);

    // min/max/abs/floor
    let zero = Variables::default();
    assert_eq!(compile("min(3, 5)").expect("compiles").eval(&zero), 3.0);
    assert_eq!(compile("max(3, 5)").expect("compiles").eval(&zero), 5.0);
    assert_eq!(compile("abs(0 - 4)").expect("compiles").eval(&zero), 4.0);
    assert_eq!(compile("floor(3.9)").expect("compiles").eval(&zero), 3.0);
}

/// Plan 0019 Phase 1: the v2 math functions each compute their mathematical
/// result on a known input, and `mod` is floored (divisor-signed) so it wraps.
#[test]
fn v2_math_functions_compute_expected_values() {
    let zero = Variables::default();
    let eval = |src: &str| compile(src).expect("compiles").eval(&zero);

    // cos(pi) = -1, and the constants resolve to the std literals.
    assert!((eval("cos(pi)") + 1.0).abs() < 1e-6);
    assert_eq!(eval("pi"), std::f32::consts::PI);
    assert_eq!(eval("tau"), std::f32::consts::TAU);
    assert!((eval("cos(tau)") - 1.0).abs() < 1e-6);

    assert_eq!(eval("sqrt(4)"), 2.0);
    assert_eq!(eval("pow(2, 3)"), 8.0);
    assert_eq!(eval("pow(9, 0.5)"), 3.0);

    // Floored modulo: a positive remainder for a positive divisor, so a
    // wrapping hue/phase never goes negative.
    assert_eq!(eval("mod(7, 3)"), 1.0);
    assert!(
        (eval("mod(0 - 0.2, 1.0)") - 0.8).abs() < 1e-6,
        "mod is floored: mod(-0.2, 1.0) wraps to 0.8, not -0.2"
    );

    // smoothstep: 0 below edge0, 1 above edge1, 0.5 at the midpoint, and the
    // eased (non-linear) value at a quarter.
    assert_eq!(eval("smoothstep(0, 1, 0.5)"), 0.5);
    assert_eq!(eval("smoothstep(0, 1, 0 - 3)"), 0.0);
    assert_eq!(eval("smoothstep(0, 1, 9)"), 1.0);
    assert_eq!(eval("smoothstep(0, 1, 0.25)"), 0.15625); // t*t*(3-2t)

    // The plan's composite expression: -1 + 2 + 8 + 1 + 0.5 = 10.5.
    let composite = "cos(pi) + sqrt(4) + pow(2,3) + mod(7,3) + smoothstep(0,1,0.5)";
    assert!((eval(composite) - 10.5).abs() < 1e-6);
}

/// Degenerate inputs to the v2 functions yield `NaN`/`inf`/`0`, never a panic —
/// `eval` must stay total on the per-frame hot path.
#[test]
fn v2_math_functions_are_total_on_degenerate_input() {
    let zero = Variables::default();
    let eval = |src: &str| compile(src).expect("compiles").eval(&zero);

    assert!(eval("sqrt(0 - 1)").is_nan(), "sqrt of a negative is NaN");
    assert!(eval("mod(1, 0)").is_nan(), "a zero divisor is NaN");
    // edge0 == edge1 divides by zero; the clamp folds the result into [0, 1].
    let degenerate = eval("smoothstep(1, 1, 2)");
    assert!(
        (0.0..=1.0).contains(&degenerate),
        "degenerate smoothstep stays bounded, got {degenerate}"
    );
}

/// A bare unknown identifier is still a compile error — the constants are
/// checked before the variable lookup, not instead of it.
#[test]
fn constants_do_not_swallow_unknown_identifiers() {
    assert!(matches!(
        compile("foo"),
        Err(lmv_core::preset::ExprError::UnknownIdent(name)) if name == "foo"
    ));
    // A constant used as a function is an unknown function, not a variable.
    assert!(compile("pi(1)").is_err());
}

#[test]
fn beat_coerces_as_a_zero_one_value() {
    let e = compile("1.0 + beat * 0.5").expect("compiles");
    assert_eq!(e.eval(&vars(0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0)), 1.5);
    assert_eq!(e.eval(&vars(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0)), 1.0);
}

#[test]
fn malformed_expressions_fail_to_compile_without_panicking() {
    for bad in [
        "bass * ",     // trailing operator
        "2 +* 3",      // operator where a value is expected
        "nope(1)",     // unknown function
        "unknownvar",  // unknown variable
        "clamp(1, 2)", // wrong arity
        "sin(1, 2)",   // wrong arity
        "1 @ 2",       // illegal character
        "(1 + 2",      // unbalanced parenthesis
        "1 2",         // trailing tokens
        "",            // empty
    ] {
        assert!(
            compile(bad).is_err(),
            "expression {bad:?} should fail to compile"
        );
    }
}

#[test]
fn compiled_eval_performs_no_heap_allocation() {
    let e = compile("clamp(bass * 2 + sin(time), 0, 1) + lerp(mid, treb, bar)").expect("compiles");
    let v = vars(0.5, 0.2, 0.1, 0.0, 1.0, 0.3, 1.23);

    // Warm up (touch any lazy statics before measuring).
    let _ = e.eval(&v);

    let before = alloc_count();
    let mut acc = 0.0f32;
    for _ in 0..10_000 {
        acc += e.eval(&v);
    }
    let after = alloc_count();

    assert!(acc.is_finite(), "sanity: evaluation produced a real number");
    assert_eq!(
        before,
        after,
        "compiled eval must not allocate; saw {} allocation(s)",
        after - before
    );
}

#[test]
fn sample_preset_parses_with_bindings_intact() {
    let src = r#"
system = "fragment_field"
name = "Test Field"

[params]
warp = "0.3 + bass * 1.5"
hue  = "time * 0.05 + treb"
kick = "beat"
"#;
    let preset = Preset::from_toml_str(src).expect("valid preset");
    assert_eq!(preset.system, SystemKind::FragmentField);
    assert_eq!(preset.name, "Test Field");
    assert_eq!(preset.params.len(), 3);

    let warp = preset
        .params
        .iter()
        .find(|b| b.name == "warp")
        .expect("warp binding present");
    let v = vars(0.2, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    assert!((warp.expr.eval(&v) - (0.3 + 0.2 * 1.5)).abs() < 1e-6);

    // Name defaults to the system when omitted.
    let unnamed = Preset::from_toml_str("system = \"swarm\"").expect("valid");
    assert_eq!(unnamed.system, SystemKind::Swarm);
    assert_eq!(unnamed.name, "swarm");
    assert!(unnamed.params.is_empty());
}

#[test]
fn bad_presets_are_rejected() {
    // Unknown system.
    assert!(Preset::from_toml_str("system = \"does_not_exist\"").is_err());
    // A parameter with a malformed expression.
    let bad = "system = \"swarm\"\n[params]\nx = \"bass * \"\n";
    assert!(Preset::from_toml_str(bad).is_err());
    // Not even valid TOML.
    assert!(Preset::from_toml_str("system = ").is_err());
    // A malformed [curve] structural config (unknown family) is a clean load
    // error, not a panic — the caller keeps the last good preset (ADR-0007).
    let bad_curve = "system = \"parametric_curve\"\n[curve]\nfamily = \"no_such_family\"\n[params]\nn = \"6\"\n";
    assert!(
        Preset::from_toml_str(bad_curve).is_err(),
        "an unknown curve family must be rejected"
    );
    // A star_pattern with an unknown tiling is likewise a clean load error.
    let bad_star =
        "system = \"star_pattern\"\n[generator]\ntiling = \"heptagon\"\ncontact_angle_deg = 30\n";
    assert!(
        Preset::from_toml_str(bad_star).is_err(),
        "an unknown star tiling must be rejected"
    );
    // A generator preset missing its [generator] table is rejected, not panicked.
    assert!(
        Preset::from_toml_str("system = \"lsystem\"").is_err(),
        "an lsystem with no [generator] table must be rejected"
    );
}

#[test]
fn curve_config_parses_into_structural_config() {
    use lmv_core::render::scenes::lines::{CurveFamily, GeneratorConfig};

    let src = "system = \"parametric_curve\"\n\
               name = \"Rose\"\n\
               [curve]\n\
               family = \"maurer_rose\"\n\
               [params]\n\
               n = \"6\"\nd = \"71\"\n";
    let preset = Preset::from_toml_str(src).expect("valid curve preset");
    assert_eq!(preset.system, SystemKind::ParametricCurve);
    match preset.config {
        Some(GeneratorConfig::Curve {
            family: CurveFamily::MaurerRose,
        }) => {}
        other => panic!("expected a Maurer-rose curve config, got {other:?}"),
    }

    // A curve preset with no [curve] table is valid — the scene uses its family
    // default (config stays None, so configure is a no-op).
    let no_table = Preset::from_toml_str("system = \"parametric_curve\"").expect("valid");
    assert!(no_table.config.is_none());
}

#[test]
fn palette_config_parses_names_stops_and_rejects_bad_tables() {
    use lmv_core::render::palette::{NamedPalette, Palette, PaletteConfig};

    // A built-in `name` parses to its config.
    let named = Preset::from_toml_str("system = \"fragment_field\"\n[palette]\nname = \"ember\"\n")
        .expect("named palette preset is valid");
    match named.palette {
        Some(PaletteConfig::Named(NamedPalette::Ember)) => {}
        other => panic!("expected the ember named palette, got {other:?}"),
    }

    // Three custom stops (hex + rgb-array forms) parse and bake to the gradient.
    let custom = Preset::from_toml_str(
        "system = \"fragment_field\"\n[palette]\n\
         stops = [ { at = 0.0, color = \"#000000\" }, \
                   { at = 0.5, color = [1.0, 0.0, 0.0] }, \
                   { at = 1.0, color = \"#ffffff\" } ]\n",
    )
    .expect("custom stops preset is valid");
    let cfg = custom.palette.expect("custom palette present");
    match &cfg {
        PaletteConfig::Custom(stops) => assert_eq!(stops.len(), 3, "all three stops kept"),
        other => panic!("expected custom stops, got {other:?}"),
    }
    // Bake + sample: start ~black, middle ~red, end ~white — the gradient renders.
    let pal = Palette::bake(&cfg);
    let start = pal.sample(0.002, 0.0);
    assert!(start.iter().all(|&c| c < 0.05), "start ~black: {start:?}");
    let mid = pal.sample(0.5, 0.0);
    assert!(
        mid[0] > 0.8 && mid[1] < 0.2 && mid[2] < 0.2,
        "middle ~red: {mid:?}"
    );
    let end = pal.sample(0.998, 0.0);
    assert!(end.iter().all(|&c| c > 0.95), "end ~white: {end:?}");

    // Malformed stop lists and selector clashes are clean load errors, not panics
    // (the loader keeps the previous good preset — NFR 10).
    let bad = [
        // Unsorted `at`.
        "system=\"swarm\"\n[palette]\nstops=[{at=0.0,color=\"#000000\"},{at=0.2,color=\"#111111\"},{at=0.1,color=\"#222222\"}]\n",
        // `at` out of range.
        "system=\"swarm\"\n[palette]\nstops=[{at=0.0,color=\"#000000\"},{at=1.5,color=\"#ffffff\"}]\n",
        // Unparseable hex color.
        "system=\"swarm\"\n[palette]\nstops=[{at=0.0,color=\"#zzzzzz\"},{at=1.0,color=\"#ffffff\"}]\n",
        // Fewer than two stops.
        "system=\"swarm\"\n[palette]\nstops=[{at=0.0,color=\"#000000\"}]\n",
        // Both `name` and `stops` (mutually exclusive).
        "system=\"swarm\"\n[palette]\nname=\"ember\"\nstops=[{at=0.0,color=\"#000000\"},{at=1.0,color=\"#ffffff\"}]\n",
        // Unknown built-in name.
        "system=\"swarm\"\n[palette]\nname=\"chartreuse_dream\"\n",
        // Empty palette table (neither selector).
        "system=\"swarm\"\n[palette]\n",
    ];
    for src in bad {
        assert!(
            Preset::from_toml_str(src).is_err(),
            "malformed palette should be rejected: {src}"
        );
    }
}

#[test]
fn embedded_default_presets_all_parse() {
    use lmv_core::preset::{EMBEDDED, Preset, default_presets};

    // The C-ABI / foobar path relies on these rendering without a preset dir.
    // The embedded set is generated from `presets/*.toml` at build time
    // (ADR-0022), so this assert is structural — every embedded preset parses,
    // above a floor — never a hardcoded count that a new preset has to bump.
    for &(name, src) in EMBEDDED {
        assert!(
            Preset::from_toml_str(src).is_ok(),
            "embedded preset `{name}` should compile"
        );
    }
    // A preset that failed to compile would be silently dropped by
    // `default_presets()`, so the two lengths agreeing proves all parsed.
    assert_eq!(
        default_presets().len(),
        EMBEDDED.len(),
        "every embedded preset compiles into the default set"
    );
    assert!(
        EMBEDDED.len() >= 8,
        "the curated library is the hand-tuned set, not the 4 proof-of-concept files"
    );
}

#[test]
fn load_dir_loads_the_good_and_reports_the_bad() {
    use std::fs;

    let dir = std::env::temp_dir().join("lmv_preset_load_test");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create temp preset dir");
    fs::write(
        dir.join("good.toml"),
        "system = \"swarm\"\n[params]\nforce = \"1 + bass * 2\"\n",
    )
    .expect("write good preset");
    fs::write(
        dir.join("bad.toml"),
        "system = \"swarm\"\n[params]\nforce = \"bass * \"\n",
    )
    .expect("write bad preset");
    fs::write(dir.join("notes.txt"), "not a preset").expect("write non-toml");

    let report = lmv_core::preset::load_dir(&dir);
    assert_eq!(report.presets.len(), 1, "only the valid .toml loads");
    assert_eq!(report.errors.len(), 1, "the malformed .toml is reported");

    // A missing directory is empty, not an error (degrade, never crash).
    let missing = lmv_core::preset::load_dir(&dir.join("does_not_exist"));
    assert!(missing.presets.is_empty() && missing.errors.is_empty());

    let _ = fs::remove_dir_all(&dir);
}
