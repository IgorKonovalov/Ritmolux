# 0150 — The application becomes Ritmolux

> **Status:** approved
> **Created:** 2026-09-02
> **Approved:** 2026-09-02 (user) - runs next, once the lanes now live have closed
> **Owner skill(s):** dev, human
> **Related ADRs:** [0160](../adrs/0160-the-application-is-renamed-to-ritmolux.md)
> **Blocks:** [0143](0143-the-documentation-gets-a-front-end.md) (parked in writing on this
> decision) and [0103](0103-the-project-gets-an-audience.md) Phases 4-6 (repository metadata,
> component submission, three posts). Neither may ship before this lands.
> **Lane guidance:** `main` directly, with **no other lane live** — see Phase 1. A worktree buys
> nothing here (the sweep touches every crate, so the lane pays a cold `target/` for no isolation)
> and ADR-0053's default does not apply to a change that conflicts with every parallel branch by
> construction.

## TL;DR

The application is renamed from `light-music-visualizer` to **Ritmolux**. Every live identifier
carrying the old name moves — the three `lmv-*` crates, the C ABI's sixteen `extern "C"` symbols
and its result codes, seven `LMV_*` environment variables, the shipped binary, the foobar component
filename, the release zips and the per-user data directory — using `rlx` where `lmv` stood and the
full name on anything a user reads. **The append-only record is not swept**: `docs/adrs/`,
`docs/plans/done/` and the two archives keep the old name, and ADR-0160 is the pointer that makes
them legible. When this lands, Plans 0143 and 0103 unpark.

## Context & problem

The name is a description that was never chosen as a product name, and two approved plans are now
waiting on it. [Plan 0143](0143-the-documentation-gets-a-front-end.md) is parked in its own header —
*"the rename is itself parked … that decision is the named trigger for this one"* — because a
GitHub Pages project site lives at a path derived from the repository name and **a renamed
repository does not redirect its Pages URLs**. [Plan 0103](0103-the-project-gets-an-audience.md)
Phase 5 submits the component to the foobar2000 component repository, and
`VALIDATE_COMPONENT_FILENAME("foo_lmv.dll")` makes that filename a contract with every installed
copy — foobar2000 refuses to load the component if the file is renamed. Publish either, and the
rename stops being a repository-internal edit and starts costing other people's bookmarks and
installs.

The surface is large but it is not uniform, and the split is the whole design of this plan:

| | occurrences | disposition |
|---|---|---|
| `core/`, `core-cabi/`, `standalone/`, `plugin-foobar/`, `milkconv/`, `lmv-ring/` | 1,042 | swept |
| live `docs/` (excludes `adrs/`, `plans/done/`, both archives) | 174 | swept |
| `.claude/`, `.github/`, `packaging/`, `scripts/`, `presets/`, `README.md`, `CLAUDE.md` | 102 | swept |
| `docs/adrs/`, `docs/plans/done/`, `README-archive.md`, `design-backlog-archive.md` | 990 | **kept** |

Counts are `git grep -InE '\blmv[-_ ]\|light-music-visualizer\|LMV_'` per path, taken 2026-09-02.
They are a scale estimate for sequencing, not a done-when — the done-whens below are exit codes.

## Decision

Rename to **Ritmolux** with the internal prefix **`rlx`**, sweeping every live surface in one plan
and leaving the append-only record untouched. Three characters replace three, so the substitution
is mechanical and no formatted line re-wraps. ADR-0160 records the decision and the rejected
alternatives: a public-surface-only rename (converts a one-time cost into a permanent seam),
keeping the C ABI symbols (buys stability for a consumer that does not exist), rewriting the record
(would edit accepted ADRs and falsify the history), and the two runner-up names — Clavilux, which
lost on an unobtainable GitHub handle, a dead `.com`, the Wilfred non-profit's `.org` and a
same-niche stub repository, and Lumefall, which sits one vowel from a live LED-lighting business.

**The central safety property is that this plan moves no golden.** Nothing here changes what is
rendered. Every phase runs the suite against the committed baselines untouched, and a moved golden
is a finding — evidence that something non-mechanical happened — never a re-bless.

