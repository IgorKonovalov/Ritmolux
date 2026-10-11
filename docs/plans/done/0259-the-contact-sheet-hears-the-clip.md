# 0259 — The contact sheet hears the clip

> **Status:** done - Phase 3 owed, ADR-0249. Phases 1-2 landed in 301c1d91 and 8cb3ae2d; conductor close review round 1: no blockers, no majors, one minor and one nit, both open; version 0.176.0.
> **Created:** 2026-10-11
> **Owner skill(s):** dev, human
> **Related ADRs:** [ADR-0100](../../adrs/0100-documentation-images-are-committed-headless-renders.md)
> **Closes:** design-backlog 0289

## TL;DR

`shot --all` beside `--signal` or `--audio` draws a contact sheet in which every preset is captured
at the same hop of the same clip, through the real analyzer. The hop is `--frame-at` if given, else
the clip's peak-onset hop past warm-up, and the sheet's stdout line names it. Every other pair that
an audio source used to swallow silently becomes a parse-time usage error. The first visible result
is one sheet of the whole shipped set reacting to one beat of real music.

## Context & problem

Backlog 0289, raised by `preset-author`: `shot --presets <dir> --all --signal dynamic:110` wrote a
filmstrip of the first preset and said nothing about the `--all` it ignored. In
`standalone/examples/shot.rs`, `run` returns `filmstrip(...)` whenever `args.signal` or `args.audio`
is set, before the mode is read. So `--all` and `--report` beside an audio source are silently
dropped (`--horizon`, `--render` and `--ui` are already refused). The `--set` sheets are the only
set-wide read today, and they synthesize the analysis frame, so they show nothing about how the set
responds to real dynamics.

## Decision

The owner chose the feature over the usage error. `--all` with an audio source captures each preset
through `Renderer::capture_audio` at one hop and tiles the result through the existing
`contact_sheet` layout. The hop defaults to the peak-onset hop because every gate the set reacts to
fires on onsets, and the sheet exists to compare those reactions. A multi-hop `--at` or a `--strip`
beside `--all` is refused rather than turned into a grid of strips. We rejected the bare usage
error because it fixes the silence without giving the set-wide read the entry asked for.

## Implementation phases

### Phase 1 — The audio sheet and the refusals
- **Owner skill:** dev
- **What:**
  - `run` dispatches `Mode::All` with an audio source to a contact sheet that captures each preset
    with `capture_audio(name, &pcm, format, &[hop])`, sharing the layout code with today's
    `contact_sheet` rather than copying it.
  - The hop is `--frame-at` if given, range-checked by `check_hops`. Otherwise it is `BandLevels::onset_peak_hop`
    from the `band_levels` pass that already prints the level table, so the sheet and the table
    name the same hop.
  - Stdout names the hop and the clip: `wrote <path> (<n> presets, <c>x<r> grid, hop <h>, <label>)`.
  - Parse-time refusals, each with a message naming both flags: `--report` beside `--signal` or
    `--audio`; `--strip` or `--at` beside `--all` with an audio source.
  - The `//!` usage header at the top of `shot.rs` documents the pair.
- **Files touched:** `standalone/examples/shot.rs`, `standalone/src/shot/args.rs`,
  `standalone/tests/suite/shot_cli.rs`.
- **Done when:**
  - `cargo run -p standalone --example shot -- --presets docs/examples/voxel --all --signal dynamic:110 --out <tmp>/sheet.png`
    writes a sheet with one cell per preset in that directory, and stdout names the hop.
  - A test in `shot_cli.rs` runs the audio sheet on a synthesized clip and asserts that the hop on
    the `wrote` line equals the hop the level table prints as onset's peak.
  - `--all --signal dynamic:110 --frame-at 40` captures hop 40, and `--frame-at` past the clip's
    end exits non-zero with `check_hops`'s message.
  - `--report --signal dynamic:110`, `--all --signal dynamic:110 --strip 4` and
    `--all --signal dynamic:110 --at 30,40` each exit non-zero at parse time, before a renderer is
    built, with a message naming both flags.
  - `--all` with no audio source still takes the held-stimulus path: the dispatch for it is
    unchanged, and every existing test in `shot_cli.rs` passes unedited.

