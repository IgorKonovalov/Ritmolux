# ADR-0134 — Motion is two readings, and anchoring is why neither can be a threshold

> **Status:** proposed
> **Date:** 2026-08-27
> **Related plan(s):** [0121](../plans/0121-a-rate-an-ink-edge-and-a-motion-reading.md)
> **Supplements:** [0083](0083-in-frame-geometry-is-measured-at-the-line-renderers-draw-seam.md), [0042](0042-reachability-measured-on-the-expression-tree.md)

## Context

Every statistic `shot --report` prints is a **settled differential**: a frame captured after a fixed
frame count, differenced against another frame captured after a fixed frame count. The reactivity
triple is a driven capture against a silent one; `anim` is
`frame_diff(silent@24, silent@48)` (`standalone/src/shot/report.rs:244`). That machinery answers
"does this react", and it answers it well. It cannot see **rate** at all — two frames 24 apart carry
no information about what happened between them — and its reactivity columns drive one band at a
time, so a world that reads several bands together is measured only in pieces.

The mono cohort's `fragment_driftmono` needed four live passes with the user, and the harness was
green and unchanged through all four:

| draft | the user's verdict | `--report` |
|---|---|---|
| v1 | "shaking way too much" | bass 0.465, anim 0.358, 13/13 pass |
| v2 | "not moving at all" | bass 0.182, anim 0.154, 13/13 pass |
| v3 | "still not moving enough" | bass 0.191, anim 0.324, 13/13 pass |
| v4 | "doesn't move to music" | bass 0.191, anim 0.324, 13/13 pass |

`anim` is the closest existing statistic and it moved in the wrong direction relative to the
verdicts. The content lane made progress only after building two throwaway instruments by hand
(design-backlog 0139) — and the second of them, silence against full drive, separates v3 from v4
cleanly where every shipped column reads them as identical.

The reason this is an ADR and not simply a feature is the caveat the raiser attached to their own
finding, having misread the number twice before noticing it. **A raw motion rate does not rank
presets by watchability.** `fragment_tiledmono` measures 8.19 and `fragment_drostemono` 6.42 — both
*higher* than the draft rejected for shaking, and both fine to watch. The difference is
**anchoring**: those two put their motion inside a repeating structure that holds still, so the eye
reads a pattern updating in place, while an unanchored world moves bodily and has to be far quieter
to read as equally calm. No pixel statistic in this harness distinguishes those two cases, and the
number that would order the library does not exist.

## Decision

We will add two columns to `shot --report`, both **printed readings and never gates**, in the shape
[ADR-0083](0083-in-frame-geometry-is-measured-at-the-line-renderers-draw-seam.md) already
established for in-frame geometry — a number read against family neighbours, with no absolute
threshold anywhere in the tree:

- **`drive`** — `frame_diff` between the silent 48-frame capture and the fully-driven 48-frame
  capture. Same frame count, same scene time, same size: a pure stimulus differential against the
  *combined* loud frame rather than one band at a time. This is the reading that separates "reacts"
  from "ignores the music", and it is the defect the user reported at v4.
- **`rate`** — the mean `frame_diff` between **consecutive** frames over the settled tail of the
  transient probe's loud plateau. This is the reading that separates frozen from boiling, and it is
  the axis the report has never had.

**Both come from captures the report already takes.** `drive` differences `late` and `fixed`, which
`build_family_report` captures today for `anim` and `cover`; `rate` walks the frame sequence
`capture_preset_over` already produces for the transient probe. Neither adds a render pass, a
readback, or a resize — the whole cost is two CPU loops over pixels already in host memory.

**`rate` names the configuration it is measured at, and it is not the same one as the other
columns.** The transient probe runs at `PROBE_SIZE` (96×96) while the rest of the table runs at
`REPORT_SIZE` (192×192), for reasons `PROBE_SIZE`'s own comment gives. `frame_diff` is a normalized
mean so the two are broadly comparable, but "broadly" is not a property, and this project has already
been bitten by a statistic that scaled with capture resolution and did not say so (design-backlog
0130). The docs state the size; the column is never compared across sizes.

