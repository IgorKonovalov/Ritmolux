# ADR-0176 — The player is driven over OSC control-in, and reports on its standard streams

> **Status:** accepted 2026-09-10 (Plan 0158)
> **Date:** 2026-09-09
> **Related plan(s):** [0158 — The player grows a studio-facing surface](../plans/done/0158-the-player-grows-a-studio-facing-surface.md)

## Context

ADR-0175 makes the studio a separate process that never renders, so everything it does reaches
the player over some channel. The channel carries two kinds of traffic with different needs.

**Studio to player** is mostly *nudges at slider rate*: set this parameter to this value, now,
thirty times a second while a drag is in progress; then select a preset, step the transport,
load a project. A dropped nudge costs nothing because the next one replaces it. Ordering matters
only within one parameter. Latency matters a great deal: a drag that lands 150 ms late feels
broken, and the reload path the preset watcher offers today is exactly that late and also snaps
the smoothers and resets the latch bank on every save (`configure_active_scene` in
`core/src/render/roster.rs`).

**Player to studio** is *facts the studio cannot see otherwise*: the player's version and the
schema it was built with, the active preset and the roster, a preset that failed to load and
where, a warning about an unknown parameter name, frame health, the geometry of the preview
stream. These must not be dropped, they must be attributable to the player instance the studio
spawned, and they must arrive in order.

Three facts about the tree shape the answer. The player already emits OSC under a versioned
`/rlx/v1` root (ADR-0164) through a hand-written encoder, so a decoder is a mirror of existing
code rather than a dependency, and a control vocabulary under the same root inherits the same
versioning rule. The player has never listened on a socket, and a listener is a new attack
surface on a machine that is also running a show. And the player is a console-subsystem
executable whose standard streams are real pipes when a parent spawns it, which the studio
always does.

## Decision

**Studio to player is OSC over UDP, opt-in, on a loopback listener.** `--control <host:port>`
or `[control] enabled = true` opens one UDP socket, bound to `127.0.0.1` unless the operator
names another host, and the player applies each message on the render thread between frames.
The vocabulary lives under **`/rlx/v1/ctl/`**, one message is one action with typed arguments,
and it is versioned exactly as telemetry is: a later action is additive under the same prefix,
and a breaking change moves the version segment. The decoder is hand-written beside the encoder
in `standalone/src/osc/`, accepts only what it understands, and counts what it rejects. The
receive thread hands decoded actions to the render thread through a bounded queue and never
touches the audio path.

**Player to studio is a structured event stream on standard error.** `--events` makes the
player print one JSON object per line, each line beginning with `{`, carrying a `"v": 1` field
and an `"ev"` name. Human diagnostics keep their existing lines, which never begin with `{`, so
a parent can split the two by the first byte. The writer is the same hand-rolled JSON the `shot`
reports use, so no JSON crate enters the player. Standard output is reserved for **bulk binary
payloads** the studio asks for, which is the preview frame stream from the frame tap's pipe sink.

**The vocabulary and the event roster are a contract**, recorded as a living spec
(`docs/specs/0003-studio-control-protocol.md`) that Plan 0158 writes as the surface lands, in the
shape `docs/specs/README.md` prescribes. Widening either is an ADR-worthy event, as the C ABI is.

The first vocabulary, illustrative of shape rather than final:

| Address | Args | Action |
|---------|------|--------|
| `/rlx/v1/ctl/param` | `s name`, `f value` | Override `name` on the active preset with `value`, in place, until cleared |
| `/rlx/v1/ctl/param/clear` | `s name` | Drop that override; the preset's own binding resumes |
| `/rlx/v1/ctl/params/clear` | none | Drop every override |
| `/rlx/v1/ctl/preset` | `s name` | Dissolve to the named preset |
| `/rlx/v1/ctl/transport` | `s next\|prev\|auto\|hold` | The console's transport, by name |
| `/rlx/v1/ctl/ping` | `i nonce` | Answered by a `pong` event carrying the nonce |

The first event roster, in the same spirit: `hello` (version, schema hash, control port), `preset`
(active name and index), `roster` (the names, on every reload), `preset_error` (file, message,
line and column when the parser provides them), `preset_warning`, `health` (once a second: fps,
frame-time p50 and p99, dropped frames), `stream` (width, height, fps, pixel format, sent once
before the first frame on standard output), and `pong`.

## Consequences

### Positive

- **A drag is a UDP datagram, not a file write and a reload.** The override lands on the next
  frame, keeps the scene's accumulated state, and clears itself when the studio saves the file.
- **The player is open to every OSC tool at once.** TouchDesigner, a lighting console, a MIDI
  bridge, and the studio all speak the same addresses. MIDI, which NFR §10 left as "worth
  exploring", becomes a bridge onto this vocabulary rather than a second control path.
- **No new dependency in the player.** A decoder beside an encoder, a JSON writer that exists,
  and two standard streams.
- **The event stream is attributable and ordered by construction.** A parent reads its own
  child's stderr; there is no port to collide on and no second player's events to confuse it.

### Negative

- **OSC has no acknowledgement and no error channel**, which ADR-0164 already records for
  telemetry. A control message the player did not understand is counted and reported in
  `health`, never answered individually. The `ping` action exists so a studio can tell a dead
  player from a quiet one.
- **The player opens a socket.** Off by default and loopback by default, but a listener is new.
  Binding elsewhere is the operator's explicit choice and is documented as such.
- **A malicious or buggy sender can nudge parameters at any rate.** The render thread drains a
  bounded queue once per frame and keeps the last value per parameter, so the cost of a flood is
  bounded and the audio path never sees it, but the picture can be made to do anything the
  preset allows. That is the feature.
- **Two channels means two versions to keep aligned.** The vocabulary version is in the address;
  the event version is a field. `hello` carries both, and the studio refuses a pair it does not
  know.

### Neutral

- Standard output as a binary channel means the player must never print prose there while a
  stream is open. Today it does not; a test holds it.

## Alternatives considered

### Alternative A — One JSON request-and-response protocol over a local socket
Cleaner for schema queries and validation round trips, and it would carry both directions on one
connection. Rejected because it buys a JSON codec and a framing layer the player does not have,
answers a question the studio can ask by spawning `ritmolux --schema` once, and gives an external
controller nothing: a lighting console speaks OSC, not a bespoke JSON dialect.

### Alternative B — Files only
The studio writes presets and projects; the player watches and reloads. Zero protocol, and it is
half of the design already. Rejected as the *whole* design because a reload is 150 ms late,
snaps the smoothers and resets the latches, so a slider drag would stutter and jump. The file is
the durable channel; the socket is the live one.

### Alternative C — Commands on standard input
Reliable, ordered, and needing no socket. Rejected as the primary channel because it reaches only
a parent process: a controller on the same desk that is not the studio would have no way in.
Nothing prevents the same vocabulary being accepted on stdin later if a parent-only channel is
ever wanted.

### Alternative D — Events over OSC too
Symmetric, and telemetry already leaves that way. Rejected because events must not drop, must be
attributable to one player instance, and carry strings a datagram fits badly: a preset error
with a message and a path is not a level for a fixture.

## Notes

The vocabulary table above is deliberately short. Project loading, cue stepping, and render
control each arrive with the plan that needs them, and the spec is the place they are recorded;
this ADR fixes the shape, the transport, and the versioning rule.
