//! A recursive-descent parser for the MilkDrop HLSL subset.
//!
//! The grammar is C expression syntax over HLSL's vector types, plus `if`,
//! `for`, `while`, plain function definitions, global declarations and the
//! `shader_body { ... }` block MilkDrop wraps the entry point in. No structs, no
//! semantics annotations, no `out` parameters — the census says the corpus has
//! none, so meeting one is a named rejection rather than a parse mystery.

use super::ShaderError;
use super::lex::Tok;

/// An expression. Type names stay strings here; the emitter owns the type
/// system.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// A numeric literal, verbatim from the lexer.
    Num(String),
    /// A bare name — a local, a shader input, or a builtin.
    Ident(String),
    /// `-x`, `!x`, `+x`.
    Unary(&'static str, Box<Expr>),
    /// A binary operator in source order.
    Binary(&'static str, Box<Expr>, Box<Expr>),
    /// `c ? t : f`.
    Ternary(Box<Expr>, Box<Expr>, Box<Expr>),
    /// A call — an intrinsic, a user function, or a type constructor
    /// (`float3(...)` is a call whose name is a type).
    Call(String, Vec<Expr>),
    /// A C-style cast, `(float3)x`.
    Cast(String, Box<Expr>),
    /// `.xyz` — a swizzle or a (rejected later) member access.
    Member(Box<Expr>, String),
    /// `a[i]`.
    Index(Box<Expr>, Box<Expr>),
}

/// A statement.
#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    /// `float3 x = e;` — one declarator (the parser expands comma lists).
    Decl {
        /// The HLSL type name, verbatim.
        ty: String,
        /// The declared name.
        name: String,
        /// The initializer, if any.
        init: Option<Expr>,
    },
    /// `target op= value;` — `op` is `None` for plain `=`.
    Assign {
        /// The left side — an ident/swizzle/index chain.
        target: Expr,
        /// The arithmetic half of a compound assignment.
        op: Option<&'static str>,
        /// The right side.
        value: Expr,
    },
    /// `if (c) { ... } else { ... }`.
    If {
        /// The condition.
        cond: Expr,
        /// The then-branch.
        then: Vec<Stmt>,
        /// The else-branch, possibly empty.
        otherwise: Vec<Stmt>,
    },
    /// `for (init; cond; update) body`.
    For {
        /// The init statements (decls or assigns).
        init: Vec<Stmt>,
        /// The condition, absent for `for(;;)`.
        cond: Option<Expr>,
        /// The update statements.
        update: Vec<Stmt>,
        /// The body.
        body: Vec<Stmt>,
    },
    /// `while (c) body`.
    While {
        /// The condition.
        cond: Expr,
        /// The body.
        body: Vec<Stmt>,
    },
    /// A bare expression statement (a call).
    Expr(Expr),
    /// `return e;` — only legal inside a user function.
    Return(Option<Expr>),
    /// `break;`
    Break,
    /// `continue;`
    Continue,
}

/// One user-defined helper function.
#[derive(Debug, Clone, PartialEq)]
pub struct Func {
    /// The declared return type name.
    pub ret: String,
    /// The function's name.
    pub name: String,
    /// `(type, name)` per parameter.
    pub params: Vec<(String, String)>,
    /// The body.
    pub body: Vec<Stmt>,
}

/// A whole shader block: what precedes `shader_body`, and the body itself.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Unit {
    /// Global declarations (become module-scope consts).
    pub globals: Vec<Stmt>,
    /// User helper functions.
    pub funcs: Vec<Func>,
    /// Sampler/texture declarations — names only; the emitter decides whether
    /// each is a builtin (a no-op) or a disk texture (a rejection).
    pub samplers: Vec<String>,
    /// The `shader_body` block.
    pub body: Vec<Stmt>,
}

/// Whether `name` is a type name this subset knows how to spell.
pub fn is_type_name(name: &str) -> bool {
    matches!(
        name,
        "float" | "int" | "uint" | "bool" | "half" | "double" | "void"
    ) || is_vector_type(name).is_some()
        || is_matrix_type(name).is_some()
}

/// `Some(n)` for `float3`, `int2`, `half4`, `bool3`, `float1`…
pub fn is_vector_type(name: &str) -> Option<u8> {
    for prefix in ["float", "half", "int", "uint", "bool", "double"] {
        if let Some(rest) = name.strip_prefix(prefix)
            && rest.len() == 1
            && let Ok(n) = rest.parse::<u8>()
            && (1..=4).contains(&n)
        {
            return Some(n);
        }
    }
    None
}

