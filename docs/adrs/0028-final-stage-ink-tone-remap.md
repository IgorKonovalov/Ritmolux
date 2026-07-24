# ADR-0028 — Final-stage duotone "ink" tone-remap (paper/ink), generalizing invert

> **Status:** proposed
> **Date:** 2026-07-24
> **Related plan(s):** 0027-attractor-ink-and-crisp-trails

## Context

The `preset-author` lane hit a wall trying to author a De Jong attractor as an "ink on paper"
look — a white background with black moving lines. Verified against
`core/src/render/scenes/particles/mod.rs`, the attractor (like the swarm and the line scenes)
draws with **additive** blend (`src=One, dst=One`) into a black float trail field, tinted by a
hardcoded iq cosine palette. Additive compositing is a *lightening* model: it only shows bright
marks on a dark base. A dark stroke adds ~nothing and stays invisible; strokes over a light
background wash *toward* white. So "black on white" is not a color choice — it is the inverse of
the whole compositing model, and no named param reaches it.

Two approved-but-unlanded plans move nearby but do **not** deliver it. Plan 0025 (full composite
coverage) switches the attractor present from opaque `REPLACE` to an alpha-blend over the
backdrop, so a light `bg_*` gradient can show through negative space — that gives a *white
background* but the strokes stay bright-additive, never dark. Plan 0020 (shared palette) makes
stroke color a bindable LUT — but a dark palette under additive blend contributes nothing. The
missing capability is a **darkening** step, and it is genuinely absent from the engine and the
roadmap.

The user chose an **engine-wide, final** mechanism (not per-scene plumbing) that nonetheless
supports **arbitrary paper and ink colors** (not just boolean black/white). Those two goals
reconcile cleanly in one construct: a duotone remap of the finished frame.

## Decision

We will add a final, skippable composite stage — `render/ink.rs` — that performs a **duotone
tone-remap** of the fully composited frame: for each pixel it reads a brightness (luminance) as an
*ink density* `d` and outputs `mix(paper, ink, d)` between two preset-configurable colors. Pure
black-on-white **invert is the default** (`paper = white`, `ink = black`), so turning the stage on
with default colors inverts tone; colored paper/ink (cream + indigo, etc.) is the same operation
with different poles. It is driven by `ink_*` layer-2 named params (ADR-0002), routed by the
renderer exactly as `bg_*`/`trails_*`/`kaleido_*` are today — so there is **no `Scene`-trait
change and the C ABI is untouched**. The stage sits **last** in the ADR-0018 fixed-order composite
(after the scene, trails, kaleidoscope, and Plan 0024's transition blend), before the text/overlay
passes, so the HUD is never inverted. Like every other post-stage it is **passthrough and unbuilt
when `ink_amount <= 0`** (the ADR-0018 skippable discipline, which also sidesteps the DX12-WARP
multi-pipeline aliasing), so the default engine and every shipped preset are byte-identical until a
preset opts in.

## Consequences

### Positive
- **Engine-wide black-on-white / ink-on-paper for every scene at once** — sparse (attractor,
  swarm, lines) and fullscreen (fragment, reaction-diffusion) — from one construct, because it
  operates on the composited frame rather than any scene's pipeline.
- **Arbitrary duotone**, not just invert: `paper`/`ink` colors are free, and `ink_amount` is a
  normal audio-bindable param (strokes can breathe between glow and ink on the beat).
- **Cheap and self-contained**: one fullscreen pass, no per-scene edits, composes with the palette
  (0020) and the alpha-present coverage (0025) instead of competing with them.
- **Neutral by default**: `ink_amount = 0` is passthrough, so golden fixtures and shipped presets
  are unchanged.

### Negative
- **Hue collapses to a duotone in ink mode.** Because the remap keys on luminance, a scene's
  colored per-particle glow becomes a monochrome ink ramp — so Plan 0020's palette and this stage
  interact: the palette still shapes *luminance structure*, but the two color poles come from
  `ink_*`, not the palette. This is the price of a single luminance-keyed remap; an `ink_tint`
  knob (bleed the underlying color into the ink) is a possible follow-up, deliberately out of scope.
- **Passthrough is lost while ink is on.** The whole composite must render into an offscreen so the
  remap can sample it — one extra render target + fullscreen pass at surface resolution. Cheap, and
  still fully skipped when off, but it is real fill against the NFR §1 iGPU floor.
- **One more ordering constraint.** The stage must stay last relative to Plan 0024's transition
  blend (ink remaps the *blended* result, not each side). Whichever of 0024/0027 lands second wires
  the order.

### Neutral
- An *ink-on* golden fixture (if we want drift coverage of the stage) needs its own blessed
  baseline; the default-off path needs none.

## Alternatives considered

### Alternative A — Per-scene darkening blend modes
Give each scene's draw pipeline a multiply/subtract blend variant so dark strokes darken a light
target directly. Rejected: it duplicates the choice across every scene, and it does **not compose**
— the trails, kaleidoscope, and transition stages downstream re-lighten or blend the result, so a
per-scene darken would be undone by the post-chain. A single final remap is one place and covers
every scene and post-effect uniformly.

### Alternative B — Strict boolean invert only (`1 - color`)
A one-line global invert of the final frame. Rejected: the user wants arbitrary paper/ink colors
(cream paper, indigo ink); a boolean invert can only do white/black and inverts hue in ways that
read as a photo negative, not ink. The duotone remap **subsumes** boolean invert as its
white/black default, at trivially more cost.

### Alternative C — Do nothing; rely on Plans 0025 + 0020
Lean on 0025's alpha-present (light background reveal) plus a dark palette from 0020. Rejected:
additive strokes over a light backdrop wash toward white and dark palette colors add ~nothing under
additive blend — you get a white background but never dark strokes. The blocker is the compositing
model, not the color choice, so no combination of those two plans reaches the look.

## Notes

- The trail-field **resolution** ask that arrived in the same feedback note is *not* an ADR-worthy
  decision (no rejected alternative worth remembering — it is a constant/parameterization change
  with an NFR §1 fill tradeoff). It rides in Plan 0027 as its own phase with a code comment, not a
  second ADR.
- Ordering with ADR-0018 (this is a new terminal stage in the fixed composite) and ADR-0024 (ink is
  strictly after the transition blend) is the only cross-ADR coupling.
- Sequenced after Plans 0020 and 0025 because it shares the attractor present/draw path with both;
  landing after them lets the ink stage ride on the LUT-colored, alpha-composited output rather than
  rebasing across their shader edits.
