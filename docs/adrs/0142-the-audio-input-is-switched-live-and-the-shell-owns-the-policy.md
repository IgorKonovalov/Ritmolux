# ADR-0142 — The audio input is switched live, and the shell owns the policy

> **Status:** proposed
> **Date:** 2026-08-28
> **Related plan(s):** [0130](../plans/0130-the-audio-input-becomes-an-operator-surface.md)

## Context

The standalone has had a working input selector since Plan 0009 Phase 2: `[input] mode` picks
`loopback` (tap a render endpoint) or `line-in` (capture an input endpoint), `[input] device`
names the endpoint, and `--list-devices` prints both rosters so a friendly name can be copied in.
It works — verified 2026-08-28 on `Line (ZOOM AMS-22 Audio)`, which negotiated 44.1 kHz where the
loopback path had been running at 48 kHz.

What it is not is *reachable*. The config is read once, before the window exists
(`standalone/src/main.rs:1622`), and there is no flag, no hotkey and no menu row. Changing input
means quitting the show, finding `%APPDATA%\light-music-visualizer\config.toml`, hand-editing
TOML, and relaunching. Every other operator-facing choice this app has — quality tier, auto-rotate,
dwell bounds, fullscreen, display, diagnostics, the two HUD toggles — is live on the `S` menu and
persisted on change. Input is the one that is not, and it is the one an operator is most likely to
get wrong at load-in: the difference between "the visualizer is broken" and "it is listening to the
wrong endpoint" is invisible from the stage.

Two constraints make this a decision rather than a chore.

**The capture thread cannot own the policy.** It is this app's audio callback and obeys NFR §5 —
after stream start it allocates nothing, locks nothing, logs nothing, touches no file. Endpoint
enumeration is COM: it allocates, it blocks, and its friendly-name strings are built once at setup
*before* the loop precisely so they never violate that discipline. Anything that decides *which*
device to run therefore lives somewhere else.

**Restarting capture is not free, and it changes a value the analyzer was built from.**
`CaptureHandle::drop` joins the polling thread; `capture_win::start` blocks until the stream is
live. `Analyzer::new(capture.format)` is called once at startup (`standalone/src/main.rs:249`), and
the two endpoints on the development box negotiate different sample rates — so a swap that crosses
44.1/48 kHz has to rebuild the analyzer, discarding the AGC's running peak and the tempo tracker's
history. That is a real cost paid at a real moment.

There is also a premise to retire. Plan 0083 built `CaptureVerdict` as a value "decided once, at
startup on the render/UI thread, rendered into a short token there, and then only *borrowed*"
(`standalone/src/capture_verdict.rs:8`), so the `diagnostics.log` row builder and the F3 overlay
could not disagree about a run. Once the input can change mid-run, "the verdict of this run" is no
longer a single fact.

Finally, the failure this makes visible already exists. `run_capture_loop` breaks its packet loop
on *any* error from `GetNextPacketSize` / `GetBuffer` / `ReleaseBuffer`, sleeps `POLL_INTERVAL`, and
loops again — indefinitely (`standalone/src/capture_win.rs:380`). Unplugging the interface today
produces a thread that spins forever delivering nothing, a silent picture, and no signal anywhere
that says why. An operator surface for input that still cannot say "your input went away" would be
half a feature.

## Decision

We will make the capture stream restartable at runtime, with **the shell owning every policy
decision and the capture thread owning none**. The render/UI thread performs the swap
synchronously — stop the old stream, start the new one, rebuild the `Analyzer` only when the
negotiated `AudioFormat` differs, re-render the capture verdict — and it is the only place that
decides which endpoint to run, whether to fall back, and when to stop trying. The capture thread's
sole new responsibility is to *report*: it sets one `AtomicBool` when its stream has died, which is
allocation-free, lock-free and log-free, and therefore inside NFR §5.

`CaptureVerdict` stops being a startup value and becomes **current state**, re-rendered on every
swap. Plan 0083's "decided once, then only borrowed" is superseded on the "once" half only; the
half that matters — one stored token, borrowed by both surfaces, so the log and the overlay cannot
disagree — is preserved exactly, because the token is still rendered in one place and stored.

Selection resolves at launch by the precedence `--input` / `--device` > `[input]` in `config.toml`,
mirroring ADR-0045's `--tier` > `LMV_TIER` > `[quality] tier` in shape, minus the environment
variable. A bad flag is a usage error that exits non-zero, as `--tier` already is; a config value
naming an endpoint that is not present degrades to the mode's default endpoint, as it already does.

Device loss recovers through the same swap path as an operator keypress: the shell observes the
reported flag once per frame, restarts on the mode's **default** endpoint, and surfaces the
fallback in the verdict. Recovery is attempted a bounded number of times and then stops, because
the failure mode being guarded against is a COM call per frame against an audio subsystem that is
not coming back.

## Consequences

### Positive

- An operator can audition inputs from the `S` menu during a show, and the choice persists to
  `config.toml` like every other row. The most likely load-in mistake becomes a two-keystroke fix
  instead of a restart.
