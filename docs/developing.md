# Developing

How to build the project from a checkout and what the local gate runs before a push. This is the
contributor's page; if you are looking for what the application does, start at
[Running the app](running.md).

## Building

A recent stable **Rust** toolchain (the workspace is edition 2024 — Rust 1.85+) and, for the
documentation gates, **Node**. From the repo root:

```sh
cargo build                                          # the everyday build
cargo run -p standalone --release                    # the app
cargo nextest run --workspace                        # the whole suite
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

`--workspace` is load-bearing on the test and lint rows rather than a flourish: the C ABI crate sits
outside the workspace's default members, so a bare `cargo nextest run` skips the ABI conformance
suite and a bare `cargo clippy` stops linting the C ABI, and both come back green while covering
nothing.

The headless capture CLI and the visual-QA harness have their own page,
[Headless capture and video](capturing.md).

### A fresh Arch Linux checkout

Everything below uses Arch package names. `libpulse`, `pkgconf`, `wayland`, `libxkbcommon`,
`ffmpeg`, `nodejs` and `python` are usually present already on a desktop install:

```sh
sudo pacman -S --needed rustup vulkan-swrast vulkan-tools \
  cargo-nextest cargo-release cargo-deny uv \
  libpulse pkgconf wayland libxkbcommon ffmpeg nodejs npm python
