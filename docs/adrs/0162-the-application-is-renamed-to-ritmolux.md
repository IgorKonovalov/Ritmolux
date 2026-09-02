# ADR-0162 — The application is renamed to Ritmolux, and the record keeps the old name

> **Status:** proposed
> **Date:** 2026-09-02
> **Related plan(s):** [0150](../plans/0150-the-application-becomes-ritmolux.md)

## Context

`light-music-visualizer` is a description, not a name. It was chosen in [ADR-0001](0001-rust-core-wgpu-cabi-foobar-shim.md)
when the repository held no product, and the abbreviation `lmv` followed it into the crate names,
the C ABI's sixteen `extern "C"` symbols, every `LMV_*` environment variable, the shipped binary,
the foobar component filename and the per-user data directory.

**Two approved plans are blocked on the name, and both bake it into a surface that is expensive to
change afterwards.** [Plan 0143](../plans/0143-the-documentation-gets-a-front-end.md) publishes the
reader-facing docs as a site and is already parked in writing — *"the rename is the named trigger
for this one"* — because the project name lands in the Starlight title, the header of every page,
and the Pages subpath every published URL carries. A renamed repository redirects its web traffic,
its git operations, its issues and its stars; **its GitHub Pages URLs do not.**
[Plan 0103](../plans/0103-the-project-gets-an-audience.md) Phase 5 submits the component to the
foobar2000 component repository, and `plugin-foobar/foo_lmv.cpp` declares
`VALIDATE_COMPONENT_FILENAME("foo_lmv.dll")` — foobar2000 refuses to load a component whose file
has been renamed, so the filename is a contract with every installed copy rather than a cosmetic
choice. Submitting first and renaming second costs a dead listing and a second component that
existing users must remove by hand.

**The name was screened before it was chosen.** Ritmolux is unregistered on crates.io, npm, PyPI,
the GitHub user and organization namespace, and on `.com .app .dev .io .net .org .live .art .es .it
.co.uk .info`, with the typo domains `rhythmolux.com` and `ritmoluxe.com` also free. It returns no
company in English or Russian and no repository on GitHub. The single hit is `ritmolux.ai`,
registered 2026-08-08 and serving an unrelated application whose own brand is a different name.
**The trademark position is a knockout search and an accepted risk, not a clearance.** Every
register serves a proof-of-work challenge, so the search is manual; run on 2026-09-02 it returned
**no `Ritmolux` on any register**. `Ritmo` and `Lux` each return many marks separately, which is
the expected result for two dictionary-adjacent elements rather than a partial hit — confusion is
judged on the mark as a whole, and a crowded field around a shared element narrows what any owner
of that element can claim. What was not done: a class-filtered `ritmo` pass in Nice 9 / 42 / 41,
and a storefront sweep for unregistered common-law use. **The user's decision is that this is
sufficient** for a project not intended as a commercial product and with no registration planned.

**The project is pre-1.0** (`0.100.1`), so nothing here owes backward compatibility; the cheapest
moment to break every identifier is now, and it stops being cheap the moment either blocked plan
ships.

## Decision

