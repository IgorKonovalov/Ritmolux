# Configuration

Every command-line flag, every environment variable and every `config.toml` key the standalone
application reads, with what each is for and which one wins when two of them disagree. What the app
does once it is open is in [Running the app](running.md).

**`ritmolux --help` prints the flag roster and exits** — that is the authority on what this binary
accepts, and a test holds it in step with the scanners, so a flag that exists is a flag `--help`
names. This page says what each flag is *for*, which is the part a roster line has no room for, and
a test holds the page in step with the roster.

An argument no flag claims is a **startup error** that names it and the nearest spelling:
`ritmolux --ocs 127.0.0.1:9000` exits rather than starting a visualizer that publishes no
telemetry.

## The flags

| Flag | Value | What it is for |
|---|---|---|
| `--help` | — | Print the flag roster and exit |
| `--console` | — | Open the operator console at launch, on a display other than the show's |
| `--list-devices` | — | Enumerate audio capture endpoints and exit (Windows-only) |
| `--list-adapters` | — | Enumerate graphics adapters and exit, from both rosters |
| `--input` | `loopback` \| `line-in` | Where audio comes from (Windows-only) |
| `--device` | `"<friendly name>"` | Which capture endpoint to open |
| `--tier` | `floor` \| `rich` | Pin the quality tier instead of letting the engine pick |
| `--osc` | `<host:port>` | Publish analyzer telemetry as OSC over UDP, and turn the sink on |
| `--soak` | `[path]` | Write a long-run frame-time trace; bare, a default path |
| `--downbeat-log` | `[path]` | Write the per-beat downbeat decomposition; bare, a default path |
| `--stream` | — | Run headless and publish every frame as a Spout sender (Windows-only) |
| `--size` | `WxH` | Published frame size, default `1280x720`. Needs `--stream` |
| `--fps` | `<n>` | Published frame rate, default `60`. Needs `--stream` |
| `--sender` | `<name>` | The published Spout sender name, default `ritmolux`. Needs `--stream` |
| `--frames` | `<n>` | Stop after this many frames. Needs `--stream` |
| `--gpu` | `<name\|index>` | Which graphics adapter to render on — the window and `--stream` both |
| `--preset` | `<name>` | Hold one scene and disable rotation |

A flag marked *Needs `--stream`* passed without it is a startup error naming both flags, rather
than silence.

### The ones that need a paragraph

**`--help`** writes to stdout and creates no window, no GPU device and no capture client, so a
script can probe the flag surface without starting a show.

**`--console`** is a presence flag with no value: it turns the console **on** for this run and
never off, and it does not write itself into `config.toml` (the same shape `--input`, `--device`
and `--osc` follow). `[console] enabled = true` is the persistent form, and the `C` hotkey and the
settings menu's **Console** row are the same path — no two of the four can disagree about whether
the console is open.

**`--list-adapters`** prints **both** rosters: the one the renderer selects through and the one the
Spout sender selects through. They are separate enumerations and are not assumed to agree on order,
so both are printed with their own indices.

**`--gpu <name|index>`** works for both the window and `--stream`. **On a machine with one GPU you
will never need it; on a hybrid laptop it is the difference between a picture and nothing.** A
Spout sender shares a D3D11 texture by handle and the receiver opens it on its own device, which
works only when both are the same physical GPU — and Windows hands a plain console process the
power-saving GPU while the receiving application runs on the discrete one. One flag moves both
halves: the renderer and the sender each resolve the name against their own roster. Unset,
`--stream`'s renderer asks for the high-performance adapter and the sender follows it by name,
printing what both resolved to.

**The window's unset behaviour is deliberately different**: it asks for whatever the graphics layer
picks for the surface, which is what it has always asked for, so no published frame-time figure
moves because this flag arrived. On a hybrid laptop that default is the power-saving GPU, and
`--gpu <name|index>` is how you move the window onto the discrete one; the startup line in
`diagnostics.log` names the adapter and says whether a flag pinned it. A named adapter that cannot
drive the window is a startup error rather than a quiet fall-back to another GPU.

**`--preset <name>`** takes the preset's **display name** — `Clifford`, `Rose Window` — as the
browse overlay and `--preset`'s own error listing spell it, not the `.toml` filename, so most of
them need quoting. An unknown name is a startup error that lists the roster, and **no window
opens**. Hotkeys still browse, so this pins where a run *starts* and turns the dwell timer off.

