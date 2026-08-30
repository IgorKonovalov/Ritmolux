# 0124 — The review fixes that move no pixels

> **Status:** in-progress
> **Created:** 2026-08-28
> **Owner skill(s):** dev
> **Related ADRs:** [ADR-0127](../adrs/0127-a-comment-carries-the-mechanism-and-the-decision-record-stays-in-docs.md) (the hygiene gate this widens), [ADR-0113](../adrs/0113-milkdrop-presets-are-translated-ahead-of-time-onto-a-warp-mesh-idiom.md) (the crate this maps), [ADR-0016](../adrs/0016-gpu-tests-opt-in-ci-scope.md) (the skip shape the shared harness keeps)

**Drafted without an interview at the user's request.** The guesses: (1) no new ADR — every
item here is a mechanism fix under a decision that already exists; (2) the ABI-check
disagreement resolves toward the **shim's** behaviour (a newer core is accepted), because the
shim's comment argues it and the spec merely paraphrases; (3) the plan runs in a worktree lane
like every plan since 0047 and lands as a **patch** bump.

## TL;DR

The 2026-08-28 whole-codebase review found fourteen majors. This plan takes the six that are
mechanical, golden-neutral and safe to land in a day: a shared `core/tests/common/` harness
replacing 27 copies of `fn headless()`, four user-facing warning strings carrying ~20 literal
spaces from a missing `\`, a `#[allow]` and its doc block attached to the wrong function, a
comment-hygiene gate whose vocabulary misses the narration shape that actually survives in the
tree (and skips `.cpp` entirely), `milkconv/` and `core/src/milk/` absent from every map a session
reads first, and a C ABI spec sentence that says the opposite of what the shim does. **No file
under `core/src/render/` changes a pixel, and the golden suite proves it.**

## Context & problem

The review ran four lenses over the tree (layering, god modules, hot-path safety, doc/test
drift). Layering, real-time safety and the ABI came back clean; what it found was maintenance
debt and a few plain defects. Plans [0125](0125-the-scenes-share-their-gpu-boilerplate.md) and
[0126](0126-the-large-files-split-along-their-seams.md) take the structural half. This plan takes
what needs no design and no judgement, so that it can go first and so that the other two inherit a
harness and a gate that already work.

The specific findings, each confirmed by reading the line:

- `core/tests/`: `fn headless()` is defined in 27 of 40 integration files (14 byte-identical),
  `fixed_frame` ×7, `probe` ×7, `capture` ×7, `golden_dir`/`encode`/`decode` ×4. No
  `tests/common/` exists. Every new integration test starts by pasting the ADR-0016 skip block.
- `core/src/preset/schema.rs:756, :982, :1762, :1787`: a `format!` string broken across two lines
  without `\` — the operator sees `(x/y/rad/ang), which                      reads 0`.
- `core/src/render/scenes/particles/mod.rs:928-938`: the `seed` doc comment and
  `#[allow(clippy::indexing_slicing, reason = "spread/centre/pos index fixed [f32; 3] …")]` sit
  on `rebuild_if_stale` (`:961`); `seed` is at `:1078`. The reason string is false where it is.
- `scripts/check-comment-hygiene.mjs:229`: `VOCABULARY` is
  `this plan|the plan|used to|no longer|is new|previously`. Fourteen non-test comments in
  `core/src` + `standalone/src` narrate as `before Plan NNNN` / `until Plan NNNN`
  (`spectrum.rs:104,119,129`, `metrics.rs:336`, `palette.rs:779`, `schema.rs:1981`, …) and pass.
  The script globs `.rs` only, so `plugin-foobar/foo_lmv.cpp:13,548-551` is ungated.
- `CLAUDE.md:52-80`, `.claude/skills/architect/references/project-context.md:21-40`,
  `.claude/skills/dev/references/project-context.md:5-30`: none names `milkconv/` (8.4k lines, a
  workspace member outside `default-members`, so `cargo nextest run` skips its tests) or
  `core/src/milk/`. `tools/sd-filter/` and `presets/pending/` are unmapped too.
