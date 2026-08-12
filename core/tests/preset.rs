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

/// All-zero variables except where overridden per test. `tempo`/`novelty` are
/// bound separately by the tests that exercise them.
fn vars(
    bass: f32,
    mid: f32,
    treb: f32,
    onset: f32,
    beat: f32,
    bar: f32,
    time: f32,
) -> Variables<'static> {
    Variables::new(bass, mid, treb, onset, beat, bar, time, 0.0, 0.0)
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

/// Plan 0038 Phase 4. `log(x)` is the **natural** logarithm, arity 1, and the
/// reason it exists is the decibel idiom in `docs/presets.md`.
#[test]
fn log_is_the_natural_logarithm_at_arity_one() {
    let zero = Variables::default();
    let eval = |src: &str| compile(src).expect("compiles").eval(&zero);

    // Exact interior values, not just "is finite": base e, not base 10.
    assert!((eval("log(1)")).abs() < 1e-6, "log(1) = 0");
    assert!(
        (eval("log(2.718281828)") - 1.0).abs() < 1e-6,
        "log(e) = 1, so this is the natural log"
    );
    assert!(
        (eval("log(10)") - std::f32::consts::LN_10).abs() < 1e-5,
        "log(10) = ln(10), NOT 1 — there is no log10"
    );
    // The literal `docs/presets.md` tells authors to divide by, guarded against
    // drifting from the real constant. Parsed from a string rather than written
    // as a float so it is the *documented* value being checked.
    assert!(
        (eval("2.302585") - std::f32::consts::LN_10).abs() < 1e-6,
        "the ln(10) constant in the docs' decibel idiom has drifted"
    );

    // The worked dB example the docs promise: a typical measured band level of
    // 0.03 reads -30.5 dB.
    let db = eval("20 * log(0.03) / 2.302585");
    assert!(
        (eval("log(0.03)") + 3.5066).abs() < 1e-3,
        "log(0.03) = -3.5066"
    );
    assert!(
        (db + 30.457).abs() < 1e-2,
        "20 * log(0.03) / ln(10) should be about -30.5 dB, got {db}"
    );

    // Arity is checked at compile time like every other call, so a `log10`-shaped
    // two-argument call is a surfaced load error rather than a silent misread.
    assert!(
        compile("log(0.1, 2)").is_err(),
        "log takes exactly one argument"
    );
    // ...and a bare `log` is an unknown identifier, not a zero-arg call.
    assert!(compile("log").is_err(), "a bare `log` is not a value");
}

/// Degenerate inputs to the v2 functions yield `NaN`/`inf`/`0`, never a panic —
/// `eval` must stay total on the per-frame hot path.
#[test]
fn v2_math_functions_are_total_on_degenerate_input() {
    let zero = Variables::default();
    let eval = |src: &str| compile(src).expect("compiles").eval(&zero);

    assert!(eval("sqrt(0 - 1)").is_nan(), "sqrt of a negative is NaN");
    // `log` follows sqrt's posture rather than inventing a new rule: honest at
    // the edges, guarded by the author with `max`/`select` (Plan 0038 Phase 4).
    assert_eq!(
        eval("log(0)"),
        f32::NEG_INFINITY,
        "log(0) is -inf, not a clamped floor"
    );
    assert!(eval("log(0 - 1)").is_nan(), "log of a negative is NaN");
    assert!(
        eval("max(log(max(0, 0.0001)), 0 - 80)").is_finite(),
        "the documented guard idiom keeps a silent input finite"
    );
    assert!(eval("mod(1, 0)").is_nan(), "a zero divisor is NaN");
    // edge0 == edge1 divides by zero; the clamp folds the result into [0, 1].
    let degenerate = eval("smoothstep(1, 1, 2)");
    assert!(
        (0.0..=1.0).contains(&degenerate),
        "degenerate smoothstep stays bounded, got {degenerate}"
    );
}

/// A binding of `time` at `t` under salt `salt` — the shape every seeded-function
/// test below evaluates against.
fn salted(t: f32, salt: u32) -> Variables<'static> {
    vars(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, t).with_salt(salt)
}

/// Plan 0047 Phase 1 / ADR-0051. `hash(x)` fills `[0, 1)` rather than clustering.
///
/// A **coarse bucket census**, not a threshold on any single value: what makes a
/// scatter usable is that it covers its range roughly evenly, and that is a
/// property a sweep can state without tuning a constant to the mixer.
#[test]
fn hash_scatters_uniformly_over_a_sweep() {
    const SAMPLES: usize = 2_000;
    const BUCKETS: usize = 10;

    let e = compile("hash(time)").expect("compiles");
    let mut counts = [0usize; BUCKETS];
    for i in 0..SAMPLES {
        // An irrational-ish step, so the arguments are ordinary floats rather
        // than the integer lattice `noise` is built on.
        let x = e.eval(&salted(i as f32 * 0.37, 12_345));
        assert!(
            (0.0..1.0).contains(&x),
            "hash is documented as [0, 1), got {x} at sample {i}"
        );
        let bucket = ((x * BUCKETS as f32) as usize).min(BUCKETS - 1);
        counts[bucket] += 1;
    }

    // Half to double the uniform expectation. Wide on purpose — this is a
    // coverage claim, not a chi-squared test — but it fails hard for a mixer that
    // correlates with its input, which is the defect worth catching.
    let expected = SAMPLES / BUCKETS;
    for (i, &count) in counts.iter().enumerate() {
        assert!(
            count >= expected / 2 && count <= expected * 2,
            "bucket {i} holds {count} of {SAMPLES}, expected about {expected}: {counts:?}"
        );
    }
}

/// Plan 0047 Phase 1 / ADR-0051. `noise(x)` is the **continuous** one: nearby
/// arguments give nearby values, distant ones do not. Asserted as a ratio between
/// two measurements from the same sweep, so nothing here is a tuned constant —
/// and contrasted against `hash` on the identical arguments, since being smooth
/// where `hash` is not is the entire reason both exist.
#[test]
fn noise_is_continuous_and_bounded_where_hash_is_not() {
    const SAMPLES: usize = 500;

    let noise = compile("noise(time)").expect("compiles");
    let hash = compile("hash(time)").expect("compiles");

    let mean_step = |e: &lmv_core::preset::Expr, step: f32| {
        let total: f32 = (0..SAMPLES)
            .map(|i| {
                let t = i as f32 * 0.113;
                (e.eval(&salted(t, 99)) - e.eval(&salted(t + step, 99))).abs()
            })
            .sum();
        total / SAMPLES as f32
    };

    for i in 0..SAMPLES {
        let v = noise.eval(&salted(i as f32 * 0.113, 99));
        assert!(
            (0.0..=1.0).contains(&v),
            "noise is documented as [0, 1], got {v} at sample {i}"
        );
    }

    let near = mean_step(&noise, 0.01);
    let far = mean_step(&noise, 0.5);
    assert!(
        near * 10.0 < far,
        "noise must move less over a short step than a long one, got {near} vs {far}"
    );
    // Not a constant function — a "smooth" noise that never moves would pass the
    // test above trivially.
    assert!(
        far > 0.05,
        "noise barely moves at all over half a cell: {far}"
    );

    let hash_near = mean_step(&hash, 0.01);
    assert!(
        hash_near > near * 10.0,
        "hash must scatter where noise glides: {hash_near} vs {near}"
    );
}

/// Plan 0047 Phase 1 / ADR-0051 — the property NFR §6 actually asks for: seeded,
/// not unpredictable. One salt reproduces **bit-exactly**; two salts disagree.
#[test]
fn a_seeded_expression_reproduces_under_one_salt_and_differs_under_two() {
    let e = compile("hash(time) + noise(time * 0.3)").expect("compiles");

    let mut differences = 0;
    for i in 0..64 {
        let t = i as f32 * 0.17;
        // Bit-exact, not approximate: an expression is a pure function of its
        // argument and its salt, so there is nothing here to drift.
        assert_eq!(
            e.eval(&salted(t, 7)),
            e.eval(&salted(t, 7)),
            "the same salt must reproduce exactly at t = {t}"
        );
        if e.eval(&salted(t, 7)) != e.eval(&salted(t, 8)) {
            differences += 1;
        }
    }
    assert_eq!(
        differences, 64,
        "two salts should disagree at every sampled argument"
    );

    // Salt `0` is not a special "off" value — it is just another salt, and the
    // one an unseeded preset gets.
    assert_ne!(e.eval(&salted(1.5, 0)), e.eval(&salted(1.5, 1)));
}

