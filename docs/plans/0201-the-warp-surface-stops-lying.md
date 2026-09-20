# 0201 — The warp surface stops lying

> **Status:** in-progress
> **Created:** 2026-09-19
> **Approved:** 2026-09-19 (user)
> **Owner skill(s):** dev
> **Related ADRs:** [0223](../adrs/0223-the-figure-contract-reaches-a-custom-wave-because-the-source-applies-it-there.md)
> (proposed), [0224](../adrs/0224-level-mode-gets-a-coverage-threshold-and-the-ink-class-stays-this-scenes.md)
> (proposed), [0199](../adrs/0199-a-converted-waveform-draws-the-sources-figure-at-the-hosts-scale.md),
> [0197](../adrs/0197-the-contour-can-be-an-ink-and-the-warp-field-can-be-coloured-by-its-level.md),
> [0212](../adrs/0212-a-converted-preset-gets-its-own-vertex-module-and-the-pipeline-is-chosen-not-branched.md),
> [0170](../adrs/0170-a-parameters-reference-row-is-generated-from-the-declaration-the-engine-reads.md)
> **Closes:** design-backlog 0244, 0245, 0249, 0251

## TL;DR

Four things `warp_mesh` tells an author are wrong or missing: its `zoom` doc says the opposite of
what its shader does and four generated surfaces carry the lie, a custom wave skips the smoothing and
the host factor every built-in figure gets, level mode cannot produce the ink class it was asked
for, and the converted warp space has no pixel baseline because every golden fixture is square. This
plan repairs all four. The first visible behaviour is a parameter reference an author can bind
`zoom` from without getting the direction backwards.

## Context & problem

**The `zoom` doc is inverted.** The `ParamSpec` reads *"above 1 the image tunnels inward"*; the
shader beside it says the reverse in a comment written to explain exactly this trap, and the plan's
own fixture agrees with the shader. The string is declared twice (`PER_VERTEX_PARAMS` and `PARAMS`)
and rendered four times — `presets/README.md`, `presets/schema/warp_mesh.schema.json` twice, and
`docs/specs/player-schema.json` — so every surface this project offers tells an author the wrong
direction. It has already produced a false paragraph in shipped content, convicted at Plan 0184's
close (backlog 0249).

**A custom wave is drawn differently from every other figure.** The eight `wave_mode` figures pass
through `SmoothWave` and carry `HOST_SAMPLE_FACTOR`; `custom_waves` gets neither, which was correctly
scoped by Plan 0180 and left standing as a question (backlog 0244).

**Level mode draws bands, not an ink.** The present writes `ink * coverage` and coverage is a
continuum, so a two-ink palette renders 851 exact colours on the shipped `warp_ladder` at 640x360,
and twelve quantized bands only bring it to 60 (backlog 0251).

**The converted warp space has no baseline.** `golden.rs` renders every fixture at 128x128, and at a
square target MilkDrop's aspect pair is the identity — so the three `warp_mesh` fixtures are
structurally incapable of catching incidental pixel drift in the corrected-space chain, which is the
class a golden exists for and the class ADR-0037 was written about (backlog 0245).

## Decision

The doc repair is mechanical: correct both declarations and regenerate the three artifacts, per
[ADR-0170](../adrs/0170-a-parameters-reference-row-is-generated-from-the-declaration-the-engine-reads.md)
— never a hand edit of a generated file. The figure contract reaches a custom wave per
[ADR-0223](../adrs/0223-the-figure-contract-reaches-a-custom-wave-because-the-source-applies-it-there.md),
because the source applies it there too. Level mode gets a default-off coverage threshold per
[ADR-0224](../adrs/0224-level-mode-gets-a-coverage-threshold-and-the-ink-class-stays-this-scenes.md).
And the converted chain gets **one fixture at a non-square size**, the way `attractor_trails` is
captured at 160x100 with its own baseline: we rejected a second size for the whole golden roster
(it doubles every bless to catch one chain) and rejected closing the gap with reasoning alone (the
per-stage tests assert what the chain computes and cannot see incidental drift, which is the whole
job of a baseline).

## Implementation phases

