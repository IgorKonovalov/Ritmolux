# Plans index

The one-minute "what's in flight" view. Read this first each session instead of
re-deriving state from `git log`. Completed plans move to `done/`; their full
close write-ups move to [README-archive.md](README-archive.md).

**Next free number: 0153** (ADRs are a separate sequence — next free there is **0165**.)

<!-- toc:begin depth=3 -->
- [Active roster](#active-roster)
- [Recommended execution sequence](#recommended-execution-sequence)
  - [The two lanes, now](#the-two-lanes-now)
  - [Then, in this order](#then-in-this-order)
  - [What this sequence assumes](#what-this-sequence-assumes)
  - [The baseline-drift control any pixel-touching plan inherits](#the-baseline-drift-control-any-pixel-touching-plan-inherits)
  - [The six plans added 2026-08-04, and why they exist](#the-six-plans-added-2026-08-04-and-why-they-exist)
- [Standing (not a plan)](#standing-not-a-plan)
- [Recently closed](#recently-closed)
- [Roadmap (agreed 2026-07-21, revised same day for the live-show use case; numbers assigned when drafted)](#roadmap-agreed-2026-07-21-revised-same-day-for-the-live-show-use-case-numbers-assigned-when-drafted)
- [Conventions](#conventions)
<!-- toc:end -->

## Active roster

Only plans still in `docs/plans/`. A closed plan leaves this table entirely —
`Recently closed` below and `done/` both already record it. Each row carries at
most two sentences of **live constraint**: what a reader needs to decide whether
to pick this plan up. Anything longer belongs in the plan file, which is where
someone who picked it up is reading.

**These rows are now inside a `roster:begin cap=320` region and
`scripts/check-index-rows.mjs` holds them to it** — the convention above stood
alone until 2026-08-29 and the rows had regrown to a 893-byte mean, 2.8x the cap
the closed-plan bullets below were already held to in this same file. That is
ADR-0116's own argument, and the repair was to extend its markers rather than to
restate the rule. **Cite ADRs by bare number here** (`ADR-0131`, not a link): the
slug filenames run past 100 bytes and are what pushed rows over in the first
place. The plan file carries the real link.

<!-- roster:begin cap=320 -->
| Plan | Title | Status | Owner | Live constraint |
|------|-------|--------|-------|-----------------|
| [0120](0120-the-standalone-ships-on-ubuntu.md) | The standalone ships on Ubuntu | approved | dev, human | ADR-0131 (proposed): a PulseAudio capture arm plus an `ubuntu-latest` CI arm. **Phase 1 is a `human` stop gate before `dev`** — only one of its three outcomes lets `dev` start. |
| [0092](0092-the-engine-draws-an-authored-path.md) | The engine draws an authored path | approved | dev, human | Hard dependency discharged: 0091 closed, and `shape_field` is the scene this draws into. Takeable even if 0087 stalls — Phase 4 may legitimately be empty. Expect morph degeneracy. |
| [0103](0103-the-project-gets-an-audience.md) | The project gets an audience | approved | dev, human | **A new Phase 1 fixes backlog 0102 + 0103 before anything advertises the component** — foobar's UI starves until playback starts. **Phases 4-6 unblocked, 0150 closed.** |
| [0128](0128-the-rendered-file-stops-looking-upscaled.md) | The rendered file stops looking upscaled | approved | dev, human | Backlog 0110 + 0130. ADR-0140 (proposed): drawn count becomes a density against the render target, **anchored so it can only add samples** — a moved golden is a finding. **Gates 0103.** |
| [0133](0133-the-engine-drives-the-lights.md) | The engine drives the lights | approved | dev, human | **Supersedes 0132's architecture, which a live set on 2026-08-29 bypassed entirely.** ADR-0145 (proposed): Art-Net straight to the fixtures. Phase 8 hard-depends on 0115 Phase 2; 1-7 do not. |
| [0138](0138-the-colour-surface-stops-misleading-its-authors.md) | The colour surface stops misleading its authors | approved | dev, human | Backlog 0153 + 0099. ADR-0151 (proposed): stops become sRGB, migrated so no golden moves. Phase 1 is a free doc fix. |
| [0140](0140-every-rate-integrates-for-real.md) | Every rate integrates, for real | approved | dev, human | Backlog 0149 + 0150 (**0142 carried**). ADR-0152 + 0153 (proposed): `dt` sanitized at the scene seam, per-element rates integrate per element. Phase 3 moves goldens; Phase 2 must not. |
| [0142](0142-the-milkdrop-import-earns-its-verdict.md) | The MilkDrop import earns its verdict | approved | dev, human | Backlog 0113 (**the only High**) + 0124. Fixes the wash, then writes ADR-0113's third Outcome. **The verdict decides whether backlog 0109 is buyable.** Needs the reference rig. |
| [0143](0143-the-documentation-gets-a-front-end.md) | The documentation gets a front end | approved | dev, human | ADR-0154 (proposed): docs publish as a Starlight site, `docs/` stays the source, 926 of 1,059 links rewrite at build time. **Unparked, 0150 closed** — pick the Pages subpath. |
| [0147](0147-what-the-show-costs-and-what-its-numbers-mean.md) | What the show costs, and what its numbers mean | approved | dev, human | Backlog 0164 + 0163; 0154 half, 0165 update. The console halves output fps and two comments deny it. **Phase 4 is a hands-off window.** Phase 1 precedes 0133. |
| [0152](0152-the-osc-root-becomes-rlx.md) | The OSC root becomes `/rlx` | approved | dev, human | ADR-0164 (proposed): discharges ADR-0162's deferred decision. One clean break, `/v1` unmoved. **Phase 5 is `human`** — every rig binding re-pointed by hand, and the break is silent. |
<!-- roster:end -->

**Added 2026-09-04 — [0152] is approved, and it runs before [0133] and [0147] rather than after.**
It moves the last operator-visible surface still carrying the old name: `ADDRESS_PREFIX` at
`standalone/src/osc.rs:52`, left standing by [0150] because that plan's greps could not match a
token followed by `/`. ADR-0164 is the decision ADR-0162 deferred — one clean break, no dual-emit,
no transition period, and `/v1` does not move because no payload did. Both 0133 and 0147 are
approved, unstarted, and name `/lmv/v1` in live text a `dev` lane would read; 0152's Phase 3 is what
re-points them, so taking 0152 first removes that window instead of closing it afterwards. Nothing
else contends: [0151] closed 2026-09-04, and the backlog preamble it rewrote is not a region 0152's
Phase 3 touches.

**Phase 5 is the operator's, and it is the only detector.** OSC has no negotiation and no error
channel, so a binding left on the old root stops firing and looks exactly like a fixture that is not
moving. Schedule that phase against a rig session with a playing track, and keep the old show file
until all fifteen addresses are confirmed.

[0152]: 0152-the-osc-root-becomes-rlx.md

~~**Added 2026-09-02 — [0150] is the rename, and it is a queue rather than a plan that slots in.**~~ — **closed 2026-09-02**, all nine phases, and the freeze held for every one of them. **[0143] and [0103] Phases 4-6 are unparked**: the repository is `IgorKonovalov/Ritmolux`, so 0143 may now choose its Pages subpath and 0103 may submit the component. The original note follows, since its reasoning is what made the freeze non-negotiable.

ADR-0162 chose Ritmolux; the plan sweeps 1,318 live sites across every crate and so cannot be merged
against a parallel branch. Its Phase 1 is a `human` stop gate that does not release `dev` until
`git worktree list` prints one line — and **no lane opens between that gate and Phase 9.** That
gate's other half, the trademark check, was discharged 2026-09-02: a knockout search found no
`Ritmolux` on any register, and the risk of stopping there was accepted for a non-commercial
project. **Both halves are clear as of 2026-09-02**: [0149] and [0148] closed and their lanes were
removed, so this plan is next and runs on `main` directly.

**Added 2026-09-01, from a backlog round after the closes of 0124, 0125, 0139, 0141 and 0144-0146**
— three new plans and one amendment, taking 21 of the ~30 live entries no plan claimed. The round's
shape, because most of what it decided was sequencing rather than design:

- **[0149] must run before [0126]** — **discharged: [0149] closed 2026-09-02.** They contended on two files: 0149's Phase 3 edits `star.rs` and
  its Phase 5 edits `schema.rs`, and 0126 **splits both**. A pure move of code that is about to
  change is the move done twice, and 0126's phases are gated on golden while 0149's Phases 2a and 2
  each deliberately re-bless the non-square line baselines. They must not run in parallel in any
  order, and the contention got worse when 0149 gained Phase 2a on 2026-09-01: `renderer.rs` and
  both its WGSL modules are now in scope too.
- **[0147]'s Phase 1 wants to land before [0133] is built.** Backlog 0163 is one sentence of prose,
  and 0133 brings in-house the exact consumer that was misled by its absence — a lighting look
  multiplying a band term into a physical output. The rest of 0147 does not gate 0133.
- ~~**[0148] was the free one, and it is now the only lane open.**~~ — **closed 2026-09-02**, all six
  phases. Phase 5's method constraint — no other lane building while the size series is taken — was
  satisfiable only in the window after [0136] and [0149] closed, and it was taken in that window.
  Its finding is that 66.7 % of the component's growth is embedded preset text, which is now in
  `docs/specs/0001-c-abi.md` and in ADR-0159's Outcome. **The lane is removed, so [0150]'s freeze
  gate was clear**, and [0150] has since closed and released [0143] and [0103].
- **The gate entries folded into [0136] rather than becoming a fourth plan**, which was the user's
  call at the interview: a second lane over `scripts/check-*.mjs` would contend with 0136 on the
  same six files for no benefit. That amendment took it from 8 phases to 10 and **falsified its own
  closing claim** — it said it did not touch `check-comment-hygiene.mjs`, *"the one gate in
  `scripts/` with no live complaint against it"*, and backlog 0170 and 0173 were filed against that
  gate the day after. Repaired in the same edit. **0170 blocks `git push` for everyone whose working
  tree holds `.venv/` or an unpacked SDK**, which makes it the most urgent thing in that plan.
- **Two entries were promoted only in part, deliberately.** Backlog 0154 gives up its *verdict* fix
  and keeps its mechanism question, because choosing between retry-in-place and a long-lived
  enumerator wants unplug evidence the box cannot produce. Backlog 0165 gives up its measurement
  half; whether the console's dual-GPU degrade path is reachable stays open, and 0147 Phase 6 is
  written so that "it stayed unexercised" is a recordable finding rather than a failure.
- **What stayed filed, and why.** The engine/content entries no plan here takes — 0140 (the band
  contour), 0146 (`warp_mesh` colours at deposit), 0100, 0101, 0095, 0092, 0069 — are look-affecting
  and larger, and several price themselves as a redesign of the composite. Backlog 0021 and 0032
  remain parked with named triggers. **0157 and 0158 are not unclaimed** despite reading that way
  from the roster: [0133]'s Phase 2 closes both.
- **One design premise was reopened and one was left alone.** [ADR-0158] supersedes the *geometry*
  half of ADR-0041 because that ADR rejected a true miter on the ground that *"a mitred corner and a
  rounded one differ by less than the blur that is already there"* — and Plan 0114 took
  `DEFAULT_SOFTNESS` to `0.25`. Its per-endpoint *granularity* stands and is what makes the fix
  cheap. [ADR-0159] settles what backlog 0177 filed rather than answered — the plugin's own cap —
  and does **not** touch the standalone exe's, which keeps its inherited value and gains only a unit.

[0143]: 0143-the-documentation-gets-a-front-end.md
[0147]: 0147-what-the-show-costs-and-what-its-numbers-mean.md
[0148]: done/0148-the-shipped-artifacts-carry-their-own-guarantees.md
[0149]: done/0149-the-line-corners-stop-being-blunt.md
[0150]: done/0150-the-application-becomes-ritmolux.md
[0151]: done/0151-the-long-documents-become-navigable.md
[ADR-0158]: ../adrs/0158-a-joined-end-carries-its-own-miter-length.md
[ADR-0159]: ../adrs/0159-the-component-gets-its-own-size-cap-and-the-recipe-carries-it.md

**Added 2026-08-19, from a MilkDrop backlog round after
[0109](done/0109-the-milkdrop-import-gets-its-geometry-back.md)'s close:
[0111](done/0111-the-milkdrop-import-stops-washing-out.md) (**closed 2026-08-20**).** Six live entries came out of the import;
the round split them on one line and took one side:

- **The four fidelity entries went into one plan** — 0113 (the wash), 0119 (the `ang` seam), 0120
  (the waveform scale) and 0121 (the `decay` fallback's units). They share four files, so three
  separate plans would have contended; and 0121 goes **first** inside the plan rather than last,
  because it is what silently corrupted Plan 0109 Phase 4's own instrument and every measurement
  after it is worth less until it lands.
- **The two reach entries stayed filed, by their own argument.** Backlog 0109 (disk textures — 1 826
  files, 88.7 % of every conversion failure) says to take it only once the fidelity work has settled
  whether converted presets are worth having more of. Two look gates have now answered that with
  *"still merely different"* (Plan 0100 Phase 7, Plan 0108 Phase 2), so the answer is not yet yes.
  0111's Phase 6 was to ask it a third time and **did not run — it was void, because that plan
  changed nothing a converted preset renders** (see its Phase 6 section). The trigger for planning
  reach is therefore still unbought, and it now rides on the successor to backlog 0113. Backlog
  0108 (the conversion tail) is 25x smaller than 0109 by its own arithmetic and waits behind it.
- **One thing the authoring turned up and the plan carries:** `gamma` is applied as a **linear
  multiply** in the present shader while being named for MilkDrop's `fGammaAdj`, so a preset at
  `fGammaAdj = 1.9` takes an unclamped 1.9x linear gain into the tonemap. It is a **lead and not a
  diagnosis** — the highest-gamma preset in the judged set reads fine and one that washes sits at
  unity — which is exactly why Phase 2 measures the seam rather than starting from it.

**Added 2026-08-16, from a backlog round after [0091](done/0091-the-figure-fills-the-frame.md)'s
close: [0098](done/0098-the-figure-nests-properly.md) and [0099](done/0099-the-horizon-reaches-its-own-length.md)
(**closed 2026-08-16**), plus one fold.** The round swept all 17 live backlog entries and promoted four of them; the sweep
result is worth keeping because most of what it found was **not** ripe:

- **[0098] takes backlog 0096 + 0097**, which came out of the same content pass and sit on the same
  two files. Its ADR-0111 is the real decision — a second *coordinate*, not a second shape — and its
  Phase 1 is a standalone defect fix that would be worth doing even if the rest were abandoned.
- **[0099] takes backlog 0093**, and is deliberately shaped so its cheapest phase runs first: the
  entry already names the one command that discriminates between the two candidate causes, and
  starting from the one-line hypothesis instead would fix a symptom without establishing which
  ceiling was removed. **That shaping paid: it closed 2026-08-16 with a third answer neither
  candidate predicted** — the wall was memory pressure, not a frame count, and retention was per
  *pass*, so every world grew and RD merely reached the allocator first.
- **backlog 0098 folded into [0087] as Phase 1b** rather than becoming a third plan — same
  subsystem, no other plan touches those files. **Placed before 0087's Phase 4 stop gate on
  purpose**, since that gate can send the whole plan to ADR-0098's Alternative C and this repair is
  independent of whether arcs ever ship.
- **Everything else stayed filed, and mostly by its own instruction.** Backlog 0021 and 0032 are
  parked with named triggers; 0038 and 0075 are content-lane items already in the brief; 0069 is
  Low and prices itself as a composite redesign; 0071 and 0073 belong to the curve-primitive family
  [0087] owns; 0079 and 0094 are advisory; 0087 (the RD glow entry) carries an explicit *park*
  verdict awaiting a second cohort; 0095 was filed hours earlier at 0091's close.
- **One entry's trigger fired and could not be acted on: backlog 0092** (every figure is unlit). It
  says to take it if Plan 0091's look gate found the flat sparkle disappointing. The gate ran, the
  stars *were* rejected — and **the reason was never captured**, which is the half the plan actually
  asked for. A rejection on silhouette points at [0098]'s coordinate; a rejection on shading points
  at lighting. One short look call settles which, and it is the open question in the content brief.

**Parked behind the whole roster, by the user's own instruction:
[ADR-0112](../adrs/0112-a-blender-model-enters-as-inline-mesh-data-and-the-gpu-scatters-its-points.md)**
(2026-08-16) — a Blender-authored model enters as inline mesh data and the GPU scatters the tier's
particle budget across its surface. It has **no plan and is not to get one until this roster
clears**, on the [ADR-0102](../adrs/0102-a-palette-coordinates-edge-is-a-per-preset-choice.md)
precedent: the decision was worth recording while the reasoning was fresh, nobody is blocked, and
the interview that produced it is the expensive part. Two things a future plan owes before its
triangle ceiling is fixed, both capable of invalidating the ADR: what triangle count a *recognizable*
decimated silhouette needs (if a hard-surface model needs thousands, the inline arithmetic collapses
and the ADR's Alternative E becomes live), and whether uniform area sampling reads at all — which
should be Phase 1, the author's own model on screen and untuned, because that is the cheapest moment
to learn that weighted sampling was never optional.

**Sequencing: both new plans run after [0087] and [0092].** [0098] contends with [0092] on
`shape_field.rs` and [0087]'s stop condition is worth resolving before more line-adjacent work; the
choice was the user's at the planning interview. [0099] contended with nothing and did not have to
wait — **it was taken by a free session and closed 2026-08-16**, which is the note working exactly
as intended.

**Five plans, written 2026-08-13 from a backlog sweep**, after the roster stood empty for the first
time in this file's history — **two now: [0083] and [0084] both closed the same day they were
written, and [0085] closed 2026-08-15**. [0088] arrived from a user request after the sweep and
**also closed the same day**. They
are ordered above roughly smallest-and-most-urgent first; see the
sequence note below. Three carry new ADRs
([ADR-0097](../adrs/0097-the-downbeat-cue-is-chosen-against-per-beat-evidence.md),
[ADR-0098](../adrs/0098-the-line-renderer-draws-arcs-as-per-pixel-distance-fields.md),
[ADR-0099](../adrs/0099-the-show-length-horizon-is-a-spot-check-and-it-splits-in-two.md)), and every
one of the five closes backlog entries that had been sitting on a demonstrated want with no route.

**Added 2026-08-13, from a second backlog pass: [0089](done/0089-the-framing-contract-stops-lying.md) and
[0090](done/0090-the-emitters-source-moves.md)**, which between them take the five items the first sweep
left unrouted — **both now closed, 2026-08-15**. [0089] is the three-item sitting (a falsified
invariant plus two doc paragraphs that each named a home and never got a carrier); [0090] came out of
an interview on the emitter's fixed source line
([backlog 0068](../design-backlog-archive.md),
option 2, **closed and archived at 0090's close**) and shipped four scalars; the world they exist for
is its `human` Phase 5 and stands. One item from that pass is deliberately **not** a plan:
[ADR-0102](../adrs/0102-a-palette-coordinates-edge-is-a-per-preset-choice.md) records the
palette-coordinate edge decision with no plan behind it — the want is real, nobody is blocked, and it
is built when a look asks.

**Two items stay parked and are not being planned**, which is the honest half of the sweep:
[backlog 0021](../design-backlog.md) (the slew release, waiting on an author who wants the look rather
than on an architect's arithmetic) and [backlog 0032](../design-backlog.md) (both analysis windows
sized in samples, so 21 of 64 bands are bin-starved at 96 kHz — pinned by a test, ADR territory,
waiting on someone reporting a mushy low end on a 96 kHz interface).

## Recommended execution sequence

**Rewritten 2026-08-18, and this is the live sequence.** What it replaced — the 2026-08-16
sequence and the prior sequence notes under it — is in
[README-archive.md](README-archive.md) under `## Prior sequencing notes (superseded)`.
Four calls set it: the next stretch is **engine and visual richness**, work runs
in **two lanes**, [0103] waits for [0104], and [0087] goes **early to de-risk** rather than late
because it is large.

**Added 2026-08-29, on show day: [0131] and [0133] are approved, and the order below is the
user's call.** Both were `draft` carrying proposed ADRs, and neither could serve the show that
evening — 0133 Phase 1 needs an evening with the rig patched and 0115 Phase 1 needs the Spout SDK
staged. **The set runs on the external Python bridge** (`WORK/lmv-lighting-probes/`, outside version
control), unmodified. Approving the plans starts the work; it changes nothing about the show.

**Added 2026-08-31, updated at [0145]'s close the same day: [0145] landed and [0146] is next.**
[0145] was tooling, and every plan queued above was paying the old gate until it landed. **What it
actually bought, measured over six runs per arm on a verified-idle box: 24.8 min on a median
six-phase plan (49.6 -> 24.9 min, 49.9 %)** — half the ~59 min projected here from the architect's
single pair, because the full suite is 446 s and not the 869 s that pair recorded. The critical-path
mechanism reproduced exactly; only the magnitudes moved. See
[ADR-0156](../adrs/0156-the-per-phase-gate-is-scoped-and-the-suite-is-owed-once-per-plan.md)'s
`Outcome`.

**[0146] was sequenced after it and was not a time plan. It landed 2026-08-31, and what it bought
is not what this paragraph projected.** The projection was *"adds ~3 min per plan on top; what it
buys is the CI run on every push (869 s -> ~539 s, modelled)"*. Measured at Phase 7: the per-phase
tier costs **+58.5 s**, so **~5.9 min** on a median six-phase plan, and the full suite fell only
**464.2 s -> 435.6 s (6.2 %)** rather than to ~539 s from 869 s, because the split trades work for
schedulability at close to par. **The CI half is backwards for one of the three jobs**: `check`
cites `-P fast` and therefore *gains* the 72-test sample rather than shedding time. What the plan
unambiguously bought is the tail, per-preset failure attribution, a **zero marginal cost per new
preset** in the implementation loop, and per-phase coverage of 24 presets where ADR-0156 had left
it at none. Its Phase 1 spike tripped its own stop condition and the architect replaced the
condition rather than the result — see the plan's `### Notes` and ADR-0157's `Measured correction`.

**Added 2026-08-29, second promotion round: [0138], [0139], [0140], [0141] and [0142], all `draft`.**
The sweep was re-run with a corrected filter — the first pass's regex was greedy and over-counted
claimed entries — leaving **32 of 58 unclaimed**. Five clusters came out; three entries were
**declined on their own instructions** rather than promoted:

- **[0141] is the one with no contention.** It is the only cluster touching `plugin-foobar/`, which
  no plan on this roster otherwise enters. Its Phase 1 is the exception: backlog 0117 calls itself a
  natural pickup for [0103] Phase 1, which rewrites the same handler.
- **[0138] belongs immediately after [0137]** — same linear-light seam one layer up. Taking them
  together is one coherent pass; six weeks apart is two half-passes. [0138] also makes backlog 0038
  measurable for the first time, using the level statistic [0137] adds. **[0137] closed 2026-09-01,
  so `mean_lit_level` and the `--report` `level` column are on `main` and this is now the moment.**
- **[0142] is the least show-compatible plan on the roster.** Three of its six phases need a free
  GPU and the `foo_vis_milk2` rig staged. It carries the backlog's **only High**.
- **[0140]'s contention with [0125] is discharged — 0125 closed 2026-08-31.** It still edits five
  scenes, whose rate params now sit beside `scenes::common`'s colour and framing blocks.
- **Declined, and the record is the reason.** Backlog 0038 is routed to `preset-author` as a content
  pass — *"no engine change and no ADR"* — and is §4 of `content-brief.md`. Backlog 0075's remaining
  half is ADR-0102, **proposed with no plan by the user's call**, holding until a look asks for the
  clamp. Backlog 0109 (1,826 files, 88.7 % of conversion failures) **forbids being taken now**:
  *"Do not take it before Plan 0108's Phase 2"*, whose verdict has twice read "still merely
  different" — so [0142] Phase 6 decides whether it is buyable, and does not take it.

**Added 2026-08-29, from a backlog-promotion round: [0135], [0136] and [0137], all `draft`.** The
round swept the 58 live entries, found **26 already claimed by some plan** and promoted three
clusters out of what remained. Deliberately filed *behind* the existing roster — the user's call —
because eleven plans were already active and the roster, not the work, was becoming the bottleneck.
Sequencing:

- **[0136] is the one to take first, and it is takeable during a show.** Phases 1-6 are Node,
  markdown and one shell script; only Phases 7-8 render. It also repairs the instruments the other
  two are verified with — `check-index-rows.mjs` currently cannot fail, so a detector matching
  nothing exits 0 at all three call sites.
- ~~**[0135] contends hard on `standalone/src/main.rs`**~~ — **closed 2026-08-30 on Phases 1-4**,
  taken *before* [0126] Phase 7 rather than after it, and the predicted rebase never happened:
  `main` was already an ancestor of the lane at the close. It landed ~690 lines into `main.rs`, so
  **[0126] Phase 7 and [0133] now split or contend with a larger file than either was sized
  against** (0126 closed 2026-09-03 having done so - `main.rs` was 4,525 lines by then, not the
  1,692 the plan assumed, and is 37 now; [0133]'s `Files touched` are repointed at `app_state.rs`) — the roster, the `--help` renderer and the four scan helpers are one contiguous,
  self-contained block beside the config helpers, which is the seam to move them on.
- ~~**[0137] contends with nothing**~~ — **closed 2026-09-01**, and the prediction held: no
  contention, no golden moved, no floor moved. Its Phase 6 re-measurement falsified the plan's own
  figures over an 81-preset library rather than the 54 it assumed, which the log records.
- **Two entries were corrected rather than promoted.** [Backlog 0160](../design-backlog.md)'s
  premise was falsified by ADR-0147 the same morning it was filed, and 0161's severity dropped with
  it; [0136] Phase 6 corrects both in place rather than planning work on a false premise.
- **The round also found archive debt**: Plan 0111 closed 2026-08-20 declaring it closes backlog
  0119 and 0120, and neither was ever archived. Not taken by any plan here — it is close-ceremony
  bookkeeping, and it needs a judgement about whether they were genuinely discharged.

- **Two lanes open now, in worktrees.** **[0104]** in a `preset-author` lane — it touches
  `presets/*.toml` only, so it contends with no Rust lane on this roster. **[0115]** in a `dev`
  lane — its Phase 1 is a `human` stop gate that writes no code, so the lane opens before any
  decision about Spout is made.
- **[0133] and [0131] follow [0115].** That plan's Phase 2 frame tap is what [0133] Phase 8
  hard-depends on. Note what the ordering does and does not buy: Phases 1 to 7 of 0133 — the
  lights themselves — depend on nothing in 0115, so this sequence buys the **picture** path, not
  the lighting path.
- ~~**Three of the four contend on `standalone/src/main.rs`.** Run them in series, **0133 before
  0131**~~ — **discharged 2026-08-30.** [0115] and [0131] both closed on `main` rather than in that
  lane, and neither ran through [0126] Phase 7 first. `main.rs` is now 3,440 lines and carries the
  console, so **[0126] Phase 7 rebases onto both**, which is the cost the bullet below predicted.
- ~~**[ADR-0141](../adrs/0141-one-artifact-store-serves-every-lane.md) applies to both open lanes.**
  The shared artifact store serializes on cargo's lock~~ — **withdrawn 2026-08-29.**
  [ADR-0147](../adrs/0147-the-shared-artifact-store-is-revoked-and-the-linker-stays.md) revoked the
  store, so lanes no longer serialize and no longer share artifacts. What comes back with it is
  ADR-0053's disk cost: each lane carries its own `target/` again, so **remove a finished lane's
  worktree**.

**Added 2026-08-28, from a "what next, functionally" round after the whole-codebase review's three
plans ([0124]/[0125]/[0126], which move no pixels): [0127](done/0127-the-picture-stops-depending-on-the-volume-slider.md)
— **closed 2026-08-28** — and [0128](0128-the-rendered-file-stops-looking-upscaled.md).** Both take defects in already-shipped
output rather than adding capability, which is why they were picked ahead of the four other
functional candidates the round surfaced (backlog 0042's bar gate, 0126's per-track variety, 0142's
2x dissolve, and the limited-ink cohort behind [0123]). Sequencing:

- **They contend with nothing on the current roster** and share no files with each other — 0127 is
  `core/src/dsp/` plus `warp_mesh`'s draw layer, 0128 is `tier.rs` plus the particles scene. Either
  can be taken by a free session, and they can run in parallel lanes.
- **[0128] goes before [0103]'s outreach phases.** Demo material made before the density law lands
  shows the engine at its grainiest, which is the same dependency [backlog 0110](../design-backlog.md)
  states in its own priority line.
- ~~**[0127] is the one to take first if only one is taken**~~ — **taken and closed 2026-08-28**, so
  [0128] is what is left of this pair. 0127's Phase 3 capture also left a number 0128 does not need
  but the next `warp_mesh` plan will: the reference draws a unit-scale mode-6 trace at 0.316 frame
  heights against our 0.3019, which is [backlog 0120](../design-backlog.md)'s whole remaining gap.
- ~~**Neither waits on [0124]/[0125]/[0126]**~~ - **discharged 2026-09-03: all three are closed.**
  0125 retired the GPU boilerplate across the 12 scenes on 2026-08-31 and 0126 split the seven large
  files on 2026-09-03, so [0128]'s `core/src/render/scenes/particles/` contention is gone.

**[0129](done/0129-the-build-stops-being-paid-three-times.md) closed 2026-08-29, and every plan on
this roster is the beneficiary.** A lane that has never built now compiles **3 workspace crates in
~24 s with zero dependencies recompiled**, against 129 crates in 105 s cold — so *"sequence this
plan behind that one to reuse its `target/`"* is no longer an argument for anything, and the cold
build is no longer a reason to keep a finished worktree around.

- **The setup is machine-local and opt-in.** `WORK/.cargo/config.toml`, outside every checkout,
  never committed; a machine without it builds into its own `target/` and every command is
  unchanged. `CLAUDE.md` carries the whole file.
- **Two things a plan must now assume.** `cargo clean` in any lane wipes the store for **all**
  lanes, and two lanes building at once **serialize** on cargo's lock — the single
  [ADR-0053](../adrs/0053-plan-lanes-run-in-git-worktrees.md) positive that
  [ADR-0141](../adrs/0141-one-artifact-store-serves-every-lane.md) knowingly revokes. A plan
  arguing from parallel lanes needs a different argument.
- **The `opt-level` question is closed, not deferred.** Phase 6 measured our unoptimized code at
  **19.1 %** of the `reactivity` suite — the minority arm, so ADR-0033's ratchet derivation is not
  reopened and no ADR is owed.

**Superseded 2026-09-04 by [0151]'s close — the layout it settles is now on `main`.** [0143] can be
written against final headings: `capturing.md` and `presets/README.md` each carry a generated
contents block, and `scripts/toc.mjs --check` holds it to the headings beneath it
([ADR-0163](../adrs/0163-a-long-document-carries-a-generated-contents-block.md)). The note that
sequenced the two is in [README-archive.md](README-archive.md).

### The two lanes, now

- **Lane A — [0110](done/0110-the-shader-surface-stops-being-invisible.md) is closed
  (2026-08-19), and its baseline is on `main` but not yet pushed.** Its Phase 6 — the CI reading
  that is the whole point — runs on the user's next push; the close projected **~92.3 %** against
  the floor of 91 from CI's own per-file table, so the `coverage` gate is expected to go green
  without further work. **Lane A’s successor
  [0109](done/0109-the-milkdrop-import-gets-its-geometry-back.md) closed 2026-08-19, so the lane is
  free.**
- **Lane B — [0087], startable now.** It touches `core/src/render/scenes/lines/` and one warning
  in `core/src/preset/schema.rs`; Lane A touches neither. **It goes early on purpose:** its Phase 3
  cost measurement and Phase 4 look gate can send it to
  [ADR-0098](../adrs/0098-the-line-renderer-draws-arcs-as-per-pixel-distance-fields.md)'s
  Alternative C, and two other plans carry phases scoped as if it lands
  ([0092](0092-the-engine-draws-an-authored-path.md) Phase 4, [0104] Phase 4). Learning that late
  wastes work written around it.

> **Amended 2026-08-25 — Lane B changed hands.** 0087 reached its Phase 4 look gate and cleared both
> its gates (the cost stop did not fire; the verdict green-lit Phase 5), so the de-risking this
> lane was sequenced early for is **done** and the answer 0092 and 0104 were waiting on exists:
> arcs shipped, Alternative C not taken. But the same verdict named a second defect — the stroke
> reads blurred — and that is [0114], which owns the same directory. **0087 parks at Phase 4 and
> Lane B runs 0114**, so its Phase 5 biarc chain is judged on the final stroke instead of through
> the defect. Phases 5, 6 and 7 of 0087 stay green-lit and unbuilt on a branch whose gate is green.
>
> **Amended 2026-08-26 — the park is discharged and Lane B is free.** [0114] closed with all ten
> phases and merged; because its lane was branched off `plan-0087-arc-primitive`, **0087's phases
> 1-4 reached `main` on the same merge**. So 0087's remaining work resumes from `main` rather than
> from its own branch, and its Phase 5 is now judged on the shipped stroke, which is what the park
> was for. The `WORK/lmv-plan-0087` worktree is stale from here on — take a fresh lane.

**The one rule these two lanes need, and it is not obvious.** Both end at the golden corpus — Lane A
adds a baseline, Lane B re-blesses 28 — and `RLX_BLESS` rewrites every baseline the run renders, not
only the intended ones. Worktrees keep that isolated while the lanes are live, so the collision is at
**merge**, not at bless: **[0087] merges `main` and re-blesses only after [0110]'s baseline is on
`main`** — which it now is, as of 2026-08-19 — then checks its diff carries only its own 28. Taken
in the other order, a bless silently reverts the new fixture and nothing fails. **The baseline to
watch is `core/tests/golden/warp_mesh_shader.png`** — the newest entry, and the one a re-bless
would revert most quietly.

> **Resolved 2026-08-25, and the collision never materialized.** 0087 merged `main` at its mid-plan
> review and **no baseline moved** — the arc primitive reaches only `circle` and `arc` motifs, and no
> shipped preset declares a `rings` roster at all, so nothing in the golden corpus draws one. The
> "28" above was stale on its own terms as well: `golden.rs` renders 18 (11 systems + 7 extras) of
> 33 baseline files. `warp_mesh_shader.png` is intact. The rule stands as written for the **next**
> lane that blesses; it cost this one nothing.

### Then, in this order

1. ~~**[0098]**~~ — **closed 2026-08-27**, which discharges what 2 and 4 below were waiting on.
2. **[0092](0092-the-engine-draws-an-authored-path.md)** — **unblocked**: it rewrites
   `core/src/render/scenes/shape_field.rs`, which [0098] has now finished with. Take it from the
   post-close `main`, not from a base predating it — that file gained a `coord_mode` branch and a
   `rotation` term. Its Phase 4 reads [0087]'s outcome, which **now exists**: the arc primitive
   shipped and ADR-0098's Alternative C was not taken, so a polyline distance field is not the only
   route.
4. **[0104]** — ~~once [0087] and 0098 have resolved~~; **both closed 2026-08-27, so it is
   unblocked in full.** Phase 2's `shape_field` cohort now has two coordinates to author against and
   a `rotation` lever, and [0087]'s Phase 4 means `star_pattern` can be authored on the arc
   primitive. `star_mandala_bordered` is the worked example of what that surface reaches. Every
   phase is a `preset-author` session in `presets/`.
5. **[0103]** — last. **Decided 2026-08-18: it waits for [0104]**, which closes the disagreement
   this section carried open since 2026-08-16. The cost being avoided is announcing into a library
   where four of eleven systems have one world each; 0101's Phase 5 adds a second reason to hold the
   demo material, a 1080p render still reading as an upscale
   ([backlog 0110](../design-backlog.md)).

**Added 2026-08-28, from a whole-codebase review (layering, god modules, hot-path safety, doc
drift): [0124](done/0124-the-review-fixes-that-move-no-pixels.md) →
[0125](done/0125-the-scenes-share-their-gpu-boilerplate.md) →
[0126](done/0126-the-large-files-split-along-their-seams.md), in that order and not in parallel.** The
review found no blocker — layering, the audio callbacks, the C ABI and determinism all came back
clean — so these are a maintenance lane, not a feature one, and they **interleave with the roster
above rather than displacing it**: 0125 and 0126 rewrite the scene files and must not run alongside
a plan that also touches them (0092 on `shape_field`, 0123 Phase 3 on `schema.rs`). The three are
ordered so each inherits the previous one's instrument — 0124's harness and widened gate, then
0125's helpers, then 0126's splits of the now-smaller files. Every phase in all three is
golden-identical unblessed; a bless anywhere in this lane is a finding.

**The whole maintenance lane is discharged: [0124] closed 2026-08-30, [0125] 2026-08-31 and
[0126] 2026-09-03**, and the lane's central property held end to end - **nothing was blessed
anywhere across all three**, so the seven oversized files were reorganized without moving a pixel.
The paragraph below is kept as the record of what 0126 inherited. 0125 landed with **nothing blessed** anywhere across its five
phases, which is the property this lane's ordering exists to protect — 0126's splits are pure moves
and inherit the same rule. Two things 0126 still inherits that are not what the plan promised.
`core/tests/common/` holds the ADR-0016
skip once, but **eleven** files still carry an inline copy inside a bespoke `capture_at`-shaped
function (`arc_cost`, `attractor`, `backdrop_palette`, `backdrop_ramp`, `background_composite`,
`beat`, `collage_cost`, `field_cost`, `mark_cost`, `palette_contour`, `reaction_diffusion`) — so a
new test can still be written by pasting. And `check-comment-hygiene.mjs` now walks `.c/.h/.cc/.cpp/.hpp`
as well as `.rs`, which puts `foo_ritmolux.cpp` under the gate for the first time; 0126's Phase on that
file is the one that meets it.

### What this sequence assumes

- **[0087] failing at its stop condition is the live risk, and it is priced rather than hedged.** If
  it ends at ADR-0098's Alternative C, [0092]'s Phase 4 may legitimately be empty and [0104]'s
  Phase 4 needs rescoping *before* it is authored — which is precisely the information early
  placement buys.
- **Two lanes is the ceiling here, not a target.** Only three groups are genuinely disjoint
  (`lines/`, `shape_field.rs`, and `dsp/` + `tools/`); a third lane starts forcing plans that share
  files into one window.

### The baseline-drift control any pixel-touching plan inherits

Kept here after [0053]'s close because it is not that plan's property — it applies to every plan
that could move a render. **Do not `git diff` the committed baselines.** On this box **eight
baselines drift from their committed bytes under `RLX_BLESS`** (`composite_bloom`, `composite_kaleido`,
`composite_overlap`, `composite_trails`, `line_joint_zigzag`, `lsystem`, `parametric_curve`,
`star_pattern`), so a naive diff convicts eight files the change never touched. Bless every binary
in scope (`--test golden --test composite --test line_joints --test attractor_trails`) and compare
**bless-to-bless**, then `git checkout -- core/tests/golden`.

[0053]'s close used a tighter form of this than the clean-`main` control it was handed, and it is
the one to reuse: bless twice **on the same branch**, differing only by reverting the change under
test. Bless output is deterministic run-to-run (it is bless-vs-*committed* that drifts), so the two
hash sets are directly comparable and everything except the change is held fixed. All of them came
back identical, which is how "the two WARP fixes moved zero pixels" was established rather than argued.

**The suite is 32 baselines as of 2026-08-17** (it said 28 until then, and 20 before 2026-08-12 —
repaired at [0080](done/0080-the-sky-gets-a-horizon.md)'s close against a directory holding 26, then
27 after [0080] and 28 after [0081]; Plan 0100's `warp_mesh` and layer fixtures took it to 32). **The
number has now gone stale twice, which is the paragraph's own point about itself.** The eight drifters are named above by label, so the numerator survives the
correction; only the denominator was wrong. Re-derive the count rather than copying a number
forward — that is what went stale here.

The other recorded collision, "**[0046] should precede [0075] Phase 3**", is **discharged**: that
phase documents [backlog 0063](../design-backlog.md)'s `spin`x`fade` smear ceiling in *frames*, and
[0046] has now made the decay time-based, so the paragraph gets written once against final
semantics. The honest phrasing there is seconds rather than frames — and note the shipped
exponent is `fade^(dt / FALLBACK_DT)`, exactly `1.0` at the capture step, so every number measured
at 60 Hz or at capture `dt` stays correct as written.

[0083]: done/0083-the-build-says-why-it-hears-nothing.md
[0084]: done/0084-two-gates-stop-lying-about-what-they-check.md
[0085]: done/0085-the-show-length-horizon-gets-an-instrument.md
[0086]: done/0086-the-downbeat-finds-a-cue-that-is-not-the-kick.md
[0095]: done/0095-the-downbeat-fold-gets-a-musical-beat.md
[ADR-0109]: ../adrs/0109-the-beat-clock-counts-onsets-not-beats.md
[0087]: done/0087-the-line-renderer-draws-a-curve.md
[0088]: done/0088-the-docs-get-pictures.md
[0089]: done/0089-the-framing-contract-stops-lying.md
[0090]: done/0090-the-emitters-source-moves.md
[0091]: done/0091-the-figure-fills-the-frame.md
[0099]: done/0099-the-horizon-reaches-its-own-length.md
[0098]: done/0098-the-figure-nests-properly.md
[0045]: done/0045-linear-light-and-bloom.md
[0046]: done/0046-transformed-feedback.md
[0052]: done/0052-the-emitter-objects-that-spawn-fall-and-die.md
[0053]: done/0053-the-suite-stops-blessing-what-warp-gets-wrong.md
[0055]: done/0055-the-fold-edge-becomes-a-choice.md
[0062]: done/0062-the-chaos-game-grows-a-fern.md
[0065]: done/0065-the-mandala-interior.md
[0066]: done/0066-the-level-lever.md
[0069]: done/0069-the-instrument-that-sees-a-figure-leave-the-frame.md
[0061]: done/0061-the-build-stops-paying-for-what-it-is-not-building.md
[0064]: done/0064-the-symmetry-stage-and-the-banded-palette.md
[0067]: done/0067-the-curation-route.md
[0068]: done/0068-why-the-downbeat-rarely-locks.md
[0071]: done/0071-light-that-adds-without-covering.md
[0072]: done/0072-the-backdrop-joins-the-palette.md
[0075]: done/0075-the-content-renaissance.md
[0076]: done/0076-the-second-layer.md
[0077]: done/0077-the-quiet-sky.md
[0078]: done/0078-the-ink-learns-to-bite.md
[0079]: done/0079-the-attractor-learns-new-figures.md
[0080]: done/0080-the-sky-gets-a-horizon.md
[0081]: done/0081-the-sky-gets-a-galaxy.md
[0082]: done/0082-the-gradient-stops-banding.md
[ADR-0037]: ../adrs/0037-internal-grid-is-a-resolution-not-a-shape.md
[backlog 0038]: ../design-backlog.md
[backlog 0058]: ../design-backlog.md

### The six plans added 2026-08-04, and why they exist

They came from a **backlog sweep**, not from six separate requests: the user asked for the backlog
to be checked, the stale entries retired, and plans made from whatever was left that nothing else
already covered. What that produced is worth stating, because the shape of it is not obvious from
the rows above.

- **Seven entries were retired**, none of which carried a marker saying it was dead — 0015 (the
  half-linear band axis, landed with Plan 0048's second analysis window), 0020's content half (Plan
  0048 Phase 7's 368-gain retune), 0030 (landed in the content lane's own `craft.md`), 0036 (retired
  unfired), 0049 (carried into Plan 0055's judged A/B), 0051 (both `star_*` presets now ship triangle
  waves) and 0007's interior half (specified at last by [0065]). The backlog had been accumulating
  answered questions faster than it was closing them.
- **Five entries stay parked deliberately** — 0009 (informational), 0021 (the slew release, awaiting
  an author who wants it), 0032 (96 kHz, awaiting a report), 0038 and 0058 (content-lane retunes,
  routed not planned), and 0055 (attractor variety, which [0062] partly covers and whose own
  re-check condition just landed). **0058 has since closed** by content on 2026-08-04 (`859ec66`),
  so the parked content-lane retune is 0038 alone — worth knowing because two later documents kept
  pairing them.
- **Two of the six plans ship no capability at all.** [0068] ships a diagnosis and explicitly no
  fix; [0069] replaces a measure that was proved not to work. Both are here because the alternative
  — tuning a threshold, or calibrating a statistic that cannot separate the cases — is the move each
  one's ADR exists to refuse.
- **[0066] and [0071] each turned out to move zero pixels**, in both cases by *arithmetic* rather
  than by a chosen default: no golden fixture binds `exposure`, and `occlude` defaults to literal
  `1.0`. Neither was designed for that outcome; both plans check it as a phase failure rather than
  claiming it. **[0066] has since closed and the claim held exactly** — zero baselines modified
  across the whole plan, and it left behind the fixture that makes the premise false going forward
  (`composite_bloom_exposed.toml` is now the suite's only `exposure`-binding fixture), so the same
  reasoning cannot be reused unchecked. **[0071] has now closed and its claim held too, but it was
  the check rather than the arithmetic that established it**: the default at literal `1.0` was the
  argument, and what was run was an `RLX_BLESS` on the change re-encoding all 19 baselines
  hash-identical to an `RLX_BLESS` on clean `main`. That is the form to reuse — a bless-against-a-
  control, not a diff against the committed files, because three baselines on this machine
  (`lsystem`, `parametric_curve`, `star_pattern`) drift from their committed bytes under `RLX_BLESS`
  on clean `main` too, and a naive diff would have convicted the change of moving them.


## Standing (not a plan)

- **Plan [0135] Phase 5 — the unplug gate. Blocked on hardware, not on judgement** (2026-08-30).
  The plan is `done` on Phases 1-4 and the policy this would test is the **repaired** one — seconds
  instead of frames, and an operator swap now resets the incident. The phase did not run because
  **there is no removable audio interface on the box**, which is the same reason
  [Plan 0130](done/0130-the-audio-input-becomes-an-operator-surface.md)'s own Phase 5 skipped it.
  **It is one item, and it lives in
  [`docs/on-device-validation.md`](../on-device-validation.md)'s unplug checkbox** — not restated
  here, because a duty recorded twice drifts in one of the two. That item now carries Plan 0135's
  three extra questions alongside Plan 0130's original three.

  **What is waiting on it:** [backlog 0154](../design-backlog.md) is **live and carried**, and its
  own text says picking between its three candidate fixes *"wants the unplug evidence rather than
  more reasoning"*. So this gate is the input to a later ADR, and nothing else is blocked on it —
  every capability Plan 0135 shipped is in `main` and tested. **A run that reproduces nothing is a
  result**, not a failed phase: `REGDB_E_CLASSNOTREG` was one activation in 22 under menu-speed
  churn, which is a single sample and not a rate.

- **Plan [0091] Phase 6 — the figure at frame scale** (2026-08-16). The plan is `done` on Phases
  1-5; this is its `human` look gate, and it is **item 6 in
  [`docs/content-brief.md`](../content-brief.md)** where the three questions and four riders live.
  It is unusual in one way worth naming here: `shape_field` shipped with **no preset at all**, so
  the gate and the authoring job are the same sitting. The question the engine cannot answer for
  itself is whether a **band count latched to the beat reads as a response or as a strobe** — the
  recorded fallback is to move the beat onto `scale` or `gamma` and let the count sit still.

- **Plan [0090] Phase 5 — the two emitter worlds. The verdicts are in; the content is not**
  (2026-08-15). The plan is `done` on all five phases: the three questions were judged the same day
  on parameter probes, and this item has **narrowed from a judgement to an authoring job**. What is
  already answered, so nobody re-litigates it: **`spawn_fade` does hide the pop** (`0.35` against a
  paired `spawn_fade = 0` control, source on the screen midline), **a prewarmed world does not switch
  in badly** (so the transition-stage crossfade followup is discharged unfired), and
  **`emitter_perseids` keeps its place** — the fast shower and the quiet sky are different looks, not
  two tunings of one. Both verdicts are a dated `Outcome` on
  [ADR-0104](../adrs/0104-the-emitters-source-is-authorable-geometry.md).
  **What is left is content-lane work, with the answers in hand:** author the **quiet drifting field**
  (the sky [backlog 0068](../design-backlog-archive.md)
  measured the emitter for and could not get past the gates) and the **point fountain / off-centre
  jet** (`source_width = 0` plus `pan_x`), and **rewrite `emitter_perseids.toml`'s header**, which
  still declares that look routed on two walls that are both down. It joins the standing sitting on
  this family. **One number to take in with you:** the slow draft measured for Phase 3 passed
  `sanity` and `animation` at `prewarm = 1` and came up **0.0195 against a 0.02 reactivity floor** —
  97.5 % of it, on a draft nobody tuned for that gate. That reads as content tuning; if it turns out
  not to be, it is a finding for [ADR-0091](../adrs/0091-the-animation-gate-scores-motion-against-the-figures-footprint.md)
  rather than a number to lower.

- ~~**Plan [0085] Phase 5 — the three paired RSS runs**~~ — **RUN 2026-08-15**, hours after the
  close that listed it here, so the plan is complete on all five phases rather than carrying one.
  **Nothing grew**: feedback with 62 switches over 1196 s went 382.6 → 367.2 MB, the no-feedback
  control at the same length and cadence 379.9 → 380.1 MB, and 1797 s with **no switching at all**
  379.7 → **328.0 MB**. The control is what makes that readable — run 1 oscillates across a ~30 MB
  band where run 2 sits inside 0.4 MB, so feedback churn is real, **per-switch, and recovered every
  switch**. [backlog 0083](../design-backlog.md) is **CLOSED** in its bounded direction and archived.
  Caveats bound the claim rather than undermining it: no audio, windowed never fullscreen, different
  presets, 165 Hz — a lighter load than the original, and **the fullscreen reconfigure that
  dominated the original observation never happened**. Phase 3's claim did get its live confirmation
  — `frame_ms_p99_steady` held its previous value on exactly the rows following a switch (58 of 239
  in run 1) and agreed with the raw column everywhere else. **And the runs falsified one more thing
  nobody asked about**: the p99 tail is *not* switch-correlated (23.960 ms in a run with zero
  switches), so 0082's first candidate response would not have worked — filed as
  [backlog 0094](../design-backlog.md).
- **Plan [0083] Phase 5 — the Mac tester reads the reason off the new column** (2026-08-13). The
  plan is `done` and all four `dev` phases landed; this is the phase the whole plan exists for, and
  it needs a person who is not on this project. **Ship the tester a build carrying this change and
  read the `capture` column off the returned `diagnostics.log`** — or the F3 `audio` line off a
  photograph, which is the same string. The four surviving suspects are distinguishable from that
  one field: a stale or mismatched TCC grant (each ad-hoc-signed build is a different app to macOS,
  so the Privacy toggle can show an older build's entry as enabled while the new binary is denied),
  macOS below 13, a ScreenCaptureKit start error, or an unexpected fourth. **A reason that turns out
  not to be actionable is still a successful outcome** — the claim being discharged is *we cannot
  tell*. **Record the answer as a fresh entry in [`docs/design-backlog.md`](../design-backlog.md)
  citing [archived 0090](../design-backlog-archive.md)** — that entry moved to the archive at the
  third batch on 2026-08-13, and the archive is closed, so a returning question is a new entry rather
  than an edit to a closed one; whatever fix it implies is a new
  plan, not scope there. If it names a stale-TCC grant, the durable fix is a stable signing identity
  across builds, which is
  [ADR-0038](../adrs/0038-tag-driven-release-unsigned-universal-mac-app.md) territory and
  wants its own ADR. **This gates nothing** — the capability shipped without it.
- **Plan [0061] Phase 9 — ~~the one verification still outstanding~~ discharged 2026-08-20** (filed
  2026-08-08). The plan is `done` and every `dev` phase landed. **Phase 8 ran and passed the same
  day**: the foobar plugin builds against the extracted `rlx-core-cabi` and `foo_ritmolux.dll` loads in
  foobar2000 v2 and renders. That closed the one risk
  [ADR-0072](../adrs/0072-the-c-abi-ships-from-its-own-crate.md) carried into C++ link time — the
  linked artifact renamed to `rlx_core_c.lib`, and CI has no plugin job that would have caught a
  stale path. **Phase 9 needed a CI run rather than this machine, and got one:** run
  [`32272926929`](https://github.com/IgorKonovalov/Ritmolux/actions/runs/32272926929)
  (`main` at `7b9781d`, `rust-cache` restore-key hit, all six jobs green), read at Plan
  [0110](done/0110-the-shader-surface-stops-being-invisible.md)'s Phase 6 — that plan's own success
  criterion is the same job. Both halves answered:
  - **`COVERAGE_FLOOR` re-derives to 91**, the number it already carries. CI reads **92.31 % lines**
    where the floor was set off 94.85 % measured locally, so the hardware/WARP asymmetry
    `ci.yml:25-34` reserved ~3 points for is real and cost **2.54**. Raising it to 92 is **refused**
    — 0.31 points is ~62 lines, and the denominator moves with any non-test code that lands. The one
    edit owed is `ci.yml`'s comment, which still tells a reader the number is unverified.
  - **`coverage` IS the longest job** — 24m05s against `check (windows-latest)`'s 11m33s, a **2.1x**
    lead. So [ADR-0073](../adrs/0073-the-windows-ci-critical-path.md)'s Alternative A (merge the two
    Windows jobs) **stays rejected**, and nothing routes back to `architect` as a supplement.
- **The content lane's five standing sittings now live in one place:
  [`docs/content-brief.md`](../content-brief.md)** (consolidated 2026-08-13). They were five
  `human` phases of five closed plans, recorded here in five separate bullets running to ~140 lines
  — and three of them are **one sitting by construction**, which no reader of five bullets would
  see. The brief sequences them, carries every rider each plan attached, and is the **single** copy:
  this section deliberately no longer restates them, because a duty recorded twice drifts in one of
  the two. In order:
  1. **The sky family — one sitting, three items.** [0077]'s quiet sky, [0080]'s dusk ground,
     [0081]'s galaxy judgement (plus the banding frame's second check, below).
  2. **The ink worlds re-judge on `ink_gamma`** — [0078] Phase 3, two headers.
  3. **The attractor binds `tuple`** — [0079]'s Followup, a different family and a different
     sitting. Opens with a curation question, not a tuning one: the family is **17 of 37 presets,
     46 % of the library**.
  4. **The `occlude` retune with [backlog 0038]** — library-wide, so it goes last.

  **One correction this consolidation carries, because it would otherwise stand in this file
  uncontested:** the [0080] Phase 7 write-up below says *"the **tonemap-knee** half of that pairing
  is now measured away."* **It is not.** That phase retired a different suspicion — that
  `bg_bright = 0.85` was reaching the tonemap's shoulder on the **backdrop ramp** (0 % of the
  column rail-pinned). Backlog 0038 is about **mid-tone figure luminance on attractor presets**
  (`attractor_clifford` 82.54 → 75.91 mean luma), which no backdrop measurement speaks to. It is
  live, and exactly one shipped preset binds `exposure` today (`lsystem_vellum.toml:60`).
- **The banding reference frame is kept in the repo, and its second check is now due**
  (2026-08-12). `core/tests/fixtures/scratch-0082/dusk_ground_banding.toml` — a `scratch-NNNN/` in
  the [0046] arrangement, so nothing includes it, no test names it and `RLX_BLESS` does not touch
  it. It is the dusk ground at `bg_ramp_gamma = 0.4`: the darkest of the Plan 0080 probes **and**
  the worst banding case, both for the same reason — a fast-dropping ramp leaves a long dim tail,
  and a flat tail is where one 8-bit level lasts longest. It is committed rather than left in a
  session directory because **a before/after taken on two different pictures would prove nothing**.
  Re-measure at **1920x1080** (plateau width is in pixels, so the resolution is part of the
  measurement). **Check 1 is discharged**: after [0082] the widest mid-range plateau went
  **58 px → 20 px** and pixels-per-level **7.5 → 2.1**, still 0 % rail-pinned, and the `human`
  verdict was that the grain does not read as texture — *not* a hairline, which is what that plan
  predicted; what the dither bought is the level count and the collapse of wide plateaus from 17 to
  3. **Check 2 is owed**: add `bg_band_amount` and confirm the dither holds under **two
  overlapping gradients**, which nothing inside [0081] checks. Run it in the same sitting as the
  galaxy judgement — it is that plan's own Phase 6 third question, and the brief's §1c says so.
- **[On-device validation — low-end Windows iGPU smoke](../on-device-validation.md)** — a
  hardware-gated checklist, **not** a phased plan and **not** in the roster above: it never blocks a
  plan from closing. Holds the low-end / older Windows iGPU checks (fps floor ≥ 60 @ 1080p; footprint
  on a second GPU vendor) the user can only run once that box is in hand. Ticked when run; deleted when
  empty. Currently home to the extracted Plan 0012 Phase 3 (also covers the identical Plan 0003 Phase 3
  iGPU-fps carry-forward).


## Recently closed

One line per plan. **The full close write-ups — review verdicts, the findings
each close recorded, the properties that outlived the plan — moved verbatim to
[README-archive.md](README-archive.md)** (Plan 0061 Phase 7b). Nothing was
deleted; it simply stopped being loaded into every session's context.

That rule held for days and then stopped, so `scripts/check-index-rows.mjs` now
holds each bullet below to 320 bytes
([ADR-0116](../adrs/0116-an-index-row-is-a-pointer-and-a-gate-holds-it-to-one.md)).
A bullet is a link, a close date, and a review verdict; the write-up goes to the
archive first.

<!-- roster:begin cap=320 -->
- [0151 — The long documents become navigable](done/0151-the-long-documents-become-navigable.md) — closed 2026-09-04. Review: **no blockers, one major, four minors.** Version: **none** (docs/chore-only). Touched `presets/README.md` only, closed no backlog entry. [Write-up](README-archive.md).
- [0126 - The large files split along their seams](done/0126-the-large-files-split-along-their-seams.md) - closed 2026-09-03. Review: **no blockers, no majors in the code; one major doc repair, five minors.** Version: **0.103.1** (patch). Touched no presets, closed no backlog entry. [Write-up](README-archive.md).
- [0150 — The application becomes Ritmolux](done/0150-the-application-becomes-ritmolux.md) — closed 2026-09-02. Review: **no blockers, four majors, two minors.** Version: **0.103.0** (minor). Filed backlog 0180 + 0181. [Write-up](README-archive.md).
- [0148 — The shipped artifacts carry their own guarantees](done/0148-the-shipped-artifacts-carry-their-own-guarantees.md) — closed 2026-09-02. Review: **Phases 1-4 only, no blockers.** Version: **0.102.0** (minor). Archived [backlog 0174-0178](../design-backlog-archive.md). [Write-up](README-archive.md).
- [0149 — The line corners stop being blunt](done/0149-the-line-corners-stop-being-blunt.md) — closed 2026-09-02. Review: **no blockers, two majors, three minors.** Version: **0.101.0** (minor). Archived [backlog 0134](../design-backlog-archive.md). [Write-up](README-archive.md).
- [0136 - The gates can convict](done/0136-the-gates-can-convict.md) - closed 2026-09-02. Review: **one blocker, two majors, two minors, one nit.** Version: **0.100.1** (patch). Archived [nine backlog entries](../design-backlog-archive.md). [Write-up](README-archive.md).
- [0137 — The metrics measure light](done/0137-the-metrics-measure-light.md) — closed 2026-09-01. Review: **no blockers, one major, four minors, one nit.** Version: **0.100.0** (minor). Archived [backlog 0130 + 0132 + 0151 + 0152](../design-backlog-archive.md). [Write-up](README-archive.md).
- [0141 — The plugin's seams stop drifting](done/0141-the-plugin-seams-stop-drifting.md) — closed 2026-09-01. Review: **no blockers, two majors, three minors, two nits.** Version: **0.99.1** (patch). Archived [0105 + 0117 + 0118](../design-backlog-archive.md), filed 0177-0178. [Write-up](README-archive.md).
- [0139 - The render path validates before it spends](done/0139-the-render-path-validates-before-it-spends.md) - closed 2026-09-01. Review: **no blockers, one major, four minors, three nits.** Version: **0.99.0**. Archived [0111 + 0112](../design-backlog-archive.md), filed 0174-0176. [Write-up](README-archive.md).
- [0146 — The preset sweeps stop being one long test](done/0146-the-preset-sweeps-stop-being-one-long-test.md) — closed 2026-08-31. Review: **one blocker, four majors, eight minors, three nits**, all repaired at the close. Version: **0.98.0** (minor). [Write-up](README-archive.md).
- [0145 — The per-phase gate stops paying for the preset library](done/0145-the-per-phase-gate-stops-paying-for-the-preset-library.md) — closed 2026-08-31. Review: **no blockers, no majors, three minors.** Version: **none** (docs/chore-only). [Write-up](README-archive.md).
- [0144 - The flags mean what they say](done/0144-the-flags-mean-what-they-say.md) - closed 2026-08-31. Review: **no blockers, no majors, five minors, three nits.** Version: **0.97.0**. Archived [backlog 0167 + 0168 + 0169](../design-backlog-archive.md). [Write-up](README-archive.md).
- [0125 - The scenes share their GPU boilerplate](done/0125-the-scenes-share-their-gpu-boilerplate.md) - closed 2026-08-31. Review: **no blockers, no majors, four minors, four nits.** Version: **0.96.0** (minor). Nothing blessed. Repaired [backlog 0146](../design-backlog.md). [Write-up](README-archive.md).
- [0124 - The review fixes that move no pixels](done/0124-the-review-fixes-that-move-no-pixels.md) - closed 2026-08-30. Review: **no blockers, one major, five minors, two nits.** Version: **0.95.1** (patch). Filed [backlog 0168 + 0169](../design-backlog.md). [Write-up](README-archive.md).
- [0135 - The show-night surfaces stop lying](done/0135-the-show-night-surfaces-stop-lying.md) - closed 2026-08-30. Review: **no blockers, one major, five minors.** Version: **0.95.0**. Archived [backlog 0155 + 0156 + 0159](../design-backlog-archive.md). Phase 5 carried - see Standing. [Write-up](README-archive.md).
- [0134 - The lanes stop sharing a store](done/0134-the-lanes-stop-sharing-a-store.md) - closed 2026-08-30. Review: **no blockers, one major, four minors.** Version: **none** (docs/chore-only). Revoked ADR-0141's store half. [Write-up](README-archive.md).
- [0131 — The operator gets a console](done/0131-the-operator-gets-a-console.md) — closed 2026-08-30. Review: **no blockers, two majors, five minors, two nits.** Version: **0.94.0** (minor). Filed [backlog 0164 + 0165](../design-backlog.md). Phase 6 part-run. [Write-up](README-archive.md).
- [0115 — The engine becomes a live video source](done/0115-the-engine-becomes-a-live-video-source.md) — closed 2026-08-30. Review: **no blockers, two majors, three minors.** Version: **0.93.0** (minor). [Write-up](README-archive.md).
- [0104 — The library stops being lopsided](done/0104-the-library-stops-being-lopsided.md) — closed 2026-08-29. Review: **one blocker, three majors, three minors, two nits.** Version: **0.92.0** (minor). Corrected [backlog 0038](../design-backlog.md). [Write-up](README-archive.md).
- [0129 - The build stops being paid three times](done/0129-the-build-stops-being-paid-three-times.md) - closed 2026-08-29. Review: **no blockers, one major, four minors.** Version: **0.91.1** (patch). Store half revoked by ADR-0147. Filed [backlog 0160 + 0161](../design-backlog.md). [Write-up](README-archive.md).
- [0132 — The lighting rig follows the visuals](done/0132-the-lighting-rig-follows-the-visuals.md) — closed 2026-08-29. Review: **no blockers, one major, three minors, two nits.** Version: **0.91.0** (minor). [Write-up](README-archive.md).
- [0127 — The picture stops depending on the volume slider](done/0127-the-picture-stops-depending-on-the-volume-slider.md) — closed 2026-08-28. Review: **no blockers, no majors, four minors.** Version: **0.90.0** (minor). Archived [backlog 0122 + 0123](../design-backlog-archive.md). [Write-up](README-archive.md).
- [0130 — The audio input becomes an operator surface](done/0130-the-audio-input-becomes-an-operator-surface.md) — closed 2026-08-28. Review: **no blockers, two majors, two minors, two nits.** Version: **0.89.0** (minor). Filed [backlog 0154-0156](../design-backlog.md). [Write-up](README-archive.md).
- [0123 — A gate, a latch and an ink](done/0123-a-gate-a-latch-and-an-ink.md) — closed 2026-08-28. Review: **no blockers, two majors, three minors.** Version: **0.88.0** (minor). Closed [backlog 0145 + 0147 + 0148](../design-backlog.md), filed 0151-0153. [Write-up](README-archive.md).
- [0122 — Every rate integrates](done/0122-every-rate-integrates.md) — closed 2026-08-28. Review: **no blockers, two majors** (both discharged), **two minors.** Version: **0.87.0** (minor). Closed [backlog 0141](../design-backlog.md), filed 0149-0150. [Write-up](README-archive.md).
- [0098 — The figure nests properly](done/0098-the-figure-nests-properly.md) — closed 2026-08-27. Review: **no blockers, no majors, five minors, two nits.** Version: **0.86.0** (minor). Closed [backlog 0096 + 0097](../design-backlog.md), filed 0144. [Write-up](README-archive.md).
- [0118 — The comments stop narrating the plans that wrote them](done/0118-the-comments-stop-narrating-the-plans-that-wrote-them.md) — closed 2026-08-27. Review: **no blockers, no majors, five minors.** Version: **0.85.0** (minor). Closed [backlog 0129](../design-backlog.md). [Write-up](README-archive.md).
- [0121 — A rate, an ink edge, and a motion reading](done/0121-a-rate-an-ink-edge-and-a-motion-reading.md) — closed 2026-08-27. Review: **no blockers, one major, four minors.** Version: **0.83.0** (minor). Closed [backlog 0131 + 0137-0139](../design-backlog.md), filed 0140-0141. [Write-up](README-archive.md).
- [0087 — The line renderer draws a curve](done/0087-the-line-renderer-draws-a-curve.md) — closed 2026-08-27. Review: **no blockers, one major, five minors.** Version: **0.82.0** (minor). Closed [backlog 0071 + 0073](../design-backlog.md), filed 0134-0136. [Write-up](README-archive.md).
- [0114 — The line stroke reads as a drawn line](done/0114-the-line-stroke-reads-as-a-drawn-line.md) — closed 2026-08-26. Review: **no blockers, one major, three minors** (none in code). Version: **0.81.0** (minor). Filed [design-backlog 0133](../design-backlog.md). [Write-up](README-archive.md).
- [0113 — The engine paints a canvas](done/0113-the-engine-paints-a-canvas.md) — closed 2026-08-26. Two reviews: the first **three majors** (became Phase 9), the second **no blockers, one major** (became Phase 10). [Write-up](README-archive.md).
- [0119 — The flatness gate gets its second term](done/0119-the-flatness-gate-gets-its-second-term.md) — closed 2026-08-26. Review: **no blockers, one major, four minors.** Version: **0.80.0** (minor). Filed [design-backlog 0130 + 0131](../design-backlog.md).

- [0116 — The sanity lens finds the ground](done/0116-the-sanity-lens-finds-the-ground.md) — closed 2026-08-26. Review: **one blocker, two majors, four minors.** Version: **0.78.1** (patch). Phase 9 did not run: its stop condition fired.

- [0117 — The downbeat log sees the counter it folds over](done/0117-the-downbeat-log-sees-the-counter-it-folds-over.md) — closed 2026-08-25. Review: **no blockers, one major, six minors.** Version: **0.78.0** (minor). Filed [design-backlog 0129](../design-backlog.md).

- [0095 — The downbeat fold gets a musical beat](done/0095-the-downbeat-fold-gets-a-musical-beat.md) — closed 2026-08-25. Review: **no blockers, one major, three minors.** Version: **0.77.0** (minor). [ADR-0109](../adrs/0109-the-beat-clock-counts-onsets-not-beats.md) accepted with an `Outcome`.

- [0106 — The frame stream passes through a diffusion model](done/0106-the-frame-stream-passes-through-a-diffusion-model.md) — closed 2026-08-25. Review: **no blockers, no majors, five minors** (repaired at close). ADR renumbered 0120 → **0122**: two lanes took the number the same day.

- [0112 — The handoff stops being a chat message](done/0112-the-handoff-stops-being-a-chat-message.md) — closed 2026-08-25. Review: **no blockers, no majors, two minors.** Version: **none** (docs/chore-only). Log: 43 lines vs a 146-line phase section.
- [0111 — The MilkDrop import stops washing out](done/0111-the-milkdrop-import-stops-washing-out.md) — closed 2026-08-20. Review: **no blockers, one major, two minors.** The bisect stopped: the wash is at the **field**. Phase 6 void.
- [0109 — The MilkDrop import gets its geometry back](done/0109-the-milkdrop-import-gets-its-geometry-back.md) — closed 2026-08-19. Review: **no blockers, two majors (repaired at close), three minors.** The gate falsified three claims; ADR-0119 + a Phase 7 followed.
- [0110 — The shader surface stops being invisible](done/0110-the-shader-surface-stops-being-invisible.md) — closed 2026-08-19. Review: **no blockers, one major, three minors.** Phase 6 read 2026-08-20: run `32272926929`, **92.31 %** vs floor 91.
- [0107 — The foobar menu picks a preset](done/0107-the-foobar-menu-picks-a-preset.md) — closed 2026-08-18. Review: **no blockers, two majors (repaired at close), four minors.** Phase 5 not run; carried to the on-device checklist. Backlog 0117 + 0118.
- [0108 — The MilkDrop import gets its tone back](done/0108-the-milkdrop-import-gets-its-tone-back.md) — closed 2026-08-17. Review: **no blockers, two majors, three minors.** Phase 2: **still merely different**; four new defects → [0109](done/0109-the-milkdrop-import-gets-its-geometry-back.md).
- [0101 — The engine renders a music video](done/0101-the-engine-renders-a-music-video.md) — closed 2026-08-17. Review: **no blockers, no majors, five minors/nits.** Phase 5: **yes, with backlog 0110** — a 1080p render reads as an upscale.
- [0100 — The engine speaks MilkDrop](done/0100-the-engine-speaks-milkdrop.md) — closed 2026-08-16. Review: **no blockers, one major (repaired at close), two minors.** Phase 7: **merely different**; fidelity backlog 0106–0108.
- [0102 — The component ships](done/0102-the-component-ships.md) — closed 2026-08-16. Review: **no blockers, three majors, four minors.** Phase 5 (`human`) carried to [on-device-validation.md](../on-device-validation.md).
- [0099 — The horizon reaches its own length](done/0099-the-horizon-reaches-its-own-length.md) — closed 2026-08-16. Review: **no blockers, no majors, four minors and a nit.**
- [0105 — The indexes go back to being indexes](done/0105-the-indexes-go-back-to-being-indexes.md) — closed 2026-08-16. Review: **no blockers, two majors, two minors, one nit.**
- [0097 — The track announces itself](done/0097-the-track-announces-itself.md) — closed 2026-08-16. Review: **no blockers, no majors, three minors and a nit.**
- [0096 — The HUD gets out of the way](done/0096-the-hud-gets-out-of-the-way.md) — closed 2026-08-16. Review: **no blockers, no majors, two minors and a nit.**
- [0091 — The figure fills the frame](done/0091-the-figure-fills-the-frame.md) — closed 2026-08-16. Review: **no blockers, no majors, three minors.**

- [0090 — The emitter's source moves](done/0090-the-emitters-source-moves.md) — closed 2026-08-15. Review: **no blockers, no majors, two minors, one nit.**
- [0094 — The two doc gates check what they claim to](done/0094-the-two-doc-gates-check-what-they-claim-to.md) — closed 2026-08-15. Review: **no blockers, no majors, two minors, two nits.**
- [0093 — The backlog stops asserting things about a repo it has not read](done/0093-the-backlog-stops-asserting-things-about-a-repo-it-has-not-read.md) — closed 2026-08-15. Review: **no blockers, two majors, one minor, two nits.**
- [0085 — The show-length horizon gets an instrument](done/0085-the-show-length-horizon-gets-an-instrument.md) — closed 2026-08-15. Review: **no blockers, one major, three minors, one nit.**
- [0089 — The framing contract stops lying, and two doc gaps close](done/0089-the-framing-contract-stops-lying.md) — closed 2026-08-15. Review: **no blockers, no majors, one minor, three nits.**
- [0088 — The docs get pictures](done/0088-the-docs-get-pictures.md) — closed 2026-08-13. Review: **no blockers, no majors, three minors, two nits.**
- [0084 — Two gates stop lying about what they check](done/0084-two-gates-stop-lying-about-what-they-check.md) — closed 2026-08-13. Review: **no blockers, no majors, three minors, one nit.**
- [0083 — The build says why it hears nothing](done/0083-the-build-says-why-it-hears-nothing.md) — closed 2026-08-13. Review: **no blockers, no majors, one minor, two nits.**
- [0079 — The attractor learns new figures: the tuple roster with per-tuple framing, and measured morph paths](done/0079-the-attractor-learns-new-figures.md) — closed 2026-08-13. Review: **no blockers, no majors, four minors, two nits.**
- [0081 — The sky gets a galaxy: the backdrop paints a curved band](done/0081-the-sky-gets-a-galaxy.md) — closed 2026-08-12. Review: **no blockers, no majors, two minors, two nits.**
- [0082 — The gradient stops banding: the display write dithers](done/0082-the-gradient-stops-banding.md) — closed 2026-08-12. Review: **no blockers, one major, five minors, three nits.**
- [0080 — The sky gets a horizon: the backdrop paints a directional ramp](done/0080-the-sky-gets-a-horizon.md) — closed 2026-08-12. Review: **no blockers, no majors, three minors, one nit.**
- [0078 — The ink learns to bite: a contrast exponent on the terminal remap](done/0078-the-ink-learns-to-bite.md) — closed 2026-08-12. Review: **no blockers, no majors, three minors, one nit.**
- [0077 — The quiet sky: the sparse idiom becomes gateable and the swarm individuates](done/0077-the-quiet-sky.md) — closed 2026-08-12. Review: **no blockers, no majors, two minors.**
- [0075 — The content renaissance: the library is rebuilt as worlds, by replacement cohorts](done/0075-the-content-renaissance.md) — closed 2026-08-11. Review: **no blockers, no majors, two minors, two nits.**
- [0076 — The second layer: a preset composes two scenes (R3)](done/0076-the-second-layer.md) — closed 2026-08-11. Review: **no blockers, no majors.**
- [0053 — The suite stops blessing what WARP gets wrong](done/0053-the-suite-stops-blessing-what-warp-gets-wrong.md) — closed 2026-08-09. Review: **no blockers, two majors, four minors.**
- [0046 — Transformed feedback: the past learns to move](done/0046-transformed-feedback.md) — closed 2026-08-09. Review: **no blockers, no majors, four minors, two nits.**
- [0068 — Why the downbeat rarely locks](done/0068-why-the-downbeat-rarely-locks.md) — closed 2026-08-09. Review: **no blockers, no majors, two minors, one nit.**
- [0067 — The curation route](done/0067-the-curation-route.md) — closed 2026-08-09. Review: **no blockers and no code findings.**
- [0064 — The symmetry stage and the banded palette](done/0064-the-symmetry-stage-and-the-banded-palette.md) — closed 2026-08-09. Review: **no blockers and no code findings.**
- [0071 — Light that adds without covering (`occlude`)](done/0071-light-that-adds-without-covering.md) — closed 2026-08-09. Review: **no blockers, two majors, three minors, two nits.**
- [0072 — The backdrop joins the palette](done/0072-the-backdrop-joins-the-palette.md) — closed 2026-08-09. Review: **no blockers, no majors, four minors, three nits.**
- [0061 — The build stops paying for what it is not building](done/0061-the-build-stops-paying-for-what-it-is-not-building.md) — closed 2026-08-08. Review: no blockers, two majors, three minors (all five doc drift, all repaired at the close). **Phases 8 and 9 are `human` and carried forward — see Standing.**
- [0074 — The figure colours by how far it has come](done/0074-the-figure-colours-by-how-far-it-has-come.md) — closed 2026-08-08. Review: no blockers, four minor items (three repaired at the close)
- [0073 — The fern unfurls and colours by what made it](done/0073-the-fern-unfurls-and-colours-by-what-made-it.md) — closed 2026-08-06. Review: no blockers, two minor doc items repaired at the close
- [0065 — The mandala interior: `star_pattern` stops being hollow](done/0065-the-mandala-interior.md) — closed 2026-08-06. Review: no blockers
- [0069 — The instrument that sees a figure leave the frame](done/0069-the-instrument-that-sees-a-figure-leave-the-frame.md) — closed 2026-08-06. Review: no blockers, three minor, one nit
- [0070 — Shaped marks](done/0070-shaped-marks.md) — closed 2026-08-05. Review: no blockers, one minor
- [0066 — The level lever](done/0066-the-level-lever.md) — closed 2026-08-05. Review: no blockers, one minor
- [0062 — The chaos game grows a fern](done/0062-the-chaos-game-grows-a-fern.md) — closed 2026-08-05. Review: no blockers, one major, four minor
- [0063 — The attractor keeps its depth](done/0063-the-attractor-keeps-its-depth.md) — closed 2026-08-04. Review: no blockers, one major, two minor
- [0036 — macOS and Windows release artifacts](done/0036-macos-and-windows-release-artifacts.md) — closed 2026-08-04. Review: no blockers, one minor, one nit
- [0055 — The fold edge becomes a choice](done/0055-the-fold-edge-becomes-a-choice.md) — closed 2026-08-04. Review: no blockers
- [0051 — the scene seam emits premultiplied alpha](done/0051-the-scene-seam-emits-premultiplied-alpha.md) — closed 2026-08-01. Review: no blockers, one major (an operator-doc gap, fixed at close), four minor
- [0039 — Line joins: the stroke stops coming apart at every vertex](done/0039-line-joins.md) — closed 2026-07-28. d Mode 4 review (no blockers; one major, three minors, two nits — nothing fixed in the close commit, the major is deliberately…)
- [0023 — Cross-preset visual transitions: MilkDrop-style dissolves between presets](done/0023-cross-preset-transitions.md) — closed 2026-07-26. Review: no blockers, no majors; five minors, one nit
- [0019 — Preset expression grammar v2: branching, math functions, tempo, typo warnings](done/0019-preset-grammar-v2.md) — closed 2026-07-25. Review: no blockers; one major, three minors, one nit
- [0015 — Preset-directory override + live iteration](done/0015-preset-dir-override-and-live-iteration.md) — closed 2026-07-25. Review: no blockers; one major, three minors, two nits
- [0029 — Attractor resize cost + ink-stage followups](done/0029-attractor-resize-cost-and-ink-followups.md) — closed 2026-07-25. Review: no blockers, no majors; two minors, two nits
- [0027 — Attractor ink-on-paper (engine-wide final tone-remap) + crisp trails](done/0027-attractor-ink-and-crisp-trails.md) — closed 2026-07-25. Plan 0029 or ADR-0030 rather than reworked
- [0025 — Full composite coverage: background + view transform for reaction-diffusion and attractor](done/0025-full-composite-coverage.md) — closed 2026-07-24. Review: no blockers, no majors; one minor, one nit
- [0028 — Parametric-curve shape params: radial offset + phase (audio-morphable rose geometry)](done/0028-parametric-curve-shape-params.md) — closed 2026-07-24. Review: no blockers, no majors; one minor, one nit
- [0020 — Shared palette system: gradient LUT, named + custom palettes, bindable color (all four scenes)](done/0020-shared-palette-system.md) — closed 2026-07-24. Review: no blockers, no majors; two minor, two nits
- [0026 — Calmer scene rotation: hold one scene by default, longer dwell, softened drop bias](done/0026-calmer-scene-rotation.md) — closed 2026-07-24. Review: no blockers, no majors; one minor, one nit
- [0018 — Engine-wide visual enrichment: zoom, atmosphere, easing, mirrors](done/0018-engine-wide-visual-enrichment.md) — closed 2026-07-23. Review: no blockers, no majors; three minor, two nits
- [0024 — Single-source the foobar component version + refresh stale plugin descriptions](done/0024-foobar-component-version-single-source.md) — closed 2026-07-23. passed Mode 4 review cold (no blockers, no majors, no minors — one nit)
- [0021 — Decouple preset content from code: build-time embedding + single-source system names](done/0021-decouple-preset-content-from-code.md) — closed 2026-07-23. Review: no blockers, no majors, no minors, no nits — a clean landing
- [0016 — GPU compute-particle scenes: strange attractors](done/0016-gpu-compute-particle-scenes.md) — closed 2026-07-23. Review: no blockers, no majors; two minor, three nits
- [0022 — Decouple the golden drift guard from shipped presets (per-system frozen fixtures)](done/0022-golden-fixtures-decouple-content.md) — closed 2026-07-23. Review: no blockers, no majors; one minor, one nit
- [0014 — Reaction-diffusion feedback scene + frame-rate-independent render clock](done/0014-reaction-diffusion-feedback-scene.md) — closed 2026-07-23. Review: no blockers, no majors; two minor, two nits
- [0009 — Live performance features (standalone)](done/0009-live-performance-features.md) — closed 2026-07-23. Review: no blockers, no majors; two minor deviations, both pre-flagged and reconciled
- [0017 — Green CI: reasoned ttf-parser advisory ignore + adapter-skip for headless GPU tests](done/0017-ci-green-advisory-and-gpu-tests.md) — closed 2026-07-23. Review: no blockers, no majors, no minors; two non-actionable nits
- [0010 — Line-geometry scenes: parametric curves, L-systems, star patterns](done/0010-line-geometry-scenes.md) — closed 2026-07-23. Review: no blockers, no majors; three minor, two nits
- [0013 — Headless scene capture + differential visual QA + golden images + shot CLI](done/0013-headless-scene-capture.md) — closed 2026-07-22. Review: no blockers, no majors; one minor, one nit
- [0008 — In-app preset browse overlay (standalone)](done/0008-preset-browse-overlay.md) — closed 2026-07-22. Review: no blockers, no majors
- [0012 — Measure the driver-memory floor + cull dead scenes](done/0012-memory-floor-measure-and-scene-cull.md) — closed 2026-07-22. Review: no blockers, no majors
- [0011 — Diagnostics harness + quick-win memory/perf trim](done/0011-diagnostics-and-memory-trim.md) — closed 2026-07-22. Review: no blockers, no majors; two nits
- [0007 — Curated preset library: robust loading + seed-on-first-run + C ABI v2](done/0007-curated-preset-library.md) — closed 2026-07-22. Review: no blockers, no majors
- [0004 — foo_lmv as an embeddable Default UI panel](done/0004-foobar-ui-element-panel.md) — closed 2026-07-21. Review: no blockers, no majors
- [0005 — Extract the lock-free ring into a wgpu-free crate for Miri](done/0005-miri-ring-extraction.md) — closed 2026-07-21. Review: no blockers, no majors
- [0003 — Generative scenes + data-driven presets](done/0003-generative-scenes-and-presets.md) — closed 2026-07-21. Review: no blockers
- [0006 — Versioning: single source of truth + cargo-release + surfacing](done/0006-versioning-wiring.md) — closed 2026-07-21. Review: no blockers, no majors
- [0002 — Rust enforcement tooling](done/0002-rust-enforcement-tooling.md) — closed 2026-07-21. Review: no blockers
- [0001 — Core + standalone MVP, then foobar parity](done/0001-core-and-standalone-mvp.md) — closed 2026-07-21. Review: no blockers; C ABI recorded in ADR-0003

<!-- roster:end -->

## Roadmap (agreed 2026-07-21, revised same day for the live-show use case; numbers assigned when drafted)

> **2026-07-30: a second, strategic roadmap now exists —
> [docs/roadmap-visual-richness.md](../roadmap-visual-richness.md)** — from the user-requested
> "why is everything dull" architecture review. It diagnoses the five capability caps (single
> quality tier, decay-only feedback, 8-bit additive composite, one-scene/fixed-chain
> composition, starved grammar) and orders the themes R0-R6 that answer them. Item 3's
> remaining half below (quality tiers + governor) is that roadmap's R0. New visual-capability
> plans should cite it.

Execution order after Plan 0001, per the NFR interviews ([docs/nfr.md](../nfr.md)):

1. **Preset / scripting engine** — layered presets per
   [ADR-0002](../adrs/0002-layered-preset-architecture.md): TOML data + expression language
   driving built-in systems (feedback/warp, boids, walkers/growth, 3D scene), with an
   optional budgeted Rhai script for staged per-track arcs (NFR §10). Replaces "scenes are
   Rust code" — Plan 0001's Scene trait becomes the rendering vocabulary presets drive, so
   keep it thin. **Delivered by [Plan 0003](done/0003-generative-scenes-and-presets.md)** (layers
   1-2: fragment-field + swarm systems, data + expression presets); Rhai (layer 3), blending, and
   compute-scale particles remain follow-ups tracked in 0003.
2. **Live performance features** — line-in/audio-interface capture, scene triggers
   (auto-rotate + hotkey/MIDI + experimental track-change detection), fullscreen on a
   chosen display/projector, 4-hour soak stability (NFR §10).
   **Delivered by [Plan 0009](done/0009-live-performance-features.md)** (standalone borderless-
   fullscreen on a chosen display, line-in capture selection, drop-biased scene director +
   hotkeys, spectral track-change novelty nudge on the native `Frame`, `--soak` instrumentation;
   C ABI frozen). **MIDI triggers and the ≥4-hour projector-rig soak run remain** — MIDI is its
   own ADR-backed follow-up; the soak run is a `human` on-device carry-forward.
3. **Adaptive quality + runtime-memory trim** — quality tiers + frame-time governor for the
   60 fps iGPU floor (NFR §1), plus cutting the standalone's ~200 MB working set (NFR §12).
   The memory trim's primary lever — compiling wgpu with only the per-OS backend feature
   (DX12/Metal), dropping the dead Vulkan/GL paths — is a cheap, low-risk win that can
   front-run the full tier system. Both validated on the older iGPU test PC (footprint stated
   before/after; the backend trim must not regress the §1 floor).
   **Front-run by [Plan 0011](done/0011-diagnostics-and-memory-trim.md)** (diagnostics harness +
   the cheap NFR §12 levers, all-three-frontend, C ABI v3 / [ADR-0008](../adrs/0008-c-abi-v3-diagnostics.md)):
   it builds the before/after measuring stick and lands the wgpu-backend + swapchain trims.
   The **adaptive-quality tiers + frame-time governor** were then **delivered by
   [Plan 0044](done/0044-quality-tiers.md) / [ADR-0045](../adrs/0045-quality-tiers-floor-and-rich.md)**
   (2026-07-30) — this sentence said they "remain for a later plan" for six weeks after they
   landed. What remains of the item is the `Rich` budget's on-device calibration (Plan 0044
   Phase 4, carried in [on-device-validation.md](../on-device-validation.md)) and NFR §12's
   memory work.
   **Before touching the governor, read its qualification** ([NFR §1](../nfr.md),
   [backlog 0082](../design-backlog.md)): `frame_ms_p99` reached **25.037 ms against an 8.749 ms
   average with zero of 28,698 frames dropped**, on preset switches and a fullscreen toggle — GPU
   resource rebuilds, not steady-state cost. A governor reading that column bare would demote a
   preset running at 165 fps. **The shipped one does not read it** — `sustained_miss` needs 75 % of
   ≥180 samples past 1.25× the budget, which a switch's handful of slow frames cannot reach — so the
   hazard is in the *description*, not the code. Three candidate responses are named in the backlog
   and **deliberately not chosen**; `--soak` carries `frame_ms_p99_steady` and a `switches` counter
   (Plan 0085 Phase 3) so the third can be measured rather than argued.
4. **Remaining v1 UX** — always-on-top / mini mode, settings persistence (NFR §11;
   fullscreen/multi-monitor land earlier with live features).
5. ~~**Packaging & release** — GitHub release zip: unsigned standalone exe +
   `.fb2k-component` (NFR §8).~~ **Delivered.** The two standalone zips landed at
   [Plan 0036](done/0036-macos-and-windows-release-artifacts.md); the `.fb2k-component` joined at
   [Plan 0102](done/0102-the-component-ships.md) (2026-08-16), so a `v*` tag now ships all three.

Later, unordered: better tempo tracking, preset sharing/library, signed installer.


## Conventions

- **Numbering:** sequential, zero-padded 4 digits. Take the next free number above, then
  bump it here in the same session.
- **Phases:** ordered, each one commit, each tagged `**Owner skill:**` with one value from the
  vocabulary `dev` (all code) or `human` (a task only the user can do). The `dev` skill reads
  this tag at the start of each phase; a missing tag is a Mode 4 review blocker. An optional
  `**Area:**` note (`core` / `standalone` / `plugin`) orients the reader but is not the tag.
- **Skills:** `architect` designs and owns `docs/`; `dev` implements all code. `architect`
  writes and closes plans; `dev` flips `draft → in-progress` at "go" and nothing else in the file.
- **Lifecycle:** `draft` → `approved` (user/architect validated it; ready for `dev`) →
  `in-progress` → `done` (then `git mv` to `done/` and drop from this roster). Review
  happens at plan end, in a fresh `/architect` session — not by the session that wrote
  the code.
[0100]: done/0100-the-engine-speaks-milkdrop.md
[0101]: done/0101-the-engine-renders-a-music-video.md
[0102]: done/0102-the-component-ships.md
[0103]: 0103-the-project-gets-an-audience.md
[0104]: done/0104-the-library-stops-being-lopsided.md
[0115]: done/0115-the-engine-becomes-a-live-video-source.md
[0123]: done/0123-a-gate-a-latch-and-an-ink.md
[0124]: done/0124-the-review-fixes-that-move-no-pixels.md
[0125]: done/0125-the-scenes-share-their-gpu-boilerplate.md
[0126]: done/0126-the-large-files-split-along-their-seams.md
[0127]: done/0127-the-picture-stops-depending-on-the-volume-slider.md
[0128]: 0128-the-rendered-file-stops-looking-upscaled.md
[0131]: done/0131-the-operator-gets-a-console.md
[0133]: 0133-the-engine-drives-the-lights.md
[0135]: done/0135-the-show-night-surfaces-stop-lying.md
[0136]: done/0136-the-gates-can-convict.md
[0137]: done/0137-the-metrics-measure-light.md
[0138]: 0138-the-colour-surface-stops-misleading-its-authors.md
[0139]: done/0139-the-render-path-validates-before-it-spends.md
[0140]: 0140-every-rate-integrates-for-real.md
[0141]: done/0141-the-plugin-seams-stop-drifting.md
[0142]: 0142-the-milkdrop-import-earns-its-verdict.md
[0145]: done/0145-the-per-phase-gate-stops-paying-for-the-preset-library.md
[0146]: done/0146-the-preset-sweeps-stop-being-one-long-test.md
