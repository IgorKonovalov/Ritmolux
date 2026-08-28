# ADR-0143 — The operator console is a second surface on the render device, and the shell owns every pixel's meaning

> **Status:** proposed
> **Date:** 2026-08-28
> **Related plan(s):** [0131 — The operator gets a console](../plans/0131-the-operator-gets-a-console.md)

## Context

The standalone drives multiple displays today and has since Plan 0009: `[output] display_name`
picks one, the `D` hotkey advances it, and `F` opens borderless-fullscreen on the chosen monitor.
What it does not have is a **second place to stand**. There is exactly one window, exactly one
`Renderer`, and both operator modals — the preset browser (`standalone/src/overlay.rs`) and the
settings menu (`standalone/src/settings.rs`) — are pure state machines whose rows are drawn *into
the show* through the core's glyphon text seam (ADR-0009). Operating the app therefore means typing
at the output: the only surface that exists is the one the audience is looking at.

The request is a desk-side operator seat — output on the far display, a small console on the near
one, carrying the two existing modals, a transport, and a live thumbnail of what is going out.
Three properties of the current renderer decide how that can be built, and each of them closes off
an option that looks obvious from outside.

**`draw_frame` cannot be called twice in a frame.** It takes an arbitrary `view` and size and is
already shared by the on-surface path and headless capture — so a second, smaller render into a
thumbnail texture reads as free. It is not: the function advances the dissolve at its end (the
`transition.as_mut().map(|tr| tr.advance(...))` block in `core/src/render/mod.rs`), promotes the
incoming side and may call `cancel_transition`. A second call per frame double-steps every
crossfade. Ping-pong scenes carry the same hazard one level down, in their own accumulation state.

**Plan 0115's frame tap is the wrong tap for this.** That plan's Phase 2 builds a
"transport-agnostic frame tap" and the overlap looks exact. It is not. That tap *re-renders*
through `draw_frame` at a caller-supplied `dt` — the hazard above — and then reads back to the CPU
through `map_async` + `poll(Wait)`, which Plan 0115's own Risks section names as forbidden in the
live display loop. It is correct for `--stream`, where its render is the only render and there is
no present deadline. It is not reusable here, and a console preview built on it would be a second
render and a blocking stall inside the frame.

**A second swapchain is a scheduling hazard, not just an allocation.** Two `Fifo` surfaces
presented from one thread serialize: with the console on a 60 Hz display and the output on a 144 Hz
one, the loop is paced by whichever swapchain runs dry first. The console is a monitor, not a show —
it has no reason to cost the output anything.

What remains genuinely open is **where the console's drawing lives**. Core owns the wgpu layer, the
device, the queue and the text stack; the shell owns the modals, the rows, and every question about
what the operator is looking at. A console needs both halves, and putting the seam in the wrong
place either teaches `core` about operator UI or forces `core` to hand out its GPU internals.

## Decision

The standalone opens a **second winit window** on a display of the operator's choosing, and `core`
gains a **secondary present target**: attach a `wgpu::Surface` to the existing device, present into
it, detach. Core's vocabulary is *surface*, never *console* — it learns that a second output exists
and what to draw there (a scaled copy of the frame it just drew, plus the text runs it was handed),
and nothing about rows, transports, staging or operators. The shell decides every pixel's meaning:
the console's content comes from the same `overlay.rs` and `settings.rs` state machines that already
exist, plus one new pure module for the console's own layout and hit-testing, and reaches core as
`TextRun`s and a present call.

The preview is a **GPU-side blit, never a readback**. While the console is open, the on-surface path
draws into a sampleable intermediate texture and reaches the output swapchain by
`copy_texture_to_texture` — an exact copy, no shader, no sRGB round-trip, so the output frame is
byte-identical to what it would have been with the console closed. The console then samples that
same intermediate with a scaled, letterboxed draw. One render, one frame, two destinations. The
intermediate exists only while the console is open; with it closed the present path is unchanged.

**The console is off by default, and it is opened from the settings menu.** That is not a
packaging detail — it is what makes the cost analysis above hold. Every price this decision pays
(a second swapchain, a full-resolution intermediate, one copy per frame) is paid only while the
console is open, so an operator who never opens one runs the app that exists today. A console
built the other way — a surface allocated at startup and merely left unread — would look
identical on screen and forfeit exactly that property.

Two rules ride along. The console's swapchain is **not** paced with the output's: it requests a
non-blocking present mode where one is available and presents on a decimated cadence, so a slower
operator display cannot become the output's frame clock. And the preview's aspect comes from the
**output render target**, never from the console window or the thumbnail's own extent — ADR-0037,
which this project has shipped wrong twice.

## Consequences

### Positive
- **The operator stops standing in the show.** The two modals move to the console when it is open,
  and the output carries only the picture. That is a behavioural fix the console gets for free, and
  it needs no new state — one modal instance, rendered wherever the operator is.
