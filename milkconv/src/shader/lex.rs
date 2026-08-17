//! Tokenizing the MilkDrop HLSL subset.
//!
//! Line comments, block comments and the preprocessor are all resolved here, so
//! the parser sees a flat token stream. The only preprocessor form the corpus
//! uses is the **parameterless** `#define` (217 files, none with parameters, so
//! a parameterized macro is a named rejection rather than a silent mis-expansion).

use std::collections::BTreeMap;

use super::ShaderError;

/// One token. Numbers keep their source text so the emitter can normalize them
/// once, in one place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tok {
    /// An identifier or keyword.
    Ident(String),
    /// A numeric literal, verbatim (suffix still attached).
    Num(String),
    /// Punctuation — one of the fixed set below.
    P(&'static str),
}

/// Multi-character puncts first, so `<=` never lexes as `<` `=`.
const PUNCTS: &[&str] = &[
    "&&", "||", "==", "!=", "<=", ">=", "+=", "-=", "*=", "/=", "%=", "++", "--", "?", ":", ";",
    ",", "(", ")", "{", "}", "[", "]", "+", "-", "*", "/", "%", "<", ">", "=", "!", ".",
];

/// Lex a shader block: strip comments, collect `#define`s, tokenize, substitute.
pub fn lex(source: &str) -> Result<Vec<Tok>, ShaderError> {
    // One pass over both comment forms, in source order. Two separate passes
    // was a shipped bug: `ret -= 0.02;//*= 0.95;` contains the byte pair `/*`
    // *inside* a line comment, and a block-comment-first pass read it as an
    // unclosed block and swallowed the rest of the shader. Whichever opener
    // comes first owns what follows — the C rule.
    let source = strip_comments(source);

    // Directives are line-shaped, so walk lines: collect defines, resolve the
    // literal `#if` family, drop their lines.
    let mut defines: BTreeMap<String, Vec<Tok>> = BTreeMap::new();
    let mut code = String::new();
    // The `#if` stack: whether the current region is kept. Only literal
    // conditions (`#if 0`, `#if 1`) and `#ifdef`/`#ifndef` on collected defines
    // appear in the corpus; anything computed is a named rejection.
    let mut keep_stack: Vec<bool> = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("#if") {
            let rest = rest.trim();
            let keep = if let Some(name) = rest.strip_prefix("def").map(str::trim) {
                defines.contains_key(name)
            } else if let Some(name) = rest.strip_prefix("ndef").map(str::trim) {
                !defines.contains_key(name)
            } else if let Ok(value) = rest.trim().parse::<i64>() {
                value != 0
            } else {
                return Err(ShaderError::new(
                    "unsupported",
                    format!("computed preprocessor condition `#if {rest}`"),
                ));
            };
            keep_stack.push(keep);
            code.push('\n');
            continue;
        }
        if trimmed.starts_with("#else") {
            if let Some(keep) = keep_stack.last_mut() {
                *keep = !*keep;
            }
            code.push('\n');
            continue;
        }
        if trimmed.starts_with("#endif") {
            keep_stack.pop();
            code.push('\n');
            continue;
        }
        if keep_stack.iter().any(|keep| !keep) {
            code.push('\n');
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("#define") {
            let rest = rest.trim_start();
            let name: String = rest
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            if name.is_empty() {
                return Err(ShaderError::new("parse", "#define with no name"));
            }
            let body = rest.get(name.len()..).unwrap_or("");
            if body.trim_start().starts_with('(') {
                return Err(ShaderError::new(
                    "unsupported",
                    format!("parameterized macro `#define {name}(...)`"),
                ));
            }
            defines.insert(name, raw_tokens(body)?);
            code.push('\n');
            continue;
        }
        if trimmed.starts_with('#') {
            let directive: String = trimmed.chars().take(12).collect();
            return Err(ShaderError::new(
                "unsupported",
                format!("preprocessor directive `{directive}`"),
            ));
        }
        code.push_str(line);
        code.push('\n');
    }

    // Tokenize, then expand defines with a depth cap so `#define a a` cannot spin.
    let tokens = raw_tokens(&code)?;
    let mut out = Vec::with_capacity(tokens.len());
    for token in tokens {
        expand(token, &defines, 0, &mut out)?;
    }
    Ok(out)
}

fn expand(
    token: Tok,
    defines: &BTreeMap<String, Vec<Tok>>,
    depth: u32,
    out: &mut Vec<Tok>,
) -> Result<(), ShaderError> {
    if depth > 8 {
        return Err(ShaderError::new("unsupported", "recursive #define"));
    }
    if let Tok::Ident(name) = &token
        && let Some(body) = defines.get(name)
    {
        for t in body {
            expand(t.clone(), defines, depth + 1, out)?;
        }
        return Ok(());
    }
    out.push(token);
    Ok(())
}

/// Both comment forms removed in one left-to-right pass: `//` to its line end
/// (the newline survives, because `#define` is line-shaped), `/* */` to its
/// close, replaced by a space so tokens on either side stay separate. An
/// unclosed block comment swallows everything to the end, as it does in C.
fn strip_comments(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let bytes = source.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if source.get(i..).is_some_and(|r| r.starts_with("//")) {
            match source.get(i..).and_then(|r| r.find('\n')) {
                Some(nl) => i += nl,
                None => break,
            }
            continue;
        }
        if source.get(i..).is_some_and(|r| r.starts_with("/*")) {
            out.push(' ');
            match source.get(i + 2..).and_then(|r| r.find("*/")) {
                Some(close) => i += 2 + close + 2,
                None => break,
            }
            continue;
        }
        if let Some(c) = source.get(i..).and_then(|r| r.chars().next()) {
            out.push(c);
            i += c.len_utf8();
        } else {
            break;
        }
    }
    out
}

