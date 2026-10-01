# ADR-0252 — The interface's look is declared once in the core, and the studio's stylesheet is generated from it

> **Status:** accepted 2026-09-30 (Plan 0231's close), with an Outcome
> **Date:** 2026-09-27
> **Related plan(s):** [0231](../plans/done/0231-the-interface-is-audited-then-learns-one-look.md)

## Context

The three applications share no visual language, and nothing would hold one if they did. The
standalone's engine-drawn UI takes its colours from hard-coded `[f32; 4]` constants spread across
`standalone/src/overlay.rs`, `hud.rs` and `console.rs`, plus `core/src/render/now_playing.rs` and the
diagnostics panel in `core/src/render/overlay.rs`. Two constants are both named `HEADER_COLOR` and hold
different values, and three are near-duplicates of one cool grey. The studio has a token block of its
own (`studio/renderer/styles.css`'s `:root` custom properties) that no Rust value knows about. The foobar
component draws almost nothing itself, so what it shows is whatever the core's constants say.

The owner asked for one look, "smooth, with a bit of retro", across all three (Plan 0231's interview,
2026-09-27). The retro part is to be subtle and modern-first. That is only achievable if the colour
roles, type scale, spacing, radii and motion timings have **one** declaration that every renderer
reads. The renderers are three different stacks, though: glyphon plus wgpu quads in the standalone, CSS
Modules in an Electron renderer, and core-drawn frames inside a C++ host.

The project has already settled how a value reaches more than one consumer. ADR-0170 generates the
parameter reference *from the declaration the engine reads*, and ADR-0190 does the same for the editor
JSON Schemas. In both, a committed generated file sits beside a test that fails on drift, and an
`RLX_UPDATE_*` variable rewrites it. That shape has held for both files.

The owner also declined to embed a font (interview, 2026-09-27), so type stays on the system's
sans-serif font. The retro accent therefore has to come from colour, panel treatment and motion rather
than from a typeface. The core's existing 5x7 bitmap face (`core/src/render/overlay_font.rs`) is the one
typographic accent available without adding bytes.

## Decision

We will declare the interface's look once, as a plain data table in `core/src/render/theme.rs`: colour
roles, a type scale at a 1080-line reference height, spacing steps, corner radii, and motion durations
with one named easing curve. Every engine-drawn surface reads its values from that table, whichever
application draws it. The studio's `:root` custom properties become a generated file,
`studio/renderer/tokens.css`, written from the same table. A core test,
`core/tests/suite/ui_tokens.rs`, fails when the committed CSS differs from what the table generates, and
`RLX_UPDATE_UI_TOKENS=1` rewrites it. The table names **roles** (`panel`, `accent`, `text_dim`,
`highlight`), never uses (`browser_row_selected`), so that a surface restyle is a change of roles rather
than a new constant. A colour literal in a surface module is a bug, and the plan's done-when greps
hold it to that.

## Consequences

### Positive
- One edit re-themes the standalone overlays, the core-drawn banner and diagnostics (so the foobar
  panel too), and the studio. That makes the "visual inconsistency" friction mechanically hard to
  reintroduce.
- It follows the ADR-0170 / ADR-0190 generate-and-gate precedent exactly, so there is no new
  mechanism to learn, and no new dependency or file format.
- Motion timings are shared values, so a studio transition and a standalone modal open move at the
  same pace.

### Negative
- `core/` gains a module that exists for UI styling, a concern the core held only incidentally (the
  banner, the diagnostics panel). It is pure data with no platform or windowing type, so the
  source-agnostic rule holds, but the core now carries values only the standalone and studio use.
- A Rust test writes a file under `studio/`, which is `studio-builder`'s tree. The file is generated
  and never hand-edited, exactly as `presets/schema/` is to the content lane, but a studio author has
  to know to run a cargo command to change a colour.
- System fonts differ per OS, so text metrics differ too. Layout has to measure rather than estimate
  (Plan 0231 adds a measurement call), and no screenshot of text can be a golden.
- The C++ side of the foobar component cannot read a Rust constant. Its one GDI placeholder stays
  outside the table unless a later decision generates a header for it, which would be a third
  generated file.

## Alternatives considered

### Alternative A — A written style guide, applied by hand
A docs page names the palette and scale, and each surface is restyled to match it. This lost because
it is today's situation plus a document. The drift the audit starts from arose with no mechanism to
stop it, and a page would not have stopped it either. It is the cheapest option only until the first
restyle.

### Alternative B — A neutral token file (TOML or JSON) generating both Rust and CSS
Put the tokens in, say, `design/tokens.toml`, and have both sides generated from it. This lost because
it adds a third representation and a new top-level directory, and it breaks the precedent that a
generated artifact is generated *from the declaration the engine reads* (ADR-0170). With the table in
Rust, the engine's own values are the source, and only one side is generated.

### Alternative C — The standalone keeps a minimal UI and the studio carries the rest
Shrink the engine-drawn surfaces to a HUD, a browser and help, and send settings to `config.toml` and
the studio. This lost in the interview: a standalone-only user would lose the in-app settings menu that
ADR-0240 made an editor of the file, and the studio would become a required companion to a player that
ships without it.

### Alternative D — Embed a display font
An OFL face bundled in both the standalone and the studio would give identical type everywhere and a
stronger retro accent. The owner declined it. It adds bytes to an exe measured against NFR section 4's
cap (ADR-0231) and brings a licence file into two archives, for an accent the owner wants subtle.
Revisit it only through a new ADR.

## Outcome (2026-09-30, Plan 0231's close)

The table landed as decided. `THEME` in `core/src/render/theme.rs` is the one declaration, and the
studio's `studio/renderer/tokens.css` is generated from it, with a test holding the two equal. The
foobar placeholder's colours are the one hand copy, and a comment marks it as outside that gate.

**For the type scale, "every renderer reads it" is not yet true.** `THEME.type_scale` is declared and
reaches the studio's tokens, but no engine surface reads it: the overlays still draw at their own fixed
sizes, identical in pixels at 1280x800 and 1920x1080. Colour, spacing and motion are the parts every
surface takes from the table. Wiring the engine's text sizes to the scale, relative to target height
per ADR-0037, is follow-up work that a plan has to take.
