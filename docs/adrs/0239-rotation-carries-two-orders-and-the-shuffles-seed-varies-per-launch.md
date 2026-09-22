# ADR-0239 — Rotation carries two orders, and the shuffle's seed varies per launch

> **Status:** proposed
> **Date:** 2026-09-20
> **Related plan(s):** [0216](../plans/0216-the-operator-owns-the-order.md)

## Context

[Plan 0205](../plans/done/0205-the-library-becomes-navigable.md) made the library navigable and, in
the same stroke, replaced the old *"the roster's successor"* rule with a shuffled traversal of the
eligible set. The argument was good and still is: an unattended show should not repeat a preset
while an unseen one remains, and a remembered-history window cannot defend that property while the
eligible set grows and shrinks between draws.

It closed 2026-09-20. The same day, watching the running app, the owner reported: *"it feels like
presets are changed by random and there is no setting for that."* That is a report against the
behaviour one day after it shipped, from the person the app is built for, and it names two separate
absences:

- **There is no order control anywhere.** Not a `config.toml` key, not a CLI flag, not a settings
  row. The shuffle is the only order, and `Space` inherits it — so the manual key that used to step
  to the next preset now hands out an arbitrary one.
- **`[rotate] source` has no in-app control.** The key exists (`standalone/src/config.rs:335`,
  `"all"` / `"favourites"`) and the director reads it, but nothing in the window can change it. An
  operator who marks favourites cannot act on the marks without editing a file and restarting.

A third fact comes from Plan 0205's own close review, finding 5, which was left open as *"changing
the seed is a code decision"*: the traversal seed is `renderer.preset_names().count()` read
**before** the reload installs the per-user or `RLX_PRESET_DIR` library. That is the embedded
count — one number per build — so **every launch of a build replays one order**. The shuffle is
therefore neither controllable nor, across launches, actually varied.

The constraint pulling the other way is this project's determinism rule: *visual jitter or
randomness, when wanted, is explicitly seeded so a scene is reproducible.* A rotation order that
reaches for a clock collides with it. The rule is about keeping the seam explicit rather than about
forbidding variation, so the decision is where the seed is chosen, not whether one exists.

## Decision

Rotation carries **two orders** — `shuffled`, which stays the default and keeps Plan 0205's
no-repeat property, and `sequential`, which walks the eligible set alphabetically by preset name and
wraps. The operator chooses between them the same three ways auto-rotate is already chosen: a
settings-menu row, a hotkey, and a `config.toml` key that the hotkey writes. `[rotate] source` gains
the same treatment, so `all` and `favourites` become a live choice rather than a file edit. In the
director, `Traversal` keeps the shared trail and the announced `upcoming` and delegates the draw to
an `Order` whose `Shuffled` variant owns the cycle state and the RNG, so the no-repeat invariant is
stated where it is implemented and no field is dead in the other mode.

The seed stays **injected**: `Traversal::new(seed)` is unchanged and the director remains a pure
function of the number it is handed. What changes is who picks it. The shell passes `[rotate] seed`
when the key is set — pinning the walk for a show that wants the same evening twice — and, when it
is absent, a launch-varying value, so the shuffle stops replaying one order per build. Headless and
test constructors keep passing an explicit number and are unaffected.

## Consequences

### Positive

- The manual `Space` key can be made to mean *the next preset* again, which is what it meant before
  Plan 0205 and what an operator stepping through a library expects.
- Favourites become actionable from the window: mark a set with `F1`, switch the source, and the
  show draws from it — the loop the marks were built for, which currently ends at a file edit.
- The shuffle starts varying between launches, which is what it has claimed to do since it shipped.
- The invariant Plan 0205 argued for survives intact, as a property of the `Shuffled` variant rather
  than of the whole type.

### Negative

- **Two orders is a second path through the draw**, and every test that asserts a rotation property
  must now say which order it is asserting. The no-repeat property is no longer a property of
  `Traversal`, and a test that reads as though it were will be silently testing one branch.
- **A varying seed costs cross-launch reproducibility from config alone.** Two launches of one build
  with the same `config.toml` no longer walk the library identically unless `[rotate] seed` is set.
  That matters most on the `--stream` path, where rotation runs even with `auto` off
  (`docs/configuration.md`) and nobody is at the keyboard to notice the order changed; the key is
  the pin, and it has to be documented as such rather than left as an escape hatch nobody finds.
- **Two more hotkeys out of a keymap that is nearly full.** `A B C D F H S` and every digit are
  already bound; `R` and `L` are two of the readable letters left, and a later feature has fewer.
- Sequential order must be computed against the eligible set *at each draw*, not a cached list —
  toggling `hidden` mid-show mutates that set, which is the same hazard Plan 0205 recorded for the
  shuffle bag, in its sequential form.

### Neutral

- Sequential order re-introduces a rule Plan 0205 deliberately removed. It returns as one of two
  options rather than as a reversal, and the default is unchanged.

## Alternatives considered

### Alternative A — Keep the shuffle as the only order; answer the report with documentation

The behaviour is defensible and `docs/running.md` already explains it. Rejected because the report
came from the app's own audience one day after the behaviour shipped, and because the same plan's
close review shows the shuffle is not doing what its documentation claims — a fixed per-build seed
is not the varied walk the page describes. Documenting it more loudly would defend a property the
code does not have.

### Alternative B — Make sequential the default and the shuffle opt-in

Rejected because Plan 0205's argument stands on its own for the unattended show, which is this
app's primary use: a long set with a repeat in it is worse than a long set in a predictable order.
The ask was for a switch, not an inversion.

### Alternative C — One combined row cycling four states

`shuffled-all`, `shuffled-favourites`, `sequential-all`, `sequential-favourites` in a single
settings row. Rejected in the interview: it costs one row, and in exchange the two axes stop being
independently readable — an operator wanting to change only the source has to know which of four
states they are in and which direction lands on the pair they want.

### Alternative D — Reseed every launch with no key to pin it

Simpler, and it fixes the replay. Rejected because it collides with the determinism rule with no
escape: a show that wants the same walk two nights running, or a headless `--stream` run someone
wants to reproduce, would have no way to ask for one. The key costs one `Option<u32>`.

## Notes

The favourites source already falls back to the whole eligible set while nothing is marked
(`standalone/src/director.rs:238`), which is the right behaviour and an invisible one: an operator
who switches the source before marking anything reads `favourites` in the menu and sees the whole
library. Surfacing that state is the plan's problem, not this decision's, but it is the reason the
row cannot simply print the config value.
