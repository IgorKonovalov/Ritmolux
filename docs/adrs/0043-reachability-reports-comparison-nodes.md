# ADR-0043 — Reachability reports comparison nodes, suppressed only where a `select()` already names them

> **Status:** proposed
> **Date:** 2026-07-29
> **Related plan(s):** [0042](../plans/0042-reachability-sees-every-comparison.md)
> **Supplements:** [ADR-0042](0042-reachability-measured-on-the-expression-tree.md) (the mechanism
> this extends; its advisory-not-gated posture is re-affirmed here, not changed)
> **Closes backlog:** [0028](../design-backlog.md)

## Context

[ADR-0042](0042-reachability-measured-on-the-expression-tree.md) established that preset
reachability is measured on the expression tree rather than inferred from frames, and Plan 0041
shipped it. It works: the first library audit run against it found four presets whose defining
mechanism had never once executed, none of which any frame-differential method could have named.

It is also blind to two shapes, and the 2026-07-29 `preset-author` audit found **five more dead
gates that the check scored as clean** — more than it caught. `Expr::flag_gates` walks the tree via
`collect_flags`, which emits a `GateFlag` for exactly two node shapes:

```rust
(Node::Call(Func::Select, args), NodeObservation::Select { saw_true, saw_false })
    if saw_true != saw_false => …
(Node::Call(Func::Clamp,  _),   NodeObservation::Clamp  { peak_fraction_of_bound })
    if peak_fraction_of_bound < 1.0 => …
```

`NodeObservation` has no variant for a comparison, and `Node::probe`'s `Node::Bin` arm only
*recurses* into its children — `record_select` and `record_clamp` are its only writers. So a
comparison that is not a `select()` condition is never observed at all. Two consequences:

**A bare comparison as the whole binding is invisible.** `reseed = "onset > 0.55"` is the idiomatic
way to write a boolean parameter — there is no `select()` anywhere in it. Combined with `onset`
being raw spectral flux (peak `0.016`, not a `0..1` envelope), **all five attractor presets shipped
without ever reseeding**, and `rose_web.mirror_reflect` had never reflected. Every one scored
`gates 0`.

**A dead band gate hides behind a live `tempo` gate.** In
`select(min(tempo > 124, bass + treb > 0.38), 4, 1)` the flag names the whole `min(...)` as the
condition, and `--report`'s own guidance correctly says a `tempo` gate is one-sided under a
single-BPM probe — so a reader dismisses it. The `bass + treb > 0.38` half is separately
unreachable (the sum peaks near `0.138`) and is never named. The excusable half launders the
inexcusable one, and `swarm_storm` carries the same shape with a *reachable* band half, so the two
are indistinguishable in today's output.

The second is the worse failure: it does not report "unknown", it reports **clean**. This is the
instrument all three lanes verify through, and a false-clean reading is worse than no reading.

## Decision

We will record every **comparison** node — the six operators `> < >= <= == !=` — as a two-valued
observation exactly like a `select()` condition, and report one that never took both values. A
comparison is **suppressed when it is the direct condition of an enclosing `select()`**, because
that `select()` already reports it, in better words: a gate flag names the *consequence* ("its
`then` branch never ran"), which a bare comparison flag cannot.

That single suppression rule resolves both blind shapes without duplicating any existing flag.
`select(bass > 0.3, …)` reports once, as it does today. `reseed = "onset > 0.55"` reports for the
first time, because its comparison is a root, not a condition. And in
`select(min(tempo > 124, bass + treb > 0.38), …)` the two comparisons are children of `min`, not
direct conditions of the `select`, so **both are named individually** — which is precisely the case
that reported clean before.

This ADR changes what is *measured and reported*. It does not change ADR-0042's advisory posture:
reachability stays a tool a human reads, and CI gating remains deferred until a library audit run
against the corrected instrument shows what is actually there.

## Consequences

### Positive
- The two shapes that hid five of nine real dead gates become visible, and the masked-band-gate case
  in particular stops reporting a false clean.
- A bare comparison is the *idiomatic* boolean-parameter form, so this covers a shape authors will
  keep writing rather than one they should avoid.
- Naming both halves of a composite condition separately means the standing `tempo` false positive
  no longer confers immunity on whatever it is `min`ed with.
- The suppression rule is a property of tree position, not of operator or variable, so it needs no
  per-preset annotation and cannot drift out of step with the grammar.

### Negative
- **The `tempo` false positive gets worse before it gets better.** Bare `tempo > N` comparisons
  outside `select()` will now flag too, on top of the 14-of-20 the existing check already produces.
  The signal-to-noise of `--report`'s gate section drops until the library is re-gained, and this
  pushes CI gating further out rather than closer — accepted deliberately (see Notes).
- One more `NodeObservation` variant and one more `record_*` writer on a hot-adjacent path. It stays
  harness-only (`eval_probed` is never called by the render loop), but the file's invariant that
  only two node kinds are observed is now three, and the `Node::Bin` arm gains a branch that must
  distinguish comparison operators from arithmetic ones.
- Reported flag counts are not comparable across the change. Every historical "N dead branches"
  figure in the backlog and in ADR-0042's Outcome section was measured under the old rule; this
  redefines the denominator, which ADR-0042 explicitly refused to do for the *stimulus* columns and
  is accepted here only because a gate flag has no historical numeric series attached to it.

### Neutral
- `Expr::eval` is untouched; probed evaluation still returns exactly `eval`'s value, so ADR-0042's
  no-divergence-by-construction property is preserved.

## Alternatives considered

### Alternative A — report every comparison, including `select()` conditions
Uniform and needs no suppression rule. Rejected because it double-reports every gate the check
already finds: today's 20 dead branches would become roughly 40, half of them the same finding in
two voices, and the duplicate is strictly the less useful of the pair — a comparison flag can say
"never went true" but not "so its `then` branch never ran". A report that repeats itself is a report
people stop reading, which is the failure mode this whole line of work exists to prevent.

### Alternative B — replace `select()` reporting with comparison reporting
Simpler still: drop the `Select` observation entirely and report only comparisons. Rejected for two
reasons. It loses the consequence phrasing above. And a `select()` whose condition is *not* a
comparison — `select(beat, a, b)` is legal and `beat` is a `0`/`1` gate — would stop being reported
at all, trading one blind spot for another.

### Alternative C — leave it, and document the limitation
Cheapest, and defensible if the check were a convenience. Rejected because the failure mode is a
false clean on the instrument every lane self-verifies through: the 2026-07-29 audit had to fall
back to grepping every threshold in the library by hand *after* `--report` said it was healthy. A
documented blind spot in a tool people trust is indistinguishable from a bug in practice.

### Alternative D — infer the dead threshold from measured band ranges instead
Compare each literal threshold against the known peak of the variables in its expression, with no
probe at all — a static check. Rejected because it only works for the trivial shapes (`onset > 0.55`
against a known `onset` peak) and cannot evaluate an arbitrary expression's range without
re-implementing interval arithmetic over the whole grammar, which is a second evaluator to keep in
step — exactly the divergence cost ADR-0042 removed by construction.

## Notes

The gating question is deliberately left where ADR-0042 put it. That ADR said "gate once the library
is clean"; the honest current reading is that **we do not know whether the library is clean** — we
know the check cannot see two whole shapes. Plan 0042 therefore ends with a re-audit, not a gate,
and the decision to gate is taken on that evidence.

The suppression rule is stated in terms of the *direct* condition child so that it degrades safely:
if the grammar later grows a construct that takes a condition (a ternary operator, a `when()`), a
comparison sitting directly under it is reported by the comparison rule until that construct is
taught to report its own gate — noisy, never silent.
