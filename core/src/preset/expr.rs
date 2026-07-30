//! A tiny pure expression language over the audio-analysis variables, compiled
//! once at preset load and evaluated per parameter per frame.
//!
//! Grammar (recursive descent, standard precedence):
//!
//! ```text
//! expr   := sum  (('>' | '<' | '>=' | '<=' | '==' | '!=') sum)*
//! sum    := term  (('+' | '-') term)*
//! term   := unary (('*' | '/') unary)*
//! unary  := ('-' | '+')? primary
//! primary:= number | ident | ident '(' expr (',' expr)* ')' | '(' expr ')'
//! ```
//!
//! Comparisons sit at the lowest precedence and yield `1.0`/`0.0`, so they
//! compose with arithmetic (`0.4 + (bass > 0.2) * 0.3`) and with `select`.
//! There are no boolean operators: with clean `0/1` results, `min` is and,
//! `max` is or, and `1 - c` is not.
//!
//! Variables: `bass mid treb onset beat bar time tempo novelty index`, plus the
//! absolute-level escapes `bass_raw mid_raw treb_raw onset_raw` — the first four
//! are normalized against their own recent peak (ADR-0049), so a threshold on
//! them means "loud for this track" rather than naming a magnitude.
//! Constants: `pi tau`.
//! Functions: `sin cos abs floor sqrt log min max pow mod clamp lerp smoothstep
//! select bin hash noise`. Compilation is fallible (a malformed expression is
//! rejected with a surfaced error, never a panic); evaluation of a compiled
//! expression is total, panic-free, and allocation-free — it walks a prebuilt
//! AST returning `f32`, so it is safe to call every frame (hot-path §5).
//!
//! `bin(x)` is the one function that reads something other than its arguments:
//! it samples the analysis frame's log-spaced spectrum, which [`Variables`]
//! carries **by borrow** (ADR-0036). The language stays scalar-only — there is
//! no array type and no indexing syntax; the band array is reachable only
//! through this call, at a normalized position, interpolated.
//!
//! `hash(x)` and `noise(x)` are the grammar's only randomness (ADR-0051), and
//! they are random the way a shader is: pure functions of `(argument, salt)`,
//! where the salt is a per-preset constant [`Variables`] carries. Nothing here
//! reads a clock or draws from an RNG — two evaluations of the same argument
//! under the same salt are bit-identical, which is exactly what NFR §6 asks of
//! visual randomness. Who supplies the salt is the preset's business (see
//! [`schema::Preset`](super::schema::Preset)); this module only mixes it in.

// Hot-path panic-denial pragma: `eval` runs per parameter per frame. This file
// is a named target in the hygiene guard's scan (tests/hygiene.rs), so the
// pragma is enforced here even though the rest of preset/ is load-time only.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use std::fmt;

/// The analysis variables an expression may reference, in slot order.
///
/// The first nine are the analysis frame's headline values, `bass` through
/// `novelty`. The four `*_raw` names after them are the absolute magnitudes the
/// first four used to carry before ADR-0049 normalized them — reachable for
/// looks that genuinely want absolute level rather than "loud for this track".
/// `index` stays **last** and is different in kind: it is not audio but the
/// *element's own position* during a per-element evaluation (Plan 0034 Phase 4),
/// and it reads `0` anywhere else.
pub const VAR_NAMES: [&str; 14] = [
    "bass",
    "mid",
    "treb",
    "onset",
    "beat",
    "bar",
    "time",
    "tempo",
    "novelty",
    "bass_raw",
    "mid_raw",
    "treb_raw",
    "onset_raw",
    "index",
];
/// Number of expression variables.
pub const VAR_COUNT: usize = VAR_NAMES.len();

/// Slot of the implicit per-element `index` variable — kept last, so it stays
/// derivable from the count however many analysis variables precede it.
const INDEX_SLOT: usize = VAR_COUNT - 1;

/// Slot of `bass_raw`, the first of the four raw levels, which occupy
/// `RAW_SLOT_BASE..RAW_SLOT_BASE + 4` in [`VAR_NAMES`] order.
///
/// A named base rather than four literals threaded through
/// [`with_raw`](Variables::with_raw), and `raw_slots_are_where_the_names_say`
/// asserts the four names really do live here — so reordering `VAR_NAMES` fails
/// a test instead of silently binding `treb_raw` to `onset_raw`. That is the
/// same "two sources that agree today and nothing ties them" failure Plan 0041's
/// review found in the old duplicated construction sites.
const RAW_SLOT_BASE: usize = 9;

/// A bound set of variable values for one evaluation. Field order matches
/// [`VAR_NAMES`]; `beat` is the caller's bool coerced to 0.0/1.0.
///
/// The spectrum is held **by borrow**, not by value (ADR-0036): the analysis
/// frame's 64 bands are 256 bytes, and this bundle is built once per frame but
/// read once per *binding*. A by-value payload would put that memcpy on the
/// per-binding path; a slice reference keeps the whole struct at nine floats
/// plus a fat pointer, so it stays cheaply `Copy`.
#[derive(Debug, Clone, Copy, Default)]
pub struct Variables<'a> {
    values: [f32; VAR_COUNT],
    /// The log-spaced band array `bin(x)` samples, borrowed from the analysis
    /// frame. Empty when no caller supplied one, which makes `bin` read `0`.
    spectrum: &'a [f32],
    /// The per-preset salt `hash()`/`noise()` mix into their argument
    /// (ADR-0051). A **load-time constant**, never per-frame entropy: the caller
    /// sets it once from the preset's `[generator] seed`, so two presets writing
    /// the same expression scatter differently while one preset reproduces frame
    /// to frame and run to run. `0` — the default — is a perfectly good salt,
    /// and the one every preset that declares no seed gets.
    salt: u32,
}