rustup show                          # inside the checkout: installs the toolchain rust-toolchain.toml pins
git config core.hooksPath .githooks  # the pre-push gate, opt-in per clone
npm --prefix studio ci               # the studio's dependencies, which the hook's studio step needs
cargo build
```

`vulkan-swrast` is lavapipe, the software Vulkan rasterizer the adapter-agnostic tests run on
([ADR-0242](adrs/0242-the-software-reference-rasterizer-is-lavapipe-and-a-warp-claim-is-re-measured.md)).
`vulkaninfo --summary` should list it as `llvmpipe` beside the hardware GPUs. `uv` provides the older
CPython that the diffusion sidecar's pinned torch needs. Desktop audio reaches the standalone through
PulseAudio's monitor source, which PipeWire serves via `pipewire-pulse`
([ADR-0131](adrs/0131-the-linux-standalone-captures-through-pulseaudios-simple-api.md)).

**As of 2026-09-22 the Linux build is not yet a working one.** `core/Cargo.toml` enables no `wgpu`
backend on Linux, and the standalone has no Linux capture arm. So `cargo build` succeeds, but with
dead-code warnings in `standalone/src/capture_verdict.rs` that `clippy -D warnings` rejects, wgpu has
no backend to find an adapter with, and the app renders silence. That holds until
[Plan 0120](plans/0120-the-standalone-ships-on-ubuntu.md) Phases 2 and 3 land. No green pre-push
gate on Linux has been recorded yet.

## Editing presets in VS Code

Install **Even Better TOML** (`tamasfe.even-better-toml`). That is the whole setup: a committed
`.taplo.toml` at the repository root tells the extension which schema each preset file gets, with
nothing per-user to configure.

**Which schema a file gets depends on its name.** A library file — one directly in `presets/`,
`presets/proposed/` or `presets/pending/` — named for its system's family (`fragment_*.toml` for
`fragment_field`, `collage_*.toml` for `shape_collage`, `curve_*.toml` for `parametric_curve`) gets
that system's own schema from `presets/schema/`. Inside it you get:

- **completion** on `[params]` keys, offering that system's parameters and the compositing stages',
  and not another system's;
- **hover documentation** on a parameter — what it does, its default, its typical range and its kind;
- **a Problems entry naming a key the system does not accept**, which is the mistake the loader
  deliberately forgives (an unknown parameter is a warning and the binding is kept, ADR-0020). The
  underline sits on the `[params]` header rather than on the key itself, so read the Problems panel
  for the name;
- the closed rosters as a dropdown on `[curve] family`, `[spectrum] layout` and every other key drawn
  from a fixed set.

Everything else gets **validation only**, from the generic `presets/preset.schema.json`: the teaching
files under `docs/examples/`, and a library file named off its family. Those still get an entry for
an unknown key, but no parameter completion and no parameter hover. So does every `[layer]`, in any
file: a layer names its own system, and a per-system schema does not narrow the layer's `[params]`.
`ritmolux --check` warns (`file-name`) on a library file named off its family, which is exactly the
file that would lose completion.

All sixteen files — the generic schema, the fourteen per-system schemas and `.taplo.toml` itself — are
**generated** from the engine's own `ParamSpec`, `TableDesc` and `SystemKind` declarations, and
`core/tests/suite/preset_schema.rs` fails if any of them is stale or `presets/schema/` holds a file no
system renders. The same test file holds a seventeenth file from the same export to it:
`docs/specs/player-schema.json`, the document `ritmolux --schema` prints, which the studio's schema
walks read when no player is built. One command regenerates all of them, and removes a schema left
behind by a system that no longer exists:

```sh
RLX_UPDATE_PRESET_SCHEMA=1 cargo nextest run -p rlx-core --test suite preset_schema::
```

**Turn format-on-save off for TOML.** The extension ships a formatter, and this project does not
format TOML: the presets carry deliberate local alignment that every formatter measured against them
destroyed (ADR-0190). `.taplo.toml` carries no `[formatting]` table, but that cannot stop your
editor's own format-on-save, so if you have it on globally, exclude TOML in your settings:

```json
"[toml]": { "editor.formatOnSave": false }
```

Without that, one save reflows the file and the diff touches every line. `.vscode/` is gitignored, so
this lives in your own settings rather than in the repository.

Optionally, a task that runs the checker on the open file. In `.vscode/tasks.json`:

```json
{
  "version": "2.0.0",
  "tasks": [
    {
      "label": "check preset",
      "type": "shell",
      "command": "cargo run -q -p standalone --bin ritmolux -- --check ${file}",
      "problemMatcher": {
        "owner": "ritmolux",
        "fileLocation": ["absolute"],
        "pattern": {
          "regexp": "^(.*):(\\d+):(\\d+): (error|warning)\\[([^\\]]+)\\]: (.*)$",
          "file": 1, "line": 2, "column": 3, "severity": 4, "code": 5, "message": 6
        }
      }
    }
  ]
}
```

That gives the checker's diagnostics in the Problems panel. The schema and the checker cover
different things and both are worth having: the schema knows every name and roster and underlines
live, and only `--check` compiles an expression. See
[the preset authoring guide](presets.md) for what it reports.

## The pre-push gate

A checked-in `.githooks/pre-push` runs the fast subset of CI before a push, so a
broken push costs seconds locally instead of minutes in CI. It is **opt-in per
clone** — enable it with:

```sh
git config core.hooksPath .githooks
```

> **An uninstalled clone has no gate.** Git will not run a hook from a tracked
> directory without that config, so until you set it nothing below happens. There
> is deliberately no auto-install ([ADR-0033](adrs/0033-testing-strategy-coverage-ratchet-and-pre-push-gate.md)
> Alternative H).

### What it runs

It stops at the first failure and names the step that failed:

| Step | Command |
|------|---------|
| Doc links | `node scripts/check-doc-links.mjs` |
| Index rows | `node scripts/check-index-rows.mjs` |
| Index rows (self-test) | `node scripts/check-index-rows.mjs --self-test` |
| Backlog claims | `node scripts/check-backlog-claims.mjs` |
| Filter figures | `node scripts/check-filter-figures.mjs` |
| Comment hygiene | `node scripts/check-comment-hygiene.mjs` |
| Contents blocks | `node scripts/toc.mjs --check` |
| Contents blocks (self-test) | `node scripts/toc.mjs --self-test` |
| Reader prose | `node scripts/check-reader-prose.mjs` |
| Release tag | `node scripts/check-release-tag.mjs` |
| Release tag (self-test) | `node scripts/check-release-tag.mjs --self-test` |
| Translations | `node scripts/check-translations.mjs` |
| Translations (self-test) | `node scripts/check-translations.mjs --self-test` |
| System counts | `node scripts/check-system-counts.mjs` |
| Gate carriers | `node scripts/check-gate-carriers.mjs` |
| Gate carriers (self-test) | `node scripts/check-gate-carriers.mjs --self-test` |
| Diffusion filter | `python3 tools/sd-filter/test_sd_filter.py` (skips with no `python3`) |
| Studio typecheck | `npm --prefix studio run typecheck` (skips with no `studio/node_modules`) |
| Studio lint | `npm --prefix studio run lint` (same guard) |
| Studio tests | `npm --prefix studio test` (same guard) |
| Format | `cargo fmt --all --check` (only when the push moved a Rust-relevant path — below) |
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` (same condition) |
| Rustdoc | `cargo doc --workspace --no-deps` under `RUSTDOCFLAGS=-D warnings` (same condition) |
| Tests | `cargo nextest run --workspace -P fast` (same condition, narrowed, and served from the suite ledger when it can be — below) |