### Phase 2 — The operator docs
- **Owner skill:** dev
- **What:**
  - `docs/capturing.md`'s flag table gains the audio-sheet row and names the default hop.
  - The log carries the replacement text for `.claude/skills/preset-author/references/render-loop.md`'s
    `--all` row and the audit-pair section, for the owner to apply in Phase 3 (ADR-0210). <!-- claude-allow: Phase 2 only logs replacement text and writes no file under .claude/; Phase 3 declares the path and applies it -->
- **Files touched:** `docs/capturing.md`.
- **Done when:** `node scripts/check-doc-links.mjs` and `node scripts/toc.mjs --check` pass.

### Phase 3 — The skill text
- **Owner skill:** human
- **Blocks merge:** no
- **What:** Apply the logged render-loop text to `.claude/skills/preset-author/references/render-loop.md`.
- **Files touched:** `.claude/skills/preset-author/references/render-loop.md`.
- **Done when:** The text is committed or declined.

## Risks & open questions

- **Cost.** `capture_audio` renders every hop up to the requested one, so each preset costs about as
  many frames as the hop index. A sheet of the shipped set at hop ~200 is minutes, not seconds. The
  phase reports the wall time of the done-when run in the log. If that is too slow,
  `capture_audio_after_warmup` can skip rasterizing the early hops, at the cost that GPU-integrating
  scenes have fewer steps behind them, and that is a follow-up rather than this plan's default.
- **The peak-onset hop on a synthesized signal** may land on the same beat for every `dynamic`
  spec. That is fine: the point is one comparable moment, and `--frame-at` moves it.

## What this plan does NOT do

- It does not make a grid of filmstrips, one row per preset.
- It does not change `--report`, `--horizon` or `--render`.
- It does not change the `--set` sheets.

## Implementation log

**Lane:** `plan-0259-the-contact-sheet-hears-the-clip`, worktree `/home/igor/Work/rlx-plan-0259`

| phase | owner | state | commit |
|---|---|---|---|
| 1 — The audio sheet and the refusals | dev | done | 301c1d91 |
| 2 — The operator docs | dev | done | 8cb3ae2d |
| 3 — The skill text | human | owed | |

### Notes

- Phase 1: the done-when run over `docs/examples/voxel` (3 presets, 1280x720) chose hop 25 and
  `--frame-at 40` chose hop 40. That run took about 14 s of wall time on the Arch box, debug
  build, already compiled. This was read from `date` before and after, because the allowlist refused
  `/usr/bin/time`, so the figure includes the tool-call overhead.
- Phase 2: the replacement text for `.claude/skills/preset-author/references/render-loop.md`, for
  Phase 3. In the flags table, replace the `--all --out sheet.png` row with:

  ````markdown
  | `--all --out sheet.png` | **contact sheet** — every preset in the loaded library as a labeled thumbnail grid. The fastest way to compare a draft against the shipped set and to offer the user side-by-side directions. **Honours `--set`** — see the audit pair below. **Beside `--signal` or `--audio`** it is an *audio* sheet instead: every preset driven by that one clip through the real analyzer and captured at one hop — `--frame-at <hop>` if given, else the clip's peak-onset hop, which the `wrote` line names. `--strip` and `--at` are refused there. |
  ````

  In `## The audit pair — one library, two excitations`, insert after the two-command `sh` block,
  before "What the pair exposes":

  ````markdown
  A third sheet hears real dynamics. `--all` beside a clip captures every preset at the same hop of
  it, by default the onset peak the level table names, so the grid shows how each preset answers one
  beat of real music rather than a held level:

  ```sh
  cargo run -p standalone --example shot -- --presets presets --all \
    --signal dynamic:110 --out audit/beat.png
  ```

  Read it against the loud sheet. A preset that moves under the loud `--set` and sits still here is
  calibrated to `--set` magnitudes rather than to what the analyzer delivers. Each preset re-renders
  every hop up to the chosen one, so a late `--frame-at` costs that many frames per preset.
  ````

### Close triggers

- `presets/` touched: none.
- Closes: design-backlog 0289.
- Shipped: feature (`shot` dev tooling: the audio contact sheet and three new parse-time refusals).
- Operator docs moved: `docs/capturing.md` (flag table row and one example). The
  `render-loop.md` text waits on Phase 3.
- `node scripts/check-backlog-claims.mjs`: exit 0, "46 stated reductions still hold across all 22
  live entries (3 unprobeable)".
