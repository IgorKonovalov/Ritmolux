//! **The EEL2 conformance suite** (Plan 0100 Phase 2's done-when): snippets
//! compiled by `milkconv` and executed by `rlx-core`'s VM, checked against the
//! values the MilkDrop authoring reference specifies.
//!
//! # Why this file is here and not in `core`
//!
//! It is the only test in the repository that exercises **both halves of the
//! ADR-0113 seam at once**. `core/src/milk/tests.rs` can assert what the VM does
//! with hand-written bytecode, and that is a different question from what a
//! *program* means: an operator precedence bug, a short-circuit that evaluates
//! both sides, an assignment that yields the wrong value — none of those are
//! visible from either side alone. So the suite lives with the compiler, which is
//! the half that can produce the bytecode, and links the half that runs it.
//!
//! # The reference
//!
//! Each case names the behaviour it pins in EEL2's terms. Where MilkDrop's
//! reference and a naive reading disagree — `^` is exponentiation rather than
//! xor; `&`/`|` are bitwise where `band`/`bor` are logical; an assignment is an
//! expression yielding what it assigned; `==` compares within an epsilon — the
//! case says so, because those are exactly the places a converted preset would
//! render subtly wrong with a green suite.

use milkconv::eel::{Symbols, compile_bundle, compile_into};
use rlx_core::milk::bytecode::COMPARE_EPSILON;
use rlx_core::milk::vm::{Budget, VmState, run};
use rlx_core::milk::{MilkRuntime, NOMINAL_FPS};

/// Compile `src` as a standalone program and run it, returning its value.
fn eval(src: &str) -> f32 {
    let mut symbols = Symbols::new();
    let code =
        compile_into(src, &mut symbols).unwrap_or_else(|e| panic!("compiling `{src}` failed: {e}"));
    let program = rlx_core::milk::bytecode::EelProgram::new(code, symbols.names().to_vec())
        .unwrap_or_else(|e| panic!("`{src}` compiled to invalid bytecode: {e}"));
    let mut state = VmState::new(program.register_count(), program.stack_depth(), 0);
    run(&program, &mut state, Budget::FRAME)
}

/// Assert a snippet's value, naming the property rather than the number.
#[track_caller]
fn check(what: &str, src: &str, expected: f32) {
    let got = eval(src);
    assert!(
        (got - expected).abs() < 1e-4,
        "{what}\n  `{src}`\n  expected {expected}, got {got}"
    );
}

/// **Assignment and sequencing** — the two things this engine's own expression
/// grammar deliberately does not have (ADR-0002 / ADR-0020), and the whole
/// reason EEL2 needs a machine of its own.
#[test]
fn assignment_and_sequencing() {
    check("a program's value is its last statement", "1; 2; 3", 3.0);
    check("an assignment yields what it assigned", "x = 7", 7.0);
    check(
        "a variable holds its value across statements",
        "x = 4; x * 2",
        8.0,
    );
    check("assignment is right-associative", "x = y = 3; x + y", 6.0);
    check("an unassigned variable reads zero", "q17", 0.0);
    check("a trailing semicolon is legal", "x = 5;", 5.0);
    check("doubled semicolons are legal", "x = 5;; x", 5.0);
    check(
        "a parenthesized sequence is an expression",
        "2 * (x = 3; x + 1)",
        8.0,
    );

    check("compound add", "x = 5; x += 3", 8.0);
    check("compound subtract", "x = 5; x -= 3", 2.0);
    check("compound multiply", "x = 5; x *= 3", 15.0);
    check("compound divide", "x = 6; x /= 3", 2.0);
    check("compound modulo", "x = 7; x %= 3", 1.0);
    check("compound power", "x = 2; x ^= 3", 8.0);

    // Identifiers are case-insensitive, which real presets rely on.
    check("`Zoom` and `zoom` are one variable", "Zoom = 3; zoom", 3.0);
}

