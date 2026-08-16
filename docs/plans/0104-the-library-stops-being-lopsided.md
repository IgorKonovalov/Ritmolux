# 0104 — The library stops being lopsided

> **Status:** approved
> **Created:** 2026-08-16
> **Approved:** 2026-08-16 (user)
> **Owner skill(s):** dev, human (every `human` phase is a `preset-author` session — see Risks)
> **Related ADRs:** [0081](../adrs/0081-the-content-lane-lands-presets-and-architect-curates-the-set.md) (the content lane lands presets), [0089](../adrs/0089-the-library-renews-by-replacement-cohorts.md) (renewal by replacement cohorts)

## TL;DR

The shipped library is **39 presets across 10 systems, and it is wildly uneven**: `attractor` has
17 worlds while `lsystem`, `shape_field`, `spectrum` and `star_pattern` have **exactly one each**.
A visitor who tries this app judges the content, not the composite, and four of ten systems
currently present themselves with a single example. This plan brings every system to **at least
four distinct worlds** — 18 new presets, taking the library to 57 — and starts by asking whether
the 17-strong family is seventeen worlds or seventeen variations.

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
Arithmetically that is `lsystem +3, shape_field +3, spectrum +3, star_pattern +3, parametric_curve
+2, swarm +2, emitter +1, reaction_diffusion +1` = **18 new presets, 39 → 57**.

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

### Phase 2 — the singletons get a range

- **Owner skill:** human *(a `preset-author` session)*
- **What:** `lsystem`, `shape_field` and `spectrum` each go from one world to four.
- **Files touched:** `presets/*.toml`.
- **Notes for the author:**
  - **`star_pattern` is deliberately not in this phase.** [Plan 0087](0087-the-line-renderer-draws-a-curve.md)
    changes what the curve family draws and has already retired three mandala presets on
    [ADR-0098](../adrs/0098-the-line-renderer-draws-arcs-as-per-pixel-distance-fields.md); a cohort
    authored now would be authored against a surface about to move. It gets its own phase, after.
  - `shape_field` shipped with **no world of its own** by design, and
    [Plan 0098](0098-the-figure-nests-properly.md) is live on `shape_field.rs` — coordinate, or
    take this after it closes.
  - The reference surfaces are [`presets/README.md`](../../presets/README.md) (every parameter),
    [`docs/presets.md`](../presets.md) (the grammar) and
    [`docs/preset-palettes.md`](../preset-palettes.md) (colour). Note that `docs/presets.md`'s
    `system =` table is **missing `shape_field`** as of this writing — a Plan 0091 close swept two
    of the three reference docs, not three. Fix it while you are in there.
- **Done when:** each of the three systems has four worlds; every new preset clears `sanity`,
  `animation`, `reactivity` and the beat gate; and the advisory distinctness report flags **no pair
  inside a family** — or, where it does, that preset's header names why the pair is intentionally
  close. The report stays advisory ([ADR-0067](../adrs/0067-coverage-measures-the-scene-not-the-backdrop.md)
  and the gate table in [`docs/capturing.md`](../capturing.md)); this phase does not promote it to a
  gate.

### Phase 3 — the thin families fill out

- **Owner skill:** human *(a `preset-author` session)*
- **What:** `parametric_curve` +2, `swarm` +2, `emitter` +1, `reaction_diffusion` +1.
- **Files touched:** `presets/*.toml`.
- **Notes for the author:** `swarm` is the weakest picture in the committed gallery — it reads as
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
- **What:** `star_pattern` from one world to four.
- **Files touched:** `presets/*.toml`.
- **Notes for the author:** **takeable only once [0087](0087-the-line-renderer-draws-a-curve.md)
  has landed or been routed to ADR-0098's Alternative C.** Either outcome settles what the family
  draws; authoring before then buys a cohort that has to be redone. If 0087 ends at Alternative C,
  the beading in `docs/images/gallery/star_pattern.png` is what the family looks like, and the
  worlds are authored to suit it rather than around it.
- **Done when:** four worlds, same gates.

### Phase 5 — curate the set, not the presets

- **Owner skill:** dev
- **What:** Re-run the census and the near-duplicate sweep over the whole 57, and sweep the preset
  headers for stale workarounds.
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
  nine samplers — and names the worlds that should be retired. Retirement is a content-lane action
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
- **Two phases are blocked on live plans** — Phase 2 partly on [0098](0098-the-figure-nests-properly.md)
  (`shape_field`), Phase 4 wholly on [0087](0087-the-line-renderer-draws-a-curve.md). Both are
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
- **It does not cover `warp_mesh`** ([Plan 0100](0100-the-engine-speaks-milkdrop.md)), which does
  not exist yet. When it does, the per-system floor applies to it too.

## Followups (after this lands)

- An ADR adding `preset-author` to the `Owner skill:` vocabulary.
- A `warp_mesh` cohort once [0100](0100-the-engine-speaks-milkdrop.md) Phase 1 lands.
- Re-run `node scripts/docs-shots.mjs` if any gallery preset is retired — the committed images name
  presets by hand.
