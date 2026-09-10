# Spec — Studio control protocol

The behavioral contract for the two channels a parent process drives and reads this player
through: the OSC control-in vocabulary under `/rlx/v1/ctl/`, and the structured event stream on
standard error. It states what must be true of the running system, not how it is implemented.

> **Where it lives:** `standalone/src/osc/decode.rs` (the vocabulary and the wire decoder),
> `standalone/src/osc/encode.rs` (the wire encoder both directions share),
> `standalone/src/control.rs` (the listener, the bounded queue and the renderer-side applier),
> `standalone/src/events.rs` (the event roster and its writer),
> `standalone/src/show.rs` (the one owner of the listener and of every emission below,
> called by the windowed and the headless paths alike),
> `standalone/src/config.rs` (`[control]`) and `standalone/src/cli.rs` (`--control`,
> `--events`).
> **Governing ADRs:** [0176](../adrs/0176-the-player-is-driven-over-osc-control-in-and-reports-on-its-standard-streams.md)
> (the transports, the versioning rule and the two rosters),
> [0175](../adrs/0175-the-studio-is-a-separate-application-that-never-draws-a-frame.md) (why the
> player is the only renderer, so everything a studio does arrives over a channel),
> [0164](../adrs/0164-the-osc-address-root-becomes-rlx-in-one-break.md) (the `/rlx/v1` root and
> the additive-under-the-prefix rule) and
> [0143](../adrs/0143-the-operator-console-is-a-second-surface-and-the-shell-owns-its-meaning.md)
> (the transport strip whose actions the wire verbs resolve) and
> [0181](../adrs/0181-the-studio-drives-one-player-and-the-show-loop-is-extracted.md) (why neither
> roster below is conditional on a run mode).
> **How the contract got here** is at the end, under *Provenance*.

## The vocabulary

One message is one action with typed arguments. Adding a row is additive under the same prefix;
changing or removing one moves the version segment.

| Address | Args | Action |
|---------|------|--------|
| `/rlx/v1/ctl/param` | `s name`, `f value` | Override `name` on the active preset with `value`, in place, until cleared |
| `/rlx/v1/ctl/param/clear` | `s name` | Drop that override; the preset's own binding resumes |
| `/rlx/v1/ctl/params/clear` | none | Drop every override |
| `/rlx/v1/ctl/preset` | `s name` | Dissolve to the named preset |
| `/rlx/v1/ctl/transport` | `s next\|prev\|auto\|hold` | The console's transport, by name |
| `/rlx/v1/ctl/ping` | `i nonce` | Answered by a `pong` event carrying the nonce |

## The event roster

One JSON object per line on standard error, each carrying `"v"` and `"ev"` before anything else.
Adding an event or a field is additive under the same `v`; changing or removing one moves it.

| `ev` | Fields | When |
|------|--------|------|
| `hello` | `version`, `schema`, `control` | Once, before the first frame of any run |
| `preset` | `name`, `index` | The preset **on screen** changed |
| `roster` | `names` | Every preset reload |
| `preset_error` | `file`, `message`, `line`, `col`, `param` | A preset failed to load |
| `preset_warning` | `file`, `message` | A preset loaded with a non-fatal problem |
| `health` | `fps`, `frame_ms_p50`, `frame_ms_p99`, `ctl_rejected`, `ctl_dropped`, `ctl_refused` | Once a second while frames are drawn |
| `stream` | `width`, `height`, `fps`, `format` | Once, before the first frame on a frame pipe |
| `pong` | `nonce` | Answering a `ctl/ping` |

## Invariants

- **Neither roster above is conditional on a run mode.** A run that holds a renderer, a preset
  directory and a rotation policy performs both tables, whether it opens a window or writes frames
  to a pipe; the two differ in the window and the sink and in nothing here. A mode that held that
  state and reported none of it would be silent rather than broken, which is why this is an
  invariant rather than a note.
  ([ADR-0181](../adrs/0181-the-studio-drives-one-player-and-the-show-loop-is-extracted.md))
