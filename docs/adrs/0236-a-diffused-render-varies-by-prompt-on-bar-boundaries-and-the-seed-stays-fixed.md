# ADR-0236 — A diffused render varies by prompt on bar boundaries, and the seed stays fixed

> **Status:** proposed
> **Date:** 2026-09-19
> **Related plan(s):** [0212](../plans/0212-the-diffused-render-gains-a-timeline.md)

## Context

The owner watched a 5:15 diffused render at Plan 0106's Phase 6 human gate on 2026-08-25 and asked for
*"more variety"* (backlog 0126). The render is one prompt, one seed and one preset from first frame to
last, which is not a defect: it is Plan 0106's *What this plan does NOT do*, in as many words — *"No
timeline, cuts, or prompt automation across a track. One prompt per render, matching 0101's one preset
per render."*

**The obvious lever is the one that cannot be pulled.** The fixed seed is load-bearing for the thing
the gate approved: Phase 1 recorded that a per-frame seed *"guarantees boiling whatever else is
tuned"*. So variety cannot be bought by unfixing the seed, and that is the first thing a designer
reaches for.

**Three levers remain, and they disturb very different amounts.** Plan 0106's own Followups already
name the shape — audio-conditioned diffusion, *"denoise from the onset envelope, prompt blend on bar
boundaries"* — filed as a nice-to-have and now an owner ask with a watched render behind it:

- **Prompt interpolation on bar or section boundaries.** The analyzer already supplies the boundaries.
  Keeps one seed and one preset, and changes only what the conditioning is, frame to frame.
- **Preset changes across a track.** This is a `shot` question before it is a filter question — the
  renderer takes one preset per render by [Plan 0101](../plans/done/0101-the-engine-renders-a-music-video.md)'s design.
- **Denoise strength driven by the onset envelope.** The one lever that reopens Plan 0106's
  deliberate decision to keep the filter seam image-only: it carries real audio data across a boundary
  the plan kept free of it.

The third is worth separating carefully. Plan 0106 Phase 2 named onset-driven denoise as the repair
*if the music stopped reading* in the diffused output — and it did not. So taking it now would be
taking it for variety rather than for reactivity, which is a different justification for the same
mechanism, and a decision to carry audio across that seam deserves its own ADR rather than arriving as
a phase of a variety plan.

## Decision

**A diffused render accepts a prompt timeline — a list of `{at_bar, prompt}` entries — and interpolates
the conditioning between adjacent entries; the seed stays fixed and the preset stays single.** Bar
boundaries come from the analysis the renderer already computes, so the timeline is expressed in
musical time rather than in frames or seconds and a track's structure drives it.

This is the smallest change that produces real variation, and the two properties it preserves are the
reasons it is the right first one: the fixed seed keeps the output from boiling, and one preset keeps
the geometry ControlNet is holding continuous. Nothing about the image-only seam moves.

**Onset-driven denoise is explicitly not taken here**, and neither is a preset change across a track.
The first needs its own ADR because it reverses a recorded decision for a new reason; the second is a
`shot` question about Plan 0101 and belongs with whoever reopens that.

## Consequences

### Positive
- The owner's ask is answered with the lever that disturbs least, and the answer is judged on a full
  track rather than argued.
- A timeline in musical time is reusable by whatever takes the other two levers later: a preset
  timeline and an onset-driven denoise both want the same bar grid.

### Negative
- **Prompt interpolation across a ControlNet-held geometry may read as a crossfade between two wrong
  images rather than as a transition.** The conditioning blend is untested here and the failure mode is
  a muddy middle at every boundary. The plan's judging phase is where that is found, and a negative
  verdict costs the plan rather than being recoverable inside it.
- **A timeline is content the renderer now needs and nothing authors.** One prompt per render was also
  one field to fill; a timeline is a small document per track, with no tooling and no validation
  beyond the sidecar refusing a malformed one.
- **Cost is unchanged per frame and the runs are long.** A 4-minute track at the `quality` profile
  already measures around 5.9 h before Plan 0106 Phase 7d's scope correction, so each judging pass is
  an overnight run and the iteration loop on this is slow.
- **The variety it buys is bounded by what a prompt changes.** If the verdict is that prompt motion is
  too weak to read, the remaining levers are the two this ADR declined, and the ask returns.

### Neutral
- `tools/sd-filter/` never ships, so nothing here touches a release artifact or the size budget.

## Alternatives considered

### Alternative A — unfix the seed, or advance it per shot
The first thing anyone reaches for. Rejected on Plan 0106 Phase 1's own measurement: a per-frame seed
guarantees boiling whatever else is tuned. A seed advanced on bar boundaries rather than per frame is a
weaker version of the same hazard — a visible discontinuity at each step — and the prompt lever
produces motion without one.

### Alternative B — drive denoise strength from the onset envelope
The lever with the most authority over the picture, and the one Plan 0106 reserved as the repair if the
music stopped reading through. Rejected *for this plan*: it reverses the image-only seam that plan
deliberately chose, the condition it was reserved for did not occur, and taking it for variety is a
different argument that deserves its own ADR and its own verdict.

### Alternative C — change presets across the track
Genuine variety, and the largest. Rejected as out of place: `shot` renders one preset per render by
design, so this is a question about Plan 0101 and the renderer, not about the filter, and answering it
here would put a `shot` decision inside a sidecar plan.

### Alternative D — treat this as a resolution question
Backlog 0125 asks for higher resolution from the same gate, on the same render. Rejected, and backlog
0126 says why in its own words: *"Do not fold this into a resolution plan. It shares a verdict with
backlog 0125 and nothing else: one is a pixel budget against a VRAM wall, the other is a timeline the
pipeline does not have."*

## Notes

Both asks came from one sitting at Plan 0106's Phase 6 gate. The resolution half is
[Plan 0211](../plans/0211-the-diffused-frames-resolution-is-measured-before-it-is-designed.md), which
measures before it designs; this half needs no measurement first because the mechanism is absent rather
than mis-sized.
