# ADR-0075 — The IFS family is parameterized by its singular values, and morphs there

> **Status:** **accepted** (Plan 0062; carries an [Outcome](#outcome-2026-08-05) section — the step
> uniform is **160 bytes, not 144**; the showcase pair named below came **last of five** and the
> figure this ADR doubted came first; and `morph`'s visible rate is **front-loaded**, not the linear
> read "every value between is a real figure" invites)
> **Date:** 2026-08-04
> **Related plan(s):** [0062](../plans/done/0062-the-chaos-game-grows-a-fern.md)

## Context

The user asked whether the Barnsley fern could give this project something "generative and organic
at the same time". The engine is closer to it than it looks: the `attractor` scene
(`core/src/render/scenes/particles/mod.rs`) is already a GPU chaos game. 50 000–150 000 particles
each iterate one map per fixed 1/60 s step and deposit additively into a decaying trail field. An
iterated function system is the same loop with a different step — pick one of N affine maps at
random, apply it — so the fern is a fifth `AttractorFamily`, not a new render idiom.

Two facts make it more than "one more attractor". First, an IFS is **not one figure**: the same
loop draws a fern, a bare tree, a dragon curve, a Sierpinski triangle and a spiral, distinguished
only by twenty-four affine coefficients and four probabilities. Second — and this is the capability
the engine does not have anywhere else — **those coefficients interpolate**. Every other structural
choice in this codebase is discrete: an L-system's production rules, `star_pattern`'s three
precomputed contact angles (which is exactly why [design-backlog 0007](../design-backlog.md) could
not blend them and needed [ADR-0060](0060-star-pattern-variants-interpolate.md) to rescue it). A
continuous path from one figure to another, drivable from audio, is new.

The obstacle is that an IFS has a cliff the existing families do not. De Jong and Clifford are
bounded for *any* `a b c d` — a preset can drive them anywhere and the cloud stays on screen. An
IFS converges to its attractor only while every map is a contraction. Push one map's linear part
past unit operator norm and the orbit diverges: positions run to infinity, then to `NaN`, and the
particle buffer is dead for the rest of the session because every subsequent step is
`NaN`-propagating. There is no recovery short of a reseed the preset may never trigger. So "audio
drives the shape" — which is the whole point — cannot be built by handing raw coefficients to the
expression grammar and hoping.

The measurements that shape the answer, computed for the canonical fern (maps `f₁`–`f₄` in
Wikipedia's order):

| map | role | largest singular value σ_max | det |
|-----|------|------------------------------|-----|
| `f₁` | stem | 0.160 | 0 (rank 1) |
| `f₂` | successively smaller leaflets | 0.851 | +0.724 |
| `f₃` | largest left frond | 0.341 | +0.104 |
| `f₄` | largest right frond | 0.379 | **−0.109** |

Three things follow. The headroom to divergence is real but not generous — `f₂` sits at 0.851, so a
global scale lever has about 17 % to give before it touches 1.0. The probability-weighted per-step
contraction is `exp(Σ pₖ ln σ_max,k) = 0.742`, so a displacement shrinks a thousandfold in ~23 steps
— **0.39 s** at the fixed 1/60 s step, which is how long the figure takes to resolve after any
disturbance. And `f₄` is **orientation-reversing**: any parameterization that cannot represent a
reflection reproduces the fern with its right-hand frond wrong, silently.

## Decision

We will add `AttractorFamily::Ifs(IfsFigure)` — a fifth family carrying one of five curated
figures — and parameterize every map not by its raw affine coefficients `a b c d e f` but by the
**singular value decomposition of its linear part**, `M = R(θ) · diag(sx, sy) · R(φ)` with `sy`
signed so a reflection is representable. All morphing and all audio-driven levers act in that
space, on the CPU, resolving to a plain 2×3 affine table uploaded to the compute step each frame.

The parameterization is chosen because it makes the cliff unreachable rather than guarded against.
`R(θ)` and `R(φ)` are isometries, so a map's contractivity is exactly `max(|sx|, |sy|) < 1` — a
scalar comparison on two numbers per map, not a property of a matrix. Three consequences fall out
for free:

- **Figure-to-figure morphing is contractive by construction.** Interpolating two contractive maps'
  singular values gives a value below 1 because both endpoints are, and interpolating their angles
  does not touch the singular values at all. No guard, no clamp, no divergence check on the morph
  path.
- **The free levers are guarded by one clamp on one number.** `vigor` multiplies every map's
  singular values and is clamped so `max σ ≤ 0.97`; `curl` adds to every `θ`; `lean` rotates the
  translation vectors, which do not enter contractivity at all; `bias` reweights the probabilities,
  which changes only where points land, never whether they converge.
- **A degenerate map stays legal.** `f₁` is rank 1 (`sx = 0`) and morphing a reflection into a
  non-reflection passes through `sy = 0`. Both are contractions; the branch momentarily collapses
  to a line and recovers. Nothing diverges.

Maps are paired for morphing **by index**, and every curated table is authored in one canonical
order — index 0 the trunk or dominant map, 1 the main body, 2 the left branch, 3 the right branch —
padded to four maps by duplicating a map at probability 0 where a figure has fewer. Pairing is an
authoring convention on six hand-written tables, not an algorithm.

The figure's **framing is fitted from the resolved table with every lever at neutral**, as a
33-entry lookup over `morph` built once at preset load and lerped per frame. Fitting at neutral is
deliberate and is the non-obvious half: a fit that included the levers would cancel them, because
`vigor` pumping the figure larger on a beat and the fit shrinking it back is a net zero, and the
most audible lever in the set would render as nothing.

## Consequences

### Positive

- **The engine gains continuous structural morphing for the first time.** A preset can drive
  `morph` from audio and cross from a fern to a spiral through figures that are valid at every
  intermediate value. Nothing else in this codebase can do that.
- **The safety property is testable on the CPU with no GPU.** `resolve(table_a, table_b, mix,
  levers) -> IfsUniform` is a pure function; a sweep over every figure pair × every lever extreme
  asserting `max σ < 1` is an ordinary unit test. Divergence is excluded before a shader runs.
- **The shader stays trivial.** It draws one uniform sample, picks a map from a cumulative
  probability table, and applies a 2×3 affine. No decomposition, no clamping, no matrix math on the
  GPU — all of which would have been untestable there.
- **No new render idiom, no new seam.** The trail field, the additive deposit and its count
  normalization ([ADR-0065](0065-the-attractor-deposit-is-normalized-by-particle-count.md)), the
  palette LUT path, the view transform, `fade`, `density` and `reseed` all apply unchanged. The
  `Scene` trait and the C ABI are untouched.
- **`reseed` acquires a good meaning for free.** The existing jitter kick is re-contracted at 0.742
  per step, so a beat visibly shakes the plant and it heals in ~10 steps (0.17 s) — a designed
  response, from a mechanism that already exists.

### Negative

- **`0.97` is a chosen number with no principled value.** It is far enough below 1.0 that
  floating-point error cannot cross it and close enough to leave `f₂`'s 0.851 room to grow, but it
  is a look constant. A preset asking for more `vigor` than the clamp allows gets silence rather
  than an error, which is the same undiscoverable-ceiling shape `presets/README.md` already has to
  document for `bloom_threshold`.
- **The curated tables must be hand-authored in canonical order, and nothing enforces it.** A table
  whose index 2 is its trunk morphs into its partner's left branch and the intermediate figures are
  ugly. This is a comment-and-review property, not a checked one.
- **Fitting at neutral levers means a hard `vigor` push can leave the frame.** That is the intended
  trade — an audible lever that can overshoot beats an inaudible one that cannot — but it is a real
  edge an author will hit, and `zoom` is the only recourse.
- **Five figures is a small, closed set.** Adding a sixth is a code change, not a preset change.
  This is deliberate (24 free coefficients with a contractivity cliff is close to unauthorable —
  most random tables are a blob), but it does mean the content lane cannot discover figures.
- **The step uniform grows from 32 to 144 bytes for every family**, including the four that ignore
  the new fields. Negligible in bandwidth; noted because it changes a struct four families share.

### Neutral

- The compute step gains a per-step random draw, salted by a monotonic step counter rather than by
  the reseed counter. Determinism is preserved exactly — the draw is a pure function of the
  particle's fixed seed and the step index, and the step index is a pure function of accumulated
  injected `dt`, which captures pin at 1/60 s.
- `JITTER_MODE` moves from shader id 4 to 5, which is what its own doc comment
  (`particles/mod.rs:344`) anticipated a fifth family would do.

## Alternatives considered

### Alternative A — raw affine coefficients with a runtime divergence guard

Hand `a b c d e f` to the preset surface directly and catch divergence after the fact: detect
`NaN`/out-of-range positions in the compute step and respawn those particles, or renormalize the
cloud by its measured extent each frame.

Rejected because the guard is both unreliable and untestable where it lives. It runs on the GPU, so
the property "this preset cannot kill the cloud" is provable only by rendering, and a preset that
diverges only on a loud passage will pass every capture. It is also the wrong shape for morphing:
linear interpolation of two raw matrices *is* contractive (the operator-norm ball is convex), but it
passes through matrices whose rotation has collapsed, so a fern morphing to a spiral goes through a
smeared intermediate rather than turning. The SVD path turns.

### Alternative B — a separate `ifs` scene rather than a fifth attractor family

Give the IFS its own `SystemKind`, free to design its whole surface without touching a scene that
carries golden baselines for four existing families.

Rejected on duplication. The attractor is the most complex scene in the repo, and a separate scene
copies its compute dispatch, its ping-pong trail field, its deposit normalization, its projection
and spin, its palette LUT binding and its view transform — every one of which is shared machinery
that has already been debugged twice
([ADR-0037](0037-internal-grid-is-a-resolution-not-a-shape.md),
[ADR-0070](0070-a-feedback-pass-addresses-its-own-target-in-framebuffer-space.md)). The IFS differs
from De Jong in one line of the step shader. The cost of the family route is that the existing four
families' output must be proved byte-identical, which is a golden assertion we already run.

### Alternative C — fit the framing from the fully-resolved table, or do not fit at all

Either compute the figure's extent from the table *after* the levers apply, so the figure always
fills the frame; or hand-author one scale and centre per figure and let the morph fall where it
falls.

Both rejected, for opposite reasons. A full fit cancels its own most valuable lever: `vigor` exists
to make the plant surge on a beat, and a fit that re-frames every frame turns that surge into no
visible change. No fit at all breaks the morph, because a figure's extent changes continuously
along the path between two figures — a hand-authored fern scale leaves the dragon curve tiny and
the tree cropped, at every intermediate value. Fitting the morph but not the levers is the only
combination where both work.

## Notes

Fern coefficients and probabilities are Wikipedia's canonical set. The singular values, the
per-step contraction figure and `f₄`'s negative determinant in the Context table were computed
directly from them and are reproducible by hand; `σ₁σ₂ = |det M|` checks each row.

The reason the plan can promise "no light is deposited off the figure" during growth is that the
fern's root `(0, 0)` is the fixed point of `f₁` and therefore lies *on* the attractor. An orbit
started there is on-figure at every step and still visibly climbs — `f₂(0,0) = (0, 1.60)`, then
`(0.064, 2.96)`, then `(0.173, 4.11)`. Any contractive map's fixed point is `(I − M)⁻¹ t` in closed
form, so this generalizes to every curated figure. That property is what the successor plan's
unfurl is built on; it is recorded here because it is a consequence of this parameterization rather
than of that plan's design.

## Outcome (2026-08-05)

Plan 0062 implemented this in full — five phases of `dev` and a `preset-author` content pass — and
the decision holds: no reachable table diverges, and the property is asserted on the CPU over all
25 ordered figure pairs × 33 morph positions × every lever extreme, with no GPU involved. Four
things this ADR recorded turned out to be wrong, and they are worth separating by *kind*, because
one is arithmetic and three are claims about what the family would look like.

**The step uniform is 160 bytes, not 144.** The arithmetic in Consequences omitted the alignment
`step_index` forces: the scalar block ahead of the `vec4` affine table was already exactly full, so
one more `u32` rounds it up to the next multiple of 16. The number is pinned by
`the_step_uniform_carries_the_ifs_table_in_one_binding`. Nothing else changes — the bind-group
layout still gains no binding, which was the load-bearing half of the claim.

**Every claim about which figures would be worth looking at was wrong, in both directions.** The
content pass swept five candidate crosses end to end as filmstrips:

- **`fern → spiral`, this ADR's own showcase pair, came last of the five.** Anything ending at the
  spiral thins into ragged streaks with half the frame empty, and the cause is in this document's
  own numbers: the spiral's dominant map contracts at only `0.93` (after `SPIRAL_ARM`), so an
  intermediate spends nearly every sample on a map that barely contracts and the orbit spreads
  instead of settling. **A figure near the contractivity ceiling is a fine endpoint and a poor
  morph target**, which is a property of the parameterization this ADR did not anticipate.
- **`sierpinski → fern` came first, by a distance** — and the reason is the exact thing this ADR
  hedged about. Consequences allowed that the Sierpinski might be "only a correctness fixture… the
  least organic thing in a plan whose brief was organic". It earns a preset (`attractor_dissolve`),
  but **not as a look — as an endpoint**: its rigidity is what makes the dissolve legible, because
  interpolating singular values and rotations *separately* rounds sub-triangles into leaflets while
  the silhouette holds. The ADR was right that it is the least organic figure and wrong that this
  made it the least valuable one.

**`morph`'s visible rate is front-loaded, and "every value between is a real figure" invited the
wrong tuning.** The statement is true — every intermediate table converges, which is the whole
decision — but it reads as an invitation to use a small `morph` for a little life. Measured on
fern → dragon, the lit width of the figure as a fraction of the frame is `0.248` at `morph = 0` and
`0.448` at `0.05`: **half again as wide by a twentieth of the range.** Per-map rotation compounds
through the recursion, so the rate is dominated by the *angle* difference between the two tables.
`morph` is a **travel** knob; a preset that wants to remain one figure should bind the four levers
and leave `morph` alone. Both shipped presets are built that way. This is not a defect in the
parameterization and no code changed for it — but it cost a draft, so it is recorded here and
documented in `presets/README.md`.

**One thing this ADR got exactly right, and it was not obvious.** Fitting the framing at neutral
levers (Alternative C) is what makes `vigor` visible: the content pass measured a `0.11` swing
taking the lit area of the frame from `0.112` to `0.297`. A full fit would have returned zero. The
accepted cost — a hard `vigor` push can leave the frame — was hit in practice and is real, and
`zoom` is indeed the recourse.

**One consequence that landed harder than predicted, and is not fixed here.** The initial fill
scatters particles over the figure's bounding box, and the plan predicted a "haze" while it
contracts. It is not a haze: it is a **legible, hard-edged, axis-aligned rectangle** — the same
artifact class [ADR-0066](0066-a-reseed-disturbs-the-cloud-rather-than-replacing-it.md) was written to remove from
`reseed` — now on every switch *into* the family, with the ~1 s preset dissolve landing entirely
inside it. Tracked as [backlog 0064](../design-backlog.md); the successor plan's staggered respawn
removes it as a side effect.
