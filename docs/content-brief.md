# Content-lane brief — the standing sittings

> **Maintained by:** `architect`. **Read by:** `preset-author`.
> **Last consolidated:** 2026-08-13, from five `human` phases spread across five closed plans.

This is the **one** copy. [`docs/plans/README.md`](plans/README.md)'s `Standing` section points here
rather than restating, because a duty recorded in two places drifts in one of them — which is the
failure this file exists to stop.

Every item below is a `human` phase of a plan that is otherwise **`done`**. None was missed; each was
deliberately left for the content lane, and each carries the riders its plan attached. A plan's own
file is still the authority on *why* — this brief is the authority on *what is outstanding and in
what order*.

**How an item leaves this file:** the lane does the sitting, lands whatever it lands through the
[Plan 0067 curation route](plans/done/0067-the-curation-route.md) (ADR-0081 — the lane commits
presets directly, gated on the behavioral suite), records the verdict where the item says to, and the
row moves to `Done` at the bottom with a date.

---

## 1. The sky family — three items, one sitting

**This is three standing phases on one family of looks, and walking the family once is the point.**
The engine side is finished and will not move underneath the work: ground
([Plan 0080](plans/done/0080-the-sky-gets-a-horizon.md)), dither
([Plan 0082](plans/done/0082-the-gradient-stops-banding.md)) and band
([Plan 0081](plans/done/0081-the-sky-gets-a-galaxy.md)) all shipped inside two days.

### 1a. Perseids' quiet sky — [Plan 0077](plans/done/0077-the-quiet-sky.md) Phase 5

Author the quiet twinkling starfield that Plan 0075 cohort 4 **routed out rather than shipped**:
sparse marks, low coverage, slow shimmer on swarm `twinkle`. Both walls it hit are now down — the
animation gate scores `metrics::footprint_diff` over the figure's own footprint (ADR-0091), and the
swarm individuates (`twinkle` / `size_spread` off the particle index).

Two riders from the plan:

- **The sanity floor is read, not fought.** If it prices the sky out, re-derive it by the floor's own
  recorded rule — the [backlog 0072](design-backlog.md) precedent — and never lower it to fit.
- **If the world binds `reseed` or any sustained force it owes one minutes-horizon soak
  observation**, verdict in the world's header. That is [backlog 0086](design-backlog.md)'s bounded
  check. **The instrument for it is [Plan 0085](plans/0085-the-show-length-horizon-gets-an-instrument.md)
  Phase 1** — if that plan has landed, use it; if not, the bounded observation stands as written.

### 1b. The dusk ground — [Plan 0080](plans/done/0080-the-sky-gets-a-horizon.md) Phase 7, content half

The judgement half is **answered and discharged** — the ramp reads as light, the horizon sits where
the `[palette]` stops put it, and the banding it exposed became Plan 0082. What remains is authoring
the dusk world onto the shipped ramp (`bg_angle`, `bg_hue_span`, `bg_shade`/`bg_shade_end`,
`bg_ramp_gamma`).

### 1c. The galaxy, judged against the reference — [Plan 0081](plans/done/0081-the-sky-gets-a-galaxy.md) Phase 6

Three questions no instrument in this repo can answer, and the first carries the decision:

- **Does it read as a galaxy, or as a smudge?** ADR-0095 rejected fbm mottling on the bet that the
  scattered starfield drawn *in front* supplies the texture the smooth band lacks. **If it reads as an
  airbrushed streak that is a result, not a failure** — the answer is Alternative A (fbm), with this
  observation as its evidence, and it gets its own ADR and plan rather than a patch. Nothing forecloses
  it: the envelope is a single multiply, so noise multiplies in later.
- **Does the arc's curvature read at a normal field of view**, or must `bg_band_curve` be pushed until
  the ends leave the frame?
- **Does it band under two overlapping gradients?** Run this on the kept banding reference frame,
  `core/tests/fixtures/scratch-0082/dusk_ground_banding.toml`, at **1920x1080** — plateau width is in
  pixels, so the resolution is part of the measurement. Add `bg_band_amount` to the frame. The first
  of that frame's two owed checks is discharged (58 px → 20 px, 7.5 → 2.1 px/level); **this is the
  second, and it is due rather than pending.**

### What to start from, across all three