impl<'a> Variables<'a> {
    /// Bind all nine variables (order matches [`VAR_NAMES`]). `tempo` is the
    /// tracked BPM (`0` until the tracker warms, then ~60-200 — not a `0..1`
    /// band); `novelty` is the experimental spectral track-change transient.
    ///
    /// The spectrum starts empty; attach one with
    /// [`with_spectrum`](Self::with_spectrum).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        bass: f32,
        mid: f32,
        treb: f32,
        onset: f32,
        beat: f32,
        bar: f32,
        time: f32,
        tempo: f32,
        novelty: f32,
    ) -> Self {
        let mut values = [0.0f32; VAR_COUNT];
        // The nine headline slots. Everything after them — the four raw levels
        // and `index` — starts at 0, so an expression naming one outside the
        // caller that supplies it reads zero rather than something undefined.
        values[..9].copy_from_slice(&[bass, mid, treb, onset, beat, bar, time, tempo, novelty]);
        Self {
            values,
            spectrum: &[],
            // Unsalted until a caller says otherwise — see `with_salt`.
            salt: 0,
        }
    }

    /// Bind the four absolute levels `bass_raw`/`mid_raw`/`treb_raw`/`onset_raw`
    /// (ADR-0049), leaving everything else as it was.
    ///
    /// A builder rather than four more positional arguments on
    /// [`new`](Self::new): that constructor is already at the argument-count lint
    /// and growing it to thirteen is how a caller silently transposes two levels.
    pub fn with_raw(self, bass_raw: f32, mid_raw: f32, treb_raw: f32, onset_raw: f32) -> Self {
        let mut next = self;
        if let Some(slots) = next.values.get_mut(RAW_SLOT_BASE..RAW_SLOT_BASE + 4) {
            slots.copy_from_slice(&[bass_raw, mid_raw, treb_raw, onset_raw]);
        }
        next
    }

    /// Bind every analysis variable from `frame`, with the clock at `time`.
    ///
    /// **This is the only place the frame-to-slot mapping is written.** Both the
    /// render loop and `shot`'s reachability probe come through here, so a tenth
    /// variable or a reordered slot is a one-file change rather than two copies
    /// that happen to agree. They did agree — and nothing could have told you
    /// which one the code actually used, which is the failure this closes: a
    /// probe binding different values than the engine would report flags about
    /// an expression the renderer never evaluates (Plan 0041 review).
    ///
    /// `time` stays an argument because it is the one variable that is not on
    /// the frame — the renderer passes its own clock, the probe the hop position
    /// it synthesized.
    ///
    /// The band array rides **by borrow** (ADR-0036), so this costs exactly what
    /// [`new`](Self::new) plus [`with_spectrum`](Self::with_spectrum) cost: no
    /// copy of the spectrum, nothing allocated, safe on the per-frame path.
    pub fn from_frame(frame: &'a crate::dsp::AnalysisFrame, time: f32) -> Self {
        Self::new(
            frame.bass,
            frame.mid,
            frame.treb,
            frame.onset,
            f32::from(frame.beat),
            frame.bar,
            time,
            frame.bpm,
            frame.novelty,
        )
        .with_raw(
            frame.bass_raw,
            frame.mid_raw,
            frame.treb_raw,
            frame.onset_raw,
        )
        .with_spectrum(&frame.spectrum)
    }

    /// Rebind the per-element `index` to `t` (the element's normalized `0..1`
    /// position), returning a fresh binding — the caller evaluates once per
    /// element against these (Plan 0034 Phase 4).
    ///
    /// By value and `Copy`, so a per-element loop rebinds one float without
    /// touching the borrowed spectrum or allocating.
    pub fn with_index(self, t: f32) -> Self {
        let mut next = self;
        if let Some(slot) = next.values.get_mut(INDEX_SLOT) {
            *slot = t;
        }
        next
    }

    /// Attach the frame's log-spaced band array, which `bin(x)` samples. A
    /// borrow rather than a copy — see the type docs. Kept a separate builder so
    /// the nine-scalar constructor stays the shape every existing caller (and
    /// every test) already uses.
    pub fn with_spectrum(self, spectrum: &'a [f32]) -> Self {
        Self { spectrum, ..self }
    }

    /// Bind the per-preset salt `hash()`/`noise()` mix in (ADR-0051).
    ///
    /// Its own builder rather than a constructor argument because the salt is a
    /// fact about the **preset**, not about the analysis frame: the render loop
    /// builds one [`Variables`] per frame from the frame alone, then re-salts it
    /// per preset. That is what keeps both sides of a dissolve on their own seed
    /// while they read the same audio.
    ///
    /// By value and `Copy`, like [`with_index`](Self::with_index) — re-salting
    /// rebinds one `u32` without touching the borrowed spectrum or allocating.
    pub fn with_salt(self, salt: u32) -> Self {
        Self { salt, ..self }
    }

    /// Value in `slot` (0.0 for an out-of-range slot — never panics; compiled
    /// expressions only ever produce valid slots).
    fn get(&self, slot: usize) -> f32 {
        self.values.get(slot).copied().unwrap_or(0.0)
    }

    /// The spectrum at normalized position `x`, linearly interpolated between
    /// the two adjacent bands — so a preset addresses a frequency *region*
    /// without ever naming the engine's band count (`SPECTRUM_BINS`).
    ///
    /// **Total by construction**, because this runs per binding per frame:
    /// `x <= 0` reads the first band, `x >= 1` the last, `NaN` clamps to the
    /// first, and an absent spectrum reads `0`. No indexing, no panic path.
    fn bin(&self, x: f32) -> f32 {
        let last = match self.spectrum.len().checked_sub(1) {
            Some(last) => last,
            // No spectrum bound at all — a `bin()` in an expression evaluated
            // outside the render loop reads a flat zero rather than erroring.
            None => return 0.0,
        };
        // Same total `max().min()` as `clamp`/`smoothstep` below and for the
        // same reason: `f32::max` returns the non-NaN operand, so a NaN input
        // folds to 0.0 instead of propagating into a scene parameter.
        #[allow(clippy::manual_clamp)]
        let pos = x.max(0.0).min(1.0) * last as f32;
        let floor = pos.floor();
        let index = floor as usize;
        let a = self.spectrum.get(index).copied().unwrap_or(0.0);
        // At the top end there is no next band; `unwrap_or(a)` makes the
        // interpolation degenerate to `a` rather than reaching past the array.
        let b = self.spectrum.get(index + 1).copied().unwrap_or(a);
        a + (b - a) * (pos - floor)
    }
}

/// Built-in functions, tagged with their arity so the parser can check it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Func {
    Sin,
    Cos,
    Abs,
    Floor,
    Sqrt,
    /// `log(x)` — **natural** logarithm (Plan 0038 Phase 4). There is no
    /// `log10`; divide by `ln(10)` = `2.302585` for a decade-based one, which is
    /// what the dB idiom in `docs/presets.md` does.
    Log,
    Min,
    Max,
    Pow,
    Mod,
    Clamp,
    Lerp,
    Smoothstep,
    Select,
    /// `bin(x)` — the log-spaced spectrum at normalized position `x`
    /// (ADR-0036). The only function whose result depends on [`Variables`]
    /// rather than on its arguments alone.
    Bin,
    /// `hash(x)` — a deterministic uniform scatter of `x` into `[0, 1)`, salted
    /// per preset (ADR-0051). Discontinuous by design: adjacent arguments give
    /// unrelated results, which is what makes `hash(floor(time * 2))` a lottery
    /// rather than a ramp.
    Hash,
    /// `noise(x)` — smooth value noise of `x` in `[0, 1]`, salted per preset
    /// (ADR-0051). The continuous counterpart to [`Hash`](Func::Hash): one call
    /// replaces a sum of incommensurate sines.
    Noise,
}

