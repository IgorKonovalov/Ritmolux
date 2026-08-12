# ADR-0096 — The display write dithers, in the encoded domain, from an integer hash

> **Status:** proposed
> **Date:** 2026-08-12
> **Related plan(s):** [0082-the-gradient-stops-banding](../plans/0082-the-gradient-stops-banding.md)
> **Supplements:** [ADR-0046](0046-linear-light-hdr-composite-bloom-tonemap.md) (the tonemap
> is the format boundary this decision attaches to),
> [ADR-0094](0094-the-backdrop-paints-a-directional-ramp.md) (the gradient that exposed it),
> [ADR-0058](0058-bind-group-layout-collisions-carry-evidence.md) (why adapter-exactness is a
> requirement here rather than a nicety), [ADR-0071](0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md)
> (the bound this plan asserts is a property, not a measurement)

## Context

[Plan 0080](../plans/done/0080-the-sky-gets-a-horizon.md) gave the backdrop a directional ramp and
asked, as its final `human` phase, whether the result bands. **It does, and it is now measured
rather than suspected.**

Decoding the 1080p renders and taking run lengths down the mid-column — a "band" is a plateau of one
identical 8-bit value wide enough for lateral inhibition to find its edge — gives, on the dusk
ground:

| configuration | mean px per level | widest mid-range plateau | plateaus ≥ 16 px |
|---|---|---|---|
| `bg_ramp_gamma = 1.0` | 4.9 | 31 px at value 30 | 18 |
| `bg_ramp_gamma = 2.5` | 4.1 | 122 px at value 225 | 4 |
| `bg_ramp_gamma = 0.4` | 7.5 | **58 px at value 11** | 17 |

Every plateau is **mid-range, not railed** — 0 % of the column is pinned at 0 or 255 on any channel
in any of the three, so this is quantization of a gradient and not a tonemap clip. The wide ones sit
in the **dark tail**, values 7 to 30, and vanish into hairlines as soon as the horizon brightens.
The user's verdict on the running app is *"reads as light, but the banding is visible"* — so the
look works and the quantization is the one thing spoiling it.

**Plan 0080's own arithmetic pointed the wrong way, and that is worth recording.** It reasoned that
"a quarter-frame fade… spends roughly two pixels per 8-bit output level at 1080p, which is the
classic Mach-band configuration". Two pixels per level is the *safe* case — dense packing. Banding
lives where a level lasts a *long* time, which is the flattest part of the curve. Its prose
instruction ("look at the low `bg_ramp_gamma` end, where the tail is flattest and the steps widest")
named the right place while its arithmetic argued for the opposite one, and following the arithmetic
would have sent the search into the bright end where there is nothing to find.

Three facts about the pipeline decide the shape of the fix, and all three are already recorded in
the source:

1. **There is exactly one 8-bit boundary.** `tonemap.rs`'s module docs state it: the linear region
   ends at that pass's *output*, which writes display-referred values at the surface format either
   into ink's input or straight into the surface. One pipeline, and the pass is never skipped by
   design (`mod.rs`: *"Unlike ink this is never skipped — it is the format boundary"*).
2. **The surface is `Rgba8UnormSrgb`.** So the shader emits **linear** light and the *hardware*
   applies the sRGB transfer function and rounds. The quantization the eye sees does not happen in
   the value the shader writes.
3. **Adapter disagreement is expensive here.** ADR-0058 and Plan 0053 exist because WARP and
   hardware rendering different pictures cost this project two silent mis-renders, and the golden
   drift guard's tolerance is 0.02 of one 8-bit level. A fix that itself varied per adapter would
   blow that on every baseline.

## Decision

**The tonemap dithers its output**: a triangular-PDF perturbation of ±1 encoded LSB, derived from an
**integer** hash of the pixel coordinates, scaled into linear space by the inverse of the sRGB
transfer function's local slope. Always on — this is a property of the display write, like the
tonemap itself, not a parameter.

Three parts, each load-bearing:

**The noise is TPDF, not uniform.** Two hashed uniforms on `[-0.5, +0.5]` summed give a triangular
distribution on `[-1, +1]` LSB. That is the standard result: TPDF at this amplitude fully
decorrelates the quantization error from the signal, which is what removes the plateau rather than
merely softening its edge. Uniform noise leaves a signal-dependent residual.

**The hash is integer bit-mixing, never `fract(sin(dot(p, k)) * 43758.5453)`.** The trig idiom is
the common one and it is disqualified here: `sin`'s precision is implementation-defined, so WARP and
hardware would disagree on essentially every pixel. An integer hash on `u32` coordinates — the
arrangement `scenes/particles/` already uses in `hash_unit` / `hash3` — is exact and identical on
every adapter. This turns the usual "compare the adapters before blessing" step into a **sharper**
instrument than normal: the two must agree *byte-for-byte*, not to within the 0.02 drift floor.

**The amplitude is scaled by the inverse sRGB slope, because the hardware encodes after the
shader.** Adding a constant amplitude in linear space is the obvious implementation and it is wrong
in both directions at once. With `E = 12.92·L` below the knee and `E = 1.055·L^(1/2.4) − 0.055`
above it, `dE/dL` runs from **12.92 near black to 0.44 at white** — a 29x spread. A flat `1/255`
linear amplitude therefore perturbs the encoded value by **12.9 LSB in the dark tail** (visible
noise) and **0.44 LSB at the bright end** (too little to dither), and the dark tail is precisely
where every measured plateau is. The shader divides by the local slope so the perturbation is one
encoded LSB everywhere.

**The dither is static** — a pure function of pixel coordinates, with no time term.

## Consequences

**Positive.**

- The banding goes away at its source, for **every** gradient in the engine at once: the backdrop
  ramp, the band [ADR-0095](0095-the-backdrop-paints-a-curved-band.md) is about to add, bloom
  falloffs, depth fades, and any gradient a future plan draws. One site, one fix.
- **Every existing byte-equality test keeps working.** Because the dither is positional and static,
  two frames rendered at the same size receive identical noise at every pixel — so an assertion like
  Plan 0075's `depth_fade` no-op, which compares two renders byte-for-byte, is untouched. This is
  not luck; it is the reason "static" was chosen over "animated".
- The adapter-exactness requirement buys a **stronger** golden instrument than the project had
  before: an integer-hash dither must produce byte-identical output on WARP and on hardware, which
  is a claim the 0.02 drift floor could never make.

**Negative — the price, stated plainly.**

- **Every golden baseline is re-blessed, once.** All 27 move, because every pixel can shift by a
  level. In a repo whose standing discipline is "zero baselines moved", this is the first deliberate
  full re-bless, and it temporarily blinds the drift guard to an unrelated regression landing
  alongside it. The mitigation is that the change is **bounded and provable**: `round(x + n)` with
  `|n| ≤ 1` differs from `round(x)` by at most one level, so the re-bless is verified by asserting
  `max |before − after| ≤ 1` across every baseline rather than by trusting it. It lands alone, in
  its own commit, with nothing else in it.
- **Assertions with a `≤ 1` tolerance lose headroom.** Plan 0080's `backdrop_ramp.rs` compares
  configurations that should agree to within one level. The dither should not amplify those — it
  shifts the rounding threshold *identically* for both frames — but this is reasoning, not a
  measurement, and Plan 0082 checks it rather than assuming it.
- **A faint static grain replaces the bands.** At one encoded LSB it is at the threshold of
  visibility, but it is a fixed pattern rather than a moving one, and on a long-held still frame a
  fixed pattern can read as texture. Alternative F is the answer if it does.
- **The degenerate fallback path does not dither.** When `tonemap.begin` cannot build its target the
  composite falls through to the old clipped 8-bit path; that frame is undithered. It is already the
  "never drop the frame" branch and is not worth a second pipeline.

## Alternatives considered

### Alternative A — ordered (Bayer) dither

