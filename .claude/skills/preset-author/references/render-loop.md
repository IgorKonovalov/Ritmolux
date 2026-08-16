# Render-and-verify with the `shot` CLI

> Confirm any flag against the arg parser in `standalone/examples/shot.rs`; the runnable
> command reference is `docs/capturing.md`. A preset you have not rendered **with audio
> injected** is a guess — this loop is what makes the lane trustworthy.

## What `shot` is

A headless capture **example** in the `standalone` crate. It loads a preset library, renders
without a window, and writes a PNG (or a metrics report). It is an example, not a shipped binary,
so it never bloats `lmv.exe`.

```sh
cargo run -p standalone --example shot -- <flags>
```

## The critical gotcha: a bare still is a DEAD still

The default stimulus is **silence** — every captured frame sees the same silence, so a plain
`--preset X --out y.png` renders the scene frozen at its defaults, reacting to nothing. Supply
audio one of two ways:

1. **`--set` a constant frame** → freezes the scene at a chosen excitation. The right tool for a
   **single still** you can judge:
   ```sh
   cargo run -p standalone --example shot -- --preset-file presets/my_draft.toml \
     --set bass=1,mid=1,treb=1,onset=1,beat=1,bar=0.5 --out loud.png
   ```
   `--set` keys: `bass mid treb onset bar` (f32) and `beat` (non-zero → true). `=1` is already at
   the top of the useful band range. Vary it to probe moments — `beat=0` for the off-beat,
   `onset=1` for a hit, `bass=0.1,mid=0.1,treb=0.05` for the quiet frame.
   **`--set` cannot set `tempo` or `novelty`** — those come from the real DSP, so a preset that
   branches on `tempo` must be judged through `--signal`/`--audio`.

2. **`--signal` / `--audio`** → synthesizes PCM, runs the **real DSP analyzer**, and renders a
   **filmstrip** (frames tiled across time) so you see motion and beat response:
   ```sh
   cargo run -p standalone --example shot -- --preset-file presets/my_draft.toml \
     --signal click:120 --strip 8 --out strip.png
   ```
   `--signal` kinds: `click:<bpm>`, `bass:<hz>`, `treble:<hz>`/`treb:<hz>`, `noise:<seed>`,
   `chord`. `--audio <clip.wav>` drives from a 16-bit PCM WAV (uncompressed only).

**Rule of thumb: `--set` for composition and colour; `--signal` for motion and beat response.**

## Pointing `shot` at your draft — just name the file

There is no `%APPDATA%` copy-over dance (that was true before `LMV_PRESET_DIR` landed). Precedence,
highest first:

1. `--preset-file <path>` — exactly one preset from that file. `--preset` is then unnecessary.
2. `--presets <dir>` — every `*.toml` in that directory.
3. `LMV_PRESET_DIR` — the environment override.
4. The per-user preset directory.
5. The presets compiled into the binary.

The two explicit flags **error** when they come up empty (a named file that doesn't parse exits
non-zero rather than silently capturing some other library); levels 3–5 degrade downward. Every
capture prints a `[source]` label naming the winner, so a PNG's provenance is never a guess.

The live-iteration loop is the same override in a shell:

```powershell
$env:LMV_PRESET_DIR = "./presets"; cargo run -p standalone --release   # app reloads an edit in ~150 ms
```

The app **never seeds** into an override folder — it is yours.

> **`shot` prints load *errors* but not load *warnings*.** An unknown/misspelled param name is a
> warning, so it is invisible here (a known open minor). The running standalone prints warnings on
> every load and hot-reload — if a binding seems to do nothing, run the app against the folder, or
> re-check the name in `presets/README.md`.

## Other modes

