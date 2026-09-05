# 0155 — The reader documents stop explaining themselves

> **Status:** draft
> **Created:** 2026-09-05
> **Owner skill(s):** dev
> **Related ADRs:** [0168](../adrs/0168-the-reader-documents-address-a-reader-and-the-record-stays-a-link.md)
> **Hard dependency:** [0154](0154-the-site-becomes-navigable.md) **Phase 3.** This plan renames
> headings, and under [ADR-0166](../adrs/0166-a-published-document-splits-into-routes-by-size.md) a
> heading is a URL. `scripts/check-doc-links.mjs` deliberately never validates fragments, so today a
> renamed heading breaks every inbound anchor silently. 0154 Phase 3 turns that into a build
> failure. **Do not start this plan before that phase has landed.**
> **Lane guidance:** build on `main` directly. Every phase is markdown; no Rust compiles.

## TL;DR

Three documents a preset author lives in carry 235 bare references to plans and ADRs — *"Measured at
Plan 0063 Phase 5"*, *"ADR-0047's treatment"* — written for a skill session reconstructing why a
threshold exists, and read by someone who wants to know what a parameter does. This plan rewrites
that prose under one rule: keep the fact, demote the provenance to the link. The 109 citations that
are already links stay links; what goes is the clause built around them. A gate then holds the
result, so the five reader documents cannot silently drift back.

## Context & problem

The user's instruction was direct: a reader *"should understand how the application works, but not
why this or that decision were made."*

The citation convention that produced this prose is deliberate and good.
[ADR-0127](../adrs/0127-a-comment-carries-the-mechanism-and-the-decision-record-stays-in-docs.md)
requires code comments to cite by bare number so a claim can be traced to the measurement that
earned it, and `CLAUDE.md` extends the habit to documentation. It has worked — this corpus is
unusually free of assertions nobody can check. It also addresses the wrong audience in the documents
a user reads.

Measured 2026-09-05 against the tree at `9dc2183`:

| Document | Bytes | Citations | Already links | **Bare, in scope** |
|---|---:|---:|---:|---:|
| `presets/README.md` | 273,211 | 220 | 63 | **157** |
| `docs/presets.md` | 75,803 | 68 | 20 | **48** |
| `docs/preset-palettes.md` | 65,009 | 54 | 24 | **30** |
| `docs/preset-guide.md` | 18,215 | 2 | 2 | 0 |
| `docs/preset-tuning-walkthrough.md` | 18,079 | 0 | 0 | 0 |

Two of the five are already clean. That is the most useful fact here: the register being asked for
is not hypothetical, it is what the illustrated guide and the tuning walkthrough already do, and the
other three can be brought to it rather than invented.

The work is bounded by what these documents are *for*. They are what the `preset-author` lane is
pointed at instead of keeping a private catalogue
([ADR-0017](../adrs/0017-preset-author-skill-lane.md)) — the private copies rotted while these
stayed current. They cannot be thinned. And much of the provenance is what makes a number checkable:
*"measured at Plan 0063 Phase 5"* is the difference between a threshold a reader can trust and one
they take on faith.

## Decision

Rewrite the prose of the three documents that carry bare citations, under the rule recorded in
[ADR-0168](../adrs/0168-the-reader-documents-address-a-reader-and-the-record-stays-a-link.md):

> **Keep the fact. Demote the provenance to the link.**

A sentence states what the software does and what the number is. The plan or ADR that earned it
becomes a link on that sentence, or is dropped where the sentence stands on its own. What is removed
is the narration of the decision — which plan measured it, in which phase, what it superseded — not
the measurement.

```
before:  Measured at Plan 0063 Phase 5; `depth_fade` above 0.8 flattens the figure
         because the far end of the attractor stops separating (design-backlog 0062).

after:   `depth_fade` above 0.8 flattens the figure: the far end of the attractor stops
         separating from the near end.
```

We rejected a mechanical strip alone (it reaches 7 % of the corpus and leaves plan numbers
mid-sentence on every long page), marked rationale regions (they edit `docs/` to serve the site and
depend on authors remembering a convention forever), and a reader-facing copy per document (the
split-source hazard ADR-0154 exists to prevent).