impl Func {
    fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "sin" => Func::Sin,
            "cos" => Func::Cos,
            "abs" => Func::Abs,
            "floor" => Func::Floor,
            "sqrt" => Func::Sqrt,
            "log" => Func::Log,
            "min" => Func::Min,
            "max" => Func::Max,
            "pow" => Func::Pow,
            "mod" => Func::Mod,
            "clamp" => Func::Clamp,
            "lerp" => Func::Lerp,
            "smoothstep" => Func::Smoothstep,
            "select" => Func::Select,
            "bin" => Func::Bin,
            "hash" => Func::Hash,
            "noise" => Func::Noise,
            _ => return None,
        })
    }

    /// The source name — the inverse of [`Func::from_name`], for
    /// [`Node::write_source`].
    fn name(self) -> &'static str {
        match self {
            Func::Sin => "sin",
            Func::Cos => "cos",
            Func::Abs => "abs",
            Func::Floor => "floor",
            Func::Sqrt => "sqrt",
            Func::Log => "log",
            Func::Min => "min",
            Func::Max => "max",
            Func::Pow => "pow",
            Func::Mod => "mod",
            Func::Clamp => "clamp",
            Func::Lerp => "lerp",
            Func::Smoothstep => "smoothstep",
            Func::Select => "select",
            Func::Bin => "bin",
            Func::Hash => "hash",
            Func::Noise => "noise",
        }
    }

    fn arity(self) -> usize {
        match self {
            Func::Sin
            | Func::Cos
            | Func::Abs
            | Func::Floor
            | Func::Sqrt
            | Func::Log
            | Func::Bin
            | Func::Hash
            | Func::Noise => 1,
            Func::Min | Func::Max | Func::Pow | Func::Mod => 2,
            Func::Clamp | Func::Lerp | Func::Smoothstep | Func::Select => 3,
        }
    }
}

/// Integer avalanche — the mixer both seeded functions are built on. Every input
/// bit affects every output bit, so two arguments one ULP apart scatter to
/// unrelated results, which is the whole point of `hash`. Wrapping arithmetic
/// throughout: there is no overflow to panic on, debug build included.
const fn mix32(mut v: u32) -> u32 {
    v ^= v >> 16;
    v = v.wrapping_mul(0x7feb_352d);
    v ^= v >> 15;
    v = v.wrapping_mul(0x846c_a68b);
    v ^= v >> 16;
    v
}

/// A mixed `u32` as a uniform `f32` in `[0, 1)`.
///
/// The top **24** bits, not all 32, because `f32` carries a 24-bit mantissa: the
/// division is then exact and every representable result is equally likely.
/// Scaling the full 32 bits would round, and round *up* at the top end — which is
/// how a generator documented as `[0, 1)` starts handing back exactly `1.0`.
fn unit(v: u32) -> f32 {
    (v >> 8) as f32 / 16_777_216.0
}

/// The scatter both seeded functions share: fold the salt in, then avalanche.
/// The salt is mixed **before** the xor so that a small seed (`1`, `2`, `7` —
/// what an author actually types) still changes every output bit.
fn scatter(bits: u32, salt: u32) -> f32 {
    unit(mix32(bits ^ mix32(salt)))
}

/// `hash(x)` — a deterministic uniform scatter of `x` into `[0, 1)` (ADR-0051).
///
/// Total for every input, infinities and `NaN` included: a float's bit pattern is
/// always a valid `u32`, so there is no domain to guard and no branch to take.
fn hash01(x: f32, salt: u32) -> f32 {
    // `0.0` and `-0.0` are the same number carrying different bits, and an author
    // writing `hash(a - b)` should not be able to see the sign of a zero.
    let bits = if x == 0.0 { 0 } else { x.to_bits() };
    scatter(bits, salt)
}

/// `noise(x)` — smooth value noise of `x` in `[0, 1]` (ADR-0051): a hashed value
/// at each integer, eased across the cell `x` falls in.
///
/// One octave, deliberately (ADR-0051): an author wanting fBm sums calls at
/// different rates, which costs them one line and costs the engine nothing.
fn value_noise(x: f32, salt: u32) -> f32 {
    // Total by construction, the same posture as `bin` and `clamp`: a non-finite
    // argument names no cell, so it reads the midpoint instead of propagating a
    // NaN into a scene parameter. Every input keeps the documented `[0, 1]`.
    if !x.is_finite() {
        return 0.5;
    }
    let cell = x.floor();
    let frac = x - cell;
    // `as` saturates rather than wrapping, so an argument past `i32` range pins
    // to one lattice point — a flat stretch, not a wrap and not a panic.
    let i = cell as i32;
    let a = scatter(i as u32, salt);
    let b = scatter(i.wrapping_add(1) as u32, salt);
    // The same eased ramp `smoothstep` uses — zero derivative at both ends, so
    // one cell joins the next without a crease.
    let t = frac * frac * (3.0 - 2.0 * frac);
    a + (b - a) * t
}

/// Bare identifiers that resolve to a literal. Resolved before the variable
/// lookup so they cannot be shadowed; an unknown bare name still errors.
fn constant(name: &str) -> Option<f32> {
    Some(match name {
        "pi" => std::f32::consts::PI,
        "tau" => std::f32::consts::TAU,
        _ => return None,
    })
}

#[derive(Debug, Clone, Copy)]
enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Gt,
    Lt,
    Ge,
    Le,
    Eq,
    Ne,
}

impl BinOp {
    /// The source token, for [`Node::write_source`].
    fn symbol(self) -> &'static str {
        match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Gt => ">",
            BinOp::Lt => "<",
            BinOp::Ge => ">=",
            BinOp::Le => "<=",
            BinOp::Eq => "==",
            BinOp::Ne => "!=",
        }
    }

    /// Which grammar tier this operator belongs to.
    fn precedence(self) -> u8 {
        match self {
            BinOp::Gt | BinOp::Lt | BinOp::Ge | BinOp::Le | BinOp::Eq | BinOp::Ne => PREC_CMP,
            BinOp::Add | BinOp::Sub => PREC_SUM,
            BinOp::Mul | BinOp::Div => PREC_TERM,
        }
    }

    /// Whether this operator yields a gate (a clean `0.0`/`1.0`) rather than a
    /// magnitude. Stated as its own predicate rather than as
    /// `precedence() == PREC_CMP`, because [`Node::probe`] observes exactly this
    /// set (ADR-0043) and a future tier reshuffle must not silently redefine it.
    fn is_comparison(self) -> bool {
        match self {
            BinOp::Gt | BinOp::Lt | BinOp::Ge | BinOp::Le | BinOp::Eq | BinOp::Ne => true,
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div => false,
        }
    }
}

/// Compiled AST node. `Box`/`Box<[_]>` allocate once at compile; evaluation
/// only reads them.
#[derive(Debug)]
enum Node {
    Const(f32),
    Var(usize),
    Neg(Box<Node>),
    Bin(BinOp, Box<Node>, Box<Node>),
    Call(Func, Box<[Node]>),
}

