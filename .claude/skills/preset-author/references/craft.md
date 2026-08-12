# Craft — what makes a preset beautiful

Reactivity is the easy part; anyone can wire `bass` to a size. **Beauty is composition** — a look
that reads as one coherent thing, moves musically, and stays alive whether the track is loud,
quiet, or between beats. None of it is enforced by the engine; it's judgment, verified by rendering
(`render-loop.md`).

## First: the additive ceiling — spend peak energy on structure, not luminance

**Every scene draws additively.** Particles, line segments, field samples and the feedback buffer all
*add* light into the frame. Luminance terms therefore stack: `brightness` + `glow`/`flash` + stroke
`thickness` + whatever `trails` has accumulated + `bg_bright`. Push past the point where the picture
can absorb them and it stops getting brighter and starts getting **flatter** — the structure that made
the look washes out at exactly the moment the music is most exciting.

> **This rule changed shape at Plan 0045, and it is worth knowing which half moved.** The composite
> used to clip per channel at 1.0: past the ceiling the frame went *flat white*, and colour died with
> it, because every channel pinned at 1.0 is white whatever the palette said. It is now
> floating-point linear light with an engine tonemap at the end, which **rolls off** instead —
> identity below ~0.6, compressive above, and hue-preserving, because all three channels are scaled
> by one factor taken from the brightest. So a hot core keeps its colour and stays readable inside,
> and no amount of stacked light reaches pure white.
>
> What that means for authoring: **overshooting is now a soft loss of contrast rather than a cliff.**
> The habit below is still the better-looking choice and still what the shipped library does — a peak
> that is 20 % brighter than the rest of the frame reads as intense; one that is 300 % brighter reads
> as a flash whatever the curve does with it — but it is a taste rule now, not a rescue. Two things
> genuinely got easier: `glow` above 1 does something (the core carries the energy rather than only
> widening a skirt), and over-range light is what `bloom_amount` finds, so a peak you used to have to
> suppress is now something you can *spend*.

This is the failure mode that broke most of an earlier version of this library, and it hides well:
every binding looks reasonable in isolation and the quiet frame looks lovely. `brightness = "0.8 +
clamp((bass + treb) * 6, 0, 1.1)"` reads like an ordinary gain-then-bound and used to render a white
rectangle; today it renders a washed-out one. The gain is not the problem — the *destination* is.

**The habit that fixes it: hold luminance nearly flat, and spend the peak on structure.** A base
around `0.7–0.9` plus a small clamped reactive term (a ceiling of ~`0.25`, not ~`1.0`) is plenty for
a hit to register. Everything else the music has to say should move shape, motion, size, colour and
the composite: `warp`, `zoom`, `size`, `spin`, `phase`, `hue_spread`/`color_span`, `trails`,
`kaleido_order`. A beat that raises `glow` reads as a camera flash washing the frame out; the same
beat on `warp` reads as the field itself being struck. The second one is what you actually wanted,
and it survives being added to whatever preset dissolves into yours.

