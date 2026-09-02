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

**Nothing.** The directory is empty of presets and that is the state to keep it in — an entry here
is a preset the library cannot have yet, not a shelf.

The mechanism above is live and the directory keeps its purpose with nothing in it: the next preset
blocked by a named engine or harness gap lands here, with its blocker in its own header and a row in
the table below.

| Preset | Blocked by | Leaves when |
|--------|-----------|-------------|
| *(none)* | | |

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
