# Ritmolux Studio

The editor. It opens beside the player, paints the player's picture in its own
window, and edits the preset that produced it. **It never renders a frame of its
own** — every pixel here arrived over a pipe from a `ritmolux` child process
(ADR-0175).

Not shipped by the player's release zips, and nothing shipped depends on it.

## One player, in one of two modes

The studio spawns **one** player and drives it over `--control`. Which sink it
asks for is a **per-machine setting** (ADR-0186), offered under _settings_ in the
window's header and remembered in `settings.json` as `playerMode`:

| `playerMode`           | Spawned with             | What you get                                                  |
| ---------------------- | ------------------------ | ------------------------------------------------------------- |
| `windowed` _(default)_ | `--preview stdout`       | The player opens the show window; the preview is a copy of it |
| `windowless`           | `--stream --sink stdout` | No window; the preview is the only picture                    |

Both carry `--events --control 127.0.0.1:0`, both perform the whole event
roster, and the studio's frame reader branches on neither — it takes the
geometry and the channel order from the `stream` event in both modes.

**`windowed` is the default because the VJ with a projector is who the player is
for.** That child is the show _and_ the source of the picture in this window, so
what is edited is by construction what an audience is watching: one loopback
capture, one adapter, one preset directory, one rotation state.

`windowless` gives that guarantee up for the session, which is why the settings
panel says so in a line. It exists for the single-screen laptop where the show
window is a window in the way.

The mode is read when the player is spawned, so changing it takes effect on the
next launch rather than restarting a running show.

Two more things worth knowing before the first launch:

- **The preview has a fixed shape.** It is a scaled, letterboxed copy at 640x360
  (ADR-0187), not the show's own resolution, so resizing or fullscreening the
  show window does not move the pipe's geometry — and a studio pixel is not a
  show pixel. Judging a one-texel seam needs the show window.
- **The channel order is announced, not assumed.** `stream.format` is `rgba8` or
  `bgra8`; the windowed mirror carries whatever the swapchain negotiated. The
  studio swaps the two bytes when it is told to, and refuses an order it cannot
  name rather than painting a guess. Nothing under `studio/` names a size, a
  rate or a channel order.

## An edit makes a copy, and rotation is held

**The studio never writes a preset it did not create** (ADR-0189). The first
gesture against one - a slider release, a palette edit, a structure or map key,
`Ctrl+S` - is held: the strip above the panel asks what to call the copy, the
whole document lands in the watched directory under that name, and the editor
switches to it. Every gesture after that writes the copy silently, one write per
gesture, which is the live loop the studio is for.

**The session is the unit.** The studio has nowhere durable to remember what it
authored, so reopening yesterday's copy asks once more and makes a second one.
That is one extra file, and no edit is lost paying for it.

A preset from the **embedded** set takes the same path. It has no file, so the
copy is its first one - built from the engine's declarations for the system on
screen plus the edit, because no event carries the embedded document's own text
and the studio resolves nothing of its own (ADR-0184).

**Rotation is held for as long as the studio is attached.** `ctl/transport hold`
goes out on every attach and the header says so, with a control that gives
rotation back. Without it the preset under the editor changes by itself at the
dwell the operator config sets, and an edit lands in whatever arrived last.
Repeating the hold is safe by contract: `auto` and `hold` are positions rather
than presses (`docs/specs/0003-studio-control-protocol.md`).

## Which configuration covers which directory

| Directory                      | Process   | Bundler                            | TypeScript project        |
| ------------------------------ | --------- | ---------------------------------- | ------------------------- |
| `electron/` (minus `preload/`) | main      | esbuild → `dist/main/index.cjs`    | `tsconfig.main.json`      |
| `electron/preload/`            | preload   | esbuild → `dist/preload/index.cjs` | `tsconfig.preload.json`   |
| `renderer/`                    | renderer  | Vite → `dist/renderer/`            | `tsconfig.renderer.json`  |
| `shared/`                      | all three | —                                  | every project includes it |
| `**/*.test.ts(x)`              | —         | Vitest                             | `tsconfig.test.json`      |

`npm run typecheck` runs all four with `--noEmit`. A Node type error in a
renderer file is the boundary catching a real bug: the renderer never imports
Node, and ESLint refuses the import as well.

## Commands

| Command             | What it does                                                               |
| ------------------- | -------------------------------------------------------------------------- |
| `npm run dev`       | Watches all three bundles and opens the window against the Vite dev server |
| `npm run build`     | Builds main, preload and the renderer once                                 |
| `npm start`         | Opens the window against the built files                                   |
| `npm test`          | Vitest                                                                     |
| `npm run lint`      | ESLint                                                                     |
| `npm run typecheck` | All four projects                                                          |

## Finding the player

In this order (ADR-0178), first hit wins:

1. **Bundled** — `resources/player/ritmolux[.exe]` inside a packaged studio, so
   a tester who unzips one configures nothing.
2. **Settings** — `"playerPath"` in `settings.json` in the per-user application
   directory (`%APPDATA%/ritmolux-studio` on Windows,
   `~/Library/Application Support/ritmolux-studio` on macOS). This is the
   developer's route: point it at a `target/release/ritmolux` of your own. The
   same file carries `"playerMode"`; the settings panel writes it, and an unset
   or unrecognised value reads as `windowed`.
3. **`PATH`**.

A player whose `hello` reports a version the studio does not know is **stopped**
and the window says so, rather than driving it and showing a picture from an
engine the panels were not generated from.

## Seeing the picture without a person at the screen

`--capture <file> [--capture-after <ms>]` saves one PNG of the window and quits.
Unpackaged only. The preview's last leg is a compositor, so no assertion reaches
the pixels; this produces the same evidence a person would otherwise be asked to
describe.

```
npm run build
npx electron . --capture ../target/studio-shots/now.png --capture-after 10000
```

## What the studio may and may not do

- Three domain IPC channels carry every player message and there are only three:
  `player:ctl`, `player:event`, `player:frame`. A fourth is a widening of the
  protocol, which is settled in `docs/specs/0003-studio-control-protocol.md`
  before it is code.
- The renderer makes **no network request of any kind** — `connect-src 'none'`.
- The security defaults (`contextIsolation`, `sandbox`, no `nodeIntegration`)
  and both CSP sites are asserted by `electron/window.csp.test.ts`. None of them
  is relaxed to make a library work.
