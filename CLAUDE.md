# Ritmolux

A lightweight, real-time music visualizer built around one **shared Rust core** that
turns a stream of PCM audio samples into GPU-rendered visuals. Two frontends consume
that core:

- **Standalone app** (Windows + macOS) — pure Rust (`winit` + `wgpu`), fed by OS
  loopback audio capture.
- **foobar2000 plugin** (Windows-first) — a thin **C++ shim** over the core's **C ABI**,
  fed by foobar's own `visualisation_stream` (no loopback needed on that path).

The core is **source-agnostic**: it takes interleaved/mono PCM frames and does not care
whether they came from loopback capture or foobar. That single abstraction is what makes
one visual codebase serve both frontends. Do not leak audio-source specifics into the core.

This file is the orientation map — it says **which part owns what** and **how they hand
off**, not how the code works. Decisions live in `docs/adrs/`; work-in-flight lives in
`docs/plans/`.

## Architecture at a glance

```
                 PCM frames (source-agnostic)
   loopback ----\                              /---- foobar visualisation_stream
                 v                            v
        [ standalone shell ]         [ foobar plugin: C++ shim ]
          (Rust: winit)                  (links core via C ABI)
                 \                            /
                  v                          v
                   +--------- core ---------+
                   |  Rust: DSP + render    |
                   |  - FFT / spectrum      |
                   |  - beat / onset detect |
                   |  - scene graph + wgpu  |
                   +------------------------+
                        wgpu -> Metal (mac) / DX12 · Vulkan (win)
```

Key architectural decisions are recorded as ADRs. The founding one is
[ADR-0001](docs/adrs/0001-rust-core-wgpu-cabi-foobar-shim.md) — Rust core, wgpu rendering,
C ABI, C++ foobar shim. **Read it before questioning the language/GPU/FFI split.**

## Where things live

