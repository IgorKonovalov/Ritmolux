//! The EEL2 machine's own contracts (Plan 0100 Phase 2): the encoding round-trip,
//! the decoder's guarantees, the VM's totality, and the per-frame/per-vertex
//! driver's rate conversion.
//!
//! **Language conformance is not here** — it needs a compiler, which lives in
//! `milkconv`, so it is `milkconv/tests/conformance.rs`. What is here is what can
//! be asserted of the shipped half alone.

// Test asserts panic on failure; allowed here over the module's pragma.
#![allow(clippy::panic, clippy::indexing_slicing, clippy::unwrap_used)]

use super::bytecode::{Binary, EelProgram, Mem, Op, ProgramError, Unary};
use super::vm::{Budget, GMEGABUF_SLOTS, MEGABUF_SLOTS, VmState, run};
use super::*;

/// Assemble a program from ops over registers named `a`, `b`, `c`, …
fn program(code: Vec<Op>, registers: usize) -> EelProgram {
    let names = (0..registers)
        .map(|i| format!("r{i}"))
        .collect::<Vec<String>>();
    EelProgram::new(code, names).expect("the fixture program validates")
}

fn execute(p: &EelProgram) -> f32 {
    let mut state = VmState::new(p.register_count(), p.stack_depth(), 0);
    run(p, &mut state, Budget::FRAME)
}

/// **The text encoding round-trips**, which is what makes a bundle both diffable
/// and lossless. Every op shape appears, so a new variant that forgets its
/// `op_from_text` arm fails here rather than in a converted preset.
#[test]
fn every_op_survives_the_assembly_round_trip() {
    let code = vec![
        Op::Const(0.0),
        Op::Const(-1.5),
        Op::Const(f32::MAX),
        Op::Const(1.0 / 3.0),
        Op::Load(0),
        Op::Store(1),
        Op::Pop,
        Op::Const(1.0),
        Op::Neg,
        Op::Not,
        Op::Const(2.0),
        Op::Add,
        Op::Const(2.0),
        Op::Sub,
        Op::Const(2.0),
        Op::Mul,
        Op::Const(2.0),
        Op::Div,
        Op::Const(2.0),
        Op::Mod,
        Op::Const(2.0),
        Op::Pow,
        Op::Const(2.0),
        Op::Above,
        Op::Const(2.0),
        Op::Below,
        Op::Const(2.0),
        Op::AboveEq,
        Op::Const(2.0),
        Op::BelowEq,
        Op::Const(2.0),
        Op::Equal,
        Op::Const(2.0),
        Op::NotEqual,
        Op::Const(2.0),
        Op::BitAnd,
        Op::Const(2.0),
        Op::BitOr,
        Op::Fn1(Unary::Sin),
        Op::Fn1(Unary::Rand),
        Op::Const(2.0),
        Op::Fn2(Binary::Sigmoid),
        Op::MemLoad(Mem::Local),
        Op::Const(3.0),
        Op::MemStore(Mem::Global),
        Op::Jump(43),
        Op::Const(1.0),
        Op::JumpIfZero(45),
        Op::Const(1.0),
        Op::JumpIfNotZero(47),
        Op::Const(1.0),
        Op::LoopBegin(50),
        Op::Const(0.0),
        Op::LoopEnd(48),
        Op::Const(1.0),
        Op::WhileBegin(53),
        Op::Const(0.0),
        Op::WhileEnd(52),
    ];
    let p = program(code, 2);
    let text = p.to_assembly();
    let back = EelProgram::from_assembly(&text).expect("the round trip decodes");
    assert_eq!(p, back, "assembly text is lossless:\n{text}");
    assert_eq!(back.to_assembly(), text, "and idempotent");

    // Every mnemonic actually appears, so a variant whose encoder arm went
    // missing shows up here rather than as a silently shorter program.
    for op in p.code() {
        assert!(
            text.contains(op_head(*op)),
            "`{op:?}` is missing from the assembly text"
        );
    }
}

