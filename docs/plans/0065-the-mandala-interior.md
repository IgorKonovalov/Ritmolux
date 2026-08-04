# 0065 — The mandala interior: `star_pattern` stops being hollow

> **Status:** draft
> **Created:** 2026-08-04
> **Owner skill(s):** dev, human
> **Related ADRs:** [0079](../adrs/0079-the-mandala-interior-is-rings-of-motifs-inside-star-pattern.md)
> (this plan's decision), [0007](../adrs/0007-line-geometry-generators.md) (the `[generator]`
> config seam it extends), [0060](../adrs/0060-star-pattern-variants-interpolate.md) (the `variant`
> morph it composes with)

## TL;DR

`star_pattern` gains an optional `rings` roster on its `[generator]` table: concentric rings of
repeated motifs, each with its own count, radius, scale and phase, drawn through the shared
`LineRenderer` alongside — or instead of — the Hankin interlace it draws today. This closes
[design-backlog 0007](../design-backlog.md)'s still-open "hollow ring" half with the *invest, do not
cut* decision the user made on 2026-07-26, and it makes the fourth reference image authorable
directly. First user-visible behavior: a preset declaring four rings draws an ornamental mandala with
a filled interior instead of a bare rim.

## Context & problem

One of the user's five reference images is not a fractal: it is a drawn ornamental mandala — rings of
discrete repeated motifs, thin bright strokes on black, each ring with its own count and radius. It
is line geometry, and this project has a line scene that has been waiting for exactly this.

[Design-backlog 0007](../design-backlog.md) recorded `star_pattern` as reading like "a hollow ring",
established by a rendered sweep rather than argument: segments sit near the rim at every
`contact_angle_deg`, with no meaningful interior change across 12 / 20 / 28 degrees. The user's
verdict moved from "idea is interesting but looks poor" to "very nice, but can we make morphing
between shapes easier, slower" once preset-side mitigation landed. **That second ask was answered** —
[ADR-0060](../adrs/0060-star-pattern-variants-interpolate.md) and
[Plan 0054](done/0054-the-line-scenes-catch-up.md) made `variant` a continuous contact angle. The
first ask, the hollow interior, was never decided; the entry says so explicitly and carries the
user's standing *invest, do not cut* call, along with three unchosen options ("more tilings, an
off-centre mirror, or drawing the underlying tiling grid").

The reference image is the missing specification. It says what to invest in.

## Decision

