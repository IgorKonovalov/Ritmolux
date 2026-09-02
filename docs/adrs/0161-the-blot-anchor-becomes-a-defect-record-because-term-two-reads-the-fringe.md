# ADR-0161 — The blot anchor becomes a defect record, because term two reads the fringe

> **Status:** proposed
> **Date:** 2026-09-02
> **Related plan(s):** [0149](../plans/0149-the-line-corners-stop-being-blunt.md) Phase 2
> **Supplements:** [0130](0130-the-structural-term-is-boundary-density-and-conditioning-the-population-is-what-made-it-work.md)
> (whose default `boundary_floor` arm this falsifies), [0128](0128-a-tonally-flat-picture-is-a-blot-only-if-it-is-also-structureless.md)
> (the conjunction), [0126](0126-the-sanity-lens-measures-departure-from-the-frames-own-ground.md)
> (the derived ground this interacts with), [0074](0074-a-ratio-against-an-in-run-control-is-not-automatically-portable.md)
> (the rule the old floor breaks), [0071](0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md)

## Context

`core/tests/sanity.rs` convicts a frame as a blot only when it fails both terms of ADR-0128's
conjunction: over `MAX_TONAL_FLATNESS = 0.90` on `tonal_flatness`, **and** under
`boundary_floor(system)` on `metrics::boundary_density`. ADR-0130 chose that second statistic and
derived its default arm, `0.31`, as the midpoint of two frozen frames — `0.2631` from the synthetic
`blown_out` fixture and `0.3602` from `Tiled Rosette Mono`, the flat-graphic composition the term
exists to admit. `sanity.rs` records what rests on the lower of those two numbers: *"this fixture is
the whole of that evidence — it is the sole anchor on the defect side of `MAX_TONAL_FLATNESS`, of
ADR-0128's conjunction, and of `boundary_floor`'s default arm. Re-blessing it moves three
thresholds."*

Plan 0149 Phase 2 gives joined corners their miter length. On `blown_out` — `parametric_curve` at
`thickness = 44`, `glow = 20`, `trails = 0.97` — that closes the stepped notches around the mass's
rim, which is what a miter is for. The frame is still a blot; rendered side by side at 960x540 it is
the same saturated single-tone disc, and a smoother one. But `boundary_density` against the derived
ground rises from `0.2700` to `0.5697`, the conjunction acquits, and the two tests that carry the
anchor fail.

**The measurement that explains it, taken on the Phase 2 tree at the suite's own 96x96 capture:**

| lens | `coverage` | `tonal_flatness` | `boundary_density` | shells |
|---|---|---|---|---|
| `BLACK` (areal) | 0.9666 | 0.9983 | **0.0382** | 10/10 |
| derived ground `[159, 254, 202]` | 0.0350 | 0.9628 | **0.5697** | 0/10 |

One frame, one statistic, two references, **14.9x apart and straddling the `0.31` floor**. The areal
reading is the blot measured as the solid mass it is — `0.0382` is the `2/r` that
`boundary_density`'s own doc block predicts for a solid disc, so the statistic is sound and reports
correctly when it is pointed at the figure.

The derived reading is not a reading of the figure at all. **A blot is its own modal band** — that is
what being a blot means, and `a_frame_with_no_tonal_structure_is_reported_flat` already asserts it of
this fixture. So `modal_ground` lands on the mass, `is_lit` is false across its interior, and the lit
set is the mass's **fringe**. `boundary_density` then reports the fringe's solidity. A ragged rim has
a thick fringe with interior and reads low; a smooth rim has a thin one that is almost all edge and
reads high. **The term is inverted on precisely the class of frame it exists to convict: the more
perfectly a frame is a blot, the more structured it reads.**

Three things follow, and none of them is caused by Plan 0149.

- **`0.2631` never measured what `boundary_floor`'s doc says it measured.** It was the thickness of
  the rasterizer's notch band on one figure. `0.3602` is a genuine figure perimeter. The two are not
  the same kind of quantity, so their midpoint is not a floor — [ADR-0074](0074-a-ratio-against-an-in-run-control-is-not-automatically-portable.md)'s
  rule, one level up from the ratio it was written about. Phase 2 did not break the floor; it removed
  the artifact the floor was resting on.
