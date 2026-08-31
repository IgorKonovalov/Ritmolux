# Project context — dev's view

Where things live and the canonical commands. Trust `Glob`/`git` over this when they disagree.

## Repo layout (Cargo workspace)

Intended shape for orientation, not an inventory — trust `Glob`/`git` for what actually exists
today (the tree has grown well past the founding scaffold).

```
core/            # package `lmv-core` — DSP + render + scenes + preset engine. NO C ABI.
                 #   crate-type = ["rlib"] ONLY (ADR-0072)
  build.rs       #   globs presets/*.toml into the EMBEDDED table (ADR-0022) — generated, not edited
  src/audio.rs   #   source-agnostic sample intake, validated at the boundary
  src/dsp/       #   bands/fft/onset/beat — pure, deterministic, unit-tested
  src/preset/    #   .toml schema (schema.rs) + the pure expression evaluator (expr.rs)
  src/milk/      #   the MilkDrop RUNTIME (ADR-0113): per-frame bytecode VM + shader emitter.
                 #   Not milkconv/ — that is the ahead-of-time converter and never ships.
  src/render/    #   wgpu layer, the composite stages, and scenes/ (NOT core/src/scenes/)
  tests/         #   incl. the behavioral gates: sanity.rs, reactivity.rs, animation.rs, golden.rs
core-cabi/       # package `lmv-core-cabi` — the C ABI and nothing else (ADR-0072).
                 #   The ONLY crate declaring cdylib/staticlib; emitted lib stem is `lmv_core_c`.
  src/lib.rs     #   the extern "C" surface (was core/src/ffi.rs)
  include/       #   lmv_core.h — the C mirror the shim compiles against
  tests/ffi.rs   #   the ABI conformance suite
                 #   OUTSIDE workspace `default-members` — see the commands table below
lmv-ring/        # package `lmv-ring` — the lock-free SPSC ring, zero-dependency so Miri gates it
standalone/      # package `standalone`, binary `lmv` — winit + wgpu + loopback capture
  examples/shot.rs #  the headless capture CLI (an example, not a bin — keeps `image` out of lmv.exe)
plugin-foobar/   # C++ shim — foobar2000 SDK glue, links core's C ABI (Windows-first)
milkconv/        # package `milkconv` — the MilkDrop `.milk` -> preset converter (ADR-0113).
                 #   NEVER ships and nothing shipped depends on it, so it is OUTSIDE
                 #   `default-members` like core-cabi: `--workspace`, `-p milkconv`, or its own
                 #   tests build it, and the everyday loop does not.
presets/         # the shipped preset library (*.toml) + README.md (the param roster)
  pending/       #   authored + approved but NOT shipped, held back by a known engine/harness gap.
                 #   build.rs's read_dir is non-recursive (ADR-0022), so it is skipped by design.
tools/sd-filter/ # Python sidecar for the diffusion-filter pass (ADR-0122). Not a cargo crate.
scripts/         # the five Node gates (doc links, index rows, backlog claims, filter figures,
                 #   comment hygiene) — pre-push and CI's `links` job run all five
docs/adrs/  docs/plans/  docs/specs/
```

## The machine-local cargo config (opt-in)

A machine-local `WORK/.cargo/config.toml` — one directory above every worktree, found by cargo's
ancestor walk — points the MSVC target at the toolchain's bundled `rust-lld`:

```toml
[target.x86_64-pc-windows-msvc]
linker = "rust-lld.exe"
```

That is the whole file. It is **never committed** and **inert when absent** — a machine without it
builds correctly, just with the default linker — so every command below is unchanged either way.

