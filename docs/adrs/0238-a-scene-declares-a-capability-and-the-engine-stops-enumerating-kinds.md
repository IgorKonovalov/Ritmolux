# ADR-0238 — A scene declares a capability, and the engine stops enumerating kinds

> **Status:** accepted 2026-09-23
> **Date:** 2026-09-20
> **Related plan(s):** [Plan 0215](../plans/done/0215-the-wide-seams-narrow-and-a-guard-holds-them.md)

## Context

`Scene` (`core/src/render/scenes/mod.rs`) is the engine's scene seam — layer 2 of
[ADR-0002](0002-layered-preset-architecture.md), which committed to keeping it thin: *"the preset
engine's vocabulary, not a public plugin API"*. It has since grown to eighteen methods, sixteen of
them carrying a default body.

Six of those methods have **exactly one implementor** across the fourteen systems. Read as of
2026-09-20:

| method | sole implementor | production caller |
|---|---|---|
| `set_per_vertex` | `warp_mesh` | `render/evaluate.rs` |
| `set_param_series` | `lines/spectrum` | `render/evaluate.rs` |
| `feedback_field` | `warp_mesh` | `render/milk_wash.rs` |
| `set_feedback` | `particles` | `render/roster.rs` |
| `sample_budget` | `particles` | **none** |
| `active_sample_count` | `particles` | **none** |

The last two are reached only by test assertions. They are trait surface that exists so a test can
interrogate one concrete scene through `dyn Scene`.

A default body means no scene is forced to stub anything, so this is **not** a Liskov break: every
implementor is substitutable. The cost is different and it is real. The trait cannot answer *whether
a scene has a capability*, so a caller that needs to know recovers the answer by matching on
`SystemKind` outside the trait. `draws_through_shared_line_renderer` is that shape: a free function
enumerating all fourteen kinds to decide whether two scenes share a `LineRenderer`. Each such
function is individually defensible and the set of them is what erodes the seam — a fifteenth system
must today be understood in the factory, in the shared-renderer predicate, and in whichever of the
eighteen methods apply to it, with nothing telling an author which.

One constraint bounds the fix, and it is the reason this is a decision rather than a cleanup.
**`shares_resources` is asked of a roster preset, not of a live scene.**
`Transition::pair_shares_resources` (`core/src/render/transition.rs`) reads
`self.roster.presets.get(from).map(|p| p.system)` — a preset at a roster index whose scene may never
have been constructed. No method on a live `&dyn Scene` can answer it.

## Decision

**A capability that a live scene has becomes a narrow trait the scene implements; a static fact about
a kind stays a kind fact, and moves into one table instead of accumulating as separate matches.**

Three parts:

1. **Four capability traits**, one per surface that is genuinely reached through `dyn Scene` by one
   consumer: per-vertex binding, series binding, feedback source, feedback sink. `Scene` exposes each
   as an accessor returning `Option<&dyn …>` (or `Option<&mut dyn …>`) whose default is `None`. A
   scene that has the capability implements the trait and returns `Some`; every other scene is
   unchanged and says so explicitly.

2. **`sample_budget` and `active_sample_count` leave the trait.** They have no production caller.
   They become inherent methods on the particles scene, and the tests that assert them reach that
   type directly rather than through the seam.

3. **The render-side static facts about a `SystemKind` consolidate into one exhaustive table** in
   `render/scenes/`, returning a struct. `draws_through_shared_line_renderer` becomes a field of that
   struct rather than a match of its own. It stays exhaustive and stays in `render/` — the fact is a
   render implementation detail and does not belong in the preset schema — but the next static kind
   fact extends a struct instead of adding a fifteenth fourteen-arm match.

The accessor default is `None` rather than a no-op body, which is the behavioural half of this
decision: **a capability asked of a scene that lacks it becomes observable at the call site instead
of being silently swallowed.**

## Consequences

### Positive

- The trait states what every scene does. A capability is visible as a capability, and an author of a
  fifteenth system is told by the compiler and the types which surfaces apply.
- The kind-enumeration stops multiplying. One table, extended by a field, replaces a growing set of
  independent exhaustive matches.
- A binding to a capability a scene lacks is a `None` a caller can report, not a default that returns
  and leaves no trace. The preset loader already rejects undeclared params
  (`preset/schema/load.rs`), so this closes the gap behind that check rather than replacing it.

### Negative

- **An accessor per capability is more trait surface than a method, not less, counted crudely.**
  Four accessors replace four methods; the win is in what they *say*, not in the count. Anyone
  reviewing this by counting methods will conclude it achieved nothing.
- **Two dispatch hops** where there was one: `scene.as_feedback_sink()?.set_feedback(cfg)`. These are
  per-preset and per-binding calls, not per-vertex ones, so the cost is not on the frame's inner
  loop — but it is not zero either, and `set_per_vertex` is the closest of the four to a hot path.
- The `None` arm is a new path that did not exist. Where the old default silently did nothing
  correctly, a caller must now decide what `None` means, and a caller that writes `let _ = …` has
  reproduced the old behaviour with more words.
- The consolidation in part 3 is a table a future edit can still grow past usefulness. It moves the
  failure mode rather than removing it; Plan 0215's guard is what makes the growth visible.

### Neutral

- No preset, no `.toml`, no parameter and no rendered frame changes. This is a seam change with no
  user-visible half; the golden suite is the evidence of that.

## Alternatives considered

### Alternative A — Make the shared-renderer question a `Scene` method

The obvious symmetry: if a capability is a trait method, so is *"do you draw through the shared
renderer"*. Rejected because it cannot be implemented. `pair_shares_resources` asks about a roster
preset that may have no constructed scene, and the veto it feeds exists precisely to decide whether
two scenes may be alive at once — the question is asked before the answer's subject exists.

### Alternative B — Leave the trait, document the union

Write the capability clusters into the trait's doc comment, record why the seam is wide, change no
code. Rejected because the documented version still cannot answer a capability question, so the
kind-enumeration keeps growing for the same reason it grew — and this repository's own history is
that a convention without a carrier regrows (the `roster:begin` cap in
[ADR-0116](0116-an-index-row-is-a-pointer-and-a-gate-holds-it-to-one.md) exists for exactly this).

### Alternative C — Downcast through `Any`

Keep `Scene` minimal and let a caller `downcast_ref::<WarpMeshScene>()` when it needs a capability.
Rejected because it reintroduces the concrete coupling the trait exists to remove, and moves the
check from compile time to run time: a renamed or replaced scene type compiles and returns `None`
forever.

### Alternative D — One `Capabilities` struct of function pointers per scene

A scene returns a record describing what it supports. Rejected as a vtable rebuilt by hand: it is
what `dyn Trait` already is, with the compiler's checking removed.
