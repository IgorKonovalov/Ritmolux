# ADR-0194 — A family-dependent range travels in the schema, and the player reports the family

> **Status:** proposed
> **Date:** 2026-09-14
> **Related plan(s):** [0179](../plans/0179-a-parameters-range-belongs-to-its-family.md)
> **Extends:** [0180](0180-a-mathematical-world-joins-a-system-as-a-family-and-a-structural-parameter-is-held.md)
> (rule 4: the reference names the family each parameter reads on),
> [0184](0184-the-player-reports-what-it-loaded-and-the-studio-re-derives-nothing.md) (every fact
> about what the player loaded travels on the event stream)

## Context

Three family-bearing systems already declare, per family, where each family-dependent parameter
reads: `parametric_curve`, `analytic_field` and `cellular`. Each has a `FAMILY_PARAMS` table, reached
through `render::scenes::family_params`. For a family that ignores a parameter, the table says
`None`. The generated parameter reference prints that table, and ADR-0180's Plan 0162 addendum
records that the exported schema was **deliberately** left at one `range` per parameter.

The studio builds every slider from that one pair (`studio/renderer/components/ParamRow.tsx`,
`const [lo, hi] = spec.range ?? [0, 1]`). On a curve preset the gap is concrete (backlog 0204):

- `d` on a Lissajous is a `1`–`360` slider whose useful travel is `1`–`12`, 3 % of it;
- `n` on a hypotrochoid is a `1`–`24` slider that cannot reach a negative value, so the
  epicycloid, half of that family, is unreachable by gesture;
- the five single-family levers show live controls on the four families each is inert on.

The attractor's `a`..`d` are the same class with no table at all. Each is declared `range: None`, so
they get a number field and no slider on every family. The CPU mirror of the maps
(`particles::family::step_once`) shows that Thomas reads `a` alone, Lorenz reads `a`, `b` and `c`, and
the IFS figures read none of the four.

Two facts are needed to fix the slider, and they come from different places. **Which range reads on
which family** is a static property of the engine, and the schema document is where static
declarations already travel. **Which family the preset on screen draws** is a fact about what the
player loaded. Spec 0003's invariant says such a fact "travels on this stream, and a parent
re-derives none of it". Today the `preset` event carries `system` and `file` and no family. It is
also deduplicated on the preset **name** alone (`Show::report_active_preset`), so a reload that
rewrites the on-screen preset's `[curve] family`, or its `system`, re-reports nothing.

## Decision

We will carry per-family ranges in the schema document, report the active family on the `preset`
event, and have the studio join the two.

1. **The schema document gains an additive `families` field** on every parameter
   `family_params(label)` answers for. It is an array in the system's family-roster order, one
   `{"family": <name as a preset writes it>, "range": [lo, hi] | null}` per family, and `null`
   means the family does not read the parameter. It is rendered from the same `FAMILY_PARAMS` walk
   the generated reference prints, so the two cannot disagree. The existing `range` stays as it is
   and is the fallback for a consumer that does not read `families`. **`SCHEMA_VERSION` does not
   move.** The field is additive, the studio's parser drops keys it does not know, and `kind` set
   this precedent in `export.rs`. The body hash moves, and that is the staleness signal a studio
   already compares against `hello`.
2. **The attractor joins `family_params`.** Rows for `a`..`d` cover every `[particles] family` in
   the export's roster order: the four maps, then the IFS figures. Inertness is what the map
   arithmetic reads. A reading family's range is declared so that it **contains every coefficient
   of that family's tuple roster, canonical entry included**, and a test holds the containment. The
   `ParamSpec` range stays `None`: one pair for four maps with different scales would be a claim
   nothing holds.
3. **The `preset` event gains `family`.** It is the active preset's family as a preset writes it, for
   a system `family_params` answers for, and `null` for every other system. It is additive under the
   same `v` (spec 0003).
4. **`preset` is re-emitted when the on-screen preset's name, system or family changes**, not only
   its name. A reload that edits one of those three is reported. A reload that changes none of them
   emits nothing, as today. A parent that receives a `preset` naming the preset it already shows
   treats it as a refresh.
