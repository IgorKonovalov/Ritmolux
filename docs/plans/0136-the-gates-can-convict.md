# 0136 — The gates can convict

> **Status:** in-progress
> **Created:** 2026-08-29
> **Owner skill(s):** dev, human
> **Related ADRs:** [0149](../adrs/0149-a-backlog-reference-is-a-bare-number-and-a-file-link.md) (proposed)
> **Closes:** design-backlog 0104, 0143, 0162, 0127, 0133, **0166, 0170, 0171, 0173**.
> **0160 and 0161 are corrected in place, not closed** — see Phase 8.
>
> **Amended 2026-09-01**, from the backlog round that wrote Plans 0147-0149. Four gate entries filed
> after this plan was approved land here rather than in a sibling plan, because a second lane over
> `scripts/check-*.mjs` would contend with this one on the same six files for no benefit. They are
> **Phase 3** (0166), a done-when on **Phase 5** (0171), and **Phase 7** (0170 + 0173). The plan's
> own closing claim that it *"does not touch `check-comment-hygiene.mjs`, the one gate in `scripts/`
> with no live complaint against it"* was falsified by 0170 and 0173 a day after it was written, and
> is repaired below.

## TL;DR

This repository verifies itself with five Node gates and a rendered-image sweep, and several of
them cannot fail when they should. `check-index-rows.mjs` has no assertion it can convict with — a
mutation that makes its row detector match nothing still exits 0 at all three call sites.
`check-backlog-claims.mjs` resolves probe paths against the working tree, so a probe pointing into
gitignored territory passes on the authoring machine and can only ever fail on CI, after the push.
`docs-shots.mjs` throws before rendering anything and has done since 2026-08-15, so four committed
gallery images show a stroke the engine no longer draws. The first visible behavior is a seeded red
fixture that makes `check-index-rows.mjs` exit 1 on demand.

## Context & problem

[ADR-0033](../adrs/0033-testing-strategy-coverage-ratchet-and-pre-push-gate.md)'s whole argument is
that **a rule nothing re-runs is a rule nobody follows**. The corollary this plan addresses is
sharper: *a check that re-runs and cannot fail is the same rule wearing a green tick.*

That failure has now been found and repaired twice —
[Plan 0084](done/0084-two-gates-stop-lying-about-what-they-check.md) found the link checker covering
one of markdown's two link forms, and
[Plan 0094](done/0094-the-two-doc-gates-check-what-they-claim-to.md) found a directory-name skip
swallowing a real tree plus a whole half of ADR-0108's rule invisible to a bullet-driven check. Both
gates now ship fixtures expecting **exit 1 with an exact break count**. `check-index-rows.mjs` is the
one of the three that was never given that treatment, and its fixture asserts exit 0 — a choice
`scripts/fixtures/README.md` argues was correct against a tree holding 136 over-cap rows, and which
Plan 0105's own later phases made false.

The demonstration is in backlog 0104 and is not hypothetical: copying the script and replacing
`TABLE_ROW` and `BULLET` with regexes that match nothing yields `3 regions, 0 rows, 0 over cap` and
**exit 0**, from the fixture and from the repository alike. Pre-push, the CI `links` job and the
architect close ceremony all go green. The per-file counts the script prints are the documented
mitigation, and they are *printed*, not asserted.

Two of the six entries here have a common shape worth naming: **the gate is green precisely where
the fix would be cheap.** 0162's probe-path bug goes green at pre-push and red on CI, so the author
learns from a runner after pushing that a claim they verified does not verify. 0133's image sweep
fails only for the human who tries to run it at a close, eleven days after the commit that broke it.

## Decision

**Repair the instruments before trusting more findings from them**, and take the entries in
increasing order of how much judgement they need. Phases 1-2 give `check-index-rows.mjs` both shapes
backlog 0104 names — they are explicitly not exclusive, the `--self-test` covers the demonstrated
mutation and the red fixture covers the reporting path that nothing currently runs. Phase 3 gives the
same gate the *shape* check it has never had. Phase 4
implements [ADR-0149](../adrs/0149-a-backlog-reference-is-a-bare-number-and-a-file-link.md). Phase 5
teaches the claim gate to ask git whether a probe path is tracked, and to stop destroying a probe's
own spacing on the way in. Phase 6 takes the figure gate's
untracked-file complaint as an advisory rather than an exit code, and Phase 7 makes two gates judge
the code this project actually wrote. Phase 8 corrects two entries whose
premise ADR-0147 falsified this morning. Phase 9 revives the image sweep, and Phase 10 is the human
look at what it re-shoots.