- **No new dependency.** Second window is `winit`, already linked; second surface is `wgpu`, already
  linked; text is the glyphon seam already behind core's `text` feature. NFR 4 is untouched, which
  is the whole reason the widget-toolkit and web-server options lost.
- **Both state machines are reused verbatim.** `overlay.rs` and `settings.rs` are window-free,
  renderer-free and config-free by construction (Plan 0008, Plan 0050 Phase 4). The console is a
  second *renderer* of state that is already pure — the property those modules were written for,
  finally collected.
- **The seam generalizes.** A secondary present target is not console-shaped. A future second
  output — a confidence monitor, a second projector fed the same frame — attaches the same way.

### Negative
- **A second swapchain is a second thing that can fail, and the failure is platform-shaped.** On a
  Windows laptop whose displays are driven by different GPUs, the console's surface may not be
  configurable on the adapter `core` already holds. The design degrades rather than aborts — the
  console opens with rows and transport and *no preview*, stating why — but that is a real
  second-class mode that only a two-GPU machine can exercise, and CI has one adapter.
- **The output pays one full-resolution texture and one exact copy per frame while the console is
  open.** The copy is a DMA-class operation and the texture is transient, but it is not nothing at
  1080p, and it is paid by the display loop this project protects hardest. It is bounded by being
  opt-in — closed is free, and asserted so — which means the cost lands on exactly the operator
  who asked for it, and never on a show nobody is driving.
- **We hand-draw every widget.** There are no buttons, no scrollbars and no text metrics — ADR-0009
  records that core exposes no text-measurement API, and `overlay.rs` already estimates its column
  width rather than measuring it. The console inherits that estimate and its truncation rule. A
  console that grows past a few rows and a thumbnail will feel the absence.
- **`main.rs` learns that windows are plural.** `window_event` currently ignores its `WindowId`.
  Routing by id is mechanical, but it touches the file Plan 0126 Phase 7 is already splitting.

### Neutral
- The console is standalone-only and the C ABI does not move. foobar2000 owns its own windows and
  its own UI; a console there would be a different design, and nothing here forecloses it.
- The console has no cue monitor. Staging shows as a text line naming what is next, not a second
  live render — the scope call the interview made, for the reason the plan records.

## Alternatives considered

### Alternative A — The shell owns its own wgpu pipeline, borrowing core's device
`standalone/` grows a `console/` module with its own render pipeline and its own glyphon instance,
and `core` exposes its `Device`, `Queue` and the intermediate texture. Core stays entirely ignorant
of second surfaces, which is the appeal. Rejected because the price is core's encapsulation: the
shell would reach past the renderer's API into the internals it exists to hide — the layering
inversion Mode 4 lens 5 is written to catch — and it duplicates the text stack the `text` feature
already provides. Handing out a device to avoid teaching the renderer about a second surface trades
a small, honest widening for a large, quiet one.

### Alternative B — A separate console process with its own device
The console becomes its own process (or its own wgpu device in-process), fully isolated, and the
preview arrives by CPU readback of each frame. Rejected on the readback: `map_async` + `poll(Wait)`
in the live display loop is the exact stall this codebase forbids and Plan 0115 declines to put
there. Isolation is worth paying for when something needs isolating; a console on the same machine,
in the same event loop, driving the same renderer, does not.

### Alternative C — Reuse Plan 0115 Phase 2's frame tap
The most tempting option, and the one this ADR was drafted believing. Rejected on the two facts in
Context: that tap re-renders through `draw_frame`, which double-advances the dissolve, and it
delivers its result on the CPU. Both are correct for a headless stream and wrong for a display-loop
thumbnail. The two taps are not one tap, and merging them would drag a blocking readback into the
frame to serve a consumer that never wanted the pixels on the CPU at all.

### Alternative D — A native widget toolkit (egui or similar)
Real buttons, real sliders, real scrolling, no hand-drawn geometry. Rejected on NFR 4: a substantial
dependency tree against a project whose first non-functional requirement is that it stays small, to
buy widgets for a surface that is a list, a transport and a thumbnail. It also introduces a second
UI idiom beside the one the two modals already establish.

### Alternative E — A localhost web control page
Serve a small page and drive the app from a browser — on any display, or a phone. Genuinely
attractive for the *remote* case, and it needs no window code at all. Rejected for this problem: it
buys reach we were not asked for, at the price of an HTTP/WebSocket dependency and a network
listener on an audio-visual app that has never opened a socket. The interview named the operator
seat as same-machine, one desk away. If reach is wanted later this is the design to revisit, and it
composes with the console rather than competing with it.

## Notes

The three properties in Context were each established by reading the tree, not from memory:
`draw_frame`'s trailing dissolve advance and its `view`-agnostic signature in
`core/src/render/mod.rs`; Plan 0115 Phase 2's re-render-and-read-back shape and its own Risks entry
naming the blocking readback; and the absence of any operator-level blackout or freeze in either
`core/src/render/mod.rs` or `standalone/src/main.rs` — the `hold` and `freeze` a grep finds are the
preset-expression latch and the dual-live budget latch, unrelated to an operator's button.
