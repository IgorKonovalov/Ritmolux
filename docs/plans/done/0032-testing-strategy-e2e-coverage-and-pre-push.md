# 0032 — Testing strategy: full-chain e2e, `shot` CLI coverage, a core coverage ratchet, and a pre-push gate

> **Status:** done
> **Created:** 2026-07-25
> **Owner skill(s):** dev, human
> **Related ADRs:** [0033](../adrs/0033-testing-strategy-coverage-ratchet-and-pre-push-gate.md)

## TL;DR

Close the three holes in a suite that grew plan-by-plan and was never audited as a whole. Add an
**end-to-end tier** — one in-process test that drives synthetic PCM through the real SPSC ring, the
real `Analyzer`, and the real `Renderer` to pixels, and one out-of-process suite that runs the built
`shot` binary and asserts exit codes, files, and JSON. Add a **line-coverage ratchet on `lmv-core`
alone**, set from a real measurement and only ever raised. Add an **opt-in `.githooks/pre-push`**
running `fmt` + `clippy` + `nextest`, so a broken push costs seconds locally instead of minutes in
CI. First user-visible behavior: after Phase 1, one `cargo nextest run` proves that pushing bass
through the ring and pushing treble through the ring produce measurably different frames — the claim
the architecture rests on and that nothing currently checks.

## Context & problem

The user asked three questions: do we have a coverage threshold, do we have e2e tests, are the happy
paths covered — and can we add a pre-push hook. The honest answers today are no, no, and *unknown*.

