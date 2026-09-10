# 0159 — The studio opens

> **Status:** in-progress
> **Created:** 2026-09-09
> **Owner skill(s):** studio-builder, dev, human
> **Related ADRs:** [0175](../adrs/0175-the-studio-is-a-separate-application-that-never-draws-a-frame.md)
> (accepted, with a 2026-09-10 `Outcome` this plan wrote),
> [0176](../adrs/0176-the-player-is-driven-over-osc-control-in-and-reports-on-its-standard-streams.md) (accepted),
> [0177](../adrs/0177-a-fourth-skill-lane-builds-the-studio.md) (proposed),
> [0178](../adrs/0178-the-studio-shell-conventions.md) (proposed),
> [0183](../adrs/0183-the-studio-drives-one-player-and-the-show-loop-is-extracted.md) (proposed),
> [0184](../adrs/0184-the-player-reports-what-it-loaded-and-the-studio-re-derives-nothing.md) (proposed),
> [0038](../adrs/0038-tag-driven-release-unsigned-universal-mac-app.md)
> **Depends on:** [0158](done/0158-the-player-grows-a-studio-facing-surface.md) Phases 1 to 5 landed
> (the override, the listener, the events, the schema, the pipe sink). **Phase 6 of 0158 — the
> windowed preview copy — is now required**: ADR-0183 makes it the mechanism the studio's preview
> uses, and it shipped.

## TL;DR

A separate Electron application, `studio/`, opens beside the lean player and edits a preset
while the music plays. It spawns **one** player — the show itself — paints a copy of its frames
in its own window, renders a parameter panel from the schema the engine exports, sends slider
drags over the control channel, and saves the file the player watches. The first user-visible
behavior is the player's picture moving with the music inside the studio window. The last is a
zip on both platforms with the player inside it, handed to a tester.

## Context & problem

ADR-0175 decides the studio never renders and the player does; ADR-0176 decides the channels;
ADR-0177 gives the studio its own lane; ADR-0178 fixes how the shell is built. Plan 0158 gives
the player everything the studio needs to drive it. What is missing is the application.

**Revised 2026-09-10, after Phases 1 and 2 landed.** The plan spawned the player headless, as
ADR-0175's Decision said. That path binds no control listener, emits two of the eight events,
and — decisively — never resolves, seeds, watches or reloads the preset directory, because
`standalone/src/preset_dir.rs` is imported by `app_state.rs` alone. The editing loop this plan
exists to close does not run there.
[ADR-0183](../adrs/0183-the-studio-drives-one-player-and-the-show-loop-is-extracted.md) settles
it: the studio drives **one windowed player** that is both the show and the preview source, and
the show loop is extracted so the headless path stops being a silent subset of it. Two phases
below are new because of it, and the phases after them are renumbered.

The editing loop the studio must close is the one `preset-author` runs by hand today: edit a
`.toml`, wait for the watcher, look at the window, read an error off `stderr`, repeat. Every
piece of that loop now has a structured form: a schema document says what a preset can
contain, an event says what went wrong and on which line, a datagram moves a parameter on the
next frame, and a pipe carries the picture. The studio is those four things in one window.

## Decision

We build the studio in `studio/` as ADR-0178 lays it out, lane `studio-builder` for the studio
itself, with `dev` owning what lies outside it — the show-loop extraction ADR-0183 calls for,
the release workflow and the pre-push hook — and `human` owning the two things only the user can
do (the tester handoff and the on-device check). The first phase is a walking skeleton that
already shows the player's picture; nothing in the plan is plumbing that shows nothing.

## Architecture diagram

```mermaid
flowchart LR
    subgraph studio["studio/"]
        subgraph main["main process"]
            SUP[player supervisor<br/>spawn / kill / resolve]
            UDP[UDP sender<br/>ctl actions]
            EVR[event reader<br/>stderr lines -> PlayerEvent]
            FRR[frame reader<br/>stdout bytes -> MessagePort]
            FS[atomic preset writer]
        end
        subgraph preload["preload"]
            API[window.api]
        end
        subgraph renderer["renderer (React)"]
            PV[preview canvas]
            PP[param panel<br/>from --schema]
            ED[expression editor<br/>CodeMirror 6]
            PL[palette + composition]
            LIB[library + errors]
        end
    end
    subgraph player["ritmolux (child) - the show"]
        P[windowed, --preview stdout<br/>--events --control]
    end
    PP -- player:ctl --> API --> UDP -- OSC --> P
    P -- stderr --> EVR -- player:event --> LIB
    P -- stdout --> FRR -- player:frame --> PV
    ED -- save --> FS -- .toml --> P
```

## Implementation phases

### Phase 1 — The skeleton shows the picture
- **Owner skill:** studio-builder
- **What:** `studio/` exists per ADR-0178: three bundles, four tsconfigs, npm with exact pins,
  ESLint, Prettier, Vitest, the security defaults and the double CSP. Main resolves and spawns
  the player headless with `--stream --sink stdout --events --control`, reads the `stream` event,
  splits standard output into frames, and posts them over a `MessagePort`; the renderer paints
  them on a canvas. A footer shows the player's version from `hello` and the dropped-frame
  count.