**Everything above the cargo steps runs on every push; the four cargo steps do not**
([ADR-0237](adrs/0237-the-hook-runs-cargo-only-when-the-push-moved-rust-and-serves-the-rest-from-the-ledger.md)).
Git hands the hook one line per pushed ref, and the hook asks
`node scripts/push-scope.mjs <remote sha> <local sha>` about each range. The cargo steps run when
any range touches a path listed in `scripts/push-scope.manifest.mjs` — Rust source, a manifest or
the lockfile, the toolchain and cargo configuration, anything under a workspace crate's directory,
`presets/`, and the few files outside the crates that a Rust test opens, such as
`docs/configuration.md`. When nothing does, one line names the ranges it read and the push is done:

```
pre-push: skipping cargo fmt, clippy, rustdoc and tests: 7f7ed2c..e3498a5 touches no Rust-relevant path (scripts/push-scope.manifest.mjs)
```

When they run, the line names the first path that matched:

```
pre-push: running the cargo steps: push-scope: f2bf1dd..60f4c28 touches Cargo.lock (rule Cargo.lock): yes
```

**Whatever the hook cannot read runs all four, and the line says why**: a ref that is new on the
remote (there is no range to compare against), an empty or unparseable stdin, a push of nothing but
deletions, a shallow clone, a sha this clone does not hold, or the hook run by hand from a terminal.
**A new tag is a new ref too**, so a push carrying a release tag — every close push made with
`git push --follow-tags` — runs the cargo steps even when the branch beside it moved no Rust; the
test step can still be served from the ledger.

**The test step is served instead of run when the suite ledger already proves the tree.** Before it,
`node tools/conductor/suite-record.mjs` asks the ledger
([ADR-0207](adrs/0207-a-suite-run-the-conductor-observed-green-is-not-run-again-on-the-same-tree.md))
whether this worktree's tree, clean, has a green `cargo nextest run --workspace`. Who wrote the
record does not matter — a conductor gate, a review session, or your own full suite run through
`tools/conductor/with-lock.mjs` — and the line names it:

```
pre-push: serving cargo nextest run -P fast from the suite ledger: tree <tree> is green in the suite ledger, run by <writer> at <time>: <nextest summary>
```

A dirty worktree, a tree whose only record is a `-P fast` run, and a tree whose newest full run was
red all run the step, after a line saying why the ledger did not serve it. So the saving lands on a tip a full suite already
proved — `main` after a conductor fast-forward — and not on every intermediate commit.

**The path list is a judgement, and it will be wrong once**: some build input nobody listed will let
a push skip a suite that would have gone red. CI runs every step unconditionally and is what catches
it. The repair is a line in the manifest and a case in `scripts/fixtures/push-scope/cases.json`,
checked by `node scripts/push-scope.mjs --self-test`.

**The two guarded groups skip rather than fail, and say so.** A clone with no
`python3` on PATH and one that has never run `npm --prefix studio ci` are both
ordinary, so those four steps print a notice naming what is missing and the
command that would make them run, and the push continues
([ADR-0016](adrs/0016-gpu-tests-opt-in-ci-scope.md)). CI runs all four
unconditionally and is the backstop under both; so does the conductor's gate,
which since
[ADR-0218](adrs/0218-a-lane-makes-its-plans-preconditions-true-and-a-skipped-check-says-so.md)
reports each skip the same way instead of dropping the step in silence.

