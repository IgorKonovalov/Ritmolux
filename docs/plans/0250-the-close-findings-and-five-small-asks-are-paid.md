# 0250 — The close findings and five small asks are paid

> **Status:** approved (2026-10-06)
> **Created:** 2026-10-06
> **Owner skill(s):** dev, studio-builder
> **Closes:** design-backlog 0260, 0275, 0282, 0283, 0284
> **Related ADRs:** ADR-0020 (an unknown name is a warning), ADR-0018 (field-space view transform),
> ADR-0216 (closing a finding), ADR-0242 (baselines on lavapipe), ADR-0260 (the shared camera)

## TL;DR

This plan fixes nine open conductor findings from the 0237-0248 merges and five mechanical backlog
entries. None of them needs a design decision. The fixes that users notice: a misspelled table such
as `[smothing]` now produces a warning that `--strict` fails on. The `zoom` reference stops saying
the opposite of what two field scenes do. A Hyprland rule can target the show and the console by app
id. A new release re-renders the browser's thumbnails. Lorenz Knot stops putting the camera nearer
than the near-plane test proves safe. The rest are test holes the close reviews named, two
studio-render leaks, and two stale gallery cards.

## Context & problem

The conductor digest of 2026-10-06 carries 14 open findings across 11 merges, and the backlog
carries 30 entries. The owner asked for one plan that pays the actionable findings and only the
backlog entries that need no decision. Entries 0261, 0262, 0276, 0277, 0274 and 0278 each want a
call from `architect` first, so they stay live.

**Finding dispositions, decided at the interview:**

| Plan | Finding | Disposition |
|---|---|---|
| 0202, 0218, 0236, 0248 | The implementation log outweighs the phases | `--wontfix` now: each close review recorded it as warranted |
| 0249 | `bless.yml`'s compare run has no `-E` filter | `--wontfix` now: `8c2ec152` retired `bless.yml` when the baselines moved to lavapipe |
| 0237 | The hue divisor in endless mode | Phase 2 |
| 0237 | Every lsystem scene builds a `seg3d` renderer | Phase 2 |
| 0240 | Lorenz Knot reaches `distance` about 1.43, below 1.5 | Phase 2 |
| 0248 | The depth-colour test recomputes its own depths | Phase 2 |
| 0238 | The sanity gate never renders a waterfall frame | Phase 3 |
| 0242 | Nothing tests that `cmdReadiness` prints advisories | Phase 4 |
| 0239 | The Braid and Maelstrom gallery cards show the 2.5D look | Phase 4 |
| 0247 | Quitting during a transcode orphans ffmpeg | Phase 5 |
| 0247 | A refused neural Start leaves `<output>.bars.json` behind | Phase 5 |

**`main`'s red CI is not a phase.** Run `37500479346` failed at `ffd0b34`, which is before
`8c2ec152`. It failed on WARP golden drift: `waterfall`, `swarm`, `attractor_depth`, the two
parametric 3D baselines and the three `layer_*` baselines. `8c2ec152` moved the baselines to
lavapipe and made `golden` print and skip on WARP, so the first CI reading that can show whether
that cured it is run `37512244863` at `5b7a76e`. If that run is red, the fix is an amendment to this
plan, not a guess written into it now.

## Decision

This is one `dev` run followed by one `studio-builder` phase. Each phase groups findings by the code
they touch, so no file is opened by two phases. Every fix takes the shape its finding or backlog
entry already proposed. The two places where an entry left a choice open are settled here:

- **0284:** two app ids, `ritmolux` for the show and `ritmolux-console` for the console. With
  separate ids, a rule can put the two windows on different monitors.
- **0240:** the preset is fixed, not the engine. The shared camera's `range` is documentary and the
  engine clamps nothing to it. A runtime clamp would change every 3D system to cure one preset.

We rejected bundling the decision-shaped backlog entries, because each would make an implementer
choose a design. We rejected trimming the four long implementation logs, because every close review
had already judged the length warranted.

## Implementation phases

### Phase 1 — The loader and the reference stop misleading an author
- **Owner skill:** dev
- **What:** Backlog 0282: `RawPreset` gains a `#[serde(flatten)]` catch-all map. Every top-level key
  it collects becomes a load *warning* (ADR-0020's severity), `unknown top-level table '[smothing]'`,
  with a nearest-known-name hint when one is within edit distance 2. The `[layer]` sub-preset's raw
  table gets the same treatment. Backlog 0283: `common.rs` gains a second constructor,
  `field_zoom`, whose doc reads "Scales the sampled window of the field; above 1 shows more of the
  field, each feature smaller". `fragment_field` and `reaction_diffusion` declare it, and the
  generated parameter reference, the per-system schemas and `.taplo.toml` are regenerated. The
  shared `zoom` doc drops "One meaning across every scene that has it".
- **Files touched:** `core/src/preset/schema/raw/preset.rs`, the loader site that collects
  warnings (under `core/src/preset/schema/`), `core/src/render/scenes/common.rs`,
  `core/src/render/scenes/fragment_field.rs`, `core/src/render/scenes/reaction_diffusion.rs`,
  `presets/README.md` (generated block), `presets/schema/*.json`, `.taplo.toml`, a test in
  `core/tests/suite/preset.rs` or the schema module's tests, `docs/configuration.md` (the `--strict`
  paragraph names the new warning class).