**Each worktree compiles into its own `target/`, and it must stay that way.**
[ADR-0141](../../../../docs/adrs/0141-one-artifact-store-serves-every-lane.md) added a
`[build] target-dir` redirect to a single shared store;
[ADR-0147](../../../../docs/adrs/0147-the-shared-artifact-store-is-revoked-and-the-linker-stays.md)
revoked it, because **the worktree path is not in cargo's fingerprint** — two lanes with the same
layout and dependency graph are indistinguishable, so cargo hands one lane the other's compiled
`lmv-core` and calls it fresh. Plan 0115's lane hit `no method named open_tap found for struct
Renderer` against source that defines it. **If you see a compile error naming a symbol you can read
in the file, suspect this before you suspect your code** — and check that no `[build] target-dir`
has reappeared in that config. The linker half above is not implicated.

What this leaves you:

- **Never hardcode `<repo>/target` in a script or a test.** Ask `cargo metadata --format-version 1`
  for `target_directory`; `plugin-foobar/build.ps1` does this. It is right under any layout.
- **Disk cost is severe and recurring** (ADR-0053): one lane held ~8 GB in
  `target/debug/incremental`. Removing a finished lane's worktree is the chore that pays it back.

## Canonical commands (run from repo root)

**The core package is `lmv-core`, not `core`** — `cargo test -p core` fails with "package ID
specification `core` did not match any packages". The directory and the package name differ.

| Task                         | Command |
|------------------------------|---------|
| Build all                    | `cargo build` |
| Run the standalone           | `cargo run -p standalone` |
| Test all / just core         | `cargo test --workspace` / `cargo test -p lmv-core` |
| **Test, per phase**          | `cargo nextest run --workspace -P fast` — the narrowed set, plus **2 presets per family** (ADR-0156, ADR-0157) |
| **Test, last phase + close** | `cargo nextest run --workspace` — the full suite, **all 81 presets**, owed once per plan |
| Lints (errors)               | `cargo clippy --workspace --all-targets -- -D warnings` |
| Format check / apply         | `cargo fmt --all --check` / `cargo fmt --all` |
| Build C-ABI artifacts        | `cargo build -p lmv-core-cabi` (emits `lmv_core_c.lib` / `.dll`) |
| Headless render check        | `cargo run -p standalone --example shot -- <flags>` (`docs/capturing.md`) |

**`--workspace` is load-bearing on BOTH test rows and on the clippy row, not a stylistic
flourish** (ADR-0072). `lmv-core-cabi` sits outside the workspace `default-members`, which is what
makes a bare `cargo build` stop re-emitting ~550 MB of artifacts nothing links — and the same
exclusion means a bare `cargo nextest run` silently skips the ABI conformance suite and a bare
`cargo clippy` silently stops linting the C ABI. Both come back green while covering nothing.
`-P fast` narrows which **binaries** run and does nothing about `default-members`, so dropping
`--workspace` from the narrowed row loses the C ABI exactly as it always did. `.githooks/pre-push`
and `ci.yml` both pass `--workspace` for this reason; match them.

**The test step is tiered (ADR-0156 and ADR-0157), and the tier is mostly about *when*.** Every
phase owes `cargo build`, `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo fmt --all --check` and the **narrowed** `cargo nextest run --workspace -P fast`, all green
before you commit it. The **full** `cargo nextest run --workspace` is owed **once per plan**, at
the last phase, before the close handoff. What `-P fast` holds back is deferred rather than
skipped: those suites sweep the shipped preset library or every scene through a real adapter, so
their price is set by the `preset-author` lane's output rather than by the change under test. Since
ADR-0157 the exclusion is not total — three of the nine run a preset sample, described next.

**Which scope renders which presets (ADR-0157).** `-P fast` renders the **representatives** — the
two presets per family that declare `representative = true`, so **24 of 81** through `animation`,
`reactivity` and `sanity`'s loudness gate. The bare `--workspace` run renders **all 81**, and also
adds `sanity`'s per-family shape gate and `distinctness`, neither of which is ever sampled. So a
defect in one of the other 57 presets waits for the close rather than failing the phase that caused
it — that is the deliberate trade, and it is why the once-per-plan full run is not optional. The
close path is unchanged, and so is CI's `coverage` job — which renders the whole library on
Windows and is what ADR-0081's curation gate actually rests on. CI's `check` job cites `-P fast`,
so it gains the sample too, at a cost nobody has measured on a CI runner.

**Two overrides are yours, and both go upward:**

- a phase whose own `Done when` names one of the nine runs that suite regardless of the default;
- a phase that changes what those suites measure — a scene, the composite, the preset engine, or
  the embedded preset set — runs the affected suite.

The default narrows; it never caps. **No gate enforces either override**: it rests on your
judgement of blast radius, which is the price ADR-0156 accepts for deferring the nine. The cost of
getting it wrong is that a `golden` or `sanity` regression surfaces at the last phase rather than
at the phase that caused it, and bisecting it costs a full suite per candidate commit.

A phase's done-when still wins over all of this when it says otherwise.

`shot` is how you eyeball a render change without launching the app — worth running when a phase
touches scenes, the composite, or the preset engine, since a green test suite doesn't prove the
picture is right.

**foobar plugin (Windows, C++):** built under `plugin-foobar/` with its own project/toolchain,
linking the core's staticlib + generated header. Read the plugin's own README for the current
invocation rather than guessing.

## Ownership map

- **`dev`** (you) — all code: `core/`, `standalone/`, `plugin-foobar/`.
- **`architect`** — all of `docs/`: plans, ADRs, diagrams, reviews.

Phase owner vocabulary: **`dev`** (all code) and **`human`** (a task only the user can do — a
product call, a signing cert, installing a system audio driver like BlackHole). There is no
sibling *implementer* skill, so you never hand off to another implementer mid-plan — only back to
architect at the end.

A third lane, **`preset-author`** (ADR-0017), owns preset *content* (`.toml` presets, expression
bindings, and the structural/palette/smoothing tables) and never engine Rust. Its engine gaps reach
you as a plan, routed through `architect`.

**Shipping a preset is no longer your code change.** Since
[ADR-0022](../../../../docs/adrs/0022-build-time-preset-embedding.md), `core/build.rs` globs
`presets/*.toml` and generates the `EMBEDDED` table — a `.toml` in `presets/` ships by existing.
There is no array to edit, no length type, and no count to bump; the older instruction to edit
`core/src/preset/mod.rs` and a count assert in `core/tests/preset.rs` describes a mechanism that no
longer exists. If a plan phase tells you to hand-edit that array, that's a stale plan — surface it
per "When the plan is wrong".

## Rules you implement against (from the architect's best-practices.md)

- **Audio callback is sacred** — no alloc/lock/log/IO on the capture or `visualisation_stream`
  thread; copy into the ring buffer and return.
- **Source-agnostic core** — no WASAPI/ScreenCaptureKit/foobar/winit types in `core/`.
- **wgpu-only rendering** — no raw Metal/DX/Vulkan outside the wgpu layer; scenes don't branch on
  backend.
- **Deterministic DSP** — FFT/onset/beat are pure functions of the input window; seed any visual
  randomness.
- **C ABI is a contract** — minimal, versioned, explicit ownership/lifetimes; don't let Rust
  panics cross FFI as UB (catch at the boundary, return an error code).
- **Quantified NFRs live in `docs/nfr.md`** — 60 fps @ 1080p floor, < 60 ms audio→visual
  latency, ~10 MB soft size cap, callback safety rules. Plans cite it by section; when a
  done-when names a number, that file is where it comes from.
