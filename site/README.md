# `site/` - the documentation front end

An [Astro Starlight](https://starlight.astro.build/) site that publishes the **reader-facing
subset** of this repository's documentation with real full-text search. Built by Plan 0143 under
ADR-0154.

It is never shipped, and no shipped artifact depends on it - the same rule that bounds `milkconv/`
and `tools/sd-filter/`. A broken site build is a documentation problem, never a release blocker.

## The one rule

**`docs/` and `presets/` are the single source, and nothing here copies them.**

`src/content.config.ts` points Astro's content loader at the repository root, one directory above
this one, and loads the published set from the `PUBLISHED` map in `src/plugins/rewrite-links.mjs`,
which is where that list is declared - beside the link rewrite that reads the same boundary, so the
loader and the rewrite cannot disagree about what is published. There is no staged copy, no synced folder, and no
second file to drift. Two consequences follow, and neither is optional:

- **No markdown file outside `site/` may be edited to serve the site.** Every transformation the
  site needs happens at build time, in this project. If a page needs something the source does not
  have, the fix goes in a plugin here.
- **A new file under `docs/` does not join the site by existing.** Add it to `PUBLISHED` in
  `src/plugins/rewrite-links.mjs` - as `{ route, title }` - **and** to the sidebar in
  `astro.config.mjs`, or it stays unpublished. The sidebar takes the source path alone: the label
  comes from the same `title`, so a menu entry and a page heading cannot disagree (ADR-0169). **Both omissions are caught**, and each by a different gate: a sidebar slug with no
  `PUBLISHED` entry fails the build, and a `PUBLISHED` entry with no sidebar item - a page that
  builds, gets indexed, and is reachable only by search - is convicted by
  `scripts/check-site-routes.mjs`, which runs in the Pages workflow because it needs a built site.

The build-time transformations, all of them here, because the sources carry no frontmatter and
never will:

| Where | What |
|---|---|
| `src/content.config.ts` | takes each page's `title` from `PUBLISHED`, falling back to its opening heading, ATX or setext |
| `astro.config.mjs` | drops that heading from the body, so Starlight's `<h1>` is not doubled |
| `astro.config.mjs` | substitutes `@VERSION@` from the workspace version, for the packaging pages |
| `astro.config.mjs` | wraps every table in a scroll container, so a wide table scrolls and the page does not |
| `src/plugins/strip-provenance.mjs` | drops a trailing `(Plan NNNN)` / `(ADR-NNNN)` from headings and block ends, before slugs are computed (ADR-0168) |
| `src/plugins/split-document.mjs` | cuts a document past the size threshold into one route per section, and emits the fragment map that keeps deep links resolving (ADR-0166 owns both constants) |
| `src/plugins/rewrite-links.mjs` | rewrites every relative link: inside the published set to a site route, outside it to a GitHub URL (ADR-0154) |
| `src/plugins/rewrite-links.mjs` | renames a link whose whole text is the target's path to the target's declared title, when the target is published (ADR-0169) |
| `rehype-mermaid`, configured in `astro.config.mjs` | renders each mermaid fence to a `<picture>` with a light and a dark SVG, at build time, with no script on the page (ADR-0171) |

## Working on it

```sh
cd site
npm install                        # first time only
npx playwright install chromium    # first time only - the diagram renderer
npm run dev                        # http://localhost:4321/ritmolux/
npm run build                      # -> site/dist/
```

**The browser is not optional.** `rehype-mermaid` renders every ```` ```mermaid ```` fence to an
SVG at build time through a headless Chromium (ADR-0171), and a build without it fails rather than
serving a page with a hole in it. It is a one-time install into a machine-local cache rather than
into `node_modules`; the Pages workflow installs and caches it the same way.

`base` is `/ritmolux/`, so the dev server serves under that subpath too - a bare
`http://localhost:4321/` is a 404 by design, not a fault.

## The empty-page guard

`astro.config.mjs` registers an `astro:build:done` hook that **fails the build if any page rendered
to an empty body**, and it is load-bearing rather than defensive. Astro's content layer renders each
entry inside a try: an entry whose remark or rehype chain throws is stored *without* its html and
the build continues, so the page ships with a title, a menu entry, a search index entry and nothing
under it.

Two of this project's own rules throw exactly that way — `rewrite-links.mjs` on a relative target
that does not resolve, and `rehype-mermaid` on a fence that does not parse — and both are supposed
to fail the build. Before the hook, neither did.

## What is published

Six groups, each named for what a reader is doing rather than for where the file came from
(ADR-0169) - **Get it**, **Use it**, **Author presets**, **Embed it**, **How it works**,
**Contribute** - listed in full in `PUBLISHED` (`src/plugins/rewrite-links.mjs`) and ordered in the
sidebar (`astro.config.mjs`). The install pages
ARE the three `packaging/*/READ-ME-FIRST.md` a tester finds inside the release zip, published as
they ship rather than rewritten, so a drift between the two is unrepresentable (ADR-0167). The
working record - plans, ADRs, the design backlog and both archives - is **not** published; links
into it are rewritten to GitHub URLs at build time.
