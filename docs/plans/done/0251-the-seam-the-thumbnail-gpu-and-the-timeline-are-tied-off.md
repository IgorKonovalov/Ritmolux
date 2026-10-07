# 0251 — The seam, the thumbnail GPU and the timeline are tied off

> **Status:** done - Phase 4 owed, ADR-0249. Phases 1-3 in b647003b, 5d5c6caa, 8900f044; conductor
> close review round 1: no blockers, no majors, two minors (fixed in ed7d9447), one nit (open).
> Version 0.168.1.
> **Created:** 2026-10-07
> **Owner skill(s):** dev, studio-builder, human
> **Closes:** design-backlog 0261, 0285, 0286
> **Related ADRs:** ADR-0230 (thumbnails are rendered by a subprocess of the player), ADR-0246 (the
> adapter is a setting and the window prefers high performance), ADR-0146 (one name selects the GPU),
> ADR-0240 (a setting lives in a file), ADR-0037 (an internal grid is a resolution, not a shape)

## TL;DR

Three small followups from the 0206 and 0250 close reviews, none needing an ADR. The swarm's
`sway_bound` becomes exact, so a swayed camera at the lowest shipped `zoom` keeps the wrap seam off
screen, and the seam test measures that case. The thumbnail child renders on the adapter the show is
rendering on, and the diagnostics log names the adapter the child used. A studio render that is
cancelled after its `--bars` read finishes no longer leaves `<output>.timeline.json` behind.

## Context & problem

- **0286.** `sway_bound` (`core/src/render/scenes/swarm.rs`) bounds `yaw` and `pitch` with a
  first-order model of how far a turn moves the seam, scaled by `SWAY_SHARE = 0.8`. Plan 0250 Phase 3
  extended `the_wrap_seam_stays_outside_the_frame_at_every_depth` to Murmuration's `zoom = 0.85`.
  At that zoom, swayed to the bound, 233 particle-frames at 1280x800 reached 0.9961 NDC, so the test
  measures that zoom at rest only. Arithmetic at the rest `fov` 0.8: the seam sits at 1.25 NDC at
  `zoom = 1` (headroom 0.25) and at 1.039 NDC at `zoom = 0.85` (headroom 0.039). The bound gives
  about 0.075-0.087 rad at `zoom = 1` and 0.014-0.016 rad at 0.85, both aspects. Below about
  `zoom = 0.82` the seam is inside the frame at rest and the bound is already zero. No shipped swarm
  preset binds `yaw` or `pitch` today, so nothing that ships shows the seam. The first preset that
  sways at a low zoom would, and no test would see it.
- **0261.** The thumbnail pass (`standalone/src/thumbs.rs`, `child_command`) spawns
  `--thumb <name>` with no `--gpu`. The child renders through `shot::renderer`, which builds
  `Renderer::new_headless_tiered` with `prefer_software: false`, which resolves
  `AdapterChoice::Default`: wgpu's plain default, the power-saving part on a hybrid machine. Since
  ADR-0246 the show's unflagged window prefers `HighPerformance`, and an operator's `[output] gpu`
  or `--gpu` pins it. The child honours neither, and nothing records the adapter it used. Plan
  0206 Phase 4 saw the show's frame-time tail move (worst p99 from 50.0 ms to 76.8 ms) only beside a
  show pinned to the integrated GPU. Because the child's adapter is not logged, that reading cannot
  say whether the two processes were sharing one GPU.
- **0285.** In `RenderService.start` (`studio/electron/render/service.ts`), the neural branch calls
  `writeTimeline(files.timeline, ...)` once `readBars` resolves, and only then checks
  `abort.signal.aborted`. `abandon()` aborts the signal, but a `--bars` read that has already
  finished resolves anyway. The `finally` removes the grid (`bars`) for a job that never launched,
  but not the timeline. The window is one event-loop turn wide.

## Decision

Each fix takes the shape its backlog entry proposed. The two choices the entries left open are
settled here:

- **0286: an exact bound, not a bigger safety factor.** `sway_bound` projects the seam's corners
  through the swayed view, at the near and far slab depths, and finds the largest shared turn that
  keeps every projected corner outside the frame expanded by the pan. The corners come from the
  `half_extent` box, and the search is a bounded bisection on the scale it already shares between
  the axes. Perspective maps the box's straight edges to straight lines, so the corners carry the
  extremes. A safety factor derived from the headroom was rejected: it would still be a guess, and
  at `zoom = 0.85` it would have to be argued from the same 0.039 headroom the first-order model
  already misjudges. The function's signature, its zero result where an axis has no headroom, and
  its non-finite guards stay as they are.