/// `Some(n)` for the square `floatNxN` family. Non-square shapes are not in the
/// corpus and are rejected by the caller with the name in hand.
pub fn is_matrix_type(name: &str) -> Option<u8> {
    for prefix in ["float", "half", "double"] {
        if let Some(rest) = name.strip_prefix(prefix)
            && rest.len() == 3
            && let Some((a, b)) = rest.split_once('x')
            && let (Ok(r), Ok(c)) = (a.parse::<u8>(), b.parse::<u8>())
            && r == c
            && (2..=4).contains(&r)
        {
            return Some(r);
        }
    }
    None
}

/// Whether `name` looks like a non-square matrix type (named in rejections).
fn is_nonsquare_matrix(name: &str) -> bool {
    for prefix in ["float", "half", "double", "int", "bool"] {
        if let Some(rest) = name.strip_prefix(prefix)
            && rest.len() == 3
            && let Some((a, b)) = rest.split_once('x')
            && a.parse::<u8>().is_ok()
            && b.parse::<u8>().is_ok()
        {
            return a != b;
        }
    }
    false
}

const SAMPLER_TYPES: &[&str] = &[
    "sampler",
    "sampler2D",
    "sampler3D",
    "Texture2D",
    "Texture3D",
];
const QUALIFIERS: &[&str] = &["static", "const", "uniform", "inline"];

/// Parse a lexed shader block.
pub fn parse(tokens: &[Tok]) -> Result<Unit, ShaderError> {
    let mut p = Parser { tokens, pos: 0 };
    let mut unit = Unit::default();

    loop {
        // Skip stray semicolons and qualifiers between items.
        while p.eat_p(";") || p.eat_qualifier() {}
        let Some(tok) = p.peek() else {
            return Err(ShaderError::new(
                "parse",
                "no `shader_body` block in the shader text",
            ));
        };
        match tok {
            Tok::Ident(name) if name == "shader_body" => {
                p.pos += 1;
                unit.body = p.block()?;
                // MilkDrop ignores anything after the body's closing brace, so
                // this does too (a handful of files carry trailing junk).
                return Ok(unit);
            }
            Tok::Ident(name) if SAMPLER_TYPES.contains(&name.as_str()) => {
                p.pos += 1;
                let sampler = p.ident()?;
                unit.samplers.push(sampler);
                // The tail — `: register(s0)`, `= sampler_state { AddressU =
                // WRAP; ... }` — is skipped whole, to the semicolon *outside*
                // any braces: a state block carries its own semicolons.
                let mut depth = 0i32;
                loop {
                    match p.peek() {
                        None => {
                            return Err(ShaderError::new(
                                "parse",
                                "unterminated sampler declaration",
                            ));
                        }
                        Some(Tok::P("{")) => depth += 1,
                        Some(Tok::P("}")) => depth -= 1,
                        Some(Tok::P(";")) if depth == 0 => {
                            p.pos += 1;
                            break;
                        }
                        _ => {}
                    }
                    p.pos += 1;
                }
            }
            Tok::Ident(name) if is_nonsquare_matrix(name) => {
                return Err(ShaderError::new(
                    "unsupported",
                    format!("non-square matrix type `{name}`"),
                ));
            }
            Tok::Ident(name) if is_type_name(name) => {
                let ty = name.clone();
                p.pos += 1;
                let ident = p.ident()?;
                if p.check_p("(") {
                    unit.funcs.push(p.function(ty, ident)?);
                } else {
                    p.declarators(ty, ident, &mut unit.globals)?;
                }
            }
            other => {
                return Err(ShaderError::new(
                    "parse",
                    format!("unexpected `{}` before shader_body", show(other)),
                ));
            }
        }
    }
}

fn show(tok: &Tok) -> String {
    match tok {
        Tok::Ident(s) => s.clone(),
        Tok::Num(s) => s.clone(),
        Tok::P(p) => (*p).to_string(),
    }
}

