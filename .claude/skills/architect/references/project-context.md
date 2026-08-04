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
core/            # Rust library crate — DSP + render engine + scenes + preset engine + C ABI.
                 #   crate-type = ["rlib", "cdylib", "staticlib"]
  build.rs       #   globs presets/*.toml into the embedded set (ADR-0022)
  src/audio.rs   #   source-agnostic sample intake (validated at boundary)
  src/dsp/       #   bands/fft/onset/beat — pure, deterministic, unit-tested
  src/preset/    #   the .toml schema + the pure expression evaluator (expr.rs, schema.rs)
  src/render/    #   wgpu device/surface/context, the composite stages, and scenes/
  src/ffi.rs     #   extern "C" surface for the plugin
lmv-ring/        # the lock-free SPSC ring, extracted zero-dependency so Miri gates it in CI
standalone/      # Rust binary + lib — winit + wgpu surface + loopback capture + the `shot` example
plugin-foobar/   # C++ shim — foobar2000 SDK glue, links core's C ABI (Windows-first)
presets/         # the curated preset library (*.toml) + README.md (the param roster)
docs/
├── adrs/        # ADR-NNNN + README index
├── plans/       # plan NNNN + README index + done/
├── specs/       # NNNN-<subsystem>.md — living behavioral contracts (C ABI, ring/DSP)
└── diagrams/    # standalone mermaid (created on first use)
.claude/
├── skills/      # architect + dev + preset-author
├── hooks/       # block-broad-git-add.js
└── settings.json
```

## Canonical commands

Rust (run from repo root):

- Build everything: `cargo build`
- Run the standalone: `cargo run -p standalone`
- Tests: `cargo test` (or `cargo test -p core` for just the core)
- Lints (treated as errors): `cargo clippy --all-targets -- -D warnings`
- Format check: `cargo fmt --all --check`  (apply: `cargo fmt --all`)
- Build the C-ABI artifacts + header: `cargo build -p core` (emits cdylib/staticlib; header via
  `cbindgen` if configured)

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
