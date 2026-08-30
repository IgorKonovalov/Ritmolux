// Fixture source for scripts/check-comment-hygiene.mjs. Not compiled — this
// directory is outside every crate in the workspace.
//
// One seeded finding, and the silences that keep the form from convicting the
// citation it decorates. Enumerated in ../README.md.
//
// The form: an elapsed-time preposition in front of a numbered citation. It is
// the shape an author reaches for when cutting the first vocabulary's phrases,
// it reads like a citation, and it is narration all the same — it dates the code
// against an event, so a reader has to reconstruct a history to decode a claim
// about the present.

// SEEDED — all five prepositions, on one line each so a regex that drops one
// shows up as a count rather than as a silence. `pre-` attaches directly and the
// other four take a separator.
//
// The wide bin was pinned to 1.0 before Plan 0038 Phase 2 bound it.
// The counter has folded over since Plan 0095.
// The key was reserved and inert until Plan 0047 read it.
// The mirror was re-derived after Plan 0055 Phase 3 deleted it.
// The pre-0070 mirror is the shape this replaces.
pub const SEEDED_ELAPSED: f32 = 0.5;

// SILENT — the bare citation, which is the form ADR-0127 asks for and the form
// the seeded lines above should be rewritten INTO. A gate that fired here would
// convict its own fix, so the preposition is the whole difference:
//
// The wide bin is pinned to 1.0 (Plan 0038 Phase 2).
// `beat_count` is the counter this folds over — Plan 0095, ADR-0046.
// Deleting the mirror re-derived the second address mode. Plan 0055 Phase 3.
pub const SILENT_CITATION: f32 = 1.0;

// SILENT — a preposition with no citation behind it. `after the fold` and
// `before the blur` are ordinary mechanism sentences about pass order, and the
// pattern requires plan / adr / phase followed by a number precisely so that
// describing a pipeline stays legal.
//
// The kaleido fold runs after the warp and before the blur chain.
// Phase 2 of the tonemap reads what phase 1 wrote.
pub const SILENT_ORDERING: f32 = 1.5;

// SILENT — an escaped false positive. `phase` is a shader stage as often as it
// is a plan phase, and the escape covers its own line and the one after it.
//
// hygiene-allow: `phase 3` here is an oscillator stage, not a plan phase
// The accumulator is sampled after phase 3 of the four-tap ring.
pub const SILENT_ESCAPED: f32 = 2.0;

// SILENT — a `//` inside a string is not a comment. The lexer half is pinned by
// seeded.rs; this asserts the widened vocabulary is read through the same lexer
// rather than grepped back in alongside it.
pub fn seeded_elapsed_strings() -> [&'static str; 2] {
    [
        "// pinned to 1.0 before Plan 0038 Phase 2 bound it",
        r#"// the counter has folded over since Plan 0095"#,
    ]
}
