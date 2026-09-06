# ADR-0169 — The site is organised by reader task, and the README stops being a reference

> **Status:** accepted 2026-09-06 (Plan 0156)
> **Date:** 2026-09-06
> **Related plan(s):** [0156](../plans/done/0156-the-site-becomes-the-reference.md)
> **Extends:** [0154](0154-the-reader-facing-docs-publish-as-a-site.md) (what is published),
> [0167](0167-the-site-owns-its-entrance-and-the-install-page-is-the-testers-own-file.md) (the entrance)
> **Narrows:** the exclusion in [Plan 0103](../plans/0103-the-project-gets-an-audience.md), a second time

## Context

The site went live on 2026-09-05 and, after Plans 0154 and 0155, it is navigable and its five
preset documents address a reader. The user's brief for the next step is broader: the site should
let a reader *"understand ins and outs of the public API"*, be comprehensive, carry a few diagrams
and solid examples, and serve users, embedders **and** contributors.

Measured against that brief on the live site at `567380d`, four things are missing, and they are
missing for structural reasons rather than for lack of prose.

**The menu is organised by where a file came from, not by what a reader is trying to do.** The
three groups are *Get it*, *Use it / author presets* and *Understand and build it*. The third holds
the quantified budgets a curious user wants beside the release process a contributor wants beside a
hardware checklist only the maintainer runs. There is no group for someone embedding the core, and
nothing on the site names a hotkey, a flag or a `config.toml` key — a person who has installed the
application and wants to know what `S` opens has to leave the site.

**Two of the four public surfaces have no home on the site, and a third has no reference.**
Ritmolux exposes four surfaces:

| Surface | Where it is documented today | On the site |
|---|---|---|
| The preset format and expression language | `docs/presets.md`, `presets/README.md` | yes, as essays; no per-system reference table ([ADR-0170](0170-a-parameters-reference-row-is-generated-from-the-declaration-the-engine-reads.md)) |
| The C ABI | `docs/specs/0001-c-abi.md` (a behavioral contract), `core-cabi/include/rlx_core.h` (the reference) | the contract only; the header is unpublished |
| The operator surface — hotkeys, flags, environment, `config.toml`, OSC | `README.md` lines 172–420 | no |
| The Rust crate | rustdoc, built by CI under `-D warnings` since Plan 0144 | no |

**The operator surface lives in `README.md`, and that is the one file the site was told to leave
alone.** ADR-0154 chose not to touch the README because Plan 0103 Phase 2 owns it, and ADR-0167
narrowed that plan's *"no website, no landing page"* exclusion to admit an entrance. The README is
650 lines. Lines 172–420 are the operator reference — every hotkey, the console, now-playing, twelve
flags with their precedence rules, two environment variables and a fourteen-row OSC address table —
and lines 545–614 are the developer setup. Plan 0103 Phase 2's own note is that a stranger's first
screen should be the product, *"then architecture"*; a 650-line README front door cannot be that.
The single-source rule ([ADR-0154](0154-the-reader-facing-docs-publish-as-a-site.md)) forbids
publishing a copy of those sections, so either the site has no operator reference or the README
stops holding one.

**Page titles disagree with the menu, and link text is a file path.** The site derives each page
title from the document's opening heading, so the roster's page is titled *"Preset authoring"* under
a menu entry reading *"Parameter roster"*, and the C ABI page is titled *"Spec — C ABI (foobar plugin
seam)"*. The sources link to each other by path, so the published pages carry link text like
`../docs/presets.md` — correct in an editor and on GitHub, and the wrong register on a site.

## Decision

**The menu is organised by what the reader is doing, in six groups, and each surface has a page.**

