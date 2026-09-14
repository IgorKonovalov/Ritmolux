# The second duty: feeding the API's evolution (and the curation handoff)

The preset surface is small and **deliberately growing** — and this lane's reports are why it
grew. Grammar v2 (`cos`/`sqrt`/`pow`/`mod`/`smoothstep`/`select`, `pi`/`tau`, comparisons,
`tempo`), per-param easing, the engine-wide composite, the palette surface and the ink remap all
started as friction reported from here. Consuming the API is half the job; **reporting where it
stopped you is the other half.**

## Mindset: friction is signal, not a dead end

When you reach for something that doesn't exist, the instinct is to work around it and move on.
**Don't work around it silently.** A workaround is a marker of a missing capability, and the person
who felt the friction is the right person to report it. You still deliver the best preset the
*current* surface allows — you just also carry out what you learned about its edges.

## How to capture and route it

Keep a running list while you work. At the end of a session, if the friction is real, hand
`architect` a short note; `architect` decides whether it becomes an ADR + a `dev` plan. You do
**not** write the ADR or the code.

```
API feedback — preset-author, <date>

Wanted: <the look/behavior you were going for, in one line>
Reached for: <the capability — e.g. "a superformula curve family", "per-bin spectrum">
Current surface can't: <why — what you had to do instead, or that it's simply absent>
Concrete example:
  <the binding or preset snippet where it bit>
Impact: <how often this comes up / how much it limits looks — one line>
Not: engine design. That's architect's call — this is the motivating friction.
```

Route it: "This is engine work, not a preset. Handing architect a feedback note; start a fresh
`/architect` session to decide if it's ADR-worthy." Then stop reaching into Rust.

**Check `docs/design-backlog.md` first** — captured-but-not-yet-promoted feedback lives there, and
re-raising an entry is still useful signal (it says the demand is real), but say so rather than
filing it as new.

## Gaps that are still real (verify before reporting — the surface keeps moving)

**Expression grammar**
- **No stateful expressions** (`smooth()`, `slew()`, a per-frame accumulator). The evaluator is
  pure by hard invariant, and state lives in render-layer tables instead: `[smoothing]` eases,
  ~~beat-latched state~~ is **delivered** — `[latch]` (ADR-0137) arms on one condition and fires on
  another, and `[hold]` re-samples a value on a musical edge and holds it in between (both in
  `docs/presets.md`). What is still absent is an expression that reads its own previous value.
- ~~**No per-bin spectrum access**~~ — **delivered by Plan 0034.** `bin(x)` samples the 64-band
  log-spaced array at a normalized position; a `spectrum` system draws N elements off it; and a
  binding naming `index` is evaluated once per element. Note `bin()` is a **narrow probe** (~2 of
  the 64 bands, a window ~0.032 wide in `x`), not a region average — see backlog 0016. Still
  absent: `bin_range(lo, hi)`.
- ~~**No randomness / noise function**~~ — **delivered by ADR-0051.** `hash(x)` scatters and
  `noise(x)` wanders, both seeded by `[generator] seed`, so determinism holds (`docs/presets.md`,
  "`hash(x)` and `noise(x)` — seeded randomness").
- No user-defined variables or intermediate bindings — a long expression cannot be factored, so a
  repeated sub-expression is written out each time.

**Scenes / vocabulary**
- **Five curve families** since Plan 0162 — `maurer_rose`, `lissajous`, `hypotrochoid`,
  `superformula`, `harmonograph` — with `pen`, `sym`, `sharpness`, `lobe` and `decay` beside them.
  The epitrochoid needs no arm of its own: it is `hypotrochoid` with a negative `n`. Fractal
  flames stay catalogued (`docs/generative-techniques-catalogue.md`) and unbuilt.
- **Four star tilings** (4/6/8/12), plus `tiling = "none"` for a rings-only figure. `variant` is a
  **continuous** contact angle since Plan 0054 (ADR-0060) — fractional values are real rosettes and
  `[smoothing]` on it morphs. ~~The rosette's interior is empty~~ — **delivered by ADR-0079**:
  `[generator] rings` fills it with concentric rings of motifs, moved by `ring_phase`,
  `ring_spread` and `ring_scale` (design-backlog 0007 is closed). The motif roster is closed, so a
  motif outside it is still feedback.