- **Files touched:** `studio/package.json`, `studio/package-lock.json`, the four `tsconfig`s,
  `studio/scripts/build-main.mjs`, `studio/scripts/build-preload.mjs`, `studio/vite.config.ts`,
  `studio/electron/main.ts`, `studio/electron/window.ts`, `studio/electron/player/`
  (`supervisor.ts`, `events.ts`, `frames.ts`, `resolve.ts`), `studio/electron/preload/`,
  `studio/shared/ipc-channels.ts`, `studio/shared/protocol.ts`, `studio/renderer/`
  (`main.tsx`, `App.tsx`, `components/Preview.tsx`), `studio/README.md`, root `.gitignore`.
- **Done when:**
  - `npm run dev` in `studio/` opens a window that shows the player's picture moving with the
    system's audio at the stream's declared size and rate, on Windows.
  - The frame reader never blocks main: a test feeds a scripted stdout of three frames with the
    renderer's port stalled and asserts the reader drops and counts rather than queues, and the
    count reaches the footer.
  - The player is found in ADR-0178's order, bundled, settings, `PATH`, and a `hello` version the
    studio does not know produces a visible refusal rather than a blank preview.
  - `contextIsolation`, `nodeIntegration`, `sandbox` and both CSP sites are asserted by a test in
    the shape market-analyzer's `window.csp.test.ts` uses.
  - `npm run typecheck`, `npm run lint` and `npm test` are green; the `dev` and `architect` skill
    vocabulary already names `studio-builder` (ADR-0177), so the plan is reviewable.

### Phase 2 — The protocol is typed once
- **Owner skill:** studio-builder
- **What:** `studio/shared/protocol.ts` holds `CtlAction` and `PlayerEvent` as discriminated
  unions with Zod schemas; main validates every parsed event and every forwarded action; a test
  holds the file to `docs/specs/0003-studio-control-protocol.md`.
- **Files touched:** `studio/shared/protocol.ts`, `studio/shared/protocol.spec.test.ts`,
  `studio/electron/player/osc.ts` (the encoder for the `ctl` addresses, mirroring the player's
  own), `studio/electron/ipc/playerHandlers.ts`, `studio/electron/preload/api/player.ts`.
- **Done when:**
  - Every address and every event name in the spec's tables appears in the unions, and every
    union member appears in the spec: the test parses the spec's markdown tables and diffs both
    ways, so a widening on either side fails until both move.
  - The OSC encoder produces byte-identical packets to the player's encoder for every `ctl`
    message: a fixture of packets captured from the player's own test suite is checked in and
    compared.
  - An event line that fails validation is counted and shown, never thrown; a test feeds a
    malformed line between two good ones and asserts both good ones arrive.

### Phase 3 — The show loop is extracted, and every mode runs it
- **Owner skill:** dev
- **What:** ADR-0183's second half. Preset-directory resolution and seeding, the watcher and its
  reload, the `Director`, the event emissions and the control drain move out of
  `standalone/src/app_state.rs` into one place both the windowed and the headless paths call.
  What stays different between the two is the window and which sink the frames go to. The
  headless path gains the listener, the watcher and the full event roster because it runs the
  same code, not because a second copy was written.
- **Files touched:** `standalone/src/run.rs` (the headless branch stops returning before the
  config, the preset directory and `resolve_control`), `standalone/src/app_state.rs`,
  `standalone/src/stream.rs`, a new module for the extracted loop, `standalone/src/preset_dir.rs`
  (no longer `app_state.rs`'s alone), `docs/specs/0003-studio-control-protocol.md` (the roster is
  no longer conditional on a run mode), `docs/configuration.md` and `docs/capturing.md` if a flag
  or its reach changed.
- **Done when:**
  - A headless run with `--events` emits the same event roster a windowed run does, for the same
    causes: a test spawns `--stream --sink stdout --events --frames N` against a preset directory
    it controls, edits a file mid-run, and asserts `roster`, `preset` and a `preset_error` all
    arrive on standard error.
  - A headless run with `--control` **binds a socket**, reports the bound address in
    `hello.control`, and applies a `ctl/param` on the next frame; the existing control tests run
    against the headless path as well as the windowed one.
  - The show management exists **once**: a test asserts `preset_dir`'s reload entry point has a
    single caller, in the extracted module, so the duplication this phase exists to prevent
    cannot be reintroduced silently.
  - A `--stream` run on a machine with no per-user preset directory still runs, on the embedded
    set, with a printed notice in ADR-0016's shape — `PresetDir::Unresolved` is honoured rather
    than treated as a failure.
  - The windowed show is unchanged: the console's byte-identity assertion and the residency test
    both still pass, and the `diagnostics.log` frame-time line is reported before and after the
    extraction so the refactor's cost is a number rather than an assumption.

### Phase 4 — The studio drives the one player
- **Owner skill:** studio-builder
- **What:** ADR-0183's first half, which is a small change to the supervisor: spawn
  `--preview stdout --events --control 127.0.0.1:0` rather than the headless set, and take the
  geometry from the `stream` event exactly as now. The picture in the studio becomes a copy of
  the show's own output.
