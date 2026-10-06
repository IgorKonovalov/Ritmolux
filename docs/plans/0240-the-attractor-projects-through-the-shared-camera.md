# 0240 — The attractor projects through the shared camera

> **Status:** in-progress (2026-10-06). Runs after Plan 0236 closes.
> **Created:** 2026-10-01
> **Owner skill(s):** dev, human
> **Related ADRs:** [ADR-0260](../adrs/0260-the-attractors-3d-families-project-through-the-shared-camera-and-perspective-retires.md) (proposed), [ADR-0076](../adrs/0076-the-attractor-keeps-the-depth-it-already-computes.md), [ADR-0093](../adrs/0093-attractor-tuples-are-content-with-per-tuple-framing.md), [ADR-0257](../adrs/0257-a-shared-camera-projects-3d-primitives-and-depth-of-field-is-a-per-endpoint-circle-of-confusion.md), [ADR-0258](../adrs/0258-a-system-takes-depth-through-one-shared-camera-block-and-its-3d-mode-forgoes-what-seg3d-does-not-draw.md) (proposed)

## TL;DR

The attractor's 3D families (Thomas, Lorenz) project through `Camera3d` instead of the
pseudo-perspective `magnify`. They gain a **pitch** and a real `distance`, the centroid orbit that
capped `perspective` near 0.3 goes away, and `focus` and `aperture` act on the camera's real lens.
`perspective` retires (ADR-0260). The shipped presets are migrated by an exact mapping, so the merged
tree renders close to today. The re-curation in motion that the owner chose comes after, as content
work. The flat families and their goldens do not move.

## Context & problem

All projection happens in the attractor's vertex shader:

- `project_figure` rotates by the spin phase (a yaw, about XZ for Lorenz and XY otherwise).
- `depth_norm` normalizes and clamps the depth to `[-1, 1]`.
- `magnify` scales by `1 / (1 - perspective * dn)`.
- `ndc = (world.x / aspect, world.y) * zoom + pan`.

Its CPU mirror, `projection_mirror.rs`, is a hand transcription that is not pinned to the GPU. Plan
0235 Phase 5 added `figure_coc` on a fixed virtual lens. 22 presets use the attractor; 10 draw a 3D
family, at `perspective` 0.18 to 0.5, and `fragment_sumi` draws Thomas at 0. The only 3D projection
golden is `attractor_depth` (Lorenz, `perspective` 0.5).

ADR-0260 records why the projection moves, why `perspective` retires rather than staying as an alias,
and why the flat families stay.

## Decision

As ADR-0260 decides:

- A **model transform** (measured centre, basis, measured framed half-extent) brings every 3D roster
  entry to unit radius.
- The attractor splices the **shared camera block**, with `spin` added to `yaw`.
- The 3D path projects with `project()` and blurs with the real `coc()`.
- `perspective`, the `dn` clamp and the virtual lens retire.
- Flat families keep their path, with the camera params declared inert.
- The 3D path's CPU mirror becomes `CameraView::clip`.
- The shipped presets are migrated by the exact mapping (`distance = E / p`, the derived `fov`,
  `pitch = 0`) in this plan. Their re-curation is owed after the merge.

## Architecture diagram

```mermaid
flowchart LR
    subgraph vs[attractor vertex shader]
        FAM{family has depth?}
        MT["model transform<br/>centre, basis, 1/framed_half"]
        PRJ["camera.wgsl project(), coc()<br/>yaw = yaw + spin"]
        FLAT["in-plane path, unchanged<br/>roll, scale, zoom, pan"]
    end
    CB[CameraParams<br/>shared block] --> PRJ
    FAM -- yes --> MT --> PRJ
    FAM -- no --> FLAT
    MIR["CPU: CameraView::clip (3D)<br/>projection_mirror.rs (flat only)"]
```

## Implementation phases

### Phase 1 — Walking skeleton: Lorenz through the camera
- **Owner skill:** dev
- **What:**
  - Splice `CameraParams` on the attractor, live on families with depth and inert on flat ones.
  - Add the model transform per roster entry, and project the 3D branch through `project()`, with
    `spin` added to `yaw`. Sprite size follows perspective (`at_depth`).
  - Point the 3D path's CPU property tests at `CameraView::clip`, and keep `projection_mirror.rs`
    for the flat path.
  - The camera params ride the attractor's uniform. If its padding lanes run out, it grows, and the
    flat path's bytes stay unchanged.