- Human phases remaining: Phase 3 (`Blocks merge: no`).
- Full suite: owed to the conductor's pre-review gate (ADR-0207). `cargo nextest run --workspace -P fast`
  at Phase 1 ran 1994 tests: 1994 passed, 103 skipped.

## Close review

Phase 3 (the skill text) is owed (ADR-0249): `.claude/skills/preset-author/references/render-loop.md`
does not yet describe the audio contact sheet, so the preset-author lane learns of it only from
`docs/capturing.md` and `shot`'s own usage text until the owner applies the logged text and nit N1's
replacement below. Round 1 was the only round; no earlier finding was resolved by a fix round.

### Plan 0259 — close review, round 1

Graded at tip `1ace5f01852340745761ad88c45d1492cab2d1d6`, lane `plan-0259-the-contact-sheet-hears-the-clip`
(`/home/igor/Work/rlx-plan-0259`).

**Verdict: Plan 0259 landed cleanly. No blockers and no majors. One minor, one nit.** Phase 3 (`human`,
`Blocks merge: no`) is owed after the merge, per ADR-0249.

#### Evidence

- **Full suite.** `node .../with-lock.mjs suite -- cargo nextest run --workspace` printed the ledger
  record instead of running:
  `with-lock: skipped cargo nextest run --workspace: tree 684a765 is green in the suite ledger, run by gate 0259-pre-review at 2026-10-11T13:46:31.638Z: 2097 tests run: 2097 passed (20 slow), 9 skipped`.
  That record is lens 1's full-suite evidence. The log's `Full suite:` bullet defers to that gate, which
  is the correct entry in conductor mode.
- `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`: clean.
- `cargo fmt --all --check`: clean. `cargo clippy -p standalone --all-targets -- -D warnings`: clean.
- **Phase 1 done-when, run here.**
  `cargo run -p standalone --example shot -- --presets docs/examples/voxel --all --signal dynamic:110 --out /tmp/rlx-0259-review/sheet.png`
  printed the level table (`onset peaks at 1.000 on hop 25`) and
  `wrote ... (4 presets, 2x2 grid, hop 25, signal dynamic:110) [--presets docs/examples/voxel]`.
  The directory now holds four presets: `main` gained `voxel_solid_dark` after the log's three-preset
  run. I opened the PNG. It shows four labelled cells, each a distinct voxel look mid-reaction.
  `--frame-at 9999` exited 1 with `shot: --frame-at 9999: the clip is only 375 analysis hops long`.
- **Phase 2 done-when.** `node scripts/check-doc-links.mjs`: OK. `node scripts/toc.mjs --check`: OK.
- `node scripts/check-comment-hygiene.mjs`: OK. `node scripts/check-backlog-claims.mjs`: exit 0, with
  46 reductions across 22 live entries. Entry 0289 is already in the archive under `### Promoted`
  (line 363), as ADR-0206 requires.

#### Lens 1 — alignment

- Every phase carries one in-vocabulary owner tag: dev, dev, human. The `Blocks merge: no` field sits
  on the human phase only, and no phase reads Phase 3's output.
- **Phase 1.**
  - `run` dispatches `Mode::All` with a clip to `audio_contact_sheet` (`standalone/examples/shot.rs:657`).
    Every other mode still reaches `filmstrip`.
  - The held-stimulus sheet and the audio sheet share `tile_sheet` through a capture closure, so the
    layout code is not copied. The no-clip `--all` path calls `capture_preset` with the same stimulus
    and frames as before.
  - The default hop is `levels.onset_peak_hop`. That value is absolute: `band_levels` returns
    `FILMSTRIP_WARMUP + peak_index` (`standalone/src/shot/args.rs:314`). It therefore lives in the same
    index space `capture_audio` takes, and is the number the table prints.
  - `--frame-at` goes through `check_hops` before a renderer is built.
  - The three refusals are parse-time (`shot.rs:384-414`), and each message names both flags.
  - The `//!` header and `print_usage` both document the pair.
- **Tests.** I read both new tests in `standalone/tests/suite/shot_cli.rs`.
  - `an_audio_contact_sheet_captures_every_preset_at_the_onset_peak_hop` asserts that the `wrote`
    line's hop equals the number after `on hop ` in the level table. It also asserts the preset count
    and clip label, that the image decodes, and that `--frame-at 40` yields hop 40. These match the
    done-when, and none is tautological.
  - `an_audio_source_refuses_what_it_would_have_ignored` covers the refusals and the past-end hop.
    Each refusal also asserts an empty stdout and no adapter message, which is the "before a renderer
    is built" claim.
  - The diff only appends to `shot_cli.rs`, so every existing test runs unedited.
