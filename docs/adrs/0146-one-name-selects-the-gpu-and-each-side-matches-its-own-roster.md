# ADR-0146 — One name selects the GPU, and each side matches its own roster

> **Status:** accepted 2026-08-30
> **Date:** 2026-08-29
> **Related plan(s):** [0115](../plans/done/0115-the-engine-becomes-a-live-video-source.md)
> **Refines:** [ADR-0125](0125-the-live-video-out-is-a-spout-sender-fed-by-a-frame-tap.md)

## Context

Plan 0115 Phase 3 found the platform fact that decides whether the live video-out works at all: a
Spout sender shares a D3D11 texture **by handle**, the receiver opens that handle on its own device,
and the open succeeds only when both devices are the same physical GPU. On a hybrid laptop —
`[0] AMD Radeon(TM) Graphics` integrated, `[1] NVIDIA GeForce RTX 3080 Laptop GPU` discrete — Windows
hands a plain console process the integrated GPU to save power while TouchDesigner runs on the
discrete one, and the receiver reports only `Unable to open shared Spout Texture`, naming neither
adapter nor the mismatch. Pinning the sender to adapter 1 made the picture appear. `lmv_spout_create`
therefore takes an adapter index, and `lmv_spout_adapter_count` / `lmv_spout_adapter_name` enumerate
so a caller can name the choices.

**That finding was read one step further than the evidence carries, and the difference decides this
ADR.** `standalone/examples/spout_probe.rs` — the instrument that produced it — uses **no `wgpu` at
all**: it generates a reference image on the CPU and hands the bytes to `SpoutSender`. So what Phase
3 established is that **`spoutDX`'s** D3D11 device must sit on the receiver's GPU. It establishes
nothing about ours, because ours was not in the experiment.

Nor is it in the frame path. On the CPU pixel path ADR-0125 chose, a frame goes from our `wgpu`
device to system RAM by readback, then into `spoutDX`'s own D3D11 device by `UpdateSubresource`,
and it is *that* device which creates and shares the texture. Our adapter never touches the shared
resource. ADR-0125's *"Spout's D3D11 device is entirely its own and never meets ours"* is intact,
and it stays intact exactly as long as the CPU path does: the day zero-copy sharing (ADR-0125
Alternative A) lands, our texture becomes the shared one and the two devices must genuinely be one
GPU. Until then they are not coupled.

So there are two constraints, and they are different in kind:

| | what it constrains | what goes wrong |
|---|---|---|
| **`spoutDX`'s adapter** | correctness | the receiver cannot open the texture, and says so uselessly |
| **`wgpu`'s adapter** | frame rate | a 60 fps source renders on the power-saving integrated GPU |

**On the development machine these resolve to the same GPU, which is why nothing here can tell them
apart.** The receiver runs on the discrete GPU, and the discrete GPU is also the fast one — so a
single correct answer satisfies both constraints and no observation on this box distinguishes "must
match the receiver" from "should be fast". This is the shape
[ADR-0037](0037-internal-grid-is-a-resolution-not-a-shape.md) is about, one level up from geometry:
two sources that agree on the one configuration we develop at.

The decision is what surface `--stream` carries, given that. Naively the two knobs must be made to
**agree**, which means matching an adapter across two APIs that share no identifier `wgpu` exposes —
`wgpu::AdapterInfo` carries `name`, `vendor` and `device`, DXGI carries a `Description`, PCI IDs and
an `AdapterLuid`, and only the first is available on both sides today without widening the shim.
That is real work, and it is work for a coupling that does not exist yet.

## Decision

`lmv --stream` takes **one `--gpu <name>` flag naming a graphics adapter, and each side matches that
string against its own roster** — the renderer against `wgpu::Instance::enumerate_adapters`, the
Spout sender against `lmv_spout_adapter_name`. The operator's string is the common key. **No adapter
identity is matched across the two APIs**, because none needs to be: the two lookups are
independent, and a name is what an operator can actually supply.

With no flag, the mode still resolves both. The renderer requests
`PowerPreference::HighPerformance`, which is the correct default for a live source on any machine
and is the discrete GPU on a hybrid one. The sender then **follows the renderer by name** —
best-effort substring match of the renderer's adapter description into the DXGI roster — and falls
back to the D3D11 default when that match fails.