5. **The editor schemas under `presets/schema/` print the per-family ranges and the inert families
   in a family-dependent parameter's hover text**, the same content as the generated reference
   cell. They add no `if`/`then` validation. A value there is an expression string and a range is a
   guide rather than a bound (ADR-0190's posture), so there is nothing a per-family constraint
   could check.
6. **The studio takes a row's range from `families` by the reported family.** A `null` range
   groups the row as inert on that family. It is not hidden, because the file may still bind it.
   A document with no `families`, a `preset` with no `family`, or a family with no matching entry
   all fall back to `range`, which is exactly today's behaviour.

## Consequences

### Positive
- **Every family-dependent parameter in the engine becomes reachable by gesture**, on all four
  family-bearing systems at once. That includes the negative half of a hypotrochoid's `n`, and the
  attractor coefficients, which have no slider today.
- **An inert lever reads as inert** in the studio, as it already does in the reference, instead of
  as a slider that does nothing.
- **A reload that changes a preset's system is now reported.** The studio's system picker compares
  its selection against the reported `system`. Under name-only deduplication that report did not
  refresh on a reload, which this ADR's investigation found in passing and did not reproduce.
- **No consumer breaks.** An older studio ignores `families` and `family` and keeps drawing today's
  single-range slider.

### Negative
- **`preset` can now arrive for a preset that did not change name.** Anything a parent does on a
  `preset` event (the library's highlight, a fork prompt, a panel reset) runs on a reload that
  edited a family or a system. The studio side has to audit its handlers for that. A handler that
  resets transient UI on every `preset` would now reset on every family edit.
- **The attractor ranges are derived, not discovered.** The hull of a known-good roster is where
  figures are known to be good, not where the map stops being chaotic. An author can reach good
  coefficients outside it by typing, not by dragging. The derivation is written beside the rows so
  the reason is not lost.
- **The attractor's slider still starts from a lie it did not introduce.** `a`..`d` declare
  `default: 0.0`, while the scene draws the active tuple's coefficients when a preset leaves them
  unbound. A per-family range makes that more visible, because a slider now exists to sit at the
  wrong place. This ADR does not decide what a tuple-dependent default is.
- **One more field is kept in step across four places**: the export, the spec 0003 row, the studio's
  union, and the generated snapshot `docs/specs/player-schema.json`. `studio/shared/protocol.spec.test.ts`
  diffs the spec's field lists against the union both ways, so between the player-side commit and
  the studio-side commit that test is red. Plan 0172 accepted the same interval.

### Neutral
- The generated reference's table cells for `parametric_curve`, `analytic_field` and `cellular` do
  not change. The attractor's four rows gain per-family cells.

## Alternatives considered

### Alternative A — the studio reads the family out of the preset document
The studio already parses the document for `[params]`, and `readKeys(text, "curve")` would return
the family. Rejected for two reasons. It re-derives a fact about what the player loaded, which
ADR-0184 forbids, for the reason that ADR recorded: a second copy of a rule that has a default, an
override and an unresolved case. And for an **embedded** preset the studio holds no document, only a
template built from the schema's defaults (`templateFor` in `Editor.tsx`). The family it read would
be the default family, not the one on screen. On a first run, the most common state, that is wrong
for every embedded curve preset.

### Alternative B — the studio asks the player for a family's ranges
A new `ctl/` verb and a reply event, asked on every preset change. Rejected because it widens both
spec 0003 rosters to move a **static** declaration the schema document already exists to carry, and
because the range would then have two sources, the schema's `range` and the reply, with nothing
holding them together.

### Alternative C — replace `range` with a per-family map, and move `SCHEMA_VERSION`
The cleaner document shape. Rejected because the studio refuses a document at any other version
(`v: z.literal(SCHEMA_VERSION)`), so a studio built against version 1 and a newer player would show
**no panel at all**. Today's behaviour is a slider with the wrong ends. The additive field loses
nothing the replacement would gain.

### Alternative D — leave the ranges to the reference and the text editor
The status quo. Rejected on the measured gap: half of one family is unreachable by gesture, one
slider offers 97 % dead travel, and four attractor coefficients have no slider at all.

## Notes

The per-family table's shape (`FamilyParam`, `FamilyRange` in `core/src/render/scenes/mod.rs`) is
unchanged. This ADR adds consumers of it and a fourth table, and no new representation.
