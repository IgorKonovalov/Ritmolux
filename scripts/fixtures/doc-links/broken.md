# Seeded link breaks — one of each class

Five breaks, and nothing else in this tree may add a sixth. See
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

## Class 4 — a backlog reference carrying a fragment

The target **exists**, which is the entire reason this class needed its own rule: for 87 references
in the repository the gate reported them clean while every one landed at the top of a 280 KB
document instead of at the entry. [backlog 0001](design-backlog.md#0001--a-heading-so-the-form-being-retired-has-something-to-have-aimed-at)
is the retired form (ADR-0149).

The same reference in the form the ADR asks for resolves and must **not** be reported:
[backlog 0001](design-backlog.md).

A document that *describes* the retired form is not using it, so this must stay silent too:
`[backlog 0001](design-backlog.md#0001--a-heading)`.

An anchor into any other file is still unchecked — this is one prohibited form, not a fragment
checker: [a fragment nothing validates](exists.md#no-such-heading).

Seeded in **both** of markdown's link forms, because covering one of the two while reporting OK
over the other is this gate's own founding defect (Plan 0084, 85 broken links of the form it did
not read). The definition below resolves as a file and is a break only on its fragment:

[anchored definition]: design-backlog.md#0001--a-heading-so-the-form-being-retired-has-something-to-have-aimed-at