- `docs/specs/0001-c-abi.md:74-75` says the shim refuses a core whose ABI it was not built
  against; `plugin-foobar/foo_lmv.cpp:984` accepts `core_abi >= LMV_ABI_VERSION` with a comment
  saying why. The spec is the authority on the contract and it is wrong about this one clause.

## Decision

One plan, six phases, each independently committable and each reverting cleanly. The harness
extraction goes first because Plans 0125 and 0126 write tests against it. The gate widening lands
with its own fixture bite and with the fourteen surviving comments rewritten in the same commit —
a gate that goes red on the tree it is added to is not a gate. We rejected folding this into 0125
(the structural plan would then carry a docs sweep and a spec edit that have nothing to do with
GPU helpers) and rejected a per-finding micro-plan each (six close ceremonies for a day's work).

## Architecture diagram

```mermaid
flowchart LR
    subgraph tests["core/tests/"]
        C[common/mod.rs<br/>headless · fixed_frame · probe · golden io]
        T1[golden.rs] --> C
        T2[sanity.rs] --> C
        T3[…38 more] --> C
    end
    subgraph gates["scripts/"]
        H[check-comment-hygiene.mjs<br/>+ before/until/since Plan · + .cpp]
        F[fixtures/comment-hygiene/<br/>+ one seeded bite per new form]
        H --> F
    end
    subgraph maps["session entry points"]
        M1[CLAUDE.md] --- M2[skills/*/project-context.md]
    end
    gates -. pre-push + CI links job .-> maps
```

## Implementation phases

### Phase 1 — One harness for forty test files
- **Owner skill:** dev
- **What:** Create `core/tests/common/mod.rs` holding the ADR-0016 headless constructor, the
  synthetic `fixed_frame`, the `probe`/`capture` helpers and the golden PNG `encode`/`decode`/
  `golden_dir` trio; every integration file that defines a copy switches to `mod common;` and
  deletes its own. Where copies differ (the size constant, `prefer_software`), the shared fn takes
  the difference as a parameter — no file's behaviour changes.
- **Files touched:** `core/tests/common/mod.rs` (new), the 27 files defining `headless()`, the
  4/7/7 defining the golden and probe helpers.
- **Done when:** `grep -c "fn headless" core/tests/*.rs` is 0 and `core/tests/common/mod.rs` is 1;
  `cargo nextest run -p lmv-core` passes with the **same test count** as before the phase (the
  skip notices still print on a CPU-only adapter — the ADR-0016 shape is preserved, not merely the
  outcome); the golden suite passes unblessed. `core/tests/hygiene.rs` is not extended — `common/`
  is test code and is outside the hot-path set by construction.

### Phase 2 — Four strings, one attribute
- **Owner skill:** dev
- **What:** Join the four broken `format!` literals in `schema.rs` with `\` continuations; move the
  `seed` doc block and its `#[allow(clippy::indexing_slicing, …)]` from `rebuild_if_stale` onto
  `seed`, and give `rebuild_if_stale` the one-line doc it actually needs.
- **Files touched:** `core/src/preset/schema.rs`, `core/src/render/scenes/particles/mod.rs`.
- **Done when:** a test in `core/tests/preset.rs` loads a preset whose `[per_vertex]`-less
  binding names `rad` and asserts the warning text contains no run of two or more spaces; clippy
  stays green with the attribute on `seed` and **fails** if it is removed (that is the proof it
  was doing work where it now sits — `dev` runs that negative once and states the lint it saw);
  golden suite unchanged.

### Phase 3 — The gate learns the narration shape that survives
- **Owner skill:** dev
- **What:** Extend `VOCABULARY` in `scripts/check-comment-hygiene.mjs` with
  `\b(before|since|until|pre-|after)\s+(plan|adr|phase)\s+\d` and `\bany more\b`; extend the file
  walk to `.cpp`/`.h` under `plugin-foobar/` with a C/C++ comment lexer of the same shape as the
  Rust one (line and block comments; string literals skipped). Seed one fixture file per new form
  under `scripts/fixtures/comment-hygiene/` so the fixture run bites on each. Then rewrite every
  comment the widened gate reports on the live tree — mechanism stays, history goes to a bare
  ADR/plan citation — in the **same commit**, so the gate lands green.
- **Files touched:** `scripts/check-comment-hygiene.mjs`, `scripts/fixtures/comment-hygiene/*`,
  the ~14 `.rs` files and `plugin-foobar/foo_lmv.cpp` it reports.
- **Done when:** `node scripts/check-comment-hygiene.mjs` exits 0 on the tree;
  `node scripts/check-comment-hygiene.mjs scripts/fixtures` reports **every** seeded file, old and
  new, and nothing else; `git diff --stat` for the comment rewrites touches no line outside a
  comment (state it in the log — a `cargo build` that emits the same binary hash before and after
  is the cheap check).

### Phase 4 — The maps name every crate

> **LANDED OUT OF BAND 2026-08-29, by `architect` during a documentation audit**, in the same pass
> that corrected the revoked artifact store in these same three files (Plan 0134 Phase 3) — the two
> phases touch the same paragraphs and splitting them would have meant editing them twice. All five
> `[workspace] members` now appear by name in all three maps; `core/src/milk/`, `tools/sd-filter/`
> and `presets/pending/` are mapped in `CLAUDE.md` and in both skill contexts; the C ABI count
> narration is retired for a rule that forbids restating the roster here at all. Both Node gates
> exit 0. **`dev` should treat this phase as done and verify rather than redo it** — the done-when
> grep above is the check. **The rest of Plan 0124 is untouched and still owed**, including the
> `core/tests/common/` harness that 0125 and 0126 depend on.

- **Owner skill:** dev
- **What:** Add `milkconv/` and `core/src/milk/` to `CLAUDE.md`'s "Where things live" (with the
  one-line reason it is outside `default-members`, in the same voice as the `core-cabi` entry),
  and to both skills' `project-context.md` crate lists; add the `--workspace` note that
  `milkconv`'s tests are skipped by a bare `cargo nextest run` next to the existing `core-cabi`
  one in `dev`'s context. Map `tools/sd-filter/` and `presets/pending/` in `CLAUDE.md` with one
  line each. Retire the "twelve, and then to thirteen" narration at `CLAUDE.md:165` for a
  count-free sentence.
