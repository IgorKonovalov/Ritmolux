# 0124 — The line stroke carries a solid core and a pixel-wide edge, and its softness is authorable

> **Status:** proposed
> **Date:** 2026-08-25
> **Related plan(s):** [0114](../plans/0114-the-line-stroke-reads-as-a-drawn-line.md)
> **Supplements:** [ADR-0056](0056-additive-scenes-emit-premultiplied-alpha.md) (the alpha model this
> keeps), [ADR-0098](0098-the-line-renderer-draws-arcs-as-per-pixel-distance-fields.md) (the arc
> primitive, whose fragment carries the same profile by construction)
> **Relates to:** [ADR-0037](0037-internal-grid-is-a-resolution-not-a-shape.md) — the edge is
> specified in **pixels of the render target**, which is the same rule seen from the other side

## Context

Every line scene strokes through one fragment, and it has drawn the same profile since Plan 0010:

```wgsl
let falloff = max(0.0, 1.0 - d);   // d is the across-the-stroke coordinate, 0 at the centreline
let g = falloff * falloff;
return vec4<f32>(in.color * g * u.v.y, g * in.alpha);
```

The quadratic runs across the **whole** half-width. There is no plateau and no edge: brightness
begins falling at the centreline and reaches zero only at the quad boundary. Measured on a ~14 px
stroke at Plan 0087 Phase 4, on a preset binding no bloom, no trails and no `glow`:

| quantity | reading |
|---|---|
| cross-section | `28 45 68 91 113 134 156 177 198 215 225 223 211 192 170 149 128 106 83 60 40` |
| within 10 % of peak | **4 px** |
| above half peak | 13 px |

A four-pixel spine inside a ten-pixel gradient. **The user's verdict, given twice in the running
app at both takes of Plan 0087's look gate, was that the stroke reads *blurred and
semi-transparent*** — alongside the positive half of that gate, that the arc-drawn circles read as
drawn curves. The first half closed Plan 0087's question. The second half is this decision.

**This is not a defect and nothing regressed.** It is [ADR-0056](0056-additive-scenes-emit-premultiplied-alpha.md)'s
premultiplied emission working exactly as specified: colour and alpha carry the same coverage, so
the quad's long edges write nothing rather than opaque black. The profile that produces the
blur is what makes the seam correct. Plan 0087 Phase 1's done-when *required* the new arc primitive
to reproduce it — "a drawing of the same curve rather than a different look" — and it does, at mean
0.0000. So the reading is identical either side of that plan, and the arc primitive neither caused
this nor could have fixed it.

**Three levers were checked and none of them reaches it.**

