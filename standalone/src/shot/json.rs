//! Hand-rolled JSON emission for `shot --report --json` (fixed numeric schema,
//! no serde — NFR 4 keeps the dependency count down).
//!
//! Escaping is the part that matters: preset names come from user-authored
//! `.toml`, so a name containing a quote must not be able to produce a document
//! the agent reading the report cannot parse.

/// Fixed 4-decimal number, so the schema is stable and parseable.
pub fn num(v: f32) -> String {
    format!("{v:.4}")
}

/// A row-major matrix of [`num`]-formatted values as a JSON array of arrays.
pub fn json_matrix(rows: &[Vec<f32>]) -> String {
    let mut s = String::from("[");
    for (i, row) in rows.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push('[');
        for (j, v) in row.iter().enumerate() {
            if j > 0 {
                s.push(',');
            }
            s.push_str(&num(*v));
        }
        s.push(']');
    }
    s.push(']');
    s
}

/// The transient probe's reading as a JSON object (Plan 0037): the two frame
/// counts stay **integers**, because they are counted frames rather than a
/// measurement, and the ratio uses the shared 4-decimal field.
/// The `settled` flags say whether each direction reached its asymptote inside
/// the probe's window (Plan 0038 Phase 8). A `false` means the frame count is a
/// **lower bound**, not a measurement — the text table marks the same thing with
/// a `+` suffix, and a consumer reading only the counts would otherwise have no
/// way to tell a truncated response from a settled one.
pub fn json_transient(
    rise_frames: u32,
    fall_frames: u32,
    ratio: f32,
    rise_settled: bool,
    fall_settled: bool,
) -> String {
    format!(
        "{{\"rise_frames\":{rise_frames},\"fall_frames\":{fall_frames},\"ratio\":{},\
         \"rise_settled\":{rise_settled},\"fall_settled\":{fall_settled}}}",
        num(ratio)
    )
}

/// Minimal JSON string escaping (quotes, backslash, control chars), including the
/// surrounding quotes.
pub fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The claim: no preset name can break the document. A quote, a backslash, a
    /// newline and a raw control byte all have to come back escaped, and the
    /// result must stay one balanced JSON string.
    #[test]
    fn json_string_escapes_everything_that_would_break_the_document() {
        assert_eq!(json_string("Aurora"), "\"Aurora\"");
        assert_eq!(json_string(r#"say "hi""#), r#""say \"hi\"""#);
        assert_eq!(json_string(r"back\slash"), r#""back\\slash""#);
        assert_eq!(json_string("a\nb"), r#""a\nb""#);
        assert_eq!(json_string("a\rb"), r#""a\rb""#);
        assert_eq!(json_string("a\tb"), r#""a\tb""#);
        // A raw control character is not legal inside a JSON string literal, so
        // it must come back as a \u escape rather than pass through.
        assert_eq!(json_string("a\u{0}b"), "\"a\\u0000b\"");
        assert_eq!(json_string("a\u{1f}b"), "\"a\\u001fb\"");
        assert_eq!(json_string(""), "\"\"");
        // Non-ASCII is valid UTF-8 JSON and passes through unescaped.
        assert_eq!(json_string("caf\u{e9}"), "\"caf\u{e9}\"");

        // Structural property over the whole escaping surface: apart from the two
        // delimiters, every remaining bare `"` is preceded by a backslash, and no
        // raw control character survives anywhere in the body.
        let nasty = "\"\\\n\r\t\u{7}\u{1b}quote\"end";
        let out = json_string(nasty);
        assert!(out.starts_with('"') && out.ends_with('"'));
        let body: Vec<char> = out[1..out.len() - 1].chars().collect();
        for (i, c) in body.iter().enumerate() {
            if *c == '"' {
                assert!(
                    i > 0 && body[i - 1] == '\\',
                    "unescaped quote at {i}: {out}"
                );
            }
        }
        assert!(
            !body.iter().any(|c| (*c as u32) < 0x20),
            "no raw control character survives: {out}"
        );
    }

    #[test]
    fn num_is_a_fixed_four_decimal_field() {
        assert_eq!(num(0.0), "0.0000");
        assert_eq!(num(1.0), "1.0000");
        assert_eq!(num(0.123_45), "0.1235", "rounds, not truncates");
        assert_eq!(num(-0.5), "-0.5000");
    }

    #[test]
    fn json_transient_keeps_frame_counts_integral() {
        // The historical 3/61 reading, and it is the worked example of why the
        // flags exist: that fall was truncated (the settled answer is 69), and
        // nothing in the three numbers alone could say so.
        assert_eq!(
            json_transient(3, 61, 20.333_334, true, false),
            r#"{"rise_frames":3,"fall_frames":61,"ratio":20.3333,"rise_settled":true,"fall_settled":false}"#
        );
        // A preset the step never moved reads as three zeros rather than a null
        // or a NaN, either of which would break a consumer parsing the schema.
        assert_eq!(
            json_transient(0, 0, 0.0, true, true),
            r#"{"rise_frames":0,"fall_frames":0,"ratio":0.0000,"rise_settled":true,"fall_settled":true}"#
        );
    }

    #[test]
    fn json_matrix_emits_rectangular_nested_arrays() {
        assert_eq!(json_matrix(&[]), "[]");
        assert_eq!(json_matrix(&[vec![]]), "[[]]");
        assert_eq!(
            json_matrix(&[vec![0.0, 1.0], vec![1.0, 0.0]]),
            "[[0.0000,1.0000],[1.0000,0.0000]]"
        );
    }
}
