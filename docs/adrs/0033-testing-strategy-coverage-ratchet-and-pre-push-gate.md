# ADR-0033 — Testing strategy: named tiers, a core-only coverage ratchet, and a local pre-push gate

> **Status:** accepted
> **Date:** 2026-07-25
> **Related plan(s):** [0032](../plans/done/0032-testing-strategy-e2e-coverage-and-pre-push.md)

## Context

The suite grew one plan at a time. Thirty-one closed plans each brought their own tests, and the
result is real: 150 `#[test]`s, sixteen `core/tests/*` integration suites, a golden-image drift
guard over frozen per-system fixtures (ADR-0023), source-scanning drift guards
(`hygiene.rs`, `declared_params_match_set_param`), CI on Windows + macOS running
`cargo build` / `cargo nextest run` / `cargo test --doc` / `cargo clippy --all-targets -D warnings` /
`cargo fmt --check`, a `cargo deny` supply-chain gate, and Miri over `lmv-ring`'s `unsafe`.

Nobody has ever asked the inverse question. There is **no measurement of what is untested**, so
the answer to "are the happy paths covered" is currently an opinion. Three concrete holes are
visible by inspection:

- **The audio half of the pipeline is never joined to the render half.** `Renderer::capture_audio`
  (`core/src/render/mod.rs:1022`) does drive real PCM through a real `Analyzer` — but it starts at
  the `Analyzer`, and exactly one test uses it (`core/tests/beat.rs`). The SPSC ring
  (`audio::intake` → `push_samples`/`pop_samples`) is tested in isolation by `lmv-ring`'s own unit
  tests and Miri, and the drain policy that joins ring to analyzer lives in
  `standalone/src/main.rs:275-281` where nothing tests it at all. The seam the whole architecture
  rests on — "the ring buffer is the seam" — has no test that crosses it end to end.
- **`shot` has no tests of any kind.** `standalone/examples/shot.rs` is the headless-capture CLI
  the `preset-author` lane self-verifies through and the visual-QA harness (Plan 0013) is built on,
  and `#[test]` does not run in an `examples/` target. Plan 0031 Phase 1 moves its *pure helpers*
  into the `[lib]` so they can be asserted in-process; that still leaves argument parsing, preset
  resolution, exit codes, and file output unexercised.
- **A broken push is discovered in CI, minutes later.** The only local hook is
  `block-broad-git-add.js`, which guards staging, not correctness.

Three constraints bound any answer. GPU rendering and live audio cannot run in CI (NFR §7) —
except that on Windows they partly can, because the DX12 **WARP software adapter** makes headless
rendering deterministic, which is why the golden suite exists and why it is Windows-only (macOS has
no software Metal fallback, ADR-0016). `standalone/` is mostly `winit` event loop plus WASAPI /
ScreenCaptureKit capture — structurally unreachable from CI on any platform. And "lightweight is a
feature" (NFR §4): a new Cargo dependency, even a dev one, is a cost that has to be argued.

## Decision

We adopt a five-tier testing model with an explicit home for every kind of test, and add three
things the model currently lacks: an **end-to-end tier** (two suites — one in-process crossing the
ring/analyzer/renderer seam, one out-of-process running the built `shot` binary), a **line-coverage
ratchet scoped to `lmv-core` alone** (`cargo llvm-cov nextest -p lmv-core --fail-under-lines FLOOR`
as a Windows-only CI job, where `FLOOR` is a single checked-in number set from a real measurement
and only ever raised), and an **opt-in local pre-push gate** (`.githooks/pre-push`, enabled per
clone with `git config core.hooksPath .githooks`) that runs the fast subset — `fmt`, `clippy`,
`nextest` — and leaves `cargo deny`, doctests, Miri, and coverage to CI.

The tiers, and the rule for where a new test goes:

