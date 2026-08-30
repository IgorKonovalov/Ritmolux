// Fixture source for scripts/check-comment-hygiene.mjs. Not compiled — this
// directory is outside every crate in the workspace.
//
// Two seeded findings, one per class, and four silences that matter more than
// the findings do. The silences are enumerated in ../README.md.

// SEEDED, class 1 — a relative link in a Rust comment. Broken on purpose, since
// a link that resolves rots exactly the same way.
//
// [the decision]: ../../../docs/adrs/0127-not-a-real-path.md

// SILENT — a rustdoc intra-doc link. rustc resolves this, so it cannot rot
// silently, and it is the linking mechanism ADR-0127 keeps: [Seeded::render],
// and the labelled form [the renderer](crate::render::Renderer).

// SILENT — a bare-number citation, which is the form ADR-0127 replaces the links
// WITH. Plan 0045 Phase 3 argued the knee, and the plans README rosters it. A
// gate that fired on the word "plan" would convict its own fix.
//
// SILENT for the same reason, in the possessive spelling a sentence
// reaches for when the citation is the subject: the Plan 0045 Phase 4b
// defect is reachable from a bound expression.
pub const SEEDED_KNEE: f32 = 0.6;

/// SEEDED, class 2 — plan-relative narration. Once a session closes there is no
/// "this plan" to resolve against; there is only the code. Exactly one finding
/// on this line: every other rejected phrase is seeded in a file of its own, so
/// a count that moves names which form moved.
pub const SEEDED_SPAN: f32 = 1.0;

// SILENT — an escaped false positive. The word list has them, and the escape
// covers its own line and the one after it.
//
// hygiene-allow: quoting the vocabulary the gate rejects
// The tonemap value used to compute the knee is no longer read here.
pub const SEEDED_ESCAPED: f32 = 2.0;

// SILENT — a `//` inside a string is not a comment, and neither is a `"` inside
// one. This is why the checker lexes Rust rather than grepping lines.
pub fn seeded_strings() -> [&'static str; 3] {
    [
        "https://example.invalid/../not/a/link",
        r#"// the plan used to live here"#,
        "a \" quote that opens nothing // and this plan is not narration",
    ]
}