We rejected doing the doc-image work first even though it is the most visibly stale thing here,
because Phase 9's manifest decision wants `warp_mesh` to have something to be a picture *of*, and
that is a content question the plan should reach with its cheap mechanical work already banked.

## Architecture diagram

```mermaid
flowchart TB
    subgraph gates["scripts/ — the five Node gates"]
        IR["check-index-rows.mjs"]
        BC["check-backlog-claims.mjs"]
        FF["check-filter-figures.mjs"]
        DL["check-doc-links.mjs"]
    end
    subgraph fixtures["scripts/fixtures/ — seeded bite checks"]
        GREEN["index-rows/ (exit 0)"]
        RED["index-rows-red/ (exit 1, 1 break)<br/>NEW — Phase 2"]
    end
    IR -->|--self-test asserts its own counts<br/>Phase 1| IR
    RED --> IR
    GREEN --> IR
    BC -->|"git ls-files: is the probe path tracked?<br/>Phase 5"| GIT["git"]
    FF -->|untracked hit becomes advisory<br/>not exit code — Phase 6| ADV["advisory block"]
    DL -->|"rejects design-backlog#fragment<br/>Phase 4, ADR-0149"| DL
    subgraph sweep["the image sweep (no GPU in the check half)"]
        DS["docs-shots.mjs"] -->|cross-check manifest vs SystemKind| SK["core/src/preset/schema.rs"]
        DS -->|renders| IMG["docs/images/gallery/"]
    end
```

## Implementation phases

### Phase 1 — The row gate asserts its own counts
- **Owner skill:** dev
- **What:** Close the first half of backlog 0104. Add `--self-test` to `check-index-rows.mjs` on the
  model `check-backlog-claims.mjs` already carries, asserting the green fixture's own counts so a
  detector that finds nothing fails loudly.
- **Files touched:** `scripts/check-index-rows.mjs`, `scripts/fixtures/README.md`.
- **Notes for the implementer:**
  - The assertion is the fixture's **exact** region and row counts (3 regions, 4 rows), not a
    non-zero check. A `> 0` assertion survives a detector that matches one row in ten.
  - `check-backlog-claims.mjs`'s `--self-test` additionally pins a non-vacuity assertion to the
    **real repository** rather than to the fixture. Do the same here — the repository's own row
    count is what a matches-nothing mutation collapses.
  - Wire `--self-test` into the same call sites the gate already has, or it is a mechanism nobody
    runs, which is the defect being repaired.
- **Done when:**
  - Replacing `TABLE_ROW` and `BULLET` with regexes that match nothing makes the gate exit non-zero.
    That mutation is backlog 0104's own reduction; `dev` states the result of running it.
  - The unmutated gate still exits 0 on the current tree.

### Phase 2 — A seeded tree the row gate rejects
- **Owner skill:** dev
- **What:** Close the second half of backlog 0104. Add `scripts/fixtures/index-rows-red/` holding one
  over-cap row inside a marked region, run as its own root and expected to exit 1 with exactly one
  break.
- **Files touched:** `scripts/fixtures/index-rows-red/**` (new), `scripts/fixtures/README.md`,
  `.githooks/pre-push` and the CI `links` job if the fixture needs its own invocation.
- **Notes for the implementer:**
  - **Do not flip the existing green fixture.** It carries four negative assertions (a row outside
    markers, an unmarked table, and so on) that are worth keeping; the entry is explicit that the two
    shapes are complementary.
  - Expect **exit 1 with an exact break count**, matching what Plans 0084 and 0094 established for
    the sibling gates. A bare "exits non-zero" passes on a crash.
  - This is the phase that exercises the reporting path — the `file:line N bytes (cap C)` formatting
    — which nothing currently runs at all.
- **Done when:** the red fixture exits 1 naming exactly one over-cap row, and a change that breaks the
  reporting format fails it rather than printing garbage and passing.

