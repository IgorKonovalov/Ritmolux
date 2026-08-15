// Fixture source for scripts/check-backlog-claims.mjs. Not compiled — this
// directory is outside every crate in the workspace.
//
// It exists to carry one symbol a probe can find, standing in for the real
// `sustained_miss` that falsified backlog entry 0082 ten days before that entry
// was written.

pub const SEEDED_PRESENT_SYMBOL: f32 = 1.0;