- The backdrop earns a preset **nothing** at `sanity` or `animation` — both are blind to `bg_*` by
  design (ADR-0067, ADR-0091's Outcome) — so however much of the frame the sky fills, **the figure
  carries both floors**.
- The look wants `bg_bright = 0` with the band alone, which the widened build condition now supports
  and no earlier configuration could reach.
- Two stale headers belong to this sitting: `fragment_vitrail.toml` still explains its onset-`flash`
  binding by *"the report is bloom-blind"* (fixed by Plan 0077 Phase 4 — the binding may stay for its
  look, but the reason is gone), and `emitter_perseids.toml`'s header records the routed-out quiet sky
  1a exists to ship.

---

## 2. The ink worlds re-judge on `ink_gamma` — [Plan 0078](plans/done/0078-the-ink-learns-to-bite.md) Phase 3

Small: two headers, both **predicted at Plan 0075's close as workarounds that would go stale the
moment this lever landed.** It has landed — `presets/README.md`'s ink section carries the three-lever
note (`ink_gamma` × `ink_amount` × `exposure`) and the measured mean-byte ladder.

- **`reaction_etching.toml`** — its duotone is painted into `[palette]` because *"the ink remap gives
  a mid-contrast field no contrast lever of its own."* Note the world has since been **inverted to
  scratchboard** (bright line work on black), so whether the palette version still earns its place is
  a live judgement, **not a foregone retune**.
- **`swarm_shatter.toml`** — its light-ground twin was routed out with *"when ink grows a contrast
  control, the light-ground twin becomes authorable."* That condition is met.

**Output per world:** a verdict in its header, judged in motion — retune onto `ink_gamma`, or a
recorded *"the palette version stays on its looks."*

**One rider from the close:** `dev`'s eyeball on `attractor_ink` found that **which way to take the
exponent depends on how dense the drawing already is** — a sparse figure's bite likely lives *below*
1, not above. If a world needs a toe **and** a shoulder, that is
[ADR-0092](adrs/0092-the-ink-remap-gains-a-contrast-exponent.md)'s named negative: file it **with the
measurement** as a new backlog entry rather than reopening 0084.

---

## 3. The attractor binds `tuple` — [Plan 0079](plans/done/0079-the-attractor-learns-new-figures.md)'s Followup

A different family and a different sitting from §1 — do not bundle it. Eleven presets landed *with*
the capability: four `attractor_*gallery` demonstrations, the pinned `attractor_torusknot`,
`attractor_valentine`, and four walk worlds.

**The question this pass should answer first is a curation question, not a tuning one.** The attractor
family is now **17 of 37 presets — 46 % of the library on one system**, the sharpest single-family
convergence the set has seen. Three things measured at the close rather than guessed:

- `attractor_dejonggallery` and `attractor_cliffordgallery` are **near-twins by construction** —
  identical `tuple` / `brightness` / `fade` / `reseed`, differing only in family and palette.
- All four galleries step on a **wall clock** (`mod(floor(time * 0.33), N)`) with audio only on
  secondary levers, which makes them demonstrations of the roster rather than worlds.
- So: **do the galleries earn standing places, or were they scaffolding for a `human` gate?** That is
  the lane's call, and it is the first thing to decide because it changes how much new attractor
  content the set can absorb.

**An entry's index is a preset-visible name** — the galleries step them, `attractor_torusknot` pins
Lorenz entry 1 — so the roster table is append-only in practice. Two roster facts a still cannot show,
both already in `presets/README.md`: the Lorenz torus knot **blooms slowly on a `reseed`** (a wide
excursion to ~2.2x its own extent, seconds to fall back, where the canonical butterfly absorbs the
same kick in a handful of frames), and Thomas past `a ≈ 0.208` closes into periodic orbits that have a
perfectly good bounding box and draw as a few dots.

**Twelve morph filmstrips are rendered but unjudged** and are recorded as such. They cost a viewing
rather than a re-render only while `target/tuple-paths/` survives; `node scripts/tuple-paths.mjs`
regenerates them.

---

## 4. The `occlude` retune, with [backlog 0038](design-backlog.md) — [Plan 0071](plans/done/0071-light-that-adds-without-covering.md) Phase 5

Library-wide, so it goes **last**: it is a walk over the whole shipped set, and doing it before §1–§3
means walking the worlds they touch twice.

Two retunes of the same shipped set against a composite that moved underneath it, judged in motion
over a lit backdrop — **one pass, not two.**

- **`occlude`** — raise the floors that were floored for the black rim, now that the ceiling above
  them is adjustable. Two things the close measured to start from: **no shipped preset binds
  `occlude` today**, and at shipped brightnesses the default's effect is almost negligible — the
  ceiling binds where the figure is **dim**, so the worlds worth walking first are the ones with a
  dimming depth cue (`swarm_storm`'s `depth_fade`, `lsystem_fern`'s `glow`-dimmed outer stems; that
  file's header says so in place).
- **[backlog 0038](design-backlog.md)** — mid-tone-dominated presets pay the tonemap knee. Measured:
  `attractor_clifford` mean luma 82.54 → 75.91, **−8.0 %**, while `attractor_leviathan` gained 5.8 %
  because it has genuinely over-range cores. The lever is one line: `exposure` (default 1.0) is a
  linear multiplier ahead of the tonemap, added for exactly this. The population to check is presets
  with **no over-range peak** — the attractor family, the softer `fragment_*`, `swarm_drift`.

**Two record corrections that matter to whoever runs this:**

- The plan's own text says to run this "with 0038 and 0058". **0058 closed by content on 2026-08-04**
  (`859ec66`, all thirteen fold-binding presets now name a `kaleido_edge`), five days before Plan 0071
  reached Phase 5. The three-way pass is a two-way pass.
- `docs/plans/README.md` states that "the *tonemap-knee* half of that pairing is now measured away" by
  Plan 0080 Phase 7. **It is not.** What that phase retired is a different suspicion — that
  `bg_bright = 0.85` was reaching the tonemap's shoulder on the *backdrop ramp* (0 % of the column
  rail-pinned on any channel). Backlog 0038's finding is about **mid-tone figure luminance on
  attractor presets**, which no backdrop measurement speaks to. **0038 is live**, and only one shipped
  preset binds `exposure` today (`lsystem_vellum.toml:60`).

---

## Done

*(Nothing yet — this file was consolidated 2026-08-13. Move a row here with its date and a
one-line verdict when its sitting is finished.)*