- **Files touched:** `core/src/render/scenes/particles/shaders.rs`,
  `core/src/render/scenes/particles/mod.rs`, `core/src/render/scenes/particles/encode.rs`,
  `core/src/render/scenes/particles/resources.rs`,
  `core/src/render/scenes/particles/projection_mirror.rs`, `core/src/render/scenes/particles/family.rs`,
  `core/src/render/scenes/particles/tests.rs`, `core/tests/`.
- **Done when:**
  - The flat-family goldens (`attractor`, `attractor_trails`, `attractor_ifs`) are byte-identical.
  - A Lorenz fixture at `pitch = 0.6` renders through `shot`, and so does the same fixture at
    `pitch = 0`; the two differ.
  - Sweeping `yaw` through a full turn moves the projected centroid of a Lorenz figure by less than the
    `0.9 * p` orbit ADR-0076's Outcome measured at the matched `p`.
  - `every_roster_entry_fills_the_frame_like_its_canonical_figure` passes for every 3D entry at the
    default camera.

### Phase 2 — The real lens, and `perspective` retires
- **Owner skill:** dev
- **What:**
  - Replace `figure_coc` and its virtual lens with the shared lens helper on the camera's real focal
    depth.
  - Remove `perspective`, `MAX_PERSPECTIVE`, `magnify` and the `dn` clamp.
  - Find out what the loader does with a binding to a param no longer declared (`ParamRoute::Unclaimed`).
    If it is silent, make a binding to `perspective` on the attractor a load error that names
    `distance` and `fov`. A user preset should fail loudly rather than lose its depth unnoticed.
- **Files touched:** `core/src/render/scenes/particles/shaders.rs`,
  `core/src/render/scenes/particles/mod.rs`, `core/src/render/scenes/particles/encode.rs`,
  `core/src/render/scenes/particles/tests.rs`, `core/src/preset/schema/load.rs`, `core/tests/`.
- **Done when:**
  - With `aperture > 0` on Thomas, sprites far from `focus` render wider and dimmer than sprites at
    it, and a flat family renders identically at any `aperture`.
  - A preset binding `perspective` fails to load with a message naming the replacement.
  - `git grep -c "MAX_PERSPECTIVE" -- core/src` finds nothing.

### Phase 3 — The shipped presets migrate by the mapping
- **Owner skill:** dev
- **What:**
  - Rewrite each shipped preset that sets `perspective`, and each attractor layer in
    `fragment_nebula` and `fragment_sumi`, onto `distance`, `fov` and `pitch = 0`. Use
    `distance = E / p` in model units and `tan(fov / 2)` from the preset's scale and zoom, as ADR-0260
    states. Leave `zoom` at 1 on the camera, since `Camera3d` divides the angle, not its tangent.
  - A `perspective = 0` layer takes a long `distance` and a matching narrow `fov`.
  - Do not re-bless `attractor_depth`: baselines are blessed on DX12 WARP only, and that is Phase 7.
    Record in the log whether its capture moved, from the off-WARP drift report the golden run prints.
  - Write a test that holds every migrated 3D preset's framed coverage within the `sanity` floor.
- **Files touched:** `presets/attractor_*.toml` (the 3D-family set), `presets/fragment_nebula.toml`,
  `presets/fragment_sumi.toml`, `core/tests/`.
- **Done when:**
  - The golden suite passes on the session's adapter, and the log says whether `attractor_depth`
    moved, quoting the drift report.
  - The `sanity`, `animation`, `reactivity` and `distinctness` suites pass on all 22 presets.
  - `git grep -c "perspective" -- presets` finds no binding.

### Phase 4 — Documentation and the references
- **Owner skill:** dev
- **What:**
  - The generated params block, `presets/schema/` and `.taplo.toml` are regenerated.
  - `docs/presets.md` replaces `perspective` with the camera block, and says which families it
    reads on.
  - The log carries the replacement text for the preset-author reference's `## attractor` section,
    for the owner to apply in Phase 8: the camera block in place of `perspective`, the families it
    reads on, and the migration rule, so the content lane can re-curate from it. A headless session
    cannot edit `.claude/` (ADR-0210).
  - `scripts/tuple-sheets.mjs` still renders 3D roster entries at the default camera.
