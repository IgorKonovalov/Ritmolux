# light-music-visualizer

A lightweight, real-time music visualizer built around one **shared Rust core** that turns a
stream of PCM audio samples into GPU-rendered visuals. Two frontends consume that core:

- **Standalone app** (Windows + macOS) — pure Rust (`winit` + `wgpu`), fed by OS loopback
  audio capture.
- **foobar2000 plugin** (Windows-first) — a thin **C++ shim** over the core's **C ABI**, fed by
  foobar's own `visualisation_stream` (no loopback needed on that path).

The core is **source-agnostic**: it takes interleaved/mono PCM frames and does not care whether
they came from loopback capture or foobar. That single abstraction is what lets one visual
codebase serve both frontends.

> **Status: pre-1.0, in active development.** Both frontends run: the standalone app renders
> live WASAPI loopback on Windows, and the foobar2000 component links the core's C ABI. The
> preset format and the C ABI may still change between releases — stability begins at 1.0.0.
> See [`docs/plans/`](docs/plans/) for what's in flight.

## Architecture

```mermaid
flowchart TD
    subgraph external["Audio sources (external)"]
        loop["OS loopback capture<br/>(WASAPI / ScreenCaptureKit)"]
        fb["foobar2000<br/>visualisation_stream"]
    end

    subgraph shells["Frontends"]
        standalone["Standalone shell<br/>Rust: winit + wgpu surface"]
        plugin["foobar plugin<br/>C++ shim over the C ABI"]
    end

    subgraph core["core/ — shared Rust brain (source-agnostic, GPU-abstract)"]
        ring["Lock-free ring buffer<br/>(SPSC seam)"]
        dsp["DSP<br/>FFT / spectrum · beat / onset"]
        scene["Scene graph"]
        render["wgpu render engine"]
        ring --> dsp --> scene --> render
    end

    loop --> standalone
    fb --> plugin
    standalone -->|"push PCM frames"| ring
    plugin -->|"push PCM frames (C ABI)"| ring
    render -->|Metal| macos["macOS"]
    render -->|"DX12 / Vulkan"| windows["Windows"]
```

The seam between audio and render is the **lock-free ring buffer**: audio arrives at the
device's cadence, frames render at the display's, and neither loop drives the other directly.

## Repository layout

```
core/                # Rust library crate — the shared brain: DSP + render engine + scenes.
                     #   Native Rust API (standalone) + C ABI (foobar plugin). No audio-source code.
lmv-ring/            # The lock-free SPSC ring, split out zero-dependency so Miri can check it in CI.
standalone/          # Rust binary + lib — winit window, wgpu surface, loopback capture, the shot example.
plugin-foobar/       # C++ shim: foobar2000 SDK integration, links the core's C ABI. Windows-first.
presets/             # The curated preset library (*.toml) — embedded at build time, seeded on first run.
packaging/           # What a `v*` tag ships: macos/bundle.sh (build, lipo, sign, zip, verify) and the
                     #   READ-ME-FIRST.md testers get in each zip. See ADR-0038.
docs/
├── nfr.md           # Quantified v1 non-functional requirements (the numbers behind "lightweight").
├── presets.md       # Preset authoring guide: the expression language, loading, and where files live.
├── preset-palettes.md  # The colour surface: built-in palettes, custom stops, the A/B crossfade.
├── capturing.md     # Headless capture + visual-QA harness: the shot CLI and the core/tests/ checks.
├── releasing.md     # The version-bump / release procedure (one bump per plan close).
├── on-device-validation.md  # The manual checklist for what CI cannot run: real GPUs, live loopback.
├── design-backlog.md  # Captured friction not yet promoted to an ADR or a plan (…-archive.md holds retired entries).
├── roadmap-visual-richness.md  # The visual-capability roadmap the recent plans are sequenced against.
├── generative-techniques-catalogue.md  # The technique survey behind the scene families.
├── adrs/            # Architecture Decision Records + rejected alternatives. Append-only.
├── specs/           # Living behavioral contracts per core subsystem (C ABI, ring/DSP determinism).
└── plans/           # Phased implementation plans (what's in flight); done/ holds completed plans.
```

