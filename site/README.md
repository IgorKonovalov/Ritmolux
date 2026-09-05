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
  unpublished. Only one of those two omissions is caught: a sidebar slug with no `PUBLISHED` entry
  fails the build, while a `PUBLISHED` entry with no sidebar item builds a page reachable only by
  search, and every gate passes it.

Two build-time transformations exist because the sources carry no frontmatter and never will:

| Where | What |
|---|---|
| `src/content.config.ts` | derives each page's `title` from its opening `# ` heading |
| `astro.config.mjs` | drops that heading from the body, so Starlight's `<h1>` is not doubled |

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

Two entrances, listed in full in `PUBLISHED` (`src/plugins/rewrite-links.mjs`) and ordered in the
sidebar (`astro.config.mjs`). The working record - plans, ADRs, the design backlog and both archives - is
**not** published; links into it are rewritten to GitHub URLs at build time.
