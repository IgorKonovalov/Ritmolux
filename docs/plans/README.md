# Plans index

The one-minute "what's in flight" view. Read this first each session instead of
re-deriving state from `git log`. Completed plans move to `done/`; their full
close write-ups move to [README-archive.md](README-archive.md).

**Next free number: 0094** (ADRs are a separate sequence — next free there is **0109**.)

## Active roster

Only plans still in `docs/plans/`. A closed plan leaves this table entirely —
`Recently closed` below and `done/` both already record it. Each row carries at
most two sentences of **live constraint**: what a reader needs to decide whether
to pick this plan up. Anything longer belongs in the plan file, which is where
someone who picked it up is reading.

| Plan | Title | Status | Owner | Live constraint |
|------|-------|--------|-------|-----------------|
| [0086](0086-the-downbeat-finds-a-cue-that-is-not-the-kick.md) | The downbeat finds a cue that is not the kick | approved | dev, human | **Measures before it chooses.** Phase 2 is a `human` gate that needs the user's own music — a synthesized backbeat is a hypothesis about backbeats, which is exactly what cannot settle this. The cue for Phase 3 is named at that gate, so the later phases state properties rather than edits. `CONFIDENCE_THRESHOLD` does not move. |
| [0087](0087-the-line-renderer-draws-a-curve.md) | The line renderer draws a curve | approved | dev, human | The largest, and the only one with a **stop condition**: Phase 3 measures per-pixel cost against the NFR §1 floor tier, and Phase 4 is a `human` look gate placed *before* the biarc work — either can send the plan to ADR-0098's Alternative C. Owes a re-bless (28 baselines) and an ADR-0058 enumeration entry. Watch [ADR-0037](../adrs/0037-internal-grid-is-a-resolution-not-a-shape.md): this family has shipped that bug three times. |
| [0092](0092-the-engine-draws-an-authored-path.md) | The engine draws an authored path | approved | dev, human | **Hard-depends on [0091](0091-the-figure-fills-the-frame.md)** (the scene it draws into) and soft-depends on [0087](0087-the-line-renderer-draws-a-curve.md) — the plan states that disagreement openly: a polyline distance field is complete alone, arcs only lower the arity, so **this is takeable even if 0087 ends at ADR-0098's Alternative C**, and Phase 4 may legitimately be empty. Phase 2's arity ceiling is a **measurement**, not [ADR-0107](../adrs/0107-an-authored-path-is-inline-svg-data-and-it-morphs-by-resampling.md)'s construction estimate. Expect morph degeneracy — Plan 0079 refused 4 of 20 swept pairs by measurement. |
| [0091](0091-the-figure-fills-the-frame.md) | The figure fills the frame | approved | dev, human | From three user reference images, and the gap turned out to be **one thing**: nesting, banding and contours all ship, but no shape-shaped scalar exists at frame scale. Phase 2 is the load-bearing one — `marks.rs:33-37` says the polygon and star arms are deliberately *not* true distance functions outside the silhouette, which is exactly the region contours read. Phase 6 is a stated cut point and may close negative. **Touches no line-renderer file, so it is independent of [0087](0087-the-line-renderer-draws-a-curve.md).** |
| [0093](0093-the-backlog-stops-asserting-things-about-a-repo-it-has-not-read.md) | The backlog stops asserting things about a repo it has not read | approved | dev | **Four falsified entries, one shape** — a claim about the repo, wrong when written or shortly after ([ADR-0108](../adrs/0108-a-backlog-claim-about-the-repo-carries-an-executable-probe.md)). Small and self-contained: one `scripts/` sibling of the link checker, 14 entries to backfill, no `core/` or `standalone/` code, so it **contends with nothing on this roster** and fits a short session. Phase 2 will very likely convict another entry — reporting that is `dev`'s, deciding what to do about it is `architect`'s. Phase 3 lands after Phase 2 on purpose, so the gate cannot red the build on day one. |
| [0090](0090-the-emitters-source-moves.md) | The emitter's source moves | approved | dev, human | Four scalars, four exact-identity defaults, so it **moves zero pixels** — one emitter baseline and three fixtures in scope. **Phase 3 (`prewarm`) is beyond the interview and is the designed cut point**: it exists because the source alone does not deliver the gate argument (~18 % of steady-state population at the 0.5 s capture), and Phases 1/2/4/5 stand without it. Its gate measurement may come back negative — that is a result, and the answer is **not** to move a floor. Phase 4's `systems.md` sweep is a done-when because that minor has been raised at four consecutive closes. |

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
[0090](0090-the-emitters-source-moves.md)**, which between them take the five items the first sweep
left unrouted. [0089] is the three-item sitting (a falsified invariant plus two doc paragraphs that
each named a home and never got a carrier); [0090] came out of an interview on the emitter's fixed
source line ([backlog 0068](../design-backlog.md) option 2) and ships four scalars plus the world they
exist for. One item from that pass is deliberately **not** a plan:
[ADR-0102](../adrs/0102-a-palette-coordinates-edge-is-a-per-preset-choice.md) records the
palette-coordinate edge decision with no plan behind it — the want is real, nobody is blocked, and it
is built when a look asks.

**Two items stay parked and are not being planned**, which is the honest half of the sweep:
[backlog 0021](../design-backlog.md) (the slew release, waiting on an author who wants the look rather
than on an architect's arithmetic) and [backlog 0032](../design-backlog.md) (both analysis windows
sized in samples, so 21 of 64 bands are bin-starved at 96 kHz — pinned by a test, ADR territory,
waiting on someone reporting a mushy low end on a 96 kHz interface).

## Recommended execution sequence

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
3. ~~**[0085]**~~ — **closed 2026-08-15**, four `dev` phases; its `human` Phase 5 is standing. Its
   "before R0 is designed" constraint turned out to be moot — **R0's governor shipped on
   2026-07-30** (Plan 0044 / ADR-0045) and does not read `p99` at all, so the qualification Phase 4
   wrote down guards a *description*, not a live demotion. See roadmap item 3 below.
4. **[0086]** — needs the user twice (Phase 2's capture, Phase 4's re-measure), so plan on bringing
   them in. Touches `core/src/dsp/` and contends with nothing.
5. **[0087]** — last, and largest. Touches `core/src/render/scenes/lines/` and owes a re-bless, so it
   wants a lane to itself. **It is also the only plan here that can end early**: two separate gates
   (a cost measurement and a `human` look verdict) can route it to ADR-0098's Alternative C.

**Added 2026-08-15 at [0085]'s close: [0093](0093-the-backlog-stops-asserting-things-about-a-repo-it-has-not-read.md)**,
and it sits outside the sequence above rather than in it. It touches only `scripts/`, `docs/`,
`.githooks/` and the CI `links` job — **no Rust, no GPU, no re-bless, and no file any other plan on
this roster names** — so it contends with nothing and can take any session, including a short one
next to a larger lane. Take it sooner rather than later for one reason: every close from here on
writes new backlog entries, and each one written before the gate exists is another entry nobody will
mechanically check. Its own Phase 2 is the cheapest audit this project has of a file it reads to
decide what to build.

**Added 2026-08-13: [0091](0091-the-figure-fills-the-frame.md), from three user reference images** —
concentric offset heart contours, and a collage the engine can only partly reach. Where it sits:

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
  whose value is not already established.

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

**Added 2026-08-13: [0090](0090-the-emitters-source-moves.md), from the same pass, by interview** —
the emitter's source becomes a position and a width, gains a spawn fade, and gains a `prewarm`.
Where it sits:

- **It contends with nothing on the roster.** It touches `core/src/render/scenes/emitter.rs` and its
  tests, `presets/`, and three docs; no other active plan is in the emitter. It shares
  `presets/README.md` with [0089] and [0088], in three different sections — **both now closed**, so
  that contention is gone.
