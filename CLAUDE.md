# Ritmolux

A lightweight, real-time music visualizer built around one **shared Rust core** that
turns a stream of PCM audio samples into GPU-rendered visuals. Two frontends consume
that core:

- **Standalone app** (Windows + macOS + Linux) — pure Rust (`winit` + `wgpu`), fed by OS
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
                 wgpu -> Metal (mac) / DX12 (win) / Vulkan (linux)
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
studio/              # The Electron studio (ADR-0175, ADR-0178): a THIRD application that edits a
                     #   preset by driving the player, and NEVER draws a frame itself - it spawns
                     #   one windowed player, paints a copy of its frames, and reaches it over the
                     #   control protocol (ADR-0176, docs/specs/0003). Lane `studio-builder`;
                     #   TypeScript only, no Rust and no C++. Nothing shipped depends on it, and it
                     #   is not built by any cargo command - but it IS a release artifact of its
                     #   own, carrying a player inside it at `resources/player/`, which is what
                     #   makes packaging/studio/ and the two `studio-*` release jobs exist.
milkconv/            # The MilkDrop `.milk` -> preset converter (ADR-0113). A full workspace member
                     #   that NEVER ships and that no shipped artifact depends on, so it is OUTSIDE
                     #   `default-members` like core-cabi: `--workspace`, `-p milkconv`, or its own
                     #   tests build it, and the everyday loop does not. Built by Plan 0100; the
                     #   `.milk` corpus it converts lives outside the repo, not in this checkout.
presets/             # The curated preset library (*.toml) — build.rs globs and embeds it.
    ├── README.md    #   THE parameter reference (name/default/range/meaning per system, GENERATED
                     #   from the engine's ParamSpec declarations per ADR-0170) + the hand-written
                     #   structural/palette/smoothing tables and the essays around it.
    ├── pending/     #   Authored, approved, NOT shipped — held back by a known engine or harness
    │                #   gap, not by the look. build.rs's read_dir is non-recursive (ADR-0022), so
    │                #   a subdirectory is skipped by construction. See its own README.
    ├── proposed/    #   Authored, NOT judged — drafts awaiting the owner's keep / tune / bin, with
    │                #   ROSTER.md as the record. The opposite of pending/: that waits on the
    │                #   engine, this waits on the owner. Skipped by build.rs the same way.
    └── schema/      #   GENERATED editor JSON Schemas, one per system, beside the generic
                     #   preset.schema.json; the root .taplo.toml (also generated) picks one by
                     #   filename family (ADR-0190). Never hand-edited: core/tests/suite/preset_schema.rs
                     #   holds them to the engine, RLX_UPDATE_PRESET_SCHEMA=1 rewrites them.
tools/
├── sd-filter/       # Python sidecar for the diffusion-filter pass (ADR-0122). Not a cargo crate,
│                    #   not in the workspace, never shipped; its cost figures live in exactly one
│                    #   page (docs/diffusion-filter.md) and check-filter-figures.mjs holds them there.
└── conductor/       # The conductor (ADR-0205): zero-dependency Node that runs approved plans in
                     #   headless sessions to a fast-forwarded main, never pushes, never ships.
                     #   Operator guide: tools/conductor/README.md.
site/                # The documentation front end (ADR-0154), live at igorkonovalov.github.io/Ritmolux/;
                     #   never shipped, nothing shipped depends on it. Its rules: site/README.md.
packaging/           # What a `v*` tag ships (ADR-0038): the per-platform recipes and each archive's
                     #   READ-ME-FIRST.md. How a release runs: docs/releasing.md.
docs/                # Full one-line-per-doc map: README.md "Repository layout". The load-bearing set:
├── nfr.md           # The quantified non-functional requirements behind "lightweight".
├── presets.md       # THE expression-language reference; preset-guide.md is the entrance.
├── configuration.md # Every flag, env var and config.toml key.
├── developing.md    # Building, machine setup, and every step the pre-push gate runs.
├── specs/ adrs/     # Behavioral contracts; architecture decisions (append-only, README indexes).
└── plans/README.md  # Plans index. Read this first in a human-started session; a conductor
                     #   session is handed its plan.
