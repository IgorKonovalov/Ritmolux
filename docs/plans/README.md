# Plans index

The one-minute "what's in flight" view. Read this first each session instead of
re-deriving state from `git log`. Completed plans move to `done/`; their full
close write-ups move to [README-archive.md](README-archive.md).

**Next free number: 0255** (ADRs are a separate sequence — next free there is **0268**; 0200 is reserved for Plan 0186 Phase 2.)

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
| [0192](0192-the-component-reaches-its-audience.md) | The component reaches its audience | approved | human | 0103's Phases 5-6 plus the release they stand on. v0.143.0-v0.146.0 each shipped six artifacts, `foobar` green; v0.146.1 and v0.147.x lost the macOS pair to ADR-0251's break, repaired in 0227. |
| [0214](0214-the-linux-arm-reports-back.md) | The Linux arm reports back | approved | human, dev | The readings 0120 cannot take: the `ubuntu-latest` arm's six steps and the adapter it resolves, a dispatch dry run's six artifacts, the tarball on the box. Three of four are `human`. Unblocked: 0120 closed 2026-09-22. |
| [0133](0133-the-engine-drives-the-lights.md) | The engine drives the lights | approved | dev, human | ADR-0145 + 0174 (proposed): Art-Net. Phases 1-3 landed on its branch. **Postponed 2026-09-18, off the queue: Phase 9 is the rig and its date is unknown.** Phases 4-8 need no rig. |
| [0246](0246-the-rig-session-measures-the-wave-modes-and-judges-the-fourth-gate.md) | The rig session measures the wave modes and judges the fourth gate | approved | human, dev | 0202's Phases 4-7. Not queued until the owner has the Windows rig; commit Phase 1's eight rows on main, then queue. |
| [0243](0243-the-full-suite-gets-faster.md) | The full suite gets faster | approved | dev | Human-started, not conductor. Measures llvmpipe thread caps, batch size, priority; applies only what beats baseline 3 of 3. |
| [0252](0252-the-lost-preset-ask-is-located-and-answered.md) | The lost preset ask is located and answered | approved | dev, studio-builder, human | ADR-0265: `ctl/preset/req` acked by `preset_ack`; studio and tests resend. Studio spec-diff test red from Phase 2 to 5. Phase 6 is a blocking loaded run. |
| [0253](0253-the-labyrinth-shows-its-longest-path.md) | The labyrinth shows its longest path | approved | dev, human | ADR-0266: GPU double sweep while the maze is quiet; reveals, fades on a bite. Phase 1 reads pass cost and change counts that set Phase 2-3 constants. |
| [0254](0254-the-studio-judges-a-preset-set.md) | The studio judges a preset set | approved | dev, studio-builder, human | ADR-0267: a Judge view over the studio's one player, after Phase 1's `--marks` flag. Replaces 0232's scratch scripts. Phase 4 is non-blocking. |
<!-- roster:end -->

