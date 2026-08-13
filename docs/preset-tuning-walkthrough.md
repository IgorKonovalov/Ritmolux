# Tuning a preset: one file, five steps

This takes one preset from "it renders" to a finished look in five numbered steps. Each step is a
committed file you can run, a picture, and **the `--report` rows that changed** — because the numbers
are what make this a method rather than a slideshow.

Read [`docs/preset-guide.md`](preset-guide.md) first if you have not written a preset before; this
page assumes you know what a system and a binding are. It does not re-explain the report's columns
either — [`docs/capturing.md`](capturing.md#what-the-reports-columns-mean) owns those.

**The five files are teaching material and never ship.** They live in
[`docs/examples/tuning/`](examples/tuning/), not in `presets/`: step 2 is deliberately bad, and a bad
preset in `presets/` would enter the preset browser, the embedded set and all five behavioral gates.

Every step's numbers come from one command:

```sh
cargo run -p standalone --example shot --release -- \
  --report --preset-file docs/examples/tuning/step-1-constants.toml
```

and every step's picture from one more:

```sh
cargo run -p standalone --example shot --release -- \
  --preset-file docs/examples/tuning/step-1-constants.toml \
  --signal dynamic:110 --frame-at 230 --size 1280x720 --tier rich --out step-1.png
```

All five pictures are captured at **the same hop of the same clip** — hop 230, which is mid-build
rather than at the peak — so they are comparable, and so step 2's failure is visible instead of being
hidden by the loudest moment in the stimulus.

---

## Step 1 — it renders

Every binding is a constant. There is a picture and there is nothing in it but the picture.

[`step-1-constants.toml`](examples/tuning/step-1-constants.toml):

```toml
system = "fragment_field"
name = "Step 1 Frame"

[params]
warp  = "0.85"
zoom  = "1.75"
glow  = "0.50"

kaleido_order  = "8"
kaleido_radial = "1.45"
kaleido_edge   = "2"
kaleido_inner  = "0.06"

hue          = "0.58"
color_span   = "1.45"
saturation   = "1.10"
```

![A radial mandala in magenta, orange and cyan: eight spoked petals repeated inward through four
concentric shrinking rings](images/walkthrough/step-1.png)

```
  preset            bass     mid    treb   onset    anim   cover  rise  fall
  Step 1 Frame     0.000   0.000   0.000   0.000   0.230   0.916   14+   34+

  at realistic levels (bass 0.661 mid 0.575 treb 0.281 onset 0.145)
  preset            bass     mid    treb   onset   gates   ceils   occ
  Step 1 Frame     0.000   0.000   0.000   0.000       0       0     0
```

**Four exact zeros, twice.** That is what "no music in this picture" looks like as a number, and it is
the only completely unambiguous reading in this whole document: nothing is bound to a band, so nothing
moves when a band comes up.

**`anim` is `0.230`, and it is not supposed to be.** The plan for this document expected `anim` at the
floor here — a preset of pure constants should have no life of its own. It does, and the reason is
worth knowing early: `anim` measures how far the frame moves between two capture depths **under
silence**, and a `fragment_field` keeps evolving on its own deterministic scene clock whether or not
anything is bound to it. So `anim` answers "does this move", not "does this move *because of the
music*" — and a preset can pass `animation` on scene motion alone while ignoring every band. The
columns that separate those two questions are the four to its left.

---

## Step 2 — bind the bands, naively

Three bands and a gate, written to the intent *react to the loud bits, ignore the quiet ones* — each
binding subtracts a floor before applying a gain. Nothing here is a typo. This is what a first draft
honestly looks like.

[`step-2-naive-bands.toml`](examples/tuning/step-2-naive-bands.toml), changed lines only:

```toml
warp  = "0.85 + clamp((treb - 0.35) * 2.40, 0, 0.90)"
zoom  = "1.75 + clamp((bass - 0.70) * 1.60, 0, 0.50)"
glow  = "0.50 + clamp((onset - 0.30) * 2.00, 0, 0.60)"

kaleido_order = "select(treb > 0.55, 12, 8)"
```

![The same mandala, its petals broadened into eight long tapered rays of magenta and orange over a
cyan field, with a small eight-pointed star at the centre](images/walkthrough/step-2.png)

```
  preset            bass     mid    treb   onset    anim   cover  rise  fall
  Step 2 Naive     0.218   0.000   0.199   0.133   0.230   0.952    4+    1+

  at realistic levels (bass 0.661 mid 0.575 treb 0.281 onset 0.145)
  preset            bass     mid    treb   onset   gates   ceils   occ
  Step 2 Naive     0.000   0.000   0.000   0.000       0       1     0
```

**The top row says this works. The bottom row says it is dead.** `bass` `0.218`, `treb` `0.199`,
`onset` `0.133` — a lively preset by the first table. At realistic levels every one of them is
**exactly `0.000`**, unchanged from step 1's constants.

The arithmetic is not subtle once you look for it. `bass` at a realistic level is `0.661`, and the
binding subtracts `0.70` before it does anything — so the clamp floors at zero and `zoom` is a
constant. `treb` is `0.281` against a `0.35` floor. `onset` is `0.145` against `0.30`. All three
bindings are switched off across the entire range real music occupies, and only wake up in the top
third of the band that the report's *first* table drives them to.

**This is the single most useful pair of rows in the report**, and the reason the second block exists
at all: the settled columns are driven at band = 1.0, which no piece of music sustains. Read the two
together or you will ship step 2.

**`mid` is `0.000` in both rows because nothing is bound to it.** A column of exact zeros in *both*
tables means "no binding", not "weak binding" — a different failure from the other three and worth
distinguishing.

The picture is the same reading. Compare it against step 1: the geometry has changed, because at
hop 230 the fold and warp constants differ from step 1's — but the *variation* the bindings were
supposed to add is absent, and under a real clip this preset holds still until a peak and then jumps.

**One thing that is fine, and it surprised me.** `gates` is `0`, and the report says *every branch was
taken under the 110 BPM probe* — so `select(treb > 0.55, 12, 8)` is not a dead branch. The
reachability probe runs the real analyzer over the same generator, where `treb` peaks at `1.000`, so
the gate does fire. It fires on peaks only, which is a design choice rather than a defect, and the
report is right not to flag it.

---

## Step 3 — calibrate against a measured level

Delete the floors. Size every gain against **the level the band actually reaches** — read off the
table a `--signal` capture prints on every run, not off a `--set` magnitude.

Those means, from the capture itself:

```
  signal      min     mean      max
  bass      0.035    0.747    1.000
  mid       0.031    0.633    1.000
  treb      0.002    0.295    1.000
  onset     0.001    0.153    1.000
```

Note how far apart they are: `treb` sits at a third of `bass` and `onset` at a fifth, so the *same*
gain on each is three different amounts of reaction.

[`step-3-calibrated.toml`](examples/tuning/step-3-calibrated.toml), changed lines only:

```toml
# treb mean 0.295 * 1.70 = 0.50, half of the 0.90 travel.
warp  = "0.85 + clamp(treb * 1.70, 0, 0.90)"
# bass mean 0.747 * 0.34 = 0.25, half of the 0.50 travel.
zoom  = "1.75 + clamp(bass * 0.34, 0, 0.50)"
# mid was bound to nothing at all. mean 0.633 * 0.28 = 0.18, half of 0.35.
glow  = "0.50 + clamp(mid * 0.28, 0, 0.35)"
# onset mean 0.153 * 1.30 = 0.20, half of 0.40.
flash = "clamp(onset * 1.30, 0, 0.40)"
```

![A soft flower mandala: rounded six-lobed blossoms in cyan and magenta outlines over a pale
green-gold field, repeated inward through five shrinking rings](images/walkthrough/step-3.png)

```
  preset            bass     mid    treb   onset    anim   cover  rise  fall
  Step 3 Calibra   0.211   0.083   0.199   0.097   0.230   0.939   39+    1+

  at realistic levels (bass 0.661 mid 0.575 treb 0.281 onset 0.145)
  preset            bass     mid    treb   onset   gates   ceils   occ
  Step 3 Calibra   0.171   0.050   0.156   0.053       0       2     0
```

**The realistic row comes up off the floor**: `0.000 / 0.000 / 0.000 / 0.000` becomes
`0.171 / 0.050 / 0.156 / 0.053`. That is the whole step.

**And now look at the top row, because it barely moved.** Step 2 read `0.218 / 0.000 / 0.199 / 0.133`
and step 3 reads `0.211 / 0.083 / 0.199 / 0.097` — `bass` and `onset` actually went *down*. The
settled table cannot tell these two presets apart, and one of them is dead on music and the other is
not. If you take one thing from this document, take that: **the first table is not the one that says
whether a preset reacts.**

Three smaller readings:

- **`mid` appears** — `0.000 → 0.083` and `0.000 → 0.050` — because step 3 bound it. Where a column
  was zero for lack of a binding, adding one is the whole fix.
- **`occ` is `0`, which is what you want**, and it is worth knowing what a non-zero value would have
  meant. `occ` counts clamps that sat *at their upper bound* for the run: a `clamp` pinned at its
  ceiling has stopped being a function of the audio and is just an expensive constant. Over-correcting
  a gain is exactly how you get one, so this is the column to watch on the step *after* a
  recalibration, not before it.
- **`ceils` is `2`**, and the report names them: *`zoom` at 68%, `glow` at 80%*. That is the opposite
  finding — bounds that never bite. Neither is wrong here; it means those two parameters have a
  narrower real range than their bindings advertise, which is worth knowing when you next widen one.

---

## Step 4 — ease it

An expression has no memory: it is re-evaluated every frame and follows its band transient for
transient. The `[smoothing]` table is the memory, and the `{ attack, release }` pair is the shape that
makes an accent read as a response rather than a flicker.

[`step-4-eased.toml`](examples/tuning/step-4-eased.toml) adds only this:

```toml
[smoothing]
warp  = { attack = 0.05, release = 0.45 }
flash = { attack = 0.03, release = 0.50 }
zoom  = 0.30
glow  = 0.25
kaleido_order = 0.70
```

![The same flower mandala with wider, more open rings: five concentric bands of cyan and magenta
petal outlines over a pale gold field](images/walkthrough/step-4.png)

```
  preset            bass     mid    treb   onset    anim   cover  rise  fall
  Step 4 Eased     0.211   0.083   0.199   0.097   0.230   0.939   39+    4+

  at realistic levels (bass 0.661 mid 0.575 treb 0.281 onset 0.145)
  preset            bass     mid    treb   onset   gates   ceils   occ
  Step 4 Eased     0.171   0.050   0.156   0.053       0       2     0
```

**Eight of the nine numbers are byte-identical to step 3.** That is correct and it is documented
behaviour, not a bug: those columns are *settled* measurements — the capture holds one stimulus for
every frame it renders, so every smoother has long since converged before the pixels are read. **No
`[smoothing]` constant can ever move them.** If you change easing and the reactivity columns move,
something else changed too.

**The picture is a different matter, and the difference between the two is the point of easing.**
Compare this frame against step 3's: the rings are wider and the petals more open, from a file whose
`[params]` block is character-for-character identical. A settled measurement asks where a parameter
*ends up*; a capture at hop 230 catches it *on the way*, and that is the only place easing exists.
So a report that cannot see your smoothing table and a picture that visibly can are both telling the
truth about different questions.

The one number that moved is `fall`: **`1+ → 4+`**, with `rise` unchanged at `39+`. The direction is
right — a release of 0.45–0.50 s should stretch the fall and leave the attack alone — and the
magnitude is not a measurement.

**The `+` is the important character in that cell**, and it means the value is a **lower bound**. The
transient probe reads back a 48-frame (0.8 s) window, and a release constant longer than about 0.35 s
does not settle inside it; the marker says the number is short, without saying by how much. On top of
that the probe measures the *frame*, not the parameter, so a fragment field's own fold motion is
mixed into every reading. So `4+` is evidence the easing is doing something and is not a measurement
of how much. Easing is *proven* in `core/tests/easing.rs` against fixtures built for the purpose;
everything here is a preset-shaped approximation of that, and
[`docs/capturing.md`](capturing.md#what-the-transient-columns-cannot-see) is honest about it at
length.

**One thing in that table is not just easing.** `kaleido_order = 0.70` smooths a parameter whose
meaning is an **integer** — a fold count. Easing makes it continuous, so it passes through 9.4 and
10.7 on its way from 8 to 12, and the engine quantizes on the way out. That is fine here and is worth
being conscious of whenever you smooth a count rather than a level.

---

## Step 5 — colour, and a beat accent

The last step gives the preset a palette of its own and one term latched to the **beat** rather than
to a band.

[`step-5-colour-and-beat.toml`](examples/tuning/step-5-colour-and-beat.toml) adds a six-stop palette,
plus:

```toml
palette_steps   = "7 + beat * 3"
palette_contour = "0.35"

[smoothing]
palette_steps = { attack = 0.05, release = 0.55 }
```

![A calm banded mandala: concentric rings of pointed petals in sage, teal, cream and apricot with
hard-edged colour bands, an eight-pointed apricot star at the centre](images/walkthrough/step-5.png)

```
  preset            bass     mid    treb   onset    anim   cover  rise  fall
  Step 5 Finishe   0.134   0.094   0.130   0.062   0.184   0.907   29+   15+

  at realistic levels (bass 0.661 mid 0.575 treb 0.281 onset 0.145)
  preset            bass     mid    treb   onset   gates   ceils   occ
  Step 5 Finishe   0.106   0.057   0.116   0.031       0       2     0
```

**The picture is finished and four of the numbers got worse.** `bass` `0.171 → 0.106`, `treb`
`0.156 → 0.116`, `onset` `0.053 → 0.031`, and `anim` `0.230 → 0.184`. Not one binding from step 4
changed. This is the honest failure mode, and retuning until the story held would have been the
dishonest one.

The cause is `palette_steps`. Quantizing colour into seven hard bands means a parameter can move a
pixel's palette coordinate a long way and land it in **the same band** — so the same underlying
reaction produces a smaller measured pixel difference. The metric is a mean per-channel frame
difference; posterizing the output lowers it by construction. The preset did not become less
reactive. The instrument became less able to see it, because the look deliberately throws colour
resolution away.

That is worth stating as a general rule, since it will catch you again: **`palette_steps`,
`palette_contour`, heavy `trails` and a low `exposure` all suppress the reactivity columns without
suppressing the reaction.** When a step lowers those numbers and the picture is better, believe the
picture — but say so out loud, as here, rather than quietly.

**`beat` is a different kind of binding, and reachability is the check for it.** A band is a level and
varies continuously; `beat` is an edge that is either taken or not. So the question is not "is the
gain right" but "does this branch ever fire under real material" — and the report answers it directly:

```
  every branch was taken under the 110 BPM probe
```

That line, and its opposite, are what to read after adding any `select()`, comparison or beat-latched
term. A branch that never fires is a piece of the preset that has never rendered, and no reactivity
column will tell you — a dead branch does not lower a number, it just quietly does not exist.

**`fall` finally opens up**: `4+ → 15+` against `rise` `29+`. The direction is *backwards* from the
textbook signature — a `{ attack, release }` pair should give `fall` much larger than `rise` — and it
is backwards for the reason step 4 already gave: the probe measures the frame, and the scene's own
motion is in the reading. Over the shipped library this is common enough to be documented: presets
carrying an asymmetric entry sit at a median `fall / rise` of about 1.02, not the large ratio the
easing actually has.

The `{ attack = 0.05, release = 0.55 }` on `palette_steps` is not optional decoration. Quantizing
colour changes every pixel at once, which is exactly the shape a strobe has; fast in and slow out is
what turns that into a swell.

---

## What the five steps add up to

| Step | What changed | The number that said so |
|---|---|---|
| 1 | constants only | four exact zeros in both blocks |
| 2 | bands bound with a floor | full-scale row lively, realistic row `0.000` across |
| 3 | gains sized to measured means | realistic row `0.000` → `0.171 / 0.050 / 0.156 / 0.053` |
| 4 | `[smoothing]`, asymmetric | `fall` `1+ → 4+`; everything else identical, by design |
| 5 | palette + beat accent | reactivity columns *fell*, and the picture is better |

Three habits are the whole method:

1. **Read the realistic-levels block, not the table above it.** Steps 2 and 3 are indistinguishable in
   the settled columns and completely different on music.
2. **Calibrate against the level table the capture prints.** Every gain in step 3 is a measured mean
   divided into a chosen travel. None of them is a guess, and none came from a `--set` magnitude.
3. **When a number moves the wrong way, say so.** Step 5's did. The alternative — retuning until the
   numbers agree with the prose — produces a document that teaches a method nobody can reproduce.

Back to [the guide](preset-guide.md); the reference for the columns used here is
[`docs/capturing.md`](capturing.md#what-the-reports-columns-mean), for the grammar
[`docs/presets.md`](presets.md), for the parameters [`presets/README.md`](../presets/README.md), and
for the palette surface [`docs/preset-palettes.md`](preset-palettes.md).