## Architecture diagram

```mermaid
flowchart TB
    subgraph swept["Live surfaces — swept to rlx / Ritmolux"]
        crates["crates<br/>lmv-core, lmv-ring, lmv-core-cabi"]
        abi["C ABI<br/>16 lmv_* symbols, LMV_* codes"]
        artifacts["artifacts<br/>lmv.exe, foo_lmv.dll, zips"]
        env["environment<br/>7 LMV_* vars, APPDATA dir"]
        strings["strings a user reads<br/>titles, --help, Spout sender"]
        livedocs["live docs, CLAUDE.md, skills, CI"]
    end
    subgraph kept["Append-only record — keeps lmv by design"]
        record["docs/adrs/, docs/plans/done/,<br/>the two archives — 990 sites"]
    end
    adr["ADR-0160<br/>the pointer that explains the split"] --> kept
    swept --> unblocks["0143 docs site · 0103 component submission"]
```

## Implementation phases

### Phase 1 — the name is cleared and the tree is frozen
- **Owner skill:** human
- **What:** The two gates that must hold before a workspace-wide sweep starts — the one check
  nobody has run, and the absence of any parallel lane.
- **What the user does:**
  1. **Clear Ritmolux on a trademark register.** Every register serves an Altcha proof-of-work
     challenge, so no part of this can be automated — it is the one axis no screening has covered,
     and the only failure that invalidates the plan *after* it has landed. **If it convicts, this
     plan stops and Lumefall is the fallback** — ADR-0160 records why.

     **Registers, in descending order of value.** The first covers most of the ground; stop early
     only if it convicts.

     | register | scope |
     |---|---|
     | `branddb.wipo.int` | 90+ national registers plus Madrid international marks, in one query |
     | `tmsearch.uspto.gov` | the US register — the one most likely to matter for distribution |
     | `euipo.europa.eu/eSearch` | the EU trade mark register |
     | `fips.ru` (Rospatent) | Russia, where `-люкс` is a crowded suffix; search Cyrillic too |

     **Terms.** Examiners weigh sound and meaning, not spelling, so the phonetic neighbours matter
     as much as the exact string: `ritmolux`, `ritmolux*`, `*ritmolux*`, `ritmo lux` (spaced),
     `rhythmolux`, `ritmoluxe`, `ritmoluks`, `rithmolux`, and — **within the four classes below
     only**, since it is a common word unfiltered — `ritmo*`. On Rospatent add `Ритмолюкс`,
     `Ритмо люкс`, `Ритмо*`.

     **Classes (Nice).** **9** downloadable software (primary) · **42** software as a service and
     software development · **11** lighting apparatus, which matters *specifically* because
     ADR-0145 puts this application next to lighting hardware · **41** entertainment and live
     performance.

     **Reading the result** — the test is likelihood of confusion, so relatedness of goods governs.
     An identical mark in an unrelated class does not block; a *similar* mark in class 9, 42, 11 or
     41 does. Live applications count as much as registrations, since a pending one ahead of us can
     mature. Dead marks do not block, but a recently-dead one names a party who may refile.
     `RITMIX` is the known near neighbour — different mark, but check which classes it holds.
  2. **Confirm the freeze.** Plans 0148 and 0149 closed and merged; no other lane opens until
     Phase 9 lands.
- **Files touched:** none.
- **Done when:** the register search is recorded in the implementation log as one row per register
  — register, date, the exact query string, the classes filtered to, the hit count, and the verdict
  — so that a later reader can tell a search that found nothing from a search that was never run;
  `git worktree list` prints exactly one line; `git status --porcelain` is empty. **If any of the
  three fails, `dev` does not start Phase 2.** That is this phase's purpose.
- **Not covered by any register, and worth the extra ten minutes:** US common-law rights arise from
  *use* rather than registration, so an unregistered product already shipping under this name
  creates real risk that no register will show. Sweep Steam, the Microsoft Store, the Mac App
  Store, Google Play and itch.io for `Ritmolux`. The domain and repository screening behind
  ADR-0160 did not cover storefronts.