- **Files touched:** `studio/electron/player/supervisor.ts` (`DEFAULT_PLAYER_ARGS` and its test),
  `studio/README.md`.
- **Done when:**
  - The studio's window shows the picture and the footer reports a **bound** control address
    rather than `control none`.
  - A `ctl/param` sent from the studio moves the picture in both windows on the next frame.
  - The frame reader is untouched: the geometry still comes from the `stream` event and no size
    or rate is hard-coded anywhere in `studio/`.

### Phase 5 — The player reports what it loaded
- **Owner skill:** dev
- **What:** [ADR-0184](../adrs/0184-the-player-reports-what-it-loaded-and-the-studio-re-derives-nothing.md).
  Four additive fields under the same `v`, each read at the site that already holds the value:
  `preset` gains `system` (the schema's own key, `SystemKind::as_str()`, not the `Scene::name()`
  display string) and `file`; `roster` gains `dir`; `health` gains `preview_sent` and
  `preview_dropped` from the counters `PreviewPipe` already keeps and writes only to
  `diagnostics.log`. `Preset` gains `source: Option<PathBuf>`, set by `load_dir` and `None` for the
  embedded set, and the renderer gains the accessors the emission site needs. Spec 0003's two
  tables move with it, and the studio's spec-diff test grows to cover the `Fields` column.
- **Files touched:** `core/src/preset/schema/mod.rs` (`Preset.source`),
  `core/src/preset/mod.rs` (`load_dir` sets it), `core/src/render/mod.rs` (the two accessors),
  `standalone/src/events.rs` (the four fields and their rendering),
  `standalone/src/show.rs` (`report_active_preset`, `report_health`),
  `standalone/src/preset_dir.rs` (`roster`'s `dir`), `standalone/src/stream.rs` (reading the
  totals out), `docs/specs/0003-studio-control-protocol.md`,
  `studio/shared/protocol.ts` and `studio/shared/protocol.spec.test.ts`.
- **Done when:**
  - A spawned run reports, for a preset loaded from a directory, a `preset` line whose `system`
    is a key the `--schema` document's `systems[]` contains: a test asserts membership against
    the document the same binary printed, over **every** system, not one — the display name and
    the schema key coincide on the four single-word systems and differ on the rest, so a test
    that names only `swarm` or `attractor` cannot tell which accessor the code used.
  - The same run's `preset` line carries the absolute path the preset was read from, and a run
    against an unresolvable directory carries `"file":null` with a `roster` carrying `"dir":null`
    — both from the same spawned-process test, so the embedded-set path is exercised rather than
    assumed. `PresetDir::Unresolved` still runs the show.
  - `roster`'s `dir` is the path the watcher polls, taken from `Show`'s own field rather than
    re-resolved: a test asserts the emitted value equals what `Show::preset_dir` returns after a
    reload, under `RLX_PRESET_DIR` as well as under the default.
  - `health` carries the preview counters, and a windowed `--preview stdout` run whose reader
    stalls reports a **rising** `preview_dropped` while the show keeps drawing. With no preview
    pipe open both fields are `null`, not `0` — "no preview" and "a preview that lost nothing"
    are different facts.
  - The spec-diff test compares the `Fields` column against each union member's own keys, both
    ways, and fails when a field is added on one side alone. Adding one of this phase's own
    fields to only the spec, or only the union, is what the test is demonstrated against.
  - The human diagnostics are unchanged: `--events` stays purely additive (spec 0003), and
    `diagnostics.log`'s preview cost line still says what it said.

### Phase 6 — Parameters move
- **Owner skill:** studio-builder
- **What:** The renderer asks main for the schema (main runs `ritmolux --schema` once at start
  and caches it), renders a panel for the active preset's system from the `ParamSpec` rows,
  with a slider or a field per parameter at its range and default, and sends `ctl/param` on
  every drag step. On release it writes the preset file atomically into the directory the player
  watches, with the dragged value as the new constant, and clears the override.
- **Files touched:** `studio/electron/player/schema.ts`, `studio/electron/preset/writer.ts`
  (tmp file plus rename, never a partial file), `studio/electron/preset/toml.ts` (a
  round-tripping TOML edit that preserves comments and table order), `studio/renderer/`
  (`views/Editor.tsx`, `components/ParamPanel.tsx`, `components/ParamRow.tsx`,
  `hooks/useSchema.ts`, `hooks/usePlayer.ts`), `studio/shared/schema.ts`.
- **Done when:**
  - Dragging a slider moves the picture on the next painted frame with no file written; a test
    asserts one `ctl/param` per drag step and zero writes until release.
  - Releasing writes the file once, atomically, and the player's `roster` event follows; the
    written file differs from the original **only** on the edited line: a test round-trips every
    shipped preset through the editor with no edit and asserts byte equality, then with one edit
    and asserts a one-line diff.
  - A parameter the preset binds to an expression rather than a constant shows the expression
    and is not draggable; the slider is offered only for a constant binding.
  - A `preset_error` after a save is shown against the file and line it names.

