# 0181 — A scene advances after its frame's bindings

> **Status:** in-progress
> **Created:** 2026-09-14
> **Owner skill(s):** `dev`
> **Related ADRs:** [0198](../adrs/0198-a-scene-advances-after-its-frames-bindings.md) (proposed),
> [0024](../adrs/0024-cross-preset-transitions.md), [0132](../adrs/0132-a-rate-parameter-integrates-a-phase.md),
> [0135](../adrs/0135-every-scene-rate-integrates-through-one-shared-phase.md)
> **Closes:** design-backlog 0191; design-backlog 0142 (falsified, see below)

## TL;DR

`evaluate_preset` calls `scene.advance(dt)` before it resets the scene's parameters and applies the
preset's bindings. Any scene that integrates a bound value inside `advance` therefore integrates the
**previous** frame's value. On the frame a preset switch lands, that previous value belongs to
another preset, or is the scene's default. Two scenes do this today: `emitter` (its sprite `spin`)
and `shape_collage` (its `drift`, `spin`, `density` fades, recomposition edge and canvas rebuild).
This plan moves `advance` to after the binding walk in both evaluation functions, so every scene
integrates the values its own frame bound, and a future scene cannot get this wrong.

The plan was drafted to add a once-per-frame guard for backlog 0142 (a same-system dissolve
advancing a shared scene twice). **Reading the tree falsified that entry**: the dissolve governor
never runs a same-system dissolve live, so the double advance is unreachable in a shipped build.
This plan builds no guard. It pins the veto that makes the path unreachable, and closes the test
hatch that could build it anyway. **This reverses an interview answer, and the user confirmed it at approval on 2026-09-14.**

## Context & problem

### Backlog 0142's premise does not hold

The entry's reasoning: `scene_for_mut` resolves a scene by `SystemKind`, so both sides of a
same-system dissolve get one `Box<dyn Scene>`, and `evaluate_preset` runs twice against it per frame.
The first half is true. The second half needs the dissolve to run **dual-live**, and the governor
refuses exactly that:

- `Renderer::dissolve_mode` (`core/src/render/transition.rs`) asks `scenes::shares_resources(a, b)`
  before anything else.
- `shares_resources` (`core/src/render/scenes/mod.rs`) is `a == b || (both draw through the shared
  LineRenderer)`.
- `dual_live_eligible(true, _, _)` returns `false` unconditionally, so the mode is `Freeze`.
- Under `Freeze`, `draw_frame` in `core/src/render/mod.rs` skips `encode_outgoing_side` (it is gated
  on `Transition::is_dual_live`). The shared scene is evaluated **once** per frame, for the active
  preset.

The governor's wiring test already asserts this for one same-system pair
(`dissolve_mode_freezes_a_shared_scene_pair_and_an_unresolvable_one` in `core/src/render/tests.rs`).
The pure function's test asserts the veto holds even with frame-time headroom. `Mode::Freeze`'s own
doc calls it "the same-scene answer".

The only way to reach a same-system dual-live frame is `Renderer::begin_transition_forced`, a
`#[cfg(test)]` helper that overwrites the governor's answer. No test uses it on a shared pair today,
and nothing stops one from doing so.

The entry's other claim, that "no test reaches the dual-live render path", is also stale:
`dissolve_at` in `core/src/render/tests.rs` drives it through that same helper for an independent
pair.

**A frame-stamp guard would not help even if the path were reachable.** Both sides of a dual-live
frame encode into one command buffer and share one submission. A single scene object encoded twice
writes its uniforms twice before that submission, so both draws would read the second write. That
is the GPU half of the veto, and the reason ADR-0024 put the veto there. A guard that made the second
`advance` a no-op would still leave the two sides drawing one set of uniforms.

### Backlog 0191 holds, and its population is exactly two scenes

`evaluate_preset` (`core/src/render/evaluate.rs`) runs, in order: `set_time`, `advance(dt)`,
`chain.set_dt`, `reset_params` on the side and the scene, the binding walk, the live overrides, the
`[per_vertex]` table, then `update`. `evaluate_layer` has the same order.

ADR-0135 already records the consequence as a convention: `Phase::step` is "called from
`Scene::update`, never from `advance`, because `advance` runs before the frame's `set_param` calls".
Most scenes follow it. They store `dt` in `advance` and integrate in `update`:
`fragment_field`, `parametric`, `spectrum`, `swarm`, `warp_mesh`, `cellular`. `particles` and
`reaction_diffusion` drain a fixed-step accumulator in `advance` that reads no parameter.

