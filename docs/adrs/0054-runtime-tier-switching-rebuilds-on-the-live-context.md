# ADR-0054 — A runtime quality-tier change rebuilds the engine's GPU resources on the live context

> **Status:** **accepted** (2026-08-04, at Plan 0050's close)
> **Date:** 2026-07-30
> **Related plan(s):** [0050](../plans/0050-in-app-settings-and-a-browse-overlay-that-fits.md)
> **Supplements:** [ADR-0045](0045-quality-tiers-floor-and-rich.md) (quality tiers), which said the
> tier resolves once at construction. This ADR adds the one way it may move afterward.

## Context

[ADR-0045](0045-quality-tiers-floor-and-rich.md) built two named tiers and resolved them **once, at
renderer construction**, from an optional pin. Three ways to choose a tier exist today and all three
are launch-time: `--tier`, `LMV_TIER`, and `[quality] tier` in `config.toml`. The only thing that
moves a tier after startup is the frame-time governor, which demotes `Rich -> Floor` one way, once
per session.

That is a bad fit for the thing the app actually is. This is a live-show tool: the operator is
looking at the picture, on the machine, with the music playing, and that is the only moment they can
judge whether the rich tier is worth it. Telling them to quit, edit a TOML file or remember a flag,
and relaunch is asking them to make the judgement from memory. The user asked for in-app
increase/decrease directly.

The tier's values are **capacities** — particle counts, segment budgets, internal-grid caps — and
every one of them is read at resource-construction time to size a GPU buffer or texture. Nothing
branches on the tier per frame; that is deliberate, and it is exactly what makes a live change
non-trivial: there is no uniform to poke, only buffers to re-create.

Two properties of ADR-0045 are load-bearing and must survive whatever we do here. First, **headless
capture is `Tier::Floor` by construction** — `Renderer::new_headless` has no tier argument, so a
golden baseline cannot be blessed at another tier by forgetting a field. Plan 0044 landed with zero
golden re-blesses because of it. A public tier mutator on `Renderer` is precisely the hole that
guarantee was shaped to exclude. Second, the demotion latch (`tier_demoted`) is documented as
one-way and **never cleared**, so the governor's decision cannot oscillate.

## Decision

We will add `Renderer::set_tier(&mut self, tier: Tier)`, which **rebuilds the engine's
tier-dependent GPU resources on the existing `RenderContext`** — the scene roster and the composite
side are constructed afresh against the new `TierConfig`, any dissolve in flight is dropped, and the
active scene is reconfigured and re-sized. The wgpu device, queue, surface, preset roster, active
preset index, engine clock, text layer and diagnostics all survive untouched, so the operator stays
on the preset they were watching.

The call is a **no-op when the context has no surface** (`RenderContext::surface.is_none()`), which
is exactly the headless capture path. ADR-0045's by-construction guarantee therefore holds: a
capture cannot leave `Floor`, whatever a caller does.

An explicit `set_tier` **pins** the tier (`tier_pinned = true`) and **clears the demotion latch**.
The latch's meaning is "the governor took a decision the operator did not ask for, and must be told
about it"; once the operator has asked for something, that history is spent, and leaving it set
would make the frontend keep reporting a demotion that a deliberate choice has superseded.

We rejected an in-place per-scene reconfigure (Alternative A), a continuous quality scale
(Alternative B), and restart-to-apply (Alternative C).

## Consequences

### Positive

- Quality becomes judgeable where it is judged: on the machine, with the music playing. The
  operator can A/B the two tiers in a couple of seconds instead of across two launches.
- **The cost is bounded and honest.** A tier change costs one renderer-sized resource rebuild and
  loses accumulated feedback (trails, reaction-diffusion state, attractor deposit) — the picture
  re-accumulates over the next second or so rather than cutting to black. That is a visible event,
  which is the correct affordance: the operator asked for it and can see that it happened.
- ADR-0045's numbers stay the single source of tier truth. `set_tier` re-reads `TierConfig`; it
  introduces no third set of capacities and no per-frame tier branch.
- The headless guarantee is now defended by a **condition the compiler and a test can check**
  (`surface.is_none()`) rather than by every capture call site remembering not to ask.

### Negative

- **`Renderer` gains a public mutator whose correctness depends on covering every tier-dependent
  resource.** Add a tier-sized buffer to a new scene and forget to route it through the same
  construction path, and a live switch leaves a stale capacity behind — a bug that a launch-time-only
  tier could not have. The mitigation is that `set_tier` reuses `create_all` and `CompositeSide::new`
  rather than duplicating them, so a new resource is covered by construction; but a scene that caches
  a capacity outside that path escapes it.
- **The demotion latch is no longer strictly one-way.** ADR-0045's text says "never cleared"; this
  ADR narrows that to "never cleared by the governor". A reader who trusts the older sentence in
  isolation will be wrong. ADR-0045 is append-only, so this ADR is the correction of record.
- **A tier change discards a dissolve in flight.** Switching quality mid-transition cuts to the
  incoming preset rather than completing the blend. Completing it would mean rebuilding two live
  `PostChain`s and the snapshot they were blending, for a case that lasts under a second.
- Nothing here helps the foobar plugin: the C ABI grows no tier entry point, so a plugin host still
  gets whatever tier the core resolved. That is deliberate scope, not an oversight.

### Neutral

- The switch is available to the standalone shell only, because it is the only frontend with an
  operator, a keyboard and a settings surface.
- Persisting the choice is a **frontend** matter and stays in `config.toml`'s existing
  `[quality] tier`, written through the `Config::save` that already exists. `--tier` and `LMV_TIER`
  keep winning over it at the next launch, unchanged.

## Alternatives considered

### Alternative A — reconfigure each scene and post stage in place, preserving feedback

Give every scene and post stage a `set_tier`/`reconfigure` path that grows or shrinks its own buffers
while keeping accumulated content, following Plan 0029's `PipelineResources`/`FieldResources` split.
It would make a tier change invisible rather than merely brief.

Rejected on cost against benefit. Every scene, the `PostChain`, and Plan 0023's dual-live dissolve
chain would each need a correct reconfigure path, and each is a place to leave a half-resized
resource that only misbehaves on the one code path nothing captures. The thing being bought is
continuity across an action the operator explicitly asked for and expects to see — the weakest case
for that much surface area. If a future plan wants seamless quality changes, this is the shape it
takes, and this ADR is what it supersedes.

### Alternative B — replace the two tiers with a continuous quality scale

A single `0.5x .. 2.0x` capacity multiplier, so quality is a slider rather than a switch.

Rejected because ADR-0045 chose two named levels on purpose: a frame has to be predictable enough to
baseline, document, and reproduce in a bug report, and "it looked wrong at 1.3x" is not a
reproduction. Taking this would mean superseding ADR-0045 rather than supplementing it, and it would
put the golden suite's `Floor` pin on a continuum. The two tiers are also what the operator actually
asked to move between.

### Alternative C — the menu records the choice; it applies on next launch

Let the settings row write `[quality] tier` to `config.toml` and show "restart to apply". Zero
engine risk.

Rejected because it does not answer the request. The reason to change quality in the app is to *see*
the difference; a control that defers the result to the next launch is the TOML file with a nicer
front end. Worth noting it is the automatic fallback if `set_tier` ever proves unreliable on some
adapter — the config write is the same either way.

## Notes

- The related risk this ADR does **not** take a position on: `Rich`'s calibration is still the
  provisional multipliers from Plan 0044, whose Phase 4 (`human`) has not run and is carried in
  `docs/on-device-validation.md`. Making the tier switchable in-app makes that calibration easier to
  do, and also easier for an operator to run into — see design-backlog 0031, where `Rich`'s 3x
  particle count blows `attractor_clifford` out to white against the un-fixed additive ceiling. Plan
  [0045] is the structural fix; until it lands, an operator switching to `Rich` on that preset will
  see the reported defect on purpose rather than by surprise.

## Outcome (added at Plan 0050's close, 2026-08-04)

Implemented as designed in `14cd9e2`, and **operated** on 2026-08-04 in Plan 0050's Phase 6.

- **The core decision held under use.** The hitch is a brief trails re-accumulation, not a freeze, a
  hang or a device loss; it survives repeated swaps (15 consecutive in one session) and survives
  being pressed *during* a dissolve — which is the case this ADR reasoned about when it said a
  dissolve cannot survive its own chains being replaced, and where the design clears
  `incoming_side` and `transition` rather than trying to keep them.
- **The pin latches.** Every explicit change logged `(pinned)`, never falling back to `(auto)`, so
  the governor cannot demote inside an operator's measurement. That property is what makes the
  live switch a usable *instrument* and not only a convenience.
- **One design note in the plan was wrong and the implementation was right to refuse it.** The plan
  told `set_tier` to re-apply the current surface size; there is nothing stale to re-apply, because
  `render/mod.rs` calls `scene.set_target_size(...)` on the shared draw path every frame and every
  `PostStage` takes `surface` as an argument. `dev` flagged it rather than adding a no-op.
- **The first thing the live switch measured was `Rich` failing.** On the dev box the governor
  demoted `Rich → Floor` within seconds of startup, before any input. That is Plan 0044 Phase 4's
  unrun calibration answering itself the moment an instrument existed to hear it — this ADR's
  Positive claim that a tier is a *look* decision best judged on the machine, discharged on its
  first outing.
