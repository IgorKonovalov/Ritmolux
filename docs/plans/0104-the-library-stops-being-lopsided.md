# 0104 — The library stops being lopsided

> **Status:** in-progress
> **Created:** 2026-08-16
> **Approved:** 2026-08-16 (user)
> **Owner skill(s):** dev, human (every `human` phase is a `preset-author` session — see Risks)
> **Related ADRs:** [0081](../adrs/0081-the-content-lane-lands-presets-and-architect-curates-the-set.md) (the content lane lands presets), [0089](../adrs/0089-the-library-renews-by-replacement-cohorts.md) (renewal by replacement cohorts)

## TL;DR

The shipped library is **39 presets across 11 systems, and it is wildly uneven**: `attractor` has
17 worlds while `lsystem`, `shape_field`, `spectrum` and `star_pattern` have **exactly one each**
and `warp_mesh` has **none**. A visitor who tries this app judges the content, not the composite,
and five of eleven systems currently present themselves with one example or zero. This plan brings
every system to **at least four distinct worlds** — 18 new presets to 57 as written, 22 to 61 once
`warp_mesh` is counted — and starts by asking whether the 17-strong family is seventeen worlds or
seventeen variations.

> **Phase 1 ran 2026-08-29 and the headline above is superseded.** The library is **54 presets
> across 12 systems**; `attractor` is still 17, but `shape_field`, `parametric_curve` and
> `star_pattern` are no longer singletons, `reaction_diffusion` has cleared the floor unaided, and a
> twelfth system (`shape_collage`) exists that this plan predates. Only `lsystem` and `spectrum` are
> still singletons and `warp_mesh` is still at zero. The floor now costs **18 new presets, 54 → 72**
> — the same total by coincidence, and not one of the same terms. The seventeen-or-seventeen
> question is answered: **thirteen distinct worlds and one four-way converged cluster.**

## Context & problem

The census, counted 2026-08-16 from `presets/*.toml`:

| System | Worlds |
|---|---|
| `attractor` | **17** |
| `fragment_field` | 8 |
| `emitter` | 3 |
| `reaction_diffusion` | 3 |
| `parametric_curve` | 2 |
| `swarm` | 2 |
| `lsystem` | **1** |
| `shape_field` | **1** |
| `spectrum` | **1** |
| `star_pattern` | **1** |
| **total** | **39** |

> **Corrected 2026-08-17 — there are eleven systems, not ten.**
> [Plan 0100](done/0100-the-engine-speaks-milkdrop.md) closed 2026-08-16, the same day this census
> was counted, and shipped **`warp_mesh`** — which the table above cannot show, because it is
> counted from `presets/*.toml` and `warp_mesh` has **zero** shipped worlds. That is the emptiest
> system in the library and it is invisible to the instrument that found the problem.
> Phase 1 re-runs the census and owns the revision; what it should carry in is that
> `warp_mesh +4` makes the arithmetic below **22 new presets, 39 → 61**, and that the four should
> be authored *after* [Plan 0108](done/0108-the-milkdrop-import-gets-its-tone-back.md), whose Phase 1
> changes the tone of the feedback field these worlds would be tuned against.

Sixty-six releases of engine work produced ten rendering systems and thirty-nine presets, and
nearly half of those presets are one family. This is not a volume problem dressed up as a
distribution problem — a hundred more attractor worlds would not fix it. **Four systems present
themselves with a single example**, which reads to a new user as four systems that do one thing.

The committed gallery shows the cost directly. `attractor` — the seventeen-world family — is the
strongest picture in the repository. `swarm` (two worlds) reads as confetti with no cohesion, and
`star_pattern` (one world) reads flat, with visible beading at every vertex. The families with the
least content are the families that look worst, and the causation runs both ways: a system with one
world has never been pushed hard enough to find out where it breaks.

## Decision

