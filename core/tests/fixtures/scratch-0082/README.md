# `scratch-0082/` — the banding reference frame

One preset, kept so the **same frame** can be re-measured after the dither lands.
A before/after taken on two different pictures would prove nothing, and the
temporary directory the original measurement ran in does not survive a session.

Not a fixture. Nothing includes it, no test names it, `LMV_BLESS` does not touch
it, and `core/build.rs` cannot see it (it globs `presets/*.toml` only). Same
arrangement as [`scratch-0046/`](../scratch-0046/README.md), for the same reason:
a phase that needs a preset needs somewhere to keep one.

## Why this one

Of the four dusk probes it has the **least light** — mean RGB `34.0 / 42.7 /
69.5` over the frame, against `82.2` for the `bg_ramp_gamma = 1.0` variant and
`122.6` for `2.5` — and it is also the **worst banding case**. Both follow from
the same cause: `bg_ramp_gamma = 0.4` drops the ramp fast and leaves a long dim
tail, and a flat tail is where one 8-bit level lasts longest.

## Run it

```sh
cargo run -p standalone --example shot --release -- \
  --preset-file core/tests/fixtures/scratch-0082/dusk_ground_banding.toml \
  --size 1920x1080 --frames 30 --out banding.png
```

**1920x1080 matters.** Plateau width is measured in pixels, so the reading is
resolution-dependent; a 1280x720 capture is a different measurement, not a
cheaper one.

To look at it live instead — the app hot-reloads an edit within ~150 ms, so the
exponent can be swept without restarting:

```sh
LMV_PRESET_DIR=core/tests/fixtures/scratch-0082 cargo run -p standalone --release
```

## What was measured, 2026-08-12, before the dither

Run lengths of identical 8-bit values down the mid-column at 1920x1080, with
rail-pinned runs excluded — a plateau of one repeated value **is** the band.

| | mean px/level | widest mid-range plateau | plateaus ≥ 16 px |
|---|---|---|---|
| **this preset (`gamma 0.4`)** | **7.5** | **58 px at value 11** | **17** |
| `gamma 1.0` variant | 4.9 | 31 px at value 30 | 18 |
| `gamma 2.5` variant | 4.1 | 122 px at value 225 | 4 |

**0 % of the column was rail-pinned** on any channel in any of the three, which
is what establishes this as a quantized gradient rather than a tonemap clip.

Per-channel on this preset: R 58/56/56/55/53 px at values 7–21, G 47/42/40/39/38
px at values 17–30, B 25/24/23/22 px. Every wide plateau is in the **dark tail**,
rows 5–436; they become hairlines as soon as the horizon brightens.

**A third set of numbers for this probe exists and is not comparable to these.**
Plan 0082's Phase 1 commit (`b6743fa`) records `136 px at value 80 -> 15 px` for
what its message describes identically — same preset, same size, rail-pinned runs
excluded. It is a different reading of the same picture, not a different picture,
and the axis each was scanned on was not written down at the time. **Use the pair
in the table below**: it is the one measured with the same instrument on both
sides, which is the only comparison that says anything. Anyone reconciling the
history should treat `b6743fa`'s figure as an artifact of how that run scanned,
and the ratio it reports (9.1x) as the shape of the effect rather than a second
measurement of this frame.

The instrument was a pure-stdlib PNG decode plus a run-length count. **Plan 0082
Phase 3 replaced it with a Rust test** —
`the_dither_dissolves_a_dark_ramps_plateaus` in
`core/src/render/tonemap/tests.rs` — which is where the permanent version
belongs; this README records the numbers, not the tool.

## What was measured, 2026-08-12, after the dither

Same preset, same size, same instrument, re-run at
[Plan 0082](../../../../docs/plans/done/0082-the-gradient-stops-banding.md) Phase 4 —
which is what this file was kept for.

| | mean px/level | widest mid-range plateau | plateaus ≥ 16 px |
|---|---|---|---|
| **before** | 7.5 | **58 px at value 11** | **17** |
| **after** | 2.1 | **20 px at value 23** | **3** |

Per-channel after: R 17 px at value 7, G 20 px at 23, B 16 px at 32 — against a
before of R 58, G 47, B 25. Still **0 % rail-pinned**, so this is the same
gradient being measured and not a frame that has started clipping.

**The widest plateau is 20 px rather than a hairline, and that is the honest
number.** The prediction in the line this replaces was "a hairline"; what the
dither actually buys here is the *level count* — 7.5 pixels per level became 2.1,
so the column now spends about a fifth as long on each value — and the collapse
of wide plateaus from 17 to 3. A residual 20-px run is a long-tail event on a
decorrelated draw over 1080 rows, not a structural step: it survives where the
ramp is locally flattest, and it moves if the frame does. The synthetic guard in
`core/src/render/tonemap/tests.rs` states the same claim as a ratio against an
undithered control resolved in the same run, which is the form that does not
depend on this picture.

## The by-eye verdict, 2026-08-12 — **it looks fine**

Plan 0082's Phase 5, the `human` one, run on this preset in the live app: release
build, `LMV_PRESET_DIR` pointed here, `rich` tier (pinned by the operator's own
`config.toml`, which is a *different* tier from every capture above — captures
pin `floor`; the backdrop has no tier-dependent budget, so the ramp is the same
one). The user's verdict on both halves was **"looks fine"**.

So the two questions this file was kept to answer are closed:

- **The bands are gone** at the setting where they were visible, and the
  measurement above says the same thing independently.
- **The grain that replaced them is not itself a problem.** This was the live
  risk rather than a formality — the dither is a *fixed* pattern and this sky is
  nearly still by design, which is close to the worst case for that choice, and
  a fixed pattern on a long-held frame can resolve as texture. It does not here.

**That retires [ADR-0096](../../../../docs/adrs/0096-the-display-write-dithers.md)
Alternative F** — an animated, time-varying dither — as a followup. It was the
answer *if* the static grain read, and it does not, so the static choice keeps
what it was chosen for: every byte-equality test between two same-sized frames
still passes. Alternative F is not foreclosed, but nothing is now asking for it.

## What to check, and when

- **After [Plan 0081](../../../../docs/plans/0081-the-sky-gets-a-galaxy.md)
  (the galactic band).** The band is a *second* wide smooth gradient over this
  same near-black sky, so this frame is the natural place to add
  `bg_band_amount` and confirm the dither still holds with two overlapping
  gradients rather than one. Nothing in Plan 0081 checks that; this is the check.

## Do not tune it

Not because a baseline depends on it — none does — but because its whole value is
being **the same frame** as the 2026-08-12 measurement above. Change a stop or an
exponent and the comparison is gone. To explore a different look, copy it.
