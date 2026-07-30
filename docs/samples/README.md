# Fold-domain confirmation samples — Plan 0045 Phase 2

**Scratch. Delete this whole directory when Phase 2 records its pick.**

[ADR-0047](../adrs/0047-kaleidoscope-fold-domain-disc-with-falloff.md) is `proposed`,
to be confirmed against rendered samples rather than from an option list. These are
those samples: the same two scenes, under four out-of-disc treatments, at two
aspects.

| file | |
|---|---|
| `fold-figure-*` | **A centred figure** — `star_rosette`, scaled up to `1.25` so it reaches the disc's edge (at its shipped `0.58` the whole figure fits inside the disc and all four treatments render identically, which decides nothing). `kaleido_order = 6`, `kaleido_angle = 0.37`. |
| `fold-field-*` | **A border-filling field** — `swarm_storm`, unchanged except for a fixed `kaleido_order = 6` and `kaleido_angle = 0.37` in place of its tempo-gated bindings. |
| `*-16x9-*` | 1920x1080. |
| `*-9x16-*` | 900x1600 — **the case that decides it.** The 16:9 configuration is what hid this defect for months (same lesson as ADR-0037); at portrait the out-of-range region is most of the frame rather than the corners. |

## The four treatments

| suffix | what it does | ADR-0047 |
|---|---|---|
| `falloff` | Clamp the sample radius to `r_max`, then fade to the backdrop over the next 0.35 `r_max`. | **the decision** |
| `hard` | Clamp the sample radius to `r_max`, no fade. | Alternative B, the A/B control |
| `wrap` | No clamp; `Repeat` addressing, so out-of-range samples tile the frame's opposite edge. | Alternative A |
| `vignette` | Fade to the backdrop **at** `r_max`, over the disc's own outer 0.20 — so nothing outside the disc is drawn at all. | **not in the ADR** — see below |

## Why there is a fourth

Rendering the ADR's three showed that its model of them is off in a way that
changes the choice.

ADR-0047 expects a plain clamp to leave "a hard flat ring" and the corners "flat".
It does not: the clamped sample still varies with angle, so the disc's rim is
replicated **outward as a sunburst of radial rays** — visible in `fold-field-9x16-hard.png`
as bright spokes running to every corner. That is the same streak family the fix
exists to remove, merely bounded and defined.

The decided treatment, `falloff`, fades those rays out rather than stopping them
being drawn: in `fold-field-9x16-falloff.png` they are still there, shortened to
1.35 `r_max` and dimming. On the figure they read as part of the design; on the
field they read as leftovers.

`vignette` is the same disc with the fade moved *inside* it, so no out-of-disc
pixel is ever painted and there is no ray to fade. It costs a rim of real content
that the other two keep, and it is the only one of the four where the frame
outside the disc is unconditionally the backdrop.

## One thing the samples cannot show you

All four fade to **black**, not to the preset's backdrop colour. ADR-0047's
Positive says the falloff "lands on the backdrop" and "composes with `bg_*`
instead of fighting it" — that is not what any of these do, because a `PostStage`
cannot see the background stage's colour. On a lit backdrop the vignette darkens
toward black. Both sample presets use a near-black backdrop, so the difference does
not appear here; `core/tests/golden/composite_kaleido.png`, which uses
`bg_bright = 0.55`, is where you can see it. If that matters, it is a plumbing
question for `architect`, not a fifth variant.
