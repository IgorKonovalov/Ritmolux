# Plans index

The one-minute "what's in flight" view. Read this first each session instead of
re-deriving state from `git log`. Completed plans move to `done/`; their full
close write-ups move to [README-archive.md](README-archive.md).

**Next free number: 0113** (ADRs are a separate sequence — next free there is **0121**.)

## Active roster

Only plans still in `docs/plans/`. A closed plan leaves this table entirely —
`Recently closed` below and `done/` both already record it. Each row carries at
most two sentences of **live constraint**: what a reader needs to decide whether
to pick this plan up. Anything longer belongs in the plan file, which is where
someone who picked it up is reading.

| Plan | Title | Status | Owner | Live constraint |
|------|-------|--------|-------|-----------------|
| [0095](0095-the-downbeat-fold-gets-a-musical-beat.md) | The downbeat fold gets a musical beat | approved | dev, human | **Succeeds [0086], which measured the defect and shipped the instrument.** The fold is indexed by onset events, not beats (1.7-2.1x, wandering 1x-4x within a track, against a control that reads 1.00). Phase 2 puts **tempo octave stability on the critical path by choice** — if Phase 1's ladder says the octave choice is a coin flip, the plan stops there with a diagnosis rather than gridding on sand. `beat`/`beat_index` are bit-identical by Phase 3's own assertion, so no preset timing moves. |
| [0087](0087-the-line-renderer-draws-a-curve.md) | The line renderer draws a curve | approved | dev, human | The largest, and the only one with a **stop condition**: Phase 3 measures per-pixel cost against the NFR §1 floor tier, and Phase 4 is a `human` look gate placed *before* the biarc work — either can send the plan to ADR-0098's Alternative C. Owes a re-bless (28 baselines) and an ADR-0058 enumeration entry. Watch [ADR-0037](../adrs/0037-internal-grid-is-a-resolution-not-a-shape.md): this family has shipped that bug three times. |
| [0098](0098-the-figure-nests-properly.md) | The figure nests properly | approved | dev, human | **Carries [ADR-0111](../adrs/0111-the-shape-field-gains-a-scaled-copy-coordinate.md) (proposed) and closes two backlog entries from Plan 0091's content pass.** `shape_field`'s level sets are offsets, and an inward offset *erodes* — which rounds a reflex corner, so a nested heart loses its top notch. That is not tunable: a sharp notch needs `palette_steps * color_span ~ 1`, which leaves ONE band inside the figure, and the user rejected that end of the trade in the running app. Phase 1 is an independent defect fix (a curved or jittered `star` returns a **negative** distance at its own centre — provably, always). **Contends with [0092](0092-the-engine-draws-an-authored-path.md) on `shape_field.rs`**, so run them in sequence or in a lane. |
| [0092](0092-the-engine-draws-an-authored-path.md) | The engine draws an authored path | approved | dev, human | **Its hard dependency is discharged — [0091](done/0091-the-figure-fills-the-frame.md) closed 2026-08-16 and `shape_field` is the scene this draws into.** Soft-depends on [0087](0087-the-line-renderer-draws-a-curve.md) — the plan states that disagreement openly: a polyline distance field is complete alone, arcs only lower the arity, so **this is takeable even if 0087 ends at ADR-0098's Alternative C**, and Phase 4 may legitimately be empty. Phase 2's arity ceiling is a **measurement**, not [ADR-0107](../adrs/0107-an-authored-path-is-inline-svg-data-and-it-morphs-by-resampling.md)'s construction estimate. Expect morph degeneracy — Plan 0079 refused 4 of 20 swept pairs by measurement. |
| [0103](0103-the-project-gets-an-audience.md) | The project gets an audience | approved | dev, human | **Amended 2026-08-16: a new Phase 1 fixes backlog [0102](../design-backlog.md) + [0103](../design-backlog.md) before anything advertises the component**, which Plan 0102's Phase 5 found starves foobar2000's own UI until playback starts. That phase is a surface-lifetime design pass, is ADR-worthy, and its fix only reaches users on the next tag — so the release must be green before Phase 5 submits. The rest is outreach whose done-whens are **artifacts, not outcomes**. Phase 3's soft want [0101](done/0101-the-engine-renders-a-music-video.md) **closed 2026-08-17 — motion can be recorded now, but its Phase 5 found a 1080p render reads as an upscale ([backlog 0110](../design-backlog.md)), so demo material made before that lands shows the engine at its grainiest.** |
| [0104](0104-the-library-stops-being-lopsided.md) | The library stops being lopsided | approved | dev, human | **Corrected 2026-08-17: eleven systems, not ten — `warp_mesh` ships with ZERO worlds and was invisible to a census counted from `presets/*.toml`, so 22 → 61 is the honest arithmetic and ~~its four wait on [0108](done/0108-the-milkdrop-import-gets-its-tone-back.md) Phase 1~~ — **that wait is discharged: 0108 Phase 1 landed 2026-08-17, so the four `warp_mesh` worlds are authorable now.** **The census is the plan: `attractor` has 17 worlds; `lsystem`, `shape_field`, `spectrum` and `star_pattern` have exactly one each.** Brings every system to a floor of four — 18 presets, 39 → 57 — under [ADR-0089](../adrs/0089-the-library-renews-by-replacement-cohorts.md)'s cohort rules. Phase 1 can revise that arithmetic before Phase 2 starts, by asking whether the 17 are seventeen worlds or a family that converged. Phase 2 partly waits on [0098](0098-the-figure-nests-properly.md) (`shape_field`) and **Phase 4 wholly on [0087](0087-the-line-renderer-draws-a-curve.md)** — authoring `star_pattern` before that settles buys a cohort that has to be redone. Every `human` phase is a **`preset-author` session**; that the owner vocabulary has no word for it is a filed followup. |
| [0106](0106-the-frame-stream-passes-through-a-diffusion-model.md) | The frame stream passes through a diffusion model | approved | dev, human | **Phase 1 is a throwaway spike and Phase 2 is a stop condition** — if a diffused attractor boils, the plan ends there having cost an afternoon and nothing is built. **Fully unblocked: [0101](done/0101-the-engine-renders-a-music-video.md) closed 2026-08-17**, so the Y4M stream every later phase needs exists on stdout today and the `shot/` lane is free. **No ADR yet, deliberately** — it is written between Phases 2 and 3, against the spike's evidence rather than a guess. Ships a script, no weights and no runtime, so `lmv.exe` and the release zip do not change; `core/` is untouched. Contends with nothing (`tools/` + `docs/` only). |

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
close: [0098](0098-the-figure-nests-properly.md) and [0099](done/0099-the-horizon-reaches-its-own-length.md)
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
([backlog 0068](../design-backlog-archive.md#0068--a-swarm-mark-has-no-per-mark-variation-so-the-only-scene-that-can-hold-a-starfield-cannot-make-one-twinkle),
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

**Rewritten 2026-08-18, and this is the live sequence — everything from "Prior sequence notes"
down is history.** Four calls set it: the next stretch is **engine and visual richness**, work runs
in **two lanes**, [0103] waits for [0104], and [0087] goes **early to de-risk** rather than late
because it is large.

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

**The one rule these two lanes need, and it is not obvious.** Both end at the golden corpus — Lane A
adds a baseline, Lane B re-blesses 28 — and `LMV_BLESS` rewrites every baseline the run renders, not
only the intended ones. Worktrees keep that isolated while the lanes are live, so the collision is at
**merge**, not at bless: **[0087] merges `main` and re-blesses only after [0110]'s baseline is on
`main`** — which it now is, as of 2026-08-19 — then checks its diff carries only its own 28. Taken
in the other order, a bless silently reverts the new fixture and nothing fails. **The baseline to
watch is `core/tests/golden/warp_mesh_shader.png`** — the newest entry, and the one a re-bless
would revert most quietly.

### Then, in this order

1. **[0098](0098-the-figure-nests-properly.md)** — after [0087]. Both edit
   `core/src/preset/schema.rs`, but each in one small localized spot, so that is a merge nuisance
   rather than a serialization constraint; the real reason it follows is that lane capacity is two.
2. **[0092](0092-the-engine-draws-an-authored-path.md)** — after 0098 and **not beside it**: both
   rewrite `core/src/render/scenes/shape_field.rs`, which is a genuine conflict. Its Phase 4 reads
   [0087]'s outcome, which by here exists.
4. **[0104]** — once [0087] and 0098 have resolved, since its Phase 2 is partly blocked on 0098 and
   its Phase 4 wholly on [0087]. Every phase is a `preset-author` session in `presets/`.
5. **[0103]** — last. **Decided 2026-08-18: it waits for [0104]**, which closes the disagreement
   this section carried open since 2026-08-16. The cost being avoided is announcing into a library
   where four of eleven systems have one world each; 0101's Phase 5 adds a second reason to hold the
   demo material, a 1080p render still reading as an upscale
   ([backlog 0110](../design-backlog.md)).

### Gap fillers — either lane, any time

- **[0095]** — `core/src/dsp/` only, contends with nothing, and by its own Phase 3 assertion
  `beat`/`beat_index` stay bit-identical, so **it moves no golden baseline**. That is what makes it
  the one plan safe to run beside a lane that is blessing. **Read
  [ADR-0109](../adrs/0109-the-beat-clock-counts-onsets-not-beats.md) first** — `beat_index` counts
  onsets, not beats, and two authoring docs still say otherwise.
- **[0106](0106-the-frame-stream-passes-through-a-diffusion-model.md)** — `tools/` and `docs/` only,
  contends with nothing. Phase 1 is a throwaway spike and Phase 2 a stop condition, so it either
  dies for the cost of an afternoon or runs straight through.

### What this sequence assumes

- **[0087] failing at its stop condition is the live risk, and it is priced rather than hedged.** If
  it ends at ADR-0098's Alternative C, [0092]'s Phase 4 may legitimately be empty and [0104]'s
  Phase 4 needs rescoping *before* it is authored — which is precisely the information early
  placement buys.
- **Two lanes is the ceiling here, not a target.** Only three groups are genuinely disjoint
  (`lines/`, `shape_field.rs`, and `dsp/` + `tools/`); a third lane starts forcing plans that share
  files into one window.

**Superseded 2026-08-18, kept as the record.** The 2026-08-16 sequence follows.

**Rewritten 2026-08-16, when five plans arrived from a competitive review rather than from the
backlog — and that origin is the thing to know about them.** The user asked how this project
compares to the field; the answer was that the engine leads on image pipeline and analysis depth
and trails badly on **content volume and distribution**, neither of which any engine plan
addresses. So [0100]–[0104] are the first cohort here aimed at the product rather than at the
renderer, and they sort into two groups that barely interact:

- **The big one is done: [0100](done/0100-the-engine-speaks-milkdrop.md) closed 2026-08-16** — all
  six dev phases plus the two human ones, the stop condition never fired, and the hedge was not
  needed. What it left behind: the Phase 7 verdict (**merely different**, re-judged after backlog
  0106) and the fidelity work list 0106–0108. Its Phase 4 contention with [0087] is discharged.
- **The cheap ones, in parallel, in this order: [0104] → ~~[0101]~~ → [0103].** 0101 is closed.
  **[0102](done/0102-the-component-ships.md) is done (closed 2026-08-16), so
  [0103](0103-the-project-gets-an-audience.md) Phase 4 is unblocked** — with one asterisk that
  belongs to 0103 rather than to 0102: the component now ships carrying two filed `Medium` defects
  a new user meets first ([backlog 0102](../design-backlog.md) and
  [0103](../design-backlog.md)), and announcing a channel is a poor moment to discover them.
  [0104](0104-the-library-stops-being-lopsided.md) is content work in `presets/` and collides with
  no engine lane. **[0101](done/0101-the-engine-renders-a-music-video.md) closed 2026-08-17**, so
  the `standalone/src/shot/` lane is free and the engine can render a music video today.
  [0103](0103-the-project-gets-an-audience.md) goes last on purpose: it is the only one whose cost
  of being early is real, since announcing before [0104] lands means visitors judge a library where
  four of ten systems have a single world. **0101's Phase 5 added a second reason to hold it**: a
  1080p render currently reads as an upscale ([backlog 0110](../design-backlog.md)), so demo
  material made before that lands shows the engine at its grainiest.

- ~~**Approved 2026-08-16 from a user request, and it collides with exactly one plan:
  [0107](done/0107-the-foobar-menu-picks-a-preset.md).**~~ **Resolved 2026-08-18: 0107 went first
  and closed.** The sequencing question this bullet posed is answered, and the answer costs
  [0103](0103-the-project-gets-an-audience.md)'s amended Phase 1 something: it now restructures a
  menu of five items and a whole-roster submenu rather than the two-item one it was scoped against,
  and 0107's Preset submenu did **not** inherit the layout-edit deference it would have got for
  free the other way round. [Backlog 0103](../design-backlog.md) carries a dated note saying the
  same, and its claim is stronger for it — more of the panel's right-click is now unreachable in
  layout-edit mode. Nothing else about that phase changed; it remains ADR-worthy with no ADR.

- **Added 2026-08-17 from the MilkDrop fidelity backlog:
  [0108](done/0108-the-milkdrop-import-gets-its-tone-back.md).** It takes backlog 0106 + 0107 and
  deliberately leaves 0108 (the conversion tail) filed, on that entry's own instruction that its
  blank-render list is contaminated by both. **It contends with nothing** — `warp_mesh/`, `milkconv/`
  and `core/src/milk/` are touched by no other live plan — so it can take any free session. Two
  things order it against itself rather than against the roster: its Phase 2 `human` gate is what
  makes Plan 0100's central claim re-judgeable, and it grades a tuning the three later phases are
  judged through, so it sits at position 2 rather than at the end. The sweep that produced it also
  falsified backlog 0107's own leading suspect before the plan started — `s_fw` is already
  `AddressMode::Repeat`, and Repeat shifts rather than reflects — which is why Phase 3 names a
  different hypothesis with arithmetic behind it.

- **Out of sequence by design: [0106](0106-the-frame-stream-passes-through-a-diffusion-model.md).**
  Its Phase 1 is a spike that depends on nothing and fits any gap; its Phase 2 is a stop condition
  that may end the plan for the cost of one afternoon. **Its dependency is discharged:
  [0101](done/0101-the-engine-renders-a-music-video.md) closed 2026-08-17, so `shot --render`
  streams Y4M on stdout today and the `standalone/src/shot/` lane is free** — the whole plan is
  takeable whenever, and it either dies cheaply at Phase 2 or runs straight through.

- **Taken first and now closed: [0105](done/0105-the-indexes-go-back-to-being-indexes.md)** (user
  call, 2026-08-16; closed the same day). It went first because it was *preventive* — every plan
  below writes rows into the three roster files at its close, and with its Phase 6 landed those
  closes now produce thin rows on their own, checked by `scripts/check-index-rows.mjs` at the same
  three call sites the link gate occupies. **What that changes for the plans below is one line in
  their close:** the roster row is a pointer under 320 bytes, and the write-up goes to
  `README-archive.md`, the ADR body's `Outcome`, or the backlog archive — never into the row.

~~**One sequencing disagreement is left open rather than decided**~~ — **answered 2026-08-18: [0103]
waits for [0104]**, see the live sequence above. It was left open because it is a product call and
not an architecture one: [0103] wants [0104] to have landed, and [0104] is four phases of content
work with two of them blocked on live engine plans. Shipping the announcement against today's
presets was the defensible other choice — the app is honestly labelled pre-1.0 — and the user made
it the other way.

**Prior sequence notes follow, and they are the record of how the previous roster ordered itself.**

**Rewritten 2026-08-13, from the backlog sweep that filled the empty roster.** Five plans, and the
ordering question is genuinely small because **none of them gates another on capability**. What
orders them is urgency, file contention and who they need:

**Updated 2026-08-13, at [0083]'s close** — it held slot 1 and landed all four `dev` phases the same
day it was written, so the sequence is **four plans** and the one file-contention constraint the
list carried is **discharged**: [0085] was told to run in sequence with [0083] rather than in a
parallel lane because both touch `standalone/src/main.rs`, and [0083] is now off the roster. It can
take any free session. The numbered list below is otherwise unchanged.

1. ~~**[0083]**~~ — **closed 2026-08-13.**
2. ~~**[0084]**~~ — **closed 2026-08-13**, also the day it was written. Both gates now check what
   they claim to; the reactivity sweep's own numbers were recalibrated in the process, so read any
   reactivity figure recorded before that date as a different measurement.
3. ~~**[0085]**~~ — **closed 2026-08-15, all five phases** (the `human` Phase 5 was run hours after
   the close; see Standing). Its
   "before R0 is designed" constraint turned out to be moot — **R0's governor shipped on
   2026-07-30** (Plan 0044 / ADR-0045) and does not read `p99` at all, so the qualification Phase 4
   wrote down guards a *description*, not a live demotion. See roadmap item 3 below.
4. ~~**[0086]**~~ — **closed 2026-08-15 at Phase 2, by its own gate.** Phase 1's instrument landed
   and Phase 2's capture ran on three genres; the verdict named a defect upstream of every cue on
   its shortlist, so Phases 3-5 were superseded rather than executed. Succeeded by **[0095]**,
   which inherits the slot: same subsystem (`core/src/dsp/`), still contends with nothing, still
   needs the user once (Phase 5's re-measure). **Read [ADR-0109] before touching anything in
   `core/src/dsp/tempo.rs` or `downbeat.rs`** — `beat_index` counts onsets, not beats, and two
   authoring docs currently say otherwise.
5. **[0087]** — last, and largest. Touches `core/src/render/scenes/lines/` and owes a re-bless, so it
   wants a lane to itself. **It is also the only plan here that can end early**: two separate gates
   (a cost measurement and a `human` look verdict) can route it to ADR-0098's Alternative C.

**[0093](done/0093-the-backlog-stops-asserting-things-about-a-repo-it-has-not-read.md) — CLOSED
2026-08-15**, the day it was written, all four `dev` phases. The gate exists and runs at all three
call sites, so the reason it was told to go early — *every close from here on writes new backlog
entries, and each one written before the gate exists is another entry nobody will mechanically
check* — is discharged. Its Phase 2 was the cheapest audit this project has of a file it reads to
decide what to build, and it convicted one entry on the first pass (backlog 0093, corrected in place
at the close: the RD family is three worlds, not two, and the third was never measured).

**Its review produced [0094](done/0094-the-two-doc-gates-check-what-they-claim-to.md), which also
closed 2026-08-15** — so the mechanism 0093 built now holds itself up: a live entry with no
verification bullet is a break, the link gate reads `core/tests/fixtures/` again, and the staleness
advisory says so rather than guessing on the shallow CI checkout.

**[0091](done/0091-the-figure-fills-the-frame.md) — CLOSED 2026-08-16**, Phases 1-5; the `human`
Phase 6 carries forward (Standing) and Phase 7 was **cut and filed as
[backlog 0095](../design-backlog.md)**. Kept because two of the three notes below were predictions
the close could grade, and one of them was wrong in an instructive direction:

- **It contends with nothing on the roster, and specifically not with [0087].** The obvious route to
  a nested outline is the line renderer, and [ADR-0105](../adrs/0105-the-mark-roster-becomes-a-fullscreen-distance-field.md)
  rejects it on measured precedent — ~20 nested thin contours maximise both backlog 0073's faceting
  and ADR-0098's vertex bead. Taking the per-pixel route instead means the two plans share no file
  and can run in either order or at the same time.
- **One phase of it is already 80 % proven and lands first.** A rendered measurement
  ([ADR-0106](../adrs/0106-two-tone-graphics-come-from-a-multiply-layer.md)) found that a `multiply`
  layer reaches luma **18.5** where the additive control cannot go below **181.6** — so
  design-backlog 0069's "a dark edge cannot exist inside the composite" is **false for field
  scenes**, and has been since the layer system landed six days after that entry was written.
  Phase 1 measures the one path left open (does it reach the *backdrop*?) and writes the route down.
- **Phase 6 is a cut point that may honestly close negative**, and it is the only part of the plan
  whose value is not already established. *(Written of what the plan file numbers **Phase 7**, the
  converging fan. It closed negative in the cleanest available way: it was never started, because
  Phase 1’s answer — a backdrop cannot be darkened by a layer — says a fan floor and a dark figure
  over it are mutually exclusive at the tones the reference has. It is [backlog 0095](../design-backlog.md).)*
- **The 80 %-proven phase came back with a correction to its own ADR**, which the note above did not
  anticipate. Phase 1 settled the open path negatively (multiply does **not** reach the backdrop —
  at `occlude = 1` the frame is byte-identical over a lit and a black one) **and falsified one of
  ADR-0106’s Negatives**: a particle layer *can* darken, to luma **0.9**, darker than the field
  route’s 18.9. The difference between the two routes is **footprint, not capability**. Both are a
  dated `Outcome` on [ADR-0106](../adrs/0106-two-tone-graphics-come-from-a-multiply-layer.md).

**[0088] closed 2026-08-13**, the day it was written, all seven phases including the `human` look
call — so its sequencing question is discharged and only one line of it is still live guidance:

- **[0087] invalidates the committed gallery, and that stays priced rather than sequenced.** The
  line-renderer plan changes what the curve family draws (three mandala presets were already retired
  on [ADR-0098](../adrs/0098-the-line-renderer-draws-arcs-as-per-pixel-distance-fields.md)), so
  `docs/images/gallery/star_pattern.png` and `docs/images/hero.png` owe a re-render once it lands.
  That is **one argument-free run of `node scripts/docs-shots.mjs`** — the entire reason the images
  are driven from a committed manifest instead of captured by hand — and nothing gates it.
  [ADR-0100](../adrs/0100-documentation-images-are-committed-headless-renders.md) deliberately has no
  CI check that would notice, so the re-run belongs in [0087]'s own close.

**[0089](done/0089-the-framing-contract-stops-lying.md) — CLOSED 2026-08-15**, and it went the way
this note predicted: one unattended session, all three phases, no `human` gate. Kept because the
reasoning is the template for the next short-session plan:

- **It is the only plan on the roster with no `human` phase**, so it closes in one unattended
  session. All three phases are `dev` and the two doc phases are independent of Phase 1 and of each
  other, so a session that runs out of room can stop cleanly after any of them.
- **File contention is genuinely zero against the other four.** Phase 1 touches
  `core/src/render/scenes/particles/ifs.rs` and its tests; Phases 2-3 touch `presets/README.md`.
  Nothing else on the roster is in either. The one thing to watch is [0088], which rewrites the
  *authoring docs* — but it adds `docs/preset-guide.md` and `docs/preset-tuning-walkthrough.md` as new
  files and leaves the three references' shape alone by
  [ADR-0101](../adrs/0101-the-preset-docs-gain-a-tutorial-layer-rather-than-a-merge.md)'s own rule, so
  the two touch `presets/README.md` for different reasons and in different sections. That ordering
  held — [0089] landed 2026-08-15, two days after [0088], so the tutorial predates the two facts
  rather than following them. No conflict resulted; the sections do not overlap.
- **It gates nothing.** Phase 1 is a contract restatement and a test; it changes no behaviour and
  moves no pixels.

**[0090](done/0090-the-emitters-source-moves.md) — CLOSED 2026-08-15**, all four `dev` phases; the
`human` Phase 5 stands (see Standing). Kept because two of the three notes below turned out to be
predictions the close could grade:

- **It contended with nothing, as written**, and nothing else touched the emitter while it ran.
- **Phase 3 was the designed cut point and was not cut** — and it is the phase that earned the plan.
  `prewarm` was not in the interview; it exists because grounding the gate argument found a *second*
  warm-up the backlog entry never named (the pool starts empty and fills at `spawn_rate`, whatever
  `source_y` is). **Its measurement named a different wall than the plan expected**: the animation
  gate passes the sparse draft **cold** at `0.0629` against a `0.01` floor, and it is `sanity` that
  convicts the slow draft blank at `prewarm = 0` (0 of 10 radial shells) and passes it at
  `prewarm = 1` (10 of 10). No floor moved — the plan forbade it and the close honoured it.
- **Its `human` Phase 5 is still the reason the plan exists**, not a trailing verdict: the two looks
  (a quiet drifting field, and a point fountain) are what backlog 0068 had been asking for, and two
  questions no test can answer ride it — whether `spawn_fade` actually hides the pop, and whether a
  prewarmed world switches in badly.

**Three of the five carried a `human` phase**, so none closes in one unattended session — the same
property the previous roster had. [0083] is the exception that proves it: its four `dev` phases
closed the plan, and its `human` Phase 5 carried forward under Standing rather than holding it open.
**[0089] is now a second exception, and a cleaner one**: it has no `human` phase at all.

**One thing worth stating because it is the whole reason these five exist:** the sweep found the
backlog carrying **twenty discharged entries** it had never archived, and **two entries whose premise
was false when written** — backlog 0078 and backlog 0081, both asserting an *absence* that the repo
already contained (those are *backlog* numbers, not the plans this file's `[0078]` / `[0081]`
resolve to). What was left after that was smaller and much sharper than the raw count suggested.

**Prior sequence notes are kept below as the record of how the roster emptied**, not as live
guidance.

**Rewritten 2026-08-09, at [0046]'s close, because the shape of this roster changed rather than
its order.** The previous sequence was written when [0075] had four open engine prerequisites, and
it opened by naming [0046] "the critical path". **All four have now landed** — [0071], [0064]
and [0067] within a week of each other, and [0046] at this close. [0068] landed the same day and
changes nothing downstream by construction (it shipped a diagnosis and no fix, on purpose).

**So there is no critical path any more, and that is the headline.** What is left is three plans
that no longer gate each other on capability — only on *file contention* and on one deliberate
preference. Two consequences worth stating plainly, because a reader skimming rows would miss both:

- **[0075]'s Phases 4+ are unblocked.** The renaissance was waiting on this and nothing else. Its
  Phases 1-3 were already ungated; the whole plan is now takeable.
- **The [0053]/[0046] concurrency collision is discharged**, not deferred. [0046] landed first,
  [0053] ran second and has now **closed** (2026-08-09) — so the sequence below is two plans, and
  the bind-group-layout guard [0076] was told to wait for is in place.

**No plan here closes in one session** — every remaining one carries a `human` phase, so plan on
bringing the user in rather than picking on that criterion.

**Updated 2026-08-13, at [0079](done/0079-the-attractor-learns-new-figures.md)'s close — and this
section is now empty, which is the headline.** 0079 held the only slot and landed all six phases,
both `human` gates included. There is no sequence because there are no plans: every ordering
question this section has answered since it was rewritten is discharged. The paragraphs below are
kept as the record of how it emptied, not as live guidance.

**One consequence worth stating before it is rediscovered:** the content lane's standing list grew
rather than shrank. It is now **four** passes on overlapping ground — [0077]'s quiet sky, [0080]'s
dusk world, [0081]'s galaxy world, and this plan's own Followup (a content pass binding `tuple` on
the attractor worlds) — plus [0071]'s `occlude` retune. The first three are one sitting by
construction; the fourth is a different family and a different sitting.

**Updated 2026-08-12, at [0081](done/0081-the-sky-gets-a-galaxy.md)'s close** — it held slot 1 and
landed the same day it was written, so the sequence is **one plan** and there is no ordering question
left. **The engine side of the sky family is finished**: ground ([0080]), dither ([0082]) and band
([0081]) all shipped inside two days, which means the three standing content items below are now
authored against a surface that will not move under them. What remains of 0081 is its `human` Phase 6
— judge the galaxy against the reference photograph — which gates no plan and is listed under
Standing.

**Updated 2026-08-12, when [0082](done/0082-the-gradient-stops-banding.md) closed** — it held slot 1
and shipped the same day it was written — from
settling [0080](done/0080-the-sky-gets-a-horizon.md) Phase 7 rather than from a new want. That phase
asked whether the ramp bands; it does, measured, and the user's verdict is *"reads as light, but the
banding is visible"* — so the look passes and the quantization is the defect. **0080's own
arithmetic pointed the wrong way** ("two pixels per 8-bit level is the classic Mach-band
configuration" — two px/level is the *safe*, dense case), which is recorded in
[ADR-0096](../adrs/0096-the-display-write-dithers.md) because that sentence would otherwise send the
next reader hunting in the bright end. It went **first** by the user's call, and has landed: the galaxy's
band is a second wide smooth gradient, and building it onto an undithered chain would have confounded
its own `human` verdict with a defect already known about. [0081](done/0081-the-sky-gets-a-galaxy.md) is
now unblocked and born onto a chain that dithers.

**Updated 2026-08-12, when [0081](done/0081-the-sky-gets-a-galaxy.md) was written**, hours after [0080]
closed and from judging that plan's own output in the running app. It goes first by the user's
explicit call at the interview — offered "ship the sky now and plan the band later" versus "build
the band first, then author the world once", they chose the second. So this is deliberately *not*
the smaller-first ordering: the world waits on the capability rather than being authored twice.
The two plans do not contend for files — 0081 is the backdrop pre-pass, 0079 is the attractor's
tuple tables — which is the same non-collision [0080] and [0079] had.

**One consequence worth stating, because it is easy to miss:** the content lane now has **three**
standing items on one family of looks — [0077]'s Phase 5 (Perseids' quiet sky), [0080]'s Phase 7
(the dusk ground), and this plan's world. They are one pass, not three, and walking the family once
is the point.

**Updated 2026-08-12, at [0080](done/0080-the-sky-gets-a-horizon.md)'s close.** It landed — all
six `dev` phases, no blockers and no majors at review — so the sequence is **one plan**, and for
the first time since this section was rewritten there is no ordering question left to answer. Its
Phase 7 (judge the dusk ground against the reference photo) is `human` and standing, not queued;
see Standing below. It gates nothing, and by construction it does not contend with [0079] for files.

**Updated 2026-08-12, at [0078](done/0078-the-ink-learns-to-bite.md)'s close.** The second of the
three landed — both `dev` phases, no blockers at review — so the sequence is one plan. Its Phase 3
(the ink worlds re-judge, content lane) is standing, not queued; see Standing below. It does not
gate [0079].

**Updated 2026-08-12, at [0077](done/0077-the-quiet-sky.md)'s close.** The first of the three
promoted handoff plans landed — all four `dev` phases, no blockers at review — so the sparse
idiom is gateable *before* any more sparse content is authored, which was the whole point of
its position. Its Phase 5 (the quiet sky itself, content lane) is standing, not queued — see
Standing below; it does not gate 0078 or 0079.

**Updated 2026-08-11, at [0075]'s close.** The renaissance is done — the sequence's anchor
("[0075] last by design") is discharged, and what remains is exactly the three promoted
handoff plans in the order the [0076]-close note below already set: **0077, then 0078, then
0079**.

**Updated 2026-08-11, at [0076]'s close.** The 2026-08-11 handoff confirmation ("[0076] remains
the next execution step") is discharged — [0076] landed the same day, all five phases, no
blockers at review. Cohort 6 authors against `presets/README.md`'s new `[layer]` section.
**The promoted handoff items have their plans** (written the same day, user-decided by
interview): [0077](done/0077-the-quiet-sky.md) (sparse idiom + swarm individuation, closes
0009/0068/0085/0088), [0078](done/0078-the-ink-learns-to-bite.md) (ink contrast, closes 0084),
[0079](done/0079-the-attractor-learns-new-figures.md) (tuple roster + measured morph paths, closes
0055). Execution order after [0075]'s remainder: **0077, then 0078 (small — any free
session), then 0079** (largest, two `human` curation gates).

### The baseline-drift control any pixel-touching plan inherits

Kept here after [0053]'s close because it is not that plan's property — it applies to every plan
that could move a render. **Do not `git diff` the committed baselines.** On this box **eight
baselines drift from their committed bytes under `LMV_BLESS`** (`composite_bloom`, `composite_kaleido`,
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
[0095]: 0095-the-downbeat-fold-gets-a-musical-beat.md
[ADR-0109]: ../adrs/0109-the-beat-clock-counts-onsets-not-beats.md
[0087]: 0087-the-line-renderer-draws-a-curve.md
[0088]: done/0088-the-docs-get-pictures.md
[0089]: done/0089-the-framing-contract-stops-lying.md
[0090]: done/0090-the-emitters-source-moves.md
[0091]: done/0091-the-figure-fills-the-frame.md
[0099]: done/0099-the-horizon-reaches-its-own-length.md
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
  argument, and what was run was an `LMV_BLESS` on the change re-encoding all 19 baselines
  hash-identical to an `LMV_BLESS` on clean `main`. That is the form to reuse — a bless-against-a-
  control, not a diff against the committed files, because three baselines on this machine
  (`lsystem`, `parametric_curve`, `star_pattern`) drift from their committed bytes under `LMV_BLESS`
  on clean `main` too, and a naive diff would have convicted the change of moving them.


## Standing (not a plan)

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
  (the sky [backlog 0068](../design-backlog-archive.md#0068--a-swarm-mark-has-no-per-mark-variation-so-the-only-scene-that-can-hold-a-starfield-cannot-make-one-twinkle)
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
  day**: the foobar plugin builds against the extracted `lmv-core-cabi` and `foo_lmv.dll` loads in
  foobar2000 v2 and renders. That closed the one risk
  [ADR-0072](../adrs/0072-the-c-abi-ships-from-its-own-crate.md) carried into C++ link time — the
  linked artifact renamed to `lmv_core_c.lib`, and CI has no plugin job that would have caught a
  stale path. **Phase 9 needed a CI run rather than this machine, and got one:** run
  [`32272926929`](https://github.com/IgorKonovalov/light-music-visualizer/actions/runs/32272926929)
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
  the [0046] arrangement, so nothing includes it, no test names it and `LMV_BLESS` does not touch
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
[0104]: 0104-the-library-stops-being-lopsided.md
