# light-music-visualizer

![A neon kaleidoscopic mandala: a deep violet eight-lobed core ringed by cyan and magenta
petals, with pale swept arcs opening outward into black](docs/images/hero.png)

A lightweight, real-time music visualizer built around one **shared Rust core** that turns a
stream of PCM audio samples into GPU-rendered visuals. Two frontends consume that core:

- **Standalone app** (Windows + macOS) — pure Rust (`winit` + `wgpu`), fed by OS loopback
  audio capture.
- **foobar2000 plugin** (Windows-first) — a thin **C++ shim** over the core's **C ABI**, fed by
  foobar's own `visualisation_stream` (no loopback needed on that path).

The core is **source-agnostic**: it takes interleaved/mono PCM frames and does not care whether
they came from loopback capture or foobar. That single abstraction is what lets one visual
codebase serve both frontends.

| | | |
|---|---|---|
| [![A luminous sea-green rosette of fine particle filaments on black](docs/images/gallery/attractor.png)](docs/preset-guide.md) | [![A radial spectrum readout: coloured spokes radiating from a dark centre](docs/images/gallery/spectrum.png)](docs/preset-guide.md) | [![A bold gold rose window of nested twelve-pointed stars](docs/images/gallery/star_pattern.png)](docs/preset-guide.md) |
| `attractor` | `spectrum` | `star_pattern` |

Ten built-in rendering systems, all driven by editable text presets —
**[see them, and how to write one](docs/preset-guide.md)**.

> Every picture in this repository is a **headless render of the engine**, captured by the `shot`
> CLI under a synthesized audio clip — not a screenshot of the application window. There is no
> picture anywhere of the preset browser, the settings menu or the `F3` overlay.

> **Status: pre-1.0, in active development.** Both frontends run **and both ship**: the standalone
> app renders live WASAPI loopback on Windows, and the foobar2000 component links the core's C ABI
> and is attached to every `v*` tag since `v0.70.0`. The preset format and the C ABI may still
> change between releases — stability begins at 1.0.0. See [`docs/plans/`](docs/plans/) for what's
> in flight.

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
packaging/           # What a `v*` tag ships, one recipe per artifact, each doing its own verification
                     #   so a local run is held to CI's bar: macos/bundle.sh (build, lipo, sign, zip,
                     #   verify) and foobar/ (fetch the pinned SDK, build, stamp, package, verify).
                     #   Plus the READ-ME-FIRST.md testers get in each zip. See ADR-0038, ADR-0115.