- **Files touched:** `presets/README.md`, `presets/schema/`, `.taplo.toml`, `docs/presets.md`,
  `docs/specs/player-schema.json`, `scripts/tuple-sheets.mjs`.
- **Done when:** the schema and param-reference tests pass on the regenerated files.
  `node scripts/toc.mjs --check`, `node scripts/check-doc-links.mjs` and
  `node scripts/check-reader-prose.mjs` pass.

### Phase 5 — Fog on the attractor's sprites
- **Owner skill:** dev
- **What:**
  - The shared camera block gained `fog` and `solid` with Plan 0248 (ADR-0263), and both were built
    for the `seg3d` stroke, which the attractor does not draw. Its families with depth now take `fog`:
    each sprite's light is multiplied by the camera's `fog_light()` at its view depth, the function
    `CAMERA_WGSL` already carries and `CameraFrame::fog_light` mirrors on the CPU. Live on the
    families with depth, inert on the flat ones, as the rest of the block is.
  - `solid` is not taken: sprites have no stroke to sort far to near. The attractor's setter answers
    only the params it declares, as the swarm's does, so a `solid` binding on the attractor is an
    undeclared param.
  - The params block, `presets/schema/`, `.taplo.toml` and `docs/specs/player-schema.json` are
    regenerated, `docs/presets.md` names `fog` among the attractor's camera params, and the log's
    reference text for Phase 8 gains `fog`.
- **Files touched:** `core/src/render/scenes/particles/mod.rs`,
  `core/src/render/scenes/particles/shaders.rs`, `core/src/render/scenes/particles/encode.rs`,
  `core/src/render/scenes/particles/tests.rs`, `core/tests/attractor.rs`, `presets/README.md`,
  `presets/schema/`, `.taplo.toml`, `docs/specs/player-schema.json`, `docs/presets.md`.
- **Done when:**
  - `preset::declared_params_match_set_param` passes, and the whole `-P fast` profile is green.
  - A test holds that, on a 3D family at `fog = 1`, the summed light of the far half of the figure
    by view depth falls relative to `fog = 0`, and the near half falls less.
  - At `fog = 0` a 3D capture is byte-identical to the same capture before this phase, and the flat
    fixtures are byte-identical, both checked on the session's adapter.

### Phase 6 — The 3D presets, re-curated in motion
- **Owner skill:** human
- **Blocks merge:** no
- **What:** the owner runs the ten 3D attractor presets and the two fragment layers live, now with
  `pitch`, a real `distance` and `fog` available, and briefs `preset-author` on which to re-curate
  and how. The migration already keeps each close to its old picture, so the merge does not wait (ADR-0249).
- **Files touched:** none.
- **Done when:** the owner records the brief, or a keep per preset, in this plan's log.

### Phase 7 — The moved baseline is blessed
- **Owner skill:** human
- **Blocks merge:** no
- **What:** if Phase 3's log says `attractor_depth` moved, re-bless it on DX12 WARP, and nothing else.
  If Plan 0218 has moved blessing to lavapipe by then, bless there instead.
- **Files touched:** `core/tests/golden/attractor_depth.png`.
- **Done when:** the Windows CI golden job is green on `main`.

### Phase 8 — The preset-author reference
- **Owner skill:** human
- **Blocks merge:** no
- **What:** the owner applies, in an interactive session, the text Phase 4 left in the log to
  `.claude/skills/preset-author/references/systems.md`'s `## attractor` section.
- **Files touched:** `.claude/skills/preset-author/references/systems.md`.
- **Done when:** the edit is committed on `main` and `node scripts/check-doc-links.mjs` exits 0.

## Data shapes

```rust
// illustrative, not the final interface
// per roster entry, derived from the measured framing that ADR-0093 already stores
struct ModelTransform {
    centre: [f32; 3],
    basis: [[f32; 3]; 3],   // identity, or Lorenz's XZ swap
    inv_framed_half: f32,   // brings the entry to unit radius
}
// migration of one preset, before re-curation:
//   distance = E / p                        (E in model units; p the old perspective)
//   tan(fov / 2) = p / (scale * E * zoom)   (camera zoom = 1)
//   pitch = 0, yaw = 0 (spin still adds)
```

