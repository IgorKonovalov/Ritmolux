# `presets/pending/` — authored content held out of the shipped set

A preset here is **finished and approved, but not yet shippable** — held back by a known engine or
harness gap rather than by anything wrong with the look.

**Nothing in this directory is embedded.** `core/build.rs` builds the shipped set with a
non-recursive `read_dir` over `presets/` plus a `*.toml` extension filter (ADR-0022), so a
subdirectory is skipped by construction. Files here are version-controlled, reviewable and diffable,
and reach neither the binary nor the behavioral suite.

**Shipping one is a `git mv` into `presets/`**, gated on `cargo nextest run -p rlx-core` — the same
gate every other preset passes. The one thing that may change on the way out is the **filename**:
the shipped set is named `<system>_<look>.toml` and `ls` is its roster, so a held file named any
other way is renamed as it lands (both authored-path worlds below were).

**To work on one in the running app**, point `RLX_PRESET_DIR` at this directory (ADR-0014); the app
hot-reloads it on a ~150 ms poll exactly as it does the shipped folder.

This directory is **not** a parking lot for work in progress or for looks that failed on their
merits. An entry needs a named blocker, recorded in the preset's own header, and it leaves as soon
as that blocker lifts.

## Held today

**None.** The directory is empty of held content; the record below is what it has carried.

## What left, and why the record is worth keeping

### `shape_maple.toml` and `shape_lion.toml` — shipped 2026-09-09

Two authored-path worlds, a maple leaf and a maned lion mask, each one closed contour of 54 SVG
commands drawn as the shape field's banded emblem. They were the first content anywhere to use a
`[path]` table ([Plan 0092](../../docs/plans/done/0092-the-engine-draws-an-authored-path.md),
[ADR-0107](../../docs/adrs/0107-an-authored-path-is-inline-svg-data-and-it-morphs-by-resampling.md)),
and they were held for **less than a day** — authored, look-gated and blocked on 2026-09-09, released
by that plan's own Phase 7 the same evening.

The blocker was the engine's, not the look's: `[path] d` was read in clip space, y-up, against SVG's
y-down, so **every pasted path arrived mirrored top to bottom** and both files authored `d` inverted
to compensate. Phase 7 negated y at parse and re-flipped both `d` strings in the same commit — the
workaround and the defect had to leave together or the presets would have rendered upside down the
moment the engine was right.

The reason to keep this short entry is that it is the **cheap** case, and it reads as the argument
for the directory rather than against it. Nothing was tuned around the defect except the one string
that had to be; the header named the blocker, so the fix knew exactly what to undo; and the presets
went out under their shipped names on their own merits at the Plan 0092 close ceremony, not as a
by-product of the fix.

### `fragment_tiledmono.toml` — shipped 2026-08-26

It shipped ([Plan 0119](../../docs/plans/done/0119-the-flatness-gate-gets-its-second-term.md)
Phase 4) after being held from Plan 0113. It is the directory's expensive case, and its history is
the argument for the directory existing.

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