Two scenes break the convention:

| Scene | What `advance` reads that a binding sets | Site |
|---|---|---|
| `emitter` | `self.spin`, integrated into `spin_integral` | `core/src/render/scenes/emitter.rs`, `fn advance` |
| `shape_collage` | `rebuild()` reads `count`, `layout`, `seed`, `size_hierarchy`, `angle_bias`, `roster`. `step(dt)` reads `drift`, `spin`, `density` and the `recompose` edge | `core/src/render/scenes/shape_collage.rs`, `fn advance` |

The backlog entry says the switch frame integrates "at the scene's defaults". That is only true for
a freshly built scene. `reset_params` runs **after** `advance`, so at `advance` time the scene holds
the values the **previous frame's** bindings wrote. On a switch those come from whichever preset last
drove that system, which may be the outgoing preset or one from much earlier. It is still one frame
of wrong rates, and still invisible as a rate. On `shape_collage` it is also one frame of wrong
recipe: a fresh capture generates the default canvas on frame 1 and regenerates the bound one on
frame 2.

**The entry's cost estimate is wrong, and it is what kept this filed.** It says reordering "changes
which frame's parameters drive which frame's integration for every preset" and moves every golden.
But a scene that stores `dt` in `advance` produces the same result whichever side of the binding
walk `advance` sits on, and so does a scene that drains a parameter-free accumulator. Only the two
scenes in the table can move. That makes the structural fix as cheap as the scene-local one.

## Decision

**0191.** `evaluate_preset` and `evaluate_layer` call `scene.set_time` and `scene.advance` **after**
the binding walk, the overrides and the `[per_vertex]` table, immediately before `scene.update`. The
`Scene::advance` doc changes accordingly: `advance` runs after this frame's parameters are applied,
so a scene may integrate a bound value there or in `update`, and both are correct.
`side.chain.set_dt` and the two `reset_params` calls stay where they are.

We rejected four alternatives:

- **Move the integration into `update` in the two offending scenes** (the scene-local fix). It is
  pixel-equivalent to the reorder, but it keeps a trait-order contract that two scenes
  already broke and the doc comment could not prevent. Holding it would need a text guard over every
  `advance` body. The reorder removes the trap instead of guarding it.
- **Reorder only on the frame a switch lands.** That is two evaluation orders chosen by a frame
  flag, which is two answers to one question (the pattern ADR-0191 retired). It also leaves the
  steady-state one-frame lag.
- **Leave it filed.** The population grows every time a rate is converted properly, as Plan 0140
  showed. The fix is cheap now.
- **A per-frame stamp making a second `advance`/`update` a no-op**, the interview's answer for 0142.
  See Context: the path is unreachable, and the guard would not make it renderable.

**0142.** No guard. `begin_transition_forced` refuses `Mode::DualLive` for a pair
`scenes::shares_resources` reports as shared and keeps `Freeze`, so a test cannot build a frame the
governor forbids. The governor wiring test is widened from one same-system pair to every
`SystemKind`. The veto is recorded in ADR-0198 as load-bearing for per-frame scene **state** as well
as GPU state. Relaxing it later (for example to let same-system dissolves animate) needs per-side
scene instances, and that is a new decision.

## Architecture diagram

```mermaid
flowchart TB
    subgraph before["evaluate_preset today"]
        A1[set_time + advance] --> A2[reset_params] --> A3[binding walk + overrides + per_vertex] --> A4[update]
    end
    subgraph after["after this plan"]
        B1[reset_params] --> B2[binding walk + overrides + per_vertex] --> B3[set_time + advance] --> B4[update]
    end
    subgraph gov["dissolve_mode (unchanged)"]
        G1{shares_resources?} -- yes --> G2[Freeze: one evaluation per frame]
        G1 -- no --> G3{headroom?}
    end
```

## Implementation phases

### Phase 1 — The shared-scene veto is pinned, and the test hatch honours it

- **Owner skill:** dev
- **What:** Widen the governor wiring test to assert `Mode::Freeze` for a same-system pair on
  **every** `SystemKind`, iterating `SystemKind::ALL` so a new system is covered when it is added.
  Make `begin_transition_forced` keep `Freeze` when the two presets' systems share resources, and
  give its doc the reason: the governor forbids that frame, and a test that builds it observes a
  defect no shipped build can have.