### Phase 7 — Expressions and palettes
- **Owner skill:** studio-builder
- **What:** A CodeMirror 6 editor for the preset file with a small language mode for the
  expression grammar, error markers placed from `preset_error` events, and a palette editor with
  draggable stops and the built-in palette names from the schema.
- **Files touched:** `studio/renderer/components/PresetEditor.tsx`,
  `studio/renderer/editor/expr-language.ts`, `studio/renderer/editor/diagnostics.ts`,
  `studio/renderer/components/PaletteEditor.tsx`, `studio/renderer/components/StopHandle.tsx`.
- **Done when:**
  - A syntax error typed into an expression shows a marker on the line the player reports within
    one save cycle, and clears when fixed; a test drives the diagnostics from a recorded event.
  - The language mode's token list is generated from the schema's function and variable
    roster, not hand-typed: a test asserts every name in the schema is highlighted and no other.
  - Moving a palette stop writes the `[palette]` table in the same round-tripping form Phase 6
    established, and the picture follows on the next reload.

### Phase 8 — Composition and the library
- **Owner skill:** studio-builder
- **What:** The system picker, the structural tables (`[particles]`, `[generator]`, `[curve]`,
  `[mesh]`, `[spectrum]`, `[per_vertex]`, `[feedback]`, `[layer]`, `[latch]`, `[smoothing]`)
  rendered from the schema's structural half, "new preset from this system's defaults",
  "save as", and a library view of the player's roster from the `roster` event with a click
  sending `ctl/preset`.
- **Files touched:** `studio/renderer/views/Library.tsx`, `studio/renderer/components/`
  (`SystemPicker.tsx`, `TableEditor.tsx`, `LayerEditor.tsx`), `studio/renderer/hooks/useRoster.ts`,
  `studio/electron/preset/templates.ts`.
- **Done when:**
  - Every structural table the schema declares has an editor, and none is hand-listed: a test
    walks the schema and asserts a component resolves for each table kind.
  - A preset built from a template loads in the player with no `preset_error` and no
    `preset_warning`: a test builds one per system and drives it through the player's loader
    (via `ritmolux --check`, or through a spawned player's events if no check flag exists, which
    is then a feedback note for `architect`).
  - Clicking a library entry dissolves the player to it, and the panel re-renders for the new
    system.

### Phase 9 — The release job and the gate
- **Owner skill:** dev
- **What:** The `v*` tag builds the studio zip for Windows and macOS with the player inside,
  beside the two existing zips; `.githooks/pre-push` runs the studio's typecheck, lint and tests
  when `studio/node_modules` exists; `docs/releasing.md` and `packaging/` say so.
- **Files touched:** `.github/workflows/` (the release workflow and a `studio` CI job),
  `.githooks/pre-push`, `packaging/macos/bundle.sh` or a sibling for the studio,
  `packaging/studio/READ-ME-FIRST.md`, `docs/releasing.md`, `docs/nfr.md` (the studio's size row,
  in bytes, from the first measured zip), `CLAUDE.md` (the `studio/` row in "Where things live").
- **Done when:**
  - A tag push produces three zips; the studio zip on each platform contains the player at
    `resources/player/`, and the studio starts against it with no path configured.
  - The macOS studio bundle is ad-hoc signed and passes the same verification `bundle.sh` runs
    on the player.
  - The size row exists and names the bytes; the pre-push hook's studio step is skipped, with
    a printed notice in ADR-0016's shape, on a clone with no `studio/node_modules`.

### Phase 10 — The tester handoff
- **Owner skill:** human
- **What:** Hand the studio zip to one VJ who has never seen the repository.
- **Done when:** They open it, see the picture, move a slider, save, and the player picks up the
  saved file, without any instruction beyond `packaging/studio/READ-ME-FIRST.md`. What they
  could not do is written into `docs/design-backlog.md` as entries with probes.

### Phase 11 — The on-device check
- **Owner skill:** human
- **What:** The studio beside a fullscreen player on the projector, driven over `--control`, for
  one full track, on the development machine and on the macOS arm.
- **Done when:** `docs/on-device-validation.md` carries the checklist with both machines'
  readings: the preview's dropped-frame count over the track, and the player's `health` line
  with the studio attached against a run without it.

## Data shapes

```ts
// illustrative — studio/shared/protocol.ts
export type CtlAction =
  | { kind: 'param'; name: string; value: number }
  | { kind: 'param_clear'; name: string }
  | { kind: 'params_clear' }
  | { kind: 'preset'; name: string }
  | { kind: 'transport'; verb: 'next' | 'prev' | 'auto' | 'hold' }
  | { kind: 'ping'; nonce: number };

export type PlayerEvent =
  | { v: 1; ev: 'hello'; version: string; schema_hash: string; control_port: number }
  | { v: 1; ev: 'preset_error'; file: string; message: string; line?: number; col?: number }
  | { v: 1; ev: 'stream'; width: number; height: number; fps: number; format: 'rgba8' }
  // ... one member per row of the spec's event table
```

## Risks & open questions

