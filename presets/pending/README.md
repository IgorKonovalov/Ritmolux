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
| `fragment_tiledmono.toml` | [design-backlog 0128](../../docs/design-backlog.md) / [ADR-0126](../../docs/adrs/0126-the-sanity-lens-measures-departure-from-the-frames-own-ground.md) — `sanity` excludes black as unlit, so a black-and-white look is measured on its white alone and reads `flatness = 0.9346` against a `0.90` ceiling | [Plan 0116](../../docs/plans/0116-the-sanity-lens-finds-the-ground.md) Phase 3 lands, whose done-when names this preset by name |