- **Done when:** A test loads a preset holding `[smothing] hue = 1` and gets exactly one warning
  that names `smothing` and suggests `smoothing`. The same preset with `[smoothing]` loads with no
  warning. A stray scalar `sytem = "x"` warns too, and the preset still loads.
  `cargo run -q -p standalone --bin ritmolux -- --check --strict presets` exits 0 over the shipped
  set, so no shipped preset carries an unknown table. `git grep -n "field_zoom" -- core/src` shows
  the constructor declared in `common.rs` and used in exactly the two field scenes.
  `cargo nextest run -p rlx-core --test suite preset_schema::` and the parameter-reference test pass
  without either `RLX_UPDATE_*` variable set.

### Phase 2 — The 3D findings: Lorenz Knot, the depth test, the L-system's renderer and hue
- **Owner skill:** dev
- **What:** Four fixes:
  - **0240.** `presets/attractor_lorenzknot.toml`'s `distance` clamp drops from `0.28` to `0.24`, so
    the nearest it binds is `1 / 0.66 = 1.515`, above the declared `1.5`. `fov` is left alone.
    `presets/README.md`'s sentence that shipped presets run "from about `1.4`" is corrected to the
    real minimum.
  - **0248.** `a_full_hue_axis_colours_a_knot_by_depth_alone` asserts on the depth values
    `render_space` actually hands to `space_coordinate`, through a seam the test can read. A
    swapped or unclipped depth inside `render_space` must turn the test red.
  - **0237, renderer.** `LSystemScene` builds its `lines3d` renderer lazily, on the first configure
    that selects the space turtle, so a flat L-system allocates no `seg3d` instance buffer.
  - **0237, hue.** In endless mode the colour ramp's divisor is the deepest nesting the stream has
    reached so far, not `bracket_bound`'s capacity. It is a running maximum that never shrinks, with
    a floor of 1, so `hue_spread = 1` spans the palette on a vine and colours do not shimmer as the
    ring turns over.
- **Files touched:** `presets/attractor_lorenzknot.toml`, `presets/README.md` (the hand-written
  camera paragraph, not the generated block), `core/src/render/scenes/lines/parametric.rs`,
  `core/src/render/scenes/lines/lsystem.rs`, `core/src/render/scenes/lines/turtle.rs` if the depth
  reading lives on the pen, `core/tests/golden/*.png` only for a baseline the hue change moves.
- **Done when:** `git grep -n "clamp(bass \* 0.33, 0, 0.24)" -- presets/attractor_lorenzknot.toml`
  matches the `distance` line. Temporarily swapping the two depth endpoints inside `render_space`
  turns the depth test red; the implementer checks this once and reverts it. A test builds a flat
  L-system scene and shows no `seg3d` renderer exists, then configures a space preset and shows one
  does. A test grows an endless vine whose reached depth is well under its `bracket_bound` and shows
  the ramp's divisor equals the reached depth and never decreases across a ring turnover. Any golden
  that moves is re-blessed by name, never with `RLX_BLESS=1` over the whole roster, and each one is
  listed in the implementation log.

### Phase 3 — The two gates that cannot see what shipped
- **Owner skill:** dev
- **What:** Two test fixes:
  - **0238.** The sanity gate's `draws_a_real_shape` covers `SystemKind::Waterfall` through a test
    fixture under `core/tests/fixtures/`, read the way the reactivity suite reads its fixtures. The
    `0.02` floor is re-derived as half that fixture's measured coverage, and its comment says it
    comes from a fixture, not a shipped preset, until one ships.
  - **0275.** `the_wrap_seam_stays_outside_the_frame_at_every_depth` measures at `zoom = 1` *and* at
    the lowest `zoom` the shipped swarm presets bind, read from `presets/swarm_*.toml`. A shipped
    swarm preset whose `zoom` is an expression rather than a constant fails the test with a message
    asking the author to state its minimum, since a hand-kept list is what 0239 deleted.
- **Files touched:** `core/tests/sanity.rs`, `core/tests/fixtures/` (one waterfall fixture),
  `core/src/render/scenes/swarm/tests.rs`.
- **Done when:** `cargo nextest run -p rlx-core --test sanity` passes and its printed table has a
  waterfall row. The seam test's failure message names the zoom it measured. Temporarily setting
  `swarm_murmuration.toml`'s `zoom` to `0.78`, the value the owner saw the seam at, turns the seam
  test red; the implementer checks this once and reverts it.