Bring every system to **at least four distinct worlds**, additively, by replacement-cohort rules
([ADR-0089](../adrs/0089-the-library-renews-by-replacement-cohorts.md)) — never a delete-all reset.
~~Arithmetically that is `lsystem +3, shape_field +3, spectrum +3, star_pattern +3, parametric_curve
+2, swarm +2, emitter +1, reaction_diffusion +1` = **18 new presets, 39 → 57**.~~

> **Revised by Phase 1, 2026-08-29 — the arithmetic held its total and changed every term.**
> The library moved from 39 presets to 54 in the nine months between approval and pickup, and it
> gained a twelfth system. Measured against the scene registry: `lsystem +3, spectrum +3,
> warp_mesh +4, swarm +2, star_pattern +2, shape_field +1, shape_collage +1, parametric_curve +1,
> emitter +1` = **18 new presets, 54 → 72**. `reaction_diffusion` reached the floor on its own
> (3 → 6) and drops out; `shape_collage` is new to the plan; `warp_mesh` is still at zero.
> The per-system numbers are in Phase 1's result block below.

Four rather than a larger number because four is the smallest count at which a system can show a
*range* — two extremes and two between — and because 18 presets is one or two content sittings
rather than a quarter. The target is a **floor, not a quota**: a family that earns more gets more.

Phase 1 asks the question that could change all of this arithmetic: whether `attractor`'s 17
entries are seventeen worlds or a family that converged.

## Implementation phases

### Phase 1 — is the big family actually big

- **Owner skill:** dev
- **What:** Measure the library as it stands and publish the result into this plan.
- **Files touched:** this plan; no code.
- **Notes for the implementer:** `shot --presets presets --report` prints the near-duplicate flags
  and per-band reactivity in one command, and `--report family=attractor` narrows it. **Read the
  whole output rather than piping it through `head`** — a close ceremony missed a fourteen-line
  preset header exactly that way.
- **Done when:** the per-family near-duplicate flags and per-band reactivity are recorded here, and
  the `attractor` family has an explicit verdict: how many of its 17 are distinct worlds. If a
  meaningful number converged, **the target arithmetic above is revised in this plan before Phase 2
  starts**, because retiring three duplicates is worth more than authoring three new ones.
  **Two things the census must not miss** (both added 2026-08-17): the system roster comes from the
  **scene registry**, not from `presets/*.toml`, or a system with zero worlds is invisible to the
  instrument looking for empty systems — which is how `warp_mesh` was missed. And the verdict states
  whether `warp_mesh`'s four land in this plan or as a follow-on cohort, given it cannot be authored
  until [Plan 0108](done/0108-the-milkdrop-import-gets-its-tone-back.md) Phase 1 settles the tone.

#### Phase 1 result — counted 2026-08-29

**Roster source: the scene registry** (`SystemKind::from_name`/`as_str` in
`core/src/preset/schema.rs`), which carries **twelve** systems — the census correction box above
says eleven, and `shape_collage` is the twelfth it predates. Counted from `presets/*.toml` by
**top-level `system =` only**: four presets carry a `[layer]` with a second system of its own
(`fragment_nebula` and `fragment_sumi` layer `attractor`, `fragment_vitrail` layers
`parametric_curve`, `fragment_interferencemono` layers `shape_field`), and a layer is not a world of
the system it borrows — counting them as worlds inflates four families and hides three shortfalls.

| System | Plan's census (2026-08-16) | Now | To floor |
|---|---|---|---|
| `attractor` | 17 | 17 | — |
| `fragment_field` | 8 | 13 | — |
| `reaction_diffusion` | 3 | 6 | — |
| `shape_field` | 1 | 3 | **+1** |
| `shape_collage` | *(system did not exist)* | 3 | **+1** |
| `parametric_curve` | 2 | 3 | **+1** |
| `emitter` | 3 | 3 | **+1** |
| `swarm` | 2 | 2 | **+2** |
| `star_pattern` | 1 | 2 | **+2** |
| `spectrum` | 1 | 1 | **+3** |
| `lsystem` | 1 | 1 | **+3** |
| `warp_mesh` | 0 | **0** | **+4** |
| **total** | **39** | **54** | **+18 → 72** |