- **Phase 3 is the designed cut point, and it is worth knowing before starting.** `prewarm` was not in
  the interview — it exists because grounding the gate argument found a *second* warm-up the backlog
  entry never named (the pool starts empty and fills at `spawn_rate`, so ~18 % of steady-state
  population is on screen at the gates' 0.5 s capture, whatever `source_y` is). Phases 1, 2, 4 and 5
  stand without it; dropping it costs only the *gateable* half of the slow look.
- **Its `human` Phase 5 is the reason the plan exists**, not a trailing verdict: the two looks
  (a quiet drifting field, and a point fountain) are what backlog 0068 has been asking for, and two
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

**The suite is 28 baselines, not 20** (repaired 2026-08-12 at [0080](done/0080-the-sky-gets-a-horizon.md)'s
close, which found the paragraph and the plan it had been copied into both saying "20" against a
directory holding 26; 27 after [0080] and 28 after [0081]). The eight drifters are named above by label, so the numerator survives the
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
[0086]: 0086-the-downbeat-finds-a-cue-that-is-not-the-kick.md
[0087]: 0087-the-line-renderer-draws-a-curve.md
[0088]: done/0088-the-docs-get-pictures.md
[0089]: done/0089-the-framing-contract-stops-lying.md
[0090]: 0090-the-emitters-source-moves.md
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

- **Plan [0085] Phase 5 — the three paired RSS runs** (2026-08-15). The plan is `done` and all four
  `dev` phases landed; this phase needs the **live app on a real machine for a real duration**, which
  is why it is `human`. It gates nothing. Three runs, all with `--soak`, and the new `switches`
  column is what makes them readable: (1) feedback presets, switching, matched length; (2) a
  **no-feedback control**, same length and same switching cadence; (3) one longer run (tens of
  minutes) with **no switching at all**. Runs 1 and 2 separate "the cost of what [0046] landed" from
  "growth that does not stop"; run 3 separates per-switch cost from monotone growth directly, by
  reading `rss_bytes` against a `switches` column that never moves. **Either answer discharges
  [backlog 0083](../design-backlog.md)** — the control climbs (there is something to fix, and it gets
  its own plan, starting from [ADR-0010](../adrs/0010-accept-gpu-driver-memory-floor.md)'s floor) or
  it does not (the growth is per-switch and bounded). The entry stays live until then: Phase 3 gave
  it the instrument, not the measurement. **Read `frame_ms_p99_steady` while you are there** — it
  should hold flat across the switches the raw `frame_ms_p99` column spikes on, which is the one
  live-app confirmation of Phase 3's claim that `soak::tests` can only make against synthetic
  metrics.
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
- **Plan [0061] Phase 9 — the one verification still outstanding** (2026-08-08). The plan is
  `done` and every `dev` phase landed. **Phase 8 ran and passed the same day**: the foobar plugin
  builds against the extracted `lmv-core-cabi` and `foo_lmv.dll` loads in foobar2000 v2 and
  renders. That closed the one risk [ADR-0072](../adrs/0072-the-c-abi-ships-from-its-own-crate.md)
  carried into C++ link time — the linked artifact renamed to `lmv_core_c.lib`, and CI has no
  plugin job that would have caught a stale path. What remains needs a CI run rather than this
  machine:
  - **Phase 9 — read a cache-warm CI run** (the *second* after the push; the first is a cold
    build, because a `[profile.dev]` edit and a new workspace member each invalidate `rust-cache`
    wholesale). It re-derives `COVERAGE_FLOOR` — currently **91**, measured on a hardware-GPU box
    where CI has WARP — and checks the one property
    [ADR-0073](../adrs/0073-the-windows-ci-critical-path.md) committed to: **`coverage` is the
    longest job**. If it is not, `check (windows-latest)` is build-dominated, and that is the
    single measurement that flips ADR-0073's Alternative A (merge the two Windows jobs) from
    rejected to worth taking — route it back to `architect` as a supplement rather than editing
    the job.
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

- [0085 — The show-length horizon gets an instrument](done/0085-the-show-length-horizon-gets-an-instrument.md) — closed 2026-08-15 (four `dev` phases: `3280136`, `a1e62e5`, `97b7227`, `9514e2b`; **Phase 5 is `human` and deliberately outstanding — see Standing**). Review: **no blockers, one major, three minors, one nit**. **The first instrument in this repo that measures past half a second.** `shot --horizon <minutes>` renders N *simulated* minutes at the fixed capture step and prints one row per interval — coverage, `peak/mean` concentration, footprint motion — plus a `delta`/`monotone` trend per statistic and no threshold anywhere ([ADR-0099](../adrs/0099-the-show-length-horizon-is-a-spot-check-and-it-splits-in-two.md)). Both determinism properties are asserted on **rendered pixels through the CLI**, not on arithmetic: two runs of one request are byte-identical, and a 0.05-minute run row-for-row prefixes a 0.1-minute one — which is what makes a recorded header verdict worth anything. The non-vacuity half runs beside it, a `fade = 0.999` de Jong reading monotone 1.00 against a static star pattern reading `delta 0.0000` on all three statistics, **both fixtures written by the test** rather than pointed at shipped content. `--soak` gained **three** columns, not the two the plan's Data shapes named, and the correction is the plan's own fault: the done-when asked for "the two frame-time columns diverging" from a log that carried **no `p99` column at all**, so the raw statistic is appended beside `frame_ms_p99_steady` or there is nothing to diverge from. The exclusion is a **frame** count with a stated derivation and a test that measures the core's `pub(crate)` ring through the public `FrameStats` and fails if it ever outgrows the constant (measured 240, constant 300). **Two core additions the plan's file list did not allow, both approved at the Step 2 gate and both right**: `metrics::peak_to_mean` (ADR-0099 called it existing; it did not exist) and `Renderer::capture_preset_at`, the long-run primitive its two siblings cannot be — asserted **byte-equal to `capture_preset` at the same length on the software adapter**, which is the strict case for the readback-allocation hazard. **Phase 4 found two stale premises and corrected them, and this is the load-bearing part.** R0 is **not** unbuilt — Plan 0044 / ADR-0045 shipped tiers and the governor on 2026-07-30 and this README said they "remain for a later plan" for six weeks — and **the shipped governor never reads `p99`**: `sustained_miss` needs 75 % of ≥180 samples past `budget × 1.25`, which a switch's handful of slow frames in a 240-sample ring cannot approach. So the hazard is real and lives in the **description**, not the code, and all three entry points a governor design starts from now say so. **The major is not against this implementation**: Phase 2 surfaced a **pre-existing** headless-capture frame ceiling — both reaction-diffusion worlds die at 3,601 frames with an invalid readback buffer after RSS reaches ~2.9 GB, with the shipped `capture_preset` run as the control before it was called a finding — filed as [backlog 0093](../design-backlog.md) with a candidate mechanism (`step_offscreen` submits per frame and never polls). Until it is fixed, "N simulated minutes" is bounded by world, and those two rows are 0.5 minutes rather than 10. **Minors:** `frame_ms_p99_steady` initializes to `0.0` and writes an impossible `0.000` if the first sample lands inside an exclusion window, against a doc comment that says it carries its last *trusted* value; `shot_cli.rs` asserts `monotone == 1.0` exactly on rendered output over three steps, and CI runs `shot` on WARP where it has never run (if it flakes, the fix is more steps, not a looser threshold); and this README carried a duplicated "The" at the roadmap item 3 seam. **Nit:** `REACTIVITY_FLOOR` is a mean-channel-difference floor reused as a coverage-fraction floor. **Curation (step 3b):** `presets/` was touched for one header and no values, and the plan fixed no engine defect, so neither sweep fires — but the header it added is the instrument's first conviction, `attractor_ink` drying out 0.199 → 0.002 over ten minutes with the silhouette intact, **recorded and deliberately not repaired**; judging it is content-lane work. The named subject `swarm_shatter` came back **clean** (monotone 0.50, wandering 0.197–0.384 with no trend), which the plan called the expected outcome.
- [0089 — The framing contract stops lying, and two doc gaps close](done/0089-the-framing-contract-stops-lying.md) — closed 2026-08-15 (three phases, one `dev` session: `e23bd04`, `d4570e7`, `52b1dc3`). Review: **no blockers, no majors, one minor, three nits**. **A stated invariant stopped being false without a pixel moving.** `FRAME_FILL = 0.88` documented that a fitted IFS figure sits inside the frame; the fit measures an *axis-aligned* box and `project` then rotates it at `spin`'s default of one revolution per 34.9 s, so only a figure at or under `sqrt(1/FRAME_FILL² − 1)` — about 1.85x taller than wide — stays inside at every angle. Measured over the roster from each figure's own `chaos_extent`: **only the fern complies** (`a = 0.4851` against the `0.5397` bound); sierpinski overruns by 34 %, tree 41, dragon 58, spiral 79 — which is why all three shipped 2-D IFS worlds independently carry a base `zoom` below 1. The new test asserts the closed form **against the shipped `fit_scale`** rather than a parallel arithmetic, derives every constant from `FRAME_FILL` (ADR-0071), derives its sweep tolerance from the sweep's own angular step, guards the knife edge against `f32` rounding, and is **non-vacuous in both directions**. [ADR-0103](../adrs/0103-the-ifs-fit-frames-a-figure-that-does-not-turn.md) accepted, with a dated `Outcome` correcting the plan's own arithmetic: horizontal binding is unsatisfiable at `aspect >= 1/FRAME_FILL = 1.136`, not at every `aspect >= 1`, and the whole derivation assumes a landscape target. **Two `dev` deviations, both correct** — Phase 3's shipped instance moved to the three `reaction_*` presets because `chthonic_coral_oracle.toml` had been retired three days before the plan was written (`d92dcb2`), and the fern's header says something different from the other two because the measurement says the fern is the one figure that *satisfies* the rotated bound. Phases 2 and 3 closed the two doc gaps that had each named a home and never got a carrier: `kaleido_tile`'s bindability and the clipped border cell, and the gain rule's exception class (**a param whose cap is a failure state rather than a maximum**, treated by pulling the range in at *both* ends, worked through Gray-Scott `feed`/`kill`). Curation (step 3b): the plan touched `presets/` for three headers and no values, fixed no engine defect, so neither sweep fires — but the three headers move the other way, and the dragon's `zoom` stops reading as a workaround while the fern's and volute's stop reading as taste.
- [0088 — The docs get pictures](done/0088-the-docs-get-pictures.md) — closed 2026-08-13 (all seven phases, written and landed the same day; six `dev` commits plus the `human` Phase 7 look call **run at the close rather than carried forward**). Review: **no blockers, no majors, three minors, two nits**, all repaired in `5dda709`. **Eighty-eight plans of a real-time graphics project, and this is the first committed image of any kind.** Sixteen of them ([ADR-0100](../adrs/0100-documentation-images-are-committed-headless-renders.md)): nine gallery, one hero, six walkthrough, every one a 1280x720 `shot` render captured **under real audio** through the real analyzer and driven from an argument-free `scripts/docs-shots.mjs` whose manifest is the only record of what produced each file. The capability came first — `shot --frame-at <hop>` (`476a989`), because the filmstrip path scales every frame to a **363x208 bordered tile** and nothing in the tool could produce a full-resolution frame under real audio. Two new documents on top of the three references rather than merged into them ([ADR-0101](../adrs/0101-the-preset-docs-gain-a-tutorial-layer-rather-than-a-merge.md)): [`docs/preset-guide.md`](../preset-guide.md) and [`docs/preset-tuning-walkthrough.md`](../preset-tuning-walkthrough.md), and the one-fact-one-home rule **held under review** — the guide reproduces no parameter, function or palette table. **Two deviations, both argued and both right, and the first is this project's own arithmetic rule failing on the authoring side.** The plan specified capture hop **340**; `dev` moved it to **300** after re-deriving it against `core/src/signal.rs:144`, where `beat % 8 → 6|7 => 0.04` puts the rest at hop 306.8 — so the plan's own number sat **34 hops inside `dynamic_groove`'s two-beat rest**, chosen from scene time and never checked against the phrase. The second was user-directed: Phase 5's subject moved from a `swarm` to a `fragment_field` mandala, because a still picture cannot teach `force`, `spin`, `field_freq`, `reseed` or `twinkle` and the five steps' method is family-agnostic. **The weight has two numbers and ADR-0100 conflated them**, which is the finding a later close must carry: the **tree** holds 16 images / **20,459,591 bytes**, but **history** holds 19 blobs / **25,489,457 bytes** — `hero.png` was written three times and `swarm.png` twice, and a superseded blob never leaves a repository that does not rewrite history. Both are inside the ≤ 22 images / ≤ 32 MB ceiling, but **the ceiling is about the history figure**, so a whole-set re-shoot costs its full weight again — recorded as a dated [Outcome](../adrs/0100-documentation-images-are-committed-headless-renders.md#outcome--2026-08-13-at-plan-0088s-close). **The close tested a done-when no phase could**: re-running the script after Phase 7's two manifest edits moved **exactly those two images** and left the other fourteen byte-identical — same machine and binary only, which is not evidence against the cross-adapter drift that keeps this out of CI. **Phase 7's verdict**: all ten committed pictures opened, eight of nine gallery picks stand, two swapped against alternatives shot for the comparison (`swarm_drift → swarm_shatter`, charcoal on black collapsing to a dark rectangle at README width; hero `fragment_supernova → fragment_tunnel`, a flat salmon field reading as wallpaper at the top of a front page). **`emitter_perseids` and `star_rosewindow` are accepted rather than good** and the hop is provably not the lever — both were re-shot at other hops with the same framing — so they went to [`docs/content-brief.md`](../content-brief.md) §5 as a **framing brief for the content lane**, each family shipping exactly one preset with nothing to swap to. **Curation (step 3b): no preset content landed** — `docs/examples/` is teaching material and never enters `presets/` — so no near-duplicate sweep was owed, and the workaround grep over `presets/*.toml` is unchanged by this plan. **Carried forward:** [0087] invalidates the curve family's gallery image and the hero, which is one script re-run and is named in the sequence section above.
- [0084 — Two gates stop lying about what they check](done/0084-two-gates-stop-lying-about-what-they-check.md) — closed 2026-08-13 (all four `dev` phases, written and landed the same day). Review: **no blockers, no majors, three minors, one nit**. **The doc-link checker sees markdown's second link form** — a use with no definition in its file, and a definition whose relative target does not resolve, both reported through the existing `file:line -> target` shape. **The narrowing that makes it usable was measured rather than assumed**, which the plan's risk section had asked for: a shortcut use is reported only when *some* file in the tree defines that label, without which the repo corpus yields 31 findings of which 24 are ordinary prose brackets, and with which 7 and no noise. It found exactly the seven breaks the plan predicted and **proved itself again at this close** — the `git mv` into `done/` broke four inbound links and it named all four, one in the definition class it had just learned to see. **The capture path can advance without rasterizing**: `capture_audio_after_warmup` takes a count of leading hops to step the analyzer and the clock with no render pass, and `capture_audio` is that call with a warm-up of zero, signature and behaviour unchanged. Measured on this Windows box's DX12 software adapter (ADR-0071 — in a docstring, not an assertion): **136.3 s -> 100.2 s over 36 presets**, the superseded 86 s -> 167 s pair kept and labelled pre-0084 rather than deleted, since it was taken on a 41-preset library. **The plan's real acceptance criterion failed and was accepted at the escalation, not absorbed** — its premise was half wrong, because the warm-up renders were also the *scene* warm-up, which `reactivity.rs` said in as many words. 35 of 36 per-band vectors moved, the exception being `spectrum/Halo`, the only preset in the set with no accumulating state; every maximum rose or held and the lowest across the library went 0.0287 -> 0.0504 against the 0.020 floor, so the tightest headroom roughly doubled. **Read any reactivity figure recorded before 2026-08-13 as a different measurement, not as drift.** **Minors:** Phase 1's fixture done-when left no committed artifact, so the script's optional `root` argument has no caller in the repo and the bite check is unrepeatable — which is the property the phase itself argued matters most, since a link checker that silently passes is worse than none; `docs/capturing.md` still named the gate's old capture call and its old ~1.8x conversion price in the exact paragraph that tells a future author how to copy this pattern, repaired in the close series; and the byte-identity test guards the property that was never at risk — the render pass structurally cannot reach the analyzer, which publishes before it skips at `capture_api.rs:321` — while the property that actually moved, GPU-integrated scene state meeting the measured window `WARMUP_HOPS` steps colder, is documented in three places and asserted in none. That last one is carried forward rather than fixed: **any gate copying this pattern onto an accumulating scene inherits the cold start silently**, which is why `docs/capturing.md`'s gate section now says so where the next author reads it. **Nit:** the use-class narrowing is deliberately blind to the mirror failure, where a definition block is deleted outright and every use of the label goes quiet — documented in the script header, and the right call against the 31-vs-7 measurement. **Curation (step 3b): `presets/` untouched and no engine defect fixed — nothing owed.** No `aspect` in the diff, no platform or audio-source type entered `core/`, and the only new public item is `AudioCapture` on the capture path.
- [0083 — The build says why it hears nothing](done/0083-the-build-says-why-it-hears-nothing.md) — closed 2026-08-13 (all four `dev` phases, written and landed the same day; **Phase 5 is `human` and deliberately outstanding — see Standing**). Review: **no blockers, no majors, one minor, two nits**. **The capture verdict becomes a value** — `CaptureVerdict::{Live,Failed,Unsupported}` in a new `standalone/src/capture_verdict.rs`, produced by all three `cfg` arms of `start_capture` through a `CaptureStart` struct that replaced a three-tuple, so **an arm that forgets to set one does not compile** rather than rendering as a success. It reaches the two artifacts a remote tester can actually send: a trailing `capture` column on every `diagnostics.log` row, and an `audio` line under the F3 panel. **Both read one `String` built once at startup and borrowed thereafter** (`maybe_log` takes `&str`), which is what makes the two surfaces unable to disagree about a run *and* keeps the per-frame row builder allocation-free. **Why a column and not a startup line** is the plan's load-bearing decision and it held: the file rotates at 1 MiB keeping one backup and the tester's log spanned 6.5 days, so a line written once is exactly what rotation deletes — and a column also catches a capture that dies mid-run. **The Windows arm was in scope for a reason that is not symmetry** — nobody on this project can execute the macOS path, so building both is what got the mechanism tested and reviewed on the development box; the Mac arm differs only in which error type it formats. **The sanitizer is tested against a deliberately hostile message** (`"  start failed:\tcode -3801\r\n\tat SCStream\n\n"` → `failed SCK start failed: code -3801 at SCStream`) rather than a real platform error, on the plan's own argument that a real one which happens to be clean proves nothing; an all-whitespace message renders `(no message)` so a row never trails off looking truncated. **The failed verdict deliberately carries no format** — both arms fall back to a hardcoded 48 kHz stereo so the analyzer has something valid, and reporting that would have the log state a format nothing is delivering; the constant is now named `FALLBACK_FORMAT` at one site instead of three inline literals. The frozen-prefix assertion in `diaglog.rs` was **widened rather than rewritten** (the fourteen pre-0083 names are the prefix, `capture` the appended tail), and the tests locate the field **by header position** rather than by index, so the next appended column moves nothing. Docs swept: both `packaging/*/READ-ME-FIRST.md` demote the Terminal relaunch from step 3 to a fallback, and `docs/on-device-validation.md` gains the column and an instruction to read the `audio` line *before* judging reactivity — flat band meters mean "capture failed" or "nothing playing", and every reactivity judgement below is worthless if it was the first. `docs/capturing.md` correctly untouched: it documents the `shot` CLI and the preset report, and has no `diagnostics.log` shape section. **Minor:** `standalone/src/overlay/tests.rs`'s `PANEL_BOTTOM` is hand-copied arithmetic over `core/src/render/overlay.rs`'s private constants, and its comment claims *"a panel that grows fails this deliberately"* — it would not; core's constants are private, nothing couples them, and the two agree at 240 px today only because someone transcribed them correctly. The check is worth keeping; the claim is the ADR-0071 prose failure one level down. **Nits:** the stale-header test's comment still says the rows carry "fourteen" columns (fifteen) and that the seeded stale header names "eight" (three) — pre-existing drift the sweep passed over; and `overlay::capture_line` formats a fresh `String` every frame the overlay is up, which matches what `queue_frame_text` already does around it and is why it is a nit rather than a finding. **Curation (step 3b): no preset content landed and no engine defect was fixed — nothing owed.** `core/` is untouched, no `aspect` appears in the diff, and nothing added runs on the capture thread: the verdict is known before the callback exists.
- [0079 — The attractor learns new figures: the tuple roster with per-tuple framing, and measured morph paths](done/0079-the-attractor-learns-new-figures.md) — closed 2026-08-13 (all six phases, **both `human` gates run and both producing a verdict rather than a default**). Review: **no blockers, no majors, four minors, two nits**. **A tuple becomes content, framing and all** ([ADR-0093](../adrs/0093-attractor-tuples-are-content-with-per-tuple-framing.md), closes [backlog 0055](../design-backlog.md) **in full — both halves**): each map family carries a curated roster whose entries hold their coefficients *and* a **measured** projection + seed box, selected by a CPU-quantized `tuple`. The wall the entry named is gone — the rho ≈ 100 Lorenz that Plan 0075 cohort 5 measured as physically unreachable (centred on `z ≈ 102` against the canonical framing's `25`, spanning twice its extent) renders centred and in frame. **"Zero baselines move" is structural rather than argued**: entry 0 *is* the pre-roster literals, and `roster_entry_zero_is_the_canonical_tuple_unchanged` spells them out so a tidying refactor fails loudly instead of as a golden diff. **ADR-0037 is unreachable here by construction** — the per-entry scale is a *ratio* against the canonical figure's own extent, so aspect handling is bit-for-bit whatever entry 0 already did, and no `aspect` appears in the diff. The Plan 0062 coupling survives *by derivation*: `jitter_extent` hangs off the entry's own box, asserted twice — as a fraction (`kick / half == JITTER_FRACTION`) and on the GPU as a measured mean `|dy|` matching the entry's prediction while **failing** the canonical framing's, which is the non-vacuity half. **The curation kept all 50 candidates** (*"honestly I love them all"*) after a four-per-family shortlist was drafted and **rejected** — judged in motion in the app, not off the sheets, because a still freezes one instant of a rotating figure. **The morph half's accepted research risk did not materialise**: of twenty swept pairs, four were refused *by measurement* before any eye reached them (a mid-walk tuple can collapse to a fixed point, which has zero extent and no scale to render at — all four on the discrete maps), four were judged in motion and ship as presets, and twelve strips are recorded as **rendered but unjudged** rather than waved through. The finding worth carrying: a walk holds where a roster steps a **single** coefficient (Thomas's `a`, Lorenz's `rho`), because there neighbouring entries are neighbouring *figures*. Three things beyond ADR-0093 are in its dated [Outcome](../adrs/0093-attractor-tuples-are-content-with-per-tuple-framing.md#outcome--2026-08-13-at-plan-0079s-close): framing alone was **not enough** (a measured entry seeds from the 4096-point on-attractor bank its own measurement collected — ADR-0087's IFS argument extended to a figure with no closed-form fixed points; without it rho ≈ 100 wanders to **2.2x its own extent** for seconds), the walk drives the **existing** `morph` param rather than a new one (so the param surface grew by the +1 the ADR budgeted, with a tuple path on an IFS a load error), and the roster's real costs (51 tuples of maintenance; ~3.7 ms per entry measured at preset load in debug, never per frame). Determinism held to the GPU by differential, not by reading: `the_cpu_step_mirrors_the_shader` runs both and compares, and `the_ode_substeps_agree_between_rust_and_wgsl` pins the constant a measurement would otherwise silently diverge on. **Minors:** the content lane's own `references/systems.md` had not learned `tuple` — the *identical* minor from Plan 0078's, 0080's and 0081's closes, and load-bearing because this plan's Followup is that lane binding it (repaired in the close series, table row + walk note); the two new `scripts/tuple-{sheets,paths}.mjs` had no operator-doc home (repaired in `docs/capturing.md`, with the caveat their output lives under gitignored `target/`); and two scope drifts recorded in the plan header rather than absorbed — **eleven presets shipped against the plan's own "does NOT do"** (user-approved as each landed, and ADR-0081 makes it legal without a plan, so a deliberate widening), and `presets/README.md`'s `tuple` row landed at Phase 1 rather than Phase 4 because the doc gate runs immediately and leaving it red across a `human` gate was not acceptable. **Nits:** `select_tuple` returns a `bool` nothing reads; `roster_len` widens the rlib's public surface (justified in place, C ABI untouched). **Curation (step 3b):** the attractor family went 6 presets → 17 of 37, **46 % of the library on one system** — the sharpest single-family convergence the set has seen, and the number to weigh before more attractor content lands. `attractor_dejonggallery` and `attractor_cliffordgallery` are near-twins by construction (identical `tuple`/`brightness`/`fade`/`reseed`, differing only in family and palette); all four galleries step on a **wall clock** (`mod(floor(time * 0.33), N)`) with audio only on secondary levers, which makes them demonstrations of the roster rather than worlds. That is a judgement for the content lane, not a re-tune here. The workaround grep over all 37 headers finds **nothing** citing the tuple wall — expected, since nothing could express a workaround for a figure that could not be reached at all.
- [0081 — The sky gets a galaxy: the backdrop paints a curved band](done/0081-the-sky-gets-a-galaxy.md) — closed 2026-08-12 (all five `dev` phases; **Phase 6 is `human` and deliberately outstanding — see Standing**). Review: **no blockers, no majors, two minors, two nits** (minors, both repaired in the close series: `docs/capturing.md`'s *"One golden baseline is lit, and it is the exception that proves the rule"* was falsified by this plan's own fixture — there are now two, which is the same doc drifting in the same paragraph that Plan 0080's close repaired; and this README's standing baseline-drift control still said "27 baselines" against a directory holding 28. Nits: `backdrop_ramp.rs`'s `0.25`-row bounds on the straight-band spread and the bow's shear are the only two thresholds in the new suite without a stated derivation, and the tighter of them has 1.6x headroom over its measured 0.16 in a statistic the dither now feeds; and the shader's `select` evaluates both arms, so a backdrop with no band pays two extra LUT fetches per pixel where an `if` would give byte-identical output and skip them). **The backdrop paints one soft curved band** ([ADR-0095](../adrs/0095-the-backdrop-paints-a-curved-band.md)): seven params — `bg_band_amount`/`bg_band_angle`/`bg_band_pos`/`bg_band_width`/`bg_band_curve`/`bg_band_hue`/`bg_band_hue_span` — drawn **additively over the ground and under the scene**, which is what unresolved starlight is, so the four-role look (ground, band, stars, figure) becomes authorable without widening [ADR-0090](../adrs/0090-a-preset-composes-two-scene-layers.md)'s one-`[layer]` cap. **Every default is an arithmetic identity and it was proved three times**, once per code phase, by the bless-to-bless control (bless twice on the branch differing only by reverting the change — never a `git diff`, since eight baselines drift from their committed bytes on this box anyway). The identity is **structural rather than arithmetic**: `bg_band_amount = 0` takes a `select` arm, so the pre-band expression is the *untaken* branch and the inertness test asserts `worst == 0` over the whole frame with every other band param bound off its default. **The widened build condition (`bg_bright > 0 || bg_band_amount > 0`) is the plan's one-line change and it has its own test** — a band over an unlit ground, the only thing in the suite that can see it, with the `amount = 0` companion coming back the plain black clear. Three axes now share **one** `axis_pos()` and both palette fetches one `palette_at()`, deliberately so ADR-0037's trap cannot be fixed in one axis and left in another. **Two plan-accuracy findings, both recorded by `dev` rather than absorbed, both in [ADR-0095's Outcome](../adrs/0095-the-backdrop-paints-a-curved-band.md#outcome--2026-08-12-at-plan-0081s-close):** (i) the plan, this README's roster row and the ADR all framed the along-band normalizer as *not* cancelling at the default angle, making Phase 2 the sole possible sighting of ADR-0037 on that axis — **it cancels** (numerator `ndc.x * aspect`, denominator `aspect`), confirmed by re-running the bow measurement with the aspect forced to 1.0 and reproducing every digit; what Phase 2 actually catches is a wrong normalizer *form*, verified to bite (dropping the aspect from the denominator alone shears the arc 1.36 rows and fails the first edge assertion), and the trap itself is one property across all three axes because they read the single aspect the pass is handed. (ii) ADR-0095's table ("its own coordinate in the same `[palette]`", absolute) and the plan's Phase 3 done-when ("leave the band on the ground's own coordinate", an offset) **cannot both hold**; `dev` raised the fork and **the user chose absolute**, on the authoring argument that the ground's coordinate varies along the band's path, so an offset would drag the ramp's sweep into the arc. **Every numeric done-when is a differential, never a magnitude.** The `1/e` half-width goes into the *control* — a flat band at the upper width rail carrying `amount/e` — because the tonemap and the sRGB encode sit between the envelope and the 8-bit write, so an asserted ratio of 0.368 would be a claim about the tonemap's shoulder; the two frames agree to 1 level at the crossings and differ by 21 at the centre. The bow is *located* column by column with a **luma-weighted centroid rather than an argmax**, a direct consequence of [0082] landing first: a gaussian's peak is flat, `pos = 0.5` puts the centre between two pixels, and the dither's ±1 LSB makes argmax report rows 31, 32 and 33 for three columns of one straight band. The fifth `EXTRA_FIXTURES` entry is **appended, never inserted**, so no pre-existing baseline is rendered from different device state, and **its seven values were tuned against the suite's own tolerance rather than for looks** — each reverted to its default in turn and re-measured, all seven clearing `MEAN_TOL = 0.02`, the tightest by 1.1x; two needed the tuning (a wide `bg_band_width` scored 0.0055 on its own revert, and the colour pair fights itself through the repeat addressing). Blessed only after comparing adapters, which the `48 → 80 B` uniform growth owes: WARP `152.200 120.540 086.088` against hardware `152.198 120.550 086.114`, 0.026 of one level, and the other 27 baselines restored to their committed bytes after the un-scoped `LMV_BLESS`. The ADR-0058 enumeration and `shot --report`'s generic binding walk were **confirmed, not edited** — a changed answer would have been the finding. Phase 5's sweep of `.claude/skills/preset-author/references/**` was a **done-when rather than a reviewer's catch**, which is the fix for the identical minor raised at both Plan 0078's and Plan 0080's closes; it landed. **Curation (step 3b):** no preset content landed — only `presets/README.md` and `docs/preset-palettes.md` — so no near-duplicate sweep owed; the workaround grep over all 27 headers finds **nothing** citing a missing band or a painted-in galaxy, which is expected, since nothing could express the shape to work around.
- [0082 — The gradient stops banding: the display write dithers](done/0082-the-gradient-stops-banding.md) — closed 2026-08-12 (four `dev` phases plus a self-repair, and the `human` Phase 5 **answered rather than left standing**). Review: **no blockers, one major, five minors, three nits** — and every finding is a consequence of something the *plan* got wrong, recorded honestly by `dev` rather than absorbed. **The tonemap dithers** ([ADR-0096](../adrs/0096-the-display-write-dithers.md)): ±1 **encoded** LSB of TPDF noise from an integer hash of the pixel coordinates, divided by the sRGB transfer function's local slope because `Rgba8UnormSrgb` means the *hardware* encodes after the shader. One site, always on, not a param, no time term. The dusk ground's dark tail went **7.5 px/level and a 58-px plateau → 2.1 and 20**, wide plateaus 17 → 3, still 0 % rail-pinned; the user's by-eye verdict on the held frame was *"looks fine"* on both halves, which **retires ADR-0096 Alternative F** (the animated dither) as a followup. **The major is the ADR, accepted with a dated [Outcome](../adrs/0096-the-display-write-dithers.md#outcome--2026-08-12-at-plan-0082s-close) that falsifies two of its claims.** (i) Its "three parts, each load-bearing" is **four**: the dither must **fade at the rails**, which the ADR never mentions — at a rail the value is already exactly representable and the write clamps, so half the noise is discarded and what survives is a **DC lift**, and an exactly-black frame came back at mean **0.18/255** over a suite where nearly every fixture runs `bg_bright = 0`. Caught by two *existing* guards (the emitter burst test at lead peak 0.1827 where it asserts empty, and bloom roundness), not a new one; `dither_offset`'s fade is provably inert at and above code value 1, because below the knee the slope is the exact constant 12.92 so `min(l, 1-l) * slope * 255` **is** the encoded byte value. (ii) Its third Positive consequence — the headline argument that an integer hash buys **byte-for-byte** adapter agreement, sharper than the 0.02 drift floor — is **false**. The hash is exact (65 536 float values, zero differing bit patterns), but the **hardware sRGB encode downstream of it** is not: DX12 permits tolerance in float-to-sRGB8 and WARP's approximation departs from the true curve below ~byte 20, so **212 of 2 049 408 re-blessed channels move by 2**. The integer hash still earned its place (Alternative C would have diverged on essentially *every* pixel), but the promised instrument does not exist. **Three of this plan's own numeric done-whens were wrong**, which is the architect-side lesson: "the golden suite goes red here" — it did not, the guard is a **tolerance** guard and a one-level shift reports 0.0007-0.0013 against 0.02, three orders of magnitude inside, so Phase 2 was a deliberate **re-pin** rather than the repair of a red build; "a delta of 2 anywhere is a finding" — bounded-by-one is a **hardware** claim and the shipped assertion reads its bound off `is_software()`; and the TL;DR's "hairlines", which the honest 20 px replaces. **The first deliberate full re-bless in the project's history** landed alone in its own commit, measured **bless-to-bless** (8 of the 27 rewrite against their committed bytes on a clean local bless, so a `git diff` would have charged that drift to the dither). Verified at the close rather than taken on trust: golden 27/27 green, `backdrop_ramp` 6/6 so the `<= 1` tolerances survived, all four byte-equality tests pass including Plan 0075's `depth_fade` no-op against a live Lorenz control — the property that made *static* the right choice — and both dither tests reproduce their recorded numbers exactly. Two scope calls at the Step-1 gate, both right: `mix32` / `unit01` promoted to `gpu::HASH_WGSL` (the shared home [0077]'s close asked a *third particle scene* to build, arriving instead from the display write), and Phase 3's guard placed **in-crate** because an integration test cannot reach a `#[cfg(test)]` field and a public off-switch is exactly what Alternative G rejects. No ADR-0058 entry was owed — the amplitude reuses the `Ctl` uniform's already-zero `.z`, so the layout shape is unchanged. Minors repaired in the close series: `docs/on-device-validation.md` had not learned the dither (three `pow` per pixel on a fullscreen draw, never measured on weak hardware; and the grain verdict was taken on **one display**, where a 6-bit + FRC panel running its own temporal dither is the case it cannot speak to), and the two "before" figures for the dusk probe disagree 2.3x with the scan axis unrecorded — written down as a **stated discrepancy** rather than an invented explanation. Nits: the corrected WARP mechanism's disproof lives in prose only and nothing asserts it; both dither tests request the software adapter so the tight hardware bound never runs automatically (fine — the derived-1/3 mean is the adapter-robust guard); and `core/Cargo.toml:31`'s last surviving mojibake, repaired. **Curation (step 3b):** no preset content landed — only `presets/README.md` — so no near-duplicate sweep owed; the workaround grep over all 27 headers finds **nothing** citing banding or a step-breaking stop, because no shipped preset binds the ramp params yet.
- [0080 — The sky gets a horizon: the backdrop paints a directional ramp](done/0080-the-sky-gets-a-horizon.md) — closed 2026-08-12 (all six `dev` phases; **Phase 7 is `human` and deliberately outstanding — see Standing**). Review: **no blockers, no majors, three minors, one nit** (minors, all repaired in the close series: the `preset-author` lane's own `references/systems.md` and `craft.md` did not know the ramp exists — the Plan 0078 `ink_gamma` minor repeated verbatim, and load-bearing because Phase 7's followup *is* that lane authoring a ramp world; `docs/capturing.md`'s "every golden baseline runs `bg_bright = 0`" was falsified by this plan's own fixture, the suite's **first lit golden baseline**; and this README's standing baseline-drift control still said "8 of 20" against a directory holding 26. Nit: `backdrop_ramp.rs:273`'s `+ 100.0` reversal margin is the one threshold in the new suite without a stated derivation, in a suite otherwise exemplary on [ADR-0071](../adrs/0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md)). **The backdrop paints a directional ramp** ([ADR-0094](../adrs/0094-the-backdrop-paints-a-directional-ramp.md), closes backlog 0091, a user-rejected workaround rather than speculation): five params (`bg_angle`, `bg_hue_span`, `bg_shade`/`bg_shade_end`, `bg_ramp_gamma`) turn one palette *sample* into a *segment* swept along one axis, with the hardcoded `mix(0.72, 1.0, ndc.y)` tilt **retired into** the shade ramp so there is one brightness ramp on the frame rather than two, and horizon placement authored by the `[palette]` stops' own `at` positions — no second placement mechanism. **Every default is an arithmetic identity, and that was proved four times**: all 26 pre-existing baselines came back hash-identical under a bless-to-bless control once per code phase (bless twice on the branch, differing only by reverting the change — never a `git diff`, on this box eight drift from their committed bytes anyway). **ADR-0037's trap was instrumented, not argued.** At `bg_angle = 0` the aspect term *provably* cancels (`d = (0,1)`, denominator `aspect * 0 + 1`), so no default-angle test anywhere could tell a right source from a wrong one; the control runs at π/4 on a 160x100 target **with `trails` active**, because with an empty chain `target.size` **is** `surface` and the control would be theatre — at that size the internal grid quantizes to a square 256x256, so the wrong source is aspect 1.0 exactly against the surface's 1.6. It was verified to **bite**: `composite_into` was temporarily re-pointed at `target.size` and the test failed on its *first* assertion at 20 levels, which is ADR-0037's symptom (turning a stage on changes the shape of the picture) stated directly. The asserted 99-column crossing is derived, not measured — Δndc.x = 2/A = 1.25, times (1 − 1/H) for the edge rows' half-pixel inset, times W/2. The exponent reuses [ADR-0092](../adrs/0092-the-ink-remap-gains-a-contrast-exponent.md)'s shipped `select(pow(s, g), s, g == 1.0)` where the branch is a **correctness requirement** (`pow(x, 1.0)` is `exp2(1.0 * log2(x))`, not bit-exact), and its test asserts *placement* as well as agreement — `e = 0.5` sits at `s = 0.5^(1/g)`, giving rows 15.0/31.5/52.2 over 64, measured 15/15, 32/31, 52/52; agreement alone would have passed with the exponent inert on both channels. The 32 → 48 B uniform growth needed no ADR-0058 entry (the enumeration records **whether** a `min_binding_size` is declared, deliberately not which) and the adapters agreed to **0.044 of one 8-bit level** before blessing. Two `dev` findings recorded rather than absorbed: the plan's "20 baselines" is 26, and `shot --report` needs no per-namespace list because it walks bindings generically (verified with a probe binding four `bg_*` names, one live gate and one dead). **Curation (step 3b):** no preset content landed — only `presets/README.md` — so no near-duplicate sweep owed; the workaround grep over all 27 headers finds **nothing** citing the missing gradient this plan supplies, which is expected: the rejected workaround was never landed.
- [0078 — The ink learns to bite: a contrast exponent on the terminal remap](done/0078-the-ink-learns-to-bite.md) — closed 2026-08-12 (both `dev` phases; **Phase 3 is `human` and deliberately outstanding — see Standing**). Review: **no blockers, no majors, three minors, one nit** (minors: Phase 1 substituted a structural argument for the bless-to-bless control its done-when named — the substitution is the *stronger* instrument and is recorded as such, not repaired; `.claude/skills/preset-author/references/systems.md`'s ink row did not know `ink_gamma` exists, repaired in the close series and load-bearing because Phase 3 is that lane retuning against that table; two shipped preset headers now describe as forced a workaround the engine no longer forces, which is Phase 3's roster rather than a fix. Nit: the mid-band mean's `+ 1.0` byte margin sits below `golden.rs`'s own `0.02`-normalized drift floor for a mean statistic — defensible here because it is a lower bound on 12-33-byte gaps through an *injected* deterministic ramp, not a rendered scene, and the tighter per-level `<= 1.0` tolerance has been measured on WARP). **`ink_gamma` lands** ([ADR-0092](../adrs/0092-the-ink-remap-gains-a-contrast-exponent.md), backlog 0084, hit 3x across two Plan 0075 cohorts): `mix(paper, ink, luma^g)`, endpoints invariant *by arithmetic* rather than by tuning, so the paper never moves at any value. **The default's exact identity had to be built, not inherited** — `pow(x, 1.0)` is `exp2(1.0 * log2(x))` and not bit-exact, so the shader takes an explicit `g == 1.0` branch; without it the zero-baseline sentence would have been false by a rounding step. **And the zero-baseline claim is structural in a stronger sense than the plan argued**: no golden fixture binds `ink_amount` at all and the stage builds its resources only when `active()`, so no committed baseline ever constructs the ink pass — which also covers the one thing a param grep cannot see, the `COPY_DST` flag added to `ink-src` for the endpoint test (the arrangement `tonemap-src` already carried). Endpoint invariance is asserted through the shipped WGSL across `g = 0.25/0.5/1/2/4` **and** over hostile values (0, negative, NaN, ±∞) on the CPU mirror; the test injects a 256-step ramp because no rendered scene reaches key 1 (ADR-0046's shoulder is bounded strictly below it). Two `dev` judgment calls the plan did not specify — the CPU-side crossfade of `gamma` alongside `ink_amount`, and the finite `0.05 .. 20` clamp — are recorded in ADR-0092's [Outcome](../adrs/0092-the-ink-remap-gains-a-contrast-exponent.md#outcome--2026-08-12-at-plan-0078s-close), which falsifies nothing in the ADR. The published mean-byte ladder (`147 / 180 / 209 / 229 / 241`) reproduces independently from pure sRGB-to-linear arithmetic (`147.1 / 180.4 / 208.6 / 228.6 / 241.0`) — a property of the math, not of a rig, which is why it is publishable without naming one. Two plan-accuracy drifts caught and recorded by `dev`: `schema.rs` needed no edit (`GLOBAL_PARAMS` aggregates `ink::PARAMS` by reference — verified, there is no second roster), and two unlisted files were required. **Curation (step 3b):** no preset content landed, so no near-dup sweep owed; the workaround grep names `reaction_etching` and `swarm_shatter`, both carried into the Standing entry.
- [0077 — The quiet sky: the sparse idiom becomes gateable and the swarm individuates](done/0077-the-quiet-sky.md) — closed 2026-08-12 (`dev` scope; **Phase 5 is `human` and deliberately outstanding — see Standing**). Review: **no blockers, no majors, two minors** (both repaired in the close series: `docs/capturing.md` had not learned the report's new footprint block / `reactivity_footprint` JSON key — the operator-doc sweep `dev` correctly left for the close; and the gate's deliberate semantic change — backdrop-only drift no longer counts as animation — lived only in the test until ADR-0091's Outcome recorded it) **and two nits** (`report.rs`'s "the reading never sits below the mean" is a practical-regime claim, not a theorem — sub-`eps` differences on unlit pixels leave the numerator while the denominator shrinks; and the emitter's `unit` hash is now mirrored verbatim into `swarm.rs`, a deliberate, commented duplication that a third particle scene should promote to a shared home). **The sparse idiom becomes gateable**: the animation gate scores `metrics::footprint_diff` — the masked form, chosen over the quotient with the reason recorded on the function — with every constant carrying its derivation (floor at half the shipped minimum; the 139-pixel mask floor capping a one-pixel flicker at 0.0072), the `bg_*` strip re-learning ADR-0067 by measurement (backdrops on, the sparse probe's footprint read 65 % of the frame), the rejected fifth-density Squall draft **passing at 0.1049** where the whole-frame statistic priced it out at 0.0057, the static control failing on a zero numerator, both pinned as a standing non-vacuity test, and the whole-library sweep convicting nothing. **The swarm individuates**: `twinkle`/`size_spread` off the particle's index through the emitter's unit hash — deliberately not `SeededRng`, whose extra stream draw would re-scatter the field — exactly 1.0 at their zero defaults so the goldens pass unblessed, with the shimmer-without-breathing bound derived from the mechanism (`8 * TWINKLE / sqrt(N_visible)`, 16x under the sheet-flash signature). **The swarm gains `reseed`** with ADR-0066's disturbance semantics (±6 % domain-relative kick, never a box respawn), catching a live defect class en route: resetting `prev_reseed` in per-frame `reset_params` turns a held gate into an edge per frame (measured diverging, 105 % coverage gap at 10 s) — the omission is now commented on both scenes. **The report sees bloom** (backlog 0088): the mean columns stay untouched and a footprint reading lands beside them at zero extra GPU cost — the bloom-only fixture reads bass 0.161 against the mean's 0.004, unbound bands stay 0.000 in both readings, and the `flash`-lever house workaround is obsolete. Plan drift recorded honestly by `dev` in the phase commits (no `schema.rs` edit exists to make; the report machinery lives in `report.rs` since Plan 0061). **Curation (step 3b):** no preset content landed, so no near-dup sweep owed; the workaround grep lists two headers for the content lane — `fragment_vitrail`'s "report is bloom-blind" rationale (fixed by Phase 4) and Perseids' routed-out quiet sky (Phase 5's own subject) — named in the Standing entry.
- [0075 — The content renaissance: the library is rebuilt as worlds, by replacement cohorts](done/0075-the-content-renaissance.md) — closed 2026-08-11. Review: **no blockers, no majors, two minors, two nits** (minors: rustfmt drift on two test files the lane touched, repaired in the close series as `6a5a9c6` — the "557/557 green" handoff claim was nextest, which does not check fmt, and the fmt-running pre-push hook never fired because the lane never pushed; the roster row's "the library is 28 worlds" against a measured 25 after cohort 5, moot with the row's deletion here. nits: `standalone/src/shot/report.rs` reaches the extent diagnostic through the deep `lmv_core::render::scenes::lines::renderer` path — a `render`-root re-export would keep the shell at arm's length; Phase 2's "Files touched" named `standalone/examples/shot.rs`, which Plan 0061 Phase 4 had already moved — `dev` caught and recorded the drift in the phase commit). **R6 lands: the library is rebuilt as 27 worlds — the brief's 9 keeps plus 18 authored fresh-slate — through six family cohorts, each landing its worlds through the [0067] route and retiring its named roster in the same series** (45 → 27, ADR-0089's mechanism held: the set was never hollow, the gates never went vacuous, and every cohort was judged live by the user before its retirements committed). Phase 1 ended the sanity floor's selecting-for-the-defect: `metrics::radial_shell_occupancy` (ten annuli over the inscribed disc) rescues a preset under its coverage floor at ≥ 4 occupied shells — the three retired ring mandalas at their honest tunings (frozen byte-for-byte, the backlog's exact pinned numbers 0.2442/0.2505/0.2544) read 10/10/9 shells, the frozen renders-nothing defect reads 0 and still fails, and every constant states its derivation (ADR-0071). Phase 2 made `depth_fade` an exact no-op on flat families — asserted by **byte equality** with a live Lorenz control so the no-op cannot pass vacuously — recorded as ADR-0076's second dated [Outcome](../adrs/0076-the-attractor-keeps-the-depth-it-already-computes.md#outcome-added-at-plan-0075s-close-2026-08-11); and the in-frame geometry fraction joined `shot --report` as the `geom` column, printed exactly where a line seam exists (JSON mirrors the omission). Phase 3 landed the measured depth-lever corrections (the `perspective` orbit and its ~0.3 ceiling, `depth_hue`'s three regimes, the `spin`×`fade` smear ceilings) in `presets/README.md` and `docs/preset-palettes.md`. Cohort 6 shipped the library's first two layered worlds (Vitrail, Sumi) on [0076]'s `[layer]`. Retirement commits froze the test fixtures they orphaned (Star Rosette's ladder source, the honest mandala tunings) rather than leaving dangling `include_str!`s. Engine feedback routed out as designed: backlog 0084–0089 plus re-raises, promoted to [0077](done/0077-the-quiet-sky.md)/[0078](done/0078-the-ink-learns-to-bite.md)/[0079](done/0079-the-attractor-learns-new-figures.md), nothing absorbed into the plan. Suite 665/665 after the merge with `main`; fmt + clippy clean. **Curation (step 3b), from a fresh `--report` run over the final 27 at this close:** zero near-duplicates below shape 0.08 in all nine families, every gate branch taken under the 110 BPM probe, no clamp saturated, `occ` 0 across the set; the workaround grep finds no header citing an already-fixed defect — three cite approved-but-unbuilt fixes (Perseids → [0077], Shatter's rebuild → [0077], Etching's duotone → [0078]), each already named inside its fixing plan. No curation action owed; the set ships as authored.
- [0076 — The second layer: a preset composes two scenes (R3)](done/0076-the-second-layer.md) — closed 2026-08-11. Review: **no blockers, no majors; one minor** (Phase 2's commit message attributed the memory measurement to WARP when it ran on the hardware adapter — corrected of record inside Phase 4's own commit, nothing left to fix) plus close-time roster/link staleness repaired in the close commit. **R3 lands: a preset composes a second scene through one optional `[layer]` table**, joined `under` (same target, one extra draw, one substance) or `over` (own offscreen, linear-light blend between kaleidoscope and bloom — `add`/`screen`/`multiply`/`overlay` fixed at load, `mix` bindable). **Per-preset scene instances ended the one-instance-per-system roster** (the user's call in ADR-0090), and the Phase 2 discovery is recorded where it was found: a shared `LineRenderer` is *not* shareable between two live line draws (`Queue::write_buffer` applies before the submission's passes), so a layer line scene carries its own. **Layerless presets are byte-identical by construction and by count** — backdrop + scene + tonemap is still exactly 3 draws, and `mix = 0` is byte-identical through both junction positions. The routing junction stays a pure function, unit-enumerated over all eight active-flag combinations without a GPU. Reachability, saturation and the report walk `[layer]` bindings under their own namespace; a dead layer gate flags. Measured on this box, stated as measurements (ADR-0071): attractor+RD ~11.9-12.9 ms/frame at 1080p Floor, layered RD pair +303 MB peak working set (debug/WARP; the release-hardware expectation is the ~33 MB texture arithmetic), layered gate fixture +0.9 ms/frame. Two golden fixtures pin both joins, adapter-compared before blessing (WARP vs hardware mean 0.0002). WARP aliases the same-system pair's identical layouts, so the independence guard runs on hardware and skips-with-notice on software (ADR-0058 posture). The Phase 5 verdicts: pre-bloom `over` reads as intended, all four modes ship under their names, and the fullscreen-`under`-occludes finding became authoring guidance in `presets/README.md`'s new `[layer]` section. No shipped preset declares a layer yet — that is [0075] cohort 6's work, now unblocked. Curation (step 3b): no preset content landed and no engine defect was fixed, so no workaround sweep was owed; verdict "no content change".
- [0053 — The suite stops blessing what WARP gets wrong](done/0053-the-suite-stops-blessing-what-warp-gets-wrong.md) — closed 2026-08-09. Review: **no blockers, two majors, four minors**. **The plan was written to prove the collisions benign and instead found two live mis-renders and fixed them.** `background-bind-layout` collided with the fullscreen scenes' single uniforms and rendered the fragment field a **flat grey** on WARP (`142.712` on all three channels against hardware's `131.010 170.559 141.381`); `blend-bind-layout` collided with `trails-bind-layout` and rendered a dissolve between two `trails`-binding presets wrong (`08.999 38.682 45.794` against `17.572 48.211 50.116`). Both fixed by an explicit `min_binding_size`, each isolated against a no-collision control that agrees to **0.02 of one 8-bit level**. That makes ADR-0058's "nothing is observed to be wrong" and "it fixes nothing that is currently broken" **false**, recorded in its [Outcome](../adrs/0058-bind-group-layout-collisions-carry-evidence.md#outcome--2026-08-09-at-plan-0053s-close). Nine colliding pairs became **four** allowlisted, each carrying a measurement; the shape the test keys on widened from binding *kinds* to kinds + visibility + whether a `min_binding_size` is declared, the first forced by ADR-0058's own boxed note and the second **measured here, twice independently**. Guard confirmed in the reverted direction at the review: dropping either fix back to `gpu::uniform` re-collides its pair and fails. **Zero baselines moved** — a bless-to-bless control differing only by reverting the two fixes came back hash-identical across all 20 PNGs. The line seam's lit-backdrop guard went from **15 channels** of reach to the whole stroke footprint (779 of 28 173 post-fix against 28 178 pre-fix) via a fourth capture at zero emitted light, and the swarm took the same arm. **Two majors left as findings, both consequences of the fix rather than defects in it:** the allowlist's `RIG` names neither adapter, in a repo whose ADR-0074 `Outcome` established five days earlier that this box's WARP 10.0.19041 is the outlier and CI's 10.0.26100 is not — so four `AGREES` entries grant permission on a build never measured; and `core/tests/background_composite.rs` still skips on every software adapter citing the quirk this plan **fixed**, so a check CI has never run may now be liftable (unmeasured — the attractor half is a different layout group). **Phase 3 is `human` and was run by `dev` under the user's explicit authorization at the gate.**
- [0046 — Transformed feedback: the past learns to move](done/0046-transformed-feedback.md) — closed 2026-08-09. Review: **no blockers, no majors, four minors, two nits**. **R2 lands, and with it the last engine gate on [0075]** — both accumulation buffers now resample their past through one shared transform, so a zoom is a tunnel and a rotation a spiral, and the standing `trails` frame-rate defect (a `fade` applied once per *frame*, a third as long at 144 Hz as at 48) is retired en route. **Zero pixels moved without opt-in**: all 20 pre-existing baselines hash-identical to a clean-`main` bless, three added, re-measured at this close after the merge. **[ADR-0037] was caught for the third time here** — `Trails::resolve` had been ignoring its `surface` argument on a premise a rotation falsifies — and the fix carries the negative control this rule has always needed: a spun ring boxes **45x46** at a portrait 100x160 target, against **44x71** with the aspect forced to `1.0`. **One contract narrowed:** `[feedback] blend` reaches the trails stage only, because the attractor deposits additively in one pass and a `max` there would cost a second pipeline — the exact WARP hazard the one-shader warp family avoids. The plan sentence changed, not the code. **Alternative D was overcharged**: including the attractor cost one shared `Transform` and one WGSL snippet concatenated into both shaders, not "a second shader and test surface". Two Phase 5 observations became [backlog 0082](../design-backlog.md) (`frame_ms_p99` spikes to 25.0 ms on preset switches, and the not-yet-built quality governor is specified to read that column) and [backlog 0083](../design-backlog.md) (RSS 385 → 663 MB over three minutes, with no no-feedback control beside it).
- [0068 — Why the downbeat rarely locks](done/0068-why-the-downbeat-rarely-locks.md) — closed 2026-08-09. Review: **no blockers, no majors, two minors, one nit**. **It shipped the diagnosis and no fix, as designed, and that was verified rather than assumed** — `CONFIDENCE_THRESHOLD` is still `0.25`, `BASS_WEIGHT` still `0.7`, the fold and the confidence measure are untouched, and the `effect_size` split is arithmetic-identical on every branch. The probe reads the gate rather than second-guessing it: `terms()` is `&self`, allocation-free, clock-free, and its `to_bits()` equality against the published `BarClock::confidence` is asserted on all six cases. **The named cause is the accent feature's bass weighting**, and the finding that names it is that backbeat rock/pop locks **48x worse** than four-on-the-floor — 0.14 % against 6.79 % over 98 minutes — because a bass accent marks every beat in one and the half-bar in the other, so it hardly ever marks the *bar*. [ADR-0082](../adrs/0082-the-downbeat-gate-holds-and-the-estimator-is-diagnosed-first.md) is **accepted** carrying that dated `Outcome`, including the limit that matters most: the 1 Hz log records **band levels, not per-beat accents**, so this is a ladder match plus a construction argument and *not* a direct measurement of the four alignment scores on real audio. **The repair has no ADR and no plan**, and that is a decision taken at this close rather than an omission: the route is named ([design-backlog 0042](../design-backlog.md), answered in place) but the fork an ADR would decide — a stronger accent feature versus a longer history window — is not yet a real fork, because ADR-0082's `Outcome` exonerates the fold and the confidence measure. It stays a backlog pointer until someone takes it.
- [0067 — The curation route](done/0067-the-curation-route.md) — closed 2026-08-09. Review: **no blockers and no code findings**. The one substantive item was a **factual error in the plan** — its claim that `bar` "stopped being a variable at ADR-0050" is false (`bar` is `VAR_NAMES[5]`, the beat phase in `[0, 1)`; ADR-0050 *added* `bar_phase` alongside it) — struck in the plan and in [ADR-0081](../adrs/0081-the-content-lane-lands-presets-and-architect-curates-the-set.md)'s `Outcome`, which repeated it. **The gate is now worth leaning on**: `reactivity` drives PCM through the real analyzer, with a non-vacuity test that has a positive control, and ADR-0081's gate-strength Negative is discharged. **Phase 1d is a recorded negative result** — the resolution ladder is flat because `frame_diff` scores occupancy and occupancy is scale-invariant, so `ANIM_FLOOR` and `SIZE` did not move and CI pays nothing; [backlog 0009](../design-backlog.md) now needs a coverage-aware statistic, which is the earned question. Two costs the lane refused to absorb went to [backlog 0080](../design-backlog.md) (reactivity 1.8x, ~85 % of it warm-up hops that are rendered and thrown away) and the Coral Oracle's gain exception to [backlog 0081](../design-backlog.md) (the house gain rule is written down nowhere).
- [0064 — The symmetry stage and the banded palette](done/0064-the-symmetry-stage-and-the-banded-palette.md) — closed 2026-08-09. Review: **no blockers and no code findings**; the three items raised were all stale text in the plan itself, corrected at the close (its "fourteen" baselines are **19**, its five LUT sample sites are **seven**, and its "duplicated at five sites" risk is **three** WGSL copies plus four calls into the one Rust function). ADR-0037 verified clean at `kaleidoscope.rs:1116` — the aspect comes from the render target, in the stage that has shipped that bug twice. All 19 pre-existing baselines byte-identical. **[ADR-0077](../adrs/0077-the-symmetry-stage-owns-one-coordinate-map.md) was accepted with an [Outcome](../adrs/0077-the-symmetry-stage-owns-one-coordinate-map.md#outcome--2026-08-09-at-plan-0064s-close): its "the inner rings alias severely" was inferred from a texel ratio and never observed** — six cutoffs, three sources, no visible onset — so `kaleido_inner` ships as styling with a protective side effect, not as the rescue. **First run of close-ceremony step 3b**: no near-duplicate geometry in any of the nine families, and nothing in `presets/` still pays for a fixed defect.
- [0071 — Light that adds without covering (`occlude`)](done/0071-light-that-adds-without-covering.md) — closed 2026-08-09. Review: no blockers, two majors, three minors, two nits. **Phase 5 is `human` and deliberately outstanding — see Standing.** The default stayed `1.0` by the user's look, no preset binds it, and **no golden baseline moved** (measured as a bless-against-a-clean-`main`-bless, all 19 hash-identical). Three ADR-0085 claims were falsified and are in its [Outcome](../adrs/0085-how-much-a-scene-occludes-the-backdrop-is-one-number.md#outcome-2026-08-09-after-plan-0071): there was no "one seam" (six sites, four shaders), the `Scene` trait *did* widen (`set_occlude`, its fourth optional method — it meets [ADR-0030](../adrs/0030-scene-target-size-hot-path-hook.md)'s three conditions, checked at the close), and the families *did* drift (additive scenes with an empty chain are unoccluded whatever they bind). It also emptied [ADR-0058](../adrs/0058-bind-group-layout-collisions-carry-evidence.md)'s `[Texture, Sampler]` group — see [0053]'s row.
- [0072 — The backdrop joins the palette](done/0072-the-backdrop-joins-the-palette.md) — closed 2026-08-09. Review: no blockers, no majors, four minors, three nits. **The last surface outside `[palette]` joined it**: no cosine copy remains in `background.rs`, and `saturation` / `palette_mix` fan out to the sky through one binding. Two of the plan's own claims were falsified and are recorded in [ADR-0086](../adrs/0086-the-backdrop-colours-through-the-preset-palette.md)'s `Outcome` — **the two fixtures it ordered re-blessed pin no pixels**, and its "fifteen" is 18 by its own grep (16 in scope, 3 moved). Zero golden baselines changed.
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
5. **Packaging & release** — GitHub release zip: unsigned standalone exe +
   `.fb2k-component` (NFR §8).

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