/// **Operator precedence and associativity**, including the three places EEL2
/// differs from what a C programmer would guess.
#[test]
fn operator_precedence() {
    check(
        "multiplication binds tighter than addition",
        "2 + 3 * 4",
        14.0,
    );
    check("parentheses override it", "(2 + 3) * 4", 20.0);
    check("subtraction is left-associative", "10 - 3 - 2", 5.0);
    check("division is left-associative", "16 / 4 / 2", 2.0);

    // `^` is EXPONENTIATION in EEL2, not xor, and it binds tighter than unary
    // minus. A converter that read it as xor would render most of the corpus
    // wrong in a way no crash would announce.
    check("`^` is exponentiation", "2 ^ 10", 1024.0);
    check("`^` binds tighter than unary minus", "-2 ^ 2", -4.0);
    check("`^` is right-associative", "2 ^ 3 ^ 2", 512.0);
    check("`^` binds tighter than `*`", "2 * 3 ^ 2", 18.0);

    check("comparison is looser than arithmetic", "1 + 1 > 1", 1.0);
    check("equality is looser than comparison", "(2 > 1) == 1", 1.0);
    check("unary not", "!0", 1.0);
    check("unary not of a non-zero", "!5", 0.0);
    check("a ternary is right-associative", "0 ? 1 : 1 ? 2 : 3", 2.0);
    check("a ternary takes the true branch", "1 ? 10 : 20", 10.0);
}

/// **The comparison family** — including that `==` is an epsilon comparison,
/// which is part of the language rather than a tolerance we chose.
#[test]
fn comparisons_and_the_epsilon() {
    check("above", "above(3, 2)", 1.0);
    check("below", "below(3, 2)", 0.0);
    check("equal", "equal(2, 2)", 1.0);
    check("`>` yields exactly one", "3 > 2", 1.0);
    check("`<` yields exactly zero", "3 < 2", 0.0);
    check("`>=` at equality", "2 >= 2", 1.0);
    check("`<=` at equality", "2 <= 2", 1.0);
    check("`!=`", "2 != 3", 1.0);

    // Within the epsilon two different numbers are equal, which is what a
    // preset comparing the result of arithmetic against zero relies on.
    let inside = COMPARE_EPSILON / 2.0;
    check(
        "`==` is an epsilon comparison, not an exact one",
        &format!("equal(0, {inside})"),
        1.0,
    );
    let outside = COMPARE_EPSILON * 2.0;
    check(
        "...and outside the epsilon it is false",
        &format!("equal(0, {outside})"),
        0.0,
    );
}

/// **Logical versus bitwise**, the other place a naive reading is silently wrong:
/// `band`/`bor` are logical and `&`/`|` are bitwise, so `3 & 4` and
/// `band(3, 4)` are different answers.
#[test]
fn logical_and_bitwise_are_different_operators() {
    check("band is logical", "band(3, 4)", 1.0);
    check("bor is logical", "bor(0, 0)", 0.0);
    check("bnot is logical", "bnot(0)", 1.0);
    check("`&` is bitwise", "3 & 4", 0.0);
    check("`&` on overlapping bits", "6 & 3", 2.0);
    check("`|` is bitwise", "1 | 4", 5.0);

    // `&&` and `||` short-circuit, which is observable: the right side's
    // assignment must not happen.
    check("`&&` short-circuits", "x = 0; 0 && (x = 9); x", 0.0);
    check("`||` short-circuits", "x = 0; 1 || (x = 9); x", 0.0);
    check("`&&` yields exactly one", "2 && 3", 1.0);
    check("`||` yields exactly one", "0 || 5", 1.0);
    check("`&&` yields exactly zero", "2 && 0", 0.0);

    // `if()` is lazy too — the untaken branch is not evaluated. `band` is NOT,
    // which is the pair's whole difference.
    check("`if` is lazy", "x = 0; if(1, 1, x = 9); x", 0.0);
    check("`band` is not lazy", "x = 0; band(0, x = 9); x", 9.0);
}

