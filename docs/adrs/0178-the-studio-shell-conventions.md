# ADR-0178 — The studio shell conventions: three processes, a mirrored protocol, and the same security defaults as the sibling repository

> **Status:** proposed
> **Date:** 2026-09-09
> **Related plan(s):** [0159 — The studio opens](../plans/0159-the-studio-opens.md)
> **Related:** [ADR-0175](0175-the-studio-is-a-separate-application-that-never-draws-a-frame.md),
> [ADR-0176](0176-the-player-is-driven-over-osc-control-in-and-reports-on-its-standard-streams.md),
> [ADR-0177](0177-a-fourth-skill-lane-builds-the-studio.md),
> [ADR-0038](0038-tag-driven-release-unsigned-universal-mac-app.md) (what a `v*` tag ships)

## Context

ADR-0175 decides Electron; ADR-0176 decides how the player is driven. What is left is the
tactical shape of the application: how three processes are built, which TypeScript
configuration covers which directory, what crosses the preload bridge, what the security
defaults are, how the preview frames get from a pipe to a canvas, and what a release zip
contains. These decisions are cheap to make once and expensive to change after a year of code,
which is why they are an ADR and not a plan phase.

The market-analyzer repository's ADR-0008 answers all of them for an Electron shell that
supervises a sidecar process, and its answers have held through a large application. The
studio supervises a sidecar too. The one structural difference is where the domain traffic
goes. There, domain operations bypass Electron IPC entirely as HTTP calls from the renderer to
a Python service, and the hard rule is *"if it is domain logic, it is a sidecar endpoint, never
an IPC channel"*. Here the sidecar is the player, it speaks OSC and its standard streams, and
only the main process can hold a UDP socket and a child's pipes. So every domain message
crosses IPC, and the rule has to be restated for that.

The repository also has its own conventions the studio must not contradict: exact-version pins
on every direct dependency (NFR §4), releases as zips with an ad-hoc-signed macOS bundle
(ADR-0038), `npm` with a lockfile as the one Node package manager (`site/`), and an opt-in
pre-push hook that runs the fast gates.

## Decision

We adopt market-analyzer ADR-0008's conventions for `studio/` **verbatim where nothing here
contradicts them**, and diverge in the named places below. On any question this ADR does not
answer, ADR-0008 is the reference the `studio-builder` lane reads, and a divergence from it is
recorded here rather than improvised.

### Build pipeline and configuration, adopted

Three artifacts, three bundlers: the main process and the preload script through `esbuild`
(`dist/main/index.cjs`, `dist/preload/index.cjs`, externals `electron` only), the renderer
through Vite and React (`dist/renderer/`). Four `tsconfig` files with the same `extends`
layout, `strict` and the unused-and-implicit checks on, aliases `@/*` for the renderer and
`@shared/*` for `studio/shared/`. `ELECTRON_RENDERER_URL` set means load the dev server, unset
means load the built file; `app.isPackaged` decides only the CSP relaxation. ESLint, Prettier,
Vitest rather than Jest because Vite is already present, Playwright for the one golden-path
end-to-end test. A `studio/README.md` table maps directory to configuration.

### Package manager and pins, diverged

**`npm` with a committed `package-lock.json`**, not `pnpm`, because `site/` already uses it and
one package manager is enough. Every direct dependency, runtime and dev, is pinned exact,
`X.Y.Z`, with no range operator, as `Cargo.toml` already requires. `studio/` is its own npm
project with its own lockfile; there is no workspace root, so the site and the studio cannot
entangle each other's dependency graphs.

### Process roles

- **Main** owns the player: it resolves the binary, spawns it with `--events`, `--control` and
  the sink flags, holds the UDP socket that sends `/rlx/v1/ctl/*`, reads the event lines from
  standard error, reads the frame bytes from standard output, and kills the child on quit. It
  owns the OS: file dialogs, the app menu, window state, the external-URL opener. It runs
  **no editing logic**: it does not know what a palette is.
- **Preload** exposes one `window.api` assembled from per-domain modules under
  `studio/electron/preload/api/`, typed as `ElectronAPI = typeof api`.
- **Renderer** is a React SPA that never imports Node. It holds the editor state, renders the
  panels from the schema document, and paints the preview.

### IPC discipline, restated for a protocol that has to cross it

Domain traffic crosses IPC here, so the hard rule becomes: **IPC mirrors the protocol and
carries no logic.** Three domain channels, and only three, carry every player message:

| Channel | Direction | Payload |
|---------|-----------|---------|
| `player:ctl` | renderer to main | one `CtlAction`, a discriminated union mirroring ADR-0176's `/rlx/v1/ctl/*` table one-to-one |
| `player:event` | main to renderer | one `PlayerEvent`, the parsed JSON line, a discriminated union on `ev` |
| `player:frame` | main to renderer | one frame, over a `MessagePort` handed across once at start, as a transferable `ArrayBuffer` |

A fourth domain channel is a widening of the protocol, and the protocol is widened in
ADR-0176's spec first. The renderer never sees an OSC address or a JSON line; it sees typed
unions. Both unions live in `studio/shared/protocol.ts` with Zod schemas beside them, and **the
main process validates every event line it parses and every action it forwards**. A test holds
`protocol.ts` equal to `docs/specs/0003-studio-control-protocol.md`'s tables, in the way this
repository holds `presets/README.md` equal to `ParamSpec` (ADR-0170): the spec is the source,
the TypeScript is generated or checked against it, never hand-drifted.

