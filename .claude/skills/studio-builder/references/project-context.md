# studio-builder — project context

Read on demand, once a task is concrete. On any conflict the ADRs win: ADR-0178 for the shell,
ADR-0176 for the protocol, ADR-0175 for what the studio is.

## What the studio is, in one paragraph

A separate Electron application under `studio/` that **drives the lean player and never draws a
frame**. It spawns `ritmolux` headless, paints the frames the player pipes back, renders
editing panels from the schema the player exports, sends parameter nudges over OSC, and saves
preset files the player already watches. The player is unchanged for anyone who does not edit;
the studio's whole cost lands on its own zip.

## Layout (ADR-0178)

| Directory | Owns | tsconfig |
|-----------|------|----------|
| `studio/electron/` | Main process: player supervisor, OSC sender, event and frame readers, IPC handlers, window, CSP | `tsconfig.main.json` |
| `studio/electron/preload/` | The `window.api` bridge, per-domain modules under `preload/api/` | `tsconfig.preload.json` |
| `studio/renderer/` | React + Vite: views, components, hooks, the CodeMirror editor, the preview canvas | `tsconfig.renderer.json` |
| `studio/shared/` | `protocol.ts` (the `CtlAction` / `PlayerEvent` unions + Zod), `ipc-channels.ts`, schema types | every tsconfig |
| `studio/scripts/` | esbuild scripts for main and preload | n/a |
| `studio/tests/` | Playwright golden path: window opens, player spawns, first frame paints | n/a |

Filenames: `PascalCase.tsx` components and views, `useCamelCase.ts` hooks, `kebab-case.ts` shared
and main-side modules. Match what is already there.

## Canonical commands

All from `studio/`:

```sh
npm install                # exact pins; the lockfile is committed
npm run dev                # esbuild watchers + Vite + Electron against the dev server
npm run build              # dist/main, dist/preload, dist/renderer
npm run typecheck          # all four tsconfigs, --noEmit
npm run lint               # eslint over electron/, renderer/, shared/
npm test                   # vitest: renderer + shared, and main in node environment
npm run test:e2e           # playwright, needs a prior build
npm run package:win        # electron-builder, zip target, player bundled under resources/player/
npm run package:mac        # same, ad-hoc signed through packaging/macos/
```

The player the studio drives is built by cargo: `cargo build -p standalone --release` produces
`target/release/ritmolux(.exe)`. In development the studio finds it through its settings path or
`PATH`; a packaged studio carries it at `resources/player/`.

## What the player provides, and where it is documented

The studio keeps **no copy** of any of these. Read the source each time.

| Thing | Source of truth |
|-------|-----------------|
| Control vocabulary (`/rlx/v1/ctl/*`) and the event roster | `docs/specs/0003-studio-control-protocol.md` |
| The wire shapes' reasoning | ADR-0176 |
| The schema document | `ritmolux --schema` at runtime; its two halves are described in Plan 0158 Phase 4 |
| The frame stream (`--stream --sink stdout`) | `docs/capturing.md`, and the `stream` event announces the geometry |
| The preset file format and the expression grammar | `presets/README.md`, `docs/presets.md`, `docs/preset-palettes.md` |
| Flags and `config.toml` keys, including `[control]` | `docs/configuration.md` |
| What a `v*` tag ships | ADR-0038, `docs/releasing.md` |

## Sibling lanes

| Lane | Owns | You touch it when |
|------|------|-------------------|
| `architect` | `docs/` — plans, ADRs, specs, the close review | any protocol widening, new IPC channel, CSP change, library adoption; a plan phase is wrong |
| `dev` | all Rust and C++ — the player side of the protocol, the schema export, the pipe sink, the release workflow | the player lacks an event, action or flag you need (via `architect`) |
| `preset-author` | preset content | never directly; its friction with the studio lands in `docs/design-backlog.md` |

## Current state

Written 2026-09-09, at the plans' draft. Nothing under `studio/` exists yet. Plan 0158 lands the
player side first; Plan 0159 Phase 1 creates `studio/`. Until Phase 1 lands, Mode 1 and Mode 3
have nothing to build on and say so.