impl Node {
    /// Nodes in this subtree, counting itself. Used only by the probe walk, to
    /// keep a node's index the same no matter which branch a `select()` took on
    /// this evaluation — an untaken subtree still occupies its indices.
    fn node_count(&self) -> usize {
        1 + match self {
            Node::Const(_) | Node::Var(_) => 0,
            Node::Neg(inner) => inner.node_count(),
            Node::Bin(_, l, r) => l.node_count() + r.node_count(),
            Node::Call(_, args) => args.iter().map(Node::node_count).sum(),
        }
    }

    /// Whether this subtree reads variable `slot`. Walked **once at compile**,
    /// never per frame.
    fn references(&self, slot: usize) -> bool {
        match self {
            Node::Const(_) => false,
            Node::Var(s) => *s == slot,
            Node::Neg(inner) => inner.references(slot),
            Node::Bin(_, l, r) => l.references(slot) || r.references(slot),
            Node::Call(_, args) => args.iter().any(|arg| arg.references(slot)),
        }
    }

    fn eval(&self, vars: &Variables<'_>) -> f32 {
        match self {
            Node::Const(c) => *c,
            Node::Var(slot) => vars.get(*slot),
            Node::Neg(inner) => -inner.eval(vars),
            Node::Bin(op, l, r) => {
                let a = l.eval(vars);
                let b = r.eval(vars);
                match op {
                    BinOp::Add => a + b,
                    BinOp::Sub => a - b,
                    BinOp::Mul => a * b,
                    // f32 division by zero yields inf/NaN, not a panic — fine
                    // for a display value; expressions never divide silently.
                    BinOp::Div => a / b,
                    // Comparisons yield a clean 0.0/1.0 so they compose with
                    // arithmetic. A NaN operand compares false everywhere, so
                    // the result is 0.0 (except `!=`, where NaN != NaN is true
                    // by IEEE rule) — total either way.
                    BinOp::Gt => f32::from(a > b),
                    BinOp::Lt => f32::from(a < b),
                    BinOp::Ge => f32::from(a >= b),
                    BinOp::Le => f32::from(a <= b),
                    BinOp::Eq => f32::from(a == b),
                    BinOp::Ne => f32::from(a != b),
                }
            }
            // Arity is guaranteed by the parser; slice patterns keep this
            // indexing- and panic-free, with a safe default for completeness.
            Node::Call(func, args) => match (func, args.as_ref()) {
                (Func::Sin, [x]) => x.eval(vars).sin(),
                (Func::Cos, [x]) => x.eval(vars).cos(),
                (Func::Abs, [x]) => x.eval(vars).abs(),
                (Func::Floor, [x]) => x.eval(vars).floor(),
                // Out-of-domain input yields NaN, not a panic.
                (Func::Sqrt, [x]) => x.eval(vars).sqrt(),
                // Same posture as `sqrt` rather than a new rule: mathematically
                // honest at the edges, so `log(0)` is -inf and `log(-1)` is NaN.
                // `max` and `select` are the guard idiom (see `docs/presets.md`).
                (Func::Log, [x]) => x.eval(vars).ln(),
                (Func::Min, [a, b]) => a.eval(vars).min(b.eval(vars)),
                (Func::Max, [a, b]) => a.eval(vars).max(b.eval(vars)),
                (Func::Pow, [b, e]) => b.eval(vars).powf(e.eval(vars)),
                // Floored (divisor-signed) modulo, so it wraps cleanly for
                // cyclic hue/time: mod(-0.2, 1.0) is 0.8, not -0.2. A zero
                // divisor yields NaN rather than panicking.
                (Func::Mod, [a, b]) => {
                    let a = a.eval(vars);
                    let b = b.eval(vars);
                    a - b * (a / b).floor()
                }
                // Manual clamp: std f32::clamp panics if lo > hi; max().min()
                // is total.
                (Func::Clamp, [x, lo, hi]) => x.eval(vars).max(lo.eval(vars)).min(hi.eval(vars)),
                (Func::Lerp, [a, b, t]) => {
                    let a = a.eval(vars);
                    let b = b.eval(vars);
                    a + (b - a) * t.eval(vars)
                }
                (Func::Smoothstep, [e0, e1, x]) => {
                    let e0 = e0.eval(vars);
                    let e1 = e1.eval(vars);
                    // Same total max().min() clamp as above, deliberately not
                    // f32::clamp: a degenerate e0 == e1 divides by zero, and
                    // max().min() folds the resulting +-inf/NaN into [0, 1]
                    // (f32::max returns the non-NaN operand) where `clamp`
                    // would propagate the NaN into the scene parameter.
                    #[allow(clippy::manual_clamp)]
                    let t = ((x.eval(vars) - e0) / (e1 - e0)).max(0.0).min(1.0);
                    t * t * (3.0 - 2.0 * t)
                }
                // Only the taken branch is evaluated, so the untaken one cannot
                // poison the result: `select(x >= 0, sqrt(x), 0)` is safe in a
                // way a `lerp` blend of both branches would not be.
                (Func::Select, [cond, x, y]) => {
                    if cond.eval(vars) != 0.0 {
                        x.eval(vars)
                    } else {
                        y.eval(vars)
                    }
                }
                // The one call that reads the variable bundle's non-scalar
                // payload. Total for every input (see `Variables::bin`).
                (Func::Bin, [x]) => vars.bin(x.eval(vars)),
                // The two seeded functions (ADR-0051). Like `bin` they read the
                // bundle rather than their arguments alone — but what they read
                // is a load-time constant, so the expression stays pure: same
                // argument, same salt, bit-identical result, every frame.
                (Func::Hash, [x]) => hash01(x.eval(vars), vars.salt),
                (Func::Noise, [x]) => value_noise(x.eval(vars), vars.salt),
                _ => 0.0,
            },
        }
    }