- The listener MUST be **off unless asked for**. With neither `--control` nor
  `[control] enabled = true`, no socket is bound — not one that ignores traffic, none. (ADR-0176)
- The listener MUST bind **loopback** unless the operator names another host. Anything that can
  reach the port can move a parameter on the running show; that is the feature, and it is also
  the whole of the exposure, since OSC carries no authentication. (ADR-0176)
- Nothing on the control path MAY touch the audio thread, the ring or the analyzer. The listener
  decodes and enqueues; the render thread drains between frames. (CLAUDE.md non-negotiables)
- The listener thread MUST NOT allocate per packet after start. The decoded action carries names
  inline rather than heap-allocated, and the queue's capacity is reserved once at construction.
- The decoder MUST **accept only what it understands**. A well-formed message to an unknown
  address, a known address with argument types other than the ones its row declares, a name
  longer than the inline capacity, and a transport verb the table does not name are each
  refused. None of them is coerced, truncated, or mapped onto a neighbouring row.
- The decoder MUST be **total**: no byte sequence reaches a panic. It is handed whatever arrived
  on a socket.
- A refusal MUST be **counted, never answered individually** — OSC has no reply channel
  (ADR-0164). The running totals are reported in aggregate through the `health` event, and
  `ctl/ping` exists so a studio can tell a dead player from a quiet one.
- The queue MUST keep the **last value per parameter name**, so a frame's work is bounded
  whatever rate a sender uses. Transport verbs and pings are events rather than levels and keep
  their arrival order, up to a fixed cap; a message past any cap is dropped and counted, never
  grown into.
- A drained frame MUST be applied in one fixed order: transport, then preset, then
  `params/clear`, then per-name clears, then per-name values, then pings. A preset change drops
  every override, so the change has to precede the values; a wholesale clear has to precede the
  values that survive it.
- A `ctl/transport` verb MUST resolve the **same action** the operator console's transport strip
  resolves, and reach the same applier. Two surfaces that agreed by having two copies of the rule
  would drift. (ADR-0143)
- `auto` and `hold` are **positions, not presses**. Each is inert when rotation is already where
  it asks for. A control surface that repeats its state MUST NOT thereby toggle rotation off.
- An override MUST survive until it is cleared, and MUST be dropped by a preset change and by any
  preset reload — the file is the durable channel and the socket the live one, so a save wins.
  (ADR-0176)
- The event stream MUST be **off unless asked for**, and MUST be purely **additive** when it is on:
  the human diagnostics are byte-identical with `--events` and without it. An operator reading the
  console cannot tell the flag was passed.
- Every event line MUST begin with `{`, end with a newline, be one balanced JSON object, and carry
  `"v"` and `"ev"` as its first two fields. No human diagnostic line may begin with `{`, so a parent
  routes on the first byte with no framing.
- A preset name or a file path MUST be escaped, so one preset on disk cannot produce a line the
  parent cannot parse.
- `hello` MUST be the first **event**, and MUST carry this build's version, the schema document's
  hash and the control port **actually bound** — which is not the port requested when the operator
  asked for an ephemeral one. It is not necessarily the first *line*: it reports a port that comes
  from the operator config, which cannot be read until the per-user directory has been resolved and
  migrated, so that migration's own notice and the tier and input lines can precede it.
- `preset` MUST report the preset **on screen**, from one site, rather than from each of the six
  that can change it. A switch dissolves, so a site announcing its own would name the incoming
  preset a frame before it was drawn.
- `preset_error` MUST carry the file and the message, and the line and column **whenever the TOML
  parser provides a span**. An expression error carries the parameter name instead: it is raised
  after the document was parsed into values that no longer carry a position, so it has no span, and
  the missing position is reported as `null` rather than omitted.
- `health` MUST be tied to the drawn frame, so a stalled or hidden player goes quiet rather than
  reporting a stale figure on a timer of its own.
