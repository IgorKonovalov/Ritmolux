//! The bytecode an EEL2 program compiles to, and its text encoding.
//!
//! **The seam between the two halves of Plan 0100.** `milkconv` compiles `.milk`
//! text into an [`EelProgram`]; `core` executes one. Nothing here parses EEL2 —
//! the parser is in the converter and never ships (ADR-0113).
//!
//! # Why the encoding is text
//!
//! A bundle carries a program as **assembly text**, not as packed bytes, and it
//! is a deliberate trade of a few kilobytes for four properties:
//!
//! - **No serialization dependency.** `serde` would have to describe an enum with
//!   thirty-odd variants into a format `toml` can carry; hand-written `Display` +
//!   `FromStr` over one op per line costs nothing and adds no crate
//!   (`lightweight is a feature`).
//! - **A bundle is diffable.** A converted preset's program shows up in review as
//!   lines, so a converter change that perturbs codegen is visible rather than a
//!   changed blob.
//! - **Round-tripping is a property, not a hope.** [`EelProgram::to_assembly`] and
//!   [`EelProgram::from_assembly`] are inverses, which is assertable and asserted.
//! - **A malformed program is a surfaced load error**, like every other preset
//!   boundary (ADR-0002 / NFR §10) — the decoder validates jump targets and
//!   register indices once, here, so the VM can trust them.
//!
//! # What the VM may assume after decoding
//!
//! [`EelProgram::from_assembly`] rejects a program whose jump target is out of
//! range or whose register index is at or past `n_regs`. Everything downstream —
//! [`vm::run`](super::vm::run) — indexes on that guarantee, which is what lets the
//! interpreter loop stay free of bounds checks it would otherwise pay per op per
//! vertex. **Nothing constructs an `EelProgram` except this decoder and the
//! converter's codegen**, and the codegen runs its own output through
//! [`EelProgram::validate`] before writing it.

// Load-time code, but this module is named in the hygiene guard's scan set
// (Plan 0100 Phase 2): the VM beside it runs per vertex per frame, and the file
// convention for anything the guard scans is the panic-denial pragma.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use std::fmt;

/// A one-argument builtin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unary {
    /// EEL2's `sin` — radians.
    Sin,
    /// EEL2's `cos` — radians.
    Cos,
    /// EEL2's `tan` — radians.
    Tan,
    /// EEL2's `asin`, on an argument clamped into `-1..=1`.
    Asin,
    /// EEL2's `acos`, on an argument clamped into `-1..=1`.
    Acos,
    /// EEL2's `atan`.
    Atan,
    /// EEL2's `sqrt`, `0` on a negative argument.
    Sqrt,
    /// `1 / sqrt(x)`, EEL2's `invsqrt`.
    InvSqrt,
    /// EEL2's `exp`.
    Exp,
    /// Natural log, EEL2's `log`.
    Log,
    /// Base-10 logarithm, EEL2's `log10`.
    Log10,
    /// EEL2's `abs`.
    Abs,
    /// EEL2's three-way `sign`: `-1`, `0` or `1`.
    Sign,
    /// `x * x`, EEL2's `sqr`.
    Sqr,
    /// EEL2's `floor`.
    Floor,
    /// EEL2's `ceil`.
    Ceil,
    /// Truncation toward zero, EEL2's `int`.
    Int,
    /// EEL2's `bnot`: `1` when the argument is zero, else `0`.
    BNot,
    /// `rand(x)` — a salted deterministic draw in `[0, x)` (ADR-0051).
    Rand,
    /// `randint(x)` — `floor(rand(x))`.
    RandInt,
}

