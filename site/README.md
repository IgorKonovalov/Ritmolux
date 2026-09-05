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
  `src/plugins/rewrite-links.mjs` **and** to the sidebar in `astro.config.mjs`, or it stays
  unpublished. **Both omissions are caught**, and each by a different gate: a sidebar slug with no
  `PUBLISHED` entry fails the build, and a `PUBLISHED` entry with no sidebar item - a page that
  builds, gets indexed, and is reachable only by search - is convicted by
  `scripts/check-site-routes.mjs`, which runs in the Pages workflow because it needs a built site.

The build-time transformations, all of them here, because the sources carry no frontmatter and
never will:

| Where | What |
|---|---|
| `src/content.config.ts` | derives each page's `title` from its opening heading, ATX or setext |
| `astro.config.mjs` | drops that heading from the body, so Starlight's `<h1>` is not doubled |
| `astro.config.mjs` | substitutes `@VERSION@` from the workspace version, for the packaging pages |
| `astro.config.mjs` | wraps every table in a scroll container, so a wide table scrolls and the page does not |
| `src/plugins/strip-provenance.mjs` | drops a trailing `(Plan NNNN)` / `(ADR-NNNN)` from headings and block ends, before slugs are computed (ADR-0168) |
| `src/plugins/split-document.mjs` | cuts a document past the size threshold into one route per section, and emits the fragment map that keeps deep links resolving (ADR-0166 owns both constants) |
| `src/plugins/rewrite-links.mjs` | rewrites every relative link: inside the published set to a site route, outside it to a GitHub URL (ADR-0154) |

## Working on it

```sh
cd site
npm install     # first time only
npm run dev     # http://localhost:4321/ritmolux/
npm run build   # -> site/dist/
```

`base` is `/ritmolux/`, so the dev server serves under that subpath too - a bare
`http://localhost:4321/` is a 404 by design, not a fault.

## What is published

Three groups - get it, use it, understand it - listed in full in `PUBLISHED`
(`src/plugins/rewrite-links.mjs`) and ordered in the sidebar (`astro.config.mjs`). The install pages
ARE the three `packaging/*/READ-ME-FIRST.md` a tester finds inside the release zip, published as
they ship rather than rewritten, so a drift between the two is unrepresentable (ADR-0167). The
working record - plans, ADRs, the design backlog and both archives - is **not** published; links
into it are rewritten to GitHub URLs at build time.