- The writer MUST NOT fail the show: standard error may be closed or full, and a lost line is a
  better outcome than a player that stopped drawing because a parent stopped reading.

## Scenarios

- WHEN neither `--control` nor `[control] enabled` is given THEN no address resolves, the
  listener's constructor is never reached, and no port is bound.
- WHEN a `ctl/param` datagram naming a parameter the active preset's system claims arrives THEN
  the next rendered frame routes that value, whether or not the preset binds that parameter.
- WHEN a `ctl/param` names a parameter nothing on the active system claims THEN the engine
  refuses it, the count rises, and no line is printed — a sender scrubbing a mistyped name at
  slider rate would otherwise produce a line per frame.
- WHEN `ctl/param/clear` arrives for a held name THEN the preset's own binding resumes on the
  next frame, easing from where its smoother actually was rather than from the held value.
- WHEN a `ctl/preset` names a preset the roster holds THEN the show **dissolves** to it, so the
  roster still names the outgoing preset for as long as the crossfade composites it. WHEN the
  name is unknown THEN nothing changes and no post-switch bookkeeping runs.
- WHEN ten thousand `ctl/param` datagrams for one name arrive between two frames THEN the render
  thread is handed one slot holding the last value the listener recorded.
- WHEN a truncated, over-long or mistyped datagram arrives THEN it is refused and counted, the
  queue is untouched, and the process continues.
- WHEN `ctl/transport auto` arrives while rotation is already on THEN nothing happens.
- WHEN a `ctl/ping` arrives THEN a `pong` carrying the same nonce goes out on the event stream —
  the one message answered individually, because it exists to tell a dead player from a quiet one.
- WHEN the player starts without `--events` THEN standard error carries no line beginning with `{`.
- WHEN the player starts with `--events` THEN the human diagnostics are exactly the lines the same
  run without it produced, and the events are interleaved among them.
- WHEN a preset fails to parse THEN a `preset_error` names the file and the line the author's
  mistake is on, so an editor can put a cursor there.

## Known gaps

- **`stream` is declared here and emitted by whichever sink carries the frames.** Its row is in
  the roster above because the geometry is part of this contract, and the numbers in it are the
  sink's: a requested size and rate from the headless video-out, the readback's size and the
  display's rate from a windowed run's `--preview stdout`.
- **Nothing asserts the whole ordering of a live run.** `hello` before the rest is asserted from a
  spawned process, and each event's own shape is asserted at the writer — but the sequence a
  ten-minute show produces is what the plan's on-device phase looks at instead.
- **No test drives a transport verb through a window.** The verb-to-action mapping is asserted
  against the strip's own `action_for` in `standalone/src/console/tests.rs`, and the applier it
  reaches is the one a click reaches — but the click-to-picture leg needs a window, which is
  what the plan's on-device phase covers instead.
- **The bounded-queue caps are stated behaviorally here**; the numbers (`PARAM_SLOTS` and its
  three siblings) live in `standalone/src/control.rs` beside the argument for each.
- **Nothing authenticates a sender.** Stated as an invariant above rather than left as a gap
  because it is a decision, not an omission: the mitigation is the loopback default, and a
  venue-network deployment that binds elsewhere has accepted it.

## Provenance

How this contract reached its current shape. None of it is a rule — the invariants above are —
and it is here so a reader who wants the history has it in one place rather than in the header.

**Written 2026-09-09**, from one plan:

- **[Plan 0158](../plans/done/0158-the-player-grows-a-studio-facing-surface.md)** built the listener,
  the decoder and the bounded queue as its second phase, against ADR-0176's vocabulary table, and
  the event stream as its third. Two places the implementation is narrower than that ADR's prose,
  both on purpose and both recorded above as invariants: `auto`/`hold` are **positions** rather
  than presses, because the console strip's rotation control is a toggle and a control surface
  needs to be able to state a position; and `hello` is the first *event* rather than the first
  *line*, because the port it reports cannot be known until the per-user directory has been
  migrated and the config read, and those steps have their own things to say.
