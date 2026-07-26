# Golden drift fixtures

These TOML files are **test-only frozen fixtures** for the golden drift guard
(`core/tests/golden.rs`), one per `SystemKind` — plus the two `composite_*`
fixtures described at the bottom, which belong to a different guard. They exist to catch **unintended
engine rendering drift** — a shader or scene-math change that silently perturbs
output — by pinning each scene's pixels to a committed baseline PNG under
`core/tests/golden/`.

Decision and rationale: [ADR-0023](../../../docs/adrs/0023-golden-drift-guard-uses-frozen-fixtures.md)
(Plan 0022).

## Do not tune

**These are not shipped presets and must not be tuned for looks.** Editing a
fixture changes its render and invalidates the committed baseline, defeating the
drift guard. The shipped presets in `presets/` are the ones the `preset-author`
lane tunes; they are guarded *behaviorally* elsewhere (`sanity`, `reactivity`,
`animation` — all iterate `default_presets()`), never pixel-pinned here. Each
fixture is deliberately minimal and deterministic (constant or lightly-bound
params) so it draws a non-trivial frame that never needs content tuning.

## Adding a scene

A new `SystemKind` variant makes `golden.rs` fail to compile until you add its
fixture here — the fixture roster is an **exhaustive `match SystemKind`** with no
wildcard arm. To add one:

1. Author `<system_name>.toml` here (mirror the header comment of the others).
2. Add the variant's arm to `fixture()` in `golden.rs`, and the variant to the
   `SYSTEMS` list.
3. Bless the baseline on Windows WARP:
   `LMV_BLESS=1 cargo test -p lmv-core --test golden`, then eyeball the new PNG
   under `core/tests/golden/` to confirm the scene actually drew.

Baselines are WARP-only (macOS skips per ADR-0016) and must be blessed on WARP or
they will drift.

## The `composite_*` fixtures are a different guard

`composite_trails.toml` and `composite_kaleido.toml` are **not** part of the
per-`SystemKind` roster and `golden.rs` never reads them. They belong to
`core/tests/composite.rs` (Plan 0035 Phase 2), and they exist because **no
fixture bound `trails` or `kaleido_*`** — so the entire post-composite path was
covered by no capture in the suite, which is how a defect that stretched the
whole frame shipped green (ADR-0037).

Two things about them differ from the rest of this directory, both deliberate:

- **They are captured at 160x100, not at `golden.rs`'s square 128.** The post
  stages round each grid axis up to a 256 px step, so 160x100 takes a 256x256
  grid — aspect 1.0 against the target's 1.6. A square or 16:9 size is returned
  aspect-exact by the policy and would make the guard blind, which is exactly why
  the defect survived at 1920x1080. **Do not "tidy" that size.**
- **`composite_kaleido.png` pins a known defect on purpose** (design-backlog
  0010, a Plan 0018 Phase 7 clamp artifact). Its header says what will and will
  not be visible and why. Fixing 0010 moves that baseline; re-bless it then.

They are otherwise governed by everything above: do not tune, bless on WARP,
eyeball before committing.
