# 0158 — The player grows a studio-facing surface

> **Status:** approved
> **Created:** 2026-09-09
> **Owner skill(s):** dev, human
> **Related ADRs:** [0175](../adrs/0175-the-studio-is-a-separate-application-that-never-draws-a-frame.md) (proposed),
> [0176](../adrs/0176-the-player-is-driven-over-osc-control-in-and-reports-on-its-standard-streams.md) (proposed),
> [0125](../adrs/0125-the-live-video-out-is-a-spout-sender-fed-by-a-frame-tap.md),
> [0143](../adrs/0143-the-operator-console-is-a-second-surface-and-the-shell-owns-its-meaning.md),
> [0164](../adrs/0164-the-osc-address-root-becomes-rlx-in-one-break.md),
> [0170](../adrs/0170-a-parameters-reference-row-is-generated-from-the-declaration-the-engine-reads.md)

## TL;DR

The player learns to be driven and to report, so that a separate studio can edit a preset while
the music plays and see the result. Six things land, all opt-in and all costing a windowed run
nothing: an in-place parameter override in the core; an OSC control-in listener under
`/rlx/v1/ctl/`; a structured event stream on standard error; a JSON export of the preset schema;
a pipe sink for the existing headless frame tap; and a preview copy of the windowed show's frame,
read back without blocking the display loop. The first user-visible behavior is a slider in
TouchDesigner moving a parameter on the running player with no file involved. The last is the
studio's preview coming from a windowed player mid-show.

## Context & problem

ADR-0175 decides the studio is a separate Electron application that never draws a frame. That
leaves the player as the only renderer, and today it can be told almost nothing from outside:
hotkeys, the console, a preset file rewritten under the watcher, and a `config.toml`. It reports
almost nothing structured either: `eprintln!` lines for preset errors, a diagnostics log, and OSC
telemetry that carries levels for lights.

The live editor needs four things the player does not offer. A parameter must move at slider
rate and stay moved without the reload path, which is around 150 ms late and, through
`configure_active_scene` in `core/src/render/roster.rs`, snaps the smoothers and resets the latch
bank on every save. The player's facts must reach a parent process in order: which preset is
active, what failed to parse and where, what the frame rate is. The studio must know what a
preset *can* contain, which the engine already declares through `ParamSpec` and the serde
schema in `core/src/preset/schema/`, and which `presets/README.md` is generated from
(ADR-0170). And the studio must see the picture. `--stream` already renders headless and reads
every frame back to the CPU; its only sink is Spout, behind a feature and Windows-only.

## Decision

We build the surface in the player and the core, in the order that makes each phase useful on
its own, and we write the protocol down as a living spec while it lands. We rejected a JSON
socket protocol (a codec and a framing layer for no external-controller benefit), files as the
only channel (150 ms late and a state reset per save), and a second engine in the studio (a
divergent preview and a port). ADR-0176 carries the reasoning.

## Architecture diagram

```mermaid
flowchart LR
    subgraph studio["studio/ (Electron, Plan 0159)"]
        UI[panels + editor]
        PV[preview canvas]
    end
    subgraph player["standalone (ritmolux)"]
        CTL[control-in listener<br/>UDP 127.0.0.1]
        EV[event writer<br/>stderr JSON lines]
        SINK[pipe sink<br/>stdout RGBA8]
        RT[render thread]
        WATCH[preset dir watcher]
    end
    subgraph core["core/ (rlx-core)"]
        OVR[param overrides]
        TAP[frame tap /<br/>preview intermediate]
        SCHEMA[schema export]
    end
    UI -- "/rlx/v1/ctl/*" --> CTL
    CTL -- bounded queue --> RT
    RT --> OVR
    UI -- "atomic .toml write" --> WATCH
    WATCH --> RT
    RT --> EV --> UI
    TAP --> SINK --> PV
    SCHEMA -. "ritmolux --schema" .-> UI
    EXT[TouchDesigner / console / MIDI bridge] -- OSC --> CTL
```

## Implementation phases

### Phase 1 — A parameter moves in place, and a save does not reset the look
- **Owner skill:** dev
- **What:** The core gains a per-name override on the active preset that shadows the evaluated
  binding until cleared, and a reload of the *same* preset with only its bindings changed keeps
  the scene's accumulated state instead of snapping and resetting.
- **Files touched:** `core/src/render/roster.rs`, `core/src/render/mod.rs`, the param smoothing
  and latch modules, `core/src/render/tests.rs`, a new `core/tests/override.rs`.
