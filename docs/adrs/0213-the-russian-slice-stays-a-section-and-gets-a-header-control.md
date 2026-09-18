# ADR-0213 — The Russian slice stays a section, and gets a header control

> **Status:** proposed
> **Date:** 2026-09-17
> **Related plan(s):** none yet
> **Amends:** [0185](0185-the-docs-translate-a-slice-and-a-stamp-makes-staleness-visible.md)
> (Alternative C, re-examined against the installed Starlight; the slice, the stamp and the
> advisory are untouched)

## Context

[ADR-0185](0185-the-docs-translate-a-slice-and-a-stamp-makes-staleness-visible.md) shipped five
Russian pages as a `Русский` sidebar group with a per-page twin link, and rejected Starlight i18n
locale routing in one sentence: five translated pages out of 164 means `/ru/` serves a site that is
97 % English behind Russian chrome.
[Plan 0166](../plans/done/0166-the-basics-read-in-russian.md) landed it. Within days **the owner
could not find how to switch languages** — the affordance was a plain link in the page's first
paragraph, and commit `90b005e8` made it a blockquote before this decision was taken. A rejected
alternative that the person who chose it then reaches for is worth re-opening with evidence rather
than with a sentence.

Three of the four questions that decide it are answerable from the installed package
(`@astrojs/starlight` 0.42.0), and two go the *opposite* way to what the one-line rejection implies.

**Locale routing would not force a staged copy.** `dist/utils/routing/index.js` derives a route's
locale from the first path segment of the content entry's `id`, and this site's `docs` collection is
served by three custom loaders that synthesize every `id` (`site/src/content.config.ts`) rather than
globbing a directory tree. Nothing would move on disk, and
[ADR-0154](0154-the-reader-facing-docs-publish-as-a-site.md)'s read-in-place rule is not the
obstacle.

**The stamps are indifferent to the question.** `scripts/check-translations.mjs` pairs
`<name>.ru.md` with `<name>.md` on disk and reads git history; it never reads the site. As long as a
translation stays a sibling of its source — which neither shape requires changing — ADR-0185's stamp
mechanism is untouched. That axis argues for neither option.

**Fallback is the whole decision, and it is not configurable.** For every non-default locale,
Starlight pushes a route at `ru/<id>` for each default-locale entry with no Russian counterpart,
carrying the **English** entry and `isFallback: true`; `Page.astro:111` then renders
`FallbackContentNotice`, which in Russian reads *«Это содержимое пока не доступно на вашем языке.»*
(`dist/translations/ru.js:17`). The loop takes no option, and `defaultLocale`'s own documentation
says so: *"The default locale will be used to provide fallback content where translations are
missing"* (`dist/utils/user-config.d.ts:53`).

The arithmetic, measured on the tree at `90b005e8` on 2026-09-17: the published set is **24 English
sources contributing 187 routes** and **5 Russian sources contributing 5**, plus the three pages the
site owns. Under `locales: { root: en, ru }` that becomes about **190 English routes and 190
Russian ones, of which 185 are an English document served at a Russian URL** under that notice — and
Pagefind indexes every one, so a Russian search would return mostly English pages announcing that
they are not in Russian.

**And the control the owner went looking for does not need any of it.** `LanguageSelect` is an
overridable component slot, rendered unconditionally by `Header.astro` beside the theme toggle and
again by `MobileMenuFooter.astro` in the mobile menu; only its *default* implementation
self-suppresses behind `config.isMultilingual`. The place a reader looks is reachable on a
monolingual site.

## Decision

**We do not migrate to Starlight i18n locales.** The Russian slice stays five real pages in a
`Русский` sidebar group, and **the language affordance moves into the site header** as an override
of the `LanguageSelect` component slot — one override, which Starlight places in the desktop header
and in the mobile menu footer.

The control renders on every page. Where the current page has a Russian twin it links to that twin;
where it does not, it links to the Russian section. It **derives the twin from the `PUBLISHED` map's
`<name>.md` / `<name>.ru.md` sibling pairing**, exactly as `translationCrossLink` in
`site/astro.config.mjs` already does, so a translation added to that map gains its header entry with
no edit here and a renamed route cannot leave it pointing at nothing. The per-page twin link stays:
the header control answers *arriving on the site*, the in-page link answers *reading this page*, and
they are different moments.

**The trigger for revisiting is a property, not a fraction we have not earned: locale routing
becomes the right shape when a Russian reader following the site's own links lands on a translated
page more often than on an untranslated one — mechanically, when the fallback count would fall below
the translated count.** Today that is 185 against 5, and nothing planned moves it.

## Consequences

### Positive

- **The control is where a reader looks**, in both viewports, from one override — Starlight decides
  the placement and we do not reason about breakpoints. The `Русский` group stays as the section's
  own entrance.
- **No route is manufactured with no content behind it.** The site keeps ~190 routes instead of
  ~380, Pagefind's index does not double, and no URL exists whose only content is a notice saying it
  has none.
