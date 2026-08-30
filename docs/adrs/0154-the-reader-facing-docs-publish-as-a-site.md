# ADR-0154 — The reader-facing docs publish as a site, and `docs/` stays the single source

> **Status:** proposed
> **Date:** 2026-08-30
> **Related plan(s):** [0143](../plans/0143-the-documentation-gets-a-front-end.md)

## Context

The documentation corpus is 6,714,932 bytes across 315 markdown files, and it is two documents
wearing one hat. About 5.5 MB is the **working record** — 154 ADRs, 144 plans,
`design-backlog.md` and `design-backlog-archive.md` at 755 KB between them,
`plans/README-archive.md` at another 508 KB. That material is written to be grepped by fresh
`architect` / `dev` / `preset-author` sessions, and it is well served by being plain files.

The remaining ~1 MB is **reader-facing**, and it is straining against the one renderer it has.
Measured 2026-08-30:

| File | Bytes | What it is |
|---|---:|---|
| `presets/README.md` | 263,729 | the per-system parameter roster |
| `docs/capturing.md` | 150,770 | the `shot` CLI and the visual-QA harness |
| `docs/presets.md` | 75,355 | the expression-language reference |
| `docs/preset-palettes.md` | 63,867 | the colour surface |
| `README.md` | 35,388 | the repository front door |
| `docs/on-device-validation.md` | 37,241 | the manual checklist |
| `docs/nfr.md` | 29,138 | the quantified budgets |

The roster is the sharpest case. `presets/README.md` is the catalogue the `preset-author` lane is
pointed at *instead of* keeping a copy of its own, and that indirection is deliberate — the private
copies rotted while this file stayed current. It is a **lookup table read non-linearly**, it is
264 KB, and the only way to find a parameter in it today is the browser's find-in-page over a
quarter-megabyte document that GitHub truncates. The same shape holds for the grammar reference and
the palette surface. None of these are long because they are padded; they are long because they are
complete, and completeness is the property we want to keep.

**The decisive constraint is not rendering — it is linking.** The reader-facing set contains 1,059
relative markdown links, and 926 of them (87 %) point at documents that are *not* in the
reader-facing set: 451 into `adrs/`, 387 into `plans/`, 87 into `design-backlog`, 1 into `specs/`.
Any site that publishes the reader-facing subset must resolve those 926 somewhere, and it may not
resolve them by editing the source: [`scripts/check-doc-links.mjs`](../../scripts/check-doc-links.mjs)
asserts that every relative link resolves on disk, it runs first in `.githooks/pre-push` and again
in CI's `links` job, and the same relative form is what makes these documents navigable in an editor
and on GitHub. Rewriting them to absolute URLs would trade a working gate and working local
navigation for a working site.

Finally, [Plan 0103](../plans/0103-the-project-gets-an-audience.md) — approved 2026-08-16, still
open — states in its exclusions: *"No website, no landing page, no domain. The repository is the
landing page."* That plan is distribution work, its Phase 2 owns the README, and its exclusion was
written against a **promotional** site. A documentation site is a different artifact aimed at people
who have already arrived, but the two meet at the front door, and that collision has to be decided
rather than assumed away.

## Decision

We will publish the reader-facing subset of the documentation as a static site built with **Astro
Starlight** and hosted on GitHub Pages, and **`docs/` remains the single source**: no markdown file
is copied, moved, forked, or edited to serve the site, and every transformation the site needs
happens at build time in a remark plugin. Links whose targets are inside the published set are
rewritten to site routes; the 926 whose targets are outside it are rewritten to absolute GitHub blob
URLs at a pinned ref. The working record — plans, ADRs, the design backlog, the archives — is not
published and stays exactly as it is.

The site does not touch `README.md`. Plan 0103 keeps the repository front door; the site's home page
is a **router** — two entrance cards and a gallery strip — and carries no product prose of its own,
so there is no second copy of the pitch to drift. This **narrows** 0103's exclusion rather than
reversing it: still no promotional landing page, still no domain, and the repository remains the
place a stranger arrives.

**Hosting arrives in two stages, and the first is deliberately manual.** The demo is published by
copying a local build into `public/lmv/` of the existing personal user site
(`IgorKonovalov/IgorKonovalov.github.io` — public, default branch `master`, and itself an Astro 5
static site deployed by its own workflow). That site's Pages deployment serves a build artifact
rather than the repository tree, and `public/**` is the one directory Astro copies verbatim into it,
so a prebuilt subdirectory published this way needs **no Pages configuration on this repository, no
change to the host's workflow, and no `.github/workflows/` edit anywhere**. Only once the site has
earned its place does it move to a Pages deployment on the project repository under CI. Both stages
are GitHub Pages; the difference is who runs the build and how often. The subpath forces Astro's
`base` to be configured from the very first build, which is the same thing the eventual project-site
path requires, so nothing about the first stage is throwaway.

## Consequences

### Positive

- **The roster becomes searchable.** Full-text search over 264 KB of parameter tables, 75 KB of
  grammar and 64 KB of palette surface is the single largest readability gain available here, and it
  is unavailable in principle from a markdown file on GitHub.
- **The `preset-author` lane's indirection gets stronger.** That skill points at these three
  documents instead of holding its own catalogue. Making them navigable makes the indirection
  cheaper to follow, which is what keeps the private copies from coming back.