- **0261: the child follows the show, and the log names both adapters.** The pass resolves the
  show's running adapter (`Renderer::adapter_description`) to a position in
  `rlx_core::render::list_adapters()` by matching `detail`, the equality `adapter_index` already
  uses. It then passes `--gpu <index>` to each child. The parent and the child are one executable on
  one machine, enumerating `Backends::all()`, so the index is stable between them. The cross-API
  objection in ADR-0146 does not apply. With no match, the child gets no flag and asks for
  `HighPerformance`, the window's own unflagged preference, rather than wgpu's default. A runtime
  adapter switch (`AppState::swap_adapter`) hands the new description to the pass, and the next
  child follows it. The child prints its adapter on standard error. The pass reads that line and
  writes one `thumbnail pass:` note per walk naming the child's adapter and the show's, and saying
  whether they differ. We rejected putting the child on whichever adapter the show is *not* on.
  It would spend a second GPU's power (waking a discrete part on battery), and it would override the
  operator's `[output] gpu` on the strength of one unexplained reading. ADR-0240's rule that the
  file is the setting covers the processes the player spawns. **This plan does not claim to cure
  0206's tail reading.** On the integrated-pinned case the child may already have been on the
  show's GPU. Phase 4 is the reading that can now say so.
- **0285: the `finally` owns both files.** The timeline joins the grid in the `finally`'s
  never-launched cleanup. That covers the abort path and every other path that writes the
  timeline and then fails to launch (a throw from `writeJob` or `launch`). The aborted check also
  moves ahead of `writeTimeline`, so a quitting studio writes nothing it then deletes.

## Implementation phases

### Phase 1 — The swarm's sway bound is exact at the shipped minimum zoom
- **Owner skill:** dev
- **What:** `sway_bound` is rewritten to the exact projected-corner bound above, keeping its
  signature and its zero-headroom and non-finite arms. Its doc comment states the new mechanism.
  `the_wrap_seam_stays_outside_the_frame_at_every_depth` measures the lowest shipped zoom
  **swayed** in all four diagonal directions, as it does `zoom = 1`. The "measured at rest only"
  paragraph and the `swayed: false` arm for the shipped zoom are removed. The usable-sway floor
  (`yaw > 0.02 && pitch > 0.02`) stays at `zoom = 1` only. At `0.85` the headroom is 0.039 NDC, and
  the first-order bound already gives about 0.014 rad, so that case asserts a strictly positive
  bound instead. `the_sway_bound_follows_the_lens_and_the_pan` keeps its ordering claims (a wider
  lens, a zoom and a pan each shrink the bound, an open lens can reach zero).
- **Files touched:** `core/src/render/scenes/swarm.rs`, `core/src/render/scenes/swarm/tests.rs`.
- **Done when:** `cargo nextest run -p rlx-core the_wrap_seam_stays_outside_the_frame_at_every_depth`
  passes. Its printed lines show, for both 1280x800 and 1920x1080, a swayed run at the lowest
  shipped zoom (0.85, Murmuration) with `0 projected inside the frame` and a sway bound above zero
  on both axes. `cargo nextest run -p rlx-core the_sway_bound_follows_the_lens_and_the_pan` passes.
  `git grep -c "measured at rest only" -- core/src/render/scenes/swarm/tests.rs` finds nothing.

### Phase 2 — The thumbnail child renders on the show's adapter and says which
- **Owner skill:** dev
- **What:** The `--thumb` mode reads `--gpu`, resolves it through `gpu::window_choice`, and renders
  through a `shot` constructor that takes an `AdapterChoice` (beside `shot::renderer`, which stays
  for its other callers). It prints `--thumb `<name>`: adapter: <description>` to standard error
  before rendering. `Pass::start` takes the show's running adapter description. The worker resolves
  it against `list_adapters()` once per walk (off the render thread) and adds `--gpu <index>` in
  `child_command` when it matches. `Pass` gains a method `AppState::swap_adapter` calls after a
  successful switch, so a later child follows the new adapter. `await_child` keeps the child's
  standard error on success too. The walk's first child that names an adapter produces one note,
  `thumbnail pass: children render on <child>, the show on <show>`, ending `(the same adapter)` or
  `(different adapters)`. When nothing matched, the note says the child took the high-performance
  default. `docs/configuration.md`'s `[thumbnails]` section gets one sentence: the pass renders on
  the show's adapter, and the log says which.
