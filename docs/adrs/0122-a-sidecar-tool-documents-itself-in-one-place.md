# ADR-0122 — A sidecar tool documents itself in one place, and a gate keeps its numbers from spreading

> **Status:** accepted 2026-08-25 (Plan 0106 Phase 7e)
> **Date:** 2026-08-25
> **Related plan(s):** [0106](../plans/0106-the-frame-stream-passes-through-a-diffusion-model.md) Phase 7e

**On the number.** This ADR was written as **0120** on the plan lane and renumbered to **0122** at
the close, 2026-08-25. 0120 had been reserved for
[Plan 0111](../plans/done/0111-the-milkdrop-import-stops-washing-out.md) Phase 3 and returned to the
pool when that phase did not run, on Phase 2's stop condition — and
[ADR-0121](0121-the-diffusion-filter-is-an-offline-stage-with-profiles-and-it-interpolates-its-own-stride.md)'s
own numbering note anticipated exactly that case: *"If 0111 takes its stop branch, 0120 stays free
for the next taker."* Two lanes then took it on the same day, in different worktrees, neither able
to see the other: `main` accepted
[ADR-0120](0120-the-close-brief-is-a-section-of-the-plan.md) at Plan 0112's close, and this lane
wrote a different 0120. The filenames differ, so the merge took both silently rather than
conflicting. `main`'s was already accepted and cited from `CLAUDE.md` and two skills, so this one
moved. **The general lesson is that a returned-to-the-pool number is a race across parallel lanes,
and nothing in the repository detects the collision** — the next-free-number line in
[`README.md`](README.md) is advisory, and two branches read the same value.

## Context

Plan 0106 shipped `tools/sd-filter/` — a Python stdio stage that sits between `shot --render` and
`ffmpeg` — and documented it in **three files at once**. The duplication is not incidental trimming
around a shared core; it is most of the substance:

| Fact | `docs/capturing.md` | `tools/sd-filter/README.md` | `README.md` |
|---|---|---|---|
| what the profiles are, and their flag expansions | full | full | — |
| `--size` is a pixel budget, not a side length | full | full | — |
| `--stride N` preserves the frame count | full | full | — |
| the standard-library check and what it asserts | full | full | — |
| the per-frame cost table | full | full | prose |

Each pair is written out in **different words**, so the two copies cannot be diffed and disagree
silently by construction. That is not a hypothetical: at Plan 0106's close review a correction to
the cost figures enumerated two of the three copies and **missed
`tools/sd-filter/README.md:90`** — found only by grepping the numerals themselves, after the file
list had already been written down and committed. The structure produced that miss; a more careful
reader would not have been the fix.

Two further forces point the same way:

- **`docs/capturing.md` is 2 119 lines**, and it documents the capture and render tooling that
  *ships*. The filter ships nothing — no model, no weights, no Python runtime — and it took 125 of
  those lines. The document absorbing a non-shipping sidecar is how it got that long.
- **The feature has three named followups** — `shot --diffuse`, the realtime plan, and the diffused
  frame re-entering the renderer — and each would land in whichever document is canonical. Choosing
  now is cheaper than choosing after three more sections have been written twice.

The audience is settled and it is narrow: this is **lab tooling for its own author**, not a
supported public feature. That lowers the polish budget and it also lowers the tolerable machinery —
whatever is built here has to be worth it for a readership of one.

## Decision

**The diffusion filter's documentation lives in one canonical page, `docs/diffusion-filter.md`, and
every other mention is a pointer that carries no fact of its own. A gate asserts that the numbers
have not spread back out.** Each clause is load-bearing:

- **One page holds everything**: what it is, what you need, setup, the canonical command, the
  profiles and their expansions, the cost table, the flag reference, `--size`, `--stride`, the
  check, and the sharp edges. A followup adds a section *here* and nowhere else.
- **`docs/capturing.md` keeps roughly twelve lines and no figures** — but it keeps the one fact that
  is genuinely about the *pipe* rather than about the filter: that because `--stride` preserves the
  frame count, the `ffmpeg` invocation carries no rate that must agree with a flag on another
  process, so the encoder line is invariant. That belongs beside the `--ffmpeg` section it qualifies.
- **`tools/sd-filter/README.md` becomes install plus a pointer.** It stays because a reader who
  arrives at the directory needs somewhere to land, and `requirements.txt`'s two environment traps
  are already commented where they are useful.
- **`README.md` keeps a paragraph, a link, and exactly one orientation figure** — roughly how long a
  four-minute track takes at `fast`, with its machine named. A link alone cannot tell a reader
  whether they are contemplating an hour or a night, and that judgement is the whole reason the line
  exists.