- **No author-supplied shader/WGSL pass** — you cannot write a look the built-in scenes can't draw.
- **Particle/segment counts are not preset-settable**: the attractor's particle count is fixed
  (`samples` on the curve is, but the swarm's and attractor's populations are not).
- **No tempo-varying structural morph on the rose** beyond `n`/`d`/`phase`/`radial_offset`.

**Composite / colour**
- The composite order is **fixed**, not a graph — no reordering, no per-stage routing. A preset
  may compose **one** second scene through `[layer]` (ADR-0090), sharing the main scene's
  `[palette]`; a third scene, or a second palette for the layer, is still absent.
- **`mirror_*` is line-only**; the screen-space kaleidoscope is the general tool.
- ~~**`[palette]` is silently inert on the three line scenes**~~ — **fixed by Plan 0054 /
  ADR-0059.** Every scene now reaches `[palette]`, `[palette_b]`, `palette_mix`, `hue_spread` and
  `saturation`. Each line scene walks `hue_spread` along its own axis (path position / generation
  depth / radius / band index) — the table is in `presets/README.md`. Two live limits: a
  bracket-free grammar (a Koch-style `F = "F+F--F+F"`, as in `lsystem_rime`) has one generation, and
  a bare `star_pattern` interlace has a flat radial ramp — declaring `rings` makes it live.
- Palette interpolation is plain RGB (no OKLab / perceptual blending yet).

**Transitions**
- Cross-preset dissolves are engine-configured policy (kind, duration) — a preset **cannot declare
  its own** `[transition]`, and dissolves are not beat-quantised. Both are named follow-ups, so
  align feedback with them rather than re-proposing.

**Determinism caveat:** feedback sims and chaotic attractors are not bit-identical across GPU
vendors — "identical on every device" holds *visually*, not pixel-exactly. Don't author a preset
that depends on exact cross-machine pixels.

## NFR limits a preset must respect

From `docs/nfr.md` — a preset that violates these is a bug, and pushing past them is engine work:

- **60 fps @ 1080p on an integrated GPU** is the floor. The levers that blow it: dense line geometry
  (`samples`, `max_depth`, `visible_depth`, high `mirror_order`), heavy additive overdraw (swarm
  `size` × density), and stacking composite stages (`trails` + `kaleido_*` + a heavy scene).
- **The line-geometry segment cap** (`TierConfig::max_segments`, per tier; author against `Floor`)
  — overflow is surfaced, not silent, but truncated.
- **Determinism / seeded randomness** (NFR §6) — there is no unseeded randomness in the grammar;
  don't assume any.

## Curation handoff — shipping a preset

**This lane lands presets itself** ([ADR-0081](../../../../docs/adrs/0081-the-content-lane-lands-presets-and-architect-curates-the-set.md)).
`core/build.rs` globs `presets/*.toml` and `include_str!`s them, so *committing a file into
`presets/` ships it* — no `EMBEDDED` array, no count to bump (ADR-0022). ADR-0017's old boundary
("`dev` embeds") stood on embedding being a Rust edit; ADR-0022 removed that premise and ADR-0081
moved the boundary. What that means in practice:

- **The gate authorizes the commit.** An embedded preset joins the behavioral suite — `sanity`,
  `reactivity`, `animation` and `distinctness` iterate the whole embedded set, so a weak preset
  fails CI for everyone. Run `ritmolux --check <file> --strict` and `cargo nextest run -p rlx-core`
  before committing, and read `--report` for what the suite cannot judge (SKILL.md step 7).
- **`architect` curates the *set*,** at plan-close cadence — whether a family converged, whether a
  preset earns its place against what already ships. That is a review of what you landed, not a
  permission you wait for.
- **`dev` does not courier content.** It edits a preset only when an engine change forces it (a
  renamed param, a retired default).

What you still hand off rather than decide: a look you think belongs in the **curated rotation**
("`<name>` renders X, reacts on bass/treble per `--report`, is not a near-dup of Y — worth weighing
against the set"), and every engine gap from the sections above.