- **Done when:**
  - `Renderer::set_param_override(name, value)` and `clear_param_override(name)` exist; an
    override applies on the next frame, survives across frames, and is dropped by a preset
    switch and by `set_presets`. A test asserts the routed value on the frame after the call
    equals the override and the frame after the clear equals the binding.
  - A reload that replaces the roster with one where only the active preset's bindings differ
    does not call `configure_active_scene`'s reset path: the smoothers keep their eased values,
    the latches keep their state, and a stateful scene keeps its field. The test drives a trails
    or feedback preset for a hundred frames, reloads with one constant changed, and asserts the
    accumulated field is not cleared. Phrase it as a property: the field's energy after the
    reload is within the same frame-to-frame change the run showed before it, not a fresh
    field's.
  - A reload that changes the system, or the roster's shape, still takes the full reset path.
  - The C ABI is untouched; `docs/specs/0001-c-abi.md` needs no reconcile.

### Phase 2 — The player listens
- **Owner skill:** dev
- **What:** `--control <host:port>` and `[control]` in `config.toml` open a UDP listener,
  loopback by default; a hand-written decoder beside the encoder parses `/rlx/v1/ctl/*`; decoded
  actions cross to the render thread through a bounded queue and apply between frames.
- **Files touched:** `standalone/src/osc.rs` (split the encoder's module into `osc/encode.rs`
  and a new `osc/decode.rs`), new `standalone/src/control.rs`, `standalone/src/cli.rs`,
  `standalone/src/config.rs`, `standalone/src/run.rs`, `standalone/src/osc/tests.rs`,
  `docs/configuration.md`, new `docs/specs/0003-studio-control-protocol.md`.
- **Done when:**
  - Every message ADR-0176's table names round-trips through the encoder and the decoder
    byte-for-byte, and a fuzz-shaped test feeds truncated, over-long and mistyped packets and
    asserts none panics and each is counted as rejected.
  - A `ctl/param` datagram sent to the loopback port changes the routed value on the next frame,
    through Phase 1's override. `ctl/preset` dissolves to the named preset. `ctl/transport`
    drives the same four verbs the console's strip does, by the same code path.
  - The listener thread allocates nothing per packet after start, never touches the ring or the
    analyzer, and the queue keeps the **last** value per parameter name under a flood so the
    render thread drains a bounded amount per frame. A test sends ten thousand datagrams for one
    name and asserts the frame after sees only the last.
  - With no `--control` and no `[control]` key, no socket is opened: a test asserts the listener
    is `None` and `netstat`-style enumeration is not needed because the constructor is never
    called.
  - `docs/specs/0003-studio-control-protocol.md` exists in the shape `docs/specs/README.md`
    prescribes, its index gains the row, and the spec's invariants name what this phase tests.

### Phase 3 — The player reports
- **Owner skill:** dev
- **What:** `--events` prints one JSON object per line on standard error, through the same
  hand-rolled writer `standalone/src/shot/json.rs` uses, for the roster ADR-0176 lists.
- **Files touched:** new `standalone/src/events.rs`, `standalone/src/preset_dir.rs` (errors and
  warnings become events as well as lines), `standalone/src/run.rs`, `standalone/src/diaglog.rs`
  (the health figures already computed there feed `health`), `core/src/preset/mod.rs` if the
  parser's span has to be surfaced, `docs/configuration.md`, `docs/specs/0003-…`.
- **Done when:**
  - Every event line begins with `{`, ends with `\n`, carries `"v": 1` and `"ev"`, and no
    existing human diagnostic line begins with `{`. A test renders every event through the writer
    and asserts each parses as one object with those two fields, and a second grep-shaped test
    holds every `eprintln!` format string in the standalone to a first character that is not `{`.
  - `preset_error` carries the file and the message, and the line and column whenever the TOML
    parser's span provides them; an expression error carries the parameter name. A test loads a
    preset with a deliberate syntax error and asserts the line matches the one the fixture put it
    on.
  - `hello` is the first line after start and carries the workspace version, the control port
    actually bound, and the schema hash Phase 4 defines; `health` arrives once a second while a
    frame is being drawn and carries fps, frame-time p50 and p99, and the rejected-control count.
  - Without `--events`, nothing on standard error changes: the test compares a run's stderr
    against the pre-plan baseline for the same arguments.

