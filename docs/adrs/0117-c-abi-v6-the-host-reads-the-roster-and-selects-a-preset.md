# ADR-0117 — C ABI v6: the host reads the roster and selects a preset

> **Status:** accepted
> **Date:** 2026-08-16
> **Related plan(s):** [0107](../plans/done/0107-the-foobar-menu-picks-a-preset.md)

## Context

ADR-0006 (v2) gave the plugin `lmv_load_presets` — seed-then-load a directory — and explicitly
deferred selection: *"selecting a preset by name or index over the ABI is not part of this
decision (the plugin's selection UX stays cycle-only)."* That deferral has now expired on a user
request: the foobar component's right-click menu should list the loaded presets and pick one, the
way the standalone's browse overlay (Plan 0008) already does. The core has carried everything
needed since that plan — `Renderer::preset_names()` and `Renderer::select_preset(index)` exist and
are what the overlay uses — so this is purely a question of how the capability crosses the C seam.

The forces are the standing ABI ones. Every added function is a permanent contract compiled
separately in C++ (drift is a link/runtime error), the spec holds the surface to "minimal", and no
allocation may cross the boundary — `LmvMetrics` (ADR-0008) set the pattern of caller-allocated
memory the core writes into. The consumer's actual need is narrow: build one menu (all names, in
roster order, plus which one is current) and act on one click (switch to index *i*).

## Decision

We will add **two** functions to the C ABI and bump `LMV_ABI_VERSION` from `5` to `6`:

> `int32_t lmv_get_presets(LmvHandle* handle, uint8_t* buf, size_t buf_len, int32_t* out_current_index);`
> `int32_t lmv_select_preset(LmvHandle* handle, int32_t index);`

`lmv_get_presets` writes the installed roster's names, in roster order, into the caller's buffer
as UTF-8 with each name followed by a single `0x00` byte, and returns the total byte count the
full list needs (`>= 0`) or a negative `LMV_ERR_*` code. If `buf_len` is smaller than that count
it writes nothing — the call-twice sizing pattern: query with `buf_len = 0`, allocate, call again.
`out_current_index` (nullable, skipped when null) receives the index the show is on — defined as
the dissolve's **target** while a transition is in flight, matching `cycle_preset`'s "name where
the show is going" convention — or `-1` on an empty roster. Before `lmv_attach_window` the call
returns `LMV_ERR_NO_WINDOW`, because the roster installs at attach (ADR-0006's pending-set rule).

`lmv_select_preset` switches to the preset at `index` — an absolute position in the list the same
handle's `lmv_get_presets` reported — with the same dissolve `lmv_cycle_scene` gets. A negative or
out-of-range index returns `LMV_ERR_INVALID_ARG` and changes nothing: the host built its menu from
a snapshot, and a stale index is a host bug worth signaling, not worth a panic or a wrap.

Indices are snapshot-scoped: they are only meaningful against the roster the same handle reported
and no roster-changing call (`lmv_load_presets`) has intervened. Both functions follow the standing
threading contract — the host's UI/render thread, never the sample-delivery thread.

## Consequences

### Positive

- The plugin reaches selection parity with the standalone's browse overlay through two functions,
  both thin wrappers over methods the core has shipped since Plan 0008 — no new core capability,
  no new dependency, no material binary growth against NFR §4's ~1.07 MB remaining headroom.
- Caller-allocated snapshot honors the no-allocation-crosses-the-boundary rule, and one call
  yields everything a menu needs (names + current), so the shim's menu build is O(1) ABI calls.
- Persist-by-name lands entirely host-side: the shim maps its stored name to an index against a
  fresh snapshot, so the ABI never grows a string-selection variant it would have to keep forever.

### Negative (the price we pay)

- **The v6 surface is fifteen functions to keep stable forever**, and it blesses *index
  addressing against a snapshot* as the selection model. If selection later wants stable IDs
  (a roster that changes under an open menu, multi-window rosters that diverge), that is another
  ABI decision, not a reinterpretation of these indices.
- The NUL-separated name list is a documented mini-format rather than a self-describing struct.
  It is the same trade `LmvMetrics` made in the other direction; a third consumer must read the
  spec, not just the header.

### Neutral

- The header stays a hand-maintained mirror (no cbindgen, per ADR-0003); the two new signatures
  are a review/link-time contract like the other thirteen.
- `lmv_cycle_scene` is unchanged and remains exactly "select next" — two routes to one roster.

## Alternatives considered

### Alternative A — Four narrow functions (count / name-at / current / select)

Conventional C enumeration with no packed format. Rejected: it widens the frozen surface to
seventeen functions where two suffice, makes a menu build N+2 boundary crossings, and each of the
four is a separate forever-contract. ADR-0003's governing value is surface minimality.

### Alternative B — Select-by-name; the shim scans the folder itself

One added function; the menu is built from `*.toml` filenames the shim lists. Rejected: the menu
would show files the core rejected as malformed and miss the distinction between a filename and
the roster name the core actually uses — the list lies exactly when a user's hand-authored file is
broken, which is exactly when they are staring at the menu. The core's installed roster is the
only honest list source.

### Alternative C — No ABI change; selection stays cycle-only

Keep v5 and let users cycle to the preset they want. Rejected: with the library heading from 39
toward ~57 presets (Plan 0104), cycling is up to N-1 dissolves to reach a chosen look, and the
user has asked for direct choice by name. This is precisely the event ADR-0006 deferred and named.

## Outcome (2026-08-18, at Plan 0107's close)

**The decision landed unchanged** — two functions, `LMV_ABI_VERSION` 5 → 6, fifteen exports in
`core-cabi/src/lib.rs` matching fifteen in the header with identical signatures, and the plugin's
persist-by-name built entirely host-side as the Positive section predicted.

**One number in the Positive section was already stale when it was written.** It says the two
functions cost "no material binary growth against NFR §4's ~1.07 MB remaining headroom", quoting
`docs/specs/0001-c-abi.md`'s Plan 0097 measurement. The no-growth half is right — the review
confirmed this plan links nothing new. The headroom half is not: `foo_lmv.dll` measured
**9,279,488 B** at this close against the spec's recorded 8,879,104 B, so the real headroom is
**~0.72 MB**. The ~400 KB predates this ADR and is unattributed; it is filed as design-backlog 0118
along with the correction the spec's table needs. Recorded here rather than edited into the body,
because an accepted ADR is append-only.

**One consequence the ADR did not consider, found in the shim.** `lmv_select_preset` is the
*dissolving* form only — the core's instant-cut `Renderer::select_preset_now` has no ABI mirror. The
plugin restores its persisted preset immediately after `lmv_attach_window`, so every fresh handle
starts on the roster's first entry and crossfades to the remembered one over ~1 s, including at
every mid-playback stream-format change. Not worth a v7 on its own; noted so the next host-side
"restore a known state" need does not rediscover it.

## Notes

Extends ADR-0003/0006/0008/0013/0110; supersedes nothing. `docs/specs/0001-c-abi.md` is the
authority on the surface shape and moves to v6 with the implementing plan. The v5→v6 handshake
runs through `lmv_abi_version` as before: the shim gates version-tiered features on the reported
value (as it already does for v2 preset loading and v3 diagnostics) rather than calling functions
that are not there — a defensive check, since the staticlib link means core and shim ship as one
artifact.