- **The gate enforces the absence of copies, not the agreement of copies.** `scripts/` gains a check
  in the family of the existing three: the canonical page marks its figures with
  `<!-- figures:begin -->` / `<!-- figures:end -->` markers, in ADR-0116's idiom, and the check
  asserts that **no other markdown file mentioning `sd-filter` or `sd_filter.py` contains a cost
  figure at all** — with the single `README.md` orientation line whitelisted and required to match a
  figure inside the canonical region. Scoping the scan to documents that name the filter is what
  keeps it precise: `0.451` also appears in
  [ADR-0040](0040-spectrum-level-curve-applies-before-the-easing.md) as a curve value, and a
  units-only regex over all of `docs/` would convict it.

## Consequences

### Positive

- **A correction has exactly one place to land.** Plan 0106 Phase 7d has to restate every cost
  figure once the instrument is fixed; after this it restates them once instead of three times, and
  cannot miss a copy it did not know about.
- **The failure mode is the one that actually happened.** The gate does not check that duplicates
  agree — it checks that duplicates do not exist. The miss at the close was a copy outside the
  enumerated list, and a same-value assertion across a known list would not have caught it either.
- **`docs/capturing.md` stops absorbing non-shipping tooling**, and drops back to roughly 2 006
  lines. The rule generalizes: the next `tools/` sidecar gets its own page rather than another
  section here.
- **The followups have a home before they are written**, so the second and third of them do not
  re-run this argument.

### Negative

- **A fourth `scripts/` gate is a fourth thing that can be wrong**, and it is being built for a
  readership of one. The honest defence is that the three existing gates all exist because a
  convention was written down in the file it governed and failed anyway — ADR-0116's rows regrew
  7.1x with *"One line per plan"* three lines above them — and this convention has already failed
  once, at the close of the plan that created it.
- **The gate can only see markdown.** A cost figure in a code comment, a commit message, or a plan's
  implementation log is out of scope and will drift. That is accepted: those are records of what was
  true when written, not instructions to a reader.
- **A reader who lands in `tools/sd-filter/` now needs one hop** to learn anything beyond how to
  install it. Mitigated by the pointer being the first thing in the file, not eliminated.
- **The move is churn against a plan that is closing**, and it invalidates section anchors that
  `README.md` and the plan already link to. Every one of those is caught by
  `scripts/check-doc-links.mjs` — except fragment anchors, which that checker does not validate, so
  the `#a-filter-stage-between-shot-and-the-encoder` style links need checking by hand.

### Neutral

- The canonical page is written **for its author**: terse, honest, no marketing. Plan 0106 Phase 5's
  done-when — that a reader who has never run this can reach an MP4 by following the page — is
  preserved, because that property is about completeness rather than polish and it was already met.
- This ADR decides where the words live, not what they say. The **content** corrections owed by
  Plan 0106 Phase 7d (the instrument measures the diffusion call, not the render) are independent
  and land in the same phase.

## Alternatives considered

### Alternative A — `tools/sd-filter/README.md` is canonical

The least churn: the substance is already there, beside the code, where anyone installing the tool
must go. Rejected because it splits the *render-pipeline narrative* across two trees — a reader
following `docs/capturing.md` through `--render` and `--ffmpeg` would step out of `docs/` mid-pipe
and back in for the encoder — and because it gives the three named followups a home outside `docs/`,
where every other design record in this project lives.

### Alternative B — `docs/capturing.md` is canonical

Keeps one pipeline narrative end to end, which is a real virtue. Rejected on the size argument: the
document is already 2 119 lines and this would grow it further for tooling that ships nothing, in
the file whose job is the tooling that does. The followups would compound it.

### Alternative C — no frozen numbers anywhere

The docs would say only *"the filter prints its own mean when it finishes; run it and read yours"*,
which removes drift permanently rather than gating it. Genuinely tempting, and it is the only option
here that needs no machinery at all. Rejected because a reader deciding **whether to start** has no
number at all until they have already committed to a render — and the gap being decided is between
roughly an hour and roughly a night. The orientation figure is worth its upkeep; the cost table is
worth having in one place.

### Alternative D — keep the duplication and gate that the copies agree

The literal shape of "add a gate": leave the tables where they are and assert the numerals match.
Rejected because it gates the wrong property. The copies that agree were never the problem — the
problem was a copy **outside the list anyone was checking**, which an agreement check across a known
list reproduces exactly. It also preserves the prose duplication, which is the larger half and which
cannot be checked by any means short of reading both.