- **Frame transfer cost in Electron.** The `MessagePort` path is the design; if the renderer
  cannot paint the stream's declared geometry at its declared rate with `putImageData`, the phase
  upgrades to a WebGL texture upload before it lowers anything. **The rate is now the show's**,
  since ADR-0183 makes the preview a copy of the windowed player's output rather than a headless
  run pinned at 640x360 and 30 fps — so this risk grew, and the dropped-frame counter is what
  makes either reading honest.
- **The extraction is the phase with the most to break and least to show.** Phase 3 touches
  `run.rs` and `app_state.rs` and ships no visible feature; its done-whens are therefore written
  as *the windowed show is unchanged* — the console's byte-identity assertion, the residency
  test, and a frame-time line reported before and after rather than assumed.
- **Round-tripping TOML with comments.** Shipped presets carry long header comments that are
  the project's own record; a writer that loses them is a regression the byte-equality test in
  Phase 6 exists to catch. If no library round-trips faithfully, the writer edits the file as
  text by line, which is enough for a constant on its own line and is what the test measures.
- **One capture, and one window that is now part of the editing workflow.** The two-capture risk
  is gone with the second player (ADR-0183). What replaces it: a user editing on a single screen
  with no projector attached has the player's window in the way, and macOS still has no loopback
  without a virtual device. The on-device check reads both.
- **The schema's structural half may not be enough to render an editor.** Phase 8's walk over
  the schema is where a missing enumeration or a missing default surfaces; each is a feedback
  note to `architect` for Plan 0158's Phase 4, not a hand-typed fallback in the studio.
- **The `render` subcommand and projects are not here.** A tester will ask for both; the plan
  says no on purpose so that the editing loop lands whole.

## What this plan does NOT do

- **No clip rendering and no diffusion pass.** Each is a plan of its own once the player has a
  `render` subcommand (ADR-0175 decides it; nothing has built it).
- **No project or show file.** Its format is an interview and an ADR first, because a cue list
  and a timeline are different products.
- **No second player.** Revised 2026-09-10: the studio drives **one** windowed player that is
  both the show and the preview source (ADR-0183), through Plan 0158 Phase 6's `--preview
  stdout`. There is no headless editing child, and the two-capture risk below is discharged
  rather than carried.
- **No remote control, no phone page.** ADR-0175 Alternative E.
- **No installer, no signing beyond ad-hoc, no auto-update.**

## Implementation log

> Written by the implementing lane — one row per phase as that phase's commit lands, and the
> close block after the last one. **The phases above are the contract; everything here is what
> happened.** **Observations, never conclusions:** this says where to look, architect decides
> how it went. No per-criterion pass list, no self-assessment, no narrative — but a deviation
> from the plan or an unmet done-when is always disclosed. Stays shorter than
> `## Implementation phases` above.

**Lane:** `WORK/rlx-plan-0159` on `plan-0159-the-studio-opens`.

| phase | owner | state | commit |
|---|---|---|---|
| 1 — The skeleton shows the picture | studio-builder | done | `f2be445` |
| 2 — The protocol is typed once | studio-builder | done | `d80b7a4` |
| 3 — The show loop is extracted | dev | done | `163b330` |
| 4 — The studio drives the one player | studio-builder | done | `97a6294` |
| 5 — The player reports what it loaded | dev | done | `bd037d4` |
| 6 — Parameters move | studio-builder | done | `ce70952` |
| 7 — Expressions and palettes | studio-builder | done | `658472f` |
| 8 — Composition and the library | studio-builder | done | `6b85624` |
| 9 — The release job and the gate | dev | done | `9a4410a` |
| 10 — The tester handoff | human | not started | |
| 11 — The on-device check | human | not started | |

### Notes

**Phase 9 — one done-when written before Plan 0102, and four files outside the
lists.**

- **"A tag push produces three zips" counts the release as it was before the
  foobar2000 component.** It already produced three, so the studio's two make
  **five**: `release.yml`'s publish gate is `-ne 5`, its `needs:` names five
  jobs, and `docs/releasing.md` says five.
- **`studio/electron-builder.yml` and two `studio/package.json` edits are
  `studio-builder`'s lane, not this phase's list.** Written here on the user's
  direction at the session's start, because nothing outside `studio/` can
  produce the zip the done-when measures. `electron-builder` 25.1.8 pinned
  exact, two `package:*` scripts, an `author` field; `npm audit --omit=dev`
  finds nothing.
- **`.gitignore` gains `studio/staging/`**, where each packaging script drops
  the platform's player for a fixed `extraResources` path to read.
- **`docs/nfr.md` section 7's gate count moved nine to ten** for the new `studio`
  CI job, beyond the size row the phase names for that file. While there: the
  same sentence says "the six Node doc gates" and the hook runs seven —
  `check-reader-prose.mjs` is absent from it. Not touched.

**Phase 9 — what was measured, and what has run nowhere.**

- **118,073,278 B**, the Windows zip, from the first one `build-studio.ps1`
  produced against a `--features spout` player. Unpacked 291,321,974 B:
  277,424,419 B prebuilt Electron, 10,701,824 B player, **3,195,731 B `app.asar`
  — everything this project wrote**. The NFR row records rather than caps.
