# ADR-0141 — One artifact store serves every lane, and its config lives above the repo

> **Status:** accepted 2026-08-29 (Plan 0129) - carries an `Outcome`
> **Date:** 2026-08-28
> **Amends:** [ADR-0053](0053-plan-lanes-run-in-git-worktrees.md)
> **Related plan(s):** [0129](../plans/done/0129-the-build-stops-being-paid-three-times.md)

## Context

[ADR-0053](0053-plan-lanes-run-in-git-worktrees.md) put plan lanes in git worktrees and listed, as a
positive, that *"parallel lanes do not share a `target/` directory, so two sessions building at once
do not thrash one cargo lock."* That was the right call for concurrency and an expensive one for
everything else, and after seventy-odd plans the bill is legible.

Measured on the development machine on 2026-08-28, with three worktrees live:

| worktree | `target/` |
|---|---|
| `light-music-visualizer` (main) | 2.4 GB |
| `lmv-plan-0123` | 12.7 GB |
| `lmv-plan-0127` | 8.1 GB |
| **total** | **23.2 GB** |

That is one dependency graph compiled three times. `Cargo.lock` resolves **305 crates** — `wgpu` and
`naga` among them — and they are compiled three times *optimized*, because
`[profile.dev.package."*"] opt-level = 2` (Plan 0061 Phase 1b) trades cold build time for WARP test
time. That trade is excellent when paid once and merely expensive when paid per lane. The result is
then linked across **41 integration-test binaries** in `core/tests/`, each its own crate and its own
`link.exe` invocation, on the platform where linking costs the most.

The cost lands hardest where the queue is longest. Seven approved plans are waiting (0120, 0123,
0124, 0125, 0126, 0127, 0128); each opens a lane and pays the cold build before its first edit
compiles. A warm checkout is not spared either: `cargo build` on `main` with no source change of our
own took **2m12s**, because `main` had moved and one dependency plus one workspace crate had to be
rebuilt at `opt-level = 2`.

Nothing here has ever been tuned for this. **There is no `.cargo/config.toml` in the repository at
all** — no shared artifact store, no compilation cache, no linker override. The pinned 1.97.1
toolchain ships `rust-lld.exe` in its sysroot and nothing points at it.

So the decision is not whether worktrees stay — they do, and ADR-0053's isolation, rollback and
parallel-session arguments are untouched. The decision is whether the single property ADR-0053
bought with 23.2 GB and a cold build per lane, namely lock-free concurrent compilation, is worth
that price. Lanes here are worked one at a time; the user has stated they would serialize if it
paid.

## Decision

Every worktree of this repository compiles into **one shared artifact store**, and the
`x86_64-pc-windows-msvc` linker is pointed at the toolchain's bundled `rust-lld`. Both live in a
single `.cargo/config.toml` placed in the **parent directory of the worktrees** (`WORK/`), which
cargo finds by walking ancestor directories from whichever lane is building — so a new lane needs no
setup of its own.

That file is **machine-local and deliberately never committed.** It cannot be: CI's
`Swatinem/rust-cache@v2` caches `./target`, so a committed `target-dir` redirect would break the
cache on every runner; the macOS arm has a different linker story; and `rust-lld` is not on `PATH`,
so reaching it means naming a sysroot path that is specific to one machine. Setup is therefore
**opt-in per machine** — documented, verifiable, and inert when skipped. That is the same shape
[ADR-0033](0033-testing-strategy-coverage-ratchet-and-pre-push-gate.md) already uses for
`git config core.hooksPath .githooks`.

This **amends ADR-0053 in both directions**, and the second direction is the stronger one. It revokes
exactly one of that ADR's stated positives — *"parallel lanes do not share a `target/` directory, so
two sessions building at once do not thrash one cargo lock"* — which is the price named above. But it
also **discharges the first of its Negatives**: *"Disk cost is severe and recurring. Each worktree
carries its own `target/`; the Plan 0049 lane held ~8 GB in `target/debug/incremental` alone and the
disk reached zero bytes mid-session, breaking a build."* That is not a cost this ADR pays around; it
is a premise this ADR removes. ADR-0053 recorded it as the standing consequence of a design choice,
and it stopped being one the moment the artifact store stopped being per-worktree.

Everything else in ADR-0053 stands untouched: the worktree layout, the merge direction, the
five-step close, the shared stash hazard, and the Windows `git worktree remove` trap. Worktrees are
not in question here — only what they each compile into.