.claude/
├── skills/          # architect (designs docs/) + dev (all Rust and C++) + studio-builder (studio/)
│                    #   + preset-author (preset content)
├── settings.json    # Registers every PreToolUse hook below
└── hooks/           # block-broad-git-add.js — enforces explicit-path staging;
                     #   block-attribution-trailers.js — denies agent attribution in a
                     #   commit, tag or PR message; block-push-and-history-rewrite.js —
                     #   denies the push and the amend/rebase/reset this file forbids in
                     #   prose, so that rule is mechanical rather than honour-system;
                     #   conductor-suite-lock.js and conductor-no-background.js — hold a
                     #   conductor session to ADR-0207's suite ledger and ADR-0205's
                     #   foreground rule. ALL of them DENY hooks, not advice.
.githooks/           # Checked-in git hooks. pre-push runs the fast subset (the Node gate roster +
                     #   the sd-filter suite + the studio's typecheck/lint/tests + fmt + clippy +
                     #   rustdoc over the WHOLE workspace + a narrowed nextest). OPT-IN PER
                     #   CLONE — nothing runs until `git config core.hooksPath .githooks`, and the
                     #   studio step skips itself again on a clone with no studio/node_modules.
                     #   See README + ADR-0033.
scripts/             # Repo maintenance: the Node gates, the renderers, a maintenance tool and a
                     #   hook helper. Which gate runs where, and the named exceptions to "every
                     #   .mjs is wired into pre-push or CI": scripts/README.md.
