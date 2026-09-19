# ADR-0228 — A preset mark is user state, keyed by name, in its own file

> **Status:** proposed
> **Date:** 2026-09-19
> **Related plan(s):** [0205](../plans/0205-the-library-becomes-navigable.md)
> **Relates to:** [ADR-0022](0022-build-time-preset-embedding.md) (the set is embedded and
> read-only), [ADR-0014](0014-preset-dir-override-for-dev-iteration.md) (the directory override),
> [spec 0001](../specs/0001-c-abi.md) (which already settled preset identity),
> [ADR-0089](0089-the-library-renews-by-replacement-cohorts.md) (what retirement actually is)

## Context

The shipped library is 114 presets and the app offers one flat roster to navigate it. The owner's
ask is to promote the good ones and stop seeing the bad ones — a **favourite** mark and a
**hidden** mark — and to be able to rotate from favourites alone.

Three existing decisions constrain where those marks can live, and together they leave less room
than it first appears.

**Preset identity is already decided, and it is the name.**
[Spec 0001](../specs/0001-c-abi.md) says it outright: *"A preset index is snapshot-scoped, not a
stable ID. An index passed to `rlx_select_preset` is an absolute position in the list the same
handle's `rlx_get_presets` reported… a host that persists a choice across runs persists the
**name**."* A mark is exactly such a persisted choice, so this is settled rather than open.

**The set is read-only at runtime.** [ADR-0022](0022-build-time-preset-embedding.md) has
`core/build.rs` glob and embed `presets/*.toml` into the binary. There is no file to write a mark
into for the shipped set, and the directory that
[ADR-0014](0014-preset-dir-override-for-dev-iteration.md)'s `RLX_PRESET_DIR` points at is a
development and browsing surface the app must not start editing.

**`config.toml` is already owned by something.** The settings menu writes it on every change —
quality, dwell bounds, display, the HUD rows — so it is a *settings* file with a known set of keys,
each documented in [`docs/configuration.md`](../configuration.md). Marks are not settings: they are
a growing data set, edited from a different surface (the browser and a hotkey), at a different
cadence, potentially while the settings menu is also open.

There is also a second consumer, decided in the same interview: the studio must show and set marks
too. That is [ADR-0229](0229-the-studio-marks-a-preset-over-the-control-protocol.md)'s subject, and
it depends on this one having a single owner for the data.

## Decision

We will store preset marks as **user state, keyed by preset name, in a file of its own beside
`config.toml`**, holding two sets: `favourite` and `hidden`.

**`hidden` means "stop showing me this", not "retire this".** A hidden preset is excluded from
auto-rotate and from the browser's default view. It is **not** deleted, not moved out of
`presets/`, not excluded from the behavioral suite, and not excluded from
`shot --presets presets --report`. Retiring a preset is deleting its file, at cohort cadence, by
[ADR-0089](0089-the-library-renews-by-replacement-cohorts.md) — a decision this mark deliberately
does not make on anyone's behalf.

**The accumulated `hidden` set is evidence, and that is a stated purpose rather than a side
effect.** [Backlog 0256](../design-backlog.md) records that nothing in this repository asks whether
a shipped preset is any good, and that the route to a curation mechanism runs through a human
verdict first. A mark made in passing, during ordinary use, is that verdict recorded at the moment
it forms — which is a better instrument than an evening set aside to produce one.

The engine is untouched. `core` learns nothing about marks, the C ABI does not move, and the foobar
component continues to rotate the whole set.

## Consequences

### Positive

- **No core change and no ABI version bump.** The concept lives entirely in the standalone, which
  is where the browser and the scene director already are. The source-agnostic core stays a core.
- **Every gate is unaffected by construction.** `sanity`, `reactivity`, `animation`, `distinctness`
  and `golden` sweep `presets/` on disk and never read user state, so hiding a preset cannot make a
  red suite go green — which is the failure mode a "hidden" concept invites and the one worth
  ruling out structurally rather than by care.
- **The curation question gets an instrument it did not have.** See above; backlog 0256's step 2
  becomes a by-product of use.

### Negative

- **A rename silently loses a preset's marks.** Under ADR-0022 the preset's name is its identity,
  so renaming the file is renaming the preset, and the old marks then refer to nothing. Nothing
  will warn. This is the direct cost of the name-keyed identity spec 0001 already committed to, and
  the alternative — a stable ID minted per preset — is a bigger change to content that ships.
- **Marks accumulate for presets that no longer exist, and nothing prunes them.** A mark on a
  deleted or never-seen name is inert and invisible. Pruning needs a rule about which library is
  authoritative, and with `RLX_PRESET_DIR` able to point anywhere there is no good answer, so the
  file grows slowly forever.
- **Two libraries with a colliding preset name share a mark.** Browsing a converted corpus through
  `RLX_PRESET_DIR` and then returning to the shipped set can carry a mark across, because the key
  is the name and nothing else. In practice names are distinctive; in principle this is unsound.
- **A second file is a second thing to find, back up and explain.** `docs/configuration.md` gains a
  section about a file it does not otherwise describe.

### Neutral

- The foobar component's behaviour does not change. If marks are ever wanted there, that is a C ABI
  question and a separate ADR — not an extension of this one.

## Alternatives considered

### Alternative A — a field in the preset `.toml`

Add `favourite = true` to the file. **Rejected on two independent counts, either decisive.** The
shipped set is embedded into the binary at build time and there is no file to write at runtime; and
a user's opinion of a preset is not a property *of* the preset, so putting it there would make a
personal mark into something that ships to every user and arrives in a pull request.

### Alternative B — keys in `config.toml`

Add `favourites = [...]` and `hidden = [...]` under a `[library]` table. **Rejected, and this was
the closest call.** It is one fewer file and `configuration.md` already documents that file
key by key. But `config.toml` is the settings menu's output, rewritten whole on every settings
change, while marks are written from the browser and a hotkey — two writers, different cadences,
on one file, for no gain beyond tidiness. The list also grows without bound in a file whose every
other key is a single scalar with a documented default, which is a shape mismatch a reader feels
immediately.

### Alternative C — key the marks by index

Store positions rather than names. **Rejected because spec 0001 already rejected it** for exactly
this use: an index is an absolute position in one snapshot of one roster, and adding a preset to
`presets/` would silently shift every mark by one.

### Alternative D — a rating from 1 to 5 instead of two marks

More expressive, and it sorts. **Rejected because it asks for a judgement that mostly does not
exist.** Most presets are a shrug, so most would go unrated and the scale would carry no signal;
and every threshold built on it ("rotate 4 and up") becomes a constant somebody has to defend, of
exactly the kind [ADR-0071](0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md)
warns about. Two marks ask a question the owner can actually answer in the moment.
