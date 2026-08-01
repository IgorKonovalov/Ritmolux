# ADR-0058 — Two bind-group layouts that can be live in one frame may not share a shape unless an allowlist carries per-pair evidence

> **Status:** proposed
> **Date:** 2026-08-01
> **Related plan(s):** [0053](../plans/0053-the-suite-stops-blessing-what-warp-gets-wrong.md)
> **Supplements:** [0016](0016-gpu-tests-opt-in-ci-scope.md) (the adapter policy that puts the
> whole golden suite on WARP). **This ADR is the first record of the WARP aliasing hazard itself** —
> see the Context; the hazard has been cited for five plans as "ADR-0021 / Plan 0020", and that is
> the shared palette system.

## Context

The DX12 **WARP software adapter** hands a pipeline whose bind-group layout matches another live
one *the other pass's resources*.

**The hazard has no record, and has been miscited for five plans.** Four code comments
(`tonemap.rs:234`, `tonemap.rs:977`, `bloom.rs:66`, `bloom.rs:430`, `gpu.rs:168`) and
[design-backlog 0039](../design-backlog.md) attribute it to "ADR-0021 / Plan 0020".
[ADR-0021](0021-shared-palette-system.md) is the **shared palette system** and says nothing about
adapters, layouts or WARP; Plan 0020 is its implementation. So the most expensive test-fidelity
hazard in this codebase has been documented only in comments pointing at an unrelated decision.
Correcting those citations is part of this ADR's plan, and this ADR is where the hazard now lives.

What is actually known about it, from Plan 0020's era onward and reproduced twice in Plan 0045, to
the byte: the tonemap was fed the kaleidoscope's uniform
(`exposure` became `kaleido_order = 6.0`), then the backdrop's (`bg_hue`, `bg_bright`), and the
bloom blur passes behaved as though handed the vertical pass's buffer.

**Every one of those was invisible on hardware.** That is the part that makes this a decision
rather than a bug: per [ADR-0016](0016-gpu-tests-opt-in-ci-scope.md) the whole golden suite
captures on WARP, so a mis-render there is not caught — it is **blessed**. The baseline becomes a
committed picture of the wrong thing, and every later run agrees with it.

Plan 0045 Phase 4b replaced a prose uniqueness claim in `tonemap.rs` with an enumeration over every
`create_bind_group_layout` call in `core/src` — 23 layouts; `standalone/` and the plugin add none.
It asserts on the tonemap alone and **prints three collision groups it asserts nothing about**:

| shape | held by |
|---|---|
| `[Uniform, Texture, Sampler]` | `ink-bind-layout`, `kaleido-bind-layout` |
| `[Texture, Sampler]` | `attractor-present-layout`, `trails-present-bind-layout` |
| `[Uniform]` | `background`, `disc`, `fragment-field-uniform`, `renderer.rs` (per-scene), `rd-init`, `swarm` |

The test's docstring calls these "older and deliberate". **Deliberate is a claim with no record
behind it** — and that is exactly the failure mode Phase 4b existed to retire, since the comment it
replaced made the same kind of claim and was false: `attractor-decay` had held the tonemap's
shipped shape all along.

**One pair is live together on shipped content today.** `attractor_clifford.toml` and
`attractor_leviathan.toml` bind `trails` on the attractor, which puts `attractor-present` and
`trails-present` — both `[Texture, Sampler]`, both plain blits — in the same command buffer. No
golden fixture covers that combination (`core/tests/fixtures/attractor.toml` binds no trails), so
the only thing rendering it on WARP is the preset behavioural suite, whose floors are coarse.
`ink` + `kaleido_*` is the same shape of risk with no shipped preset binding both *today* — nothing
stops the content lane writing one tomorrow.

Nothing is observed to be wrong. Hardware is unaffected. This is a **test-fidelity** hazard, and it
is pre-existing — Plan 0045 surfaced it, it did not cause it.

## Decision

**We will assert that no two bind-group layouts which can be live in one frame share a shape,
except where an explicit allowlist entry carries the evidence that the pair is safe.** An allowlist
entry is not a suppression: it names the pair, and it records the *measurement* — the same
configuration rendered on the hardware adapter and on WARP, compared — that establishes the pair
does not alias in practice.

A pair with no entry fails the test. An entry with no recorded evidence is not an entry.