**A note on the owner tag.** Every phase below is tagged `dev` because the plan vocabulary is `dev`
or `human` and nothing else. This is editorial work on documentation, which is closer to
`architect`'s territory than to `dev`'s, and closer still to `preset-author`'s for the roster. The
vocabulary has no way to say that. It is flagged here rather than worked around; if this recurs, the
tag vocabulary is what needs an ADR.

## Architecture diagram

```mermaid
flowchart LR
    subgraph before["today"]
        B1["'Measured at Plan 0063 Phase 5;<br/>depth_fade above 0.8 flattens<br/>the figure (design-backlog 0062).'"]
    end

    subgraph rule["the rule"]
        R1["keep the fact"]
        R2["provenance → the link,<br/>or dropped"]
        R3["remove the narration<br/>of the decision"]
    end

    subgraph after["after"]
        A1["'depth_fade above 0.8 flattens<br/>the figure: the far end stops<br/>separating from the near end.'"]
    end

    subgraph held["what holds it"]
        G1["check-reader-prose.mjs<br/>no bare citation outside a link"]
        G2["0154 Phase 3 fragment gate<br/>a renamed heading fails the build"]
        G3["toc.mjs<br/>contents blocks regenerate"]
    end

    B1 --> R1 --> A1
    B1 --> R2 --> A1
    B1 --> R3 --> A1
    A1 --> G1
    A1 --> G2
    A1 --> G3
```

## Implementation phases

### Phase 1 — the rule, on the smallest real case

- **Owner skill:** dev
- **What:** Rewrite `docs/preset-palettes.md` — 30 bare citations, 65,009 bytes. This is the phase
  where the rule is established in practice, so its result is the reference for the four that
  follow. Where a passage cannot be rewritten without losing a fact, keep the citation as a link and
  note the passage.
- **Files touched:** `docs/preset-palettes.md`.
- **Done when:** No `Plan NNNN` or `ADR-NNNN` appears in the file outside a markdown link. Every
  measurement, threshold and named parameter that was in the document before is still in it — this
  is a rewrite of framing, not a cut, and the file does not get materially shorter. `node
  scripts/toc.mjs` regenerates the contents block and `node scripts/check-doc-links.mjs` exits 0.

### Phase 2 — the expression language

- **Owner skill:** dev
- **What:** Rewrite `docs/presets.md` — 48 bare citations, 75,803 bytes. This is the grammar
  reference; the `preset-author` skill points at it as **the** expression-language authority, so
  every function, variable, constant, operator and error message it documents must survive intact.
- **Files touched:** `docs/presets.md`.
- **Done when:** No bare citation outside a link. The grammar surface is unchanged — the set of
  documented functions, variables and constants before and after is identical, checked by listing
  them from both versions rather than by reading.

### Phase 3 — the roster's own sections

- **Owner skill:** dev
- **What:** Rewrite the ten smaller `##` sections of `presets/README.md` — the skeleton, the
  representative flag, the expression language summary, the palette surface, and the `[smoothing]`,
  `[latch]`, `[occupancy]`, world-space, `[layer]` and starting-point sections. Together they are
  about 40 KB and they are where a reader arrives.
- **Files touched:** `presets/README.md`.
- **Done when:** No bare citation outside a link in those sections. Every parameter name, default and
  range is unchanged. The three large sections are deliberately untouched and are Phase 4.

### Phase 4 — the three sections that are most of the document

- **Owner skill:** dev
- **What:** Rewrite `## Systems and their named parameters` (109,093 B), `## Engine-wide controls`
  (63,013 B) and `## Structural config` (47,757 B). This is the bulk of the 157 and the bulk of the
  risk: these sections are reference tables wrapped in prose, and the tables are what the
  `preset-author` lane reads.
- **Files touched:** `presets/README.md`.
- **Done when:** No bare citation outside a link anywhere in the file. The parameter roster is
  complete — the set of named parameters documented before and after is identical, listed from both
  versions and compared, not eyeballed. Under 0154's split these sections are 32 routes; each still
  opens with something that tells a reader what the section is for.

### Phase 5 — the gate, and the sweep