    /// Walk the subtree rooted here — whose own node index is `index` — and
    /// record what each comparison, each `select()` condition and each `clamp()`
    /// bound did under `vars`. Descends only into the branch a `select()`
    /// actually took, which is the whole point: an unreached subtree stays
    /// [`NodeObservation::Untouched`].
    ///
    /// **This records; it does not compute.** The value of a probed evaluation
    /// comes from [`Node::eval`] itself (see [`Expr::eval_probed`]), so there is
    /// no second copy of the arithmetic to drift out of step with the first —
    /// the divergence ADR-0042 names as this approach's main cost is removed by
    /// construction rather than merely tested for. The price is that comparisons,
    /// conditions and clamp arguments are evaluated twice per probed call — and a
    /// comparison nested under another one compounds it — which is free:
    /// expressions are pure and nothing but the harness calls this.
    fn probe(&self, vars: &Variables<'_>, obs: &mut Observations, index: usize) {
        match self {
            Node::Const(_) | Node::Var(_) => {}
            Node::Neg(inner) => inner.probe(vars, obs, index + 1),
            Node::Bin(op, l, r) => {
                // A comparison is a gate whether or not it sits in a `select()`
                // (ADR-0043): `reseed = "onset > 0.55"` is the idiomatic boolean
                // form and contains no `select()` at all. Arithmetic operators
                // carry no branch, so they only recurse.
                if op.is_comparison() {
                    obs.record_compare(index, self.eval(vars) != 0.0);
                }
                // Both operands are evaluated either way, so both are live.
                l.probe(vars, obs, index + 1);
                r.probe(vars, obs, index + 1 + l.node_count());
            }
            Node::Call(func, args) => match (func, args.as_ref()) {
                (Func::Select, [cond, x, y]) => {
                    let taken = cond.eval(vars) != 0.0;
                    obs.record_select(index, taken);
                    let cond_at = index + 1;
                    let x_at = cond_at + cond.node_count();
                    cond.probe(vars, obs, cond_at);
                    // Only the live branch, matching what `eval` executes.
                    if taken {
                        x.probe(vars, obs, x_at);
                    } else {
                        y.probe(vars, obs, x_at + x.node_count());
                    }
                }
                (Func::Clamp, [x, lo, hi]) => {
                    obs.record_clamp(index, x.eval(vars), hi.eval(vars));
                    let x_at = index + 1;
                    let lo_at = x_at + x.node_count();
                    x.probe(vars, obs, x_at);
                    lo.probe(vars, obs, lo_at);
                    hi.probe(vars, obs, lo_at + lo.node_count());
                }
                // Every other call evaluates all of its arguments, so all of
                // them are on the live path.
                (_, rest) => {
                    let mut at = index + 1;
                    for arg in rest {
                        arg.probe(vars, obs, at);
                        at += arg.node_count();
                    }
                }
            },
        }
    }

    /// Re-render this subtree as source text, parenthesized only where its own
    /// precedence is below `parent_prec`. Used to *name* a flagged gate: a node
    /// index tells a reader nothing, and the preset's original text is not kept
    /// past compile.
    ///
    /// Round-trips through [`compile`] (asserted in the tests) but is not
    /// character-identical to what the author wrote — whitespace and redundant
    /// parentheses are gone, and `2` prints for `2.0`.
    fn write_source(&self, out: &mut String, parent_prec: u8) {
        match self {
            Node::Const(c) => out.push_str(&c.to_string()),
            Node::Var(slot) => out.push_str(VAR_NAMES.get(*slot).copied().unwrap_or("?")),
            Node::Neg(inner) => {
                // Unary binds tighter than everything but a call, so it only
                // needs wrapping inside another unary-or-higher context.
                let wrap = parent_prec > PREC_UNARY;
                if wrap {
                    out.push('(');
                }
                out.push('-');
                inner.write_source(out, PREC_UNARY);
                if wrap {
                    out.push(')');
                }
            }
            Node::Bin(op, l, r) => {
                let prec = op.precedence();
                let wrap = prec < parent_prec;
                if wrap {
                    out.push('(');
                }
                l.write_source(out, prec);
                out.push(' ');
                out.push_str(op.symbol());
                out.push(' ');
                // The right operand of a left-associative tier needs one more
                // level, so `a - (b - c)` keeps its parentheses.
                r.write_source(out, prec + 1);
                if wrap {
                    out.push(')');
                }
            }
            Node::Call(func, args) => {
                out.push_str(func.name());
                out.push('(');
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    arg.write_source(out, PREC_CMP);
                }
                out.push(')');
            }
        }
    }

    /// This subtree as source text, at statement level.
    fn source(&self) -> String {
        let mut out = String::new();
        self.write_source(&mut out, PREC_CMP);
        out
    }
}

/// Precedence tiers for [`Node::write_source`], matching the grammar in the
/// module docs: comparisons are loosest, a call or literal binds tightest.
const PREC_CMP: u8 = 0;
const PREC_SUM: u8 = 2;
const PREC_TERM: u8 = 4;
const PREC_UNARY: u8 = 6;

/// A compiled expression: parse once, [`eval`](Expr::eval) every frame.
#[derive(Debug)]
pub struct Expr {
    root: Node,
    /// Whether the expression names `index` anywhere — decided **once at
    /// compile**, because it is a property of the source text and cannot change
    /// while the preset is loaded. This is what lets the frame loop ask "is this
    /// binding per-element?" for the price of reading a `bool`.
    uses_index: bool,
}

impl Expr {
    /// Evaluate against a variable binding. Total and allocation-free.
    pub fn eval(&self, vars: &Variables<'_>) -> f32 {
        self.root.eval(vars)
    }

    /// Evaluate exactly as [`eval`](Expr::eval) does, additionally accumulating
    /// per-node reachability into `obs` (Plan 0041 / ADR-0042).
    ///
    /// **Harness only.** Nothing on the render path calls this: it allocates
    /// (the observation arena grows on first touch) and it walks the tree twice.
    /// `eval` is untouched and remains the only thing a frame executes.
    ///
    /// Call it repeatedly with the *same* `obs` across a run of varying
    /// [`Variables`] — one `Observations` per expression. What accumulates is
    /// which way each comparison and each `select()` condition went, and how
    /// close each `clamp()` came to its upper bound;
    /// [`flag_gates`](Expr::flag_gates) reads the verdict back out.
    pub fn eval_probed(&self, vars: &Variables<'_>, obs: &mut Observations) -> f32 {
        self.root.probe(vars, obs, 0);
        // The value is `eval`'s, not a re-derivation of it.
        self.root.eval(vars)
    }

    /// The gates `obs` never saw exercised, named by their source text.
    ///
    /// Read every one as a **suspect, not a conviction**: it says the run these
    /// observations came from never drove the gate both ways, which is a
    /// property of the stimulus as much as of the preset. A gate on `tempo` is
    /// correctly one-sided under a single-BPM generator.
    ///
    /// Nodes never reached at all are silent — a `select()` buried inside a dead
    /// branch is not a second finding, it is the same one. Fix the outer gate
    /// and the inner one starts reporting.
    pub fn flag_gates(&self, obs: &Observations) -> Vec<GateFlag> {
        let mut flags = Vec::new();
        // The root is nobody's condition, so a bare `onset > 0.55` reports.
        collect_flags(&self.root, obs, 0, false, &mut flags);
        flags
    }

    /// This expression rendered back to source text (normalized whitespace and
    /// parentheses, not the author's original characters).
    pub fn source(&self) -> String {
        self.root.source()
    }

    /// Whether this expression references the per-element `index`, i.e. whether
    /// it wants to be evaluated **once per element** rather than once per frame
    /// (Plan 0034 Phase 4). Free to call — the answer was computed at compile.
    pub fn uses_index(&self) -> bool {
        self.uses_index
    }
}

