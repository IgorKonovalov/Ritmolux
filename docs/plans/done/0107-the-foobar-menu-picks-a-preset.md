# 0107 — The foobar menu picks a preset

> **Status:** done
> **Created:** 2026-08-16
> **Approved:** 2026-08-16
> **Closed:** 2026-08-18
> **Owner skill(s):** dev, human
> **Related ADRs:** [0117](../../adrs/0117-c-abi-v6-the-host-reads-the-roster-and-selects-a-preset.md) (C ABI v6), [0006](../../adrs/0006-c-abi-v2-preset-loading.md) (the folder + seed-then-load this builds on)

**Closed 2026-08-18.** Phases 1-4 landed as `1ea486b` (ABI v6), `bdadf47` (the menu),
`2919b7b` (persistence), `4d1f450` (the doc sweep). Mode 4 review: **no blockers, two majors**,
both doc-side and both repaired at the close in `cc1b7ef` — the Phase 4 sweep had written `B` for
the standalone's browse overlay in `docs/presets.md` (the binding is `Tab`), and
`docs/on-device-validation.md` was absent from Phase 4's holder list and kept describing the
component as Space-only.

Verified at the review rather than taken on trust: fifteen exports in `core-cabi/src/lib.rs` and
fifteen in the header with identical signatures; `fmt` clean, `clippy -p lmv-core-cabi
--all-targets` clean, 9/9 cabi tests; and the new `#[cfg(windows)]` roster test **really attaches**
on the dev box — re-run under `--success-output final` it prints no skip notice, so its assertions
ran. That is the one thing a CI pass cannot tell you here, which is why the spec now records the
silent-skip as a known gap.

