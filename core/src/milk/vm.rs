//! The stack VM that executes an [`EelProgram`](super::bytecode::EelProgram).
//!
//! **The only half of the EEL2 machine that ships.** The compiler lives in
//! `milkconv` and never enters `lmv.exe` or `foo_lmv.dll` (ADR-0113).
//!
//! # The three properties this file exists to keep
//!
//! **Total.** No operation panics and no input value can make one. Division by
//! zero yields `0`, `log(0)` yields `0`, an out-of-range `megabuf` index reads
//! `0` and writes nowhere, an unbalanced pop reads `0`, and every loop is bounded
//! by [`MAX_LOOP_ITERATIONS`]. `unwrap`/`expect`/`panic`/`indexing` are denied on
//! this path by the Plan 0002 pragma below, and `core/tests/hygiene.rs` scans
//! this directory so the denial is enforced rather than intended.
//!
//! **Allocation-free per frame.** Everything a run needs — the operand stack, the
//! loop frames, the register file, the scratch arenas — lives in a [`VmState`]
//! allocated **once at preset load** and reused. [`run`] borrows it. This executes
//! on the render thread, per vertex, so NFR §5 governs it.
//!
//! **Deterministic.** No clock, and the only randomness is a splitmix stream
//! seeded from the preset's salt (ADR-0051) and advanced by `rand()` calls. Two
//! runs of the same program over the same register file and the same VM state are
//! bit-identical, which is what keeps the capture harness a pure function of its
//! inputs.
//!
//! # What "total" costs, stated
//!
//! A total VM cannot report an error, so a program that is wrong renders wrong
//! rather than refusing. That is the right trade *here* — the alternative is a
//! preset that can stop a frame — and it is why the errors that can be caught are
//! caught at the boundary instead: the decoder validates jumps, register indices
//! and stack balance once at load
//! ([`EelProgram::from_assembly`](super::bytecode::EelProgram::from_assembly)),
//! and the converter validates its own codegen before writing a bundle.

// Hot-path panic-denial pragma (Plan 0002 Phase 2, extended to core/src/milk by
// Plan 0100 Phase 2). `run` executes per vertex per frame.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use super::bytecode::{Binary, COMPARE_EPSILON, EelProgram, Mem, Op, Unary};

/// How many slots a preset's `megabuf` holds.
///
/// EEL2's reference `megabuf` is 8 388 608 slots, which at `f32` is 32 MB **per
/// preset** — by a wide margin the largest memory number Plan 0100 could
/// introduce, and one that would be allocated whether or not a preset touched a
/// single slot. The corpus census says only **4 %** of it (435 of 10 347 files)
/// mentions `megabuf` or `gmegabuf` at all.
///
/// So the arena is sized from what the corpus plausibly uses rather than from the
/// reference: 65 536 slots, 256 KB, allocated once per loaded preset and never
/// grown. An index outside it reads `0` and writes nowhere — total, like every
/// other edge here.
///
/// **This is a number with a shelf life and a named successor.** Plan 0100
/// Phase 5's `--report` runs the converter over the corpus and ranks the failure
/// classes; a preset whose `megabuf` addressing runs past this appears there by
/// name rather than silently rendering wrong. Raise it from that evidence, not
/// from a hunch.
pub const MEGABUF_SLOTS: usize = 65_536;

/// How many slots the bundle-shared `gmegabuf` holds. Smaller than
/// [`MEGABUF_SLOTS`] because it is genuinely a *shared* scratch — the three
/// programs of one bundle passing values between frames — rather than a preset's
/// working set.
pub const GMEGABUF_SLOTS: usize = 8_192;

/// How deep `loop`/`while` frames may nest.
///
/// Four is past anything the corpus census suggests and keeps the frame stack a
/// fixed array. A `loop` opened past this depth runs its body **once** rather
/// than looping, which is the total degradation this file applies everywhere.
pub const MAX_LOOP_DEPTH: usize = 8;

