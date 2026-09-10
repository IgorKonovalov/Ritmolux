# 0167 — The studio becomes handable

> **Status:** in-progress
> **Created:** 2026-09-10
> **Owner skill(s):** dev, studio-builder, human
> **Related ADRs:** [0186](../adrs/0186-the-studios-player-mode-is-a-per-machine-setting.md) (proposed),
> [0187](../adrs/0187-the-preview-pipe-has-a-fixed-shape-and-names-its-true-format.md) (proposed),
> [0178](../adrs/0178-the-studio-shell-conventions.md) (accepted, with an Outcome this plan retires
> half of), [0183](../adrs/0183-the-studio-drives-one-player-and-the-show-loop-is-extracted.md)
> (accepted, with an Outcome this plan discharges),
> [0184](../adrs/0184-the-player-reports-what-it-loaded-and-the-studio-re-derives-nothing.md),
> [0170](../adrs/0170-a-parameters-reference-row-is-generated-from-the-declaration-the-engine-reads.md)
> **Closes:** design-backlog 0199, design-backlog 0200, design-backlog 0201
> **Depends on:** [0159](done/0159-the-studio-opens.md) closed 2026-09-10 owing its two `human`
> phases; **this plan runs them as Phases 7 and 8.**

## TL;DR

The studio shipped and cannot be handed to anyone. Three High backlog entries stand against it: the
preview paints red and blue swapped, the preview dies the first time the show window is resized, and
a one-screen machine gets a show window permanently in the way. This plan repairs all three at the
source, discharges the two studio debts earlier closes left, and then runs Plan 0159's unrun tester
handoff and on-device check. The last thing it produces is a VJ who has used the studio.

## Context & problem

Plan 0159 closed on 2026-09-10 with nine of eleven phases landed. The two that did not run are
`human` — the tester handoff and the on-device check — and they were deferred deliberately, because
running the packaged artifact on the development machine produced three defects in one session:

- **[backlog 0199]** the studio always spawns a windowed player, so a single-screen machine gets two
  overlapping windows and no way to ask for one.
- **[backlog 0200]** the `stream` event declares `rgba8` while the windowed preview intermediate is
  built at the negotiated swapchain format, so on a BGRA backend every studio frame is drawn with red
  and blue swapped.
- **[backlog 0201]** the preview stops updating mid-session while the show keeps drawing. Diagnosed
  at Plan 0159's close: a show-window resize changes the readback's size, the sink refuses the
  mismatched frame, and the writer thread breaks on that error and discards its message.

None of the three is a thing to hand a VJ, and 0201 in particular is undiagnosable from the studio's
own instruments — its footer reads `0 dropped` throughout.

Underneath 0200 and 0201 sits one fact: **the `stream` event describes a pipe that is not the pipe.**
It names a format the windowed path does not use and a geometry that moves out from under it. Repair
that and both defects go with it, which is why they are one ADR rather than two.

Two debts also fall due here, both studio-facing and both cheap while the studio is open:

- Plan 0161 landed `kind` on every `ParamSpec` and two `[hold]` table descriptors in the exported
  schema. The studio has neither re-exported nor grown an editor for them.
- Plan 0159 Phase 7's second done-when is unmet: `--schema` carries no function or variable roster,
  so `presetLanguage` is handed nothing and an expression's identifiers are uncoloured.

## Decision

We fix the two defects **at the source in `standalone/` and `core/`**, and add the windowless mode as
a real mode rather than as a way of avoiding them. That distinction is the whole shape of this plan:
windowless makes all three symptoms vanish — the headless path is `rgba8` by construction, there is
no window to resize, and there is no window in the way — and if it shipped alone the windowed path
would stay broken while looking fixed. **The windowed path is the VJ case, and the VJ is who the
player is for.**

[ADR-0187](../adrs/0187-the-preview-pipe-has-a-fixed-shape-and-names-its-true-format.md) gives the
preview a fixed shape (a scaling blit into a sized target, so the geometry never moves) and makes
`format` report what the pipe actually carries.
[ADR-0186](../adrs/0186-the-studios-player-mode-is-a-per-machine-setting.md) makes the mode a
per-machine setting.

The phases are ordered **`dev` first, then `studio-builder`, then `human`** — one lane handoff rather
than the five Plan 0159 paid for.

## Architecture diagram

```mermaid
flowchart TB
    subgraph player["ritmolux (child)"]
        SHOW["show loop<br/>(one, both modes)"]
        WIN["windowed: swapchain<br/>surface_format = negotiated"]
        HEAD["windowless: offscreen<br/>HEADLESS_FORMAT = rgba8"]
        BLIT["scaling blit -> PreviewTarget<br/><b>fixed size, never resized</b>"]
        RB["readback -> PreviewPipe"]
    end
    subgraph studio["studio/"]
        SET["StudioSettings<br/>playerPath + <b>playerMode</b>"]
        SUP["supervisor<br/>picks the argv"]
        SPL["FrameSplitter<br/>frameBytes, set once"]
        PAINT["paint<br/><b>swizzle iff format = bgra8</b>"]
    end
    SET --> SUP
    SUP -->|"--preview stdout"| WIN
    SUP -->|"--stream --sink stdout"| HEAD
    SHOW --- WIN
    SHOW --- HEAD
    WIN --> BLIT
    HEAD --> BLIT
    BLIT --> RB
    RB -->|"frames, one size forever"| SPL --> PAINT
    RB -.->|"stream: w, h, fps, <b>format</b>"| SPL
```