**The mirror failure: over-driven motion evacuates the frame.** The swarm shows it most clearly —
`force = "1.1 + clamp(bass * 16, 0, 3)"` peaks near `4` against a scene default of `1.4`, hard enough
to fling every particle to the frame edge and leave the middle black (`DEFAULT_FORCE` really is `1.4`
— check a scene's defaults before you decide what "hard" means). Loud rendered as *less*. Keep swarm
`force` inside roughly `0.7–2.4` and `burst` inside a couple of units, and treat every param that can
push the picture out of the frame — `force`, `burst`, `scale`, `zoom`, the attractor's `a`..`d`
coefficients — with the same suspicion.

**The tell is an inversion, and it's cheap to check.** Render the loud frame and the quiet frame and
compare them: if the loud one carries *less* legible structure than the quiet one, you are over the
ceiling in one direction or the other. Across a whole set the same check is the loud/quiet
contact-sheet pair (`render-loop.md`) — the fastest audit in this lane. `--report` puts a number on
it: `cover` near zero at the loud frame means the peak has no structure against its own background,
whether it blew out, died, or emptied.

## Layer motion across the time-scales

A preset that only responds to `bass` pumps like a subwoofer meter. Beautiful presets layer motion
so something is evolving at every rhythm:

- **Slow evolution (`time`).** A gentle unending drift so the look never sits still even in
  silence — almost always on `hue` (`+ time * 0.02..0.06`), sometimes rotation or `kaleido_angle`.
- **Per-beat breathing (`bar`).** `bar` ramps `0→1` between beats: `zoom = "1.0 + bar * 0.25"`, or
  `draw_progress` riding `bar` to redraw a figure each beat. Musical pulse, not spasm.
- **Beat accents (`beat`).** The `0/1` gate for a discrete snap — a size bump, a `burst`, a
  `variant` swap, an edge-triggered `reseed`/`inject`.
- **Transient flares (`onset`).** Sharper and more frequent than `beat` — flashes and stabs that
  track every hit's attack.
- **Section-scale change (`tempo`, `novelty`).** A `select(tempo > 128, …, …)` gives one file two
  personalities; `novelty` spikes at a track/section boundary (experimental — never the only
  mechanism).

Aim for at least: a `time` drift on colour + one beat-locked motion + one continuous band response.
A look using three or four of these feels *composed*; one feels like a meter.

## Reactivity that reads musically

- **Gain-then-bound, always.** Pick the gain so ordinary material reaches the *middle* of the range
  and only peaks hit the top; pick the ceiling so a peak looks intense, not broken. On a luminance
  param "not broken" means a *small* ceiling — see the additive ceiling above.
- **Keep a floor.** `base + reactive`, never bare reactive: `glow = "0.4 + clamp(...)"` stays alive
  in a quiet passage; `glow = "clamp(...)"` flickers to black and reads as dead.
- **Match the driver to the band.** Bass for weight/force/size; treble for colour/shimmer/detail;
  mid for the middle (flow speed, spin); `onset`/`beat` for punctuation. Cross-wiring reads as noise.
- **Ease what shouldn't snap.** `[smoothing]` is the right tool for a jittery band-driven param
  (`zoom = 0.12` gives a punchy-but-smooth pump; `hue = 0.4` a fluid drift). Don't fake easing with
  arithmetic — and don't smooth an accent you *want* sharp (`flash`, `burst`, `reseed`).
- **Don't over-react.** If everything jumps on every beat, nothing stands out. Move one or two
  things hard and let the rest drift. Restraint separates a designed look from a light-organ.

## Colour

The four shader-coloured scenes sample a shared **palette LUT**; the line scenes use their own
cosine `hue`. So colour is a two-part decision: *which gradient*, and *where in it you sit*.

- **Pick the gradient first.** A built-in (`ember`, `ice`, `mono`, `aurora`, or the default
  full-hue `spectrum`) or custom `stops` for an exact mood. A tight custom gradient is usually more
  elegant than the full wheel.
- **Then narrow the window.** `color_span` (fragment/RD) and `hue_spread` (swarm/attractor) are the
  cohesion knobs: **low = one colour family**, high = a rainbow. This is the single biggest lever
  between "designed" and "novelty screensaver". Watch the RD exception — its field only reaches
  ~`0..0.4`, so a full custom gradient there needs `color_span` around `2.0–2.5`.
- **Move the centre, not the whole wheel.** `color_center` / `hue_center` sliding gently on treble
  keeps colour alive without losing the base tone.
- **Crossfade for section change.** `[palette_b]` + `palette_mix` bound to `bar`, a `tempo`
  comparison or `novelty` gives a colour *shift* rather than a colour *strobe*.
- **A slow `time` hue drift** walks the palette across a set — gorgeous for long shows and what
  keeps rotation-static scenes alive.

## The composite is part of the look

These are engine-wide and bindable, so treat them as instruments, not decoration:

- **`bg_*`** — a tinted, vignetted backdrop turns the sparse scenes (lines, swarm, attractor) and
  RD's voids from "shapes on black" into an atmosphere. No effect behind `fragment_field`. Since
  Plan 0080 it also paints a **directional ramp** — a segment of your `[palette]` swept along one
  axis (`bg_angle`, `bg_hue_span`), with its own brightness ramp (`bg_shade`/`bg_shade_end`) and
  easing (`bg_ramp_gamma`) — so a horizon, a ground, a lit sky is a *backdrop* now and does not
  spend the scene slot or the one `[layer]`. Roster and the worked dusk example: `systems.md`'s
  engine-stage table and `presets/README.md`. **It earns you nothing at `sanity` or `animation`**
  (both are blind to `bg_*`), so the figure still has to carry both floors.
  **A long dim tail is safe to author.** Since Plan 0082 the display write dithers by one encoded
  level (ADR-0096, always on, no param), so a wide smooth ramp no longer bands. This is worth
  knowing because the risky end is the counter-intuitive one: a band is a run of pixels sharing one
  8-bit value, so it is widest where the ramp is **flattest**, and sRGB's near-black slope made the
  *dim tail* the flattest part of any ramp reaching toward black — a low `bg_ramp_gamma` was the
  dangerous setting, not a high one. Measured, the dusk ground held one value for 58 pixels at
  1080p before the dither and 20 after. So reach for the low exponent, the near-black sky, the
  horizon glow above a dark ground, and do not add stops to break up a step you can see — that
  workaround is retired.
  **And since Plan 0081 it paints a *band* over that ground** — one soft gaussian swell whose
  centreline bows, for a Milky Way arc standing over a horizon (`bg_band_amount` to switch it on,
  then `bg_band_angle`/`bg_band_pos`/`bg_band_width`/`bg_band_curve` for the shape and
  `bg_band_hue`/`bg_band_hue_span` for its own segment of the same palette). Four things to know
  before reaching for it: `bg_band_width` is a **`1/e` half-width**, so the visible band is several
  times wider than the number; `bg_band_amount > 0` lights the pass **on its own**, so the
  near-black sky this wants needs no `bg_bright` at all; the band is additive *over* the ground and
  *under* the scene, so an opaque scene hides it exactly as it hides the ramp; and it shares your
  `[palette]` with the ground **and** the figure **and** the `[layer]`, which is the one real
  constraint — a palette fully spent on a horizon has no stops left for an arc. It earns nothing at
  either gate either, and a more capable sky makes that easier to forget.
- **`trails`** — needs real motion to read; it turns a spinning curve into a light-painting. High
  values plus a bright scene wash out fast (the accumulation stacks luminance the way everything else
  does — see the ceiling note above, which the tonemap softened but did not remove). It also
  restarts from empty on a preset switch, so a look that *is* the accumulation takes a second to
  arrive after a dissolve — judge it a few beats in, not on the opening frames.
- **`kaleido_*`** — instant symmetry on any scene; ride `kaleido_angle` on `time` so the fold turns
  rather than sits.
- **`bloom_*`** (Plan 0045) — the reason to *have* an over-range peak instead of suppressing it.
  `bloom_amount = 0` is off and free; the default `bloom_threshold = 1.0` means it finds exactly the
  light the display could not have shown, so switching it on halos the hot spots and leaves the rest
  alone. `bloom_radius` spreads the same energy wider without adding any, which makes it the better
  build-up parameter of the two — a rising radius opens the frame up where a rising amount just
  brightens it. This is the stage a beat belongs on if you want a hit to feel like light rather than
  like a gain change.
  **The trap, and it will get you first: a preset written to the old keep-it-under-1.0 habit gets
  *nothing* from this stage.** The default threshold selects light that is genuinely *over* range,
  so a frame that never crosses 1.0 hands the bright-pass an empty picture and the stage does
  literally nothing — measured, a draft holding `brightness` under 1.0 rendered **pixel-identical**
  with bloom on and at `bloom_amount = 0`. Something must deliberately cross 1.0, and the cheapest
  fuel is **`glow`**, because it drives the stroke's core rather than its width (raising `thickness`
  spreads the same light over a bigger quad and can move the peak the wrong way). `presets/star_lantern.toml`
  is the shipped worked example and its header records what the renders taught.
  **And verify it on a moving stimulus, never on a `--set` still.** A held `--set bass=1` flatters
  every stage, but the threshold makes this one a cliff rather than a slope: at `bass = 1` the
  figure sits far over range and the halo is enormous, while on real material — bass *mean* around
  0.007 against peaks near 0.19 — the frame may never cross the threshold at all. Use
  `--signal dynamic:<bpm>` or `--audio`, and read a `--set` still as the loudest single frame the
  preset will ever have.
