# ADR-0167 — The site owns its entrance, and the install page is the tester's own file

> **Status:** proposed
> **Date:** 2026-09-05
> **Related plan(s):** [0154](../plans/0154-the-site-becomes-navigable.md)
> **Narrows:** [0154](0154-the-reader-facing-docs-publish-as-a-site.md) — the router clause

## Context

[ADR-0154](0154-the-reader-facing-docs-publish-as-a-site.md) decided that *"the site's home page is
a **router** — two entrance cards and a gallery strip — and carries no product prose of its own, so
there is no second copy of the pitch to drift."* That clause was written to protect
[Plan 0103](../plans/0103-the-project-gets-an-audience.md), which owns `README.md` and excludes
*"No website, no landing page, no domain. The repository is the landing page."*

The site went live on 2026-09-05 with exactly that shape, and the shape has a hole in it. Of the 15
published routes, **not one says what Ritmolux is, how to obtain it, or how to run it.** A stranger
who arrives at the deployed URL sees a tagline — *"Every reference this project has, with a search
box in front of it"* — a gallery strip, and two cards that route into reference material written for
someone who already has the application. The tagline is accurate, and that is the tell: the site
documents a program the site cannot help you get.

The router clause assumed the repository would always be the arrival point. That assumption held
while the site was an internal convenience. It stops holding the moment the site has a URL people
can be sent, which it now does.

Meanwhile the installation instructions already exist, are already maintained, and are already the
version testers actually follow. `packaging/` holds three of them, shipped inside the release zips
by [ADR-0038](0038-tag-driven-release-unsigned-universal-mac-app.md):

| File | Bytes | Audience |
|---|---:|---|
| `packaging/windows/READ-ME-FIRST.md` | 4,398 | the standalone on Windows |
| `packaging/macos/READ-ME-FIRST.md` | 4,430 | the standalone on macOS, unsigned-build steps included |
| `packaging/foobar/READ-ME-FIRST.md` | 4,797 | the foobar2000 component |

None of them is published. Any installation prose written for the site would therefore be a
**fourth** copy of instructions that already exist in three places, and the copy least likely to be
updated when the real ones change — because the real ones change as part of packaging, which is
where someone is already looking.

## Decision

The site gains an authored entrance: a landing page that says what Ritmolux is and what it looks
like, and a **Start here** route that orients a newcomer and hands them off. This **narrows**
ADR-0154's router clause rather than reversing it — the site carries *orientation* prose, and it
carries no *installation* prose at all.

The three `packaging/*/READ-ME-FIRST.md` files join `PUBLISHED` and become the install pages. They
are the same bytes a tester finds in the zip, so the site and the download cannot disagree. The
download link points at `https://github.com/IgorKonovalov/Ritmolux/releases/latest` — a redirect
that names no version and therefore never goes stale.

Two mechanical facts about those files have to be handled at build time, and neither is a reason to
copy them:

- **They use setext headings** (`Ritmolux - Windows` over a rule of `=`), which the title-deriver in
  `site/src/content.config.ts` does not match — its regex requires a leading `# `. The deriver
  learns setext rather than the files learning frontmatter.
- **They carry an `@VERSION@` placeholder** substituted during packaging. The site substitutes the
  workspace version at build time; it must not display the raw token.

Plan 0103 keeps `README.md` and keeps the repository as a landing page. What changes is that it is
no longer the *only* one, and this ADR records that as a deliberate narrowing of that plan's
exclusion, decided while the plan is still open.

## Consequences

### Positive

- **The site can be sent to someone.** A single URL now answers "what is this", "what does it look
  like", "how do I get it" and "how do I use it" in that order, which no surface in this project did
  before.
- **Installation truth has exactly one source per platform**, and it is the source that ships inside
  the artifact the reader downloads. A drift between the site and the zip is not merely unlikely; it
  is unrepresentable.
- **The `@VERSION@` substitution makes the site version-aware where it matters.** The install pages
  name the version they were built from, which is the one place on a current-`main` site where a
  version genuinely helps.

### Negative

- **The site now carries prose that can drift from `README.md`.** This is precisely the cost
  ADR-0154's router clause was written to avoid, and it is being accepted rather than solved. The
  mitigation is a boundary, not a mechanism: the site's prose says what the thing *is* and where to
  go, and never how to install, configure or operate it. Both surfaces still need a human to keep
  them consistent about the pitch.
- **It further narrows an open plan's exclusion.** Plan 0103 is approved and unstarted; its Phase 2
  owns the README. Whoever takes that plan inherits a front door that now has a second entrance, and
  the coordination is manual.
- **`packaging/` becomes reader-facing.** Those files were written for someone holding a zip. Read
  on a website by someone who has not downloaded anything yet, the framing is slightly wrong —
  *"unzip anywhere and run ritmolux.exe"* assumes a step the reader has not taken. The Start-here
  page carries that step; the packaging files are not rewritten for the site.
- **A fifth and sixth file join the published set** without joining `docs/`, which weakens the tidy
  claim that the site publishes "the reader-facing subset of `docs/`". The published set is now
  `docs/` plus `presets/README.md` plus `packaging/*/READ-ME-FIRST.md`.

### Neutral

- The landing page keeps the gallery strip and the two entrance cards; they move below the
  introduction rather than being replaced.

## Alternatives considered

### Alternative A — publish `README.md` into the site

Zero drift by construction, since the site and the repository would share one file. It lost on two
counts. It collides directly with Plan 0103, which owns that file and is still open, so the site
would be publishing a document another plan is about to rewrite. And a README is written for someone
already looking at a repository — it opens with badges, layout and build instructions, which is the
wrong first paragraph for a visitor who wanted to know what the program does.

### Alternative B — authored installation steps in `site/`

The obvious approach, and the one that creates a fourth copy of instructions that already exist in
three maintained places. Rejected on the same reasoning that keeps the `preset-author` lane pointed
at `presets/README.md` instead of holding a private catalogue: the private copies rot while the
shared one stays current.

### Alternative C — a richer landing page and no new routes

One page carrying the whole introduction. It avoids a new route and keeps the surface small, but it
either stays too shallow to help or grows into a page that is a site of its own, and it still leaves
installation unanswered or duplicated.

### Alternative D — keep the router clause, do nothing

Defensible while the site had no audience. The site now has a public URL at the project's own
repository, which is a change of circumstance rather than of opinion — ADR-0154's clause was
correct for the artifact it described and is being narrowed for the one that now exists.