## Risks & open questions

- **Lorenz's far lobes overran the clamp.** Its depth reaches about `1.22 * E`, and those points now
  project unclamped. If one comes near the near plane at a preset's migrated `distance`, the minimum
  `distance` must exceed the model radius plus a margin. Phase 1 sets that bound, and Phase 3's coverage
  test catches a preset that violates it.
- **The uniform may need to grow.** The four padding lanes Plan 0235 Phase 5 used are taken. Growing
  the uniform is fine as long as the flat path's goldens stay byte-identical, which Phase 1 asserts.
- **Silent loss of `perspective` on a user preset** is the failure ADR-0260 names. Phase 2 makes it
  loud, or the log states why that was not possible.
- **The tuple contact sheets** are manual renderers, not gates. If `tuple-sheets.mjs` breaks, nothing
  goes red. Phase 4 runs it once by hand.

## What this plan does NOT do

- Move the flat families onto the camera (ADR-0260, Alternative C).
- Change any roster entry's index, coefficients or measured framing (ADR-0093).
- Re-curate the presets' look. That is Phase 6's brief and `preset-author`'s work.

## Implementation log

**Lane:** branch `plan-0240-the-attractor-projects-through-the-shared-camera`, worktree
`/home/igor/Work/rlx-plan-0240`

| phase | owner | state | commit |
|---|---|---|---|
| 1 — Walking skeleton: Lorenz through the camera | dev | done | 19386418 |
| 2 — The real lens, and `perspective` retires | dev | done | 80566e42 |
| 3 — The shipped presets migrate by the mapping | dev | done | 7a43c794 |
| 4 — Documentation and the references | dev | done | a430d6fa |
| 5 — Fog on the attractor's sprites | dev | done | 1ed3c786 |
| 6 — The 3D presets, re-curated in motion | human | owed | |
| 7 — The moved baseline is blessed | human | owed | |
| 8 — The preset-author reference | human | owed | |

### Notes

- Phase 1: the spin composes with the camera as `yaw - spin_phase` (`encode::spun`), not `+`:
  the camera's yaw turns the eye, so subtracting turns the figure the way a positive `spin`
  turned it. `the_mapped_camera_reproduces_the_retired_magnification` pins the direction against
  the old mirror.
- Phase 1: a sprite's size is stated at the orbit target (`viewport.z = distance`), not at the
  focal plane as on the plexus, so `focus` never resizes a sprite. Its radius is the old world
  size divided by the entry's footprint (`scale * framed half`), which makes the mapping exact
  for sprites too.
- Phase 1: from this phase until Phase 2, `perspective` is inert on the 3D path, and every shipped
  3D preset renders through the default camera until Phase 3.
- Phase 1: "flat goldens byte-identical" was checked on this session's adapter, not on WARP:
  `shot` captures of `attractor`, `attractor_trails`, `attractor_ifs` and `attractor_fb_rotate`
  compared with `cmp` before and after the phase, all identical. `attractor_depth` differs.
- Phase 1: the pitch done-when was run through `shot` on scratch fixtures under `target/`, and is
  also held by `a_lorenz_figure_pitches_through_the_camera` in `core/tests/attractor.rs`.
- Phase 1: the centroid swing over a full yaw turn measured 0.116 NDC at the camera matched to
  `p = 0.25` and 0.141 at `p = 0.5` (`a_yaw_sweep_barely_moves_the_lorenz_centroid`).
- Phase 1: the near-plane bound is held by `every_3d_entry_stays_clear_of_the_near_plane`: every
  banked point of every 3D entry, in model units, is nearer its centre than `distance`'s declared
  minimum (1.5) less `camera::NEAR`. The shader also drops a particle nearer than `NEAR`.
- From Phase 1 until Phase 4 regenerates them, three generated-file checks are red in `-P fast`:
  `the_parameter_reference_block_is_current`, `the_player_schema_snapshot_is_current` and
  `the_generated_editor_files_are_current`. The second compares `docs/specs/player-schema.json`,
  which Phase 4's file list does not name; `RLX_UPDATE_PRESET_SCHEMA=1` rewrites it with the rest.
  The other 1882 tests pass.