impl Unary {
    /// The name the assembly text uses, which is also the EEL2 spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Unary::Sin => "sin",
            Unary::Cos => "cos",
            Unary::Tan => "tan",
            Unary::Asin => "asin",
            Unary::Acos => "acos",
            Unary::Atan => "atan",
            Unary::Sqrt => "sqrt",
            Unary::InvSqrt => "invsqrt",
            Unary::Exp => "exp",
            Unary::Log => "log",
            Unary::Log10 => "log10",
            Unary::Abs => "abs",
            Unary::Sign => "sign",
            Unary::Sqr => "sqr",
            Unary::Floor => "floor",
            Unary::Ceil => "ceil",
            Unary::Int => "int",
            Unary::BNot => "bnot",
            Unary::Rand => "rand",
            Unary::RandInt => "randint",
        }
    }

    /// Parse an assembly/EEL2 name.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "sin" => Unary::Sin,
            "cos" => Unary::Cos,
            "tan" => Unary::Tan,
            "asin" => Unary::Asin,
            "acos" => Unary::Acos,
            "atan" => Unary::Atan,
            "sqrt" => Unary::Sqrt,
            "invsqrt" => Unary::InvSqrt,
            "exp" => Unary::Exp,
            "log" => Unary::Log,
            "log10" => Unary::Log10,
            "abs" => Unary::Abs,
            "sign" => Unary::Sign,
            "sqr" => Unary::Sqr,
            "floor" => Unary::Floor,
            "ceil" => Unary::Ceil,
            "int" => Unary::Int,
            "bnot" => Unary::BNot,
            "rand" => Unary::Rand,
            "randint" => Unary::RandInt,
            _ => return None,
        })
    }

    /// Whether this builtin reads the VM's RNG rather than only its argument.
    /// The two that do are the reason a program's execution is a function of
    /// `(inputs, salt, call sequence)` rather than of the inputs alone.
    pub fn is_random(self) -> bool {
        matches!(self, Unary::Rand | Unary::RandInt)
    }
}

/// A two-argument builtin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Binary {
    /// EEL2's `min`.
    Min,
    /// EEL2's `max`.
    Max,
    /// EEL2's `pow`.
    Pow,
    /// EEL2's `atan2(y, x)`.
    Atan2,
    /// EEL2's `sigmoid(x, c)`.
    Sigmoid,
    /// EEL2's `band` — non-lazy logical and. `&&` compiles to jumps instead.
    BAnd,
    /// EEL2's `bor` — non-lazy logical or.
    BOr,
    /// EEL2's `above(a, b)`.
    Above,
    /// EEL2's `below(a, b)`.
    Below,
    /// EEL2's `equal(a, b)`.
    Equal,
}

impl Binary {
    /// The name the assembly text uses.
    pub fn as_str(self) -> &'static str {
        match self {
            Binary::Min => "min",
            Binary::Max => "max",
            Binary::Pow => "pow",
            Binary::Atan2 => "atan2",
            Binary::Sigmoid => "sigmoid",
            Binary::BAnd => "band",
            Binary::BOr => "bor",
            Binary::Above => "above",
            Binary::Below => "below",
            Binary::Equal => "equal",
        }
    }

    /// Parse an assembly/EEL2 name.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "min" => Binary::Min,
            "max" => Binary::Max,
            "pow" => Binary::Pow,
            "atan2" => Binary::Atan2,
            "sigmoid" => Binary::Sigmoid,
            "band" => Binary::BAnd,
            "bor" => Binary::BOr,
            "above" => Binary::Above,
            "below" => Binary::Below,
            "equal" => Binary::Equal,
            _ => return None,
        })
    }
}

/// Which scratch arena a memory op addresses.
///
/// `megabuf` is the preset's own; `gmegabuf` is shared across the three programs
/// of one bundle. Both are **fixed arenas allocated once at preset load** —
/// nothing here grows per frame, because this runs on the render thread
/// (NFR §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mem {
    /// EEL2's `megabuf`.
    Local,
    /// EEL2's `gmegabuf`.
    Global,
}