| Group | What it holds |
|---|---|
| **Get it** | Start here; the three install pages (unchanged, ADR-0167) |
| **Use it** | *Running the app* (controls, menus, the console, now playing, tiers); *Configuration* (every flag, environment variable and `config.toml` key, with defaults and precedence; the OSC address table); the gallery |
| **Author presets** | the five preset documents (unchanged); *Headless capture and video* (the `shot` CLI, `--render`, `--stream`) |
| **Embed it** | *Embedding the core* (what the ABI is, the threading contract, the lifecycle, a host example); *The C ABI header*, published verbatim; the C ABI and ring contracts; the Rust API (rustdoc) |
| **How it works** | *How it works* (architecture, the analysis chain, the frame pipeline, with the site's diagrams); the budgets; the technique catalogue |
| **Contribute** | *Developing* (building, the gates); *Testing and visual QA* (the `core/tests/` harness); *MilkDrop conversion*; releasing; on-device validation; the diffusion filter |

Three consequences are decided with it:

1. **The operator reference and the developer setup move out of `README.md` into `docs/`**, and the
   README keeps a short controls table and links to the site for the rest. `docs/` is then the
   single source for that material and the site publishes it. Plan 0103 Phase 2 keeps ownership of
   the README's *opening* — what it leads with and in what order — and inherits a shorter file to
   reorder. The architecture diagram moves with the rest; the README links to it rather than
   carrying a second copy.
2. **A published source declares its page title beside its route**, in the `PUBLISHED` map, and
   the menu label and the page title are the same string. The opening heading is still stripped
   from the body; it is no longer what names the page. A link whose text is the target's path is
   rewritten at build time to the target's title when the target is published. Off-site links keep
   their path text, because a path *is* the honest name of a file on GitHub.
3. **The Pages artifact carries rustdoc** under `/api/`, built by a job in the same workflow from
   the same commit the site is built from, and the *Embed it* group links to it. The site's
   build stamp already says which commit that is.

Existing routes keep their paths. The regrouping is a change to the sidebar, not to any URL, so a
link into the site written since 2026-09-05 keeps resolving.

## Consequences

### Positive

- A reader can find every public surface from the menu, and each group is named for a task rather
  than for a directory.
- The README becomes what Plan 0103 wanted: a front door. Its operator and developer sections stop
  being the only copy of facts that a site with search should own.
- The C ABI header and the rustdoc are published from the commit they describe, with no hand-kept
  copy to drift.
- Titles and link text stop leaking the repository's layout.

### Negative

- **`README.md` loses ~300 lines to `docs/`, and Plan 0103 Phase 2 is reshaped by it.** That plan's
  note *"everything below Architecture is already strong and should mostly keep its wording"* is
  honoured by moving the wording rather than rewriting it, but the phase's done-when is now met
  partly by this ADR's plan. The coordination is written into both plans.
- **The Pages workflow gains a Rust toolchain.** A `cargo doc --workspace --no-deps` job is minutes
  of cold build where the site build today is under a minute. `Swatinem/rust-cache` bounds it, and
  the job runs beside the site build rather than before it, but a documentation edit now waits on a
  compile before it deploys.
- **Whether `cargo doc --workspace` succeeds on `ubuntu-latest` is unverified.** CI's `check` job
  runs it on Windows and macOS only; the standalone carries a `not(any(windows, macos))` fallback
  arm, so a Linux build is plausible and not proven. The plan names the fallback (build the docs on
  `windows-latest` and hand the artifact across) rather than assuming.
- Six groups is more menu than three. The mitigation is that every group's first entry is a page
  a reader can start from, and `scripts/check-site-routes.mjs` already holds every route to being
  reachable from the menu.

### Neutral

- The `engine/*`, `guide/*` and `install/*` route prefixes no longer match the group names they sit
  under. Route stability was judged worth more than prefix consistency for a site that has been live
  for days and has already been linked to.

## Alternatives considered

### Alternative A — Keep the operator reference in the README and publish the README

Publishing `README.md` as a page would carry the operator sections to the site without moving
them. It would also carry the architecture pitch, the repository layout and the developer setup,
duplicating the site's own entrance (ADR-0167) and making the README's *"what is this"* screen the
site's second copy of the same pitch. ADR-0154 rejected exactly that duplication.

### Alternative B — Defer the operator pages until Plan 0103 lands

Plan 0103 has been approved since 2026-08-16 and is sequenced last, behind five other plans. The
operator surface is what a user who has just installed the application needs first; making it wait
on a distribution plan puts the wrong thing on the critical path. The reverse dependency — 0103
Phase 2 inheriting a shorter README — is the cheaper coupling.

### Alternative C — Publish rustdoc to a separate host

docs.rs requires a published crate, and `rlx-core` is not on crates.io and has no reason to be. A
second GitHub Pages site is not available: a repository has one. Committing the rustdoc output to
the tree was rejected for the same reason ADR-0154 rejected a staged copy of the docs — a
generated artifact in `git` is a copy that drifts. The Pages artifact is the one place that is
rebuilt from the commit it describes.

### Alternative D — Take the page title from the sidebar label automatically

Starlight's sidebar entries and the content collection are loaded separately, and the split
routes (ADR-0166) generate their own labels from headings. Declaring the title once in `PUBLISHED`,
beside the route, is one line per document and is what both the loader and the sidebar already
read.