~~**Added 2026-09-14 - [0170], [0171], [0172] and [0173] are approved, and they run as two
lanes.**~~ - **fully spent 2026-09-14**, when [0172] closed behind [0171]. All four plans closed the
same day in the order the note set. What survives it is one pointer: [0142] reads the MilkDrop source
at `xeiraex/milkdrop2` `d4c843a`, the commit 0173's log names. The note is
[in the archive](README-archive.md#prior-sequencing-notes-superseded).

[0170]: done/0170-the-horizon-reads-the-frames-own-ground.md
[0171]: done/0171-one-stall-policy-and-a-guarded-clock.md
[0172]: done/0172-the-studios-readings-become-true.md
[0173]: done/0173-the-milkdrop-geometry-reads-the-source.md

~~**Added 2026-09-09 — [0158] and [0159] are drafted, and they are a program rather than a
pair.**~~ — **fully spent 2026-09-10**, when [0159] closed behind [0158]. Both halves of the
program landed in the order the note argued. What survives it is the tail it always named: clip
rendering, show projects and the diffusion pass from the studio are each a later plan with its own
interview, and ADR-0175's `render` subcommand decision is still recorded with nothing built against
it. The note is [in the archive](README-archive.md#prior-sequencing-notes-superseded).

[0158]: done/0158-the-player-grows-a-studio-facing-surface.md
[0159]: done/0159-the-studio-opens.md
[0160]: done/0160-the-silhouettes-preconditions-stop-being-silent.md
[0161]: done/0161-the-structural-parameter-is-held.md
[0162]: done/0162-the-curve-families.md
[0163]: done/0163-the-analytic-field.md
[0164]: done/0164-the-cellular-system.md
[0165]: done/0165-the-release-path-stops-being-the-first-compile.md
[0167]: done/0167-the-studio-becomes-handable.md
[0168]: done/0168-the-studio-stops-surprising-the-author.md
[0180]: ../adrs/0180-a-mathematical-world-joins-a-system-as-a-family-and-a-structural-parameter-is-held.md

~~**Added 2026-09-07 - [0157] is drafted, and it is the only plan that unblocks `main`.**~~ - **closed 2026-09-07.** Both phases landed the same day the note was written; `main` is green and the route gate passes against a built site with `dist/api/` populated. The note is in [README-archive.md](README-archive.md) under `## Prior sequencing notes (superseded)`, which also records the one durable half: the cost probes still run in `-P fast` on every arm of every push.

[0157]: done/0157-the-cost-probes-estimate-a-duration.md

~~**Added 2026-09-06 — [0153] is approved.**~~ — **closed 2026-09-07.** Both phases landed and ADR-0165 is accepted, so the disk reading the approval rested on is spent. The note is in [README-archive.md](README-archive.md) under `## Prior sequencing notes (superseded)`.

[0153]: done/0153-the-debug-tree-stops-carrying-dependency-line-tables.md

~~**Added 2026-09-06 - [0156] is drafted, and it is the documentation lane's next plan.**~~ - **closed 2026-09-06.** Its Phase 7 landed after the `main` merge the note asks for, and its Phase 2 shortened the `README.md` that [0103] Phase 2 reorders - which is now that plan's own roster constraint. The note is in [README-archive.md](README-archive.md) under `## Prior sequencing notes (superseded)`.

[0156]: done/0156-the-site-becomes-the-reference.md

~~**Added 2026-09-04 — [0152] is approved, and it runs before [0133] and [0147] rather than after.**~~ — **closed 2026-09-05.** Its Phase 3 re-pointed [0133] and [0147], so the window this note existed to close is gone and both plans now read `/rlx/v1`. The note is in [README-archive.md](README-archive.md) under `## Prior sequencing notes (superseded)`.

**Phase 5 outlived the plan and is the operator's.** OSC has no negotiation and no error channel, so a binding left on `/lmv/v1` stops firing and looks exactly like a fixture that is not moving. It was extracted at the close to [`docs/on-device-validation.md`](../on-device-validation.md), which is where `human` work that waits on hardware or a rig lives; schedule it against a rig session with a playing track, and keep the old show file until all fourteen addresses are confirmed.

[0152]: done/0152-the-osc-root-becomes-rlx.md

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
- ~~**[0147]'s Phase 1 wants to land before [0133] is built.**~~ — **spent 2026-09-06**, when
  [0147] closed with that phase landed: `README.md` states the `level/*` ceiling property, so
  [0133] meets it already written. The note is in [README-archive.md](README-archive.md) under
  `## Prior sequencing notes (superseded)`. **Backlog 0163 stays live for its other half** —
  `docs/presets.md` still does not state the property for the expression grammar.
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
  half; 0147 Phase 6 ran on 2026-09-06 and published the discrete row, and the degrade path **stayed
  unexercised** — a recordable finding, as that phase was written to allow. Both entries keep the
  half they were promoted without.
- **What stayed filed, and why.** The engine/content entries no plan here takes — 0140 (the band
  contour), 0146 (`warp_mesh` colours at deposit), 0100, 0101, 0095, 0092, 0069 — are look-affecting
  and larger, and several price themselves as a redesign of the composite. Backlog 0021 and 0032
  remain parked with named triggers. **0157 and 0158 are not unclaimed** despite reading that way
  from the roster: [0133]'s Phase 3 closes both.
- **One design premise was reopened and one was left alone.** [ADR-0158] supersedes the *geometry*
  half of ADR-0041 because that ADR rejected a true miter on the ground that *"a mitred corner and a
  rounded one differ by less than the blur that is already there"* — and Plan 0114 took
  `DEFAULT_SOFTNESS` to `0.25`. Its per-endpoint *granularity* stands and is what makes the fix
  cheap. [ADR-0159] settles what backlog 0177 filed rather than answered — the plugin's own cap —
  and does **not** touch the standalone exe's, which keeps its inherited value and gains only a unit.

[0143]: done/0143-the-documentation-gets-a-front-end.md
[0147]: done/0147-what-the-show-costs-and-what-its-numbers-mean.md
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
  **Answered 2026-09-18, by [0142]'s Phase 4 gate and Phase 6 decision.** The third re-take read one
  pair better, two good, one fixed, two still washed and one wrong on structure — ADR-0113 now
  carries that as its third `Outcome`, so the trigger is **unbought a third time**. Backlog 0109
  carries the dated no-go and 0108 a dated re-rank behind it; neither is picked up, and the wash the
  bullet above rides on is named at the reference's source and partly repaired rather than open.
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

**Sequencing: both new plans ran after [0087] and [0092]** — all three are now closed. [0098]
contended with [0092] on `shape_field.rs` and [0087]'s stop condition was worth resolving before more
line-adjacent work; the choice was the user's at the planning interview. [0099] contended with nothing and did not have to
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

**Added 2026-09-23 — the queue holds three plans chosen for needing no hardware, and one arithmetic
governs the choice.** Both lanes were stalled on `human` phases — [0207]'s Floor reading (since
deferred) and [0224]'s Windows reading (still owed) — and nothing else was queued. [0215], [0216] and
[0217] are the only approved plans whose every phase belongs to an implementer, so each runs to a
close instead of parking. **A parked plan keeps its worktree**: 0224 holds one, `max_open_worktrees`
is 3, and two live lanes take the rest, so a plan that parks partway costs the third slot and stops
the next lane. That is why [0202], [0211], [0212] and [0220] stay off the queue despite being ready —
each parks at a human phase mid-plan — and they are the first to add once a park settles.

**Lane a serialises the operator surface, lane b stays out of it.** [0206]'s browser pane, [0216]'s
rotation rows and hotkeys and [0217]'s settings-file gate all edit the standalone's menu, its
`config.toml` keys and the same two operator pages, so they run one after another rather than beside
each other; 0216 precedes 0217 so that 0217's gate is written against a settings surface that already
carries the two rotation orders. [0215]'s work is the `Scene` capability seam in `core`, which is what
makes it the safe parallel rather than a second editor of the same files.

- **[0216] closed 2026-09-23**, first of the three and lane a's first link. The rest of the note
  stands: [0217] is next on lane a, against the settings surface 0216 just widened, and [0215] runs
  beside it.
- **[0224] closed 2026-09-23**, and with it the arithmetic above loosens: its Windows reading was
  taken on the owner's box and the plan ran to a close, so it no longer holds a worktree and the
  third `max_open_worktrees` slot is free. The reason [0202], [0211], [0212] and [0220] stay off the
  queue is unchanged — each still parks at a human phase mid-plan.
- **[0215] closed 2026-09-23**, second of the three and the whole of lane b's parallel. What is left
  of the note is [0217] on lane a, and a free lane beside it.
- **[0225] takes lane b 2026-09-24, ahead of [0223].** The `Pages` workflow is red and the site has
  not deployed since 2026-09-23, so the plan that repairs it goes first. It is the same shape as the
  three above — every phase belongs to `dev`, so it runs to a close instead of parking — and it edits
  only `site/` and `scripts/`, which is what makes it safe beside anything on lane a.
- **[0218] is approved 2026-09-24 and deliberately not queued.** It is a fourth case of the
  arithmetic above, and the earliest: its `human` phase is **Phase 1**, so a lane would park before
  running anything. The readings are owed into its own log first, and [0214] has still to close.
- **[0217] closed 2026-09-24**, last of the three and the end of lane a's serial run. All three
  are now closed, and lane a is free.
- **[0225] closed 2026-09-24**, and lane b is free for [0223]. The split recurses, the largest route
  is 29,528 B, and `Pages` goes green on the push that carries it.
- **[0220] closed 2026-09-24** with its Phase 7 owed (ADR-0249), so it holds no lane. Of the four
  the first note kept off the queue, [0202], [0211] and [0212] remain.
- **[0223] closed 2026-09-26**, and lane b is free again. The integrated Rich row is 0.75; at
  2560x1440 no Rich scale holds, which keeps backlog 0259 live.
- **[0211] closed 2026-09-27.** Its phases ran interactively on `main`, and it was queued only for
  its review and close. The owner's verdict closed it at Phase 2. Of the plans the first note kept
  off the queue, [0202] and [0212] remain.
- **[0212] closed 2026-09-28.** Its human phase was taken interactively, and it was queued for its
  review and close. Of the plans the first note kept off the queue, only [0202] remains.
- **[0202] closed 2026-10-02** at Phase 3, its rig and corpus phases split to [0246]. Of the plans
  the first note kept off the queue, none remains.
- **[0218] closed 2026-10-06** with its Phase 6 rehearsal and Phase 1's second-display readings
  owed (ADR-0249). The goldens assert on lavapipe, on the Arch box and on CI's `ubuntu-latest` arm.

[0202]: done/0202-the-three-mechanisms-get-their-gate.md
[0246]: 0246-the-rig-session-measures-the-wave-modes-and-judges-the-fourth-gate.md
[0206]: done/0206-the-browser-shows-the-look.md
[0207]: done/0207-the-commitments-get-their-instruments.md
[0211]: done/0211-the-diffused-frames-resolution-is-measured-before-it-is-designed.md
[0212]: done/0212-the-diffused-render-gains-a-timeline.md
[0215]: done/0215-the-wide-seams-narrow-and-a-guard-holds-them.md
[0216]: done/0216-the-operator-owns-the-order.md
[0217]: done/0217-every-setting-has-a-file-and-a-gate-says-so.md
[0220]: done/0220-the-dependencies-catch-up-and-npm-gets-its-gate.md
[0218]: done/0218-the-reference-machine-becomes-arch.md
[0214]: 0214-the-linux-arm-reports-back.md
[0223]: done/0223-the-heavy-presets-fit-the-integrated-gpu.md
[0225]: done/0225-the-split-goes-one-level-deeper.md
[0224]: done/0224-the-adapter-becomes-a-setting.md

**Spent 2026-09-22, when [0222] closed behind [0221].** The note as written:
**Added 2026-09-22, later - [0222] is approved and queued in lane `a` behind [0221], with `after: ["0221"]`.**
0222 repairs the check that parked 0221, but 0221 settles without it (`adopt-close 0221`, then
`resume 0221`), since an adopted close carries no findings for the check to read. The `after` holds
0222 until 0221 has merged, so the lane never runs the fix beside the plan it was found on.

[0222]: done/0222-a-repaired-finding-follows-its-file-into-done.md

**Added 2026-09-22 - `queue.json` holds [0221] and nothing else, until 0219 Phase 5's run is recorded.**
[0221] is a docs-only fixture written so the conductor's first Linux run has a plan to take to a
close. The queue dropped the rest on purpose. [0120] is done. [0202] is mid-flight on its
`origin/plan-0202-...` branch, and its Phases 5-6 need the rig. [0207] and [0206] come off lane `a`
so that `run` picks up the fixture alone. 0206's `after: ["0207"]` stays in `plans`, where an
unlisted plan's entry is inert. Re-queue 0207 and 0206 once 0219 Phase 5 closes.
**Due 2026-09-22:** 0219 closed; the re-queue of 0207 and 0206 is the owner's to make before the next `run`.
**Half spent 2026-09-23**, when 0207 was re-queued, ran and closed. What is left of the note is 0206:
its `after: ["0207"]` is now satisfied, so nothing holds it but the re-queue itself.
**Fully spent 2026-09-26**, when 0206 was re-queued, ran and closed.

[0221]: done/0221-the-arch-block-names-the-studios-settings-file.md
[0206]: done/0206-the-browser-shows-the-look.md

~~**Added 2026-09-20 - [0215] is approved and runs last, behind everything in the roster
above.**~~ — **spent 2026-09-23**, when it closed. Moved verbatim to
[README-archive.md](README-archive.md)'s `## Prior sequencing notes (superseded)`, which also
records what the ordering was and was not worth: 0215 ran ahead of [0206] and [0209] rather than
behind them, and the dated-evidence rule inside it is why that cost nothing.

[0215]: done/0215-the-wide-seams-narrow-and-a-guard-holds-them.md


~~**Added 2026-09-19 - [0204] is approved, it sits behind [0201], and it is not a conductor plan.**~~
— **spent 2026-10-01**, when 0204 closed. Moved verbatim to
[README-archive.md](README-archive.md)'s `## Prior sequencing notes (superseded)`.

[0204]: done/0204-the-library-learns-from-the-corpus-it-will-not-ship.md

**Added 2026-09-19 - a backlog round promoted seven plans, [0196] through [0203], and the order is
infrastructure first because three of the others need it.** Seventeen live entries left the file for
the archive (ADR-0206); four more stay live with a dated bullet naming the half a plan took. The
round's shape, since most of what it decided was sequencing:

- **[0196] and [0197] share a lane, in either order.** Both edit `tools/conductor/lib/gate.mjs`,
  `lib/lane.mjs` and the conductor README; run in parallel they conflict on every one. They are first
  because two later plans wait on them: [0201] and [0203] regenerate parameter surfaces, which a
  conductor session cannot do until 0197's Phase 4 admits the two `RLX_UPDATE_*` spellings
  (backlog 0250), and [0198]'s `studio-builder` phase needs the lane install 0196 Phase 4 adds
  (backlog 0242).
  **Fully spent 2026-09-19 — [0196] and [0197] both closed**, so the shared-file conflict
  is gone; the lane install [0198] Phase 5 waits on is in `main`, and the `RLX_UPDATE_*`
  wall [0201] and [0203] were waiting on is down.
- ~~**[0199] is independent and can run beside them**, in the other lane. It touches `.config/nextest.toml`
  and the sweep harness and nothing either infrastructure plan opens.~~ — **spent 2026-09-19, when
  [0199] closed.** It ran beside them exactly as written and conflicted with neither.
  is gone; the `RLX_UPDATE_*` wall [0201] and [0203] were waiting on is down, and [0198] closed the
  same day on the lane install 0196 Phase 4 added.
- **[0199] is independent and can run beside them**, in the other lane. It touches `.config/nextest.toml`
  and the sweep harness and nothing either infrastructure plan opens.
- **[0202] parks twice, by design.** Its Phases 5 and 6 are `human` - a rig session against
  `foo_vis_milk2` and a corpus census - and both need what lives outside this checkout. Its first four
  phases need no rig, so the parks come last.
- **[0203] is last, and it is the weakest-justified plan of the seven.** Each of its three entries says
  to take it when someone wants the thing; only the roster morph has a want on record. Every default is
  an identity, so the cost of being early is the lane's time.
  **Spent 2026-09-20, when [0203] closed** — the seven are through, and the judgement the bullet
  deferred is now `preset-author`'s: a look per lever is the only thing that can say whether any of
  the three earned its place.
- **Two clusters were deliberately not promoted.** Backlog 0092 (lighting) has a trigger that fired and
  resolved negatively, and its own instruction is that a future lighting plan needs a fresh want; and
  backlog 0109's reach work stays unbought for the third time, by the verdict [0202] Phase 5 re-takes
  rather than by anyone's inattention.

[0196]: done/0196-the-gate-roster-stops-drifting.md
[0197]: done/0197-the-conductor-becomes-operable.md
[0198]: done/0198-the-control-path-stops-failing-quietly.md
[0199]: done/0199-the-gates-cost-is-measured-before-it-is-cut.md
[0198]: done/0198-the-control-path-stops-failing-quietly.md
[0199]: 0199-the-gates-cost-is-measured-before-it-is-cut.md
[0201]: done/0201-the-warp-surface-stops-lying.md
[0202]: done/0202-the-three-mechanisms-get-their-gate.md
[0203]: done/0203-the-figure-gains-the-levers-it-was-measured-to-lack.md

**Added 2026-09-16, at [0180]'s close - the conductor's stand-down is lifted and the first
unwatched run is the owner's call.** [0180] has landed, which is the condition the 2026-09-15 note
set; that note is [in the archive](README-archive.md). What it leaves standing:

- **The conductor may run again.** [0191]'s served tier and [0190]'s run-survival work are on `main`
  and inert until a run uses them, so the first plan the conductor takes is also the first
  measurement of either - and ADR-0211's `Outcome` is owed by it.
- **Lane b stays retired as a question**, not deferred: [0189] measured the serialized suite fraction
  and a second lane buys nothing while single-lane runs still park on their own infrastructure.
  `queue.json` keeps its empty `b` lane, so re-opening costs nothing.
- ~~**[0142] is next in the engine lane and needs a human**, which is the one thing the conductor
  cannot supply: its Phase 4 is a rig session against `foo_vis_milk2`, and that session now also owes
  ADR-0199's unit-scale mode-0 capture and the "does the reference seam?" reading.~~ — **spent
  2026-09-18**, when [0142] closed. The rig session ran, and all three asks came back: the seven
  pairs (one better, two good, one fixed, two washed, one wrong on structure), the mode-0 capture
  (`k ~ 0.068` against mode 6's `0.158`, which does **not** support ADR-0199's inference, now its
  `Outcome`) and the seam reading (**none in either renderer**, which closes that half of backlog
  0215). What it leaves for the engine lane is not a rig: the two pairs still washed point at a
  per-frame deposit against a per-second transform rate, and the plan names the `shot --render`
  comparison that would settle it.

[0189]: done/0189-the-conductor-can-be-watched-and-stops-re-proving-a-green-tree.md
[0190]: done/0190-the-conductor-survives-a-run-nobody-is-watching.md
[0142]: done/0142-the-milkdrop-import-earns-its-verdict.md
[0191]: done/0191-a-green-tree-is-not-tested-four-times.md

**Added 2026-09-14 - a backlog sweep drafted [0176] through [0186] and amended all eight active
plans; delivery and infrastructure go first, which is the user's call.** Every active plan was
re-checked against the tree and none was withdrawn; each carries a dated `Amended 2026-09-14` note
naming what moved. The order:

- **Delivery and infrastructure lane.** [0174] closed first, then [0176] (both 2026-09-14), which
  discharges the wait on a tag that reliably reaches origin — an [0103] phase then, [0192] Phase 1
  now. [0177] closed 2026-09-15, its fold re-derived from 0174's final run-alone filter. [0166] closed
  2026-09-17 and took the gate-count prose count-free, as [0176] had; [0178] closed 2026-09-18 onto
  that wording and restored no number — it retired the ordinals beside the counts instead.
- **Engine lane.** [0181], [0185] and then [0175] closed 2026-09-15, and none moved a golden.
  [0180] parked `plan_wrong` on 2026-09-14 and was amended on its lane branch 2026-09-15; resuming it
  is next. The opening this replaced is [in the archive](README-archive.md). [0180] runs before [0142]: 0180 re-draws the
  waveform 0142's wash is measured on and needs no rig. ~~[0186] closed 2026-09-17 having edited no
  preset and blessed no golden, so it constrains nothing and [0184] is what is left of that pair.~~ -
  **spent 2026-09-17**, when [0184] closed behind [0186] the same day. Neither blessed a golden; 0184
  landed two presets and re-rendered two gallery cards, which is the whole of what that pair left
  behind.
  [0179] closed 2026-09-16 and [0183] 2026-09-17, moving no baseline.
  [0182] closed 2026-09-15 ahead of them; it touched only the report, so it constrains none.
- **Still gated on a human:** [0133] Phase 9, all three phases of [0192], and three of
  [0214]'s four — [0120] Phase 1 discharged 2026-09-20 on the owner's box, which is what let 0120
  be re-cut into implementation here and witnessing there. [0103]
  closed 2026-09-18, its Phase 4 discharged by the owner the same day - topics and the social
  preview. [0166] Phase 3
  is discharged - the owner read the five translations on 2026-09-16, 44 corrections.
- **All eleven approved 2026-09-14 with [0175], and [0181]'s reversal of an interview pick confirmed.** Backlog 0142's double-advance is
  unreachable - `shares_resources` answers true for any same-system pair, so every such dissolve
  freezes - and the once-per-frame guard chosen for it would guard nothing.
- **Not promoted, on their own instructions:** backlog 0021, 0032, 0038, 0042, 0069, 0075, 0079,
  0087, 0092, 0094, 0095, 0100, 0101, 0154, 0187 (parked on a trigger, routed, declined or waiting
  on hardware); 0108 and 0109 wait on [0142]'s verdict; 0125 and 0126 wait on a human side-by-side
  of the diffusion `quality` profile.

[0176]: done/0176-a-release-tag-reaches-origin.md
[0177]: done/0177-the-test-tree-stops-costing-disk-and-touching-the-machine.md
[0178]: done/0178-what-the-operator-reads-is-true.md
[0179]: done/0179-a-parameters-range-belongs-to-its-family.md
[0180]: done/0180-the-converted-picture-follows-the-source.md
[0181]: done/0181-a-scene-advances-after-its-frames-bindings.md
[0182]: done/0182-the-report-hears-a-counter.md
[0183]: done/0183-a-low-density-is-a-trace-count.md
[0184]: done/0184-a-contour-that-is-an-ink-and-a-warp-field-that-bands.md
[0185]: done/0185-a-fullscreen-field-lets-the-sky-through-with-no-post-stage.md
[0186]: done/0186-the-flatness-gate-tells-a-figure-from-its-ground.md
[0174]: done/0174-the-clock-reading-tests-run-alone.md
[0175]: done/0175-an-eased-value-arrives.md
[0120]: done/0120-the-standalone-ships-on-ubuntu.md
[0214]: 0214-the-linux-arm-reports-back.md
[0166]: done/0166-the-basics-read-in-russian.md

**Rewritten 2026-08-18, and this is the live sequence.** What it replaced — the 2026-08-16
sequence and the prior sequence notes under it — is in
[README-archive.md](README-archive.md) under `## Prior sequencing notes (superseded)`.
Four calls set it: the next stretch is **engine and visual richness**, work runs
in **two lanes**, [0103] waits for [0104], and [0087] goes **early to de-risk** rather than late
because it is large. **All four are spent: [0103] closed 2026-09-18 behind [0104], which is the
last of the four to resolve.**

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

**Added 2026-08-29, second promotion round: [0139], [0140], [0141] and [0142], all `draft`** (the round also promoted [0138], which closed 2026-09-04 — its note is in [README-archive.md](README-archive.md)).
The sweep was re-run with a corrected filter — the first pass's regex was greedy and over-counted
claimed entries — leaving **32 of 58 unclaimed**. Five clusters came out; three entries were
**declined on their own instructions** rather than promoted:

- **[0141] is the one with no contention.** It is the only cluster touching `plugin-foobar/`, which
  no plan on this roster otherwise enters. Its Phase 1 is the exception: backlog 0117 calls itself a
  natural pickup for [0103] Phase 1, which rewrites the same handler.
- **[0142] is the least show-compatible plan on the roster.** Three of its six phases need a free
  GPU and the `foo_vis_milk2` rig staged. It carries the backlog's **only High**.
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

**The 2026-08-28 "what next, functionally" round that produced [0127] and [0128] is spent** — both closed, and its sequencing note moved verbatim to [README-archive.md](README-archive.md)'s `Prior sequencing notes (superseded)` on 2026-09-04. The one thing it parked that is still open is 0127's Phase 3 capture number, which [backlog 0120](../design-backlog.md) carries for the next `warp_mesh` plan.

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

~~**Added 2026-09-10 — [0165] runs beside the two live lanes, and goes first.**~~ — **closed 2026-09-10.** 0165 landed first as planned and is closed, so the ordering it argued is spent; 0159 Phase 6 now merges `main` onto the settled workflows. The note is in [README-archive.md](README-archive.md) under `## Prior sequencing notes (superseded)`.

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
  ([0092](done/0092-the-engine-draws-an-authored-path.md) Phase 4, [0104] Phase 4). Learning that late
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
2. ~~**[0092](done/0092-the-engine-draws-an-authored-path.md)**~~ — **closed 2026-09-09.** It was
   taken from the post-close `main` as this note asked, and its Phase 4 did read [0087]'s outcome:
   the arc chain landed, and Phase 2's measurement had by then made it load-bearing rather than
   optional. The full record is in [README-archive.md](README-archive.md).
4. **[0104]** — ~~once [0087] and 0098 have resolved~~; **both closed 2026-08-27, so it is
   unblocked in full.** Phase 2's `shape_field` cohort now has two coordinates to author against and
   a `rotation` lever, and [0087]'s Phase 4 means `star_pattern` can be authored on the arc
   primitive. `star_mandala_bordered` is the worked example of what that surface reaches. Every
   phase is a `preset-author` session in `presets/`.
5. ~~**[0103]** — last, waiting for [0104].~~ — **closed 2026-09-18**, and with it this whole
   numbered sequence is spent. Both reasons for holding it expired first: [0104] closed and the
   library grew several-fold, and the demo material was shot at 720p. The note is
   [in the archive](README-archive.md#prior-sequencing-notes-superseded).

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

~~**Added 2026-09-22, from the heavy-preset analysis: [0223](done/0223-the-heavy-presets-fit-the-integrated-gpu.md)
runs after 0214 and beside 0218, not before them.**~~ — **spent 2026-09-26**, when 0223 closed. What
it leaves is one open question: whether the per-pass table replaces 0207's frame-cost column or
feeds it. The note is [in the archive](README-archive.md#prior-sequencing-notes-superseded).

~~**Added 2026-09-22: 0224 runs before [0223](done/0223-the-heavy-presets-fit-the-integrated-gpu.md).**~~
— **spent 2026-09-23**, when [0224](done/0224-the-adapter-becomes-a-setting.md) closed. The flip is
on `main`, so 0223 measures against the adapter an operator actually gets, which is all the note
asked for. What it leaves 0223 is one live fact rather than an ordering: the integrated part stays
the default on any machine whose `[output] gpu` names it and on every single-adapter box, so the
tuning is still worth doing. The note is
[in the archive](README-archive.md#prior-sequencing-notes-superseded).

### What this sequence assumes

- ~~**[0087] failing at its stop condition is the live risk, and it is priced rather than hedged.**~~
  — **spent 2026-09-09.** [0087] did not fail: the arc primitive shipped, and [0092] closed with a
  Phase 4 that was **not** empty and in fact carried the plan — its own Phase 2 measurement made the
  arc chain the thing that made a per-pixel contour viable, which is the inverse of the risk this
  bullet priced. [0104]'s Phase 4 still reads it, and needs no rescoping. The note is in
  [README-archive.md](README-archive.md) under `## Prior sequencing notes (superseded)`.
- **Two lanes is the ceiling here, not a target.** Only three groups are genuinely disjoint
  (`lines/`, `shape_field.rs`, and `dsp/` + `tools/`); a third lane starts forcing plans that share
  files into one window.

### The baseline-drift control any pixel-touching plan inherits

Kept here after [0053]'s close because it is not that plan's property — it applies to every plan
that could move a render. **Do not `git diff` the committed baselines.** On this box **eight
baselines drift from their committed bytes under `RLX_BLESS`** (`composite_bloom`, `composite_kaleido`,
`composite_overlap`, `composite_trails`, `line_joint_zigzag`, `lsystem`, `parametric_curve`,
`star_pattern`), so a naive diff convicts eight files the change never touched. Bless every scope
(`--test golden`, then `--test suite -- composite:: line_joints:: attractor_trails::`) and compare
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
[0092]: done/0092-the-engine-draws-an-authored-path.md
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
  re-check condition just landed). **0058 has since closed** by content on 2026-08-04 (`ca43dff`),
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


~~**Added 2026-09-09 - [0161] through [0164] are the mathematics wave.**~~ — **fully spent
2026-09-11**, when [0163] and [0164] closed together behind [0161] and [0162]. All four landed, in
the dependency order the note fixed, and [0180] framed every one of them. The note and the
merge-in-series ordering it carried are
[in the archive](README-archive.md#prior-sequencing-notes-superseded). **What survives it is the
tail the note always named:** Lenia, Voronoi, quasicrystal, hyperbolic tiling and fractal flames are
**placed** by [0180] and built by nobody — each is now a family arm on a system that exists, which
is the whole point of the wave. Fractal flames are the cheapest of the five: they reuse the
attractor's IFS and [0163]'s HDR posture.

**Added 2026-09-10, spent 2026-09-10 when [0161] closed.** That note paired [0161] and [0159] as
the two lanes to open, and named the schema as their one coupling. It is
[in the archive](README-archive.md#prior-sequencing-notes-superseded); what survives it is a debt.
**That debt outlived [0159], which closed on 2026-09-10 without taking it**: 0161 landed `kind` on
every `ParamSpec` and two `[hold]` table descriptors in the exported document, additively, with
`SCHEMA_VERSION` deliberately still at 1 - the body hash is what moved, which is the staleness
signal the studio already compares. So **a schema re-export and a `[hold]` editor are owed by the
next `studio-builder` plan**, not by a rebase and not by anything now in flight.

> **Discharged 2026-09-10 by [0167] Phase 6.** The re-export and the `[hold]` editor both landed —
> and the phase found the gap was not a missing component but a missing **reach**: `hold` is
> declared only as a map *element* kind, and [0159] Phase 8's walk read `key.kind` and stopped at
> `map`. The debt above is spent; the rest of the note stands as the record.

~~**Added 2026-09-10 — [0167] does not close, and [0168] is what unblocks it.**~~ — **spent 2026-09-11**, when 0167 closed short of both `human` phases by the owner's decision. What survives it is the *Standing* entry below. The note is [in the archive](README-archive.md#prior-sequencing-notes-superseded).

## Standing (not a plan)

- **Plan [0167] Phases 7 and 8 — the studio has never been validated by a person** (2026-09-11).
  The plan is `done` on Phases 1-6, **closed with both `human` phases short by the owner's
  decision** so that it would not go stale. That is the second plan to close owing them
  ([0159]'s Phases 10-11 were the first), and this entry is where they now live — not a third plan,
  which would only inherit the pattern. Two items, each with one home:
  - **Phase 8's macOS arm** — the studio beside a fullscreen player for one full track. The Windows
    half ran (`140.1 -> 125.8` fps, −10.2 % against Plan 0159's −15.7 %); the comparison the phase
    asks for needs both machines. **It lives in
    [`docs/on-device-validation.md`](../on-device-validation.md)'s studio entry.**
  - **Phase 7, the tester handoff** — one VJ who has never seen the repository, the studio zip, and
    nothing but `packaging/studio/READ-ME-FIRST.md`. Unblocked since [0168] closed. What they cannot
    do becomes backlog entries with probes.

  **What is waiting on them:** nothing is blocked in code. What is unknown is whether the studio is
  usable by someone other than its author, and a studio release that claims so is unsupported until
  Phase 7 runs.

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
  (`main` at `567bc28`, `rust-cache` restore-key hit, all six jobs green), read at Plan
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
archive first. The list keeps the 15 newest bullets; every older one moved verbatim to
[Closed earlier (index bullets)](README-archive.md#closed-earlier-index-bullets).

<!-- roster:begin cap=320 -->
- [0251 - The seam, the thumbnail GPU and the timeline are tied off](done/0251-the-seam-the-thumbnail-gpu-and-the-timeline-are-tied-off.md) - closed 2026-10-07, Phase 4 owed. Review: **no blockers, no majors, two minors (fixed), one nit.** Version: **0.168.1**. Closed 0261, 0285, 0286. [Write-up](README-archive.md).
- [0250 - The close findings and five small asks are paid](done/0250-the-close-findings-and-five-small-asks-are-paid.md) - closed 2026-10-07. Review: **no blockers, no majors, two minors (filed 0285, 0286), three nits (two fixed).** Version: **0.168.0**. Closed 0260, 0275, 0282-0284. [Write-up](README-archive.md).
- [0218 - The reference machine becomes Arch](done/0218-the-reference-machine-becomes-arch.md) - closed 2026-10-06, Phase 6 owed. Review: **two rounds; 1 major (fixed), 4 minors (three fixed), 1 nit (fixed).** Version: none. ADR-0243 accepted, Outcome. Filed 0284. [Write-up](README-archive.md).
- [0239 - The swarm moves into a real camera](done/0239-the-swarm-moves-into-a-real-camera.md) - closed 2026-10-06, Phases 7-8 owed. Review: **no blockers, no majors, four minors (three fixed), one nit.** Version: **0.167.0**. ADR-0259 accepted, Outcome. [Write-up](README-archive.md).
- [0249 - A dispatched CI job blesses the named WARP baselines](done/0249-a-dispatched-ci-job-blesses-the-named-warp-baselines.md) - closed 2026-10-06, Phase 3 owed. Review: **no blockers, no majors, two minors (one fixed), one nit (fixed).** Version: none. ADR-0264 accepted. [Write-up](README-archive.md).
- [0240 - The attractor projects through the shared camera](done/0240-the-attractor-projects-through-the-shared-camera.md) - closed 2026-10-06, Phases 6-8 owed. Review: **no blockers, no majors, five minors (four fixed).** Version: **0.166.0**. ADR-0260 accepted, Outcome. [Write-up](README-archive.md).
- [0237 - The L-system turtle turns in space, and can grow without end](done/0237-the-l-system-turtle-turns-in-space.md) - closed 2026-10-06, Phases 8-9 owed. Review: **no blockers, no majors, five minors (three fixed).** Version: **0.165.0**. [Write-up](README-archive.md).
- [0248 - 3D strokes gain joins, depth cues and a solid mode](done/0248-3d-strokes-gain-joins-depth-cues-and-a-solid-mode.md) - closed 2026-10-05, Phases 6-8 owed. Review: **no blockers, no majors, three minors (one fixed), one nit (fixed).** Version: **0.164.0**. ADR-0263 accepted. [Write-up](README-archive.md).
- [0238 - The waterfall system](done/0238-the-waterfall-system.md) - closed 2026-10-02, Phase 4 judged: rows show through. Review: **no blockers, no majors, two minors (one fixed).** Version: **0.163.0**. [Write-up](README-archive.md).
- [0247 - The studio renders a neural clip](done/0247-the-studio-renders-a-neural-clip.md) - closed 2026-10-02, Phase 6 owed. Review: **no blockers, no majors, two minors (one fixed), one nit.** Version: **0.162.0**. ADR-0262 accepted. [Write-up](README-archive.md).
- [0236 - Space curves, and the camera becomes a shared block](done/0236-space-curves-and-the-camera-becomes-a-shared-block.md) - closed 2026-10-02, Phase 6 judged: three engine findings. Review: **no blockers, no majors, two minors (one fixed).** Version: **0.161.0**. ADR-0258 accepted. [Write-up](README-archive.md).
- [0244 - Sessions start lighter](done/0244-sessions-start-lighter.md) - closed 2026-10-02, Phase 3 done the same day. Review: **no blockers, no majors, one minor, one nit (both fixed).** Version: none. [Write-up](README-archive.md).
- [0202 - The three mechanisms get their gate](done/0202-the-three-mechanisms-get-their-gate.md) - closed 2026-10-02 at Phase 3. Review: **no blockers, no majors, five minors (four fixed).** Version: **0.160.2**. [Write-up](README-archive.md).
- [0245 - A gate that runs a built binary checks it is current](done/0245-a-gate-that-runs-a-built-binary-checks-it-is-current.md) - closed 2026-10-01. Review: **two rounds; 1 major, 3 minors (all fixed).** Version: **0.160.1**. [Write-up](README-archive.md).
- [0242 - Readiness is read when the plan is approved](done/0242-readiness-is-read-when-the-plan-is-approved.md) - closed 2026-10-01, Phase 3 done 2026-10-02. Review: **no blockers, no majors, one minor (fixed), one nit (open).** Version: **0.160.0**. [Write-up](README-archive.md).

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
- **Conductor-run plans:** an `approved` plan listed in `tools/conductor/queue.json` runs under the
  conductor (ADR-0205): its approval is the go, each same-owner run and the review-and-close are
  separate headless sessions, a `human` phase or any judgement it cannot make parks the plan, the
  close review is committed as the plan's `## Close review` section, and nothing is pushed.

[0100]: done/0100-the-engine-speaks-milkdrop.md
[0101]: done/0101-the-engine-renders-a-music-video.md
[0102]: done/0102-the-component-ships.md
[0103]: done/0103-the-project-gets-an-audience.md
[0192]: 0192-the-component-reaches-its-audience.md
[0193]: done/0193-the-digest-says-what-is-happening-and-where-you-are-needed.md
[0104]: done/0104-the-library-stops-being-lopsided.md
[0115]: done/0115-the-engine-becomes-a-live-video-source.md
[0123]: done/0123-a-gate-a-latch-and-an-ink.md
[0124]: done/0124-the-review-fixes-that-move-no-pixels.md
[0125]: done/0125-the-scenes-share-their-gpu-boilerplate.md
[0126]: done/0126-the-large-files-split-along-their-seams.md
[0127]: done/0127-the-picture-stops-depending-on-the-volume-slider.md
[0128]: done/0128-the-rendered-file-stops-looking-upscaled.md
[0131]: done/0131-the-operator-gets-a-console.md
[0133]: 0133-the-engine-drives-the-lights.md
[0135]: done/0135-the-show-night-surfaces-stop-lying.md
[0136]: done/0136-the-gates-can-convict.md
[0137]: done/0137-the-metrics-measure-light.md
[0138]: done/0138-the-colour-surface-stops-misleading-its-authors.md
[0139]: done/0139-the-render-path-validates-before-it-spends.md
[0140]: done/0140-every-rate-integrates-for-real.md
[0141]: done/0141-the-plugin-seams-stop-drifting.md
[0142]: done/0142-the-milkdrop-import-earns-its-verdict.md
[0145]: done/0145-the-per-phase-gate-stops-paying-for-the-preset-library.md
[0146]: done/0146-the-preset-sweeps-stop-being-one-long-test.md