- **`exposure`** (Plan 0045) — one linear multiplier on the whole frame before the tonemap. The honest
  way to make a finished preset brighter or darker without re-balancing every element against its own
  background. Binding it to audio pumps the entire picture, which reads as the *camera* reacting; that
  is occasionally what you want and usually not.
- **`mirror_*`** (line scenes) — folds the *geometry*, so it builds true fractal structure rather
  than a pixel mirror. Costs segments: high order on a dense curve hits the cap.
- **`ink_amount`** — the only route to a *dark-on-light* look, because the scenes draw additively.
  `"1"` alone is black-on-white; `paper_*`/`ink_*` make any duotone. It collapses colour to two
  tones, so in ink mode use the palette and `saturation` to sculpt **contrast**, not hue. Rest at
  `0` or `1` — a partial amount greys the paper. There is one ink pass for the whole blended frame, so
  its params **crossfade with a preset switch**: an ink preset landing after a glowing one travels
  through that greyed state for about a second (fine — it's a transition), and two ink presets sitting
  next to each other in the `presets/` filename order will walk between their two duotones, so pick
  neighbouring poles that look intentional on the way across.

## Make it survive a real track, not just the loud frame

Before calling a preset done, look at all three:

- **The loud frame** (`--set bass=1,...`) — is the peak intense or broken?
- **A quiet frame** (`--set bass=0.1,mid=0.1,treb=0.05`) — does it still look intentional, or
  collapse? The base terms carry it.
- **The two of them together** — more structure loud than quiet, or less? Less is the inversion, and
  it means the ceiling, not the taste.
- **A filmstrip** (`--signal click:120`) — does the motion read musically, or strobe? Is the beat
  response legible?
- **`--report`** — does `cover` sit somewhere sane, does every band you bound show a non-zero column,
  is `anim` alive, and is it flagged `NEAR-DUP` against something already shipped?

Beautiful at peak **and** alive at rest **and** musical in motion is the bar, and the report is how
you tell that in numbers instead of hope.

## House conventions

- Start the file with a `#` comment: what the scene is and what drives what ("bass swells the warp,
  treble drifts the hue"). Every shipped preset does this.
- `name = "…"` something evocative — it shows in the title bar, the browser and the contact sheet.
- Keep discrete params integer-clean with `floor` (`n`, `samples`, `variant`, `visible_depth`,
  `mirror_order`, `kaleido_order`).
- Group `[params]` by intent (shape, colour, composite) even though load order doesn't matter.
