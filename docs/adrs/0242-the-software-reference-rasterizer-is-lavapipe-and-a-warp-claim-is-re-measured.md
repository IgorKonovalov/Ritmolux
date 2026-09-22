# ADR-0242 — The software reference rasterizer is lavapipe, and a WARP claim is re-measured rather than renamed

> **Status:** accepted
> **Date:** 2026-09-20
> **Related plan(s):** [0218](../plans/0218-the-reference-machine-becomes-arch.md)
> **Related ADRs:** [0241](0241-linux-leads-and-windows-is-a-peer.md) (the stance this serves),
> [0023](0023-golden-drift-guard-uses-frozen-fixtures.md) (the drift guard),
> [0016](0016-gpu-tests-opt-in-ci-scope.md) (the skip shape),
> [0071](0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md) (a measurement names
> its machine), [0058](0058-bind-group-layout-collisions-carry-evidence.md) (the hardware-vs-software
> comparison)

## Context

Every differential test here renders through `force_fallback_adapter: true`
(`core/src/render/context.rs`), so the picture a gate judges is a **software rasterizer's**. On
Windows that resolves to **WARP**, Microsoft's DX12 software device; on Linux it resolves to
**lavapipe**, Mesa's. The mechanism is identical on both and needs no change. What changes under
[ADR-0241](0241-linux-leads-and-windows-is-a-peer.md) is which of the two a baseline was blessed
on, and therefore which machine can judge one.

The surface this touches was counted rather than estimated, on 2026-09-20, by grep over the
checkout:

- **44** committed baselines in `core/tests/golden/`.
- **274** mentions of WARP across **73** `.rs` files.
- **94** sites keying on the software adapter — the skips ADR-0016 shapes.
- **5** live reader documents naming it: `docs/testing.md`, `docs/capturing.md`, `docs/nfr.md`,
  `docs/on-device-validation.md`, `docs/roadmap-visual-richness.md`.
- A further **89** files under `docs/plans/done/` and `docs/adrs/`, which are append-only records
  and are not touched by any of this.

The trap is in the second and third figures rather than the first. Most of those 274 are not the
token "WARP" waiting to be swapped — they are **claims about what WARP does**: *"WARP mis-renders
both"*, *"blessed on WARP, so it is coverage, not evidence of correctness"*, *"the two-level moves
are the blessing adapter's"*. [ADR-0071](0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md)
says a measurement names the machine it was taken on and does not travel; a sentence about WARP
rewritten to say lavapipe is a measurement nobody took. And a skip that exists because WARP
mis-renders a pipeline set is, on lavapipe, a skip whose reason is unverified in **both**
directions — the defect may be absent, or a different one may be present.

`docs/testing.md` already records the honest version of this for the one case anyone checked:
ADR-0058's collision was found by comparing WARP against hardware, and whether a related skip can
now be lifted is *"open and unmeasured"*.

## Decision

**The software reference rasterizer becomes lavapipe, and the golden roster is blessed and gated
there.** The 44 baselines are recaptured on the Arch box; the golden suites run on the reference
adapter and **skip elsewhere**, with a printed notice in
[ADR-0016](0016-gpu-tests-opt-in-ci-scope.md)'s shape, so the Windows CI arm stops running a
comparison whose baselines are not its own rather than going red against them.

**A WARP claim is re-measured or marked, never translated.** Every one of the 274 mentions takes
exactly one of three exits, and which exit it takes is a judgement made per site:

1. **Re-measured** — the claim is checked on lavapipe and rewritten to state what was observed,
   naming lavapipe as ADR-0071 requires.
2. **Generalised** — the claim was never about WARP in particular but about *a* software
   rasterizer (a skip because no hardware is present, a "coverage, not correctness" caveat), and
   is reworded to say the class it always meant.
3. **Marked** — the claim stays as a WARP reading, with a dated note that it is unverified on
   lavapipe. This exit is legitimate and expected; it is what an honest record of a machine
   nobody runs any more looks like.

**A recaptured baseline is judged, not blessed blind.** A first baseline is compared against
nothing, which is why `warp_mesh_wide`'s was opened and judged by a person before its test was
released (Plan 0201 Phase 4b). Forty-four recaptures are forty-four first baselines. Each is
diffed against its WARP predecessor and the differences are read; a difference that is not
incidental rasterizer drift is a finding, not a bless.

## Consequences

### Positive
- The machine that judges a picture is the machine the owner is looking at. Under the alternative
  every blessing is a push and a CI artifact away.
- **The goldens become runnable on any Linux runner with no GPU at all.** lavapipe is a Mesa
  package, so the reference adapter is installable on `ubuntu-latest` — the golden roster stops
  depending on what a particular hosted runner's graphics stack happens to be.
- The re-measurement sweep is the first time anyone has read those 274 claims against a second
  rasterizer. ADR-0058's open question — whether a skip attributed to WARP is still needed — gets
  asked 94 times instead of once.

### Negative
- **This is the expensive answer, and the expense is judgement rather than compute.** Recapturing
  44 pictures is minutes; opening 44 diffs and deciding which differences are drift and which are
  defects is a person's afternoon, and it cannot be delegated to a tolerance.
- **The Windows arm loses its drift guard.** With the goldens gated to the reference adapter,
  nothing pins what Windows draws. A DX12-only regression would ship. The hardware-vs-software
  comparison ADR-0058 describes is the only instrument that would catch it, and it is run by hand.
- **The old baselines are discarded.** Their history stays in git, but the comparison they
  represent — this engine against itself on WARP, accumulated over the project's life — stops
  being the live guard, and the new corpus starts with no accumulated history at all.
- **The third exit is a licence to leave claims stale.** "Marked unverified" is honest and is also
  the cheapest exit, so most sites will take it. Nothing gates the ratio.

### Neutral
- `force_fallback_adapter` and every capture path stay exactly as they are; no test harness
  mechanism changes. What moves is the machine and the PNGs.
- ADR-0023's tolerances are stated as means and max-outliers rather than as a rasterizer's
  behaviour, so they carry across unchanged unless the recapture says otherwise — which the
  judging phase is what would say.

## Alternatives considered

### Alternative A — Keep WARP as the reference, blessed from Windows CI
Cheapest by far: no recapture, no sweep, 274 claims stay true. Rejected because it puts every
blessing and every judgement on a machine the owner no longer develops on — a baseline could only
be inspected by pushing, waiting for a job, and downloading an artifact, which is precisely the
loop that stops happening.

### Alternative B — Per-adapter baselines, both carried
A baseline per rasterizer, selected by the adapter the run resolved; nothing is discarded, and both
machines can judge. Rejected as the default because the judging cost is the dominant one and this
doubles it — every new fixture would need two blessings, on two machines, forever — and because a
selection rule in the harness is a second thing that can be wrong. It stays available **per
fixture** for a case where the two rasterizers genuinely disagree about something worth pinning.

### Alternative C — Recapture without judging
Run the bless, commit whatever lavapipe draws, move on. Rejected outright: a baseline compared
against nothing certifies nothing, and this would freeze any lavapipe mis-render into the guard
that exists to catch mis-renders.