## Consequences

### Positive
- **A new lane starts warm.** The 305-crate dependency build is paid once per machine rather than
  once per plan, which is the cost the user named first.
- **Disk stops multiplying.** One store replaces N copies; the 23.2 GB currently held becomes
  roughly one worktree's worth, and ADR-0053's *"the disk reached zero bytes mid-session, breaking a
  build"* failure gets much harder to reach.
- **41 link steps get cheaper**, on every build in every lane, from the linker half of the change.
- **`opt-level = 2` on dependencies stops being a recurring tax.** The Plan 0061 Phase 1b trade was
  argued on WARP test time and silently charged per lane; now it is charged once, which is the
  regime it was argued in.
- **Nothing about what is tested changes.** No suite is skipped, no scope narrowed, no threshold
  moved. This is compilation only.

### Negative
- **Concurrent builds serialize.** Cargo takes an exclusive lock on the store, so a second lane
  building at the same moment waits rather than proceeding. This is the property ADR-0053 bought and
  this ADR sells; it is the whole price.
- **Hopping between lanes re-fingerprints the workspace crates.** Dependencies stay cached, but
  `lmv-core` and its 41 test binaries recompile on each switch between branches. The linker half
  reduces this but does not remove it, and a session that alternates lanes frequently will feel it.
- **`cargo clean` becomes a shared, destructive act.** Run in any lane it wipes the store for all of
  them. Nothing warns; the only defence is knowing.
- **The store grows monotonically.** Cargo never garbage-collects, so artifacts from abandoned
  branches accumulate forever in one directory instead of dying with a deleted worktree. Periodic
  cleanup becomes a real chore rather than a side effect of `git worktree remove`.
- **The configuration is invisible to the repository.** Nothing in the tree records that it is in
  effect, no gate can check it, and two machines can silently disagree about how this project
  builds. Plan 0129 answers this with documentation only, which is weaker than a gate and is
  accepted as such.
- **The config applies to every Rust project under `WORK/`,** not only this one, because ancestor
  discovery does not know about repository boundaries. Cargo namespaces artifacts by package,
  version and feature hash, so this is untidy rather than incorrect — unrelated projects share disk,
  never each other's binaries.
- **One committed script breaks and must be repaired.** `plugin-foobar/build.ps1:31` resolves the C
  ABI staticlib as `$repo\target\release\lmv_core_c.lib`, an assumption the redirect falsifies. The
  foobar plugin would fail to link, on the one path this repository cannot test in CI.

### Neutral
- CI is untouched, because the file is not committed. That is the intent, and it also means CI keeps
  paying the cost this ADR removes locally — a separate question, deliberately not answered here.
- `standalone/tests/shot_cli.rs` already resolves the `examples/` sibling through
  `std::env::current_exe()` and documents itself as robust to `CARGO_TARGET_DIR` (Plan 0032). It
  needs no change; Plan 0129 Phase 4 verifies rather than assumes that.

## Alternatives considered

### Alternative A — sccache underneath per-worktree `target/` directories
A content-addressed compiler cache would leave ADR-0053 entirely intact: every worktree keeps its
own `target/`, no lock is shared, lanes stay genuinely parallel, and cross-lane hopping hits cache
instead of thrashing fingerprints. It is strictly the better answer for a project with concurrent
builders. It lost on three counts: it does not reduce the 23.2 GB (each lane still materializes its
own artifacts), it cannot cache incremental compilation — so it wants `CARGO_INCREMENTAL=0`, trading
the inner-loop speed the user also named as a pain — and it is a tool to install, pin and keep
working on a project whose stated concurrency need is *"sometimes, but I could serialize if it
paid."* Paying complexity for a property we do not use is the wrong trade. It stays the documented
answer if lanes ever genuinely go parallel.

### Alternative B — commit the config with a relative `target-dir`
Cargo resolves relative config paths against the config file's own directory, so a committed
`.cargo/config.toml` with `target-dir = "../.lmv-target"` would land every sibling worktree in one
store with no per-machine setup — and it would be recorded in the tree, which the chosen option is
not. It lost decisively on CI: `Swatinem/rust-cache@v2` caches `./target`, so the redirect would
move artifacts outside what the action saves and every runner would rebuild from scratch on every
push, converting a local win into a CI regression across three jobs and two operating systems. The
`rust-lld` half could not be committed either way, since it needs a sysroot path.