- **No floor separates them any more.** The blot reads **1.58x above** the composition on the term
  that is supposed to convict it. This is not a threshold to nudge, and `boundary_floor`'s own doc
  already argues that a single global number is impossible on this library — for a reason narrower
  than the real one.
- **The obvious repair does not work.** Pointing term two at `BLACK` convicts the blot at `0.0382`
  and would drag `Tiled Rosette Mono`'s light paper toward a solid mass, under the floor, convicting
  the preset the term was built to admit. The ground that makes term two work for a two-ink print is
  the ground that inverts it for a blot.

`docs/design-backlog.md` entry 0128 already carries the mechanism — *"under a derived ground a
saturated blot is its own modal band, so its lit mask is the mass's fringe, and a fringe is a thin
ring that every structural statistic scores as structured … Start from the fringe mechanism"*. What
is new here is that it now falsifies ADR-0130's **shipped** separation rather than the candidates
0130 rejected.

One thing bounds the risk. The main gate does not let this frame through: against the derived ground
it reads `coverage 0.0350` against a `0.33` floor with `0/10` radial shells, so
`draws_a_real_shape` fails it as **"blank"**. That is a diagnostic inversion — an author would be
told their saturated frame is empty — but it is not silence.

## Decision

**Plan 0149 Phase 2 lands, no threshold moves, no fixture is retuned, and the anchor's second-term
assertions invert into a defect record in the `KNOWN_FLAT` shape `core/tests/sanity.rs` already
uses.** `MAX_TONAL_FLATNESS`, both `boundary_floor` arms and `blown_out`'s parameters are untouched.
In both `a_frame_with_no_tonal_structure_is_reported_flat` and
`each_term_of_the_flatness_conjunction_is_load_bearing`, the assertion that the blot reads **under**
its boundary floor becomes an assertion that it reads **over**, citing this ADR, so that repairing
term two fails the test and instructs the repairer to restore the conviction rather than leaving a
stale exemption behind. Each test additionally gains `boundary_density` against `BLACK` as a
**positive control**, asserted under the same floor: the statistic convicts this blot when it is
measured against the figure, and it is the conditioning rather than the statistic that is broken.
`boundary_floor`'s doc block stops claiming `0.2631` as a derivation and names what it actually was.
The redesign of term two's ground is not attempted here; it is recorded on backlog 0128, which
already owns the fringe mechanism.

## Consequences

### Positive

- **Phase 2 lands on its merits.** The miter is correct, the render is better, and a correct
  rendering change is not held hostage to an oracle defect that predates it and that it merely
  exposed.
- **The tests get stronger, not weaker.** Today they pin one claim — the conjunction convicts. After
  this they pin three: term one still convicts (`0.9628` against `0.90`), the areal lens still
  convicts on every check including `boundary_density` at `0.0382`, and term two's derived-ground
  conditioning is inverted. A repair in either direction fails a test.
- **The defect cannot rot silently.** This is the `KNOWN_FLAT` property, and that list's own doc
  block explains why the shape was chosen: an entry asserted to *still* be broken forces its own
  deletion on repair. The alternative — a deleted assertion — leaves nothing behind at all.
- **`boundary_floor`'s doc stops overclaiming.** Its default arm is honest about being a two-point
  measurement whose lower point is now known to have measured a different quantity, which is the
  ADR-0071 ceremony applied to a constant that had drifted out of it.
- **The `shape_collage` arm (`0.13`) is untouched and unaffected.** It is derived by the ordinary
  ceremony from two legitimate members, not from the blot, and ADR-0123's knee means the additive
  stack cannot occur in that family at all.

### Negative

- **The conjunction has no demonstrated true positive left.** ADR-0130 already accepted that the gate
  convicts nothing in the shipped library — it is a landmine for the mono-conversion roadmap — but
  `blown_out` was the proof it *could* convict. Until term two's ground is repaired, nothing shows
  the conjunction is capable of a conviction, and its `0.31` arm is a constant with no live
  derivation. **This is the price, and it is the reason this ADR exists rather than a comment.**
- **A saturated blot ships uncaught as a blot.** It would be caught as "blank" by the coverage term,
  with a message telling an author to add material to a frame that is already saturated. That
  misdiagnosis is now a known behaviour rather than an unknown one.
