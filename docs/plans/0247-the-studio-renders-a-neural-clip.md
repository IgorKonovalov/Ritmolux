# 0247 — The studio renders a neural clip

> **Status:** in-progress (2026-10-02)
> **Created:** 2026-10-01
> **Owner skill(s):** dev, studio-builder, human
> **Related ADRs:** [0262](../adrs/0262-the-studio-renders-a-clip-by-piping-three-children-and-transcodes-what-the-player-cannot-read.md)
> (proposed; this plan's decision), [0175](../adrs/0175-the-studio-is-a-separate-application-that-never-draws-a-frame.md)
> (the `render` subcommand it promised), [0236](../adrs/0236-a-diffused-render-varies-by-prompt-on-bar-boundaries-and-the-seed-stays-fixed.md)
> (the prompt timeline it drives), [0240](../adrs/0240-a-setting-lives-in-a-file-and-the-menu-edits-that-file.md)
> (every new choice has a `settings.json` key)

## TL;DR

The studio gains a **Render** view. You pick an MP3, FLAC or WAV and any preset from the library. A
strip appears showing the track's waveform with the player's bars over it. You drop one or more prompts
onto bars, switch **neural** on or off, and press Start. The studio transcodes the track with `ffmpeg`
and runs `ritmolux --render | sd_filter.py | ffmpeg` as three piped children. It shows progress and an
ETA, and keeps the machine awake until an MP4 lands beside a job file that can reopen the same render.
The player gains `--render` and `--bars`, promoted from `shot`; nothing in `core/` changes.

## Context & problem

The owner's ask, 2026-10-01: *"start render of neural clip from studio. I will be able to point to mp3
or flac song, choose preset and add some prompt, or several."*

Today that is a hand-composed three-stage shell pipe from a source checkout (`docs/diffusion-filter.md`
"The one command"), with a WAV-only renderer and a timeline JSON written by hand against a bar-grid file
nobody can see before the render. The plans index has carried *"clip rendering … and the diffusion pass
from the studio are each a later plan with its own interview"* since Plan 0159 closed. This is that plan.

The interview settled five things: the studio transcodes with ffmpeg rather than the player decoding;
prompts are placed on a waveform-and-bar strip; one panel does plain and neural renders; any library
preset can be chosen; and the studio pipes three children itself, a render dying with the studio being
the accepted price.

## Decision

The four forks are argued in ADR-0262. In one sentence: the player's two new modes come from `shot`
code, the studio wires child-to-child OS pipes and reads progress from `ffmpeg -progress`, and MP3/FLAC
reach the WAV-only reader through an ffmpeg transcode the studio caches for the session. We rejected a
decoder crate in the player (ffmpeg is already mandatory for the output), a detached job (too much
machinery for a first version; it is the followup), the player spawning the chain (the shipped binary
would learn about Python), and a studio-side beat tracker (prompts would land on bars the render does
not have).

## Architecture diagram

```mermaid
flowchart LR
    subgraph studio["studio/ (Electron main)"]
        T[transcode<br/>ffmpeg -> WAV cache]
        J[job runner]
        V[Render view<br/>strip + prompts]
    end
    subgraph player["ritmolux (shipped player)"]
        B["--bars"]
        R["--render"]
    end
    subgraph ext["user-installed"]
        S[sd_filter.py<br/>optional]
        F[ffmpeg encode]
    end
    SRC[(mp3 / flac / wav)] --> T --> B -->|bars.json| V
    V -->|job| J
    J -.spawns.-> R
    R -->|Y4M pipe| S -->|Y4M pipe| F
    R -.neural off.-> F
    F -->|-progress| J
    F --> MP4[(clip.mp4)]
```

## Implementation phases

### Phase 1 — The player renders and reports bars
- **Owner skill:** dev
- **What:** `ritmolux` gains two answer-and-exit modes, dispatched before the launch path the way
  `--thumb` is. `--render <wav> --preset <name> [--fps] [--size] [--tier]` writes Y4M to stdout through
  `standalone::shot::render`'s own walk. `--bars <wav> [--fps] --out <path>` writes
  `shot::render::bar_grid`'s JSON. Presets resolve against the player's library (embedded set plus the
  user preset directory, as `thumbs::library()` resolves it). Exit 2 means the arguments are wrong in
  shape, including a preset no library holds. Exit 1 means a recognised request failed. `--ffmpeg`
  stays `shot`-only.
- **Files touched:** `standalone/src/cli.rs`, `standalone/src/main.rs`, a new `standalone/src/render_mode.rs`
  (or the name the existing modes suggest), `standalone/src/shot/render.rs` (only to expose what the mode
  calls), tests under `standalone/tests/` or beside the module, `docs/configuration.md`, `docs/capturing.md`.
- **Done when:**
  - A test renders a short WAV it synthesizes (the repo commits none) through `ritmolux --render` and
    through `shot`'s render entry point with the same preset, fps, size and tier, and asserts the two
    streams are **byte-identical**.
  - A test asserts `--bars` writes the same JSON as `shot --render --bar-grid` for the same WAV and fps.
  - `--bars` constructs no `Renderer` and requests no adapter. This is a structural property: the mode's
    code path holds no GPU type. The log states how it was checked.
  - An unknown preset exits 2, a non-PCM WAV exits 1, and each prints one line naming what was wrong.
  - The release binary's size before and after this phase is recorded in the log as two measured
    numbers, and NFR §4's cap still holds.
  - `docs/configuration.md` lists both flags. `docs/capturing.md` says in a short paragraph that the
    player's `--render` is the shipped twin of `shot --render`, and carries no copy of `shot`'s flag
    table.

### Phase 2 — A plain clip from the studio (walking skeleton)
- **Owner skill:** studio-builder
- **What:** A **Render** view beside Library / Editor / Settings with: a file picker for an audio file,
  a picker over the whole library, fps (default 30), size (default 1920x1080), tier (default `rich`), an
  output path, and Start. The main process transcodes the source with `ffmpeg -i <src> -c:a pcm_s16le
  -ac 2 <cache>/<key>.wav`, keeping the sample rate, into `userData/render-cache/`. The key covers the
  source path, size and mtime, and the cache is emptied when the studio quits. It then runs `--bars`
  to learn `frames`, and starts `ritmolux --render … | ffmpeg <canonical encode> -progress pipe:3`, with
  the WAV as the encoder's audio input. The player's stdout is passed as the encoder's stdin stream, so
  frames never reach JavaScript. The view shows a progress bar and ETA from `frame=` against `frames`.
  **Cancel** kills both children and deletes the partial output. While a job runs,
  `powerSaveBlocker.start('prevent-app-suspension')` is held and quitting asks for confirmation. On
  failure the last stderr lines of each stage go to `<output>.render.log` and appear in the view.
  New `settings.json` keys: `render.ffmpegPath` (absent means `ffmpeg` on `PATH`) and
  `render.outputDir` (absent means the user's Videos directory). The Settings view edits both.
- **Files touched:** `studio/electron/render/` (new: transcode, pipeline, progress parser), `studio/electron/settings.ts`,
  `studio/electron/ipc/`, `studio/electron/preload/`, `studio/shared/ipc-channels.ts`, `studio/renderer/views/Render.tsx` (new),
  `studio/renderer/App.tsx`, `studio/renderer/views/Settings.tsx`, tests beside each.
- **Done when:**
  - A vitest test parses a recorded `ffmpeg -progress` transcript into a monotone frame count, and
    reports done only on `progress=end`.
  - A test with stub children asserts that Cancel kills every child and removes the output file, and
    that a non-zero exit from any stage fails the job and names that stage.
  - A test asserts the encoder is spawned with the player's stdout as its stdin stream, not with a
    `'pipe'` that JavaScript forwards.
  - The settings test reads and writes both new keys, and `node scripts/check-settings-have-files.mjs`
    exits 0.
  - The by-hand duration check moved to Phase 6 on 2026-10-02: a headless session cannot drive a
    built studio, and `ffprobe` is not on its allowlist.

### Phase 3 — The strip and the prompts
- **Owner skill:** studio-builder
- **What:** Once a track is chosen, the view draws its **waveform**, using min/max peaks per pixel
  column computed in the main process from the cached WAV, with the **bars** from `--bars` over it.
  Locked bars (`bar_locked`) are drawn distinctly from fallback bars, and the grid's summary line
  (*N bars, M on a downbeat*) appears under the strip. Clicking a bar adds a prompt marker there.
  Markers can be dragged and snap to bar starts, edited in a text field, and deleted. Between two
  markers the strip shades the blend, because ADR-0236's conditioning interpolates across the whole
  span rather than switching at the marker. The first marker is at bar 1 by default. Changing fps
  re-runs `--bars`, and markers keep their **bar numbers**. The view holds the timeline as
  `[{at_bar, prompt}]`, ascending and unique, which is exactly what `sd_filter.py --timeline` accepts.
- **Files touched:** `studio/electron/render/peaks.ts` (new), `studio/renderer/components/BarStrip.tsx` (new),
  `studio/renderer/views/Render.tsx`, `studio/shared/` (the timeline type and its validator), tests beside each.
- **Done when:**
  - The peaks function, fed a synthetic WAV (a 1 s silence then a 1 s full-scale square wave), returns
    zero columns for the first half and ±1 for the second within one column of the boundary.
  - The timeline validator refuses what `sd_filter.py`'s `parse_timeline` refuses: an empty list, a bar
    below 1, a non-ascending or duplicate bar, an empty prompt, and a bar past the grid's last.
    A table-driven test covers each case.
  - A component test drags a marker between bars and asserts it lands on a bar start and keeps its
    prompt text.
  - A component test asserts locked and fallback bars render with different classes.

### Phase 4 — The neural toggle
- **Owner skill:** studio-builder
- **What:** A **neural** switch adds the sidecar to the pipeline as `<python> <script> --profile
  <quality|fast> --timeline <out>.timeline.json --bar-grid <out>.bars.json [--negative] [--seed]`.
  The studio writes both files beside the output before spawning. `--bars` writes the grid file
  directly, so the sidecar reads the same bars the strip showed. New `settings.json` keys:
  `render.diffusion.python` (an interpreter path) and `render.diffusion.script` (a path to
  `tools/sd-filter/sd_filter.py` in a checkout). Both are absent by default. The **readiness probe** runs when the Render view opens:
  it checks the script exists, then runs `<python> -c "import torch; print(torch.cuda.is_available())"`.
  The switch stays disabled with one line naming the key or the failure, for example *"torch reports no
  CUDA - see docs/diffusion-filter.md"*, until the probe passes. The result is cached for the session,
  with a re-check button. Start refuses a neural job with no prompt before spawning anything. The
  progress line also shows the sidecar's own per-frame figure, read from its stderr, beside the
  encoder's count.
- **Files touched:** `studio/electron/render/` (probe, pipeline), `studio/electron/settings.ts`, `studio/renderer/views/Render.tsx`,
  `studio/renderer/views/Settings.tsx`, tests beside each.
- **Done when:**
  - An integration test runs the real three-stage pipeline with `sd_filter.py --passthrough`, a built
    player and `ffmpeg` on a two-second WAV, and asserts the MP4's frame count equals `--bars`'
    `frames`. It skips with a printed notice when any of the three is missing (ADR-0016's shape).
  - A test asserts the probe's three outcomes (no script, no CUDA, ready) produce three distinct
    disabled or enabled states, each carrying its own message.
  - The timeline and bars files written beside the output parse with `sd_filter.py`'s own loader.
    The passthrough run above proves this, because it passes `--timeline`.
  - The two new keys round-trip in the settings test, and `node scripts/check-settings-have-files.mjs`
    exits 0.

### Phase 5 — The job is a file, and the docs say so
- **Owner skill:** studio-builder
- **What:** Start writes `<output>.render.json` (shape below) beside the MP4, and an **Open job**
  action restores the whole view from one: track, preset, fps, size, tier, neural settings and prompts.
  If the source has moved, it asks for it again and keeps the prompts. Docs: `studio/README.md` gains a
  *Rendering a clip* section. `docs/configuration.md` gains the four `render.*` keys.
  `docs/diffusion-filter.md` gains a short *From the studio* section that points at the studio and
  carries **no figure**, because the figures gate holds them to one page. `docs/capturing.md` gets a
  one-line pointer.
- **Files touched:** `studio/electron/render/job.ts` (new), `studio/renderer/views/Render.tsx`, `studio/README.md`,
  `docs/configuration.md`, `docs/diffusion-filter.md`, `docs/capturing.md`.
- **Done when:**
  - A test round-trips a job document through write and open and asserts equality, and asserts a
    document with an unknown `version` is refused with a message.
  - `node scripts/check-filter-figures.mjs`, `node scripts/check-doc-links.mjs` and
    `node scripts/check-reader-prose.mjs` each exit 0.

### Phase 6 — A real neural clip, judged
- **Owner skill:** human
- **Blocks merge:** no
- **What:** On the CUDA machine, from a **packaged** studio (`resources/player/` carrying the new
  player) with `render.diffusion.*` pointed at a checkout's venv, render one MP3 and one FLAC with at
  least three prompts each, one at `fast` and one at `quality`. Watch both.
- **Done when:** The owner records in the log that both files play with audio in sync, that the
  prompt changes land near the bars they were placed on, or that they do not and why, and whether a
  studio quit mid-render asked first. Before the neural renders, a plain clip of each of the two files
  (neural off) has an MP4 duration that `ffprobe` reports within one frame of the source's, and the
  log names the two files.

## Data shapes

```jsonc
// illustrative — <output>.render.json, written by the studio, read only by the studio
{
  "version": 1,
  "audio": "/home/me/Music/track.flac",
  "preset": "Supernova",
  "fps": "30",
  "size": "1920x1080",
  "tier": "rich",
  "output": "/home/me/Videos/track-supernova.mp4",
  "neural": {                      // null when the switch is off
    "profile": "fast",
    "negative": null,              // null = the sidecar's default
    "seed": null,
    "timeline": [{ "at_bar": 1, "prompt": "a vast canyon of luminous rock" },
                 { "at_bar": 33, "prompt": "a cathedral rose window in stained glass" }]
  }
}
```

Beside it at render time: `<output>.bars.json` (the player's `--bars`), `<output>.timeline.json`
(`neural.timeline` verbatim), and `<output>.render.log` (only on failure).

## Risks & open questions

- **The binary size cost of `--render` is unmeasured.** `standalone::shot` is in the lib, but the
  player does not link the render walk today. If Phase 1's measurement breaks NFR §4's cap, stop and
  return to architect. Gating the modes behind a cargo feature would change what the packaged studio
  carries, and that is a decision, not a fix.
- **A crash loses a night.** Accepted in ADR-0262. If it happens even once, ADR-0262's Alternative B
  (detached job) is the followup, and the job file from Phase 5 is already the launcher's input.
- **Prompt placement trusts a mostly-fallback grid.** A marker on "bar 33" may not sit on a musical
  phrase boundary. The strip makes that visible, and fixing the estimator is backlog 0042's question,
  not this plan's.
- **Is the CUDA machine the Arch box or the Windows laptop?** The sidecar was measured on Windows.
  Phase 6 runs wherever CUDA is. The Linux venv recipe is already in `tools/sd-filter/README.md`.
- **Electron child stdio across platforms.** Passing one child's `stdout` stream as another's `stdin` is
  supported by Node on all three platforms, but has not been exercised in this repo. Phase 2's test
  pins the wiring, and Phase 4's integration test proves it end to end on CI's Linux runner.

## What this plan does NOT do

- **No preset changes across a track** and **no onset-driven denoise.** ADR-0236 declined both, and
  this plan inherits that.
- **No detached or resumable renders.** No render queue: one job at a time.
- **No preview of diffused frames during the run.** Progress is numbers. The MP4 is the picture.
- **No shipped Python.** The diffusion stage still needs a user-built venv and a checkout's script.
  The studio says so; it does not install anything.
- **No player decoding of compressed audio**, and no change under `core/`.

## Implementation log

**Lane:** branch `plan-0247-the-studio-renders-a-neural-clip`, worktree `/home/igor/Work/rlx-plan-0247`

| phase | owner | state | commit |
|---|---|---|---|
| 1 — The player renders and reports bars | dev | done | committed with this row |
| 2 — A plain clip from the studio | studio-builder | done | committed with this row |
| 3 — The strip and the prompts | studio-builder | not started | |
| 4 — The neural toggle | studio-builder | not started | |
| 5 — The job is a file, and the docs say so | studio-builder | not started | |
| 6 — A real neural clip, judged | human | not started | |

### Notes

- Phase 1, outside the file list: `standalone/src/thumbs.rs` (`library()` made `pub(crate)` so
  `--render` resolves through it) and `standalone/tests/suite/main.rs` (registers the new
  `render_cli` module).
- Phase 1, `cli.rs`: `FlagSpec::requires` became a list read as "any one of", so `--size` reads
  `[requires --stream or --render]` and `--fps` adds `--bars`. A third new flag, `--out`
  (`[requires --bars]`), carries `--bars`' path. `run.rs` is unchanged.
- Phase 1, defaults the plan left open: with no flag, `--render` takes `shot`'s 60 fps, 1280x720
  and tier `floor`, and `--preset` is required.
- Phase 1, the structural `--bars` check is a source-reading unit test,
  `render_mode::tests::the_bars_mode_holds_no_gpu_type`. It asserts that the bodies of `fn bars`
  and `fn read_clip` name none of `Renderer`, `renderer`, `render::run`, `wgpu`, `Tier` or
  `adapter`, and that `fn render_clip` names `render::run`.
- Phase 1, binary size: `target/release/ritmolux` from `cargo build --release -p standalone --bin
  ritmolux`, Linux. 12,854,504 B at `f77a476a`, before the phase. 12,928,352 B after it, which is
  77.1 % of NFR §4's 16,777,216 B cap.
- Phase 2, outside the file list: `studio/shared/render.ts` (the request, grid and event shapes,
  in `shared/` because the renderer's project type-checks the preload's imports and cannot see
  Node), `studio/electron/render/service.ts` (the rules, kept free of Electron so they test
  plainly), `studio/electron/render/commands.ts`, `studio/electron/ipc/appHandlers.ts` and
  `preload/api/app.ts` (`AppInfo` carries `render`), the two new and two touched CSS modules,
  `studio/renderer/App.test.tsx` (its `window.api` stub gains `render`), and `studio/README.md`:
  `settings.doc.test.ts` holds the settings table to `StudioSettings`, so the `render` row lands
  with the key.
- Phase 2, the IPC surface: eight OS channels under `render:` (pick-audio, prepare,
  suggest-output, pick-output, start, cancel, event, set-settings). No domain channel was added.
  A path the renderer sends back is accepted only when main produced it, a dialog's answer or the
  output it suggested, which is the preset channels' guard applied to a second pair of paths.
- Phase 2, the encode: `encoderArgs` is `shot::render::ffmpeg_args` at CRF 18 plus
  `-progress pipe:3`. `commands.test.ts` reads the Rust function's string literals and holds the
  TypeScript list to them in order.
- Phase 2, verdicts the plan left open. The first stage to exit badly is named, and the others
  are stopped. An upstream stage that dies after the encoder finished cleanly is not blamed,
  because `-shortest` can close the encoder's stdin a frame early. An encoder that exits 0
  without `progress=end` fails. A failed job removes its partial MP4 as well as writing the log,
  because a file cut off before its index does not play. The transcode cache is emptied at
  start as well as at quit, so a crash leaves nothing behind for a later session.
- Phase 2, wiring checked by hand before it was written: Node's `stdio: [child.stdout, ...]`
  with the parent's copy destroyed moved 50,000,000 bytes from `head` to `wc -c` intact. With
  the reader exiting early, the writer saw `SIGPIPE` rather than blocking.

### Close triggers

## Followups (after this lands)

- ADR-0262 Alternative B, the detached job, if a render is ever lost to a studio exit.