/// The mnemonic an op is written as, for the coverage check above.
fn op_head(op: Op) -> &'static str {
    match op {
        Op::Const(_) => "const",
        Op::Load(_) => "load",
        Op::Store(_) => "store",
        Op::Pop => "pop",
        Op::Neg => "neg",
        Op::Not => "not",
        Op::Add => "add",
        Op::Sub => "sub",
        Op::Mul => "mul",
        Op::Div => "div",
        Op::Mod => "mod",
        Op::Pow => "pow",
        Op::Above => "above",
        Op::Below => "below",
        Op::AboveEq => "aboveeq",
        Op::BelowEq => "beloweq",
        Op::Equal => "equal",
        Op::NotEqual => "notequal",
        Op::BitAnd => "bitand",
        Op::BitOr => "bitor",
        Op::Fn1(_) => "fn1",
        Op::Fn2(_) => "fn2",
        Op::MemLoad(_) => "memload",
        Op::MemStore(_) => "memstore",
        Op::Jump(_) => "jump",
        Op::JumpIfZero(_) => "jz",
        Op::JumpIfNotZero(_) => "jnz",
        Op::LoopBegin(_) => "loopbegin",
        Op::LoopEnd(_) => "loopend",
        Op::WhileBegin(_) => "whilebegin",
        Op::WhileEnd(_) => "whileend",
    }
}

/// **The decoder is the boundary** (ADR-0002 / NFR §10): a malformed program is a
/// surfaced error, and everything downstream may then assume jumps land and
/// registers exist.
#[test]
fn the_decoder_rejects_what_the_vm_would_have_to_check() {
    let bad_jump = EelProgram::new(vec![Op::Jump(9)], vec![]);
    assert!(matches!(
        bad_jump,
        Err(ProgramError::BadJump { at: 0, target: 9 })
    ));

    let bad_reg = EelProgram::new(vec![Op::Load(3)], vec!["a".into()]);
    assert!(matches!(
        bad_reg,
        Err(ProgramError::BadRegister { at: 0, index: 3 })
    ));

    let underflow = EelProgram::new(vec![Op::Add], vec![]);
    assert!(matches!(
        underflow,
        Err(ProgramError::StackUnderflow { at: 0 })
    ));

    let duplicate = EelProgram::new(vec![], vec!["q1".into(), "q1".into()]);
    assert!(matches!(duplicate, Err(ProgramError::DuplicateRegister(_))));

    // A jump to exactly `len` is the ordinary "branch over the last instruction"
    // target the codegen emits, and must be accepted.
    assert!(EelProgram::new(vec![Op::Const(0.0), Op::JumpIfZero(2)], vec![]).is_ok());

    // Text-level failures.
    assert!(matches!(
        EelProgram::from_assembly("const 1\n"),
        Err(ProgramError::BadHeader(_))
    ));
    assert!(matches!(
        EelProgram::from_assembly(".regs\n.code\nfrobnicate\n"),
        Err(ProgramError::BadLine { .. })
    ));
    // Comments and blank lines are ignored, so a bundle stays legible.
    let commented =
        EelProgram::from_assembly("# a program\n.regs a\n\n.code\nconst 1  # push one\nstore 0\n")
            .expect("comments decode");
    assert_eq!(commented.code().len(), 2);
}

/// **The VM is total.** Every one of these would panic, hang or poison a register
/// under a naive implementation; all of them must produce a finite number and
/// return.
#[test]
fn the_vm_is_total_on_every_edge() {
    let cases: Vec<(&str, Vec<Op>, f32)> = vec![
        (
            "division by zero",
            vec![Op::Const(1.0), Op::Const(0.0), Op::Div],
            0.0,
        ),
        (
            "modulo by zero",
            vec![Op::Const(5.0), Op::Const(0.0), Op::Mod],
            0.0,
        ),
        (
            "log of zero",
            vec![Op::Const(0.0), Op::Fn1(Unary::Log)],
            0.0,
        ),
        (
            "log of a negative",
            vec![Op::Const(-3.0), Op::Fn1(Unary::Log)],
            0.0,
        ),
        (
            "sqrt of a negative",
            vec![Op::Const(-4.0), Op::Fn1(Unary::Sqrt)],
            0.0,
        ),
        (
            "invsqrt of zero",
            vec![Op::Const(0.0), Op::Fn1(Unary::InvSqrt)],
            0.0,
        ),
        (
            "asin out of range",
            vec![Op::Const(5.0), Op::Fn1(Unary::Asin)],
            std::f32::consts::FRAC_PI_2,
        ),
        (
            "a huge power",
            vec![Op::Const(1.0e30), Op::Const(1.0e30), Op::Pow],
            0.0,
        ),
        (
            "a megabuf index past the arena",
            vec![Op::Const(1.0e9), Op::MemLoad(Mem::Local)],
            0.0,
        ),
        (
            "a negative megabuf index",
            vec![Op::Const(-1.0), Op::MemLoad(Mem::Local)],
            0.0,
        ),
    ];
    for (what, code, want) in cases {
        let value = execute(&program(code, 0));
        assert!(value.is_finite(), "{what} produced {value}");
        assert!(
            (value - want).abs() < 1e-4,
            "{what}: expected {want}, got {value}"
        );
    }

    // A `NaN` can never be stored, because a stored NaN would be absorbing: every
    // comparison against it is false, so the register would be dead for the rest
    // of the preset's run.
    let p = program(
        vec![
            Op::Const(0.0),
            Op::Const(0.0),
            Op::Div,
            Op::Store(0),
            Op::Load(0),
        ],
        1,
    );
    assert_eq!(execute(&p), 0.0);
}

