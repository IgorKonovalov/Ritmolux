# ADR-0187 — The preview pipe has a fixed shape and names the format it actually carries

> **Status:** proposed
> **Date:** 2026-09-10
> **Related plan(s):** [0167 — The studio becomes handable](../plans/0167-the-studio-becomes-handable.md)
> **Related:** [ADR-0176](0176-the-player-is-driven-over-osc-control-in-and-reports-on-its-standard-streams.md)
> (the event roster, and the `stream` row this changes),
> [ADR-0178](0178-the-studio-shell-conventions.md) (the studio's frame path, and its `Outcome` on
> what the drop counter cannot see),
> [ADR-0183](0183-the-studio-drives-one-player-and-the-show-loop-is-extracted.md) (the windowed
> preview this repairs), [ADR-0186](0186-the-studios-player-mode-is-a-per-machine-setting.md) (the
> two modes that hand the studio this pipe),
> [spec 0003](../specs/0003-studio-control-protocol.md) (the `stream` event's contract)

## Context

Two defects filed against Plan 0159's packaged artifact have one root: **the `stream` event
describes a pipe that is not the pipe.**

**Backlog 0200 — the format is a fixed string.** `STREAM_FORMAT` is `"rgba8"`, and `show.rs` emits
it into every `stream` event on both run modes. The **windowed** preview intermediate is built at
`self.ctx.surface_format()` — the format the swapchain negotiated, not one this project chose — so
on a backend that negotiates `Bgra8UnormSrgb` the pipe carries BGRA under an `rgba8` label. The
studio does the only correct thing with what it was told, `new ImageData(pixels, …)`, which is RGBA
by definition, and paints the Clifford attractor blue where the player draws it orange. The headless
path renders into `HEADLESS_FORMAT`, which is `Rgba8UnormSrgb`, so the same label is **true there
and false on the windowed path** — one declared format naming two byte layouts depending on how the
player was started.

**Backlog 0201 — the geometry moves and kills the writer.** `open_preview_pipe` runs once, at window
creation, and spawns a `PreviewPipe` for the readback's size at that moment. `WindowEvent::Resized`
calls `Renderer::resize`, which rebuilds the preview target and reopens the readback **at the new
size** — the readback is an exact copy of the intermediate, and `preview_readback_size`'s own doc
comment says so: *"the copy out of it is exact, and an exact copy has one size."* `StdoutSink::send`
then refuses a frame whose size is not the announced one, correctly, and the writer thread's loop is
`if sink.send(..).is_err() { break; }` — so the refusal ends the thread permanently and discards the
error's message. Standard output goes quiet, the canvas holds its last frame, the studio's counter
stays at `0`, the player keeps drawing, and nothing reaches standard error.

Underneath both sits a third fact that is not a defect but shapes the repair. The windowed preview
inherits the show's resolution, so at 1920x1080 it moves **8,294,400 B per frame**, and Plan 0159
Phase 4 measured the show falling from ~42 fps to 35.4 fps with the studio attached — a **15.7 %**
loss on the reference machine. A preview whose cost scales with the show's resolution gets worse
exactly when the show matters most.

## Decision

**The preview readback samples into a fixed target instead of copying the intermediate exactly, and
`--preview` takes that target's size.** `PreviewTarget` gains a size; the readback is opened against
it; a scaling blit fills it from the intermediate each frame. The show's window may then be resized,
maximized or thrown fullscreen and **the pipe's geometry does not move**.

**The `stream` event's `format` names the order the pipe actually carries** — `rgba8` or `bgra8`,
read from the texture format the frames are produced at rather than from a constant.

Together these make the `stream` event true and keep it emit-once:
spec 0003's *"once, before the first frame on a frame pipe"* invariant is unchanged, because after
this there is nothing about the pipe left to change.

`--sink stdout` is untouched. It stays raw, exact, and at the show's own geometry, because its
consumer is `ffmpeg` and its contract is a byte stream of whole frames at a size named on the
command line.

## Consequences

**Positive.**

- **The writer cannot die of a size disagreement**, because sizes no longer disagree. Backlog 0201's
  mechanism is removed rather than reported.
- **`stream` stays emitted once**, so spec 0003 does not gain a repeatable event and the studio's
  `FrameSplitter` never has to be rebuilt mid-stream. This is what avoids the unmarked-boundary race
  described under Alternative A.
- **The preview's cost stops scaling with the show's.** At 640x360 a frame is **921,600 B** against
  1920x1080's **8,294,400 B** — exactly **9x** — and a 4K show costs the preview nothing extra.
- **The studio can ask for what it can paint.** A preview larger than the canvas that displays it
  was never worth its bytes.
- A consumer that reads `format` gets a fact rather than a claim, on either run mode.

**Negative.**

- **A studio pixel is no longer a show pixel.** The preview is a scaled copy, so it cannot be used to
  judge anything at pixel scale — aliasing on a thin stroke, a one-texel seam, the exact width of a
  contour. Whoever needs that judgement needs the show window, and
  [ADR-0186](0186-the-studios-player-mode-is-a-per-machine-setting.md)'s `windowed` mode is where it
  lives.
- **A sampling blit per preview frame**, in `core/`, on the show's own encoder. It is cheaper than
  the readback it feeds and far cheaper than the pipe write it replaces, but it is not free and it
  is paid on every frame the preview is open.
- **`format` has two values, so every consumer must read it.** Today the studio is the only one, and
  spec 0003's row moves from a constant to a closed set — which is a contract change, not an
  addition.
- **The two stdout paths now differ in kind.** `--preview` is a fixed-size mirror and `--sink stdout`
  is an exact feed. That is two things where there was almost one, and the flags' help text has to
  say so or someone will reach for the wrong one.

## Alternatives considered

**A — Re-emit `stream` on resize and respawn the pipe.** The obvious repair, and what
ADR-0183's `Outcome` names before this ADR was written. *Rejected on the pipe boundary.* The studio's
`FrameSplitter` slices at a fixed `frameBytes`; when the geometry changes, bytes already buffered in
the OS pipe are still the old size and **nothing in the byte stream marks where the new size
begins**. The studio would learn the new geometry out-of-band, on stderr, with no way to align it
against stdout. Making that honest means a per-frame header — magic, sequence, dimensions, length —
and a raw frame can contain any bytes, so no sentinel works without one. A header breaks
`--sink stdout`'s documented raw contract with `ffmpeg`, and giving the two paths different framing
to avoid that is the same divergence this decision reaches by a cheaper road.

**B — Pin the show window while a preview is open.** Refuse `Resized` and the fullscreen toggle
whenever the studio is attached. *Rejected:* no spec change, no core work and no race, but it takes
away going fullscreen on the projector, which is a VJ's primary gesture, for as long as the editor
is connected. The tool would be least usable in exactly the situation it exists for.

**C — Convert to the declared format before writing.** Keep `format` a single value and have the
player swizzle BGRA to RGBA on its way out. *Rejected:* a per-frame pass over 8 MB on the show's own
thread is precisely the cost [ADR-0178](0178-the-studio-shell-conventions.md) wanted kept honest, and
its `Outcome` records that the preview already costs the show 15.7 % of its frame rate. Paying more
there to save the studio a branch is the wrong side of the seam: the studio has a GPU too, and a
swizzle in a fragment shader is free where a CPU pass over every frame is not.

**D — Let the studio detect the channel order.** Sample a known pixel, or guess from the adapter
name. *Rejected:* it is a guess dressed as a measurement, it fails on a frame with no known pixel,
and the player already holds the fact. The whole of
[ADR-0184](0184-the-player-reports-what-it-loaded-and-the-studio-re-derives-nothing.md) is that the
studio re-derives nothing the player knows.

## Notes

The scaled preview also retires, for the windowed path, the throughput half of ADR-0178's `Outcome`
— but **not** its accounting half. `FramePump` still counts a drop only when a frame arrives while
one is unacknowledged, so loss upstream of it stays invisible; a cheaper pipe makes that loss rarer
without making it visible. Whoever fixes the accounting is fixing a different thing, and backlog 0201
records what it would have to be able to tell apart.

The writer thread still exits silently on a size disagreement. After this decision no size
disagreement can arise, so the silence is unreachable rather than repaired — a deliberate call at
Plan 0167's interview, recorded in that plan's "What this plan does NOT do".