### Phase 4 — The engine states what a preset can contain
- **Owner skill:** dev
- **What:** `ritmolux --schema` prints the preset schema as JSON: every system, every
  `ParamSpec` row (name, default, range, doc line), and every structural table with its keys,
  types, enumerations and defaults; plus a stable hash of it that `hello` reports.
- **Files touched:** `core/src/preset/schema/` (a descriptor per table beside its serde struct),
  new `core/src/preset/schema/export.rs`, `standalone/src/cli.rs`, `core/tests/preset.rs`,
  `docs/capturing.md` or `docs/configuration.md` for the flag.
- **Done when:**
  - The parameter half is generated from the same `ParamSpec` declarations the README block is
    (ADR-0170), so the two cannot disagree: a test renders both from one walk and asserts the
    names, defaults and ranges are identical.
  - The structural half cannot silently miss a key: a test round-trips every shipped preset and
    every `docs/examples/` preset through the descriptor, asserting that every key each file uses
    is one the descriptor names, and that every key the descriptor names is accepted by the
    loader. A key added to a serde struct without a descriptor row fails the test.
  - The hash changes when and only when the emitted document changes: two runs on one build agree,
    and a test that mutates one default sees a different hash.
  - No JSON crate is added to any workspace crate.

### Phase 5 — The headless tap writes to a pipe
- **Owner skill:** dev
- **What:** `--stream --sink stdout` sends raw RGBA8 frames on standard output at the requested
  size and rate, on every platform and with no feature, with a `stream` event announcing the
  geometry before the first frame. The Spout sink is unchanged.
- **Files touched:** `standalone/src/stream.rs` (the featureless `run` becomes real; the sink
  becomes a small trait with two implementations), `standalone/src/cli.rs`,
  `docs/capturing.md`.
- **Done when:**
  - A parent process reading standard output receives exactly `width * height * 4` bytes per
    frame, frames arrive in order, and the first bytes of standard output are the first frame:
    a test spawns the player with `--frames 30 --sink stdout` on the software adapter and asserts
    the byte count and that the `stream` event on stderr preceded them.
  - Nothing else is ever printed on standard output while a sink is open: the grep-shaped test
    from Phase 3 extends to `println!` and `print!` in the standalone, allowing only the sink.
  - The default preview request is 640x360 at 30 fps; the cost line the Spout path prints
    (`render+readback`, `send`) is printed for this sink too, so a figure exists for a reader on
    another machine.
  - The writer blocks on a full pipe rather than dropping, and the deadline pacing absorbs it:
    a test stalls the reader for one second and asserts the run resumes at the correct frame
    index, in ADR-0125's shape, rather than drifting.

### Phase 6 — The windowed show gets a preview copy without a stall
- **Owner skill:** dev
- **What:** `--preview stdout` on a windowed run routes the frame through the console's
  preview intermediate (`Renderer::open_preview`, ADR-0143), copies it to a staging buffer at the
  preview size, maps it asynchronously and consumes it **one frame late** with a non-blocking
  poll, and writes it to the same pipe format Phase 5 defines from a writer thread.
- **Files touched:** `core/src/render/preview.rs`, `core/src/render/mod.rs`, new
  `core/src/render/preview_readback.rs`, `standalone/src/run.rs`, `standalone/src/stream.rs`
  (shared sink), `core/tests/console_preview.rs`, `docs/running.md`.