- Phase 2: the loader was not silent before this phase. An undeclared param was a load warning
  (`unknown parameter ... binding kept, but nothing reads it`), not an error. The done-when names a
  load error, so `load.rs` gained `RETIRED_PARAMS`: a binding to `perspective` on the attractor, at
  the top level or in a layer, fails with `PresetError::Config` naming `distance` and `fov`.
- Phase 2: the `dn` clamp left `depth_norm`, which is gone, and became a saturation in `depth01`,
  because the haze goes negative past the far extent. The flat path's `dn` is now a literal 0.
- Phase 2: `core/tests/fixtures/attractor_depth.toml` moved off `perspective = 0.5`,
  `zoom = 1.30` onto the mapping (`distance = 2.0`, `fov = 1.1839`, `pitch = 0`, zoom 1),
  because the retired binding no longer loads. Its baseline was not re-blessed.
- Phase 2 touched `core/src/render/scenes/particles/projection_mirror.rs`, which only Phase 1
  lists. Its transcriptions of `depth_norm`, `magnify` and `figure_coc` went with the shader
  functions they mirror, `world` lost its `perspective` argument, and `depth01` gained the
  saturation.
- Phase 2: the flat fixtures stayed byte-identical on this adapter (the same `shot` and `cmp`
  check as Phase 1).
- Between Phase 2 and Phase 3, the shipped presets that still bind `perspective` fail to load. In
  this phase that reddened `attractor_contract` and `attractor_projects_at_the_target_aspect`,
  which load `attractor_walkdejong`.
- Phase 3: the shipped set no longer matches the plan's counts. 12 presets draw the attractor, not
  22, and 6 draw a 3D family, not 10: Ink on Paper, Lorenz Knot, Thomas Gallery, Thomas on Red and
  Thomas Walk, plus the `fragment_sumi` layer. Each was migrated onto `distance`, `fov` and
  `pitch = 0`, with `E` and the footprint measured per roster entry. `attractor_fernmono` (fern)
  and `attractor_walkdejong` (De Jong) also bound `perspective`, which was inert on their flat
  families, so their bindings were deleted with no replacement. `fragment_nebula`'s layer is De
  Jong and binds nothing the camera replaced, so it is unchanged.
- Phase 3: mapping approximations.
  - Ink on Paper and Thomas Gallery step a 13-entry roster. They use `E = 1` and footprint 0.572,
    which 11 entries share. Entry 0's footprint is 0.63, and entries 1 and 4 have `E` of 0.72 and
    0.84.
  - Lorenz Knot's `perspective` was bass-driven. `distance = 1 / p` follows it exactly. Expressions
    have no `atan`, so `fov` is a quadratic fit in the bass term, within 0.0008 rad.
  - Thomas Walk's `zoom = 1.0 + bar * 0.04` stays on the camera's `zoom`, which divides the angle
    and not its tangent, so the pulse is near the old one but not equal.
  - Sumi's layer takes `distance = 20` and `fov = 0.1299`. That is a 1.1:1 near-to-far ratio, and
    both values are outside the declared ranges, which document and do not clamp.
- The camera divides `pan_x` by the aspect, which the attractor's in-plane path did not. This
  landed in Phase 1 and was not noted there. In Phase 3, Ink on Paper's `pan_x` swing was scaled by
  16/9, which reproduces the old drift on a 16:9 target only.
- Phase 3: the `attractor_depth` capture moved. The golden run on llvmpipe (off WARP, so the
  comparison is reported and not asserted) printed `attractor_depth    mean 0.0022 (tol 0.02)
  max_outlier 62 (tol 48)`. The unchanged flat `attractor` fixture printed mean 0.0018 /
  max_outlier 114 on the same adapter. A `shot` capture of the fixture differs byte-wise from the
  pre-plan capture. Not re-blessed (Phase 7).
- Phase 3: `every_migrated_3d_preset_clears_the_sanity_floor` (`core/tests/attractor.rs`) is the
  coverage test. Coverage at drive 0.4 and 1.0 ran 0.258 (Thomas on Red) to 0.535 (Lorenz Knot)
  against the attractor floor of 0.11, and Sumi read 0.92 against 0.21. Every preset spread over
  four quadrants.
