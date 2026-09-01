# ADR-0159 — The component gets its own size cap, and the recipe that builds it is what carries it

> **Status:** proposed
> **Date:** 2026-09-01
> **Related plan(s):** [0148](../plans/0148-the-shipped-artifacts-carry-their-own-guarantees.md)
> **Supplements:** [ADR-0038](0038-tag-driven-release-unsigned-universal-mac-app.md) (the recipe this extends),
> [ADR-0071](0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md) (why a
> measurement is printed and a property is asserted)
> **Amends:** `docs/nfr.md` §4 — the one place in this project where an ADR edits an NFR line
> rather than only citing it
> **Backlog entry closed:** [0177](../design-backlog.md)

## Context

`docs/nfr.md` §4 reads, in full:

> **Soft cap ~10 MB** for the standalone release exe; plugin DLL in the same ballpark.

Three separate things are wrong with using that as a gate, and they compound.

**It names no unit.** `foo_lmv.dll` measured **9,789,952 B** on 2026-09-01. That is **97.9 %** of
the decimal reading and **93.4 %** of the binary one. Which of those two numbers is true decides
whether the component has 210,048 B of headroom or 695,808 B — a factor of 3.3 — and nothing in the
repository says.

**Its subject is the standalone exe.** The plugin is covered by *"in the same ballpark"*, which is
an assumption that was never measured against what the two artifacts actually contain. They are not
the same thing: the component carries the whole core, the embedded preset library and the foobar SDK
shim, and does *not* carry `winit`, the window, or the WASAPI capture stack.

**Nothing reads the number.** `packaging/foobar/build-component.ps1` produces the DLL and then runs
seven fatal checks over it — it is an x64 PE, it exports `foobar2000_get_interface`, it carries the
workspace version, the archive holds exactly one file. It parses PE headers by hand to do this.
**It never reads the file's length**, which is the cheapest fact about the artifact and the only one
NFR §4 constrains.

So the size series in [`docs/specs/0001-c-abi.md`](../specs/0001-c-abi.md) is only as current as the
last person who remembered to look, and the record says that is not often:

| Measured at | `foo_lmv.dll` | Moved by |
|---|---|---|
| Plan 0097, before `text` | 6,774,784 B | — |
| Plan 0097, after `text` | 8,879,104 B | +2,104,320 B (+31.1 %) |
| 2026-08-18, Plan 0107's close | 9,279,488 B | +400,384 B (+4.5 %) |
| 2026-09-01, Plan 0141 Phase 2 | 9,789,952 B | +510,464 B (+5.5 %) |

**+3,015,168 B, and every byte of it was noticed retroactively** — twice, by a reviewer at a close,
never by the build.
[Plan 0141](../plans/done/0141-the-plugin-seams-stop-drifting.md) Phase 2 replaced a re-measure
trigger that could not fire (*"when a dependency is added to this crate"* — the growth arrived as
code behind the ABI, with no new crate) with one that always fires: at every release. That is a real
improvement and it is still **a duty a person performs from memory**.

The growth is also **step-shaped rather than linear**. The `text` step alone is 70 % of the total and
it bought a font system; the rest is one MilkDrop converter and ordinary drift. A cap for this
artifact is therefore not a rate extrapolation — it is a question of how many more steps of that
size are allowed to land before someone has to have a conversation about it.

## Decision

**The component gets its own cap of 12 MiB — 12,582,912 B — recorded in `docs/nfr.md` §4 beside the
exe's, and `packaging/foobar/build-component.ps1` prints the DLL's length on every build and warns
when it exceeds 90 % of that figure. It never fails a release over it.**

The figure is derived, not chosen: **today's 9,789,952 B plus one more step the size of the `text`
step (+2,104,320 B) is 11,894,272 B**, and 12 MiB is the next round binary boundary above it. So the
cap admits exactly one more feature of the largest class this project has actually shipped, and the
second one has to be argued for. Headroom on delivery is **2,792,960 B**, which is **22.2 % of the cap** — the component sits at 77.8 % of it, and that is the figure `docs/specs/0001-c-abi.md` records; the warning
threshold sits at 11,324,620 B, which is **1,534,668 B above today**.