What exists is substantial and should not be undersold: 150 `#[test]`s, sixteen `core/tests/*`
suites, a golden-image drift guard over frozen per-system fixtures (ADR-0023), source-scanning drift
guards (`hygiene.rs`'s panic-pragma scan, `declared_params_match_set_param`), CI on Windows + macOS
running build / nextest / doctests / clippy `-D warnings` / fmt, `cargo deny` for supply chain, and
Miri over `lmv-ring`'s `unsafe`. That is a healthier baseline than most projects this age.

What is missing is the inverse view. Three specific holes:

1. **The ring seam is never crossed by a test.** `Renderer::capture_audio`
   (`core/src/render/mod.rs:1022`) drives real PCM through a real `Analyzer` and is used by exactly
   one test (`core/tests/beat.rs`) — but it *starts* at the `Analyzer`. The SPSC ring is tested in
   isolation (`lmv-ring`'s four unit tests plus Miri), and the drain policy joining ring to analyzer
   lives at `standalone/src/main.rs:275-281`, tested by nothing. So the sentence "the ring buffer is
   the seam between audio and render" is defended by design review, not by a test.
2. **`shot` has zero tests.** `standalone/examples/shot.rs` is the CLI the `preset-author` lane
   self-verifies through and the Plan 0013 visual-QA harness is built on, and `#[test]` does not run
   in an `examples/` target. Plan 0031 Phase 1 moves its pure helpers into the `[lib]`; that still
   leaves argument parsing, preset resolution, exit codes, and file output unexercised — and Plan
   0031 will be refactoring that file with no behavioral net under it.
3. **Nothing measures what is untested, and nothing gates locally.** The only hook is
   `block-broad-git-add.js`, which guards staging.

## Decision

Implement ADR-0033's five-tier model by adding what tier 4 (end to end) and the measurement layer
currently lack, in four `dev` phases plus one `human` install step. Coverage gates `lmv-core` only,
as a **ratchet** set from measurement rather than an aspirational target. The pre-push hook runs the
**fast subset** only — `fmt`, `clippy`, `nextest` — leaving `cargo deny`, doctests, Miri, and
coverage to CI.

We rejected a workspace-wide coverage gate (it would mostly measure how much of `standalone/` is
structurally untestable, and would push toward mocking WASAPI), a fixed 80 % target (arbitrary, and
a cliff), a named-module inventory guard instead of a percentage (blind to partial coverage, which
is the actual shape of the gap), promoting `shot` to a `[[bin]]` (drags the `image` dev-dependency
into the shipped binary and renames every documented invocation, including ones in
`.claude/skills/**` that cannot be edited), and full-CI-parity in the hook (minutes, so it gets
disabled). ADR-0033 records each with its decisive reason.

## Architecture diagram

```mermaid
flowchart LR
    subgraph ext[External]
        LB[loopback / foobar]
    end

    subgraph shell[standalone shell]
        DRAIN["drain loop<br/>main.rs:275"]
        SHOT["shot CLI<br/>examples/shot.rs"]
    end

    subgraph core[core]
        RING[SPSC ring]
        AN[Analyzer]
        REN[Renderer + PostChain]
        PX[(pixels)]
    end

    LB --> DRAIN --> RING
    RING --> AN --> REN --> PX
    SHOT --> REN

    subgraph tiers[Test tiers]
        T1["T1 unit<br/>lmv-ring, expr, curves"]
        T2["T2 behavioral<br/>core/tests/dsp,preset,director"]
        T3["T3 drift<br/>golden, hygiene"]
        T4A["T4 e2e in-process<br/>NEW core/tests/chain.rs"]
        T4B["T4 e2e subprocess<br/>NEW standalone/tests/shot_cli.rs"]
    end

    T1 -.covers.-> RING
    T2 -.covers.-> AN
    T3 -.covers.-> REN
    T4A -.covers.-> RING
    T4A -.covers.-> AN
    T4A -.covers.-> REN
    T4B -.covers.-> SHOT
```

The gap the diagram makes visible: every existing tier covers **one** box. Nothing today follows a
sample from `RING` through `AN` into `REN`, and nothing runs `SHOT` at all.

## Implementation phases

### Phase 1 — Full-chain e2e: PCM through the ring, the analyzer, and the renderer to pixels

- **Owner skill:** dev
- **What:** A new `core/tests/chain.rs` that joins the two halves of the pipeline for the first
  time — synthetic PCM pushed into a real `audio::intake` SPSC pair in capture-callback-sized
  bursts, drained through `pop_samples` into a fixed scratch buffer exactly as
  `standalone/src/main.rs:275-281` does, fed to `Analyzer::push_interleaved` / `take_frame`, and
  rendered through `Renderer::new_headless` + `capture_frame`. This is the walking skeleton of
  tier 4 and the phase that carries the plan's value on its own.
- **Files touched:** `core/tests/chain.rs` (new).
- **Done when:** all four claims below hold under `cargo nextest run -p lmv-core -E 'binary(chain)'`
  on Windows, and the suite skips with a printed reason where the software adapter is unavailable
  (**corrected at close** — this line originally said `test(chain)`, a *name* filter that matches
  only the two chain tests with "chain" in their name plus three unrelated `render::post` tests,
  and silently skips `band_routing_survives_the_ring_seam` and
  `intake_rejects_bad_formats_without_panicking`. `binary(chain)` is the suite selector; all four
  pass under it)
  (mirror `golden.rs`'s existing guard rather than inventing a second one):
  1. **Band routing survives the seam.** A bass-heavy signal and a treble-heavy signal, pushed
     through the *same* ring and the *same* reactive preset, render frames whose `frame_diff`
     exceeds a stated floor. Verify the assertion is non-vacuous by temporarily feeding both runs
     the same signal and confirming it fails.
  2. **Determinism holds through the ring.** Two identical runs of the same PCM produce
     byte-identical captures — the NFR §6 property, now checked across the seam rather than only
     downstream of it.
  3. **Overflow is lossy, not fatal.** Pushing a burst larger than the ring's capacity returns a
     short count from `push_samples`, and the chain keeps producing frames afterward. (An audio
     callback that overruns must drop samples, never block or panic.)
  4. **Boundary validation rejects, never panics.** `intake` at 4 kHz, 0 channels, and 9 channels
     each return the matching `FormatError`, and no path panics.

### Phase 2 — `shot` as a subprocess: exit codes, files, and report JSON

- **Owner skill:** dev
- **What:** A new `standalone/tests/shot_cli.rs` that runs the **built `shot` binary** the way a
  user does. It locates `target/<profile>/examples/shot[.exe]` by walking up from
  `std::env::current_exe()` to find the `examples/` sibling (robust to `CARGO_TARGET_DIR` and to a
  `--target <triple>` layout); if the binary is absent it fails with the actionable message
  `run \`cargo build -p standalone --example shot\` first` rather than silently passing.
  `shot` is deliberately **not** promoted to a `[[bin]]` — see ADR-0033 Alternative E.
- **Files touched:** `standalone/tests/shot_cli.rs` (new); `.github/workflows/ci.yml` and
  `.githooks/pre-push` only if the build-step check below comes back negative.
- **Done when:**
  1. **The GPU-free cases pass on both platforms.** Every one of these exits non-zero *before* a
     renderer is constructed (`parse_args` → `load_library` → renderer, `shot.rs:295`), so they
     need no adapter: an unknown flag; `--presets <missing dir>`; `--preset-file <missing file>`;
     `--preset X` with no `--out`. Each also names the offending input on stderr.
  2. **The GPU cases pass on Windows and skip with a printed reason on macOS** (no software Metal,
     ADR-0016): `--preset-file presets/<one>.toml --out <tmp>.png` exits 0 and writes a PNG that
     decodes at the requested `--size`; `--report --json` exits 0 and its stdout parses as JSON
     carrying the expected top-level keys; `--presets presets --preset <name> --out <tmp>.png`
     exits 0 and stdout carries the `[--presets presets]` source label — the provenance line Plan
     0015 added.
  3. **The build-step question is answered, not assumed.** Confirm whether `cargo nextest run`
     builds `examples/` targets. If it does not, add an explicit
     `cargo build -p standalone --example shot` step to CI and to the Phase 3 hook, and say so in
     the commit message.

### Phase 3 — The pre-push gate

- **Owner skill:** dev
- **What:** A checked-in `.githooks/pre-push` (POSIX `sh` — runs under Git-for-Windows' bundled
  shell and macOS `sh` alike) that runs the fast subset, stops at the first failure, names the step
  that failed, and prints the `--no-verify` escape.
- **Files touched:** `.githooks/pre-push` (new), `README.md`.
- **Done when:**
  1. The hook runs, in order, `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`,
     `cargo nextest run` — and **not** `cargo deny`, doctests, Miri, or coverage (ADR-0033
     Alternative F).
  2. **Warm wall time is measured and recorded in the README**, not estimated. If it exceeds ~90 s
     warm, the hook's test step narrows to a nextest filter that excludes the GPU-heavy suites
     (`golden`, `attractor`, `reaction_diffusion`, `background_composite`, `ink`) — and it
     **prints which suites it skipped** so the narrowing is never silent. CI keeps running all of
     them either way.

     **Superseded at close by measurement, with the user's approval.** The full gate came back at
     ~98 s, over the bar — but this list was drafted from a guess about which suites are heavy and
     it is wrong. The measured bottlenecks are `reactivity` 89 s, `animation` 73 s, `sanity` 46 s
     and `distinctness` 41 s; the five suites named above are worth ~8 s together, so following
     this line literally would have left the gate at ~86 s. The hook excludes the **measured**
     nine — the five above plus those four — landing at ~28 s / 166 of 180 tests. Do not "restore"
     the list above.
  3. An induced `rustfmt` violation blocks a real `git push`, the message names `cargo fmt` as the
     failing step, and `git push --no-verify` bypasses it.
  4. The README gains a short developer section: the one-line install
     `git config core.hooksPath .githooks`, what the hook runs, the measured time, and the explicit
     statement that **an uninstalled clone has no gate** (git will not run hooks from a tracked
     directory without that config).

### Phase 4 — Measure coverage, then set the ratchet

- **Owner skill:** dev
- **What:** A `coverage` CI job on `windows-latest` only — WARP makes the render paths genuinely
  executable there, while on macOS every GPU test skips and would report that code as dead. It runs
  `cargo llvm-cov nextest -p lmv-core --fail-under-lines $COVERAGE_FLOOR` with `cargo-llvm-cov`
  installed via `taiki-e/install-action`, alongside the existing `nextest` and `cargo-deny`
  installs. **Measure before choosing the number** — the floor is set from reality, not aspiration.
- **Files touched:** `.github/workflows/ci.yml`, `docs/nfr.md` (§7).
- **Done when:**
  1. The job runs green against the current suite, having landed **after** Phases 1-2 so the floor
     reflects the improved suite rather than being stale on arrival.
  2. `COVERAGE_FLOOR` is `floor(measured) - 2` (a margin so an unrelated change does not trip on
     rounding), lives in exactly **one** place — an `env:` key in `ci.yml` — and carries a comment
     stating the ratchet rule: raise it at a close ceremony when a plan improves coverage; never
     lower it without a one-line note naming the plan that lowered it and why.
  3. **The gate is proven to bite.** Delete a covered test file locally, confirm the number drops
     and the job fails, restore it. A gate never seen to fail is not known to work.
  4. The per-file breakdown lands in the GitHub job summary, and the measured baseline plus the
     five least-covered `core/` modules are recorded in this plan's close notes so the next plan
     can aim at them.
  5. `docs/nfr.md` §7 gains the coverage line and the pre-push gate, so the CI section stops being
     a stale description of a three-check pipeline.

### Phase 5 — Install the hook in your clone

- **Owner skill:** human
- **What:** `core.hooksPath` is per-clone local config; nothing in the repo can set it for you, and
  no phase above has any effect until you do.
- **Files touched:** none (local git config).
- **Done when:** `git config core.hooksPath .githooks` is set, and a deliberately misformatted file
  causes a real `git push` to be refused before any object is sent.

## Data shapes

No new structs, no schema, no ABI change. The one shape worth pinning is the CI floor, so nobody
has to hunt for where the number lives:

```yaml
# illustrative — .github/workflows/ci.yml
env:
  # ADR-0033 ratchet: line coverage of lmv-core only. Raise at a plan close when
  # coverage improves; never lower without a note naming the plan and the reason.
  COVERAGE_FLOOR: "<set from the Phase 4 measurement>"
```

## Risks & open questions

- **The ratchet can be tripped by a legitimate refactor.** Plan 0031 collapses three `Renderer`
  constructors and deletes duplicated GPU boilerplate; removing code changes the ratio in either
  direction. Deleting *well-covered* code lowers it. The escape is deliberate and visible — lower
  the floor with a note — and is the accepted cost of having a ratchet (ADR-0033, Negative
  consequences).
- **Plan 0023 is in flight and adds substantial new render code** (`core/src/render/transition.rs`
  is uncommitted in the working tree as of 2026-07-25). New, thinly-tested code lowers the ratio.
  That pressure is the point, but it is unpleasant to discover mid-plan — hence the sequencing note
  below recommending Phase 4 land after 0023 closes.
- **`cargo llvm-cov` against the WARP/DX12 path is unverified.** llvm-cov instruments Rust only, so
  the graphics driver is unaffected in principle, but instrumented binaries are slower and the
  GPU-heavy suites may stretch noticeably. Mitigation: it is a separate job off the critical path.
  If it proves unusable on Windows, the fallback is to gate coverage over the **non-GPU** subset of
  `lmv-core` (`dsp`, `preset`, `signal`, `audio`) with a correspondingly higher floor — a narrower
  gate, not no gate. Do not silently move it to macOS: the GPU tests skip there.
- **Warm `cargo nextest run` wall time is unmeasured.** A concurrent `dev` session held the tree
  mid-edit while this plan was written, so no local timing was taken. Phase 3 measures it; the ~90 s
  narrowing rule exists precisely because the number could come back either way.
- **Whether `cargo nextest run` builds `examples/`** decides if Phase 2 needs an explicit build step
  in CI and the hook. Phase 2 verifies rather than assumes.
- **The e2e chain test duplicates `main.rs`'s drain policy** instead of sharing it, so the two can
  drift. Extracting that loop into `standalone/src/lib.rs` (or `core`) is a followup below, not
  this plan — it would put a shell behavior change inside a testing plan.
- **`--fail-under-lines` failing is a hard CI stop.** If the margin proves too tight in practice
  (unrelated PRs tripping it), widen the margin rather than removing the gate.

## What this plan does NOT do

- **No coverage gate on `standalone/` or `plugin-foobar/`.** ADR-0033 Alternative A.
- **No standalone boot smoke test.** Launching the real `winit` window headlessly to prove surface
  creation, capture init, and the first frames was offered in the interview and not selected. It
  remains the one path CI has never executed; a future plan can take it with a `--selftest` flag.
- **No expansion of the C ABI lifecycle tests.** `core/tests/ffi.rs`'s five tests are left as they
  are; a fuller create → push → render → resize → free misuse matrix was offered and not selected.
- **No mocking of WASAPI or ScreenCaptureKit**, and no traits introduced to make platform capture
  "coverable".
- **No change to the golden tolerance, the `LMV_BLESS` flow, or any existing suite.** This plan adds
  tiers and gates; it does not retune what is already there.
- **No new Cargo dependency.** `cargo-llvm-cov` is a CI-installed tool, not a manifest entry — NFR
  §4's budget is untouched.
- **No `PreToolUse` push hook.** CLAUDE.md already forbids the assistant from pushing (ADR-0033
  Alternative G).
- **No `docs/testing.md`.** The tier table lives in ADR-0033; the README carries the operator-facing
  hook instructions. A living guide can be split out later if the table starts wanting edits.

## Sequencing

Phases 1-3 are independent of every active plan and can land at any time. Two ordering notes:

- **Phase 2 is worth landing before Plan 0031**, whose Phase 1 refactors `standalone/examples/shot.rs`
  (1028 lines, 45 functions, currently zero tests). A subprocess suite is exactly the behavioral net
  that refactor should be done under.
- **Phase 4 is best landed after Plan 0023 closes.** Setting the floor while a large, new,
  thinly-tested render subsystem is mid-flight either sets it artificially low or trips it
  immediately. This is a judgment call, not a hard dependency — if the user prefers the floor set
  now, widen the Phase 4 margin instead.

## Followups (after this lands)

- Extract `main.rs`'s ring-drain loop (`main.rs:275-281`) into shared testable code so the e2e chain
  test exercises the real policy rather than a copy of it.
- Raise `COVERAGE_FLOOR` at the next close ceremony that improves coverage, and record the new
  number's justification.
- Aim a targeted-coverage plan at the five least-covered `core/` modules identified by Phase 4.
- Reconsider the standalone boot smoke test and the fuller C ABI lifecycle matrix — both were
  scoped out here by user choice, not because they lack value.
- Revisit `plugin-foobar/` once the shim carries any logic of its own; today it is a thin
  `extern "C"` caller over a surface `core/tests/ffi.rs` covers.

## Close (2026-07-26)

**Status: done.** Passed the Mode 4 review — **no blockers, no majors**; four minors, two nits.
Four `dev` phase commits plus one architect close commit:

| Commit | Phase |
|--------|-------|
| `332720f` | 1 — `core/tests/chain.rs`, the ring→analyzer→renderer e2e suite |
| `108e21a` | 2 — `standalone/tests/shot_cli.rs`, the `shot` binary as a subprocess |
| `ee89905` | 3 — `.githooks/pre-push` + the README developer section |
| `a4b7045` | 4 — the `coverage` CI job, the `COVERAGE_FLOOR` ratchet, `docs/nfr.md` §7 |

Phase 5 is `human`: `core.hooksPath` is now `.githooks` in the user's clone (verified at review).
The other half of its done-when — a misformatted file causing a real `git push` to be refused — is
observable on the user's next push and is deliberately not something the assistant can run.

### What landed

The suite grew a **tier 4** it did not have. `core/tests/chain.rs` pushes synthetic PCM into a real
`audio::intake` SPSC pair in 20 ms capture-callback-sized bursts, drains it through `pop_samples`
into a fixed scratch with the shell's own `pump_audio` policy, feeds a real `Analyzer`, and renders
a real `Renderer` to real pixels — the first test that crosses the seam CLAUDE.md opens with.
`standalone/tests/shot_cli.rs` runs the built `shot` binary the way a user does, closing the fact
that the CLI the `preset-author` lane self-verifies through had **no tests of any kind** (`#[test]`
does not run in an `examples/` target). `shot` stayed an example rather than a `[[bin]]`
(ADR-0033 Alternative E), located by walking `current_exe()`'s ancestors for the `examples/`
sibling. `.githooks/pre-push` runs `fmt` + `clippy` + a narrowed `nextest` in ~28 s, opt-in per
clone. A `windows-latest` `coverage` job gates `lmv-core` line coverage behind one `env:` key.

### Verified at review, not taken on trust

- **166/166 green** in the hook's narrowed set in **22 s** wall (README says ~26 s for the test
  step; the measurement holds), and `cargo nextest list` reports **180** total — the README's
  "166 of 180" is exact.
- **`cargo llvm-cov nextest -p lmv-core --fail-under-lines 88` re-run locally: exit 0, line
  coverage 90.13 %** — the measured number in the `ci.yml` comment and in `docs/nfr.md` §7 is
  correct to the digit, and the 2.13-point margin is real.
- **The band-routing claim is non-vacuous, and more strongly than its own doc comment claims.**
  Re-running the probe with the *treble* bindings neutralized (`hue = 0`, `glow = 0.15`) drops the
  difference to **0.0243 and fails the 0.05 floor**; re-running with the *bass* bindings
  neutralized (`warp`/`zoom` fixed) still **passes**. So the treble half is load-bearing — the
  test cannot be satisfied by bass alone, which is the failure mode a band-routing test most
  plausibly has.
- The determinism claim asserts `first.rgba == second.rgba` (byte-identical, not a tolerance), and
  the format-validation claim pins the exact `FormatError` variant per case plus a valid-format
  control so the rejections are rejecting the *format*, not the call.
- `.githooks/pre-push` is mode **100755** in the index, so it will execute on a POSIX clone — the
  portability trap a checked-in hook usually falls into, avoided.
- `chain.rs`'s drain loop is a faithful copy of `main.rs`'s `pump_audio` (`standalone/src/main.rs:284`),
  and says in its own header that it is a copy and can drift.
- **Manifests untouched** — `Cargo.toml`/`Cargo.lock` are not in the diff range, so "no new Cargo
  dependency" is true in fact. The hand-rolled string/escape-aware JSON helpers are what bought
  it, and they carry their own unit test with **negative** cases (mismatched closer, unclosed
  object, unclosed string, nested-key exclusion).
- `clippy --all-targets -- -D warnings` and `cargo fmt --all --check` clean.
- C ABI untouched (v4); `Scene` untouched; no production-code change of any kind.

### Accepted `dev` judgment calls

- **The Phase 3 narrowing list was replaced by measurement** (see the struck done-when above) —
  approved by the user mid-phase.
- **Hand-rolled JSON assertions instead of `serde_json`**, to keep "no new Cargo dependency" true
  rather than nearly true. Correct call, and the helpers are themselves tested.
- **Local dev tooling installed** for the Phase 4 measurement (`rustup component add
  llvm-tools-preview`, `cargo install cargo-llvm-cov`). Neither touches the manifests.

### Findings (all minor or below; the three fixable ones are fixed in this close commit)

1. **Minor — the coverage job's summary step reported a different scope than the gate.**
   `cargo llvm-cov report --summary-only` re-reports every object in the profile data, which
   includes `lmv-ring`; the step below the "floor: 88 %" heading was therefore not the number the
   step above enforces. Confirmed locally: the unscoped report carries an `lmv-ring/src/lib.rs`
   row the gated run does not. Today both come to 90.13 % by coincidence. **Fixed:**
   `cargo llvm-cov report -p lmv-core --summary-only`, with a comment saying why the flag is
   load-bearing.
2. **Minor — `ci.yml`'s own header comment contradicted the file it heads.** It still said
   "build, test, clippy, fmt" and "GPU rendering and live audio are out of CI scope" — the exact
   sentence this plan corrected in `docs/nfr.md` §7 while leaving the workflow's own header
   stale. **Fixed.**
3. **Minor — `CLAUDE.md` did not know `.githooks/` exists.** A new checked-in top-level directory
   holding an opt-in dev gate was absent from the orientation map, whose hook paragraph named only
   `.claude/hooks/block-broad-git-add.js`. **Fixed** — the tree now carries `.githooks/` with the
   opt-in caveat.
4. **Minor — plan-text drift, corrected above:** Phase 1's `test(chain)` selector and Phase 3's
   guessed narrowing list.
5. **Nit — `scratch()` never cleans up.** `standalone/tests/shot_cli.rs:95` creates
   `target/shot-cli-tests/<pid>-<name>/` per run and never removes it, so directories accumulate
   across runs. Under `target/`, so `cargo clean` sweeps them; not worth a fix on its own.
6. **Nit — Phase 1 claim 3 asserts analyzer frames, not rendered frames.** The plan's wording was
   "the chain keeps producing frames afterward";
   `ring_overflow_drops_samples_and_the_chain_keeps_running` asserts `analysis.bass > 0.0` and
   never renders. The substitution makes the test GPU-free, which is a net gain — but the renderer
   is not in the loop for that one claim.

### Unverifiable here, first-push discoveries

The `coverage` job and the three GPU `shot_cli` tests have **never run in CI** — nothing has been
pushed since these landed. Two specifics to watch on the first push: whether `windows-latest`
exposes an adapter that satisfies `shot`'s `force_fallback_adapter: false` request (if not, those
three skip with a printed reason rather than fail — the skip is keyed on the adapter error, not on
the OS, so the suite degrades correctly either way), and how much the instrumented WARP suites
stretch the coverage job's wall time. `.github/workflows/ci.yml` is in this range, so the push
needs a credential with the `workflow` OAuth scope (`gh auth refresh -s workflow`).

### Baseline for the followup coverage plan

Measured `lmv-core` line coverage **90.13 %** (floor **88**). The five least-covered `core/`
modules, which the followup below should aim at:

| Module | Line coverage |
|--------|---------------|
| `render/overlay_font.rs` | 0.00 % |
| `render/overlay.rs` | 30.69 % |
| `render/context.rs` | 34.71 % |
| `ffi.rs` | 56.60 % |
| `diag/mod.rs` | 65.75 % |

### Version

**No bump** — deliberate, not a miss. Every commit in the range is chore-class (`test`, `test`,
`build`, `ci`); no production code shipped, the binary is unchanged, and `docs/releasing.md` gives
a docs/chore-only plan no bump. The version stays **0.16.0**.