We will rename the application to **Ritmolux**, using **`rlx`** as the internal prefix wherever
`lmv` stood — crate names, the C ABI's symbols and result codes, and the environment variables —
and spelling the name in full on every artifact a user meets: `ritmolux.exe`,
`foo_ritmolux.dll`, `ritmolux-v<version>-*.zip`, and `%APPDATA%\Ritmolux\`. Three characters
replace three, so the substitution is mechanical and no formatted line re-wraps.

**The sweep stops at the append-only record.** `docs/adrs/`, `docs/plans/done/`,
`docs/plans/README-archive.md` and `docs/design-backlog-archive.md` hold roughly 990 occurrences of
the old name, and they keep it. Those documents are append-only by standing rule — an accepted ADR
is superseded, never edited — and rewriting nine hundred lines of decision history to agree with a
name chosen afterwards would be both the first history rewrite this project has performed and a
falsification of the record: ADR-0072 argued about a symbol named `lmv_render`, and that is what it
argued about. **This ADR is the pointer that makes the old name legible**, and a reader who meets
`lmv_create` in a 2026 ADR is reading a true document about a renamed thing.

The C ABI version does **not** move. `LMV_ABI_VERSION` becomes `RLX_ABI_VERSION` carrying its
existing value, because [ADR-0003](0003-c-abi-v1-surface.md)'s counter exists so a mismatched shim
can detect an incompatible *shape* at runtime — and a renamed symbol fails at C++ link time, before
anything can consult it. Bumping it would assert a shape change that did not happen.

## Consequences

### Positive

- **The two blocked plans unpark**, and each pays its one-time cost once: the component is
  submitted under its final filename, the Pages subpath is chosen once, and no published URL dies.
- **One name in the live tree.** No seam where a reader has to know that `rlx-core` used to be
  `lmv-core` in order to grep effectively.
- **The break is loud where it matters.** The C ABI's only consumer is `plugin-foobar/`, compiled
  from this repository against this header, so a missed symbol is an unresolved external at link
  time — the safest possible failure for an ABI change, and the reason the full sweep is affordable
  at all ([ADR-0072](0072-the-c-abi-ships-from-its-own-crate.md) records the same reasoning about
  the same seam).

### Negative

- **A roughly 1,300-site mechanical diff**, touching nearly every file in the workspace, which
  cannot be merged against a parallel lane. Every plan branch open when it lands would conflict on
  almost every file it touched, so it forces a freeze.
- **The record and the code now disagree on the name**, permanently and by design. Every ADR and
  every closed plan says `lmv`. This is a real cost paid to avoid a larger one, and it is only
  survivable because this document exists to explain it.
- **The Spout sender name changes**, and it is the one identifier here that lives outside this
  repository entirely: `standalone/src/stream.rs` publishes `"lmv"`, and a receiver — OBS,
  Resolume, a saved show file — binds to that string. Every saved source must be re-pointed by
  hand, once. The live rig of 2026-08-29 is exactly the artifact this affects.
- **Existing foobar2000 installations keep a stale `foo_lmv.dll`.** The filename validation means
  the new component cannot simply replace the old one; both load, and the old one must be removed
  through Preferences. Cheap now, when the install base is the author and a handful of testers, and
  not cheap after Plan 0103 Phase 5.
- **GitHub Pages does not redirect, and the old repository name must never be reused** — recreating
  `light-music-visualizer` would silently break every redirect the rename set up.
- **The name is knocked out, not cleared, and that is a deliberate ceiling.** A knockout search
  answers whether the name is *obviously* taken; it does not answer whether a similar mark in a
  related class could be asserted. The gap is accepted because this is not a commercial product and
  no registration is planned — **so the decision carries a condition rather than a guarantee: if
  commercial distribution is ever contemplated, this is the first thing to revisit**, and the
  remedy is a different name (Lumefall), not a different architecture. Every phase of Plan 0150 is
  name-agnostic, so a second rename would cost the same sweep again and nothing more.

### Neutral

- The `standalone` package keeps its generic name; it was never named after the product.
- The foobar component's GUIDs are unchanged, so an existing Default UI layout survives the
  filename change.
- The per-user directory is renamed with a one-time migration rather than abandoned, which adds a
  small amount of code that becomes dead at 1.0.

## Alternatives considered

### Alternative A — rename the public surface only

Rename the product, the repository, the binary, the component filename and the documentation, and
leave every internal identifier — `lmv-core`, the sixteen `lmv_*` symbols, `LMV_PRESET_DIR` — in
place. It is by far the cheapest option and the least likely to break the build.

Rejected because it converts a one-time cost into a permanent one. The codebase would be named
after an application that no longer exists, every future reader would have to learn the mapping,
and the seam would never close on its own — there is no later moment when renaming `lmv-core` gets
cheaper than it is today, and several (a public component, a published site, 1.0) when it gets
dearer.

### Alternative B — rename everything except the C ABI

Keep the sixteen `lmv_*` functions and the `LMV_*` result codes for ABI stability while renaming
the crates, the binary and the artifacts. It halves the risk of the riskiest phase.

Rejected because the stability it buys is imaginary. The ABI has exactly one consumer, it is
compiled from this repository against this header in the same build, and pre-1.0 nothing external
links it — so there is no party the stability protects. What it costs is concrete: a header named
`rlx_core.h` declaring `lmv_create`, which is the single most confusing artifact the rename could
produce.

### Alternative C — rewrite the record too

Sweep `docs/adrs/` and `docs/plans/done/` along with everything else, so the whole repository reads
consistently.

Rejected on two counts. It would edit accepted ADRs, which this project has never done and which
[ADR-0116](0116-an-index-row-is-a-pointer-and-a-gate-holds-it-to-one.md)'s neighbouring rules treat
as inviolable — the mechanism for a changed decision is a superseding document, not an edit. And it
would make the history *wrong*: those documents record arguments that were actually had about
identifiers that actually had those names.

### Alternative D — Clavilux, and Alternative E — Lumefall

**Clavilux** was the preferred name on taste — Thomas Wilfred's 1919 colour organ is this
application's literal ancestor — and it lost on namespace. `clavilux.com` has been held since 2000
and serves nothing; the GitHub handle belongs to a dormant but real account and GitHub's username
policy releases a name only on a trademark complaint, never for inactivity; `clavilux.org` belongs
to a live 501(c)(3) restoring Wilfred's Model E; and `github.com/jptrsn/clavilux` is a stub
repository described as *"synchronized audio-reactive lighting … a colour organ"* — this
application's exact niche and the direction [ADR-0145](0145-the-engine-drives-the-fixtures-directly-over-art-net.md)
is taking. A name whose canonical `.com` and canonical handle are both unavailable is a permanently
compromised namespace.

**Lumefall** is fully clear on every registry and TLD checked, and lost by one vowel:
`lumifall.com` is a live home-and-office LED lighting business. Trade-class proximity is the axis a
trademark examiner weighs, and this project sells light. Ritmolux has no such neighbour. Lumefall
remains the fallback if the name is ever revisited — note that its own defect is precisely a
trade-class one, so it would need the class-filtered search Ritmolux was not put through.

## Notes

The screening evidence — RDAP queries validated against a known-registered and a known-free control,
registry lookups on crates.io, npm and PyPI, GitHub user and repository searches, and English- and
Russian-language web searches — was gathered 2026-08-29 and re-verified 2026-09-02. `Afterglow` and
`Lumenfall` were eliminated earlier in the same process: Afterglow collides twice inside this
project's own domain (Deep Symmetry's DMX light-show engine, Nextec's DMX interfaces) and Lumenfall
is a live company.