- **The bundled-player done-when is verified from the extracted archive.**
  Unpacked to a scratch directory, no settings file, `Ritmolux Studio.exe`
  launched: it spawned `...esources\playeritmolux.exe --preview stdout
  --events --control 127.0.0.1:0`.
- **Nothing macOS was executed.** `bundle-studio.sh` is `bash -n` clean and
  carries `packaging/macos/bundle.sh`'s assertions plus two — the bundled
  player's own `lipo -archs`, and `--strict --deep` — but the ad-hoc signature,
  the universal Electron lipo and the plist check run first on the
  `studio-macos` job of a tag push.
- **Both arms of the hook's guard were exercised** against the real lines, the
  absent arm with `studio/node_modules` moved aside. 15.2 s warm: typecheck
  7.7 s, lint 2.6 s, tests 4.6 s.
- **`-P fast` is the tier run** (1459 passed, 230 skipped). The full suite is not
  run here — no Rust, shader or preset is touched — and is owed at the close,
  which is not this session's: Phases 10 and 11 are `human` and unstarted.
- **Two things a tester sees that no done-when names:** electron-builder reports
  `default Electron icon is used`, and `packaging/studio/READ-ME-FIRST.md` is not
  in `site/`'s `PUBLISHED` map, so unlike the other three it publishes as no
  install page.

**Phase 5 is blocked on two facts the protocol does not carry.** The lane
stopped before writing any of it.

Phase 5 renders "a panel for the active preset's system" and writes "the preset
file atomically into the directory the player watches". The event roster carries
neither:

