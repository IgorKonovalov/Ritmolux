# ADR-0067 — Coverage measures the scene, not the backdrop

> **Status:** proposed
> **Date:** 2026-08-03
> **Related plan(s):** [0058](../plans/0058-the-gate-can-see-an-empty-frame.md)
> **Supplements:** [ADR-0062](0062-clamp-occupancy-is-the-saturation-instrument.md) (the previous
> "an instrument reports a picture is alive when it is not" decision — same class, different layer),
> [ADR-0049](0049-analysis-v2-dual-resolution-axis-normalized-bands.md) (the normalization whose
> blast radius this failed to contain), [design-backlog 0053](../design-backlog.md).

## Context

`core/tests/sanity.rs` asks three questions of every shipped preset — is something there
(`coverage`), is it more than a dot (`quadrant_spread`), does it have an interior
(`tonal_flatness`). All three are built on one predicate:

```rust
fn is_lit(px: &[u8], bg: [u8; 4], eps: u8) -> bool   // core/src/render/metrics.rs
```

and the `bg` the gate hands it is **pixel (0, 0)** — the top-left corner of the frame.

`bg_vignette` darkens the frame toward its edges, so on any preset that sets one **the corner is the
darkest pixel in the image** and essentially every pixel toward the centre differs from it by more
than `eps = 10`. The backdrop therefore reads as a large, well-spread, lit figure. **24 of the 35
shipped presets bind both `bg_vignette` and `bg_bright`**, and the coverage floor for every sparse
system — swarm, all three line families, attractor, spectrum — is **0.01**. For those presets the
floor is satisfied by the backdrop alone, whatever the scene does. It is not a weak gate; it is an
unfalsifiable one.

This is not hypothetical. `spectrum_ridge` shipped with `scale = 3.20`, a world height per unit of
band level chosen before [ADR-0049](0049-analysis-v2-dual-resolution-axis-normalized-bands.md)
normalized the bands to `0..1`. Afterwards the same constant multiplied a value roughly five times
larger, putting a driven element about **3.3 world units** up against a visible half-height of
`1.0`. Rendered under `--signal noise:7` the frame came back **empty except the vignette**. The gate
scored it healthy for that preset's entire broken life, and the `tonal_flatness = 1.000` that
[design-backlog 0052](../design-backlog.md) was raised about **was the vignette**, not the preset —
so the flat-frame statistic [ADR-0062](0062-clamp-occupancy-is-the-saturation-instrument.md) bought
convicted the right preset for the wrong reason.

The root confusion is a category error. `coverage` exists to ask *did the scene draw something*. The
backdrop is an **engine stage** (`core/src/render/background.rs`), not the scene, and it is
composited underneath every scene identically. Measuring the scene against a reference that includes
the backdrop means the metric can never distinguish "the scene drew a figure" from "the engine drew
a gradient".

## Decision

**The `sanity` capture renders with the background stage at its defaults, and `is_lit` compares
against black.** A preset's `bg_*` bindings are not applied for this capture, so "lit" means "the
scene put light here".

This needs no new shader path and no engine capability. `background.rs` already has
`DEFAULT_BRIGHT = 0.0` and `DEFAULT_VIGNETTE = 0.0`, so suppressing the backdrop is exactly *not
applying three bindings* — the stage renders the black it renders for any preset that never
mentions `bg_*`.

Because this changes what every number means, **the per-system coverage floors are re-measured from
the shipped library in the same change**, and recorded on the constant the way ADR-0062's occupancy
threshold and the tonal-flatness ceiling already are. A floor derived from inflated numbers is not a
floor.

## Consequences

**Positive**

- `coverage` means what its name says, and `quadrant_spread` and `tonal_flatness` inherit the fix
  for free, because all three share `is_lit`.
- **A figure that has left the frame becomes detectable at all.** That entire defect class was
  invisible, and it is the class ADR-0049's blast radius put presets into.
- The floors become falsifiable, so tightening them becomes a meaningful act. Today raising `0.01`
  would mostly measure the backdrop.
- It composes with ADR-0062 rather than duplicating it. Occupancy catches a binding that stopped
  varying; this catches a binding that varies itself off-screen. Both are "the picture is not what
  the numbers say".

**Negative**

- **The gate stops testing the composite as shipped.** A preset whose look genuinely depends on
  figure-over-backdrop interaction is now judged without that interaction. Accepted: the golden
  baselines cover the composite pixel-exactly and are the right instrument for it, while `sanity` is
  a per-preset liveness check and liveness is a property of the scene.
- **Every floor number moves and must be re-measured**, and they are measured constants with a shelf
  life — one more table to re-derive when the library changes materially.
- A preset that draws *only* through the backdrop would newly fail. None exists today, and such a
  preset arguably should fail a scene-liveness gate, but it is a real behaviour change rather than a
  pure tightening.
- `sanity` diverges from what the operator sees, which is a standing cost of every synthetic
  stimulus this project uses and is worth restating rather than discovering again.

## Alternatives considered

**Harden the background sampler** — keep the rendered backdrop and estimate it better: take the
median of the four corners, or fit and subtract the radial vignette. **Rejected because it treats
the symptom rather than the definition.** The defect is not that the reference pixel is badly
chosen; it is that a lit backdrop is not a figure and no reference pixel makes it one. A fitted
model would also have to keep pace with every future backdrop shape (`bg_hue` gradients today, and
[Plan 0046](0046-transformed-feedback.md)'s transformed feedback will add more), and it degrades
silently when it stops fitting — the exact failure mode being fixed.

**An in-frame geometry fraction** — ask the scene what share of its geometry landed inside the
render target. **Rejected as the primary mechanism, and it is the strongest of the three.** It names
the actual defect ("the figure left the frame") instead of inferring it from pixels, and it would
catch an off-frame figure even against a black backdrop. It loses on reach and on cost: it needs a
new accessor on or beside the `Scene` trait — a seam [ADR-0002](0002-scene-trait.md) deliberately
keeps thin — and it only works for scenes that build a CPU-side geometry list. `fragment_field`,
`reaction_diffusion` and `attractor` draw no segment list at all, and `attractor` is where the next
instrument is already wanted ([backlog 0031](../design-backlog.md)). Worth revisiting as a
*supplement* for the line and spectrum families if the pixel measure proves too blunt.

**Leave the metric and tighten the floors** — raise `0.01` until it bites. **Rejected as
arithmetically impossible to do honestly.** The backdrop's contribution varies per preset with
`bg_bright` and `bg_vignette`, so any floor high enough to catch an empty frame on one preset
convicts a legitimately sparse figure on another. There is no single number, which is the tell that
the measurement rather than the threshold is wrong.
