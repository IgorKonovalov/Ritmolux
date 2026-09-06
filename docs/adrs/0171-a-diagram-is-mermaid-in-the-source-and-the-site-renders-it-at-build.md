# ADR-0171 — A diagram is mermaid in the source, and the site renders it at build

> **Status:** accepted 2026-09-06 (Plan 0156)
> **Date:** 2026-09-06
> **Related plan(s):** [0156](../plans/done/0156-the-site-becomes-the-reference.md) Phase 3
> **Extends:** [0154](0154-the-reader-facing-docs-publish-as-a-site.md) (build-time transforms, one source)

## Context

The published set contains no diagram. The one diagram in the corpus is the architecture flowchart
in `README.md`, written as a ```` ```mermaid ```` fence, which GitHub renders natively and the site
does not carry. The user's brief asks for diagrams, *"but not too much"*, and for a reader to
understand the public API — which for an embedder means a lifecycle across the C ABI, and for a
preset author means what happens between a PCM frame and a parameter value.

This repository's convention for diagrams is settled: `CLAUDE.md` and the architect skill both say
*mermaid in markdown — renders in GitHub and VS Code, diffs cleanly*. Every plan carries one. The
question is only how the site renders a fence that Starlight does not render on its own, and the
options differ in where the rendering happens and what it costs.

Three facts bound the choice:

- **The source may not change to suit the site** (ADR-0154). Whatever renders the diagram reads the
  fence; nothing under `docs/` is rewritten to an image link.
- **The site ships no client-side JavaScript beyond Starlight's own**, and it renders in both
  colour schemes. A diagram has to be readable on the dark theme without a second copy.
- **Committed renders are this repository's habit for pictures** (ADR-0100), and they exist because
  a render is not byte-reproducible across machines. A diagram is not a GPU render; the same source
  produces the same SVG on every machine, so the argument for committing the output does not carry.

## Decision

**Diagrams are written as mermaid fences in the source documents, and `site/` renders each fence
to an inline SVG at build time with `rehype-mermaid`, using a headless Chromium that the Pages
workflow installs.** The output is a `<picture>` carrying a light and a dark rendering, so the
diagram follows the theme with no script on the page. The source keeps the fence, so GitHub and an
editor keep rendering it.

The plan names four diagrams and where each lives, and that number is a ceiling by intent rather
than a target:

| Diagram | Kind | Where |
|---|---|---|
| The architecture — sources, shells, core, GPU | `flowchart` | `docs/how-it-works.md` (moved from `README.md`) |
| The frame — PCM → ring → analysis → variables → expressions → scene → composite chain → present | `flowchart` | `docs/how-it-works.md` |
| A preset's life — file → parse → compile → load, hot-reload, rejection, dissolve | `stateDiagram-v2` | `docs/presets.md` |
| The embedding lifecycle — host and core across the C ABI, two threads | `sequenceDiagram` | `docs/embedding.md` |

A fifth diagram needs a reason in its commit message, and a diagram over about twelve nodes is two
diagrams.

## Consequences

### Positive

- One source, two renderers, no copy: GitHub renders the fence, the site renders the SVG, and the
  diagram is edited in one place.
- No JavaScript reaches the reader, and the dark theme is handled by the renderer rather than by a
  hand-tuned stylesheet.
- A diagram that fails to parse fails the site build, which is the same place a broken link fails.

### Negative

- **The Pages workflow installs a browser.** `playwright install chromium` is on the order of
  150 MB and adds tens of seconds cold; it is cached across runs by the same mechanism that caches
  `node_modules`. A local `npm run build` needs the same one-time install, and `site/README.md`
  says so.
- **`rehype-mermaid` and `playwright` are two more npm dependencies**, pinned exactly like the four
  the site already has. They are build-time only and nothing shipped depends on them, which is the
  same posture as `site/` itself.
- **Mermaid's SVG carries its own inline styles**, and Starlight's typography does not reach inside
  it. Font and colour are set by the renderer's theme configuration, in one place in
  `astro.config.mjs`, and the plan's human phase checks the result on both themes.

### Neutral

- The fence renders slightly differently on GitHub and on the site, because they are two mermaid
  versions. The content is the same; the pixels are not, and nothing gates the pixels.

## Alternatives considered

### Alternative A — Client-side mermaid

A script on every page that finds `pre.mermaid` blocks and renders them in the browser. Rejected
because it ships mermaid's runtime (over a megabyte) to every reader for four diagrams, renders
after the page paints so the layout jumps, and has to be told the theme by hand on every toggle.
It is what most Starlight sites do, and it is the wrong trade for a site with four diagrams and a
"lightweight" project behind it.

### Alternative B — Committed SVGs, rendered by a script

`mmdc` (the mermaid CLI) renders each fence to `docs/images/*.svg`, the source links the image, and
the site publishes it as it publishes every other picture. Rejected on two counts. The source would
carry an image link *instead of* the fence, so GitHub would stop rendering the diagram natively and
an edit would need a regeneration step — the exact copy-that-drifts ADR-0154 exists to prevent.
And `mmdc` needs the same headless Chromium, so nothing is saved.

### Alternative C — No diagrams; prose and tables only

The site is readable without them. Rejected because the brief asks for them, and because two of the
four — the frame pipeline and the embedding sequence — are the two things a reader of the *public
API* most needs to see rather than read: what order the host calls the ABI in, and where a
parameter's value comes from.

## Outcome (2026-09-06, at [Plan 0156](../plans/done/0156-the-site-becomes-the-reference.md)'s close)

**The mechanism landed as decided and *"the diagram follows the theme"* is true of the reader's OS
and not of the site.** The Decision's `<picture>` selects its source with a `prefers-color-scheme`
media query; Starlight's theme control sets `data-theme` on the root element, which a `<source>`
cannot read. So a reader on a light OS who toggles the site to dark gets the light rendering. That
is precisely the capability Alternative A was rejected for needing — *"has to be told the theme by
hand on every toggle"* — and the cost of not having it is a mismatch this ADR did not anticipate.
The four diagrams ship; the toggle case is what Plan 0156's Phase 8 walk exists to judge.

**The build-failure property the plan asked of this mechanism does not hold on its own.** Astro's
content layer renders each entry inside a try and stores an entry whose chain threw **without its
html**, so a malformed fence produces a page with a title, a menu entry and an empty body, and the
build exits 0. The site build gained an `astro:build:done` hook that fails on any empty page, which
is what makes a broken fence red — and which immediately caught a page shipped empty by an earlier
phase for an unrelated reason, a stale fragment link. **A renderer that swallows its own failure
needs a gate on the artifact, not on the renderer.**
