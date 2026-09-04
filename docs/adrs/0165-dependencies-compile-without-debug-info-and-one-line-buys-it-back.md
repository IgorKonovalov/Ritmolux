# ADR-0165 — Dependencies compile without debug info, and one line buys it back

> **Status:** proposed
> **Date:** 2026-09-04
> **Related plan(s):** [0153-the-debug-tree-stops-carrying-dependency-line-tables](../plans/0153-the-debug-tree-stops-carrying-dependency-line-tables.md)

## Context

[ADR-0141](0141-one-artifact-store-serves-every-lane.md) pointed every worktree at one artifact
store and [ADR-0147](0147-the-shared-artifact-store-is-revoked-and-the-linker-stays.md) revoked it,
restoring a per-lane `target/`. ADR-0147's Negative states the price and the defence in one breath:
*"Disk multiplies again, at roughly 7-15 GB per live lane ... the defence is its own prescription -
remove a finished lane's worktree - which is discipline and not a gate."*

That framing has a gap, and a measurement on 2026-09-04 found it. **The cost is not only per lane.**
The main checkout alone, with no worktree live, held **24 GB**: 13 GB in `target/debug/deps`, 8.2 GB
in `target/debug/incremental`, and the remainder spread across `release/`, `doc/` and test scratch.
Removing a finished lane's worktree recovers none of it. The prescription ADR-0147 relies on does
not reach the checkout the work is actually done in.

Inside that 13 GB, one component dominates and is separable. This workspace builds **46 integration
test targets** across `core/tests/` and `standalone/tests/`, and cargo links each into its own
executable carrying the whole dependency graph — wgpu, naga, winit, windows-rs. On Windows the MSVC
toolchain emits a separate `.pdb` per linked binary with no split-debuginfo packing, so debug info
is duplicated per target rather than shared. The duplication is the graph's, not the tests':
`core/tests/easing.rs` is pure-math with no GPU work and its `.pdb` is **40.5 MB**, the same order as
`warp_mesh` at 45 MB.

A controlled measurement isolates the dependency share. Adding `debug = 0` to the existing
`[profile.dev.package."*"]` block and rebuilding one test binary, with no other change:

| `easing` test binary | `.pdb` | `.exe` |
|---|---|---|
| `debug = "line-tables-only"` (inherited from `profile.dev`) | 40.5 MB | 13.29 MB |
| `debug = 0` on dependencies | 15.0 MB | 10.53 MB |
| | **-63 %** | -21 % |

**25.5 MB of every linked test binary is dependency line tables.** Across roughly 50 linked
artifacts that is ~1.2 GB per build generation, and cargo retains stale generations indefinitely —
the `deps/` directory held 4 to 8 hash variants of most targets, none older than fourteen days.

One thing had to be checked rather than assumed, because getting it wrong would move the ADR-0033
coverage ratchet's floor. Cargo's `"*"` glob applies to dependencies and **not** to workspace
members, so `rlx-core` keeps `profile.dev`'s `line-tables-only`. Verified empirically rather than
from documentation: `librlx_core.rlib` came out at 52.9 MB under `debug = 0`, against 52.4-53.9 MB
across the four preceding builds. Unchanged. The line mapping the ratchet is derived from is
untouched.

Measurements were taken on `x86_64-pc-windows-msvc`, rustc 1.97.1, on the machine described in
`CLAUDE.md`'s linker-override section. The percentages are a reading from one configuration, not a
property of the toolchain.

## Decision

We will set `debug = 0` under `[profile.dev.package."*"]` in the root `Cargo.toml`, so every
dependency compiles with no debug info while every workspace crate keeps `profile.dev`'s
`line-tables-only`. The setting is **committed**, not machine-local: unlike the `rust-lld` override
it names nothing machine-specific, and a build profile that silently differs between two clones is
the hazard class ADR-0147 was written about. When a session genuinely needs to symbolize a frame
inside a dependency, the escape hatch is to delete that one line and rebuild — a documented,
reversible ~87 s round trip, not a rediscovery.

## Consequences

### Positive
- **A measured 63 % cut to each linked test binary's `.pdb`**, ~1.2 GB per build generation across
  the workspace's ~50 linked artifacts, multiplied by however many stale hash generations `deps/`
  is holding at the time.
