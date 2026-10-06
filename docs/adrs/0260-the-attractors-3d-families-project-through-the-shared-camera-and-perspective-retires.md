# ADR-0260 — The attractor's 3D families project through the shared camera, and `perspective` retires

> **Status:** proposed
> **Date:** 2026-10-01
> **Related plan(s):** [0240](../plans/0240-the-attractor-projects-through-the-shared-camera.md)
> **Amends:** ADR-0076 (its projection), ADR-0257 (its "the attractor's projection stays")

## Context

ADR-0076 gave the attractor a view depth taken from its rotation's third output, normalized by the
family's half-extent and clamped to `[-1, 1]`. That depth drives a pseudo-perspective
`magnify = 1 / (1 - perspective * dn)`, which scales both position and sprite size. `perspective` is
clamped to 0.8. Its Outcome recorded that this projection makes the figure's centroid orbit by about
`0.9 * p` NDC, which `zoom` cannot correct, so the practical ceiling is about 0.3. The spin is a yaw
only. There is no pitch.

ADR-0257 gave the attractor the shared circle of confusion but kept this projection, because the
tuple rosters' framing (ADR-0093) was curated against it. Plan 0235 Phase 5 built it on a fixed
virtual lens, since the attractor has no camera distance to put a focal plane at.

On reading, the curation is narrower than ADR-0257 assumed. ADR-0093's rosters curate
**coefficients**, and a roster entry's framing (scale, dimension, centre) is **measured** from an
orthographic fit. Only `zoom` on each 3D preset was tuned against the orbit, at that preset's own
`perspective`. 10 of the 22 attractor presets use a 3D family (Lorenz six, Thomas four), and
`fragment_sumi` draws Thomas at `perspective = 0`.

`magnify` is algebraically a pinhole camera at `distance = E / p` on the family's depth extent `E`,
with the depth clamp removed. So a migration can start each preset from an almost identical picture.
The owner chose a full camera with re-curated presets over that exact mapping.

## Decision

We will project the attractor's **3D families** (Thomas, Lorenz, and any later family whose
`inv_depth_extent` is non-zero) through `Camera3d`:

- A **model transform** subtracts the roster entry's measured centre, applies its basis (Lorenz's
  XZ swap), and divides by its measured framed half-extent. Every entry therefore arrives at the
  camera at unit radius. The orthographic framing of ADR-0093 survives unchanged as that transform.
- The attractor splices the **shared camera block** (ADR-0258): `yaw`, `pitch`, `distance`, `fov`,
  `focus`, `aperture`. The integrated `spin` phase adds to `yaw`, so a preset's spin keeps its
  meaning.
- `perspective` and the `dn` clamp **retire**. Depth of field uses the camera's real lens, and the
  fixed virtual lens retires with them.

The **flat families** (De Jong, Clifford, every IFS figure) keep their in-plane path and their bytes.
The camera params are declared inert on them (ADR-0180 rule 4).

The shipped presets are migrated **by the exact mapping first** (`distance = E / p`, the derived
`fov`, `pitch = 0`), so the merged tree renders close to today. Their re-curation in motion, now with
`pitch` and a real `distance`, is owed afterwards as content work. A `perspective = 0` preset migrates
to a long `distance` with a narrow `fov`, which is near-orthographic but not byte-identical.

The 3D path's CPU mirror becomes `CameraView::clip`, which is already pinned against the GPU.
`projection_mirror.rs` keeps the flat path only.

## Consequences

### Positive
- The attractor's 3D figures get a pitch and a true perspective, without the centroid orbit that
  capped `perspective` at about 0.3.
- `focus` and `aperture` on the attractor mean what they mean on the plexus, rather than acting on a
  virtual lens.
- The 3D path's CPU mirror is pinned to the GPU by the camera's existing test, which closes the
  unpinned hand transcription in `projection_mirror.rs` for those families.
- The flat families and their goldens are untouched.

### Negative
- **`perspective` disappears from the preset surface.** Every shipped preset that sets it is migrated
  in the same phase. A user preset outside the repository that binds it stops loading, or loses the
  binding, depending on how the loader treats an undeclared param. Plan 0240 states which, and the
  release notes say so.
- **The 3D attractor presets were judged in motion, and that judgement is spent.** The exact mapping
  is near-identical only where no point passed the `dn` clamp. Lorenz reaches about `1.22 * E`, so its
  far lobes now project unclamped. Ten presets owe a fresh look.
- `fragment_sumi` and any other `perspective = 0` layer change by a near-orthographic approximation.
- The `attractor_depth` golden is re-blessed, and any test that pins `magnify` arithmetic is retired
  or rewritten against the camera.

## Alternatives considered

### Alternative A — the exact mapping with `perspective` kept as an alias
`perspective` would set `distance` and `fov` behind the scenes, presets would keep their files, and
only clamped pixels would move. It lost on the owner's call: a preset surface carrying both
`perspective` and the camera block has two ways to say one thing, and the point of the move is that
the attractor's camera is the engine's camera.

### Alternative B — keep the projection, as ADR-0257 decided
Nothing would move. It lost because the CoC on a fixed virtual lens is the "half true" sharing
ADR-0257 itself named as a negative, and the attractor would stay the one 3D figure without a pitch.

### Alternative C — move the flat families onto the camera too
One path for every family would be the simplest shader. It lost because the flat families rotate in
plane (a roll), which `Camera3d` does not have. Their pictures have no depth for a camera to reveal,
and moving them would re-bless the goldens that need not move.

## Notes

- ADR-0093's rosters are append-only and preset-visible. Nothing here changes an entry's index,
  coefficients or measured framing.

## Outcome (2026-10-06, Plan 0240 close)

Plan 0240 built the Decision as written, and the tree differs from the body in these details:

- **The spin is subtracted from `yaw`, not added.** The camera's `yaw` turns the eye round the
  figure, so the composition is `yaw - spin_phase` (`encode::spun`). That is what keeps a positive
  `spin` turning the figure the way it turned under the retired projection, which is the meaning the
  Decision meant to keep.
- **The counts were wrong against the tree.** 12 presets draw the attractor, not 22, and 6 draw a
  3D family, not 10: Ink on Paper, Lorenz Knot, Thomas Gallery, Thomas on Red and Thomas Walk, plus
  the `fragment_sumi` layer. Six presets, not ten, owe the fresh look.
- **On the 3D path `pan_x` is in frame heights.** The camera divides it by the aspect, which the
  in-plane path did not, so a pan written for a flat family drifts `16/9` as far on a 16:9 target.
- **A binding to `perspective` is a load error** naming `distance` and `fov`, not a lost binding:
  the loader's `RETIRED_PARAMS` table answers the Negative's "depending on how the loader treats an
  undeclared param".
- The camera block grew `fog` (ADR-0263) during the plan, and the attractor's 3D families take it;
  `solid` is not an attractor param.
