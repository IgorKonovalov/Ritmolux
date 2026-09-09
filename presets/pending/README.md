# `presets/pending/` — authored content held out of the shipped set

A preset here is **finished and approved, but not yet shippable** — held back by a known engine or
harness gap rather than by anything wrong with the look.

**Nothing in this directory is embedded.** `core/build.rs` builds the shipped set with a
non-recursive `read_dir` over `presets/` plus a `*.toml` extension filter (ADR-0022), so a
subdirectory is skipped by construction. Files here are version-controlled, reviewable and diffable,
and reach neither the binary nor the behavioral suite.

**Shipping one is a `git mv` into `presets/`**, gated on `cargo nextest run -p rlx-core` — the same
gate every other preset passes. Nothing else has to change.

**To work on one in the running app**, point `RLX_PRESET_DIR` at this directory (ADR-0014); the app
hot-reloads it on a ~150 ms poll exactly as it does the shipped folder.

This directory is **not** a parking lot for work in progress or for looks that failed on their
merits. An entry needs a named blocker, recorded in the preset's own header, and it leaves as soon
as that blocker lifts.

## Held today

**None.** Both entries left on 2026-09-09.

`path_maple.toml` and `path_lion.toml` — the first authored content anywhere to use a `[path]` table
(Plan 0092, ADR-0107), a maple leaf and a maned lion mask, each one closed contour of 54 SVG commands
drawn as the shape field's banded emblem. They passed the look gate in the running app on 2026-09-09
and were held the same day, because both authored `d` with the figure inverted to compensate for the
engine reading an SVG contour in clip space.

Plan 0092 Phase 7 negated y at parse and re-flipped both files' `d` in the same commit, which is
exactly the condition the row below named. **The blocker is discharged and neither file carries a
workaround any more**; both stay here only until the content lane judges the look on its merits,
which is a curation call rather than an engine one.

| Preset | Was blocked by | Discharged |
|--------|----------------|-----------|
| `path_maple.toml` | The authored-path Y-flip: `[path] d` was read in clip space (Y-up) against SVG's Y-down, so `d` was authored inverted | Plan 0092 Phase 7, 2026-09-09 — the parser negates y and both `d` strings were re-flipped in that commit |
| `path_lion.toml` | The same Y-flip | The same |

### What left, and why the record is worth keeping

`fragment_tiledmono.toml` shipped on 2026-08-26 ([Plan 0119](../../docs/plans/done/0119-the-flatness-gate-gets-its-second-term.md)
Phase 4) after being held from Plan 0113. It is the only entry this directory has ever had, and its
history is the argument for the directory existing.

It was blocked by exactly one number — `tonal_flatness = 0.9413` against a `0.90` ceiling — while
clearing every other gate. Three attempts to treat that as a measurement error failed:
[ADR-0126](../../docs/adrs/0126-the-sanity-lens-measures-departure-from-the-frames-own-ground.md)
read it as a ground problem and [Plan 0116](../../docs/plans/done/0116-the-sanity-lens-finds-the-ground.md)
Phase 1 falsified that against all three candidate estimators; Phase 8 then measured three candidate
structural statistics and killed all three;
[ADR-0129](../../docs/adrs/0129-the-structural-term-is-measured-at-composition-scale-not-pixel-scale.md)
proposed a fourth and Plan 0119 Phase 1 falsified *it*. What finally released the preset was ADR-0129's
**secondary** finding — that a conjunction's second term is only ever judged over the frames that
failed the first — which turned one of the three "failed" candidates into the one that ships. See
[ADR-0130](../../docs/adrs/0130-the-structural-term-is-boundary-density-and-conditioning-the-population-is-what-made-it-work.md).

Two things that cost real time are worth carrying forward. **The preset never changed**; every
attempt was aimed at the gate, which is what the blocker being *named* bought. And **the held frame
became a calibration anchor** for the constant that released it, so it is frozen into
`core/tests/sanity.rs` and its shipped header says not to retune it casually.