the system, the file and the directory. (The table that stood here is
reproduced in ADR-0184's Context, which is now where it lives.)

The spec's own invariants already name the missing concept — *"a parameter the
active preset's system claims"* — so the contract knows about the system and the
stream does not report it.

The studio can resolve the directory itself, by applying `RLX_PRESET_DIR` else
the OS data root plus `Ritmolux/presets`. That is a second copy of a resolution
rule, and two copies that agree today are what
[ADR-0183](../adrs/0183-the-studio-drives-one-player-and-the-show-loop-is-extracted.md)
was written about; a studio that resolved differently would edit files the player
is not watching. Reading the path out of the `loaded N preset(s) from ...`
diagnostic is the same bet on prose the event stream exists to end.

What is buildable without either fact: the schema fetch and its cache, the
`ParamSpec`-driven panel given a system, and the round-tripping TOML writer with
its byte-equality test over `presets/*.toml` in this checkout. What is not: the
panel knowing which system to render, and the save landing where the player will
see it.

> **Architect, 2026-09-10.** Answered, and the answer is a phase: the three facts
> travel on the event stream from the sites that already hold them
> ([ADR-0184](../adrs/0184-the-player-reports-what-it-loaded-and-the-studio-re-derives-nothing.md)),
> carried by the new **Phase 5** above, owned by `dev`. The refusal of a second
> resolver stands; what the note does not reach is that the system has two names
> here, and Phase 5's first done-when is written against it. The **preview
> accounting** the Phase 4 note routes here is in the same phase — the producer
> already counts it. The **ADR-0181 collision** is decided in `docs/adrs/README.md`
> beside 0120's and 0160's: 0181 stays with Plan 0165, whose number is cited bare
> from `ci.yml` and from the append-only backlog archive, and this lane's studio
> ADR is now **0183**. Every phase from the parameter panel down is renumbered by
> one, so **the numbers in the notes below are the ones the plan carried when each
> note was written** — this note's Phase 5 is now Phase 6, the on-device check 11.

**Phases 6 to 8 — one unmet done-when, one missing player flag, and four
files outside the lists.**

- **Phase 7's second done-when is not met.** The language mode was to generate
  its token list from *"the schema's function and variable roster"*; `--schema`
  declares systems, stages and tables and carries no such roster, and the
  engine's `VAR_NAMES` and `Func::from_name` are not exported. Rather than type
  the list into the studio, `presetLanguage` takes a roster and colours exactly
  what it is given — nothing today. Structure is coloured either way, and
  `expr-language.test.ts` pins both arms, so passing a roster is the only change
  needed. **Feedback for architect: the schema export needs a `grammar` section.**
- **Phase 8's `ritmolux --check` does not exist.** The done-when names the
  fallback and that is what runs: one spawned player over a directory of twelve
  templates, asserting the roster names all of them and that no `preset_error`
  or `preset_warning` arrives. 2.7 s, and it skips with a notice where there is
  no built player, capture endpoint or adapter. The absent flag is the feedback
  note the done-when itself calls for.
- **Four files sit outside the phase lists**, each because a rule outranked the
  list. `shared/toml.ts` and `shared/fields.ts` are pure string and schema work
  the renderer needs, and ADR-0178 says main runs no editing logic — the plan put
  both under `electron/`. `electron/ipc/presetHandlers.ts` and
  `renderer/hooks/useActivePreset.ts` have no home in the lists at all. Three IPC
  channels are new (`app:get-schema`, `preset:read`, `preset:write`), all **OS**
  channels carrying no protocol message; the three domain channels are untouched.
- **CodeMirror 6 and `@lezer/highlight`, pinned exact** (ADR-0178 chose it).
  Every advisory `npm audit` reports is dev-only: `npm audit --omit=dev` finds
  none.

**Phases 6 to 8 — four things the real player and the real presets corrected.**

- **A `[params]` value is always a quoted string.** The loader deserializes that
  table into `BTreeMap<String, String>`, so a bare TOML number there is a load
  error; a constant is a numeric *expression*. The first corpus run found zero
  constants across the shipped set and said so.
- **A regex `.` excludes the carriage return.** The obvious entry pattern matched
  nothing in a CRLF preset, so every parameter read as unbound.
- **A parameter's `range` can be `null`, and thirty-one are** — the pan offsets
  and their siblings. Those get a number field; a slider would offer travel the
  engine never promised.
- **A table key's `of` is an object** carrying a kind and sometimes a table, not
  a string.

**Phase 5 — three things outside what the phase names, and one done-when a
test does not reach.**

- `standalone/src/app_state.rs` is touched and the phase's file list does not
  name it: the windowed path owns the `PreviewPipe`, so it is the caller that
  can hand `report_health` the totals.
- **`roster`'s `dir` is absolute, and making it so changed an operator-visible
  line.** ADR-0184 says `preset.file` is absolute and says nothing about `dir`.
  The override arm of `startup_preset_dir` now runs `std::path::absolute`
  before it prints, so the `RLX_PRESET_DIR set: ...` notice, the settings row
  and the event name one path; a relative override reached the event verbatim
  before, unusable to a parent with its own working directory.
- `studio/shared/protocol.ts` and its spec test are in the phase's file list
  and were **not** touched, on the user's direction at the session's start:
  they are `studio-builder`'s, and the Zod widening and the field-diff test
  open Phase 6. Nothing goes red between — the existing spec-diff test compares
  `ev` names, which did not move.
- **The windowed half of the preview-counter done-when has no test.** A
  `PreviewPipe` exists only on the windowed path, so a stalled reader raising
  `preview_dropped` needs a window, which is Phase 11. Covered instead: the
  `null` arm end to end in `a_headless_run_reports_no_preview_counters`, and
  both arms of the rendering in the `events.rs` fixture.

Two readings taken while verifying: swapping `active_system_key` for
`active_system_name` fails the twelve-system walk on `fragment_field`
(`left: "fragment field"`), so it discriminates the accessors rather than
assuming them; and `star_pattern` and `lsystem` need more than their key to
load, which `a_bare_system_preset_compiles_for_every_system` is what says.

**Phase 4 — what the capture shows, and one done-when with no originator yet.**

`target/studio-shots/` is not committed; the capture the phase was verified from
reads, in the footer: `player 0.113.0`, `control 127.0.0.1:56511`,
`stream 1920x1080 @ 165 rgba8`, `59 painted - 0 dropped`, with the show's picture
in the canvas and `Clifford` in the title. The geometry is the show's rather than
the headless preview default, which is the third done-when; `--size` and `--fps`
appear nowhere under `studio/` and a test now says so.

**The second done-when has no originator until Phase 5.** A `ctl/param` sent from
the studio needs something in the renderer to send it, and the parameter panel is
Phase 5. The path below it is whole and exercised — `window.api.player.send` to
`player:ctl` to the validated forward to the UDP sender aimed at what
`hello.control` reported — and the player's half is driven from a spawned process
by `standalone/tests/stream_show.rs`. What no test yet covers is a datagram the
**studio** composed being accepted by a real player; Phase 5's slider is where
that becomes observable.

**Phase 4 — the preview at the show's resolution, measured.**

Two readings taken while verifying the phase, both on the reference machine with
`--preset Clifford` at 1920x1080:

- **The pipe carries 8.29 MB per frame.** Draining it with a fast reader for 20 s
  took 3 060 633 600 bytes — 369 frames, about 153 MB/s, while the show itself
  drew at ~42 fps. So the preview yields roughly 18 frames/s where the show draws
  42.
- **The show slows with the studio attached**: `health` reports ~42 fps draining
  into a file and 35.4 fps with the studio painting. That is one machine and one
  preset, and it is the comparison Phase 10 is there to take properly.

**The dropped-frame counter reads 0 while frames are plainly being lost.** The
studio painted 59 frames in ~15 s against a show drawing 42/s. `FramePump`
counts a drop only when a frame arrives while one is in flight, so loss that
happens upstream of it — in the OS pipe, or in main's read cadence — is invisible
to the reading ADR-0178 relies on to make the preview's cost honest. Nothing here
is a Phase 4 regression: the frame reader is untouched, which is what the phase
asked for. Where the accounting belongs is a question for the review.

**ADR number 0181 is claimed twice.** This lane's studio/show-loop decision and
main's `0181-the-gate-compiles-every-feature-a-release-ships.md` (Plan 0165) were
both written on 2026-09-10 and both files now exist in this branch. The merge
that brought them together kept both roster rows and moved the next free number
to 0183; which of the two renames is an architect call, as it was for 0120 and
0160. Every citation of either is by its own filename, so nothing resolves to the
wrong document in the meantime.

**Phase 3 — two things outside what the phase names.**

- `standalone/src/control.rs` is touched, and the phase's file list does not name
  it. `Control::last_drained` was added: the fixed order a drained frame is
  applied in puts the transport verbs before the rest, the transport step needs
  the caller's own state, and calling `drain` a second time to come back for the
  rest would swap in an empty buffer and discard the frame.
- **The verb-to-action mapping is shared; the applier is not.** Spec 0003
  requires a `ctl/transport` verb to resolve the same action the console strip
  resolves, and both paths go through `console::action_for_transport`. What the
  windowed path does with the result needs a window — a title, a soak note, a
  redraw — so the headless path carries the three reachable actions out itself,
  in `stream::apply_transport`, against a `SettingsView` built from the values a
  run with no window actually has. `every_transport_action_is_applied` walks the
  mapping and fails if it ever resolves to an action that applier ignores.

The halves the Phase 2 note below records as unreachable are reachable;
`standalone/tests/stream_show.rs` drives them from a spawned process.

**Phase 3 — the frame-time reading the phase asks for.**

Windowed, release, `--preset Pulse`, 1920x1080, AMD Radeon integrated on DX12,
rich tier, ~70 s per run, read from `diagnostics.log`'s one-second rows:

| | before | after |
|---|---|---|
| rows | 67 | 66 |
| fps, median | 165.000 | 165.000 |
| `frame_ms_avg`, median | 6.062 | 6.061 |
| `frame_ms_p99`, median | 6.736 | 6.607 |

The two runs' standard error is byte-identical. A first attempt used
`--preset Lorenz`, which is GPU-bound at 7.6 fps on this adapter; `Pulse` was
chosen because its baseline spread is +/-0.5%, which is what makes a per-frame
CPU cost visible at all.

**The headless path the plan spawns emits two events and binds no listener.**
Phase 1 spawns `--stream --sink stdout --events --control` as the plan's phase
says. On that invocation `standalone/src/run.rs` returns at the headless branch
before `resolve_control` is reached, so `--control` is accepted as a token and
no socket is bound; `hello` reports `"control":null`, which the studio's footer
shows as `control none`. The only two events the path emits are `hello`
(`run.rs`, the headless branch) and `stream` (`stream.rs`). `preset`, `roster`,
`preset_error`, `preset_warning` and `health` are emitted from `app_state.rs`
only, and the path publishes no OSC telemetry either.

What that leaves unmet, by phase, is the half of each that needs the player to
say or do anything:

What that leaves unmet is the half of each phase that needs the player to say or
do anything. Phase numbers below are the ones this plan carried when the note was
written; the renumber that followed makes them 5, 6 and 7:

(The table of unreachable done-whens that stood here is discharged: Phases 3,
4 and 5 landed the listener, the watcher and the full roster.)

The halves that do not depend on it are untouched and buildable: `--schema`
works and exports 59 KB covering every system's params with defaults, ranges and
prose, and the atomic round-tripping preset writer is a filesystem question.

The lane stopped at that boundary rather than build panels against a player that
reports nothing.

> **Architect, 2026-09-10.** The interim decision recorded here — keep the studio
> headless and bind the listener on that path — was superseded the same day, once
> the missing watcher was found alongside the missing listener. See
> [ADR-0183](../adrs/0183-the-studio-drives-one-player-and-the-show-loop-is-extracted.md)
> and Phases 3 and 4 above.

**Two places the implementation differs from what a phase or an ADR says.**

- ADR-0178's preview path says frames cross "as transferable buffers, so the
  copy across processes is one move rather than a structured clone".
  `MessagePortMain.postMessage(message, transfer)` accepts **only
  `MessagePortMain` objects** in `transfer` (`electron.d.ts:8841`), and main and
  the renderer are separate OS processes, so a frame is serialized whatever is
  asked for. The frame is cut once out of the pipe's chunks and viewed in place
  by `ImageData`; there is no second copy on either side, but the mechanism the
  ADR names is not available in that direction.
- Phase 2 asks the OSC fixture to be "captured from the player's own test
  suite". The player *encodes* telemetry and only *decodes* `ctl`, and the
  headless path publishes no telemetry, so `shared/fixtures/osc-packets.json` is
  fourteen packets captured off a short **windowed** run's encoder over UDP. All
  three argument types on the `ctl` roster appear in it.

### Close triggers

- **`presets/` touched:**
- **Plan header `Closes:`** none
- **What shipped:**
- **Operator docs touched:**
- **Backlog probes (`node scripts/check-backlog-claims.mjs`):**
- **Full suite:**
- **Outstanding `human` phases:**

## Followups (after this lands)

- Clip rendering from the studio (needs the `render` subcommand).
- Show projects (needs an interview and an ADR on the file's shape).
- The diffusion pass from the studio (spawns `tools/sd-filter/`, ships nothing).
