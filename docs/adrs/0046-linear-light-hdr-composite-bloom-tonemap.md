# ADR-0046 — The composite accumulates in linear-light `Rgba16Float`, with a bloom stage and one engine-fixed tonemap at present

> **Status:** proposed
> **Date:** 2026-07-30
> **Related plan(s):** 0045-linear-light-and-bloom (R1 of [docs/roadmap-visual-richness.md](../roadmap-visual-richness.md))
> **Supplements:** 0018/0031 (the chain), 0028/0032 (ink stays terminal), 0021 (palette; gamma deferral ends here)

## Context

Every intermediate in today's composite runs at the surface format — 8-bit — and scenes
accumulate additively into it with no tone mapping and no gamma management (palette.rs:54
defers it explicitly). **No ADR chose this; it is the undecided default**, and its costs are
measured all over the record: the "additive ceiling" (`glow` above 1 saturates the core and
only widens the skirt — backlog 0019's measurement), Cathedral first rendering as a solid
white disc, mirrored halos summing to blowout on the quietest part of a spectrum readout
(Plan 0039 Phase 5), the ink pass flattening mid-luminance fields to slate grey (backlog
0027), and the twice-user-requested bloom stage (backlog 0005) having nothing correct to
bloom from — a bright-pass over an already-clipped 8-bit frame reads as haze, not light.

Every neon reference look in the visual-richness review requires the standard genre pipeline:
HDR accumulation → bloom → tonemap. The `PingPongField` already ships `Rgba16Float`
(feedback.rs:43), so the format is proven on our targets; what is missing is the *composite*
in that format and the two stages around it.

The 2026-07-30 interview settled scope: full linear-light conversion (not scene-only, not
bloom-on-8-bit), with `exposure` and bloom parameters preset-bindable and the tonemap curve
engine-fixed.

## Decision

We will convert the composite to **linear-light `Rgba16Float` end to end** — scene targets,
trails, kaleidoscope, the transition blend, and a new bloom stage all read and write float
linear values — and add **one engine-fixed tonemap + exposure pass** at the boundary where
the frame becomes display-referred, immediately **before ink** (which keeps ADR-0028/0032's
contract: ink remaps the displayed frame). The frame order becomes:

```
background -> scene -> trails -> kaleidoscope -> bloom -> [transition blend]
    -> tonemap/exposure -> ink -> present (8-bit surface)
```

Bloom joins the `PostChain` as its third `PostStage` (ADR-0031 priced that at an array
element and a `STAGE_COUNT` bump): threshold bright-pass, separable multi-level blur,
additive recombine — with the level count a `TierConfig` value (ADR-0045). The
preset-bindable surface is `exposure` (default 1.0), `bloom_amount` (default 0.0 — off),
`bloom_threshold`, and `bloom_radius`; all four are ordinary named params, audio-drivable, so
a drop can literally bloom. The tonemap curve itself is engine-fixed (chosen in the plan; the
required property is monotone, hue-preserving, and near-identity below the mid-range so that
existing sub-1.0 content shifts subtly rather than wholesale).

Both tiers run the same float pipeline — formats do not fork by tier, only sizes and level
counts do — so shaders and pipelines stay singular. If the floor tier misses NFR §1 with the
float chain on device, the floor's post-resolution cap comes down, exactly as the post.rs
docstring already prescribes ("lower this rather than re-fixing the grids").

## Consequences

### Positive
- The additive ceiling dissolves: stacked strokes, mirrored copies, and fold multiplication
  roll off through the tonemap instead of clipping per channel. Every existing preset gains
  headroom without an edit.
- Bloom is *correct*: the bright-pass sees real energy above 1.0, so halos follow actual
  light. Backlog 0005 closes with the architecture it always wanted.
- ADR-0012's fractal-flame-class looks (log-tonemapped density) and R2's transformed feedback
  both land on a pipeline that can carry them.
- The palette gamma deferral (ADR-0021 Alt E) finally has a principled home: LUT values are
  decoded to linear at sample time, and perceptual ramp work becomes possible later without
  re-plumbing.

### Negative
- **Every golden baseline moves once.** The re-bless must be deliberate and eyes-on per
  scene; `LMV_BLESS` rewrites all baselines, so the restore-unrelated-files trap applies at
  full width. This is the plan's dominant verification cost.
- Memory: at the 1080p post cap, each float intermediate is ~16.6 MB against ~8.3 MB today
  (rough estimate; the chain carries a handful plus a bloom pyramid at ~⅓ of full-res area).
  Floor-tier arithmetic against NFR §12's ~350 MB soft ceiling must be redone in the plan;
  rich-tier headroom on a discrete GPU is not a concern.
- Bandwidth roughly doubles across the composite — a floor-tier risk on 2015-class iGPUs,
  answered by the cap-lowering lever above, and validated on device, not assumed.
- More pipelines, against the documented WARP software-adapter sensitivity to pipeline count;
  the test strategy must gate per-stage rather than one mega-composite.

### Neutral
- `ink` now operates on tonemapped display-referred values — which is what it operates on
  today, so its semantics are unchanged; only its position relative to the new pass is fixed
  by this decision.

## Alternatives considered

### Alternative A — float scene target + bloom only; post stages stay 8-bit
Smaller diff, but trails would accumulate tonemapped values, so feedback glow blooms wrongly
and R2's transformed feedback inherits the flaw permanently. Rejected because it re-does
itself: the conversion would be paid twice.

### Alternative B — bloom on today's 8-bit pipeline
Fastest visible win, but bright areas are clipped at 1.0 before the bright-pass sees them —
bloom reads as uniform haze rather than light, which is precisely the "lifted floor makes the
halo flat paint" failure backlog 0005 measured with `bg_bright`. Rejected as a known re-do.

### Alternative C — full grading surface (tonemap choice, white point, lift/contrast per preset)
Maximum authoring power, rejected in the interview: a large new param family to document, QA,
and keep from producing broken looks, for power the content lane has not asked for. The
engine-fixed curve can be revisited by a later ADR if it bites.

## Notes

Ordering inside the chain: bloom sits **after** the kaleidoscope so halos are computed on the
folded image (a symmetric input yields symmetric halos either way, but bloom-last keeps the
bright-pass on the final HDR frame, which is the conventional and cheapest arrangement).
