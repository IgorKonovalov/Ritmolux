# ADR-0190 — Preset TOML is checked by the engine's own loader and never reformatted

> **Status:** proposed
> **Date:** 2026-09-11
> **Related plan(s):** [0169 — A preset is checked before it is rendered](../plans/0169-a-preset-is-checked-before-it-is-rendered.md)
> **Related:** [ADR-0170](0170-a-parameters-reference-row-is-generated-from-the-declaration-the-engine-reads.md)
> (the parameter declarations the editor schema is generated from, and the regenerate-by-env-var
> shape its drift test copies), [ADR-0020](0020-preset-grammar-v2-branching-functions-tempo.md)
> (an unknown parameter is a soft typo warning and the binding is kept), [ADR-0022](0022-build-time-preset-embedding.md)

## Context

A preset is checked today in exactly one place: the `core/tests/preset.rs` suite loads every
**embedded** preset through `Preset::from_toml_str` and asserts it parses with zero warnings. That is
correct and it is slow — the author's loop to learn that a binding does not compile is a `cargo
nextest` run or a `shot` render — and it covers only `presets/*.toml`. `presets/proposed/`,
`presets/pending/` and `docs/examples/` are never embedded, so nothing checks them at all. And the
one class the loader deliberately forgives is the one a typo produces: an unknown parameter name is a
warning and the binding is kept (ADR-0020), so a misspelled `glwo` loads, renders, and does nothing.

The obvious tools for "lint and pretty-print TOML" were measured against this repository on
2026-09-11, running `taplo fmt` 0.9.0 over the 124 TOML files under `presets/` and `docs/examples/`
at `b301507` in a scratch copy, under three configurations:

| taplo config | files changed | lines changed | parse differently | crashes |
|---|---|---|---|---|
| defaults | 99 / 124 | 1,564 | — | 8 |
| `align_entries` + `align_comments`, no array rewrap | 121 / 124 | 1,425 | 0 | 0 |
| no alignment, no array rewrap | 107 / 124 | 1,653 | 0 | 0 |

Every change is whitespace, and no file parses to different data. But no configuration comes near
the corpus — its closest fit leaves 3 of 124 files alone — because the presets carry **deliberate,
local** alignment no formatter models: column-padded inline tables (`{ at = 0.0,  color = … }`), a
two-space gap before a trailing comment that taplo has no option to keep, and `=` columns aligned
per block by hand. Under the default configuration the formatter **panics** on eight shipped presets
(`assertion failed: entry.comment.is_none()`), including `fragment_tunnel` and `reaction_mitosis`.
Aligned comments drift to column 53 when one expression in the block is long.

The decisive measurement is the studio. `studio/shared/toml.ts` edits a preset **by line** under a
stated contract — a file written back unedited is byte-identical, and one moved parameter differs on
one line. Changing `gravity = "0"` to `"0.015"` that way, on a file already in taplo's aligned form,
leaves a file taplo wants to re-pad. A `taplo fmt --check` gate would therefore fail after every
studio save, and the only repairs are to make the studio a formatter or to make the gate lie.

Two candidate lint rules were falsified by the same measurement. A "constant parameter within its
declared range" rule finds 75 violations — because `ParamSpec::range` is documented as *"not a clamp
and not a validation bound"*, and presets such as the tuning walkthrough set `saturation = "1.10"`
on purpose. A "filename prefix equals `system`" rule fails every file: the prefix is a family name
(`analytic_*` is `analytic_field`, `collage_*` is `shape_collage`). What does hold on all 124 files is
weaker and true: the prefix is one `_`-separated segment of the `system` value.

## Decision

We will **check** preset TOML and **never reformat** it. One checker, in Rust, behind
`ritmolux --check <path>`, owns every diagnostic: it runs the engine's own loader and reports its
errors and warnings with a `file:line:col` position recovered from `toml::de::DeTable`'s spans (the
`toml` crate already pinned — no new dependency), then applies a short list of house-style rules
that the loader does not own and that hold on the whole corpus today. Errors fail; warnings print and
exit 0 unless `--strict`. A GPU-free integration test runs the same checker over `presets/`,
`presets/proposed/`, `presets/pending/` and `docs/examples/`, so the gate is the existing
`nextest` run and needs no new CI wiring. For the editor, the engine's `--schema` export is rendered a
second time as a JSON Schema, committed and associated with preset files through a committed
`.taplo.toml`, so Even Better TOML offers completion, hover documentation and an underline on an
unknown key — **and nothing else from Taplo is adopted: no formatter, no `taplo fmt --check`**.

A rule the engine already enforces is never restated by the checker (palette stop order and range,
a quoted `[params]` value). A rule is admitted only if it holds on the corpus the day it lands; a
property that does not hold is either not a rule or is a content change routed to `preset-author`
first.

## Consequences

### Positive
- The author's loop to "does this compile" drops from a test run or a render to one process start
  that never opens a GPU adapter, and the `preset-author` lane gets a precise verdict before it spends
  a `shot`.
- `proposed/`, `pending/` and `docs/examples/` are checked for the first time.
- The typo class ADR-0020 forgives is visible twice — underlined live in the editor by the schema,
  and printed by `--check` — without changing the loader's forgiving behaviour.
- Diagnostics cannot disagree with the engine, because the engine is what produces them.
- The studio's byte-preserving contract is untouched: nothing in this decision writes a file.

### Negative
- **Style is not enforced beyond a handful of rules.** Hand alignment will keep varying between
  presets, and nothing will make two presets look alike. That is the price of refusing a formatter,
  and it is paid knowingly.
- Only a TOML syntax error carries a position from the loader today. An expression error is placed by
  looking its parameter name up in the spanned document; a structural-config error and every warning
  are reported at the file level (line 1) until the loader's warnings carry structure. In the editor
  the schema covers the most common warning — the unknown key — with a real position.
- The JSON Schema is a second rendering of the export and a committed generated file; it is held to
  the engine by a drift test, and it is one more generated artifact to regenerate with an env var.
- Even Better TOML ships a formatter. It runs only if the user's VS Code formats TOML on save, and
  a committed `.taplo.toml` cannot switch it off; the developer docs say so and the plan's `human`
  phase verifies a save leaves the file byte-identical.

## Alternatives considered

### Alternative A — `taplo fmt` (or `prettier-plugin-toml`, which wraps it) as formatter and gate
Rejected on the measurement above: it rewrites 99-121 of 124 files, crashes on eight shipped
presets under its defaults, destroys alignment the authors chose, and a `--check` gate conflicts
with the studio's line editor on the first saved slider. The npm build additionally cannot discover
files on Windows (it reports zero files found; only stdin works).

### Alternative B — an ESLint plugin (`eslint-plugin-toml` + local rules)
Rejected because it is a second checker beside the engine: it would need a third npm project (the
two existing ones belong to `studio/` and `site/`), two new dependencies, and a copy of the
parameter schema to reason from — and it can never see an expression error, which is the diagnostic
an author most needs. Its one advantage, live underlines, the JSON Schema already buys for the class
it matters most for.

### Alternative C — a dependency-free Node gate in `scripts/`
Rejected because the rules worth having need the document's structure, and a line matcher over TOML
is the fragility `toml.ts` documents and accepts only because it edits and never judges. It also
duplicates the engine's knowledge in a second language.

## Notes

The dry-run scripts (`run.sh`, the three configurations, a whitespace classifier and a
parse-equivalence check) were run from a session scratchpad and are not committed; the table above is
their output.