### Phase 1 — `zoom` says what the shader does
- **Owner skill:** dev
- **What:** correct both `ParamSpec` declarations to the replacement text Plan 0184's close review
  wrote — *"Scale the previous frame is resampled at, per vertex; above 1 the past is magnified and
  the image travels outward"* — and regenerate the parameter reference and the editor schemas with
  `RLX_UPDATE_PARAM_REFERENCE=1` and `RLX_UPDATE_PRESET_SCHEMA=1`. Sweep the shipped presets for any
  header that reasons from the old direction.
- **Files touched:** `core/src/render/scenes/warp_mesh/mod.rs`, `presets/README.md` (generated),
  `presets/schema/warp_mesh.schema.json` (generated), `docs/specs/player-schema.json` (generated),
  any `presets/*.toml` header the sweep convicts
- **Done when:** no surface in the repository says `zoom` above 1 tunnels inward — the two
  declarations, the parameter table, both schema strings and the player schema all say what the
  shader does; `core/tests/suite/preset_schema.rs` is green against the regenerated files, so the
  regeneration was committed rather than the file hand-edited; and the preset sweep's result is named
  in the log, whether or not it convicted anything.

### Phase 2 — A custom wave draws through the contract
- **Owner skill:** dev
- **What:** `custom_waves` passes through the same midpoint insertion the eight built-in figures use
  — except when the wave draws dots, which is the source's own exception — and carries
  `HOST_SAMPLE_FACTOR` on its sample term.
- **Files touched:** `core/src/render/scenes/warp_mesh/draw.rs`,
  `core/src/render/scenes/warp_mesh/tests.rs`
- **Done when:** a custom wave's vertex count after the draw matches what the source's smoothing
  produces for the same input, and a dots wave's does not; the sample term carries the host factor,
  asserted as the ratio between a known input and the drawn coordinate rather than as a frozen pixel
  figure; and the eight built-in figures are unchanged, shown by their existing assertions staying
  green without re-blessing.

### Phase 3 — Level mode can hold an ink
- **Owner skill:** dev
- **What:** a `warp_mesh` parameter thresholds coverage in level mode — at or above it the ink,
  below it the paper — default off, hard edge, declared through `ParamSpec` so ADR-0170's reference
  row and the editor schema are generated from it. `docs/preset-palettes.md` states which limited-ink
  property this gives and which one it does not (ADR-0138's draw-seam guarantee, which does not
  move).
- **Files touched:** `core/src/render/scenes/warp_mesh/mod.rs`,
  `core/src/render/scenes/warp_mesh/shaders.rs`, `presets/README.md` (generated),
  `presets/schema/warp_mesh.schema.json` (generated), `docs/specs/player-schema.json` (generated),
  `docs/preset-palettes.md`
- **Done when:** with the shipped `presets/warp_ladder.toml` at 640x360 loud, a two-ink palette and
  a **banded** coordinate (`palette_steps = "12"`, what that preset ships), the threshold produces
  the **ink class** — paper, plus the two inks each within one encoded level of its stop, and no
  value between them — against the same frame with the threshold off; the default renders that
  preset byte-identically to today, so no golden and no card move; and `docs/preset-palettes.md`
  names the aliasing cost, and that the print needs both levers, rather than leaving an author to
  find either.

  **Amended 2026-09-19, after Phase 3's own measurement convicted the original.** It asked for *two
  exact ink values plus the background at `palette_steps = "0"`*, which no coverage threshold can
  reach: two continua sit downstream of the ink and this phase removes one. The 256-texel LUT is
  sampled **linearly**, so a run boundary is one texel wide whatever the stops say, and an unbanded
  coordinate sweeps it continuously; and the display write dithers by one encoded level (ADR-0096),
  so even a perfect two-ink frame counts more than two exact values. The measured table is in the
  `## Implementation log`: 886 to 802 at `"0"`, and **145 to 11** at `"12"`, where the 11 decomposes
  as paper plus two dithered ink clusters with nothing between. The figures are measurements on the
  development machine, not properties (ADR-0071) — what is asserted above is the class, which holds
  wherever the two levers are both on. ADR-0224's first Positive carried the same overreach and is
  corrected in the same commit.