/// A fixed mix-in for the VM's RNG seed, so salt `0` — the salt every preset
/// that declares no `[generator] seed` gets — is not a degenerate stream, and so
/// two subsystems seeded from the same salt do not draw the same sequence.
///
/// The ASCII of `MILKDR`, which is arbitrary and only has to be non-zero.
const MILK_SEED_MIX: u64 = 0x4D49_4C4B_4452;

/// What one [`run`] may spend: how many iterations any single loop may take, and
/// how many instructions the whole run may execute.
///
/// **Per program rather than one constant, because the three programs of a bundle
/// cost wildly different amounts.** `per_frame_init` runs once at load;
/// `per_frame` runs once a frame; `per_vertex` runs once per *vertex*, thousands
/// of times a frame. A single bound tight enough for the third would break the
/// first — the corpus's commonest `per_frame_init` idiom is
/// `loop(10000, megabuf(index) = .1; index = index + 1)`, seeding a scratch
/// array, and 84 presets in the largest pack do exactly that.
///
/// Both numbers are bounds, not budgets: nothing is reserved and an honest
/// program never approaches either. What they buy is that **untrusted program
/// text cannot hang a frame**, which is ADR-0113's stated residual risk on the
/// shader side and is closed on this side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budget {
    /// The most iterations any one `loop()` or `while()` may run.
    pub loops: u32,
    /// The most instructions the whole run may execute — the backstop under the
    /// loop bound, so a program whose bare jumps form a cycle still terminates.
    pub instructions: u32,
}

impl Budget {
    /// `per_frame_init`: runs **once**, at preset load, off the hot path. The
    /// loop bound is MilkDrop's own (`1 << 20`), and the instruction bound is
    /// generous enough that seeding a whole `megabuf` fits.
    pub const INIT: Self = Self {
        loops: 1_048_576,
        instructions: 16_000_000,
    };
    /// `per_frame`: once a frame. A million instructions is a few milliseconds
    /// on the CPUs this ships to — far past any real preset, and far under a
    /// stall.
    pub const FRAME: Self = Self {
        loops: 1_048_576,
        instructions: 1_000_000,
    };
    /// `per_vertex`: once per **vertex**, so its bound is multiplied by the mesh.
    /// At the rich tier's 5 963 vertices this ceiling is still 49 M instructions
    /// a frame in the worst case, which is why it is three orders under the
    /// others — real per-vertex programs are a few hundred instructions, and the
    /// measured tier ladder (`TierConfig::mesh_grid`) is priced against that.
    pub const VERTEX: Self = Self {
        loops: 1_024,
        instructions: 8_192,
    };
}

/// One open loop, on the VM's own frame stack.
#[derive(Clone, Copy)]
struct LoopFrame {
    /// Iterations left after the current one.
    remaining: u32,
}

/// Everything a run needs beyond the program: the register file, the scratch
/// arenas, the operand stack and the RNG.
///
/// **Allocated once at preset load.** A frame borrows it mutably and returns it
/// unchanged in shape — nothing here resizes while a preset renders, which is the
/// whole of the real-time claim.
pub struct VmState {
    /// The named variables, positionally by the program's `.regs` order.
    registers: Vec<f32>,
    /// The preset's own `megabuf`.
    megabuf: Vec<f32>,
    /// The bundle-shared `gmegabuf`.
    gmegabuf: Vec<f32>,
    /// The operand stack, sized from the program's validated depth.
    stack: Vec<f32>,
    /// Open `loop`/`while` frames.
    frames: [LoopFrame; MAX_LOOP_DEPTH],
    depth: usize,
    /// The splitmix stream `rand()` draws from (ADR-0051). Seeded from the
    /// preset's salt at load and advanced per call, so a run is reproducible and
    /// two presets writing the same expression scatter differently.
    rng: u64,
    /// The seed, kept so [`reset_rng`](Self::reset_rng) can restore it — a
    /// capture re-runs a preset from the top and must draw the same sequence.
    seed: u64,
}