/// A loop runs its body the stated number of times, is bounded whatever it is
/// asked for, and is an **expression** — the property that lets it appear inside
/// arithmetic.
#[test]
fn a_loop_is_bounded_and_is_an_expression() {
    // r0 = 0; loop(5, r0 = r0 + 1); r0
    let counted = |count: f32| {
        program(
            vec![
                Op::Const(0.0),
                Op::Store(0),
                Op::Pop,
                Op::Const(count),
                Op::LoopBegin(11),
                Op::Load(0),
                Op::Const(1.0),
                Op::Add,
                Op::Store(0),
                Op::Pop,
                Op::Const(0.0),
                Op::LoopEnd(5),
                Op::Pop,
                Op::Load(0),
            ],
            1,
        )
    };
    assert_eq!(execute(&counted(5.0)), 5.0);
    assert_eq!(execute(&counted(1.0)), 1.0);
    assert_eq!(execute(&counted(0.0)), 0.0, "a zero count runs no body");
    assert_eq!(
        execute(&counted(-3.0)),
        0.0,
        "a negative count runs no body"
    );
    assert_eq!(
        execute(&counted(f32::NAN)),
        0.0,
        "a non-finite count runs no body"
    );
    // **The bounds, which are what stop untrusted text hanging a frame.** There
    // are two and either may bite first, so each is exercised with the other
    // opened out — a single assertion against the default budget would silently
    // stop testing the loop cap the moment the instruction backstop got tighter.
    let billion = counted(1.0e9);
    let mut state = VmState::new(billion.register_count(), billion.stack_depth(), 0);
    assert_eq!(
        run(
            &billion,
            &mut state,
            Budget {
                loops: 64,
                instructions: 1_000_000,
            }
        ),
        64.0,
        "the loop cap bounds a loop asking for a billion iterations"
    );
    let mut state = VmState::new(billion.register_count(), billion.stack_depth(), 0);
    let ran = run(
        &billion,
        &mut state,
        Budget {
            loops: 1_000_000,
            instructions: 700,
        },
    );
    assert!(
        ran > 0.0 && ran < 200.0,
        "the instruction backstop bounds it even with the loop cap wide open, \
         got {ran}"
    );
    // And under the shipped per-vertex budget — the one that runs thousands of
    // times a frame — it terminates well short of either.
    let mut state = VmState::new(billion.register_count(), billion.stack_depth(), 0);
    let ran = run(&billion, &mut state, Budget::VERTEX);
    assert!(
        ran <= f64::from(Budget::VERTEX.loops) as f32,
        "a per-vertex loop cannot exceed its own cap, got {ran}"
    );
}