### Phase 4 — The converted chain gets a fixture that will see it
- **Owner skill:** dev
- **What:** one converted `warp_mesh` fixture rendered at a non-square size, in the shape
  `attractor_trails` already uses — everything about the guard except the baseline image itself,
  which Phase 4b captures.
- **Files touched:** `core/tests/golden.rs` or a sibling module, `docs/testing.md`
- **Done when:** the fixture renders at a size whose aspect is **not** 1:1 and not 16:9 — the two
  shapes ADR-0037 records as the ones that hide this class — and the test's header says why that
  size; `docs/testing.md` records what the new fixture guards that the square ones cannot; and
  **the suite is green at the end of this phase** — the test does not run while its baseline is
  absent and prints why, in ADR-0016's skip shape, rather than failing or asserting against nothing.

### Phase 4a — The fixture is made able to see what it guards

- **Owner skill:** dev
- **What:** Phase 4's fixture renders a picture the aspect correction does not move, so it would
  pass forever. Replace the fixture's subject with a converted preset whose picture the correction
  demonstrably moves — or establish that none does, which is a finding and not a baseline.
- **Files touched:** `core/tests/suite/warp_mesh_wide.rs`, `docs/testing.md`, and the fixture's
  preset if the subject changes.
- **The measurement that convicts the current fixture**, taken 2026-09-19 on the development
  machine at 160x120, each probe applied alone and reverted:

  | probe | what it removes | `warp_mesh_wide` |
  |---|---|---|
  | `mesh.rs` `vertex_position`: drop `* aspect` | the native mesh's x correction | passes |
  | `shaders.rs`: drop `p.x = p.x * aspect` | the native shader's correction | passes |
  | `milk/mod.rs`: force `self.aspect = 1.0` | **the converted chain's own aspect entry** | passes, `mean 0.0000 (tol 0.02) max_outlier 0 (tol 48)` |

  The third is the decisive one: with the correction removed at the converted chain's entry the
  capture is **byte-identical**, not merely inside tolerance. The blessed picture is a smooth
  gradient, and a smooth gradient has no geometry for a geometric correction to move.
- **Done when** the third probe above **fails** the fixture and the three square `warp_mesh`
  fixtures in `golden` still pass unchanged, with both readings recorded; and `docs/testing.md`
  says what the fixture guards in terms of that probe rather than in terms of its size.
- **Stop condition:** if no converted preset the repository ships renders a picture that probe
  moves, **stop and say so** rather than blessing anything. That result would mean the converted
  chain's corrected space does not reach a shipped picture at all, which is a finding about
  backlog 0245's premise and belongs in this plan's log and a backlog entry — not in a baseline
  that cannot fail.

### Phase 4b — The baseline is captured by someone who looks at it
- **Owner skill:** human
- **What:** capture Phase 4's baseline, and confirm the guard actually guards.
- **Files touched:** `core/tests/golden/` (one new baseline), and the line Phase 4 left holding the
  test back.
- **How:** `RLX_BLESS=1` for that test alone, then **open the PNG** — a first baseline is not
  checked by any comparison, only by a person deciding the picture is what the fixture meant to
  render. Then release the test and re-run it green.
- **Done when:** the baseline is committed, the test runs, and a deliberate one-term change in the
  corrected-space arithmetic moves it while the existing square fixtures do not move at all —
  reverted after, with the reading recorded in the `## Implementation log`.

  **Split from Phase 4 on 2026-09-19, and the reason is the decision not to widen the allowlist.**
  `RLX_BLESS=1` is not in `tools/conductor/settings.conductor.json` and is not being added: it
  rewrites **any** baseline that differs, so a session that can create one can also erase the
  evidence that it broke another, and the goldens are this project's only guard against silent
  visual drift. That is ADR-0210's reasoning applied to a second surface — the repair a headless
  session must not make is the owner's. A first baseline has nothing to compare against, so
  capturing it is a look judgement in the sense ADR-0081 uses, not a command.

## Risks & open questions