/// Walk `node` (whose index is `index`) and report the gates `obs` never saw
/// exercised. Recurses into every child, including a flagged gate's own
/// branches — a dead gate can still contain a live one.
///
/// `is_select_condition` is true only when `node` is the **direct** first
/// argument of an enclosing `select()`. A one-sided comparison there is
/// suppressed, because that `select()` already reports it and in better words:
/// a gate flag names the *consequence* ("its `then` branch never ran"), which a
/// comparison flag cannot (ADR-0043). Stating the rule as tree position rather
/// than as a property of the operator is what keeps it from drifting out of step
/// with the grammar — a construct that later grows a condition child reports
/// noisily through the comparison rule until it is taught to report its own.
fn collect_flags(
    node: &Node,
    obs: &Observations,
    index: usize,
    is_select_condition: bool,
    out: &mut Vec<GateFlag>,
) {
    match (node, obs.node(index)) {
        (
            Node::Call(Func::Select, args),
            NodeObservation::Select {
                saw_true,
                saw_false,
            },
        ) if saw_true != saw_false => {
            // The condition is the text worth printing, not the whole call: it
            // is the part an author has to re-gain.
            out.push(GateFlag {
                kind: GateKind::Select { always: saw_true },
                source: args.first().map(Node::source).unwrap_or_default(),
            });
        }
        (
            Node::Bin(..),
            NodeObservation::Compare {
                saw_true,
                saw_false,
            },
        ) if saw_true != saw_false && !is_select_condition => {
            // The comparison names itself: unlike a `select()`, there is no
            // enclosing call whose branches are the interesting part.
            out.push(GateFlag {
                kind: GateKind::Compare { always: saw_true },
                source: node.source(),
            });
        }
        (
            Node::Call(Func::Clamp, _),
            NodeObservation::Clamp {
                peak_fraction_of_bound,
            },
        ) if peak_fraction_of_bound < 1.0 => {
            out.push(GateFlag {
                kind: GateKind::Clamp {
                    peak_fraction_of_bound,
                },
                source: node.source(),
            });
        }
        _ => {}
    }

    let mut at = index + 1;
    match node {
        Node::Const(_) | Node::Var(_) => {}
        Node::Neg(inner) => collect_flags(inner, obs, at, false, out),
        Node::Bin(_, l, r) => {
            collect_flags(l, obs, at, false, out);
            collect_flags(r, obs, at + l.node_count(), false, out);
        }
        Node::Call(func, args) => {
            for (i, arg) in args.iter().enumerate() {
                // Only argument 0 of a `select()` is a condition. A comparison
                // one level deeper — `select(min(tempo > 124, bass > 0.38), …)` —
                // is *not* suppressed, which is the whole point: the excusable
                // half no longer launders the inexcusable one.
                let condition = matches!(func, Func::Select) && i == 0;
                collect_flags(arg, obs, at, condition, out);
                at += arg.node_count();
            }
        }
    }
}

/// Per-AST-node reachability accumulated across a run of probed evaluations
/// (ADR-0042). One of these belongs to one [`Expr`].
///
/// Lives only in the harness path: [`Expr::eval`] neither reads nor writes it,
/// and no type a frame touches gained a field for it.
#[derive(Debug, Default, Clone)]
pub struct Observations {
    /// Indexed by the node's pre-order position in its expression's tree. The
    /// index of a node does **not** depend on which branch a `select()` took —
    /// an untaken subtree still occupies its slots — so observations from
    /// different evaluations land in the same places.
    nodes: Vec<NodeObservation>,
}

impl Observations {
    /// An empty set of observations. Grows to fit as nodes are touched.
    pub fn new() -> Self {
        Self::default()
    }

    /// What was observed at `index`; [`NodeObservation::Untouched`] for a node
    /// this run never reached.
    pub fn node(&self, index: usize) -> NodeObservation {
        self.nodes.get(index).copied().unwrap_or_default()
    }

    /// Every recorded slot, in node order.
    pub fn nodes(&self) -> &[NodeObservation] {
        &self.nodes
    }

    /// The slot for `index`, growing the arena to fit. `None` is unreachable
    /// after the resize — it is how this file stays free of a panic path.
    fn slot(&mut self, index: usize) -> Option<&mut NodeObservation> {
        if self.nodes.len() <= index {
            self.nodes.resize(index + 1, NodeObservation::Untouched);
        }
        self.nodes.get_mut(index)
    }

    fn record_select(&mut self, index: usize, taken: bool) {
        let Some(slot) = self.slot(index) else {
            return;
        };
        let (was_true, was_false) = match *slot {
            NodeObservation::Select {
                saw_true,
                saw_false,
            } => (saw_true, saw_false),
            _ => (false, false),
        };
        *slot = NodeObservation::Select {
            saw_true: was_true || taken,
            saw_false: was_false || !taken,
        };
    }

    /// Record which way a comparison operator went. Same two-valued shape as
    /// [`record_select`](Self::record_select) — deliberately, so the reporting
    /// logic reads the same verdict out of both (ADR-0043).
    fn record_compare(&mut self, index: usize, taken: bool) {
        let Some(slot) = self.slot(index) else {
            return;
        };
        let (was_true, was_false) = match *slot {
            NodeObservation::Compare {
                saw_true,
                saw_false,
            } => (saw_true, saw_false),
            _ => (false, false),
        };
        *slot = NodeObservation::Compare {
            saw_true: was_true || taken,
            saw_false: was_false || !taken,
        };
    }

    /// Record how close `value` came to the clamp's upper bound `hi`.
    ///
    /// A non-positive or non-finite bound is recorded as *reached*: "fraction of
    /// the bound" means nothing there, and a false accusation is worse than a
    /// missed one for an advisory check.
    fn record_clamp(&mut self, index: usize, value: f32, hi: f32) {
        let fraction = if hi.is_finite() && hi > 0.0 {
            value / hi
        } else {
            1.0
        };
        let Some(slot) = self.slot(index) else {
            return;
        };
        let previous = match *slot {
            NodeObservation::Clamp {
                peak_fraction_of_bound,
            } => peak_fraction_of_bound,
            _ => f32::NEG_INFINITY,
        };
        *slot = NodeObservation::Clamp {
            // `max` returns the non-NaN operand, so a NaN inner value cannot
            // poison the peak.
            peak_fraction_of_bound: previous.max(fraction),
        };
    }
}

/// What one AST node did across a run.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum NodeObservation {
    /// Never evaluated — either not a comparison/`select()`/`clamp()`, or inside
    /// a branch the run never took.
    #[default]
    Untouched,
    /// A `select()` condition: did it ever go each way?
    Select {
        /// The condition evaluated non-zero at least once.
        saw_true: bool,
        /// The condition evaluated zero at least once.
        saw_false: bool,
    },
    /// A comparison operator (`> < >= <= == !=`), wherever it sits in the tree.
    /// Same two-valued shape as [`Select`](Self::Select) — deliberately, so the
    /// reporting logic is shared (ADR-0043). A comparison that is the direct
    /// condition of a `select()` is still observed here; it is *reporting* that
    /// suppresses it, because the `select()` names it in better words.
    Compare {
        /// The comparison evaluated true at least once.
        saw_true: bool,
        /// The comparison evaluated false at least once.
        saw_false: bool,
    },
    /// A `clamp()`: the highest fraction of its upper bound the inner value
    /// reached. Below `1.0` across a whole run means the bound never bit at this
    /// stimulus — the ceiling is decorative and the parameter's real range is
    /// narrower than the preset reads.
    Clamp {
        /// Peak of `value / upper_bound` over the run.
        peak_fraction_of_bound: f32,
    },
}