- **Every link writes fewer bytes**, on a workspace whose cold path links 46 test binaries.
- **It costs one line and no structural change.** Nothing about the test layout, the nextest
  profiles, the gates or the crate graph moves — which is what makes it separable from the two
  larger piles below.

### Negative
- **Backtraces lose line numbers inside dependency frames** — wgpu, naga, winit, windows-rs. A wgpu
  validation failure is normally diagnosed from its message rather than a backtrace line, which is
  what makes this affordable, but it is a real loss on the day it is not.
- **The escape hatch is not free.** Toggling the line invalidates the dependency graph's fingerprint,
  so each direction costs a full rebuild of dependencies at `opt-level = 2` — 87 s measured on the
  reference machine.
- **This is the smallest of the three known levers by total bytes.** The incremental caches
  (8.2 GB accumulated in a single day) and the stale hash generations in `deps/` are each larger.
  This ADR deliberately takes the one with no structural cost and leaves those to
  [backlog 0183](../design-backlog.md) and [0184](../design-backlog.md), which means
  the disk problem is reduced here and not solved.
- **The dependency profile block now carries two settings argued from different evidence.**
  `opt-level = 2` is Plan 0061 Phase 1b's, argued from WARP test time; `debug = 0` is this ADR's,
  argued from artifact size. The block's comment has to keep both straight, and the existing comment
  warns specifically against widening `opt-level` to workspace members — a warning that does not
  transfer to `debug` and must not be read as if it did.

### Neutral
- **CI writes fewer bytes and gains nothing durable.** Runners are ephemeral, so there is no
  accumulation there to prevent; the change is neither a win nor a cost in that lane.
- **The ADR-0033 coverage ratchet is unaffected**, verified above rather than assumed.

## Alternatives considered

### Alternative A — Keep `line-tables-only` and sweep the build tree periodically
Recover the space with a scheduled `rm -rf target/debug/incremental` and a stale-generation prune
instead of producing less of it. Rejected because sweeping is exactly the discipline-not-a-gate
defence ADR-0147 already named, and the 24 GB checkout is what that defence produced in practice.
It also recovers nothing on the *next* build: the 25.5 MB per binary is re-emitted every link,
whereas this decision stops emitting it. The two are complementary rather than competing, and
sweeping remains worth doing — as [backlog 0184](../design-backlog.md), not as a substitute
for this.

### Alternative B — A machine-local `[profile]` block in `WORK/.cargo/config.toml`
Follow the `rust-lld` precedent exactly: opt-in per machine, inert when absent. Rejected because
the precedent does not transfer. The linker override is machine-local out of necessity — reaching
`rust-lld` any other way means naming a sysroot path that exists on one machine, and the macOS arm
has a different linker story. A `debug` setting has no such constraint: it is portable, reviewable,
and belongs in the file that already carries the profile it modifies. Making it machine-local would
mean two clones building measurably different artifacts with nothing surfacing the difference, which
is the precise failure mode ADR-0147 was written to end.

### Alternative C — Exempt wgpu and naga, drop debug info for everything else
Preserve GPU-layer debuggability by scoping `debug = 0` to the rest of the graph. Rejected because
wgpu and naga are plausibly most of the 25.5 MB, so the exemption forfeits most of the win to
protect a layer whose failures surface as validation messages rather than backtraces. The documented
escape hatch covers the rare session that needs those frames at a far lower standing cost, and a
per-package exemption list is a thing that rots as the graph changes.

## Notes

**Not an alternative, a complement.** A partial merge of the test targets — the nine suites named by
`binary()` predicates in `.config/nextest.toml` stay separate, the other 37 fold into one — would
remove 36 links and 36 `.pdb` files outright, and it composes with this decision rather than
competing. It is structural, touches
[ADR-0156](0156-the-per-phase-gate-is-scoped-and-the-suite-is-owed-once-per-plan.md)'s design, and
is filed as [backlog 0182](../design-backlog.md) rather than bundled here.

The 24 GB reading and every figure in the Context section come from the 2026-09-04 session recorded
in Plan 0153's Context. The reclaim performed that day — stale `lmv_*` artifacts from the crate
rename, the incremental tree, and pre-fix `shot-cli-tests` residue — took the checkout to 15 GB
before any of this landed, so the baseline a future measurement should compare against is 15 GB and
not 24.