- **Phases 1 and 3 need an `RLX_UPDATE_*` regeneration**, which a conductor session cannot run
  today (backlog 0250). [Plan 0197](done/0197-the-conductor-becomes-operable.md) Phase 4 makes it
  runnable, so **this plan runs after 0197** or its first phase parks.
- **Phase 2 changes what converted content draws.** No shipped preset is affected — `presets/`
  carries no `[milk]` bundle — so the blast radius is the conversion corpus, which lives outside
  this checkout.
- **Phase 3's threshold aliases by design** (ADR-0224's first Negative). The done-when asserts the
  colour count, not the edge quality; whether the edge is acceptable in motion is a content
  judgement for `preset-author`, and a look that wants it will arrive as a preset rather than as a
  test.
- **Phase 4 adds a baseline, and baselines are re-blessed.** One more fixture in every bless scope,
  which is the cost ADR-0222's sibling question weighs elsewhere and is small here.

## What this plan does NOT do

- It does not move ADR-0138's draw-seam definition of limited ink. ADR-0224 says explicitly that a
  second premultiplied-light scene asking the same question is the trigger for that, not a third copy
  of this parameter.
- It does not re-bless the three square `warp_mesh` golden fixtures. They keep guarding what they
  guard; Phase 4 adds, it does not replace.
- It does not take backlog 0248 (whether a groundless luminous field is a composition or a fill),
  which is a curation question for the content lane rather than an engine gap.

## Implementation log

**Lane:** `WORK/rlx-plan-0201` on `plan-0201-the-warp-surface-stops-lying`

| phase | owner | state | commit |
|---|---|---|---|
| 1 — `zoom` says what the shader does | dev | done | f14bcd47 |
| 2 — A custom wave draws through the contract | dev | done | 48120eee |
| 3 — Level mode can hold an ink | dev | done | c832e76c |
| 4 — The converted chain gets a fixture that will see it | dev | done | 4c7f739b |
| 4a — The fixture is made able to see what it guards | dev | done | committed with this row |
| 4b — The baseline is captured by someone who looks at it | human | done | committed with this row |

### Notes

- **Phase 4b — the baseline was opened before the test was released, and by whom.** Captured with
  `RLX_BLESS=1 cargo test -p rlx-core --test suite warp_mesh_wide::` and judged in an
  owner-directed session rather than by the owner, who asked for the phase to be settled on their
  behalf. `Geiss - Fog Tunnel` at 160x120 renders a bright core with radial streaks over a dark
  field, the subject the source names: the waveform's light accumulated through an exponential zoom
  (`zoom` 1.042, `zoomexp` 3.4) with `decay` 0.98 and the 1.289 echo, greyscale as the palette and
  `wave_r/g/b` 0.65 declare, nothing clipped and no empty frame. Measured on the committed PNG, the
  core's bounding box is 90x78 px above half luminance-peak (`w/h` 1.154) and 71x60 above 0.9
  (1.183) — **not** round, and correctly so: MilkDrop's waveform is drawn in the host's normalized
  square and stretches with the window, while the aspect pair corrects what the *warp* does. A
  circular core here would have been the finding.
- **Phase 4b — the one-term experiment, and the term the done-when named is the wrong one.**
  Dropping `* aspect` from `mesh::vertex_position` leaves this fixture **byte-identical** — `mean
  0.0000 / outlier 0` — because that function serves the native `warp_mesh` path and the converted
  chain corrects at its own entry. The term that moves it is the one the fixture is documented
  against: forcing `self.aspect = 1.0` in `milk::run_frame` fails it at **mean 0.0139 (tol 0.02) /
  outlier 75 (tol 48)**, now against a committed baseline rather than Phase 4a's scratch capture.
  Under that same probe `cargo test -p rlx-core --test golden` is 3 passed with
  `warp_mesh 0.0002/2`, `warp_mesh_milk 0.0000/0`, `warp_mesh_shader 0.0000/1`,
  `warp_mesh_stroke 0.0000/0` — **the same four readings as with the probe reverted**, so the square
  fixtures do not move at all. Both probes reverted; the tree carries neither.
