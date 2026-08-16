//! EEL2 → bytecode: the compiler half of Plan 0100's seam.
//!
//! **This never ships.** It parses the imperative program text a `.milk` file
//! carries and emits the [`EelProgram`] `lmv-core`'s VM executes, ahead of time
//! ([ADR-0113](../../docs/adrs/0113-milkdrop-presets-are-translated-ahead-of-time-onto-a-warp-mesh-idiom.md)).
//!
//! # The language, as MilkDrop uses it
//!
//! EEL2 is an expression language whose *statements* are expressions: `;`
//! sequences them and the last one's value is the program's. Everything is `f32`,
//! everything is an r-value except a bare variable and `megabuf(i)`, and
//! identifiers are **case-insensitive** — `Zoom` and `zoom` are one variable,
//! which real presets rely on.
//!
//! Precedence, loosest to tightest, matching the reference:
//!
//! ```text
//! =  +=  -=  *=  /=  %=  ^=        right-associative
//! ?:                               right-associative
//! ||
//! &&
//! |                                bitwise
//! &                                bitwise
//! ==  !=
//! <  >  <=  >=
//! +  -
//! *  /  %
//! -x  +x  !x                       unary
//! ^                                exponentiation, right-associative
//! ```
//!
//! # What the codegen guarantees
//!
//! Every construct it emits is **stack-balanced**: each branch of a conditional
//! pushes exactly one value, every statement leaves one, and a `;` pops the one
//! before it. That is what makes the linear stack walk in
//! [`EelProgram::new`](lmv_core::milk::bytecode::EelProgram) exact rather than
//! approximate, and it is why the compiler runs its own output through that
//! constructor before returning: a codegen bug is a converter failure, not a
//! preset that renders wrong.

use std::collections::HashMap;

use lmv_core::milk::MilkBundle;
use lmv_core::milk::bytecode::{Binary, EelProgram, Mem, Op, Unary};

/// Where a compile failed, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EelError {
    /// What went wrong, in one line.
    pub message: String,
    /// Byte offset into the source the compiler was reading.
    pub at: usize,
    /// Which section of the `.milk` file, once a caller supplies one.
    pub section: &'static str,
}

impl std::fmt::Display for EelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.section.is_empty() {
            write!(f, "{} (at byte {})", self.message, self.at)
        } else {
            write!(
                f,
                "{}: {} (at byte {})",
                self.section, self.message, self.at
            )
        }
    }
}

impl std::error::Error for EelError {}

impl EelError {
    fn new(message: impl Into<String>, at: usize) -> Self {
        Self {
            message: message.into(),
            at,
            section: "",
        }
    }

    /// Tag this error with the `.milk` section it came from.
    pub fn in_section(mut self, section: &'static str) -> Self {
        self.section = section;
        self
    }
}

// ---------------------------------------------------------------------------
// Lexer
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f32),
    Ident(String),
    /// Any operator or punctuator, as its literal text.
    Sym(&'static str),
}

#[derive(Debug, Clone)]
struct Token {
    tok: Tok,
    at: usize,
}

/// Every multi-character operator, **longest first** so `<=` is never read as
/// `<` followed by `=`.
const SYMBOLS: &[&str] = &[
    "+=", "-=", "*=", "/=", "%=", "^=", "==", "!=", "<=", ">=", "&&", "||", "+", "-", "*", "/",
    "%", "^", "<", ">", "=", "!", "&", "|", "?", ":", ";", ",", "(", ")",
];