**The blind spot the plan named is still open, and the report has it too.** `shot --presets presets
--report` emits **eleven** family sections for twelve registered systems: `warp_mesh` has no section
because it has no presets, so the instrument that measures the library cannot see its emptiest
system. The roster above is therefore taken from the registry and the counts joined onto it, not
read off the report.

**Near-duplicate flags — `attractor` is the only family that flagged.** Every other family reports
`near-duplicate geometry: none below shape 0.08`. `attractor` reports five pairs over four presets:

```
NEAR-DUP: Lorenz Gallery ~ Valentine
NEAR-DUP: Lorenz Gallery ~ Butterfly to Knot
NEAR-DUP: Valentine ~ Butterfly to Knot
NEAR-DUP: Valentine ~ Rho Walk
NEAR-DUP: Butterfly to Knot ~ Rho Walk
```

Five of the six possible pairs among `{Lorenz Gallery, Valentine, Butterfly to Knot, Rho Walk}` —
only `Lorenz Gallery ~ Rho Walk` is unflagged. The four are also the family's quietest: bass 0.020 —
0.045 and mid 0.000 — 0.026 against a family spread topping out at Clifford's 0.155 / Leviathan's
0.172 treble, and all four sit in the low-motion `rate` band (0.0028 — 0.0041).

**Verdict on the 17: thirteen distinct worlds and one four-way converged cluster.** The family is
not seventeen variations — twelve of the seventeen are geometrically distinct by the report's own
measure and the convergence is confined to a single low-reactivity cluster. **This does not change
the authoring arithmetic above.** Retiring three of the four would leave `attractor` at 14, still
three times the floor, and retirement runs through
[ADR-0089](../adrs/0089-the-library-renews-by-replacement-cohorts.md)'s cohort rules with Phase 6
producing the list — it is not a Phase 1 action and it substitutes for none of the 18.

**Per-band reactivity, the families at or below the floor** (whole-frame means at full drive; read
the footprint and realistic-level tables in the raw report against these):

| Preset | system | bass | mid | treb | onset | anim | cover |
|---|---|---|---|---|---|---|---|
| Vellum | `lsystem` | 0.151 | 0.003 | 0.000 | 0.003 | 0.028 | 0.307 |
| Halo | `spectrum` | 0.047 | 0.048 | 0.039 | 0.132 | 0.012 | 0.993 |
| Drift | `swarm` | 0.105 | 0.123 | 0.007 | 0.099 | 0.104 | 0.954 |
| Shatter | `swarm` | 0.099 | 0.064 | 0.013 | 0.106 | 0.092 | 0.997 |
| Star Mandala Bordered | `star_pattern` | 0.102 | 0.097 | 0.040 | 0.007 | 0.069 | 0.597 |
| Rose Window | `star_pattern` | 0.082 | 0.006 | 0.009 | 0.007 | 0.038 | 0.514 |
| Contour Mono | `shape_field` | 0.484 | 0.000 | 0.508 | 0.707 | 0.145 | 0.554 |
| Facet | `shape_field` | 0.110 | 0.021 | 0.043 | 0.122 | 0.038 | 0.798 |
| Pulse | `shape_field` | 0.343 | 0.000 | 0.014 | 0.000 | 0.017 | 0.987 |
| Broadside | `parametric_curve` | 0.085 | 0.165 | 0.000 | 0.022 | 0.128 | 0.462 |
| Ion Wake | `parametric_curve` | 0.008 | 0.000 | 0.003 | 0.035 | 0.010 | 0.576 |
| Nightbloom | `parametric_curve` | 0.071 | 0.016 | 0.011 | 0.006 | 0.023 | 0.522 |
| Drift Field | `emitter` | 0.021 | 0.001 | 0.000 | 0.000 | 0.004 | 0.105 |
| Ember Jet | `emitter` | 0.043 | 0.002 | 0.002 | 0.000 | 0.006 | 0.253 |
| Perseids | `emitter` | 0.050 | 0.012 | 0.000 | 0.022 | 0.024 | 0.277 |
| Collage Mono | `shape_collage` | 0.056 | 0.002 | 0.000 | 0.000 | 0.002 | 0.876 |
| On White | `shape_collage` | 0.037 | 0.002 | 0.003 | 0.000 | 0.021 | 0.992 |
| Suprematist | `shape_collage` | 0.048 | 0.007 | 0.002 | 0.000 | 0.008 | 0.291 |