impl Mem {
    /// The name the assembly text uses.
    pub fn as_str(self) -> &'static str {
        match self {
            Mem::Local => "megabuf",
            Mem::Global => "gmegabuf",
        }
    }

    /// Parse an assembly/EEL2 name.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "megabuf" => Mem::Local,
            "gmegabuf" => Mem::Global,
            _ => return None,
        })
    }
}

/// One bytecode instruction.
///
/// A **stack** machine, not a register one, and deliberately: EEL2 is an
/// expression language whose statements are expressions, so a stack matches its
/// shape and the codegen is a post-order walk with no allocation. The named
/// variables *are* registers ([`Op::Load`] / [`Op::Store`]); the stack is only
/// the arithmetic's scratch.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Op {
    /// Push a literal.
    Const(f32),
    /// Push register `n`.
    Load(u16),
    /// Pop, store into register `n`, and **push the value back** — EEL2's
    /// assignment is an expression that yields what it assigned.
    Store(u16),
    /// Discard the top of the stack. Emitted between the statements of a
    /// sequence, whose value is its last statement's.
    Pop,
    /// Arithmetic negation.
    Neg,
    /// Addition.
    Add,
    /// Subtraction.
    Sub,
    /// Multiplication.
    Mul,
    /// Division. **Total**: a zero divisor yields `0`, which is EEL2's own
    /// behaviour and what keeps the VM panic-free (Plan 0100 Phase 2).
    Div,
    /// Remainder, on the integer parts, EEL2's `%`. Total the same way.
    Mod,
    /// EEL2's `^`, which is exponentiation rather than xor.
    /// EEL2's `pow`.
    Pow,
    /// `>` — pushes `1` or `0`.
    Above,
    /// `<`.
    Below,
    /// `>=`.
    AboveEq,
    /// `<=`.
    BelowEq,
    /// `==`, against EEL2's comparison epsilon.
    Equal,
    /// `!=`, against the same epsilon.
    NotEqual,
    /// `!` — `1` when the operand is zero, else `0`.
    Not,
    /// EEL2's `&` — **bitwise** and on the truncated integer parts, not a
    /// logical one. `band` is the logical operator, and the two are different
    /// functions: `3 & 4` is `0` where `band(3, 4)` is `1`.
    BitAnd,
    /// EEL2's `|` — bitwise or, the counterpart of [`Op::BitAnd`].
    BitOr,
    /// One-argument builtin.
    Fn1(Unary),
    /// Two-argument builtin.
    Fn2(Binary),
    /// Pop an index, push that slot.
    MemLoad(Mem),
    /// Pop a value and an index (value on top), store, and push the value back.
    MemStore(Mem),
    /// Unconditional jump to an absolute instruction index.
    Jump(u32),
    /// Pop; jump when the value is zero.
    JumpIfZero(u32),
    /// Pop; jump when the value is non-zero.
    JumpIfNotZero(u32),
    /// Pop a count, open a loop frame, and jump past [`Op::LoopEnd`] when the
    /// count rounds to less than one. The count is clamped to
    /// [`MAX_LOOP_ITERATIONS`].
    LoopBegin(u32),
    /// Close a loop iteration: discard the body's value, decrement, and jump back
    /// to the operand while iterations remain. Pushes `0` when the loop ends,
    /// because a loop is an expression.
    LoopEnd(u32),
    /// Open a `while` frame with [`MAX_LOOP_ITERATIONS`] iterations, jumping past
    /// its [`Op::WhileEnd`] never — the test is at the end, because EEL2's
    /// `while(body)` runs the body first.
    WhileBegin(u32),
    /// Close a `while` iteration: pop the body's value, decrement, and jump back
    /// to the operand while it is non-zero and iterations remain. Pushes `0`
    /// when the loop ends.
    WhileEnd(u32),
}

/// EEL2's comparison epsilon: `==` is true within this, and `!=` outside it.
///
/// Not a tolerance we chose — it is part of the language, and a preset written
/// against it (`equal(x, 0)` after arithmetic that lands near zero) reads
/// differently under an exact comparison.
pub const COMPARE_EPSILON: f32 = 0.00001;