/// Tokenize EEL2 source. Comments (`//` to end of line, `/* */`) and whitespace
/// are dropped; identifiers are lowercased, because the language is
/// case-insensitive and half the corpus writes `Zoom`.
fn tokenize(src: &str) -> Result<Vec<Token>, EelError> {
    let bytes = src.as_bytes();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        let c = bytes[i];
        if c.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        // Comments.
        if c == b'/' && bytes.get(i + 1) == Some(&b'/') {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if c == b'/' && bytes.get(i + 1) == Some(&b'*') {
            let start = i;
            i += 2;
            loop {
                if i + 1 >= bytes.len() {
                    return Err(EelError::new("unterminated /* comment", start));
                }
                if bytes[i] == b'*' && bytes[i + 1] == b'/' {
                    i += 2;
                    break;
                }
                i += 1;
            }
            continue;
        }
        // `$`-constants: `$pi`, `$e`, `$phi`, and `$xHEX`.
        if c == b'$' {
            let start = i;
            i += 1;
            let from = i;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            let word = src.get(from..i).unwrap_or("").to_ascii_lowercase();
            let value = match word.as_str() {
                "pi" => std::f32::consts::PI,
                "e" => std::f32::consts::E,
                "phi" => 1.618_034,
                hex if hex.starts_with('x') => {
                    let digits = hex.get(1..).unwrap_or("");
                    u32::from_str_radix(digits, 16)
                        .map(|v| v as f32)
                        .map_err(|_| EelError::new(format!("bad hex constant '${hex}'"), start))?
                }
                other => {
                    return Err(EelError::new(format!("unknown constant '${other}'"), start));
                }
            };
            out.push(Token {
                tok: Tok::Num(value),
                at: start,
            });
            continue;
        }
        // Numbers: `1`, `1.5`, `.5`, `1e-3`.
        if c.is_ascii_digit() || (c == b'.' && bytes.get(i + 1).is_some_and(u8::is_ascii_digit)) {
            let start = i;
            while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                i += 1;
            }
            if i < bytes.len() && (bytes[i] | 0x20) == b'e' {
                let save = i;
                i += 1;
                if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
                    i += 1;
                }
                if i < bytes.len() && bytes[i].is_ascii_digit() {
                    while i < bytes.len() && bytes[i].is_ascii_digit() {
                        i += 1;
                    }
                } else {
                    // Not an exponent after all — `1e` is a number then an
                    // identifier, which EEL2 would also read that way.
                    i = save;
                }
            }
            let text = src.get(start..i).unwrap_or("");
            let value = text
                .parse::<f32>()
                .map_err(|_| EelError::new(format!("bad number '{text}'"), start))?;
            out.push(Token {
                tok: Tok::Num(value),
                at: start,
            });
            continue;
        }
        // Identifiers.
        if c.is_ascii_alphabetic() || c == b'_' {
            let start = i;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            let text = src.get(start..i).unwrap_or("").to_ascii_lowercase();
            out.push(Token {
                tok: Tok::Ident(text),
                at: start,
            });
            continue;
        }
        // Operators and punctuation.
        match SYMBOLS
            .iter()
            .find(|sym| src.get(i..).is_some_and(|rest| rest.starts_with(**sym)))
        {
            Some(sym) => {
                out.push(Token {
                    tok: Tok::Sym(sym),
                    at: i,
                });
                i += sym.len();
            }
            None => {
                return Err(EelError::new(
                    format!("unexpected character '{}'", c as char),
                    i,
                ));
            }
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// The symbol table
// ---------------------------------------------------------------------------

/// The register roster a bundle's three programs share.
///
/// **One table for all three**, which is what makes `q1` written by `per_frame`
/// the same `q1` `per_vertex` reads: the shared register file *is* the bridge, so
/// the three programs must agree about which index a name is
/// ([`MilkBundle::from_assembly`] refuses one where they do not).
#[derive(Debug, Default)]
pub struct Symbols {
    names: Vec<String>,
    index: HashMap<String, u16>,
    temps: usize,
}

/// The most registers one bundle may declare.
///
/// A register index is a `u16`, so this is a real bound rather than a policy —
/// and it is two orders above what any preset in the corpus needs (the roster is
/// tens of names plus `q1`–`q32` plus a preset's own working variables).
pub const MAX_REGISTERS: usize = 4096;

impl Symbols {
    /// A fresh roster.
    pub fn new() -> Self {
        Self::default()
    }

    /// The index of `name`, interning it if it is new.
    fn intern(&mut self, name: &str, at: usize) -> Result<u16, EelError> {
        if let Some(index) = self.index.get(name) {
            return Ok(*index);
        }
        if self.names.len() >= MAX_REGISTERS {
            return Err(EelError::new(
                format!("more than {MAX_REGISTERS} distinct variables"),
                at,
            ));
        }
        let index =
            u16::try_from(self.names.len()).map_err(|_| EelError::new("too many variables", at))?;
        self.names.push(name.to_string());
        self.index.insert(name.to_string(), index);
        Ok(index)
    }

    /// A fresh hidden register for a compound `megabuf` assignment.
    ///
    /// Named with a `$` so it cannot collide with an EEL2 identifier, which the
    /// lexer never produces one of.
    fn temp(&mut self, at: usize) -> Result<u16, EelError> {
        let name = format!("$t{}", self.temps);
        self.temps += 1;
        self.intern(&name, at)
    }

    /// The roster, in index order — what every program of the bundle declares.
    pub fn names(&self) -> &[String] {
        &self.names
    }

    /// Whether `name` was referenced by any program compiled against this table.
    /// Phase 3's roster check reads this to name what a preset asked for and the
    /// engine does not supply.
    pub fn contains(&self, name: &str) -> bool {
        self.index.contains_key(name)
    }
}

// ---------------------------------------------------------------------------
// Parser + codegen (one pass, no AST)
// ---------------------------------------------------------------------------

/// A builtin's shape, resolved from its name.
enum Builtin {
    Unary(Unary),
    Binary(Binary),
    /// `if(cond, then, else)` — lazy, so it compiles to jumps.
    If,
    /// `loop(count, body)`.
    Loop,
    /// `while(body)`.
    While,
    /// `exec2(a, b)` / `exec3(a, b, c)` — evaluate all, yield the last.
    Exec(usize),
    /// `megabuf(i)` / `gmegabuf(i)` — an l-value as well as an r-value.
    Mem(Mem),
}

fn builtin(name: &str) -> Option<Builtin> {
    if let Some(f) = Unary::from_name(name) {
        return Some(Builtin::Unary(f));
    }
    if let Some(f) = Binary::from_name(name) {
        return Some(Builtin::Binary(f));
    }
    Some(match name {
        "if" => Builtin::If,
        "loop" => Builtin::Loop,
        "while" => Builtin::While,
        "exec2" => Builtin::Exec(2),
        "exec3" => Builtin::Exec(3),
        "megabuf" => Builtin::Mem(Mem::Local),
        "gmegabuf" => Builtin::Mem(Mem::Global),
        _ => return None,
    })
}

/// Compiles one program against a shared [`Symbols`], emitting bytecode as it
/// parses — there is no AST, because every construct in EEL2 is a post-order walk
/// and an intermediate tree would buy nothing.
struct Compiler<'a> {
    tokens: Vec<Token>,
    pos: usize,
    code: Vec<Op>,
    symbols: &'a mut Symbols,
    end: usize,
}