The ring generator lives **inside `star_pattern`**, as an optional `rings` array on its existing
`[generator]` table, alongside the Hankin parameters rather than replacing them. A preset draws the
interlace alone (today's behaviour, and what `rings`-absent means), the rings alone (the reference),
or both composited. The motif roster is a **closed curated set** chosen from rendered samples.

It goes there rather than into a new scene because the scene *has* the defect this fixes — a sibling
scene would leave `star_pattern` hollow and the backlog entry open, spending the investment decision
on something else — and because the two geometries share the one structural property that matters,
n-fold rotational symmetry about the frame centre, so a ring count and a fold order are chosen
together rather than fought. Rejected alternatives (a new `mandala` scene; deriving the interior from
the tiling; an open motif grammar) are in
[ADR-0079](../adrs/0079-the-mandala-interior-is-rings-of-motifs-inside-star-pattern.md).

## Architecture diagram

```mermaid
flowchart LR
    subgraph cfg["preset [generator] — ADR-0007 declarative config"]
        HK["Hankin params<br/>(tiling, contact angle)"]
        RG["rings = [<br/> {motif, count, radius, scale, phase},<br/> ...]"]
    end
    subgraph scene["star_pattern"]
        IL["interlace geometry<br/>(today)"]
        PL["ring placement:<br/>k copies at 2*pi*i/k + phase"]
        MO["motif outlines<br/>(closed curated roster)"]
    end
    HK --> IL
    RG --> PL
    MO --> PL
    IL --> SEG["one SegmentInstance list"]
    PL --> SEG
    SEG --> LR["shared LineRenderer<br/>(stroke, joins, glow, palette)"]
```

## Implementation phases

### Phase 1 — rings exist and the interior fills

- **Owner skill:** dev
- **What:** The `rings` roster on `[generator]`, a provisional motif set, and the placement
  arithmetic. The walking skeleton: a preset with four rings renders a filled ornament.
- **Files touched:** `core/src/render/scenes/lines/star.rs`, `core/src/preset/schema.rs`,
  `presets/star_mandala.toml` (new).
- **The shape:** each ring is `{ motif, count, radius, scale, phase }`; motif `i` of a ring of `k` is
  placed at angle `2πi/k + phase`, scaled by `scale`, at distance `radius`. Motifs are parametric
  outlines sampled to segments — the same thing `parametric_curve` already does, placed rather than
  drawn once. Validate the roster once at load per the project's boundary rule: an unknown motif name
  is a load error, a zero or negative `count` is a load error.
- **The budget, stated because it decides how ambitious a preset may be:**
  `TierConfig::max_segments` is **20 000** at `Floor` (`tier.rs:241`) and 60 000 at `Rich`. A dense
  mandala of 8 rings × 32 motifs × 24 segments is **6 144** — comfortable, with room for the
  interlace on top. Truncation at the cap must be the existing silent-truncation behaviour rather
  than a new failure mode, and a preset that would exceed it is a thing Phase 5 documents.
- **Done when:** `rings` absent renders `star_rosette` and `star_lantern` **byte-identically** — the
  Hankin path is untouched. With `rings` present and the Hankin count at zero, the capture is the
  ornament alone. The measured lit-pixel coverage of a four-ring preset is materially above
  `star_rosette`'s, which is the "hollow ring" complaint stated as a number rather than as an
  opinion.

### Phase 2 — the sample set

- **Owner skill:** dev
- **What:** A rendered grid of motifs × counts × ring spacings, so the roster is chosen from images
  rather than from names.
- **Files touched:** captures under the scratch/report path; no shipped file changes.
- **What to render:** every provisional motif at three ring counts {8, 16, 32} and two stroke widths,
  plus three whole-mandala compositions of four to eight rings, plus one showing the rings composited
  **inside** the Hankin interlace — which is the composition backlog 0007 actually asked for and the
  one the reference image does not show. At 16:9 and portrait.
- **Also render the question ADR-0079 deliberately left open:** the reference's outer boundary is a
  *scalloped closed curve*, not a ring of separate motifs. Show it both ways — a motif ring whose
  members touch, and a separate boundary curve — because that is a look decision and not a design
  one.
- **Done when:** the grid exists with a plain index naming each cell, including the boundary A/B.

### Phase 3 — pick the roster

- **Owner skill:** human
- **What:** The user picks which motifs ship, whether the scalloped boundary is a motif ring or its
  own thing, and which compositions become presets.
- **Done when:** the roster is closed with a decision per motif. **Dropping motifs here is expected**
  — a curated set is the point, and ADR-0079 records that a look outside it routes back through
  `architect` and `dev` rather than being added on request.

### Phase 4 — the rings move

- **Owner skill:** dev
- **What:** Per-ring audio reach, so the ornament is a visualizer and not a wallpaper.
- **Files touched:** `core/src/render/scenes/lines/star.rs`,
  `core/src/render/scenes/lines/mod.rs` (`PARAMS`).
- **The levers:** a global `ring_phase` that advances alternate rings in opposite directions
  (counter-rotation is the single strongest ornamental motion and costs one sign), a `ring_spread`
  scaling every radius about the centre, and a `ring_scale` on the motifs. All bindable, all
  defaulting to the static configuration so Phase 1's captures do not move.
- **The gate hazard this phase must design around, not discover:** `core/tests/animation.rs` renders
  at 96×96 and diffs whole frames, so a rotationally symmetric figure is nearly invariant under
  rotation — [design-backlog 0009](../design-backlog.md) documented exactly this penalty for
  `star_rosette`, and a ring mandala is *more* rotationally symmetric than the rosette was. **Spin
  alone will not pass the gate.** `ring_spread` and `ring_scale` are radial and will; the shipped
  presets must carry their animation on those.
- **Done when:** a shipped mandala preset passes `animation.rs` on a **radial** binding, and the
  counter-rotation is asserted on the placement arithmetic (adjacent rings' phases move in opposite
  directions for a positive `ring_phase`) rather than on pixels.

### Phase 5 — presets and the doc sweep

- **Owner skill:** dev
- **What:** Ship the chosen compositions and update the docs the plan moved.
- **Files touched:** `presets/star_mandala.toml` and any siblings from Phase 3, `presets/README.md`.
- **Docs the sweep owes:** `presets/README.md`'s structural table gains the `rings` roster with its
  five keys, the closed motif names, the segment budget and what happens at the cap, and the three
  new params. It is load-bearing for the `preset-author` lane, which keeps no catalogue of its own.
  `docs/presets.md` is **not** touched: no grammar change. **Also strike through
  [design-backlog 0007](../design-backlog.md)** with a pointer here — its interior half is what this
  plan closes, and the entry has been half-struck since Plan 0054.
- **Done when:** the docs are swept, backlog 0007 is struck through in full, and every existing
  golden baseline is byte-identical (this plan adds geometry behind an absent-by-default roster and
  moves no pixel of anything already shipped).

### Phase 6 — judge it against music

- **Owner skill:** human
- **What:** A `preset-author` pass tuning the mandala presets live.
- **Questions it answers:** does counter-rotation read as one ornament breathing or as two unrelated
  figures? At what ring count does the figure stop being legible and start being lace? Does the
  interlace-plus-rings composition earn its cost, or is the ornament better alone — which is the half
  of backlog 0007 that only a live judgement can settle?
- **Done when:** the presets ship tuned, and anything that could not be made to read goes to
  `docs/design-backlog.md`.

## Data shapes

```rust
// illustrative — not the final interface

/// One concentric ring of repeated motifs. `[generator] rings` is an array of these.
struct Ring {
    motif: Motif,   // closed curated roster; unknown name is a load error
    count: u32,     // copies around the ring; 0 is a load error
    radius: f32,    // world units from the centre
    scale: f32,     // motif size multiplier
    phase: f32,     // angular offset, radians
}
```

## Risks & open questions

- **`star_pattern`'s config surface roughly doubles**, and half of it has nothing structurally to do
  with Hankin tilings. ADR-0079 accepts this as the price of not leaving the scene hollow; a reader
  of `[generator]` will find contact angles and ring rosters side by side, and only a doc comment
  explains why.
- **The animation gate is the likeliest phase failure**, for a reason that is a property of the
  *test resolution* rather than of the look (backlog 0009). Phase 4 names the mitigation up front so
  it is designed for rather than diagnosed.
- **Two ways to exhaust one segment cap.** The interlace's count grows with the tiling, the rings' as
  rings × count × resolution, and the failure is the same silent truncation from either direction.
  Phase 5 documents it; nothing detects it.
- **Thin concentric strokes are the worst case for the additive ceiling.** Where adjacent rings'
  halos overlap they sum, and [Plan 0040](done/0040-line-joins-finish-the-job.md)'s close found
  exactly this on a mirrored line preset — the *quietest* part of the readout rendering as its
  brightest. Phase 6 should watch `glow` and `thickness` together, not separately.
- **Phase 3 is `human` and gates Phases 4-6**, so this plan does not close in one session by
  construction.
- **[Plan 0055](0055-the-fold-edge-becomes-a-choice.md) and
  [Plan 0064](0064-the-symmetry-stage-and-the-banded-palette.md) do not touch this file at all** —
  this plan is line-geometry work and shares nothing with the post chain. It can run in parallel with
  either.

## What this plan does NOT do

- **No open motif grammar.** ADR-0079 Alternative C — once motifs are authorable, so are their fills,
  joins and nesting, and the project already has `parametric_curve` for a future motif to be promoted
  from.
- **No new scene and no new `SystemKind`.** ADR-0079 Alternative A — the ninth scene slot is not
  spent and the exhaustive `SystemKind::ALL` factory is untouched.
- **No off-centre placement.** Rings are concentric about the frame centre, which is what makes them
  compose with the Hankin geometry and with `kaleido_*`. An off-centre mirror was one of backlog
  0007's three unchosen options and stays unchosen.
- **No new render idiom** — the shared `LineRenderer` draws it, as it draws the other four line
  scenes.
- **No C ABI change, no `Scene` trait change, no new dependency, and no existing golden baseline
  moved.**

## Followups (after this lands)

- Promoting a `parametric_curve` expression into the motif roster, if Phase 6 wants a shape the
  closed set does not have.
- Whether the interlace and the rings should be able to take different palette positions, if the
  composited look survives Phase 6.