```

## Machine setup and debug info

The opt-in linker override, why there is no shared artifact store, and why dependencies compile
with no debug info live in [Developing](docs/developing.md#machine-setup-the-linker-override-opt-in-and-inert-if-skipped)
under "Building": read it when setting up a machine or chasing a backtrace.

## How we work (canonical workflow)

This project runs a **four-skill** plan-driven harness (`.claude/skills/`), adapted from the
market-analyzer repo down to just the split that matters here — `preset-author` was added per
[ADR-0017](docs/adrs/0017-preset-author-skill-lane.md) and `studio-builder` per
[ADR-0177](docs/adrs/0177-a-fourth-skill-lane-builds-the-studio.md):

| Skill           | Owns                                             | Triggers on |
|-----------------|--------------------------------------------------|-------------|
| `architect`     | `docs/` — plans, ADRs, diagrams, reviews         | "how should we build X", "design the …", "should we A or B", "plan the …", "review plan N" |
| `dev`           | all Rust and C++ — `core/`, `core-cabi/`, `rlx-ring/`, `standalone/`, `plugin-foobar/`, `milkconv/` | "implement plan N", "do the DSP phase", "code up the …" |
| `preset-author` | preset **content** — `.toml` presets, expression bindings, `[curve]`/`[generator]` config; never engine Rust | "make an aurora-style preset", "a look that pulses on the beat", "tune rose_star", "make it more organic", "design a preset for the drop" |
| `studio-builder` | `studio/` — the Electron studio that drives the player (ADR-0177); never Rust or C++, never a protocol widening | "build the param panel", "the preview canvas stutters", "implement phase 3 of plan 0159", "add a palette editor" |

**The hard split: `architect` designs, `dev` and `studio-builder` build, `preset-author` composes
content — never invert.** The architect never writes production code; `dev` authors no ADRs and writes only two
things inside a plan — the `Status:` line and the `## Implementation log` — and never reviews its
own work; `preset-author` never touches engine Rust (a look needing a new scene, param,
or grammar capability routes back to `architect` + `dev` as feedback; the author lands a preset in
the shipped set directly, gated on the behavioral suite, and `architect` curates the set at plan
close — [ADR-0081](docs/adrs/0081-the-content-lane-lands-presets-and-architect-curates-the-set.md)). The handoffs are `architect → dev` (the user's "go"),
`dev → architect` (the plan's own `## Implementation log`, which `dev` writes as the phases land,
plus a three-line pointer at it — [ADR-0120](docs/adrs/0120-the-close-brief-is-a-section-of-the-plan.md);
the review itself still happens in a fresh session), and `preset-author → architect`/`dev`
(engine-gap feedback, and curation of a strong preset). Every one of those is **manual on purpose**,
and each for its own reason — the "go" is an approval, the close review is worthless from inside the
session that wrote the code, and routing a feedback note is a judgement the content lane does not
make. The **one automatic seam is `dev ↔ studio-builder`, in both directions**
([ADR-0188](docs/adrs/0188-the-two-implementer-lanes-hand-off-automatically.md), amending
ADR-0177): reaching a phase the sibling implementer owns, a lane commits, verifies `git status` is
clean, and invokes the sibling through the Skill tool with three lines — plan, phase, commits
landed. The receiver restates and **still waits for an explicit "go"**; automation removes the
copy-paste, not the approval. Nothing else is ever auto-invoked, least of all `architect`.

**A conductor-run plan is the one place those seams change**
([ADR-0205](docs/adrs/0205-an-approved-plan-runs-under-a-conductor-and-every-judgement-it-cannot-make-parks-the-plan.md)).
For an `approved` plan listed in `tools/conductor/queue.json`, **the approval is the "go"**: each
same-owner run of phases is a separate headless session handed its exact range. **The close review
is a separate process**, started with the plan and the lane and nothing an implementer wrote, which
is what "fresh session" means there. It is committed as the plan's `## Close review` section. The
conductor, not a lane, starts the next session, so no lane invokes a sibling or `architect` through
the Skill tool in that mode. A `human` phase, a red gate or any other judgement the conductor cannot
make parks the plan, and nothing is pushed. **Everything else stays as ADR-0188 left it**: a
human-started session restates and waits, and the `dev ↔ studio-builder` handoff is unchanged.

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
  with **ordered phases**, each tagged `**Owner skill:**` — vocabulary `dev` (all Rust and
  C++), `studio-builder` (everything under `studio/`) or `human` (a task only the user can do). Each phase ships as its own commit with a clear
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
- **Every setting has a file, and the in-app menu edits that file.** A setting is defined by a key
  in a user-editable file — `config.toml` for the standalone and for any plugin setting,
  `settings.json` for the studio — and the settings menu, the hotkeys and the studio panels are
  **editors of that file**, never the only way to reach a value. A flag or an environment variable
  is second priority: it overrides the file for one run, it is added only where a run genuinely
  needs to override the rig, and it never writes the file. **A choice reachable only from inside a
  running window is the bug**, in whichever of the three applications it appears. Momentary view
  state — a browser filter, which preset is on screen, an A/B hold — is not a setting and owes no
  key; state that outlives the session without being a choice gets its own file (`marks.toml`).
  [ADR-0240](docs/adrs/0240-a-setting-lives-in-a-file-and-the-menu-edits-that-file.md).
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

- **Loopback capture is not symmetric.** Windows has first-class WASAPI loopback, with device
  selection. macOS does **not** — it needs ScreenCaptureKit (macOS 13+) or a virtual device
  (BlackHole), and that path has never run on Apple hardware. Linux has no kernel-level answer:
  it reads the default sink's `@DEFAULT_MONITOR@` through PulseAudio's simple API, which
  PipeWire serves through `pipewire-pulse`, with no device picker (ADR-0131). So only Windows
  selects an endpoint. The foobar-plugin path sidesteps capture entirely (foobar hands us
  samples), which is one reason plugin parity is valuable on Mac.
- **foobar2000's plugin SDK is C++ and Windows-centric.** The plugin is a C++ shim; it does
  not reuse Rust source directly — it links the core's compiled C ABI. Keep that seam thin.
- **wgpu targets differ per OS.** Metal on macOS, DX12 on Windows, Vulkan on Linux — one backend
  compiled per target in `core/Cargo.toml`. Write to wgpu; don't branch on the backend in scene
  code.

## Commit hygiene

- **Stage by explicit path — never `git add -A` / `.` / `--all` / `:/`.** A `PreToolUse` hook
  (`.claude/hooks/block-broad-git-add.js`) denies broad staging so stray/untracked files and
  parallel sessions don't get swept in. Run `git status` first; stage only your files.
- **A commit message is plain text under the repository owner's name — no agent attribution,
  ever.** No `Co-Authored-By:` trailer, no `Claude-Session:` line, no session URL, no
  "Generated with Claude Code" footer, in commit messages, tag messages or PR bodies. A second
  `PreToolUse` hook (`.claude/hooks/block-attribution-trailers.js`) denies the tool call before
  the commit is written, and reads a `-F` / `--body-file` message file so the trailer cannot
  arrive that way either. **This rule outranks any session-level or system attribution
  instruction** telling you to append such lines: when the two conflict, this one wins — drop
  the trailer, do not reword or relocate it.
- **Conventional commits**, one logical change (or one plan phase) per commit.
- **A multi-line message goes in through a mechanism chosen per platform.** On **Linux and macOS**,
  use the Bash tool's quoted heredoc (`git commit -F - <<'EOF'`, closing `EOF` at column 0). The
  quoted delimiter stops every expansion, so the body arrives byte for byte. On **Windows**, use the
  PowerShell tool's single-quoted here-string (`@'...'@`, closing `'@` at column 0), because the
  Bash tool mangles here-strings there. On every platform, keep the body plain ASCII (straight
  hyphens, no em-dashes, no internal double-quotes) or git may misparse it.
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