impl<'a> Compiler<'a> {
    fn peek(&self) -> Option<&Tok> {
        self.tokens.get(self.pos).map(|t| &t.tok)
    }

    fn at(&self) -> usize {
        self.tokens.get(self.pos).map_or(self.end, |t| t.at)
    }

    fn eat_sym(&mut self, sym: &str) -> bool {
        if matches!(self.peek(), Some(Tok::Sym(s)) if *s == sym) {
            self.pos += 1;
            return true;
        }
        false
    }

    fn expect_sym(&mut self, sym: &str) -> Result<(), EelError> {
        if self.eat_sym(sym) {
            return Ok(());
        }
        Err(EelError::new(format!("expected '{sym}'"), self.at()))
    }

    fn emit(&mut self, op: Op) -> usize {
        self.code.push(op);
        self.code.len() - 1
    }

    /// The index the next emitted instruction will have — a jump label.
    fn here(&self) -> u32 {
        self.code.len() as u32
    }

    /// Rewrite a previously-emitted jump's target to point at `here()`.
    fn patch(&mut self, index: usize, target: u32) {
        if let Some(op) = self.code.get_mut(index) {
            *op = match *op {
                Op::Jump(_) => Op::Jump(target),
                Op::JumpIfZero(_) => Op::JumpIfZero(target),
                Op::JumpIfNotZero(_) => Op::JumpIfNotZero(target),
                Op::LoopBegin(_) => Op::LoopBegin(target),
                Op::LoopEnd(_) => Op::LoopEnd(target),
                Op::WhileBegin(_) => Op::WhileBegin(target),
                Op::WhileEnd(_) => Op::WhileEnd(target),
                other => other,
            };
        }
    }