- **Files touched:** `CLAUDE.md`, `.claude/skills/architect/references/project-context.md`,
  `.claude/skills/dev/references/project-context.md`.
- **Done when:** every `[workspace] members` entry in root `Cargo.toml` appears by name in all
  three files (`dev` states the grep); `node scripts/check-doc-links.mjs` exits 0.

### Phase 5 — The spec says what the shim does
- **Owner skill:** dev
- **What:** Rewrite `docs/specs/0001-c-abi.md:74-75` to state the shim's actual rule — it refuses
  a core whose `LMV_ABI_VERSION` is **lower** than the one it was built against and accepts an
  equal or newer one — and carry the shim's one-sentence justification from `foo_lmv.cpp:984`.
  This is a wording correction to the contract document, not a shape change; the `extern "C"`
  surface and `LMV_ABI_VERSION` do not move.
- **Files touched:** `docs/specs/0001-c-abi.md`.
- **Done when:** the spec's compatibility clause and the comparison at `foo_lmv.cpp:984` agree
  under a plain reading; the spec's function roster still lists exactly the fifteen names in
  `core-cabi/include/lmv_core.h`.

### Phase 6 — The unwired scripts get a line or get deleted
- **Owner skill:** dev
- **What:** `scripts/docs-shots.mjs`, `tuple-sheets.mjs`, `tuple-paths.mjs` are live tools
  referenced from docs — add a "Renderers, not gates" line to `CLAUDE.md`'s `scripts/` entry
  naming them. `milk-softness.mjs` and `softness-sheets.mjs` are Plan 0114 one-shot judging
  renderers referenced only from that closed plan — `git rm` them and re-point the two plan
  references at the commit that held them.