/// A `while` runs until its body reads zero, and is bounded the same way.
#[test]
fn a_while_runs_until_its_body_is_zero_and_is_bounded() {
    // r0 = 0; while(r0 = r0 + 1; r0 < 4); r0
    let bounded_by_body = program(
        vec![
            Op::Const(0.0),
            Op::Store(0),
            Op::Pop,
            Op::Const(0.0),
            Op::WhileBegin(13),
            Op::Load(0),
            Op::Const(1.0),
            Op::Add,
            Op::Store(0),
            Op::Pop,
            Op::Load(0),
            Op::Const(4.0),
            Op::Below,
            Op::WhileEnd(5),
            Op::Pop,
            Op::Load(0),
        ],
        1,
    );
    assert_eq!(execute(&bounded_by_body), 4.0);

    // A body that never reads zero stops at the cap rather than hanging.
    let forever = program(
        vec![
            Op::Const(0.0),
            Op::Store(0),
            Op::Pop,
            Op::Const(0.0),
            Op::WhileBegin(11),
            Op::Load(0),
            Op::Const(1.0),
            Op::Add,
            Op::Store(0),
            Op::Pop,
            Op::Const(1.0),
            Op::WhileEnd(5),
            Op::Pop,
            Op::Load(0),
        ],
        1,
    );
    // Bounded by whichever budget bites first; under the default that is the
    // instruction backstop, and under a wide one it is the loop cap.
    let mut state = VmState::new(forever.register_count(), forever.stack_depth(), 0);
    assert_eq!(
        run(
            &forever,
            &mut state,
            Budget {
                loops: 50,
                instructions: 1_000_000,
            }
        ),
        50.0,
        "a `while` whose body never reads zero stops at the loop cap"
    );
    let ran = execute(&forever);
    assert!(
        ran > 0.0 && ran.is_finite(),
        "...and terminates under the shipped budget too, got {ran}"
    );
}

/// `megabuf` and `gmegabuf` round-trip within their arenas and are inert outside
/// them, and the two are genuinely separate.
#[test]
fn the_scratch_arenas_round_trip_and_do_not_alias() {
    let p = program(
        vec![
            // megabuf(7) = 3.5
            Op::Const(7.0),
            Op::Const(3.5),
            Op::MemStore(Mem::Local),
            Op::Pop,
            // gmegabuf(7) = -2
            Op::Const(7.0),
            Op::Const(-2.0),
            Op::MemStore(Mem::Global),
            Op::Pop,
            // megabuf(7) + gmegabuf(7)
            Op::Const(7.0),
            Op::MemLoad(Mem::Local),
            Op::Const(7.0),
            Op::MemLoad(Mem::Global),
            Op::Add,
        ],
        0,
    );
    assert_eq!(execute(&p), 1.5, "the two arenas are separate");

    // A store outside the arena writes nowhere and reads back zero, rather than
    // wrapping into a slot some other index owns.
    let outside = program(
        vec![
            Op::Const(MEGABUF_SLOTS as f32),
            Op::Const(9.0),
            Op::MemStore(Mem::Local),
            Op::Pop,
            Op::Const(0.0),
            Op::MemLoad(Mem::Local),
        ],
        0,
    );
    assert_eq!(execute(&outside), 0.0);
    const {
        assert!(
            GMEGABUF_SLOTS < MEGABUF_SLOTS,
            "the shared arena is smaller"
        )
    };
}

/// `rand()` is a **pure function of the salt and the call sequence** (ADR-0051):
/// two runs of one state diverge, two fresh states at one salt agree, and two
/// salts disagree. That is what keeps a capture reproducible while letting a
/// preset be different every launch.
#[test]
fn randomness_is_seeded_rather_than_drawn() {
    let p = program(vec![Op::Const(1.0), Op::Fn1(Unary::Rand)], 0);

    let draws = |salt: u32, n: usize| -> Vec<f32> {
        let mut state = VmState::new(0, p.stack_depth(), salt);
        (0..n).map(|_| run(&p, &mut state, Budget::FRAME)).collect()
    };
    let a = draws(7, 8);
    assert_eq!(a, draws(7, 8), "one salt, two fresh states: identical");
    assert_ne!(a, draws(8, 8), "two salts: different");
    assert!(
        a.windows(2).any(|w| w[0] != w[1]),
        "the stream must advance, not repeat"
    );
    assert!(
        a.iter().all(|v| (0.0..1.0).contains(v)),
        "rand(1) stays in [0, 1): {a:?}"
    );

    // The reset restores the stream, which is what a re-run of a capture needs.
    let mut state = VmState::new(0, p.stack_depth(), 7);
    let first: Vec<f32> = (0..4).map(|_| run(&p, &mut state, Budget::FRAME)).collect();
    state.reset_rng();
    let again: Vec<f32> = (0..4).map(|_| run(&p, &mut state, Budget::FRAME)).collect();
    assert_eq!(first, again);

    assert!(p.uses_random());
    assert!(!program(vec![Op::Const(1.0)], 0).uses_random());
}