/// One compiled EEL2 program: flat bytecode over a fixed register file.
///
/// Fixed-size at load; nothing here grows per frame.
#[derive(Debug, Clone, PartialEq)]
pub struct EelProgram {
    /// The instructions, in execution order. Jump operands are indices into this.
    code: Vec<Op>,
    /// Register names, positionally — `names[i]` is register `i`. The host
    /// resolves the roster it cares about (`zoom`, `q1`, `bass`, …) to indices
    /// **once at load** through [`register`](Self::register), so the frame loop
    /// never looks a name up.
    names: Vec<String>,
    /// The deepest the operand stack can get, computed at validation. The VM
    /// sizes its stack from it, so a running program cannot outgrow one.
    stack_depth: usize,
    /// Every register this program can write, ascending and deduplicated.
    ///
    /// The per-vertex driver's whole optimization. MilkDrop runs a per-vertex
    /// program against a **copy** of the per-frame register state, so writes
    /// inside it do not leak from one vertex to the next; copying the whole file
    /// per vertex is thousands of floats per frame for the sake of a handful that
    /// actually move. Snapshotting only these is the same semantics at a fraction
    /// of the memory traffic, and it is exact rather than an approximation —
    /// a register no `Store` names cannot change.
    written: Vec<u16>,
}

/// What is wrong with a program, as a surfaced load error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgramError {
    /// A jump operand addresses no instruction.
    BadJump {
        /// The instruction holding the jump.
        at: usize,
        /// The operand.
        target: u32,
    },
    /// A register index is at or past the declared register count.
    BadRegister {
        /// The instruction holding it.
        at: usize,
        /// The index.
        index: u16,
    },
    /// The program pops more than it pushes somewhere, so the stack would
    /// underflow.
    StackUnderflow {
        /// The instruction that would underflow.
        at: usize,
    },
    /// A `.regs` name appears twice, so [`register`](EelProgram::register) could
    /// not say which one a host meant.
    DuplicateRegister(String),
    /// An assembly line the decoder does not recognize.
    BadLine {
        /// 1-based line number in the assembly text.
        line: usize,
        /// The offending text.
        text: String,
    },
    /// A `.regs` or `.code` header is missing or out of order.
    BadHeader(&'static str),
}

impl fmt::Display for ProgramError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProgramError::BadJump { at, target } => {
                write!(
                    f,
                    "instruction {at} jumps to {target}, which is not an instruction"
                )
            }
            ProgramError::BadRegister { at, index } => {
                write!(
                    f,
                    "instruction {at} addresses register {index}, which is not declared"
                )
            }
            ProgramError::StackUnderflow { at } => {
                write!(f, "instruction {at} pops from an empty stack")
            }
            ProgramError::DuplicateRegister(name) => {
                write!(f, "register '{name}' is declared twice")
            }
            ProgramError::BadLine { line, text } => {
                write!(f, "line {line}: unrecognized instruction '{text}'")
            }
            ProgramError::BadHeader(what) => write!(f, "{what}"),
        }
    }
}

impl std::error::Error for ProgramError {}

impl EelProgram {
    /// A program with no instructions and no registers — the identity, and what
    /// an absent section in a bundle means.
    pub fn empty() -> Self {
        Self {
            code: Vec::new(),
            names: Vec::new(),
            stack_depth: 0,
            written: Vec::new(),
        }
    }

    /// Assemble from code and register names, validating both.
    ///
    /// **The only constructor**, so the invariants [`validate`](Self::validate)
    /// establishes hold of every `EelProgram` that exists.
    pub fn new(code: Vec<Op>, names: Vec<String>) -> Result<Self, ProgramError> {
        let mut program = Self {
            code,
            names,
            stack_depth: 0,
            written: Vec::new(),
        };
        program.stack_depth = program.validate()?;
        program.written = program
            .code
            .iter()
            .filter_map(|op| match op {
                Op::Store(index) => Some(*index),
                _ => None,
            })
            .collect();
        program.written.sort_unstable();
        program.written.dedup();
        Ok(program)
    }