### Phase 3 — The row gate reads a row's shape, not just its length
- **Owner skill:** dev
- **What:** Close backlog 0166. `check-index-rows.mjs` measures a row's bytes and never asks what
  kind of row it is, so a closed-plan bullet dropped into the active-plans table passes.
- **Files touched:** `scripts/check-index-rows.mjs`, `scripts/fixtures/**`.
- **Notes for the implementer:**
  - The entry was filed *"by the reviewer making the mistake and having the gate wave it through"* —
    it is a real close-ceremony error, not a hypothetical. Steps 2, 3 and 3c of that ceremony each
    rewrite a roster, which is exactly when the wrong shape gets pasted into the right region.
  - **A shape check is a different assertion from a length check** and belongs beside it, not
    instead of it. A 200-byte bullet in a table region is under cap and still wrong.
  - The marked regions already declare what they hold; the check is that a row's form matches its
    region's, and the message says which form was expected.
- **Done when:** a closed-plan bullet placed inside the active-plans `roster:begin` region fails the
  gate by name; a correctly-shaped row of the same length passes; and the red fixture from Phase 2
  gains this case so the reporting path is exercised for it too.

### Phase 4 — Backlog references stop using fragments
- **Owner skill:** dev
- **What:** Close backlog 0143. Implement
  [ADR-0149](../adrs/0149-a-backlog-reference-is-a-bare-number-and-a-file-link.md): rewrite all 24
  anchored references to the bare-number-plus-file-link form, and teach `check-doc-links.mjs` to
  reject a `design-backlog.md#…` or `design-backlog-archive.md#…` fragment.
- **Files touched:** `scripts/check-doc-links.mjs`, plus roughly 20 files under `docs/adrs/` and
  `docs/plans/done/`.
- **Notes for the implementer:**
  - The 20 archived numbers are listed in ADR-0149's Context. Re-derive the set rather than trusting
    the list — it was measured 2026-08-27 and closes have run since.
  - **This edits append-only documents.** ADR-0149's Decision permits it and says why; the edit is
    the link form and nothing else. Do not reword surrounding prose, and do not add `Outcome`
    sections.
  - The new rule needs its own fixture bite, or it joins the class this plan exists to fix.
- **Done when:**
  - No `design-backlog*.md#` fragment remains in the repository, and adding one back fails
    `check-doc-links.mjs`.
  - The gate still exits 0 on the repaired tree.

### Phase 5 — A probe path is checked against the repository, not the disk
- **Owner skill:** dev
- **What:** Close backlog 0162. `check-backlog-claims.mjs` asks git whether a probe path is tracked,
  and reports *"probe path is not tracked"* rather than passing locally and failing only on CI.
- **Files touched:** `scripts/check-backlog-claims.mjs`, `scripts/fixtures/**`.
- **Notes for the implementer:**
  - `git ls-files -- <path>` returns nothing for an untracked path and handles directories, which
    probe paths often are, so a non-empty result is the whole test. `git check-ignore -q` answers
    from the other side.
  - **Batch it.** One invocation covering the whole probe set keeps this to a single process, the way
    the staleness advisory already batches its `git log`.
  - The message matters as much as the check: a CI reader currently gets *"does not exist"* for a
    file sitting in front of them in their own tree.
  - Preserve the shallow-clone courtesy already in the advisory — if git cannot answer, print a
    notice in ADR-0016's shape rather than failing 25 entries.
- **Done when:**
  - A probe naming a path under `renders/` (gitignored in full) is reported as untracked by a local
    run, where today it passes.
  - **Closes backlog 0171 in the same file and the same function.** `check-backlog-claims.mjs:225`
    extracts each probe with `.map((m) => m[1].replace(/\s+/g, " ").trim())`, which is right for the
    reason it was written — a markdown bullet may wrap across source lines and the pattern has to
    survive that — and silently rewrites any probe whose regex contains **two or more consecutive
    spaces** into a different regex. Such a probe can never fire. The wrap must still be absorbed;
    what must stop is a run of spaces inside the pattern being collapsed. A probe asserting on a run
    of spaces is added to the fixtures and **fires**, which is the check that this was fixed rather
    than described.
  - The gate still exits 0 on the current tree, and its `--self-test` still holds.