/// **`if` / `above` / `below` / `equal`** — the four branching
/// builtins, plus the value each yields.
#[test]
fn the_branching_builtins() {
    check("if takes the true branch", "if(1, 10, 20)", 10.0);
    check("if takes the false branch", "if(0, 10, 20)", 20.0);
    check("if treats any non-zero as true", "if(-3, 10, 20)", 10.0);
    check(
        "the four compose the way a preset writes them",
        "b = 0.7; if(above(b, 0.5), 2, if(below(b, 0.2), 0, 1))",
        2.0,
    );
    check(
        "...and take the middle arm",
        "b = 0.35; if(above(b, 0.5), 2, if(below(b, 0.2), 0, 1))",
        1.0,
    );
}

/// **`loop` with a bounded count.** The count is evaluated once, the
/// body runs that many times, and the whole thing is an expression.
#[test]
fn bounded_loops() {
    check(
        "a loop runs its body n times",
        "x = 0; loop(5, x = x + 1); x",
        5.0,
    );
    check(
        "a loop's count is an expression",
        "n = 3; x = 0; loop(n * 2, x = x + 1); x",
        6.0,
    );
    check(
        "a zero count runs nothing",
        "x = 0; loop(0, x = x + 1); x",
        0.0,
    );
    check(
        "a negative count runs nothing",
        "x = 0; loop(-4, x = x + 1); x",
        0.0,
    );
    check(
        "a loop body may hold a sequence",
        "x = 0; y = 0; loop(3, x = x + 1; y = y + x); y",
        6.0,
    );
    check("loops nest", "x = 0; loop(3, loop(4, x = x + 1)); x", 12.0);
    check(
        "a loop is an expression, and its value is zero",
        "1 + loop(3, 7)",
        1.0,
    );

    // `while` runs its body until the body reads zero.
    check(
        "while runs until its body is false",
        "x = 0; while(x = x + 1; x < 4); x",
        4.0,
    );
}

/// **`megabuf` round-trips**, including the compound-assignment form, which is
/// the one construct in the language that needs a hidden temporary.
#[test]
fn megabuf_round_trip() {
    check("a slot round-trips", "megabuf(10) = 42; megabuf(10)", 42.0);
    check("a store yields what it stored", "megabuf(3) = 7", 7.0);
    check(
        "slots are independent",
        "megabuf(1) = 5; megabuf(2) = 9; megabuf(1)",
        5.0,
    );
    check(
        "the index is an expression",
        "i = 4; megabuf(i * 2) = 11; megabuf(8)",
        11.0,
    );
    check(
        "a compound assignment reads then writes the SAME slot",
        "megabuf(6) = 10; megabuf(6) += 5; megabuf(6)",
        15.0,
    );
    check(
        "...and evaluates its index exactly once",
        "i = 0; megabuf(2) = 10; megabuf((i = i + 1) + 1) += 5; i",
        1.0,
    );
    check(
        "gmegabuf is a separate arena",
        "megabuf(0) = 1; gmegabuf(0) = 2; megabuf(0)",
        1.0,
    );
    check(
        "a loop over megabuf is the idiom presets use it for",
        "loop(8, megabuf(i) = i * i; i = i + 1); megabuf(5)",
        25.0,
    );
}