### Phase 2 — the crates take the prefix
- **Owner skill:** dev
- **What:** Rename the three `lmv-*` crates and their library targets. This is the walking
  skeleton: the crates are the deepest edge in the dependency graph, so renaming them forces every
  consumer — `standalone`, `core-cabi`, `milkconv`, and the C++ shim's build — to move at once. If
  the rename is structurally hard anywhere, it is hard here, before any user-visible surface has
  changed.
- **Mapping:** `lmv-core` → `rlx-core` (lib `lmv_core` → `rlx_core`); `lmv-ring` → `rlx-ring`
  (directory renamed with `git mv`); `lmv-core-cabi` → `rlx-core-cabi` (cdylib/staticlib
  `lmv_core_c` → `rlx_core_c`). The `standalone` and `milkconv` packages keep their names — neither
  was named after the product.
- **Files touched:** root `Cargo.toml` (`members`, `default-members`), `core/Cargo.toml`,
  `core-cabi/Cargo.toml`, `lmv-ring/` → `rlx-ring/`, `standalone/Cargo.toml`, `milkconv/Cargo.toml`,
  every `use lmv_core::` / `use lmv_ring::` and `lmv_core::` path across the workspace, `Cargo.lock`,
  `.github/workflows/*.yml` wherever a `-p lmv-*` scope is named.
- **Done when:** `cargo build --workspace` and `cargo nextest run --workspace` are green;
  `git grep -nE 'lmv[-_](core|ring)' -- . ':!docs'` returns nothing; **no golden was re-blessed** —
  the suite passed against the committed baselines.

### Phase 3 — the C ABI takes the prefix
- **Owner skill:** dev
- **What:** The sixteen `extern "C"` symbols, the result codes, the header's filename and include
  guard, and every C++ call site.
- **Mapping:** `lmv_*` → `rlx_*` for all sixteen functions and the opaque handle type;
  `LMV_OK` / `LMV_ERR_*` / `LMV_ABI_VERSION` / `LMV_VERSION` → `RLX_*`. **`RLX_ABI_VERSION` keeps
  its current value** — per ADR-0160, the counter guards a runtime shape check that a renamed
  symbol can never reach, so bumping it would assert a change that did not happen.
- **Files touched:** `core-cabi/src/**`, `core-cabi/include/lmv_core.h` → `rlx_core.h` (`git mv`),
  `plugin-foobar/foo_lmv.cpp` and its `#include`, `plugin-foobar/build.ps1`,
  `packaging/foobar/build-component.ps1`, and **`docs/specs/0001-c-abi.md`** — the spec is the
  authority on the ABI's shape and is a living contract, not part of the record.
- **Done when:** the component links with no unresolved externals;
  `git grep -nE '\blmv_|\bLMV_' -- core-cabi plugin-foobar docs/specs` returns nothing; the symbol
  roster in `docs/specs/0001-c-abi.md` names the same sixteen symbols the header declares; the
  suite is green with no golden re-blessed.