    /// The instructions.
    pub fn code(&self) -> &[Op] {
        &self.code
    }

    /// The register names, positionally.
    pub fn names(&self) -> &[String] {
        &self.names
    }

    /// How many registers the program declares.
    pub fn register_count(&self) -> usize {
        self.names.len()
    }

    /// The deepest the operand stack gets. The VM sizes its stack from this.
    pub fn stack_depth(&self) -> usize {
        self.stack_depth
    }

    /// Every register this program can write — see the field.
    pub fn written_registers(&self) -> &[u16] {
        &self.written
    }

    /// The index of the register called `name`, or `None`.
    ///
    /// **Called at load, never per frame.** The host resolves the whole roster it
    /// cares about once and then addresses registers by index.
    pub fn register(&self, name: &str) -> Option<u16> {
        self.names
            .iter()
            .position(|n| n == name)
            .and_then(|i| u16::try_from(i).ok())
    }

    /// Whether the program can draw from the RNG, i.e. whether its output depends
    /// on anything but its inputs and its register file.
    ///
    /// Read by the capture harness's determinism argument: a program that says
    /// `false` here is a pure function of its inputs, and one that says `true` is
    /// a pure function of its inputs **and the VM's seeded RNG state**, which is
    /// itself reset with the preset (ADR-0051).
    pub fn uses_random(&self) -> bool {
        self.code
            .iter()
            .any(|op| matches!(op, Op::Fn1(f) if f.is_random()))
    }

    /// Check every jump target, every register index, and that the stack never
    /// underflows; return the deepest the stack gets.
    ///
    /// The stack walk is a **linear** pass rather than a flow-sensitive one: it
    /// takes each instruction's net effect in program order. That is exact for
    /// the codegen this crate's converter emits — every branch it generates is
    /// stack-balanced at its join — and conservative nowhere, because an
    /// unbalanced branch would show up as a mismatch at the join in a
    /// flow-sensitive walk and as a wrong depth here. The VM does not rely on the
    /// depth being tight: it is a capacity, and the interpreter guards its own
    /// pops (returning `0` on an empty stack) so a hand-written program that
    /// defeats this walk misbehaves rather than panicking.
    fn validate(&self) -> Result<usize, ProgramError> {
        let len = self.code.len();
        for (at, op) in self.code.iter().enumerate() {
            match *op {
                Op::Jump(t)
                | Op::JumpIfZero(t)
                | Op::JumpIfNotZero(t)
                | Op::LoopBegin(t)
                | Op::LoopEnd(t)
                | Op::WhileBegin(t)
                | Op::WhileEnd(t) => {
                    // A jump to exactly `len` is the ordinary "fall off the end"
                    // target the codegen emits for a branch over the last
                    // instruction, so it is in range.
                    if t as usize > len {
                        return Err(ProgramError::BadJump { at, target: t });
                    }
                }
                Op::Load(index) | Op::Store(index) if index as usize >= self.names.len() => {
                    return Err(ProgramError::BadRegister { at, index });
                }
                _ => {}
            }
        }
        for (i, name) in self.names.iter().enumerate() {
            if self.names.iter().take(i).any(|n| n == name) {
                return Err(ProgramError::DuplicateRegister(name.clone()));
            }
        }

        let mut depth = 0i64;
        let mut peak = 0i64;
        for (at, op) in self.code.iter().enumerate() {
            let (pops, pushes) = op.stack_effect();
            depth -= pops as i64;
            if depth < 0 {
                return Err(ProgramError::StackUnderflow { at });
            }
            depth += pushes as i64;
            peak = peak.max(depth);
        }
        // One slot of headroom, so an interpreter push never has to test the
        // capacity it was sized from.
        Ok(peak.max(0) as usize + 1)
    }
}