Two things a cohort author should carry in from that table. **`emitter` and `shape_collage` are
onset-deaf** — six of these seven presets report onset `0.000` and the seventh (Perseids) 0.022,
against `shape_field`'s Contour Mono at 0.707; whatever the new worlds do, they should not be a
fourth and a fifth entry with no transient. And **`mid` is the dead band library-wide** — eleven of
the eighteen sit below 0.010, which is the axis a range should be built across rather than around.

**`warp_mesh`'s four land inside this plan**, as a new Phase 4b. Its stated blocker is discharged:
[Plan 0108](done/0108-the-milkdrop-import-gets-its-tone-back.md) Phase 1 landed 2026-08-17, so the
feedback field's tone is settled and a cohort authored on it is authored against the shipped
surface. Splitting it to a follow-on would leave the plan closing with the library's only
zero-world system still at zero, which is the exact defect the plan was opened to fix.

### Phase 2 — the singletons get a range

- **Owner skill:** human *(a `preset-author` session)*
- **What:** ~~`lsystem`, `shape_field` and `spectrum` each go from one world to four.~~
  **Retargeted by Phase 1:** `lsystem` **+3** (1 → 4), `spectrum` **+3** (1 → 4), `shape_field`
  **+1** (3 → 4) — **seven presets**. `shape_field` is no longer a singleton; Plan 0091 and Plan
  0098 left it at three, so it needs one world rather than three.
- **Files touched:** `presets/*.toml`.
- **Notes for the author:**
  - **`lsystem` and `spectrum` are the two real singletons**, and they are the phase's weight.
    Vellum (`lsystem`) reports mid 0.003 / treble 0.000 / onset 0.003 and coverage 0.307 — it is a
    bass-only world on a third of the frame, so three siblings have most of the axis space free.
    Halo (`spectrum`) is the inverse: coverage 0.993 and the library's strongest onset-to-band
    ratio (onset 0.132 against bass 0.047), with `anim` 0.012 — a nearly still full-frame world.
  - **`star_pattern` is deliberately not in this phase.** [Plan 0087](done/0087-the-line-renderer-draws-a-curve.md)
    changes what the curve family draws and has already retired three mandala presets on
    [ADR-0098](../adrs/0098-the-line-renderer-draws-arcs-as-per-pixel-distance-fields.md); a cohort
    authored now would be authored against a surface about to move. It gets its own phase, after.
  - `shape_field` shipped with **no world of its own** by design, and
    [Plan 0098](done/0098-the-figure-nests-properly.md) is live on `shape_field.rs` — coordinate, or
    take this after it closes.
  - The reference surfaces are [`presets/README.md`](../../presets/README.md) (every parameter),
    [`docs/presets.md`](../presets.md) (the grammar) and
    [`docs/preset-palettes.md`](../preset-palettes.md) (colour). ~~Note that `docs/presets.md`'s
    `system =` table is **missing `shape_field`** as of this writing — a Plan 0091 close swept two
    of the three reference docs, not three. Fix it while you are in there.~~ **Discharged, verified
    Phase 1:** that table carries all twelve systems, `shape_field` and `shape_collage` included.
- **Done when:** each of the three systems has four worlds; every new preset clears `sanity`,
  `animation`, `reactivity` and the beat gate; and the advisory distinctness report flags **no pair
  inside a family** — or, where it does, that preset's header names why the pair is intentionally
  close. The report stays advisory ([ADR-0067](../adrs/0067-coverage-measures-the-scene-not-the-backdrop.md)
  and the gate table in [`docs/capturing.md`](../capturing.md)); this phase does not promote it to a
  gate.

### Phase 3 — the thin families fill out