### Phase 6 — The figure gate stops convicting untracked files
- **Owner skill:** dev
- **What:** Close backlog 0127. `check-filter-figures.mjs` keeps walking the working tree, but an
  untracked hit becomes an **advisory that does not set the exit code**.
- **Files touched:** `scripts/check-filter-figures.mjs`, `scripts/fixtures/**`.
- **Notes for the implementer:**
  - The entry costs the counter-argument honestly and lands on this shape: restricting the scan to
    `git ls-files` buys ergonomics and **gives up the gate's whole reason for existing**, which is
    that the copy that broke it was the one outside the list anyone was checking (ADR-0122).
    Advisory keeps both.
  - Use the shape `check-backlog-claims.mjs`'s advisory block already uses, so there is one idiom for
    "reported, never part of the exit code" rather than two.
- **Done when:** a gitignored file carrying a cost figure is named in the advisory and does not fail
  the pre-push hook; a **tracked** one still fails it.

### Phase 7 — The gates judge the code this project wrote
- **Owner skill:** dev
- **What:** Close backlog 0170 and 0173. `check-comment-hygiene.mjs` enumerates with `readdirSync`
  from the repo root and **never asks git what is tracked**, so a gitignored vendored tree is
  invisible to CI and scanned in full everywhere else; and its broken-literal detector cannot see the
  defect in the form that actually produces it.
- **Files touched:** `scripts/check-comment-hygiene.mjs`, `scripts/fixtures/**`,
  `scripts/fixtures/README.md`.