- **Phase 4a — the subject changed, and the probe chose it.** The fixture now captures
  `milk_wash_fog_tunnel.toml`. Four candidates were measured at 160x120 with `self.aspect = 1.0`
  forced at the converted chain's entry, each probe applied alone and reverted: `milk_wash_fog_tunnel`
  **mean 0.0139 / outlier 75** against tolerances of 0.02 and 48, which fails the fixture as the
  phase asks; `warp_mesh_milk` 0.0057 / 21, which moves but stays inside tolerance; `warp_mesh_stroke`
  and `milk_wash_blur_mix_3` both 0.0000 / 0. All four declare `zoom`, `rot` or `warp`. Under the same
  probe `cargo test -p rlx-core --test golden` is **3 passed**, unchanged, which is the second half of
  the done-when.
- **`warp_mesh_shader.toml` was not edited.** It carries `GOLDEN FIXTURE - do not tune` and
  ADR-0023, and `golden.rs` and `warp_mesh.rs` both read it; the subject moved to another fixture
  instead. Its own probe reading is 0.0000 / 0 — it declares no mesh motion, so the corrected
  coordinates reach nothing it draws.
- **The blessed `warp_mesh_wide.png` from the earlier attempt was deleted**, not replaced: its
  subject no longer exists in this fixture. The baseline is Phase 4b's and stays absent until then,
  with the drift half skipping under ADR-0016.

- **Phase 1's preset sweep convicted nothing.** Five shipped `warp_mesh` presets bind `zoom`, and
  every header already reasons from the shader's direction: `warp_wellhead` (*"Above 1 expands"*),
  `warp_millrace` (*"just under 1, so the field creeps inward"*), `warp_ladder` (which spells the
  inverse out in full), `warp_tracery` (0.988, *"creeps toward the pivot"*) and `warp_sirocco` (held
  at 1). No `presets/*.toml` was touched.
- **Phase 1 also regenerated `presets/preset.schema.json`**, which the phase's `Files touched` does
  not name. It carries the same `zoom` string twice and is written by the same
  `RLX_UPDATE_PRESET_SCHEMA=1` run as `presets/schema/warp_mesh.schema.json`; leaving it out would
  have left the generic editor schema saying the old direction and
  `preset_schema::the_generated_editor_files_are_current` red.
- **Phase 2 also edited `docs/milkdrop-conversion.md`**, which the phase's `Files touched` does not
  name. Its "what a conversion does and does not carry" table listed *a custom wave's `SmoothWave`
  pass* as **not carried**, which the phase makes false; the row moved to the carried column with
  ADR-0223 cited. The two custom-wave tests are in `core/src/render/scenes/warp_mesh/tests.rs` as
  the phase names, built from a bundle written in the VM's own assembly, so `core` needs no EEL2
  compiler to carry a custom wave.
- **Phase 3's code landed in `c832e76c`, before the plan was amended; this row is its own
  `docs(plans)` commit.** The phase's original first done-when convicted itself under measurement and
  the phase stopped there; the amendment above restated it, and the measurement below is that
  restated done-when re-run against `c832e76c`'s tree.
- **Phase 3's measurement.** The `warp_ladder` frame at 640x360, `--set bass=1,mid=1,treb=1`, 120
  frames, counting exact frame colours:

  | palette | `palette_steps` | threshold off | threshold on |
  |---|---|---|---|
  | the shipped one (runs 0.02 apart) | `"0"` | 886 | 802 |
  | runs that jump (`0.1599` -> `0.1601`) | `"0"` | 625 | 517 |
  | the shipped one, shipped banding | `"12"` | 145 | **11** |

  The 11 decompose as `#000000` paper over 25.6% of the frame, a dark ink of `#0c0c0c`/`#0d0d0d`/
  `#0e0e0e`, a light ink of seven values every one of which is within one encoded level of `#e4e2de`
  per channel, and nothing between the two inks. The 145 it replaces run continuously from the paper
  up through `#0c0c0c`.