## Implementation phases

### Phase 1 — The preview pipe stops moving under the reader
- **Owner skill:** dev
- **What:** [ADR-0187](../adrs/0187-the-preview-pipe-has-a-fixed-shape-and-names-its-true-format.md)'s
  first half. `PreviewTarget` gains a target size; the preview readback is opened against that size
  and filled by a sampling blit from the intermediate rather than an exact copy; `--preview` takes a
  size (`--preview stdout` keeps a default). `Renderer::resize` rebuilds the intermediate as it does
  today and **leaves the preview target's size alone**. Closes [backlog 0201].
- **Files touched:** `core/src/render/preview.rs` (the target's size), `core/src/render/mod.rs`
  (`open_preview`, `resize`, and `preview_readback_size`'s doc comment, whose *"an exact copy has one
  size"* is exactly what stops being true), `core/src/render/preview_readback.rs`,
  `standalone/src/cli.rs` (`--preview` takes a value of the form `stdout` or `stdout@WxH`),
  `standalone/src/app_state.rs`, `docs/capturing.md`, `docs/configuration.md`.
- **Done when:**
  - **A show-window resize does not stop the preview.** A test drives a windowed run through a
    resize and asserts frames continue: the pipe's byte count after the resize is non-zero and the
    frame size is the one `stream` announced. This is the assertion backlog 0201 exists for, and it
    is the one nothing in the tree makes today.
  - Fullscreen toggle and maximize are covered by the same test, because they arrive at the same
    `WindowEvent::Resized`.
  - **The preview target's size is independent of the surface's**: a test opens a preview at a size
    that is neither the window's nor a divisor of it, resizes twice, and asserts
    `preview_readback_size` returns the requested size all three times.
  - `--sink stdout` is byte-identical to before this phase for the same `--size`: the existing
    `stream_pipe.rs` assertions still pass unchanged, which is what says the `ffmpeg` contract did
    not move.
  - **The frame-time cost of the blit is reported, not assumed.** `diagnostics.log`'s one-second rows
    before and after, windowed, release, one preset, one machine, named — in the shape Plan 0159
    Phase 3 used. No threshold: the reading is the deliverable.

### Phase 2 — The `stream` event names the format it actually carries
- **Owner skill:** dev
- **What:** [ADR-0187](../adrs/0187-the-preview-pipe-has-a-fixed-shape-and-names-its-true-format.md)'s
  second half. `STREAM_FORMAT` stops being a constant; the emission reads the texture format the
  preview frames are produced at and reports `rgba8` or `bgra8`. Spec 0003's `stream` row moves from
  one value to a closed set. Closes the player half of [backlog 0200].
- **Files touched:** `standalone/src/stream.rs` (the constant goes, a mapping arrives),
  `standalone/src/show.rs` (`emit_stream` takes the format), `standalone/src/app_state.rs`,
  `core/src/render/mod.rs` (an accessor for the preview target's format, beside the size one),
  `docs/specs/0003-studio-control-protocol.md`, `docs/capturing.md`.
- **Done when:**
  - **The declared format equals the actual one, on both run modes.** A test asserts the string in
    the `stream` event against the `wgpu::TextureFormat` the preview target was built with, for the
    windowed and the windowless path. A property, not a fixture: it holds on an adapter that
    negotiates either order, and it is the assertion that would have caught 0200 the day it shipped.
  - The mapping is total over the formats a preview target can be built at, and a format outside
    that set is a named error rather than a silent `rgba8` — the failure mode this phase exists to
    end must not be reachable by a different road.
  - Spec 0003's `stream` row names both values and says which run mode produces which; the studio's
    spec-diff test, which already walks that table, covers the `Fields` column for it.

### Phase 3 — `--schema` declares the grammar
- **Owner skill:** dev
- **What:** The exported schema document gains a `grammar` section: the variable roster from
  `VAR_NAMES` and the function roster from `Func`. Generated by walking the engine's own
  declarations, never restated — the discipline
  [ADR-0170](../adrs/0170-a-parameters-reference-row-is-generated-from-the-declaration-the-engine-reads.md) already holds the
  parameter half to. Discharges Plan 0159 Phase 7's unmet done-when.
- **Files touched:** `core/src/preset/expr.rs` (whatever export the roster needs; `VAR_NAMES` is
  already `pub`, `Func::from_name` is not), `core/src/preset/schema/export.rs`,
  `core/src/preset/schema/tests.rs`, `core/tests/preset.rs`, `docs/presets.md`.
- **Done when:**
  - **Every name the engine knows appears, and no other.** A test diffs the exported grammar against
    `VAR_NAMES` and against the roster `Func::from_name` accepts, **in both directions** — the shape
    the studio's spec-diff test uses, so a name added to the engine and not the export fails, and so
    does the reverse.
  - The document's `hash` moves and `SCHEMA_VERSION` does not: this is additive, and the body hash is
    the staleness signal the studio already compares.
  - `docs/presets.md` stays the human reference and says the roster is exported, without restating
    it — a second copy here is the drift ADR-0170 exists to end.

### Phase 4 — The studio paints what it was told
- **Owner skill:** studio-builder
- **What:** The renderer reads `format` off the `stream` event and paints correctly for either order.
  Closes the studio half of [backlog 0200]. Where the swizzle goes is the studio's call: a fragment
  shader on a texture upload is free and also lifts the `putImageData` ceiling Plan 0159's own risk
  section anticipated; a CPU swizzle is simpler and is enough at 640x360. **Take the reading before
  choosing** — the paint path is a measurement, not a preference.
- **Files touched:** `studio/shared/protocol.ts` (the `format` union), `studio/shared/frames.ts`,
  `studio/renderer/components/Preview.tsx`, `studio/renderer/framePort.ts`,
  `studio/shared/protocol.spec.test.ts`.
- **Done when:**
  - **A `bgra8` frame and an `rgba8` frame of the same picture paint to the same colours.** A test
    feeds both, reads the canvas back, and asserts equality — the assertion that says the swizzle is
    applied to exactly one of them.
  - An unknown `format` value is a visible refusal, not a guess and not a blank canvas: the same
    treatment Phase 1 of Plan 0159 gave an unknown `hello` version.
  - The frame reader still takes its geometry from `stream` and no size, rate or format is
    hard-coded anywhere under `studio/`; the existing test that asserts this is extended to `format`.

### Phase 5 — The player mode is a per-machine setting
- **Owner skill:** studio-builder
- **What:** [ADR-0186](../adrs/0186-the-studios-player-mode-is-a-per-machine-setting.md).
  `StudioSettings` gains `playerMode`; the supervisor picks the argv; the UI offers the choice
  somewhere a setting belongs rather than in the editing surface. Closes [backlog 0199].
- **Files touched:** `studio/electron/settings.ts`, `studio/electron/player/supervisor.ts`
  (`DEFAULT_PLAYER_ARGS` becomes a function of the mode), `studio/electron/ipc/appHandlers.ts`,
  `studio/renderer/` (the settings surface), `studio/README.md`, `packaging/studio/READ-ME-FIRST.md`
  (a tester on one screen needs to know this exists).
- **Done when:**
  - **`windowless` opens no window**, and the studio shows a picture anyway: a test spawns the real
    player in that mode and asserts frames arrive and no window is created. It skips with a printed
    notice in ADR-0016's shape where there is no adapter or capture endpoint, as the studio's
    existing spawned-player test does.
  - **Both modes carry `--events` and `--control`.** A test walks both argument vectors and asserts
    it, because a flag added to one and not the other is the mode-dependent bug Plan 0159 Phase 3
    existed to end.
  - The mode survives a restart, and an unset or unrecognised value reads as `windowed` — the
    default is the VJ case.
  - The studio does not describe the windowless preview as a show feed. ADR-0186's Negative is that
    the by-construction guarantee is given up in that mode; the surface says so in one line.

### Phase 6 — The two debts, discharged
- **Owner skill:** studio-builder
- **What:** The studio re-exports the schema and grows the surfaces the last two closes left owed: an
  editor for the `[hold]` table and for `ParamSpec`'s `kind` (Plan 0161), and expression colouring
  driven from Phase 3's grammar roster (Plan 0159 Phase 7).
- **Files touched:** `studio/shared/schema.ts`, `studio/renderer/components/TableEditor.tsx`,
  `studio/renderer/components/ParamRow.tsx` (an integer parameter is not a continuous slider),
  `studio/renderer/editor/expr-language.ts`, `studio/renderer/hooks/useSchema.ts`, and the tests
  beside each.
- **Done when:**
  - **`[hold]` has an editor and none is hand-listed.** Plan 0159 Phase 8's test walks the schema and
    asserts a component resolves for every table kind; it now covers `[hold]` **because the schema
    declares it**, with no edit to the test's own roster. If that test needed editing, the walk was
    not a walk.
  - **A parameter the engine rounds is not offered a continuous control.** `kind` drives the widget:
    an integer parameter gets integer steps, and a test asserts it for every parameter the schema
    marks.
  - **Every name in the grammar roster is highlighted, and no other.** This is Plan 0159 Phase 7's
    done-when, verbatim, now reachable — `expr-language.test.ts` already pins both arms and needs a
    roster passed to it.

### Phase 7 — The tester handoff
- **Owner skill:** human
- **What:** Plan 0159's Phase 10, run at last. Hand the studio zip to one VJ who has never seen the
  repository.
- **Done when:** They open it, see the picture **in the right colours**, move a slider, save, and the
  player picks up the saved file, without any instruction beyond
  `packaging/studio/READ-ME-FIRST.md`. What they could not do is written into
  [`docs/design-backlog.md`](../design-backlog.md) as entries with probes.

### Phase 8 — The on-device check
- **Owner skill:** human
- **What:** Plan 0159's Phase 11. The studio beside a fullscreen player on the projector, driven over
  `--control`, for one full track, on the development machine and on the macOS arm.
- **Done when:** [`docs/on-device-validation.md`](../on-device-validation.md) carries the checklist
  with both machines' readings: the preview's dropped-frame count over the track, and the player's
  `health` line with the studio attached against a run without it. **The comparison against Plan 0159
  Phase 4's 42 -> 35.4 fps is the point** — Phase 1 is supposed to have narrowed that gap, and this
  is where the claim is either earned or not. Fullscreen is toggled at least once during the track,
  because that is the gesture that used to end the preview.

## Risks & open questions

- **The blit's cost is not yet measured.** ADR-0187 argues it is cheaper than the readback it feeds,
  which is an argument rather than a reading. Phase 1's last done-when is where it becomes a number,
  and if it is not cheap the honest answer is a smaller default rather than reverting the shape.
- **A scaled preview may be judged as if it were exact.** The mode exists so an author can watch the
  music move the picture; someone will eventually use it to judge a one-texel seam and be misled.
  Phase 5's last done-when puts one line on the surface; whether that is enough is a question for the
  tester in Phase 7.
- **`bgra8` may not reproduce on this machine.** If the development adapter negotiates RGBA, Phase 2's
  cross-mode assertion still holds but the *visible* half of 0200 cannot be seen locally, and the
  swizzle in Phase 4 is exercised only by its synthetic test. Say so in the log rather than claiming a
  visual confirmation that was not taken.
- **Two `human` phases can be deferred twice.** They were deferred once already. If they are deferred
  again, this plan does not close — that is the point of putting them here rather than in a followup.

## What this plan does NOT do

- **It does not make the writer thread loud.** `PreviewPipe`'s writer still exits on a sink error
  without printing it. After Phase 1 no size disagreement can arise, so the silence is unreachable
  rather than repaired — chosen deliberately at this plan's interview. If a later change reintroduces
  a way for that error to fire, the `eprintln!` comes with it.
- **It does not fix the drop accounting.** `FramePump` still counts a drop only when a frame arrives
  while one is unacknowledged, so loss upstream of it stays invisible. A cheaper pipe makes that loss
  rarer without making it visible; ADR-0178's `Outcome` and [backlog 0201] both record what a real
  fix would have to distinguish.
- **It does not add a mid-session mode toggle.** ADR-0186 Alternative A.
- **It does not touch clip rendering, show projects, or the diffusion pass.** Each is its own plan,
  and the first of them needs a `render` subcommand nobody has built.
- **It does not publish `packaging/studio/READ-ME-FIRST.md` as a site install page.** That needs a
  route and a menu entry together (`check-site-routes.mjs` requires both) and is its own small plan.

## Implementation log

> Written by the implementing lane — one row per phase as that phase's commit lands, and the
> close block after the last one. **The phases above are the contract; everything here is what
> happened.** **Observations, never conclusions:** this says where to look, architect decides
> how it went. No per-criterion pass list, no self-assessment, no narrative — but a deviation
> from the plan or an unmet done-when is always disclosed. Stays shorter than
> `## Implementation phases` above.

**Lane:** `main` directly.

| phase | owner | state | commit |
|---|---|---|---|
| 1 — The preview pipe stops moving under the reader | dev | done | 80fc61c |
| 2 — The `stream` event names the format it actually carries | dev | done | fbae516 |
| 3 — `--schema` declares the grammar | dev | done | 2cd608a |
| 4 — The studio paints what it was told | studio-builder | done | a001a52 |
| 5 — The player mode is a per-machine setting | studio-builder | done | 428a2ee, a967927 |
| 6 — The two debts, discharged | studio-builder | done | d033ce6 |
| 7 — The tester handoff | human | not started | |
| 8 — The on-device check | human | not started | |

### Notes

**Phase 1 — the frame-time reading.** Windowed, release, `--preset Pulse`, 1920x1080, AMD Radeon
integrated on DX12, rich tier, ~70 s per run, read from `diagnostics.log`'s one-second rows. "after"
is `--preview stdout` at the new 640x360 default with the blit in the path:

| | before (no `--preview`) | after (`--preview stdout`) |
|---|---|---|
| rows | 68 | 68 |
| fps, median | 165.000 | 165.000 |
| `frame_ms_avg`, median | 6.061 | 6.061 |
| `frame_ms_p99`, median | 6.338 | 6.412 |
| `frames_dropped`, max | 0 | 0 |

A separate 12 s run with `--events` and standard output to a file: `stream` announced
`640x360 @ 165`, the file took 1 374 105 600 B — 1 491 whole frames at 921 600 B — and `health`
reported `preview_sent` climbing at ~165/s with `preview_dropped` 0 throughout. The display is
vsync-capped at 165 Hz, so the fps column has headroom in it and `frame_ms_avg` is the column that
carries the reading.

**Phase 1 — deviations.**

- **Two files outside the phase's list**: `standalone/src/run.rs` (its `App::preview_pipe` is what
  carries the parsed size from `cli.rs` to `app_state.rs`, so it changes type with them) and
  `standalone/tests/console_preview_memory.rs` (one `open_preview_readback` call site).
- **`the_readback_yields_the_frame_before_and_nothing_on_the_first` no longer asserts byte
  identity.** The readback reads a sampled tap rather than an exact copy, so the claim is now
  nearest-of-two — the readback is closer to the previous frame than to the current one. The
  byte-identity assertion on the **show's own** pixels
  (`the_shows_pixels_are_unchanged_with_the_readback_open`) is untouched and still exact.
- **`cargo test` on `core/tests/console_preview.rs` crashes with `STATUS_ACCESS_VIOLATION` when the
  harness runs its tests in parallel.** Verified against the unmodified tree at this phase's parent
  commit: it crashes there too, so it predates this work. `cargo nextest` gives each test its own
  process and is green.

**Phase 1 — noticed, not acted on.** The tap blit is a sampling pass with a format on both sides, so
converting `bgra8` to `rgba8` there would cost the same as the format-preserving blit that shipped —
which is not the per-frame CPU pass over 8 MB that ADR-0187 rejects under Alternative C. If that is
right, `format` could stay a single value and Phases 2 and 4 would have nothing to do. Implemented as
the ADR specifies; recording the observation rather than acting on it.

**Phase 2 — where the mapping and the refusal live.** The plan puts the mapping in
`standalone/src/stream.rs`; it is in `core/src/render/mod.rs` instead, as `PixelOrder` with
`Renderer::pixel_order()`, because `standalone` has no `wgpu` dependency and so cannot name a
`TextureFormat`. The refusal is `RenderError::UnnameablePixelOrder` and fires at
`open_preview_readback` and at `pixel_order()`, so no pipe opens at a format its announcement could
not describe. `standalone/src/show.rs`, `app_state.rs` and `stream.rs` pass what the renderer says.

One source serves both run modes: the frame tap, the capture target and the preview intermediate are
all built at `ctx.surface_format()`, so there is one question and not two.

**Phase 2 — how the BGRA half is reached.** A software adapter negotiates nothing, so
`the_declared_pixel_order_is_the_one_the_frames_carry` moves `ctx.config.format` and asserts the
textures the frames come out of follow it, on both orders. Nothing is drawn after the move — the
scene pipelines were built at the original format — and the claim is about what the textures are.
The **visible** half of backlog 0200 was not reproduced on this machine: the reading in Phase 1 was
taken on an adapter that negotiates RGBA, so no locally-rendered frame has had its channels swapped
and the swizzle Phase 4 adds will be exercised by its own test rather than by a picture.

**Phase 2 — files outside the list**: `standalone/tests/stream_pipe.rs` (a comment on why `rgba8` is
this sink's answer), `core/src/render/context.rs` (the new `RenderError` variant),
`core/src/render/preview_readback.rs` and `core/src/render/tests.rs` (the refusal and the tests).

**Phase 3 — a third roster, and why.** The plan names two, `VAR_NAMES` and `Func`. The exported
`grammar` carries three: `variables`, `functions` and **`constants`**. `is_reserved_ident` — the
engine's own answer to "is this name taken" — has three arms, and exporting two of them would leave
`pi` and `tau` uncoloured in the editor Phase 6 builds, which is the gap this phase closes. Two
strings, and the test diffs each roster against its own source.

**Phase 3 — `variables` is not `VAR_NAMES`.** The four reserved `[latch]` placeholders are in
`VAR_NAMES` as storage and are filtered out of the parser's identifier lookup, so a roster published
straight from `VAR_NAMES` would offer an editor four spellings that do not compile. The filter is now
one predicate (`is_bindable_slot`) that the parser and the export both read, and
`the_published_variable_roster_is_what_the_parser_accepts` asks `compile` rather than a second list.

**Phase 3 — `Func::from_name`'s match became a table.** `FUNCS` is `[(&str, Func); 17]`;
`from_name`, `name` and `function_names` all resolve through it, which is what makes "in both
directions" true by construction rather than by a test comparing two hand-written lists. `constant`
got the same treatment (`CONSTANTS`). A variant added to `Func` without a `FUNCS` entry is
constructed by nothing, so `dead_code` fails the build — which is why no test hand-lists the
variants.

**Phase 3 — files outside the list**: `docs/configuration.md` (the `--schema` paragraph names the
new object).

**Phase 4 — the reading the phase asked for, taken before the paint path was chosen.** A CPU swap
over one preview frame at the fixed 640x360 Phase 1 gave it: 0.326 ms to view the buffer, 0.551 ms
to view and swap it, so **0.225 ms of swap** — 3.7 % of one core at 165 frames a second, and in the
renderer process rather than on the show's thread. A WebGL upload with the swizzle in a fragment
shader would save that 0.225 ms and cost a GL context, a program and a disposal path in a component
that has none. CPU, in `shared/frames.ts`, in place. (At 1920x1080 the same swap is 2.15 ms, which
is the case Phase 1's fixed shape stopped arising.)

**Phase 4 — the refusal is narrower than "the same treatment `hello` gets".** An unknown `hello`
version **stops** the player. An unknown `format` does not: the supervisor opens no splitter and
reads the pipe into nothing, and the preview shows the refusal where the picture would be. Killing
a valid show because the studio cannot name a channel order takes the projector down over a preview
question. Both halves of "visible, not a guess, not a blank canvas" hold; the stop does not.

**Phase 4 — `format` is a `z.string()` and not a `z.enum`.** An enum makes a line naming an unknown
order a *malformed* `stream`, and a dropped `stream` is a studio that never learns its geometry — a
blank canvas and a count in a log, which is the failure this phase exists to end.
`isKnownPixelFormat` is the guard, after the line is accepted.

**Phase 4 — the spec-diff test was already red at this phase's parent commit.** Phase 2 wrote the
closed set into spec 0003's `Fields` column and `codes()` reported six fields for a four-field line.
The cell parser now lifts a parenthesised group out and keys it by the field it follows, and two
tests hold it: the value set equals `PIXEL_FORMATS`, and exactly one field in the whole roster
carries such a set — the second is what stops the first passing vacuously on an empty object.

**Phase 4 — no `bgra8` picture was seen.** As Phase 2 recorded, this machine's adapter negotiates
RGBA; the capture taken at Phase 6 announced `rgba8`. The swizzle is exercised by
`Preview.test.tsx`, which feeds both orders of one picture through a stubbed 2D context and asserts
the painted bytes are equal, and not by a photograph.

**Phase 5 — how "no window" is asserted.** `MainWindowHandle` on Windows only; there is no
cross-platform way to ask the OS whether a child opened one, and the other platforms get a printed
notice. Verified non-vacuous by hand: a windowed run of the same build reports `2558100` where the
windowless run reports `0`. The frames half is asserted everywhere — whole frames, of exactly the
geometry the `stream` event announced, with nothing left over.

**Phase 5 — the settings surface, and one bug it hid.** A panel off the header rather than a sixth
editor tab (the plan rules the editing surface out) and rather than an application menu (main has
none, and building one to hold one control is a larger change than the control). A capture of the
built window then showed the radio on `windowed` while `settings.json` said `windowless`, with the
note reading *"Saved."* and nothing saved: `running` arrives one IPC round trip after mount and
`useState(running)` had frozen the pre-info default. Fixed in `a967927`, with tests that rerender
with a changed `running` — the originals passed a settled value and could not see it.

**Phase 5 — files outside the list**: `studio/shared/player-mode.ts` (new — the vocabulary has to be
free of Node because the renderer offers the choice and `electron/settings.ts` imports `node:fs`),
`studio/shared/ipc-channels.ts`, `studio/electron/main.ts`, `studio/electron/preload/api/app.ts`,
`studio/renderer/App.tsx`. `writeSettings` is new in `settings.ts`, whose header said the file was
"never written by this phase".

**Phase 6 — why `[hold]` had no editor, which is not what the plan assumed.** The plan reads as
though a component were missing. What was missing is a *reach*: `hold` is declared only as a map
**element** kind (`map` of `hold`, on the root and on `[layer]`), and Plan 0159 Phase 8's walk read
`key.kind` and stopped at `map`. So the walk never saw `hold`, `editorForKind` was never asked for
one, and the row rendered as the word `(map)`. The walk now reaches past a composite, and one test
asserts exactly that reach.

**Phase 6 — the walk's hand-kept fallback roster is deleted, not extended.** `fields.test.ts` held a
`KINDS` array "for the no-player fallback". Adding `hold` to it is the edit this phase's done-when
forbids, and keeping it without `hold` would leave a list claiming to be "every kind the engine
declares today" that was not. It now skips with a notice when there is no built player, in the shape
`templates.test.ts` already uses.

**Phase 6 — `[hold]`'s home, and where it is not.** The root's map keys have no `TableSpec` of their
own and `structuralTables` drops the root, so they had nowhere to render; they are now a section of
the structure tab, filtered by `mapElementEditor` rather than by name. `[layer.hold]` renders too,
through `TableEditor`. Two element kinds are held back and neither by name: `expr` (the parameter
panel and the file tab already own those lines) and anything resolving to `readonly` (not one line).
`easing` moved from the `enum` control — a `<select>` with no options — to the same `scalar` control,
which is what lets `[smoothing]` arrive from this code. An author's inline `{ attack, release }` is
carried back unchanged rather than quoted.

**Phase 6 — a third roster in the editor, matching Phase 3's third in the export.** `Grammar` gains
`constants` and a `grammarConstant` token, because `pi` and `tau` are names an author may write and
an uncoloured one reads as a typo.

**Phase 6 — files outside the list**: `studio/shared/toml.ts` (`removeKey` — a map row needs a way
off), `studio/shared/fields.ts`, `studio/renderer/components/MapEditor.tsx` (new),
`studio/renderer/components/PresetEditor.tsx` (it takes the roster),
`studio/renderer/views/Editor.tsx`, `studio/electron/player/schema.test.ts` (its fixture predates
`grammar`). `studio/renderer/hooks/useSchema.ts` is **not touched** — the document already carries
the grammar, so nothing there had to change.

**Phase 6 — what a capture of the built window shows.** `windowless`, the Clifford attractor in
`rgba8` at 640x360, `[smoothing]` listing eleven real entries including one whose value is an inline
easing table preserved as its own literal, and `[hold]` below it. Saved under
`target/studio-shots/`, uncommitted.

**Not fixed, and it stands in front of Phase 7.** `EXPECTED_PLAYER_VERSION` is `0.113.0` and
`[workspace.package] version` is `0.115.0`, so **the studio in this checkout refuses the player in
this checkout** — on screen as *"This player is not one the studio drives"*. The two packaging
scripts override `studio/package.json`'s version at build time, but that constant is compiled into
the renderer bundle and nothing rewrites it, so a packaged studio would refuse the player it
carries. Nothing gates the two against each other. Left for the close's version bump, or for a
decision to gate it; the captures above were taken with the constant overridden locally, and it was
reverted.

**Settled, after the phase commits, on the user's direction** (`be9f2e7`). Both studio copies follow
`Cargo.toml` at `0.115.0`, and `studio/shared/version.test.ts` holds all three equal so the next bump
fails the studio's suite until they do. Verified it bites: reverting the constant alone fails with
*expected '0.113.0' to be '0.115.0'*. The **Full suite** trigger's disk figure was also settled the
same way — `target/debug`, `target/tmp`, `target/doc` and `target/aarch64-apple-darwin` were deleted,
taking `C:` from 5.2 GB free to 70.8 GB. Those are build caches; `target/release` was kept because
two studio tests and the smoke run spawn the player out of it.

### Close triggers

> Filled after Phase 6. **Phases 7 and 8 have not run**, so this is a partial brief and the two
> readings that need the whole plan — the suite and the backlog — are stated as they stand.

- **`presets/` touched:** none. No file under `presets/` is in any of the six phase commits.
- **Plan header `Closes:`** design-backlog 0199, 0200, 0201
- **What shipped:** fix, plus one feature. Across Phases 4–6 (`a001a52`, `428a2ee`, `a967927`,
  `d033ce6`): 40 files under `studio/` — 19 in `renderer/`, 11 in `shared/`, 9 in `electron/`, plus
  its README — and one under `packaging/`. 2 283 insertions, 229 deletions. No Rust and no C++.
- **Operator docs touched:** `studio/README.md` and `packaging/studio/READ-ME-FIRST.md`. Phases 1–3
  touched `docs/capturing.md`, `docs/configuration.md`, `docs/presets.md` and
  `docs/specs/0003-studio-control-protocol.md`.
- **Backlog probes (`node scripts/check-backlog-claims.mjs`):** exit 1 — **3 broken, and all three
  are this plan's own `Closes:` list coming due**. 0200's two `present:` probes were falsified by
  Phase 2 (`STREAM_FORMAT` is gone from `standalone/src/stream.rs`, and `show.rs` no longer reads
  it); 0199's `absent: --stream in: studio/electron/player/supervisor.ts` was falsified by Phase 5,
  which is what putting it there means. 0201 is `unprobeable`. Repairing or closing them is the
  `architect` call the script's own summary names, at the close.
- **Full suite:** **not run, and deliberately.** `C:` has **5.2 GB free of 954 GB (100 % used)** and
  this worktree's `target/` is **62 GB**, while `cargo nextest run --workspace` builds every test
  binary in debug — the disk-filling failure ADR-0053's *"disk cost is severe and recurring"*
  describes, live on this machine. Reclaiming that 62 GB is what the suite needs before it can run. Phases 4–6
  touched **no Rust and no C++**, so the Rust suite's last meaningful reading is the one the `dev`
  lane took at Phase 3. The studio's own gate is green at `d033ce6`: `npm run typecheck` (four
  projects), `npm run lint`, and `npm test` — **24 files, 237 tests, all passing**, including the
  two that spawn the real player.
- **Outstanding `human` phases:** **both** — 7 (the tester handoff) and 8 (the on-device check).
  Neither can be started from this lane. The version mismatch that stood in front of Phase 7 was
  settled in `be9f2e7` and is no longer blocking; what the developer-machine smoke run **did** turn
  up is in [What the developer-machine smoke run found](#what-the-developer-machine-smoke-run-found)
  below, and **finding A should be read before anyone is handed a build** — the studio rewrites
  whichever preset rotation last brought in, without asking.

## What the developer-machine smoke run found

> **Read this before closing.** A smoke run on 2026-09-10, after Phase 6, with the owner driving the
> built studio against `target/release/ritmolux.exe`. It is **not** Phase 7 — that needs someone who
> has never seen the repository, working from the zip and `packaging/studio/READ-ME-FIRST.md` alone.
> The owner's verdict on the three phases was *"all looks good, somehow works"*. What follows is
> everything else the session turned up. **None of it is fixed** and none of it is in scope for this
> plan's phases; it is handed over for the architect to route.

### A — The studio rewrites an existing preset with no confirmation, and rotation decides which one

**The owner's decision, in their words:** *"we are changing preset when studio is opened. we should
never do that. When there is change in palette we should prompt user to save it under another name,
never save without ask to existing preset."*

**The mechanism.** Every editing gesture writes straight to the path the player named in its `preset`
event, with no prompt and no undo: a parameter slider's **release** (`ParamRow` → `onCommit` →
`setConstant`), a palette **name**, a stop **drag** and a stop **recolour** (`PaletteEditor` →
`edit`), any structure-tab key (`TableEditor` → `setKey`), any map row added, edited or removed
(`MapEditor`, new in Phase 6), and `Ctrl+S` in the file tab. That is Plan 0159's model working as
designed — *"the picture follows the finger and the disk sees one write per gesture"* — and it
assumes the preset under the editor is the one the author meant to edit.

**Rotation is what turns that assumption into damage, and this is the part nothing records.**
Rotation is on by default with a 20–130 s dwell from the operator config, so **the preset under the
editor changes by itself while the author works**. An edit therefore lands in whichever file rotation
most recently brought in. The evidence is unambiguous: in six minutes, **three different presets**
each received `system = "swarm"` from what the owner experienced as clicking the system picker —
`attractor_cliffordgallery` at 20:37, `attractor_dejonggallery` at 20:38, `attractor_ink` at 20:39.
One gesture per file, one file per dwell.

**Blast radius, measured.** Exactly **four** files in the watched directory
(`%APPDATA%\Ritmolux\presets`, 123 presets) were written during the session:

| file | what the studio changed |
|---|---|
| `attractor_clifford.toml` | all five `[palette] stops` recoloured — 49 differing lines |
| `attractor_cliffordgallery.toml` | `[palette] name` `ice`→`ember`, `system`→`swarm`, brightness bindings — 25 differing lines |
| `attractor_dejonggallery.toml` | `system`→`swarm` |
| `attractor_ink.toml` | `system`→`swarm`, plus an appended `[palette] name = "spectrum"` |

**The repository's `presets/` was never at risk** — `git status presets/` stayed clean throughout.
The damage is confined to the per-user seeded copy. All four were **restored** from the shipped set
on the owner's instruction and verified byte-identical; the modified versions were backed up to the
session scratchpad first, and that backup is not durable.

A separate measurement worth having, because it will otherwise be mistaken for this: **70 further
files in that directory differ from the shipped set and none of it is the studio's doing.** The
per-user directory is seeded once and never re-seeded, so every preset the repository has edited
since that seed differs by construction. Only the four above carry a modification time from the
session.

**What it contradicts.** Plan 0159 Phase 3 decided the release writes the file, and its
byte-equality test — *the written file differs from the original only on the edited lines* — is the
contract that model is held to. Reversing it to *"never write to an existing preset without asking"*
is an ADR with the current behaviour as the named rejected alternative, not a plan phase.

**The second-order question the architect should settle in the same breath.** A save-as prompt is
**not sufficient on its own**: rotation can move the preset between the gesture and the author's
answer, so the fork would be taken from, or written next to, the wrong preset. Whatever is decided
has to answer whether the studio **pins the preset** for as long as it is attached — which is a
`ctl/transport hold` the studio already has the vocabulary to send, so it may be a studio-only
change rather than a protocol one. A related question with the same root: an edit to a preset that
came from the **embedded** set has nowhere to land at all — `state.status === 'embedded'` already
disables writes, and a fork-on-edit model would give that case an answer it currently does not have.

### B — Only the first of several problems is ever shown

**The owner's words:** *"there is warnings when I'm changing system, i guess it could be multiple -
we should let user open modal window and give details about each warning."*

`usePlayerEvents` keeps up to **32** problems, newest first. `App.tsx` renders `player.problems[0]`
and nothing else, and no count says there are more. The full list does reach `PresetEditor`, which
turns it into gutter markers in the **file** tab — so an **error** is recoverable if the author
thinks to look there. A **warning** is not: spec 0003 gives `preset_warning` only `file` and
`message`, with no line or column, so `markersFor` has nothing to anchor it to.

Changing the system is the reliable way to produce several at once, and for a structural reason: the
rewrite keeps the outgoing system's `[params]` bindings and structural tables, and each one the
incoming system does not declare is its own warning. It is also, per finding A, the gesture most
likely to have landed on a preset the author did not intend.

**Cost, so the routing is informed.** The banner-with-a-count and the modal are buildable **entirely
inside `studio/`** from state that already exists — no new event, no new field, no protocol
widening, one phase. The span question is the only part that is not: giving `preset_warning` a line
and column would move spec 0003 and need a `dev` phase, and it is what would let a warning be marked
in the file tab like an error. Those are separable and the architect can take the first without the
second.

### C — Readings and observations, recorded rather than acted on

- **The quit is clean, verified.** Closing the studio window took the `ritmolux` child with it — no
  orphan process, launcher exit code 0. That is `will-quit` calling `supervisor.stop()`, exercised
  through a real window close rather than a hard kill.
- **The mode change works end to end, on the machine.** `windowed` was spawned with the player
  holding window handle `10486594`; the panel wrote `windowless`, and after a relaunch the same
  build spawned a player with handle **`0`** while the studio kept its picture. `playerPath` survived
  the write. That is backlog 0199 demonstrated live rather than only in a test.
- **The windowless frame rate drifts down, unexplained.** `diagnostics.log` over six consecutive
  one-second rows: fps `52.4 → 50.6 → 48.7 → 46.8 → 45.0 → 44.3`, with `frame_ms_avg` climbing
  `19.1 → 22.6 ms` and `frames_dropped` at `0` throughout. Windowless has no swapchain to pace it, so
  it free-runs against the preview pipe rather than against a display. Six seconds is far too short
  to call this a regression, and it is exactly what Phase 8's full-track reading would show if it is
  one. **Recorded so Phase 8 has a prior to compare against, not as a claim.**
- **The capture endpoint was the microphone, not loopback**: `live WASAPI 48000/4 Microphone Array
  (Realtek(R) Audio)`. Not a defect and not this plan's business, but it is a strong candidate for
  a Phase 7 tester reporting *"it does not react to my music"*, and
  `packaging/studio/READ-ME-FIRST.md` currently promises *"there is no audio setup"*.

## Followups (after this lands)

From the plan as written:

- The preview's drop accounting, so a stalled preview is distinguishable from a slow one.
- `packaging/studio/READ-ME-FIRST.md` as a published install page.
- Clip rendering from the studio (needs the `render` subcommand).
- Show projects (needs an interview and an ADR on the file's shape).

From the smoke run, detailed in the section above — the first is the one that gates a handoff:

- **A save is never silent, and rotation cannot move the preset under an editor** (finding A). An
  ADR: it reverses Plan 0159 Phase 3's model, whose byte-equality test is the current contract.
- **Every problem is reachable, not only the first** (finding B). A studio-only phase; the
  separable half — giving `preset_warning` a span so a warning can be marked in the file tab —
  moves spec 0003 and needs `dev`.

[backlog 0199]: ../design-backlog.md
[backlog 0200]: ../design-backlog.md
[backlog 0201]: ../design-backlog.md
