# 0217 — Every setting has a file, and a gate says so

> **Status:** in-progress
> **Created:** 2026-09-20
> **Owner skill(s):** dev, studio-builder
> **Related ADRs:** [0240](../adrs/0240-a-setting-lives-in-a-file-and-the-menu-edits-that-file.md)

## TL;DR

[ADR-0240](../adrs/0240-a-setting-lives-in-a-file-and-the-menu-edits-that-file.md) makes a file the
definition of every setting and the in-app menu an editor of that file. This plan makes that true
and keeps it true: the one live violation — the diagnostics overlay, which the settings menu and
`F3` both toggle and nothing persists — gets a `[hud] diagnostics` key, and two checks make a
future violation fail rather than ship. A user who turns the overlay on finds it on after a
restart, and a user who wants it on from the start writes one line in `config.toml` without
launching anything.

## Context & problem

The standalone already honours the rule fourteen rows out of fifteen. `config.toml` is read at
startup and written back whenever a hotkey or a settings row changes a choice, every other
`SettingsRow` maps to a `[section] key`, and `standalone/tests/suite/configuration_doc.rs` already
holds `docs/configuration.md` to the config schema and to the flag roster.

What is missing is the part that makes it a rule rather than a habit:

- **`SettingsRow::Diagnostics` persists nowhere.** No field in `standalone/src/config.rs`, no row in
  `docs/configuration.md`. `docs/running.md`'s Controls table documents the hole — *"Every change
  applies immediately and (except diagnostics) is written to `config.toml`"* — which is how it
  survived: a hole that is written down reads as a decision.
- **Nothing connects a menu row to a key.** `configuration_doc.rs` checks that every key the config
  serialises is *documented*; no check runs the other way, from the thing the user can change to
  the key that must hold it. A fifteenth menu row with no key would be green everywhere.
- **The other two applications are unguarded.** The studio persists through
  `studio/electron/settings.ts` and uses no browser storage today, which is a fact about the code
  as it stands rather than a property anything holds. The plugin declares one host-side
  `cfg_string`, and whether that is resume state or a setting is currently nobody's claim.

## Decision

Repair the violation, then build the two checks the rule needs, each in the place that can actually
see its half: a **Rust test** for the standalone, because the row-to-key property needs the types;
a **Node gate** on the roster ([ADR-0217](../adrs/0217-the-node-gate-roster-is-one-manifest-and-a-checker-holds-every-carrier-to-it.md))
for the cross-language halves, because one carrier beats two and that roster already runs in the
hook, in CI and under the conductor; and a **vitest test** inside the studio for its own third
copy. We rejected a single universal gate (it would have to parse Rust types out of source text to
see the property the Rust test gets for free) and rejected leaving the studio and plugin to review
alone (the rule binds all three applications, and a rule with no carrier is what ADR-0240's
Alternative C rejects).

The plugin's one `cfg_string` is **resume state, not a setting**: it records which preset was on
screen, exactly the thing ADR-0240's bound excludes. The repair there is a claim in
`docs/configuration.md` naming it as such, which is also what lets the gate tell resume state from
an unclaimed setting.

## Architecture diagram

```mermaid
flowchart LR
    subgraph files["User-editable files — the definition"]
        CT["config.toml"]
        SJ["settings.json"]
    end

    subgraph standalone["standalone/"]
        MENU["settings menu + hotkeys"]
        RUN["the running show"]
    end

    subgraph studio["studio/"]
        PANEL["settings panel"]
    end

    subgraph plugin["plugin-foobar/"]
        CFGVAR["cfg_var — resume state only"]
    end

    CT -->|read at start| RUN
    MENU -->|writes back| CT
    RUN --> MENU
    SJ -->|read at start| PANEL
    PANEL -->|writes back| SJ
    CT -->|read at start| plugin

    FLAGS["--flags / RLX_* env"] -.->|override this run only| RUN
```

## Implementation phases

### Phase 1 — The diagnostics overlay gets a key
- **Owner skill:** `dev`
- **What:** `[hud] diagnostics` joins the config, `F3` and the `Diagnostics` row write it back like
  every sibling row, and the documented exception goes away.
