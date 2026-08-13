# ADR-0103 — the IFS fit frames a figure that does not turn, and says so

> **Status:** proposed
> **Date:** 2026-08-13
> **Related plan(s):** [0089 — the framing contract stops lying](../plans/0089-the-framing-contract-stops-lying.md)
> **Supplements:** [ADR-0075](0075-ifs-family-morphs-in-singular-value-space.md),
> [ADR-0093](0093-attractor-tuples-are-content-with-per-tuple-framing.md)

## Context

`FRAME_FILL = 0.88` (`core/src/render/scenes/particles/ifs.rs:500`) is documented as *"the fraction
of the frame a fitted figure occupies along its binding axis"*, and `fit_scale`'s docstring states
that taking the smaller of the vertical and horizontal fits *"is what makes 'inside the frame' true
at every aspect rather than at one"*. A shipped world falsifies it:
[backlog 0089](../design-backlog.md) reported the Heighway dragon overrunning the **frame corner** at
the default view at 1280x720, worked around with a base `zoom = 0.92`.

**The mechanism is rotation, and the arithmetic makes it general.** The fit measures an
**axis-aligned** bounding box (`chaos_extent` → `Extent::half`); the 2D branch of `project`
(`core/src/render/scenes/particles/shaders.rs:448`) then centres the figure and rotates it in-plane
by the spin phase. A centred AABB of half-extents `(hx, hy)` rotated by `θ` has half-extents
`hx·|cos θ| + hy·|sin θ|` and `hx·|sin θ| + hy·|cos θ|`, both reaching `r = sqrt(hx² + hy²)` at
their worst angle. With `a = hx / hy` and the fit `s = FRAME_FILL · min(1/hy, aspect/hx)`:

- vertical-binding (`a <= aspect`) stays inside at every angle only if
  `FRAME_FILL · sqrt(1 + a²) <= 1`, i.e. `a <= sqrt(1/FRAME_FILL² − 1) = 0.5397`;
- horizontal-binding (`a > aspect`) is **unsatisfiable** at any aspect at or above 1, since
  `sqrt(1+a²)/a > 1` and `0.88 · 16/9 = 1.564`.

So a **square** figure overruns by 24.4 % at 45°, and only a figure at least `1.85x` taller than
wide is safe at every angle. This is not one figure's bug.

**Three things make it a decision rather than a patch.**

1. **The default turns.** `spin` defaults to `1.0` and `SPIN_RATE = 0.18` gives one revolution per
   34.9 s (`particles/family.rs:969`), so the overrun is reachable with nothing bound and no lever
   pushed. Every author of a 2D IFS world meets it on their first render.
2. **The same fit already has a documented unmodelled input, and the project accepted it.**
   `FitLut` is built with every lever neutral, deliberately — ADR-0075 Alternative C, because a fit
   that saw `vigor` would shrink the figure by exactly the surge `vigor` exists to produce, for a
   net zero — and its docstring prices it: *"a hard `vigor` push can leave the frame. That is the
   intended trade … and `zoom` is the recourse."* Rotation is the second such input and is missing
   from that sentence.
3. **The shipped library already pays the price three times over.** All three presets on a 2D IFS
   figure — `attractor_dragon` (dragon), `attractor_fern` (fern), `attractor_volute` (spiral) —
   independently bind `spin` down to a small rock **and** set base `zoom` below 1 (0.92 / 0.96 /
   0.96). Three authors converged on one workaround, and only one header names why.

## Decision

**The fit's contract is that it frames a figure at neutral levers and zero rotation, and `zoom` is
the recourse for both.** We restate the invariant to be true, pin the closed form above as a test
over every figure, and say in the docstrings and in `presets/README.md` that rotation is an axis the
fit does not cover — the same shape ADR-0075 already gave the levers, extended to the input it
forgot. Nothing about the fit's arithmetic changes; no pixel moves.