/// Wrong arity is a load error for the seeded pair exactly as for every other
/// function — the parser checks them through the same `Func::arity`.
#[test]
fn the_seeded_functions_take_exactly_one_argument() {
    for src in ["hash(1, 2)", "hash()", "noise(1, 2)", "noise()"] {
        assert!(compile(src).is_err(), "`{src}` must be a compile error");
    }
    assert!(compile("hash").is_err(), "a bare `hash` is not a value");
    assert!(compile("noise").is_err(), "a bare `noise` is not a value");
}

/// Plan 0047 Phase 1: `[generator] seed` stops being reserved — it is the salt,
/// for **any** system, not just the L-system whose table it has always sat in.
#[test]
fn generator_seed_becomes_the_preset_salt() {
    let salt_of = |table: &str| {
        Preset::from_toml_str(&format!("system = \"swarm\"\nparams = {{}}\n{table}"))
            .expect("valid preset")
            .salt
    };

    assert_eq!(salt_of(""), 0, "a preset declaring no seed is salted 0");
    assert_ne!(salt_of("[generator]\nseed = 7\n"), 0);
    assert_ne!(
        salt_of("[generator]\nseed = 7\n"),
        salt_of("[generator]\nseed = 8\n"),
        "two seeds must not fold to one salt"
    );
    // The declared key is a `u64`; a seed living entirely above bit 32 must not
    // fold away to nothing.
    assert_ne!(
        salt_of("[generator]\nseed = 4294967296\n"),
        0,
        "the high half of a 64-bit seed reaches the salt"
    );

    // ...and the salt is the one the preset's own expressions are evaluated
    // under, which is what makes two identical presets look different.
    let seeded = |seed: u32| {
        let preset = Preset::from_toml_str(&format!(
            "system = \"swarm\"\n[params]\nhue = \"hash(time)\"\n[generator]\nseed = {seed}\n"
        ))
        .expect("valid preset");
        preset
            .params
            .first()
            .expect("one binding")
            .expr
            .eval(&salted(1.0, preset.salt))
    };
    assert_ne!(
        seeded(1),
        seeded(2),
        "the same expression under two preset seeds must scatter differently"
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

/// Plan 0019 Phase 2: comparisons yield a clean `1.0`/`0.0` at the lowest
/// precedence tier, so they compose with arithmetic.
#[test]
fn comparisons_yield_one_or_zero_and_compose_with_arithmetic() {
    let zero = Variables::default();
    let eval = |src: &str| compile(src).expect("compiles").eval(&zero);

    assert_eq!(eval("1 + (2 > 1)"), 2.0);
    assert_eq!(eval("1 + (1 > 2)"), 1.0);

    // Each operator on both sides of its boundary, plus the boundary itself.
    assert_eq!(eval("2 > 2"), 0.0);
    assert_eq!(eval("2 < 2"), 0.0);
    assert_eq!(eval("2 >= 2"), 1.0);
    assert_eq!(eval("2 <= 2"), 1.0);
    assert_eq!(eval("3 >= 2"), 1.0);
    assert_eq!(eval("1 >= 2"), 0.0);
    assert_eq!(eval("1 <= 2"), 1.0);
    assert_eq!(eval("3 <= 2"), 0.0);
    assert_eq!(eval("2 == 2"), 1.0);
    assert_eq!(eval("2 == 3"), 0.0);
    assert_eq!(eval("2 != 3"), 1.0);
    assert_eq!(eval("2 != 2"), 0.0);

    // Lowest precedence: the sums on each side bind tighter than the compare.
    assert_eq!(eval("1 + 1 > 3 - 2"), 1.0);
    // The and/or/not idiom the grammar ships instead of boolean operators.
    let v = vars(0.8, 0.1, 0.0, 0.0, 0.0, 0.0, 0.0);
    let both = compile("min(bass > 0.5, mid > 0.5)").expect("compiles");
    let either = compile("max(bass > 0.5, mid > 0.5)").expect("compiles");
    let not_bass = compile("1 - (bass > 0.5)").expect("compiles");
    assert_eq!(both.eval(&v), 0.0);
    assert_eq!(either.eval(&v), 1.0);
    assert_eq!(not_bass.eval(&v), 0.0);
}

/// `select` returns the branch its condition picks — and evaluates **only**
/// that branch, so an untaken `NaN` cannot poison the result.
#[test]
fn select_picks_a_branch_without_evaluating_the_other() {
    let e = compile("select(bass > 0.5, 10, 20)").expect("compiles");
    assert_eq!(e.eval(&vars(0.9, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0)), 10.0);
    assert_eq!(e.eval(&vars(0.1, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0)), 20.0);
    // Exactly at the threshold the condition is false (`>`, not `>=`).
    assert_eq!(e.eval(&vars(0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0)), 20.0);

    let zero = Variables::default();
    let eval = |src: &str| compile(src).expect("compiles").eval(&zero);

    // The lazy-branch guarantee: the untaken sqrt(-1) never runs, so the
    // result is 5, not NaN. A lerp-based blend could not do this.
    assert_eq!(eval("select(0, sqrt(0 - 1), 5)"), 5.0);
    assert_eq!(eval("select(1, 5, sqrt(0 - 1))"), 5.0);

    // Truthiness is `!= 0.0`, so any nonzero condition takes the first branch.
    assert_eq!(eval("select(0 - 3, 1, 2)"), 1.0);
    assert_eq!(eval("select(0, 1, 2)"), 2.0);
}

/// A bare `!` or `=` is an explicit tokenizer error — they are only valid as
/// the two-char comparison forms.
#[test]
fn bare_bang_or_equals_is_a_compile_error() {
    for bad in ["1 ! 2", "1 = 2", "!bass", "bass = 1"] {
        assert!(
            compile(bad).is_err(),
            "expression {bad:?} should fail to compile"
        );
    }
    // A trailing bare `>` tokenizes as Gt and then fails in the parser (an
    // unexpected end), rather than panicking on the missing lookahead char.
    assert!(compile("bass >").is_err());
    assert!(compile("bass >=").is_err());
}

#[test]
fn beat_coerces_as_a_zero_one_value() {
    let e = compile("1.0 + beat * 0.5").expect("compiles");
    assert_eq!(e.eval(&vars(0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0)), 1.5);
    assert_eq!(e.eval(&vars(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0)), 1.0);
}

/// Plan 0019 Phase 3: `tempo` and `novelty` read through to their slots, and
/// appending them left the seven original variables on their own slots.
#[test]
fn tempo_and_novelty_read_through_without_shifting_the_other_slots() {
    // A distinct value per slot, so a mis-wired slot reads a wrong number
    // rather than coincidentally matching.
    let v = Variables::new(1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 128.0, 0.75);
    for (name, expected) in [
        ("bass", 1.0),
        ("mid", 2.0),
        ("treb", 3.0),
        ("onset", 4.0),
        ("beat", 5.0),
        ("bar", 6.0),
        ("time", 7.0),
        ("tempo", 128.0),
        ("novelty", 0.75),
    ] {
        let e = compile(name).unwrap_or_else(|err| panic!("{name} compiles: {err}"));
        assert_eq!(e.eval(&v), expected, "{name} reads its own slot");
    }

    // The idiom the comparisons enable for tempo's unbounded 60-200 scale.
    let gate = compile("select(tempo > 120, 1, 0)").expect("compiles");
    assert_eq!(gate.eval(&v), 1.0);
    let cold = Variables::default();
    assert_eq!(gate.eval(&cold), 0.0, "tempo is 0 before the tracker warms");
}

/// Plan 0041 review: every analysis variable reads the `AnalysisFrame` field it
/// is named for, asserted through `Variables::from_frame` — the one place that
/// mapping is written.
///
/// This is the guard the finding asked for. The renderer and `shot`'s
/// reachability probe both bind a frame; until this constructor existed they did
/// it with two hand-written copies of the same nine positional arguments, and
/// nothing would have caught them drifting apart — a probe reading `treb` where
/// the engine reads `mid` reports dead gates about an expression the renderer
/// never evaluates.
///
/// Deliberately asserted against the **grammar's own variable names** rather
/// than against a locally-written `Variables::new(..)`: comparing the
/// constructor to a second copy of itself would restate the duplication this
/// closes, in a test.
#[test]
fn from_frame_binds_every_analysis_variable_to_its_own_field() {
    // A distinct value per field, none of them a plausible neighbour, so a
    // crossed pair reads a wrong number rather than coincidentally matching.
    // `spectrum` is ramped so `bin()` cannot pass by reading a flat array.
    let frame = lmv_core::dsp::AnalysisFrame {
        bass: 0.11,
        mid: 0.22,
        treb: 0.33,
        onset: 0.44,
        beat: true,
        bar: 0.66,
        bpm: 128.0,
        novelty: 0.88,
        // The four raw levels (ADR-0049) are the pairing most worth guarding:
        // each sits beside the normalized value of the same name, so a crossed
        // slot would bind `bass` where `bass_raw` was asked for and the
        // difference is invisible in any test that only checks one of them.
        bass_raw: 0.011,
        mid_raw: 0.022,
        treb_raw: 0.033,
        onset_raw: 0.044,
        // ADR-0050 Layer 1. `beat_index` is the one variable that crosses a type
        // boundary — `u32` on the frame, `f32` in the grammar — so binding it to
        // the wrong slot would still produce a plausible number.
        beat_index: 37,
        time_since_beat: 0.123,
        beat_in_bar: 2,
        bar_index: 9,
        bar_phase: 0.625,
        // Diagnostics-only, and the assertion below that they are NOT bindable is
        // the point: ADR-0050 keeps the gate out of the grammar.
        downbeat_confidence: 0.77,
        downbeat_locked: true,
        spectrum: std::array::from_fn(|i| i as f32 / 64.0),
    };
    // Not on the frame: the renderer supplies its own clock here, the probe the
    // hop position it synthesized. That is why it stays an argument.
    let v = lmv_core::preset::Variables::from_frame(&frame, 7.5);

    for (name, expected) in [
        ("bass", 0.11),
        ("mid", 0.22),
        ("treb", 0.33),
        ("onset", 0.44),
        ("beat", 1.0),
        ("bar", 0.66),
        ("time", 7.5),
        ("tempo", 128.0),
        ("novelty", 0.88),
        ("bass_raw", 0.011),
        ("mid_raw", 0.022),
        ("treb_raw", 0.033),
        ("onset_raw", 0.044),
        ("beat_index", 37.0),
        ("time_since_beat", 0.123),
        ("beat_in_bar", 2.0),
        ("bar_index", 9.0),
        ("bar_phase", 0.625),
    ] {
        let e = compile(name).unwrap_or_else(|err| panic!("{name} compiles: {err}"));
        assert_eq!(
            e.eval(&v),
            expected,
            "`{name}` does not read the AnalysisFrame field it is named for"
        );
    }

    // `beat` is a bool on the frame and a float in the grammar, so the
    // conversion is part of the mapping and gets its own claim.
    let quiet = lmv_core::dsp::AnalysisFrame {
        beat: false,
        ..frame
    };
    let e = compile("beat").expect("compiles");
    assert_eq!(
        e.eval(&lmv_core::preset::Variables::from_frame(&quiet, 7.5)),
        0.0,
        "a frame without a beat must bind `beat` to 0"
    );

    // The band array comes across too, and by borrow — `bin()` reading the ramp
    // is what says the spectrum was attached at all.
    let last = compile("bin(1)").expect("compiles");
    assert_eq!(
        last.eval(&v),
        63.0 / 64.0,
        "from_frame did not attach the frame's spectrum"
    );

    // `index` is not audio: it belongs to a per-element evaluation and must
    // start at zero here rather than pick up a frame field.
    let index = compile("index").expect("compiles");
    assert_eq!(index.eval(&v), 0.0, "`index` is not fed by the frame");
}

/// Plan 0034 Phase 1: `bin(x)` samples the log-spaced band array at a
/// **normalized** position, interpolating between adjacent bands — so a preset
/// addresses a frequency region without ever naming `SPECTRUM_BINS`.
#[test]
fn bin_samples_the_spectrum_at_a_normalized_interpolated_position() {
    // Three deliberately non-monotonic bands, so hitting the right one is a
    // real claim rather than something a wrong index could match by luck.
    let spectrum = [0.2f32, 0.9, 0.4];
    let v = Variables::default().with_spectrum(&spectrum);
    let eval = |src: &str| compile(src).expect("compiles").eval(&v);

    // The endpoints are the first and last band exactly, not one short.
    assert_eq!(eval("bin(0)"), 0.2, "bin(0) is the first band");
    assert_eq!(eval("bin(1)"), 0.4, "bin(1) is the last band");
    // An interior position that lands exactly on a band reads it exactly.
    assert_eq!(
        eval("bin(0.5)"),
        0.9,
        "bin(0.5) is the middle band of three"
    );

    // Halfway between two bands is their linear interpolation, both on a
    // rising pair (0.2 -> 0.9) and a falling one (0.9 -> 0.4).
    assert!(
        (eval("bin(0.25)") - 0.55).abs() < 1e-6,
        "bin(0.25) interpolates 0.2 and 0.9, got {}",
        eval("bin(0.25)")
    );
    assert!(
        (eval("bin(0.75)") - 0.65).abs() < 1e-6,
        "bin(0.75) interpolates 0.9 and 0.4, got {}",
        eval("bin(0.75)")
    );

    // The argument is an expression like any other, so a computed position
    // works — this is what Phase 4's `bin(index)` rests on.
    assert_eq!(eval("bin(0.25 + 0.25)"), 0.9);
}

/// `bin` runs per binding per frame, so it must be **total**: out-of-range
/// clamps, `NaN` yields a finite value, an absent spectrum reads zero, and no
/// input path panics.
#[test]
fn bin_is_total_for_every_input() {
    let spectrum = [0.2f32, 0.9, 0.4];
    let v = Variables::default().with_spectrum(&spectrum);
    let eval = |src: &str| compile(src).expect("compiles").eval(&v);

    // Out of range clamps to the ends rather than erroring or reading past.
    assert_eq!(
        eval("bin(0 - 5)"),
        0.2,
        "below zero clamps to the first band"
    );
    assert_eq!(eval("bin(17)"), 0.4, "above one clamps to the last band");

    // NaN and infinities fold to a finite value (`f32::max` returns the
    // non-NaN operand, so a NaN position clamps to the first band).
    for degenerate in ["bin(sqrt(0 - 1))", "bin(1 / 0)", "bin(0 - 1 / 0)"] {
        let got = compile(degenerate).expect("compiles").eval(&v);
        assert!(got.is_finite(), "{degenerate} must stay finite, got {got}");
    }

    // No spectrum bound at all — an expression evaluated outside the render
    // loop reads a flat zero instead of misbehaving.
    let bare = Variables::default();
    assert_eq!(compile("bin(0.5)").expect("compiles").eval(&bare), 0.0);

    // A single-band spectrum is degenerate (there is no "next" band to
    // interpolate toward) and still answers everywhere.
    let one = [0.7f32];
    let single = Variables::default().with_spectrum(&one);
    for src in ["bin(0)", "bin(0.5)", "bin(1)"] {
        assert_eq!(compile(src).expect("compiles").eval(&single), 0.7);
    }
}

/// `bin` is a registered function, so an arity mistake is the same surfaced
/// load error as any other call — not a silently-different meaning.
#[test]
fn bin_is_a_known_function_with_arity_one() {
    assert!(matches!(
        compile("bin(0.1, 0.2)"),
        Err(lmv_core::preset::ExprError::WrongArity { func, expected: 1, got: 2 }) if func == "bin"
    ));
    assert!(compile("bin()").is_err(), "zero arguments is an error");
    // Bare `bin` is not a variable — the spectrum is reachable only by call.
    assert!(matches!(
        compile("bin"),
        Err(lmv_core::preset::ExprError::UnknownIdent(name)) if name == "bin"
    ));
}

/// Plan 0034 Phase 4 done-when 1: `index` is an ordinary variable that reads
/// **`0`** outside a per-element evaluation, and appending it left the nine
/// analysis slots exactly where they were.
#[test]
fn index_reads_zero_outside_a_per_element_evaluation() {
    let e = compile("index").expect("compiles");
    assert_eq!(
        e.eval(&Variables::default()),
        0.0,
        "outside a per-element evaluation `index` is 0, not undefined"
    );
    // Every analysis variable still reads its own value, so the new slot did not
    // shift any of them.
    let v = Variables::new(1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 128.0, 0.75);
    assert_eq!(e.eval(&v), 0.0, "`index` is not fed by the analysis frame");
    for (name, expected) in [("bass", 1.0), ("novelty", 0.75), ("tempo", 128.0)] {
        let read = compile(name).unwrap_or_else(|err| panic!("{name} compiles: {err}"));
        assert_eq!(read.eval(&v), expected, "{name} kept its slot");
    }

    // `with_index` rebinds only that slot.
    let at_half = v.with_index(0.5);
    assert_eq!(e.eval(&at_half), 0.5);
    assert_eq!(
        compile("bass").expect("compiles").eval(&at_half),
        1.0,
        "rebinding index leaves the analysis variables alone"
    );

    // The per-element flag is a property of the source text, decided at compile.
    for (src, per_element) in [
        ("index", true),
        ("0.01 + bin(index) * 0.05", true),
        ("select(index > 0.5, 1, 0)", true),
        ("bass * 2", false),
        ("bin(0.4)", false),
    ] {
        assert_eq!(
            compile(src).expect("compiles").uses_index(),
            per_element,
            "{src:?} per-element flag"
        );
    }
}

/// Per-element evaluation multiplies the per-frame work by the element count, so
/// the thing it must never do is allocate. `with_index` rebinds one float on a
/// `Copy` bundle that borrows (not owns) the spectrum.
#[test]
fn per_element_evaluation_performs_no_heap_allocation() {
    let spectrum = [0.4f32; 64];
    let e = compile("0.01 + bin(index) * 0.05 + bass").expect("compiles");
    let v = vars(0.5, 0.2, 0.1, 0.0, 1.0, 0.3, 1.23).with_spectrum(&spectrum);

    // Warm up (touch any lazy statics before measuring).
    let _ = e.eval(&v.with_index(0.0));

    let before = alloc_count();
    let mut acc = 0.0f32;
    // 240 frames at 24 elements — a minute of a default readout.
    for _ in 0..240 {
        for i in 0..24 {
            acc += e.eval(&v.with_index(i as f32 / 23.0));
        }
    }
    let after = alloc_count();

    assert!(acc.is_finite(), "sanity: evaluation produced a real number");
    assert_eq!(
        before,
        after,
        "per-element evaluation must not allocate; saw {} allocation(s)",
        after - before
    );
}

/// A `[smoothing]` entry naming a per-element binding cannot work — the smoother
/// holds one scalar and a series has no single value — so it is a **surfaced
/// warning** rather than a silent no-op, and it points at the table that does.
#[test]
fn smoothing_a_per_element_binding_warns_instead_of_doing_nothing() {
    let preset = Preset::from_toml_str(
        "system = \"spectrum\"\n[spectrum]\nelements = 8\n\
         [params]\nthickness = \"2 + bin(index) * 8\"\nbrightness = \"0.5 + bass\"\n\
         [smoothing]\nthickness = 0.3\nbrightness = 0.3\n",
    )
    .expect("valid preset");

    assert_eq!(preset.warnings.len(), 1, "only the per-element one warns");
    let warning = preset.warnings.first().expect("the warning");
    assert!(
        warning.contains("thickness") && warning.contains("index"),
        "the warning names the binding and why: {warning}"
    );
    assert!(
        warning.contains("[spectrum] smoothing"),
        "the warning points at the table that does work: {warning}"
    );

    // The scalar binding's easing is untouched, and the ignored one is instant.
    let tau_of = |name: &str| {
        preset
            .params
            .iter()
            .find(|b| b.name == name)
            .map(|b| b.tau)
            .unwrap_or_else(|| panic!("{name} is bound"))
    };
    assert_eq!(
        tau_of("brightness"),
        lmv_core::preset::Easing::symmetric(0.3)
    );
    assert_eq!(tau_of("thickness"), lmv_core::preset::Easing::INSTANT);
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

/// Plan 0019 Phase 4: an unknown parameter name is a **warning**, not a
/// rejection — the preset loads and its good bindings survive.
#[test]
fn an_unknown_param_warns_but_still_loads_the_preset() {
    let src = r#"
system = "fragment_field"
name = "Typo Field"

[params]
warp = "0.4"
wrap = "0.4"
"#;
    let preset = Preset::from_toml_str(src).expect("a typo does not reject the preset");

    // The good binding is present and applied.
    assert!(
        preset.params.iter().any(|b| b.name == "warp"),
        "the real param survived the typo"
    );

    // Exactly one warning, naming the offending param and the system.
    assert_eq!(preset.warnings.len(), 1, "one unknown param, one warning");
    let warning = preset.warnings.first().expect("the warning");
    assert!(
        warning.contains("wrap"),
        "the warning names the bad param: {warning}"
    );
    assert!(
        warning.contains("fragment_field"),
        "the warning names the system: {warning}"
    );
    assert!(
        !warning.contains("'warp'"),
        "the warning does not blame the good param: {warning}"
    );
}

/// A preset binding only known names — including the **global** compositing
/// params any system may bind — warns about nothing.
#[test]
fn known_params_including_global_ones_produce_no_warnings() {
    let src = r#"
system = "swarm"

[params]
force      = "1.2"
hue        = "0.3"
bg_hue     = "0.5"
trails     = "0.4"
kaleido_order = "6"
ink_amount = "0.8"
paper_hue  = "0.1"
"#;
    let preset = Preset::from_toml_str(src).expect("valid preset");
    assert!(
        preset.warnings.is_empty(),
        "known params must not warn, got {:?}",
        preset.warnings
    );

    // Every shipped preset is clean — the check must not cry wolf on the
    // curated library.
    for &(name, src) in lmv_core::preset::EMBEDDED {
        let preset = Preset::from_toml_str(src)
            .unwrap_or_else(|err| panic!("embedded preset {name} parses: {err}"));
        assert!(
            preset.warnings.is_empty(),
            "embedded preset {name} warns: {:?}",
            preset.warnings
        );
    }
}

/// `load_dir` aggregates each loaded preset's warnings with its path, so a
/// frontend can point the author at the file.
#[test]
fn load_dir_reports_warnings_alongside_the_loaded_presets() {
    let dir = std::env::temp_dir().join("lmv_warn_dir_test");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    std::fs::write(
        dir.join("typo.toml"),
        "system = \"fragment_field\"\n[params]\nwarp = \"0.4\"\nwrap = \"0.4\"\n",
    )
    .expect("write preset");
    std::fs::write(
        dir.join("clean.toml"),
        "system = \"fragment_field\"\n[params]\nwarp = \"0.4\"\n",
    )
    .expect("write preset");

    let report = lmv_core::preset::load_dir(&dir);
    assert_eq!(report.presets.len(), 2, "both presets load");
    assert!(report.errors.is_empty(), "a typo is not an error");
    assert_eq!(report.warnings.len(), 1, "only the typo warns");
    let (path, warning) = report.warnings.first().expect("the warning");
    assert!(
        path.ends_with("typo.toml"),
        "the warning names the file: {}",
        path.display()
    );
    assert!(warning.contains("wrap"), "the warning names the param");

    let _ = std::fs::remove_dir_all(&dir);
}

/// Plan 0034 Phase 3 done-when 1: `[spectrum]` is validated at the load
/// boundary. A bad element count or an unknown layout name is a **surfaced
/// error** naming what was expected — never a panic and never a silent fallback,
/// matching every other declarative config (ADR-0007).
#[test]
fn a_bad_spectrum_table_is_a_surfaced_load_error() {
    let with = |table: &str| {
        Preset::from_toml_str(&format!(
            "system = \"spectrum\"\n[params]\nbase = \"0.2\"\n[spectrum]\n{table}\n"
        ))
    };

    // An unknown layout names the offender and lists what it accepts, so the
    // author can fix it without reading the source.
    let Err(err) = with("layout = \"waterfall\"") else {
        panic!("an unknown layout is rejected");
    };
    let message = err.to_string();
    for expected in ["waterfall", "bars", "polyline", "radial_ring"] {
        assert!(
            message.contains(expected),
            "the error must mention {expected}: {message}"
        );
    }

    // The count is bounded on both sides: one element has no figure to draw, and
    // above the band count the 64 -> N reduction stops being a partition.
    for bad in [
        "elements = 0",
        "elements = 1",
        "elements = 65",
        "elements = 4096",
    ] {
        let err = with(bad)
            .err()
            .unwrap_or_else(|| panic!("{bad} is rejected"));
        assert!(
            err.to_string().contains("2..=64"),
            "{bad} must name the accepted range: {err}"
        );
    }

    // A negative easing constant is caught by the same check the `[smoothing]`
    // table uses, and the message says which table it came from.
    let Err(err) = with("smoothing = -0.5") else {
        panic!("a negative easing constant is rejected");
    };
    assert!(
        err.to_string().contains("[spectrum]"),
        "the error must name the table it came from: {err}"
    );

    // The whole table is optional, and every key within it is.
    for good in [
        "",
        "elements = 2",
        "layout = \"radial_ring\"",
        "smoothing = 0.2",
    ] {
        assert!(
            with(good).is_ok(),
            "a spectrum preset with `{good}` must load"
        );
    }
    assert!(
        Preset::from_toml_str("system = \"spectrum\"\n").is_ok(),
        "the [spectrum] table is optional"
    );
}

/// Param names that are deliberately absent from `presets/README.md`.
///
/// **Empty, and a new entry needs a reason next to it** (the shape ADR-0058 uses
/// for evidence). A name here is a promise that an author never has to reach for
/// it; an unexplained skip is how the third copy drifts in the first place.
const README_EXEMPT: &[(&str, &str)] = &[
    // ("param_name", "why an author never binds this"),
];

/// **The third copy of every parameter name** (Plan 0061 Phase 7).
///
/// The name of a param exists in three places: the scene's `PARAMS` list, its
/// `set_param` match, and `presets/README.md`. `declared_params_match_set_param`
/// below guards code against code. Nothing guarded code against the **doc**, and
/// that doc is the surface `preset-author` composes against (ADR-0017) — a param
/// missing from it is a capability the content lane cannot discover exists.
///
/// The scan is recursive rather than a hand-kept file list precisely so a new
/// scene is covered the day it lands, without anyone remembering to add it here.
/// It matches the README's own convention — a documented param appears in
/// backticks — so a bare mention of `size` inside `--size` does not count as
/// documenting the `size` param.
#[test]
fn every_declared_param_is_documented_in_the_presets_readme() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let readme_path = root
        .parent()
        .expect("core has a workspace-root parent")
        .join("presets/README.md");
    let readme = std::fs::read_to_string(&readme_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", readme_path.display()));

    let mut sources = Vec::new();
    collect_rs(&root.join("src"), &mut sources);

    let mut checked = 0usize;
    let mut undocumented: Vec<String> = Vec::new();
    for file in &sources {
        let text = std::fs::read_to_string(file)
            .unwrap_or_else(|e| panic!("read {}: {e}", file.display()));
        for name in params_declared_in(&text) {
            if README_EXEMPT.iter().any(|(n, _)| *n == name) {
                continue;
            }
            checked += 1;
            if !readme.contains(&format!("`{name}`")) {
                undocumented.push(format!(
                    "  `{name}` (declared in {})",
                    file.strip_prefix(root).unwrap_or(file).display()
                ));
            }
        }
    }

    assert!(
        checked > 100,
        "only {checked} params scanned — the PARAMS scan has stopped finding \
         declarations, so this guard would pass vacuously"
    );
    assert!(
        undocumented.is_empty(),
        "{} declared parameter(s) are missing from presets/README.md:\n{}\n\
         Document each one there, or add it to README_EXEMPT with a reason.\n\
         That file is the surface `preset-author` writes against (ADR-0017): an \
         undocumented param is a capability the content lane cannot find.",
        undocumented.len(),
        undocumented.join("\n"),
    );
}

/// Every `.rs` under `dir`, recursively.
fn collect_rs(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    let entries = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("read_dir {}: {e}", dir.display()))
        .map(|e| e.expect("dir entry").path());
    let mut entries: Vec<_> = entries.collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect_rs(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// The string literals of a `const PARAMS … = [ … ];` declaration, if the file
/// has one. Both spellings in the tree are covered: `&[&str]` slices and the
/// `[&str; N]` array `marks.rs` uses.
fn params_declared_in(text: &str) -> Vec<String> {
    let Some(start) = text.find("const PARAMS") else {
        return Vec::new();
    };
    let Some(open) = text.get(start..).and_then(|t| t.find('[')) else {
        return Vec::new();
    };
    let body_start = start + open;
    let Some(end) = text.get(body_start..).and_then(|t| t.find("];")) else {
        return Vec::new();
    };
    let body = text.get(body_start..body_start + end).unwrap_or_default();

    let mut names = Vec::new();
    let mut rest = body;
    while let Some(q) = rest.find('"') {
        let after = rest.get(q + 1..).unwrap_or_default();
        let Some(close) = after.find('"') else { break };
        names.push(after.get(..close).unwrap_or_default().to_string());
        rest = after.get(close + 1..).unwrap_or_default();
    }
    names
}

/// Drift guard (ADR-0020's flagged risk): each declared `PARAMS` list must be
/// exactly the set of names its `set_param` match handles. The two sit side by
/// side in the source, so this compares them by scanning it — which covers the
/// GPU-backed scenes a headless test cannot instantiate.
#[test]
fn declared_params_match_set_param() {
    use lmv_core::preset::SystemKind;

    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");

    // (source file, the declared vocabulary it must match).
    let scenes: Vec<(std::path::PathBuf, &[&str])> = vec![
        (
            src.join("render/scenes/fragment_field.rs"),
            SystemKind::FragmentField.param_names(),
        ),
        (
            src.join("render/scenes/swarm.rs"),
            SystemKind::Swarm.param_names(),
        ),
        (
            src.join("render/scenes/lines/parametric.rs"),
            SystemKind::ParametricCurve.param_names(),
        ),
        (
            src.join("render/scenes/lines/lsystem.rs"),
            SystemKind::LSystem.param_names(),
        ),
        (
            src.join("render/scenes/lines/star.rs"),
            SystemKind::StarPattern.param_names(),
        ),
        (
            src.join("render/scenes/reaction_diffusion.rs"),
            SystemKind::ReactionDiffusion.param_names(),
        ),
        (
            src.join("render/scenes/particles/mod.rs"),
            SystemKind::Attractor.param_names(),
        ),
        (
            src.join("render/scenes/lines/spectrum.rs"),
            SystemKind::Spectrum.param_names(),
        ),
        (
            src.join("render/scenes/emitter.rs"),
            SystemKind::Emitter.param_names(),
        ),
        // The global compositing stages, declared the same way.
        (
            src.join("render/background.rs"),
            &[
                "bg_hue",
                "bg_bright",
                "bg_vignette",
                // The ramp (ADR-0094): a palette segment swept along one axis,
                // with the brightness tilt that used to be hardcoded retired
                // into the two ends of a ramp on that same axis.
                "bg_angle",
                "bg_hue_span",
                "bg_shade",
                "bg_shade_end",
                "bg_ramp_gamma",
                // The band (ADR-0095): one gaussian swell added over that
                // ground, across an axis of its own.
                "bg_band_amount",
                "bg_band_angle",
                "bg_band_pos",
                "bg_band_width",
            ],
        ),
        (
            src.join("render/trails.rs"),
            &[
                "trails",
                // ADR-0048's transform on the accumulation this stage already
                // owns: the affine, then the centre it turns about.
                "fb_zoom",
                "fb_rotate",
                "fb_dx",
                "fb_dy",
                "fb_center_x",
                "fb_center_y",
                // The strength of whichever `[feedback] warp` the preset selected;
                // the kind itself is structural and never a param.
                "fb_warp",
            ],
        ),
        (
            src.join("render/kaleidoscope.rs"),
            &[
                "kaleido_order",
                "kaleido_angle",
                "kaleido_center_x",
                "kaleido_center_y",
                "kaleido_edge",
                // ADR-0077's composed map, in the order the stage applies it
                // (destination-to-source): tile -> fold -> radial -> spiral.
                "kaleido_tile",
                "kaleido_radial",
                "kaleido_spiral",
                "kaleido_zoom",
                "kaleido_inner",
            ],
        ),
        (
            src.join("render/ink.rs"),
            &[
                "ink_amount",
                "paper_hue",
                "paper_sat",
                "paper_bright",
                "ink_hue",
                "ink_sat",
                "ink_bright",
                // ADR-0092's response exponent on the luminance key.
                "ink_gamma",
            ],
        ),
    ];

    for (file, declared) in &scenes {
        let text = std::fs::read_to_string(file)
            .unwrap_or_else(|e| panic!("read {}: {e}", file.display()));
        let handled = set_param_arm_names(&text, file);

        // The `fb_*` seven are **deliberately invisible to this scan** (ADR-0048).
        // Both sinks that declare them — the trails stage and the attractor —
        // delegate to one shared `feedback::Transform::set_param` rather than
        // matching seven names each, which is the right factoring (one definition
        // of what `fb_dx` means) and costs exactly this text scan's coverage. What
        // replaces it is `feedback.rs`'s
        // `both_sinks_declare_exactly_the_shared_fb_vocabulary`, which asserts the
        // same property programmatically and across both sinks at once.
        let mut declared_sorted: Vec<&str> = declared
            .iter()
            .copied()
            .filter(|name| !name.starts_with("fb_"))
            .collect();
        declared_sorted.sort_unstable();
        let mut handled_sorted: Vec<String> = handled.clone();
        handled_sorted.sort();

        assert_eq!(
            declared_sorted,
            handled_sorted
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            "{}: the declared PARAMS list and the set_param match have drifted. \
             Update whichever is wrong so the two stay in sync.",
            file.display(),
        );
    }
}

/// **The three additive-particle scenes spell their level lever the same way**
/// (Plan 0066 Phase 1 / ADR-0080).
///
/// `swarm`, `emitter` and `attractor` all draw additive particle marks, and until
/// this plan only two of them could say how bright. The one without it is the one
/// whose shipped presets reached for the engine-wide `exposure` instead — which
/// crossfades across a dissolve and sits downstream of the bloom bright-pass, so
/// the substitution cost them both.
///
/// Pinned on the **name**, not just on the capability, because ADR-0080
/// Alternative D is exactly the temptation this guards against: a distinct name
/// for the attractor's (`intensity`, on the grounds that its deposit accumulates
/// into a trail field) would leave three sibling scenes with two words for one
/// lever, and `presets/README.md` explaining the distinction on every mention.
#[test]
fn the_additive_particle_scenes_share_one_level_param() {
    use lmv_core::preset::SystemKind;

    for system in [
        SystemKind::Swarm,
        SystemKind::Emitter,
        SystemKind::Attractor,
    ] {
        assert!(
            system.param_names().contains(&"brightness"),
            "{system:?} draws additive particle marks and must expose the level \
             lever under the shared name `brightness` (ADR-0080); it declares \
             {:?}",
            system.param_names()
        );
    }
}

/// The parameter names a file's `set_param` body matches on, read from the
/// source: every `"name" =>` arm, plus the `if name == "…"` single-param form
/// `trails` uses. Scanning the source is what lets this guard cover scenes that
/// need a GPU device to instantiate.
fn set_param_arm_names(text: &str, file: &std::path::Path) -> Vec<String> {
    let start = text
        .find("fn set_param")
        .unwrap_or_else(|| panic!("{}: no set_param found", file.display()));
    // The body ends at the first line that closes the fn at 4-space indent.
    let rest = &text[start..];
    let end = rest
        .find("\n    }\n")
        .unwrap_or_else(|| panic!("{}: set_param body is not 4-space indented", file.display()));
    let body = &rest[..end];

    let mut names = Vec::new();
    for line in body.lines() {
        let line = line.trim();
        if line.starts_with("//") {
            continue;
        }
        // `"name" => …` match arms.
        if let Some(rest) = line.strip_prefix('"')
            && let Some((name, tail)) = rest.split_once('"')
            && tail.trim_start().starts_with("=>")
        {
            names.push(name.to_string());
            continue;
        }
        // The `if name == "…"` single-param form.
        if let Some(rest) = line.strip_prefix("if name == \"")
            && let Some((name, _)) = rest.split_once('"')
        {
            names.push(name.to_string());
        }
    }
    assert!(
        !names.is_empty(),
        "{}: parsed no set_param arms — the scan is broken, not the code",
        file.display()
    );
    names
}

#[test]
fn compiled_eval_performs_no_heap_allocation() {
    // The seeded pair rides in here too (Plan 0047): they are the newest thing on
    // the per-parameter-per-frame path, and "allocation-free" is a claim their
    // docs make.
    let e = compile(
        "clamp(bass * 2 + sin(time), 0, 1) + lerp(mid, treb, bar) + hash(time) + noise(time)",
    )
    .expect("compiles");
    let v = vars(0.5, 0.2, 0.1, 0.0, 1.0, 0.3, 1.23).with_salt(31);

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

/// The `[feedback]` table (ADR-0048) is two **closed rosters**, validated at the
/// load boundary: an unknown warp kind or blend mode rejects the preset with a
/// message naming what was expected.
///
/// A load error rather than an ADR-0020 warning, and the difference is the point.
/// An unknown *param* name is a warning because one typo must not discard an
/// otherwise-good preset, and the binding it names simply reaches nobody. A
/// structural key selects a **code path**: falling back to the default here would
/// render a look the author never asked for, with nothing on screen to say so.
#[test]
fn the_feedback_table_parses_its_rosters_and_rejects_anything_else() {
    use lmv_core::render::feedback::{Deposit, Warp};

    // Absent table: both defaults, i.e. the identity in both fields.
    let bare = Preset::from_toml_str("system = \"swarm\"").expect("valid");
    assert_eq!(bare.feedback.warp, Warp::None);
    assert_eq!(bare.feedback.blend, Deposit::Max);

    // Every kind in the roster round-trips through its declared name.
    for kind in Warp::ALL {
        let src = format!(
            "system = \"swarm\"\n[feedback]\nwarp = \"{}\"\n",
            kind.as_str()
        );
        let preset = Preset::from_toml_str(&src)
            .unwrap_or_else(|e| panic!("warp '{}' must parse: {e}", kind.as_str()));
        assert_eq!(preset.feedback.warp, kind);
        assert_eq!(
            preset.feedback.blend,
            Deposit::Max,
            "a table naming only `warp` still defaults its blend"
        );
    }

    // Both blends, likewise.
    for (name, want) in [("max", Deposit::Max), ("add", Deposit::Add)] {
        let src = format!("system = \"swarm\"\n[feedback]\nblend = \"{name}\"\n");
        let preset = Preset::from_toml_str(&src)
            .unwrap_or_else(|e| panic!("blend '{name}' must parse: {e}"));
        assert_eq!(preset.feedback.blend, want);
        assert_eq!(preset.feedback.warp, Warp::None);
    }

    // An unknown value is a surfaced load error, and the message says what was
    // expected — the author has to be able to find the roster from the failure.
    let bad_warp = Preset::from_toml_str("system = \"swarm\"\n[feedback]\nwarp = \"vortex\"\n")
        .expect_err("an unknown warp kind must be rejected");
    let text = bad_warp.to_string();
    assert!(
        text.contains("vortex") && text.contains("swirl"),
        "the rejection must name the bad value and the roster, got: {text}"
    );

    let bad_blend = Preset::from_toml_str("system = \"swarm\"\n[feedback]\nblend = \"screen\"\n")
        .expect_err("an unknown blend must be rejected");
    let text = bad_blend.to_string();
    assert!(
        text.contains("screen") && text.contains("add"),
        "the rejection must name the bad value and the roster, got: {text}"
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

/// Plan 0038 Phase 9. `log(0)` is `-inf`, which silence produces every time the
/// music stops — and a `[smoothing]`-listed binding that sees it must not be
/// dead for the rest of the preset's run.
///
/// **The defect was never one bad frame; it was permanence.** `Easing::step`
/// computes `held + alpha * (raw - held)`, so a `-inf` held against a `-inf` raw
/// is `-inf + alpha*NaN` = `NaN` — and `NaN` is absorbing there, because
/// `raw > held` is false for *every* `raw`, so the release branch is taken and
/// the result stays `NaN` no matter what the audio does afterwards. Only a preset
/// switch, which resets the smoother, cleared it.
///
/// So this asserts the **recovery**, not the absence of a `NaN`. Asserting only
/// that one frame survives would pass on a fix that leaves the state poisoned.
#[test]
fn a_non_finite_value_cannot_poison_a_smoother_permanently() {
    use lmv_core::preset::Easing;
    const DT: f32 = 1.0 / 60.0;
    let tau = Easing::symmetric(0.25);

    // Silence, through the dB idiom `docs/presets.md` documents.
    let silent = (0.0f32).ln();
    assert!(silent.is_infinite(), "log(0) is the reachable path here");

    // The render layer seeds its state with the first value it sees, so the
    // held value and the raw value are both -inf on the frame after a reset.
    let mut held = tau.step(silent, silent, DT);
    assert!(
        !held.is_nan(),
        "a -inf held against a -inf raw produced NaN — this is the frame the \
         binding used to die on"
    );

    // Audio returns, and stays. The binding must track it again.
    for _ in 0..600 {
        held = tau.step(held, 0.5, DT);
    }
    assert!(
        (held - 0.5).abs() < 0.001,
        "the binding never recovered: after 10 s of a finite 0.5 input it holds \
         {held}. A smoother that cannot come back from silence is the defect, \
         not the single frame that started it"
    );

    // The same for the other reachable non-finite: `sqrt(-1)` is NaN, and it
    // must not stick either.
    let mut held = 0.5f32;
    held = tau.step(held, (-1.0f32).sqrt(), DT);
    for _ in 0..600 {
        held = tau.step(held, 0.25, DT);
    }
    assert!(
        (held - 0.25).abs() < 0.001,
        "a NaN input poisoned the smoother permanently: holds {held}"
    );
}

// ---------------------------------------------------------------------------
// Probed evaluation and reachability (Plan 0041 Phase 2 / ADR-0042)
// ---------------------------------------------------------------------------

/// A deterministic sweep of variable bindings spanning both scales the harness
/// reads at: realistic band levels (bass ~0.04, mid/treb ~0.006) and the
/// full-scale `1.0` the report has always used, plus the odd extreme. Fixed LCG,
/// no wall clock, so a failure reproduces exactly.
fn variable_sweep(spectrum: &[f32]) -> Vec<Variables<'_>> {
    let mut state: u32 = 0x5eed_1041;
    let mut next = |hi: f32| {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (state >> 8) as f32 / (1u32 << 24) as f32 * hi
    };
    let mut out = Vec::new();
    for i in 0..256 {
        // Half the samples at music-like magnitudes, half at full scale, so an
        // expression gated anywhere in between is crossed by the sweep.
        let scale = if i % 2 == 0 { 0.12 } else { 1.0 };
        out.push(
            Variables::new(
                next(scale),
                next(scale),
                next(scale),
                next(scale),
                f32::from(i % 3 == 0),
                next(1.0),
                next(120.0),
                // `tempo` is a BPM, not a 0..1 band — and 0 while the tracker
                // is still cold.
                if i % 7 == 0 { 0.0 } else { 60.0 + next(140.0) },
                next(scale),
            )
            .with_spectrum(spectrum)
            .with_index(next(1.0)),
        );
    }
    out
}

/// Equality that treats two NaNs as equal — an expression is allowed to produce
/// NaN (`sqrt(-1)`), and both paths must agree on that too.
fn same_value(a: f32, b: f32) -> bool {
    a == b || (a.is_nan() && b.is_nan())
}

#[test]
fn probed_evaluation_returns_exactly_what_eval_returns_across_the_library() {
    // The risk ADR-0042 names as this approach's main cost: a second evaluation
    // path that quietly disagrees with the one the render loop runs, making the
    // report describe a preset that does not exist. Pinned over the real shipped
    // set rather than a spot check.
    let spectrum: Vec<f32> = (0..64).map(|i| (i as f32 * 0.017).sin().abs()).collect();
    let sweep = variable_sweep(&spectrum);
    let presets = lmv_core::preset::default_presets();
    assert!(!presets.is_empty(), "the embedded library is not empty");

    let mut checked = 0usize;
    for preset in &presets {
        for binding in &preset.params {
            let mut obs = lmv_core::preset::Observations::new();
            for v in &sweep {
                let plain = binding.expr.eval(v);
                let probed = binding.expr.eval_probed(v, &mut obs);
                assert!(
                    same_value(plain, probed),
                    "`{}` param `{}`: eval gave {plain}, eval_probed gave {probed}",
                    preset.name,
                    binding.name,
                );
                checked += 1;
            }
        }
    }
    assert!(
        checked > 1000,
        "expected the library to supply a substantial number of evaluations, got {checked}"
    );
}

#[test]
fn a_bare_comparison_is_observed_two_valued_and_arithmetic_is_not_observed_at_all() {
    // The shape Plan 0041 was blind to (ADR-0043): `reseed = "onset > 0.55"` is
    // the idiomatic boolean-param form and holds no `select()`, so nothing in
    // the tree used to be observed. `onset` is raw spectral flux — it never
    // approaches 0.55 in real material, which is how all five attractor presets
    // shipped without ever reseeding.
    let spectrum = [0.0f32; 64];
    let dead = compile("onset > 0.55").expect("compiles");
    let mut obs = lmv_core::preset::Observations::new();
    for onset in [0.0f32, 0.004, 0.016, 0.1, 0.3] {
        dead.eval_probed(&vars(0.04, 0.006, 0.006, onset, 0.0, 0.0, 0.0), &mut obs);
    }
    assert_eq!(
        obs.node(0),
        lmv_core::preset::NodeObservation::Compare {
            saw_true: false,
            saw_false: true,
        },
        "a threshold nothing in the run crosses is observed one-sided"
    );

    // The same expression against stimuli that straddle 0.55 records both.
    let mut obs = lmv_core::preset::Observations::new();
    for v in &variable_sweep(&spectrum) {
        dead.eval_probed(v, &mut obs);
    }
    assert_eq!(
        obs.node(0),
        lmv_core::preset::NodeObservation::Compare {
            saw_true: true,
            saw_false: true,
        },
        "the sweep drives `onset` past 0.55, so the same node is two-valued"
    );

    // Arithmetic carries no branch, so it is not observed — the arena stays
    // sparse rather than growing a slot for every node in the tree.
    let arithmetic = compile("bass + mid").expect("compiles");
    let mut obs = lmv_core::preset::Observations::new();
    for v in &variable_sweep(&spectrum) {
        arithmetic.eval_probed(v, &mut obs);
    }
    assert!(
        obs.nodes()
            .iter()
            .all(|n| *n == lmv_core::preset::NodeObservation::Untouched),
        "an arithmetic tree records nothing: {:?}",
        obs.nodes()
    );
}

#[test]
fn a_condition_that_never_crosses_is_reported_one_sided() {
    // A threshold past even full scale, so this select can only ever pick `y`.
    let dead = compile("select(bass > 1.5, 10, 2)").expect("compiles");
    let spectrum = [0.0f32; 64];
    let mut obs = lmv_core::preset::Observations::new();
    for v in &variable_sweep(&spectrum) {
        dead.eval_probed(v, &mut obs);
    }
    let flags = dead.flag_gates(&obs);
    assert_eq!(
        flags.len(),
        1,
        "expected exactly one flagged gate: {flags:?}"
    );
    let flag = flags.first().expect("one flag");
    assert_eq!(
        flag.kind,
        lmv_core::preset::GateKind::Select { always: false },
        "the condition never went true, so the `10` branch is dead"
    );
    assert_eq!(
        flag.source, "bass > 1.5",
        "the flag names the condition, so an author knows what to re-gain"
    );

    // ...and a threshold the sweep does cross reports nothing.
    let live = compile("select(bass > 0.05, 10, 2)").expect("compiles");
    let mut obs = lmv_core::preset::Observations::new();
    for v in &variable_sweep(&spectrum) {
        live.eval_probed(v, &mut obs);
    }
    assert!(
        live.flag_gates(&obs).is_empty(),
        "a condition that goes both ways is not a finding"
    );
}

#[test]
fn a_comparison_that_is_a_selects_own_condition_is_not_reported_twice() {
    // ADR-0043's suppression rule. The `select()` already names this condition,
    // and in better words — it can say which branch never ran, which a bare
    // comparison flag cannot. Reporting both would double every finding the
    // check already makes.
    let e = compile("select(bass > 1.5, 10, 2)").expect("compiles");
    let spectrum = [0.0f32; 64];
    let mut obs = lmv_core::preset::Observations::new();
    for v in &variable_sweep(&spectrum) {
        e.eval_probed(v, &mut obs);
    }
    let flags = e.flag_gates(&obs);
    assert_eq!(flags.len(), 1, "one gate is one finding: {flags:?}");
    assert_eq!(
        flags.first().map(|f| f.kind),
        Some(lmv_core::preset::GateKind::Select { always: false }),
        "the surviving flag is the select one, not the comparison"
    );
}

#[test]
fn a_bare_comparison_is_reported_as_its_own_gate() {
    // The reseed shape: no `select()` anywhere, so nothing suppresses it and
    // nothing used to report it either.
    let e = compile("onset > 0.55").expect("compiles");
    let mut obs = lmv_core::preset::Observations::new();
    for onset in [0.0f32, 0.004, 0.016, 0.1, 0.3] {
        e.eval_probed(&vars(0.04, 0.006, 0.006, onset, 0.0, 0.0, 0.0), &mut obs);
    }
    let flags = e.flag_gates(&obs);
    assert_eq!(
        flags.len(),
        1,
        "expected exactly one flagged comparison: {flags:?}"
    );
    let flag = flags.first().expect("one flag");
    assert_eq!(
        flag.kind,
        lmv_core::preset::GateKind::Compare { always: false },
        "the comparison never went true, so the param sat at 0 for the whole run"
    );
    assert_eq!(flag.source, "onset > 0.55", "the flag names the comparison");
}

#[test]
fn both_halves_of_a_composite_condition_are_named_separately() {
    // The case that reported *clean* before this plan (ADR-0043): the flag named
    // the whole `min(...)`, and the report's own guidance says a `tempo` gate is
    // legitimately one-sided under a single-BPM probe — so a reader dismissed a
    // flag whose other half was separately dead. Neither comparison is the
    // `select()`'s direct condition, so neither is suppressed.
    let e = compile("select(min(tempo > 124, bass + treb > 0.38), 4, 1)").expect("compiles");
    let mut obs = lmv_core::preset::Observations::new();
    // The probe's own conditions: one BPM, realistic band levels.
    for i in 0..64 {
        let t = i as f32 / 64.0;
        e.eval_probed(
            &Variables::new(0.040, 0.006, 0.006, 0.010, 0.0, t, t, 110.0, 0.0),
            &mut obs,
        );
    }
    let flags = e.flag_gates(&obs);
    let comparisons: Vec<&str> = flags
        .iter()
        .filter(|f| matches!(f.kind, lmv_core::preset::GateKind::Compare { .. }))
        .map(|f| f.source.as_str())
        .collect();
    assert_eq!(
        comparisons,
        vec!["tempo > 124", "bass + treb > 0.38"],
        "each half is named on its own, so the excusable one cannot launder the other"
    );
    // The `select()` still reports the composite condition it was always handed
    // — ADR-0043 adds the halves, it does not replace the gate flag (its
    // Alternative B). The finding is three-part, and the two new parts are the
    // ones an author can act on.
    assert_eq!(
        flags.len(),
        3,
        "the select flag stands alongside the two comparison flags: {flags:?}"
    );
    assert_eq!(
        flags
            .iter()
            .find(|f| matches!(f.kind, lmv_core::preset::GateKind::Select { .. }))
            .map(|f| f.source.as_str()),
        Some("min(tempo > 124, bass + treb > 0.38)"),
    );
}

#[test]
fn a_dead_branch_hides_the_gates_inside_it_rather_than_doubling_the_finding() {
    // The outer gate never fires, so the inner one is never evaluated. It stays
    // untouched and silent: fixing the outer gate is what makes it reportable.
    let e = compile("select(bass > 1.5, select(mid > 0.5, 1, 2), 3)").expect("compiles");
    let spectrum = [0.0f32; 64];
    let mut obs = lmv_core::preset::Observations::new();
    for v in &variable_sweep(&spectrum) {
        e.eval_probed(v, &mut obs);
    }
    let flags = e.flag_gates(&obs);
    assert_eq!(
        flags.len(),
        1,
        "one dead outer gate is one finding, not two: {flags:?}"
    );
    assert_eq!(flags.first().map(|f| f.source.as_str()), Some("bass > 1.5"));
}

#[test]
fn a_nodes_index_does_not_move_with_the_branch_the_run_took() {
    // Both inner gates are reached (the outer one crosses), and each is dead in
    // the opposite direction. If a node's index shifted with the branch taken —
    // the obvious way to write this walk — the two would land in the same slot,
    // merge into a healthy-looking two-sided reading, and neither would be
    // reported. Two findings here is what proves the indices are static.
    let e = compile("select(bass > 0.05, select(mid > 1.5, 1, 2), select(treb < 1.5, 3, 4))")
        .expect("compiles");
    let spectrum = [0.0f32; 64];
    let mut obs = lmv_core::preset::Observations::new();
    for v in &variable_sweep(&spectrum) {
        e.eval_probed(v, &mut obs);
    }
    let flags = e.flag_gates(&obs);
    let sources: Vec<&str> = flags.iter().map(|f| f.source.as_str()).collect();
    assert_eq!(
        sources,
        vec!["mid > 1.5", "treb < 1.5"],
        "both inner gates are one-sided and both must be named"
    );
}

#[test]
fn a_clamp_bound_the_value_never_reaches_is_flagged_and_one_it_reaches_is_not() {
    // The gain-against-full-scale mistake in its other form: a ceiling written
    // for `bass = 1` that real levels never come near.
    let decorative = compile("clamp(bass * 0.001, 0, 0.5)").expect("compiles");
    let reached = compile("clamp(bass * 0.1, 0, 0.02)").expect("compiles");
    let spectrum = [0.0f32; 64];
    let sweep = variable_sweep(&spectrum);

    let mut obs = lmv_core::preset::Observations::new();
    for v in &sweep {
        decorative.eval_probed(v, &mut obs);
    }
    let flags = decorative.flag_gates(&obs);
    match flags.first().map(|f| f.kind) {
        Some(lmv_core::preset::GateKind::Clamp {
            peak_fraction_of_bound,
        }) => assert!(
            peak_fraction_of_bound < 1.0,
            "flagged with a peak of {peak_fraction_of_bound}, which is not below the bound"
        ),
        other => panic!("expected a flagged clamp, got {other:?}"),
    }
    assert_eq!(
        flags.first().map(|f| f.source.as_str()),
        Some("clamp(bass * 0.001, 0, 0.5)"),
        "the flag names the whole call, bound included"
    );

    let mut obs = lmv_core::preset::Observations::new();
    for v in &sweep {
        reached.eval_probed(v, &mut obs);
    }
    assert!(
        reached.flag_gates(&obs).is_empty(),
        "a bound the value actually hits is not a finding"
    );
}

#[test]
fn observations_untouched_by_a_plain_eval_claim_nothing() {
    // The structural half of "the render path pays nothing": a frame only ever
    // calls `eval`, so an Observations no probe ever wrote to must yield no
    // findings rather than reporting every gate as dead.
    let e = compile("select(bass > 0.2, clamp(mid, 0, 1), 0)").expect("compiles");
    let v = vars(0.5, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0);
    for _ in 0..100 {
        let _ = e.eval(&v);
    }
    let untouched = lmv_core::preset::Observations::new();
    assert!(untouched.nodes().is_empty(), "eval recorded nothing");
    assert!(
        e.flag_gates(&untouched).is_empty(),
        "no observations means no claims"
    );
}

#[test]
fn a_flagged_gate_is_named_in_source_that_compiles_back() {
    // The report prints these strings; an author has to be able to paste one
    // into a preset and have it mean the same thing.
    let spectrum: Vec<f32> = (0..64).map(|i| i as f32 / 64.0).collect();
    for src in [
        "bass + mid > 0.34",
        "select(min(tempo > 124, bass + treb > 0.38), 4, 1)",
        "clamp(bass * 0.1, 0, 0.045)",
        "-bass * (mid + treb) / 2 - (bar - time)",
        "lerp(1, 2, smoothstep(0, 1, bin(0.5)))",
    ] {
        let original = compile(src).expect("compiles");
        let text = original.source();
        let round_tripped = compile(&text).expect("the rendered source compiles");
        for v in variable_sweep(&spectrum).iter().take(32) {
            assert!(
                same_value(original.eval(v), round_tripped.eval(v)),
                "`{src}` rendered as `{text}`, which evaluates differently"
            );
        }
    }

    // The same property on the strings a flag actually carries, comparisons
    // included: a `Compare` flag names a sub-expression rather than a whole
    // binding, and that fragment has to stand on its own too.
    let mut compared = 0usize;
    for src in [
        // Thresholds past the top of the sweep, so each is genuinely one-sided.
        "onset > 1.5",
        "select(min(tempo > 400, bass + treb > 2.5), 4, 1)",
        "clamp(bass * 0.001, 0, 0.5) + (mid >= 1.5)",
    ] {
        let e = compile(src).expect("compiles");
        let mut obs = lmv_core::preset::Observations::new();
        for v in &variable_sweep(&spectrum) {
            e.eval_probed(v, &mut obs);
        }
        let flags = e.flag_gates(&obs);
        assert!(!flags.is_empty(), "`{src}` was expected to flag");
        for flag in &flags {
            let re = compile(&flag.source).unwrap_or_else(|err| {
                panic!(
                    "`{src}` flagged `{}`, which does not compile: {err:?}",
                    flag.source
                )
            });
            assert_eq!(
                re.source(),
                flag.source,
                "`{}` does not re-render as itself",
                flag.source
            );
            if matches!(flag.kind, lmv_core::preset::GateKind::Compare { .. }) {
                compared += 1;
            }
        }
    }
    assert!(
        compared >= 4,
        "expected the comparison flags to be covered, saw {compared}"
    );
}
