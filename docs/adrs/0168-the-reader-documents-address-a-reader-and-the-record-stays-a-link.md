# ADR-0168 — The reader documents address a reader, and the working record stays a link

> **Status:** accepted 2026-09-05 (Plan 0154)
> **Date:** 2026-09-05
> **Related plan(s):** [0154](../plans/done/0154-the-site-becomes-navigable.md) (the mechanical half),
> [0155](../plans/0155-the-reader-documents-stop-explaining-themselves.md) (the prose half)

## Context

This repository has a strong, deliberate citation convention.
[ADR-0127](0127-a-comment-carries-the-mechanism-and-the-decision-record-stays-in-docs.md) requires
code comments to cite by bare number — `ADR-0046`, `Plan 0045 Phase 3` — and `CLAUDE.md` extends the
same habit to the documentation. It exists so a claim can be traced to the measurement that earned
it, and it has worked: this corpus is unusually free of assertions nobody can check.

It also addresses the wrong audience in the four documents a user reads. Measured 2026-09-05 across
the 13 published files, there are **792** references to a plan or an ADR:

| Class | Count | Reachable by a build-time transform? |
|---|---:|---|
| Headings ending in a provenance parenthetical — `## Engine-wide controls (Plan 0018)` | 31 | Yes |
| Body lines ending in a standalone provenance parenthetical | 26 | Yes |
| Woven into sentence grammar — *"Measured at Plan 0063 Phase 5"*, *"ADR-0047's treatment"*, or a parenthesised ADR link dropped mid-clause | 735 | **No** |

The last row is 93 % of the total, and it cannot be removed mechanically without producing
sentences that no longer parse. The user's instruction is explicit: a reader *"should understand how
the application works, but not why this or that decision was made."*

The heading class is worse than cosmetic under
[ADR-0166](0166-a-published-document-splits-into-routes-by-size.md), which makes a heading into a
URL. Left alone, the site would serve `.../guide/parameter-roster/engine-wide-controls-plan-0018/`,
and a plan number would be a permanent part of the address.

Two facts bound what can be done about the other 93 %. These documents are what the
`preset-author` lane is pointed at *instead of* keeping a private catalogue
([ADR-0017](0017-preset-author-skill-lane.md)), and the private copies rotted while these stayed
current — so they cannot be thinned into a marketing surface. And much of the provenance is what
makes a number checkable: *"measured at Plan 0063 Phase 5"* is the difference between a threshold a
reader can trust and one they have to take on faith.

## Decision

The reader documents are rewritten to address a reader, in two halves that ship in two plans.

**The mechanical half, at build time (Plan 0154).** A remark plugin strips the trailing provenance
parenthetical from headings and from body lines whose only content after the fact is the citation.
It runs **before** slugs are computed, so routes and anchors are built from clean headings. It
touches 57 sites and no source file.

**The prose half, in the source (Plan 0155).** The 735 woven citations are rewritten under one rule:

> **Keep the fact. Demote the provenance to the link.**

A sentence states what the software does and what the number is; the plan or ADR that earned it
becomes a link on that sentence, or is dropped when the sentence is self-evident. What is removed is
the *narration of the decision* — which plan measured it, in which phase, and what it superseded —
not the measurement itself.

```
before:  Measured at Plan 0063 Phase 5; `depth_fade` above 0.8 flattens the figure
         because the far end of the attractor stops separating (design-backlog 0062).

after:   `depth_fade` above 0.8 flattens the figure: the far end of the attractor stops
         separating from the near end.
```

**The 233 citations that are already links stay links.** A link is opt-in — it does not interrupt
reading, and it is the escape hatch for a reader who does want the reasoning. What gets removed is
the clause built around it, not the door.

**Scope is Entrance A, not the whole published set.** The Entrance B documents — `docs/nfr.md`,
`docs/capturing.md`, `docs/on-device-validation.md`, `docs/releasing.md`, the two specs and the
technique catalogue — **keep their citations**, because their readers are contributors for whom the
working record is the point rather than the noise. The mechanical strip still applies to all of
them, since a plan number in a URL is wrong either way.

That leaves the five documents a user or preset author reads, and measuring them per file shrinks
the job by two thirds:

| Document | Bytes | Citations | Already links | **Bare, in scope** |
|---|---:|---:|---:|---:|
| `presets/README.md` | 273,211 | 220 | 63 | **157** |
| `docs/presets.md` | 75,803 | 68 | 20 | **48** |
| `docs/preset-palettes.md` | 65,009 | 54 | 24 | **30** |
| `docs/preset-guide.md` | 18,215 | 2 | 2 | 0 |
| `docs/preset-tuning-walkthrough.md` | 18,079 | 0 | 0 | 0 |

