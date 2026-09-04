# ADR-0151 — Palette stops are authored in sRGB and converted at load

> **Status:** accepted 2026-09-04 (Plan 0138)
> **Date:** 2026-08-29
> **Supersedes in part:** ADR-0021 (Alternative E's deferral of gamma management, for palette stops only)
> **Related plan(s):** [0138](../plans/done/0138-the-colour-surface-stops-misleading-its-authors.md)

## Context

`LUT_TEXTURE_FORMAT` is `Rgba8Unorm` and `core/src/render/palette.rs` records that the entries are
used as colour directly — *"no perceptual/gamma management; that is deferred, ADR-0021 Alt E"*. So a
stop written as ordinary sRGB hex is consumed as **linear** light and the display encode lifts it.
Measured at Plan 0123 Phase 9:

| stop written | renders as |
|---|---|
| `#c81423` | `#dd4c64` — the green channel nearly quadrupled; the ink arrives coral |
| `#930204` (the sRGB-to-linear value of `#c81423`) | `#c81622` — within 2/255 of the colour named |

This is also why `collage_mono`'s `#b00808` arrives as `#d63131`.

The deferral was deliberate and it was right for a long time: while every palette was an abstract
gradient chosen by eye, "the stop is a number that produces a colour" is a perfectly serviceable
contract, and nobody was naming an ink. **[ADR-0138](0138-limited-ink-is-a-supported-palette-class-defined-at-the-draw-seam.md)
changed the population.** Limited ink is now a supported palette class, and its authors are exactly
the people who write `#c81423` because they mean vermilion — the one audience for whom "close
enough, and it is fine" is not fine.

The forcing detail is that the shift is **not** unavoidable, which is what makes this a decision
rather than a known cost. Below the tonemap knee at linear `0.6` the curve is exactly the identity,
so an author can correct it precisely by pre-converting the stop. The measurement above is that
correction working to within 2/255. Today the engine makes them do it by hand, and
`docs/preset-palettes.md` tells them not to bother.

## Decision

**A palette stop is authored in sRGB, and the engine converts it to linear at load.** `#c81423`
renders as `#c81423`. The conversion happens once when the LUT is built, not per-sample, so the hot
path is unchanged.

**Every shipped palette and every custom stop in the library is migrated in the same change** — each
existing stop is replaced by its sRGB-encoded equivalent, so the rendered output of the shipped set
is **byte-identical before and after**. This is what makes the change safe to take: it is a
re-basing of the *inputs* against a new interpretation, chosen so that the *outputs* do not move.
A golden that shifts is a migration error, not an accepted cost.

The `[palette]` table gains no opt-out. Two interpretations of a hex triple, selectable per preset,
is the shape that guarantees nobody can read a palette file and know what it means.

## Consequences

### Positive
- An author who writes an ink gets that ink, which is the whole promise of the limited-ink class.
- `docs/preset-palettes.md` stops instructing authors to accept a shift they can eliminate.
- The hand-conversion recipe — currently the only route — stops being required knowledge, and the
  two shipped presets that worked around it (`collage_mono`, and the `#c81423` case) stop carrying
  a correction the engine should have made.

### Negative
- **Every palette definition in the repository changes in one commit.** The diff is large, entirely
  mechanical, and its correctness rests on the goldens not moving — which is a strong check but the
  only one. A stop mistyped during migration produces a colour nobody notices until a look gate.
- **It re-bases the authoring surface.** Any preset a user wrote outside the repository against the
  old interpretation shifts when they upgrade. Pre-1.0 this is acceptable and consistent with
  [ADR-0126](0126-the-sanity-lens-measures-departure-from-the-frames-own-ground.md)'s
  precedent, but it is a real break for anyone with local content.
- **It contradicts a comment in `palette.rs` that a reader may have relied on.** The deferral is
  cited in the code; leaving a stale pointer to ADR-0021 Alt E would be worse than the original
  problem.
- The conversion is exact only below the tonemap knee. Above it the channels scale together and the
  plateau survives, so the practical effect is nil — but "exact" is a claim with a domain and the
  doc must say so.

### Neutral
- No change to the LUT's size, format, sampling or address mode. This is a decision about what the
  numbers in a `.toml` file mean, not about the rendering path.

## Alternatives considered

### Alternative A — Re-affirm ADR-0021 Alt E and document the shift precisely
The null option, and it is genuinely defensible: no migration, no risk to the shipped set, and the
backlog entry's second half (the doc repair) is worth having either way. Rejected because it leaves
the engine's most colour-sensitive audience doing a manual sRGB-to-linear conversion on every stop,
forever, to get the colour they typed — and the failure is silent for anyone who does not know to.
Documenting a trap precisely is worse than removing it when removal is this cheap.

### Alternative B — A per-preset `[palette] space = "srgb" | "linear"` switch
Preserves every existing palette untouched and lets new work opt in. Rejected because two
interpretations of a hex triple is exactly the ambiguity that makes a palette file unreadable: a
reviewer looking at `#c81423` would have to scroll for the mode before knowing what it means, and
the default would have to be the wrong one forever for compatibility this project does not owe
pre-1.0.

### Alternative C — Convert in the shader at sample time
Avoids the migration entirely — the stored LUT stays as it is and the decode happens per-sample.
Rejected on cost and on honesty: it puts a transfer function in the hot path for a value that is
constant for the life of the LUT, and it would leave the stored table holding numbers that are
neither what the author wrote nor what renders.

## Notes

Discharges half 1 of [design-backlog 0153](../design-backlog.md). Half 2 — the `preset-palettes.md`
Remaps section telling a limited-ink author that *"a limited-ink frame's plateaus almost never carry
the palette's literal RGB, and that is fine"* — is worth repairing whatever this ADR decided, and is
Phase 1 of the plan rather than a consequence of this decision.