**Phase 5 (human, on-device) did not run before the close and is not claimed to have.** It was
carried forward as a standing item in
[`docs/on-device-validation.md`](../../on-device-validation.md), the way Plan 0102's Phase 5 was —
that file's own escalation rule is that on-device checks do not gate closes here. **It ran
2026-08-24 and all four parts pass**; the reading is [below](#phase-5--on-device-verification-2026-08-24),
and it leaves two followups, neither of them a defect in this plan's code.

Three minors were filed rather than fixed: the modal-menu staleness argument in
`plugin-foobar/foo_lmv.cpp` (backlog 0117), the component's DLL growth past the size the C ABI
spec records (backlog 0118), and backlog 0103's now-stale description of the menu it convicts
(updated in place, entry stays live — Plan 0103 Phase 1 owns the fix).

## TL;DR

The foobar component's right-click menu gains a **Preset** submenu listing every loaded preset
with a checkmark on the current one, a **Reload presets** item that picks up files dropped into
the shared preset folder without restarting foobar, and an **Open presets folder** item that
makes the folder discoverable at all. The chosen preset persists across foobar restarts by name.
The one engine change is ABI v6 (ADR-0117): two functions — a roster snapshot and a
select-by-index — wrapping core capability the standalone's browse overlay has had since Plan
0008. Presets already ship as files: the plugin seeds the embedded set into
`%APPDATA%\light-music-visualizer\presets` (write-if-absent, shared with the standalone) and
loads whatever it finds. This plan makes that folder *usable* from inside foobar.

## Context & problem

The user's request: ship presets as files, load new presets by dropping a file in a folder, and
choose a preset from the component's right-click menu. The first two half-exist — ADR-0006's
`lmv_load_presets` seeds and loads the shared APPDATA directory, and the shim calls it at handle
creation — but three gaps make the capability invisible to a foobar user:

1. **A dropped file needs a restart.** The shim loads presets only at handle creation and on a
   stream-format change; nothing re-scans on demand.
2. **Selection is cycle-only.** ADR-0006 explicitly deferred pick-by-name over the ABI; the
   context menu today has exactly "Next scene" and the stats-overlay toggle. Reaching a chosen
   preset in a 39-preset library (heading to ~57 with Plan 0104) is up to N-1 dissolves.
3. **Nothing tells the user the folder exists.** The seeding is silent; discoverability is zero.

Interview decisions (all four the recommended option): keep the shared APPDATA folder (no
profile-local split), pick up dropped files via an explicit **Reload presets** menu item (no
watcher, no rescan-on-open), a **flat Preset submenu with a checkmark** (no grouping yet), and
**persist the chosen preset by name** across restarts.

## Decision

Widen the C ABI to v6 with exactly two functions (ADR-0117): `lmv_get_presets` (caller-buffer
snapshot of the roster names, NUL-separated, plus the current index; call-twice sizing) and
`lmv_select_preset` (switch by snapshot index, with the standard dissolve). Everything else is
shim-side: menu construction from the snapshot, reload via a re-call of the existing
`lmv_load_presets`, persistence via the shim's first `cfg_var` (name-based, re-resolved against a
fresh snapshot after every load). We rejected four narrow enumeration functions (surface grows to
seventeen for no consumer benefit) and select-by-name with a shim-side folder scan (the menu
would lie exactly when a user's hand-authored file is malformed) — see ADR-0117.

## Architecture diagram

```mermaid
sequenceDiagram
    participant U as User (right-click)
    participant S as foo_lmv.cpp (shim, UI thread)
    participant A as C ABI (v6)
    participant C as core Renderer

    U->>S: WM_CONTEXTMENU
    S->>A: lmv_get_presets(buf=0) → bytes needed
    S->>A: lmv_get_presets(buf, len, &current)
    A->>C: preset_names() / active-or-target index
    S->>U: Preset ▸ [names…, ✓ current], Reload presets, Open presets folder
    U->>S: click preset i
    S->>A: lmv_select_preset(i)
    A->>C: select_preset(i)  — dissolve
    S->>S: cfg_var ← name[i]
    U->>S: click Reload presets
    S->>A: lmv_load_presets(APPDATA dir)  — seed-then-load, unchanged
    S->>A: lmv_get_presets → fresh snapshot
    S->>A: lmv_select_preset(index of cfg_var name, if present)
```

## Implementation phases

### Phase 1 — ABI v6: snapshot + select
- **Owner skill:** dev
- **What:** `lmv_get_presets` and `lmv_select_preset` in `core-cabi`, `LMV_ABI_VERSION` 5 → 6,
  the header mirror, and the Rust-side FFI coverage. Core side: expose whatever thin accessor the
  wrapper needs for "current index, defined as the dissolve target while one is in flight" —
  `preset_names()` and `select_preset()` already exist.
- **Files touched:** `core-cabi/src/lib.rs`, `core-cabi/include/lmv_core.h`,
  `core/src/render/mod.rs` (accessor only, if needed), `core/tests/ffi.rs`,
  `docs/specs/0001-c-abi.md` (surface moves to v6; reconciled-through this plan).
- **Done when:** `core/tests/ffi.rs` drives create → load_presets → get_presets → select → free
  across the boundary and defends these behavioral claims: the sizing call (`buf_len = 0`)
  returns exactly the byte count a subsequent fill needs and writes nothing; the filled buffer
  contains as many NUL-terminated names as `lmv_load_presets` reported installed, in roster
  order; after `lmv_select_preset(i)` succeeds, `out_current_index` reads `i` (the dissolve
  target, per ADR-0117's convention); a negative and an out-of-range index each return
  `LMV_ERR_INVALID_ARG` and leave `out_current_index` unchanged; pre-attach `lmv_get_presets`
  returns `LMV_ERR_NO_WINDOW`. `-p lmv-core-cabi` builds clean (the crate is outside
  `default-members`; a bare `cargo build` will not compile it).

### Phase 2 — The menu: Preset submenu, Reload, Open folder
- **Owner skill:** dev
- **What:** Rework `WM_CONTEXTMENU` to build: **Preset ▸** (flat, one item per roster name in
  roster order, `MF_CHECKED` radio on the current index), **Next scene** (kept), **Reload
  presets**, **Open presets folder** (ShellExecute on the resolved APPDATA dir), and the existing
  overlay toggle. Preset command IDs take a reserved range (e.g. base 2000 + index) disjoint from
  the existing 1001/1002. Names are UTF-8 from the snapshot; convert to UTF-16 for `AppendMenuW`
  (preset names may be non-ASCII). Reload = `lmv_load_presets` re-call, then re-select the
  previously current name against a fresh snapshot if it still exists (core's `set_presets`
  keeps the *index*, not the name, so without this a reordered folder silently moves the
  selection). All of it on the foobar main thread — the menu is modal, so the snapshot an item
  click acts on cannot go stale between build and dispatch.
- **Files touched:** `plugin-foobar/foo_lmv.cpp`.
- **Done when:** in foobar2000, right-click shows the Preset submenu naming every preset the
  component loaded (not every file in the folder — a deliberately malformed `.toml` dropped in
  the directory does **not** appear); clicking a name switches with a dissolve and the checkmark
  follows; dropping a valid new `.toml` then clicking Reload presets makes it appear in the menu
  without restarting foobar, and the active preset's *name* is unchanged by the reload when its
  file still exists.

### Phase 3 — Persistence by name
- **Owner skill:** dev
- **What:** The shim's first `cfg_var` (a `cfg_string` under a new GUID): written on every
  successful selection (menu pick or Next scene), read after `load_presets_into` at handle
  creation and resolved by name against a fresh snapshot — found: select it (via
  `lmv_select_preset`); missing or empty: leave the roster's default, don't guess. The
  re-selection must run after `lmv_attach_window` (the roster installs at attach; pre-attach the
  snapshot call returns `LMV_ERR_NO_WINDOW`).
- **Files touched:** `plugin-foobar/foo_lmv.cpp`.
- **Done when:** pick a non-default preset, exit foobar2000 fully, relaunch — the same preset is
  rendering and carries the checkmark; delete that preset's file, relaunch — the component comes
  up on the roster default with no error surfaced (a stale name degrades silently, per the
  interview's name-based-persistence rationale).

### Phase 4 — Operator-doc sweep
- **Owner skill:** dev
- **What:** Every user-facing doc that describes the plugin's controls or preset delivery says
  what ships now: the right-click menu's new items, the shared folder path, and drop-a-file +
  Reload as the authoring loop on the foobar path. Known holders: `README.md` (component
  section), `plugin-foobar/README.md`, `packaging/foobar/`'s READ-ME-FIRST text, and
  `docs/presets.md` / `docs/capturing.md` only if they state the plugin cannot select (grep for
  cycle-only claims rather than assuming). Count-free phrasing throughout — no "39 presets" that
  re-drifts when Plan 0104 lands.
- **Files touched:** the docs above as the grep finds them.
- **Done when:** `node scripts/check-doc-links.mjs` exits 0 and no shipped doc still describes
  the plugin's selection as cycle-only or the preset folder as standalone-only.

### Phase 5 — On-device verification
- **Owner skill:** human
- **What:** The four-part loop in a real foobar2000 v2 install, since CI builds no C++: (1) pick
  a preset from the menu mid-playback, (2) drop a fresh `.toml` (author one or copy-rename an
  existing file with an edited name) and Reload, (3) restart foobar and confirm persistence, (4)
  confirm Open presets folder lands in the right Explorer window. Also worth one glance: menu
  behavior while foobar is in layout-edit mode, which is Plan 0103 Phase 1's territory — note
  what you see rather than fixing it here.
- **Files touched:** none.
- **Done when:** all four behave as Phases 2–3 describe on the real host, or the failures are
  written into this plan as findings.

## Data shapes

```c
// illustrative — the header mirror is authoritative once Phase 1 lands
// v6 additions (ADR-0117)
int32_t lmv_get_presets(LmvHandle *handle,
                        uint8_t  *buf,       // UTF-8, each name followed by one 0x00
                        size_t    buf_len,   // if < needed: nothing written
                        int32_t  *out_current_index); // nullable; -1 = empty roster
// returns: total bytes the full list needs (>= 0), or LMV_ERR_*
int32_t lmv_select_preset(LmvHandle *handle, int32_t index); // 0 or LMV_ERR_*
```

`out_current_index` is the dissolve's **target** while a transition is in flight — the same
"where the show is going" convention `cycle_preset`'s return value already uses — so the menu
checkmark always matches the most recent user action.

## Risks & open questions

- **File contention with Plan 0103 Phase 1** (approved), which redesigns this same
  `WM_CONTEXTMENU` handler to stop shadowing foobar's layout-edit menu (backlog 0103). Run the
  two **in sequence, either order, never in parallel lanes**. If 0103 Phase 1 lands first, the
  Preset submenu inherits the edit-mode deference for free; if this lands first, 0103's fix
  restructures a slightly bigger menu. No capability dependency either way.
- **Reload restarts the active scene's simulation state.** `set_presets` calls
  `configure_active_scene()`, so even a no-change reload re-seeds the running look — same
  property as the standalone's hot-reload. Acceptable because reload is an explicit user action;
  the plan does not try to diff-and-skip. If Phase 5 finds it jarring, that is a finding, not a
  license to add a watcher.
- **Roster identity is by name.** Two files declaring the same preset name make persist-by-name
  and reselect-after-reload ambiguous (first match wins). Not new — the roster already tolerates
  it — but worth one sentence in the Phase 4 docs.
- **Real-time safety is untouched by construction:** every new call sits on the foobar main
  thread; nothing here goes near `lmv_push_samples` or the ring. Phase 1's boundary code must
  keep the standing rules — validate at the boundary, no panic across it.
- **NFR §4 headroom is ~1.07 MB** after the v5 text feature. This plan links nothing new, so no
  re-measure is owed — but if Phase 1 somehow pulls a dependency, re-measure rather than assume
  (the spec's own instruction).

## What this plan does NOT do

- No file watcher / live hot-reload in the plugin (explicit menu item only — interview decision).
- No grouped or nested preset menu; flat list with checkmark. Regrouping at ~57 presets is a
  future UX call.
- No profile-local or portable-install preset folder; the shared APPDATA directory stays the one
  location (interview decision).
- No standalone changes — its browse overlay already selects by name.
- No fix for backlog 0102 (1x1 surface attach) or 0103 (menu shadows layout-edit) — Plan 0103
  Phase 1 owns both. This plan only touches the same handler.
- No stable preset IDs over the ABI; indices are snapshot-scoped (ADR-0117's recorded limit).
- No preset content: nothing in `presets/` changes.

## Phase 5 — on-device verification (2026-08-24)

**Done, six days after the close. All four parts pass on the real host, plus the persist-by-name
degrade. No finding; two confirmations of already-filed defects and one wording correction owed to
this file.**

foobar2000 v2, Windows 10, the dev box. **The installed component was stale and had to be
replaced first** — `%APPDATA%\foobar2000-v2\user-components-x64\foo_lmv\foo_lmv.dll` was dated
2026-08-16 17:28, which predates both `bdadf47` (the menu) and `2919b7b` (persistence), landed
2026-08-17. A run against it would have exercised a component with no Preset submenu at all and
read as a total failure of Phases 2-3. Rebuilt from `main` at `10b0701` via `build.ps1 -Install`,
version 0.75.1. **Anyone running a carried-forward human phase should date the installed artifact
before trusting what they click** — the gap between a plan closing and its on-device phase running
is exactly long enough for the profile to hold a pre-plan build.

Both step-(b) fixtures were **pre-flighted through `shot --preset-file`** before being dropped, so
a wrong result would convict the menu rather than the file: `zz_phase5_probe.toml` (a clone of
`attractor_clifford` with `name = "ZZ Phase5 Probe"`) rendered 7201 frames clean and reported its
new name; `zz_phase5_broken.toml` failed to parse at line 4, `unclosed table, expected ]`. Both
were removed afterwards and the folder is back to the 76 files it held.

| part | result |
|---|---|
| **(a)** pick from **Preset ▸** mid-playback | **Pass** — list marked on the one showing, pick dissolves across, mark follows |
| **(b)** drop + **Reload presets** | **Pass** — `ZZ Phase5 Probe` appears with no restart; the malformed file is **absent**, so the list is the core's roster and not a directory listing; the watched preset is unchanged by the reload |
| **(c)** **Open presets folder** | **Pass** — lands in `…\light-music-visualizer\presets` itself, not the parent |
| **(d)** restart, then delete-and-restart | **Pass with a wrinkle, below** — the picked preset returns and carries the mark; with its file deleted the component comes up on the roster default, **nothing surfaced**, menu opens normally, no ghost entry for the deleted name |

**The wrinkle in (d), and it is this file's wording that is wrong rather than the code.** On
relaunch the panel shows the **roster default while no track is loaded**, and switches to the
persisted preset only once playback starts. That is the documented mechanism working — the handle
is created when the visualisation stream delivers a format, and the restore runs after
`lmv_attach_window` — but Phase 3's done-when says "relaunch — the same preset is rendering",
which reads as *at launch*. It is not: it is *at the first track boundary*. For a user who looks
before pressing play, persistence looks broken. Same neighbourhood as
[backlog 0102](../../design-backlog.md), and the correction belongs in the operator docs rather
than in a new backlog entry, so it is filed as a followup below rather than escalated.

**[Backlog 0103](../../design-backlog.md) confirmed failing on a second machine, post-0107.** In
layout-editing mode the panel's right-click still surfaces the component's own menu — Preset,
Next scene and the rest — wholly in place of foobar's Cut / Copy / Replace / Remove. Noted, not
fixed; Plan 0103 Phase 1 owns it. The entry already carries the post-0107 update; today adds an
on-device sighting to what was a code probe plus one reporter's account.

**Two things this run did not settle.** The Risks section's "reload re-seeds the running scene"
question went unremarked — the reload happened mid-playback and the watched preset survived it,
but nobody was watching for a simulation re-seed specifically, so it is neither confirmed jarring
nor cleared. And the preset folder again held **76** files against the 40 the repo ships, the
stale-cohort accumulation [Plan 0102](0102-the-component-ships.md) Phase 5 already
recorded; the submenu lists all 76, which is the first place that accumulation becomes something
a user sees rather than a folder detail.

## Followups (after this lands)

- **The restore lands at the first track boundary, not at launch, and no doc says so** (Phase 5).
  Phase 3's done-when and [`docs/on-device-validation.md`](../../on-device-validation.md) both say
  "relaunch — the same preset is rendering", which is only true once audio starts; before that the
  panel shows the roster default and persistence looks broken to a user who checks before pressing
  play. A `dev` doc edit: say *at the first track boundary* wherever the restart behaviour is
  described. The code is doing what the handle lifecycle requires; nothing to fix there.
- **Does an explicit Reload visibly re-seed the running scene?** The Risks section flagged that
  `set_presets` calls `configure_active_scene()` even for a no-change reload, and asked Phase 5 to
  judge whether it is jarring. Phase 5 did not look for it. Cheap to settle on the next on-device
  pass: reload while watching a long-trail preset and see whether the accumulation restarts.