**The mode prints both resolved adapters by name at startup, always, and warns when the machine has
more than one and no `--gpu` was given.** It proceeds rather than refusing: this is a headless
show-night source, and a mode that will not start unattended is its own hazard. The warning names
the symptom to expect, because the receiver's own message cannot.

Selecting the renderer's adapter is a **`core` change and not a C ABI change**: the headless context
constructor grows an adapter preference expressed in `wgpu`'s own vocabulary — a power preference, a
name, or the existing software-fallback request. That names no platform type and no vendor, so the
GPU-abstract rule ([ADR-0001](0001-rust-core-wgpu-cabi-foobar-shim.md)) holds; `LMV_ABI_VERSION`
does not move ([ADR-0003](0003-c-abi-v1-surface.md)).

## Consequences

### Positive
- **One operator-facing concept.** *"The GPU TouchDesigner is on"* is one string, which is the model
  the operator already has. Two rosters resolving it is an implementation detail they never see.
- **The hard problem is not built.** No LUID plumbing, no PCI-ID matching, no widened C shim, no
  backend pinning — none of which buys anything while the frame path runs through system RAM.
- **The default works unattended on this machine.** `HighPerformance` plus a name-matched sender
  resolves to the RTX 3080 on both sides with no flag, which is the configuration the user runs.
- **The renderer stops being handed the integrated GPU.** That is a frame-rate defect nothing in
  Plan 0115 was going to catch, since Phase 6 measures the finished mode and would have reported the
  slow number as the engine's cost.
- **Both adapters are printed, so the silent failure gets a voice.** The receiver's
  `Unable to open shared Spout Texture` names nothing; our stderr names both choices and the flag
  that changes them.
- **The `human` gate keeps its instrument.** `spout_probe.rs` already prints the Spout roster and
  takes an index; the mode is the same behaviour with a name instead of an integer.

### Negative
- **A name match is a heuristic, and it can miss.** `wgpu`'s DX12 backend reports the DXGI
  description and its Vulkan backend reports `VkPhysicalDeviceProperties::deviceName`; the two are
  usually the same string and are not guaranteed to be. When the match fails the sender silently
  reverts to the D3D11 default, which on a hybrid box is the wrong GPU — so the fallback **must**
  announce itself, and the flag is the guaranteed escape hatch rather than a convenience.
- **A substring can be ambiguous.** Two adapters whose descriptions share a substring, or a machine
  with two identical GPUs, cannot be separated by name at all. The roster print is what makes that
  legible, and an index remains accepted alongside a name for exactly this case.
- **This decision expires when zero-copy lands.** ADR-0125 Alternative A makes our texture the
  shared one, at which point the two adapters must be one adapter and a name match on two rosters is
  no longer sufficient — that is a genuine identity problem and it will want the LUID this ADR
  declines to plumb. The exit is stated so the next reader does not mistake this for a permanent
  answer.
- **`core`'s headless constructor grows a parameter** that every existing caller — the golden
  harness, `shot`, the QA tests — must pass. They all want the same value they have today, so the
  churn is mechanical, but it is churn in the one constructor every capture path goes through.
- **Nothing in CI can see any of it.** The GitHub runners have one adapter, and the hybrid-laptop
  behaviour this ADR exists for is unreachable there. Every claim here is a human reading on one
  machine, which is the same standing limitation ADR-0125 already records for this whole mode.

### Neutral
- Two GPU drivers resident in one process, should the renderer and the sender ever land on different
  adapters, costs roughly a second copy of the driver floor
  ([ADR-0010](0010-accept-gpu-driver-memory-floor.md)). It is a reason to prefer agreement, not a
  reason to enforce it.
- `--gpu` is a `--stream` flag and not a global one. The window has a surface, and a surface already
  constrains adapter selection through `compatible_surface`; widening the flag to the windowed app
  is a separate question this does not answer.

## Alternatives considered

### Alternative A — two independent flags (`--gpu` and `--spout-adapter`)
The honest surface for two independent lookups: name the renderer's adapter, index the sender's.
It lost on the failure mode rather than on the model. The two knobs must agree in practice, nothing
checks that they do, and the symptom of disagreement is a receiver-side message that names neither
adapter — so the operator debugs a working sender against a working receiver with no instrument
pointing at the seam. One name that feeds both cannot produce that state.

