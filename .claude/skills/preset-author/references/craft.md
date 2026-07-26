# Craft — what makes a preset beautiful

Reactivity is the easy part; anyone can wire `bass` to a size. **Beauty is composition** — a look
that reads as one coherent thing, moves musically, and stays alive whether the track is loud,
quiet, or between beats. None of it is enforced by the engine; it's judgment, verified by rendering
(`render-loop.md`).

## First: the additive ceiling — spend peak energy on structure, not luminance

**Every scene draws additively.** Particles, line segments, field samples and the feedback buffer all
*add* light into the frame, and the frame clips per channel at 1.0. Luminance terms therefore stack:
`brightness` + `glow`/`flash` + stroke `thickness` + whatever `trails` has accumulated + `bg_bright`.
Push past the ceiling and the picture does not get brighter — it goes **flat white**. Colour dies with
it (every channel pinned at 1.0 is white, whatever the palette said), and the structure that made the
look vanishes at exactly the moment the music is most exciting.

This is the failure mode that broke most of an earlier version of this library, and it hides well:
every binding looks reasonable in isolation and the quiet frame looks lovely. `brightness = "0.8 +
clamp((bass + treb) * 6, 0, 1.1)"` reads like an ordinary gain-then-bound and renders a white
rectangle. The gain is not the problem — the *destination* is.

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
  RD's voids from "shapes on black" into an atmosphere. No effect behind `fragment_field`.
- **`trails`** — needs real motion to read; it turns a spinning curve into a light-painting. High
  values plus a bright scene bloom out fast (it accumulates *into* the additive ceiling). It also
  restarts from empty on a preset switch, so a look that *is* the accumulation takes a second to
  arrive after a dissolve — judge it a few beats in, not on the opening frames.
- **`kaleido_*`** — instant symmetry on any scene; ride `kaleido_angle` on `time` so the fold turns
  rather than sits.
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