/// The written-register set is exact, which is what makes the per-vertex restore
/// both correct and cheap.
#[test]
fn the_written_register_set_is_exactly_what_a_program_stores() {
    let p = program(
        vec![
            Op::Const(1.0),
            Op::Store(2),
            Op::Store(0),
            Op::Pop,
            Op::Load(1),
            Op::Pop,
            Op::Const(1.0),
            Op::Store(2),
            Op::Pop,
        ],
        3,
    );
    assert_eq!(
        p.written_registers(),
        &[0, 2],
        "sorted, deduplicated, and not including the register only READ"
    );
}

// ---------------------------------------------------------------------------
// The bundle and its driver
// ---------------------------------------------------------------------------

/// A bundle whose three programs disagree about their registers is a **load
/// error**, because the shared register file *is* the `q1`–`q32` bridge.
#[test]
fn a_bundle_with_mismatched_rosters_is_rejected() {
    let ok = MilkBundle::from_assembly(
        None,
        Some(".regs q1 zoom\n.code\nconst 2\nstore 0\npop\n"),
        Some(".regs q1 zoom\n.code\nload 0\nstore 1\npop\n"),
    );
    assert!(ok.is_ok());

    let mismatch = MilkBundle::from_assembly(
        None,
        Some(".regs q1 zoom\n.code\nconst 2\nstore 0\npop\n"),
        Some(".regs zoom q1\n.code\nload 0\nstore 1\npop\n"),
    );
    assert!(matches!(
        mismatch,
        Err(BundleError::RosterMismatch {
            section: "per_vertex"
        })
    ));

    let broken = MilkBundle::from_assembly(None, Some(".regs a\n.code\nload 5\n"), None);
    assert!(matches!(
        broken,
        Err(BundleError::Program {
            section: "per_frame",
            ..
        })
    ));
}

/// **The `q1`–`q32` bridge**: what `per_frame` leaves in a register is what
/// `per_vertex` reads, at every vertex, and a write inside `per_vertex` does not
/// leak to the next one.
#[test]
fn the_q_bridge_carries_per_frame_into_every_vertex() {
    // per_frame: q1 = 5
    // per_vertex: zoom = q1 + x; q1 = q1 + 100   <- the leak this must not have
    let bundle = MilkBundle::from_assembly(
        None,
        Some(".regs q1 zoom x\n.code\nconst 5\nstore 0\npop\n"),
        Some(
            ".regs q1 zoom x\n.code\n\
             load 0\nload 2\nadd\nstore 1\npop\n\
             load 0\nconst 100\nadd\nstore 0\npop\n",
        ),
    )
    .expect("the bridge bundle decodes");
    let mut runtime = MilkRuntime::new(bundle, 0);
    let frame = crate::dsp::AnalysisFrame::default();
    runtime.run_frame(&frame, 0.0, 1.0 / 60.0, (4, 4), 1.0);

    // `zoom` is output 0, and is a factor, so it comes back as `raw^NOMINAL_FPS`.
    let zoom_at = |runtime: &mut MilkRuntime, x: f32| -> f32 { runtime.run_vertex(x, 0.5)[0] };
    let a = zoom_at(&mut runtime, 0.0);
    let b = zoom_at(&mut runtime, 0.0);
    assert_eq!(
        a, b,
        "two identical vertices must give identical answers — `q1` is restored \
         from the per-frame snapshot, so the +100 does not accumulate"
    );
    // 5 + 0 = 5 at x = 0, and 5 + 1 = 6 at x = 1, both raised to NOMINAL_FPS.
    assert!((a - 5.0f32.powf(NOMINAL_FPS)).abs() / a < 1e-3, "got {a}");
    let c = zoom_at(&mut runtime, 1.0);
    assert!(c > a, "a larger `x` must give a larger zoom: {c} vs {a}");
}