    /// `statement (';' statement)*` — the whole of a program, and also what a
    /// parenthesized group holds. Leaves exactly one value on the stack.
    fn sequence(&mut self, stop: Option<&str>) -> Result<(), EelError> {
        let mut emitted = false;
        loop {
            // A trailing or doubled `;` is legal and common in real presets.
            while self.eat_sym(";") {}
            let done = match stop {
                Some(sym) => matches!(self.peek(), Some(Tok::Sym(s)) if *s == sym),
                None => self.peek().is_none(),
            };
            if done {
                break;
            }
            if emitted {
                self.emit(Op::Pop);
            }
            self.assignment()?;
            emitted = true;
            if !matches!(self.peek(), Some(Tok::Sym(";"))) {
                break;
            }
        }
        if !emitted {
            // An empty program (or `()`) is the constant zero, which keeps every
            // construct stack-balanced without a special case anywhere else.
            self.emit(Op::Const(0.0));
        }
        Ok(())
    }

    /// `ternary (assign_op assignment)?`, right-associative.
    fn assignment(&mut self) -> Result<(), EelError> {
        // An assignment target is syntactically restricted, so it is recognized
        // by lookahead rather than by compiling the left side and undoing it.
        if let Some(op) = self.lookahead_assignment() {
            return self.compile_assignment(op);
        }
        self.ternary()
    }

    /// If the next tokens are `ident <assign-op>` or `megabuf ( … ) <assign-op>`,
    /// the operator; else `None`.
    fn lookahead_assignment(&self) -> Option<&'static str> {
        const OPS: [&str; 7] = ["=", "+=", "-=", "*=", "/=", "%=", "^="];
        let name = match self.tokens.get(self.pos).map(|t| &t.tok) {
            Some(Tok::Ident(name)) => name.as_str(),
            _ => return None,
        };
        // `x = …`
        if let Some(Tok::Sym(sym)) = self.tokens.get(self.pos + 1).map(|t| &t.tok)
            && OPS.contains(sym)
        {
            // A builtin is not an l-value: `sin = 1` is a syntax error, not a
            // variable called `sin`.
            return (builtin(name).is_none()).then_some(*sym);
        }
        // `megabuf(<expr>) = …` — scan to the matching close paren.
        if !matches!(name, "megabuf" | "gmegabuf") {
            return None;
        }
        if !matches!(
            self.tokens.get(self.pos + 1).map(|t| &t.tok),
            Some(Tok::Sym("("))
        ) {
            return None;
        }
        let mut depth = 0i32;
        let mut i = self.pos + 1;
        while let Some(token) = self.tokens.get(i) {
            match &token.tok {
                Tok::Sym("(") => depth += 1,
                Tok::Sym(")") => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
            i += 1;
        }
        match self.tokens.get(i + 1).map(|t| &t.tok) {
            Some(Tok::Sym(sym)) if OPS.contains(sym) => Some(*sym),
            _ => None,
        }
    }

    fn compile_assignment(&mut self, op: &'static str) -> Result<(), EelError> {
        let at = self.at();
        let Some(Tok::Ident(name)) = self.peek().cloned() else {
            return Err(EelError::new("expected an assignment target", at));
        };
        self.pos += 1;

        let mem = match name.as_str() {
            "megabuf" => Some(Mem::Local),
            "gmegabuf" => Some(Mem::Global),
            _ => None,
        };

        match mem {
            None => {
                let reg = self.symbols.intern(&name, at)?;
                self.pos += 1; // the assign op
                if op != "=" {
                    self.emit(Op::Load(reg));
                }
                self.assignment()?;
                self.compound(op);
                self.emit(Op::Store(reg));
            }
            Some(which) => {
                self.expect_sym("(")?;
                self.sequence(Some(")"))?;
                self.expect_sym(")")?;
                self.pos += 1; // the assign op
                if op == "=" {
                    // [index, value] -> MemStore
                    self.assignment()?;
                    self.emit(Op::MemStore(which));
                } else {
                    // The index is needed twice (read, then write), and there is
                    // no `dup` — so it goes through a hidden register. This is
                    // the only construct in the language that needs one.
                    let temp = self.symbols.temp(at)?;
                    self.emit(Op::Store(temp));
                    self.emit(Op::Pop);
                    self.emit(Op::Load(temp)); // the index the store will use
                    self.emit(Op::Load(temp));
                    self.emit(Op::MemLoad(which));
                    self.assignment()?;
                    self.compound(op);
                    self.emit(Op::MemStore(which));
                }
            }
        }
        Ok(())
    }