- **Notes for the implementer:**
  - **0170 is the one that blocks pushes.** The gate went from green to **490 findings** between two
    pushes twenty minutes apart with no commit touching it: `.venv/` (419 findings — torch, numpy and
    markupsafe C headers) and `plugin-foobar/sdk/` (71). Both are gitignored, so **CI's fresh clone
    is green by construction** and every working tree is not. The natural escape is `--no-verify`,
    which is what makes it worth fixing: *a gate that fires on vendor code teaches its users to skip
    the gate that fires on theirs.*
  - Plan 0134's close patched the two instances **by name** (`SKIP_DIRS` gained `.venv`, a new
    `VENDORED_TREES` holds the SDK path). That fixes these two and not the class — the next
    `pip install` or unpacked SDK re-breaks it. **Enumerate from `git ls-files`**, which makes "code
    we own" and "code the gate judges" the same set by construction and costs one call.
  - **The one thing to preserve:** `node scripts/check-comment-hygiene.mjs scripts/fixtures` must
    still report its 10 findings. Those fixtures are tracked, so `ls-files` reaches them.
  - **Check whether the sibling gates share the shape.** `check-doc-links.mjs` walks markdown the
    same way and is green today only because neither vendored tree happens to carry a
    relative-linked `.md` — that is luck, not a property.
  - **0173 is the narrower half.** `brokenLiteral` rejects any literal whose decoded text holds a
    newline, which is exactly the shape a lost `\` continuation produces **before** anyone reflows
    it — confirmed at Plan 0144's close by seeding a two-line literal with an 18-space indent and
    watching it go unreported. It catches the form this tree actually produces, so backlog 0168 was
    discharged in substance; what is left is the unrejoined form, plus a fixture README that states
    that silence as a general truth.
- **Done when:**
  - A gitignored directory carrying comment-hygiene violations is **not** scanned, verified by
    seeding one; the fixture bite still reports exactly its 10 findings; and the by-name
    `VENDORED_TREES` / `.venv` patches are **removed**, not left beside the general fix.
  - A two-line string literal produced by a lost `\` continuation, with an indented second line, is
    reported — and `scripts/fixtures/README.md` no longer states the gate's blindness to it as a
    general truth.
  - `check-doc-links.mjs` is either given the same enumeration or carries a one-line note saying why
    it does not need it. Silence on it is not an answer.

### Phase 8 — Two entries whose premise the store revocation falsified
- **Owner skill:** dev
- **What:** Correct backlog 0160 and 0161 in place. Neither is closed.
- **Files touched:** `docs/design-backlog.md`, `standalone/tests/shot_cli.rs`,
  `packaging/macos/bundle.sh`, `renders/README.md`.
- **Notes for the implementer:**
  - **Read this before editing.** Both entries were filed 2026-08-29 against
    [ADR-0141](../adrs/0141-one-artifact-store-serves-every-lane.md)'s shared store, and
    [ADR-0147](../adrs/0147-the-shared-artifact-store-is-revoked-and-the-linker-stays.md) revoked
    that store the same day. Entry 0160's stated defect — *"a `target/` inside the worktree that no
    redirect reaches"* — **no longer holds**: with each lane writing to its own `target/`,
    `repo_root().join("target")` is the build tree again and the doc comment it convicts is correct.
    Per the architect's rule, an entry whose premise turns out false is **corrected in place and
    stays live**, because a wrong live entry sends the next reader to do work already done.
  - What survives in 0160 is smaller and real: `scratch()` derives a build path from
    `CARGO_MANIFEST_DIR` rather than from where cargo writes, which is fragile if a redirect ever
    returns. `env!("CARGO_TARGET_TMPDIR")` is the fix and is right under any layout.
  - 0161 survives intact — the documentation still says *"never hardcode `<repo>/target` in a script
    or a test"* and `packaging/macos/bundle.sh` still does, on a **release** path. The two
    `renders/` scripts are archived one-offs; a line in `renders/README.md` is an honest resolution
    for those.
  - **Re-run `node scripts/check-backlog-claims.mjs` after editing.** Several probes on these two
    entries were written to go red on delivery.
- **Done when:**
  - 0160 carries a dated update naming ADR-0147 as what falsified its premise, and states the
    reduced claim that remains.
  - `bundle.sh` resolves `target_directory` from `cargo metadata`, the way `plugin-foobar/build.ps1`
    already does.
  - The claim gate exits 0.

### Phase 9 — The image sweep runs again
- **Owner skill:** dev
- **What:** Close backlog 0133. Add the three missing gallery manifest entries so `docs-shots.mjs`
  stops throwing, and move its manifest-vs-`SystemKind` cross-check somewhere that runs without
  rendering.
- **Files touched:** `scripts/docs-shots.mjs`, `docs/images/gallery/**`, and the cross-check's new
  home (`core/tests/` or the CI `links` job).
- **Notes for the implementer:**
  - The guard is **not** the bug — it is doing exactly what its comment says and it caught exactly
    what it was built for. What failed is the cadence: the only thing that executes it is a human at
    a close, and [ADR-0100](../adrs/0100-documentation-images-are-committed-headless-renders.md)
    deliberately keeps the *rendering* out of CI because renders are not byte-reproducible.
  - **The cross-check is pure text.** It reads the manifest and `schema.rs` and needs no GPU, so it
    can fail on the commit that ships a scene instead of silently disabling the sweep for eleven
    days. That is a different claim from "the images are current", which ADR-0100 correctly refuses
    to gate — keep them separate.
  - `shape_field` and `shape_collage` have obvious shipped worlds (`shape_pulse`,
    `collage_suprematist`). `warp_mesh` ships none, so it needs a fixture bundle or a converted
    `.milk` named in the manifest's provenance line like every other entry — that is the Phase 10
    question, so leave a placeholder rather than guessing.
  - **This phase renders.** It needs a free GPU and must not run while a show is live.
- **Done when:**
  - `node scripts/docs-shots.mjs` completes without throwing and re-shoots the eight images that
    have nothing to do with the three missing systems.
  - Adding a thirteenth `SystemKind` with no gallery entry fails a check that runs without a GPU.

### Phase 10 — What the pictures are of
- **Owner skill:** human
- **What:** Decide what `warp_mesh` should be a picture *of*, and look at the re-shot gallery.
- **Files touched:** `scripts/docs-shots.mjs` (the provenance line), `docs/images/gallery/**`.
- **Notes for the implementer:**
  - Four of the nine gallery images (`parametric_curve`, `lsystem`, `star_pattern`, `spectrum`) show
    a stroke the engine no longer draws — Plan 0114 moved the line stroke's default `softness` and
    retuned six line presets. Phase 7 re-shoots them; **this phase is where someone confirms the new
    ones are actually better**, which no script can answer.
  - `warp_mesh` ships no preset. The options are a fixture bundle or a converted `.milk`, and the
    choice is content, not engineering.
- **Done when:** the gallery has twelve images, each with a provenance line, and the user has seen
  them.

## Risks & open questions

- **Phase 4 edits ~20 append-only documents.** ADR-0149 argues a link form is not content, and that
  argument is the load-bearing part of this plan's most reversible-looking phase. If it is rejected
  at review, Phase 4 reverts cleanly and ADR-0149 is superseded; nothing else in the plan depends on
  it.
- **Phase 8 is the one phase that edits the backlog rather than the code**, and its judgement — that
  0160's premise is falsified — could be wrong if a redirect returns. It is written as a dated
  correction rather than an archive precisely so it survives that.
- **Phase 9 needs a GPU and this plan may be taken while shows are running.** Phases 1-8 need
  none — they are Node, markdown and one shell script — so the plan is safely splittable at that
  seam if the machine is busy.
- **The gates repaired here are the ones that verified this plan's own siblings.** Anything Phase 1,
  Phase 3, Phase 5 or Phase 7 turns up may retroactively weaken a finding from an earlier close.
  That is the point, and it should be reported rather than quietly absorbed.
- **Phase 7 changes what every local push scans**, so it is the one phase here whose regression is
  invisible on CI by construction — CI's clone never held the vendored trees. Its done-when is
  written around seeding a gitignored violation for that reason.

## What this plan does NOT do

- **It does not add a general fragment checker.** ADR-0149 handles one class by prohibition and says
  why the general checker got less likely as a result.
- **It does not gate image freshness.** ADR-0100's refusal to put non-reproducible renders in CI
  stands; only the text cross-check moves.
- **It does not close backlog 0160 or 0161.** Phase 8 corrects and reduces them.
- **It does not build a shared enumeration layer for the five gates.** Phase 7 gives
  `check-comment-hygiene.mjs` the `git ls-files` source and asks the same question of
  `check-doc-links.mjs`; the other three walk sets that are already correct, and a shared helper for
  two callers is not obviously worth its own indirection.
- **It does not re-derive the shipped preset roster or re-shoot anything outside the gallery.**
  Phase 9's sweep is the twelve gallery images, not `docs/images/` at large.

## Implementation log

> Written by `dev` — one row per phase as that phase's commit lands, and the close block after the
> last one. **The phases above are the contract; everything here is what happened.**

**Lane:** `plan-0136-the-gates-can-convict` in `WORK/lmv-plan-0136`.

| phase | owner | state | commit |
|---|---|---|---|
| 1 — The row gate asserts its own counts | dev | done | `df9b7a1` |
| 2 — A seeded tree the row gate rejects | dev | done | `f58fa1b` |
| 3 — The row gate reads a row's shape | dev | done | `8986147` |
| 4 — Backlog references stop using fragments | dev | done | `672bdf2` |
| 5 — A probe path is checked against the repository | dev | done | `790e6f8` |
| 6 — The figure gate stops convicting untracked files | dev | done | `6ee06ac` |
| 7 — The gates judge the code this project wrote | dev | done | `0f1ac37` |
| 8 — Two entries whose premise the store revocation falsified | dev | done | committed with this row |
| 9 — The image sweep runs again | dev | not started | |
| 10 — What the pictures are of | human | not started | |

### Notes

- **Phase 1's `--self-test` is wired into the call sites in Phase 2, not Phase 1.** Phase 1's file
  list is the script and the fixtures README; `.githooks/pre-push` and the CI `links` job are named
  in Phase 2's. Both invocations land together there.
- **Phase 3 takes the region's kind from the MAJORITY of its rows, not from its first measured
  row.** Backlog 0166's "What a fix looks like" and this plan's Phase 3 both name first-row
  inference. Measured against the instance the entry itself records - a closed-plan bullet seeded
  immediately under `roster:begin` and above the table header in `docs/plans/README.md`, which is
  where the real one landed - first-row inference reports **14 breaks, none of them the stray row**,
  because the stray row is the one it adopts as the region's form. The majority rule reports one:
  `docs/plans/README.md:27  a bullet row in a table region (expected a table row; 14 of the 15 rows
  in this region have that form)`. A region split evenly has no majority, so it is reported once at
  its own opening line rather than at a guessed row; `index-rows-red/` seeds that case too.
- **Phase 4 re-derived the set as 87 links across 29 files**, against the plan's *"roughly 20 files
  under `docs/adrs/` and `docs/plans/done/`"* and ADR-0149's 24, which is a count of distinct entry
  NUMBERS rather than of sites. Ten occurrences survive and are all inline code spans in prose that
  describes the retired form - `.claude/skills/architect/SKILL.md:550`, ADR-0149's own Context and
  Decision, backlog 0143's body, this plan, and two closed plans.
- **Phase 4 also repaired an asymmetry the new fixture exposed.** `check-doc-links.mjs`'s inline
  regex stops at the `#` and its DEFINITION regex ran the target to whitespace, so a `[label]:
  target#anchor` definition was resolved as a path CONTAINING the fragment and reported as a
  missing file. No such definition existed in the tree, which is why it was green. Seeding class 4
  in both link forms produced two findings on one line; the definition target now drops its
  fragment before the existence check, so both forms answer the same question.
- **`scripts/check-backlog-claims.mjs` held a NUL byte on `main`**, in the advisory's dedup key
  (`${probe.entry}` NUL `${probe.path}`, where a space was meant). git classified the file as
  binary, so `git diff` printed nothing for it and `grep` skipped it. Replaced with a space in this
  phase's commit, since the file is Phase 5's own and the defect makes the gate undiffable. Not
  found by a gate - found by `grep` refusing to search it.
- **The plan's *"must still report its 10 findings"* was 12 before this phase and is 13 after.**
  Plan 0144 added `seeded-literal.rs` with two, and 0173's unrejoined seed adds the third. The
  fixture README carried 12 and was the accurate number; the plan and backlog 0170 both quote 10.
- **`check-doc-links.mjs` got the enumeration rather than a note**, which is the half of Phase 7's
  done-when that permitted either. Not on principle: seeding `.venv/pkg/README.md` with two
  relative links made it report both and exit 1, so the sibling shares the defect outright and was
  green only because neither vendored tree happened to carry a relative-linked `.md`.
- **0173's first probe still holds while its claim no longer does.**
  `present: text\.includes.{0,40}return null` asserts the newline exclusion is *"still
  unconditional and still ahead of the run check"*. The early return is still in the source, so the
  probe matches; the new continuation-indent arm now runs **before** it, so the claim is false.
  Green here means the reduction still matches the tree, which is what the gate's own header says.
- **Phase 8's `renders/README.md` resolution was not taken, and the reason is this plan's own
  subject.** `renders/` is gitignored in full (`.gitignore:54`), so that README is untracked: a line
  in it is absent from every checkout, reaches no reader, and cannot be committed - the same shape
  Phase 5 taught the claim gate to reject one entry earlier. Recorded in backlog 0161 itself
  instead, which is tracked and is where the roster lives. Raised with the user, who chose that.
- **Phase 8's *"the claim gate exits 0"* is met for 0160 and 0161 and not for the gate overall.**
  Both entries' probes are re-pointed at the repaired code and hold. Seven probes remain red across
  the five entries this plan CLOSES - they were written to go red on delivery, and archiving them is
  the close ceremony's act. Raised with the user, who chose that reading.
- **Entry 0104's own probe goes red on delivery**, from Phase 1 onward:
  `absent: self-test in: scripts/check-index-rows.mjs` now matches. Archiving a discharged entry is
  an `architect` act at the close, and the claim gate's own failure text says repairing a falsified
  entry is not a `dev` call - so it is left red and recorded here rather than edited. The
  `Backlog probes` close trigger carries the full roster.
- **`check-backlog-claims.mjs --self-test` is wired into no call site at all.** Phase 1's note holds
  it up as the model to follow and as the thing that must be wired *"or it is a mechanism nobody
  runs, which is the defect being repaired"*. Grepped 2026-09-01 across `.githooks/`, `.github/` and
  `.claude/`: the string `self-test` appears in none of them. Not acted on - it is outside every
  phase's file list.

### Close triggers

- **`presets/` touched:**
- **Plan header `Closes:`** design-backlog 0104, 0143, 0162, 0127, 0133, 0166, 0170, 0171, 0173
- **What shipped:**
- **Operator docs touched:**
- **Backlog probes (`node scripts/check-backlog-claims.mjs`):**
- **Full suite:**
- **Outstanding `human` phases:**

## Followups (after this lands)

- Whether the remaining three Node gates need the `git ls-files` enumeration Phase 7 gives one of
  them.