- **Files touched:** `CLAUDE.md`, `scripts/milk-softness.mjs`, `scripts/softness-sheets.mjs`,
  `docs/plans/done/0114-*.md`.
- **Done when:** every file in `scripts/*.mjs` is either wired into `.githooks/pre-push` or
  `.github/workflows/*.yml`, or named in `CLAUDE.md`; `check-doc-links` exits 0.

## Data shapes

None new. Illustrative shape of the shared harness, so the parameterisation is agreed rather
than discovered:

```rust
// illustrative — core/tests/common/mod.rs
pub fn headless(width: u32, height: u32) -> Option<Renderer>   // None = ADR-0016 skip, notice printed
pub fn fixed_frame() -> AnalysisFrame
pub fn golden_dir() -> PathBuf
pub fn encode(img: &CaptureImage, path: &Path)
pub fn decode(path: &Path) -> CaptureImage
```

## Risks & open questions

- **Phase 3 rewrites comments in files Plans 0125/0126 will restructure.** Ordering this plan
  first makes those merges trivial; running it in parallel would not. The sequence in
  `docs/plans/README.md` states it.
- **The widened regex may catch a legitimate mechanism sentence** ("after Phase 2 of the pipeline"
  where "phase" is a shader stage). `hygiene-allow: <reason>` exists for exactly that; `dev`
  reports each escape it adds in the log so the reviewer can judge whether the vocabulary
  over-reaches. More than three escapes on the live tree is the signal to narrow the pattern.
- **Phase 1's "same test count" can be satisfied by a harness that silently skips more.** The
  done-when therefore also requires the skip notices to be unchanged in shape; `dev` runs the
  suite once on the software adapter and once on hardware and reports both counts.
- **Open (architect, not this plan):** the review measured `core/src` at 37 % comment lines with
  three files above 1:1, mostly transcribed ADR arguments and measurements. ADR-0127 drew the
  mechanism/decision line and this plan widens its gate, but a ratio is not gated and the user
  has not yet said whether it should be. That is an ADR question and is parked until asked.

## What this plan does NOT do

- No GPU helper, no scene edit, no file split — [0125](0125-the-scenes-share-their-gpu-boilerplate.md)
  and [0126](0126-the-large-files-split-along-their-seams.md).
- Does not move `new_from_win32_hwnd` out of `core` or the `shot` thread-local diagnostic off
  the scene renderer — both are seam moves and belong to 0126.
- Does not touch the comment **weight** question; see the open item above.
- Does not change `LMV_ABI_VERSION` or any `extern "C"` signature.

## Implementation log

> Written by `dev` — one row per phase as that phase's commit lands, and the close block after the
> last one. **The phases above are the contract; everything here is what happened.**
> **Observations, never conclusions:** this says where to look, architect decides how it went.
> No per-criterion pass list, no self-assessment, no narrative — but a deviation from the plan or
> an unmet done-when is always disclosed. Stays shorter than `## Implementation phases` above.

**Lane:** `WORK/lmv-plan-0124` on `plan-0124-review-fixes-no-pixels` (the branch name is
shorter than the placeholder above guessed).

| phase | owner | state | commit |
|---|---|---|---|
| 1 — One harness for forty test files | dev | done | 709544f |
| 2 — Four strings, one attribute | dev | done | committed with this row |
| 3 — The gate learns the narration shape that survives | dev | not started | |
| 4 — The maps name every crate | dev | not started | |
| 5 — The spec says what the shim does | dev | not started | |
| 6 — The unwired scripts get a line or get deleted | dev | not started | |

### Notes

**Phase 1.** `grep -c "fn headless" core/tests/*.rs` is 0. `core/tests/common/mod.rs` holds
**four** constructors, not the one the done-when names: `headless` / `headless_on` /
`headless_hardware` / `headless_tiered`. The copies differed in three ways, and only two of them
are the "size constant, `prefer_software`" the phase anticipated — `layer.rs` also required a
hardware adapter and skipped on WARP (ADR-0058), and `bloom.rs` called `new_headless_tiered`.
Those are different behaviour, not different arguments. The ADR-0016 skip block itself **is**
singular: one private `build()` holds it, the four public entry points delegate, and
`grep -c 'eprintln!("skipped: no GPU adapter'` over `core/tests/common/mod.rs` is 1.

