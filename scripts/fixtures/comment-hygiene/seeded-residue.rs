// Fixture source for scripts/check-comment-hygiene.mjs. Not compiled — this
// directory is outside every crate in the workspace.
//
// One seeded finding: the residue phrase. Enumerated in ../README.md.
//
// It is the trailing half of the first vocabulary's negation family and survives
// a rewrite of it, because an author cutting those phrases reaches for this one
// as the synonym. Same defect: it asserts a change against an unnamed earlier
// state, so the sentence decodes only for a reader who knows what that was.

// SEEDED — the phrase, in the position it actually shows up in: a sentence
// explaining why something is absent.
//
/// The scene does not resolve its own extent any more; the composite hands one
/// down.
pub const SEEDED_RESIDUE: f32 = 0.25;

// SILENT — the same fact stated as a property of the code, which is the rewrite
// this form asks for: no earlier state to reconstruct, and nothing that expires.
//
/// The extent is handed down by the composite, not resolved here.
pub const SILENT_PROPERTY: f32 = 0.5;

// SILENT — `any` and `more` adjacent across a sentence boundary, which is not
// the phrase. A word-boundary pattern that dropped the space would fire here.
//
/// Rejects any input with more stops than the ramp has slots.
pub const SILENT_ADJACENT: f32 = 0.75;
