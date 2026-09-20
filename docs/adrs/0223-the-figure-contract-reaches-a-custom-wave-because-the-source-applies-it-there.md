# ADR-0223 — The figure contract reaches a custom wave, because the source applies it there

> **Status:** accepted 2026-09-20 (Plan 0201 Phase 2)
> **Date:** 2026-09-19
> **Related plan(s):** [0201](../plans/done/0201-the-warp-surface-stops-lying.md)
> **Extends:** [0199](0199-a-converted-waveform-draws-the-sources-figure-at-the-hosts-scale.md)
> (whose clause 2 this widens past the eight built-in modes)

## Context

Plan 0180 Phase 6 rebuilt the eight `wave_mode` figures the way `CPlugin::DrawWave` builds them:
each passes through `SmoothWave` and each carries `HOST_SAMPLE_FACTOR` on its sample term
([ADR-0199](0199-a-converted-waveform-draws-the-sources-figure-at-the-hosts-scale.md) clause 2). A
custom wave gets neither. It reads the same smoothed, `wave_scale`d traces — `custom_waves` in
`core/src/render/scenes/warp_mesh/draw.rs` hands `value1`/`value2` from the analyzer's levelled pair
— so the channels are right; what it does not get is the midpoint insertion or the host factor.

That scope was deliberate and stated: backlog 0216's figure contract covered the eight built-in
modes, and Plan 0180 says so twice. This is the question that scope left standing, not a defect it
missed.

The two cases genuinely differ, which is what makes it a decision. A built-in figure is the source's
own construction and the host's smoothing is part of what it looks like. A custom wave is the
preset author's per-point program, which may already place its points exactly where it wants them —
and smoothing inserts points the program did not compute, which is a change to someone else's
drawing.

Against that stands one fact that settles it: **the source smooths a custom wave too.** MilkDrop
applies `SmoothWave` to a custom wave unless it draws dots (`milkdropfs.cpp` l.2722). The whole
purpose of this chain is that a converted preset looks like it does in `foo_vis_milk2`; a place where
we knowingly do something the reference does not is a place where the comparison will fail, and the
person who will find it is whoever runs Plan 0142 Phase 4's side-by-side session.

The host factor is the weaker half of the question and points the same way: it is a property of how
a sample reaches the draw, and a custom wave reads the same samples.

## Decision

We will apply the figure contract to a custom wave: `SmoothWave`'s midpoint insertion, **except when
the wave draws dots**, and `HOST_SAMPLE_FACTOR` on its sample term — matching what the source does at
the same point in its own draw. ADR-0199 clause 2 therefore covers every waveform the converted
scene draws, not only the eight built-in modes.

## Consequences

### Positive
- A converted preset's custom wave matches the reference at the one place someone will hold the two
  side by side, which is what the whole converted chain exists for.
- One rule rather than two. "A waveform this scene draws passes through the host's smoothing and
  carries the host's factor" has no exception to remember except the source's own dots case.

### Negative
- **We are changing what a preset author's program draws.** A custom wave that placed its points
  deliberately now has midpoints inserted between them, and a program written against the current
  behaviour will look different. No shipped preset is affected (`presets/` carries no `[milk]`
  bundle), so the cost falls entirely on converted content.
- **The dots exception is a second branch in the draw path** that exists only because the source has
  it, and it is testable only against the source's behaviour rather than against a property.
- The host factor changes a custom wave's amplitude, so any converted preset already tuned against
  the unfactored value moves. Again: converted content only.

### Neutral
- Nothing changes for the native (non-converted) scenes, which do not read this path at all.

## Alternatives considered

### Alternative A — Leave a custom wave raw
Treat the author's per-point program as final, on the ground that smoothing inserts points nobody
asked for. It is the more principled reading in the abstract and it loses to one fact: the source
does not do this, so the resulting picture is wrong against the only reference this chain is
measured by.

### Alternative B — Apply the host factor only
Take the half that is about how a sample reaches the draw and leave the smoothing alone. Rejected
because it splits one contract into two for no gain: the smoothing is the half that visibly changes a
thin figure's weight, which is exactly the difference the side-by-side session will report.

## Notes

Raised as backlog 0244, 2026-09-16, by Plan 0180's own **Not done here** section, and written down
before [Plan 0142](../plans/done/0142-the-milkdrop-import-earns-its-verdict.md) Phase 4's comparison
session so the question is decided rather than discovered there.
