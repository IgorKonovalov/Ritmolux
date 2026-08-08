# ADR-0072 — The C ABI ships from its own crate

> **Status:** accepted (2026-08-08, Plan 0061 close)
> **Date:** 2026-08-04
> **Related:** [ADR-0001](0001-rust-core-wgpu-cabi-foobar-shim.md) (the C ABI seam),
> [ADR-0003](0003-c-abi-v1-surface.md) (the ABI surface + versioning),
> [ADR-0033](0033-testing-strategy-coverage-ratchet-and-pre-push-gate.md) (the coverage gate this moves),
> [Plan 0061](../plans/done/0061-the-build-stops-paying-for-what-it-is-not-building.md)

## Context

`core/Cargo.toml` declares `crate-type = ["rlib", "cdylib", "staticlib"]`. Every consumer in the
workspace links the **rlib**: the standalone shell, all 25 integration test targets, and both
examples. The **cdylib** and **staticlib** exist for exactly one consumer — the foobar2000 C++ shim,
which is built by a separate MSVC toolchain through `plugin-foobar/build.ps1`, on demand, and never
in CI (`.github/workflows/ci.yml` has no plugin job).

Cargo emits every declared crate type on every build of the lib target. So each `cargo build`,
`cargo test` and `cargo run` regenerates artifacts nobody in that command's dependency graph reads.

Measured on the development machine (Windows, steady state, one-file touch of
`core/src/render/mod.rs`, three runs each):

| `crate-type` | incremental `cargo build -p lmv-core` |
|---|---|
| `["rlib", "cdylib", "staticlib"]` (today) | ~5.0 s |
| `["rlib"]` | ~2.3 s |

Each of those rebuilds also writes a 412 MB `lmv_core.lib`, a 24 MB `lmv_core.dll` and a 108 MB
`lmv_core.pdb` — roughly 550 MB of disk traffic per edit to the core, for a link that no test and no
`cargo run` will consume. Under [ADR-0053](0053-plan-lanes-run-in-git-worktrees.md) each plan lane
carries its own `target/`, so the cost is paid once per concurrent lane.

This is not a hot-path or correctness problem. It is a **tax on the edit-compile loop**, which is the
thing a maintainer pays on every iteration, and the loop is the reason it is worth an ADR rather than
a comment.

## Decision

We will extract the C ABI into its own workspace member, `core-cabi/`, which depends on `lmv-core`
and is the **only** crate declaring `cdylib` and `staticlib`. `core/src/ffi.rs` moves there wholesale
along with `core/include/lmv_core.h` and the `core/tests/ffi.rs` conformance suite; `lmv-core` drops
to `crate-type = ["rlib"]`.

The move is mechanical because `ffi.rs` already reaches only the public surface of `lmv-core` —
`crate::audio`, `crate::dsp`, `crate::preset`, `crate::render`, all `pub mod` in `core/src/lib.rs`.
No visibility widens to make this compile.

`plugin-foobar/build.ps1` changes its `cargo build --release -p lmv-core` to name the new crate. The
`extern "C"` surface, `LMV_ABI_VERSION`, the header's contents and the threading contract in
[`docs/specs/0001-c-abi.md`](../specs/0001-c-abi.md) are all **unchanged** — this ADR moves where the
ABI is compiled, not what it is. ADR-0003's rule still holds: the ABI version moves only on an
`extern "C"` shape change, and this is not one.

## Consequences

### Positive

- The everyday loop roughly halves (~5.0 s → ~2.3 s measured) and stops writing ~550 MB per core
  edit. Every `cargo test` run gets the same reduction, since the test binaries link the rlib.
- The seam becomes visible in the directory tree. Today "the C ABI is a contract" is a rule in
  `CLAUDE.md` enforced by review; afterwards it is a crate boundary, and a change to the ABI is a
  diff in a crate whose entire purpose is the ABI.
- `lmv-core` becomes buildable and testable without ever invoking a linker for a dynamic or static
  library — relevant on any machine or runner without the MSVC link toolchain warmed up.

### Negative

- **A fourth workspace member.** More manifest surface, another entry for `cargo deny` to walk,
  another crate to remember when adding a workspace-wide lint or profile setting. Small, but real,
  and "lightweight is a feature" applies to the repo's own structure.
- **The coverage gate's denominator changes.** CI runs
  `cargo llvm-cov nextest -p lmv-core --fail-under-lines 88` (ADR-0033). Removing 493 lines of
  `ffi.rs` and its 163-line test from that scope will move the reported percentage in a direction we
  have **not** measured. The floor must be re-derived from the first post-move run rather than
  assumed, and the new crate needs its own gate or its coverage goes unwatched — a real regression in
  what CI defends, and the price of the split.
- **One more place for the plugin build to break.** `build.ps1` and the `.vcxproj` link line name a
  specific artifact path. Getting the new crate's emitted library name wrong fails at C++ link time,
  which no Rust-side check catches — hence Plan 0061's `human` verification phase.
