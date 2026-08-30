# 0139 — The render path validates before it spends

> **Status:** approved
> **Created:** 2026-08-29
> **Owner skill(s):** dev, human
> **Related ADRs:** none — both engine entries state *"No ADR needed"*. Phase 4 gathers the evidence
> an ADR on backlog 0125 would need.
> **Closes:** design-backlog 0111, 0112. **0125 is carried, not closed** — see Phase 4.

## TL;DR

`shot --render` spawns the encoder and builds a GPU device **before** it checks that `--preset`
names anything, so a typo exits 1 and leaves a **262-byte playable MP4** at the destination — a real
file a glance cannot tell from a short render. The same path's one canonical `ffmpeg` invocation is
archival-grade with no lever: a 4:41 track at 1080p60 rich measured **3.73 GB at 106 Mbit/s**, about
9x a typical upload recommendation. The first visible behavior is a misspelt `--preset` failing
instantly, naming the roster's keys, and writing no file.

## Context & problem

Both entries were raised from Plan 0101 Phase 5's real render, and both are about the same seam:
**the convenience path spends before it validates, and offers no lever once it is spending.**

Backlog 0111's failure is worse than untidiness. ADR-0114's rule for this path is that a missing
encoder is a named error and *never* a silent fallback, because a quietly-substituted encoder makes
an exported file untrustworthy. This is that hazard one step over — nothing was substituted, but the
artifact left at the destination is a valid, playable MP4 containing only the muxed audio, because
`ffmpeg` had already been spawned and exited 0. Reproduced with `--preset attractor_leviathan`, the
*filename*, where the roster key is the preset's `name` field `"Leviathan"` — which is also the
confusion an error naming the roster's keys would have caught.

Backlog 0112 is a convenience complaint with a real justification behind it: nothing is missing from
the *capability* — omit `--ffmpeg`, redirect stdout, run your own encoder, which `capturing.md`
documents — but the convenience path's whole stated reason to exist is that there is exactly one
command line to fix rather than a wiki of incantations. Today adjusting it means editing
`ffmpeg_args`. The entry also flags that it **may be discharged by backlog 0110 rather than on its
own**, since most of those bits are encoding shot noise that Plan 0128's density work should remove.

This plan exists in front of Plan 0103's outreach phases, which need demo material out of this path.

## Decision

**Fix the two cheap defects, and treat backlog 0125 as an evidence problem rather than a design
one.** Phases 1-2 are the validation and the `--crf` lever, both small and both explicitly ADR-free
by their own entries. Phase 3 is the doc sweep. Phase 4 is a `human` look gate rendering the *same
clip* at both diffusion profiles, because backlog 0125's own text says the cheap first move is a
side-by-side still at both budgets, **not a design** — the clip that drew the *"resolution would be
higher"* verdict was rendered at `fast` (680x384), and `quality` is 2.25x the pixels and **has never
been rendered on a real track**.

We rejected folding backlog 0126 into this plan, because that entry says so in as many words: *"Do
not fold this into a resolution plan. It shares a verdict with backlog 0125 and nothing else — one
is a pixel budget against a VRAM wall, the other is a timeline the pipeline does not have."*

## Architecture diagram

```mermaid
flowchart TB
    ARGS["shot --render --preset X --out Y"] --> VAL{"is X in the roster?<br/>NEW — Phase 1"}
    VAL -->|no| ERR["exit 1, name the roster's keys<br/>NO process, NO device, NO file"]
    VAL -->|yes| SPAWN["Encoder::spawn"]
    SPAWN --> DEV["build GPU device"]
    DEV --> CAP["Renderer::capture_stream"]
    CAP -->|"raw frames on stdout"| FF["ffmpeg — -crf now a flag (Phase 2)"]
    FF --> OUT["the .mp4"]
    subgraph old["today, and the reason for Phase 1"]
        SPAWN -.->|"name checked only here,<br/>after both costs"| CAP
        SPAWN -.->|"leaves a valid 262-byte<br/>audio-only MP4"| OUT
    end
```

## Implementation phases

### Phase 1 — Validate the preset name before spending anything
- **Owner skill:** dev
- **What:** Close backlog 0111. Check `name` against `presets` in `render::run()` before
  `Encoder::spawn`.
- **Files touched:** `standalone/src/shot/render.rs`.
- **Notes for the implementer:**
  - **The roster is already in hand** — the `(None, [only])` arm reads it two lines up. This is a
    membership test, not new plumbing.
  - **The error must name the roster's keys.** The reproduction is `--preset attractor_leviathan`
    (the filename) against a roster keyed on the `name` field (`"Leviathan"`), and an error listing
    the keys is what turns that from a puzzle into a typo.
  - Verify **no file is left at `--out`** on the failure path. That is the actual defect — a 262-byte
    playable MP4 that a glance cannot distinguish from a short render.
- **Done when:**
  - `shot --render --preset <not-a-preset> --out <path>` exits 1 naming the roster's keys, spawns no
    child process, builds no GPU device, and leaves **nothing** at `<path>`.
  - Passing a preset's *filename* rather than its `name` produces an error a reader can act on.