**235 bare citations across three documents**, not 735 across five — the guide and the walkthrough
are already clean, which is itself evidence that the register being asked for is achievable here and
has been achieved before. The 735 figure is corpus-wide and mostly lands in Entrance B, which this
decision deliberately leaves alone.

**A gate holds the result.** `scripts/check-reader-prose.mjs` asserts that in those five documents
no `Plan NNNN` or `ADR-NNNN` appears outside a markdown link. The rule is exactly the decision above
— keep the fact, demote the provenance to the link — expressed as something a machine can check, so
the convention does not depend on every future author remembering it.

**This is not a violation of ADR-0154's one-source rule.** That rule forbids a second, site-shaped
copy of a document, and forbids editing a source file to perform a transformation the build should
perform. A prose rewrite that improves the document for every surface it is read on — the editor,
GitHub, and the site — is an edit to the source, which is exactly where an edit belongs.

## Consequences

### Positive

- **The documents get better on GitHub too.** The rewrite is not a site feature. `presets/README.md`
  is read in an editor by the `preset-author` lane far more often than it is read on a website, and
  it stops opening every third paragraph with the archaeology of its own thresholds.
- **URLs and search results stop carrying plan numbers.** Under ADR-0166 the heading strip is what
  keeps 112 new routes from being addressed by the plans that created them.
- **The citation convention is preserved where it earns its place.** ADR-0127 governs code comments
  and is untouched; Entrance B is untouched; the working record itself is untouched. The change is
  scoped to the surface where the convention was serving the wrong reader.

### Negative

- **It is a large, unmechanical edit to three documents totalling 414 KB**, done by judgement, one
  passage at a time. The gate above checks that citations are gone; **no gate can tell a good
  rewrite from a bad one**, and none fails if a fact is lost along with its provenance. This is the
  principal cost and it is not small.
- **Information will be lost, and some of it will matter.** A clause naming the phase that measured
  a threshold is sometimes the only record that the threshold was measured at all. The rule keeps
  the fact, but a rewriter who misjudges a passage removes evidence with no way to notice.
- **Heading changes move routes.** Under ADR-0166 a heading is a URL, so the rewrite churns the
  address space of the site it is meant to improve. The plan ordering exists for this reason: the
  fragment gate lands first, so a broken inbound anchor fails the build rather than rotting.
- **Two documents now have two registers.** `presets/README.md` is reader-facing prose wrapped
  around reference tables that are unavoidably technical. The seam will be visible.
- **The regression gate constrains how these five documents may be written from now on.** A future
  author adding a section to the roster cannot cite a plan by bare number in it, which is the
  opposite of what ADR-0127 and `CLAUDE.md` ask for everywhere else in the repository. Two rules now
  apply in two places, and the boundary between them is a list of five filenames in a gate script.

### Neutral

- The generated contents blocks (ADR-0163) are regenerated from the rewritten headings by
  `scripts/toc.mjs`; the rewrite must run it, and `--check` catches the omission at pre-push.

## Alternatives considered

### Alternative A — the mechanical strip only, no prose rewrite

The cheap option, and the recommendation this ADR did not take. It removes 57 of 792 citations —
**7 %** — and leaves the reader meeting plan numbers mid-sentence on every long page. It is
recorded because it is the fallback if the prose half stalls: the mechanical half is independently
valuable, ships in a different plan, and is a hard dependency of ADR-0166's slug rule regardless.

### Alternative B — marked rationale regions in the source

Authors wrap decision prose in a marker the site drops at build time, with a gate keeping markers
balanced. Complete in principle, and it fails on two counts: it edits `docs/` to serve the site,
which is the thing ADR-0154 exists to prevent, and it depends on every future author remembering a
convention forever — the same dependency that has already failed here for close-ceremony archiving
and index-row length, both of which needed gates rather than conventions.

### Alternative C — two files, one per audience

A reader-facing document and a working-record document, per subject. It is the split-source hazard
ADR-0154 was written to avoid, restated: two files with overlapping content, one of which will rot,
and the rotting one will be whichever the skill lanes do not open daily.

### Alternative D — strip the citation links as well as the surrounding clause

Considered and rejected at interview. A link costs a reader nothing — it is inert until clicked —
and removing all 233 would leave residual references to measurements with no way to reach them,
which reads as unexplained jargon rather than as clean prose.
