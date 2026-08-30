# Project context — architect's view

The source of truth for concrete facts about this repo. Read it to ground a decision; trust
`Glob`/`git` over it when they disagree (and surface the drift).

## What the project is

A lightweight, real-time music visualizer. One **shared Rust core** turns a stream of PCM
samples into GPU-rendered visuals via **wgpu**. Two frontends consume the core:

- **Standalone** (Windows + macOS): pure Rust, `winit` window + `wgpu` surface, fed by OS
  loopback capture (WASAPI on Windows; ScreenCaptureKit / BlackHole on macOS).
- **foobar2000 plugin** (Windows-first): a thin **C++ shim** over the core's **C ABI**, fed by
  foobar's `visualisation_stream`. No loopback needed on this path.

The core is **source-agnostic**: it accepts PCM frames and a render target and knows nothing
about where they came from. That single abstraction is why one visual codebase serves both.

## Repo layout

Cargo workspace. This is the intended shape for orientation, not an inventory — trust
`Glob`/`git` for what actually exists today (the tree has grown well past the founding scaffold).

```
core/            # Rust library crate — DSP + render engine + scenes + preset engine. NO C ABI.
                 #   crate-type = ["rlib"] only (ADR-0072)
  build.rs       #   globs presets/*.toml into the embedded set (ADR-0022)
  src/audio.rs   #   source-agnostic sample intake (validated at boundary)
  src/dsp/       #   bands/fft/onset/beat — pure, deterministic, unit-tested
  src/preset/    #   the .toml schema + the pure expression evaluator (expr.rs, schema.rs)
  src/milk/      #   the MilkDrop RUNTIME (ADR-0113) — bytecode VM + shader emitter. Distinct
                 #   from milkconv/, the ahead-of-time converter below.
  src/render/    #   wgpu device/surface/context, the composite stages, and scenes/
core-cabi/       # the C ABI and nothing else (ADR-0072) — the only crate declaring
                 #   cdylib/staticlib, emitted stem `lmv_core_c`. src/lib.rs + include/lmv_core.h
                 #   + tests/ffi.rs. OUTSIDE workspace `default-members`, so `--workspace` is
                 #   load-bearing on every test/clippy invocation that must cover the ABI.
lmv-ring/        # the lock-free SPSC ring, extracted zero-dependency so Miri gates it in CI
standalone/      # Rust binary + lib — winit + wgpu surface + loopback capture + the `shot` example
plugin-foobar/   # C++ shim — foobar2000 SDK glue, links core's C ABI (Windows-first)
milkconv/        # the MilkDrop `.milk` -> preset converter (ADR-0113, Plan 0100). Never ships and
                 #   nothing shipped depends on it, so it sits OUTSIDE `default-members` like
                 #   core-cabi — one more reason `--workspace` is the load-bearing scope.
presets/         # the curated preset library (*.toml) + README.md (the param roster)
  pending/       #   authored + approved but NOT shipped, held back by a known engine/harness gap.
                 #   Non-recursive read_dir in build.rs (ADR-0022) skips it by construction.
                 #   A plan that closes such a gap owes a look at what this holds.
tools/sd-filter/ # Python sidecar for the diffusion-filter pass (ADR-0122). Not a cargo crate.
scripts/         # the five Node gates; the first three are yours at every close (see SKILL.md)
docs/
├── adrs/        # ADR-NNNN + README index
├── plans/       # plan NNNN + README index + done/
├── specs/       # NNNN-<subsystem>.md — living behavioral contracts (C ABI, ring/DSP)
└── diagrams/    # standalone mermaid — declared as an output location, never yet created.
                 #   133 docs carry EMBEDDED mermaid instead; prefer embedding.
.claude/
├── skills/      # architect + dev + preset-author
├── hooks/       # block-broad-git-add.js
└── settings.json
```

## The machine-local cargo config (opt-in)

Lanes run in git worktrees ([ADR-0053](../../../../docs/adrs/0053-plan-lanes-run-in-git-worktrees.md)),
and **each carries its own `target/`**. The one machine-local override is a `WORK/.cargo/config.toml`
above every worktree pointing the MSVC target at `rust-lld` — **never committed**, **inert when
absent**, and worth 171 s -> 145 s on the cold path to every test binary.

There was briefly a second half: [ADR-0141](../../../../docs/adrs/0141-one-artifact-store-serves-every-lane.md)
redirected `build.target-dir` to one shared store, and
[ADR-0147](../../../../docs/adrs/0147-the-shared-artifact-store-is-revoked-and-the-linker-stays.md)
revoked it the following day. **The worktree path is not in cargo's fingerprint**, so two lanes with
the same layout and dependency graph were served each other's compiled `lmv-core` as fresh — Plan
0115's lane compiled against a `core` that does not contain its own methods.

