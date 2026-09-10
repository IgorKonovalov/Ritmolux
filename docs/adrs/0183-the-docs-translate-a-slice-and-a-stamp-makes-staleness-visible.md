# ADR-0183 — The documentation translates a slice, and a stamp makes staleness visible

> **Status:** proposed
> **Date:** 2026-09-10
> **Related plan(s):** [0166](../plans/0166-the-basics-read-in-russian.md)
> **Extends:** [0154](0154-the-reader-facing-docs-publish-as-a-site.md) (`docs/` is the single
> source, read in place), [0167](0167-the-site-owns-its-entrance-and-the-install-page-is-the-testers-own-file.md)
> (the site's install page is the tester's own file)

## Context

The site is English-only, and the audience that most needs it is not. The foobar2000 component is
the frontend with a large Russian-speaking user base, and it is also the path where a reader who
stalls stalls on something language-shaped: an unsigned binary, a SmartScreen dialog, dropping a
`.fb2k-component` into the right directory. None of that is helped by fluency in the expression
grammar.

The corpus resists translation, and the resistance is arithmetic rather than principle. The
published set is **149,178 words across 23 sources**. Git touched those sources with **1,339
commits in 60 days, on 48 distinct days**; nine of them alone took **+6,848 / −1,070 lines in 30
days**. `presets/README.md` is **53,792 of those words — 36 % of the corpus** — and since
[ADR-0170](0170-a-parameters-reference-row-is-generated-from-the-declaration-the-engine-reads.md)
its per-parameter rows are generated from `ParamSpec` declarations, so translating the reference
means putting Russian doc strings inside `core/`, or shipping a page whose tables are English and
whose essays are not.

A translation also contradicts the axiom the documentation architecture rests on. Every second copy
in this repository is either **eliminated by generation** — the C ABI header
([ADR-0169](0169-the-site-is-organised-by-reader-task-and-the-readme-stops-being-a-reference.md)),
the parameter rows (ADR-0170), the contents blocks
([ADR-0163](0163-a-long-document-carries-a-generated-contents-block.md)) — or **held by a gate**,
of which there are nine. A translation is a second copy that **no gate can check**. A machine
verifies that a link resolves, that a roster row fits 320 bytes, that a route fits its ceiling; no
machine here can verify that a Russian page still says what the English one now says.

One collision is purely mechanical and would have to be resolved before a full translation could
build at all. `ROUTE_SOURCE_CEILING` is **30,000 UTF-8 source bytes**, measured with
`Buffer.byteLength`, and Cyrillic costs two bytes per character. A prose-heavy page inflates by
roughly 1.5–1.8x, which puts about **ten of the site's 164 routes** over the ceiling — and
[ADR-0166](0166-a-published-document-splits-into-routes-by-size.md)'s rule is that a route over the
cap means the arithmetic is redone, never that the constant is raised.

## Decision

**We translate a five-document slice into Russian and nothing else:** the three
`packaging/*/READ-ME-FIRST.md`, `docs/running.md` and `docs/how-it-works.md` — about **5,578 words,
3.7 % of the published corpus**. Each translation is a **sibling `.ru.md` beside its source**, so
the pair is adjacent to any sweep that opens either. Each carries a **`translated-from: <sha>` HTML
comment on line 1**; the site renders a dated notice when the source has moved past that sha, and
the close ceremony prints which translations have drifted. **Neither blocks a build.** A *missing or
malformed* stamp does fail, because that is machine-checkable and not a judgement.

The slice is chosen for **stability** as much as for audience: those five sources took 8, 5, 7, 3
and 2 commits in the same 60 days that `presets/README.md` took 193.

## Consequences

### Positive

- A Russian reader gets over the one wall that is language-shaped. Someone deep enough to author
  presets is reading English identifiers regardless.
- **No route splits, and ADR-0166's arithmetic is untouched.** The largest file in the slice is
  `docs/running.md` at 11,331 bytes; at a 1.8x inflation it reaches ~20,000 — under
  `DOCUMENT_SPLIT_BYTES` (40,000) and under the 30,000-byte route ceiling. This holds as long as the
  slice is this slice.
- The stamp travels inside the prose it describes, so the two cannot be updated separately.
- **ADR-0167's identity survives for the foobar path**: the site's Russian install page *is* the
  `READ-ME-FIRST.ru.txt` inside the component zip.

### Negative

- **`docs/` now holds two languages**, and the close ceremony's operator-doc sweep gains a second
  column for five files. Nothing enforces the sweep; the banner is what makes a miss visible.
- **A drifted translation still publishes.** The banner is honest, not corrective — a reader who
  ignores it reads a stale page. Hard-failing was rejected because it makes the Russian slice a
  hostage of every hotkey change, on a project with one translator.
- **The two standalone zips stay English.** ADR-0167's identity holds for the foobar component and
  not for the Windows and macOS downloads.
- **One packager gains a strip step.** `packaging/foobar/build-component.ps1` must remove the stamp
  comment in its existing `@VERSION@` substitution pass; get it wrong and the first line a tester
  reads is raw HTML.
- **Only the owner can review the Russian prose.** That review is the plan's single human gate and
  its only real bottleneck.

### Neutral

- `running.ru.md` and `how-it-works.ru.md` join `check-reader-prose.mjs`'s `READER_DOCS`. Both
  sources are already in it, and
  [ADR-0168](0168-the-reader-documents-address-a-reader-and-the-record-stays-a-link.md)'s rule —
  every Plan/ADR citation inside a markdown link — is language-independent and free to preserve.

## Alternatives considered

### Alternative A — Translate the whole published set

149,178 words carrying 1,339 commits in 60 days is not a project but a standing obligation the size
of the documentation work itself, and 36 % of it is generated from the engine rather than written.
It lost on arithmetic, before principle was reached.

### Alternative B — Machine-translate at build time

Every page in every language, for no maintenance. Rejected because this corpus is dense technical
prose whose terms are the product's own vocabulary: a machine-translated parameter roster that says
the wrong thing about `fold_speed` is worse than the English one, and the build has no reviewer.
The failure mode is also invisible — nothing in this repository can tell a fluent wrong translation
from a right one.

### Alternative C — Starlight i18n locale routing

Proper `/ru/` routes and a built-in language picker. Rejected because five translated pages out of
164 means `/ru/` serves a site that is 97 % English behind Russian chrome, and the picker promises a
Russian site that does not exist. A standalone menu group is honest about being a slice.

### Alternative D — Convention only, no stamp

A line in the close ceremony saying to sweep the translations. Rejected on this repository's own
evidence: [Plan 0061](../plans/done/0061-the-build-stops-paying-for-what-it-is-not-building.md)
Phase 7b wrote *"One line per plan."* three lines above the rows it governed, and the section
regrew **7.1x in eight days**. A rule with no carrier is not a rule here.

## Notes

The stamp was considered as a `docs/translations.toml` manifest instead of an in-file comment. It
lost because it puts the stamp and the prose it describes in different files, so a translation can
be corrected without the sha moving — the precise second-copy drift this ADR exists to make visible.

The advisory/hard-fail split is modelled on
[ADR-0108](0108-a-backlog-claim-about-the-repo-carries-an-executable-probe.md)'s probe gate, which
already prints a pass/fail line over an advisory block that never affects the exit code, and which
already withholds its git-history half on a shallow clone rather than printing false rows.