- `--input` / `--device` make input scriptable, which is what a launcher shortcut or a second
  machine profile needs, and what `--list-devices` has implied was possible since Plan 0009.
- Unplugging an interface mid-show stops being silent. Today it is a picture that dies with no
  explanation on any surface; after this it recovers to the default endpoint and says so.
- The verdict becoming current state makes `diagnostics.log` answer "what is it listening to *now*",
  which is strictly more useful to a remote tester than "what did it start on".
- The capture thread's real-time discipline is not merely preserved but *narrowed*: it gains a flag
  store and no decisions.

### Negative

- **A swap is a synchronous hitch on the render thread.** Joining the poll thread plus WASAPI device
  activation is tens of milliseconds and can be worse on a cold device. It lands on a deliberate
  keypress, which is the only reason it is acceptable — and it is not acceptable anywhere else, so
  the recovery path inherits a hitch that the operator did *not* ask for.
- **A cross-rate swap discards analyzer state.** The AGC's running peak and the tempo estimate
  restart, so the picture re-adapts over a second or two. There is no way to carry that state across
  a rate change honestly, and pretending to would be worse than the visible re-adaptation.
- **A rate change is now reachable by keypress**, which promotes design-backlog 0032 from theoretical
  to operable: both analysis windows are sized in samples, so a 96 kHz interface loses a third of the
  band axis to one-bin resolution. This ADR does not fix that; it makes it easier to hit.
- **Endpoint enumeration becomes a thing the shell must schedule.** `settings_view()` runs every
  frame the modal is up (`standalone/src/main.rs:842`), and COM enumeration there would be a
  per-frame allocation and block on the render thread. The roster must be cached, which is state the
  shell did not previously hold and which can go stale against reality.
- **Bounded retry is a policy with no obviously right constant.** Too few and a device that returns
  slowly is missed; too many and a dead subsystem stutters the show.
- Two more `SettingsRow` variants and four more `SettingsView` fields, on platforms that cannot use
  either.

### Neutral

- The rows exist on macOS and Linux as read-only, which is the treatment the `Presets` row already
  gets. The menu keeps one shape everywhere, and a Mac user is told why the value cannot move
  rather than left to wonder where the row went.
- `core/` is untouched. Capture is a shell concern by ADR-0001 and stays one; nothing here reaches
  past `Analyzer::new`.

## Alternatives considered

### Alternative A — Selection edits config only, applying at next launch

The `S` rows would write `[input]` and display `(next launch)`, exactly as a settings file editor
would. This costs nothing: no hitch, no analyzer rebuild, no verdict lifetime change, no recovery
path. It lost on the single question the feature exists to answer — *is it listening to the right
thing?* — which cannot be answered without hearing it. An operator who must restart to find out
will keep hand-editing TOML, because a restart is the expensive part either way, and the row would
be furniture. The whole cost of this ADR is buying the audition.

### Alternative B — A supervisor thread owns the capture lifecycle

A third thread would hold the handle, take swap requests over a channel, and keep the join and the
device activation off the render thread — no hitch at all. Rejected because the hitch is not the
expensive half: the `Analyzer` rebuild lives on the render side and would have to be marshalled
back, so the seam buys a smooth frame during a deliberate keypress and pays for it with a second
concurrency boundary in the one subsystem where this project has been strictest about having
exactly one (the ring buffer). One perceptible hitch on a keypress the operator just made is a
better trade than a permanent structural seam.

### Alternative C — Recovery policy inside the capture thread

The thread already knows its stream died; letting it re-enumerate and reopen the default endpoint
is the shortest path in lines of code. Rejected outright by NFR §5: enumeration allocates and
blocks, and friendly-name resolution allocates strings. This is the exact discipline
`capture_win.rs`'s module header was written to defend, and the fact that the thread is *already*
looping on a dead device is not a licence to give it more to do — it is the argument for having it
say so and stop.

### Alternative D — Hot-plug through `IMMNotificationClient`

Windows' supported mechanism: register a COM notification client and receive device add/remove/
default-change events. It is strictly more capable than polling a flag — it would catch "the
default endpoint changed underneath us", which this design cannot see. Rejected for this plan
because it delivers callbacks on an arbitrary COM thread that must then be marshalled into the
render loop, which is a second reporting seam built before the first one has shipped, and because
the case that actually strands a show is *the stream we are holding died*, which the flag covers at
the cost of one relaxed atomic load per frame. Named here as the upgrade path, not as a rejection
on merit.

## Notes

- Verified live 2026-08-28: `--list-devices` reports two render and two capture endpoints on the
  development box; `[input] mode = "line-in"` with `device = "Line (ZOOM AMS-22 Audio)"` produced
  `live WASAPI 44100/2` with moving band levels, against `live WASAPI 48000/2` and all-zero bands on
  the loopback rows immediately above it in the same `diagnostics.log`.
- The 44.1/48 kHz split on one machine is what makes the analyzer rebuild a certainty rather than an
  edge case, and it is why the swap path cannot assume the format is stable.
- ADR-0045 is the precedent for the launch precedence chain and for a menu row that writes back what
  it changed; Plan 0083 is the premise this supersedes on its "once" half.