    /// The arithmetic a compound assignment applies. `=` emits nothing.
    fn compound(&mut self, op: &str) {
        match op {
            "+=" => {
                self.emit(Op::Add);
            }
            "-=" => {
                self.emit(Op::Sub);
            }
            "*=" => {
                self.emit(Op::Mul);
            }
            "/=" => {
                self.emit(Op::Div);
            }
            "%=" => {
                self.emit(Op::Mod);
            }
            "^=" => {
                self.emit(Op::Pow);
            }
            _ => {}
        }
    }

    /// `logical_or ('?' assignment ':' ternary)?`
    fn ternary(&mut self) -> Result<(), EelError> {
        self.logical_or()?;
        if !self.eat_sym("?") {
            return Ok(());
        }
        let to_else = self.emit(Op::JumpIfZero(0));
        self.assignment()?;
        let to_end = self.emit(Op::Jump(0));
        let else_at = self.here();
        self.patch(to_else, else_at);
        self.expect_sym(":")?;
        self.ternary()?;
        let end = self.here();
        self.patch(to_end, end);
        Ok(())
    }

    /// `a || b`, short-circuit, yielding exactly `1` or `0`.
    fn logical_or(&mut self) -> Result<(), EelError> {
        self.logical_and()?;
        if !matches!(self.peek(), Some(Tok::Sym("||"))) {
            return Ok(());
        }
        let mut trues = Vec::new();
        trues.push(self.emit(Op::JumpIfNotZero(0)));
        while self.eat_sym("||") {
            self.logical_and()?;
            trues.push(self.emit(Op::JumpIfNotZero(0)));
        }
        self.emit(Op::Const(0.0));
        let to_end = self.emit(Op::Jump(0));
        let true_at = self.here();
        for index in trues {
            self.patch(index, true_at);
        }
        self.emit(Op::Const(1.0));
        let end = self.here();
        self.patch(to_end, end);
        Ok(())
    }

    /// `a && b`, short-circuit.
    fn logical_and(&mut self) -> Result<(), EelError> {
        self.bit_or()?;
        if !matches!(self.peek(), Some(Tok::Sym("&&"))) {
            return Ok(());
        }
        let mut falses = Vec::new();
        falses.push(self.emit(Op::JumpIfZero(0)));
        while self.eat_sym("&&") {
            self.bit_or()?;
            falses.push(self.emit(Op::JumpIfZero(0)));
        }
        self.emit(Op::Const(1.0));
        let to_end = self.emit(Op::Jump(0));
        let false_at = self.here();
        for index in falses {
            self.patch(index, false_at);
        }
        self.emit(Op::Const(0.0));
        let end = self.here();
        self.patch(to_end, end);
        Ok(())
    }

    fn bit_or(&mut self) -> Result<(), EelError> {
        self.bit_and()?;
        while self.eat_sym("|") {
            self.bit_and()?;
            self.emit(Op::BitOr);
        }
        Ok(())
    }

    fn bit_and(&mut self) -> Result<(), EelError> {
        self.equality()?;
        while self.eat_sym("&") {
            self.equality()?;
            self.emit(Op::BitAnd);
        }
        Ok(())
    }

    fn equality(&mut self) -> Result<(), EelError> {
        self.relational()?;
        loop {
            if self.eat_sym("==") {
                self.relational()?;
                self.emit(Op::Equal);
            } else if self.eat_sym("!=") {
                self.relational()?;
                self.emit(Op::NotEqual);
            } else {
                return Ok(());
            }
        }
    }

    fn relational(&mut self) -> Result<(), EelError> {
        self.additive()?;
        loop {
            let op = if self.eat_sym("<=") {
                Op::BelowEq
            } else if self.eat_sym(">=") {
                Op::AboveEq
            } else if self.eat_sym("<") {
                Op::Below
            } else if self.eat_sym(">") {
                Op::Above
            } else {
                return Ok(());
            };
            self.additive()?;
            self.emit(op);
        }
    }

