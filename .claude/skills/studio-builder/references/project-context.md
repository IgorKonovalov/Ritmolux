# studio-builder — project context

Read on demand, once a task is concrete. On any conflict the ADRs win: ADR-0178 for the shell,
ADR-0176 for the protocol, ADR-0175 for what the studio is, and the later studio ADRs
(0183, 0184, 0186, 0187, 0189, 0190) for what they each amended.

## What the studio is, in one paragraph

A separate Electron application under `studio/` that **drives the lean player and never draws a
frame**. It spawns one `ritmolux` — windowed by default, mirroring the show window's frames, or
windowless on a machine set that way (ADR-0186) — paints the frames the player pipes back, renders
editing panels from the schema the player exports, sends parameter nudges over OSC, and writes
forks of presets into the directory the player already watches (ADR-0189). The player is
unchanged for anyone who does not edit; the studio's whole cost lands on its own zip.

## Layout (ADR-0178)

| Directory | Owns | tsconfig |
|-----------|------|----------|
| `studio/electron/` | Main process: `main.ts`, `window.ts` (security defaults and CSP), `settings.ts`, `capture.ts` | `tsconfig.main.json` |
| `studio/electron/player/` | The player supervisor (`playerArgs`, spawn, both pipes), binary resolution, control and OSC sender, schema fetch, event reader, frame splitter and pump | `tsconfig.main.json` |
| `studio/electron/ipc/` | IPC handlers, one module per domain (app, player, preset) | `tsconfig.main.json` |
| `studio/electron/preset/` | The atomic preset writer | `tsconfig.main.json` |
| `studio/electron/preload/` | The `window.api` bridge, per-domain modules under `preload/api/` | `tsconfig.preload.json` |
| `studio/renderer/` | React + Vite: `views/`, `components/`, `hooks/`, the preview canvas | `tsconfig.renderer.json` |
| `studio/renderer/editor/` | The CodeMirror expression language and its diagnostics | `tsconfig.renderer.json` |
| `studio/shared/` | `protocol.ts` (the `CtlAction` / `PlayerEvent` unions + Zod, `EXPECTED_PLAYER_VERSION`), `ipc-channels.ts`, `player-mode.ts`, `schema.ts`, `toml.ts` (the line-level preset editor), `fields.ts`, `templates.ts`, `frames.ts` | every tsconfig |
| `studio/scripts/` | esbuild scripts for main and preload | n/a |

`tsconfig.test.json` is the fourth tsconfig `npm run typecheck` runs, covering the test files.
Tests are co-located (`*.test.ts` / `*.test.tsx`) and run under Vitest; there is **no end-to-end
suite** — ADR-0178 named Playwright for a golden path, and it was never built. Glob the tree
before adding a directory; the table is a map, not a roster.

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
npm test                   # vitest, run once
npm run package:win        # electron-builder --win --dir: an UNPACKED app, no zip, no signing
npm run package:mac        # electron-builder --mac --universal --dir: same
```

The three gate commands (`typecheck`, `lint`, `test`) are what `.githooks/pre-push` and the CI
`studio` job run; pre-push skips the step on a clone with no `studio/node_modules`.

**Release zips are not made by npm.** `studio/electron-builder.yml` deliberately declares no
target; `packaging/studio/build-studio.ps1` (Windows) and `packaging/studio/bundle-studio.sh`
(macOS) build the player, place it under `resources/player/`, run the unpacked package, then zip
and verify by reading the archive back — the macOS script also ad-hoc signs before zipping. The `studio-windows` and `studio-macos` jobs in `.github/workflows/release.yml`
call those two scripts, and `bundle-studio.sh` runs the same on a Mac as in CI.

The player the studio drives is built by cargo: `cargo build -p standalone --release` produces
`target/release/ritmolux(.exe)`. In development the studio finds it through its settings path or
`PATH`; a packaged studio carries it at `resources/player/`.

**The version travels in three files.** `Cargo.toml`'s `[workspace.package] version`, `version` in
`studio/package.json`, and `EXPECTED_PLAYER_VERSION` in `studio/shared/protocol.ts`.
`cargo release` moves only the first; `studio/shared/version.test.ts` fails until the other two
follow.

## What the player provides, and where it is documented

The studio keeps **no copy** of any of these. Read the source each time.

| Thing | Source of truth |
|-------|-----------------|
| Control vocabulary (`/rlx/v1/ctl/*`) and the event roster, with every field | `docs/specs/0003-studio-control-protocol.md` |
| The wire shapes' reasoning | ADR-0176; the loaded-preset fields (`preset.file`, `preset.system`, `roster.dir`, `health.preview_*`) ADR-0184 |
| The schema document | `ritmolux --schema` at runtime; `docs/specs/player-schema.json` is its committed snapshot, held to the engine by a test |
| The frame pipe — `--preview stdout` (windowed) or `--stream --sink stdout` (windowless) | `docs/capturing.md`; the `stream` event announces the geometry and the `format` (`rgba8` / `bgra8`, ADR-0187) |
| The preset file format and the expression grammar | `presets/README.md`, `docs/presets.md`, `docs/preset-palettes.md` |
| Which parameters a system accepts, as JSON Schema | `presets/schema/<system>.schema.json`, generated from the engine (ADR-0190) |
| Flags and `config.toml` keys, including `[control]` | `docs/configuration.md` |
| What a `v*` tag ships | ADR-0038, `docs/releasing.md` |

## Sibling lanes

| Lane | Owns | You touch it when |
|------|------|-------------------|
| `architect` | `docs/` — plans, ADRs, specs, the close review | any protocol widening, new IPC channel, CSP change, library adoption; a plan phase is wrong |
| `dev` | all Rust and C++ — the player side of the protocol, the schema export, the pipe sinks, the release workflow | the player lacks an event, action or flag you need (via `architect`); a plan phase it owns (automatic handoff, ADR-0188) |
| `preset-author` | preset content | never directly; its friction with the studio lands in `docs/design-backlog.md` |

## Current state

The shell, the editor and the handable studio have shipped, and the studio is a release artifact
of its own. Which studio work is live is not recorded here, because it moves every plan: read
`docs/plans/README.md`'s roster and open any plan carrying an `Owner skill: studio-builder` phase —
the plan file's `Status:` line is the authority when it and the roster disagree.