The Node steps come first because they are the cheapest (tens of milliseconds
between them): every relative markdown link in the repo must resolve, every row
inside a marked roster region must stay under 320 bytes
([ADR-0116](adrs/0116-an-index-row-is-a-pointer-and-a-gate-holds-it-to-one.md)),
every live `design-backlog.md` entry must carry a probe that still holds
([ADR-0108](adrs/0108-a-backlog-claim-about-the-repo-carries-an-executable-probe.md)),
the diffusion filter's cost figures must live in one file
([ADR-0122](adrs/0122-a-sidecar-tool-documents-itself-in-one-place.md)), no `.rs`
comment may carry a relative link or plan-relative narration
([ADR-0127](adrs/0127-a-comment-carries-the-mechanism-and-the-decision-record-stays-in-docs.md)),
every citation in a reader document must sit inside a link
([ADR-0168](adrs/0168-the-reader-documents-address-a-reader-and-the-record-stays-a-link.md)),
every generated contents block must still match the headings beneath it
([ADR-0163](adrs/0163-a-long-document-carries-a-generated-contents-block.md)),
the version root `Cargo.toml` declares must carry an annotated tag on `HEAD`'s
history, because `git push --follow-tags` never sends a lightweight one
([ADR-0203](adrs/0203-a-release-tag-is-annotated-and-origin-is-what-is-checked.md)),
every `.ru.md` translation must open with the `translated-from: <sha>` stamp
naming the commit its source was translated from
([ADR-0185](adrs/0185-the-docs-translate-a-slice-and-a-stamp-makes-staleness-visible.md)),
and this table's own Node steps must equal the ordered roster in
`scripts/gates.manifest.mjs`, as must CI's `links` job and the conductor's gate
([ADR-0217](adrs/0217-the-node-gate-roster-is-one-manifest-and-a-checker-holds-every-carrier-to-it.md)) —
so a gate added to one carrier and not the others is red here, at the push.
The four that could go green on a rule that had quietly stopped working — a roster
detector matching nothing, an anchor rule that is merely plausible, a tag-type check
that no longer looks, a stamp reader that finds no translations at all — carry a
`--self-test` beside their check. The roster gate carries one for a different
reason: its plain run cannot go vacuously green (a parser that stopped matching
reports an empty list against a roster that is not), so its self-test is there for
the reporting path and the three drift shapes instead.
**A translation that has drifted is never a failure.** The stamp check reports a
source that has moved as an advisory row and exits 0: nothing mechanical can judge
whether the Russian still says what the English now says, and hard-failing would
make that slice a hostage of every hotkey edit. A **missing or malformed** stamp is
the exit code, because that much is mechanical.
**The release-tag step refuses a push, not only a release:** from the moment a close
moves the version, every push fails until that version's tag is annotated. CI's
`links` job cannot read local tags, so it runs the same script with `--remote` on a
push to `main` instead, and asks `origin`.
If `node` is not on your `PATH` they all **skip with a notice** rather than
failing the push; nothing else here needs Node. That skip is about the hook only
— CI's `links` job runs the same checks on `ubuntu-latest`, where they cannot
skip and are not bypassable.

### What it costs

**What a push costs depends on which of the three shapes above it takes.** Measured on 2026-09-22,
one run each, the whole hook driven with a real ref line on stdin, on the Windows reference machine
(AMD Radeon integrated, DX12), in a worktree whose `target/` was already built and which had no
`studio/node_modules` — so the studio's three steps skipped, and add their 15.2 s wherever they run:

| Push | Wall time | What ran |
|------|----------:|----------|
| A range touching no Rust-relevant path | **8.2 s** | the Node roster and the Python suite; every cargo step skipped |
| A Rust range whose tree has a full-suite record | **12.1 s** | the same, plus `fmt`, `clippy` and `rustdoc` with nothing to rebuild, and the ledger lookup; the test step served. The record was seeded into a scratch ledger for the reading, since no tree of the measuring lane had a real one |
| A Rust range whose tree has no record | **557.2 s** | everything; the test step alone took 544.8 s, 1695 tests passed and 86 skipped |

**The test step is the whole difference between the last two rows.** On 2026-09-14 an earlier
~410 s reading of it on the same machine was about 165 s idle, and the reason is still true:
every test that asserts on wall-clock time runs **alone**: nextest waits for the running tests to
drain, runs it, and starts nothing beside it. So you will see the run pause on those tests.
`.config/nextest.toml` names them, and a guard in `core/tests/suite/hygiene.rs` holds that list to
the tests that read the clock
([ADR-0193](adrs/0193-a-test-that-reads-the-clock-runs-alone.md)). The hook excludes the nine
GPU-heavy suites that iterate every shipped preset or scene through a real
adapter. **Which nine is not written here** — since
[ADR-0156](adrs/0156-the-per-phase-gate-is-scoped-and-the-suite-is-owed-once-per-plan.md)
the list is the `fast` profile's `default-filter` in `.config/nextest.toml`, and the hook and CI's
`check` job both cite `-P fast` rather than restating it.
The test runner **names the skipped binaries itself** on every run, on the profile's
authority, so the narrowing is never silent, and **CI runs
all of them regardless** — though since [ADR-0073](adrs/0073-the-windows-ci-critical-path.md)
it runs those nine in the `coverage` job alone rather than in two Windows jobs, so
the promise is now underwritten by one job instead of a redundancy between two.

