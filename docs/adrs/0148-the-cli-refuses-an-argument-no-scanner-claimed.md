# ADR-0148 — The CLI refuses an argument no scanner claimed, against one gated roster

> **Status:** proposed
> **Date:** 2026-08-29
> **Related plan(s):** [0135](../plans/0135-the-show-night-surfaces-stop-lying.md)

## Context

`standalone`'s argument handling grew one scanner at a time. `parse_soak_arg`, `parse_tier_arg`,
`parse_input_args_from` and their siblings are each a `while let Some(arg) = args.next()` over a
fresh iterator, every one of them walking the whole argument list looking for the single shape it
cares about. **No structure anywhere holds the set of recognized flags**, so nothing is in a
position to notice an argument that no scanner claimed.

Measured on the pinned 2026-08-29 show build, both starting the app normally rather than exiting:

| command | outcome |
|---|---|
| `lmv --definitely-not-a-flag` | starts, draws, no diagnostic |
| `lmv --ocs 127.0.0.1:9000` | starts, draws, **publishes no telemetry**, no diagnostic |

The second is the decision-forcing one. A misspelt `--osc` is a running visualizer with a dark
rig, and on a show floor that presents exactly as a cable, a subnet or a controller fault — the
three things an operator checks first, none of which is wrong. The individual scanners *are* strict
about their own values: `--osc=` and a bare `--osc` are both refused, with tests. That makes the
outcome sharper rather than softer, because the app is demonstrably careful about the flags it
recognizes and silent about the ones it does not.

There is also no `--help`. `lmv --help` falls through every scanner unclaimed and starts the
visualizer, so there is no way to check a flag's spelling short of reading `README.md` or the
source. A guard written for the lighting runner tried to probe the flag list by shelling out to
`--help` and **hung the runner**, which is how this was found.

The forcing constraint is that [Plan 0133](../plans/0133-the-engine-drives-the-lights.md) is about
to make the standalone drive physical fixtures over Art-Net. Once the only evidence a flag took
effect is a physical thing in the room being lit, a silently-ignored flag stops being a desktop
annoyance and becomes the failure that reads as a hardware fault.

## Decision

**`main` gains one roster of recognized flag names, consults it in a single pass before any scanner
runs, and refuses to start when an unclaimed `--`-prefixed argument is present** — naming the
offending argument and the nearest roster entry by edit distance. `--help` prints that same roster
and exits 0.

**The roster's duplication is retired by a test, not by discipline.** A `standalone` unit test
extracts every `--`-prefixed string literal reachable from the scanner functions and asserts each
one appears in the roster. The roster stays a plain `&[FlagSpec]` — name, whether it takes a value,
one line of help text — and adding a flag without rostering it fails that test rather than shipping
a flag `--help` does not mention.

Refusal is a hard exit, not a warning. A warning on a show floor is a line in a scrollback nobody
is reading; the operator's own report is that the failure they need to distinguish from a cable
fault is the app running *normally*.

## Consequences

### Positive
- A misspelt flag is a startup error naming the misspelling, not a dark rig.
- `--help` exists, prints the roster, and exits — so a guard can probe it without hanging, and an
  operator can check a spelling without reading source.
- The roster is one place to read what the binary accepts. `README.md`'s flags section becomes
  checkable against it rather than being the only copy.

### Negative
- **A roster is a second copy of a fact the scanners already encode**, and the test that keeps them
  in step is a string-literal scan — a scanner that builds a flag name by concatenation would slip
  past it. The test asserts the direction that matters (every scanner literal is rostered) and
  cannot assert the reverse (every roster entry is reachable), so a retired flag can linger.
- **A hard exit is a behavior change on a path that currently always starts.** Any script or shortcut
  passing a stale flag begins failing at launch instead of being quietly tolerated. That is the
  point, but it will surface on the first run after the change rather than at a chosen moment.
- One more thing to update when a flag is added: three places (scanner, roster, `README.md`) instead
  of two, with only the first two gated.

### Neutral
- The scanners keep their current shape. This adds a pass in front of them and changes none of them,
  which is what keeps the change small enough to make while other plans are live in `main`.

## Alternatives considered

### Alternative A — Have every scanner report the spans it claimed, then reject the remainder
The structurally correct answer, and the one the backlog entry names first: no duplicated roster,
no drift, and the "unclaimed" set is computed rather than declared. Rejected for this pass because
it requires changing the signature and body of every scanner simultaneously, which is a
wide-blast-radius edit to `standalone/src/main.rs` at the same time as
[Plan 0126](../plans/0126-the-large-files-split-along-their-seams.md) Phase 7 is scheduled to split
that file and [Plan 0131](../plans/0131-the-operator-gets-a-console.md) is being built on `main`
inside it. The roster-plus-gate reaches the same user-visible guarantee with a fraction of the
contention, and does not foreclose this — a later plan can compute the roster instead of declaring
it, and the test keeps passing.

### Alternative B — Adopt `clap` (or another argument-parsing crate)
Gets the roster, `--help`, near-miss suggestions and value validation for free, all of them better
than what is proposed here. Rejected on **"lightweight is a feature"**: `clap` with its derive and
suggestion features pulls a non-trivial dependency tree into a binary whose whole argument surface
is roughly a dozen flags, and this project pins direct dependencies to exact versions and justifies
each one. The cost is not the compile time so much as taking a dependency on a shape the app does
not otherwise need. Worth revisiting if the flag surface grows past what one roster reads well.

### Alternative C — Warn on an unclaimed argument and start anyway
The conservative option, and it was rejected by the evidence that produced the entry. The failure
being repaired is that the app **runs normally** while doing less than asked; a warning preserves
exactly that property and adds a line to a log nobody reads on a show night. It also leaves
`--help` unsolved, since a warning gives an operator no way to discover the correct spelling.

## Notes

Discharges [design-backlog 0159](../design-backlog.md). The two commands in the Context section are
that entry's own reduction and are re-runnable against any build, which is what stands in for a
probe here — that no unclaimed argument is rejected is a negative about the whole of `main`'s
argument handling and is not a match count in one file.
