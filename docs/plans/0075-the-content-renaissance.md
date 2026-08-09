# 0075 — The content renaissance: the library is rebuilt as worlds, by replacement cohorts

> **Status:** **approved 2026-08-09** — direction decided as
> [ADR-0089](../adrs/0089-the-library-renews-by-replacement-cohorts.md), phase roster
> user-approved the same day. The named guesses (cohort size, cohort count, keep-list
> criteria) stand as defaults; Phase 4's brief is where they get re-decided cheaply if the
> rendered evidence argues otherwise.
> **Created:** 2026-08-09
> **Owner skill(s):** dev, human
> **Related ADRs:** [0089](../adrs/0089-the-library-renews-by-replacement-cohorts.md) (the
> mechanism), [0081](../adrs/0081-the-content-lane-lands-presets-and-architect-curates-the-set.md)
> (the landing route)
> **Serves:** [roadmap-visual-richness R6](../roadmap-visual-richness.md)
> **Closes (Phases 1-3):**
> [backlog 0072](../design-backlog.md#0072--sanityrss-coverage-floor-forces-dense-thin-stroke-line-scenes-into-washed-out-tuning-and-it-is-measuring-the-halo),
> [0067](../design-backlog.md#0067--depth_fade-is-a-uniform-dimmer-on-every-flat-family-where-the-other-two-depth-cues-are-exact-no-ops),
> [0070](../design-backlog.md#0070--the-in-frame-geometry-fraction-cannot-gate-new-content-and-the-number-it-computes-for-every-line-preset-is-not-in-the-authors-report),
> [0061](../design-backlog.md#0061--perspective-moves-the-figure-far-more-than-it-enlarges-it-so-the-documented-way-to-recover-the-framing-does-not-work),
> [0062](../design-backlog.md#0062--depth_hue-is-a-lightness-cue-on-a-lightness-ramp-it-wraps-at-the-ends-and-it-is-structurally-dead-under-ink_amount),
> [0063](../design-backlog.md#0063--spins-usable-ceiling-is-set-by-fade-not-by-taste-and-the-pair-is-undocumented)
> **Gated on (Phases 4+ only):** [0071](0071-light-that-adds-without-covering.md),
> [0064](0064-the-symmetry-stage-and-the-banded-palette.md),
> [0046](0046-transformed-feedback.md), and [0067](0067-the-curation-route.md)'s Phases 1-2
> and 4. **Phases 1-3 are gated by nothing** and can interleave with the roster any time.

## TL;DR

The engine has outgrown the library, and the user has asked for a library that could not have
been written by editing the old one. [ADR-0089](../adrs/0089-the-library-renews-by-replacement-cohorts.md)
sets the mechanism: **replacement cohorts under a fresh-slate rule, never a delete-all reset**.
This plan first fixes the instruments and documentation the new library will be authored
against (the backlog items that would otherwise shape or mislead every new world), then runs
the brief and the cohorts. Each cohort lands a few genuinely new worlds through the
[Plan 0067](0067-the-curation-route.md) route and retires a named list of old presets in the
same series — the set is never hollow, the gates never go vacuous.

## Context & problem

The full analysis is in ADR-0089's Context. In one paragraph: ~55 % of the 41 shipped presets
are template clones; most predate linear light, normalized bands, phrase time, `noise()`,
shaped marks, the IFS family and the depth levers; three shipped files are documented cases of
workarounds outliving the defects they dodged. The roadmap's R6 already promised a renaissance
once the capability waves landed. They now substantially have — what remains in front are the
last three look-surface movers (0071 in progress, 0064, 0046) and the gate-credibility work
(0067).

**The backlog investigation this plan came from** (2026-08-09, full sweep of open entries)
found three kinds of open item bearing on a rebuild:

1. **Instruments that would actively shape new content wrong** — the `sanity` coverage floor
   that selects for washed-out tuning on dense line scenes (0072, "will recur on the next
   mandala"), the animation-gate resolution question (0009, already routed to 0067 Phase 1d),
   and the geometry-fraction number that never reaches the author's report (0070). These are
   Phases 1-2 here, because a gate that rejects the better-looking draft (`emitter_squall`'s
   history) must not be the examiner of a whole new library.
2. **Documentation that points authors the wrong way** — the depth-lever trio (0061: the
   `perspective` orbit and its ~0.3 usable ceiling; 0062: `depth_hue`'s three regimes; 0063:
   the `spin`x`fade` smear ceiling). Phase 3, because these cost the last content pass a
   rendered ladder each, and the renaissance multiplies that cost by every new world.
3. **Capability gaps that are real but must NOT be prerequisites** — the slew-release
   smoothing form (0021, parked "awaiting an author want"), per-mark swarm variation (0068),
   the attractor tuple roster (0055), the LUT-coordinate wrap question (0075 item 2), the
   scalloped-curve primitive (0071), two-tone marks (0069). The cohorts are exactly the
   author-want generator these entries are waiting for; they get promoted through the normal
   loop **when a cohort hits them**, not pre-emptively. Bundling them here is the scope
   gravity the roadmap warns about.

## Decision

Run the renaissance as ADR-0089 specifies: instrument fixes first (dev), then a user-decided
brief from rendered evidence, then replacement cohorts (human, through the content lane),
with engine-gap feedback routed between cohorts. Sequenced last on the active roster, after
0071/0064/0046/0067 — except Phases 1-3, which are independent and can land whenever a
session has room.

## Architecture diagram

```mermaid
flowchart TD
    subgraph pre["prerequisites (other plans)"]
        P71["0071 occlude"] ~~~ P64["0064 symmetry stage"] ~~~ P46["0046 transformed feedback"] ~~~ P67["0067 trustworthy gate + route"]
    end
    subgraph this["this plan"]
        F["Phases 1-3<br/>instruments + docs stop lying"] --> B["Phase 4 brief:<br/>keep list, retirement roster,<br/>cohort briefs (user, from renders)"]
        B --> C1["cohort: author worlds<br/>fresh-slate rule"]
        C1 --> G["gates + 0067 route<br/>land new .toml"]
        G --> R["retire named clones<br/>same series"]
        R --> FB["engine-gap feedback<br/>-> backlog -> architect"]
        FB -->|next cohort| C1
    end
    pre --> B
```

## Implementation phases

### Phase 1 — the sanity floor stops selecting for the defect

- **Owner skill:** dev
- **Area:** core (tests)
- **What:** resolve
  [backlog 0072](../design-backlog.md#0072--sanityrss-coverage-floor-forces-dense-thin-stroke-line-scenes-into-washed-out-tuning-and-it-is-measuring-the-halo):
  at 96x96 the `sanity.rs` coverage statistic cannot see a dense thin-stroke figure (the bare
  rosette and a 46x-denser mandala score identically), so the only lever that clears the floor
  is inflating `glow`/`trails` — the exact look the user rejected. The entry names the two
  candidate mechanisms: a per-family floor that acknowledges what thin strokes render at
  96x96, or a structural occupancy measure (the radial-shell count already prototyped in the
  Plan 0065 lane). Choose at implementation and record why in the test.
- **Files touched:** `core/tests/sanity.rs` (and a helper if the structural measure is chosen).
- **Done when:** the honest tunings that failed the old floor pass the new measure — the three
  retired ring-mandala presets at `glow = 1.0` with no trails are recoverable from git history
  and their failing numbers are pinned in the backlog entry — **and** a scene that renders
  nothing still fails, which is the one job the current floor demonstrably does. No universal
  threshold is invented: whatever constant ships states its derivation next to itself
  (ADR-0071's rule).

### Phase 2 — two author traps close in code

- **Owner skill:** dev
- **Area:** core, standalone
- **What:** the two small code items from the sweep.
  1. **`depth_fade` becomes an exact no-op on flat families**
     ([backlog 0067](../design-backlog.md#0067--depth_fade-is-a-uniform-dimmer-on-every-flat-family-where-the-other-two-depth-cues-are-exact-no-ops),
     its option 2): multiply the fade term by the family's has-depth flag, restoring
     ADR-0076's stated property that all three depth cues are identities at zero depth extent.
     The entry records that no shipped preset binds `depth_fade` on a flat family — verify
     with a grep at implementation; if that has changed, the affected baseline moves are
     listed in the commit.
  2. **The in-frame geometry fraction joins `shot --report`**
     ([backlog 0070](../design-backlog.md#0070--the-in-frame-geometry-fraction-cannot-gate-new-content-and-the-number-it-computes-for-every-line-preset-is-not-in-the-authors-report)):
     the number `LineRenderer::draw` already computes gets a column in the author's own
     metrics table, which is where the over-scale defect class is actually introduced and the
     only place it can be caught for new content (ADR-0083 twice proved no absolute threshold
     exists).
- **Files touched:** `core/src/render/scenes/particles/` (one multiply in WGSL or its uniform
  prep), `standalone/examples/shot.rs`.
- **Done when:** on a `dn ≡ 0` family, captures with `perspective`, `depth_hue` and
  `depth_fade` each set alone are pixel-identical to the unset capture; `shot --report` prints
  the fraction for every line-family preset and omits the column for families that have no
  line seam.

### Phase 3 — the depth-lever docs stop pointing the wrong way

- **Owner skill:** dev
- **Area:** docs
- **What:** land the three measured corrections from the Plan 0063 content pass in the two
  files an author reads first:
  [0061](../design-backlog.md#0061--perspective-moves-the-figure-far-more-than-it-enlarges-it-so-the-documented-way-to-recover-the-framing-does-not-work)
  (`perspective`'s dominant effect is a phase-varying **translation** at ~0.9x the parameter,
  a `zoom` cannot recover it, usable ceiling ~0.3 of the 0.8 legal range),
  [0062](../design-backlog.md#0062--depth_hue-is-a-lightness-cue-on-a-lightness-ramp-it-wraps-at-the-ends-and-it-is-structurally-dead-under-ink_amount)
  (`depth_hue` reads as a hue cue only on a hue-travel ramp; keep it under
  `2 * min(hue_center, 1 - hue_center)` or it wraps; inert under a duotone remap), and
  [0063](../design-backlog.md#0063--spins-usable-ceiling-is-set-by-fade-not-by-taste-and-the-pair-is-undocumented)
  (`spin` and `fade` are one look; the "2-4 is where rotation becomes legible" advice is wrong
  for every attractor preset that ships and goes).
- **Files touched:** `presets/README.md` (depth section), `docs/preset-palettes.md`
  (the `depth_hue` regime note). ADR-0076 itself gets a dated Outcome section — that edit is
  **architect's**, at this plan's close, per the append-only rule.
- **Done when:** each of the three entries' "What a fix would be" documentation items is
  present, with the measured numbers quoted rather than paraphrased; the stale "2-4" sentence
  is gone.

### Phase 4 — the brief: the target library, decided from renders

- **Owner skill:** human
- **What:** the user, with `architect` and the content lane, decides what the renaissance is
  aiming at — from rendered evidence, not file names (the concrete-examples workflow). Inputs:
  a contact sheet of all shipped presets under one real-material stimulus, and the roadmap's
  reference-look table. Outputs, appended to this plan file:
  1. **The keep list** — presets that survive on merit. Candidate criteria (a guess, to be
     re-decided here): authored geometry-first or post-craft-principles, measured distinct,
     and liked in the running app.
  2. **The retirement roster** — everything else, grouped into cohorts, each retirement named
     against the world that replaces it.
  3. **Cohort briefs** — one line per new world naming its reference look and mechanism.
     Guess: **three cohorts of 4-6 worlds**, target library **~18-24 worlds** total.
- **Done when:** the three lists exist in this file and the user has said "go" on cohort one.

### Phase 5 — the cohorts (repeat until the retirement roster is empty)

- **Owner skill:** human
- **What:** per cohort, the content lane authors its worlds under the **fresh-slate rule**
  (ADR-0089: a new world never begins by opening an old preset file), lands them through the
  [Plan 0067](0067-the-curation-route.md) route, and retires that cohort's named presets in
  the same commit series. Between cohorts, every wall the lane hit is handed to `architect`
  as backlog entries — this is the valve through which the parked capability entries (0021,
  0055, 0068, the LUT wrap, the curve primitive) get promoted **on demonstrated want**, each
  through its own ADR/plan, never absorbed into this one.
- **Done when (each cohort):** the new worlds are committed and green through the behavioral
  suite; the cohort's retirements are committed; the full suite runs green after the
  retirements (per-family floors re-derived by their own recorded rule where a family minimum
  moved); the distinctness report has been read and anything it flags is filed, not shipped
  around.
- **Done when (the phase):** the retirement roster from Phase 4 is empty.

### Phase 6 — the set's bookkeeping

- **Owner skill:** dev
- **Area:** docs
- **What:** the operator docs are swept for the new set — `presets/README.md` and
  `docs/preset-palettes.md` (count-free phrasing throughout, per the standing rule),
  `README.md` if the preset count or family roster is named anywhere user-facing, and the
  distinctness family array's comment if any family emptied. The workaround-header sweep
  (0067 Phase 4's grep) runs once over the final set.
- **Done when:** the docs describe the library that ships; no doc names a count that the next
  cohort would re-drift.

## Risks & open questions

- **R3 (layered composition) is now designed —
  [ADR-0090](../adrs/0090-a-preset-composes-two-scene-layers.md) /
  [Plan 0076](0076-the-second-layer.md) (approved 2026-08-09) — and this plan still does not
  hard-gate on it.** The preference is 0076 landing before Phase 4's brief so cohort worlds
  can be layered (the collage look is R3's own acceptance evidence); if it has not landed,
  the brief marks the layered worlds as a later cohort and the renaissance proceeds.
- **Scope gravity** (the roadmap's own warning). Phases 1-3 are deliberately the *only*
  engine work here; every capability the cohorts ask for routes out through the backlog.
  The Mode 4 review should treat any engine code beyond Phases 1-2's named items as drift.
- **The gates still shape content.** Phase 1 removes the worst known instance, and 0067
  Phase 1d measures the other; but a new instrument can select for a new defect. Each
  cohort's done-when includes reading the reports, not just passing them — and a gate fix
  mid-renaissance is a legitimate feedback outcome.
- **The fresh-slate rule is unenforced.** It is stated in ADR-0089, this plan, and belongs in
  the content lane's skill docs (Phase 4's "go" is the moment to add it there). Holding it is
  a per-cohort review duty.
- **In-flight retunes may be partially discarded** (0071 Phase 5, backlog 0038/0058 walk
  presets the roster may retire). Accepted in ADR-0089: those passes keep the shipped app
  good during the transition.
- **Re-check backlog 0055 (attractor variety) before the brief** — its own text says the
  Plan 0059 Phase 4 levers may have closed some of the gap; the brief should know whether an
  attractor world needs the tuple roster or already has its variety.

## What this plan does NOT do

- **Delete the library up front** — ADR-0089's Alternative A, rejected.
- **Build the parked capabilities** (slew-release, swarm twinkle, tuple roster, LUT clamp,
  curve primitive, two-tone marks, R3). Each waits for a cohort to demonstrate the want, then
  goes through its own ADR/plan.
- **Touch the golden fixtures.** ADR-0023 decoupled them from content; retirements and
  landings move no baseline.
- **Change the curation boundary.** ADR-0081 stands; this plan is that route's heaviest user,
  not its editor.

## Followups (after this lands)

- The backlog entries the cohorts generate are the next design queue — by construction the
  highest-signal one this project will have had, since every entry comes from an author
  blocked on a real look.
- If the renaissance ends with a family empty (no world earned its scene), that is a finding
  about the scene, and it goes to `architect` as a scene-retirement question rather than
  quietly shipping a dead code path.