/// **The rate conversion**, which is the plan's most consequential translation:
/// MilkDrop's per-frame factors and rates become this engine's per-second ones,
/// so a converted preset moves at the speed its author saw on any refresh.
#[test]
fn per_frame_rates_become_per_second_ones() {
    // per_frame: zoom = 1.01; rot = 0.02; dx = 0.003; cx = 0.25; decay = 0.98
    let bundle = MilkBundle::from_assembly(
        None,
        Some(
            ".regs zoom rot cx dx decay\n.code\n\
             const 1.01\nstore 0\npop\n\
             const 0.02\nstore 1\npop\n\
             const 0.25\nstore 2\npop\n\
             const 0.003\nstore 3\npop\n\
             const 0.98\nstore 4\npop\n",
        ),
        None,
    )
    .expect("the rates bundle decodes");
    let mut runtime = MilkRuntime::new(bundle, 0);
    let (outputs, decay) = runtime.run_frame(
        &crate::dsp::AnalysisFrame::default(),
        0.0,
        1.0 / 60.0,
        (8, 8),
        16.0 / 9.0,
    );

    // A factor composes multiplicatively: 30 frames of 1 % is `1.01^30` a second.
    assert!(
        (outputs[0] - 1.01f32.powf(NOMINAL_FPS)).abs() < 1e-3,
        "zoom: {}",
        outputs[0]
    );
    // A rate composes additively: 30 frames of 0.02 rad is `0.6` rad a second.
    assert!(
        (outputs[1] - 0.02 * NOMINAL_FPS).abs() < 1e-5,
        "rot: {}",
        outputs[1]
    );
    assert!(
        (outputs[4] - 0.003 * NOMINAL_FPS).abs() < 1e-5,
        "dx: {}",
        outputs[4]
    );
    // A **position** is neither, and passes through untouched. Scaling `cx` would
    // put the fixed point outside the frame on the first frame.
    assert_eq!(outputs[2], 0.25, "cx is a position, not a motion");
    let decay = decay[0].expect("the program names `decay`");
    assert!(
        (decay - 0.98f32.powf(NOMINAL_FPS)).abs() < 1e-4,
        "decay: {decay}"
    );

    // An output the program never names comes back at the identity, so a partial
    // program leaves the rest of the transform still.
    assert_eq!(outputs[6], 1.0, "unnamed `sx` is the unit factor");
    assert_eq!(outputs[3], 0.5, "unnamed `cy` is the middle of the frame");
    assert_eq!(outputs[8], 0.0, "unnamed `warp` is off");
}

/// Every input the host promises is actually written, and the aspect pair follows
/// MilkDrop's convention rather than this engine's ratio.
#[test]
fn the_host_writes_the_inputs_a_program_reads() {
    // The sum lands in `cx`, which is a **position** and so passes through the
    // rate conversion untouched — a factor output would be raised to
    // NOMINAL_FPS and a sum near 95 overflows a thirtieth power.
    let bundle = MilkBundle::from_assembly(
        None,
        Some(
            ".regs bass mid treb time frame fps meshx meshy aspectx aspecty cx\n.code\n\
             load 0\nload 1\nadd\nload 2\nadd\nload 3\nadd\nload 4\nadd\nload 5\nadd\n\
             load 6\nadd\nload 7\nadd\nload 8\nadd\nload 9\nadd\nstore 10\npop\n",
        ),
        None,
    )
    .expect("the inputs bundle decodes");
    let mut runtime = MilkRuntime::new(bundle, 0);
    let frame = crate::dsp::AnalysisFrame {
        bass: 0.5,
        mid: 0.25,
        treb: 0.125,
        ..Default::default()
    };
    // Two frames, so `frame` has advanced off zero.
    runtime.run_frame(&frame, 0.0, 1.0 / 60.0, (32, 24), 16.0 / 9.0);
    let (outputs, _) = runtime.run_frame(&frame, 3.0, 1.0 / 60.0, (32, 24), 16.0 / 9.0);
    // The bands are doubled into MilkDrop's convention (see `BAND_SCALE`).
    let expected =
        (0.5 + 0.25 + 0.125) * 2.0 + 3.0 + 1.0 + NOMINAL_FPS + 32.0 + 24.0 + 1.0 + 16.0 / 9.0;
    // `cx` is output index 2.
    let raw = outputs[2];
    assert!(
        (raw - expected).abs() < 1e-2,
        "expected {expected}, got {raw}"
    );
}

