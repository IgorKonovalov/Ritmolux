// Seeded bite check for the broken-literal class (design-backlog 0168).
//
// A Rust string literal broken across source lines without a trailing `\` keeps
// the newline AND the continuation line's indent, so the reader gets a run of
// spaces in the middle of a sentence. Rejoined onto one line afterwards, the run
// is all that is left. Both forms are seeded here.
//
// This file is NOT compiled — nothing declares it as a module, and the fixture
// tree is skipped on a repo walk. It exists to be read by the checker.

/// The already-rejoined form: one source line, the run left behind.
pub fn rejoined() -> String {
    String::from("the waveform is the tail of the analysis window and cannot be longer            than it")
}

/// The wide form a continuation indent produces, with a format placeholder in
/// front of it so it reads like the diagnostics this class actually lands in.
pub fn wide(count: usize) -> String {
    format!("only {count} of the particles moved in a frame, so a stalled cloud                      would satisfy the endpoint check trivially")
}

/// The UNREJOINED form, which is the shape an author actually types. The `\` is
/// missing, so the newline survives, the continuation indent survives, and the
/// reader gets a run of spaces mid-sentence. The gate was blind to precisely
/// this: a literal whose decoded text still held a newline returned early as a
/// formatted block, so the defect was caught only after someone joined the lines
/// — while the message printed named the shape it structurally could not see.
pub fn unrejoined() -> String {
    String::from(
        "the analysis window is the tail of the stream and this sentence lost its
                 continuation, so a run of spaces lands in the middle of it",
    )
}

/// **Not a finding, and the reason is the whole rule.** A correct `\`
/// continuation removes the newline and the next line's indent, so what the
/// reader gets is one clean sentence. Convicting this shape would convict most
/// wrapped literals in the tree.
pub fn continued() -> String {
    String::from(
        "this literal is wrapped correctly and reads as one sentence, \
         because the backslash eats the break and the indent behind it",
    )
}

/// **Not a finding.** A literal carrying a line break is a formatted block, and
/// its column spacing is layout the author typed rather than a lost escape.
pub fn table() -> String {
    String::from("seam          fog tunnel     blur mix\nA field       0.00          0.00")
}

/// **Not a finding.** Eleven spaces is under the width a continuation indent
/// produces; hand-typed alignment lives down here and is left alone.
pub fn aligned() -> String {
    String::from("note     : this machine has more than one adapter")
}