- **Phase 2.** `docs/capturing.md` gains the flag-table row naming the default hop, plus one example.
  The log carries the render-loop replacement text for Phase 3.
- **The log.** It is present, and shorter than the phases section. It reports the 14 s wall time the
  Risks section asked for.

#### Lens 2 — layering and real-time

The change touches only the `shot` example, one doc comment in `standalone/src/shot/args.rs`, and
tests. It touches nothing in `core/`, the C ABI, the control protocol, or any audio callback.

#### Lens 3 — docs and bookkeeping

- `docs/capturing.md` is swept. Its `--signal` row ("synth-audio filmstrip") is still true without
  `--all`.
- `.claude/skills/preset-author/references/render-loop.md` is Phase 3's, and that phase is owed. See
  nit N1 for one more line it misses.
- **What the close owes:**
  - Set `Status: done - Phase 3 owed, ADR-0249` and `git mv` the plan to `done/`.
  - Move archive row 0289 from `### Promoted` to `### Closed` and add the `CLOSED` marker to its body.
  - Write the plans README bullet, naming Phase 3 as owed.
  - Bump the **minor** version (a feature: new `shot` behaviour) and sync the studio's two version copies.
- `presets/` is untouched, so no curation is owed. The plan pairs with no new ADR.

#### Lens 4 — correctness

- The default hop and the table share one `band_levels` pass, so the two cannot disagree.
- `u32::try_from` on the peak hop is error-mapped, not unwrapped.
- No numeric literal is asserted beyond the hop the test itself passed (`40`).
- The level table now prints after the sheet is written, as it does for the filmstrip. On a mid-set
  capture failure the operator sees the error and no table, which is acceptable.

#### Lens 5 — design integrity

Extracting `tile_sheet` and `load_clip` removes duplication and leaves no new seam. `strip` becoming
`Option<u32>` with `DEFAULT_STRIP` makes "was it passed" an exact question, and the filmstrip default is
unchanged.

#### Findings

##### minor

**M1. `--set` and `--frames` beside an audio source are still accepted and silently ignored**
(`standalone/examples/shot.rs:389`).

- **What the plan promised.** The TL;DR says "Every other pair that an audio source used to swallow
  silently becomes a parse-time usage error". Phase 1's own list of three refusals is met.
- **What still slips through.** `--all --set bass=1,... --signal dynamic:110` builds an audio sheet,
  and its `--set` never reaches a frame, because `audio_contact_sheet` reads no `args.stimulus` and no
  `args.frames`. The same holds for `--frames` there, and for both flags beside a filmstrip.
- **Why it matters.** The audit-pair workflow in `render-loop.md` trains authors to type `--all --set`,
  so adding `--signal` to it is the likely slip. That is the exact silence backlog 0289 was raised about.
- **Fix.** Add a parse-time refusal in the same `if let Some(audio_flag)` block. It needs to know
  whether `--set` and `--frames` were passed, which means making them `Option` the way `strip` became.
  Alternatively, narrow the TL;DR's "every other pair" in a follow-up plan.

##### nit

**N1. `render-loop.md` line 41 still says a clip "renders a filmstrip" unconditionally**
(`.claude/skills/preset-author/references/render-loop.md:41`). The Phase 3 text logged in the plan
covers the `--all` row and the audit-pair section but not this line. With `--all` it is no longer always
true. This sits under `.claude/`, so it is the owner's to apply (ADR-0210). It can join Phase 3.

- **Current lines 41-42:**
  > 2. **`--signal` / `--audio`** → synthesizes PCM, runs the **real DSP analyzer**, and renders a
  >    **filmstrip** (frames tiled across time) so you see motion and beat response:
- **Replacement:**
  > 2. **`--signal` / `--audio`** → synthesizes PCM, runs the **real DSP analyzer**, and renders a
  >    **filmstrip** (frames tiled across time) so you see motion and beat response — or, beside
  >    `--all`, one audio contact sheet of every preset at a single hop (see "Other modes"):

## Followups (after this lands)