- **Owner skill:** human *(a `preset-author` session)*
- **What:** ~~`parametric_curve` +2, `swarm` +2, `emitter` +1, `reaction_diffusion` +1.~~
  **Retargeted by Phase 1:** `swarm` **+2** (2 → 4), `parametric_curve` **+1** (3 → 4), `emitter`
  **+1** (3 → 4), `shape_collage` **+1** (3 → 4) — **five presets**. `reaction_diffusion` **drops
  out of this plan**: it reached 6 worlds on its own and is over the floor. `shape_collage`
  (Plan 0113) takes the slot; the plan predates the system.
- **Files touched:** `presets/*.toml`.
- **Notes for the author:** **`emitter` and `shape_collage` are onset-deaf and that is the thing to
  fix, not the count.** Drift Field, Ember Jet, Collage Mono, On White and Suprematist all report
  onset **0.000**, and Perseids 0.022; a fourth world in either family that also ignores transients
  makes the family bigger without making it a range. `emitter` additionally has the library's
  smallest footprints (coverage 0.105 / 0.253 / 0.277), so a wider world is free variety. `swarm` is
  the weakest picture in the committed gallery — it reads as
  scattered confetti rather than a flock. Two more worlds is the ask; **whether the family can read
  as cohesive at all is a question for the engine**, and if the answer is no, that belongs in
  [`docs/design-backlog.md`](../design-backlog.md) as a feedback note rather than in a preset.
  `emitter` has a standing want already: [Plan 0090](done/0090-the-emitters-source-moves.md)'s
  `human` Phase 5 asks for a quiet drifting field and a point fountain, and those are two of the
  worlds this phase would author anyway.
- **Done when:** the four systems reach their counts under the same gate and distinctness rules as
  Phase 2.

### Phase 4 — `star_pattern`, after the renderer settles

- **Owner skill:** human *(a `preset-author` session)*
- **What:** ~~`star_pattern` from one world to four.~~ **Retargeted by Phase 1:** `star_pattern`
  **+2** (2 → 4). A second world (Rose Window) landed while this plan sat.
- **Files touched:** `presets/*.toml`.
- **Notes for the author:** ~~**takeable only once [0087](done/0087-the-line-renderer-draws-a-curve.md)
  has landed or been routed to ADR-0098's Alternative C.**~~ **Blocker discharged: 0087 closed
  2026-08-27** and ADR-0098's Alternative C was *not* taken — the arc primitive shipped, so the
  family draws arcs as per-pixel distance fields and the beading the plan hedged against is gone.
  Author to the arc surface. Either outcome settles what the family
  draws; authoring before then buys a cohort that has to be redone. ~~If 0087 ends at Alternative C,
  the beading in `docs/images/gallery/star_pattern.png` is what the family looks like, and the
  worlds are authored to suit it rather than around it.~~ Both shipped worlds are onset-flat
  (0.007 each) and Rose Window is mid-flat at 0.006 — the two new ones carry the family's range.
- **Done when:** four worlds, same gates.

### Phase 4b — `warp_mesh` gets its first worlds

- **Owner skill:** human *(a `preset-author` session)*
- **What:** `warp_mesh` **+4** (0 → 4). **Added by Phase 1**, which resolved the scope question the
  census correction box left open: the four land inside this plan rather than as a follow-on cohort.
- **Files touched:** `presets/*.toml`.
- **Notes for the author:** this is the only system in the library with **zero** worlds, and it is
  the reason the plan's own instrument could not see it — `shot --report` prints a section per
  family that has presets, so a system at zero is absent rather than flagged. Its blocker is
  discharged: [Plan 0108](done/0108-the-milkdrop-import-gets-its-tone-back.md) Phase 1 landed
  2026-08-17 and the feedback field's tone is settled. `warp_mesh` **draws nothing of its own** — it
  resamples the previous frame through a per-vertex transform grid (ADR-0113), so a world is
  authored as motion over a seed rather than as a figure, and there is no sibling preset to read as
  a starting point. [`docs/presets.md`](../presets.md) carries the only worked example, and
  `per_vertex` params are the system's own surface (`warp_mesh::PER_VERTEX_PARAMS`; binding
  `per_vertex` on any other system is rejected by the loader).