### Phase 4 — The standalone names its windows and dates its thumbnails; the tooling catches up
- **Owner skill:** dev
- **What:** Four fixes:
  - **0284.** On Linux both windows carry an app id: the show `ritmolux`, the console
    `ritmolux-console`. It is set through winit's Wayland `with_name` and the X11 equivalent.
    `docs/running.md` gains the two ids and one example Hyprland rule.
  - **0260.** The thumbnail entry header carries `CARGO_PKG_VERSION`, and a mismatch reads as stale,
    so a cache written by an older build, or by one with no version field, re-renders.
  - **0242.** A `cli.test.mjs` case runs `readiness` against a test-written wrapper that emits a
    `ready` with one advisory, and asserts that the output contains `advisory (never parks):`.
  - **0239.** `node scripts/docs-shots.mjs swarm_braid swarm_maelstrom` re-renders the two gallery
    cards.
- **Files touched:** `standalone/src/run.rs`, `standalone/src/app_state.rs`,
  `standalone/src/thumbs.rs`, `docs/running.md`, `tools/conductor/test/cli.test.mjs`,
  `docs/images/gallery/presets/swarm_braid.png`, `docs/images/gallery/presets/swarm_maelstrom.png`.
- **Done when:** `git grep -n "ritmolux-console" -- standalone/src docs/running.md` matches in both
  places. A `thumbs.rs` test writes an entry stamped with a different version and reads it back as
  stale, and an entry stamped with the current version as fresh. `node --test tools/conductor/test/cli.test.mjs`
  passes with the new case. `git show --stat` on the phase commit lists both gallery PNGs.

### Phase 5 — The render service stops leaking on quit and on refusal
- **Owner skill:** studio-builder
- **What:** Two fixes in `studio/electron/render/service.ts`:
  - **0247, quit.** `abandon()` also stops work that is still in `start`'s preparation window, which
    is the transcode and the `--bars` read. The service holds the in-flight child or an abort signal
    from the moment `start` begins, so "Quitting stops it" is true at every point.
  - **0247, refusal.** A neural Start refused by `timelineProblem` removes the `<output>.bars.json`
    that `readBars` wrote for it.
- **Files touched:** `studio/electron/render/service.ts`, `studio/electron/render/service.test.ts`.
- **Done when:** A studio test calls `abandon()` while a stubbed transcode is pending and shows the
  stub was told to stop and no job launches afterwards. A second test drives a neural Start whose
  timeline is wrong for the grid and shows no `.bars.json` remains beside the output.
  `npm --prefix studio run typecheck`, `npm --prefix studio run lint` and `npm --prefix studio test`
  pass.

## Risks & open questions

- **Phase 2's hue change can move a baseline.** No shipped L-system preset is known to run endless
  mode at a depth well under its bound, but the implementer does not assume so: any moved baseline
  is blessed by name and listed, and the owed human phase 0237 Phase 8, which judges the look, sees
  the result.
- **Lorenz Knot looks slightly different at full bass.** The lens no longer quite holds the figure's
  size at the extreme. The owed 0240 Phase 6 re-curation is where the owner judges that.
- **The catch-all map must not swallow known tables.** `serde(flatten)` together with
  `deny_unknown_fields` on a child is a known serde trap. The Phase 1 done-when's clean `--strict`
  run over the shipped set and its known-table test are what catch it.
- **The X11 app id** has no reader on the reference box, which runs Wayland. It is set because it is
  one line, and it is unverified.

## What this plan does NOT do

- Backlog 0261, 0262, 0276 and 0277: each needs `architect`'s call first.
- Backlog 0274 and 0278: these are features, each with its own interview.
- The owed human phases (0218 P6, 0237 P8-9, 0239 P8, 0240 P6+P8, 0247 P6, 0248 P6+P8). This plan
  only makes some of them easier to judge.
- A runtime clamp of camera `distance` to its declared range.
- Any change to `main`'s CI beyond what run `37512244863` turns out to need.

## Implementation log

**Lane:**

| phase | owner | state | commit |
|---|---|---|---|
| 1 — The loader and the reference stop misleading an author | dev | not started | |
| 2 — The 3D findings | dev | not started | |
| 3 — The two gates that cannot see what shipped | dev | not started | |
| 4 — Window names, thumbnail dates, tooling | dev | not started | |
| 5 — The render service stops leaking | studio-builder | not started | |

### Notes

### Close triggers

- **`presets/` touched:**
- **Plan header `Closes:`** design-backlog 0260, 0275, 0282, 0283, 0284
- **What shipped:**
- **Operator docs touched:**
- **Backlog probes (`node scripts/check-backlog-claims.mjs`):**
- **Full suite:**
- **Outstanding `human` phases:** none in this plan

## Followups (after this lands)

- After the merge, the owner records `finding NNNN <ref> --done "Plan 0250 Phase N"` for each
  finding this plan fixed, unless the close already marked it `fixed_in`.