An 8x8 threshold matrix, the cheapest possible implementation and trivially deterministic across
adapters. **Rejected because it substitutes one visible artifact for another**: ordered dither
leaves a regular cross-hatch on flat areas, and flat areas are the entire problem here — a
near-black sky is the largest flat area this engine draws. Its determinism advantage is real but
duplicated by the chosen option, since an integer hash is equally exact.

### Alternative B — a baked blue-noise texture

Perceptually the best available: blue noise pushes its energy into the high spatial frequencies the
eye filters hardest, so it hides at lower amplitude than white noise. **Rejected on blast radius
rather than on quality.** It needs a baked binary asset committed to the repo, a texture and sampler
binding on the tonemap, and therefore a **new bind-group-layout shape** — which is ADR-0058
territory, with an enumeration entry and an adapter measurement, on a pass that is live in every
single frame. Worth revisiting if the hashed grain reads; not worth paying up front.

### Alternative C — the `fract(sin(dot(p, k)) * 43758.5453)` hash

The idiom every shader on the internet uses. **Rejected on adapter-exactness**: `sin`'s precision is
implementation-defined, so the two adapters would produce different noise on essentially every
pixel. The fix would then be indistinguishable from the class of defect ADR-0058 exists to catch,
and it would make the golden suite permanently unable to compare adapters.

### Alternative D — a constant dither amplitude in linear space

Simpler by one function. **Rejected on arithmetic**, and it is the alternative most likely to be
reintroduced by someone tidying the shader: `dE/dL` spans 29x across the range, so a flat linear
amplitude is ~12.9x too strong near black and ~2.3x too weak at white — wrong in both directions,
and worst exactly in the dark tail where every measured plateau lives.

### Alternative E — a non-sRGB surface with the encode done in the shader

Move to `Rgba8Unorm`, apply the sRGB curve in the tonemap, and dither directly in the encoded
domain. The cleanest possible dither math, with no slope term at all. **Rejected on blast radius:**
the format is chosen in `context.rs` and consumed by the standalone surface configuration, the
capture path and the foobar shim, and a shader-side encode differing from the hardware's by a
rounding step would shift the colour of every baseline — a much larger and less bounded change than
the one being made.

### Alternative F — an animated, time-varying dither

Re-hash per frame so the grain moves. Perceptually better on still images, where a fixed pattern can
be resolved as texture. **Rejected for now on two grounds:** at one LSB on a dark flat sky a 60 Hz
flicker risks reading as shimmer, which is a worse artifact than the one being fixed; and it would
give up the property that makes this decision cheap — that every existing byte-equality test between
two same-sized frames still passes. Revisit if the static grain reads as texture; it is one term.

### Alternative G — a `dither` param, defaulting off

No baseline moves and nothing is forced on anyone. **Rejected because nobody would ever turn it
on**, and more importantly nobody would deliberately turn it *off*: correct quantization of the
display write is not a look, it is what the write is supposed to do. A param here would mean the
shipped library keeps the defect while the engine carries the cure.

### Alternative H — do nothing, or reach for more bits

Accept the bands, or render to a 10/16-bit surface. **Rejected:** the swapchain is
`Rgba8UnormSrgb`, most displays this ships to are 8-bit, and the user's verdict on the running app
was that the banding is visible and spoiling an otherwise working look.

## Notes

- The measurement above was taken on the dusk-ground probes at 1920x1080 with a pure-stdlib PNG
  decoder, counting run lengths of identical 8-bit values down the mid-column and separating
  rail-pinned runs from mid-range ones. The instrument is worth keeping — Plan 0082 turns it into a
  test, because a plateau-width assertion on a synthetic dark ramp is exactly what would catch
  someone removing the slope term from Alternative D.
- This decision is sequenced **before** [ADR-0095](0095-the-backdrop-paints-a-curved-band.md) /
  [Plan 0081](../plans/0081-the-sky-gets-a-galaxy.md) by the user's call, so the galactic band is
  born onto a chain that already dithers and its own `human` verdict is not confounded by a defect
  already known about.