The **rustdoc step** fails a broken or private intra-doc link in **any of the five workspace
members** before CI's `cargo doc --workspace` job does. Scoping it to `-p rlx-core` left the other
four documented in CI and nowhere else — a rustdoc error in one of them was unreachable before a
push, and it fired twice in sixteen days, both times repaired after a release tag had been written
on top of the red
([ADR-0217](adrs/0217-the-node-gate-roster-is-one-manifest-and-a-checker-holds-every-carrier-to-it.md)).
On the reference machine the widened step measures
7.8 s warm after an edit to `rlx-ring` (the deepest crate, so every member re-documents), 0.5 s warm
with nothing changed, and 20.1 s after a `cargo clean --doc` — about two seconds more than the
scoped step it replaces.

`cargo deny`, doctests, Miri, and the coverage job are deliberately *not* in the
hook — they push it into minutes, and a gate that hurts gets disabled
([ADR-0033](adrs/0033-testing-strategy-coverage-ratchet-and-pre-push-gate.md) Alternative F).
They are also the checks least likely to break from a local edit.

The **`spout` compile job** (ADR-0181) is outside the hook too, and for neither of those reasons.
It stages a third-party SDK over the network before it compiles anything, which does not fit the
hook's budget at all — and it is the one CI gate that *is* likely to break from an ordinary local
edit, because code behind `#[cfg(feature = "spout")]` is not type-checked when the feature is off,
so every `cargo build`, `clippy --all-targets` and `nextest` run here is blind to it by
construction. If you are refactoring anything `standalone/src/stream.rs` reaches, compile it
yourself before pushing:

```powershell
# Windows only: Spout is a Windows SDK, and the feature compiles nothing elsewhere
powershell -File packaging/spout/fetch-sdk.ps1   # once per checkout
cargo check -p standalone --features spout
```

Bypass once with `git push --no-verify`.

## Disk

Every worktree builds into its own `target/`, and cargo collects nothing in it on its own.

**`target/debug/deps/` keeps every generation of every unit.** A dependency bump, a feature flip,
or a narrowed `cargo nextest run -p <crate>` whose feature set differs from the workspace build
writes a second `rlx_core`, `wgpu` and `windows` beside the first, and nothing collects the old one.
`scripts/prune-target.mjs` deletes what the everyday loop no longer uses:

```sh
node scripts/prune-target.mjs                 # dry run: what would go, and the bytes
node scripts/prune-target.mjs --apply         # delete it
node scripts/prune-target.mjs --verify-fresh  # every artifact the loop reports is still fresh
```

The live set is **what cargo reports**: the script runs `cargo build --workspace`,
`cargo clippy --workspace --all-targets` and `cargo nextest run --workspace -P fast --no-run` with
JSON messages, and keeps every file one of them names. So the three have to be green, and no other
cargo process may run in the same checkout while it does. Deleting something the loop needed costs a
rebuild, never a wrong build: cargo sees a missing output as dirty. The target directory comes from
`cargo metadata`, so a `CARGO_TARGET_DIR` redirect is followed.

Run it when the disk is short, and after anything that changes many units at once: a toolchain or
dependency bump, or a session of narrowed `-p` runs.

**`target/debug/incremental/` holds one directory per compiled unit, and up to two sessions in
each.** Measured on the reference machine on 2026-09-15, in a lane built by the three commands
above: 171 unit directories and 1.95 GB. One edit to `core/src/render/metrics.rs` and the three
commands again took it to 3.62 GB with no new directory, because each rebuilt unit now kept its
previous session beside the new one. Three more edit-and-revert rounds left it at 3.59 GB, still
171 directories and never more than two sessions in one. So the loop alone does not grow it without
bound. What adds directories is a **new unit**: a different feature set, a narrowed `-p` build, a
dependency or toolchain change, each of which leaves the old unit's directory behind.

`prune-target.mjs` does not touch this directory, because nothing cargo reports names which unit a
directory belongs to. Delete it instead, when it has grown past what the loop above needs:

```sh
rm -rf target/debug/incremental                           # sh
Remove-Item -Recurse -Force target\debug\incremental      # PowerShell
```

That costs one non-incremental rebuild of the workspace crates on the next build, and nothing else:
dependencies are never built incrementally, and no output outside that directory depends on it.

## Running approved plans under the conductor