The OS channels are the single-digit set ADR-0008 lists: app info, a directory picker, a file
picker, and the external-URL opener. Channel names are constants in
`studio/shared/ipc-channels.ts`; a push channel's preload binding returns a cleanup function.

### The preview path

Frames are read from the child's standard output in the main process, split at the byte
length the `stream` event declared, and posted to the renderer over a `MessagePort` handed
across once at start. **The frame is copied exactly once, and no transfer list is involved:**
`MessagePortMain.postMessage(message, transfer)` accepts only `MessagePortMain` objects in
`transfer`, and main and the renderer are separate OS processes, so the bytes are serialized by
the boundary whatever is asked for. What the design buys is the absence of a *second* copy — the
frame is cut once out of the pipe's chunks into a buffer that owns its memory, and the renderer
views that buffer in place rather than re-copying it. The renderer paints with `putImageData` on
a canvas sized to the stream, and upgrades to a WebGL texture upload only if a measurement says
the 2D path cannot hold the stream's declared geometry at its declared rate.
**Backpressure is dropped frames, never a stalled read:** if the renderer
has not consumed the last frame, main drops the new one and counts it, and the count is shown.
The main process never blocks on the child's pipe.

### Security defaults, adopted whole

`contextIsolation: true`, `nodeIntegration: false`, `sandbox: true`, the preload path pointing
at the built bundle, `show: false` until `ready-to-show`. The Content Security Policy is set
twice, as the `<meta>` tag and as the response header that strips any incoming policy
case-insensitively, and it is **tighter than ADR-0008's**: there is no `connect-src` relaxation,
because the renderer makes no network call of any kind. `'unsafe-inline'` in `script-src` only
while unpackaged. External URLs go to `shell.openExternal`; `will-navigate` and the window-open
handler are intercepted. None of these lines is relaxed to make a library work.

### Packaging, diverged

`electron-builder` with **zip targets on both platforms**, not NSIS and not DMG, because
ADR-0038 ships zips and the studio joins that release as a third artifact. The macOS bundle is
ad-hoc signed, exactly as the player is, by the same `packaging/macos/` recipe extended rather
than a second one. **The player binary ships inside the studio zip** under `resources/player/`,
so a tester receives one zip; the studio resolves the player in this order: bundled, then a
path from its settings, then `PATH`, and it refuses a player whose `hello` reports a version it
does not know. The studio's zip size is measured and recorded as its own NFR §4 row on the first
release, in bytes.

### Editor and UI libraries

React with CSS Modules, no component library, per ADR-0008's deferral and for the same reason.
**CodeMirror 6** for the expression editor rather than Monaco: it is modular, several times
smaller, and the expression grammar needs a small language mode with markers, not a full
language service. Adopting a component library, a state library beyond React's own, or a
second editor is an ADR each.

### What is deliberately not adopted from ADR-0008

- The localhost HTTP client and the bearer token: there is no HTTP.
- `husky` and `lint-staged`: this repository's hooks are the opt-in `.githooks/`, and the
  studio's checks join `pre-push` there, guarded on `studio/node_modules` existing.
- NSIS, DMG and the auto-updater.
- `pnpm` and the workspace root.
- The automated `ui-builder` to `dev` handoff (ADR-0177).

## Consequences

### Positive

- **Nothing about the shell is invented here.** The build, the configs, the bridge and the
  security lines come from a shipped application; what changes is named.
- **The protocol cannot fork in TypeScript.** One `protocol.ts`, held to the spec by a test.
- **The renderer has no network and no Node**, which is the smallest possible surface for a
  window that will be handed to strangers with a preset file in it.

### Negative

- **Everything crosses IPC**, including every preview frame — at the show's own rate, since
  [ADR-0183](0183-the-studio-drives-one-player-and-the-show-loop-is-extracted.md) makes the
  preview a copy of the windowed player's output rather than a headless run at a rate of its own.
  The `MessagePort` path and the single copy are the mitigation, and both are measured in
  Plan 0159, not assumed.
- **Dropped preview frames are a normal reading**, not a defect, and the studio must say so on
  screen or a user will chase them.
- **A third release artifact** with the player inside it means the release job builds the
  player before the studio, and the version check in `hello` is what stops a stale bundle from
  driving a newer player.

## Alternatives considered

### Alternative A — Keep ADR-0008's rule and put a small HTTP server in the player
Then the renderer could talk to the player directly. Rejected in ADR-0176 already: a server in
the player is a dependency and a listener the player does not need, and the preview frames
would still have to cross something.

### Alternative B — Read the frame pipe in a utility process, not main
Electron's `utilityProcess` could own the child and forward frames, keeping main free.
Deferred, not rejected: it is the upgrade if the main process's event loop is measurably
disturbed by the byte stream, and the `MessagePort` design already allows it because the
renderer does not care which process posts.

### Alternative C — Monaco
The richer editor. Rejected on weight and on fit: the expression grammar is small, the errors
come from the player as events, and CodeMirror 6's language and lint packages cover that at a
fraction of the size.

## Notes

The reference implementation for every adopted convention is
`../trading/market-analyzer/desktop/`, which is not part of this repository and is cited as a
worked example only. The double-CSP header hook and the `ELECTRON_RENDERER_URL` contract are the
two pieces most worth copying line by line.
