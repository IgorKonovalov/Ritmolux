# Seeded link breaks — one of each class

Three breaks, and nothing else in this tree may add a fourth. See
`scripts/fixtures/README.md` for the expected output.

## Class 1 — inline

An inline link whose target does not exist: [the missing plan](0000-not-a-file.md).

The same line in a code span is **not** a link and must not be reported: `[a](0000-not-a-file.md)`,
because a document that describes link syntax is not making a link.

## Class 2 — definition

A link reference definition whose target does not exist:

[gone]: 0000-also-not-a-file.md

## Class 3 — a use with no definition in this file

Markdown scopes reference definitions per document, so this use renders as literal brackets even
though a sibling file defines the label: [orphan].

An ordinary bracketed phrase that nothing anywhere defines is not a lost link and must not be
reported: [just prose].
