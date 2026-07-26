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
| `--all --out sheet.png` | **contact sheet** — every preset in the loaded library as a labeled thumbnail grid. The fastest way to compare a draft against the shipped set and to offer the user side-by-side directions. |
| `--report` / `--report --json` | a **metrics table** (reactivity / animation / coverage / near-duplicate), no image. `family=<system>` filters it to one system. Use it to check a preset actually reacts and isn't a near-dup. |
| `--frames <N>` | frames advanced before capture (default 120). More frames = later in any `time`-driven animation. |
| `--size <WxH>` | render size (default 1280x720). Render near 1080p when judging the real look — the attractor's detail in particular follows the target size. |
| `--strip <N>` | frames tiled along the audio (default 8). |
| `--out <path>` | output PNG (parent dirs auto-created). For `--all`, a `.png` path is used verbatim; any other path is treated as a dir and the sheet lands at `<out>/contact-sheet.png`. |

## The loop in practice

1. Write the draft (repo `presets/` or your own folder).
2. `--set` loud still → composition right? colour cohesive? reacting at all?
3. `--set` quiet still → does it still look intentional at rest, or collapse to nothing?
4. `--signal click:120` filmstrip → does it move musically, or strobe?
5. Tune, re-render, repeat.
6. To offer directions: render 2–3 variants (or `--all`) and show the stills side by side — this
   project decides by looking, not by prose. Save the stills somewhere the user can flip through.