| Tier | Home | What belongs here | Runs in CI |
|------|------|-------------------|------------|
| 1 — unit | `#[cfg(test)] mod tests` beside the code | Pure functions: expression eval, curve math, routing, palette bake, ring semantics | Everywhere |
| 2 — behavioral | `core/tests/*.rs`, `standalone/src/*.rs` unit tests | A claim about observable behavior of one subsystem: DSP reacts per band, presets load and warn, the director rotates | Everywhere (GPU ones skip on macOS) |
| 3 — drift guard | `core/tests/golden.rs`, `hygiene.rs`, the `declared_params_match_set_param` scan | Pins that fail when something changes *without intent*: baseline PNGs, panic pragmas, param-name drift | Windows (golden), everywhere (source scans) |
| 4 — end to end | `core/tests/chain.rs` (new), `standalone/tests/shot_cli.rs` (new) | The whole chain, joined: PCM in → pixels out; the built CLI as a user invokes it | Windows fully, macOS for the GPU-free cases |
| 5 — manual / on-device | `docs/on-device-validation.md` | Anything needing a real window, a real GPU vendor, or real loopback audio | Never — human-gated by design |

The coverage floor is a **ratchet, not a target**: it is set from what the suite actually achieves,
it rises at a close ceremony when a plan improves it, and lowering it requires a one-line note in
the CI file saying which plan lowered it and why. It gates `lmv-core` only; `standalone/` and
`plugin-foobar/` are explicitly ungated.

## Consequences

### Positive

- **"Is the happy path covered" becomes a number instead of an opinion**, and the per-file report
  tells the next plan where the holes are rather than leaving it to inspection.
- **The ring seam gets a test that crosses it.** The architecture's central claim — audio and
  render are decoupled through a lock-free SPSC ring — is currently asserted by design review only.