- **Files touched:** `standalone/src/thumbs.rs`, `standalone/src/shot/mod.rs`,
  `standalone/src/cli.rs` (a `--gpu` reader usable before the launch path, if `parse_thumb_arg`'s
  neighbours do not already provide one), `standalone/src/app_state.rs` (the two `Pass::start`
  sites and `swap_adapter`), `docs/configuration.md`.
- **Done when:** A unit test on `child_command` shows `--gpu 1` in the arguments when the pass holds
  index 1, and no `--gpu` when it holds none. A unix stub-executable test, in the shape of the
  existing `no adapter for $2` stub, echoes an `adapter:` line and exits 0. The pass then sends
  exactly one `thumbnail pass: children render on` note for the walk, naming the stub's adapter. A
  second stub reporting a different adapter from the show's produces a note containing
  `different adapters`. A `cli.rs` test shows `--thumb x --gpu 1` is not refused for a missing
  companion. `cargo nextest run -p standalone thumbs::` passes, and
  `git grep -c "children render on" -- standalone/src/thumbs.rs` finds the note's writer.

### Phase 3 — A cancelled render leaves no timeline beside the output
- **Owner skill:** studio-builder
- **What:** In `RenderService.start`, the neural branch records `files.timeline` beside `bars`.
  The `finally` removes both when the job did not launch. The `abort.signal.aborted` refusal moves
  ahead of `writeTimeline`. The test harness's `hold` learns to hold a `--bars` read open (today it
  holds only the transcode). A new test starts a neural render with the `--bars` read held, calls
  `abandon()`, then finishes the read. It asserts the start is refused with `the studio is
  quitting`, and that neither `<output>.bars.json` nor `<output>.timeline.json` exists.
- **Files touched:** `studio/electron/render/service.ts`, `studio/electron/render/service.test.ts`.
- **Done when:** `npm --prefix studio test -- electron/render/service.test.ts` passes with the new
  test. The test fails against the pre-phase `service.ts` (its log names the commit it was checked
  against). `npm --prefix studio run typecheck` and `npm --prefix studio run lint` exit 0.

### Phase 4 — The thumbnail reading is re-taken with both adapters named
- **Owner skill:** human
- **Blocks merge:** no
- **What:** On the reference laptop, with the show pinned to the integrated GPU and the thumbnail
  cache cleared, re-take Plan 0206 Phase 4's pair (pass running against pass off). Record the
  `thumbnail pass: children render on` line beside the frame-time readings. The reading answers what
  0261 could not: whether the moved tail was two processes on one GPU.
- **Files touched:** this plan's implementation log only.
- **Done when:** the log carries both runs' median fps, worst p99 and lowest one-second sample, and
  the note naming the child's adapter and the show's.

## Risks & open questions

- **The exact bound may be tighter than the first-order one at `zoom = 1`.** If it falls under the
  0.02 usable-sway floor at either target, that is a finding about the old bound over-allowing at
  rest zoom as well. `dev` records it and does not loosen the floor.
