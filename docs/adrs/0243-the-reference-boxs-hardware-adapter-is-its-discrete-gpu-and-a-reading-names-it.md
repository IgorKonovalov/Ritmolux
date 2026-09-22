# ADR-0243 — The reference box's hardware adapter is its discrete GPU, and a reading names it

> **Status:** proposed
> **Date:** 2026-09-22
> **Related plan(s):** [0219](../plans/0219-the-arch-box-builds-tests-and-runs-every-lane.md),
> [0218](../plans/0218-the-reference-machine-becomes-arch.md)
> **Related ADRs:** [0241](0241-linux-leads-and-windows-is-a-peer.md) (Linux leads),
> [0242](0242-the-software-reference-rasterizer-is-lavapipe-and-a-warp-claim-is-re-measured.md)
> (the software reference), [0071](0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md)
> (a measurement names its machine), [0016](0016-gpu-tests-opt-in-ci-scope.md) (the skip shape)

## Context

ADR-0241 makes an Arch box the machine this project is judged on, and ADR-0242 settles its
**software** adapter: lavapipe, for every picture a gate compares. Neither says which **hardware**
adapter a hardware reading is taken on. On the Windows box that question never came up, because
there was one GPU.

The Arch box has two. `lspci` on 2026-09-22 lists an **NVIDIA GeForce RTX 3080 Laptop GPU**
(GA104M, proprietary driver `nvidia-open-dkms` 610.57.04) and an **AMD Radeon Vega** iGPU (Cezanne,
Mesa RADV via `vulkan-radeon` 26.2.2). Both have Vulkan ICDs installed, so a Vulkan build of wgpu
sees two hardware adapters where the Windows box saw one.

Three things depend on which one a run picks:

- **The hardware-only tests.** Eleven call sites go through `headless_hardware*` and skip when only
  a software adapter exists: four in `suite/layer.rs`, the five `*_cost.rs` timing tests,
  `background_composite` and `reaction_diffusion`. Several of those assert timings or ratios, and a
  timing taken on a Vega iGPU and one taken on a 3080 are not the same measurement.
- **Every frame-time reading.** `docs/nfr.md`'s budgets, Plan 0207's frame-cost column and a live
  rehearsal's `diagnostics.log` all report a number from one adapter.
- **Continuity with what came before.** The Windows box's readings were taken on a desktop RTX 3080.
  The laptop part shares a name and not a power envelope, but it is the nearest thing this project
  has to a like-for-like comparison.

The engine already leans one way. `core/src/render/context.rs` asks wgpu for
`PowerPreference::HighPerformance`, and its doc comment says that means the discrete GPU on a hybrid
machine. So the default pick is already the NVIDIA part. What is missing is the decision that this
is *the* adapter, and the rule that stops a reading from leaving out which adapter took it.

## Decision

**The reference box's hardware adapter is its discrete NVIDIA GPU, reached by the engine's existing
`HighPerformance` preference with no environment override, and every hardware reading names the
adapter and driver that took it.** A hardware test that runs on the box resolves the dGPU. A reading
that resolved anything else, such as the iGPU after a driver update or a session where the dGPU is
powered down, is not a reference reading. It is recorded as a reading of the adapter it actually
took, and never as a failure or a pass of the reference. The performance floor is unchanged:
`docs/nfr.md` §9 already assigns it to a separate, older machine, and this ADR does not move it onto
the iGPU.

Naming the adapter means `adapter.get_info()`'s `name` and `driver_info`, printed next to the
number. That is ADR-0071's rule applied to a machine with two answers to "which GPU".

## Consequences

### Positive
- Timings keep a lineage. A frame cost measured here can be put beside a Windows-box reading with a
  stated caveat (laptop part, different power limit), instead of beside a number from a different
  class of GPU.
- No new mechanism. The preference is already in `context.rs`; the decision makes it load-bearing
  instead of incidental.
- A misrouted run is visible, not silent. On a hybrid laptop the likeliest failure is a run quietly
  landing on the iGPU. Because every reading names its adapter, that shows up in the output rather
  than in a mysteriously slower number.

### Negative
- **The dGPU is the least predictable part of the box.** It is power-managed, its driver is
  out-of-tree, and it is updated by `pacman -Syu` on the owner's schedule, not the project's. A
  driver update can move a timing, and it will look like a code change unless the driver version in
  the reading is compared.
- **The live window presents across two GPUs.** On a hybrid laptop the internal panel is usually
  wired to the iGPU, so a frame rendered on the dGPU is copied across (PRIME) before it is shown. A
  frame time read on the internal panel and one read on an external display wired to the dGPU may
  differ for reasons that have nothing to do with the engine. Plan 0218's rehearsal has to say which
  display it read from.
- **The iGPU gets no standing.** A defect that shows only on RADV is caught by nothing on this box
  unless someone deliberately runs there.

### Neutral
- Software-adapter work is untouched. Goldens and every `force_fallback_adapter` path stay on
  lavapipe per ADR-0242. This ADR is about the hardware half only.

## Alternatives considered

### Alternative A — The AMD iGPU as the hardware reference
It is always powered, it is on an in-tree Mesa driver, and it sits closer to the performance floor.
Rejected because none of the project's history is comparable to it, and because the floor already
has a machine of its own. Treating the iGPU as a floor stand-in would give a second floor that
agrees with neither.

### Alternative B — Both adapters, each named, both required
Every hardware test and reading runs twice. Rejected because it doubles the hand-validation surface
that ADR-0241's Negative section already calls fragile, for a class of defect (RADV-only) that no
report has ever shown.

### Alternative C — Select the adapter by environment (`DRI_PRIME`, `__NV_PRIME_RENDER_OFFLOAD`)
Rejected because it puts the choice in the shell rather than in the engine. A run started without
the variable would pick an adapter nobody chose, and a reading would depend on how it was launched.
The engine's own preference already expresses the choice.