**`--input loopback|line-in`** overrides `[input] mode`. `loopback` taps whatever the system is
playing; `line-in` captures an input endpoint (an audio interface, a mixer feed). A value that is
neither is a usage error and the app exits, the same way a bad `--tier` does. **The input also
moves while the app is running** — the settings menu's **Input mode** and **Input device** rows swap
the capture stream in place and write the choice to `config.toml`, so `--input` pins the launch
rather than the session. Expect a brief hitch on the swap (the old stream is stopped and the new
one opened synchronously), and, when the two endpoints negotiate different sample rates, a second
or two of re-adaptation while the level tracking rebuilds.

**`--device "<friendly name>"`** overrides `[input] device`. Copy a name out of `--list-devices`; a
substring is enough. **The two flags override independently** — `--device` alone keeps the
configured mode, `--input` alone keeps the configured device name. A name that matches no active
endpoint of the selected mode is *not* an error: capture falls back to that mode's default endpoint
and says so on stderr, because the interface being unplugged is a fact about the world rather than
a typo in the flag. Giving **no name at all** — a trailing `--device`, or `--device=` — *is* an
error: an empty value selects the default endpoint, which is the opposite of what naming a device
asks for. What happens when an endpoint disappears mid-show is in
[Running the app](running.md#when-the-input-goes-away).

**`--stream`** runs **headless** as a live video source and publishes every frame as a **Spout
sender**, for TouchDesigner (or any Spout receiver) on the same machine. No window, no swapchain,
no codec. Windows-only, and present only in a build with the `spout` feature — the release
`ritmolux.exe` has it; a plain `cargo build` does not, and `--stream` there fails with a named error
saying so. Presets rotate on the operator config's dwell timer exactly as they do in the window
(rotation is **on** here even where `[rotate] auto` is off, since a headless source has nobody to
press `Space`). Ctrl-C stops it and prints the run's frames, wall clock and scene clock. See
[Headless capture and video](capturing.md#the-live-video-out-ritmolux---stream) for the TouchDesigner
side.

**`--tier floor|rich`** pins the quality tier. Unpinned, the app starts on `rich` and a frame-time
governor demotes it to `floor` once if the display's frame budget is not being held. A pin is never
demoted, so this is also how you keep `rich` on a machine a transient stall demoted. The tier also
moves while the app is running — see [Quality tiers](running.md#quality-tiers).

**`--osc <host:port>`** both aims the sink and turns it on, so `enabled = false` in `config.toml`
cannot veto a target typed for this run. **Off unless you ask for it.** A target that will not
resolve is a usage error and the app exits — the same way a bad `--tier` does — whereas a stale
target in `config.toml` degrades to no sink and says so, because a config file must not stop a
show. Sends are **non-blocking and dropped on failure**: a broken link costs the telemetry, never a
frame, and the app prints one line when it starts failing and one when it recovers rather than a
line per frame.

## Environment variables

| Variable | Value | What it is for |
|---|---|---|
| `RLX_PRESET_DIR` | a directory | Read presets from here instead of the seeded per-user directory; edits to `*.toml` there hot-reload live |
| `RLX_TIER` | `floor` \| `rich` | The same pin as `--tier`, for a one-off run |

`RLX_PRESET_DIR` is read by the headless [`shot`](capturing.md) CLI as well as by the app, so a
capture and a live run resolve the same library.

## `config.toml`

A small per-user file under the same directory the presets live in — `%APPDATA%\Ritmolux\` on
Windows. It is read once at startup and written back whenever a hotkey or a settings row changes a
choice, so a stage setup survives a restart.

**Every key is optional.** A missing file, a missing section and an unknown extra key all degrade
to the built-in default rather than failing, so the file below is a complete listing rather than
something you have to write.

### `[output]`

Which display the show opens on, and whether it opens fullscreen.

| Key | Default | What it means |
|---|---|---|
| `display` | `0` | Target monitor index — the fallback when no `display_name` matches |
| `display_name` | unset | Preferred monitor identity, matched by name *before* the raw index. Unset means "use the index" |
| `fullscreen` | `false` | Open borderless-fullscreen on the target display; windowed otherwise |

The name is tried before the index because the window system's monitor ordering can shift across a
boot or a hotplug, so a stored index alone may point at the wrong screen.

### `[input]`

Where audio comes from. Windows-only; the macOS path taps system audio and takes no endpoint
choice.

| Key | Default | What it means |
|---|---|---|
| `mode` | `"loopback"` | `"loopback"` taps a render device (what the system is playing); `"line-in"` captures an input device |
| `device` | `"default"` | Friendly device name to capture. `"default"`, or a name matching no active endpoint, falls back to the selected mode's default endpoint |

### `[rotate]`

The scene director's auto-rotate policy.

| Key | Default | What it means |
|---|---|---|
| `auto` | `false` | Auto-rotate on the dwell timer; manual-only (`Space`) when off |
| `min_dwell_secs` | `20` | Never rotate sooner than this many seconds after the last change |
| `max_dwell_secs` | `90` | Always rotate by this many seconds, even through a steady passage |
| `track_change` | `true` | Let the track-change novelty signal nudge rotation in on the same dwell |

**Hold one scene by default.** Out of the box the app stays on a single scene until you opt in — the
`A` hotkey, or `auto = true` here. Manual `Space` works either way. When auto is on the defaults
favour a mostly-predictable, timer-led rotation: a steady passage holds to the 90 s cap and never
rotates before 20 s. An energy drop can land a change early, but only well past the minimum dwell
(a softened gate, around 37.5 s at these defaults), so it cannot flip scenes every few seconds.

### `[quality]`

| Key | Default | What it means |
|---|---|---|
| `tier` | `"auto"` | `"auto"` lets the engine resolve `rich` and demote it if the frame time says so; `"floor"` and `"rich"` pin it |

Precedence, highest first: `--tier`, then `RLX_TIER`, then `[quality] tier`, then auto. A pin made
from inside the app (`[`, `]`, or the settings menu) is written here, so it survives a restart —
and the two above still win at the next launch.

### `[hud]`

The furniture the shell paints over the show. Separate from `[output]` because it is about what is
*painted*, not about which screen the window opens on.

| Key | Default | What it means |
|---|---|---|
| `preset_name` | `true` | Draw the active preset's name in the top-left corner. Even when on, the name yields to a menu and to the `F3` panel — this is "never show it", not "show it always" |
| `now_playing` | `true` | Announce the current track in the lower-left corner when it changes. Off means no track ever reaches the visualizer, not a banner drawn transparent |

### `[osc]`

The lighting telemetry sink. **Off by default**, like every optional sink: a user who runs no
lighting rig must not have a socket bound or a datagram leaving their machine because they
installed the app.

| Key | Default | What it means |
|---|---|---|
| `enabled` | `false` | Publish telemetry |
| `target` | `"127.0.0.1:9000"` | Where to send, as `host:port`. Inert until `enabled` (or `--osc`) turns the sink on |
| `rate_hz` | `60` | Datagram sets per second; `0` means every rendered frame, whatever the frame rate |

`--osc` overrides `target` and turns the sink on, and leaves `rate_hz` to the file — the one key it
has no spelling for.

### `[console]`

The operator console's second window. **Off by default**, for the reason every optional surface
here is: a config that has never heard of a console produces exactly one window, one surface, no
intermediate render target and no extra copy per frame.

| Key | Default | What it means |
|---|---|---|
| `enabled` | `false` | Open the console at launch |
| `display_name` | unset | Preferred monitor identity for the console, matched by name before the index. Unset means "use the index" |
| `display` | `1` | Fallback monitor index when no `display_name` matches |

`display` defaults to **1, not 0**: a console's whole point is to be on a display other than the
show's, and the show defaults to `0`. On a single-monitor machine this falls back to the only
monitor there is, which is the correct degrade rather than a failure.

### A complete file

Every key at its default. `display_name` in `[output]` and `[console]` is absent because "unset" is
its default and the file format has no spelling for one.

<!-- The block below is round-tripped through the config type by
     `standalone/tests/configuration_doc.rs`; a value edited here without the code
     moving fails that test. -->

```toml
[output]
display = 0
fullscreen = false

[input]
mode = "loopback"
device = "default"

[rotate]
auto = false
min_dwell_secs = 20
max_dwell_secs = 90
track_change = true

[quality]
tier = "auto"

[hud]
preset_name = true
now_playing = true

[osc]
enabled = false
target = "127.0.0.1:9000"
rate_hz = 60

[console]
enabled = false
display = 1
```

## Precedence

A flag beats an environment variable beats the file, and nothing beats a change made inside the
running app *for that session*.

| Setting | Highest | | | Lowest |
|---|---|---|---|---|
| Quality tier | `--tier` | `RLX_TIER` | `[quality] tier` | auto |
| Preset directory | `RLX_PRESET_DIR` | | | the seeded per-user directory |
| Input mode | `--input` | | `[input] mode` | `loopback` |
| Input device | `--device` | | `[input] device` | the mode's default endpoint |
| OSC target | `--osc` | | `[osc] target` | — |
| OSC on/off | `--osc` (on) | | `[osc] enabled` | off |
| Console on/off | `--console` (on) | | `[console] enabled` | off |

`--input`, `--device`, `--osc` and `--console` pin a **run** and never write themselves into
`config.toml`; the file is the persistent form. There is no environment variable for the input
selection, because an input is a property of a rig and already persists to the config.

## OSC addresses

`--osc <host:port>`, or `[osc] enabled = true`, publishes the analyzer's telemetry as OSC over UDP
so a lighting console or a bridge can follow the music.

**The address space is versioned in the addresses**, so a *later signal* is additive under the same
`/rlx/v1` prefix and a mapping you have already bound keeps working across it. One argument per
address, so a console binds a parameter to an address rather than to a position inside a message.

| Address | Type | What it carries |
|---------|------|-----------------|
| `/rlx/v1/level/bass` | `f` | Bass level, peak-normalized to `0`–`1` |
| `/rlx/v1/level/mid` | `f` | Mid level, peak-normalized |
| `/rlx/v1/level/treb` | `f` | Treble level, peak-normalized |
| `/rlx/v1/level/onset` | `f` | Spectral-flux onset envelope, peak-normalized |
| `/rlx/v1/level/rms` | `f` | Broadband RMS of the waveform trace — **un-normalized**, unlike the four above it, because the trace it comes from deliberately is. Map it with a gain in the console |
| `/rlx/v1/raw/bass` | `f` | Raw mean magnitude in the bass band — the absolute twin of `level/bass` |
| `/rlx/v1/raw/mid` | `f` | Raw mean magnitude, mid |
| `/rlx/v1/raw/treb` | `f` | Raw mean magnitude, treble |
| `/rlx/v1/raw/onset` | `f` | Raw spectral-flux envelope |
| `/rlx/v1/beat/trigger` | `i` | `1` on a frame an onset fired, `0` otherwise — the discrete event |
| `/rlx/v1/beat/index` | `i` | Monotone count of onset detections. **Not a musical beat count** — the detector fires 1.2x–2.3x per beat depending on material, so no fixed multiplier turns it into bars. Useful as a ratchet, not as a meter |
| `/rlx/v1/beat/phase` | `f` | Beat phase in `[0, 1)`: `0` on each beat, ramping to the next |
| `/rlx/v1/tempo` | `f` | Tempo estimate in BPM, `0` until the tracker warms. Expect a warm-up of tens of seconds before it settles |
| `/rlx/v1/preset` | `s` | The active preset's name |

Telemetry rides the rendered frame, so it stops when the window is hidden and the preset name lags
a switch by one frame. Nothing here is a musical timebase you can drive a sequencer from — it is a
level feed for lights.

**The root moved in this release: `/lmv/v1` became `/rlx/v1`.** There is no transition period and
no dual-emit, and OSC has no error channel — so **a binding left on the old root stops firing and
reports nothing**, which looks exactly like a fixture that happens not to be moving. Re-point every
address by hand, and keep the old show file until each one is confirmed against a playing track.
**`/v1` did not move**, because no payload, type tag, address suffix, vocabulary or send cadence
changed: re-point the root, change nothing else, and the mapping is correct.

## macOS

Loopback capture **is** implemented — the standalone taps system audio through **ScreenCaptureKit**,
so it needs **macOS 13+** and the **Screen Recording** permission (that API will not run an
audio-only stream, so the capture carries a throwaway 2x2 px stub video alongside the audio). Grant
it, then **relaunch** — the app does not pick the permission up mid-run.

The caveat is that **this path has never run on Apple hardware**: CI compiles it on every push, but
no runner can play audio or drive a real Metal adapter, so the first live run is also its
validation. A window with visuals but no reaction to music means capture did not start, not a
crash — launch from Terminal to see the reason.
