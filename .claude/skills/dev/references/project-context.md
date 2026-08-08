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
presets/         # the shipped preset library (*.toml) + README.md (the param roster)
docs/adrs/  docs/plans/  docs/specs/  docs/diagrams/
```

## Canonical commands (run from repo root)

**The core package is `lmv-core`, not `core`** — `cargo test -p core` fails with "package ID
specification `core` did not match any packages". The directory and the package name differ.

| Task                         | Command |
|------------------------------|---------|
| Build all                    | `cargo build` |
| Run the standalone           | `cargo run -p standalone` |
| Test all / just core         | `cargo test --workspace` / `cargo test -p lmv-core` |
| Test via nextest             | `cargo nextest run --workspace` (what plan closes verify with) |
| Lints (errors)               | `cargo clippy --workspace --all-targets -- -D warnings` |
| Format check / apply         | `cargo fmt --all --check` / `cargo fmt --all` |
| Build C-ABI artifacts        | `cargo build -p lmv-core-cabi` (emits `lmv_core_c.lib` / `.dll`) |
| Headless render check        | `cargo run -p standalone --example shot -- <flags>` (`docs/capturing.md`) |

**`--workspace` is load-bearing on the test and clippy rows, not a stylistic flourish** (ADR-0072).
`lmv-core-cabi` sits outside the workspace `default-members`, which is what makes a bare `cargo
build` stop re-emitting ~550 MB of artifacts nothing links — and the same exclusion means a bare
`cargo nextest run` silently skips the ABI conformance suite and a bare `cargo clippy` silently
stops linting the C ABI. Both come back green while covering nothing. `.githooks/pre-push` and
`ci.yml` both pass `--workspace` for this reason; match them.

All four of build / test / clippy / fmt-check must be green before you commit a phase, unless the
phase's done-when says otherwise.

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