- **Files touched:** `standalone/src/config.rs`, `standalone/src/settings.rs`,
  `standalone/src/run.rs` (or wherever `ToggleDiagnostics` is executed and the overlay's initial
  state is set), `standalone/tests/suite/configuration_doc.rs` (the complete-example round trip),
  `docs/configuration.md` (the `[hud]` table and the complete file),
  `docs/running.md` (the Controls table's `S` row and the settings-menu paragraph),
  `docs/running.ru.md` if its source moved.
- **Done when:** a `config.toml` carrying `diagnostics = true` under `[hud]` starts the app with
  the overlay already painted; toggling it with `F3` or from the menu and restarting comes back to
  what was left; the complete example in `docs/configuration.md` still parses to
  `Config::default()`; and no sentence anywhere still says a settings change is not written to the
  file.

### Phase 2 — Every settings row declares the key it edits
- **Owner skill:** `dev`
- **What:** each `SettingsRow` declares either the `config.toml` path it edits (`"hud.diagnostics"`)
  or that it edits nothing, through an exhaustive match so a new row cannot be added without
  answering; a test asserts every declared path resolves in a serialised `Config::default()`.
- **Files touched:** `standalone/src/settings.rs`, `standalone/src/settings/tests.rs`,
  `standalone/tests/suite/configuration_doc.rs`.
- **Done when:** removing the `[hud] diagnostics` field from `Config` fails the test with a message
  naming the row whose declared path no longer resolves; a row declared read-only states why in one
  line; and the declared leaf key of every row is named in `docs/configuration.md` in backticks —
  the property `configuration_doc.rs` already asserts for the schema, now reached from the menu
  side. The `Presets` row is the one read-only row and stays so.

### Phase 3 — A gate for the two applications a Rust test cannot see
- **Owner skill:** `dev`
- **What:** `scripts/check-settings-have-files.mjs`, joined to the roster in
  `scripts/gates.manifest.mjs`, asserting two properties: no browser-storage persistence
  (`localStorage`, `sessionStorage`, `indexedDB`) anywhere under `studio/` outside its own
  allowlist, and every `cfg_*` declaration under `plugin-foobar/` named in `docs/configuration.md`.
  Plus the short section in `docs/configuration.md` naming what the plugin keeps host-side and why
  it is resume state rather than a setting.
- **Files touched:** `scripts/check-settings-have-files.mjs`, `scripts/gates.manifest.mjs`,
  `scripts/fixtures/` (the seeded bite checks, in the shape the sibling gates use),
  `.githooks/pre-push`, `.github/workflows/ci.yml` (the `links` job),
  `docs/configuration.md`, `docs/developing.md` (the gate roster a reader walks).
- **Done when:** the seeded fixtures bite both halves — a `localStorage.setItem` under `studio/`
  and a `cfg_int` in a plugin source no document names each exit non-zero and print the file and
  line; the real tree is green; and `node scripts/check-gate-carriers.mjs` is green, which is what
  says the hook and the CI job took the new row in the manifest's order. Excluding a studio path
  from the browser-storage check requires a reason in the allowlist beside it.

### Phase 4 — The studio's own third copy
- **Owner skill:** `studio-builder`
- **What:** a vitest test holding `StudioSettings` and `studio/README.md` to each other, so a new
  studio setting cannot reach the panel without both a key in the settings file and a documented
  row — the pattern `configuration_doc.rs` uses on the Rust side.
- **Files touched:** `studio/electron/settings.test.ts` (or a sibling test file),
  `studio/README.md`.
- **Done when:** adding a key to `StudioSettings` with no row in `studio/README.md` fails the test
  naming the key, and the reverse also fails; `npx vitest run` in `studio/` is green on the tree as
  it stands, with both existing keys already satisfying it.

## Data shapes

```rust
// illustrative — not the final interface
impl SettingsRow {
    /// The `config.toml` path this row edits, or `None` for the one read-only row.
    /// Exhaustive on purpose: a new row cannot be added without answering this.
    fn config_path(self) -> Option<&'static str> {
        match self {
            SettingsRow::Quality => Some("quality.tier"),
            SettingsRow::Diagnostics => Some("hud.diagnostics"),
            // The path display: it shows where the presets are and changes nothing.
            SettingsRow::Presets => None,
            // …
        }
    }
}
```

## Risks & open questions

- **The overlay's default is a product call, not a mechanical one.** `false` keeps today's
  behaviour and is the safe answer; anything else changes what a new install looks like. Take
  `false` unless the user says otherwise.
- **A grep-shaped gate produces false positives.** A comment or a string mentioning `localStorage`
  would bite. The allowlist is the escape, and it takes a reason beside each entry — the shape
  `check-comment-hygiene.mjs` already uses with `hygiene-allow:`.
- **`Option`-shaped keys are invisible to a walk of the serialised default** (ADR-0240's Negative
  section, and the workaround already in `configuration_doc.rs` for the two `display_name` keys).
  If Phase 2's path check walks a default serialisation, an optional key declared by a future row
  will report as unresolved; walk a value with the optional keys set, as the existing test does.
- **The plugin half has no test harness behind it.** Nothing compiles `plugin-foobar/` in CI, so
  the Node gate reading its source text is the whole of the enforcement there, and it can only see
  declarations spelled the way it greps for.

## What this plan does NOT do

- **No flag is added.** ADR-0240 puts flags second and only where a run needs to override a rig;
  `[hud] diagnostics` is set once per machine, so it gets no flag and no environment variable.
- **No migration machinery.** The compatibility cost ADR-0240 names is real and stays unaddressed:
  nothing here renames a shipped key, and nothing here builds the migration that a rename would
  need.
- **No change to `marks.toml`, to the preset files, or to the control protocol.** Marks stay user
  state in their own file (ADR-0228), and no OSC address moves.
- **No audit of the studio's React state.** Panel widths, scroll positions and which view is open
  are momentary view state under ADR-0240's bound and owe no key.

## Implementation log

> Written by `dev` — one row per phase as that phase's commit lands, and the close block after the
> last one. **The phases above are the contract; everything here is what happened.**

**Lane:** `plan-0217-every-setting-has-a-file-and-a-gate-says-so` in `/home/igor/Work/rlx-plan-0217`

| phase | owner | state | commit |
|---|---|---|---|
| 1 — The diagnostics overlay gets a key | dev | done | 14ae5f69 |
| 2 — Every settings row declares the key it edits | dev | done | committed with this row |
| 3 — A gate for the two applications a Rust test cannot see | dev | not started | |
| 4 — The studio's own third copy | studio-builder | not started | |

### Notes

- **Phase 1's first done-when is asserted by construction rather than by a launched window.**
  Nothing in the suite builds an `AppState` — it needs a winit window — so "a `config.toml`
  carrying `diagnostics = true` starts with the overlay painted" is carried by
  `AppState::new` calling the same `renderer.set_overlay` that `toggle_diagnostics` calls, seeded
  from `config.hud.diagnostics`. The round trip through the file is asserted in
  `config.rs`'s `the_diagnostics_overlay_defaults_off_and_round_trips`.
- **Phase 2's two menu-side tests are unit tests in `standalone/src/settings/tests.rs`, not in
  `standalone/tests/suite/configuration_doc.rs`.** `SettingsRow` lives in the `ritmolux` binary
  rather than in the `standalone` library, so an integration test cannot reach it — the same wall
  that makes `configuration_doc.rs` shell out to `--help` for the flag roster. The listed file gets
  a paragraph in its module doc naming where the fourth property lives and why.
- **Phase 2's first done-when was verified by mutating the declared path, not by removing the
  `Config` field.** Removing `hud.diagnostics` from `Config` is a compile error at the two
  production reads in `app_state.rs` before any test runs. Spelling the path
  `hud.diagnostics_MUTANT` instead fails both new tests with
  `["Diagnostics -> hud.diagnostics_MUTANT"]`, which is the message the criterion asks for.
- `SettingsRow::edit` now returns `None` for any row whose `config_path` is empty, so "read-only"
  is the declaration rather than a second hand-written arm. The `Presets` arm stays for
  exhaustiveness.

### Close triggers

- **`presets/` touched:**
- **Plan header `Closes:`** none
- **What shipped:**
- **Operator docs touched:**
- **Backlog probes (`node scripts/check-backlog-claims.mjs`):**
- **Full suite:**
- **Outstanding `human` phases:**

## Followups (after this lands)

- The foobar plugin reads no `config.toml` today. ADR-0240 says a plugin *setting* belongs in that
  file; the plugin currently has none, so nothing is owed — the day it grows one, this is where the
  work is.
