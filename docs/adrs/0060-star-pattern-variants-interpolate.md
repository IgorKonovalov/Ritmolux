# ADR-0060 — `star_pattern` builds its rosette at a continuous contact angle, so `variant` interpolates instead of cutting

> **Status:** proposed
> **Date:** 2026-08-01
> **Related plan(s):** [0054](../plans/0054-the-line-scenes-catch-up.md)
> **Supplements:** [0007](0007-line-geometry-generators.md) (the generator config surface this
> changes), [0059](0059-line-scenes-colour-along-their-generator-axis.md) (the colour half of the
> same plan).

## Context

`preset-author` filed [design-backlog 0007](../design-backlog.md) with the user's verdict on the
Hankin star scene — **"idea is interesting but looks poor"** — and, separately, "change between
star rosette shapes should be smooth". On 2026-07-26 the user decided **invest, do not cut**: the
scene earns its slot rather than being retired. It has been waiting for its ADR since.

Two asks, both verified against code:

**`variant` cannot be blended.** `star.rs` precomputes one rosette per **contact-angle offset** in
`build`, off the hot path, and per frame picks one:
`idx = (self.variant.max(0.0) as usize).min(variants - 1)`. That is a `floor` into a small array,
so there is nothing between two variants to interpolate. `[smoothing]` on `variant` only spends
time on fractional indices that the floor collapses back to the same three shapes — which reads as
a stutter rather than a transition, and the only preset-side mitigation found was making the cut
rare (~50 s) and hiding it under a continuous redraw.

**The rosette reads as a hollow ring.** Swept by the lane rather than read from code: segments sit
near the rim at every `contact_angle_deg` tried (12 / 20 / 28, no meaningful interior change).
Because a Hankin rosette is rotationally symmetric about the frame centre, `mirror_order` rotates
copies onto the original and is close to a no-op; only non-dividing fold orders (5, 7 against a
12-fold star) add anything.

The two asks are related through one fact: **the contact angle is the shape lever, and it is
currently quantized into a handful of cached variants.** The cache exists because building a
rosette is generator work that ADR-0007 deliberately keeps off the hot path, and `build` is called
from `configure` — i.e. at load, not per frame.

## Decision

**`star_pattern` builds its rosette from a continuous contact angle, and `variant` becomes that
angle's audio-bindable offset rather than an index into a cache.** The scene keeps a cache, but
keyed on a **quantized** angle with hysteresis: it rebuilds when the requested angle moves more
than a fixed step from the built one, and reuses otherwise. A rebuild is bounded generator work at
`TierConfig::max_segments`, not per-frame work.

This makes a swept `variant` a genuine geometry morph — the contact angle is a continuous parameter
of the Hankin construction, so intermediate angles are real rosettes rather than blends of two
unrelated vertex arrays.

**The quantization step is a resolution, not a shape** (the ADR-0037 habit): it bounds how often a
rebuild can happen, and it must be fine enough that the steps are invisible in motion and coarse
enough that a fast `variant` sweep cannot rebuild every frame. The plan measures it rather than
this ADR asserting a number.

The interior half is **not** decided here. Making the rosette read as more than a ring is a
generator question — more tilings, an off-centre construction, or drawing the underlying tiling
grid — and the lane's own note calls it the lower-confidence half. This ADR takes the ask that has
a clear answer and leaves the look question to a content pass against a scene that can finally be
swept continuously.

## Consequences

### Positive

- **`variant` becomes a real lever.** A bound `variant` morphs the figure continuously, and
  `[smoothing]` on it does what an author expects instead of easing through values a floor throws
  away.
- **The audio binding gets somewhere to go.** The scene's most distinctive parameter currently has
  three reachable values; after this it has a continuum, which is the difference between a beat
  swapping between three shapes and a figure that breathes.
- **It removes a documented preset-side workaround.** "Make the cut rare and hide it under a
  redraw" stops being the only mitigation.
- **The cache stays, so the hot path stays clean.** ADR-0007's off-hot-path generator rule is
  preserved; what changes is the cache key.

### Negative

- **A rebuild is now reachable from a bound param, which it was not before.** Today `build` runs at
  `configure`. After this, a preset that sweeps `variant` fast can trigger rebuilds during playback.
  The hysteresis step bounds the rate, but the worst case is a rebuild inside a frame, and that
  worst case did not exist before. The plan measures the rebuild cost against the frame budget; if
  it does not fit, the step widens or the decision is wrong.
- **`[smoothing]` on `variant` now sweeps a param through values whose geometry is rebuilt.** This
  is the `smoothing-sweeps-params-through-invalid-values` shape from the kaleido seam: an eased
  param is continuous, and here continuity is the *point*, but the quantized rebuild means the
  visible geometry still steps. The steps must be below perception or the change has bought
  nothing.
- **The precomputed-variant vocabulary disappears.** Any preset binding `variant` to a small
  integer expecting one of three specific shapes gets a different figure at the same number. Only
  `star_rosette` and `star_lantern` ship, so the blast radius is two files and a golden, but it is
  a behaviour change and their baselines will move.
- **It does not answer "does this scene earn its slot".** The user's verdict was that the scene
  looks poor; smooth transitions between three ring-shaped rosettes are still three ring-shaped
  rosettes. The interior question is the one that decides whether the scene is good, and it is
  deferred.

## Alternatives considered

### Alternative A — cross-fade two cached variants by drawing both

Keep the cache, draw variant `floor(v)` and `ceil(v)` with alpha weights from the fraction. No
generator change, no rebuild, and it lands entirely in the draw path. **Rejected because it is a
dissolve, not a morph.** Two Hankin rosettes at different contact angles have different segment
counts and no vertex correspondence, so the overlap reads as two figures ghosting through each
other — and on an additive pipeline the overlap region is *brighter*, which makes the transition
announce itself exactly where it should be invisible. It also doubles the segment count mid-
transition against `max_segments`.

### Alternative B — interpolate the cached vertex arrays pairwise

Lerp corresponding vertices between two cached rosettes. Cheap, no rebuild, and a true geometric
in-between when the arrays correspond. **Rejected because they do not correspond.** The Hankin
construction's segment count and topology change with the contact angle; there is no
index-to-index mapping between two variants, so the lerp would either require a correspondence
solver (far more machinery than rebuilding) or produce a figure that is not a rosette at any
intermediate value.

### Alternative C — cut the scene

Named in the backlog as a legitimate answer, given "looks poor". **Rejected by the user on
2026-07-26** — invest, do not cut. Recorded here because it was a real option and the decision to
keep the scene is what makes this ADR worth writing.

### Alternative D — rebuild every frame, no cache

Simplest correct thing: the angle is continuous, so build the rosette each frame from the current
angle. **Rejected on ADR-0007's own rule** — generator work stays off the hot path — and because
the cost is unmeasured. The hysteresis cache gets the same visual result while keeping the
guarantee.

## Notes

The interior ask deserves its own record when someone takes it. The lane named three candidate
routes (more tilings, an off-centre mirror, drawing the underlying tiling grid) and swept
`contact_angle_deg` finding no meaningful interior change at 12 / 20 / 28 degrees — which is
evidence that the interior is not reachable from the current construction at all, rather than that
the right angle has not been found. That is the starting measurement for whoever picks it up.
