# ADR-0235 — A gallery card's hop is chosen per family, and the signal outlasts it

> **Status:** accepted 2026-09-22 (Plan 0210)
> **Date:** 2026-09-19
> **Related plan(s):** [0210](../plans/done/0210-the-gallery-card-shows-the-world-it-names.md)

## Context

`scripts/docs-shots.mjs` renders one gallery card per shipped preset at `--frame-at 300` — about 3.5 s
of scene time at 110 BPM — unless the preset is named in `CARD_HOP_OVERRIDES`. That roster holds four
presets, all `swarm_*`, all at hop 374. So the mechanism for *"this world needs longer"* exists, has
been used once, for one family, and nothing generalized it.

**A world built on a feedback field is not developed at 3.5 s and cannot be.** `warp_ladder`'s own
header records its horizon: the field is still filling for its first two minutes, coverage 0.408 at the
30 s row against 0.619 at 300 s. `warp_tracery`'s committed card shows the contours closed around each
of seven lobes; the same preset at 30 s shows them merged into the rosette the preset is named for.
Both pictures are true and only one of them is the look (backlog 0254). It is not only that family —
trails, a `[feedback]` table, reaction-diffusion and the particle worlds all have an accumulation axis,
and [ADR-0099](0099-the-show-length-horizon-is-a-spot-check-and-it-splits-in-two.md) already names that
set for a different purpose.

**The obvious repair is the wrong one, and the script says why in its own header.** Hop 300 is not
arbitrary: it is the last hop of the loudest beat in the synthesized signal, and a later hop lands in
the two-beat rest `dynamic_groove` takes next. So raising the constant for everyone trades a card taken
too early for a card taken in a lull, which is why the four swarm overrides at 374 are described as the
exception rather than the better default. The constraint is that the card wants *both* a developed world
and a loud moment, and past ~3.5 s the current signal stops offering the second.

The card is also the one picture most readers ever see of a preset, and the gate beside it —
`every_shipped_preset_has_a_gallery_card` — deliberately checks existence rather than freshness. So
nothing is red and nothing will become red.

## Decision

**A card's hop is a per-family default, and the synthesized signal is long enough that a late hop is
still a loud one.** The per-preset `CARD_HOP_OVERRIDES` roster stays for the genuine exception, and the
default it falls back to stops being one number for the whole library: the accumulating families get a
hop past their development horizon, and the signal driving the capture is extended so that hop still
lands on a beat rather than in `dynamic_groove`'s rest.

Both halves are required and neither works alone. A later hop on today's signal captures a lull; a
longer signal with one global hop leaves the non-accumulating presets captured later than they need for
no benefit. The family is the right grain because accumulation is a property of the system and its
feedback configuration, not of the individual preset — which is also why a per-preset roster grew to
four entries and stopped.

**A card is judged by a person against a late render of the same preset**, and that judgement is a
phase rather than a threshold. Coverage statistics say when a field is still filling; they do not say
which frame is the look.

## Consequences

### Positive
- The gallery starts doing its actual job for the accumulating share of the library, which grows with
  every feedback-based system added.
- The rule generalizes instead of accumulating per-preset exceptions, so a new `warp_mesh` preset gets
  a developed card by existing.

### Negative
- **Every re-rendered card is a committed binary diff that records driver drift as well as the intended
  change.** `docs-shots.mjs` is not byte-reproducible across machines, so a family's cards must be
  re-rendered as a set on one machine and reviewed as pictures rather than as a diff.
- **A longer signal costs capture time on every card, including the ones that did not need it**, unless
  the hop and the signal length are resolved per family — which is more configuration in the manifest
  than one constant and one override roster.
- **Nothing will notice this decaying again.** The existence gate stays an existence gate; the freshness
  question is deliberately not gated, so a family added later with no hop entry inherits the default
  silently. The declared-roster discipline of
  [ADR-0234](0234-an-instruments-system-roster-is-derived-from-the-enum-the-engine-reads.md) is the
  shape that would fix that and is not taken here.

### Neutral
- The `--frame-at` flag and the per-preset override mechanism are unchanged in form; what changes is
  what the default resolves to.

## Alternatives considered

### Alternative A — raise the hop for everyone
One constant, no per-family data. Rejected on the script's own argument: hop 300 is the last hop of the
loudest beat, and a uniformly later hop lands the whole library in the rest that follows. It trades a
true objection for a quieter one.

### Alternative B — extend the per-preset override roster as presets land
Keep one default and name each accumulating preset individually. Rejected because it is what exists:
the roster has four entries covering one family, and the family it was built for is not the family that
needed it most. A per-preset list of a property the system determines is the accumulation this decision
replaces.

### Alternative C — judge freshness with a coverage statistic and gate it
Render two frames, compare a coverage metric, fail when the early one is still filling. Rejected
because the statistic answers a different question than the card does — it says a field is still
changing, not that the picture is wrong — and this repository has three recorded instances of a
threshold standing in for a look judgement and being wrong about it. The judgement stays a person's.

## Notes

Backlog 0254 carries the `warp_ladder` coverage rows and the `warp_tracery` comparison; 0255 carries the
all-or-nothing runner that makes re-rendering one family expensive, which is why Plan 0210 takes it
first.