/// Tokenize text with no comments and no directives in it.
fn raw_tokens(text: &str) -> Result<Vec<Tok>, ShaderError> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0usize;
    'outer: while i < bytes.len() {
        let c = bytes.get(i).copied().unwrap_or(b' ');
        if c.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if c.is_ascii_alphabetic() || c == b'_' {
            let start = i;
            while i < bytes.len()
                && bytes
                    .get(i)
                    .is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_')
            {
                i += 1;
            }
            out.push(Tok::Ident(
                text.get(start..i).unwrap_or_default().to_string(),
            ));
            continue;
        }
        // A number: digits, or a dot followed by a digit. `.5`, `2.`, `1e-3`,
        // `1.5f`, `0x1F` all appear in the corpus.
        let next_digit = bytes.get(i + 1).is_some_and(u8::is_ascii_digit);
        if c.is_ascii_digit() || (c == b'.' && next_digit) {
            let start = i;
            if c == b'0' && bytes.get(i + 1).is_some_and(|b| *b == b'x' || *b == b'X') {
                i += 2;
                while i < bytes.len() && bytes.get(i).is_some_and(u8::is_ascii_hexdigit) {
                    i += 1;
                }
            } else {
                while i < bytes.len()
                    && bytes
                        .get(i)
                        .is_some_and(|b| b.is_ascii_digit() || *b == b'.')
                {
                    i += 1;
                }
                if bytes.get(i).is_some_and(|b| *b == b'e' || *b == b'E') {
                    let mut j = i + 1;
                    if bytes.get(j).is_some_and(|b| *b == b'+' || *b == b'-') {
                        j += 1;
                    }
                    if bytes.get(j).is_some_and(u8::is_ascii_digit) {
                        i = j;
                        while i < bytes.len() && bytes.get(i).is_some_and(u8::is_ascii_digit) {
                            i += 1;
                        }
                    }
                }
            }
            // Type suffixes are noise here: every scalar is f32.
            while bytes
                .get(i)
                .is_some_and(|b| matches!(b, b'f' | b'F' | b'h' | b'H' | b'l' | b'L' | b'u' | b'U'))
            {
                i += 1;
            }
            let raw = text.get(start..i).unwrap_or_default();
            // Suffixes come off — except on a hex literal, whose digits include
            // the same letters.
            let lexeme = if raw.starts_with("0x") || raw.starts_with("0X") {
                raw.to_string()
            } else {
                raw.trim_end_matches(['f', 'F', 'h', 'H', 'l', 'L', 'u', 'U'])
                    .to_string()
            };
            out.push(Tok::Num(lexeme));
            continue;
        }
        for p in PUNCTS {
            if text.get(i..).is_some_and(|r| r.starts_with(p)) {
                out.push(Tok::P(p));
                i += p.len();
                continue 'outer;
            }
        }
        return Err(ShaderError::new(
            "unsupported",
            format!("character `{}` in shader code", c as char),
        ));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn idents(tokens: &[Tok]) -> Vec<String> {
        tokens
            .iter()
            .filter_map(|t| match t {
                Tok::Ident(s) => Some(s.clone()),
                _ => None,
            })
            .collect()
    }

    /// A `#define` substitutes its token list wherever the name appears — the
    /// one preprocessor form the corpus uses (217 files, all parameterless).
    #[test]
    fn a_parameterless_define_substitutes() {
        let tokens = lex("#define TWO_PI 6.283\nret = TWO_PI * x;").expect("lexes");
        assert!(tokens.contains(&Tok::Num("6.283".into())));
        assert!(!idents(&tokens).contains(&"TWO_PI".to_string()));
    }

    /// A parameterized macro is a named rejection, not a silent mis-expansion.
    #[test]
    fn a_parameterized_define_is_rejected_by_name() {
        let err = lex("#define f(x) (x*2)\n").expect_err("rejects");
        assert_eq!(err.class, "unsupported");
    }

    /// Comments end where the reference says: `//` at the line, `/* */` anywhere,
    /// including spanning lines.
    #[test]
    fn comments_are_stripped_before_tokens() {
        let tokens = lex("a = 1; // b = 2;\nc /* mid\nline */ = 3;").expect("lexes");
        let names = idents(&tokens);
        assert_eq!(names, vec!["a", "c"]);
    }

    /// The number shapes the corpus actually writes: leading dot, trailing dot,
    /// exponent, `f` suffix, hex.
    #[test]
    fn number_shapes_lex_whole() {
        let tokens = lex(".5 2. 1e-3 1.5f 0x1F").expect("lexes");
        assert_eq!(
            tokens,
            vec![
                Tok::Num(".5".into()),
                Tok::Num("2.".into()),
                Tok::Num("1e-3".into()),
                Tok::Num("1.5".into()),
                Tok::Num("0x1F".into()),
            ]
        );
    }
}