**Phase 1, `probe`/`capture` not extracted.** The phase lists `probe` x7 and `capture` x7 among
the copies. They are not copies: the seven `probe` functions build seven different presets
(`arc_cost` a star roster, `palette_contour` a fragment_field, `mark_cost` a swarm, `layer` a
string substitution returning `String`, ...) and the seven `capture` functions take seven
different argument lists. They share a name and nothing else, so there was nothing to hoist and
`common/` holds neither.

**Phase 1, the eleven remaining inline skip blocks.** `arc_cost`, `attractor`,
`backdrop_palette`, `backdrop_ramp`, `background_composite`, `beat`, `collage_cost`,
`field_cost`, `mark_cost`, `palette_contour` and `reaction_diffusion` still spell the ADR-0016
block out inside a bespoke `capture_at`-shaped function rather than in a `fn headless`. They were
outside the phase's stated set (the 27 files defining `headless()`) and are untouched.

**Phase 2, six strings rather than four.** The phase names four broken `format!` literals at
`schema.rs:756/982/1762/1787`. The tree carries **six**, all the same defect and all in the named
file: the two per-vertex warnings (`:806`, `:1141`) and the four `[particles]` tuple-path
rejections (`:1941`, `:1949`, `:1958`, `:1966`). All six are rejoined. A seventh of the same shape
sits at `core/src/dsp/mod.rs:57` and is **untouched** — outside the phase's file list; it is in the
followups below.

**Phase 2, the negative clippy check does not fail.** The phase asks `dev` to remove the
`#[allow(clippy::indexing_slicing, ...)]` once and state the lint it saw. Removing it from `seed`
leaves `cargo clippy --workspace --all-targets -- -D warnings` **green**, so the attribute is
inert where it now sits — and was equally inert on `rebuild_if_stale`. The mechanism: the module
denies the lint at `mod.rs:49-54`, but `indexing_slicing` does not fire on a constant index into a
fixed-size array, which is exactly the case the attribute's own reason string describes
("index fixed [f32; 3] at constant offsets"). Two probes pin this down — `spread[0] + center[0]`
inserted into `seed` with the attribute absent produced no diagnostic, while `Some(1u32).unwrap()`
in the same position produced `error: used unwrap() on Some value ... note: the lint level is
defined here --> mod.rs:49`, so the deny does reach the function. Both probes were removed. The
attribute is moved as the phase says and **not** deleted: whether a dead `#[allow]` should stay is
a call this phase does not authorize.

**Phase 2, `rebuild_if_stale` needed no new doc.** The phase asks for "the one-line doc it
actually needs". It already had its own correct doc block sitting *below* the misplaced one; only
the orphaned prose and the attribute moved, and nothing was written for it.

**Phase 2, the test covers all six, not one.** `operator_messages_carry_no_run_of_spaces` in
`core/tests/preset.rs` walks both per-vertex warnings and all four tuple-path errors rather than
the single `rad` case the phase names, since a test guarding one of six rejoined strings leaves
five unguarded. It rejects `
` and `	` alongside a two-space run. Bite check: re-breaking the
first literal fails it with `per-vertex reach: message is not a single clean sentence: "... which
                      reads 0 ..."`; the string was then restored.

**Phase 1, evidence for "same test count".** `#[test]` attributes under `core/tests/` are 236 at
`HEAD` and 236 in the tree. `cargo nextest run --workspace`: 1199 passed, 5 skipped, golden suite
green unblessed. The renderer is hardware on this box, so the CPU-only skip path was not
exercised at runtime; what is checked is that its branch and notice text are unchanged from the
copies they replace.

### Close triggers

- **`presets/` touched:**
- **Plan header `Closes:`** none
- **What shipped:**
- **Operator docs touched:**
- **Backlog probes (`node scripts/check-backlog-claims.mjs`):**
- **Outstanding `human` phases:**

## Followups (after this lands)
