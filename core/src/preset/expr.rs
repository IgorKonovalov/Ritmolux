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
//! Variables: `bass mid treb onset beat bar time`. Constants: `pi tau`.
//! Functions: `sin cos abs floor sqrt min max pow mod clamp lerp smoothstep
//! select`. Compilation is fallible (a malformed expression is
//! rejected with a surfaced error, never a panic); evaluation of a compiled
//! expression is total, panic-free, and allocation-free — it walks a prebuilt
//! AST returning `f32`, so it is safe to call every frame (hot-path §5).

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
pub const VAR_NAMES: [&str; 7] = ["bass", "mid", "treb", "onset", "beat", "bar", "time"];
/// Number of expression variables.
pub const VAR_COUNT: usize = VAR_NAMES.len();

/// A bound set of variable values for one evaluation. Field order matches
/// [`VAR_NAMES`]; `beat` is the caller's bool coerced to 0.0/1.0.
#[derive(Debug, Clone, Copy, Default)]
pub struct Variables {
    values: [f32; VAR_COUNT],
}

impl Variables {
    /// Bind all seven variables (order matches [`VAR_NAMES`]).
    #[allow(clippy::too_many_arguments)]
    pub fn new(bass: f32, mid: f32, treb: f32, onset: f32, beat: f32, bar: f32, time: f32) -> Self {
        Self {
            values: [bass, mid, treb, onset, beat, bar, time],
        }
    }

    /// Value in `slot` (0.0 for an out-of-range slot — never panics; compiled
    /// expressions only ever produce valid slots).
    fn get(&self, slot: usize) -> f32 {
        self.values.get(slot).copied().unwrap_or(0.0)
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
    Min,
    Max,
    Pow,
    Mod,
    Clamp,
    Lerp,
    Smoothstep,
    Select,
}

impl Func {
    fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "sin" => Func::Sin,
            "cos" => Func::Cos,
            "abs" => Func::Abs,
            "floor" => Func::Floor,
            "sqrt" => Func::Sqrt,
            "min" => Func::Min,
            "max" => Func::Max,
            "pow" => Func::Pow,
            "mod" => Func::Mod,
            "clamp" => Func::Clamp,
            "lerp" => Func::Lerp,
            "smoothstep" => Func::Smoothstep,
            "select" => Func::Select,
            _ => return None,
        })
    }

    fn arity(self) -> usize {
        match self {
            Func::Sin | Func::Cos | Func::Abs | Func::Floor | Func::Sqrt => 1,
            Func::Min | Func::Max | Func::Pow | Func::Mod => 2,
            Func::Clamp | Func::Lerp | Func::Smoothstep | Func::Select => 3,
        }
    }
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
    fn eval(&self, vars: &Variables) -> f32 {
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
                _ => 0.0,
            },
        }
    }
}

/// A compiled expression: parse once, [`eval`](Expr::eval) every frame.
#[derive(Debug)]
pub struct Expr {
    root: Node,
}

impl Expr {
    /// Evaluate against a variable binding. Total and allocation-free.
    pub fn eval(&self, vars: &Variables) -> f32 {
        self.root.eval(vars)
    }
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
    Ok(Expr { root })
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
