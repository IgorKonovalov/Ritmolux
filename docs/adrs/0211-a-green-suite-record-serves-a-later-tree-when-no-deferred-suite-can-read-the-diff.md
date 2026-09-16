# ADR-0211 — A green suite record serves a later tree when no deferred suite can read the diff

> **Status:** accepted
> **Date:** 2026-09-16
> **Related plan(s):** [0191](../plans/done/0191-a-green-tree-is-not-tested-four-times.md)
> **Amends:** [0207](0207-a-suite-run-the-conductor-observed-green-is-not-run-again-on-the-same-tree.md)
> (the ledger key), and rests on [0156](0156-the-per-phase-gate-is-scoped-and-the-suite-is-owed-once-per-plan.md)
> + [0157](0157-the-preset-sweeps-split-per-preset-and-the-phase-tier-samples-a-declared-representative.md)
> for what `-P fast` defers

## Context

[ADR-0207](0207-a-suite-run-the-conductor-observed-green-is-not-run-again-on-the-same-tree.md) keyed
the suite ledger on the worktree's whole tree. Every commit changes the tree, so every conductor
stage whose tree moved runs the full workspace suite again — 737-767 s each on the reference machine.
A clean plan with no fix round pays it at least twice, and what separates the two trees is rarely
code. Measured on 2026-09-15: **0175 spent 58 min in full suites against 24 min of sessions**
(`pre-review` 11.2, a red `post-close` 10.8, `post-close` 11.4, `remerge` 12.3); 0177, which met
ADR-0207's bound of two, spent 24.5 min against 64 min of sessions. The `remerge` run fires whenever
`main` moved after the close, even by a commit touching only `tools/conductor/`.

Three facts decide the shape.

**The full suite and `-P fast` differ by exactly nine named suites.** ADR-0156 defines that set once,
as the `fast` profile's `default-filter` in `.config/nextest.toml`, and ADR-0157 unions a 24-preset
sample back into three of them. Every consumer — the pre-push hook, CI's `check` job, the `dev` lane's
per-phase gate — cites `-P fast` rather than restating it. So "what the full suite adds" is not a
judgement; it is a list with one definition.

**CI never runs those nine.** The runners have no GPU (ADR-0016), so `ci.yml` runs
`cargo nextest run --workspace -P fast`. The conductor's own gate is the only place the nine meet a
finished tree. There is no backstop under a blanket skip, which is why this decision cannot be one.

**A close tip's merge can now carry another lane's render work.** Lane `b` was re-opened on
2026-09-16, the same day Plan 0190 retired the question, and the note retiring it said it re-opens at
no cost. The entry's premise — that a close tip differs from the reviewed tree "only in prose, a
version and a merge" — stops holding the moment two lanes merge into one `main`.

The hazard any shape must answer is that *"only prose changed"* is not *"no test reads it"*:
`hygiene.rs` scans documents, `preset.rs` checks the generated block in `presets/README.md`, and the
version bump reaches every crate reading `CARGO_PKG_VERSION`.

## Decision

**A green full-suite record for tree A also serves tree B when every path in `git diff --name-only A
B` is on a declared list that no deferred suite can read. When it serves, the gate runs `-P fast` in
the full suite's place — never nothing.**

That last clause is the whole safety argument, and it is why the hazard above does not bite. **This
decision never skips a test outright.** The only question it ever answers is *full suite or
`-P fast`*, and `-P fast` is what CI runs on every push to `main`. `hygiene.rs`, `preset.rs` and every
`CARGO_PKG_VERSION` reader are in `-P fast`; so is ADR-0157's 24-preset sample. The floor of this
decision is CI's own floor, and what is traded away is only the nine GPU suites' full sweep on a tree
whose diff cannot reach them.

The gate's suite step therefore has three states, and the ledger line says which one happened:

| State | Condition | What runs |
|---|---|---|
| `skipped` | a green record for **this exact tree** | nothing (ADR-0207, unchanged) |
| `served` | a green record for tree A, and the diff A→B is entirely served paths | `cargo nextest run --workspace -P fast` |
| `ran` | neither | `cargo nextest run --workspace` |

**The list is an allowlist, and that direction is load-bearing.** A path is served only by being named;
anything unlisted falls through to the full suite. Written the other way round — a denylist of
`.rs`, `.wgsl` and `presets/**` that re-arms the suite — a path type nobody thought of would be
silently under-gated, and the failure would be invisible. An allowlist that forgets a path is merely
slow. The served set is:

- `docs/**`, `.claude/**`, `tools/**`, `site/**`, `studio/**`, `packaging/**`, `renders/**`
- any `*.md` anywhere in the tree
- `Cargo.toml` and `Cargo.lock` **only when their diff touches nothing but the version line** — the
  shape a `cargo release` bump produces, and no more

Everything else — every `.rs`, every `.wgsl`, `presets/*.toml`, `core/tests/goldens/**`,
`.config/nextest.toml`, `.github/**` — is unserved by omission, so a diff containing one runs the full
suite. Because the diff is measured **against the green tree** rather than against the close's own
commits, a `git merge main` that brought in another lane's render change re-arms the suite by
construction; one that brought in only documents does not.

**The scope is the conductor's gate alone.** A session's own wrapped suite keeps ADR-0207's exact-tree
lookup and gains nothing here, so `docs/specs/`, the three lanes' `## Conductor mode` sections and the
`with-lock` contract are all untouched — which also keeps the implementing plan free of any `.claude/`
phase and therefore runnable under the conductor (ADR-0210).

## Consequences

### Positive

- **A plan pays the full suite once rather than two to four times.** On 0175's shape that is roughly
  35 of its 58 suite-minutes returned; on 0177's, 13.6 of 24.5. With a lane serialized behind it,
  those are also minutes the next plan in the queue was waiting.
- **The two-lane case is answered by construction rather than by a rule anyone has to remember.** The
  diff is against the green tree, so who moved `main` and why is not a question the gate has to ask.
- **It retires backlog 0227's fourth shape.** The operator rule — *nothing is committed to `main`
  while a closed plan waits for its fast-forward* — existed to stop a tools-only commit costing a
  12.3-minute remerge. A tools-only commit is now a served diff, so the rule buys nothing and is not
  adopted.
- **The nine suites still run against every tree that could have changed them**, and against the
  merged result at the next plan's `pre-review` regardless.

### Negative

- **The served list is a list, and lists go stale.** A future suite that reads something under
  `tools/` or `docs/` at runtime would be under-gated on a served tree, and nothing would say so. The
  allowlist direction bounds the damage to paths someone deliberately named, and the implementing plan
  owes a test that every deferred suite's own source sits outside the served set — but what a test
  *reads at runtime* cannot be asserted from its path. This is the price, and it is the reason the
  list is short and boring.
- **`-P fast` needs a GPU, so a served tree is not free.** ADR-0157's sample makes `-P fast` cost more
  than it did; the saving is the difference between the tiers, not the whole suite.
- **Everything ADR-0207 already put outside the key stays outside it** — gitignored inputs, the GPU
  adapter, the installed toolchain. A served tree inherits that risk twice over, once for the green
  record and once for the diff.
- **A red `-P fast` on a served tree is a weaker signal than a red full suite**, and the operator has
  to read the ledger line to know which tier produced it. That is why the line names the state and the
  tree it leaned on rather than printing the same `skipped` it prints today.

## Alternatives considered

### Alternative A — Make the suite cheaper instead of skipping it
`reactivity` (1566 s), `animation` (1291 s) and `sanity` (1099 s) are 54 % of 7378 test-seconds and
grow with every shipped preset; sharing one headless renderer per binary, or sampling at the gate and
covering the library whole nightly, attacks the cost itself rather than routing around it. Rejected
**here, not at all** — it is `core/tests/` work with a different blast radius and its own measurement,
and it does not stop one unchanged tree being tested four times. It is backlog 0239.

### Alternative B — A declared list of paths, consulted as a denylist
The same list written the other way round: any `.rs`, `.wgsl`, preset or `Cargo.lock` in the diff
re-arms the suite, everything else serves. Rejected for its failure direction. A denylist that omits a
path *skips a suite that should have run*, silently and with no artifact saying so; an allowlist that
omits a path merely runs a suite it need not have. Given that CI cannot catch the nine, only one of
those two is affordable.

### Alternative C — Always run the full suite when `main` moved since `pre-review`
Simple and strict, and it answers the two-lane case without a path list at all. Rejected because it
re-arms on exactly the case that motivated the entry: a commit touching only `tools/conductor/` cost
0175 a 12.3-minute remerge, and this rule keeps that cost while buying nothing the diff test does not
buy more precisely.

### Alternative D — Narrow the ledger key to a hash of code-reachable paths
Instead of comparing two trees, key the record on a digest of the reachable subset, so two trees
differing only in prose share a key outright. Rejected as the same rule with a worse record: the
ledger line would name a synthetic key that matches no git object, so nobody reading it afterwards
could say which tree was actually tested — and ADR-0207's whole value is that the key names something
`git` can resolve.