- **A second inversion is unswept.** `coverage`, `quadrant_spread` and `radial_shell_occupancy` are
  built on the same `is_lit`, and the table above shows all three inverting on this frame together
  (`0.9666` to `0.0350`, `10/10` shells to `0/10`). Only `boundary_density`'s inversion is recorded
  here, because only it changed a verdict. Whether the other three mislead anywhere in the shipped
  library is unmeasured.
- **Backlog 0128 grows a third live half.** It was raised on the flat-graphic conviction, half
  discharged twice, and its surviving residue was the four full-coverage presets. It now also owns
  the falsification of the repair that discharged its title.

### Neutral

- Nothing that renders changes. `metrics.rs` is untouched; the whole edit is in `core/tests/sanity.rs`
  and its doc blocks.
- The C ABI, the `Scene` trait and the preset surface are untouched. No preset moves and no author
  sees a new parameter.
- `MAX_TONAL_FLATNESS` and term one are undisturbed in value and in meaning.

## Alternatives considered

### Alternative A — Re-derive `0.31` against the post-miter reading

Take `0.5697` as the new defect anchor and re-derive the floor. It fails on arithmetic before it
fails on principle: the composition this term exists to admit reads `0.3602`, **below** the blot, so
the midpoint ceremony would place the floor above the preset it must acquit. There is no number in
the direction required. `boundary_floor`'s doc already reaches this conclusion for `Suprematist` at
`0.2565`; the post-miter blot makes it true of the default arm's own upper anchor as well.

### Alternative B — Retune `blown_out` until it convicts again

Widen `glow` so the fringe is thick by construction rather than by notch geometry, then re-derive
`0.31` from whatever it reads. This is the closest rejected option and it has a real argument — a
fixture that does not depend on a rasterization artifact is a better fixture. It loses on two counts.
It is tuning a fixture until a threshold is satisfied, which is the failure mode this project's plan
ceremony exists to prevent; and it would re-freeze a two-point midpoint against a number nobody has
measured, on a statistic now known to be inverted for this class of frame, which buys a green suite
and no additional truth. A fixture built after the repair, against a term that reads the figure, is
worth more than one built now against a term that reads the fringe.

### Alternative C — Hold Phase 2 until term two is repaired

Leave the miter uncommitted and fix ADR-0130's second term first. It keeps the gate demonstrably
honest at every moment, which is not nothing. Rejected because the defect predates Phase 2 by three
plans and is exposed rather than caused by it: the same hole is open on `main` today for any
smooth-rimmed saturated frame. Holding a correct rendering fix — and stranding Plan 0149's Phases 6
and 7, which need a mitred tree to judge stroke weight against — buys no safety that landing does
not.

### Alternative D — Measure term two against `BLACK`

Give term two the areal reference, which reads the blot correctly at `0.0382`. Rejected on
measurement: `Tiled Rosette Mono` and `Sumi` are light-ground frames whose whole lit partition
changes under `BLACK`, and the mono print's paper would read as a solid mass under the floor —
convicting the composition ADR-0130 was written to admit. The two frames need different ground
treatment, which is a design question about the role the modal band is playing in a frame, not a
substitution.

### Alternative E — Delete the failing assertions

Cheapest, and it leaves the tests green on term one alone. Rejected because it removes the
conjunction's only demonstrated true positive with nothing recording that it is gone — the exact
decay `KNOWN_FLAT`'s doc block was written against, and the one thing `sanity.rs` explicitly forbids
in the comment above the assertion in question.

## Notes

Measurements taken 2026-09-02 on Plan 0149's lane at Phase 2's uncommitted tree, at `sanity.rs`'s own
capture (96x96, `FRAMES = 30`, all bands at `LOUD = 1.0`, backdrop suppressed), on this box's
hardware adapter. Every figure in the Context table is reproducible from `blown_out()` and the two
references; `boundary_density` against `BLACK` is not read anywhere in the suite today, which is why
it took a probe to find.

**Both readings are bound to the 96x96 capture.** `boundary_density` goes as ~`1/L` in the capture's
linear size, which `metrics.rs` states and `boundary_floor`'s doc repeats. Neither the numbers nor
the 14.9x ratio carries to another size, and the ratio is not offered as a property — it is a
measurement on one frame at one size, named here so the redesign starts from it.