- **Done when:**
  - The show's pixels are unchanged with the preview on: the byte-identity assertion
    `core/tests/console_preview.rs` already makes is extended to the readback path.
  - No blocking wait enters the display loop: a grep-shaped test holds `core/src/render/` to
    no `wait_indefinitely` outside `capture.rs` and `capture_api.rs`, and the readback consumes
    the buffer mapped by the *previous* frame's submission.
  - Residency does not grow: the headless tap waits on purpose because a headless loop outruns
    the GPU (the note above `render_tapped`'s poll in `core/src/render/capture_api.rs`); the
    windowed loop is paced by the swapchain, and this phase asserts it, with the 300-frame
    residency test the console already runs extended to the preview readback.
  - The cost is measured and printed, not assumed: the `diagnostics.log` line for the console
    gains the preview's frame-time p50 and p99 with the readback on and off, in the shape
    ADR-0172 requires with a witness that the readback ran. The plan does not set a threshold;
    the number is reported and the human phase judges it.
  - The default stays **off**. Nothing changes for a run without the flag.

### Phase 7 — The on-device check
- **Owner skill:** human
- **What:** Two things CI cannot see: a real controller and a real show.
- **Done when:**
  - From TouchDesigner or any OSC sender on the same machine, `ctl/param` moves a parameter on a
    playing preset and `ctl/preset` switches it, with `--events` showing the acknowledgement in
    `health`'s counters.
  - A windowed player on the projector with `--preview stdout` piped to a file plays a track for
    ten minutes; the preview frames are visibly the show, and the frame-time line from Phase 6 is
    read and recorded in `docs/on-device-validation.md`.

## Data shapes

```jsonc
// illustrative — one event line on stderr
{"v":1,"ev":"preset_error","file":"C:\\...\\aurora.toml","line":41,"col":9,
 "message":"unknown function `bins`"}
{"v":1,"ev":"stream","width":640,"height":360,"fps":30,"format":"rgba8"}
{"v":1,"ev":"health","fps":59.8,"frame_ms_p50":4.1,"frame_ms_p99":9.7,"ctl_rejected":0}
```

```rust
// illustrative — the override, core side
impl Renderer {
    pub fn set_param_override(&mut self, name: &str, value: f32) -> Result<(), ParamError>;
    pub fn clear_param_override(&mut self, name: &str);
    pub fn clear_param_overrides(&mut self);
}
```

The schema document's exact shape is Phase 4's to fix; the plan holds it to two properties (the
params half is generated from `ParamSpec`, the structural half is round-trip gated) rather than
to a layout.

## Risks & open questions

- **Keeping the scene's state across a reload may be harder than skipping a reset.** If a
  scene's `configure` rebuilds GPU resources from the preset, "same system, new bindings" needs a
  narrower entry point than the current one. Phase 1 may land the override first and the
  state-keeping reload as its second commit; both are required for the phase to be done.
- **The TOML parser's span may not survive the loader's error type.** Phase 3's line and column
  are "whenever the parser provides them"; if `PresetError` flattens the span, the phase adds it
  to the type rather than parsing the message.
- **A blocking pipe write on a windowed run is a stall.** Phase 6 writes from a thread with a
  bounded queue that drops frames when the reader is slow; Phase 5's headless writer may block
  because that loop has no present deadline. The two policies are deliberate and documented.
- **A UDP listener on a show machine.** Loopback by default, off by default, no message can
  allocate or reach the audio path. The one real exposure is that anything on the machine can
  move a parameter, which is the feature.
- **Two lanes on `standalone/src/run.rs`.** Plan 0133 and Plan 0120 are approved and both touch
  the run loop; this plan should open after or beside them in a worktree and merge `main` before
  Phase 6.

## What this plan does NOT do

- **No studio.** Plan 0159 builds it; this plan is what it drives.
- **No `render` subcommand and no project file.** ADR-0175 decides both; each arrives with the
  studio plan that needs it (clip rendering, show projects), so the vocabulary grows with a use.
- **No MIDI.** A MIDI bridge is a program that speaks `ctl/*`; nothing in the player learns MIDI.
- **No Spout changes**, no macOS Syphon, and no change to telemetry's addresses or cadence.

## Implementation log

> Written by `dev` — one row per phase as that phase's commit lands, and the close block after the
> last one. **The phases above are the contract; everything here is what happened.**
> **Observations, never conclusions:** this says where to look, architect decides how it went.
> No per-criterion pass list, no self-assessment, no narrative — but a deviation from the plan or
> an unmet done-when is always disclosed. Stays shorter than `## Implementation phases` above.

**Lane:** _(`main` directly, or the worktree path plus its branch — `WORK/rlx-plan-0158` on
`plan-0158-<slug>`)_

| phase | owner | state | commit |
|---|---|---|---|
| 1 — A parameter moves in place | dev | not started | |
| 2 — The player listens | dev | not started | |
| 3 — The player reports | dev | not started | |
| 4 — The engine states what a preset can contain | dev | not started | |
| 5 — The headless tap writes to a pipe | dev | not started | |
| 6 — The windowed show gets a preview copy | dev | not started | |
| 7 — The on-device check | human | not started | |

### Notes

### Close triggers

- **`presets/` touched:**
- **Plan header `Closes:`** none
- **What shipped:**
- **Operator docs touched:**
- **Backlog probes (`node scripts/check-backlog-claims.mjs`):**
- **Full suite:**
- **Outstanding `human` phases:**

## Followups (after this lands)

- The same `ctl/*` vocabulary on standard input, if a parent-only channel is ever wanted
  (ADR-0176 Alternative C).