- **Owner skill:** dev
- **What:** `scripts/check-reader-prose.mjs` asserts that no bare `Plan NNNN` / `ADR-NNNN` appears
  outside a markdown link in the five Entrance A documents. Wire it into `.githooks/pre-push` and
  CI's `links` job beside the other gates, add its seeded bite check under `scripts/fixtures/`, and
  update `CLAUDE.md`'s `scripts/` block, which states the gate count.
- **Files touched:** `scripts/check-reader-prose.mjs` (new), `scripts/fixtures/`,
  `.githooks/pre-push`, `.github/workflows/ci.yml`, `CLAUDE.md`.
- **Done when:** The gate exits 0 on the rewritten tree, and exits 1 naming file and line when a bare
  citation is introduced into any of the five — provoked in all three directions (a bare citation
  added, a linked citation left alone, a citation in an Entrance B document ignored) and the tree
  restored. `CLAUDE.md`'s gate count is correct. The `preset-author` skill's pointers into these
  three documents still describe what is there.

## Risks & open questions

- **A fact can leave with its provenance, and nothing catches it.** The gate checks that citations
  are gone; no gate can check that a rewrite is good. The per-phase done-whens are written to make
  the loss checkable where it is mechanical — parameter sets, grammar surfaces, thresholds compared
  before and after by listing rather than reading. Prose judgement is not covered and cannot be.
- **This is 414 KB of judgement in one `dev` session.** If it does not fit, stopping cleanly at a
  phase boundary is correct and expected; the phases are ordered so each is independently valuable
  and leaves the tree consistent.
- **Heading renames move URLs.** 0154 Phase 3 makes inbound breakage a build failure, which is the
  hard dependency in the header. External links and bookmarks are not covered by anything.
- **The `preset-author` lane reads these files while they change.** A session running against a
  half-rewritten roster sees a consistent document at every phase boundary but not mid-phase.
- **Two conventions will now coexist.** Entrance B and all code comments keep bare-number citation
  per ADR-0127; these five documents forbid it. The boundary is a filename list in a gate script,
  and someone will eventually be surprised by it.

## What this plan does NOT do

- **It does not touch Entrance B.** `docs/capturing.md`, `docs/nfr.md`,
  `docs/on-device-validation.md`, `docs/releasing.md`, the two specs and the technique catalogue keep
  their citations, because their readers are contributors for whom the working record is the point.
- **It does not shorten the documents.** Completeness is the property ADR-0154 named as worth
  keeping, and the `preset-author` lane depends on it. A materially shorter file is a defect here,
  not a success.
- **It does not remove the 109 citations that are already links.** A link is inert until clicked.
- **It does not change any code, default, parameter name or behaviour.** Nothing under `core/`,
  `standalone/`, `plugin-foobar/` or `presets/*.toml` is touched.
- **It does not touch the site.** All of that is [Plan 0154](0154-the-site-becomes-navigable.md).

## Implementation log

> Written by `dev` — one row per phase as that phase's commit lands, and the close block after the
> last one. **The phases above are the contract; everything here is what happened.**
> **Observations, never conclusions:** this says where to look, architect decides how it went.

**Lane:** _(`main` directly, per the header's Lane guidance)_

| phase | owner | state | commit |
|---|---|---|---|
| 1 — the rule, on the smallest real case | dev | | |
| 2 — the expression language | dev | | |
| 3 — the roster's own sections | dev | | |
| 4 — the three sections that are most of the document | dev | | |
| 5 — the gate, and the sweep | dev | | |

### Notes

### Close triggers

- **`presets/` touched:**
- **Plan header `Closes:`** none
- **What shipped:**
- **Operator docs touched:**
- **Backlog probes (`node scripts/check-backlog-claims.mjs`):**
- **Full suite:**
- **Outstanding `human` phases:**

## Followups (after this lands)

- The same treatment for `docs/capturing.md` if it turns out non-contributors read it — it is the
  largest Entrance B document at 165,028 bytes and its `shot` CLI section is genuinely user-facing.
- An ADR on the phase owner-tag vocabulary, if editorial work recurs often enough that tagging it
  `dev` keeps being wrong.