impl Op {
    /// How many operands this instruction pops and pushes.
    ///
    /// The loop ops are the interesting ones: `LoopBegin` pops its count and
    /// pushes nothing (the counter lives on the VM's separate loop stack), and
    /// `LoopEnd` pops the body's value and pushes the loop's own result — so a
    /// loop is net neutral, which is what makes it usable as a sub-expression.
    fn stack_effect(self) -> (u8, u8) {
        match self {
            Op::Const(_) | Op::Load(_) => (0, 1),
            Op::Store(_) => (1, 1),
            Op::Pop => (1, 0),
            Op::Neg | Op::Not | Op::Fn1(_) | Op::MemLoad(_) => (1, 1),
            Op::Add
            | Op::Sub
            | Op::Mul
            | Op::Div
            | Op::Mod
            | Op::Pow
            | Op::Above
            | Op::Below
            | Op::AboveEq
            | Op::BelowEq
            | Op::Equal
            | Op::NotEqual
            | Op::BitAnd
            | Op::BitOr
            | Op::Fn2(_) => (2, 1),
            Op::MemStore(_) => (2, 1),
            Op::Jump(_) => (0, 0),
            Op::JumpIfZero(_) | Op::JumpIfNotZero(_) => (1, 0),
            Op::LoopBegin(_) | Op::WhileBegin(_) => (1, 0),
            Op::LoopEnd(_) | Op::WhileEnd(_) => (1, 1),
        }
    }
}

// ---------------------------------------------------------------------------
// The text encoding
// ---------------------------------------------------------------------------

impl EelProgram {
    /// This program as assembly text — the form a bundle carries.
    ///
    /// ```text
    /// .regs zoom rot _t0
    /// .code
    /// const 1.5
    /// load 0
    /// mul
    /// store 0
    /// ```
    ///
    /// The inverse of [`from_assembly`](Self::from_assembly), which is asserted
    /// rather than intended.
    pub fn to_assembly(&self) -> String {
        let mut out = String::new();
        out.push_str(".regs");
        for name in &self.names {
            out.push(' ');
            out.push_str(name);
        }
        out.push_str("\n.code\n");
        for op in &self.code {
            out.push_str(&op_to_text(*op));
            out.push('\n');
        }
        out
    }

    /// Decode assembly text, validating it into a program the VM may trust.
    ///
    /// Blank lines and `#` comments are ignored, so a bundle stays legible.
    pub fn from_assembly(text: &str) -> Result<Self, ProgramError> {
        let mut names: Vec<String> = Vec::new();
        let mut code: Vec<Op> = Vec::new();
        let mut seen_regs = false;
        let mut seen_code = false;
        for (index, raw) in text.lines().enumerate() {
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            if let Some(rest) = line.strip_prefix(".regs") {
                if seen_regs {
                    return Err(ProgramError::BadHeader("a program declares .regs twice"));
                }
                seen_regs = true;
                names = rest.split_whitespace().map(str::to_string).collect();
                continue;
            }
            if line == ".code" {
                if !seen_regs {
                    return Err(ProgramError::BadHeader(".code appears before .regs"));
                }
                seen_code = true;
                continue;
            }
            if !seen_code {
                return Err(ProgramError::BadHeader(
                    "an instruction appears before .code",
                ));
            }
            let op = op_from_text(line).ok_or_else(|| ProgramError::BadLine {
                line: index + 1,
                text: line.to_string(),
            })?;
            code.push(op);
        }
        if !seen_regs || !seen_code {
            return Err(ProgramError::BadHeader(
                "a program needs a .regs line and a .code line",
            ));
        }
        Self::new(code, names)
    }
}

