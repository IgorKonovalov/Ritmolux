# 0124 — The line stroke carries a solid core and a pixel-wide edge, and its softness is authorable

> **Status:** accepted 2026-08-26 (Plan 0114) — carries an `Outcome`
> **Date:** 2026-08-25
> **Related plan(s):** [0114](../plans/0114-the-line-stroke-reads-as-a-drawn-line.md)
> **Supplements:** [ADR-0056](0056-additive-scenes-emit-premultiplied-alpha.md) (the alpha model this
> keeps), [ADR-0098](0098-the-line-renderer-draws-arcs-as-per-pixel-distance-fields.md) (the arc
> primitive, whose fragment carries the same profile by construction)
> **Relates to:** [ADR-0037](0037-internal-grid-is-a-resolution-not-a-shape.md) — the edge is
> specified in **pixels of the render target**, which is the same rule seen from the other side
> **Amended 2026-08-26, before acceptance:** this ADR said the fragment reaches *four* line
> families. It reaches **five consumers** — `warp_mesh` strokes through the identical fragment, and
> because all three entry points funnel into one `draw_all` writing one uniform, it cannot abstain.
> The amendment adds the second constant and its rationale; the mechanism above is unchanged.

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

**Two constants, because there are two judges.** `LineRenderer::draw`, `draw_split` and `draw_arcs`
all funnel into one private `draw_all` that writes **one** uniform, so every caller supplies a
`softness` and none can abstain. `warp_mesh` is a caller: it strokes the MilkDrop waveform, every
custom wave, every shape outline, both borders and the motion grid through this same fragment. It
does **not** take the line default. It passes its own named constant, pinned at `1.0` — the
pre-0114 profile — because it answers to a different instrument.

The four line families are judged by "does this read as a drawn line", against an eye. `warp_mesh`
is judged against **`foo_vis_milk2`**, which [ADR-0113](0113-milkdrop-presets-are-translated-ahead-of-time-onto-a-warp-mesh-idiom.md)
names as the fidelity reference and against which the conversion has already been judged
side-by-side. Its stroke width was chosen *through* this profile — `draw.rs:171` records a thick
MilkDrop line, drawn there as two or four offset passes, being reproduced here as one stroke of
twice the width, "the same gesture through this engine's soft-falloff primitive". A number picked by
Plan 0114's look gate is an answer to a question nobody asked of that surface.

**What the reference actually wants is open, and this ADR does not guess it.** Plan 0114 carries a
`human` phase that judges `warp_mesh`'s stroke against the reference rig and sets the constant from
that verdict. Until it runs, the pin holds the profile the conversion was judged under.

## Consequences

### Positive

- **The stroke reads as a drawn line.** The defect the user named is a property of the profile, and
  this is the only place the profile lives.
- **One fragment change reaches all four line families** — `parametric_curve`, `lsystem`,
  `star_pattern` and `spectrum` all stroke through `LineRenderer` — **and the arc primitive comes
  along for free.** (It reaches a fifth consumer, `warp_mesh`, which the Decision pins rather than
  moves; that is the price recorded below, not a benefit.) ADR-0098's arc fragment applies the same falloff to its own distance, so it
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
- **The engine carries two stroke profiles once the default moves**, and a reader of the fragment
  cannot see why without following the constant. That is the cost of the pin, and it is paid in
  naming: two named constants at the call sites, never a bare `1.0`.
- **Nothing in the golden corpus shades a `warp_mesh` stroke, so the pin is the only thing holding
  that surface.** All three `warp_mesh` baselines — `warp_mesh.png`, `warp_mesh_milk.png`,
  `warp_mesh_shader.png` — are warp field, deposit and shader output with **no line geometry in
  them**; the fixtures set no wave and no border. The line path is covered at the CPU-geometry level
  only (`every_wave_mode_builds_a_different_figure`,
  `borders_and_motion_vectors_each_draw_their_own_figure` assert which segments get *built*, not how
  they are *shaded*). So a change to this fragment can alter every MilkDrop stroke and no baseline
  in the repo moves. Plan 0114 closes the gap by adding one.
- **MilkDrop's own widths sit below the range the plan's arithmetic reasons about.** `draw.rs`'s
  `THIN = 0.0025` NDC-y is a **1.35 px** half-width at 1080p and **1.0 px** at 1280x800, against the
  1.5–3.2 px shipped range the line families span. `fwidth(d) ~ 1 / half-width-in-pixels` is
  therefore ~0.74 and ~1.0 there — at the small target it reaches the cap exactly. The cap is what
  makes the pin *exact* rather than approximate: with `edge` capped at 1.0, `max(1.0, edge)` is
  1.0, so `core = u` and `g = u²` term for term. Remove the cap and the pin silently stops being
  byte-identical on small targets.

### Neutral

