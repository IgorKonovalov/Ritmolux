# ADR-0270 — A paper backdrop is painted after the tonemap, and the scene lies over it by coverage

> **Status:** proposed
> **Date:** 2026-10-11
> **Related plan(s):** [0257](../plans/0257-the-backdrop-reaches-paper-white.md)

## Context

The owner asked for the voxel solid teaching preset to be drawn black on white (backlog 0287).
The backdrop is the only way to paint a ground, and it is painted in **linear light**, into the
chain's destination, before the tonemap (ADR-0055, ADR-0046). The tonemap's shoulder
`1 - (1-k)^2 / (x + 1 - 2k)` tends to 1 and never reaches it. So `bg_bright = 1` over a white
palette stop leaves the tonemap at `0.800` linear, which is a light grey on screen. No value of
`bg_bright` gives display white, and its range already stops at 1.

`docs/examples/voxel/solid.toml` works around this with `exposure = "3"`. `exposure` multiplies the
whole frame, so the ground reaches near white only because the cubes are near black. On any scene
with mid-tones, the figure is lifted with the ground. `ink_amount = 1` also gives a white page, and
it was rendered and rejected: ink remaps luminance, so a lit solid loses its shading.

Three facts make a cleaner route available:

- **The chain carries premultiplied coverage.** Every stage propagates alpha (ADR-0055), a
  fullscreen scene presents premultiplied over the backdrop (ADR-0201), and a solid voxel hit writes
  `alpha = occlude`. The only reason the tonemap's input is opaque everywhere is that the backdrop
  pass clears it to opaque black, or paints an opaque gradient, first.
- **Bloom never reads the backdrop.** The backdrop is outside the chain's input (ADR-0055), so a
  ground painted somewhere else changes nothing the bright-pass sees.
- **The tonemap writes the display target with `REPLACE`.** Giving that write a premultiplied OVER
  blend is a pipeline state, not a new pass.

## Decision

We will add a structural backdrop param, **`bg_paper`** (0 or 1, default 0). At `bg_paper = 1`:

- **The backdrop leaves the linear composite.** The backdrop pass clears the chain's destination
  to **transparent** and draws nothing, so the tonemap's input alpha is the scene's coverage.
- **The backdrop is painted display-referred.** The same backdrop shader, with every `bg_*` param
  meaning what it means today, draws into the tonemap's **output** target at the surface format,
  before the tonemap resolves. A palette stop of `#ffffff` at `bg_bright = 1` writes display white,
  255.
- **The tonemap lies over the paper.** The tonemap maps the premultiplied scene colour exactly as
  it does today, and writes it with a premultiplied OVER blend, so a pixel reads
  `tonemap(scene) + (1 - coverage) * paper`. A fully covered pixel is byte-identical to the same
  scene over a black ground.

At `bg_paper = 0` nothing changes. The backdrop pass, the tonemap pipeline and its `REPLACE` write
are the ones that ship today, taken through a `select` or a pipeline choice rather than arithmetic,
so every golden is byte-identical. The paper pipeline is built lazily, the first time a preset sets
the param.

**A dissolve holds the paper of whichever side has one.** The transition keeps the outgoing side's
paper the way it already keeps the outgoing `exposure`. A non-paper side's frame is opaque, so the
blended alpha moves smoothly between the two sides' coverage, and the paper shows only where a paper
side leaves it uncovered. When **both** sides are paper, the incoming side's paper is drawn from the
first frame, so their grounds cut rather than fade.

## Consequences

### Positive

- **A ground reaches display white without touching the scene.** The teaching preset drops
  `exposure = "3"`, and any `present = "solid"` voxel, `shape_collage`-style flat scene or particle
  system can sit on a white or coloured page at its own exposure.
- **No new pass and no new param family.** One structural switch, and the backdrop's existing
  params gain a second meaning: in paper mode, `bg_bright` is a display value rather than a light.
- **The scene's own pixels are provably untouched**, which is the property the owner asked for and
  is a byte-equality test rather than a judgement.

### Negative

- **Paper is not light.** It does not bloom, it is not folded by the kaleidoscope, it does not
  smear under `trails`, and `exposure` does not move it. That is the point, and it also means a
  preset that switches modes changes how its ground responds to every one of those.
- **An additive glow over paper composites by its coverage, not by adding.** A swarm particle over
  white reads as a coloured mark, not as a brighter spot, because adding light to white has nowhere
  to go. Content written for a black ground does not port to paper unchanged.
- **The paper write is not dithered.** The dither (ADR-0096) lives in the tonemap and applies to the
  scene part only. A flat paper does not band; a paper **gradient** at 8 bits can.
- **Two pipelines where there was one** for the tonemap and for the backdrop, in a module with a
  recorded sensitivity to pipeline count on WARP (ADR-0046; unverified on lavapipe). Both are built
  only when a preset sets `bg_paper`.
- **Two paper presets cut their grounds at a dissolve** rather than fading them.
- **Ink sees the paper.** Ink remaps the tonemap's output, so a paper preset with ink on remaps its
  own page. That is consistent with what ink does today, and it is a second way to lose the white.

## Alternatives considered

### Alternative A — widen `bg_bright`'s range

Let `bg_bright` reach about 8, where the shoulder gives 0.98. One range edit. It lost because it
still never reaches 255, it leaves the ground inside the light pipeline (a lit ground under `trails`
or a transition behaves like a light), and it gives a "white" whose exact value depends on `KNEE`.

### Alternative B — a tonemap white point

A preset `white` param above which linear values map to exactly 1. True white, but the shoulder is
reshaped for the whole frame, so the bright parts of the scene move as well. That fails the one
property the owner asked for: the scene is not lifted.

### Alternative C — paper as part of ink

Ink already has `paper_hue`/`paper_sat`. It lost because ink's whole operation is
`mix(paper, ink, luminance)`, which is what flattens a lit solid. Splitting ink into a remap and a
separate ground would be this ADR in a less obvious place.