/// One instruction as a line of assembly.
fn op_to_text(op: Op) -> String {
    match op {
        // `{:?}` on an `f32` round-trips exactly (Rust prints the shortest
        // representation that parses back to the same bits), which is what makes
        // the encoding lossless.
        Op::Const(v) => format!("const {v:?}"),
        Op::Load(n) => format!("load {n}"),
        Op::Store(n) => format!("store {n}"),
        Op::Pop => "pop".into(),
        Op::Neg => "neg".into(),
        Op::Not => "not".into(),
        Op::Add => "add".into(),
        Op::Sub => "sub".into(),
        Op::Mul => "mul".into(),
        Op::Div => "div".into(),
        Op::Mod => "mod".into(),
        Op::Pow => "pow".into(),
        Op::Above => "above".into(),
        Op::Below => "below".into(),
        Op::AboveEq => "aboveeq".into(),
        Op::BelowEq => "beloweq".into(),
        Op::Equal => "equal".into(),
        Op::NotEqual => "notequal".into(),
        Op::BitAnd => "bitand".into(),
        Op::BitOr => "bitor".into(),
        Op::Fn1(f) => format!("fn1 {}", f.as_str()),
        Op::Fn2(f) => format!("fn2 {}", f.as_str()),
        Op::MemLoad(m) => format!("memload {}", m.as_str()),
        Op::MemStore(m) => format!("memstore {}", m.as_str()),
        Op::Jump(t) => format!("jump {t}"),
        Op::JumpIfZero(t) => format!("jz {t}"),
        Op::JumpIfNotZero(t) => format!("jnz {t}"),
        Op::LoopBegin(t) => format!("loopbegin {t}"),
        Op::LoopEnd(t) => format!("loopend {t}"),
        Op::WhileBegin(t) => format!("whilebegin {t}"),
        Op::WhileEnd(t) => format!("whileend {t}"),
    }
}

/// One line of assembly as an instruction, or `None` if it is not one.
fn op_from_text(line: &str) -> Option<Op> {
    let mut parts = line.split_whitespace();
    let head = parts.next()?;
    let arg = parts.next();
    if parts.next().is_some() {
        return None;
    }
    let index = || arg.and_then(|a| a.parse::<u16>().ok());
    let target = || arg.and_then(|a| a.parse::<u32>().ok());
    Some(match (head, arg) {
        ("const", Some(v)) => Op::Const(v.parse::<f32>().ok()?),
        ("load", _) => Op::Load(index()?),
        ("store", _) => Op::Store(index()?),
        ("pop", None) => Op::Pop,
        ("neg", None) => Op::Neg,
        ("not", None) => Op::Not,
        ("add", None) => Op::Add,
        ("sub", None) => Op::Sub,
        ("mul", None) => Op::Mul,
        ("div", None) => Op::Div,
        ("mod", None) => Op::Mod,
        ("pow", None) => Op::Pow,
        ("above", None) => Op::Above,
        ("below", None) => Op::Below,
        ("aboveeq", None) => Op::AboveEq,
        ("beloweq", None) => Op::BelowEq,
        ("equal", None) => Op::Equal,
        ("notequal", None) => Op::NotEqual,
        ("bitand", None) => Op::BitAnd,
        ("bitor", None) => Op::BitOr,
        ("fn1", Some(name)) => Op::Fn1(Unary::from_name(name)?),
        ("fn2", Some(name)) => Op::Fn2(Binary::from_name(name)?),
        ("memload", Some(name)) => Op::MemLoad(Mem::from_name(name)?),
        ("memstore", Some(name)) => Op::MemStore(Mem::from_name(name)?),
        ("jump", _) => Op::Jump(target()?),
        ("jz", _) => Op::JumpIfZero(target()?),
        ("jnz", _) => Op::JumpIfNotZero(target()?),
        ("loopbegin", _) => Op::LoopBegin(target()?),
        ("loopend", _) => Op::LoopEnd(target()?),
        ("whilebegin", _) => Op::WhileBegin(target()?),
        ("whileend", _) => Op::WhileEnd(target()?),
        _ => return None,
    })
}