docs/
├── nfr.md           # Quantified v1 non-functional requirements (the numbers behind "lightweight").
├── preset-guide.md  # START HERE for presets: the illustrated entrance — the systems, one
│                    #   picture each, and the loop you work in.
├── preset-tuning-walkthrough.md  # One preset tuned over five steps, with the picture AND the
│                    #   --report row that changed at each one.
├── presets.md       # Preset authoring guide: the expression language, loading, and where files live.
├── preset-palettes.md  # The colour surface: built-in palettes, custom stops, the A/B crossfade.
├── capturing.md     # Headless capture: the shot CLI, the core/tests/ checks, and --render (video).
├── releasing.md     # The version-bump / release procedure (one bump per plan close).
├── on-device-validation.md  # The manual checklist for what CI cannot run: real GPUs, live loopback,
│                    #   and installing the foobar2000 component (no runner can load foobar2000).
├── design-backlog.md  # Captured friction not yet promoted to an ADR or a plan (…-archive.md holds retired entries).
├── roadmap-visual-richness.md  # The visual-capability roadmap the recent plans are sequenced against.
├── generative-techniques-catalogue.md  # The technique survey behind the scene families.
├── images/          # The committed documentation renders, regenerated by scripts/docs-shots.mjs.
├── examples/        # Teaching presets for the guide + walkthrough. Never shipped, never seeded.
├── adrs/            # Architecture Decision Records + rejected alternatives. Append-only.
├── specs/           # Living behavioral contracts per core subsystem (C ABI, ring/DSP determinism).
└── plans/           # Phased implementation plans (what's in flight); done/ holds completed plans.
```

The per-system parameter tables live in [`presets/README.md`](presets/README.md), beside the
preset files they document.

## Download

Prebuilt binaries are attached to each tag on the
[Releases page](https://github.com/IgorKonovalov/light-music-visualizer/releases). Three zips per
release, each carrying a `READ-ME-FIRST.txt`:

| Zip | What's in it |
|-----|--------------|
| `…-macos-universal.zip` | `LightMusicVisualizer.app` — universal (Apple Silicon + Intel), **macOS 13+** |
| `…-windows-x64.zip` | `lmv.exe` — Windows x64 |
| `…-foobar2000-component.zip` | `foo_lmv.fb2k-component` — foobar2000 v2, **x64 only** |

The two standalone zips also carry a reference copy of the presets.

All three are **unsigned**, so each host objects once. On Windows, SmartScreen says "Windows
protected your PC" → More info → Run anyway. On macOS, the app is ad-hoc signed only, so either
right-click it and choose **Open**, or strip the quarantine attribute first:

```sh
xattr -dr com.apple.quarantine LightMusicVisualizer.app
```

The macOS build then asks for the **Screen Recording** permission — that is the only first-party
way to tap system audio — and needs a **relaunch** after you grant it. Releases are marked
prerelease while the app is `0.x`. The `READ-ME-FIRST.txt` in each zip has the rest.

### The foobar2000 component

Unzip, then in foobar2000: **File → Preferences → Components → Install…**, pick
`foo_lmv.fb2k-component`, **Apply**, and let it restart. Open it from **View → Light Music
Visualizer**, or dock it into the layout as a *Playback visualisation* element. `Space` cycles
scenes.

It needs **64-bit foobar2000 v2 on Windows** — there is no 32-bit build and no macOS component
([ADR-0001](docs/adrs/0001-rust-core-wgpu-cabi-foobar-shim.md); the SDK is Windows-centric). A
32-bit install will simply not list it.

Because it reads what foobar2000 is already decoding, there is no audio capture to permit and no
output device to route — it is the path with the fewest ways to go wrong. If you run both, the
component and the standalone app **share one preset folder**, so a preset edited in either shows
up in both.

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
| `S`       | Open/close the settings menu — quality, auto-rotate, dwell bounds, fullscreen, display, diagnostics, preset name, now playing. `↑`/`↓` pick a row, `←`/`→` change it, `Esc` closes. Every change applies immediately and (except diagnostics) is written to `config.toml` |
| `[` / `]` | Drop / raise the quality tier live — pins it for the session and persists the choice |
| `F`       | Toggle fullscreen                                           |
| `Esc`     | Leave fullscreen (with no menu open). Does nothing in a window, and never quits |
| `D`       | Cycle to the next display/monitor                           |
| `F3`      | Toggle the diagnostics overlay                              |

The browser lays the roster out in **as many columns as the window fits**, so a
library taller than the screen is visible at once rather than scrolled past. When
even the columns can't hold it, the list scrolls by whole columns and keeps the
highlighted preset on screen.

Both menus are modal and only one is open at a time: `S` opens settings when the
browser is closed (while it's open, `s` is a filter character), and `Tab` from
settings hands over to the browser.

The active preset's **name** sits in the top-left corner, and it gets out of the
way on its own: either menu or the `F3` overlay hides it, and it comes straight
back when they close. For a permanently clean canvas, turn the settings menu's
**Preset name** row off — that is `[hud] preset_name` in `config.toml`, and it
survives a restart.

### Now playing

When the track changes, the **artist and title fade in** over the visuals in the
lower-left corner, hold a few seconds, and fade out — an announcement, not a
status bar. A long title is truncated to fit rather than running off the screen.

On **Windows** the metadata comes from the system's now-playing feed (SMTC) —
the same one behind the media flyout — so it works with whatever player is
publishing to it, foobar2000 included. **Not every player publishes there.** One
that doesn't simply produces no banner: this is a nicety that stays silent when
it has nothing to say, never an error. Closing the player clears it.

On **macOS** the standalone gets no metadata: the OS has no supported equivalent
(`MediaRemote` is private and restricted), which is the same asymmetry loopback
capture already has. The foobar2000 plugin is the answer on that platform.

To turn it off entirely, use the settings menu's **Now playing** row — that is
`[hud] now_playing` in `config.toml`, and like the preset name it survives a
restart. Off means no track ever reaches the visualizer.

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
- **The C ABI is a versioned contract**, and [`docs/specs/0001-c-abi.md`](docs/specs/0001-c-abi.md)
  is the authority on its shape — not this list, which paraphrased five functions long enough for
  the real surface to reach thirteen. Changing that shape is an ADR-worthy event.
- **Lightweight is a feature.** Small binaries, few dependencies, low idle CPU/GPU.

## Presets

Visuals are driven by **presets** — small TOML files that bind a built-in
rendering system's parameters to short expressions over the live audio analysis
(no Rust, no rebuild). The whole curated set ships across every built-in system —
fragment field, particle swarm, parametric curve, L-system, star pattern,
reaction-diffusion, attractor, spectrum readout, ballistic emitter, shape field —
seeded into a per-user directory that both the standalone app and the foobar
plugin share.

**Start with [`docs/preset-guide.md`](docs/preset-guide.md)** — the illustrated
entrance: a complete preset in ten lines, what each built-in system looks
like and when to reach for it, and the loop you work in. Then
[`docs/preset-tuning-walkthrough.md`](docs/preset-tuning-walkthrough.md) tunes
one preset over five steps, showing the picture **and the `--report` row** that
changed at each one.

The three references the guide links into, each owning one surface:

| Document | Owns |
|---|---|
| [`presets/README.md`](presets/README.md) | every parameter each system takes, plus the structural, smoothing and engine-stage tables |
| [`docs/presets.md`](docs/presets.md) | the expression grammar — variables, functions, `select()`, and how a bad preset is reported |
| [`docs/preset-palettes.md`](docs/preset-palettes.md) | the colour surface — palettes, custom stops, the A/B crossfade |

Set **`LMV_PRESET_DIR`** to run against a custom preset folder instead of the
per-user one — `LMV_PRESET_DIR=./presets cargo run -p standalone` points the app
at the repo's own presets and hot-reloads an edit within ~150 ms.

## Rendering a music video

The engine renders a track to a video file **offline** — not by recording the
window. From a source checkout, one command walks a WAV end to end and produces
an MP4 with the audio muxed in:

```bash
cargo run -p standalone --example shot -- \
  --preset "Supernova" --render track.wav --fps 30 --size 1920x1080 \
  --ffmpeg ffmpeg --out track.mp4
