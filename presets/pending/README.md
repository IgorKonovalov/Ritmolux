# `presets/pending/` — authored content held out of the shipped set

A preset here is **finished and approved, but not yet shippable** — held back by a known engine or
harness gap rather than by anything wrong with the look.

**Nothing in this directory is embedded.** `core/build.rs` builds the shipped set with a
non-recursive `read_dir` over `presets/` plus a `*.toml` extension filter (ADR-0022), so a
subdirectory is skipped by construction. Files here are version-controlled, reviewable and diffable,
and reach neither the binary nor the behavioral suite.

**Shipping one is a `git mv` into `presets/`**, gated on `cargo nextest run -p lmv-core` — the same
gate every other preset passes. Nothing else has to change.

**To work on one in the running app**, point `LMV_PRESET_DIR` at this directory (ADR-0014); the app
hot-reloads it on a ~150 ms poll exactly as it does the shipped folder.

This directory is **not** a parking lot for work in progress or for looks that failed on their
merits. An entry needs a named blocker, recorded in the preset's own header, and it leaves as soon
as that blocker lifts.

## Held today

| Preset | Blocked by | Leaves when |
|--------|-----------|-------------|
| `fragment_tiledmono.toml` | [ADR-0127](../../docs/adrs/0127-a-tonally-flat-picture-is-a-blot-only-if-it-is-also-structureless.md) — `tonal_flatness` convicts it whatever it is measured against. A duotone has two large populations and `is_lit` removes whichever one is the ground, so the other holds ~94 % of what remains either way: `0.9346` under the old black reference, `0.9413` under the derived ground. **Not a ground problem** — [Plan 0116](../../docs/plans/0116-the-sanity-lens-finds-the-ground.md) Phase 1 measured all three candidate estimators and none repairs it | [Plan 0116](../../docs/plans/0116-the-sanity-lens-finds-the-ground.md) **Phase 9** gives the flatness ceiling a second condition, so a picture is convicted as a blot only if it is also structureless. Phase 8 is a measured stop gate on whether such a statistic exists; if it does not, this row's blocker becomes that phase's recorded outcome |