### Alternative B — index parity between the two enumerations
`wgpu`'s DX12 backend and `spoutDX` both enumerate through DXGI, so adapter *i* is very likely the
same GPU on both sides, and the whole problem dissolves into passing one integer twice. Rejected
because it is true by coincidence of backend: it holds only while `wgpu` selects DX12, breaks
silently the moment it selects Vulkan, and would break in the way that costs a debugging session
rather than an error. It is also unfalsifiable on this machine, where the two orders agree.

### Alternative C — match by PCI IDs or LUID, through a widened shim
`DXGI_ADAPTER_DESC` carries `VendorId`, `DeviceId` and `AdapterLuid`; `wgpu::AdapterInfo` carries
`vendor` and `device`, which are the PCI IDs on both the DX12 and Vulkan backends. Two more shim
entry points would make the match exact rather than heuristic. Rejected as the right answer to a
question not yet asked: it plumbs a hard identity through the C seam to enforce an agreement that
buys nothing while the frames travel through system RAM, and it widens the C++ surface Phase 3
deliberately kept at six functions. **This is the alternative that wins the day zero-copy lands**,
and it is recorded here so that day does not start from scratch.

### Alternative D — pin `--stream` to the DX12 backend and select by DXGI index
Forcing `Backends::DX12` makes Alternative B true by construction rather than by luck. Rejected
because it pins a backend for the whole mode to solve an adapter question, contradicting ADR-0001's
*"write to wgpu; don't branch on the backend"* — and it would make the streaming path the one place
in this engine that renders on a different backend from the window, which is a correctness surface
of its own for no gain.

### Alternative E — select the sender's adapter only, and leave the renderer alone
The smallest change, and it is complete for correctness: the picture arrives. Rejected because it
leaves the renderer on whatever Windows hands a console process, which on this machine is the
integrated GPU. Phase 6 measures *"the largest size and frame rate that hold steady"* and would have
reported the integrated GPU's number as this engine's cost, with nothing in the reading to say
which GPU produced it.

## Notes

The claim that our `wgpu` device is absent from the frame path is read from
`standalone/src/spout/shim.cpp` and the SDK at the pinned tag 2.007.017: `lmv_spout_send` is one
`spoutDX::SendImage`, which after `CheckSender` is a single `ID3D11DeviceContext::UpdateSubresource`
of the caller's bytes into `spoutDX`'s own shared texture, followed by a `Flush`. There is no other
device in that call.

`wgpu::Instance::enumerate_adapters(Backends)` exists at the pinned `=30.0.0` and returns a future
resolving to `Vec<Adapter>`, so both halves of the name lookup are available without a new
dependency.

## Outcome — 2026-08-30, at Plan 0115's close

The decision stands and shipped: `--gpu` resolves both sides, the control run separates them
(`"Radeon"` gives an empty texture in the receiver, `"RTX 3080"` gives the picture), and the
fallback prints rather than reverting silently. Three findings outlived the plan.

- **The heuristic held, and the enumerations are demonstrably independent.** Both rosters print
  byte-identical names for the two shared adapters. They are **not** the same enumeration: wgpu
  lists **three** (the two plus `Microsoft Basic Render Driver`) against the sender's **two**.
  Positions 0 and 1 happen to agree here, so nothing on this machine would catch an index handed to
  the wrong API — the extra entry is what proves the orders are independent, and it is the reason
  this ADR matches by name and not by index.
- **The byte-identity the no-flag default rests on is not what the code actually tests.**
  `AdapterDescription` was introduced with two fields for exactly this reason — `name` is the match
  key, `detail` adds backend, device type and driver "and would wreck a match" — but `follow_renderer`
  is handed `Renderer::adapter_description()`, which is the **detail** string. Its exact-equality
  arm is therefore structurally unreachable, and the resolution succeeds only through the
  reverse-containment tolerance beneath it. It resolves correctly on this machine and on any
  machine where the detail string contains the bare name, which is every machine wgpu describes
  this way — but the match is looser than this ADR designed, and no test can distinguish the two
  arms while the wrong string is passed. **The repair is to carry the bare name alongside the
  description on `RenderContext` and match against that**; it is filed as a followup, not fixed at
  the close.
- **The instrument gave a false negative before it gave a verdict.** A Spout receiver that loses
  its sender keeps presenting the last texture it received, so a static test image makes a live
  feed and a frozen frame identical. The probe now stamps a stepping liveness marker. Three
  symptoms mean three different things and are worth knowing apart: an **empty texture** is the
  wrong GPU, **`No Active Sender Found`** is the wrong sender name, and a **correct-looking
  picture** may be either live or frozen.