**A `rate` cell whose response had not settled is marked, not published bare.** The probe already
tracks `rise_settled` per preset for exactly this reason, and a rate reading taken while the step
response is still travelling measures the transient rather than the steady motion. It gets the same
marking treatment the transient cells get.

## Consequences

### Positive

- **The lane can see the axis it has been tuning blind.** Four live round trips on one preset is the
  measured cost of not having this; three of the four were about rate.
- **Free.** No new capture, no new readback, no resize — `--report`'s wall clock is unchanged, which
  matters because a full-library run already sits in a bracket CI pays for twice.
- **`drive` is a genuinely new question, not a fifth band.** The per-band columns drive `bass`,
  `mid`, `treb` and `onset` in isolation; a preset that combines them nonlinearly can read low on
  every one and still be strongly driven — or read plausibly on all four and be driven by none of
  them together. `drive` asks the question the listener asks.
- **It composes with [ADR-0042](0042-reachability-measured-on-the-expression-tree.md)'s habit.** The report's
  method is already "read the pair, the gap is the signal"; `rate` beside `drive` is another such
  pair — a world can move a lot and mean none of it.

### Negative

- **Neither number orders the library, and the anchoring argument says no refinement of them will.**
  A reader who sorts by `rate` and trusts the order gets a wrong answer, in the specific way the
  raiser got one twice. The docs must carry the anchoring caveat next to the column, not in a
  footnote — and being unable to gate is the honest position, not a deferral.
- **Two more columns on a table that is already wide**, on a report the content lane reads many times
  a session. That table is also the one with a name-truncation collision live today, which is why
  Plan 0121 fixes that in the same pass.
- **`rate` is measured at a different size from its neighbours.** Stated rather than fixed: equalizing
  the sizes would mean either a slower probe or a coarser report, and neither is worth paying for a
  number that is never thresholded.
- **A marked `rate` cell is common, not exceptional.** `PROBE_WINDOW`'s own comment records that "a
  great many presets do not settle inside" 48 frames, so the mark will appear often enough that a
  reader could learn to ignore it. It still beats a bare number that quietly means something else.

### Neutral

- No gate moves, no threshold is introduced, and no existing column changes meaning. This is
  additive to the reading surface only.

## Alternatives considered

### Alternative A — gate on the motion rate

The reflex, and the reason this ADR exists. Rejected on the raiser's own evidence: two presets that
are comfortable to watch measure *higher* than the draft that was rejected for shaking, because
motion inside a static repeating structure reads as calm and the same motion in an unanchored world
does not. Any threshold consistent with the rejected draft would fail two shipped presets, and any
threshold that passes those two would pass the rejected draft. The statistic that separates them
would have to model anchoring, which nothing in this harness does.

### Alternative B — `drive` only, and no `rate` column

The smallest addition, and it catches the defect that was actually reported to the user (v4). Rejected
because three of the four round trips were about rate, not about reactivity — "shaking way too much"
and "not moving at all" are both rate verdicts, and `drive` is silent on both. Shipping half the
instrument would leave the majority of the observed cost unpaid.

### Alternative C — a separate `shot --motion` mode printing a per-hop series

Closest to the throwaway instrument the raiser actually built, and richer: a series shows *where* in
the response the motion sits, which a mean cannot. Rejected because it is a second command the
content lane has to remember to run, on a lane whose failure mode is precisely not measuring before
showing — and because `--report` would stay silent on motion, which is the state that produced this
entry. A series mode remains available later; it is strictly additive to a printed column.

### Alternative D — redefine `anim` to measure consecutive frames

One column instead of two, and it repairs the statistic that misled rather than leaving it beside a
better one. Rejected because `anim`'s existing question — does this world move *at all* with no
audio — is a real and different one, several gates and a good deal of prose already lean on it, and
every historical `--report` number would silently change meaning. The project's precedent here is
explicit: when the footprint reading was added, the mean columns were left untouched so that every
historical number kept meaning what it said.