The per-system parameter tables live in [`presets/README.md`](presets/README.md), beside the
preset files they document.

## Download

Prebuilt standalone binaries are attached to each tag on the
[Releases page](https://github.com/IgorKonovalov/light-music-visualizer/releases). Two zips per
release, each carrying the app, a reference copy of the presets, and a `READ-ME-FIRST.txt`:

| Zip | What's in it |
|-----|--------------|
| `…-macos-universal.zip` | `LightMusicVisualizer.app` — universal (Apple Silicon + Intel), **macOS 13+** |
| `…-windows-x64.zip` | `lmv.exe` — Windows x64 |

Both are **unsigned**, so each OS objects once. On Windows, SmartScreen says "Windows protected
your PC" → More info → Run anyway. On macOS, the app is ad-hoc signed only, so either right-click
it and choose **Open**, or strip the quarantine attribute first:

```sh
xattr -dr com.apple.quarantine LightMusicVisualizer.app
```

The macOS build then asks for the **Screen Recording** permission — that is the only first-party
way to tap system audio — and needs a **relaunch** after you grant it. Releases are marked
prerelease while the app is `0.x`. The `READ-ME-FIRST.txt` in each zip has the rest.

## Running the standalone app

You need a recent stable **Rust** toolchain (the workspace is edition 2024 — Rust 1.85+). From
the repo root:

```sh
cargo run -p standalone --release
```

That builds and launches `lmv`, the standalone window. **On Windows it captures whatever is
already playing** (system audio, via WASAPI loopback) — start some music, and the visuals react.
`--release` is recommended: this is real-time graphics, and the debug build is noticeably slower.

### Controls

By default the app **holds one scene** — pick a look and it stays. Press `A` to
opt into auto-rotate (or set `auto = true` under `[rotate]` in `config.toml`);
when it's on, a scene holds ~20–90 s and an energy drop can nudge a change early.

Every preset change — `Space`, a pick from the browser, or an auto-rotate — **dissolves**
over about a second rather than cutting, so the show reads as continuous. The engine
rotates through a small library of dissolves (crossfade, additive burn, luma dissolve,
wipe), and a switch arriving mid-dissolve finishes the one in flight and starts the new
one, so you always land where you asked.

| Key       | Action                                                      |
|-----------|-------------------------------------------------------------|
| `Space`   | Next preset — dissolves (and restarts the auto-rotate timer) |
| `A`       | Toggle auto-rotate on/off (off by default)                  |
| `Tab`     | Open/close the preset browser — opens on the preset you're watching. `↑`/`↓` walk the list and wrap at both ends, `←`/`→` step a column, holding an arrow scrolls, type to filter, `Enter` selects (also dissolves), `Esc` closes |
| `S`       | Open/close the settings menu — quality, auto-rotate, dwell bounds, fullscreen, display, diagnostics. `↑`/`↓` pick a row, `←`/`→` change it, `Esc` closes. Every change applies immediately and (except diagnostics) is written to `config.toml` |
| `[` / `]` | Drop / raise the quality tier live — pins it for the session and persists the choice |
| `F`       | Toggle fullscreen                                           |
| `D`       | Cycle to the next display/monitor                           |
| `F3`      | Toggle the diagnostics overlay                              |

The browser lays the roster out in **as many columns as the window fits**, so a
library taller than the screen is visible at once rather than scrolled past. When
even the columns can't hold it, the list scrolls by whole columns and keeps the
highlighted preset on screen.

Both menus are modal and only one is open at a time: `S` opens settings when the
browser is closed (while it's open, `s` is a filter character), and `Tab` from
settings hands over to the browser.

### Flags & environment

- `--list-devices` — enumerate audio capture devices (Windows-only).
- `--soak [path]` — write a long-run instrumentation trace (frame-time stats) for stability
  testing; a bare `--soak` logs to a default path under the per-user data dir.
- `--tier floor|rich` — pin the quality tier instead of letting the engine pick. Unpinned, the app
  starts on `rich` and a frame-time governor demotes it to `floor` once if the display's frame
  budget is not being held (announced on stderr and marked with a `*` in the `F3` overlay). A pin
  is never demoted, so this is also how you keep `rich` on a machine a transient stall demoted.
  **The tier also moves while the app is running** — `[` / `]`, or the settings menu's Quality
  row. An in-app change **pins** the tier for the session (so the governor stops touching it) and
  writes it to `config.toml`'s `[quality] tier`. `--tier` and `LMV_TIER` still win at the next
  launch, so the precedence below is unchanged. Expect a brief re-accumulation of trails and
  feedback when it switches: the tier sizes GPU resources, so changing it rebuilds them.
- `LMV_PRESET_DIR=<dir>` — point the app at a custom preset folder instead of the seeded per-user
  directory; edits to `*.toml` there hot-reload live.
- `LMV_TIER=floor|rich` — the same pin as `--tier`, for a one-off run. Precedence is
  `--tier` > `LMV_TIER` > `config.toml`'s `[quality] tier` > auto.

> **macOS:** loopback capture **is** implemented — `standalone/src/capture_mac.rs` taps system
> audio through **ScreenCaptureKit**, so it needs **macOS 13+** and the **Screen Recording**
> permission (SCK will not run an audio-only stream, so the capture carries a throwaway 2x2 px
> stub video alongside the audio). Grant it, then **relaunch** — the app does not pick the
> permission up mid-run. The caveat is that **this path has never run on Apple hardware**: CI
> compiles it on `macos-latest` every push, but no runner can play audio or drive a real Metal
> adapter, so the first live run is also its validation. A window with visuals but no reaction
> to music means capture did not start, not a crash — launch from Terminal to see the reason.
> See **Platform notes** below.

## Design principles

This is real-time audio + graphics, so a few rules are non-negotiable:

- **The audio callback is sacred.** The capture / `visualisation_stream` thread never blocks,
  allocates, locks, or logs — it hands samples to the core through the ring buffer and returns.
- **The core stays source-agnostic and GPU-abstract.** No WASAPI / ScreenCaptureKit / foobar
  types in `core/`; no raw Metal/DX/Vulkan outside the wgpu layer. Swappability is the point.
- **Determinism where it's testable.** DSP math is a pure function of its input window; visual
  randomness, when wanted, is explicitly seeded.
- **The C ABI is a versioned contract.** Minimal surface — opaque handle, push-samples,
  render, resize, free. Changing its shape is an ADR-worthy event.
- **Lightweight is a feature.** Small binaries, few dependencies, low idle CPU/GPU.

## Presets

Visuals are driven by **presets** — small TOML files that bind a built-in
rendering system's parameters to short expressions over the live audio analysis
(no Rust, no rebuild). The whole curated set ships across every built-in system —
fragment field, particle swarm, parametric curve, L-system, star pattern,
reaction-diffusion, attractor, spectrum readout, ballistic emitter — seeded into a
per-user directory that both the standalone app and the foobar plugin share.

See **[`docs/presets.md`](docs/presets.md)** for the authoring guide and the
expression reference — grammar, variables, constants, functions, comparisons and
`select()`, how a bad preset is reported, and where the files live on disk. The
per-system parameter tables live in
**[`presets/README.md`](presets/README.md)**, beside the preset files.

Set **`LMV_PRESET_DIR`** to run against a custom preset folder instead of the
per-user one — `LMV_PRESET_DIR=./presets cargo run -p standalone` points the app
at the repo's own presets and hot-reloads an edit within ~150 ms.

## Visual QA / headless capture

Scenes can be rendered **with no window** — the core draws into an offscreen
texture and returns raw RGBA. A `shot` CLI writes PNGs (and a text/JSON metrics
report), and a differential harness in `core/tests/` hard-tests every preset for
reactivity, animation, shape sanity, and beat response (with an advisory
distinctness report and golden-image regression). It's dev/agent tooling — the
`image` crate is a dev-dependency only, so the shipped binary is untouched.

The synthetic `--signal` path (e.g. `--signal click:120`) needs no audio file.
For the `--audio` path, drop a **16-bit PCM WAV** into `assets/test/` — that
folder is gitignored, so test audio is added manually and never committed (use
your own or a royalty-free / CC0 clip).

`shot` reads the same library the app does — including `LMV_PRESET_DIR` — and
takes `--presets <dir>` / `--preset-file <path>` to capture a specific folder or
file, so an edit can be shot without touching the seeded copy.

See **[`docs/capturing.md`](docs/capturing.md)** for the runnable commands.

## Developer setup: the pre-push gate

A checked-in `.githooks/pre-push` runs the fast subset of CI before a push, so a
broken push costs seconds locally instead of minutes in CI. It is **opt-in per
clone** — enable it with:

```sh
git config core.hooksPath .githooks
```

> **An uninstalled clone has no gate.** Git will not run a hook from a tracked
> directory without that config, so until you set it nothing below happens. There
> is deliberately no auto-install ([ADR-0033](docs/adrs/0033-testing-strategy-coverage-ratchet-and-pre-push-gate.md)
> Alternative H).

What it runs, stopping at the first failure and naming the step that failed:

| Step | Command |
|------|---------|
| Doc links | `node scripts/check-doc-links.mjs` |
| Format | `cargo fmt --all --check` |
| Lint | `cargo clippy --all-targets -- -D warnings` |
| Tests | `cargo nextest run` (narrowed — see below) |

The doc-link step is first because it is the cheapest (~50 ms) — every relative
markdown link in the repo must resolve. It is the one step with **no CI
counterpart**, so for it this opt-in, `--no-verify`-bypassable hook is currently
the only gate. If `node` is not on your `PATH` it **skips with a notice** rather
than failing the push; nothing else here needs Node.

**Measured warm wall time: ~41 s** (fmt ~0.5 s, clippy ~0.7 s, tests ~40 s —
most of the suite, minus the nine excluded below). The full suite is
~121 s, so the hook excludes the nine
GPU-heavy suites that iterate every shipped preset or scene through a real
adapter — `golden`, `attractor`, `reaction_diffusion`, `background_composite`,
`ink`, `reactivity`, `animation`, `sanity`, `distinctness`. It **prints which
suites it skipped** on every run, so the narrowing is never silent, and **CI runs
all of them regardless**.

`cargo deny`, doctests, Miri, and the coverage job are deliberately *not* in the
hook — they push it into minutes, and a gate that hurts gets disabled (ADR-0033
Alternative F). They are also the checks least likely to break from a local edit.

Bypass once with `git push --no-verify`.

## Architecture decisions

Key decisions are recorded as ADRs in [`docs/adrs/`](docs/adrs/). Start with
[ADR-0001](docs/adrs/0001-rust-core-wgpu-cabi-foobar-shim.md) — the founding decision (Rust core,
wgpu rendering, C ABI, C++ foobar shim), with the rejected alternatives (C++ core, Electron,
OpenGL) recorded.

## Platform notes

- **Loopback capture is not symmetric.** Windows has first-class WASAPI loopback and needs no
  permission; macOS has no equivalent, so the Mac path goes through **ScreenCaptureKit** (macOS
  13+) and a user-granted Screen Recording permission. Both are implemented; only the Windows
  one has been exercised on real hardware. A virtual device (BlackHole) remains the fallback if
  the SCK route disappoints — set it as the output and no capture code is needed. The foobar
  plugin sidesteps capture entirely, which is part of why plugin parity is valuable on Mac.
- **The Mac build is made by CI, not here.** The dev box is Windows and cannot link a Mach-O
  binary, so a macOS runner is the only build host — which is why the `.app` arrives through a
  tag-driven release rather than from anyone's machine
  ([ADR-0038](docs/adrs/0038-tag-driven-release-unsigned-universal-mac-app.md)). `packaging/macos/bundle.sh`
  is checked in and runs standalone on any Mac, so that is not a permanent condition.
- **wgpu targets differ per OS** — Metal on macOS, DX12/Vulkan on Windows. Scene code writes to
  wgpu and does not branch on the backend.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in
this project by you, as defined in the Apache-2.0 license, shall be dual licensed as above,
without any additional terms or conditions.
