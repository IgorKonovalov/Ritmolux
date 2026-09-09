# ADR-0179 — A precondition on a figure is checked at load, or it is written down

> **Status:** proposed
> **Date:** 2026-09-09
> **Related plan(s):** [0160 — The silhouette's preconditions stop being silent](../plans/0160-the-silhouettes-preconditions-stop-being-silent.md)
> **Related:** [ADR-0107](0107-an-authored-path-is-inline-svg-data-and-it-morphs-by-resampling.md) (the authored path, and the two things it refuses by name),
> [ADR-0111](0111-the-shape-field-gains-a-scaled-copy-coordinate.md) (the scaled-copy coordinate, whose precondition this is about),
> [ADR-0020](0020-preset-grammar-v2-branching-functions-tempo.md) (the warn-but-load precedent: a preset degrades with a surfaced warning, it does not vanish)

## Context

Until Plan 0092 every silhouette this engine drew was one of five names — `disc`, `ring`,
`polygon`, `star`, `heart`. A closed roster is a closed set of geometries, and the engine took
advantage of that in a way nobody wrote down: **wherever a capability needed a precondition, the
roster was small enough to check the precondition by name.**

`coord_mode = 1` is the worked example. ADR-0111's scaled-copy coordinate needs a boundary radius
along a ray from the figure's centre, which exists only if the figure is **star-shaped about that
centre** — one crossing per ray. Exactly one roster arm violates it, the `ring`, because an annulus's
centre lies in its hole. So `core/src/preset/schema/load.rs` warns on `shape == RING_SHAPE`, and
`presets/README.md` carries a block quote teaching that the `ring` is the case to watch. Both were
correct, and both encode *the roster's* membership rather than *the geometry's* requirement.

`[path]` replaced the name with arbitrary geometry, and the proxy stopped holding. A koi authored
against that surface is not star-shaped about its centroid; a ray crosses its fins more than once,
the shader takes the outermost crossing, and the figure collapses to a dot inside four huge rays.
Nothing warns, nothing errors, and the author is looking at a plausible wrong picture that reads as
a design decision rather than as a mistake — the exact failure mode ADR-0107 wrote the parser's
character-offset errors to prevent, arriving one layer above the parser.

The same session found three more, and the pattern is what makes this a decision rather than a bug
report. Two are constraints the engine cannot check and nobody wrote down: an **inward offset
erodes**, so interior bands eat a figure's thin features and the band count is set by the thinnest
one; and a **band edge sits at every multiple of `1 / palette_steps` while the outline sits at
`color_center + color_span`**, so missing that alignment dissolves the silhouette — which silently
forbids binding `color_span` at all and restricts `color_center` to whole-band increments. The
fourth is a documented capability that is not reachable: the **arc chain** is sold as *"a smooth
figure gets it"*, when the real gate is a piece count against a tolerance fixed at the tightest
figure size, and a 39-vertex all-smooth contour is discarded exactly as a spiky one is.

Four preconditions, one shape: the surface refuses loudly what the *parser* can see (`A`, a second
subpath) and says nothing about anything else, because before `[path]` none of the four could be
reached. The parser's refusals are not the standard — they are the subset that happened to be
mechanically visible.

## Decision

We will hold this surface to one rule: **a precondition an author can violate is either checked at
load and named, or it is written down in `presets/README.md`. Silence is not an option, and which of
the two applies is decided by whether the engine can see the violation, never by how hard it is to
say.**

Where the engine can check, it checks **the geometry, not the roster name**. `coord_mode = 1` tests
the contour for star-shapedness about its centre at parse time — an O(N) walk over at most
`MAX_SAMPLES` points, once, on a path the engine has already fully materialised — and warns in
ADR-0020's shape, drawing the distance coordinate instead. The existing `ring` branch becomes one
instance of that general test rather than a special case beside it, so the two cannot disagree.

Where the engine cannot check — erosion against feature thickness, band alignment against a binding
that may sweep, the arc fit's reachability — the constraint is stated in the parameter reference,
next to the parameter it constrains, in terms of what the author must do about it.

## Consequences

**Positive.**

- The failure that costs an author an afternoon — a plausible wrong picture — is converted into a
  sentence at load or a paragraph in the reference, which is the same trade ADR-0107 already made
  for the parser and ADR-0020 made for inert bindings.
- The `ring` warning stops being a special case. A sixth roster arm, or a seventh, inherits the
  check without anyone remembering to extend a name list.
- `presets/README.md` gains the three constraints that actually decided the shape of every figure
  built against this surface so far. The `preset-author` lane keeps no catalogue of its own and
  reads that file as the reference, so a constraint absent from it is a constraint the lane
  rediscovers by hand each time.

**Negative.**

- **The star-shapedness test is a load-time cost on a path that has none today**, and it is one more
  thing that can be wrong. A contour that is star-shaped by a hair will be judged by a tolerance, and
  the tolerance is a number this ADR does not fix — the plan measures it against real contours and
  records what it chose.
- **A warning is not a picture.** An author who ignores the line still gets the degenerate figure. We
  take that over a refusal because the `ring` precedent already took it for identical geometry, and
  because a legal binding that renders acceptably in a corner of its range should not be fatal.
- **The written-down half has no gate.** Nothing tests that a documented constraint stays true, and
  the arc-chain sentence is proof that this surface's prose can be confidently wrong. The mitigation
  is only that these three sit in a generated file's hand-written neighbourhood, where ADR-0170's
  regeneration puts a reader in front of them.
- The rule scopes to the silhouette surface. It is not a claim about every parameter in the engine,
  and pretending otherwise would make it unfalsifiable.

## Alternatives considered

- **Refuse the combination as a load error**, in the shape of the parser's `A` and multi-subpath
  refusals. Rejected on the `ring` precedent: ADR-0111 and Plan 0098 Phase 4 already faced this exact
  geometry, rendered the three defensible answers, and chose a warning plus a fallback. Making the
  same geometry fatal on a path and advisory on a ring would be two rules for one condition.
- **Keep the roster-name proxy and add `path` to the list of names that warn.** Rejected because it
  is wrong in both directions: it convicts every authored contour including the star-shaped ones,
  for which `coord_mode = 1` is exactly the capability they want, and it would still be a name test
  standing in for a geometric fact.
- **Silently fall back to the distance coordinate on a non-star-shaped contour.** Rejected for the
  reason ADR-0111 already recorded when it rejected the silent `ring` fallback: it renders the same
  pixels as the announced version and costs an author the afternoon spent finding out why.
- **Document all four rather than check any.** Cheapest, and it is what the surface does today. The
  star-shapedness case is mechanically decidable from data the engine is already holding, and a
  precondition that can be checked and is instead written down is one an author meets at the wrong
  end.
- **Widen the roster instead**, adding the figures authors want as named arms whose preconditions are
  known. Rejected by ADR-0107, which took `[path]` precisely to stop answering only the asks someone
  has already had; this ADR is the cost of that decision arriving.
