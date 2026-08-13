# ADR-0101 — The preset docs gain a tutorial layer rather than a merge

> **Status:** accepted (2026-08-13, at Plan 0088's close)
> **Date:** 2026-08-13
> **Related plan(s):** [0088](../plans/done/0088-the-docs-get-pictures.md)

## Context

Preset authoring is documented across three files totalling **4,819 lines**, and all three are
**reference**:

| Doc | Lines | What it is |
|---|---|---|
| [`presets/README.md`](../../presets/README.md) | 2,943 | the per-system parameter roster + the structural / palette / smoothing tables |
| [`docs/presets.md`](../presets.md) | 1,143 | the expression-grammar reference — variables, functions, operators, the error surface |
| [`docs/preset-palettes.md`](../preset-palettes.md) | 733 | the colour surface — palettes, custom stops, A/B crossfade |

Each is accurate, actively maintained, and load-bearing: the `architect` close ceremony carries a
sweep table naming these three specifically, because the `preset-author` lane deliberately keeps *no*
catalogue of its own and points at them instead — the arrangement that fixed the private-copy rot of
2026-07-26.

What none of them is, is an **entrance**. `docs/presets.md` opens with a quickstart and then becomes
a grammar reference for a thousand lines; `presets/README.md` opens with the gate contract and then
becomes tables. Someone who has never written a preset has no document that answers "what are the
nine systems, what does each one look like, and how do I get from an idea to a tuned file" — and with
Plan 0088 adding pictures, the question of *where a picture goes* forces the structural question that
was already there.

The tempting move is consolidation: one illustrated preset README instead of three references plus a
guide. That is the decision this ADR is about.

## Decision

We will add a **tutorial layer** — `docs/preset-guide.md`, the illustrated entrance, and
`docs/preset-tuning-walkthrough.md`, a worked example that tunes one preset over numbered steps
with a picture and a `--report` row per step — and
leave the three references **structurally untouched**. The guide **links into** the references and
duplicates no table from them; where a reader needs a parameter, a grammar function or a palette
name, the guide says which document owns it and links there.

The rule that follows, and the reason this is an ADR rather than a preference: **a fact lives in
exactly one of the four documents.** The references own every enumeration — parameters, functions,
palettes, structural fields. The guide owns orientation, pictures and sequence, and owns no
enumeration at all. A future session that finds itself copying a table into the guide is doing the
thing this decision exists to prevent.

## Consequences

### Positive

- **A first-time author has a door.** Nine systems, nine pictures, one "reach for this when" line
  each, then a worked tuning example — which is what the user asked for and what 4,819 lines of
  reference cannot be.
- **The close-ceremony sweep table keeps working.** Its three bolded rows still name the three
  documents that own the enumerations, so a plan that adds a scene parameter still has exactly one
  place to update and the content lane still authors against a surface that is current.
- **Every inbound link survives.** Dozens of ADRs, plans and backlog entries cite these three paths
  and their anchors; a merge would break all of them at once, and the link checker only sees the
  paths, not the anchors.
- **Pictures land where they help most and cost least.** The guide is a new file with a small,
  stable image set; illustrating a 2,943-line roster parameter-by-parameter would be an unbounded
  image budget against ADR-0100's cap.

### Negative

- **A fourth document to keep true**, and it is the one with no gate on it. The references are swept
  at every plan close by a named duty; the guide joins that table, but a duty is a human step and
  this project's own history is that untriggered duties get skipped.
- **Two documents now open with a quickstart.** `docs/presets.md`'s existing quickstart overlaps the
  guide's opening by intent. The plan resolves it by pointing that quickstart at the guide rather
  than deleting it, but the overlap is real and is the first place drift will appear.
- **The split is a convention, not a mechanism.** Nothing prevents a table being pasted into the
  guide; only this ADR and a reviewer do.

## Alternatives considered

### Alternative A — Merge the three references into one illustrated preset README

One place to look, which is genuinely what a newcomer wants. Rejected on cost and blast radius: it is
a ~4,800-line rewrite landing as one unreviewable diff, it breaks every inbound path and anchor cited
across the ADR and plan corpus, and it collapses the three-row sweep table the review ceremony and
the `preset-author` lane both depend on into one row that no longer says which surface changed. The
newcomer's problem is solved by an entrance, not by demolishing the reference.

### Alternative B — Add screenshots into the existing documents, with no new overview

The smallest possible change: illustrate `docs/presets.md` and `presets/README.md` in place, and add
only the tuning walkthrough as a new file. Rejected because it leaves the entrance being a 1,143-line
grammar reference, which is the actual complaint. It also puts images inside the two documents most
likely to be edited by an unrelated plan, where an image reference is the easiest thing to leave
pointing at a look that no longer exists.

### Alternative C — Put the tutorial in `README.md` itself

No new file, and the repository front page is where a first-time reader actually arrives. Rejected
because the README is already 313 lines covering architecture, download, controls, flags, the
pre-push gate and platform notes for *two* frontends; a preset tutorial with nine images would make
presets the dominant subject of a document that is not about presets. The README gets a hero image, a
short gallery and a link — which is the job it is good at.

## Notes

Line counts measured 2026-08-13. The four-document split follows the reference/tutorial distinction
in the Diátaxis framework; the framework is not otherwise adopted here and no other document is being
reclassified.
