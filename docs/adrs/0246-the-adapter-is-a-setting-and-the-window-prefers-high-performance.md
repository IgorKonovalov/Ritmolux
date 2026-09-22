# ADR-0246 — The adapter is a setting, not a launch constant: the window prefers high performance, a file key holds the choice, and it moves without a restart

> **Status:** proposed
> **Date:** 2026-09-22
> **Related plan(s):** [0224](../plans/0224-the-adapter-becomes-a-setting.md)
> **Amends:** [ADR-0155](0155-the-window-takes-the-adapter-and-the-preset-the-operator-names.md) (the
> windowed default, which that ADR declined to change)
> **Supplements:** [ADR-0146](0146-one-name-selects-the-gpu-and-each-side-matches-its-own-roster.md)
> (one name selects the GPU), [ADR-0054](0054-runtime-tier-switching-rebuilds-on-the-live-context.md)
> (a rebuild on the live context), [ADR-0240](0240-a-setting-lives-in-a-file-and-the-menu-edits-that-file.md)
> (a setting lives in a file), [ADR-0243](0243-the-reference-boxs-hardware-adapter-is-its-discrete-gpu-and-a-reading-names-it.md)
> (the reference box's adapter)

## Context

**The gap between this laptop's two adapters is the largest single frame-cost fact this project has
measured, and the default lands on the wrong side of it.** One preset, one build, one pinned `Rich`
tier, the same 2560x1440 surface, 2026-09-22 on the Arch reference box:

| adapter | fps median | `frame_ms_avg` | `frame_ms_p99` | dropped |
|---|---|---|---|---|
| AMD Radeon (RADV RENOIR, Vulkan) — what an unflagged window takes | 25 | ~40 | 79 | — |
| NVIDIA RTX 3080 Laptop (Vulkan) — `--gpu NVIDIA` | 164.9 | 6.065 | 6.3-6.5 | 0 |

The fullscreen bench rows agree within a frame (`scripts/bench/results/linux-2026-09-22-live.tsv`:
25.3 fps against 164.9), so this is not a windowing artifact. **A factor of six and a tail an order
of magnitude worse is the difference between the two adapters, not between two builds** — and the
operator reaches the fast one only by knowing a flag exists.

**[ADR-0155](0155-the-window-takes-the-adapter-and-the-preset-the-operator-names.md) declined to fix
that, and named its reason.** Its Negative section records the cost in as many words — *"The
windowed default stays the power-saving GPU on a hybrid machine, and this ADR declines to fix
that"* — because changing it *"would move every frame-time number in `docs/nfr.md` and on the
on-device checklist in the same commit that added a CLI flag, and those numbers are a measurement
question (design-backlog 0165) rather than an argument-parsing one."* **That measurement has since
been taken.** Plan 0147 Phase 6 published a matched pair in [`nfr.md`](../nfr.md) — the discrete part
at 165.0 fps median with a worst p99 sample of 8.936 ms, the integrated part at 112.8 with 21 of 171
samples under the 60 fps floor — each row naming its adapter per
[ADR-0071](0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md). The blocker
ADR-0155 was waiting on is discharged; what is left is the decision it deferred.

**Two further forces arrived after ADR-0155.**
[ADR-0240](0240-a-setting-lives-in-a-file-and-the-menu-edits-that-file.md) requires that every
user-visible choice be defined by a key in a user-editable file, with flags as a per-run override;
`--gpu` has no `config.toml` key at all, so on a hybrid machine the single most consequential choice
in the application is reachable only by retyping a flag at every launch. And the choice is fixed at
renderer construction, so an operator who discovers mid-show that the window came up on the wrong
adapter has no move but to quit — on a machine where
[ADR-0054](0054-runtime-tier-switching-rebuilds-on-the-live-context.md) already established that a
rebuild on the live context is both possible and worth having for the *tier*.

## Decision

**We will make the adapter an ordinary setting.** Three parts, one decision:

1. **The window prefers the high-performance adapter when nothing says otherwise.**
   `window_choice(None)` in `standalone/src/gpu.rs` resolves `AdapterChoice::HighPerformance`, which
   is what the `--stream` path has always asked for, so the two unflagged arms ADR-0155 held apart
   now deliberately agree. **The flip lives in the standalone, not in `core`:**
   `RendererOptions::default()` is untouched, so the foobar shim's adapter behaviour does not move
   and the C ABI does not move ([ADR-0003](0003-c-abi-v1-surface.md)).
2. **`[output] gpu` in `config.toml` holds the choice**, spelled the way `--gpu` is spelled and
   resolved through the same [ADR-0146](0146-one-name-selects-the-gpu-and-each-side-matches-its-own-roster.md)
   name-or-index rule. Precedence is `--gpu` over the key over the default, matching `--tier` over
   `[quality] tier`. **A key naming an adapter that is absent or cannot present falls back to the
   default with a startup line naming both**, while `--gpu` keeps ADR-0155's hard refusal: a file
   outlives the machine state that made it valid — an undocked laptop, a driver update, an eGPU
   unplugged — and a show binary that will not start is the wrong failure for a persisted
   preference. A flag is typed for one run and can be wrong loudly.
3. **`Renderer::set_adapter(AdapterChoice)` rebuilds the GPU state on the live context**, the
   sibling of ADR-0054's `set_tier` one level down. The retained `wgpu::Instance` yields a new
   adapter, device and queue; the window's `Surface` is re-configured against the new device, or
   re-created from the window handle the shell still owns if a backend refuses that; and every
   GPU-owning member is rebuilt. The window, the preset, the engine clock, the audio capture, the
   ring and the diagnostics survive untouched — the same set `set_tier` preserves — and any dissolve
   in flight is dropped, as there too. **Accumulated GPU state does not survive**: trail fields and
   simulation domains live in memory belonging to the device being destroyed, so a switch is visibly
   a fresh start of the picture, not a seamless handover.

The settings menu gains a row that lists the adapter roster, calls `set_adapter` and writes
`[output] gpu` — an editor of the file, per ADR-0240, exactly as the tier row is.

## Consequences

### Positive

- **The default stops being the slowest thing the machine can do.** On the reference box an
  unflagged launch goes from 25 fps to 165 with no operator knowledge required, which is the
  complaint this decision started from.
- **The choice survives the session.** A hybrid machine is configured once, in a file, rather than
  through a flag that has to be remembered at every launch — and the menu row means it is reachable
  without leaving the application, which is the pair ADR-0240 asks for.
- **A wrong adapter is recoverable in place.** An operator who lands on the wrong GPU, or whose
  machine changed under a stored key, fixes it from the menu while the show is up.
- **The `--stream` and window arms stop disagreeing.** ADR-0155 held them apart to protect published
  figures; with those figures now naming their adapters, one rule covers both paths and the
  divergence test becomes an agreement test.

### Negative

- **The discrete GPU is powered on at every launch on a hybrid laptop.** That is battery and heat
  against a project whose `CLAUDE.md` names *"low idle CPU/GPU"* as a feature. The battery-aware
  `auto` that would answer it was declined (Alternative B) and is revisitable **on top of** this
  mechanism rather than instead of it.
- **Every unflagged windowed figure this project published before this lands was taken on the
  integrated part, and a fresh run no longer reproduces it.** The rows in `nfr.md` name their
  adapter, so they remain readable as history; what is void is reading any of them as *"what you get
  by default"*. Correcting that reading is a documentation obligation of the implementing plan, not
  a footnote.
- **On Linux the panel is wired to the integrated part, so the new default renders on the discrete
  adapter and presents through a cross-GPU (PRIME) copy.** Measured affordable on this box — 164.9
  fps, zero dropped, p99 6.3-6.5 ms — and **unverified on any other Linux machine**. A weaker link
  could make the new default worse than the one it replaces, which is why the plan re-measures per
  platform and the flip is revertible per platform rather than globally.
- **A second teardown path exists beside `set_tier`, with overlapping responsibilities and no shared
  proof.** A member that is rebuilt on one path and forgotten on the other fails as a device-lost or
  a stale handle at runtime, not as a compile error. Alternative D would have unified them and was
  declined as scope.
- **A live `--stream` switch breaks the sender's contract with anything already connected.** The
  Spout sender's adapter is a correctness constraint rather than a frame-rate one
  ([ADR-0146](0146-one-name-selects-the-gpu-and-each-side-matches-its-own-roster.md)): a receiver can
  only open a sender on the GPU that renders it, so a switch re-opens the sender and a connected
  receiver drops and must re-acquire.

### Neutral

- **On Apple Silicon the mechanism is inert.** One adapter is exposed, so the preference resolves it,
  the key matches it and the menu row has one entry. Nothing regresses and nothing improves.
- **`AdapterChoice` is already public core vocabulary** (ADR-0146), so none of this widens a seam:
  no C ABI function, no control-protocol address, no `Scene` trait change.
- **The key sits in `[output]` beside `display` and `display_name`**, whose own doc comment records
  why a *name* is stored before an index — winit's monitor ordering shifts across boot and hotplug.
  The adapter roster has the same property across operating systems, so it inherits the same rule
  rather than a new one.

## Alternatives considered

### Alternative A — Keep `AdapterChoice::Default` and let the new key carry the whole cure

The file key alone would make the fast adapter reachable and persistent, with nothing published
moving underneath it. Rejected because it leaves the out-of-the-box experience at a sixth of the
machine's capability and cures it one machine at a time, by an operator who must first know that the
other adapter exists. ADR-0155's reason for holding the default was a measurement debt, and that
debt is paid; keeping the default now would be keeping a decision past its argument.

### Alternative B — A battery-aware `auto`: discrete on AC, power-saving on battery

The behaviour most people would want. Rejected for this decision because it needs a platform power
API in the standalone for each of three operating systems, and because a preference that re-decides
itself moves the picture under the operator mid-show — on a laptop that is unplugged during a set,
at the worst possible moment. **It is a candidate for a later ADR**, and the runtime switch decided
here is its precondition: without `set_adapter` an automatic policy could only act at launch anyway.

### Alternative C — Change the adapter by relaunching the process, preserving state through a file

The cheapest to build and how most applications do it. Rejected because the requirement was
explicitly a switch without an application restart: a relaunch tears down the audio capture and the
ring, drops the window off the screen and back, and on a show floor is indistinguishable from a
crash.

### Alternative D — One `Renderer::rebuild(RenderSpec { tier, adapter })` replacing `set_tier`

Architecturally the better shape: exactly one path that tears down and rebuilds GPU state, with two
reasons to call it, and one place to get the teardown right. Rejected as scope — it re-opens
accepted ADR-0054 behaviour and turns a feature into a refactor of code that works. **Revisit if the
two paths drift**, which is the specific failure the Negative section above names.

### Alternative E — The standalone drops and rebuilds the whole `Renderer`, carrying a `SessionState`

No new core method: the shell constructs a fresh renderer on the new adapter and transplants the
preset index, clock, rotation timer and A/B hold. Rejected because anything not named in that
carrier is silently lost — a failure with no compile error and no test that would notice — and
because re-attaching the console's auxiliary surface and the sender would become the shell's problem
in three separate places rather than the renderer's in one.

## Notes

The readings behind the Context table are in `scripts/bench/results/` (the `linux-2026-09-22-*` and
`windows-2026-09-22-*` files) and in this project's `diagnostics.log` for 2026-09-22; the bench's own
method and caveats are in [`scripts/bench/README.md`](../../scripts/bench/README.md). The Windows
half of that comparison is **not** evidence for this decision: its two adapters were read without the
tier being recorded, which is the flaw [ADR-0245](0245-an-internal-grid-is-a-fraction-of-the-target-resolved-per-tier-and-adapter-class.md)
also had to work around. Only the same-box, same-tier, same-surface pair above is load-bearing here,
and both halves of it are Linux.

[Backlog 0165](../design-backlog.md) is the history of this question and stays live for its own
remaining half (the console's dual-GPU degrade branch, still never executed); this decision takes the
default and the mechanism, not that branch — though Plan 0224's console phase is the most likely
thing yet to fire it.