- **The five Russian URLs do not move.** A migration renames every one of them
  (`ru/install-windows` → `ru/install/windows`, and so on), and this site has no redirect machinery.
- **`scripts/check-site-routes.mjs` keeps its shape.** It reads the sidebar from one built page and
  requires every built route to appear in it; under locales the 185 fallback routes carry a
  localized menu the gate would not be reading, and all 185 would report as orphans. The gate would
  have to become locale-aware — real work, in a gate whose whole value is that it is simple enough
  to trust.
- **ADR-0185 stands as written.** Slice, stamp, banner and close-ceremony advisory are unchanged.

### Negative

- **The five Russian pages are served with `lang="en"`, and this decision does not change that.**
  `Page.astro:50` sets `<html lang>` from the route's locale, and a monolingual site has exactly
  one. A screen reader announces Russian prose in an English voice, and a search engine reads those
  pages as English. **This is the one thing the migration would have fixed that we are not fixing.**
  A `Head` component override is the candidate repair and it is **unverified** — nobody has
  established that the head can be given a per-entry `lang` from data an override actually receives.
  Whoever builds the header control should probe it in the same sitting: if it is cheap, take it,
  and if it is not, this cost stands recorded rather than repaired.
- **A hand-written control is a second implementation of something the framework ships**, and
  nothing gates it. Deriving the twin from `PUBLISHED` removes the list that would rot, but not the
  component. The backstop is incidental rather than designed: the control renders into every page,
  so a target that resolves to nothing fails `scripts/check-site-links.mjs` on ~190 pages at once.
- **A Russian reader on an untranslated page is sent to a section, not to the page they wanted.**
  That is the same information the fallback notice carries, delivered as a shorter walk instead of
  as a page. It is honest and it is not satisfying.
- **This decision is clear at 185-to-5 and says nothing about 100-to-90.** The revisit trigger names
  the crossing; it does not pre-compute the answer there, and someone will have to do that work.

### Neutral

- No change to [ADR-0166](0166-a-published-document-splits-into-routes-by-size.md)'s route-ceiling
  arithmetic: the slice and its sources are the same five files.
- The work is `dev`'s by precedent — Plan 0166 tagged every `site/` phase `dev`, and the owner-skill
  vocabulary still has no lane named for the site.

## Alternatives considered

### Alternative A — Migrate to Starlight i18n locales

A header picker Starlight maintains, `/ru/` URLs, `lang="ru"` on the five translated pages, and a
sidebar localized from one config through per-item `translations`. It loses on what comes with it
and cannot be declined: 185 English documents published at Russian URLs under a notice saying they
are not Russian, a doubled Pagefind index, every existing Russian URL renamed, a locale-aware
rewrite of `check-site-routes.mjs`, and about thirty sidebar labels needing Russian strings that no
gate can check. It buys one affordance and one `lang` attribute at the price of making 97 % of the
Russian site an announcement of its own absence.

### Alternative B — Migrate, and suppress the fallback

Configure the locales and stop Starlight serving English at `/ru/`. There is no setting: the loop
that builds those routes takes no option, and `defaultLocale`'s documentation says the default
locale *is* the fallback. Suppression means forking `getRoutes` or filtering `dist/` after the
build — buying a header picker at the price of owning the router, in the one part of this site that
is entirely someone else's code.

### Alternative C — Move `Русский` to the top of the sidebar and change nothing else

One line, no new component. Rejected because the menu is not where the miss happened: the group is
the seventh of seven, below a menu whose largest document alone contributes dozens of rows, and on a
narrow viewport the whole sidebar is behind a button while the theme toggle — the control a reader
pattern-matches a language switch to — sits in the menu footer with the language slot empty beside
it. Moving the group up is compatible with this decision and is not a substitute for it.

### Alternative D — Leave it: the twin link is a blockquote now

`90b005e8` had already turned the in-page link into a bordered, tinted control before this question
was asked, so the cheapest response is to wait and see whether that was enough. Rejected because the
in-page link, at any weight, is absent from the ~185 pages with no twin and absent from the moment
that matters most — a reader arriving from search or from the component zip, deciding whether this
site has anything for them at all.

## Notes

Everything asserted about Starlight here was read from `site/node_modules/@astrojs/starlight` at
version 0.42.0 on 2026-09-17, not from its published documentation: `dist/utils/routing/index.js`
(locale from the entry id, the unconditional fallback loop), `dist/components/Page.astro` (the
`lang` attribute, the fallback notice), `dist/components/Header.astro` and
`dist/components/MobileMenuFooter.astro` (the `LanguageSelect` slot in both), and
`dist/components/LanguageSelect.astro` (the default's `isMultilingual` guard). A version bump can
falsify any of it, and the fallback loop is the one to re-read first.

The route counts were computed by running `splitDocument` over the `PUBLISHED` map at `90b005e8`,
which is the same code `scripts/check-site-routes.mjs` uses to enumerate routes without a build.
ADR-0185 wrote 164 routes and this ADR writes 192; the corpus grew, and neither number is wrong for
its date.