- **`shot` stops being the untested foundation of two workflows.** It also gives Plan 0031 Phase 1
  (which refactors `shot`'s internals) a behavioral safety net it would otherwise not have.
- **Broken pushes cost seconds locally instead of minutes in CI**, and the failing step is named.
- **New render code inherits pressure to be tested**, because uncovered new lines move the ratio
  toward the floor.

### Negative

- **A line-coverage floor is gameable** — a test that calls a function and asserts nothing raises
  it. The floor is a backstop against silent erosion, not a quality measure; the Mode 4 review's
  "open the test and read the assertion body" step remains the actual quality gate, and this ADR
  does not weaken it.
- **A legitimate refactor can trip the ratchet.** Deleting a well-covered module lowers the ratio
  even though nothing got worse. The escape is deliberate and visible (lower the floor with a
  note), which is the cost of having a ratchet at all.
- **The coverage job adds CI time and a tool install**, and instrumented binaries make the
  WARP-rendered suites slower. It is a separate job, so it does not lengthen the critical path.
- **The pre-push hook is opt-in per clone and bypassable** with `--no-verify`. Git will not run a
  hook from a tracked directory without `core.hooksPath`, so an uninstalled clone silently has no
  gate — that is a real hole, accepted in exchange for not inventing an install mechanism.
- **The e2e chain test re-implements the drain policy** that `main.rs` owns, rather than sharing
  it. Extracting that loop into testable code is a followup, not this decision; until then the two
  can drift.
- **Coverage is measured on Windows only**, so a macOS-only code path (`capture_mac.rs`) reads as
  uncovered — moot while the gate is core-only, and a trap if the gate is ever widened.

### Neutral

- `cargo-llvm-cov` enters as a CI-installed tool via `taiki-e/install-action`, alongside `nextest`
  and `cargo-deny`. It is not a Cargo dependency and does not touch `Cargo.lock` or NFR §4's
  dependency budget.

## Alternatives considered

### Alternative A — Workspace-wide coverage percentage

Gate the whole workspace instead of `lmv-core`. Rejected: `standalone/` is a `winit` event loop
plus two platform capture backends that CI cannot execute on any runner, so the number would
mostly measure *how much of the app is structurally untestable*, not how much is untested. Worse,
the pressure it creates points the wrong way — toward mocking WASAPI and ScreenCaptureKit behind
traits so the mocks can be "covered", which buys nothing and violates the keep-the-seam-thin
posture the platform code is written in.

### Alternative B — A fixed absolute target (80 %)

Pick a conventional number and work toward it. Rejected: it is arbitrary here, and it has exactly
two states — comfortably above (no pressure) or below (a cliff that blocks unrelated work until
someone writes filler tests). A ratchet anchored to measured reality applies the same one-way
pressure with no cliff and no negotiation about what the number should be.

### Alternative C — Region or branch coverage as the gate

Gate on llvm-cov's region coverage instead of line coverage. Rejected for the gate: region counts
are noisier across a crate this shader- and match-heavy, and they move for reasons unrelated to
test quality, which makes a ratchet argue with itself. Line coverage gates; region coverage is
still emitted in the report for information.

### Alternative D — A named-module inventory guard instead of a percentage

Extend the existing `hygiene.rs` style: keep a list of modules that must have tests, fail when a
listed module has none. Rejected (user call): it is precise about *absence* but blind to *partial*
coverage, which is the actual shape of the gap — `dsp/fft.rs` and `preset/schema.rs` are exercised
from outside by `core/tests/dsp.rs` and `core/tests/preset.rs`, so an inventory guard would report
them green while saying nothing about which of their branches never execute.

### Alternative E — Promote `shot` to a `[[bin]]` so `CARGO_BIN_EXE_shot` resolves it

The clean way to locate a binary from an integration test. Rejected: `image` is a **dev-dependency**
precisely so the PNG codec stays out of the shipped `lmv.exe` (Plan 0012 sized that binary against
NFR §4), and a `[[bin]]` does not get dev-dependencies — the codec would have to move into
`[dependencies]`. It also renames the invocation for every documented workflow: `docs/capturing.md`
has nine `cargo run -p standalone --example shot` lines, and the `preset-author` skill's references
carry more — and `.claude/skills/**` cannot be edited by the assistant, so those would go stale with
no way to fix them. The test locates the example under `target/<profile>/examples/` by walking up
from `current_exe()` instead.

### Alternative F — Full CI parity in the pre-push hook

Run everything CI runs before every push. Rejected (user call): `cargo deny`, doctests, and the
coverage job push the gate from tens of seconds into minutes, and a gate that hurts gets disabled —
at which point it is worth less than the fast subset it replaced. Supply-chain and UB checks are
also the checks least likely to break from a local edit, so they are the right things to leave in
CI.

### Alternative G — A `PreToolUse` hook mirroring the gate for assistant sessions

Guard `git push` from the assistant the way `block-broad-git-add.js` guards staging. Rejected as
moot: CLAUDE.md already forbids the assistant from pushing at all, so the hook would gate an action
that never happens. Revisit only if that rule changes.

### Alternative H — `cargo-husky` or `rusty-hook` to auto-install the hook

Let a crate install the git hook on first build. Rejected: a build-time dependency, a `build.rs`
side effect that writes into `.git/`, and a surprise for anyone who did not ask for it — all to
avoid documenting one `git config` line.

## Notes

- Warm `cargo nextest run` wall time is **unmeasured** at the time of writing; a concurrent `dev`
  session held the tree mid-edit. Plan 0032 Phase 3 measures it and narrows the hook's test step to
  a filtered set (printing what it skipped) if it exceeds ~90 s.
- Whether `cargo nextest run` builds `examples/` targets — which decides if the `shot` subprocess
  test needs an explicit build step in CI and the hook — is likewise unverified and is a Phase 2
  done-when, not an assumption.
- The C++ shim (`plugin-foobar/`) stays untested by this ADR. It is a thin `extern "C"` caller with
  no logic of its own; the surface it calls is covered by `core/tests/ffi.rs`.