/// A gate that a run never exercised, with the source text that names it.
#[derive(Debug, Clone, PartialEq)]
pub struct GateFlag {
    /// Which kind of gate, and what it did.
    pub kind: GateKind,
    /// The gate's source: a `select()`'s **condition**, a comparison's own text,
    /// or a `clamp()`'s whole call. Re-rendered from the AST (see
    /// [`Expr::source`]), so whitespace and redundant parentheses will not match
    /// the preset file character for character.
    pub source: String,
}

/// The three structural findings [`Expr::flag_gates`] reports.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GateKind {
    /// A `select()` whose condition only ever went one way — so one branch is
    /// dead and the preset renders as if the `select()` were the constant it
    /// always chose.
    Select {
        /// The side it always took: `true` means the condition never went false.
        always: bool,
    },
    /// A comparison that only ever took one value, and that no `select()` flag
    /// already names (ADR-0043). Either the whole binding is the comparison
    /// (`reseed = "onset > 0.55"` — a boolean param stuck at one value), or it
    /// is a term inside a composite condition, where it is the half a
    /// `select()` flag would have hidden behind the other.
    Compare {
        /// The value it always took: `true` means the comparison never went
        /// false.
        always: bool,
    },
    /// A `clamp()` whose inner value never approached its upper bound.
    Clamp {
        /// Peak of `value / upper_bound` over the run.
        peak_fraction_of_bound: f32,
    },
}

/// Why an expression failed to compile. Evaluation never errors.
#[derive(Debug, Clone, PartialEq)]
pub enum ExprError {
    /// A character the tokenizer does not recognize.
    UnexpectedChar(char),
    /// A numeric literal that does not parse as `f32`.
    BadNumber(String),
    /// An identifier that is neither a known variable nor function.
    UnknownIdent(String),
    /// A function called with the wrong number of arguments.
    WrongArity {
        /// Function name.
        func: String,
        /// Arity the function requires.
        expected: usize,
        /// Arity supplied.
        got: usize,
    },
    /// A token appeared where the grammar did not allow it.
    UnexpectedToken(String),
    /// The expression ended earlier than the grammar allows.
    UnexpectedEnd,
    /// Extra tokens remained after a complete expression.
    TrailingTokens,
}

impl fmt::Display for ExprError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExprError::UnexpectedChar(c) => write!(f, "unexpected character '{c}'"),
            ExprError::BadNumber(s) => write!(f, "invalid number '{s}'"),
            ExprError::UnknownIdent(s) => write!(f, "unknown variable or function '{s}'"),
            ExprError::WrongArity {
                func,
                expected,
                got,
            } => write!(f, "{func}() takes {expected} argument(s), got {got}"),
            ExprError::UnexpectedToken(s) => write!(f, "unexpected token '{s}'"),
            ExprError::UnexpectedEnd => write!(f, "unexpected end of expression"),
            ExprError::TrailingTokens => write!(f, "unexpected trailing tokens"),
        }
    }
}

impl std::error::Error for ExprError {}

/// Compile a source expression into an evaluatable [`Expr`].
pub fn compile(src: &str) -> Result<Expr, ExprError> {
    let tokens = tokenize(src)?;
    let mut parser = Parser { tokens, pos: 0 };
    let root = parser.parse_expr()?;
    if parser.pos != parser.tokens.len() {
        return Err(ExprError::TrailingTokens);
    }
    let uses_index = root.references(INDEX_SLOT);
    Ok(Expr { root, uses_index })
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Num(f32),
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    LParen,
    RParen,
    Comma,
    Gt,
    Lt,
    Ge,
    Le,
    EqEq,
    NotEq,
}

impl Token {
    fn describe(&self) -> String {
        match self {
            Token::Num(n) => n.to_string(),
            Token::Ident(s) => s.clone(),
            Token::Plus => "+".into(),
            Token::Minus => "-".into(),
            Token::Star => "*".into(),
            Token::Slash => "/".into(),
            Token::LParen => "(".into(),
            Token::RParen => ")".into(),
            Token::Comma => ",".into(),
            Token::Gt => ">".into(),
            Token::Lt => "<".into(),
            Token::Ge => ">=".into(),
            Token::Le => "<=".into(),
            Token::EqEq => "==".into(),
            Token::NotEq => "!=".into(),
        }
    }
}