### Phase 4 — the shipped artifacts take the name
- **Owner skill:** dev
- **What:** Everything a release produces is spelled with the full name, not the prefix.
- **Mapping:** binary `lmv` → `ritmolux`; `foo_lmv.dll` → `foo_ritmolux.dll` (including
  `VALIDATE_COMPONENT_FILENAME`); `foo_lmv.cpp` → `foo_ritmolux.cpp` and
  `foo_lmv_version.h` → `foo_ritmolux_version.h`; the component's
  `DECLARE_COMPONENT_VERSION("Light Music Visualizer", …)` name and description → Ritmolux; the
  install directory `%APPDATA%\foobar2000-v2\user-components-x64\foo_lmv\` → `…\foo_ritmolux\`;
  the zips `light-music-visualizer-v<version>-*.zip` → `ritmolux-v<version>-*.zip`.
- **The component GUIDs are not touched.** They are the identity foobar2000 stores a Default UI
  layout against, so keeping them means an existing layout survives the filename change.
- **Files touched:** `standalone/Cargo.toml` (`[[bin]] name`), `plugin-foobar/foo_lmv.cpp` (`git mv`),
  `plugin-foobar/build.ps1`, `packaging/foobar/build-component.ps1` (`$DllName`),
  `packaging/macos/bundle.sh`, both `packaging/**/READ-ME-FIRST.md`,
  `.github/workflows/release.yml` (the `$name` and the three asserted top-level filenames),
  `docs/releasing.md`, `docs/on-device-validation.md`.
- **Done when:** `packaging/foobar/build-component.ps1` produces `foo_ritmolux.fb2k-component`;
  `release.yml`'s asserted top-level names match what the packaging step writes; `docs/releasing.md`
  names `foo_ritmolux.dll` where it states the component's size cap; and
  `docs/on-device-validation.md` carries a new item for the one thing no gate can check — that
  foobar2000 loads the renamed component and lists it as Ritmolux (verified in Phase 9).

### Phase 5 — the environment takes the prefix
- **Owner skill:** dev
- **What:** All seven environment variables move: `LMV_PRESET_DIR`, `LMV_BLESS`, `LMV_TIER`,
  `LMV_DEBUG_OVERLAY`, `LMV_SAMPLE_DIR`, `LMV_TRANSITION_STRIP`, `LMV_SPOUT_LOG` → `RLX_*`.
- **Files touched:** `standalone/src/lib.rs` (the `PRESET_DIR_ENV` / `TIER_ENV` constants),
  `core/tests/**`, `standalone/src/spout/shim.cpp`, `plugin-foobar/foo_ritmolux.cpp`,
  `.github/workflows/**`, `docs/capturing.md`, `docs/presets.md`, `presets/README.md`,
  `docs/on-device-validation.md`, `CLAUDE.md`, `.claude/skills/**`.
- **Done when:** `git grep -n 'LMV_' -- . ':!docs/adrs' ':!docs/plans/done'
  ':!docs/plans/README-archive.md' ':!docs/design-backlog-archive.md' ':!docs/plans/0150-*'`
  returns nothing; the golden suite still blesses under `RLX_BLESS`; the suite is green with no
  golden re-blessed.

### Phase 6 — the user's machine is migrated
- **Owner skill:** dev
- **What:** `APP_DIR_NAME` moves from `"light-music-visualizer"` to `"Ritmolux"`, and an existing
  directory is carried across rather than orphaned. That directory holds the user's edited presets,
  `config.toml` and the diagnostics log; leaving it behind would look exactly like data loss.
- **Behavior:** on resolution, if the new directory is absent and the old one exists, move it and
  log once. If both exist, use the new one and leave the old alone. If neither exists, seed fresh
  exactly as today. The `RLX_PRESET_DIR` override continues to win over all three.
- **Files touched:** `standalone/src/lib.rs` (`APP_DIR_NAME`, `resolve_preset_dir_from`),
  `standalone/src/main.rs`, `plugin-foobar/foo_ritmolux.cpp` (the literals its own comment requires
  be kept in step with `APP_DIR_NAME`), and the tests beside them.
- **Done when:** three tests state the behavior — an old directory with no new one ends up at the
  new path with nothing left at the old one; both present leaves the old one untouched and reads
  the new one; neither present produces a fresh seed identical to today's. A fourth asserts the
  environment override still takes precedence over a pending migration.
- **Note for `dev`:** the migration necessarily names the string `"light-music-visualizer"` in
  code, and a comment explaining *what* the move does is mechanism, which ADR-0127 permits. A
  comment narrating that the directory *used to be* called that is plan-relative history, which it
  does not.

### Phase 7 — the strings a user reads
- **Owner skill:** dev
- **What:** The window title, the console title, the `--help` banner, the component description,
  and the Spout sender name.
- **Mapping:** `APP_TITLE` → Ritmolux; `CONSOLE_TITLE` `"lmv console <version>"` →
  `"Ritmolux console <version>"`; the help banner `"lmv — a real-time music visualizer … usage: lmv
  [flags]"` → `ritmolux`; **`DEFAULT_SENDER: &str = "lmv"` → `"Ritmolux"`**.
- **`DEFAULT_SENDER` is the one identifier in this plan that lives outside the repository.** A
  Spout receiver — OBS, Resolume, a saved show file — binds to that string, so changing it
  re-points every saved source by hand, once. Phase 9 carries the checklist item.
- **Files touched:** `standalone/src/main.rs`, `standalone/src/stream.rs` (and the test asserting
  `request.sender`), `plugin-foobar/foo_ritmolux.cpp`, `README.md`.
- **Done when:** `ritmolux --help` names `ritmolux` in both the banner and the usage line; the
  window title leads with Ritmolux; `--stream` announces the `Ritmolux` sender; the suite is green
  with no golden re-blessed.

### Phase 8 — the live documentation, and the gates that name paths
- **Owner skill:** dev
- **What:** The remaining prose, and repair of the two gates this plan is structurally able to
  break.
- **Two gates are at specific risk, and both must be repaired here, not merely observed:**
  - **`check-backlog-claims.mjs`.** ADR-0108 anticipates this failure in writing — *"a rename turns
    the gate red for a reason that is not the entry's fault"*. Every live backlog probe naming
    `lmv-ring/`, `core-cabi/include/lmv_core.h`, an `LMV_` symbol or a renamed path must be
    re-pointed at the new path. A probe that goes red here is a stale probe, not a falsified claim.
  - **`check-doc-links.mjs`.** Phases 2, 3 and 4 each `git mv` a file that documents link to.
- **Also in this phase:** `Cargo.toml`'s `repository` key, which today reads
  `https://github.com/eastsphere/light-music-visualizer` while the actual remote is
  `IgorKonovalov/…` — **pre-existing drift, unrelated to the rename, fixed here** because this is
  the commit that touches the line. `CLAUDE.md`'s repository tree and its worktree convention
  (`WORK/lmv-plan-NNNN` → `WORK/rlx-plan-NNNN`), `.claude/skills/**`, `README.md`, and the live
  `docs/` pages.
- **Done when:** all five gates exit 0 (`check-doc-links`, `check-index-rows`,
  `check-backlog-claims`, `check-filter-figures`, `check-comment-hygiene`);
  `cargo nextest run --workspace` is green with no golden re-blessed; and
  `git grep -inE '\blmv[-_]|LMV_|light.music.visualizer' -- . ':!docs/adrs' ':!docs/plans/done'
  ':!docs/plans/README-archive.md' ':!docs/design-backlog-archive.md' ':!docs/plans/0150-*'`
  returns nothing.
- **The exclusion of this plan's own file is required, not cosmetic.** Plan 0150 names the old
  identifiers on nearly every line; a grep that does not exclude it can never pass, and adjusting
  the grep to make it pass is how a done-when gets tuned into meaninglessness.

### Phase 9 — the repository takes the name
- **Owner skill:** human
- **What:** The rename outside the working tree, and the on-device checks no gate can run.
- **What the user does:**
  1. Rename the repository on GitHub: `light-music-visualizer` → `ritmolux`. Web traffic, git
     `clone`/`fetch`/`push`, issues, wikis, stars and followers all redirect permanently.
     **GitHub Pages URLs do not redirect** — which is why Plan 0143 waited — and a workflow that
     `uses:` an action hosted in a renamed repository breaks with `repository not found`. This
     repository hosts no such action (`.github/` contains no `uses: ./`), so nothing here is
     affected.
  2. **Never recreate a repository named `light-music-visualizer`** — it silently kills every
     redirect the rename just created.
  3. `git remote set-url origin https://github.com/IgorKonovalov/ritmolux.git`, then rename the
     local checkout `WORK/light-music-visualizer` → `WORK/ritmolux`. The MSVC linker override in
     `WORK/.cargo/config.toml` sits *above* the checkout and keeps working untouched.
  4. Install `foo_ritmolux.fb2k-component` in foobar2000; confirm the component list reads
     Ritmolux; **remove the old `foo_lmv` component through Preferences**, since the filename
     validation means both would otherwise load.
  5. Re-point every Spout receiver to the `Ritmolux` sender.
  6. Optional: claim the free `ritmolux` GitHub organization and the free `ritmolux.com`,
     `rhythmolux.com` (the English-speaker typo) and `ritmolux.app`.
- **Done when:** `git remote -v` shows the new URL and `git fetch` succeeds; the foobar2000
  component list reads Ritmolux and nothing named `foo_lmv` remains under
  `%APPDATA%\foobar2000-v2\user-components-x64\`; a Spout receiver binds the `Ritmolux` sender.

## Risks & open questions

- **The trademark register is the one check nobody has run**, and it is the only failure that
  invalidates this plan after it lands rather than before it starts. Phase 1 is a stop gate for
  exactly this reason, and Lumefall is the recorded fallback.
- **A blind textual substitution can corrupt what it does not understand.** `lmv` appears 879 times
  as a bare word and 1,020 times as an identifier prefix, and the repository contains binary golden
  baselines. Any scripted sweep must be word-boundary aware and must exclude binary files; the
  per-phase `cargo nextest run --workspace` with **no golden re-blessed** is what proves it stayed
  mechanical. A moved golden in any phase of this plan is a finding to investigate, never a bless.
- **The freeze is load-bearing and has no enforcement.** Nothing prevents a new worktree being
  opened mid-plan, and a lane that starts after Phase 2 would conflict on nearly every file it
  touches. Phase 1 states the agreement; keeping it is the user's.
- **The live rig loses its Spout binding.** Phase 7 renames the sender, and the memory of the
  2026-08-29 set is that the rig ran eight hours on that path. The re-point is one field per source,
  but it must happen before the next performance, not during it.
- **`.claude/skills/**` edits are classifier-dependent.** If a write under that path is refused,
  the sweep is incomplete in a place no gate inspects — `check-doc-links.mjs` covers the skills'
  links but nothing checks their prose. Report it rather than working around it.
- **The name of the abbreviation is now a second thing to know.** Users meet `Ritmolux`; the code
  says `rlx`. This is deliberate (ADR-0160) and cheap, but it is not free.

## What this plan does NOT do

- **It does not rewrite the record.** `docs/adrs/`, `docs/plans/done/` and the two archives keep
  `lmv` in 990 places, by design. ADR-0160 exists to make them legible.
- **It does not clear a trademark** — Phase 1 asks the user to, and stops if it fails.
- **It does not publish anything.** The docs site is [0143](0143-the-documentation-gets-a-front-end.md);
  the component submission, the repository metadata and the posts are
  [0103](0103-the-project-gets-an-audience.md) Phases 4-6. This plan only removes the reason both
  are waiting.
- **It does not move to 1.0.** The version bump at close is an ordinary one, decided by the
  architect against what shipped. Whether the rename and 1.0 coincide is a separate call.
- **It does not rename the `standalone` or `milkconv` packages**, nor the component's GUIDs.

## Implementation log

> Written by `dev` — one row per phase as that phase's commit lands, and the close block after the
> last one. **The phases above are the contract; everything here is what happened.**

**Lane:** _(to be filled by `dev` — expected `main` directly, per the lane guidance in the header)_

| phase | owner | state | commit |
|---|---|---|---|
| 1 — the name is cleared and the tree is frozen | human | not started | |
| 2 — the crates take the prefix | dev | not started | |
| 3 — the C ABI takes the prefix | dev | not started | |
| 4 — the shipped artifacts take the name | dev | not started | |
| 5 — the environment takes the prefix | dev | not started | |
| 6 — the user's machine is migrated | dev | not started | |
| 7 — the strings a user reads | dev | not started | |
| 8 — the live documentation, and the gates that name paths | dev | not started | |
| 9 — the repository takes the name | human | not started | |

### Notes

### Close triggers

- **`presets/` touched:**
- **Plan header `Closes:`** none
- **What shipped:**
- **Operator docs touched:**
- **Backlog probes (`node scripts/check-backlog-claims.mjs`):**
- **Full suite:**
- **Outstanding `human` phases:**

## Followups (after this lands)

- Unpark [0143](0143-the-documentation-gets-a-front-end.md): remove the `Parked until` block and
  choose the Pages subpath under the new repository name.
- Unblock [0103](0103-the-project-gets-an-audience.md) Phases 4-6.
- Decide whether the rename and 1.0 coincide, and whether `RLX_ABI_VERSION` moves at 1.0 for
  reasons of its own.