impl VmState {
    /// Allocate for a program of `registers` registers and `stack_depth` operand
    /// slots, with the RNG seeded from `salt`.
    ///
    /// Sized for the **largest** of a bundle's programs by the caller, so the
    /// three of them share one state and the values a per-frame program leaves in
    /// its registers are what the per-vertex program starts from — which is
    /// MilkDrop's own execution model.
    pub fn new(registers: usize, stack_depth: usize, salt: u32) -> Self {
        Self {
            registers: vec![0.0; registers],
            megabuf: vec![0.0; MEGABUF_SLOTS],
            gmegabuf: vec![0.0; GMEGABUF_SLOTS],
            stack: vec![0.0; stack_depth.max(1)],
            frames: [LoopFrame { remaining: 0 }; MAX_LOOP_DEPTH],
            depth: 0,
            // Mixed rather than used raw, so salt 0 — the salt a preset that
            // declares no seed gets — is not a degenerate stream.
            rng: splitmix(u64::from(salt) ^ MILK_SEED_MIX),
            seed: u64::from(salt) ^ MILK_SEED_MIX,
        }
    }

    /// Grow the register file and operand stack to hold `program`, if they do not
    /// already. **Load-time only** — called when a bundle's programs are attached,
    /// never per frame.
    pub fn accommodate(&mut self, program: &EelProgram) {
        if self.registers.len() < program.register_count() {
            self.registers.resize(program.register_count(), 0.0);
        }
        if self.stack.len() < program.stack_depth() {
            self.stack.resize(program.stack_depth(), 0.0);
        }
    }

    /// Read register `index`, or `0` for one that does not exist.
    pub fn get(&self, index: u16) -> f32 {
        self.registers.get(index as usize).copied().unwrap_or(0.0)
    }

    /// Write register `index`, ignoring one that does not exist.
    pub fn set(&mut self, index: u16, value: f32) {
        if let Some(slot) = self.registers.get_mut(index as usize) {
            *slot = value;
        }
    }

    /// Zero every register — what a preset switch does before the first
    /// `per_frame_init` run.
    pub fn clear_registers(&mut self) {
        self.registers.fill(0.0);
    }

    /// Zero both scratch arenas. Load-time: a preset must not inherit the
    /// previous one's `megabuf`.
    pub fn clear_memory(&mut self) {
        self.megabuf.fill(0.0);
        self.gmegabuf.fill(0.0);
    }

    /// Restore the RNG to its seed, so a re-run of a preset from frame zero draws
    /// the same sequence (ADR-0051 / NFR §6).
    pub fn reset_rng(&mut self) {
        self.rng = splitmix(self.seed);
    }

    /// The next draw in `[0, 1)`.
    fn next_unit(&mut self) -> f32 {
        self.rng = self.rng.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let h = splitmix(self.rng);
        // The top 24 bits as a unit fraction — the same construction
        // `gpu::HASH_WGSL`'s `unit01` uses, so CPU and shader randomness read the
        // same way even though they are separate streams.
        (h >> 40) as f32 / (1u64 << 24) as f32
    }
}