- **Done when:** four worlds, same gates as Phase 2 — `sanity`, `animation`, `reactivity`, the beat
  gate, and no unexplained intra-family pair in the advisory distinctness report. The report gains a
  twelfth family section, which is the observable proof the blind spot is closed.

### Phase 5 — curate the set, not the presets

- **Owner skill:** dev
- **What:** Re-run the census and the near-duplicate sweep over the whole ~~57~~ **72**, and sweep
  the preset headers for stale workarounds. The re-census takes its roster from the **scene
  registry** for the reason Phase 1 recorded, and counts **top-level `system =` only** — a `[layer]`
  sub-system is not a world of the system it borrows.
- **Files touched:** this plan; `presets/*.toml` headers only if a workaround is stale.
- **Notes for the implementer:** the workaround sweep is one grep, and the bare `backlog NNNN` form
  is in the pattern because omitting it once cost a finding:

  ```sh
  grep -rn "ADR-00NN\|Plan 00NN\|design-backlog 00NN\|backlog 00NN" presets/*.toml
  ```

  A preset written to dodge an engine defect keeps paying for it after the fix lands, and **no
  instrument in this repository can see that** — the workaround renders fine and passes every gate.
- **Done when:** every system has ≥ 4 worlds, the final census is recorded here, and any preset
  header naming a defect that has since been fixed is listed for the content lane.

### Phase 6 — does the set read as a library

- **Owner skill:** human
- **What:** Walk the whole set in the running app, on real music, with auto-rotate on.
- **Done when:** the user says whether it reads as a library rather than as one strong family and
  ~~nine~~ **eleven** samplers — and names the worlds that should be retired. **Phase 1 hands this
  phase a starting list:** the four-way `attractor` cluster (Lorenz Gallery, Valentine, Butterfly to
  Knot, Rho Walk), five of whose six pairs the distinctness report flags. Retirement is a content-lane action
  under ADR-0089's cohort rules, not a deletion in this phase.

## Risks & open questions

- **The `Owner skill:` vocabulary has no value for the content lane, and this is the first plan to
  feel it.** The architect skill fixes it at `dev | human`, but
  [ADR-0081](../adrs/0081-the-content-lane-lands-presets-and-architect-curates-the-set.md) made
  `preset-author` a lane that lands its own commits. Marking those phases `human` is accurate
  (the user starts that session and nothing else can) but it undersells them. **Worth an ADR
  adding `preset-author` to the vocabulary** — filed as a followup rather than resolved here,
  because changing the vocabulary changes what `dev` branches on.
- **Phase 1 may invalidate the arithmetic**, and that is the point of putting it first.
- **Two phases are blocked on live plans** — Phase 2 partly on [0098](done/0098-the-figure-nests-properly.md)
  (`shape_field`), Phase 4 wholly on [0087](done/0087-the-line-renderer-draws-a-curve.md). Both are
  stated rather than sequenced, so a session can take whatever is free.
- **The weak-family problem may not be a content problem.** If `swarm` cannot read as cohesive at
  four worlds either, the finding is an engine gap and belongs in the backlog. Authoring harder
  against a real limitation produces four worlds that all look wrong.
- **Contention:** `presets/*.toml` only, plus this file. It contends with any live plan that edits
  presets for an engine reason — `dev` still does that when a param is renamed
  ([ADR-0081](../adrs/0081-the-content-lane-lands-presets-and-architect-curates-the-set.md)).

## What this plan does NOT do

- **It does not touch engine Rust.** Anything a look needs that the surface cannot express is a
  backlog note, not a phase.
- **It does not raise any gate's floor.** New content passes the gates as they stand.
- **It does not delete presets.** Retirement runs through ADR-0089's cohort rules, and Phase 6 only
  produces the list.
- **It does not target 100 presets.** The floor is four per system, arrived at by counting rather
  than by ambition.
