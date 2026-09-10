# ADR-0175 — The studio is a separate application that never draws a frame, and the player stays the only renderer

> **Status:** accepted 2026-09-10 (Plan 0158)
> **Date:** 2026-09-09
> **Related plan(s):** [0158 — The player grows a studio-facing surface](../plans/done/0158-the-player-grows-a-studio-facing-surface.md),
> [0159 — The studio opens](../plans/0159-the-studio-opens.md)

## Context

The request is for Ritmolux to become a professional tool: presets authored live in a window
while the music plays, projects that gather a set of presets for one show, clips rendered from a
track, and the diffusion pass driven from the same window. Every one of those is a screen with
sliders, a code editor, a file list or a progress bar. At the same time the player must stay
exactly what it is today: one small executable that opens, captures, and draws, with nothing
installed beside it. Two constraints and four existing mechanisms decide how those fit together.

**The player executable is already at its size cap.** `target/release/ritmolux.exe` measures
10,277,888 B against [NFR §4](../nfr.md#4-size-and-dependencies)'s 10,000,000 B soft cap. A
widget toolkit, a text editor, a timeline, and a file browser compiled into that binary push it
well past the cap, and every user pays for them whether or not they ever edit a preset.
ADR-0143 already declined a native widget toolkit for the operator console on exactly this
ground, and it named a page served from the app as *"the design to revisit"* if reach were
wanted later. A studio is that revisit, one level up.

**ADR-0001's rejection of Electron does not reach a tool that draws nothing.** That ADR rejected
an Electron *visualizer* on two counts: the visual code would not reuse in the C++ foobar plugin,
and a heavy binary contradicts "lightweight". A studio that never renders has no visual code to
reuse, and its weight lands on an artifact the player never links. Reopening Electron here is
therefore not a reversal, and the rule stays: the core and the player are Rust and wgpu.

**Four things the tree already does make the studio mostly a client.**

- `standalone/src/preset_dir.rs` watches the per-user preset directory and reloads an edited file
  within about 150 ms, so a program that writes a `.toml` is already editing the running player.
- `ritmolux --stream` renders headless through `Renderer::render_tapped` and reads every frame back
  to the CPU (ADR-0125). Its only sink is a Spout sender behind the `spout` feature; the tap
  itself is transport-agnostic and compiled on every configuration.
- The operator console (ADR-0143) routes the show's frame through a preview intermediate on the
  same device, drawn once, with the show's pixels asserted byte-identical.
- OSC telemetry leaves the player under a versioned `/rlx/v1` address space (ADR-0164) through a
  hand-written encoder in `standalone/src/osc.rs`, with no OSC crate.

**The audience is the author plus a few VJs, handed a zip.** The player must need nothing. The
studio's editing and show features must work out of the box; rendering and the diffusion pass may
say what is missing, since `ffmpeg`, Python and CUDA are the user's to install today and stay so.

## Decision

We will build the studio as a **separate Electron application** in `studio/`, and it will
**never render a frame**. It is a control surface and a job runner: every pixel it shows comes
from a Ritmolux process. The live preview is the player itself, run headless with its frame tap
writing raw frames to the studio over a pipe, and later the windowed player with a preview copy
of its output. Clip rendering is the player's `render` subcommand, spawned with progress read
from its events. The diffusion pass is the Python sidecar, spawned when the studio finds Python,
CUDA and `ffmpeg` and declined with a plain message when it does not.

The player gains a small **studio-facing surface** and nothing else: an opt-in control channel
and a structured event stream (ADR-0176), a pipe sink for the existing frame tap, a JSON export
of the preset schema the engine already declares, the `render` subcommand promoted from the
`shot` example, and later a project file it can open without the studio. The core gains an
in-place parameter override and the schema export. **The core learns nothing about a UI, a
process, or a pipe**, and stays source-agnostic and GPU-abstract exactly as ADR-0001 has it.

**Size budgets are per artifact.** The player keeps NFR §4's cap untouched; the studio gets its
own row in `docs/nfr.md`, stated in bytes when Plan 0159 first measures a zip, and the two never
share a number. The foobar component already took its own figure in ADR-0159 for the same reason.

## Consequences

### Positive

- **The player is unchanged for everyone who does not edit.** A control listener that is off by
  default, an event flag nobody passes, and a sink nobody selects cost a windowed run nothing.
  The size cost of a real editor lands entirely on a binary nobody is forced to run.
- **The preview is the real engine, pixel for pixel.** No second engine, no port to another
  language, no divergence between what the editor shows and what the projector gets.
- **The player becomes drivable from outside.** The same control vocabulary the studio speaks
  is open to TouchDesigner, a lighting console, or a MIDI bridge, which is a professional feature
  the studio gets for free rather than a studio-only protocol.
- **The richest widget ecosystem for the editor itself.** A code editor with markers for the
  expression grammar, a timeline for shows and renders, drag-and-drop palettes. None of that is
  hand-drawn geometry through the glyphon seam.
- **Both platforms from one studio codebase**, with Chromium bundled so the zip runs on a
  stranger's Windows 10 machine without a webview install.

### Negative

- **A second shipped build system.** `studio/` is an npm project with a per-platform packaging
  job beside the two release zips. The repository already carries npm for the site, so the
  toolchain is known, but the release now has three artifacts and three version stamps to keep
  equal, and the studio must refuse a player whose version it does not know.
- **The studio zip is on the order of 100 MB and idles near 150 MB of memory.** That is the
  Electron price and it is paid in full. It is acceptable for the stated audience and would not
  be for a public player; this ADR does not make the studio a requirement for anything.
- **The preview crosses a process boundary as bytes.** At the default 640x360 and 30 fps that is
  about 28 MB/s over a local pipe, which is nothing for the pipe and something for the studio's
  renderer process to paint. A larger or faster preview is a measured setting, not a default.
- **The windowed-player preview is engine work with a frame-time cost.** Reading the console's
  preview intermediate back without blocking the display loop is a phase with a measured
  done-when, not a flag. Until it lands, editing during a live show means the headless player on
  the projector rather than the windowed one, or the studio beside a windowed player with no
  picture of its own.
- **The `dev` lane grows a language.** The studio is TypeScript, and the skill that implements
  plans lists Rust and C++ today. Plan 0159's first phase carries that change explicitly.

### Neutral

- The Python sidecar stays a sidecar. Nothing about the studio moves inference into a shipped
  artifact; ADR-0122's "nothing here ships" holds.
- `docs/` stays the single source. The studio's own page joins the site only by entering the
  publish map, as ADR-0154 has it for every document.

## Alternatives considered

### Alternative A — One executable, with an edit mode compiled in
The studio behind a flag or a hotkey in `ritmolux.exe`. Rejected on NFR §4: the binary is
already past the cap without a single widget, and every user would carry the editor. It also
puts a UI toolkit's event loop beside the display loop that ADR-0143 fought to keep unpaced.

### Alternative B — A native Rust studio with its own engine for preview
A third workspace binary on egui, hosting a second `Renderer` and showing its output as a texture
in the same window. The strongest option while the studio was expected to draw. Once the preview
is the player's own frame over a pipe, its one decisive advantage is gone, and what remains is a
thinner editor: no code editor with markers, no timeline, no file browser without a further crate
each. Not rejected on merit, and it is the design to return to if Electron's weight ever becomes
the problem.

### Alternative C — The studio embeds the player, and the lean player is a stripped build
One codebase, two build profiles. Rejected because the lean build becomes a variant that drifts
unless CI holds it, and the player is the thing that must not drift: it is what a foobar user and
a VJ on a small laptop run.

### Alternative D — Tauri
The same web UI at a tenth of the size, using the operating system's webview. Rejected for this
audience only: WebView2 is not guaranteed on a Windows 10 machine the zip is handed to, and the
studio must work out of the box. If the audience becomes a public release with an installer that
can carry the WebView2 bootstrapper, this alternative is the cheap swap, because the UI code is
the same.

### Alternative E — A browser page served by a bridge process
ADR-0143's Alternative E, one level up: a small Rust server links the core, serves a page, and the
studio runs in any browser, on a phone or a second laptop. Rejected as the *first* studio because
a browser cannot receive the preview pipe, and the remote case was not asked for. It remains the
design for remote control, and it composes with this one: the bridge would speak the same
control vocabulary.

### Alternative F — Grow the existing hand-drawn overlay into an editor
The settings menu and preset browser are pure state machines drawn through the glyphon text seam.
Rejected because a slider, a text field with a cursor, and a scrolling code editor are each a
widget toolkit's worth of work in that idiom, and the console already showed where the idiom's
ceiling is.

## Outcome (2026-09-10, Plan 0159 Phases 1 to 2)

**The preview leg named in the Decision is inverted, on a ground this ADR's own Context states
without following through.** The Decision reads *"the live preview is the player itself, run
headless with its frame tap writing raw frames to the studio over a pipe, and later the windowed
player with a preview copy of its output"*. The headless run is the wrong half of that pair to
start from: the Context lists `standalone/src/preset_dir.rs` as one of the four mechanisms that
make the studio "mostly a client", and that module is imported by `app_state.rs` alone — so the
**headless path never resolves, seeds, watches or reloads the preset directory**. The editing
loop the studio exists to close does not run there. That path also binds no control listener
(`run.rs` returns before `resolve_control`) and emits two of the eight events.

Nothing about *"the studio never draws a frame"* is affected — that half stands, and is what
Phases 1 and 2 built against.
[ADR-0181](0181-the-studio-drives-one-player-and-the-show-loop-is-extracted.md) changes which
player produces the pixels: the studio drives **one windowed player** that is both the show and
the preview source, and the show loop is extracted so the headless path stops being a silent
subset of it.

## Notes

The two facts this ADR rests on were read from the tree rather than remembered: the frame tap and
its transport split in `standalone/src/stream.rs` and `core/src/render/capture_api.rs`, and the
absence of a `windows_subsystem` attribute in the standalone, which is what makes its standard
streams real pipes for a parent process. The executable size is the release build on the
development machine dated 2026-09-05, not a CI measurement.
