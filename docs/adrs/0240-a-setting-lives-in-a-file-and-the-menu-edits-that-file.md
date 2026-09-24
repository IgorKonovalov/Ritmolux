# ADR-0240 — A setting lives in a file, and the in-app menu edits that file

> **Status:** accepted
> **Date:** 2026-09-20
> **Related plan(s):** [0217](../plans/done/0217-every-setting-has-a-file-and-a-gate-says-so.md)

## Context

Three applications in this repository hold user choices, and each already has somewhere to put
them: the standalone writes `config.toml` (and `marks.toml` for marks) under `%APPDATA%\Ritmolux\`,
the studio writes `settings.json` in its per-user application directory, and the foobar2000 plugin
uses the host's own `cfg_var` store. So the question is not whether files exist. It is whether a
file is the **definition** of a setting or merely one of several places a value might happen to
live.

Today it is mostly the definition, and once it is not. `docs/configuration.md` says `config.toml`
is read once at startup and written back whenever a hotkey or a settings row changes a choice, and
all fourteen rows of the settings menu map to a `[section] key` — except `Diagnostics`. The F3
overlay has no field in `standalone/src/config.rs` and no row in `docs/configuration.md`, and
`docs/running.md`'s Controls table records the hole as a parenthetical: *"Every change applies
immediately and (except diagnostics) is written to `config.toml`"*. One exception, written down in
the operator's own manual, is what a rule looks like the day before it stops being one.

The forces that make a file the right home here are specific rather than general. This app is
operated **live**, so a rig is a thing that must be reproducible on another machine and survive a
reinstall. It is driven by **other programs** — the studio over the control protocol, `shot` in
CI, the conductor in headless sessions — none of which can open a window and press a key. It is
developed by **agents**, which can read a file and cannot see a menu. And a bug report that carries
a config file is a bug report that can be reproduced.

The counter-force is real, and it is what makes this a decision: a menu-only toggle is the cheapest
thing in the codebase to add. Routing it through a file costs a serde field and a default, a row in
the documentation table, a line in the complete example that must round-trip, and a mapping the
menu row declares. Roughly twenty lines where three would do, every time, forever.

## Decision

**Every setting is defined by a key in a user-editable file, and every other way of reaching it is
an editor of that file.** The in-app settings menu, the hotkeys and the studio's panels change a
value *and write it back*; none of them is the only way to reach a value. A command-line flag or an
environment variable is **optional and second priority** — it overrides the file for one run, it is
added only where a run genuinely needs to override the rig, and it never writes the file. The rule
binds all three applications: `config.toml` for the standalone and for any plugin setting,
`settings.json` for the studio.

The bound is what a setting **is**: a choice the user expects to outlive the process. Momentary
view state is not a setting and owes no key — the browser's three narrowings, which preset is on
screen, an A/B hold. State that outlives the session but is not a choice, such as the marks, gets a
file of its own rather than a config key
([ADR-0228](0228-a-preset-mark-is-user-state-keyed-by-name-in-its-own-file.md)). A host-side store
that is not a user-editable file — foobar's `cfg_var` — may hold **resume state**, never a setting,
and each one it holds is named in `docs/configuration.md`, so the distinction is a claim someone
can check rather than an assumption.

## Consequences

### Positive
- A rig is a file: copyable, diffable, versionable, attachable to a bug report, and restorable
  after a reinstall without re-walking a menu.
- Every automated consumer can set up a run without a window — the studio, `shot`, CI, the
  conductor, and an agent session that must verify what a value *is* rather than what a menu would
  have shown.
- `docs/configuration.md` becomes the complete surface rather than most of it, and the round-trip
  property already asserted in `standalone/tests/suite/configuration_doc.rs` — the complete example
  parses to `Config::default()` — makes that completeness mechanical.

### Negative
- **Every setting costs about twenty lines instead of three**, across four places, and the cost is
  paid on the cheapest kind of change. Some settings will be added grudgingly, and some will not be
  added at all. That is a feature only if you accept the premise that a choice nobody can persist
  should not have been a choice.
- **A shipped key is a compatibility surface.** Once a key is in people's files, renaming it is a
  migration rather than an edit — and the file's tolerance for unknown keys, where a missing or
  unknown key degrades to the default rather than failing, means a rename fails *silently*, by
  reverting to a default the user never chose.
- `Option`-shaped keys are absent from a default serialisation, so they are invisible to any check
  walking the serialised form. `configuration_doc.rs` already carries a workaround for the two
  `display_name` keys, and every future optional key inherits that trap.

### Neutral
- The **format** stays whatever each application natively parses: TOML for the Rust side, JSON for
  the studio's Node side. The rule is about reachability, not uniformity.
- A flag roster that stays small by design means the precedence table keeps rows where the file is
  the only writer. That is now a deliberate shape rather than an accident of what nobody got to.

## Alternatives considered

### Alternative A — Menu-first, with the file optional
The ordinary visualizer shape: settings live in whatever store the UI owns, and a file, if one
exists at all, is an export. Rejected because every automated consumer in this project would then
need a window and a keystroke to set a value, and three of them — CI, the conductor, an agent
session — have no window at all.

### Alternative B — A file key *and* a flag for every setting
Full parity between `config.toml` and the flag roster. Rejected because it roughly doubles `--help`
and the test that holds the documentation to it, in exchange for per-run control over values nobody
sets per-run. The stated priority is the file always, and a flag where a run needs to override a
rig.

### Alternative C — State the rule in prose and rely on review
Rejected on this repository's own record: an ungated prose rule drifts here, which is the reason
`check-index-rows.mjs`, `check-backlog-claims.mjs`, `check-system-counts.mjs` and
`check-comment-hygiene.mjs` all exist. The live exception is itself the evidence — it survived
because it was *documented* as an exception, which reads as a decision rather than as a hole.

### Alternative D — One settings format across all three applications
Rejected because it buys nothing the rule is about. The studio's file is read by Node before any
Rust exists in the process, and the plugin's host is C++; a shared format would cost a parser on
two sides to make two files look alike, while reachability — the actual property — is already
satisfied by each file in its own format.

## Notes

The violation this ADR was written against, at the time of writing: the diagnostics overlay (`F3`,
and the settings menu's `Diagnostics` row) has no key in `standalone/src/config.rs`, no row in
`docs/configuration.md`, and an explicit carve-out in `docs/running.md`'s Controls table. Plan 0217
repairs it and builds the gate this ADR's Alternative C says the rule needs.
