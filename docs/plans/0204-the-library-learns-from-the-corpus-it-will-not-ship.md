# 0204 — The library learns from the corpus it will not ship

> **Status:** approved
> **Created:** 2026-09-19
> **Approved:** 2026-09-19 (user)
> **Owner skill(s):** human (the `preset-author` lane throughout)
> **Related ADRs:** [0227](../adrs/0227-a-borrowed-look-is-authored-natively-and-the-reference-never-enters-the-repository.md)
> (proposed), [0113](../adrs/0113-milkdrop-presets-are-translated-ahead-of-time-onto-a-warp-mesh-idiom.md),
> [0081](../adrs/0081-the-content-lane-lands-presets-and-architect-curates-the-set.md),
> [0089](../adrs/0089-the-library-renews-by-replacement-cohorts.md),
> [0017](../adrs/0017-preset-author-skill-lane.md)
> **Sequenced with:** [Plan 0232](done/0232-the-library-is-walked-cut-and-refilled.md) (added 2026-09-27):
> Phases 1-2 may run any time; Phase 2's cohort is chosen against 0232's `## Gaps`, and Phase 3
> lands after 0232 Phase 3's cull.

## TL;DR

The user browsed the converted MilkDrop corpus and picked twenty-one looks worth having. This plan
turns a small, judged first cohort of them into **native** presets — each routed to whichever
system expresses it best, authored from a rendered reference that never enters the repository, and
landed through the ordinary curation route. The first visible behaviour is a reference sheet under
`WORK/milk-browse/refs/` that the content lane can author against; the last is a look verdict that
decides whether the remaining picks are worth doing at all.

## Context & problem

**The content gap is real and the import route is closed.**
[ADR-0113](../adrs/0113-milkdrop-presets-are-translated-ahead-of-time-onto-a-warp-mesh-idiom.md)
sized it honestly: this engine's library is measured in the low hundreds, the MilkDrop lineage's in
the tens of thousands. It built `milkconv` and the `warp_mesh` idiom, and then declined to decide
whether anything converted may ship —
[Plan 0100](done/0100-the-engine-speaks-milkdrop.md) Phase 8 still owns that, and its standing
answer is *nothing third-party in the repository or a release*.

**So the corpus became a catalogue instead of a source.** On 2026-09-19 the `cream-of-the-crop`
collection was converted into a scratch directory outside the checkout and browsed theme by theme
through `RLX_PRESET_DIR`. The picks that came back, with the theme they were found in:

| # | Theme | Pick | System |
|---|---|---|---|
| 1 | Reaction | `$$$ Royal - Mashup (157)` — molten gold, black veining on cream | `warp_mesh` - a radial zoom whose feedback advects the orange, black and cream veining outward, a streaked tunnel |
| 2 | Reaction | `$$$ Royal - Mashup (275)` — fire field, red into orange, black cracks | `warp_mesh` - a rotation-plus-zoom vortex churning yellow into red round a hot eye; the swirl is the look |
| 3 | Reaction | `A HD ADAMFX Creation !!FT Martin + flexi + …INFECTIONFX A` — dark, faint blue streaks | `cellular` (`larger_than_life`) - white blobs crawling on black; the flip to white is `ink_amount` stepped to `1` on a held edge |
| 4 | Reaction | `A Remixed Digital Echasketch Again 2 martin …mstress + 4` — vertical bands, olive into teal | `warp_mesh` - a downward drift with per-column stretch carries the drips and the vertical bands |
| 5 | Reaction | `AdamFX 2 Geiss - Mash-Up Sphere Xibit Graffiti Warp me Tydye4` — blue field, green cellular veins | `fragment_field` - a blue ripple field folded eightfold by the engine-wide `kaleido_*` |
| 6 | Reaction | `Flexi - alien web bouncer [39]` — yellow and orange, black-cored red webbing | `reaction_diffusion` - black-cored Gray-Scott spots ringed in red, on a fire palette |
| 7 | Dancer | `$$$ Royal - Mashup (139)` | `parametric_curve` - two pastel ribbons swinging on a light ground, the ground from `ink_*`/`paper_*` |
| 8 | Dancer | `EoS - nematodes C` | `parametric_curve` - one smoky orange streamer, a single long line gesturing on black over `trails` |
| 9 | Dancer | `Isosceles mashup13` | `parametric_curve` - a dense knot of glowstick dash-trails, a high-order curve under heavy `trails` |
| 10 | Drawing | `EVET - Brainlocknrelease` — dark maroon | `emitter` - an upward fountain on a maroon ground whose accumulated sparks build a glowing dome |
| 11 | Drawing | `Stahlregen & fiSHbRaiN + flexi + Geiss + shifter - Stonecraft (Dense)` — angular terracotta blocks | `emitter` - rectangular sparks fanned out and folded by `mirror_*` (alternative: `shape_collage`) |
| 12 | Geometric | `EoS - magnetosphere 03` | `plexus` (partial) - an orbiting wire network; there is no sphere layout, so the turning sphere is only approximated |
| 13 | Sparkle | `Serge + martin - crystal palace001d` — deep blue, white streak | `fragment_field` - a busy cyan field with circular filigree, mirrored by `kaleido_*` |
| 14 | Sparkle | `Waltra - Square Party` — green field, yellow square spray | `emitter` - spawned squares folded into a diamond lattice by `kaleido_*` |
| 15 | Particles | `EoS - more waveforms 5` — orange diagonal on black | `swarm` - point trails orbiting a hot ring core in a vortex flow |
| 16 | Particles | `amandio c - pulse - we are going to need your permission` — dense grain, red form | `fragment_field` + `[layer]` `shape_field` - a grain ground with the solid red form joined `over` it (ADR-0090) |
| 17 | Supernova | `Geiss - 3D - Luz` — monochrome diagonal streaking | `emitter` - monochrome streaks thrown radially from a dark disc |
| 18 | Hypnotic | `Flexi - alien complex 02` — blue gradient | `fragment_field` - a slow magenta-to-green domain-warped gradient with one wandering filament |
| 19 | Hypnotic | `TonyMilkdrop - Nuclear [Flexi - help out + multiverse] --- Isosceles edit1` — amber marble, red veins | `fragment_field` - amber marbling with red veins (0232's gap table routes it to `warp_mesh`) |
| 20 | Hypnotic | `goody + flexi - emotive dissonance - integral anomaly - dissonant rift3` — silver folded satin | `fragment_field` - folded satin from domain-warped noise on a silver palette |
| 21 | Hypnotic | `i made hexcollies day!` — magenta and green diagonal stripes | `shape_field` - a four-sided polygon at 45 degrees whose `palette_steps` contours are the nested diamonds, two-tone and colour-cycling |

Phase 2 filled the `System` column on 2026-10-01, reading the Phase 1 clips rather than the stills. The look notes are thin on purpose —
they were taken from a window title strip, not from the frame, which is the problem Phase 1 exists
to fix.

**Three constraints make this a plan rather than a sitting.** The looks must be reached natively,
with nothing from the corpus entering the tree, including renders — that is
[ADR-0227](../adrs/0227-a-borrowed-look-is-authored-natively-and-the-reference-never-enters-the-repository.md).
The picks are warp-idiom in origin but not all warp-idiom in nature, so each needs routing to a
system rather than a blanket assignment. And
[ADR-0089](../adrs/0089-the-library-renews-by-replacement-cohorts.md) says the library renews by
cohorts, so twenty-one presets is not a thing to author before anyone has seen whether one works.

**It sits behind [Plan 0201](done/0201-the-warp-surface-stops-lying.md), and that ordering is
load-bearing.** That plan's own TL;DR says `warp_mesh`'s `zoom` doc *"says the opposite of what its
shader does and four generated surfaces carry the lie"* — and that the lie has **already produced a
false paragraph in shipped content**. Any pick that routes to `warp_mesh` would be authored against
a parameter reference known to be inverted. Waiting costs a queue position; not waiting costs
re-tuning every preset that bound `zoom` from the reference.

## Decision

We will author a **small first cohort** from the picks, **routing each to the system that expresses
it best**, working from **rendered references held outside the repository**, and **judging the
result in the live app before committing to the rest**.

Each of those four was chosen against a named alternative. We rejected authoring everything on
`warp_mesh` because several picks read as cellular, reaction-diffusion or analytic-field looks that
happen to have been built on a mesh, and routing them by origin rather than by nature would
reproduce the mesh's constraints for no gain. We rejected starting before Plan 0201 because the
`zoom` reference is known false today. We rejected all twenty-one in one campaign because nothing
yet says a native reading of a borrowed look is worth having, and ADR-0089's cohort mechanism
exists for exactly this. And we rejected committing the reference renders because a render of a
preset is derived from that preset — ADR-0227 Alternative C.

## Architecture diagram

```mermaid
flowchart LR
    subgraph outside["outside the checkout — WORK/"]
        corpus["milkdrop-corpus/<br/>.milk files"]
        browse["milk-browse/&lt;theme&gt;/<br/>converted .toml"]
        refs["milk-browse/refs/<br/>rendered stills + clips"]
        picks["milk-browse/picks.tsv<br/>the 21 names"]
    end

    subgraph repo["the repository"]
        plan["docs/plans/0204<br/>the routing table"]
        presets["presets/*.toml<br/>native worlds"]
    end

    corpus -->|milkconv, dev tool| browse
    browse -->|shot| refs
    browse -->|RLX_PRESET_DIR browse| picks
    picks --> plan
    refs -.->|read by eye only| presets
    plan --> presets

    classDef ext fill:#f5f5f5,stroke:#999,color:#333
    class corpus,browse,refs,picks ext
```

The dotted edge is the whole decision: the reference is **looked at**, never read as source and
never copied across the boundary.

## Implementation phases

Every phase here is `human` — this is content-lane work throughout, in the sense
[ADR-0017](../adrs/0017-preset-author-skill-lane.md) established and the standing sittings in
[`docs/content-brief.md`](../content-brief.md) already use. There is no Rust or C++ in this plan.
A conductor run would park at Phase 1 and stay parked, per
[ADR-0205](../adrs/0205-an-approved-plan-runs-under-a-conductor-and-every-judgement-it-cannot-make-parks-the-plan.md);
that is correct, not a defect.

**This plan's `human` phases may write to the routing table above.** That is a scoped exception to
the usual rule that an implementer touches only `Status:` and the log — the table is this plan's
working surface, and Phase 2's output has nowhere better to live.

### Phase 1 — The reference sheet

- **Owner skill:** human
- **What:** Render every pick to something the content lane can actually judge, under
  `WORK/milk-browse/refs/`, outside the checkout.
- **Files touched:** none in the repository. Output is `WORK/milk-browse/refs/<NN>-<slug>/` plus a
  `refs/README.md` **outside the repo** recording the exact `shot` invocations.
- **Done when:** each of the twenty-one picks has, at minimum, a loud frame and a quiet frame at a
  stated `--set` stimulus; and every pick whose interest is its *motion* rather than its palette
  also has a short clip. A still of a feedback world understates it — that is the failure this
  phase exists to avoid, so the judgement of which picks need a clip is part of the phase, not a
  preamble to it. The `refs/README.md` records the commands, so the sheet is reproducible by
  anyone who has the corpus.

### Phase 2 — The routing table and the cohort

- **Owner skill:** human
- **What:** Fill the `System` column for all twenty-one picks, and name the first cohort.
- **Files touched:** this plan (the routing table in `## Context & problem`).
- **Done when:** every row carries a `SystemKind` and one sentence naming what in that system
  carries the look — `warp` for a drifting-sinusoid churn, the reaction-diffusion field for a
  veined skin, `analytic_field` for a banded interference, and so on. A pick with **no** native
  home is recorded as such rather than forced into one: that is an engine-gap finding for
  [`docs/design-backlog.md`](../design-backlog.md), and routing it to the nearest system anyway
  would bury exactly the signal this plan is best placed to produce. The cohort named for Phase 3
  is four to six picks and **spans at least three distinct systems** — a cohort routed to one
  system tests the idiom, not the routing, and the routing is this plan's claim.

### Phase 3 — The cohort is authored

- **Owner skill:** human
- **What:** Author the cohort as native presets and land them in the shipped set.
- **Files touched:** `presets/*.toml` (new files, named by their system's filename family so the
  generated editor schema applies), and `presets/README.md` only if a hand-written structural or
  palette table needs it.
- **Done when:** every preset in the cohort passes the behavioral suite and lands through the
  [ADR-0081](../adrs/0081-the-content-lane-lands-presets-and-architect-curates-the-set.md) route;
  each carries a header stating its own mechanism and **naming no third-party preset**, per
  ADR-0227; and each was rendered and looked at before it was committed. Note what the gate is
  worth while leaning on it: of the five, only `reactivity` drives PCM through the real analyzer
  ([`docs/testing.md`](../testing.md) carries the table), so a green suite says these presets are
  *sound*, never that they are *good*.

### Phase 4 — The verdict, in the live app

- **Owner skill:** human
- **What:** Decide whether a native reading of a borrowed look is worth having, and therefore
  whether picks 7–21 proceed.
- **Files touched:** this plan (the verdict), and `docs/content-brief.md` if the answer is yes and
  the remainder becomes a standing sitting.
- **Done when:** the cohort has been judged **in the running app against its reference sheet, not
  as stills** — launched from a filtered directory via `RLX_PRESET_DIR`, which hot-reloads — and
  the verdict is written down as one of three: the route is worth the remaining picks; it is worth
  it only for certain systems (named); or it is not worth it and the picks are abandoned with the
  reason recorded. **A negative verdict is a real outcome of this plan, not a failure of it** —
  Plan 0142 Phase 4 has already returned a "no" on the neighbouring question three times, and a
  fourth honest one is worth more than a cohort nobody wanted.

## Data shapes

None. This plan introduces no types, no parameters and no engine surface. If Phase 2 finds a look
that needs one, that is a feedback note to `architect`, not a phase of this plan.

## Risks & open questions

- **Plan 0201 slips, or is reordered ahead of this one's start.** Then any `warp_mesh` routing is
  authored against the inverted `zoom` reference. Mitigation: Phase 2 may route and Phase 1 may
  render regardless — only Phase 3's `warp_mesh` entries actually block on 0201. If the wait
  becomes real, author the cohort's non-`warp_mesh` members first.
- **The routing hypothesis may not survive contact.** It is entirely possible that what makes these
  looks good *is* the mesh, and that a reaction-diffusion reading of a mesh-built veining is a pale
  thing. Phase 2 recording "no native home" is the designed escape; the failure mode to avoid is
  quietly widening `warp_mesh` to swallow the difference, which would be engine work smuggled into
  a content plan.
- **The references are unreproducible from a checkout.** ADR-0227's stated price. The corpus is not
  in this repository and will not be; a future reader has the plan's prose and nothing else. There
  is no mitigation, only the disclosure.
- **"In the spirit of" has no done-when.** Phase 4 is a human look call by construction. This plan
  does not pretend otherwise and does not invent a metric to launder the judgement.
- **ADR-0089's cohort mechanism is invoked but not exercised.** A four-to-six preset cohort added to
  the shipped set grows it; whether anything is displaced is `architect`'s curation call at close
  (close-ceremony step 3b), not a phase here. Open question: if the verdict is strongly positive,
  does the remainder run as one campaign or as further cohorts? Phase 4 answers it.

## What this plan does NOT do

- **It does not answer Plan 0100 Phase 8.** Provenance stays open, and nothing here may be cited as
  having settled it.
- **It does not import, convert or ship anything third-party.** No `.milk`, no `milkconv` output,
  no render of either, in the repository or a release.
- **It does not touch engine Rust.** No new scene, param, expression function, curve family or
  shader. Anything a look needs that the preset surface cannot express routes to `architect` as a
  backlog entry — that is the [ADR-0017](../adrs/0017-preset-author-skill-lane.md) boundary and
  this plan does not bend it.
- **It does not author picks 7–21.** Only the Phase 2 cohort. The rest wait on Phase 4's verdict.
- **It does not fix the warp surface.** [Plan 0201](done/0201-the-warp-surface-stops-lying.md) owns
  that; this plan consumes it.
- **It does not add a copy key to the app.** Collecting preset names by watching the window title
  from outside was sufficient; a clipboard binding would be a `dev` plan and a new dependency
  against NFR §4.

## Implementation log

> Written by the lane — one row per phase as that phase's commit lands, and the close block after
> the last one. **The phases above are the contract; everything here is what happened.**

**Lane:** `main` for the log; Phase 1 writes nothing in the repository

| phase | owner | state | commit |
|---|---|---|---|
| 1 — The reference sheet | human | done | committed with this row |
| 2 — The routing table and the cohort | human | done | committed with this row |
| 3 — The cohort is authored | human | not started | |
| 4 — The verdict, in the live app | human | not started | |

### Notes

- **Phase 1, 2026-09-29** (built by an agent session at the owner's request, from `017c1a35`). The
  sheet is `WORK/milk-browse/refs/`, outside the checkout, 125 MB: one folder per pick
  (`01-molten-gold` to `21-hexcollies`), each holding `converted.toml`, a loud and a quiet still at
  960x540 after 300 frames (the `--set` stimuli of the `preset-author` skill's loud/quiet commands),
  an 8 s clip at 640x360 cut from the 0212 music excerpt, and the logs. `refs/README.md` records every
  command, and `make.sh` with `picks.tsv` re-creates the sheet from the corpus.
  - **All 21 converted and none was rejected.** The only conversion notes are names the engine does
    not use, such as `monitor`. No render is blank (`sheet_loud.png` tiles the 21 loud stills); pick
    03 is mostly dark, and its README line says so.
  - **Every pick got a clip**, because all 21 convert to feedback worlds and a still understates
    one. The README gives one line per pick on what its still misses. That judgement is the
    phase's own, and the owner may prune it.
  - **Two departures from the plan's text.** The picks were converted fresh from
    `WORK/milkdrop-corpus`, because the plan's `WORK/milk-browse/<theme>/` conversions exist only on
    the Windows box. Picks 03 and 04, shortened with "..." in the table, were matched to the files
    the full names imply (*INFECTIONFX A*, *mstress + 4*), and 08, 15 and 21 use the exact-name file
    where siblings share a stem.

- **Phase 2, 2026-10-01** (an agent session in the `preset-author` lane, with the owner choosing
  the cohort). Each pick was routed from four frames of its clip, taken 2 s apart, because all 21
  are feedback worlds and the loud stills alone misread several.
  - **The cohort is 02, 17, 15 and 08**: `warp_mesh` fire field, `emitter` luz, `swarm` more
    waveforms and `parametric_curve` nematodes. That is four systems, and each pick fills one of
    rows 1 to 4 of Plan 0232's `## Gaps` table, so none of them only adds a preset to the set.
  - **Two departures from that table's suggestions, both chosen by the owner.** For row 1
    (`emitter`) the table named 10 or 14, and the cohort takes 17, whose streaks thrown radially
    from a dark disc are the row's sparks-on-each-hit. It is also monochrome, which matches the
    owner's standing black-white ask. For row 2 (`swarm`) the cohort takes the table's alternative,
    15, because most of 16's look is a solid red form rather than its grain. Row 3 takes 02, the
    churning vortex, over 01, whose radial tunnel sits close to `warp_wellhead`'s depth idiom.
  - **One pick has no native home: 16.** A grain field under a solid sweeping form is two layers,
    and a preset names one system. Two picks are only partial. 03's whole-frame flip to white has
    no counterpart in `cellular`. 12's turning wire sphere is approximated by `plexus`, which has
    only the `cloud` and `sheet` layouts. All three go to `architect` as feedback. This session
    filed no backlog entry itself, because one needs a probe and the backlog is not this lane's
    file.
  - **Corrected by `architect` 2026-10-01: two of the three gaps are not gaps.** Pick 16 has a
    native home. The `[layer]` table (ADR-0090) composes a second system, and its `over` join puts
    a solid form on a field. Pick 03's flip is `ink_amount = "1"`, which already draws black marks
    on a white field, so binding it on a held edge flips the frame. Only 12's sphere is a real gap,
    filed as backlog 0278 and deliberately low priority. The routing table carries the corrections.
  - **Phase 3 still waits on Plan 0232's cull reaching main**: on 2026-10-01 it was done in that
    plan's lane (`72f29c96`) but not merged.

### Close triggers

- **`presets/` touched:**
- **Plan header `Closes:`** none
- **What shipped:**
- **Operator docs touched:**
- **Backlog probes (`node scripts/check-backlog-claims.mjs`):**
- **Full suite:**
- **Outstanding `human` phases:**

## Followups (after this lands)

- Picks 7–21, if Phase 4's verdict says the route is worth it — as a campaign or as further
  cohorts, per that verdict.
- Any engine gap Phase 2 records as "no native home", as a `docs/design-backlog.md` entry with a
  probe.
