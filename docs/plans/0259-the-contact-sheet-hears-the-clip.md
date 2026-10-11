# 0259 — The contact sheet hears the clip

> **Status:** draft
> **Created:** 2026-10-11
> **Owner skill(s):** dev, human
> **Related ADRs:** [ADR-0100](../adrs/0100-documentation-images-are-committed-headless-renders.md)
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
    `--all` row and the audit-pair section, for the owner to apply in Phase 3 (ADR-0210).
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

**Lane:** _(not started)_

| phase | owner | state | commit |
|---|---|---|---|
| 1 — The audio sheet and the refusals | dev | not started | |
| 2 — The operator docs | dev | not started | |
| 3 — The skill text | human | not started | |

### Notes

### Close triggers

## Followups (after this lands)