| Flag | Effect |
|------|--------|
| `--all --out sheet.png` | **contact sheet** — every preset in the loaded library as a labeled thumbnail grid. The fastest way to compare a draft against the shipped set and to offer the user side-by-side directions. **Honours `--set`** — see the audit pair below. |
| `--report` / `--report --json` | a **metrics table** (reactivity / animation / coverage / near-duplicate), no image. `family=<system>` filters it to one system. See "Reading the report" below. |
| `--frames <N>` | frames advanced before capture (default 120). More frames = later in any `time`-driven animation. |
| `--size <WxH>` | render size (default 1280x720). Render near 1080p when judging the real look — the attractor's detail in particular follows the target size. |
| `--strip <N>` | frames tiled along the audio (default 8). |
| `--out <path>` | output PNG (parent dirs auto-created). For `--all`, a `.png` path is used verbatim; any other path is treated as a dir and the sheet lands at `<out>/contact-sheet.png`. |
| `--horizon <minutes>` | the **long-run drift check** — N *simulated* minutes at capture cadence, one statistics row per interval, no image. Only for worlds that accumulate; see below. |
| `--interval <secs>` | simulated seconds between horizon rows (default 30). |

## The horizon — the only instrument that measures past half a second

Every other mode on this page photographs the first moments of a preset's life. The behavioural
gates capture 30 frames; a live set runs for hours. A world whose mechanism **accumulates** can
drift across that gap invisibly — Plan 0075's Shatter piled onto its flow field's attractors over
minutes and collapsed on stage three times with the whole suite green.

```sh
# Ten simulated minutes of a draft, a row every 30 s
cargo run -p standalone --example shot -- --preset-file presets/my_draft.toml \
  --horizon 10 --size 96x96 --set bass=0.6,mid=0.45

# The same run as JSON, and at a finer interval
cargo run -p standalone --example shot -- --preset-file presets/my_draft.toml \
  --horizon 10 --interval 15 --json
```

**Run it when the mechanism has an accumulation axis** — trails / `fade`, a `[feedback]` table,
particles in a flow field, reaction-diffusion. Anything where this frame's output is next frame's
input. A look with none of those has nothing for the horizon to find, and it costs minutes.

The stimulus is `--set`, **held for the whole run** — that is what makes a row at minute nine
comparable with a row at minute one, and it is why `--horizon` and `--signal`/`--audio` are mutually
exclusive rather than one silently winning.

Each row carries `coverage` (lit fraction), `peak/mean` (concentration — has the population piled
onto a few places?) and `footprint` (motion over the figure's own footprint since the previous row;
the first row prints `-` because it has no predecessor). Under the table, a trend line per
statistic: `delta` is end-to-end travel, `monotone` the share of consecutive steps that went that
way.

**Read it against a static control.** A preset with no `time`, audio or feedback term renders one
frame forever and prints `delta 0.0000, monotone 0.00` on all three. These statistics are
image-domain proxies for a simulation-domain event — particles piling up *reads* as coverage
falling and concentration rising, which is a strong correlation and not an identity — so the flat
control is what makes a sloped subject mean something.

**Nothing here is a threshold and nothing is a gate.** A world grinding into a corner reads a large
`delta` at `monotone` near `1.00`; a world breathing around a stable mean reads `delta` near zero
whatever its monotone. Where the line falls between drifting and alive is a judgement about the
look, so the tool declines to make it and you make it instead. **Record the verdict in the world's
own header** — `presets/attractor_ink.toml` carries the first one and is the shape to copy.

Two live bounds worth knowing before trusting a row:

- A horizon **shorter than the world's own warm-up reads settling as drift**. `reaction_verdigris`
  reads `monotone 1.00` over 30 s purely because its pattern is still establishing.
- **A run that ends short says so**, since Plan 0099: a `--horizon` the `--interval` does not divide
  is floored to the last whole interval and the header states the length actually **reached**, and a
  run that dies prints a `TRUNCATED` block where the table goes. *(This bullet used to say the
  reaction-diffusion worlds die past ~3,600 frames. That was a capture path that never polled,
  repaired 2026-08-16; all three now reach the full ten minutes.)*

Cost: ~3,600 renders per simulated minute — measured at 96x96 on a hardware adapter, a ten-minute
horizon is **16 s** for a single-pass world and **54 s** for a reaction-diffusion one. Slow by
construction, which is exactly why it is a spot-check rather
than a gate (ADR-0099); budget a sitting. Full reference: the "horizon" section of
`docs/capturing.md`.