- ~~**It does not cover `warp_mesh`** ([Plan 0100](done/0100-the-engine-speaks-milkdrop.md)), which does
  not exist yet. When it does, the per-system floor applies to it too.~~
  **Stale as of 2026-08-17: it exists.** Plan 0100 closed 2026-08-16 and `warp_mesh` ships with zero
  worlds, so by this plan's own floor it is in scope and is the emptiest system in the library — see
  the correction box under the census. ~~Whether the four land inside this plan or as a follow-on
  cohort is Phase 1's call; the one constraint is that they come after
  [Plan 0108](done/0108-the-milkdrop-import-gets-its-tone-back.md) Phase 1, which moves the feedback
  field's tone under anything authored on it.~~ **Phase 1 answered it 2026-08-29: the four land
  inside this plan, as Phase 4b.** 0108 Phase 1 landed 2026-08-17, so the constraint is discharged.
- **It does not cover `shape_collage` beyond the floor.** The twelfth system
  ([Plan 0113](done/0113-the-engine-paints-a-canvas.md)) postdates this plan entirely; Phase 1 folds
  its `+1` into Phase 3 because the floor applies to it like any other system, not because this plan
  has anything to say about the canvas idiom.

## Implementation log

> Written by `dev` — one row per phase as that phase's commit lands, and the close block after the
> last one. **The phases above are the contract; everything here is what happened.**

**Lane:** `WORK/lmv-plan-0104` on `plan-0104-library-lopsided`, branched from `main` at `5590a4f`.

| phase | owner | state | commit |
|---|---|---|---|
| 1 — is the big family actually big | dev | done | `7561492` |
| 2 — the singletons get a range | human (`preset-author`) | done | `b2866ec` |
| 3 — the thin families fill out | human | not started | — |
| 4 — `star_pattern`, after the renderer settles | human | not started | — |
| 4b — `warp_mesh` gets its first worlds | human | not started | — |
| 5 — curate the set, not the presets | dev | not started | — |
| 6 — does the set read as a library | human | not started | — |

### Notes

- **Phase 1 edited the Phase 2, 3, 4, 5 and 6 blocks, which this lane is normally prohibited from
  doing.** The user authorized it explicitly at the Step-2 gate, choosing it over recording the
  falsified targets and routing them to architect. Phase 1's own done-when licenses revising "the
  target arithmetic above"; rewriting per-phase system lists and adding **Phase 4b** goes past that,
  and architect should read those five blocks as `dev`-authored.
- **Phase 4b is new** — a phase this lane added, not one the architect wrote.
- The census counts **top-level `system =` only**. Four presets carry a `[layer]` with a second
  system; counting layers as worlds reads `shape_field` and `parametric_curve` at 4 (at floor) when
  they are at 3, and inflates `attractor` to 19. The first pass of this phase made that error and
  the corrected count is what every number in the plan now carries.
- `reaction_diffusion` is dropped from Phase 3 — it is at 6 worlds, over the floor, and needs
  nothing.
- Two followups noticed and not acted on, both out of this plan's files-touched: `shot --report`
  prints a section per populated family rather than per registered system, and
  `docs/capturing.md`'s `--report family=` row lists 8 of 12 systems. Both are filed under
  `## Followups`.
- Phase 1's done-when asks for a verdict on `attractor`; the verdict does **not** reduce the
  authoring count, and the reasoning for that is in the result block rather than here.

#### Phase 2 result — authored 2026-08-29

Seven worlds, all fresh-slate ([ADR-0089](../adrs/0089-the-library-renews-by-replacement-cohorts.md)):
`lsystem` **1 → 4** (Coral, Rime, Bower), `spectrum` **1 → 4** (Skyline, Ridge, Anemone),
`shape_field` **3 → 4** (Aperture). Library **54 → 61**.