Where a pair is cheap to separate, separating it is preferred to allowlisting it — a layout that
cannot collide needs no evidence and no maintenance. The allowlist exists for the case where
separation is the worse cure: six single-uniform groups is the natural shape for a fullscreen pass,
and reshuffling all of them to dodge an aliasing bug in one software adapter would distort the code
to satisfy a test.

## Consequences

### Positive

- **The claim becomes checkable.** "Older and deliberate" stops being prose and becomes either a
  measurement or a failure. This is the same move Phase 4b made for the tonemap, applied to the
  three groups it left unasserted.
- **A new pass cannot quietly join a collision group.** Adding a `[Uniform]` layout today is
  invisible; after this it fails until someone either separates it or measures it. That is the
  property worth having, since the collision groups grow every time a stage is added — and
  [Plan 0052](../plans/0052-the-emitter-objects-that-spawn-fall-and-die.md)'s emitter is about to
  add one.
- **The riskiest pair gets covered where it actually ships.** `attractor-present` +
  `trails-present` is live on two shipped presets and no fixture renders it; the plan adds one.
- **The evidence is durable.** A recorded hardware-vs-WARP comparison outlives the session that ran
  it, which is precisely what the three prose claims before it did not.

### Negative

- **"Can be live in one frame" is not decidable from the layout list alone.** It depends on which
  stages a preset composes, and the honest approximation is "any two layouts in `core/src` that a
  preset could put in one chain" — which is nearly all of them. The assertion is therefore coarser
  than the real property, and it will flag pairs that no preset actually combines. Allowlisting
  those costs a measurement each.
- **Gathering evidence needs two adapters, and one of them is not on CI.** ADR-0016 keeps GPU tests
  opt-in precisely because a hardware adapter is not guaranteed. So the evidence is produced by a
  human on a machine with a discrete GPU, recorded in the allowlist, and thereafter trusted. That
  is a weaker guarantee than a test that re-derives it, and it is the honest cost of the hazard
  living in a software adapter.
- **The allowlist can rot.** A pair measured safe today can alias tomorrow after an unrelated
  change to either pass. The entry records when it was measured; it does not re-measure itself.
- **It fixes nothing that is currently broken.** No mis-render is known to be shipping. This buys
  the ability to notice, at the cost of machinery and of a `human` phase.

### Neutral

- The tonemap's existing single-layout assertion is subsumed rather than replaced; it becomes the
  first entry in the general form.

## Alternatives considered

### Alternative A — reshuffle every colliding layout so no two shapes match

Add a dummy binding, or reorder entries, until all 23 layouts are pairwise distinct. Needs no
allowlist, no evidence, and no `human` phase — the property becomes structural. **Rejected for the
`[Uniform]` group specifically, which is six of the eleven colliding layouts.** A single uniform
buffer is the correct and minimal shape for a fullscreen pass; padding six of them with a dummy
binding to work around an aliasing bug in one software adapter distorts shipping code to satisfy a
test, and every future fullscreen pass inherits the distortion. For the two smaller groups
separation may well be the right answer, which is why the decision prefers it where it is cheap
rather than forbidding it.

### Alternative B — stop capturing the golden suite on WARP

The hazard exists because ADR-0016 puts the suite on a software adapter. Capture on hardware and
the aliasing disappears. **Rejected because it trades a narrow, checkable hazard for an unbounded
one:** goldens would then be pinned to whichever GPU the developer happened to have, and the suite
would stop running at all on a machine without one. ADR-0016's reasoning is unchanged.

### Alternative C — assert the collision groups are empty, with no allowlist

The strict form: no two layouts in `core/src` may share a shape, full stop. **Rejected because it
is Alternative A with the work deferred** — it fails on day one against eleven layouts and the only
way to green it is the reshuffle already rejected, done under time pressure.

### Alternative D — leave it; document the hazard and move on

It is what the codebase does today, it costs nothing, and nothing is known to be broken.
**Rejected because the same reasoning already failed three times.** The hazard *was* documented —
in four code comments, which cited the wrong ADR for five plans, so nobody following the reference
would have found anything. The tonemap comment documented a uniqueness claim, and the claim was
false while the documentation sat there reading as reassurance. And the collision groups are
printed by a test today that asserts nothing about them. Documentation of a hazard a test could
check is precisely how this class of defect keeps shipping.

## Notes

`bloom.rs`'s module docs make the same prose uniqueness claim for its four layouts (bright, blur,
up, mix). The existing enumeration's printout shows it holds today, so converting that claim from
prose to assertion is a few lines and is worth doing while in the file — a cheap instance of
exactly what this ADR is about.
