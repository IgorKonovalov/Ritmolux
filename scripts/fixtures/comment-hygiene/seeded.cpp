// Fixture source for scripts/check-comment-hygiene.mjs. Not compiled — this
// directory is outside every build in the repository.
//
// Two seeded findings, one per class, in the dialect the foobar shim is written
// in. The shim is compiled separately from the core and drifts the same way, so
// the gate reads it through the same two classes; what differs is the lexer, and
// the silences below are the three places C and Rust disagree. Enumerated in
// ../README.md.

// SEEDED, class 1 — a relative link in a C++ comment. Broken on purpose, since
// a link that resolves rots exactly the same way.
//
// [the decision]: ../../../docs/adrs/0127-not-a-real-path.md

// SEEDED, class 2 — plan-relative narration, in the spelling the shim reaches
// for: `visible` used to be a latch driven by window messages alone.
static const float kSeededSpan = 1.0f;

/* SILENT — a C block comment does NOT nest. The /* below is an ordinary two
   characters inside a comment, and the first */
/* close ends it. A lexer running Rust's nesting rules here would swallow the
   rest of the file and report nothing at all, which is the failure mode this
   case exists to catch. */
static const float kSilentBlock = 2.0f;

// SILENT — a bare-number citation, which is the form ADR-0127 replaces the
// links WITH. Plan 0110 added the roster call and ADR-0117 argued its shape.
static const int kSilentCitation = 3;

// SILENT — a `//` inside a string is not a comment, in both of C++'s spellings.
// The raw form takes a caller-chosen delimiter rather than Rust's hash count,
// which is the second place the two lexers diverge.
static const char* kSilentStrings[] = {
    "https://example.invalid/../not/a/link",
    R"tag(// the plan used to live here)tag",
    "a \" quote that opens nothing // and this plan is not narration",
};

// SILENT — a char literal. C has no lifetimes, so every `'` opens one; Rust's
// `'a` does not, and that is the third divergence.
static const char kSilentChar = '"';
static const char kSilentEscape = '\'';

// SILENT — an escaped false positive, same escape and same two-line reach as
// the Rust side.
//
// hygiene-allow: quoting the vocabulary the gate rejects
// The handle used to be freed here and is no longer.
static const float kSilentEscaped = 4.0f;
