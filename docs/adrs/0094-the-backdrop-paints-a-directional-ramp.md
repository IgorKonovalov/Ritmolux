# ADR-0094 — The backdrop paints a directional ramp through the preset's palette

> **Status:** proposed
> **Date:** 2026-08-12
> **Related plan(s):** [0080-the-sky-gets-a-horizon](../plans/0080-the-sky-gets-a-horizon.md)
> **Supplements:** [ADR-0018](0018-engine-wide-scene-compositing.md) (the background pre-pass),
> [ADR-0086](0086-the-backdrop-colours-through-the-preset-palette.md) (the backdrop samples the
> preset's LUT), [ADR-0090](0090-a-preset-composes-two-scene-layers.md) (the one-`[layer]` budget)

## Context

The engine has no static, screen-anchored, oriented gradient. A photoreal dusk — a bright orange
horizon band whose light fades smoothly upward through deep blue to near-black over roughly the
lower third of the frame — was attempted in the content lane against a user-supplied reference
photo, and the best approximation is a hard-edged slab. The user's verdict on it is
"unacceptable". This is a demonstrated want with a rejected workaround, not speculation.

**Three surfaces were tried and each fails for a structural reason, not a tuning one.**

- **`fragment_field` cannot be it.** Its field is `0.5 + 0.5 * sin(p.x + p.y + t * 0.5)`
  (`fragment_field.rs:142`): both axes are weighted equally by construction, so its bands are
  diagonal and there is no orientation lever; and `t` enters the field phase directly, so even
  `warp = 0` cannot hold a frame still. There is no time-scale or freeze param on the scene.
- **The backdrop pre-pass cannot be it *today*.** It takes **one** palette sample at `bg_hue` and
  multiplies it by a *fixed* vertical brightness tilt, `mix(0.72, 1.0, 0.5 + 0.5 * ndc.y)`
  (`background.rs:124`), plus a radial vignette. The tilt is hardcoded, is 28 % of brightness
  rather than a colour ramp, and points **up** — brighter at the top, which is the wrong way round
  for a horizon.
- **A line-scene fake cannot finish it.** A `spectrum` slab at `scale = 0` gives a static
  screen-anchored band, and that is what the demo ships — but `glow` is a local stroke halo and
  the bloom pyramid's spatial radius is bounded (`bloom_radius = 3.0` measured, barely visible).
  No stage downstream can turn a hard edge into a quarter-frame fade.

**The layer budget is the second half of the problem.** A look of this class needs three roles —
ground gradient, reactive figure, star layer — and ADR-0090 caps composition at a main scene plus
one `[layer]`. The dusk look already spends its layer on the star swarm, so the ground has to come
from the main scene or from the backdrop; it cannot come from a third scene slot.

**What reframes the decision is that the palette is already the ramp.** The demo's own
`[palette]` runs `#060b24 → #1b2a5e → #c74b1d → #ff7a1f → #ffd06e` — near-black through deep blue
to hot amber. That *is* the dusk gradient, already authored, already baked into the LUT the
backdrop has sampled since ADR-0086. The backdrop takes one point on it. Sweeping the coordinate
along a screen axis instead makes the smooth orange→blue→black fade fall out of the palette's own
stops, and each stop's `at` position becomes the horizon's vertical placement — so the smallest
available change is also the most expressive one, and it adds no second colour language.

## Decision

**The background pre-pass gains one ramp axis, and paints a segment of the preset's palette along
it instead of a single sample.** Five new bindable `bg_*` params, all defaulting to today's
behaviour:

| param | default | meaning |
|-------|---------|---------|
| `bg_angle` | `0.0` | the ramp direction, in **radians**, `0` = bottom-to-top (the axis the fixed tilt already used; the zero-is-up convention `launch_angle` set) |
| `bg_hue_span` | `0.0` | palette-coordinate **travel** from the ramp's start to its end. `bg_hue` keeps its ADR-0086 meaning as the coordinate at the start |
| `bg_shade` | `0.72` | brightness factor at the ramp's start |
| `bg_shade_end` | `1.0` | brightness factor at its end |
| `bg_ramp_gamma` | `1.0` | the ramp's **response exponent** — eases *where* things sit along the axis, applied before both channels |

The shader becomes, in outline (illustrative):

```wgsl
let d = vec2<f32>(sin(bg_angle), cos(bg_angle));          // 0 rad = up
let q = vec2<f32>(in.ndc.x * aspect, in.ndc.y);           // pixel-proportional
let s = clamp(0.5 + 0.5 * dot(q, d) / (aspect * abs(d.x) + abs(d.y)), 0.0, 1.0);
let e = select(pow(s, bg_ramp_gamma), s, bg_ramp_gamma == 1.0);   // ADR-0092's form
let u = bg_hue + bg_hue_span * e;                         // a SEGMENT, not a point
let tint = textureSample(lut, samp, vec2<f32>(u, 0.5));   // repeat-addressed, as today
let col  = tint * bg_bright * mix(bg_shade, bg_shade_end, e) * vig;
```

**The ramp is linear in screen space, and light is not** — so one exponent shapes the falloff.
`bg_ramp_gamma > 1` holds the ramp near its start before falling away (a hot band at the horizon,
then a long fade); `< 1` drops fast and leaves a dim tail. It applies to the axis position `e`
rather than to either channel, so colour and brightness stay locked as **one** ramp — the same
reason the fixed tilt retires into the ramp rather than sitting beside it.

**It is not redundant with stop placement, and the reason is load-bearing.** A `[palette]`'s `at`
positions can already shape the colour response arbitrarily — but that palette is **shared with
the scene** (ADR-0086, and with both layers under ADR-0090), so re-authoring stops to shape the
sky's falloff re-maps the figure's colours too. This exponent shapes the backdrop's *mapping* onto
the axis and touches nothing else. It is also the only shape control the **brightness** ramp has
at all: `mix(bg_shade, bg_shade_end, ·)` is a straight line, and no stop can bend it.

**The `g == 1.0` branch is a correctness requirement, not an optimization**, and it is the form
`ink.rs:135` shipped at Plan 0078: `pow(x, 1.0)` compiles to `exp2(1.0 * log2(x))` and is not
bit-exact, so without the `select` the *default* would perturb every backdrop-binding preset and
every future golden by a rounding step. The exponent is clamped positive on the CPU
(`ink.rs::applied_gamma`'s arrangement — non-finite falls back to `1.0`, finite is held in a
positive range) so `1.0` reaches the uniform exactly and `pow` never sees an undefined `0^0`.

**The fixed `0.72 → 1.0` tilt retires *into* this ramp rather than sitting beside it.** There is
one brightness ramp on the frame, on the same axis as the colour ramp, and its defaults are the
two constants the tilt already used. That is the whole answer to "two ramps that can point
different ways": there are not two.

**The defaults are an arithmetic identity, not a close match.** At `bg_angle = 0`, `d = (0, 1)`
exactly, so `dot(q, d) = ndc.y` and the denominator is `aspect * 0 + 1 = 1` — the aspect term
cancels and `s ≡ 0.5 + 0.5 * ndc.y`, today's expression. At `bg_ramp_gamma = 1.0` the `select`
takes `e ≡ s`. `u = bg_hue + 0.0 * e ≡ bg_hue`. The shade `mix` is the identical instruction with
the identical constants. So every preset that does not opt in renders byte-identically, and the
plan proves it by a bless-to-bless control rather than claiming it.

**The backdrop stays opaque, and no alpha is added.** It owns the frame clear and writes through
`BlendState::REPLACE`, so nothing renders beneath it for an alpha to reveal. The want that
normally motivates a transparency ramp — a soft edge — is answered structurally instead: the
ground and the sky are one continuous sweep of one ramp, so there is no object boundary anywhere
in the frame to soften. Fading to the palette's dark end *is* fading to nothing.

**The swept coordinate is not clamped** — it inherits the repeat addressing every other palette
coordinate in this engine uses. This is settled by evidence rather than by consistency: two
shipped presets already depend on the wrap (`bg_hue = "0.62 + time * 0.015"` grows without bound,
and `bg_hue = "0.02 + sin(time * 0.013) * 0.04"` dips to `-0.02`), so clamping would break
working content. The authoring consequence — a segment leaving `[0, 1]` wraps, and can land the
palette's hot end at *both* ends of the sky — is documented, in the shape backlog 0062 already
uses for `depth_hue`.

**The aspect comes from the destination surface.** `composite_into` hands the backdrop
`destination`, sized `surface`; the `target.size` sitting on the next line is the chain's
quantized, capped internal grid (ADR-0034). Taking the second would be
[ADR-0037](0037-internal-grid-is-a-resolution-not-a-shape.md) for the third time, and it is
invisible at `bg_angle = 0` where the term cancels — so the plan carries a negative control at a
non-zero angle.

## Consequences

### Positive

- **The dusk ground becomes authorable, and the three-role budget is answered without widening
  composition.** Backdrop = ground, main scene = figure, `[layer]` = stars. ADR-0090's cap stands
  untouched.
- **No new colour language.** The ramp is a segment of the `[palette]` that already exists, so
  `saturation`, `palette_mix` and the A/B crossfade reach it for free, exactly as ADR-0086 fanned
  them out.
- **Vertical placement is authored where positions already live** — each stop's `at`. There is no
  second placement mechanism that can disagree with the palette.
- **The fixed tilt becomes a choice.** A backdrop can now be brighter at the bottom, which it
  never could be, and the 0.72 constant stops being invisible.
- **Every ramp param is bindable**, so a breathing horizon costs nothing extra — the `bg_*` route
  already carries expressions.

### Negative

- **The backdrop is invisible to every behavioral gate, and this makes that matter more.**
  Coverage measures the scene, not the backdrop (ADR-0067), and the animation gate strips `bg_*`
  bindings (ADR-0091's Outcome). So a world whose *ground* is the backdrop gets no credit for it:
  the whole gate burden falls on the figure and the star layer. This is correct — it is why a more
  capable backdrop cannot be used to game the gates — but it means a frame that looks full can
  still read as sparse to `sanity`, which is backlog 0072's shape.
- **`bg_bright` near `1.0` is untested territory.** Shipped presets sit at `≤ 0.039` — a dim wash.
  A bright plate interacts with the tonemap knee, with `occlude` (ADR-0085, whose retune is Plan
  0071 Phase 5, still standing), and with the additive families' behaviour over a lit backdrop, and
  none of that has been judged at this level.
- **The fragment field still draws opaquely over the backdrop**, so a dusk ground is unusable
  under that one system. Worse, this raises the odds of the exact configuration `background.rs`'s
  ADR-0058 note says to re-measure ("if a fragment-field preset ever binds `bg_bright`"), because
  a bright backdrop is now worth binding.
- **Five more names on a global namespace.** `bg_*` is unioned into every preset's typo check
  (ADR-0020), so this widens the vocabulary every author sees whether or not they want a ramp.
- **The uniform grows** from 32 to 48 bytes — one new `vec4` for `bg_angle`, `bg_hue_span`,
  `bg_shade`, `bg_shade_end`, with `aspect` and `bg_ramp_gamma` taking two of the three words
  already sitting unused in `v.w` and `c.z`. That changes the pass's `min_binding_size`, which is
  a Plan 0053 fix against a measured WARP mis-render — it further separates the layout rather than
  colliding it, but the ADR-0058 enumeration has to be re-run rather than assumed.
- **A wide smooth ramp is where 8-bit banding shows.** A quarter-frame fade crossing most of the
  luminance range at 1080p spends roughly two pixels per output level, which is the classic
  Mach-band configuration. The chain is float and linear until the tonemap, so the quantization is
  at the final write only — but nothing here dithers, and whether it bands is a measurement this
  ADR does not have. Plan 0080 Phase 7 is where it gets looked at; a dither is a separate decision
  if it turns out to be owed.

### Neutral

- The vignette stays radial and independent of the ramp. Nothing here makes it directional.
- The ramp is a linear interpolation along one axis. Any non-linearity in the gradient comes from
  the palette's stop spacing, which is where it belongs.

## Alternatives considered

### Alternative A — a `gradient` ground scene (a tenth `SystemKind`)

A scene that draws an oriented ramp. Rejected on the **layer budget**: the dusk look needs ground
+ figure + stars, and a ground scene would occupy either the main slot or the one `[layer]`,
leaving the third role homeless. It is also strictly more machinery — a new system, its own
params, goldens, gate coverage and roster entry — to produce a picture the backdrop pass is
already positioned to paint, one draw earlier, with the palette already bound.

### Alternative B — an orientation + time-scale lever on `fragment_field`

Add an axis-weight and a clock-scale so the field can be made axis-aligned and frozen. Rejected
because the shader's two axes are coupled *inside* the warp loop and its clock is threaded through
the field phase, so this is a rework of a scene's core expression to serve one static case — and
the fragment field draws opaquely over the backdrop, so it would then be competing with the ground
role it was being asked to fill. The *moving* oriented field remains a legitimate separate want;
nothing here forecloses it.

### Alternative C — explicit ramp position and width params (`bg_ramp_center`, `bg_ramp_width`)

Place and size the ramp on the axis independently of the palette. Rejected because it splits
placement across two tables that can disagree: the `[palette]` stops already carry positions, and
an author who moves a stop would then have to move a param to match. Audio-driving the placement
is the one thing it buys, and `bg_hue`/`bg_hue_span` are bindable, so a breathing horizon is
reachable anyway by sliding the segment.

### Alternative D — clamp the swept coordinate to `[0, 1]`

Defensible on its face: a sweep across a whole frame is exactly where a wrap does the most damage,
putting the palette's hot end at both ends of the sky. Rejected on measurement — two shipped
presets drive `bg_hue` outside `[0, 1]` today and rely on the repeat, so a clamp is a behavioural
break for working content, and making one coordinate special contradicts every other LUT sampler
in the engine. Documented instead.

### Alternative E — keep the fixed `0.72 → 1.0` tilt and multiply an authorable ramp on top

The cheapest possible identity guarantee: the legacy tilt is untouched and the new ramp defaults
to a constant `1.0`. Rejected because the legacy tilt is welded to `+y` while the new ramp can
point anywhere, so any angled backdrop would carry a second, invisible vertical gradient that no
param explains. Retiring the tilt into the ramp costs nothing — its constants become the ramp's
defaults — and leaves one mechanism to reason about.

### Alternative F — spans for both channels, for symmetry (`bg_shade` + `bg_shade_span`)

`X` = value at the start, `X_span` = travel, applied to both colour and brightness. Rejected for a
narrow but real reason: `mix(bg_shade, bg_shade_end, s)` compiles to the *same instruction with the
same constants* as today's `mix(0.72, 1.0, s)`, so byte-identity at defaults is structural rather
than a hope about how a compiler folds `0.72 + 0.28 * s`. Colour keeps the span form because
`bg_hue` is a shipped name whose meaning ADR-0086 fixed, and a `bg_hue_end` could not default to
"whatever `bg_hue` is".

### Alternative G — a smoothstep (or a per-channel curve) instead of one exponent

Two shapes were weighed against the single positional exponent. A **smoothstep** `s²(3 - 2s)`
eases both ends symmetrically, which is a genuinely different curve from any power — but it takes
no parameter, so it is a look rather than a lever, and the flat-then-fall shape a horizon actually
wants is what `bg_ramp_gamma > 1` already produces. A **separate curve per channel** (one for
colour, one for brightness) was rejected for the same reason Alternative E rejects a second
brightness ramp: it lets the two halves of one ramp disagree, and the incoherence is harder to see
in a curve than in a direction. If a symmetric ease is ever wanted, it is one more `select` arm on
the same position, not a second mechanism.

### Alternative H — an alpha ramp on the backdrop

Fade the backdrop to *transparent* rather than to its palette's dark end. Rejected as inert: the
pass owns the frame clear and writes with `BlendState::REPLACE`, so there is nothing underneath
for alpha to reveal, and the swapchain ignores it. The distinct capability in this neighbourhood —
a **foreground** ramp, drawn after the scene and occluding the bottom of the figure as a haze — is
a different pass at a different point in the composite and is not what the dusk look needs (its
horizon sits *behind* the stars). Out of scope here, and unblocked by nothing this ADR decides.

## Notes

- The look that motivated this, its rejected workaround (`quiet_sky_demo.toml`, a `spectrum` slab
  at `scale = 0`) and the `sun_v8..v10` renders came from a content session riding Plan 0077's new
  swarm params. The reference photo is the user's.
- Raised as [design-backlog 0091](../design-backlog.md) and promoted the same day.
- The neighbouring standing content work is **Plan 0077 Phase 5** (Perseids' quiet sky, content
  lane, outstanding by design). That look and this ground are the same family; the ground is what
  a dusk variant of it was missing.