We do **not** buy the stronger guarantee now. The two routes that would make the original invariant
true are **priced and deferred with a trigger**, not rejected on their merits: they shrink every
shipped 2D figure, re-frame all three worlds on top of compensating `zoom` values they already
carry, and owe a golden re-bless plus a content pass — for a guarantee no author has asked for. The
trigger is someone wanting a 2D IFS world at **full default rotation**, and the thing that decides
it is a rendered comparison, because framing in this project is judged from side-by-side output
rather than from argument.

## Consequences

**Positive**

- A stated property stops being false, which is the whole reason this is an ADR and not a backlog
  line. The project's standing rule is that a falsified stated invariant does not stand.
- **Three presets' sub-1 base `zoom` becomes explicable.** Two of them currently read as taste, so
  the next author tidying them up would silently re-break the framing.
- The closed form is a **property, not a measurement** (ADR-0071): dimensionless, exact, and
  identical on every adapter and every display. It can be asserted universally without naming a
  machine.
- The fit stays a load-time table, a pure function of the figure pair and `morph`. Everything
  ADR-0075 bought — no per-frame chaos game, nothing stochastic shimmering between frames, and a
  `vigor` that actually surges — is untouched.

**Negative**

- **Nothing guarantees a 2D IFS figure is on screen at the default view.** That is the honest cost
  and it is worse than the lever case it is modelled on, because a lever overrun follows an author's
  push while this one is the default. What makes it acceptable is that the recourse is one static
  number, the three shipped worlds already found it, and it is now written down where the fourth
  author will read it.
- **A dark corner is invisible to every instrument we own.** Coverage measures the scene
  ([ADR-0067](0067-coverage-measures-the-scene-not-the-backdrop.md)) and would read a clipped figure
  as a *fuller* frame, not an emptier one; the in-frame geometry fraction
  ([ADR-0083](0083-in-frame-geometry-is-measured-at-the-line-renderers-draw-seam.md)) measures at the
  **line renderer's** draw seam and no particle scene reaches it. So this failure is found by looking,
  and only by looking.
- **The deferral is real work postponed, not work avoided.** If a fourth 2D figure ships wider than
  `a = 0.54` — which is every figure in the roster except the fern — its author pays the same
  rediscovery until one of the deferred routes lands.

## Alternatives considered

- **A — fit against the rotation-invariant radius `r = sqrt(hx² + hy²)`.** One line in `fit_scale`,
  and the original invariant becomes true at every angle for every figure. **Deferred, not
  rejected.** It shrinks every 2D figure by `sqrt(1 + a²)` — about 11 % for the fern, which already
  satisfies the bound and would pay for nothing — and it lands *on top of* three presets' existing
  sub-1 `zoom`, so all three become a double shrink and want re-framing in the same breath. It also
  moves goldens. The lost decisive reason is that it charges the compliant figure for the
  non-compliant one.
- **B — a per-figure measured fill, in ADR-0093's shape.** Each figure carries the fill that keeps
  *it* inside at every angle, the way an attractor tuple carries its own measured framing.
  **Deferred, not rejected**, and it is the better of the two if the trigger fires: the cost lands
  per figure instead of uniformly, so the fern keeps its framing. It loses *today* on the same
  content price as A — three worlds re-framed, a golden re-bless — bought for a guarantee that is
  currently hypothetical.
- **C — re-fit per frame against the current rotation.** Rejected on the look, not the cost: the
  scale would then change with `θ`, so a rotating figure would pulse in size — a zoom pump driven by
  the spin. It is also the exact mistake ADR-0075 Alternative C already refused for the levers, one
  input over.
- **D — clamp or fade the figure at the frame edge.** Rejected as the wrong layer. It hides the
  symptom by cropping deliberately, and this project already has a per-preset edge-treatment
  decision for a *screen-space* stage ([ADR-0061](0061-kaleidoscope-edge-treatment-is-a-per-preset-choice.md));
  a scene's own framing is not that.
- **E — leave the invariant standing and treat the dragon as a one-off.** Rejected because the
  arithmetic says it is not a one-off: every figure but the fern is affected, and two of the three
  shipped worlds are already paying for it without knowing.