/// Consume a following `=` (the second half of `>=`/`<=`/`==`/`!=`), reporting
/// whether one was there. At end of input `peek` yields `None`, so a trailing
/// bare `>` tokenizes as `Gt` instead of reading past the end.
fn eat_eq(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> bool {
    if matches!(chars.peek(), Some('=')) {
        chars.next();
        true
    } else {
        false
    }
}

fn tokenize(src: &str) -> Result<Vec<Token>, ExprError> {
    let mut tokens = Vec::new();
    let mut chars = src.chars().peekable();
    while let Some(&c) = chars.peek() {
        match c {
            c if c.is_whitespace() => {
                chars.next();
            }
            '+' => {
                chars.next();
                tokens.push(Token::Plus);
            }
            '-' => {
                chars.next();
                tokens.push(Token::Minus);
            }
            '*' => {
                chars.next();
                tokens.push(Token::Star);
            }
            '/' => {
                chars.next();
                tokens.push(Token::Slash);
            }
            '(' => {
                chars.next();
                tokens.push(Token::LParen);
            }
            ')' => {
                chars.next();
                tokens.push(Token::RParen);
            }
            ',' => {
                chars.next();
                tokens.push(Token::Comma);
            }
            // Two-char comparison forms need one char of lookahead. A trailing
            // bare `>`/`<` at end of input still tokenizes (peek yields None).
            '>' => {
                chars.next();
                let tok = if eat_eq(&mut chars) {
                    Token::Ge
                } else {
                    Token::Gt
                };
                tokens.push(tok);
            }
            '<' => {
                chars.next();
                let tok = if eat_eq(&mut chars) {
                    Token::Le
                } else {
                    Token::Lt
                };
                tokens.push(tok);
            }
            // `=` and `!` are only valid as the two-char forms; a bare one is an
            // explicit error rather than a silently-dropped character.
            '=' | '!' => {
                chars.next();
                if !eat_eq(&mut chars) {
                    return Err(ExprError::UnexpectedChar(c));
                }
                tokens.push(if c == '=' { Token::EqEq } else { Token::NotEq });
            }
            c if c.is_ascii_digit() || c == '.' => {
                let mut num = String::new();
                while let Some(&d) = chars.peek() {
                    if d.is_ascii_digit() || d == '.' {
                        num.push(d);
                        chars.next();
                    } else {
                        break;
                    }
                }
                let value: f32 = num.parse().map_err(|_| ExprError::BadNumber(num.clone()))?;
                tokens.push(Token::Num(value));
            }
            c if c.is_ascii_alphabetic() || c == '_' => {
                let mut ident = String::new();
                while let Some(&d) = chars.peek() {
                    if d.is_ascii_alphanumeric() || d == '_' {
                        ident.push(d);
                        chars.next();
                    } else {
                        break;
                    }
                }
                tokens.push(Token::Ident(ident));
            }
            other => return Err(ExprError::UnexpectedChar(other)),
        }
    }
    Ok(tokens)
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<&Token> {
        let tok = self.tokens.get(self.pos);
        if tok.is_some() {
            self.pos += 1;
        }
        tok
    }

    /// The lowest-precedence tier: comparisons over sums. Left-associative, so
    /// a chained `a > b > c` parses as `(a > b) > c` — legal but rarely
    /// intended (the docs discourage it).
    fn parse_expr(&mut self) -> Result<Node, ExprError> {
        let mut left = self.parse_sum()?;
        while let Some(op) = match self.peek() {
            Some(Token::Gt) => Some(BinOp::Gt),
            Some(Token::Lt) => Some(BinOp::Lt),
            Some(Token::Ge) => Some(BinOp::Ge),
            Some(Token::Le) => Some(BinOp::Le),
            Some(Token::EqEq) => Some(BinOp::Eq),
            Some(Token::NotEq) => Some(BinOp::Ne),
            _ => None,
        } {
            self.pos += 1;
            let right = self.parse_sum()?;
            left = Node::Bin(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_sum(&mut self) -> Result<Node, ExprError> {
        let mut left = self.parse_term()?;
        while let Some(op) = match self.peek() {
            Some(Token::Plus) => Some(BinOp::Add),
            Some(Token::Minus) => Some(BinOp::Sub),
            _ => None,
        } {
            self.pos += 1;
            let right = self.parse_term()?;
            left = Node::Bin(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_term(&mut self) -> Result<Node, ExprError> {
        let mut left = self.parse_unary()?;
        while let Some(op) = match self.peek() {
            Some(Token::Star) => Some(BinOp::Mul),
            Some(Token::Slash) => Some(BinOp::Div),
            _ => None,
        } {
            self.pos += 1;
            let right = self.parse_unary()?;
            left = Node::Bin(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Node, ExprError> {
        match self.peek() {
            Some(Token::Minus) => {
                self.pos += 1;
                Ok(Node::Neg(Box::new(self.parse_unary()?)))
            }
            Some(Token::Plus) => {
                self.pos += 1;
                self.parse_unary()
            }
            _ => self.parse_primary(),
        }
    }

    fn parse_primary(&mut self) -> Result<Node, ExprError> {
        match self.advance() {
            Some(Token::Num(n)) => Ok(Node::Const(*n)),
            Some(Token::LParen) => {
                let inner = self.parse_expr()?;
                self.expect(&Token::RParen)?;
                Ok(inner)
            }
            Some(Token::Ident(name)) => {
                let name = name.clone();
                if matches!(self.peek(), Some(Token::LParen)) {
                    self.parse_call(name)
                } else if let Some(c) = constant(&name) {
                    // Checked before the variable lookup, so a constant can
                    // never be shadowed by a future variable of the same name.
                    Ok(Node::Const(c))
                } else if let Some(slot) = VAR_NAMES.iter().position(|&v| v == name) {
                    Ok(Node::Var(slot))
                } else {
                    Err(ExprError::UnknownIdent(name))
                }
            }
            Some(other) => Err(ExprError::UnexpectedToken(other.describe())),
            None => Err(ExprError::UnexpectedEnd),
        }
    }

    fn parse_call(&mut self, name: String) -> Result<Node, ExprError> {
        let func = Func::from_name(&name).ok_or(ExprError::UnknownIdent(name.clone()))?;
        self.expect(&Token::LParen)?;
        let mut args = Vec::new();
        if !matches!(self.peek(), Some(Token::RParen)) {
            loop {
                args.push(self.parse_expr()?);
                match self.peek() {
                    Some(Token::Comma) => {
                        self.pos += 1;
                    }
                    _ => break,
                }
            }
        }
        self.expect(&Token::RParen)?;
        if args.len() != func.arity() {
            return Err(ExprError::WrongArity {
                func: name,
                expected: func.arity(),
                got: args.len(),
            });
        }
        Ok(Node::Call(func, args.into_boxed_slice()))
    }

    fn expect(&mut self, want: &Token) -> Result<(), ExprError> {
        match self.advance() {
            Some(tok) if tok == want => Ok(()),
            Some(other) => Err(ExprError::UnexpectedToken(other.describe())),
            None => Err(ExprError::UnexpectedEnd),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The slot-base constants are the one place this module trades a name for a
    /// number, so they get the assertion. Inline rather than in
    /// `core/tests/preset.rs` because both constants are private — and they
    /// should stay private, which makes this the only place the claim is
    /// checkable.
    ///
    /// Without it, inserting a variable before `bass_raw` would leave
    /// [`Variables::with_raw`] writing four floats into `novelty` and the three
    /// slots after it, quietly, with every existing test still green: the raw
    /// values would simply read as each other.
    #[test]
    fn raw_slots_are_where_the_names_say() {
        assert_eq!(
            VAR_NAMES.get(RAW_SLOT_BASE..RAW_SLOT_BASE + 4),
            Some(["bass_raw", "mid_raw", "treb_raw", "onset_raw"].as_slice()),
            "with_raw writes four floats starting at RAW_SLOT_BASE; those are the names it must land on"
        );
        assert_eq!(
            VAR_NAMES.get(INDEX_SLOT),
            Some(&"index"),
            "`index` must stay last: INDEX_SLOT is derived from the variable count"
        );
    }

    /// `with_raw` fills exactly its own four slots — it must not disturb the
    /// headline levels it sits beside, which is the failure a copy_from_slice
    /// with a wrong base would produce.
    #[test]
    fn with_raw_touches_only_the_raw_slots() {
        let base = Variables::new(0.1, 0.2, 0.3, 0.4, 1.0, 0.5, 6.0, 120.0, 0.7);
        let with = base.with_raw(0.01, 0.02, 0.03, 0.04);
        assert_eq!(
            base.values.get(..RAW_SLOT_BASE),
            with.values.get(..RAW_SLOT_BASE),
            "the nine headline slots must be untouched"
        );
        assert_eq!(
            with.values.get(RAW_SLOT_BASE..RAW_SLOT_BASE + 4),
            Some([0.01f32, 0.02, 0.03, 0.04].as_slice())
        );
        assert_eq!(
            with.values.get(INDEX_SLOT),
            Some(&0.0),
            "`index` sits after the raw block and must not be clipped by it"
        );
    }
}