    fn additive(&mut self) -> Result<(), EelError> {
        self.multiplicative()?;
        loop {
            let op = if self.eat_sym("+") {
                Op::Add
            } else if self.eat_sym("-") {
                Op::Sub
            } else {
                return Ok(());
            };
            self.multiplicative()?;
            self.emit(op);
        }
    }

    fn multiplicative(&mut self) -> Result<(), EelError> {
        self.unary()?;
        loop {
            let op = if self.eat_sym("*") {
                Op::Mul
            } else if self.eat_sym("/") {
                Op::Div
            } else if self.eat_sym("%") {
                Op::Mod
            } else {
                return Ok(());
            };
            self.unary()?;
            self.emit(op);
        }
    }

    fn unary(&mut self) -> Result<(), EelError> {
        if self.eat_sym("-") {
            self.unary()?;
            self.emit(Op::Neg);
            return Ok(());
        }
        if self.eat_sym("+") {
            return self.unary();
        }
        if self.eat_sym("!") {
            self.unary()?;
            self.emit(Op::Not);
            return Ok(());
        }
        self.power()
    }

    /// `primary ('^' unary)?` — exponentiation binds tighter than unary minus and
    /// is right-associative, so `-2^2` is `-4` and `2^3^2` is `2^9`.
    fn power(&mut self) -> Result<(), EelError> {
        self.primary()?;
        if self.eat_sym("^") {
            self.unary()?;
            self.emit(Op::Pow);
        }
        Ok(())
    }

    fn primary(&mut self) -> Result<(), EelError> {
        let at = self.at();
        match self.peek().cloned() {
            Some(Tok::Num(v)) => {
                self.pos += 1;
                self.emit(Op::Const(v));
                Ok(())
            }
            Some(Tok::Sym("(")) => {
                self.pos += 1;
                self.sequence(Some(")"))?;
                self.expect_sym(")")
            }
            Some(Tok::Ident(name)) => {
                self.pos += 1;
                let is_call = matches!(self.peek(), Some(Tok::Sym("(")));
                match (builtin(&name), is_call) {
                    (Some(b), true) => self.call(b, &name, at),
                    // A builtin's name used as a bare variable is a real preset
                    // idiom nowhere, and treating it as one would hide a typo.
                    (Some(_), false) => Err(EelError::new(
                        format!("'{name}' is a function and needs arguments"),
                        at,
                    )),
                    (None, true) => Err(EelError::new(format!("unknown function '{name}'"), at)),
                    (None, false) => {
                        let reg = self.symbols.intern(&name, at)?;
                        self.emit(Op::Load(reg));
                        Ok(())
                    }
                }
            }
            Some(Tok::Sym(sym)) => Err(EelError::new(format!("unexpected '{sym}'"), at)),
            None => Err(EelError::new("unexpected end of program", at)),
        }
    }

    /// One argument of a call: a full statement sequence, so `if(a, (b; c), d)`
    /// and `loop(4, x = x + 1)` both parse.
    ///
    /// **A trailing `;` before the `)` or the `,` is legal**, and the corpus is
    /// full of it — a multi-line `loop` body is conventionally written with every
    /// statement terminated:
    ///
    /// ```text
    /// loop (10000,
    ///   megabuf(index) = .1;
    ///   index = index + 1;
    /// );
    /// ```
    ///
    /// Refusing that cost 84 presets in the largest pack before it was allowed,
    /// which is the second-largest single failure class the converter had.
    fn argument(&mut self) -> Result<(), EelError> {
        self.assignment()?;
        while self.eat_sym(";") {
            // A doubled `;` is an empty statement, which is legal and which the
            // corpus contains — `trig = megabuf(bbase+n); ;` inside a `loop`
            // body. Skipping the run rather than parsing an expression after each
            // one is what makes both that and the trailing form work.
            while self.eat_sym(";") {}
            if matches!(self.peek(), Some(Tok::Sym(")") | Tok::Sym(","))) {
                break;
            }
            self.emit(Op::Pop);
            self.assignment()?;
        }
        Ok(())
    }