- Phase 3: `sanity` (32 run, 2 `#[ignore]` instruments skipped) and `animation`, `reactivity`,
  `distinctness` (46 run, 1 `#[ignore]` skipped) pass on llvmpipe. `-P fast` is 1881 of 1884,
  and the three red tests are the generated-file checks named under Phase 1.
- Followup noticed: `core/src/render/scenes/particles/ifs.rs` (the `SIGMA_CEILING` doc) still cites
  `perspective` as a silently clamped param. The file is outside every phase of this plan.
- Phase 4: `docs/presets.md` never named `perspective`. It gained a section, "The attractor's 3D
  families are seen through the camera", and the `attractor` row of its systems table links to it.
  The `perspective` essay was in `presets/README.md` ("Attractor depth"), which was rewritten
  around the camera block with the migration rule. Its two measured `perspective` bullets (the
  silent 0.8 clamp, and the centroid-orbit table) were removed, and two bullets replace them:
  the magnification ratio `(D + 1) / (D - 1)` and how `spin` composes with `yaw`.
- Phase 4: the regeneration also rewrote `presets/preset.schema.json`, which the file list does
  not name. `.taplo.toml` did not change. `docs/specs/player-schema.json` was rewritten by
  `RLX_UPDATE_PRESET_SCHEMA=1`, as the Phase 1 note says.
- Phase 4: `scripts/tuple-sheets.mjs` was not changed. It was run once
  (`node scripts/tuple-sheets.mjs target/tuple-sheets`), and it rendered all four sheets, Thomas
  and Lorenz at the default camera (`pitch` 0.25, `distance` 3.5). On the Lorenz sheet, entry 0
  draws as a near-solid filled blob at the script's `size = 0.4`. Every other entry shows its
  strands.
- Phase 4: the done-when's generated-file tests (`preset_schema::` and
  `the_parameter_reference_block_is_current`, 14 tests) pass without the regenerate switches.
  `toc.mjs --check` was stale after the README heading rename. `node scripts/toc.mjs` rewrote
  the block, and the check, `check-doc-links.mjs` and `check-reader-prose.mjs` all exit 0.
- Phase 5: the attractor's `set_param` no longer delegates to `CameraParams::set`. It matches the
  seven camera names it declares itself, because the delegation also answers `solid`. With that
  change, `declared_params_match_set_param` scans those seven names like the rest of the
  attractor's arms.
- Phase 5 touched `core/src/render/scenes/particles/family.rs`, which its file list does not name:
  `FAMILY_PARAMS` gained a `fog` row, live on `thomas` and `lorenz` and inert on the flat families,
  as the other six camera rows are. The generated reference's `fog` row comes from it.
- Phase 5: fog is applied at the head's view depth, on the scale of the unit model volume
  (`MODEL_RADIUS`), and it multiplies the light only. It rides `mdl.w`, which was unused. The
  uniform did not grow.
- Phase 5: the near/far test is `fog_dims_the_far_half_of_a_3d_figure_more_than_the_near_half`
  (`particles/tests.rs`). It reads a converged GPU cloud back, splits it at the median view depth,
  and sums `CameraFrame::fog_light`, the CPU mirror. It also checks the shader's 3D path for the
  `fog_light` call. At `fog = 1` Thomas's near half keeps 0.721 of its light and its far half
  0.281. Lorenz's keep 0.639 and 0.356. The engine-level test,
  `fog_darkens_a_3d_figure_and_leaves_a_flat_one_alone` (`core/tests/attractor.rs`), measured
  the figure's mean luma at 78.27 -> 66.83 on Thomas and 103.57 -> 97.62 on Lorenz. It also holds
  `fog = 0` byte-equal to an unset `fog`, and holds De Jong byte-equal at `fog = 1`.
- Phase 5: the byte-identity done-when was checked on this session's adapter with `shot` captures
  (320x180, 60 frames) before and after the phase, compared with `cmp`. The 3D captures were
  `attractor_depth` and `presets/attractor_thomasgallery.toml`, both at `fog` unset. The flat
  captures were `attractor`, `attractor_trails` and `attractor_ifs`. All five were identical.