- `spectrum`'s bars and `lsystem`'s straight stems get the same treatment. That is wanted — the
  complaint was never specific to curves — but it means the change is engine-wide across the line
  families rather than scoped to the ornament work that surfaced it. `warp_mesh` is the one
  consumer held out, and for a stated reason rather than by oversight.
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

### Alternative D0 — Let `warp_mesh` follow the new default like every other consumer

One profile engine-wide, nothing to explain, and no second constant. Rejected because it changes a
**fidelity variable against an external reference** with nothing watching: no gate in this repo
compares `warp_mesh` to `foo_vis_milk2`, and — see the Negative section — no golden baseline even
shades a `warp_mesh` stroke, so the regression would be invisible until someone next sat down with
the rig. It also lands MilkDrop's `THIN` stroke, at 1.0–1.35 px of half-width, in exactly the
sub-two-pixel regime this ADR's own Negative section says the edge term stops describing.

The asymmetry is the point: for the four line families, Plan 0114's look gate **is** the instrument
and a human has already returned the verdict that motivated the change. For `warp_mesh` the
instrument exists but has not been pointed at this question. Following the default would be
answering it by default.

### Alternative D — Ship it opt-in, with today's profile as the permanent default

The safe version: no baseline moves, no retune, new presets opt in. **Rejected by the user on
2026-08-25**, and the reasoning is worth keeping — an opt-in leaves the shipped library looking
exactly the way the complaint describes, which is not a fix but a place to file one. The default is
the deliverable.

### Alternative E — Fix the default here, in this ADR

Rejected on the Plan 0087 precedent. The profile is a look, the look gate is the instrument that
found the problem, and a number written here would be a guess dressed as a decision. The gate picks
it; this ADR fixes the *mechanism* and the fact that a default exists.

## Outcome — 2026-08-26, at Plan 0114's `dev`-arm close

Recorded rather than edited into the body above, per this project's rule that an accepted ADR is
append-only. **The decision stands in full and was built as written**; one supporting *reason* in the
Negative section is false, and one number the Decision reasons from is confirmed exactly.

**Falsified: why no golden baseline shaded a `warp_mesh` stroke.** The Negative section gives the
reason as the three fixtures "setting no wave and no border". They set no *border*; they all set a
wave, because `wave_a` defaults to `1.0` (`core/src/milk/outputs.rs:166`) and nothing in those
fixtures turns it off. Every `warp_mesh` fixture in the repo has always stroked a waveform.

**The real cause is the golden suite's 128 px capture, and the conclusion is unaffected — it is
stronger.** At that size `draw.rs`'s `THIN` is **0.16 px** of half-width and `THICK` is **0.38 px**,
both far under the one-pixel floor the profile's edge term is capped at, so the ramp is the whole
half-width and *every* value of `MILKDROP_SOFTNESS` draws the identical frame. Measured at the close
by driving the pin `1.0 -> 0.0` and reading the whole corpus: `warp_mesh.png`, `warp_mesh_milk.png`
and `warp_mesh_shader.png` move by **mean 0.0000 / outlier 0**, and so do `parametric_curve.png`,
`lsystem.png` and `star_pattern.png`. So the blindness is not a property of those three fixtures at
all — it is a property of the **capture size**, and it reaches the line families too.

Two things follow, and both are why this matters more than a corrected sentence:

- **A fixture that merely "sets a wave" would have added a fourth blind baseline.** Plan 0114 Phase
  9's `warp_mesh_stroke.toml` therefore strokes a **fat border** (`ob_size = 0.12`, 7.7 px of
  half-width at 128 px) as well as a wave, and `the_warp_mesh_stroke_fixture_shades_a_resolvable_stroke`
  guards the border's width in pixels so it cannot quietly degrade back. That baseline is verified
  to convict: the same `1.0 -> 0.0` sweep moves it to **mean 0.0336 / outlier 162** against
  tolerances of 0.02 / 48.
- **The line families' own coverage gap is now stated but not closed.** Only `spectrum.png` and
  `line_joint_zigzag.png` moved when the default flipped; the other line baselines cannot see this
  fragment. The pin is guarded; the *default* is guarded only incidentally.

**Confirmed exactly: the cap is what makes the pin byte-identical rather than approximate.** The
Negative section's `THIN` arithmetic — 1.35 px at 1080p, 1.0 px at 1280x800, `fwidth` reaching the cap
at the small target — is what
`the_edge_term_never_exceeds_the_softness_term` fixtures directly, on that real shipped geometry
rather than on a synthetic width alone.

**Alternative D0 was tested by the instrument it named, and held.** Plan 0114 Phase 8 put a spread of
`softness` beside `foo_vis_milk2` and returned **`1.0`** — keep the pin as it stands — which the plan
names as a legitimate outcome that closes the question rather than a null result. `MILKDROP_SOFTNESS`
is unchanged, and it is now a *judged* constant rather than a held-over default.


## Notes

`fwidth` exists only in a fragment shader. `palette.rs:776` already records a copy landing outside
one as a compile error, and the same applies here.