- **The gallery can be exhaustive rather than curated.** 81 presets ship; the gallery in
  `docs/images/gallery/` holds 9 images, one per *system*. A site page has room for one card per
  preset, which no markdown file does.
- **One source keeps every existing gate valid.** `check-doc-links.mjs`, `check-index-rows.mjs`,
  `check-backlog-claims.mjs` and the pre-push hook run against an unchanged tree and need no
  awareness that a site exists.

### Negative

- **This is the repository's first `package.json` and first npm dependency tree.** Every gate here
  is a bare `.mjs` run by `node` with no dependencies at all, and the one Python sidecar
  (`tools/sd-filter/`) is quarantined outside the workspace. Starlight is a real toolchain: a lockfile
  to maintain, transitive dependencies to update, and a build that can break for reasons unrelated to
  any documentation change. **This is the price of the choice and it is not small.** It is bounded by
  the same rule that bounds `milkconv/` and `tools/sd-filter/` — the site is never shipped and no
  shipped artifact depends on it — but the maintenance is real.
- **A second renderer means markdown can render two ways.** Anything GitHub tolerates and Astro does
  not (or vice versa) becomes a defect visible only on one of the two surfaces. The corpus uses
  mermaid in exactly one reader-facing file, which bounds the worst of this, but tables, footnotes
  and raw HTML remain a live divergence risk.
- **The link rewrite is invisible in the source.** A reader of `docs/presets.md` sees a relative
  link; the site serves a GitHub URL. When the rewrite is wrong, nothing in the tree shows it and
  `check-doc-links.mjs` still passes, because the source was never the thing that broke. This is why
  the plan owes a gate that checks the *built* site rather than the source.
- **The pinned ref in the off-site URLs is a staleness surface.** 926 links point into the repository
  at some ref. Pinned to a tag they rot as documents move; pinned to `main` they can 404 between a
  site deploy and a subsequent rename.
- **The site can silently fall behind `docs/`.** A documentation edit that never triggers a rebuild
  produces a site that is confidently wrong, which is worse than no site. The manual first stage has
  this property by construction — a hand-copied build is stale the moment the next doc commit lands
  — which is acceptable for a demo and is the specific debt the second stage exists to pay off.
- **The demo stage puts derived output in a second repository.** That is consistent with the
  one-source rule (it is generated, git-ignored here, and never edited), but it means the demo URL
  and this repository can disagree with nobody noticing, and that the personal site's own history
  accumulates build output.

### Neutral

- GitHub Pages must be enabled on the repository, which is a one-time human action outside any
  automation.
- The published set is a curation decision that will need revisiting as documents are added; a new
  file under `docs/` does not join the site by existing.

## Alternatives considered

### Alternative A — MkDocs with the Material theme

The strongest technical fit and the recommendation this ADR did not take. `docs_dir: docs` is
MkDocs' own default, so the source tree would not move at all; relative `.md` links are resolved to
built pages natively; an `on_page_markdown` hook of roughly thirty lines covers the 926-link
boundary; search is built in; and Python is already a precedent in this repository through
`tools/sd-filter/requirements.txt`. It lost on presentation and on future per-release versioning,
which the user weighed higher than the toolchain cost. Recording it here because if the npm tree
becomes a burden, this is the migration target and the reason is already written down.

### Alternative B — mdBook

Rust-native, installable with `cargo install mdbook`, and it would add no new language to a Rust
repository. It lost on a structural conflict: mdBook requires a `SUMMARY.md` and a `src/` root, so
`docs/` would have to move or be staged into a second tree — which is precisely the split-source
hazard this decision exists to avoid — and its preprocessor interface is the least convenient of the
three for the link rewrite that dominates the work.

### Alternative C — keep the repository as the only surface (Plan 0103's position)

The status quo, and a defensible one: GitHub renders markdown, the links already resolve, the gates
already hold, and there is no toolchain. It lost to a single measurement. A 264 KB parameter roster
with no search is not a document anyone reads; it is a document people give up on, and the lane most
dependent on it is the one this project keeps pointing *away* from private copies. No amount of
markdown discipline fixes find-in-page over a quarter of a megabyte.

### Alternative D — rewrite the 926 links to absolute URLs in the source

This would let any generator work unmodified. It loses the `check-doc-links.mjs` gate, which is the
only thing standing between this corpus and the 74 broken links across 23 files that accumulated the
last time link discipline was left to attention, and it breaks navigation in the editor and on
GitHub for every one of the 926. The gate and local navigation are worth more than generator
freedom.

## Notes

Measurements in this ADR were taken on 2026-08-30 against the working tree at commit `a94fd18`.
The 1,059/926 link split was produced by extracting relative `.md` link targets from `README.md`,
`docs/*.md` and `presets/README.md` and bucketing them by destination directory.

The one-source rule here is the same rule [ADR-0022](0022-build-time-preset-embedding.md) applied to
presets: the tree is the source, the build derives from it, and nothing derived is ever edited by
hand. A staged copy of the markdown under the site tree does **not** violate this decision provided
it is generated on every build, git-ignored, and never edited — the hazard is a second *source*, not
a second *file*.
