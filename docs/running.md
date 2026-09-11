# Running the app

What the standalone application does once it is open: the keys, the two menus, the second window
you drive a show from, the track banner, and the two things that change how it looks while it is
running. Everything here is the running app — the flags and the `config.toml` keys that decide how
it *starts* are in [Configuration](configuration.md).

## Starting it

A [release download](https://github.com/IgorKonovalov/Ritmolux/releases/latest) unzips and runs;
nothing installs into the system, and the READ-ME-FIRST file in each zip walks the first launch on
[Windows](../packaging/windows/READ-ME-FIRST.md), [macOS](../packaging/macos/READ-ME-FIRST.md) and
[foobar2000](../packaging/foobar/READ-ME-FIRST.md).

From a source checkout you need a recent stable **Rust** toolchain (the workspace is edition 2024 —
Rust 1.85+). From the repo root:

```sh
cargo run -p standalone --release
```

That builds and launches `ritmolux`, the standalone window. **On Windows it captures whatever is
already playing** (system audio, via WASAPI loopback) — start some music, and the visuals react.
`--release` is recommended: this is real-time graphics, and the debug build is noticeably slower.

## Controls

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
| `Tab`     | Open/close the preset browser — opens on the preset you're watching. Arrow keys walk the list and wrap at both ends, left/right step a column, holding an arrow scrolls, type to filter, `Enter` selects (also dissolves), `Esc` closes |
| `S`       | Open/close the settings menu — quality, auto-rotate, dwell bounds, fullscreen, display, diagnostics, input mode, input device, preset name, now playing, console. Up/down pick a row, left/right change it, `Esc` closes. Every change applies immediately and (except diagnostics) is written to `config.toml` |
| `C`       | Open/close the **operator console** — a second window on another display carrying the browser, the settings menu, a transport strip and a live preview of the output |
| `[` / `]` | Drop / raise the quality tier live — pins it for the session and persists the choice |
| `F`       | Toggle fullscreen                                           |
| `Esc`     | Leave fullscreen (with no menu open). Does nothing in a window, and never quits |
| `D`       | Cycle to the next display/monitor                           |
| `F3`      | Toggle the diagnostics overlay                              |

## The browser and the settings menu

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

## The operator console

`C` opens a second window on a display **other than the show's**, so you can drive the
app from a desk without typing at the projector. It carries the preset browser and the
settings menu — while it is open, neither of those draws on the show any more — plus a
**transport strip** (`prev`, `next`, `rotate`, `auto`, `dwell -/+`, clickable), a line
naming **what the rotation will take next**, and a **live preview of the output**
letterboxed in the corner.

It is **off by default and costs nothing while closed**: no second surface, no
intermediate render target and no extra copy per frame. Open it from the settings
menu's **Console** row, with `C`, or at launch with `--console` / `enabled = true`
under `[console]` in `config.toml`; all four are one path, so they cannot disagree
about whether it is open. The console picks its display by the same name-over-index
rule the show's own display uses — `[console] display_name` first, then
`[console] display` — and where that lands on the screen the show is on, it moves to
another if there is one. Closing it from its own settings menu leaves that menu on the
show, so you never lose the menu along with the window.

On a single-monitor machine it opens as an ordinary window on that monitor, which is a
supported way to work rather than an error.

**An open console costs the output nothing outside measurement noise.** That was measured across
five combinations of the two pacing keys — `frame_latency` and `present_every_n`, both documented
in [Configuration](configuration.md) — and three frame-time regimes on this project's development
box, so the defaults are the shipped ones and there is no tuning to perform. The
`diagnostics.log` line `console opened:` names the mode, the frame latency **after clamping** and
the cadence actually in force, and a periodic `console open:` note carries the presented / skipped
/ decimated totals, which is what makes a cost reading on some other machine believable rather
than merely low.

### Mirroring the show to another program

`--preview stdout` sends the frames the projector is showing to whatever spawned the player, as
raw pixels on standard output. It is how an editor watches the **real** show rather than a second
headless one. The mirror is a **scaled, letterboxed copy at a fixed size** — 640x360 unless
`--preview stdout@WIDTHxHEIGHT` names another — so resizing or fullscreening the window never
changes what the reader receives, and the channel order it carries is named in the `stream`
event's `format` field rather than assumed. The frame is drawn to its real destination unchanged;
the copy for the mirror is taken beside it. The full comparison with the headless stream is in
[`docs/capturing.md`](capturing.md).

**The display loop never waits for it.** The staging copy rides the frame's own submission and its
mapping is taken on the **next** frame with a non-blocking poll, so a frame that is not ready yet
costs the show nothing at all. The mirror is therefore one frame behind the projector, which is
the whole price of never stalling.

**It drops rather than stalls.** A reader slower than the show loses frames; the show carries on.
That is the opposite of the headless `--sink stdout`, which blocks — a headless loop has no
present deadline to miss and this one does.

It needs no console, and closing the console does not take it away: the two are separate consumers
of one intermediate. The geometry is the **output's**, announced before the first frame by the
`stream` event on standard error (with `--events`), so a reader knows how to cut the pipe up.

**What it costs is written down, not assumed.** On exit, and when the console closes,
`diagnostics.log` gains a `preview` note naming the regime, the show's frame-time p50 and p99, and
— when the mirror was on — how many frames it wrote and how many it dropped. The frame counts are
what make the reading a reading: a run reporting a frame time and zero frames written measured a
mirror that never delivered. Comparing a run with the flag against one without it is what prices
the feature; the note names its regime so the two are never confused.

## Now playing

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

## Quality tiers

The engine renders at one of two tiers, `rich` and `floor`, and `floor` is the one sized for an
integrated GPU. Unpinned, the app starts on `rich` and a frame-time governor demotes it to `floor`
**once** if the display's frame budget is not being held — announced on stderr and marked with a
`*` in the `F3` overlay.

`[` and `]` move the tier while the app is running, as does the settings menu's **Quality** row. An
in-app change **pins** the tier for the session, so the governor stops touching it, and writes the
choice to `config.toml`. That is also how you keep `rich` on a machine a transient stall demoted.
Expect a brief re-accumulation of trails and feedback when it switches: the tier sizes GPU
resources, so changing it rebuilds them.

A tier can also be pinned before launch, and what wins there is
[the precedence in Configuration](configuration.md#quality). The measured budgets each tier is held
to are in [Non-functional requirements](nfr.md).

## Displays and fullscreen

`D` cycles the window to the next monitor and `F` toggles borderless fullscreen on the one it is
on; `Esc` leaves fullscreen and never quits. Both write themselves to `config.toml` under
`[output]`, so a rig set up once opens the same way tomorrow.

A monitor is remembered **by name before index** — `[output] display_name` first, then
`[output] display` — because the window system's monitor ordering is not stable across a reboot or
a hotplug, so a stored index alone can point at the wrong screen.

## When the input goes away

If the capture endpoint disappears mid-show — the interface is unplugged, the driver resets — the
app says so and reopens on that mode's default endpoint, a few times and then no more. `F3` and the
`capture` column of `diagnostics.log` name what it fell back to, or say `lost …` if nothing worked.

Re-plugging does **not** restore the device, and that is why a recovery is the one input change
that is *not* written to `config.toml`: your `[input] device` still names the interface you chose,
so the next launch goes back to it. Pick it again from the `S` menu to return to it in this run.