## Reading the report

`--report` renders the loaded library at fixed stimuli and prints one row per preset, grouped by
family. It builds its own frames (a silent base, one per band, a late frame, a loud frame), so it
ignores `--set` — but it does honour library selection, which matters:

```sh
# The repo working tree, not whichever library wins precedence
cargo run -p standalone --example shot -- --presets presets --report
cargo run -p standalone --example shot -- --presets presets --report family=swarm
cargo run -p standalone --example shot -- --presets presets --report --json > report.json
```

```
=== swarm (5 presets) ===
  preset            bass     mid    treb   onset    anim   cover
  Flow             0.150   0.019   0.000   0.120   0.092   0.944
  NEAR-DUP: Burst ~ Storm
```

| Column | What it is | What a bad value means |
|--------|-----------|------------------------|
| `bass` `mid` `treb` `onset` | how much the frame changes when that input alone is lit | `0.000` — nothing you bound to it reaches the picture: a typo'd param, a gain too small, or a term the clamp swallows. One live column is enough for CI; a *designed* preset usually wants two or three. |
| `anim` | change between an early and a late silent frame | near zero — frozen at rest. It needs a `time` drift somewhere. |
| `cover` | fraction of the loud frame differing from its own corner pixel | near zero — the peak has **no structure against its own background**: dead, flung out of frame, or so blown out that the corner is lit too. Which one it is, only the still will tell you (`craft.md`, the additive ceiling). |
| `NEAR-DUP` | pairwise shape distance under `0.08` | it is not a new look. Change the geometry, not the colour. |

The report is a **suspect finder**, not a verdict: it costs one command for a whole library and it
points you at the two or three files worth rendering. Run it before you start editing, not just
before you ship.

## The audit pair — one library, two excitations

`--all` honours `--set`, so the same library rendered loud and quiet gives two sheets you can flip
between. This is the highest-yield move in the lane when the task is "tune the set" rather than "make
one look":

```sh
cargo run -p standalone --example shot -- --presets presets --all \
  --set bass=1,mid=1,treb=1,onset=1,beat=1,bar=0.5 --out audit/loud.png
cargo run -p standalone --example shot -- --presets presets --all \
  --set bass=0.1,mid=0.1,treb=0.05 --out audit/quiet.png
```

What the pair exposes that neither sheet alone does:

- **Inversions** — any thumbnail with *less* legible structure loud than quiet is over the additive
  ceiling (blown white) or has thrown its geometry out of frame. Loud must read as more.
- **Dead presets** — identical loud and quiet thumbnails means nothing is reaching the picture.
- **Family sameness** — a grid makes near-duplicates and one-note families obvious in a way a
  sequence of single stills never does, and it confirms what `--report` flagged as `NEAR-DUP`.
- **Neighbour clashes** — the sheet is laid out in the same filename order the engine's rotation
  walks, so adjacent cells are the presets that will dissolve into each other.

Pair it with `--report`: the table gives the numbers, the sheets say which failure produced them.

## The loop in practice

**Fixing or tuning an existing set** — diagnose before you edit:

1. `--report` over the working tree → which presets are dead, blown, frozen or duplicated?
2. The audit pair of contact sheets → which of those failures each one actually is, and does anything
   invert from quiet to loud?
3. Only now open the two or three files that the numbers and the sheets agree about.

**Authoring one new look** — render every step:

1. Write the draft (repo `presets/` or your own folder).
2. `--set` loud still → composition right? colour cohesive? reacting at all?
3. `--set` quiet still → does it still look intentional at rest, or collapse to nothing?
4. Loud vs quiet → more structure at peak, not less (the inversion check).
5. `--signal click:120` filmstrip → does it move musically, or strobe?
6. Tune, re-render, repeat.
7. `--report` before you'd ship it → bands live, `anim` alive, `cover` sane, no `NEAR-DUP`.
8. To offer directions: render 2–3 variants (or `--all`) and show the stills side by side — this
   project decides by looking, not by prose. Save the stills somewhere the user can flip through.