```
core/                # Rust library crate — the shared brain. DSP + render engine + scenes.
                     #   `rlib` ONLY (ADR-0072): every consumer in the workspace links the
                     #   rlib, so a core edit stops re-emitting artifacts nothing reads.
                     #   NO audio-source code here.
    └── src/milk/    #   The MilkDrop runtime side of ADR-0113: the per-frame bytecode VM and
                     #   shader emitter a converted preset drives. Distinct from milkconv/, which
                     #   is the ahead-of-time converter and never ships.
core-cabi/           # The C ABI, and nothing else (ADR-0072) — the only crate declaring
                     #   cdylib/staticlib, plus include/rlx_core.h. Deliberately OUTSIDE the
                     #   workspace `default-members`, so a bare `cargo build` never emits it;
                     #   `--workspace` (CI, pre-push) and `-p rlx-core-cabi` do.
rlx-ring/            # The lock-free SPSC ring, extracted zero-dependency so Miri gates it in CI.
standalone/          # Rust binary + lib — winit window, wgpu surface, loopback capture, `shot`.
plugin-foobar/       # C++ shim: foobar2000 SDK integration, links core's C ABI. Windows-first.
milkconv/            # The MilkDrop `.milk` -> preset converter (ADR-0113). A full workspace member
                     #   that NEVER ships and that no shipped artifact depends on, so it is OUTSIDE
                     #   `default-members` like core-cabi: `--workspace`, `-p milkconv`, or its own
                     #   tests build it, and the everyday loop does not. Built by Plan 0100; the
                     #   `.milk` corpus it converts lives outside the repo, not in this checkout.
presets/             # The curated preset library (*.toml) — build.rs globs and embeds it.
    ├── README.md    #   THE per-system parameter roster + structural/palette/smoothing tables.
    └── pending/     #   Authored, approved, NOT shipped — held back by a known engine or harness
                     #   gap, not by the look. build.rs's read_dir is non-recursive (ADR-0022), so
                     #   a subdirectory is skipped by construction. See its own README.
tools/
└── sd-filter/       # Python sidecar for the diffusion-filter pass (ADR-0122). Not a cargo crate,
                     #   not in the workspace, never shipped; its cost figures live in exactly one
                     #   page (docs/diffusion-filter.md) and check-filter-figures.mjs holds them there.
site/                # The documentation front end (ADR-0154): an
                     #   Astro Starlight site publishing the READER-FACING subset of docs/ with real
                     #   search, live at igorkonovalov.github.io/Ritmolux/. The repository's only npm
                     #   project; never shipped, nothing shipped depends on it. `docs/` stays the
                     #   SINGLE SOURCE - the content loader reads docs/, docs/specs/ and
                     #   presets/README.md IN PLACE, there is no staged copy, and no markdown file
                     #   outside site/ may be edited to serve it. The publish boundary is the
                     #   PUBLISHED map in src/plugins/rewrite-links.mjs, which also rewrites every
                     #   relative link at build time: inside the set to a site route, outside it to
                     #   a github.com blob URL. A new doc does not join the site by existing.
packaging/           # What a `v*` tag ships (ADR-0038). macos/ holds bundle.sh — build both
                     #   Apple targets, lipo, substitute the plist version, ad-hoc sign, zip AND
                     #   verify — so packaging runs the same on a Mac as in CI, not CI-only magic.
                     #   Plus the two READ-ME-FIRST.md a tester finds in the zip.
docs/                # Full one-line-per-doc map: README.md "Repository layout". The load-bearing set:
├── nfr.md           # Quantified v1 non-functional requirements — the numbers behind every
│                    #   "lightweight" / "real-time" / "stable frame rate" in the plans.
├── preset-guide.md  # START HERE for presets — the illustrated entrance, one picture per system.
├── presets.md       # Preset authoring guide: THE expression-language reference.
├── preset-palettes.md  # The colour surface: palettes, custom stops, A/B crossfade.
├── preset-tuning-walkthrough.md  # One preset tuned over five steps, picture + --report row each.
├── capturing.md     # Headless `shot` CLI + the core/tests/ visual-QA harness + `--render` video.
├── on-device-validation.md  # The manual checklist for what CI cannot run: real GPUs, live
│                    #   loopback, installing the foobar2000 component.
├── design-backlog.md  # The preset-author -> architect inbox: captured friction not yet an ADR or
│                    #   a plan. Every live entry carries an executable probe (ADR-0108).
│                    #   -archive.md holds retired entries; both are append-only records.
├── roadmap-visual-richness.md  # The visual-capability roadmap recent plans are sequenced against.
├── generative-techniques-catalogue.md  # The technique survey behind the scene families.
├── content-brief.md # What the shipped preset set is FOR — the curation brief behind the library.
├── diffusion-filter.md  # The diffusion filter's cost figures, held to one page by a gate.
├── releasing.md     # How the version moves (one bump per plan close) + the tag push that
│                    #   builds and publishes the two release zips.
├── images/          # Committed documentation renders, regenerated by scripts/docs-shots.mjs.
├── examples/        # Teaching presets for the guide + walkthrough. Never shipped, never seeded.
├── specs/           # NNNN-<subsystem>.md — living behavioral contracts (C ABI, ring/DSP).
│                    #   Deliberately minimal: the two highest-value contracts, no enforcement
│                    #   machinery (ADR-0004). Not a gap — see its own README.
├── adrs/            # NNNN-<slug>.md — architecture decisions + rejected alternatives. Append-only.
│   └── README.md    #   ADR index
└── plans/           # NNNN-<slug>.md — phased implementation plans (what's in flight)
    ├── README.md    #   Plans index: roster + next free number. Read this first each session.
    └── done/         #   Completed plans move here
.claude/
├── skills/          # architect (designs docs/) + dev (all code) + preset-author (preset content)
├── settings.json    # Registers the block-broad-git-add PreToolUse hook
└── hooks/           # block-broad-git-add.js — enforces explicit-path staging
.githooks/           # Checked-in git hooks. pre-push runs the fast subset (doc links + fmt +
                     #   clippy + a narrowed nextest, ~28 s). OPT-IN PER CLONE — nothing runs
                     #   until `git config core.hooksPath .githooks`. See README + ADR-0033.
scripts/             # Repo maintenance. Seven Node gates. SIX run by pre-push and by the CI
                     #   `links` job; the seventh, check-site-links.mjs, runs in neither, because it
                     #   needs a BUILT site - it lives in .github/workflows/pages.yml and asserts
                     #   that no site-relative href in site/dist/ ends in .md, that every one
                     #   resolves to a built file, and that every off-site href is absolute https
                     #   (ADR-0154). Of the six, the first three and toc.mjs also run in the close
                     #   ceremony, because a close is what breaks them. check-doc-links.mjs asserts
                     #   every relative markdown link resolves (moving a plan to plans/done/ breaks
                     #   links in both directions, and rejects a design-backlog fragment outright
                     #   per ADR-0149); check-index-rows.mjs holds every roster row to 320 bytes AND
                     #   to its region's form (ADR-0116); check-backlog-claims.mjs re-runs each live
                     #   entry's probe (ADR-0108); check-filter-figures.mjs keeps the diffusion
                     #   filter's cost figures on one page; check-comment-hygiene.mjs rejects
                     #   relative links and plan-relative narration in .rs and .cpp/.h comments
                     #   (ADR-0127); toc.mjs regenerates the contents block in each of the six long
                     #   documents from the headings under it, and --check reports drift (ADR-0163)
                     #   — a block is generated, never hand-edited.
                     #   scripts/fixtures/ holds their seeded bite checks.
                     #   RENDERERS, NOT GATES: docs-shots.mjs (regenerates docs/images/),
                     #   tuple-sheets.mjs + tuple-paths.mjs (attractor roster/walk contact
                     #   sheets) and milk-softness.mjs + softness-sheets.mjs (the stroke-profile
                     #   judging sheets). Nothing runs these - an author does, by hand, and the
                     #   output lands under target/ uncommitted. They are here so that "every
                     #   .mjs is wired into pre-push or CI" reads as a rule with five named
                     #   exceptions rather than as a claim that is simply false.
```

