# ADR-0113 — MilkDrop presets are translated ahead of time onto a warp-mesh idiom

> **Status:** accepted (2026-08-16, user approval)
> **Date:** 2026-08-16
> **Related plan(s):** [0100](../plans/done/0100-the-engine-speaks-milkdrop.md)

## Context

This engine has ten scene systems and **39 shipped presets**. [projectM](https://github.com/projectM-visualizer/projectM)
— the MilkDrop lineage — has a community library measured in **tens of thousands**
(a single publicly circulated collection is 52,000 `.milk` files). A visualizer's
perceived quality is its content, and no amount of engine work closes a three-order-of-magnitude
content gap by authoring.

The gap runs the other way on capability. MilkDrop and projectM composite in **8-bit additive**;
this engine accumulates in linear-light `Rgba16Float` with one engine-fixed tonemap, a real
bright-pass bloom and a dithered display write ([ADR-0046](0046-linear-light-hdr-composite-bloom-tonemap.md),
[ADR-0096](0096-the-display-write-dithers.md)). MilkDrop hands a preset `bass`/`mid`/`treb`,
their attenuated variants, and a beat heuristic; this engine's grammar carries tempo in BPM, a
beat index, bar phase and spectral novelty ([ADR-0049](0049-analysis-v2-dual-resolution-axis-normalized-bands.md),
[ADR-0050](0050-downbeat-and-phrase-tracking-with-confidence-fallback.md)). **A MilkDrop preset rendered by
this engine would look better than the same preset rendered by MilkDrop**, which is what makes
importing worth doing rather than merely cheap.

The decision is *how*. A `.milk` file is not a data format — it is **two programs and two
shaders**. It carries per-frame and per-vertex code in Nullsoft's EEL2 (an imperative C-like
language with assignment, sequencing, `megabuf`/`gmegabuf` scratch memory and `loop`), up to four
custom waves and four custom shapes each with their own init/per-frame/per-point code, and — since
MilkDrop 2 — a **warp** and a **composite** pixel shader written in HLSL against Shader Model 2/3.
None of that fits the preset surface this project has: [ADR-0002](0002-layered-preset-architecture.md)
and [ADR-0020](0020-preset-grammar-v2-branching-functions-tempo.md) make the expression layer
deliberately **pure and total** — no assignment, no sequencing, no memory, no shader compile — and
that purity is load-bearing for [determinism](../nfr.md#6-determinism) and for the property that a
preset cannot crash the app.

The user's scope call is **full fidelity, including MD2 custom pixel shaders**. That is the
largest of the options considered and it is the one that reaches the most of the library; it also
puts a shader translator on the critical path and reopens the "a preset cannot crash you"
property. The question this ADR answers is where the translation happens.

**The strongest existing implementation already answered it.** [Butterchurn](https://github.com/jberg/butterchurn)
runs original MilkDrop 2 presets in WebGL2 at high fidelity, and it does **not** interpret `.milk`
at runtime: a separate `milkdrop-preset-converter` compiles EEL2 to JavaScript
([`milkdrop-eel-parser`](https://github.com/jberg/milkdrop-eel-parser)) and HLSL to GLSL
([`milkdrop-shader-converter`](https://github.com/jberg/milkdrop-shader-converter), built on
`hlsl2glslfork` + `glsl-optimizer`), and ships **pre-converted bundles**. Full fidelity does not
require a runtime HLSL compiler. That is the fact that makes this decision affordable and safe.

## Decision

We will support MilkDrop presets by **translating them ahead of time**, in a dev-only converter
binary, into a **native LMV bundle** — and by adding to the engine the one render idiom the format
needs: a **warp mesh**.

Three pieces, and the split is the whole decision:

1. **`warp_mesh` is a new `SystemKind`, designed as a native scene rather than as a compatibility
   shim.** It resamples the previous frame through a **per-vertex mesh** whose UVs are computed at
   grid points and interpolated by the GPU, then draws waveforms, shapes and borders over the
   result. This is a strict generalization of [ADR-0048](0048-transformed-feedback.md)'s
   transformed feedback, which resamples the past through **one shared** affine-plus-procedural
   transform; the mesh makes that transform per-vertex. `preset-author` can bind it through the
   ordinary grammar and author native worlds on it that never touch MilkDrop. **MilkDrop
   compatibility falls out of an idiom the engine wanted anyway** — it is not a parallel pipeline.

2. **EEL2 becomes bytecode, and the engine gains a small stack VM to run it.** The converter parses
   EEL2 and emits a bounded instruction stream; `core` executes it against a fixed-size register
   and scratch arena, allocated once at preset load and never grown. The expression grammar is
   **not** the target and is not widened — ADR-0002's purity survives untouched, because the
   imperative language gets its own machine rather than being smuggled into the pure one. The VM
   reads no clock and no unseeded randomness, so [NFR §6](../nfr.md#6-determinism) holds.

3. **HLSL becomes WGSL in the converter, never in the shipped binary.** The bundle carries WGSL
   text; the runtime hands it to `naga` at **preset load** — off the audio thread and off the
   per-frame render path — and a failed compile **rejects that preset with a named error** in the
   same shape a bad expression is reported today. No `.milk` text, no HLSL and no shader translator
   ever enters `lmv.exe` or `foo_lmv.dll`.

The converter (`milkconv`) is a workspace member **outside `default-members`**, exactly as
`lmv-core-cabi` is ([ADR-0072](0072-the-c-abi-ships-from-its-own-crate.md)), so a bare `cargo build`
never emits it and its dependency weight never reaches a shipped artifact.

## Consequences

### Positive

- **The content gap closes by import rather than by authoring.** The library that took the
  MilkDrop community two decades becomes reachable in one plan.
- **Those presets render in linear light with a real tonemap.** The import is not parity — it is
  the same presets, better. That is a demonstrable claim, side by side against `foo_vis_milk2` on
  one track, and it is the plan's `human` gate.
- **The engine gains a fifth render idiom** (beside lines, ping-pong, fragment and compute
  particles — [ADR-0015](0015-gpu-compute-particle-idiom.md)'s catalogue), authorable natively.
  If the import half never finishes, the idiom still stands.
- **The shipped binary's threat surface does not move.** It parses no untrusted program text and
  compiles no untrusted HLSL. The safety property ADR-0002 bought is preserved by construction,
  not by review.
- **[NFR §4](../nfr.md#4-size-and-dependencies) is unaffected by the expensive half.** The parser,
  the EEL2 compiler and the HLSL→WGSL chain live in `milkconv`. What ships is a stack VM and a mesh
  scene, both small Rust.

### Negative

- **A converted preset can still trip a driver TDR.** naga validates WGSL — no out-of-bounds, no
  unbounded loops in what we generate — but a pathological converted shader can be slow enough to
  hit a GPU watchdog reset. This is the residual of the user's full-fidelity call, and it is
  strictly smaller than the runtime-compiler alternative rather than absent. The converter caps
  loop bounds and reports instruction count; the runtime does not police it further.
- **Bundles go stale.** A converted preset is frozen against the converter that made it. Improving
  the translation means re-running `milkconv` over the corpus and re-shipping, not patching in
  place. This is Butterchurn's cost too, and it is the price of not shipping a compiler.
- **Fidelity will be partial and we cannot say by how much until it is measured.** MilkDrop's
  pipeline has corners — `wave_mode` variants, motion vectors, `echo_orient`, texture sampling from
  a user `textures/` directory, `sampler_noisevol_*` volumes — and every one is a place a preset
  can look wrong rather than fail. The plan's coverage phase reports what it finds; this ADR
  deliberately promises no percentage.
- **Two languages to maintain forever.** The pure grammar for native presets and EEL2 for imported
  ones. They will drift, and an author reading one will occasionally assume the other.
- **Preset provenance is unresolved and is not a technical problem.** The large circulating
  collections have no clear licensing. Shipping third-party presets in this repository is a
  decision this ADR does not make; the import path works on a user-supplied directory regardless.

### Neutral

- `warp_mesh` presets are heavier than the current scenes: a mesh resample plus a composite shader
  pass, per frame. It is governed by the existing tier system ([ADR-0045](0045-quality-tiers-floor-and-rich.md))
  — the mesh grid is a **capacity**, exactly like every other tier value, and
  [ADR-0037](0037-internal-grid-is-a-resolution-not-a-shape.md) applies to it in full: the mesh is
  a resolution, and every screen-destined coordinate takes its aspect from the render target.

## Alternatives considered

### Alternative A — a runtime `.milk` interpreter

Parse `.milk`, compile EEL2 and translate HLSL at preset-load time, inside `lmv.exe`. **Rejected on
three counts, any one decisive.** It puts an EEL2 parser *and* a HLSL→WGSL translator in a binary
under a ~10 MB soft cap; the only mature HLSL translators are large C++ trees (`hlsl2glslfork`,
DXC) with no embeddable Rust equivalent; and it lands untrusted program text in the shipped parser,
in a process that is sometimes **foobar2000's**, not ours. Butterchurn — the highest-fidelity
implementation in existence — declined this too.

### Alternative B — convert `.milk` onto the existing preset surface

Translate a `.milk` into ordinary LMV TOML plus expression bindings. This was the cheap option and
it is the one the architect recommended before the interview. **Rejected on fidelity, which was the
user's stated priority.** The expression grammar is pure and total by ADR-0002/ADR-0020 and cannot
express assignment, sequencing or `megabuf`, so per-frame state would have to be dropped; and with
no warp mesh to target, the per-vertex half has nowhere to land. The output would be a caricature
of each preset rather than the preset.

### Alternative C — link projectM and hand it a texture

Embed projectM as a library, let it render MilkDrop presets into a texture, composite that. **Rejected
because it forfeits the entire reason to do this.** It would put a second GPU stack (OpenGL) inside
a wgpu application, contradicting [ADR-0001](0001-rust-core-wgpu-cabi-foobar-shim.md); it would drag
a large C++ dependency into a project whose dependency gate is a stated NFR; and the presets would
render in **projectM's** 8-bit additive pipeline, so none of the HDR, tonemap, bloom or analysis
advantages — the only reasons this import is interesting rather than merely useful — would apply.

### Alternative D — equations and warp mesh only, no custom shaders

Everything above except the HLSL half, capping fidelity at MilkDrop 1.x-class presets. **Rejected by
the user's explicit scope call**, and recorded here because it is not gone: Plan 0100 Phase 6 carries
a stop condition, and if the translation chain proves unviable, **this is the landing zone** — Phases
1–5 stand on their own and the plan closes at partial fidelity rather than failing.

## Notes

- MilkDrop's own authoring guide is the implementation spec:
  [milkdrop_preset_authoring.html](https://www.geisswerks.com/hosted/milkdrop2/milkdrop_preset_authoring.html).
  It is the authority on the variable roster (`zoom`/`rot`/`warp`/`cx`/`cy`/`dx`/`dy`/`sx`/`sy`/
  `decay`/`echo_*`/`wave_*`/`ob_*`/`ib_*`/`mv_*`, `q1`–`q32`, `t1`–`t8`), on the shader inputs
  (`uv`, `uv_orig`, `rad`, `ang`, `sampler_main`, `GetBlur1/2/3`, `texsize`, `aspect`, `rand_frame`,
  `rand_preset`, the `rot_*` matrix families), and on the mesh range — **`meshx` 8–128, `meshy`
  6–96**.
- Prior art worth reading before Phase 3: [`milkdrop-preset-converter`](https://github.com/jberg/milkdrop-preset-converter),
  [`milkdrop-eel-parser`](https://github.com/jberg/milkdrop-eel-parser),
  [`milkdrop-shader-converter`](https://github.com/jberg/milkdrop-shader-converter).
  `foo_vis_milk2` is the fidelity reference to compare against on Windows, and it is already the
  only serious visualizer in the foobar2000 component repository.

## Outcome (2026-08-16, from the corpus census taken before Plan 0100 started)

**No decision here changes.** A feature census over 10,347 public `.milk` files — the two projectM
preset packs, on disk and outside this repository — put numbers on three things this ADR reasoned
about qualitatively, and two of them cut against the optimistic reading. Recorded here rather than
edited into the body above, per the append-only rule; the full table is in
[Plan 0100](../plans/done/0100-the-engine-speaks-milkdrop.md#the-corpus-measured-2026-08-16).

- **"Fidelity will be partial and we cannot say by how much" is still right, but the corners are
  not all corners.** This ADR's Negative names `sampler_noisevol_*` volumes in a list of things a
  preset can look wrong on. **51 % of the corpus reads a procedural noise sampler** — the most
  common sampler after `main`. That is a scope item, not a corner, and Plan 0100 Phase 6 now names
  the six textures explicitly. They are generated internally from a seeded RNG, so supplying them
  costs no disk file and no bundle payload.
- **The same Negative's `textures/` corner is 19 % of the corpus** (1,937 files), which the plan
  continues to exclude — at a price now stated rather than assumed (user's call, same day).
- **The Alternative D landing zone is smaller than "MilkDrop 1.x-class" suggests.** 82 % of the
  corpus declares `MILKDROP_PRESET_VERSION=201` and carries HLSL, so stopping at Phase 6 lands
  ~1,847 presets plus the native idiom, not most of the library. The stop condition is still the
  right shape; what it decides is four fifths of the content, and that is worth knowing before it
  is reached rather than at it.
- **The Positive that "[NFR §4](../nfr.md#4-size-and-dependencies) is unaffected by the expensive
  half" was reasoned against headroom that no longer exists.** It remains true of `milkconv`, which
  still never ships. But [Plan 0097](../plans/done/0097-the-track-announces-itself.md) closed the
  same day this ADR was accepted and took the shipped `foo_lmv.dll` from 6,774,784 to
  **8,879,104 B against the ~10 MB soft cap**, leaving ~1.07 MB. The half that *does* ship — the VM,
  the loader, the `warp_mesh` scene — spends from that, and Plan 0100's Phase 6 measurement is the
  first thing that will say by how much.

## Outcome (2026-08-16, at Plan 0100's close — the Phase 7 judgment)

**The decision stands; the motivating claim came back provisionally negative.** This ADR's Context
argues "the same preset should look better here" — the linear-light HDR pipeline against the
reference's 8-bit additive. Judged by the user over seven presets side by side against
`foo_vis_milk2` 0.2.0.0 (Plan 0100 Phase 7), the verdict was **merely different, not better** —
and the plan's own words apply: that finding is worth more than the feature. The qualifier the
evidence supports: the verdict is dominated by a single defect this ADR's framing predicts —
**the float field never truncates where the reference's 8-bit target does** (design-backlog 0106),
which washes or inverts every feedback-heavy preset. The one pair whose tone survived (*Blur Mix
3*) looked genuinely good, so the claim is unfalsified where the defect is absent. The HDR
question is re-judged after 0106 lands; conversion fidelity itself was judged *mostly there, with
defects* (0106–0108), and the provenance question (Phase 8) was deferred with nothing shipping.
