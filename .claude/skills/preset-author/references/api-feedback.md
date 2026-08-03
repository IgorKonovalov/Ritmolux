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
- **No stateful expressions** (`smooth()`, `slew()`, a per-frame accumulator, a latch). The
  evaluator is pure by hard invariant; smoothing lives in the render layer as `[smoothing]`, which
  covers easing but **not** "hold this value until the next beat". Beat-latched state is a real,
  repeatedly-felt gap.
- ~~**No per-bin spectrum access**~~ — **delivered by Plan 0034.** `bin(x)` samples the 64-band
  log-spaced array at a normalized position; a `spectrum` system draws N elements off it; and a
  binding naming `index` is evaluated once per element. Note `bin()` is a **narrow probe** (~2 of
  the 64 bands, a window ~0.032 wide in `x`), not a region average — see backlog 0016. Still
  absent: `bin_range(lo, hi)`.
- **No randomness / noise function** (by design: determinism, NFR §6).
- No user-defined variables or intermediate bindings — a long expression cannot be factored, so a
  repeated sub-expression is written out each time.

**Scenes / vocabulary**
- **One curve family** (`maurer_rose`). Superformula, harmonograph, epicycloid are catalogued
  (`docs/generative-techniques-catalogue.md`) and cheap, but not built.
- **Four star tilings** (4/6/8/12). `variant` is a **continuous** contact angle since Plan 0054
  (ADR-0060) — fractional values are real rosettes and `[smoothing]` on it morphs. The rosette's
  **interior** is still empty at every angle, which is the open half of design-backlog 0007.
- **No author-supplied shader/WGSL pass** — you cannot write a look the built-in scenes can't draw.
- **Particle/segment counts are not preset-settable**: the attractor's particle count is fixed
  (`samples` on the curve is, but the swarm's and attractor's populations are not).
- **No tempo-varying structural morph on the rose** beyond `n`/`d`/`phase`/`radial_offset`.

**Composite / colour**
- The composite order is **fixed**, not a graph — no reordering, no per-stage routing, no
  multi-scene compositing (two scenes at once).
- **`mirror_*` is line-only**; the screen-space kaleidoscope is the general tool.
- ~~**`[palette]` is silently inert on the three line scenes**~~ — **fixed by Plan 0054 /
  ADR-0059.** Every scene now reaches `[palette]`, `[palette_b]`, `palette_mix`, `hue_spread` and
  `saturation`. Each line scene walks `hue_spread` along its own axis (path position / generation
  depth / radius / band index) — the table is in `presets/README.md`. Two live limits: a
  bracket-free grammar (`lsystem_arrowhead`) has one generation, and `star_pattern`'s radial ramp
  is measurably flat until its interior is redesigned.
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
- **`MAX_SEGMENTS = 20_000`** caps line geometry; overflow is surfaced, not silent, but truncated.
- **Determinism / seeded randomness** (NFR §6) — there is no unseeded randomness in the grammar;
  don't assume any.

## Curation handoff — shipping a preset

**Embedding is no longer a Rust edit.** `core/build.rs` globs `presets/*.toml` and `include_str!`s
them, so *dropping a file into `presets/` ships it* — no `EMBEDDED` array, no length type, no count
assert (ADR-0022). What that changes for this lane:

- The mechanical work of curation is now a **content commit** you can prepare in full.
- The **decision** to ship still isn't unilateral: an embedded preset joins the behavioral gates —
  `sanity` (not blank, not a dot), `reactivity` (moves for at least one band) and `animation` (not
  frozen) iterate the whole embedded set, so a weak preset fails CI for everyone. Verify with
  `--report` before proposing.
- ADR-0017 drew the lane boundary at "`dev` embeds" when embedding meant editing Rust. That premise
  is gone; if the user wants this lane to land curated presets directly, that is a boundary change
  worth an `architect` note rather than an improvisation.

Hand off like: "Preset `<name>` is a strong ship candidate — it renders X, passes `--report` with
reactivity on bass/treble, and is not a near-dup of Y. Shipping it is now just committing
`presets/<name>.toml`; say the word and I'll prepare that commit, or route it to `dev`."