What this changes when you design:

- **A new lane starts cold**, and opening a worktree implies a full dependency build again. ADR-0141's
  *"a new lane starts warm"* is withdrawn; do not sequence or cost a plan on it.
- **ADR-0053's concurrency positive is restored** — two lanes no longer serialize on one cargo lock,
  so a plan may assume parallel lanes build independently.
- **ADR-0053's disk Negative is live again**: *"disk cost is severe and recurring."* One lane held
  ~8 GB in `target/debug/incremental` and filled the disk mid-session. A plan that opens a lane owes
  the worktree removal at its close.
- **Nothing may hardcode `<repo>/target`.** `cargo metadata`'s `target_directory` is the answer that
  holds under any layout, and it is what keeps a future redirect from being a breaking change.

## Canonical commands

Rust (run from repo root):

- Build everything: `cargo build`
- Run the standalone: `cargo run -p standalone`
- Tests: `cargo test --workspace` (or `cargo test -p lmv-core` for just the core — the package is
  `lmv-core`, not `core`; the directory and the package name differ)
- Lints (treated as errors): `cargo clippy --workspace --all-targets -- -D warnings`
- Format check: `cargo fmt --all --check`  (apply: `cargo fmt --all`)
- Build the C-ABI artifacts: `cargo build -p lmv-core-cabi` (emits `lmv_core_c.lib`/`.dll`; the
  header is hand-maintained at `core-cabi/include/lmv_core.h`, no `cbindgen` — ADR-0003)

**`--workspace` is load-bearing, not stylistic** (ADR-0072): `lmv-core-cabi` is outside the workspace
`default-members`, so the bare forms come back green having never touched the C ABI.

foobar plugin (Windows, C++): built with its own project/toolchain under `plugin-foobar/` linking
the core's staticlib + generated header. Check the plugin's own README for the current invocation.

Headless visual QA: `cargo run -p standalone --example shot -- <flags>` (see `docs/capturing.md`).
That is how the `preset-author` lane self-verifies, and how you can eyeball a render change during
a Mode 4 review without launching the app.

## Non-functional requirements

**[docs/nfr.md](../../../../docs/nfr.md)** holds the quantified v1 NFRs (agreed 2026-07-21):
adaptive quality with a 60 fps @ 1080p iGPU floor, Win10 1903+ / macOS 13+ baseline,
< 60 ms audio→visual latency, ~10 MB soft size cap, CI from the start, GitHub-zip
distribution, and the confirmed v1 UX scope. Plans reference these by section; a done-when
that contradicts that file is a plan bug.

## Decisions on the record

The live, complete list is **`docs/adrs/README.md`** — read it for anything current; this file
does not enumerate the ADRs (there are many). The one you must know cold is the founding decision:

- **[ADR-0001](../../../../docs/adrs/0001-rust-core-wgpu-cabi-foobar-shim.md)** (accepted) — Rust
  core, wgpu rendering, C ABI, C++ foobar shim. Rejected: C++ core, Electron/web, OpenGL, two
  separate implementations. Everything else hangs off it; don't reopen it without a superseding ADR.

## Plans in flight

Read **`docs/plans/README.md`** for the live roster, execution order, and next-free-number — it is
the authority, not this file. Closed plans live in `docs/plans/done/`. Don't hardcode a plan list
here; the index is one glob away and this file would only go stale.

## Ownership map

Three skills. `architect` (this skill) owns `docs/` — plans, ADRs, diagrams, reviews. `dev` owns
all code: `core/`, `standalone/`, `plugin-foobar/`. `preset-author` owns preset **content** —
`.toml` presets, expression bindings, and the structural/palette/smoothing tables — and never
engine Rust (ADR-0017). Phase owner tags use the vocabulary `dev` (all code) and `human` (a task
only the user can do — a product call, a cert, installing a system audio driver); preset-authoring
is its own lane, not a phase owner. There are no sibling *implementer* skills — `dev` owns all code.

`preset-author` feeds you through **`docs/design-backlog.md`** — captured friction not yet promoted
to an ADR or plan. It is the one inbound channel that isn't a conversation, so check it when
deciding what to design next.

## Platform realities

- **Windows loopback is first-class (WASAPI); macOS is not.** Mac needs ScreenCaptureKit
  (macOS 13+, prompts for screen-recording permission) or a virtual device (BlackHole). Treat
  Mac capture as an asterisked, later phase — the plugin path sidesteps capture on Mac.
- **foobar2000's SDK is C++ and Windows-centric.** The plugin does not reuse Rust source; it
  links the compiled C ABI. Keep that seam thin.
- **wgpu backends differ per OS** (Metal / DX12 / Vulkan). Write to wgpu; don't branch on backend.