struct Parser<'a> {
    tokens: &'a [Tok],
    pos: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Tok> {
        self.tokens.get(self.pos)
    }

    fn peek_at(&self, ahead: usize) -> Option<&Tok> {
        self.tokens.get(self.pos + ahead)
    }

    fn check_p(&self, p: &str) -> bool {
        matches!(self.peek(), Some(Tok::P(q)) if *q == p)
    }

    fn eat_p(&mut self, p: &str) -> bool {
        if self.check_p(p) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn eat_qualifier(&mut self) -> bool {
        if let Some(Tok::Ident(name)) = self.peek()
            && QUALIFIERS.contains(&name.as_str())
        {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn expect_p(&mut self, p: &'static str) -> Result<(), ShaderError> {
        if self.eat_p(p) {
            Ok(())
        } else {
            Err(ShaderError::new(
                "parse",
                format!(
                    "expected `{p}`, found `{}`",
                    self.peek().map_or("end of shader".into(), show)
                ),
            ))
        }
    }

    fn ident(&mut self) -> Result<String, ShaderError> {
        match self.peek() {
            Some(Tok::Ident(name)) => {
                let name = name.clone();
                self.pos += 1;
                Ok(name)
            }
            other => Err(ShaderError::new(
                "parse",
                format!(
                    "expected a name, found `{}`",
                    other.map_or("end of shader".into(), show)
                ),
            )),
        }
    }

    /// A declarator's initializer: an expression, or a C brace initializer —
    /// `float2x2 rot = { a, b, c, d };` is the corpus's favourite way to build
    /// a rotation — read as the declared type's constructor. Nested braces
    /// flatten, which is what fxc did with them.
    fn initializer(&mut self, ty: &str) -> Result<Expr, ShaderError> {
        if !self.check_p("{") {
            return self.expr();
        }
        let mut args = Vec::new();
        self.brace_list(&mut args)?;
        Ok(Expr::Call(ty.to_string(), args))
    }

    fn brace_list(&mut self, out: &mut Vec<Expr>) -> Result<(), ShaderError> {
        self.expect_p("{")?;
        loop {
            if self.eat_p("}") {
                return Ok(());
            }
            if self.check_p("{") {
                self.brace_list(out)?;
            } else {
                out.push(self.expr()?);
            }
            if !self.eat_p(",") {
                self.expect_p("}")?;
                return Ok(());
            }
        }
    }

    /// One or more comma-separated declarators after `ty ident`.
    fn declarators(
        &mut self,
        ty: String,
        first: String,
        out: &mut Vec<Stmt>,
    ) -> Result<(), ShaderError> {
        let mut name = first;
        loop {
            if self.check_p("[") {
                return Err(ShaderError::new(
                    "unsupported",
                    format!("array declaration `{name}[...]`"),
                ));
            }
            let init = if self.eat_p("=") {
                Some(self.initializer(&ty)?)
            } else {
                None
            };
            out.push(Stmt::Decl {
                ty: ty.clone(),
                name,
                init,
            });
            if self.eat_p(",") {
                name = self.ident()?;
                continue;
            }
            self.expect_p(";")?;
            return Ok(());
        }
    }

    fn function(&mut self, ret: String, name: String) -> Result<Func, ShaderError> {
        self.expect_p("(")?;
        let mut params = Vec::new();
        if !self.check_p(")") {
            loop {
                while self.eat_qualifier() {}
                if let Some(Tok::Ident(q)) = self.peek()
                    && matches!(q.as_str(), "in" | "out" | "inout")
                {
                    if q != "in" {
                        return Err(ShaderError::new(
                            "unsupported",
                            format!("`{q}` parameter on function `{name}`"),
                        ));
                    }
                    self.pos += 1;
                }
                let ty = self.ident()?;
                if !is_type_name(&ty) {
                    return Err(ShaderError::new(
                        "unsupported",
                        format!("parameter type `{ty}` on function `{name}`"),
                    ));
                }
                let pname = self.ident()?;
                params.push((ty, pname));
                if !self.eat_p(",") {
                    break;
                }
            }
        }
        self.expect_p(")")?;
        let body = self.block()?;
        Ok(Func {
            ret,
            name,
            params,
            body,
        })
    }

    fn block(&mut self) -> Result<Vec<Stmt>, ShaderError> {
        self.expect_p("{")?;
        let mut out = Vec::new();
        while !self.eat_p("}") {
            if self.pos >= self.tokens.len() {
                return Err(ShaderError::new("parse", "unterminated block"));
            }
            self.stmt_into(&mut out)?;
        }
        Ok(out)
    }

    /// A statement or a `{}` block, as a body.
    fn body(&mut self) -> Result<Vec<Stmt>, ShaderError> {
        if self.check_p("{") {
            self.block()
        } else {
            let mut out = Vec::new();
            self.stmt_into(&mut out)?;
            Ok(out)
        }
    }

    fn stmt_into(&mut self, out: &mut Vec<Stmt>) -> Result<(), ShaderError> {
        if self.eat_p(";") {
            return Ok(());
        }
        if self.check_p("{") {
            out.extend(self.block()?);
            return Ok(());
        }
        if let Some(Tok::Ident(word)) = self.peek() {
            match word.as_str() {
                "if" => {
                    self.pos += 1;
                    self.expect_p("(")?;
                    let cond = self.expr()?;
                    self.expect_p(")")?;
                    let then = self.body()?;
                    let otherwise = if let Some(Tok::Ident(e)) = self.peek()
                        && e == "else"
                    {
                        self.pos += 1;
                        self.body()?
                    } else {
                        Vec::new()
                    };
                    out.push(Stmt::If {
                        cond,
                        then,
                        otherwise,
                    });
                    return Ok(());
                }
                "for" => {
                    self.pos += 1;
                    self.expect_p("(")?;
                    let mut init = Vec::new();
                    if !self.eat_p(";") {
                        loop {
                            self.simple_stmt_into(&mut init)?;
                            if !self.eat_p(",") {
                                break;
                            }
                        }
                        self.expect_p(";")?;
                    }
                    let cond = if self.check_p(";") {
                        None
                    } else {
                        Some(self.expr()?)
                    };
                    self.expect_p(";")?;
                    let mut update = Vec::new();
                    if !self.check_p(")") {
                        loop {
                            self.simple_stmt_into(&mut update)?;
                            if !self.eat_p(",") {
                                break;
                            }
                        }
                    }
                    self.expect_p(")")?;
                    let body = self.body()?;
                    out.push(Stmt::For {
                        init,
                        cond,
                        update,
                        body,
                    });
                    return Ok(());
                }
                "while" => {
                    self.pos += 1;
                    self.expect_p("(")?;
                    let cond = self.expr()?;
                    self.expect_p(")")?;
                    let body = self.body()?;
                    out.push(Stmt::While { cond, body });
                    return Ok(());
                }
                "do" => {
                    return Err(ShaderError::new("unsupported", "`do` loop"));
                }
                "return" => {
                    self.pos += 1;
                    let value = if self.check_p(";") {
                        None
                    } else {
                        Some(self.expr()?)
                    };
                    self.expect_p(";")?;
                    out.push(Stmt::Return(value));
                    return Ok(());
                }
                "break" => {
                    self.pos += 1;
                    self.expect_p(";")?;
                    out.push(Stmt::Break);
                    return Ok(());
                }
                "continue" => {
                    self.pos += 1;
                    self.expect_p(";")?;
                    out.push(Stmt::Continue);
                    return Ok(());
                }
                "discard" => {
                    return Err(ShaderError::new("unsupported", "`discard`"));
                }
                _ => {}
            }
        }
        self.simple_stmt_into(out)?;
        // The comma operator at statement level: `a = b, c = d;` is one C
        // expression statement, and the corpus writes it (most often as a
        // comma where a semicolon was meant, which fxc happily sequenced).
        while self.eat_p(",") {
            self.simple_stmt_into(out)?;
        }
        self.expect_p(";")?;
        Ok(())
    }

    /// A declaration, assignment, increment or call — the statement kinds legal
    /// in a `for` header. No trailing `;` is consumed.
    fn simple_stmt_into(&mut self, out: &mut Vec<Stmt>) -> Result<(), ShaderError> {
        while self.eat_qualifier() {}
        // A declaration: type name followed by a name that is not a call.
        if let (Some(Tok::Ident(ty)), Some(Tok::Ident(_))) = (self.peek(), self.peek_at(1))
            && is_type_name(ty)
        {
            let ty = ty.clone();
            self.pos += 1;
            let mut name = self.ident()?;
            loop {
                if self.check_p("[") {
                    return Err(ShaderError::new(
                        "unsupported",
                        format!("array declaration `{name}[...]`"),
                    ));
                }
                let init = if self.eat_p("=") {
                    Some(self.initializer(&ty)?)
                } else {
                    None
                };
                out.push(Stmt::Decl {
                    ty: ty.clone(),
                    name,
                    init,
                });
                if self.eat_p(",") {
                    name = self.ident()?;
                    continue;
                }
                return Ok(());
            }
        }
        // Prefix increment: `++i`.
        for (tok, op) in [("++", "+"), ("--", "-")] {
            if self.check_p(tok) {
                self.pos += 1;
                let target = self.postfix()?;
                out.push(Stmt::Assign {
                    target,
                    op: Some(op),
                    value: Expr::Num("1".into()),
                });
                return Ok(());
            }
        }
        let target = self.expr()?;
        // Postfix increment, compound assignment, or plain assignment.
        for (tok, op) in [("++", "+"), ("--", "-")] {
            if self.eat_p(tok) {
                out.push(Stmt::Assign {
                    target,
                    op: Some(op),
                    value: Expr::Num("1".into()),
                });
                return Ok(());
            }
        }
        for (tok, op) in [
            ("=", None),
            ("+=", Some("+")),
            ("-=", Some("-")),
            ("*=", Some("*")),
            ("/=", Some("/")),
            ("%=", Some("%")),
        ] {
            if self.eat_p(tok) {
                let mut value = self.expr()?;
                // Chained assignment, `a = b = c`: the inner assignments run
                // first, and the chain's value is the right-most operand — the
                // operands are pure, so re-evaluating it costs only ops.
                while self.eat_p("=") {
                    let rhs = self.expr()?;
                    out.push(Stmt::Assign {
                        target: value,
                        op: None,
                        value: rhs.clone(),
                    });
                    value = rhs;
                }
                out.push(Stmt::Assign { target, op, value });
                return Ok(());
            }
        }
        out.push(Stmt::Expr(target));
        Ok(())
    }

    // --- expressions, by precedence ---

    fn expr(&mut self) -> Result<Expr, ShaderError> {
        let cond = self.or()?;
        if self.eat_p("?") {
            let then = self.expr()?;
            self.expect_p(":")?;
            let otherwise = self.expr()?;
            return Ok(Expr::Ternary(
                Box::new(cond),
                Box::new(then),
                Box::new(otherwise),
            ));
        }
        Ok(cond)
    }

    fn or(&mut self) -> Result<Expr, ShaderError> {
        let mut left = self.and()?;
        while self.eat_p("||") {
            let right = self.and()?;
            left = Expr::Binary("||", Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn and(&mut self) -> Result<Expr, ShaderError> {
        let mut left = self.equality()?;
        while self.eat_p("&&") {
            let right = self.equality()?;
            left = Expr::Binary("&&", Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn equality(&mut self) -> Result<Expr, ShaderError> {
        let mut left = self.relational()?;
        loop {
            let op = if self.eat_p("==") {
                "=="
            } else if self.eat_p("!=") {
                "!="
            } else {
                return Ok(left);
            };
            let right = self.relational()?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
    }

    fn relational(&mut self) -> Result<Expr, ShaderError> {
        let mut left = self.additive()?;
        loop {
            let op = if self.eat_p("<=") {
                "<="
            } else if self.eat_p(">=") {
                ">="
            } else if self.eat_p("<") {
                "<"
            } else if self.eat_p(">") {
                ">"
            } else {
                return Ok(left);
            };
            let right = self.additive()?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
    }

    fn additive(&mut self) -> Result<Expr, ShaderError> {
        let mut left = self.multiplicative()?;
        loop {
            let op = if self.eat_p("+") {
                "+"
            } else if self.eat_p("-") {
                "-"
            } else {
                return Ok(left);
            };
            let right = self.multiplicative()?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
    }

    fn multiplicative(&mut self) -> Result<Expr, ShaderError> {
        let mut left = self.unary()?;
        loop {
            let op = if self.eat_p("*") {
                "*"
            } else if self.eat_p("/") {
                "/"
            } else if self.eat_p("%") {
                "%"
            } else {
                return Ok(left);
            };
            let right = self.unary()?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
    }

    fn unary(&mut self) -> Result<Expr, ShaderError> {
        if self.eat_p("-") {
            return Ok(Expr::Unary("-", Box::new(self.unary()?)));
        }
        if self.eat_p("!") {
            return Ok(Expr::Unary("!", Box::new(self.unary()?)));
        }
        if self.eat_p("+") {
            return self.unary();
        }
        self.postfix()
    }

    fn postfix(&mut self) -> Result<Expr, ShaderError> {
        let mut expr = self.primary()?;
        loop {
            if self.eat_p(".") {
                let member = self.ident()?;
                expr = Expr::Member(Box::new(expr), member);
            } else if self.eat_p("[") {
                let index = self.expr()?;
                self.expect_p("]")?;
                expr = Expr::Index(Box::new(expr), Box::new(index));
            } else {
                return Ok(expr);
            }
        }
    }

    fn primary(&mut self) -> Result<Expr, ShaderError> {
        if self.check_p("(") {
            // A C-style cast: `(float3)x`.
            if let (Some(Tok::Ident(ty)), Some(Tok::P(")"))) = (self.peek_at(1), self.peek_at(2))
                && is_type_name(ty)
            {
                let ty = ty.clone();
                self.pos += 3;
                let value = self.unary()?;
                return Ok(Expr::Cast(ty, Box::new(value)));
            }
            self.pos += 1;
            let mut inner = self.expr()?;
            // The C comma operator: `(q3,q3)` appears in the corpus, and with
            // side-effect-free operands its value is simply the last one.
            while self.eat_p(",") {
                inner = self.expr()?;
            }
            self.expect_p(")")?;
            return Ok(inner);
        }
        match self.peek().cloned() {
            Some(Tok::Num(text)) => {
                self.pos += 1;
                Ok(Expr::Num(text))
            }
            Some(Tok::Ident(name)) => {
                self.pos += 1;
                if self.eat_p("(") {
                    let mut args = Vec::new();
                    if !self.check_p(")") {
                        loop {
                            args.push(self.expr()?);
                            if !self.eat_p(",") {
                                break;
                            }
                        }
                    }
                    self.expect_p(")")?;
                    Ok(Expr::Call(name, args))
                } else {
                    Ok(Expr::Ident(name))
                }
            }
            other => Err(ShaderError::new(
                "parse",
                format!(
                    "expected an expression, found `{}`",
                    other.as_ref().map_or("end of shader".into(), show)
                ),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shader::lex::lex;

    fn parse_src(src: &str) -> Unit {
        parse(&lex(src).expect("lexes")).expect("parses")
    }

    /// The canonical MilkDrop block shape: declarations and helpers before
    /// `shader_body`, the entry code inside it.
    #[test]
    fn the_shader_body_shape_parses_whole() {
        let unit = parse_src(
            "float pi = 3.14159;\n\
             float3 tint(float3 c, float k) { return c * k; }\n\
             shader_body {\n\
             \x20   ret = tint(tex2D(sampler_main, uv).xyz, pi * 0.3);\n\
             }\n",
        );
        assert_eq!(unit.globals.len(), 1);
        assert_eq!(unit.funcs.len(), 1);
        assert_eq!(unit.body.len(), 1);
    }

    /// Comma declarators, compound assignment, ternaries, prefix and postfix
    /// increments — the statement shapes the corpus writes.
    #[test]
    fn the_statement_shapes_parse() {
        let unit = parse_src(
            "shader_body {\n\
             \x20 float a = 1, b;\n\
             \x20 a += b > 0 ? 2 : 3;\n\
             \x20 for (int i = 0; i < 8; i++) a *= 1.1;\n\
             \x20 while (a > 9) { a -= 1; }\n\
             }\n",
        );
        assert_eq!(unit.body.len(), 5);
    }

    /// A sampler declaration is collected by name — the emitter decides whether
    /// it is a builtin or a disk texture.
    #[test]
    fn sampler_declarations_are_collected_not_parsed_as_code() {
        let unit = parse_src("sampler sampler_noise_lq;\nshader_body { ret = uv.xyx; }\n");
        assert_eq!(unit.samplers, vec!["sampler_noise_lq"]);
    }

    /// The constructs the census says are absent stay *named* rejections.
    #[test]
    fn out_of_subset_constructs_reject_by_name() {
        for (src, class) in [
            ("shader_body { do { } while (1); }", "unsupported"),
            ("shader_body { discard; }", "unsupported"),
            ("float3x2 m;\nshader_body { }", "unsupported"),
            ("float a[4];\nshader_body { }", "unsupported"),
            ("ret = 1;", "parse"), // no shader_body at all
        ] {
            let err = parse(&lex(src).expect("lexes")).expect_err(src);
            assert_eq!(err.class, class, "{src}");
        }
    }
}