- **Files touched:** `core/src/render/transition.rs` (`begin_transition_forced`);
  `core/src/render/tests.rs` (`dissolve_mode_freezes_a_shared_scene_pair_and_an_unresolvable_one`,
  plus one new test for the forced helper).
- **Done when:**
  - For every `SystemKind`, two presets of that system dissolve at `Mode::Freeze`. The test builds
    its pair list from `SystemKind::ALL`, not from a literal list.
  - `begin_transition_forced(to, Mode::DualLive)` on a same-system pair leaves
    `Transition::is_dual_live()` false on every dissolve frame. On an independent pair it still
    turns dual-live on, which the existing `dissolve_at` tests exercise.
  - The existing dual-live tests (`dissolve_at` and its callers, the layered-pair reproducibility
    test) pass unchanged. Every pair they force is independent.

### Phase 2 — Every scene advances on the values its frame bound

- **Owner skill:** dev
- **What:** In `evaluate_preset` and `evaluate_layer`, move `scene.set_time(time)` and
  `scene.advance(dt)` below the `[per_vertex]` block, directly above `scene.update(frame)`. Rewrite
  `Scene::advance`'s doc to state the new contract. Add the two behavioural tests below **before** moving the lines, and record in
  the log that both fail on the unmoved tree.
- **Files touched:** `core/src/render/evaluate.rs`; `core/src/render/scenes/mod.rs` (the
  `Scene::advance` doc); tests beside the existing emitter and collage tests (or in
  `core/src/render/tests.rs` through a headless renderer, whichever reaches the integrated value). A
  `#[cfg(test)]` accessor on the **concrete** scene type is acceptable. A new `Scene` trait method is
  not (ADR-0002).
- **Done when:**
  - **The switch frame integrates the bound rate.** A fresh `emitter` preset that binds `spin = K`
    (with `K` ≠ the default) has `spin_integral == K * dt` after its first evaluated frame, not
    `DEFAULT_SPIN * dt`. Compare exactly: both sides are one product of the same two `f32` values.
  - **The switch frame builds the bound canvas.** A fresh `shape_collage` preset that binds `seed`
    (or `count`) to a non-default value regenerates its canvas **once** across its first two frames,
    and the canvas on frame 1 is the bound recipe's. Today it builds the default canvas on frame 1
    and regenerates on frame 2. Read the generation count or the recipe, not pixels.
  - Both tests fail on the tree before the lines move.
  - The latch countdown in `LatchBank::advance` is **not** touched. Its `dt.max(0.0)` belongs to
    Plan 0175, which rules on backlog 0212's three sign guards.
  - **Bless discipline.** `cargo nextest run --workspace`: the baselines that may move are
    `core/tests/golden/emitter.png`, `shape_collage.png` and `shape_collage_roster.png`, plus any
    other golden whose fixture declares `system = "emitter"` or `"shape_collage"` at the top level
    or in a `[layer]`. A moved baseline outside that set is a **stop**: it means some other scene's
    `advance` read a parameter, which contradicts the population table above. Name it in the log
    rather than blessing it. The per-preset sweeps (`sanity`, `animation`, `distinctness`,
    `reactivity`) must stay green without retuning. They are gates, and a one-frame shift should not
    cross one.

### Phase 3 — The prose follows the order

- **Owner skill:** dev
- **What:** Rewrite the code comments that argue from the old order:
  - the "Stored, not integrated" comments in `fragment_field.rs` and `lines/parametric.rs`, which
    say this frame's rate "has not been set yet";
  - `Phase`'s doc in `scenes/mod.rs`, which says integrating in `advance` "would use the previous
    frame's";
  - the emitter and collage `advance` docs;
  - any comment in `evaluate.rs` that explains `advance`'s position;
  - the negative-control comment in `a_degenerate_frame_delta_cannot_reach_a_scene`
    (`core/src/render/tests.rs`). It explains that the stretched frame "has to be" the second one
    because `evaluate_preset` advances before the bindings. After Phase 2 the reason is gone. The
    second frame still works, so keep it, and state what the test needs instead of the old order.

  A scene that stores `dt` in `advance` and integrates in `update` stays correct, so no code in those
  scenes changes. Only the stated reason does.
- **Files touched:** `core/src/render/scenes/fragment_field.rs`, `lines/parametric.rs`, `mod.rs`,
  `emitter.rs`, `shape_collage.rs`, `core/src/render/evaluate.rs`, `core/src/render/tests.rs`
  (comments only).