- Phase 5: `.taplo.toml` did not change on regeneration. `presets/preset.schema.json` did, and the
  file list does not name it. `-P fast`: 1920 run, 1920 passed. The deferred `attractor` suite
  passed, 11 of 11.

#### Replacement text for Phase 8 (`.claude/skills/preset-author/references/systems.md`, `## attractor`)

Replace the paragraph that begins `**Also declared, and worth knowing exist:**` with the two
paragraphs below, and leave the rest of the section as it is:

```markdown
**The 3D families — `thomas` and `lorenz` — are seen through the shared camera** (ADR-0260), the
same block as `plexus`: `yaw` (on top of `spin`), `pitch` (default `0.25`, slightly from above),
`distance` (in **figure radii**, `1.5 – 8`; nearer exaggerates the perspective, near-to-far
magnification `(D + 1) / (D - 1)`), `fov` (`0.2 – 2` rad; the engine `zoom` divides it), the
real lens `focus` / `aperture` (off-focus sprites draw wider and dimmer), and `fog` (`0 – 1`; at `1`
the figure's farthest point is black and its nearest keeps its light, light only, never size; it
multiplies with `depth_fade`). All seven are **inert on `de_jong`, `clifford` and the five IFS
figures**, which keep their in-plane view. `solid` is **not** an attractor param: a sprite has no
stroke to sort, so binding it warns as undeclared. **`perspective` is
retired, and binding it is a load error** naming `distance` and `fov`. To migrate an old value `p`
on an entry of depth half-extent `E`: `distance = E / p`, `tan(fov / 2) = p / (scale * E * zoom)`,
camera `zoom = 1`, `pitch = 0`. A `perspective = 0` look takes a long `distance` with a matching
narrow `fov` (Sumi's layer: `20`, `0.13`). The shipped 3D presets were migrated by that rule, so
they sit at `pitch = 0`. Re-curating one means reaching for `pitch` and a nearer `distance`, which
the old projection could not do without sliding the figure round the frame. On these two families
`pan_x` is in frame heights, not NDC.

**Also declared, and worth knowing exist:** `depth_fade`, `depth_hue` and `spin` (README,
"Attractor depth: the camera"); the map shapers `curl`, `vigor` and `lean`; and `emergence`, the
seconds the figure takes to settle out of its starting cloud. In `[particles]`, `density` sets how
much of the tier's particle budget is drawn (ADR-0069 / ADR-0195; absent is the whole budget), and
`morph_to` names a second **IFS** figure for `morph` to travel towards (ADR-0075). All in
`presets/README.md`.
```

### Close triggers

- **`presets/` touched:** yes. Phase 3 rewrote `attractor_ink`, `attractor_lorenzknot`,
  `attractor_thomasgallery`, `attractor_thomasred`, `attractor_walkthomas`, `attractor_fernmono`,
  `attractor_walkdejong` and `fragment_sumi`. Phase 4 regenerated `presets/README.md`'s params
  block, `presets/preset.schema.json` and `presets/schema/attractor.schema.json`, and rewrote the
  README's attractor-depth section. Phase 5 regenerated the same three files for `fog` and added
  a `fog` paragraph to that section. No preset `.toml` changed in Phase 4 or 5.
- **Plan header `Closes:`** none
- **What shipped:** feature. The attractor's `thomas` and `lorenz` take the camera block, `fog`
  included and `solid` excluded, and `perspective` is retired as a load error.
- **Operator docs touched:** `presets/README.md`, `docs/presets.md`, `docs/specs/player-schema.json`.
- **Backlog probes (`node scripts/check-backlog-claims.mjs`):** exit 0, 58 reductions hold across
  29 live entries, 4 unprobeable (re-run after Phase 5).
- **Full suite:** owed to the conductor's pre-review gate (ADR-0207).
- **Outstanding `human` phases:** 6 (re-curation brief), 7 (re-bless `attractor_depth`, which
  Phase 3 recorded as moved) and 8 (apply the text above to the preset-author reference). All three
  are `Blocks merge: no`.

## Followups (after this lands)

- `preset-author`: the re-curation per Phase 6's brief.
- Accept ADR-0260 at the close, and mark ADR-0076 and ADR-0257 amended in the ADR index.