## Machine setup: the linker override (opt-in, and inert if skipped)

**Every worktree compiles into its own `target/`.** The one machine-local override is the MSVC
linker, and it lives in a file one directory *above* the worktrees — `WORK/.cargo/config.toml`,
beside `ritmolux/` rather than inside it — so cargo's ancestor walk finds it from
whichever lane is building and a new lane needs no setup of its own:

```toml
[target.x86_64-pc-windows-msvc]
linker = "rust-lld.exe"
```

That is the whole file. `rust-lld.exe` is not on `PATH` and still resolves — rustc finds it in its
own sysroot, so no explicit path and no linker-flavor flag are needed. It took the cold path to
every test binary from 171 s to 145 s while moving no golden (ADR-0141's `Outcome`, which stands).

**It is never committed, and it cannot be.** The macOS arm has a different linker story, and
reaching `rust-lld` any other way means naming a sysroot path specific to one machine. Like
`git config core.hooksPath .githooks`
([ADR-0033](docs/adrs/0033-testing-strategy-coverage-ratchet-and-pre-push-gate.md)), this is
**opt-in per machine and inert when skipped** — a machine without the file builds correctly, just
with the default linker.

**There is no shared artifact store, and a `[build] target-dir` redirect must not go back in this
file.** [ADR-0141](docs/adrs/0141-one-artifact-store-serves-every-lane.md) pointed every worktree at
one store; [ADR-0147](docs/adrs/0147-the-shared-artifact-store-is-revoked-and-the-linker-stays.md)
revoked that half, because **the worktree path is not in cargo's fingerprint** — two lanes with the
same layout and dependency graph are indistinguishable, so one lane is served the other's compiled
`rlx-core` as fresh. Plan 0115's lane hit `no method named open_tap found for struct Renderer`
against committed source that defines it. The failure is silent, it is not a cache miss, and no gate
catches it. The linker half above is not implicated and stays.

The cost that comes back with the revocation is disk, and it is not gated:

- **[ADR-0053](docs/adrs/0053-plan-lanes-run-in-git-worktrees.md)'s *"disk cost is severe and
  recurring"* is live again.** One lane held ~8 GB in `target/debug/incremental` and filled the
  disk mid-session. **Remove a finished lane's worktree** — on Windows `git worktree remove` fails
  with `Permission denied` while any shell still has its working directory inside it.

## How we work (canonical workflow)

This project runs a **three-skill** plan-driven harness (`.claude/skills/`), adapted from the
market-analyzer repo down to just the split that matters here (the third lane, `preset-author`,
was added per [ADR-0017](docs/adrs/0017-preset-author-skill-lane.md)):

| Skill           | Owns                                             | Triggers on |
|-----------------|--------------------------------------------------|-------------|
| `architect`     | `docs/` — plans, ADRs, diagrams, reviews         | "how should we build X", "design the …", "should we A or B", "plan the …", "review plan N" |
| `dev`           | all code — `core/`, `standalone/`, `plugin-foobar/` | "implement plan N", "do the DSP phase", "code up the …" |
| `preset-author` | preset **content** — `.toml` presets, expression bindings, `[curve]`/`[generator]` config; never engine Rust | "make an aurora-style preset", "a look that pulses on the beat", "tune rose_star", "make it more organic", "design a preset for the drop" |

**The hard split: `architect` designs, `dev` builds, `preset-author` composes content — never
invert.** The architect never writes production code; `dev` authors no ADRs and writes only two
things inside a plan — the `Status:` line and the `## Implementation log` — and never reviews its
own work; `preset-author` never touches engine Rust (a look needing a new scene, param,
or grammar capability routes back to `architect` + `dev` as feedback, and `dev` — not the author —
embeds a preset into the shipped set). The handoffs are `architect → dev` (the user's "go"),
`dev → architect` (the plan's own `## Implementation log`, which `dev` writes as the phases land,
plus a three-line pointer at it — [ADR-0120](docs/adrs/0120-the-close-brief-is-a-section-of-the-plan.md);
the review itself still happens in a fresh session), and `preset-author → architect`/`dev`
(engine-gap feedback, and curation of a strong preset). All are manual on purpose — their value is
the clean-context boundary.

The loop:

```
interview  ->  ADR (if a real tradeoff)  ->  plan (phased)  ->  implement phase-by-phase  ->  fresh-session review at plan end
```

- **Interview before writing** (architect Mode 1). For any non-trivial feature, ask 3-5
  tight questions (batch them via `AskUserQuestion`) before designing. A one-minute
  interview beats a rewrite. Skip only if the user says "just draft it" — then state what
  you're guessing.
- **ADR when there's a rejected alternative.** If you can name an option you're *not*
  taking and future-you would want to know why, write an ADR (`docs/adrs/`). If you can't
  name a rejected alternative, you don't need an ADR — just a comment.
- **Plan before implementing.** Non-trivial work gets a numbered plan in `docs/plans/`
  with **ordered phases**, each tagged `**Owner skill:**` — vocabulary `dev` (all code) or
  `human` (a task only the user can do). Each phase ships as its own commit with a clear
  "done when". `dev` implements the whole plan in one session, no review between phases.
- **Review at plan end** (architect Mode 4), in a fresh session, not per phase. Check the
  implementation against the plan and the cross-cutting rules below, then flip the plan to
  `done`, `git mv` it to `plans/done/`, and refresh `docs/plans/README.md`.

Numbering: sequential, zero-padded 4 digits (`0001`). ADR and plan numbers are independent
sequences. List existing files and take the next number; the plans README tracks the next
free number so you don't have to re-glob.

## Cross-cutting non-negotiables

These apply to every part of the project. They exist because this is **real-time
audio + graphics**, where the usual "just allocate and log it" habits cause glitches.

- **The audio callback is sacred.** The thread that receives capture / `visualisation_stream`
  data must never block, allocate on the heap, lock a contended mutex, log, or do file I/O.
  Hand samples to the core through a lock-free ring buffer (SPSC) and return immediately.
  An underrun is an audible click; a blocked callback is a stutter.
- **Render and audio are decoupled.** Audio arrives at the device's cadence; frames render
  at the display's. Never drive one loop directly off the other — the ring buffer is the seam.
- **Determinism where it's testable.** DSP math (FFT bins, onset envelope, beat estimate)
  is a pure function of its input window. No wall-clock reads, no unseeded randomness inside
  analysis. Visual jitter/randomness, when wanted, is explicitly seeded so a scene is reproducible.
- **The core stays source-agnostic and GPU-abstract.** No WASAPI / ScreenCaptureKit / foobar
  types in `core/`. No raw Metal/DX/Vulkan calls outside the wgpu layer. The whole point of the
  split is swappability; a leak here forfeits it.
- **The C ABI is a contract.** The `extern "C"` surface the plugin links against is versioned
  and minimal, and **[`docs/specs/0001-c-abi.md`](docs/specs/0001-c-abi.md) is the authority on
  its shape** — not this file. **Never restate the function roster or its size here**: a paraphrase
  of it in this spot drifted twice before being retired, because a count is falsified by every ABI
  change and nothing gates one written in prose. Changing that shape is an ADR-worthy event, not a
  casual edit: the C++ side is compiled separately, so a mismatch fails at link time or, worse, at
  runtime.
- **Validate at the boundary, trust inside.** Sample-rate, channel count, and buffer sizes get
  checked once where audio enters the core; the hot path downstream assumes them valid.
- **Lightweight is a feature.** Small binaries, few dependencies, low idle CPU/GPU. Every new
  crate is a cost — justify it. Pin direct dependencies to exact versions in `Cargo.toml`.
- **A comment carries the mechanism; the decision record stays in `docs/`.** A comment states what
  the code does, the invariant it holds, the trap that would bite whoever changes it, and any
  formula or constant a reader cannot re-derive. Why an approach beat the alternative, what was
  measured, and what a threshold was argued from belong in the ADR or plan — cited by **bare
  number** (`ADR-0046`, `Plan 0045 Phase 3`), never by a relative link, which rots on the next
  `plans/done/` move and does not resolve in rustdoc anyway. Rustdoc intra-doc links stay; `rustc`
  resolves those. **No plan-relative narration** — describe the code as it is (*"the phase is
  locked, not free-running"*), never as a history (*"used to be free-running until Plan 0095"*).
  `scripts/check-comment-hygiene.mjs` gates those two mechanical classes at pre-push and in CI;
  `hygiene-allow: <reason>` in a comment escapes a false positive. Length is not gated — see
  [ADR-0127](docs/adrs/0127-a-comment-carries-the-mechanism-and-the-decision-record-stays-in-docs.md).

## Platform realities (don't rediscover these)

- **Loopback capture is not symmetric.** Windows has first-class WASAPI loopback. macOS does
  **not** — it needs ScreenCaptureKit (macOS 13+) or a virtual device (BlackHole). So
  "capture any app's audio" is Windows-first; the Mac capture path is a later, asterisked phase.
  The foobar-plugin path sidesteps capture entirely (foobar hands us samples), which is one
  reason plugin parity is valuable on Mac.
- **foobar2000's plugin SDK is C++ and Windows-centric.** The plugin is a C++ shim; it does
  not reuse Rust source directly — it links the core's compiled C ABI. Keep that seam thin.
- **wgpu targets differ per OS.** Metal on macOS, DX12/Vulkan on Windows. Write to wgpu; don't
  branch on the backend in scene code.

## Commit hygiene

- **Stage by explicit path — never `git add -A` / `.` / `--all` / `:/`.** A `PreToolUse` hook
  (`.claude/hooks/block-broad-git-add.js`) denies broad staging so stray/untracked files and
  parallel sessions don't get swept in. Run `git status` first; stage only your files.
- **Conventional commits**, one logical change (or one plan phase) per commit.
- **On Windows, commit multi-line messages via the PowerShell tool's single-quoted here-string**
  (`@'...'@`, closing `'@` at column 0) — the Bash tool mangles here-strings. Keep the body plain
  ASCII (straight hyphens, no em-dashes, no internal double-quotes) or git may misparse it.
- **Never rewrite history** (no amend/rebase/reset) and **never push** — the user pushes.

## Pitfalls to avoid

- **Don't put audio-source or platform code in `core/`.** It breaks the one abstraction the
  whole design rests on.
- **Don't allocate or block in the audio callback.** See the non-negotiables — this is the
  #1 source of real-time audio bugs.
- **Don't take an aspect ratio from an internal grid.** An internal render grid (a trail
  accumulation, a post stage's offscreen, a simulation field) is a **resolution, not a shape**:
  it is quantized and capped, so its aspect is *not* the target's, and every present is a plain
  normalized stretch. Any pass computing screen-destined geometry — a projection, a fold, a
  distance — takes its aspect from the **render target**, so the grid's own aspect cancels out.
  A `f32` aspect derived from a grid size is the bug. This has shipped twice ([Plan 0029] Phase 5
  on the attractor, [Plan 0033] Phase 6 on the composite), both times invisible at 1920x1080 and
  glaring at 1280x800 — see [ADR-0037](docs/adrs/0037-internal-grid-is-a-resolution-not-a-shape.md).
- **Don't skip the ADR for cross-cutting decisions.** New dependency, C ABI change, a second
  GPU backend, a new capture mechanism → ADR, even if the edit feels small.
- **Don't implement without a plan for non-trivial work**, and don't review your own work in
  the same session that wrote it — the fresh-context review is where drift gets caught.
- **Trust `git` / `Glob` over stale docs.** If a plan or ADR names a module that isn't there
  (or vice versa), surface the drift rather than papering over it.

[Plan 0029]: docs/plans/done/0029-attractor-resize-cost-and-ink-followups.md
[Plan 0033]: docs/plans/done/0033-internal-resolution-and-preset-surface.md