```

Because the render is offline it is **decoupled from real time**: every frame is
drawn at an exact `1/fps` step regardless of how long it took, so the result is
deterministic and never drops a frame the way a screen recorder does. It is also
the mode where `--tier rich` is most worth paying for — there is no 60 Hz
deadline for the frame-time governor to miss.

**`ffmpeg` is a prerequisite and no encoder ships with this project.** A static
`ffmpeg` is larger than the application's entire size budget
([NFR §4](docs/nfr.md#4-size-and-dependencies)), so `shot` streams Y4M frames to whichever `ffmpeg` you
point it at and lets it own the container
([ADR-0114](docs/adrs/0114-the-engine-renders-video-offline-and-delegates-encoding.md)).
Without `--ffmpeg` the raw frame stream goes to stdout for any encoder to read.

See **[`docs/capturing.md`](docs/capturing.md#--render-a-music-video-from-a-track)**
for the frame-rate rules, the exact `ffmpeg` command line it generates, and what
it reports about a long render.

## Visual QA / headless capture

Scenes can be rendered **with no window** — the core draws into an offscreen
texture and returns raw RGBA. That is the same path [the video renderer](#rendering-a-music-video)
runs on. A `shot` CLI writes PNGs (and a text/JSON metrics
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

See **[`docs/capturing.md`](docs/capturing.md)** for the runnable commands. The
images in this README and in the preset docs come from that same CLI, driven by
**`node scripts/docs-shots.mjs`** — an argument-free script whose manifest records
the preset, stimulus, hop, size and tier behind every committed picture
([ADR-0100](docs/adrs/0100-documentation-images-are-committed-headless-renders.md)).

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
| Index rows | `node scripts/check-index-rows.mjs` |
| Backlog claims | `node scripts/check-backlog-claims.mjs` |
| Format | `cargo fmt --all --check` |
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` |
| Tests | `cargo nextest run --workspace` (narrowed — see below) |

The three Node steps come first because they are the cheapest (tens of
milliseconds between them): every relative markdown link in the repo must
resolve, every row inside a marked roster region must stay under 320 bytes
([ADR-0116](docs/adrs/0116-an-index-row-is-a-pointer-and-a-gate-holds-it-to-one.md)),
and every live `docs/design-backlog.md` entry must carry a probe that still holds
([ADR-0108](docs/adrs/0108-a-backlog-claim-about-the-repo-carries-an-executable-probe.md)).
If `node` is not on your `PATH` all three **skip with a notice** rather than
failing the push; nothing else here needs Node. That skip is about the hook only
— CI's `links` job runs the same three checks on `ubuntu-latest`, where they
cannot skip and are not bypassable.

**Measured warm wall time: ~48.6 s** (2026-08-08; dominated by the tests — `fmt`
and `clippy` are under two seconds between them). The hook excludes the nine
GPU-heavy suites that iterate every shipped preset or scene through a real
adapter — `golden`, `attractor`, `reaction_diffusion`, `background_composite`,
`ink`, `reactivity`, `animation`, `sanity`, `distinctness`. It **prints which
suites it skipped** on every run, so the narrowing is never silent, and **CI runs
all of them regardless** — though since [ADR-0073](docs/adrs/0073-the-windows-ci-critical-path.md)
it runs those nine in the `coverage` job alone rather than in two Windows jobs, so
the promise is now underwritten by one job instead of a redundancy between two.

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