- **The `#[deny]` panic pragma and the hygiene guard follow the file.** `core/tests/hygiene.rs` scans
  a hard-coded set that names `src/ffi.rs`; if the guard is not extended to the new crate, the pragma
  silently stops being enforced on the ABI — exactly the failure mode that test exists to prevent.

## Alternatives considered

**A. Leave it as is.** The status quo costs ~2.7 s and ~550 MB per core edit and nothing is broken.
Rejected because the cost is paid on every iteration by every lane forever, and the fix is a
one-time mechanical move of a file that already compiles against only public API. The 55.7 GB
`target/debug` this repo currently carries is the same tax compounding.

**B. Feature-gate the crate types.** Ship the cdylib/staticlib only under a `cabi` feature.
Rejected because Cargo's `crate-type` is a static property of the `[lib]` target — it cannot be made
conditional on a feature. There is no form of this alternative that exists.

**C. Keep `ffi.rs` in `lmv-core` and make `core-cabi` a re-export shim** (`pub use lmv_core::ffi::*;`).
Rejected because `#[unsafe(no_mangle)] extern "C"` symbols defined in an **rlib** are not guaranteed
to survive into a downstream cdylib/staticlib: the linker is free to drop an object file nothing
references, and a `pub use` creates no reference. Making it reliable means `-C link-arg=/WHOLEARCHIVE`
or a hand-written forwarding stub per entry point — a footgun aimed at the one seam in the project
that fails silently and at C++ link time. Moving the source removes the question entirely.

**D. A separate cargo profile for plugin builds.** Rejected because profiles select optimization and
debug settings, not which crate types are emitted. It addresses none of the cost.

## Outcome (2026-08-08, at Plan 0061's close)

Landed as `d442f7a`. The decision holds unchanged — `lmv-core` is `crate-type = ["rlib"]`, a plain
`cargo build` emits no `lmv_core*.lib` and no `.dll`, and `cargo build -p lmv-core-cabi` emits both.
`LMV_ABI_VERSION` is still `4` and the twelve-function surface is byte-for-byte what it was. Both
Negatives this ADR named were closed rather than accepted-and-forgotten: `core/tests/hygiene.rs`
follows the file, so the panic pragma is still enforced on the ABI (verified by deleting the pragma
and watching the guard fail by name), and `CABI_COVERAGE_FLOOR` gates the new crate so its coverage
did not silently stop being watched.

**Two things the implementation found that this ADR did not anticipate. Both are recorded here
rather than edited into the body above, because both are load-bearing for anyone reading this
decision later.**

**1. `default-members` was required, and this ADR never named it.** Making `core-cabi` a workspace
member is *not* sufficient: a bare `cargo build` builds every member, so the new crate would have
re-emitted on every iteration exactly what the split exists to stop emitting. The root manifest
excludes it from `default-members`, which is what actually delivers the win. The cost is a second
edge this ADR also did not price: `--workspace` becomes load-bearing on every `nextest` and `clippy`
invocation that must cover the ABI, since the bare forms now come back green having tested and
linted nothing of it. `ci.yml` and `.githooks/pre-push` both took `--workspace` in the same plan;
`docs/nfr.md` and both skills' references were swept at this close.

**2. The preferred `lmv_core` lib stem is not merely risky — it is unusable, and for a sharper
reason than the filename clash the plan predicted.** The emitted *files* would not have collided
(`liblmv_core.rlib` against `lmv_core.lib`). The *crate name* does: `core-cabi/tests/ffi.rs` is an
integration test of this crate, and cargo passes both the crate under test and this package's
dependencies as `--extern`. Two `--extern lmv_core=` cannot coexist, `lmv-core` wins, and every ABI
symbol resolves to a crate that no longer defines one — so the ABI would have shipped **untested** to
satisfy a file name. The plan's stated fallback was taken: the stem is `lmv_core_c`, and
`plugin-foobar/build.ps1` was updated in the same commit. There is no `.vcxproj` in this repo; the
plan's reference to one was stale.

**Verified end to end (2026-08-08, Plan 0061 Phase 8).** `plugin-foobar/build.ps1` completes and
`foo_lmv.dll` loads in foobar2000 v2 and renders. This mattered more than a formality: no
`lmv_core_c.lib` existed on disk beforehand, so the ABI crate built from scratch, and the artifact
rename in item 2 above is the **only realistic failure mode this decision had** — the `extern "C"`
surface did not change, so a missed rename would have failed at C++ link time with unresolved
externals rather than anywhere subtler. It linked. Machine-checked alongside: the built DLL is x64
and exports `foobar2000_get_interface`; the core is genuinely statically linked in (the binary
carries `0.44.1`, the scene names and `wgpu`); every import resolves except foobar's own
`shared.dll`, which resolves from the host process directory and is correct.

**The gap this closes is that CI has no plugin job**, so nothing automated would have caught a
broken link — the shim compiles under a separate MSVC toolchain, on demand. That remains true, and
it is why an ADR that moves the ABI's build location owes a manual link check at its close rather
than at some later convenience.
