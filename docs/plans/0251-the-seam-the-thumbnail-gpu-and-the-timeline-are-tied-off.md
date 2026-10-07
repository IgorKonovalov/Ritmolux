# 0251 — The seam, the thumbnail GPU and the timeline are tied off

> **Status:** in-progress
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
| 1 — The swarm's sway bound is exact at the shipped minimum zoom | dev | done | committed with this row |
| 2 — The thumbnail child renders on the show's adapter and says which | dev | not started | |
| 3 — A cancelled render leaves no timeline beside the output | studio-builder | not started | |
| 4 — The thumbnail reading is re-taken with both adapters named | human | not started | |

### Notes

- Phase 1: the exact corner bound alone reproduced the first-order sizes (zoom 0.85 at 1280x800:
  yaw 0.0146, pitch 0.0158) and still put 238 particle-frames at 0.9961 NDC. Per the plan's
  sprite-radius risk, the bound now insets the seam by the largest seeded sprite at the default
  `size` (new `SPRITE_SIZE_MAX = 0.011`, the scatter's existing upper limit) and `SWAY_SHARE`
  stays 0.8 on top. Resulting bounds: zoom 1 yaw 0.0770 / 0.0724, pitch 0.0833 / 0.0820; zoom 0.85
  yaw 0.0089 / 0.0083, pitch 0.0096 / 0.0094 (1280x800 / 1920x1080). The `size` and `size_spread`
  params can draw sprites larger than the inset covers; `sway_bound`'s signature does not see them.

### Close triggers

- **`presets/` touched:**
- **Plan header `Closes:`** design-backlog 0261, 0285, 0286
- **What shipped:**
- **Operator docs touched:**
- **Backlog probes (`node scripts/check-backlog-claims.mjs`):**
- **Full suite:**
- **Outstanding `human` phases:**

## Followups (after this lands)