`tools/conductor/` runs approved plans to a merged `main` with nobody at the keyboard. It works in up
to two worktree lanes, and each plan gets separate headless `claude -p` sessions: its implementer
runs, then a review that also closes the plan, then a fast-forward of `main` and removal of the lane
([ADR-0205](adrs/0205-an-approved-plan-runs-under-a-conductor-and-every-judgement-it-cannot-make-parks-the-plan.md)).
It never pushes. Anything it cannot decide — a `human` phase, a red gate, a review still failing
after two fix rounds, a spend cap — **parks** the plan and moves on to the next one.

```sh
node tools/conductor/conductor.mjs check          # local.json, the CLI version, the queue
node tools/conductor/conductor.mjs run            # both lanes, until the queue is merged or parked
node tools/conductor/conductor.mjs status         # what each lane is doing, and every park
```

**Before the first run:**

- **Spend caps.** Write `tools/conductor/local.json` with your per-step caps. It is gitignored, and
  the conductor refuses to start without it.
- **The main checkout.** Keep it clean while a run is live: a fast-forward refuses a dirty one.

**What changes for a plan it runs:**

- **The approval is the go.** The plan's `Status: approved` is the only approval it needs.
- **The review is a separate process.** A fresh session, handed the plan and the lane and nothing an
  implementer wrote, performs the close review.
- **The review is committed.** It lands in the plan's `## Close review` section.

Every other seam stays exactly as the skills describe it.

**Watch it from the terminal that started it.** `run` prints one line per milestone as it happens: a
step starting and ending with its duration, spend and turn count, each commit that lands in the
worktree, each phase whose log row flips to done, each test and gate command with its counts and lock
wait, the 5-hour and 7-day usage readings, and every command a permission rule denied. The same lines
go to `tools/conductor/state/live.log`, under one header per run, for a run you did not watch.

**Next morning, read `tools/conductor/digest.md`.** It is the current state in two sections and
nothing else
([ADR-0214](adrs/0214-the-digest-is-a-current-state-page-and-history-is-regenerated-on-demand.md)).
**Needs you** comes first and is the whole worklist: each park with its age, the worktree it holds —
or the branch `resume` reopens it from, when you have already removed that worktree — the usage
reading its session ended on and its resume command; each lane stopped at the worktree cap; each lane
still on disk after a merge; and every merge's still-open findings with their `file:line`. A park the
repository itself shows as finished closes that section under **Already settled, clear the record**,
counted apart from the live ones, so a plan you closed by hand is never repeated at you as work.
**Now** follows: per lane, the plan, the step and how long it has been in it, and what it has spent.
A first section that says it found nothing means nothing is waiting on you.

**What last night produced is one command away:** `node tools/conductor/conductor.mjs digest
--history` writes `digest-history.md` beside it, newest run first, each run with its own closed
plans, its failures and its Totals — the usage windows at run start and run end, and gate minutes
split into the full workspace suite and everything else, with the count of suite runs skipped because
the conductor had already seen that exact tree pass
([ADR-0207](adrs/0207-a-suite-run-the-conductor-observed-green-is-not-run-again-on-the-same-tree.md)).
Nothing writes that file until you ask for it, and a closed plan's own committed `## Close review`
carries its review either way.

The operator guide — every command, what to do about each kind of park, and how to verify a new CLI
version — is [`tools/conductor/README.md`](../tools/conductor/README.md). Its tests need no network
and spend nothing:

```sh
node --test "tools/conductor/test/*.test.mjs"
```

## What else there is to read

- [Testing and visual QA](testing.md) — the `core/tests/` harness that hard-tests every preset for
  reactivity, animation, shape sanity and beat response.
- [Headless capture and video](capturing.md) — the `shot` CLI, `--render` and the live video-out.
- [MilkDrop conversion](milkdrop-conversion.md) — reading what `milkconv` produced.
- [Embedding the core](embedding.md) — putting the engine inside another application.
- [On-device validation](on-device-validation.md) — the manual checklist for what CI cannot run:
  real GPUs, live loopback, installing the foobar2000 component.
- [Releasing](releasing.md) — how the version moves and what a `v*` tag builds.
- [Non-functional requirements](nfr.md) — the quantified budgets behind every "lightweight" and
  "real-time" claim.
- [The architecture decisions](adrs/) — start with
  [ADR-0001](adrs/0001-rust-core-wgpu-cabi-foobar-shim.md), the founding decision, with the rejected
  alternatives recorded.