### Alternative C — a warm worktree pool
Keep two or three long-lived pre-built worktrees and `git reset --hard` a new lane onto one, rather
than creating a fresh directory. It needs no configuration at all and kills the cold build outright.
It lost because it trades away the property ADR-0053 valued most and states plainly — *"a lane can
be abandoned by deleting a directory"* — replacing an operation that cannot go wrong with a
destructive reset onto a tree that may hold another lane's uncommitted work. It also does not
address disk, which is the other half of the measured cost.

### Alternative D — discipline: clean finished lanes promptly
ADR-0053 already prescribes removing a finished worktree, and that is not what hurts. Even with
perfect hygiene the *next* lane still pays a full 305-crate cold build, which is the cost the user
named first. Discipline addresses the disk symptom and leaves the time cost exactly where it is.

## Notes

The `rust-lld` half is separable and carries its own risk: it is a different linker, and a
link-time behavioural difference would surface as a test failure rather than a build error. Plan
0129 lands it in its own phase, ahead of the store, so a bisect has somewhere to land.

Whether `lmv-core` itself should compile above `opt-level = 0` — the other half of the suite wall
time, currently forbidden by root `Cargo.toml` because inlining muddies the line mapping ADR-0033's
coverage ratchet is derived from — is **not decided here**. Plan 0129 Phase 5 measures where the
89-second `reactivity` suite actually spends its time and reports; if our unoptimized code turns out
to be a minority of it, the question dies for free.

## Outcome (added at Plan 0129's close, 2026-08-29)

**The Decision stands and is delivered.** A worktree that has never built compiles 3 workspace
crates in 15-24 s with **zero dependencies recompiled**, against 129 crates in 105 s cold; the
store holds 5.4-7.2 GB where one lane's `target/` held 14.65 GB; `rust-lld` took the cold path to
every test binary from 171 s to 145 s and moved no golden; the result set is identical at 1122
tests. Every argument above survives.

**Four of the quantified premises in Context do not**, all measured on the same machine on
2026-08-28 by Plan 0129 Phase 1, and they are recorded here rather than edited away.

| Context claims | Phase 1 measured |
|---|---|
| three live worktrees, 23.2 GB total | **one** live worktree (0123 and 0127 were removed, 0132 closed the same night) |
| that worktree's `target/` at 2.4 GB | **15,002 MB** |
| 305 crates compiled per lane | **129** on a default `cargo build`; 305 is the `Cargo.lock` entry count, and `default-members` excludes `core-cabi` and `milkconv` |
| `cargo build` warm on `main`: 2m12s | a **cold** build takes **105 s** - faster than the figure cited for a warm one |

The user's report that *"a small change is taking hours"* is reproduced by no figure Plan 0129
took: the warm one-file-edit rebuild is **28 s**. So the multiplier this ADR argued from was 1x,
not 3x, and the cold-build tax it names was smaller than stated. **What it got right is the shape
rather than the size** - the cost is real, it is paid per lane, and paying it once per machine is
the correct trade at any multiplier. Nothing in Decision, Consequences or Alternatives turns on
the arithmetic above.

Three smaller corrections. The **Notes** section cites "Plan 0129 Phase 5" for the `reactivity`
measurement; it is **Phase 6**, and that suite measures **126.1 s**, not the 89 s quoted. Its
finding lands on the arm this ADR hoped for: our unoptimized code is **24.1 s, 19.1 %** of the
suite, a minority, **so the `opt-level` question dies here with no ADR owed**. And `core/tests/`
holds **40** top-level integration-test binaries, not the 41 repeated throughout.

**One Neutral claim was half wrong, and it is the finding worth carrying forward.**
`standalone/tests/shot_cli.rs` was recorded here as needing no change. Its `shot_exe()` is indeed
redirect-safe, but its sibling `scratch()` builds output paths as `repo_root().join("target")`,
which no redirect reaches - so a **test run** re-creates a `target/` inside the worktree, holding
`shot-cli-tests/` and nothing else. `**/target` is gitignored, so nothing surfaces it. The
Negative section's *"One committed script breaks"* is likewise an instance of a class:
`packaging/macos/bundle.sh` and the two `renders/plan-0106-p*/run.sh` scripts still resolve cargo
output under `<repo>/target`. Both are filed to the design backlog - entries **0160** and **0161** - rather than fixed at the close.
