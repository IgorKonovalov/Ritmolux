# Ritmolux Studio

The editor. It opens beside the player, paints the player's picture in its own
window, and edits the preset that produced it. **It never renders a frame of its
own** — every pixel here arrived over a pipe from a `ritmolux` child process
(ADR-0175).

Not shipped by the player's release zips, and nothing shipped depends on it.

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
   developer's route: point it at a `target/release/ritmolux` of your own.
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