Three properties this decision insists on:

- **The cap is stated in bytes, with the unit in the number.** `12,582,912 B` cannot be read two
  ways. NFR §4's existing exe line is amended to the same discipline in the same edit.
- **The recipe warns; it does not `Die`.** The seven existing checks are properties of a correct
  artifact — a wrong architecture or a missing export is a broken component. A size is a
  *measurement* (ADR-0071), it is compared against a soft cap, and a release must not fail on it.
  The point is that the figure appears in the release log where a human already reads output,
  rather than in a spec nobody opens to cut a tag.
- **The number the recipe prints is the number the spec's series records**, in the same units, so
  the series can be extended by copying a line out of a build log instead of by re-deriving a
  measurement.

## Consequences

**Positive.**

- The cap becomes a **guard** rather than a duty: it is read by the process that produces the
  artifact, at the moment it produces it, with no one having to remember.
- The unit ambiguity is gone from the one artifact that was closest to the cap, and the exe's line
  is repaired in passing.
- The 90 % threshold fires with ~1.5 MB still available, so the warning arrives while there is room
  to respond rather than at the moment a release is blocked.
- Extending the spec's series stops being a measurement task.

**Negative — these are the price.**

- **A cap this project chose is now a number this project has to defend.** "~10 MB in the same
  ballpark" could absorb any outcome; `12,582,912 B` cannot, and the first feature that would cross
  it forces a decision that today's wording lets everyone avoid. That is the intended effect and it
  is still a cost.
- **The exe's cap is left as `~10 MB` in substance** — this ADR does not measure the exe, so its
  figure stays inherited while the component's is argued. Writing it as **10,000,000 B** is
  nonetheless a *choice*, not only a unit: the two readings of `~10 MB` differ by 4.9 %, and this
  takes the tighter one, which is what `docs/specs/0001-c-abi.md` already said to plan against. Note
  that the same reading is **rejected** for the component two sections below, on a ground that does
  not transfer — there it left 210,048 B of headroom and would have warned permanently, and here
  nothing reads the number at all. The two artifacts now have caps derived by different methods.
- **A warning that never fires teaches nothing.** Until the component crosses 11,324,620 B the new
  branch is unexercised, so the plan seeds it against a forced threshold rather than shipping an
  untested arm.
- **The recipe now has a constant that can drift from the NFR.** Two copies of a number is exactly
  the shape this project keeps finding rot in; the plan answers it by having the script cite NFR §4
  by section and by asserting the constant in the packaging test, not by a comment.

## Alternatives considered

**Adopt 10,000,000 B, the decimal reading.** The most conservative choice and the one that needs no
new judgement. Rejected because it leaves **210,048 B** of headroom — the next ordinary plan trips
it, the warning fires immediately and permanently, and a warning that is always on is noise that
gets filtered. It would also assert that a figure written with a tilde is exact.

**Adopt 10,485,760 B, the binary reading.** Same objection, one step weaker: 695,808 B is a little
over one `+510,464 B` release step. It buys one plan of quiet and then behaves like the decimal
reading.

**Leave the NFR alone; print the size and warn on nothing.** Genuinely tempting, and it discharges
the larger half of backlog 0177 — the figure reaches the release log either way. Rejected because
"is this number bad?" is then still a judgement made from memory at a tag, which is the exact defect
being repaired. A printed number with no threshold is a better duty, not a guard.

**Make the check fatal, like the other seven.** Rejected on the soft/hard distinction: NFR §4 writes
the cap with a tilde and calls it soft. A hard gate would convert a budget into a release blocker,
and the first legitimate feature to cross it would be met by someone editing the constant under time
pressure at a tag — which is worse than no gate, because it also destroys the record.

**Have CI enforce it instead of the recipe.** Rejected because the recipe is where the artifact is
produced and where the number is free; CI would have to rebuild the component to learn it, and the
macOS and Linux arms have no component to measure at all.