| Preset | system | bass | mid | treb | onset | anim | cover |
|---|---|---|---|---|---|---|---|
| Coral | `lsystem` | 0.044 | 0.035 | 0.004 | 0.000 | 0.012 | 0.810 |
| Rime | `lsystem` | 0.045 | 0.004 | 0.009 | 0.004 | 0.014 | 0.866 |
| Bower | `lsystem` | 0.022 | 0.031 | 0.007 | 0.023 | 0.013 | 0.813 |
| Skyline | `spectrum` | 0.040 | 0.070 | 0.006 | 0.088 | 0.002 | 0.913 |
| Ridge | `spectrum` | 0.065 | 0.038 | 0.008 | 0.028 | 0.007 | 0.817 |
| Anemone | `spectrum` | 0.020 | 0.011 | 0.004 | 0.031 | 0.012 | 0.526 |
| Aperture | `shape_field` | 0.070 | 0.106 | 0.030 | 0.040 | 0.031 | 0.981 |

**Gates: `cargo nextest run -p lmv-core`, 851 passed, 5 skipped.** The advisory distinctness
report flags **no pair inside any of the three families** — the only `NEAR-DUP` lines the library
emits are still the four-way `attractor` cluster Phase 1 recorded, so no header has to justify an
intentional pair.

Three things a later phase should carry forward.

- **The phase's own guidance held: `mid` was the axis to build across.** Phase 1 measured eleven of
  eighteen at-or-below-floor presets under 0.010 on it. Four of these seven now report mid above
  0.030, and Aperture's **0.106** is the strongest mid in `shape_field` by a factor of five.
- **A branching or line figure has too little area for a level term to register**, and this cost
  several rounds. Coral, Rime, Bower and Ridge all measured under or barely at the reactivity
  floor with every band riding the stroke; what fixed each was moving the level response to a
  **whole-frame** stage — `exposure`, or `bg_bright` — and leaving the stroke's own brightness
  nearly flat, which is also what the additive ceiling wants. Ridge went 0.010 → 0.065 on that one
  change.
- **`shot --report`'s `anim` column and the `animation` gate do not agree on these worlds.** Skyline
  reports `anim` 0.002 and Ridge 0.007 against the gate's own 0.01 floor, and both **pass**
  `every_preset_animates_over_time` — as does Coral at 0.012 and Bower at 0.013. The idle motion is
  real and visible (rendered at frames 24 and 48, the bar heights differ plainly); the report's
  column simply measures a narrower thing than the gate does. Worth knowing before someone tunes a
  preset to move that number. **Filed under `## Followups`.**

## Followups (after this lands)

- An ADR adding `preset-author` to the `Owner skill:` vocabulary.
- ~~A `warp_mesh` cohort once [0100](done/0100-the-engine-speaks-milkdrop.md) Phase 1 lands.~~
  ~~**Its condition is met** — 0100 closed 2026-08-16 — so this is no longer a followup but a scope
  question this plan's Phase 1 answers; see the census correction box.~~ **Answered: it is Phase 4b.**
- **`shot --report` cannot see a system with zero worlds**, which is how `warp_mesh` stayed invisible
  to the census that opened this plan. Phase 4b hides the symptom by giving it presets; the
  instrument still prints a section per *populated* family rather than per *registered* system.
  Worth a `dev` plan — the roster is one call to `SystemKind::from_name`'s inverse away.
- **[`docs/capturing.md`](../capturing.md)'s `--report family=<sys>` row lists eight systems**, not
  twelve — `emitter`, `shape_field`, `warp_mesh` and `shape_collage` are missing. Noticed by Phase 1
  and left alone: this plan's files-touched is `presets/*.toml` plus itself.
- Re-run `node scripts/docs-shots.mjs` if any gallery preset is retired — the committed images name
  presets by hand.
- **`shot --report`'s `anim` column disagrees with the `animation` gate**, noticed by Phase 2 and
  left alone. Four presets that pass `every_preset_animates_over_time` report `anim` at or below the
  gate's own `ANIM_FLOOR` — Skyline at 0.002, Ridge at 0.007. The report's figure is a diff between
  two silent captures 0.4 s apart; the gate measures something else, and the column is the number an
  author reaches for first. Either the column should say what it measures or the two should agree.