### Phase 2 — A size lever on the convenience path
- **Owner skill:** dev
- **What:** Close backlog 0112. Add a `--crf <n>` passthrough to the generated `ffmpeg` command.
- **Files touched:** `standalone/examples/shot.rs`, `standalone/src/shot/render.rs`,
  `docs/capturing.md`.
- **Notes for the implementer:**
  - The measured anchors, on a 30 s slice of `attractor_leviathan` at 1080p60 rich: shipped `-crf 18`
    is 119 Mbit/s, `-crf 23` is 60, `-crf 28` is 27. A typical 1080p60 upload recommendation is
    ~12 Mbit/s, so the shipped default is about **9x** it. Keep `18` as the default — it is the
    archival choice and it is deliberate; this adds a lever, it does not move the default.
  - **The existing tests pin the load-bearing arguments** so the colour tags cannot be lost. Extend
    them rather than replacing — a `--crf` that silently drops a colour tag is a worse defect than
    the one being fixed.
  - If this lands alongside Plan 0128, **re-measure**: backlog 0112 notes most of these bits are
    encoding shot noise that the density work should remove, so the numbers above may not survive it.
- **Done when:** `--crf 23` produces a materially smaller file than the default on the same input,
  with the colour tags intact, and `docs/capturing.md` documents the flag **and** names the raw-stream
  path as the other size-control route.

### Phase 3 — The capture doc says what the levers are
- **Owner skill:** dev
- **What:** Sweep `docs/capturing.md` for the two changes.
- **Files touched:** `docs/capturing.md`.
- **Notes for the implementer:**
  - This file is 144 KB behind six headings. **Put the new material where a reader looking for
    `--render` flags will find it**, not at the end.
  - State the failure behaviour from Phase 1 explicitly — that a bad `--preset` now costs nothing and
    writes nothing — because the old behaviour left artifacts people may have on disk.
- **Done when:** `--crf` and the validation behaviour are both documented in the `--render` section.

### Phase 4 — The resolution look gate (evidence only, no design)
- **Owner skill:** human
- **What:** Render the same clip through the diffusion filter at both profiles and record the
  verdict against backlog 0125.
- **Files touched:** `docs/design-backlog.md` (a dated update on entry 0125).
- **Notes for the implementer:**
  - **This phase designs nothing and changes no code.** Backlog 0125's own text: the clip that drew
    the *"it would obviously be great if resolution would be higher"* verdict was rendered at
    **`fast`** — a 262,144 px budget, 680x384 at 16:9, resampled to 1920x1080. **`quality` is
    1024x576, 2.25x the pixels, and has never been rendered on a real track.** So an unknown and
    possibly large share of the complaint is a profile choice rather than a wall.
  - **This is expensive and it needs a free machine.** Phase 2b measured 2.721 s/frame at the
    `quality` budget; a 4-minute track measures ~5.9 h before Plan 0106 Phase 7d's 1.406x scope
    correction. **Do not start it during a show.** A shorter representative slice is a legitimate
    substitute and should be stated as such.
  - What to record: whether `quality` alone answers the ask, and if not, by how much it falls short.
  - **The walls, so nobody reads a verdict as a mandate:** SD1.5 duplicates or mirrors content above
    roughly 768²; SDXL plus ControlNet is ~7.5 GB against an 8 GB card with the spike already at
    5.68 GB; cost scales with pixels.
- **Done when:** backlog 0125 carries a dated update stating what `quality` looks like on real
  material and whether the ask survives it.

## Risks & open questions

- **Phase 4 may not resolve backlog 0125 at all**, and that is an acceptable outcome. If `quality`
  does not answer the ask, the next step is an ADR reopening
  [ADR-0121](../adrs/0121-the-diffusion-filter-is-an-offline-stage-with-profiles-and-it-interpolates-its-own-stride.md)'s
  Alternative C — *diffuse at a smaller budget and upscale*, which **this same user rejected in the
  design interview** on the ground that generated detail is worth its price against inferred detail.
  That rejection was made before anyone had watched five minutes of output, which is what makes it
  reopenable rather than settled. A tiled or multi-pass approach that *generates* at higher
  resolution is the option neither the ADR nor Plan 0106 has costed.
- **Phase 2's measurements may be obsoleted by Plan 0128.** Backlog 0112 predicts it may be
  discharged by 0110 rather than on its own. If 0128 lands first, re-measure before claiming a
  default is right.
- **Phase 1 is small enough to look trivial and is the whole reason for the plan.** Resist merging it
  into a larger refactor of `render::run()`.

## What this plan does NOT do

- **It does not take backlog 0126** (nothing varies across a track). That entry is squarely inside
  Plan 0106's stated non-scope, its fixed seed is load-bearing — a per-frame seed *"guarantees
  boiling whatever else is tuned"* — and its denoise-from-onset lever reopens Plan 0106's
  no-audio-conditioning decision, which wants its own ADR and interview. It is the next plan in this
  area, not a phase of this one.
- **It does not change the default `-crf`.** Archival-grade is the deliberate default; this adds a
  lever beside it.
- **It does not touch the diffusion filter's profiles.** Phase 4 renders what already exists.