/// **A factor a program drives past what a thirtieth power can represent
/// saturates rather than reverting to the identity.**
///
/// Not a corner: raising to [`NOMINAL_FPS`] overflows `f32` at a per-frame factor
/// of about 13, and a fallback to `1.0` there would render the most extreme zoom
/// a preset can ask for as no zoom at all — the opposite of what it says.
#[test]
fn an_overflowing_factor_saturates_in_the_direction_it_asked_for() {
    let with_zoom = |v: &str| -> f32 {
        let bundle = MilkBundle::from_assembly(
            None,
            Some(&format!(".regs zoom\n.code\nconst {v}\nstore 0\npop\n")),
            None,
        )
        .expect("decodes");
        MilkRuntime::new(bundle, 0)
            .run_frame(
                &crate::dsp::AnalysisFrame::default(),
                0.0,
                1.0 / 60.0,
                (4, 4),
                1.0,
            )
            .0[0]
    };
    assert!(
        with_zoom("50") > 1.0e20,
        "a runaway zoom stays a runaway zoom"
    );
    assert!(
        with_zoom("0.01") < 1.0e-20,
        "and a collapsing one stays collapsing"
    );
    // A non-positive factor is not a factor: it reads as the identity rather
    // than as a mirror or a hole.
    assert_eq!(with_zoom("0"), 1.0);
    assert_eq!(with_zoom("-2"), 1.0);
}

/// A bundle is **reset with the preset**: a second run from a fresh runtime gives
/// the same numbers, whatever the first one left in the register file or the
/// `megabuf`. This is the load-time half of the byte-identity claim the golden
/// baseline rests on.
#[test]
fn a_bundle_reruns_identically_from_a_reset() {
    // A program with state in every place one can hide: a register that
    // accumulates, a `megabuf` slot, and an RNG draw.
    let source = ".regs q1 zoom\n.code\n\
                  load 0\nconst 1\nadd\nstore 0\npop\n\
                  const 3\nload 0\nmemstore megabuf\npop\n\
                  const 3\nmemload megabuf\nconst 1\nfn1 rand\nadd\nstore 1\npop\n";
    let bundle = MilkBundle::from_assembly(None, Some(source), None).expect("decodes");
    let frame = crate::dsp::AnalysisFrame::default();

    let run_n = |n: usize| -> Vec<f32> {
        let mut runtime = MilkRuntime::new(bundle.clone(), 11);
        (0..n)
            .map(|i| {
                runtime
                    .run_frame(&frame, i as f32 / 60.0, 1.0 / 60.0, (8, 8), 1.0)
                    .0[0]
            })
            .collect()
    };
    assert_eq!(run_n(6), run_n(6), "two fresh runtimes agree exactly");

    // ...and `reset` gets a used runtime back to the same place.
    let mut runtime = MilkRuntime::new(bundle, 11);
    let first: Vec<f32> = (0..6)
        .map(|i| {
            runtime
                .run_frame(&frame, i as f32 / 60.0, 1.0 / 60.0, (8, 8), 1.0)
                .0[0]
        })
        .collect();
    runtime.reset();
    let again: Vec<f32> = (0..6)
        .map(|i| {
            runtime
                .run_frame(&frame, i as f32 / 60.0, 1.0 / 60.0, (8, 8), 1.0)
                .0[0]
        })
        .collect();
    assert_eq!(first, again);
}

/// `per_frame_init` runs exactly once, at load — the third program's whole
/// contract.
#[test]
fn per_frame_init_runs_once_at_load() {
    let bundle = MilkBundle::from_assembly(
        // q1 = 40
        Some(".regs q1 cx\n.code\nconst 40\nstore 0\npop\n"),
        // cx = q1; q1 = q1 + 1   (`cx` is a position, so it is not rate-converted)
        Some(".regs q1 cx\n.code\nload 0\nstore 1\npop\nload 0\nconst 1\nadd\nstore 0\npop\n"),
        None,
    )
    .expect("decodes");
    let mut runtime = MilkRuntime::new(bundle, 0);
    let frame = crate::dsp::AnalysisFrame::default();
    let raw = |runtime: &mut MilkRuntime| -> f32 {
        runtime.run_frame(&frame, 0.0, 1.0 / 60.0, (4, 4), 1.0).0[2]
    };
    assert!((raw(&mut runtime) - 40.0).abs() < 1e-2, "the init ran");
    assert!(
        (raw(&mut runtime) - 41.0).abs() < 1e-2,
        "and did not run again — `per_frame` alone advanced q1"
    );
}