    fn call(&mut self, b: Builtin, name: &str, at: usize) -> Result<(), EelError> {
        self.expect_sym("(")?;
        match b {
            Builtin::Unary(f) => {
                self.argument()?;
                self.expect_sym(")")?;
                self.emit(Op::Fn1(f));
            }
            Builtin::Binary(f) => {
                self.argument()?;
                self.expect_sym(",")?;
                self.argument()?;
                self.expect_sym(")")?;
                self.emit(Op::Fn2(f));
            }
            Builtin::Mem(which) => {
                self.argument()?;
                self.expect_sym(")")?;
                self.emit(Op::MemLoad(which));
            }
            Builtin::If => {
                self.argument()?;
                self.expect_sym(",")?;
                let to_else = self.emit(Op::JumpIfZero(0));
                self.argument()?;
                let to_end = self.emit(Op::Jump(0));
                let else_at = self.here();
                self.patch(to_else, else_at);
                self.expect_sym(",")?;
                self.argument()?;
                self.expect_sym(")")?;
                let end = self.here();
                self.patch(to_end, end);
            }
            Builtin::Loop => {
                self.argument()?;
                self.expect_sym(",")?;
                let begin = self.emit(Op::LoopBegin(0));
                let body = self.here();
                self.argument()?;
                self.expect_sym(")")?;
                self.emit(Op::LoopEnd(body));
                let end = self.here();
                self.patch(begin, end);
            }
            Builtin::While => {
                // The operand `WhileBegin` pops; the test is at the end, because
                // EEL2's `while(body)` runs the body first.
                self.emit(Op::Const(0.0));
                let begin = self.emit(Op::WhileBegin(0));
                let body = self.here();
                self.argument()?;
                self.expect_sym(")")?;
                self.emit(Op::WhileEnd(body));
                let end = self.here();
                self.patch(begin, end);
            }
            Builtin::Exec(n) => {
                for i in 0..n {
                    if i > 0 {
                        self.expect_sym(",")?;
                        self.emit(Op::Pop);
                    }
                    self.argument()?;
                }
                self.expect_sym(")")?;
            }
        }
        let _ = name;
        let _ = at;
        Ok(())
    }
}

/// Compile one EEL2 program against a shared symbol table, returning its
/// bytecode.
///
/// The program is **not** yet an [`EelProgram`]: the register roster is not final
/// until every section of the bundle has been compiled, because the three share
/// one table. [`compile_bundle`] is what closes it.
pub fn compile_into(src: &str, symbols: &mut Symbols) -> Result<Vec<Op>, EelError> {
    let tokens = tokenize(src)?;
    let mut compiler = Compiler {
        tokens,
        pos: 0,
        code: Vec::new(),
        symbols,
        end: src.len(),
    };
    compiler.sequence(None)?;
    if compiler.pos < compiler.tokens.len() {
        let at = compiler.at();
        return Err(EelError::new("trailing tokens", at));
    }
    Ok(compiler.code)
}

/// Compile a whole bundle's three sections against one shared register roster.
///
/// Returns the bundle and the roster, so a caller (Phase 3's converter) can check
/// which names a preset referenced against what the engine supplies — the
/// "unrecognized name is a named warning, not a silent zero" rule.
pub fn compile_bundle(
    per_frame_init: &str,
    per_frame: &str,
    per_vertex: &str,
) -> Result<(MilkBundle, Symbols), EelError> {
    let mut symbols = Symbols::new();
    let init =
        compile_into(per_frame_init, &mut symbols).map_err(|e| e.in_section("per_frame_init"))?;
    let frame = compile_into(per_frame, &mut symbols).map_err(|e| e.in_section("per_frame"))?;
    let vertex = compile_into(per_vertex, &mut symbols).map_err(|e| e.in_section("per_vertex"))?;

    // Every section declares the whole roster, which is what makes the register
    // file a bridge rather than three private files.
    let names = symbols.names().to_vec();
    let build = |code: Vec<Op>, section: &'static str| -> Result<EelProgram, EelError> {
        EelProgram::new(code, names.clone()).map_err(|err| {
            EelError::new(
                format!(
                    "the compiler emitted bytecode the engine's own validator \
                     rejects: {err}. That is a converter defect, not a preset one"
                ),
                0,
            )
            .in_section(section)
        })
    };
    let bundle = MilkBundle {
        per_frame_init: build(init, "per_frame_init")?,
        per_frame: build(frame, "per_frame")?,
        per_vertex: build(vertex, "per_vertex")?,
    };
    Ok((bundle, symbols))
}