- **Sprite radius.** The seam test counts particles within one sprite radius of the seam and
  projects their centres. If the exact bound still leaves a centre a few thousandths inside the
  frame, the residual is the sprite term the projection omits. The fix then belongs in the bound
  (the seam inset by the largest sprite's NDC radius), not in `SWAY_SHARE`.
- **Roster drift between parent and child.** An adapter that appears or disappears between the
  parent's enumeration and the child's start (an eGPU unplugged) shifts indices. The child would
  then render on a neighbour, and the note would say `different adapters`. That is acceptable for a
  160x90 floor-tier still, and the note makes it visible.
- **A `--gpu` index the child cannot open** fails that child the way any render failure does
  (`thumbnail failed: ...`). After `GIVE_UP_AFTER` failures in a row the pass gives up for the
  launch, which is the existing policy.

## What this plan does NOT do

- It does not change `SWAY_SHARE`, any swarm preset, or what `zoom` means. No shipped preset binds
  `yaw` or `pitch`.
- It does not add a thumbnail setting or put the child on a different GPU from the show. It does
  not throttle the pass beyond the existing `nice` and floor tier.
- It does not touch the transcode-leak fixes Plan 0250 Phase 5 landed, or any other start path.

## Implementation log

> Written by `dev` — one row per phase as that phase's commit lands, and the close block after the
> last one. **The phases above are the contract; everything here is what happened.**

**Lane:** branch `plan-0251-the-seam-the-thumbnail-gpu-and-the-timeline-are-tied-off`, worktree
`/home/igor/Work/rlx-plan-0251`

| phase | owner | state | commit |
|---|---|---|---|
| 1 — The swarm's sway bound is exact at the shipped minimum zoom | dev | done | b647003b |
| 2 — The thumbnail child renders on the show's adapter and says which | dev | done | 5d5c6caa |
| 3 — A cancelled render leaves no timeline beside the output | studio-builder | done | 8900f044 |
| 4 — The thumbnail reading is re-taken with both adapters named | human | owed | |

### Notes

- Phase 1: the exact corner bound alone reproduced the first-order sizes (zoom 0.85 at 1280x800:
  yaw 0.0146, pitch 0.0158) and still put 238 particle-frames at 0.9961 NDC. Per the plan's
  sprite-radius risk, the bound now insets the seam by the largest seeded sprite at the default
  `size` (new `SPRITE_SIZE_MAX = 0.011`, the scatter's existing upper limit) and `SWAY_SHARE`
  stays 0.8 on top. Resulting bounds: zoom 1 yaw 0.0770 / 0.0724, pitch 0.0833 / 0.0820; zoom 0.85
  yaw 0.0089 / 0.0083, pitch 0.0096 / 0.0094 (1280x800 / 1920x1080). The `size` and `size_spread`
  params can draw sprites larger than the inset covers; `sway_bound`'s signature does not see them.
- Phase 2: `cli.rs` gained no reader; the child reads `--gpu` through the existing
  `windowed_flag`, and `cli.rs` carries only the companion test. A switch through
  `AppState::swap_adapter` reaches the next walk (the worker places the adapter once per walk), not
  a later child of a walk already running.
- Phase 3: the new test was run against `service.ts` as of 5d5c6caa and failed on the
  `.timeline.json` assertion. The neural branch keeps the existing aborted check after the branch
  and adds a second one ahead of `writeTimeline`, rather than moving the one.

### Close triggers

- **`presets/` touched:** none
- **Plan header `Closes:`** design-backlog 0261, 0285, 0286
- **What shipped:** fix-only (core swarm sway bound, standalone thumbnail child adapter and note,
  studio render-start cleanup)
- **Operator docs touched:** `docs/configuration.md` (`[thumbnails]`, Phase 2)
- **Backlog probes (`node scripts/check-backlog-claims.mjs`):** exit 0, 43 reductions across 20
  live entries, 3 unprobeable
- **Full suite:** owed to the conductor's pre-review gate (ADR-0207)
- **Outstanding `human` phases:** Phase 4 (blocks merge: no)

## Close review

Conductor close, round 1, 2026-10-07. **Phase 4 is owed** (ADR-0249): the reading on the reference
laptop has not been taken, so nothing yet says whether Plan 0206's moved frame-time tail was the
show and the thumbnail child sharing one GPU. Both minors below were repaired at the close in
`ed7d9447`; the nit is open. No earlier round raised findings.

### Plan 0251 — close review, round 1

Graded at `2638a5c9729fab40a313ee420feffe678ecac059` (tree `99a75a2e`), lane
`plan-0251-the-seam-the-thumbnail-gpu-and-the-timeline-are-tied-off`.

**Verdict: Plan 0251 landed cleanly. No blockers, no majors, two minors and one nit.** All three
machine phases do what their contracts say, and every done-when was re-run here. Phase 4 (human,
`Blocks merge: no`) is correctly `owed`.

### Evidence

- **Full suite (lens 1):** `node .../with-lock.mjs suite -- cargo nextest run --workspace` printed
  `with-lock: skipped cargo nextest run --workspace: tree 99a75a2 is green in the suite ledger, run by
  gate 0251-pre-review at 2026-10-07T13:03:05.748Z: 2023 tests run: 2023 passed (13 slow), 8 skipped`.
  `git rev-parse HEAD^{tree}` is `99a75a2e...`, so the record covers exactly this tip.
- `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`: clean.
- `cargo fmt --all -- --check`: clean. `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- Phase 1: `cargo nextest run -p rlx-core --no-capture the_wrap_seam_stays_outside_the_frame_at_every_depth the_sway_bound_follows_the_lens_and_the_pan`
  (through the suite wrapper) passed. Printed lines:
  - `1280x800 at zoom 0.85 (the lowest shipped zoom, from swarm_murmuration.toml, swayed): 313823 particle-frames ..., 0 projected inside the frame; nearest 1.0053 ndc (sway bound yaw 0.0089, pitch 0.0096)`
  - `1920x1080 at zoom 0.85 (..., swayed): 322503 ..., 0 projected inside the frame; nearest 1.0063 ndc (sway bound yaw 0.0083, pitch 0.0094)`
  - zoom 1: yaw 0.0770 / 0.0724, pitch 0.0833 / 0.0820, nearest 1.0575 / 1.0551, 0 inside.
  `git grep -c "measured at rest only" -- core/src/render/scenes/swarm/tests.rs`: no match.
- Phase 2: `cargo nextest run -p standalone thumbs:: the_thumbnail_child_takes_a_gpu`: 18 passed.
  `git grep -c "children render on" -- standalone/src/thumbs.rs`: 6.
- Phase 3: `npm --prefix studio test -- electron/render/service.test.ts`: 12 passed;
  `npm --prefix studio run typecheck` and `npm --prefix studio run lint`: exit 0.
- `node scripts/check-comment-hygiene.mjs`, `check-doc-links.mjs`, `check-reader-prose.mjs`: OK.
  `node scripts/check-backlog-claims.mjs`: OK, 43 reductions across 20 live entries, 3 unprobeable.

### Lens 1 — alignment

- Every phase carries one in-vocabulary `**Owner skill:**`; Phase 4's `Blocks merge: no` sits on a
  `human` phase whose output nothing later reads. Implementation log present, shorter than the
  phases, phase-to-commit mapping matches `git log main..HEAD`.
- **Phase 1.** `sway_bound` keeps its signature, the zero-headroom and non-finite arms, and now
  bisects one shared scale against `seam_clear`, which projects the eight slab corners through the
  real `Camera3d::view` at every sign combination of the two turns, either alone included. I checked
  the corner condition: requiring `a*x/w >= 1 && b*y/w >= 1` at every corner is exactly the union of
  "the `u = a` face is past `x = a`" and "the `v = b` face is past `y = b`", since each face's four
  corners are the corners sharing its sign. The inset is world-space and depth-independent as its
  comment derives, so the inset face stays planar and the convex-quad argument holds. The sprite
  inset (`SPRITE_SIZE_MAX`) was added under the plan's own sprite-radius risk, which named the bound
  as where that fix belongs; `SWAY_SHARE` is unchanged, as the plan requires. The test sways the
  shipped zoom in four diagonals, keeps the 0.02 floor at `zoom = 1` only and asserts `> 0` at 0.85.
- **Phase 2.** The child reads `--gpu` through `windowed_flag` and `gpu::window_choice`, whose `None`
  arm is `HighPerformance`; it builds through `shot::renderer_on` and prints
  ``--thumb `<name>`: adapter: <description>``. The parent places `adapter_description()` in
  `list_adapters()` by `detail` equality, the same rule `AppState::adapter_index` uses; the child's
  `AdapterChoice::Index` resolves against the same `new_without_display_handle` instance and
  `Backends::all()` enumeration (`core/src/render/context.rs:280,343,601`), so the index is stable.
  Both `Pass::start` sites and `swap_adapter` pass the description. The two stub tests assert one
  note per walk, the `--gpu 1` each child received, no `--gpu` with no roster match, and
  `different adapters`. The one deviation (a switch reaches the next walk, not the next child) is
  disclosed in the log; see minor 1.
- **Phase 3.** `beside` holds both files and the `finally` removes both on a non-launch; a second
  aborted check sits ahead of `writeTimeline` (the log discloses keeping the existing one too, which
  is harmless). The new test holds the `--bars` read, abandons, finishes it, and asserts the refusal,
  no spawn, and neither file. Against the pre-phase `service.ts` the timeline is written and not
  removed, so the test's last assertion is the one that would fail, as the log reports.

### Lens 2 — layering and real-time safety

No core layering change: `sway_bound` stays pure scene math. The pass's new `Mutex<String>` is taken
by the render thread only inside `swap_adapter` (a menu action) and briefly by the worker once per
walk; no audio-thread involvement. `list_adapters()` runs on the worker thread. No C ABI or control
protocol change.

### Lens 3 — docs and bookkeeping

`docs/configuration.md` `[thumbnails]` gained the sentence Phase 2 asked for (see minor 1 for its
precision). No other operator doc names the behaviour. Close owes: status `done - Phase 4 owed,
ADR-0249`, move to `done/`, `## Close review`, plans README, backlog 0261/0285/0286 to `### Closed`
in the archive, and a **patch** bump (the log says fix-only, and the three changes are fixes), with
the studio's two version copies. No ADR is paired. `presets/` untouched, so no curation.

### Lens 4 — correctness and determinism

`sway_bound` is deterministic and finite-guarded; the bisection's invariant (`hi` never clears, `lo`
clears or is zero) holds, and `clear(SCALE_MAX)` short-circuits the open case. The bound takes its
aspect from the render target passed into `camera_frame`, not a grid. No new numeric assertion is a
frozen measurement: the seam test's `inside == 0` is a property and the floors are unchanged.

### Lens 5 — design integrity

No seam widened. `shot::renderer` delegates to `renderer_on`, so its other callers are byte-for-byte
unchanged.

### Findings

#### minor

1. **`docs/configuration.md:543` says the pass follows "a switch from the settings menu", but a switch
   reaches only the next walk.** `serve` places the adapter once per walk
   (`standalone/src/thumbs.rs:703`), so after a switch during the first launch's full-library walk
   every remaining child of that walk renders on the old adapter, and a parked pass renders nothing
   until a rescan. The plan's Decision said "the next child follows it"; the log discloses the
   difference. Repair (prose, close-repairable): make the sentence read "Each render runs on the
   graphics adapter the show is rendering on, following an `[output] gpu` pin; after a switch from
   the settings menu, the next walk of the library follows the new adapter, and the pass's ..." A code
   fix (re-placing the show per child against the walk's cached roster) is the alternative, and is a
   `dev` change.
2. **`core/src/render/scenes/swarm.rs:125-128`: the `SWAY_SHARE` doc comment claims its remaining 20 %
   is room for a sprite a bound `size` or `size_spread` draws larger than `SPRITE_SIZE_MAX`, and
   nothing derives or probes that.** The seam test runs only the default `size`, which is the one case
   the exact bound already covers, so the claim is the same unprobed shape 0286 was opened about (a
   preset not yet written exposes it). Repair (comment text, close-repairable): replace the doc with
   "The share of [`sway_bound`]'s exact seam-corner bound the sway may take. The bound covers sprites
   up to [`SPRITE_SIZE_MAX`] at the default `size`; a bound `size` or `size_spread` can draw larger
   ones, which this share is not derived to cover." A backlog entry for a `size`-aware bound is the
   architect's call at the close.

#### nit

3. **`core/src/render/scenes/swarm.rs:806`: `sway_bound` now runs up to 25 `seam_clear` calls, each
   building up to nine view matrices, from `camera_frame` every frame.** Short-circuiting keeps it to
   microseconds, so it is no budget risk, but its inputs (`fov`, `zoom`, `aspect`, `pan`) rarely
   change; memoising on them would make the per-frame cost a comparison. Leave unless a profile
   points here.

### Close notes

- Minor 1: fixed in `ed7d9447` with the review's replacement sentence. Minor 2: fixed in `ed7d9447`
  with the review's replacement doc comment; no backlog entry is filed for a `size`-aware bound,
  because no shipped swarm preset binds `yaw`, `pitch`, `size` or `size_spread` with a sway. Nit 3:
  open.
- Upstream CI (`check-upstream-ci.mjs`): run 36297464014 on `main` at `4ffed87`, success.
- Backlog 0261, 0285 and 0286 moved to the archive's `### Closed` with their `CLOSED` markers.
- No paired ADR. `presets/` untouched, so no curation.
- Translation advisory: `docs/running.ru.md` trails `docs/running.md` (stamped `fb237a6c`, source at
  `b25cd8cf`); this plan did not move it.

## Followups (after this lands)