/// One round of splitmix64. Deterministic, dependency-free, and the same mixer
/// `scenes::SeededRng` uses — one hash in this crate, not two.
fn splitmix(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Execute `program` against `state`, returning the value it leaves on top.
///
/// Total on every input: see the module docs. The register file, the arenas and
/// the RNG are `state`'s and persist across calls, which is what lets a
/// per-frame program set up values a per-vertex program reads.
pub fn run(program: &EelProgram, state: &mut VmState, budget: Budget) -> f32 {
    let code = program.code();
    // A run starts with an empty stack and no open loops. Both are properties of
    // the *call*, not of the state, so a program that left something behind
    // cannot poison the next one.
    let mut sp = 0usize;
    state.depth = 0;
    let mut pc = 0usize;
    let mut fuel = budget.instructions;

    while let Some(&op) = code.get(pc) {
        if fuel == 0 {
            break;
        }
        fuel -= 1;
        pc += 1;
        match op {
            Op::Const(v) => push(state, &mut sp, v),
            Op::Load(index) => {
                let v = state.get(index);
                push(state, &mut sp, v);
            }
            Op::Store(index) => {
                // Assignment is an expression: the value stays on the stack.
                let v = peek(state, sp);
                state.set(index, v);
            }
            Op::Pop => {
                pop(state, &mut sp);
            }
            Op::Neg => unary(state, &mut sp, |a| -a),
            Op::Not => unary(state, &mut sp, |a| f32::from(a == 0.0)),
            Op::Add => binary(state, &mut sp, |a, b| a + b),
            Op::Sub => binary(state, &mut sp, |a, b| a - b),
            Op::Mul => binary(state, &mut sp, |a, b| a * b),
            // EEL2 yields 0 on a zero divisor rather than an infinity, and the
            // non-finite guard is ours: a NaN here would poison a register for
            // the rest of the preset's run.
            Op::Div => binary(state, &mut sp, |a, b| {
                finite(if b == 0.0 { 0.0 } else { a / b })
            }),
            Op::Mod => binary(state, &mut sp, |a, b| {
                let d = b.trunc();
                if d == 0.0 { 0.0 } else { finite(a.trunc() % d) }
            }),
            Op::Pow => binary(state, &mut sp, |a, b| finite(a.powf(b))),
            Op::Above => binary(state, &mut sp, |a, b| f32::from(a > b)),
            Op::Below => binary(state, &mut sp, |a, b| f32::from(a < b)),
            Op::AboveEq => binary(state, &mut sp, |a, b| f32::from(a >= b)),
            Op::BelowEq => binary(state, &mut sp, |a, b| f32::from(a <= b)),
            Op::Equal => binary(state, &mut sp, |a, b| {
                f32::from((a - b).abs() < COMPARE_EPSILON)
            }),
            Op::NotEqual => binary(state, &mut sp, |a, b| {
                f32::from((a - b).abs() >= COMPARE_EPSILON)
            }),
            // Bitwise on the truncated integer parts, in EEL2's own 32-bit
            // domain. A value outside `i32` saturates rather than wrapping —
            // `as` on a float does that in Rust, and it is the total answer.
            Op::BitAnd => binary(state, &mut sp, |a, b| ((a as i32) & (b as i32)) as f32),
            Op::BitOr => binary(state, &mut sp, |a, b| ((a as i32) | (b as i32)) as f32),
            Op::Fn1(f) => {
                let a = pop(state, &mut sp);
                let v = apply_unary(f, a, state);
                push(state, &mut sp, v);
            }
            Op::Fn2(f) => {
                let b = pop(state, &mut sp);
                let a = pop(state, &mut sp);
                push(state, &mut sp, apply_binary(f, a, b));
            }
            Op::MemLoad(which) => {
                let index = pop(state, &mut sp);
                let v = mem_read(state, which, index);
                push(state, &mut sp, v);
            }
            Op::MemStore(which) => {
                let value = pop(state, &mut sp);
                let index = pop(state, &mut sp);
                mem_write(state, which, index, value);
                // Like `Store`, an assignment yields what it assigned.
                push(state, &mut sp, value);
            }
            Op::Jump(t) => pc = t as usize,
            Op::JumpIfZero(t) => {
                if pop(state, &mut sp) == 0.0 {
                    pc = t as usize;
                }
            }
            Op::JumpIfNotZero(t) => {
                if pop(state, &mut sp) != 0.0 {
                    pc = t as usize;
                }
            }
            Op::LoopBegin(end) => {
                let count = pop(state, &mut sp);
                // Non-finite or under one means no iterations at all. The clamp is
                // what makes an untrusted program unable to hang a frame.
                let iterations = if count.is_finite() && count >= 1.0 {
                    (count as u32).min(budget.loops)
                } else {
                    0
                };
                if iterations == 0 || !open_frame(state, iterations) {
                    // A loop that runs no body is still an expression.
                    push(state, &mut sp, 0.0);
                    pc = end as usize;
                }
            }
            Op::LoopEnd(start) => {
                // The body's value is discarded; the loop's own value is 0.
                pop(state, &mut sp);
                if step_frame(state) {
                    pc = start as usize;
                } else {
                    close_frame(state);
                    push(state, &mut sp, 0.0);
                }
            }
            Op::WhileBegin(end) => {
                // `while(body)` runs the body first, so the operand its codegen
                // pushed is only the iteration bound.
                pop(state, &mut sp);
                if !open_frame(state, budget.loops) {
                    push(state, &mut sp, 0.0);
                    pc = end as usize;
                }
            }
            Op::WhileEnd(start) => {
                let value = pop(state, &mut sp);
                if value != 0.0 && step_frame(state) {
                    pc = start as usize;
                } else {
                    close_frame(state);
                    push(state, &mut sp, 0.0);
                }
            }
        }
    }
    if sp == 0 { 0.0 } else { peek(state, sp) }
}

/// Coerce a non-finite result to `0`.
///
/// **Not cosmetic.** A `NaN` written into a register survives every comparison
/// (`NaN > x` is false, `NaN == x` is false), so a single one poisons a
/// per-frame variable for the rest of the preset's run and the picture never
/// comes back. The same reasoning `Easing::step` carries for the smoother.
fn finite(v: f32) -> f32 {
    if v.is_finite() { v } else { 0.0 }
}

fn push(state: &mut VmState, sp: &mut usize, value: f32) {
    if let Some(slot) = state.stack.get_mut(*sp) {
        *slot = value;
        *sp += 1;
    }
    // A full stack drops the push rather than growing: the depth came from the
    // decoder's validation, so this is unreachable for a validated program and
    // must not allocate for a hand-written one.
}

fn pop(state: &VmState, sp: &mut usize) -> f32 {
    match sp.checked_sub(1) {
        Some(next) => {
            *sp = next;
            state.stack.get(next).copied().unwrap_or(0.0)
        }
        None => 0.0,
    }
}

fn peek(state: &VmState, sp: usize) -> f32 {
    sp.checked_sub(1)
        .and_then(|i| state.stack.get(i).copied())
        .unwrap_or(0.0)
}

fn unary(state: &mut VmState, sp: &mut usize, f: impl Fn(f32) -> f32) {
    let a = pop(state, sp);
    push(state, sp, finite(f(a)));
}

fn binary(state: &mut VmState, sp: &mut usize, f: impl Fn(f32, f32) -> f32) {
    let b = pop(state, sp);
    let a = pop(state, sp);
    push(state, sp, f(a, b));
}

/// Open a loop frame, or report that the nesting limit is reached.
fn open_frame(state: &mut VmState, iterations: u32) -> bool {
    if state.depth >= MAX_LOOP_DEPTH {
        return false;
    }
    if let Some(frame) = state.frames.get_mut(state.depth) {
        frame.remaining = iterations.saturating_sub(1);
        state.depth += 1;
        return true;
    }
    false
}

/// Consume one iteration of the innermost frame; `true` to go round again.
fn step_frame(state: &mut VmState) -> bool {
    let Some(index) = state.depth.checked_sub(1) else {
        return false;
    };
    match state.frames.get_mut(index) {
        Some(frame) if frame.remaining > 0 => {
            frame.remaining -= 1;
            true
        }
        _ => false,
    }
}

fn close_frame(state: &mut VmState) {
    state.depth = state.depth.saturating_sub(1);
}

fn apply_unary(f: Unary, a: f32, state: &mut VmState) -> f32 {
    let v = match f {
        Unary::Sin => a.sin(),
        Unary::Cos => a.cos(),
        Unary::Tan => a.tan(),
        Unary::Asin => a.clamp(-1.0, 1.0).asin(),
        Unary::Acos => a.clamp(-1.0, 1.0).acos(),
        Unary::Atan => a.atan(),
        // A negative argument is `0` rather than a NaN — total, and the value a
        // preset squaring a difference and taking its root expects at zero.
        Unary::Sqrt => a.max(0.0).sqrt(),
        Unary::InvSqrt => {
            let r = a.max(0.0).sqrt();
            if r == 0.0 { 0.0 } else { 1.0 / r }
        }
        Unary::Exp => a.exp(),
        Unary::Log => {
            if a > 0.0 {
                a.ln()
            } else {
                0.0
            }
        }
        Unary::Log10 => {
            if a > 0.0 {
                a.log10()
            } else {
                0.0
            }
        }
        Unary::Abs => a.abs(),
        // EEL2's `sign` is a three-way: -1, 0 or 1. `f32::signum` says 1.0 for
        // +0.0, which is a different function.
        Unary::Sign => {
            if a > 0.0 {
                1.0
            } else if a < 0.0 {
                -1.0
            } else {
                0.0
            }
        }
        Unary::Sqr => a * a,
        Unary::Floor => a.floor(),
        Unary::Ceil => a.ceil(),
        Unary::Int => a.trunc(),
        Unary::BNot => f32::from(a == 0.0),
        Unary::Rand => state.next_unit() * a,
        Unary::RandInt => (state.next_unit() * a).floor(),
    };
    finite(v)
}

fn apply_binary(f: Binary, a: f32, b: f32) -> f32 {
    let v = match f {
        // `f32::min`/`max` return the non-NaN operand, which is the total
        // behaviour wanted here.
        Binary::Min => a.min(b),
        Binary::Max => a.max(b),
        Binary::Pow => a.powf(b),
        Binary::Atan2 => a.atan2(b),
        // EEL2's sigmoid: `1 / (1 + exp(-x * c))`, with `c = 0` degenerating to
        // the midpoint rather than to a division by zero.
        Binary::Sigmoid => {
            let t = 1.0 + (-a * b).exp();
            if t == 0.0 { 0.0 } else { 1.0 / t }
        }
        Binary::BAnd => f32::from(a != 0.0 && b != 0.0),
        Binary::BOr => f32::from(a != 0.0 || b != 0.0),
        Binary::Above => f32::from(a > b),
        Binary::Below => f32::from(a < b),
        Binary::Equal => f32::from((a - b).abs() < COMPARE_EPSILON),
    };
    finite(v)
}

/// The slot `index` addresses, or `None` when it is outside the arena.
///
/// EEL2 indexes `megabuf` by a float; the truncation is the language's, and the
/// range check is ours.
fn slot(index: f32, len: usize) -> Option<usize> {
    if !index.is_finite() || index < 0.0 {
        return None;
    }
    let i = index as usize;
    (i < len).then_some(i)
}

fn mem_read(state: &VmState, which: Mem, index: f32) -> f32 {
    let arena = match which {
        Mem::Local => &state.megabuf,
        Mem::Global => &state.gmegabuf,
    };
    slot(index, arena.len())
        .and_then(|i| arena.get(i).copied())
        .unwrap_or(0.0)
}

fn mem_write(state: &mut VmState, which: Mem, index: f32, value: f32) {
    let arena = match which {
        Mem::Local => &mut state.megabuf,
        Mem::Global => &mut state.gmegabuf,
    };
    if let Some(cell) = slot(index, arena.len()).and_then(|i| arena.get_mut(i)) {
        *cell = finite(value);
    }
}