- **Phase 3 finding: the ink an author reads in the frame is not the hex the stop declares.** The two
  stops are `#111010` and `#f4f2ee`; they land at `#0d0d0d` and `#e4e2de`. So *"within one encoded
  level of its stop"* is satisfied against the ink the stop renders as, not against the declared
  literal — the light path between them is a one-to-one remap, the kind `docs/preset-palettes.md`
  already distinguishes from a mixer. Nothing in the frame is between the two inks either way, which
  is the class the done-when names.
- **Phase 3's scratch material is in the lane's `target/plan0201/`**, uncommitted and gitignored: the
  five fixtures, the PNG colour counter and the decomposer that produced both readings above.
- **Phase 3 regenerated `presets/preset.schema.json` as well**, for the reason Phase 1's note above
  records: the same `RLX_UPDATE_PRESET_SCHEMA=1` run writes it, and a `warp_mesh` parameter reaches
  the generic editor schema too.
- **Phase 4 landed the guard and not the picture, which is the shape the split gives it.**
  `core/tests/suite/warp_mesh_wide.rs` captures `core/tests/fixtures/warp_mesh_shader.toml`
  unmodified at **160x120**, with `golden.rs`'s `FRAMES = 60` and both of its tolerances, so the
  square capture there and this one differ in the target size and in nothing else. Its drift test
  skips in ADR-0016's shape while `core/tests/golden/warp_mesh_wide.png` is absent; its second test
  needs no GPU, holds the capture size off 1:1 and off 16:9, and runs meanwhile.
- **Phase 4 also edited `core/tests/suite/main.rs`**, which its `Files touched` does not name: a
  sibling module needs its `mod` line, and the phase reads *`core/tests/golden.rs` or a sibling
  module*.
- **What Phase 4b touches besides the baseline**, named here so its holder need not find them: the
  skip block in `warp_mesh_wide.rs`, marked in the file as the block to delete — after which the
  `path.exists()` assertion under it is the live guard, the one every other baseline carries — and
  the closing sentence of `docs/testing.md`'s `warp_mesh_wide` row, which says the first baseline is
  a person's. The one-term experiment that done-when asks for is dropping `* aspect` from
  `mesh::vertex_position`: the identity at 1:1, not at 4:3.

### Close triggers

- **`presets/` touched:** yes, and only files that are generated — `presets/README.md`,
  `presets/schema/warp_mesh.schema.json` and `presets/preset.schema.json`, all written by
  `RLX_UPDATE_PARAM_REFERENCE=1` / `RLX_UPDATE_PRESET_SCHEMA=1` in Phases 1 and 3. No `presets/*.toml`
  was edited, and no preset was added or removed.
- **Plan header `Closes:`** design-backlog 0244, 0245, 0249, 0251. 0245's fixture is committed and its
  baseline is Phase 4b's, so that chain has a guard that does not yet compare anything.
- **What shipped:** a feature — a new `warp_mesh` parameter (the level-mode coverage threshold,
  default off), a change to what a converted custom wave draws (midpoint smoothing plus the host
  sample factor, dots excepted), the corrected `zoom` direction in both `ParamSpec` declarations and
  in every generated surface that renders them, and a golden fixture at 160x120.
- **Operator docs touched:** `docs/preset-palettes.md` (the threshold's limited-ink property and its
  aliasing cost), `docs/milkdrop-conversion.md` (the custom-wave row moved to the carried column),
  `docs/testing.md` (the new fixture's row and its bless scope), plus the generated
  `presets/README.md` and `docs/specs/player-schema.json`.
- **Backlog probes (`node scripts/check-backlog-claims.mjs`):** exit 0 — *59 stated reductions still
  hold across all 26 live entries (4 unprobeable)*, with 30 advisory moved-path rows.
- **Full suite:** owed to the conductor's pre-review gate (ADR-0207). The per-phase gate run on this
  tree was `cargo nextest run --workspace -P fast`: 1725 passed, 0 failed, 311 skipped, in 459.5 s.
- **Outstanding `human` phases:** none. Phase 4b was settled in an owner-directed session on
  2026-09-20 — the baseline is committed, the skip block is gone, and both readings the phase asks
  for are in the Notes above. Every phase is committed.

## Followups (after this lands)

- A `preset-author` look that uses the coverage threshold, which is what will say whether the hard
  edge is usable in motion.