/// **The maths builtins**, against the reference's own definitions — including
/// the three that are not what a C programmer expects.
#[test]
fn the_maths_builtins() {
    check("sin", "sin(0)", 0.0);
    check("cos", "cos(0)", 1.0);
    check("sqrt", "sqrt(9)", 3.0);
    check("pow", "pow(2, 8)", 256.0);
    check("abs", "abs(-3.5)", 3.5);
    check("min", "min(3, 5)", 3.0);
    check("max", "max(3, 5)", 5.0);
    check("floor", "floor(-1.5)", -2.0);
    check("ceil", "ceil(-1.5)", -1.0);
    check("atan2", "atan2(1, 1)", std::f32::consts::FRAC_PI_4);
    check("exp and log invert", "log(exp(2))", 2.0);
    check("log10", "log10(1000)", 3.0);
    check("invsqrt", "invsqrt(4)", 0.5);
    check("sqr", "sqr(7)", 49.0);

    // `int` TRUNCATES toward zero where `floor` rounds down — different
    // functions on a negative, and presets use both.
    check("int truncates toward zero", "int(-1.7)", -1.0);
    check("floor rounds down", "floor(-1.7)", -2.0);
    // `sign` is three-way, so it is zero at zero rather than one.
    check("sign is zero at zero", "sign(0)", 0.0);
    check("sign of a negative", "sign(-9)", -1.0);
    // `%` operates on the integer parts, which is EEL2's definition.
    check("`%` is an integer remainder", "7.9 % 3", 1.0);

    check("$pi", "$pi", std::f32::consts::PI);
    check("a hex constant", "$xff", 255.0);
    check("sigmoid at zero is a half", "sigmoid(0, 1)", 0.5);

    check("exec2 yields its last argument", "exec2(x = 1, x + 1)", 2.0);
    check("exec3 yields its last argument", "exec3(1, 2, 3)", 3.0);
    check("exec2 evaluates both", "x = 0; exec2(x = 5, 0); x", 5.0);
}

/// Comments and whitespace are dropped, which real `.milk` per-frame blocks are
/// full of.
#[test]
fn comments_and_whitespace() {
    check("a line comment", "x = 1; // this is ignored\nx + 1", 2.0);
    check("a block comment", "x = /* mid-expression */ 3; x", 3.0);
    check("newlines separate nothing", "x\n=\n5;\nx", 5.0);
}

/// A malformed program is a **surfaced converter error**, never a panic and never
/// a program that quietly means something else.
#[test]
fn malformed_programs_are_errors() {
    let cases = [
        ("an unclosed paren", "x = (1 + 2"),
        ("a dangling operator", "x = 1 +"),
        ("a stray comma", "x = 1, 2"),
        ("an unknown function", "x = frobnicate(1)"),
        ("a builtin used as a variable", "x = sin"),
        ("assigning to a builtin", "sin = 1"),
        ("a ternary with no colon", "x = 1 ? 2"),
        ("an unterminated block comment", "x = 1 /* oops"),
        ("a stray character", "x = 1 @ 2"),
        ("a wrong-arity call", "x = min(1)"),
    ];
    for (what, src) in cases {
        let mut symbols = Symbols::new();
        assert!(
            compile_into(src, &mut symbols).is_err(),
            "{what}: `{src}` compiled, and should not have"
        );
    }
}

/// **The `q1`–`q32` and `t1`–`t8` bridges**, in full.
///
/// The bridge is not a feature of the language: it is the shared register file.
/// A `q` name written by `per_frame` and read by `per_vertex` is one register
/// because [`compile_bundle`] compiles all three sections against one symbol
/// table, and this is the assertion that it does.
#[test]
fn the_q_and_t_bridges_are_one_register_file() {
    // per_frame fills q1..q32 and t1..t8; per_vertex reads them back.
    let mut init = String::new();
    let mut frame = String::new();
    let mut vertex = String::from("cx = 0;");
    for i in 1..=32 {
        frame.push_str(&format!("q{i} = {i};"));
        vertex.push_str(&format!("cx = cx + q{i};"));
    }
    for i in 1..=8 {
        frame.push_str(&format!("t{i} = {};", i * 100));
        vertex.push_str(&format!("cx = cx + t{i};"));
    }
    init.push_str("q1 = -1;");

    let (bundle, symbols) = compile_bundle(&init, &frame, &vertex).expect("the bundle compiles");
    for i in 1..=32 {
        assert!(symbols.contains(&format!("q{i}")), "q{i} is in the roster");
    }
    for i in 1..=8 {
        assert!(symbols.contains(&format!("t{i}")), "t{i} is in the roster");
    }
    // One roster, so the three programs address the same registers.
    assert_eq!(bundle.per_frame.names(), bundle.per_vertex.names());
    assert_eq!(bundle.per_frame_init.names(), bundle.per_vertex.names());

    let mut runtime = MilkRuntime::new(bundle, 0);
    runtime.run_frame(
        &rlx_core::dsp::AnalysisFrame::default(),
        0.0,
        1.0 / 60.0,
        (8, 8),
        1.0,
    );
    // `cx` is output index 2 and is a position, so it is not rate-converted.
    let cx = runtime.run_vertex(0.0, 0.5)[2];
    let expected = (1..=32).sum::<i32>() as f32 + (1..=8).map(|i| i * 100).sum::<i32>() as f32;
    assert!(
        (cx - expected).abs() < 1e-2,
        "the bridge carried {cx}, expected {expected}"
    );
}