- **Done when:**
  - `grep -rn "set yet\|previous frame's" core/src/render/scenes` finds no comment that places
    `advance` before the bindings. These comments wrap across lines, so read each hit rather than
    counting them.
  - `node scripts/check-comment-hygiene.mjs` exits 0.
  - `cargo nextest run --workspace` moves nothing beyond what Phase 2 blessed.

## Risks & open questions

- **The user chose the guard in the interview.** This plan rejects it on evidence gathered after
  the interview. If the user still wants same-system dissolves to animate live, that is a different
  plan: per-side scene instances, which ADR-0030's mid-run allocation argument weighs against. Settle
  it at approval.
- **A scene `set_param` that depends on state `advance` just computed.** None found, since every
  `set_param` in the roster stores. If Phase 2's run turns one up, it shows as a golden outside the
  bless set, which is the stop.
- **`shape_collage`'s recomposition edge fires one frame earlier.** A `recompose` binding that
  crosses its threshold now recomposes on the frame it crosses. Collage presets bound to `beat` will
  shift their recomposition by one frame. That is the fix, not a regression, but a curator comparing
  renders across the change will see it.
- **Backlog probes.** 0191's probes match `scene\.advance\(dt\);` and `scene\.reset_params\(\);` in
  `evaluate.rs`. Both lines survive the move, so the probes stay green while the claim becomes false.
  0142's probes (`routing.rs`'s `.find`, `particles`' `spin_time.step`) are unaffected. `dev` reports
  the probe exit and leaves both entries alone. Archiving them, 0142 as falsified, is the close's
  step 3c.

## What this plan does NOT do

- **No once-per-frame guard, and no per-side scene instances.** Same-system dissolves keep freezing.
- **No change to `LatchBank::advance`** or any frame-delta guard. Plan 0175 owns backlog 0212.
- **No change to what `advance` means for fixed-step scenes.** `particles` and
  `reaction_diffusion` still drain their accumulators there.
- **No preset edits.** No preset works around the one-frame lag, so there is nothing to un-dodge.

## Implementation log

> Written by `dev` — one row per phase as that phase's commit lands, and the close block after the
> last one. **The phases above are the contract; everything here is what happened.**
> **Observations, never conclusions:** this says where to look, architect decides how it went.
> No per-criterion pass list, no self-assessment, no narrative — but a deviation from the plan or
> an unmet done-when is always disclosed. Stays shorter than `## Implementation phases` above.

**Lane:** `C:\Users\Igor Konovalov\WORK\rlx-plan-0181` on branch `plan-0181-a-scene-advances-after-its-frames-bindings`

| phase | owner | state | commit |
|---|---|---|---|
| 1 — The shared-scene veto is pinned, and the test hatch honours it | dev | done | 00c9587 |
| 2 — Every scene advances on the values its frame bound | dev | done | b4d2c15 |
| 3 — The prose follows the order | dev | done | committed with this row |

### Notes

- Phase 2: both behavioural tests (`a_fresh_emitter_integrates_its_bound_spin_on_its_first_frame`,
  `a_fresh_collage_builds_its_bound_canvas_once_on_its_first_frame`, in `core/src/render/tests.rs`)
  failed on the unmoved tree: the emitter integrated `0` against `K * dt = 0.05416667`, and the
  collage's frame-1 canvas was generated from seed `0` rather than `4242`. They reach the concrete
  scene through a test-only forwarding `Scene` wrapper swapped into the renderer's roster.
- Phase 2: `cargo nextest run --workspace` after the move was green with no bless: no golden,
  inside or outside the emitter/collage set, left its tolerance.
- Phase 3 deviation: also rewrote the comment in `WarpMeshScene::update`
  (`core/src/render/scenes/warp_mesh/mod.rs`), which said `advance` runs before the frame's
  `set_param` calls. The file is not in the phase's list; the done-when grep over
  `core/src/render/scenes` required it. The failing-frame assertion message in
  `a_degenerate_frame_delta_cannot_reach_a_scene` still says "a longer first frame" while the
  stretched frame is the second; the phase is comments-only, so it was left.

### Close triggers

- **`presets/` touched:**
- **Plan header `Closes:`**
- **What shipped:**
- **Operator docs touched:**
- **Backlog probes (`node scripts/check-backlog-claims.mjs`):**
- **Full suite:**
- **Outstanding `human` phases:**

## Followups (after this lands)