- **`glow`** multiplies `u.v.y` into the *colour* and not into `g`. It scales the light, deliberately
  ([`gpu::ADDITIVE_LIGHT_SATURATING_COVERAGE`]), so a dimmed stroke still covers its footprint.
  It cannot narrow a gradient. (`presets/README.md` calls it "the line renderer's per-segment
  **falloff** multiplier", which is wrong and is repaired by the plan.)
- **`thickness`** scales `w`. A thinner stroke is a smaller blur, not a sharper one — the ratio of
  spine to gradient is scale-invariant, which is why the profile survived four years of tuning
  without anyone being able to tune it out.
- **The bloom stage** adds halo; it cannot remove one. The measurement above was taken with bloom
  unbound.

**What the engine already knows how to do.** `palette.rs:744` computes a screen-constant contour
width from `fwidth`, for exactly this reason: a width specified in pixels of the render target is
resolution-independent without a uniform carrying the resolution. The precedent is in the tree.

## Decision

**The line fragment gains a plateau and a pixel-wide edge, and the width of the plateau is an
authorable parameter.**

Let `u = 1 - d/w`, running 0 at the stroke edge to 1 at the centreline. The fragment becomes

```wgsl
// illustrative, not the final source
let core = clamp(u / max(softness, edge), 0.0, 1.0);
let g = core * core;
```

where `edge` is derived from `fwidth` so the transition is **about one pixel of the render target**
whatever the stroke width, the resolution, or the aspect. `softness` is a new line-scene parameter
on `[0, 1]`:

- **`softness = 1.0` reproduces today's fragment exactly** — `core = u`, `g = u²` — so the parameter
  ships byte-identical before any default moves.
- **`softness = 0.5`** makes the inner half of the stroke solid and ramps across the outer half.
- **`softness → 0`** is a solid stroke with a one-pixel antialiased edge, floored by `edge` so it
  never divides by zero and never aliases.

**The default is chosen by a look gate against rendered samples, not by this ADR**, and the library
is retuned to whatever it picks. That is deliberate and it is the lesson of Plan 0087: this profile
is a claim about what an eye does, no test in this repo settles it, and the one instrument that
found the problem was a human looking at the running app. Writing a number here would be inventing
the answer the gate exists to produce.

**[ADR-0056](0056-additive-scenes-emit-premultiplied-alpha.md) is untouched.** Colour and alpha still
carry the same coverage `g`; only the shape of `g` changes. The emission model, the blend state and
the OVER range are exactly as they were.

## Consequences

### Positive

- **The stroke reads as a drawn line.** The defect the user named is a property of the profile, and
  this is the only place the profile lives.
- **One fragment change reaches all four line families** — `parametric_curve`, `lsystem`,
  `star_pattern` and `spectrum` all stroke through `LineRenderer` — **and the arc primitive comes
  along for free.** ADR-0098's arc fragment applies the same falloff to its own distance, so it
  inherits the profile by construction and Plan 0087's arc-equals-polyline equivalence test keeps
  holding at every `softness`. That test becomes the guard on this change rather than a casualty of
  it.
- **The edge is specified in pixels of the render target**, so it is one pixel at 1280x800 and at
  4K, and on a non-16:9 target. `fwidth` reads the actual rasterization, so no uniform grows and
  nothing can source the wrong size — the ADR-0037 failure mode is unreachable here rather than
  merely avoided.
- **`softness = 1.0` is an escape hatch, not dead surface.** A preset that wants the luminous smear
  keeps it by binding one number, so the register is not lost, only stopped being compulsory.
- **`Uniforms.v.zw` is already unused**, so the parameter costs no uniform growth and no new bind
  group — nothing for [ADR-0058](0058-bind-group-layout-collisions-carry-evidence.md) to enumerate.

### Negative

- **Every line golden baseline moves, once, when the default flips.** That is the real price. It is
  paid deliberately at one phase with a bless-to-bless comparison against a control, and adapters
  compared first — this repo has blessed rasterizer garbage before.
- **Five shipped presets were tuned against the old stroke** — `curve_nightbloom`, `curve_ionwake`,
  `lsystem_vellum`, `star_rosewindow`, and `fragment_vitrail`'s line layer — and need a
  `preset-author` sitting. A crisper stroke reads *brighter* at the same `brightness`, so this is a
  real retune and not a mechanical edit.
- **`fwidth` is a derivative, so it is undefined across a primitive edge** and quantized to the 2x2
  quad. On a stroke thinner than about two pixels the edge term stops being meaningful and the
  result is a dimmer line rather than a sharper one. That interacts with the `thickness` dead zone
  Plan 0087 Phase 1b just made warnable, and the floor there is now the right place to state it.
- **It adds a parameter to a surface that already has `thickness`, `brightness` and `glow`**, and
  the four interact. The plan owes the authoring docs a sentence on which one to reach for.

### Neutral

- `spectrum`'s bars and `lsystem`'s straight stems get the same treatment. That is wanted — the
  complaint was never specific to curves — but it means the change is engine-wide across the line
  families rather than scoped to the ornament work that surfaced it.
- The name `softness` is chosen over `feather` and `hardness` because it reads correctly at both
  ends for an author (`0` sharp, `1` soft) and does not invert the sense of the existing default.

## Alternatives considered

### Alternative A — Leave the stroke and let the bloom stage carry the halo

Rejected on a measurement. The reading above was taken on a preset binding **no bloom, no trails and
no `glow`**, so the blur is in the stroke itself and is present for every preset whether or not it
binds a post stage. Bloom can add halo; nothing downstream can remove one that is already in the
coverage.

### Alternative B — Expose the exponent, `g = u^k`

The obvious one-line change, and it does not work. Raising `k` narrows the *bright core* but the
gradient still reaches zero only at the quad boundary, because `u^k` is zero exactly where `u` is.
It moves the spine and leaves the smear — which is the same failure `thickness` has, one derivative
up. No `k` produces an edge.

### Alternative C — A screen-space sharpen in the post chain

Rejected on layering. The composite at that point contains every scene and every post stage; a
filter there would sharpen the attractor's trails and the reaction-diffusion field along with the
strokes, and it would have no way to know what was meant to be soft. The profile is a property of
the stroke and belongs in the fragment that draws it.

### Alternative D — Ship it opt-in, with today's profile as the permanent default

The safe version: no baseline moves, no retune, new presets opt in. **Rejected by the user on
2026-08-25**, and the reasoning is worth keeping — an opt-in leaves the shipped library looking
exactly the way the complaint describes, which is not a fix but a place to file one. The default is
the deliverable.

### Alternative E — Fix the default here, in this ADR

Rejected on the Plan 0087 precedent. The profile is a look, the look gate is the instrument that
found the problem, and a number written here would be a guess dressed as a decision. The gate picks
it; this ADR fixes the *mechanism* and the fact that a default exists.

## Notes

`fwidth` exists only in a fragment shader. `palette.rs:776` already records a copy landing outside
one as a compile error, and the same applies here.