/// **A whole preset-shaped program**, compiled and executed the way one really
/// runs — the integration this suite exists to make possible.
#[test]
fn a_preset_shaped_bundle_drives_the_mesh() {
    let (bundle, _) = compile_bundle(
        // per_frame_init
        "q1 = 0.5;",
        // per_frame: the shape a real MilkDrop preset's block takes
        "q1 = q1 + 0.01;
         zoom = 1.01 + bass * 0.02;
         rot = 0.01 * sin(time * 0.3);
         decay = 0.97;
         cx = 0.5 + 0.1 * cos(time);",
        // per_vertex: a transform that varies with the vertex's own position
        "zoom = zoom + rad * 0.05;
         rot = rot + q1 * (x - 0.5);",
    )
    .expect("the preset-shaped bundle compiles");

    let mut runtime = MilkRuntime::new(bundle, 0);
    let frame = rlx_core::dsp::AnalysisFrame {
        bass: 0.5,
        ..Default::default()
    };
    let (outputs, extra) = runtime.run_frame(&frame, 1.0, 1.0 / 60.0, (32, 24), 16.0 / 9.0);

    // `zoom` = 1.01 + (0.5 * 2) * 0.02 = 1.03 per frame, converted per second.
    let zoom = outputs[0].powf(1.0 / NOMINAL_FPS);
    assert!((zoom - 1.03).abs() < 1e-3, "zoom per frame: {zoom}");
    let decay = extra.decay;
    assert!(
        (decay.powf(1.0 / NOMINAL_FPS) - 0.97).abs() < 1e-3,
        "decay per frame: {}",
        decay.powf(1.0 / NOMINAL_FPS)
    );
    assert!(
        (outputs[2] - (0.5 + 0.1 * 1.0f32.cos())).abs() < 1e-4,
        "cx: {}",
        outputs[2]
    );

    // The per-vertex program actually varies with `rad`.
    let centre = runtime.run_vertex(0.5, 0.5)[0];
    let rim = runtime.run_vertex(1.0, 0.5)[0];
    assert!(
        rim > centre,
        "`zoom = zoom + rad * 0.05` must grow with rad: rim {rim} vs centre {centre}"
    );
    // ...and a write inside it does not leak: the same vertex twice agrees.
    assert_eq!(
        runtime.run_vertex(0.5, 0.5)[0],
        centre,
        "the per-frame snapshot is restored before every vertex"
    );
}

/// The compiler's output is **deterministic**: the same source compiles to the
/// same bytecode, byte for byte, so a bundle in review only changes when the
/// preset or the converter does.
#[test]
fn compilation_is_deterministic() {
    let src = "q1 = 0.5; zoom = 1.01 + bass * sin(time); loop(4, q1 = q1 * 1.1);";
    let assemble = || {
        let (bundle, _) = compile_bundle("", src, "").expect("compiles");
        bundle.per_frame.to_assembly()
    };
    assert_eq!(assemble(), assemble());
    // ...and it decodes back to the same program, which is the seam's whole
    // round-trip in one line.
    let text = assemble();
    let back = rlx_core::milk::bytecode::EelProgram::from_assembly(&text).expect("decodes");
    assert_eq!(back.to_assembly(), text);
}
