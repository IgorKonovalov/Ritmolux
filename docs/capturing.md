# Headless capture & visual QA

The renderer can draw a scene with **no window** — a surface-less wgpu context
draws into an offscreen texture and hands back raw RGBA pixels. Two things are
built on that (Plan 0013):

- a **`shot` CLI** (`standalone/examples/shot.rs`) that writes PNGs an agent can
  read and a metrics report it can parse, and
- a **differential visual-QA harness** in `core/tests/` that hard-tests every
  preset for reactivity, animation, shape sanity, and beat response, with an
  advisory distinctness report and golden-image regression.

A headless render is a **pure function** of `(preset, input, frame-count, size)` —
scenes are reseeded per capture, every frame steps at the fixed
`scenes::FALLBACK_DT` (the live app injects its real `dt` instead; Plan 0014),
every capture path pins the preset's **declared numeric seed** so the grammar's
`hash()`/`noise()` reproduce even where the preset asked for `seed = "random"`
(Plan 0047 / [ADR-0051](adrs/0051-seeded-grammar-randomness-with-per-run-opt-in.md)
— see [Seeded randomness](../presets/README.md#seeded-randomness--hash-noise-and-generator-seed)),
and the DSP is deterministic — so renders are reproducible and diff-able.

> The seed pin is the one place a capture deliberately shows you something other
> than the live app: a `seed = "random"` preset's filmstrip is *an* instance of it,
> not the instance a user will see. Tune with a number.

## Captures pin the floor tier

**Every capture path renders at the `Floor` quality tier, and it cannot do
otherwise by accident.** `Renderer::new_headless` takes no tier argument and
resolves `Floor` by construction (Plan 0044 / [ADR-0045](adrs/0045-quality-tiers-floor-and-rich.md)),
so there is no field a test can forget and no environment variable that can change
what a baseline looks like. `shot` defaults to `floor` for the same reason and
deliberately does **not** read `LMV_TIER`.

Two reasons, and both are load-bearing:

- **Reproducibility.** A tier sets capacity — particle counts, the segment budget,
  the internal-grid caps — so a baseline blessed on a rich-tier run and compared
  against a floor-tier one differs for a reason that has nothing to do with the
  change under test. A capture is a pure function of its inputs (NFR §6), and the
  tier would otherwise be a hidden input.
- **Suite cost.** The golden and visual-QA suites run on the WARP software
  adapter, where fill and instance count translate directly into wall-clock. At
  rich values the same suite would draw 3x the attractor particles into a 4K-capped
  trail grid on a CPU rasterizer.

`--tier rich` is the deliberate opt-in, for spot-checking that the raised budgets
actually render. Use it to *look*, not to bless: a rich capture must never be
written into `core/tests/golden/`.

> The consequence ADR-0045 names and accepts: rich-tier regressions are caught only
> by those spot checks and by on-device runs, not by the suite. That is a real hole,
> not a solved problem.

> `--size` is part of that tuple, and since Plan 0033 it does more than crop: the
> `trails` and `kaleido_*` stages size their internal grid from the render target
> (ADR-0034), so a preset composing either one genuinely renders *differently* at
> 640x360 than at 1080p rather than merely smaller. A given size is still exactly
> reproducible; two sizes are no longer scaled versions of one picture. Capture at
> the size you are judging.

Everything here is **dev/agent tooling**. The `image` crate is a *dev-dependency*
only (ADR-0011), so the shipped `lmv.exe` is untouched; the CLI is a
`cargo run --example`, not a subcommand of the app.

> Package name note: the standalone crate is `standalone`, so the invocation is
> `cargo run -p standalone --example shot -- …`.

## The `shot` CLI

Render one preset to a PNG (the agent then Reads the file):

```bash
cargo run -p standalone --example shot -- --preset "Aurora" --frames 120 --out shot.png
```

Flags:

| flag | meaning |
|------|---------|
| `--preset <name>` | preset to render (by name, as shown in the report / library); optional when the library holds exactly one preset |
| `--presets <dir>` | load the library from `<dir>` instead of the resolved preset directory |
| `--preset-file <path>` | load exactly one preset from `<path>` (beats `--presets`) |
| `--set k=v,...` | constant stimulus frame: `bass,mid,treb,onset,bar,novelty` (0..1), `tempo` (BPM), `beat` (non-zero = true). Keys are the **grammar's** names, so `tempo` is what a binding writes. It reaches the frame's scalars only — **not** the 64-band spectrum, so `bin(x)` reads `0` — see [the calibration traps](#the-three-calibration-traps) before trusting a value |
| `--frames <N>` | frames to advance before capture (default 120) |
| `--size <WxH>` | render size (default 1280x720) |
| `--out <path>` | output PNG (single shot) or dir/file (`--all`) |
| `--all` | contact sheet of every preset, labeled (needs `--out`) |
| `--report [family=<sys>]` | per-family metrics table — reactivity, animation, coverage and the [transient probe](#the-transient-columns); `family=` takes any `system` name (`fragment_field`, `swarm`, `parametric_curve`, `lsystem`, `star_pattern`, `reaction_diffusion`, `attractor`, `spectrum`) |
| `--json` | emit the report as JSON instead of a text table |
| `--signal <kind:param>` | synth-audio filmstrip (see below) |
| `--audio <clip.wav>` | filmstrip from a 16-bit PCM WAV |
| `--strip <N>` | frames tiled along the audio (default 8) |
| `--tier floor\|rich` | quality tier to capture at (default `floor` — see below) |

Bad arguments and unknown presets exit non-zero with a message.

### The three calibration traps

`--set` is a **held** stimulus: it writes the analysis frame directly and that
same frame drives every captured frame. That makes it perfect for isolating one
binding and wrong for three things people reach for it anyway.

**Trap 1 — `--set beat=1` holds the beat gate high for the whole capture.** Real
beats are transient: `beat` fires on one hop and is false on the next. Held high,
every `beat`-driven accent in the preset is at full deflection in the still you
are looking at — a `+ beat * 0.155` thickness term that should flash for a frame
instead reads as the preset's baseline. The result is that a *working* preset
looks broken in a still (permanently blown-out, or so busy the geometry is
unreadable), and the natural response — turning the accent down — breaks it for
real on live audio. If you want a beat, capture one:

```bash
# Transient beats through the real onset detector, no asset needed
cargo run -p standalone --example shot -- --preset "Pulse Field" \
  --signal click:120 --strip 8 --out click.png
```

**Trap 2 — `--set` band magnitudes are not real levels.** `--set bass=0.8` writes
`0.8` onto the frame, but a band that arrives through the analyzer is normalized,
and it does not get anywhere near there. Measured through this very harness: a
**full-scale 60 Hz sine** (`--signal bass:60`) reads `bass ≈ 0.19`, and a **120
BPM click track** peaks at `bass ≈ 0.011`. A gain tuned to look right at
`bass=0.8` is roughly four times too weak on the loudest pure tone the harness
can synthesize, and the preset barely moves on real music.

So every `--signal` / `--audio` filmstrip now **prints the levels it measured**,
and those are the numbers to calibrate a gain against:

```
audio levels over 367 analysis hops (past warm-up) — calibrate gains against these, not against --set magnitudes:
  band       min     mean      max
  bass     0.187    0.187    0.187
  mid      0.000    0.000    0.000
  treb     0.000    0.000    0.000
```

`--audio <clip.wav>` on real material is the one that answers "what does my music
actually produce"; `--signal` answers it for a known synthetic tone. Use `--set`
to ask "does this binding do anything at all", not to decide how much of it to
apply.

**Trap 3 — `--set` leaves the 64-band spectrum silent, so `bin(x)` reads `0`.**
`--set` writes the analysis frame's *scalars*; there is no key for the log-band
array, and `AnalysisFrame::default()` leaves all 64 bands at zero. So under any
`--set` capture:

- every `bin(x)` call in an expression returns `0`, whatever `bass`/`mid`/`treb`
  say, and
- the whole **`spectrum`** system draws its `base` resting comb and nothing else —
  the readout is inert, which looks exactly like a broken preset.

This is not a bug in `--set` (it writes what you ask and nothing else); it is the
one part of the frame it cannot reach. **`bin(x)` and the `spectrum` system have
to be verified through `--signal` or `--audio`**, both of which run the real
analyzer over real samples and therefore populate the array:

```bash
# The band array through the real FFT - the readout actually moves
cargo run -p standalone --example shot -- --preset "Spectrum Comb" \
  --signal chord --strip 3 --out comb.png
```

`--report` builds its stimulus frames in code rather than from `--set`, and those
**do** light the band array (each named band lights the slice of the log spectrum
it summarises), so the report's numbers are real for a spectrum preset.

### What the report's columns mean

```
  preset            bass     mid    treb   onset    anim   cover  rise  fall
  Smooth Pulse     0.167   0.087   0.250   0.206   0.076   0.927    26    31
```

| column | question it answers |
|---|---|
| `bass` `mid` `treb` `onset` | how far the frame moves when that stimulus alone comes up, against silence — "does this preset respond to bass at all" |
| `anim` | how far the frame moves between two capture depths **under silence** — does it have a life of its own |
| `cover` | fraction of the frame that differs from the corner background — [a low value is often correct](#a-low-cover-is-not-a-defect) |
| `rise` `fall` | the **transient probe** (below) — frames to settle after a step up, and after the matching step down; a **`+` suffix** means the value is a *lower bound*, not a measurement (below); [read them as evidence, not a verdict](#what-the-transient-columns-cannot-see) |

Every one of those but the last two is a **settled** measurement: the capture
holds one stimulus for every frame it renders, so each smoother has converged
long before the pixels are read. That is the right question for "does it
respond", and it is exactly why those columns are **identical for any
`[smoothing]` constant**.

#### The transient columns

`rise` and `fall` are the one pair that is *not* settled. The probe drives a
step — silence, a held stimulus, silence again — reads back **every** frame, and
counts how many it takes for the frame to reach 90 % of its total change each
way ([ADR-0039](adrs/0039-verify-easing-with-a-transient-probe-not-a-committed-clip.md)).
That makes [ADR-0035](adrs/0035-asymmetric-attack-release-easing.md)'s
`{ attack, release }` pair visible: a scalar `[smoothing]` entry selects the same
constant both ways, so its two numbers match; a pair snaps up and glides down, so
`fall` runs well past `rise`.

Reading them:

- **`1` and `1`** — no easing on whatever the stimulus drives. The frame is fully
  there the frame after the step.
- **equal, both large** — a scalar `[smoothing]` entry, or an asymmetric one whose
  two constants are close.
- **`fall` much larger than `rise`** — an `{ attack, release }` pair doing its job.
- **`0` and `0`** — the step did not move the frame at all. Check the reactivity
  columns: this usually means the preset does not respond to the stimulus, not
  that its easing is instant.

#### What the transient columns cannot see

**The probe measures the frame, not the parameter.** It reads pixels, so it can
only see easing through whatever curve the scene puts between a bound value and
its output — and for most scenes that curve is neither linear nor even monotone.
Two consequences, both real and neither fixable by measuring harder:

- **A saturating response reads flat.** `rose_trails` is the worked example: its
  1.25 spin against a max-decay feedback drives the frame to the same place
  whatever `thickness` says, which is why the content lane once rendered five
  values from 1.10 to 2.30 — *including the untouched original* — and could not
  tell them apart. A preset like that will report a transient that has nothing to
  do with its `[smoothing]` table, and no column here will warn you.
- **The scene's own motion is measured too.** A fragment field's fold, a feedback
  trail and a particle cloud all keep changing while the parameter settles, and
  the probe cannot separate that from the response. Over the shipped library this
  shows up as presets reporting `fall` *below* `rise`, which is backwards for any
  easing.

Measured over the shipped set on 2026-07-27 — a snapshot of what the probe sees,
not a figure anyone maintains — presets carrying at least one
`{ attack, release }` entry had a median `fall / rise` of **1.02**, with
`fall > rise` in about half of them; presets with only scalar entries sat at
**0.60**, with barely any. So the columns separate the two populations
*directionally* and lose the magnitude almost entirely:
`Smooth Pulse`, the worked asymmetric example with a 0.60 s release,
reads `26 / 31` where a purpose-built near-linear fixture at a 0.5 s release reads
`3 / 61`.

> **Those numbers were taken before the `+` marker existed, and every one of them
> would carry it today** (Plan 0038 Phase 8). They were produced by exactly the
> defect this section goes on to describe: at `PROBE_WINDOW` = 48 a 0.60 s release
> is 1.33 τ, leaving ~26 % of the travel undone, and the fixture's `3 / 61` is now
> known to be `3 / 69` when measured to settlement. Read the snapshot for its
> *shape* — two populations, separated directionally, magnitude lost — and not for
> its magnitudes, which is the same warning the rest of this section gives, now
> with the arithmetic behind it. It has not been re-taken: re-snapshotting is not
> what makes the columns trustworthy, marking them is.

**Read the columns as evidence, not as a verdict.** A wide `fall / rise` gap is
good evidence the easing is working. A narrow one is not evidence it is broken.
The place easing is proven is `core/tests/easing.rs`, against fixtures built to
have a near-linear response precisely so the measurement is of the easing and not
of a scene; everything else is a preset-shaped approximation of that.

One smaller limit, and it is sharper than it was first written: the probe's window
is **48 frames (0.8 s) each way**, so a release constant longer than about 0.35 s
does not fully settle inside it. This page used to say such a response "reads
*clamped*" — **it does not, and that word was the trap.** `frames_to_settle`
normalizes against *the segment's own last frame*, so when that frame is still
travelling the measured total is short and every threshold is crossed early. The
number that comes back is not pinned at the window length and is not obviously
wrong; it is a plausible, smaller frame count. Worse, the bias is uneven — the
0.9 threshold is pulled in harder than the 0.5 one — so a truncated fall also
reads as a *more even* fall than it is.

That is not hypothetical. Plan 0038 Phase 3 measured two easing orderings, one of
which had an effective time constant of 1.0 s against a 1.6 s window, and read the
truncation as a difference in the shape of the two falls. Measured to settlement
the two shapes are identical and differ only in speed — 73 frames against 145,
where the truncated run said 61 against 78. See
[ADR-0040](adrs/0040-spectrum-level-curve-applies-before-the-easing.md)'s Outcome.

**The rule, and the function that enforces it.** `frames_to_settle` cannot detect
this about itself: normalizing against the last frame *guarantees* the threshold
is crossed inside the segment, so `frames_to_settle(seg, f) < seg.len()` is a
tautology rather than a check. Before trusting a frame count, gate it on
`metrics::segment_settled(segment, tol)`, which extrapolates the geometric tail
from three points spread across the segment and answers whether the last frame is
within `tol` of the asymptote. Sample widely rather than from the end: captures
are 8-bit, and a response slow enough to outrun its window moves by *less than one
code value per frame* near the end, so adjacent frames decode as identical and
read as settled exactly when they are not.

**So `--report` marks rather than pretends** (Plan 0038 Phase 8). A transient cell
carries a **`+`** when `segment_settled` cannot certify the response arrived —
`61+` means *at least 61 frames*, never 61. Each family's table then names how
many of its presets marked. `--json` carries the same fact as `rise_settled` and
`fall_settled` booleans, so a consumer reading only the counts cannot mistake a
truncated response for a settled one.

**Expect most of the shipped set to mark, and for two different reasons the
suffix does not separate.** One is the window, above. The other is far more
common here and is not a defect in the probe at all: a scene whose own motion
never stops — a fragment field's fold, a feedback trail, a particle cloud — has
**no asymptote to settle to**, so `segment_settled` correctly declines to certify
one. That is the same limitation this page already describes as "the scene's own
motion is measured too"; the mark just moves it from a caveat you have to remember
into the cell itself. A preset whose cells are *unmarked* is the interesting case:
it means the number is a measurement.

Widening the `--report` window does not fix that table's *separation* problem,
which is a different thing — measured at 96 frames the scalar-only median got
**worse** (0.60 → 0.92) for double the wall clock. Scene saturation is what hides
the magnitude there. Window length is what corrupts a slow response's shape, and
the two are not the same defect.

#### A low `cover` is not a defect

`cover` counts pixels differing from the corner-sampled background by more than a
threshold, on any channel — a **symmetric** difference, so dark-on-light and
light-on-dark are measured identically. An ink-remapped look is not penalised by
construction, and a low reading is not evidence of one.

What a low `cover` means is that the frame is sparse, and sparse is often the
intent. `reaction_coral_bloom` reports **0.128**, about as low as the shipped set
goes, and is healthy — it is the family's ink-on-paper variant, a pale print whose
chaotic-branching regime genuinely covers an eighth of the frame. The number is
truthful; what it cannot tell you is "sparse on purpose" from "dead".

So the column **names suspects rather than convicting them**. A low `cover`
alongside a dead `anim` and flat reactivity columns is worth investigating; a low
`cover` on a preset that is deliberately a thin figure on a wide ground is the
report working.

#### The second reading: the same columns at realistic levels

Under each family's table is a second block
([ADR-0042](adrs/0042-reachability-measured-on-the-expression-tree.md)):

```
  at realistic levels (bass 0.661 mid 0.575 treb 0.281 onset 0.145) — read the *gap* ...
  preset            bass     mid    treb   onset   gates   ceils
  Aurora           0.212   0.199   0.228   0.048       0       0
  Ember            0.104   0.100   0.000   0.090       0       0
```

> **The realistic levels changed meaning in Plan 0048.** They are now fractions of
> each signal's own recent peak (ADR-0049), not magnitudes, which is why they read
> `0.661` where they used to read `0.04`. Any `--report` number quoted in an older
> commit message, ADR Outcome or backlog entry was measured on the raw scale.

The four columns are measured exactly as the ones above, from stimuli set to
[what real material produces](#what-real-material-actually-produces) instead of
to `1.0`. **The gap between the two rows is the reading, not either number
alone**, and it has a direction:

| what you see | what it means |
|---|---|
| both healthy | the preset responds at levels it will actually meet |
| **full scale lively, realistic ~0** | gained or gated against a magnitude music never reaches. This is the defect that hid six presets for months |
| realistic close to full scale | a compressive `curve`/`smoothstep` doing its job, or a binding already saturating at low input |
| **realistic > full scale** | something inverted or saturating — a parameter past its useful range at full scale, reading *back* down |

The last row is why the full-scale columns stayed. They also keep every number
quoted in an older commit, ADR or backlog entry meaning what it said.

Two things this pair cannot see. **`beat` is an event, not a magnitude**, so it
is `true` in both readings — a beat-latched binding holds across the gap while an
`onset`-scaled one falls away, which is a distinction, not a fault. And the
**band array is on its own scale**: the low stimulus lights the `spectrum` slice
to the same level as the scalar, while real material's per-band mean is `0.020`
with peaks to `0.338`, so a `bin()`-reading preset reads *lower* here than the
scalar gap suggests.

#### Reachability: gates the probe never drove both ways

The `gates` and `ceils` counts are not measured from pixels at all. They come
from walking each preset's **expression trees** while evaluating them over 12 s
of `dynamic:110` through the real analyzer, recording which way every
**comparison** and every `select()` condition went, and how close every `clamp()`
came to its upper bound. A frame differential structurally cannot answer this —
`select(c, 6, 8)` and `select(c, 6, 6)` diff identically, and neither names
*which* gate.

Three kinds of finding come out of that walk. The first two are named one per
line underneath the table; the third is summarized:

- **`GATE`** — a `select()` whose condition never went both ways. One branch of
  the preset has never rendered. Named with its source text, so the threshold to
  re-gain is in front of you.
- **`COMP`** — a **comparison** (`> < >= <= == !=`) that only ever took one
  value, so it read as a constant `0` or `1`
  ([ADR-0043](adrs/0043-reachability-reports-comparison-nodes.md)). This catches
  two shapes a `GATE` line cannot. One is the bare comparison as a whole
  binding — `reseed = "onset > 0.55"`, the idiomatic boolean-param form, which
  holds no `select()` at all. The other is one **half of a composite condition**:
  in `select(min(tempo > 124, bass + treb > 0.38), 4, 1)` the `GATE` line names
  the whole `min(...)`, and since a `tempo` gate is legitimately one-sided here
  (below), a reader would dismiss it — so each half is also reported on its own,
  and the excusable one can no longer launder the other.
- **clamp ceilings** — a `clamp()` upper bound the value never approached. The
  bound is decorative and the parameter's real range is narrower than it reads.
  These are **not** printed one per line: a single summary line per family gives
  the count and names the furthest three. All of them are in `--json`.

A comparison that is the **direct condition** of a `select()` reports once, as
the `GATE` line only — that line already names it and says which branch never
ran, which a `COMP` line cannot. So `GATE` and `COMP` never double-report the
same finding.

The `gates` column counts `GATE` + `COMP` together: both say a branch of the
preset's behavior has never happened. `ceils` counts the ceilings.

**A flag is a suspect, not a conviction.** It says *this* stimulus never drove
the gate both ways, which is a fact about the probe as much as about the preset.
The standing false positive is **`tempo`**: the probe runs at one BPM, so
`select(tempo > 132, ...)` is *correctly* one-sided and will flag forever. Check
those two by hand with `--set tempo=90` / `--set tempo=160` (see
[Examples](#examples)); a gate on a band is the one worth acting on.

The probe runs 12 s rather than the 4 s a `--signal` filmstrip synthesizes,
because the tempo tracker needs about 4 s to lock. Under a short clip `tempo`
reads a flat `0` and every `tempo` comparison flags for the wrong reason.

This is advisory output. It is **not** a CI gate, and deliberately so — and as of
Plan 0048 **both** of the reasons are live again.

The instrument is one of them, and that has not changed: the `tempo` single-BPM
false positive above accounts for 17 of the 26 flags the shipped set currently
produces, so a naive "fail if flags > 0" would fail CI permanently and a threshold
would be tuned to noise. The precondition remains a multi-BPM probe or an explicit
`tempo` exemption
([ADR-0043](adrs/0043-reachability-reports-comparison-nodes.md)).

The library is the other, and it regressed on purpose. Plan 0042's re-audit
measured **0 genuinely dead gates**, and that held until ADR-0049 changed what a
band level means. Nine bindings written against raw levels now compare against
normalized ones and never go false, so their `else` branches are dead. That is the
priced cost of the one-time retune ADR-0049 chose, the flags are catalogued in
[analysis-v2-before-flags.md](analysis-v2-before-flags.md), and clearing them is
Plan 0048's Phase 7. **Until that lands, a non-zero Group 2 count is expected
rather than news.**

### Which preset library a shot uses

Highest precedence first:

1. `--preset-file <path>` — one preset, parsed from that file.
2. `--presets <dir>` — every `*.toml` in that directory.
3. **`LMV_PRESET_DIR`** — the environment override
   ([ADR-0014](adrs/0014-preset-dir-override-for-dev-iteration.md)).
4. The per-user preset directory (`%APPDATA%\light-music-visualizer\presets` on
   Windows; see [`presets.md`](presets.md#where-preset-files-live)).
5. The presets compiled into the binary.

The `[source]` label printed after every capture names the winner, so a PNG's
provenance is never a guess. The two flags are **errors** when they come up empty
— a missing file, unparseable TOML, or a directory with no valid presets exits
non-zero rather than quietly capturing some other library. Levels 3–5 degrade
downward instead, exactly as the app does.

`shot` resolves levels 3 and 4 through the **same** `standalone` library function
the app calls, so the two can never disagree about which folder your edit landed
in.

### Editing presets live

Point both surfaces at the repo's version-controlled `presets/` and edit a
`.toml` — no rebuild, no relaunch:

```bash
# Windows (PowerShell): the app reloads the edited file within ~150 ms
$env:LMV_PRESET_DIR = "./presets"; cargo run -p standalone --release

# ...and every shot in that shell reads the same folder
cargo run -p standalone --example shot -- --preset "Aurora" --out shot.png
```

For a one-off capture, the flags say it explicitly and need no environment:

```bash
# The whole repo library
cargo run -p standalone --example shot -- --presets presets --preset "Aurora" --out a.png

# A single file — --preset is unnecessary, the one-entry library names itself
cargo run -p standalone --example shot -- --preset-file presets/fragment_aurora.toml --out a.png

# Metrics for the repo library rather than the seeded per-user copy
LMV_PRESET_DIR=./presets cargo run -p standalone --example shot -- --report
```

The app hot-reloads an override folder but **never seeds** into it (it is yours,
not ours), and `diagnostics.log` / `config.toml` stay under the per-user app
directory. The foobar2000 plugin does not read `LMV_PRESET_DIR`.

### Examples

```bash
# Shot a preset under a loud beat, at a custom size
cargo run -p standalone --example shot -- --preset "Pulse Field" \
  --set bass=1,onset=1,beat=1 --size 960x540 --out pulse.png

# Both sides of a tempo gate, e.g. `select(tempo > 130, ..., ...)`
cargo run -p standalone --example shot -- --preset-file presets/rose_zoom.toml \
  --set tempo=90 --out slow.png
cargo run -p standalone --example shot -- --preset-file presets/rose_zoom.toml \
  --set tempo=160 --out fast.png

# Labeled contact sheet of the whole library
cargo run -p standalone --example shot -- --all --out gallery/

# Metrics report as a text table, or JSON for parsing
cargo run -p standalone --example shot -- --report
cargo run -p standalone --example shot -- --report --json > report.json

# Beat filmstrip from a synthesized click track (no asset needed)
cargo run -p standalone --example shot -- --preset "Pulse Field" \
  --signal click:120 --strip 8 --out click.png

# ...or from the one synthesized kind with dynamics
cargo run -p standalone --example shot -- --preset "Pulse Field" \
  --signal dynamic:110 --strip 8 --out groove.png

# Filmstrip from a real clip (16-bit PCM WAV)
cargo run -p standalone --example shot -- --preset "Burst" \
  --audio assets/test/clip.wav --strip 8 --out clip.png
```

`--signal` kinds: `click:<bpm>`, `bass:<hz>`, `treble:<hz>`, `noise:<seed>`,
`chord`, `dynamic:<bpm>`. The synth path needs no committed asset. `--audio`
reads uncompressed 16-bit PCM WAV only (a hand-rolled reader — no decoder
dependency); other encodings are a followup.

#### `dynamic:<bpm>` — the one kind that rises and falls

Every other kind is a **steady** tone or steady noise, and the band report says
so: `bass:60` reads min/mean/max `0.187 / 0.187 / 0.187` — zero variance — and
`chord` `0.058 / 0.059 / 0.060`. A filmstrip of those exercises the DSP with
material that never changes, which is not what any preset is authored against.
`click:<bpm>` has real transients but peaks at `bass ≈ 0.011`, far below anything
a shipped preset is gained for.

`dynamic:<bpm>` is three layers on a beat grid — a pitch-dropping kick every
beat (bass), eighth-note hats (treble), a harmonic pad that swells across each
beat (mid) — under an **8-beat phrase** that builds for six beats and rests for
two. Measured at 110 BPM through the real analyzer:

| band | min | mean | max | `max / mean` |
|---|---|---|---|---|
| bass | 0.0035 | 0.0399 | 0.1063 | **2.67** |
| mid | 0.0005 | 0.0062 | 0.0189 | **3.07** |
| treb | 0.0000 | 0.0059 | 0.0320 | **5.45** |

against `noise:<seed>`'s 1.78 / 1.15 / 1.07 — and it was the liveliest kind there
was. Like every generator here it is a pure function of its arguments, so a
filmstrip of it is reproducible.

Those are **raw** magnitudes, so since ADR-0049 they describe `bass_raw` /
`mid_raw` / `treb_raw`. Through the normalizers the same clip reads:

| variable | min | mean | max |
|---|---|---|---|
| `bass` | 0.035 | 0.661 | 1.000 |
| `mid` | 0.031 | 0.575 | 1.000 |
| `treb` | 0.002 | 0.281 | 1.000 |
| `onset` | 0.001 | 0.145 | 1.000 |

The crest factors above are what normalization *preserves* — it divides by a
slowly-moving peak, so a clip's dynamics survive while its absolute level does
not. Note every variable reaches `1.000`: full scale is a state real material
visits, not a corner.

> **It exercises dynamics. It is not evidence about real loopback levels.** A
> preset that looks right under `dynamic:110` is a preset that survives material
> which rises and falls — that is all this says. Nothing synthesized can tell you
> whether your gains match what your music actually produces; only `--audio` on
> real material does — see [the reference range](#what-real-material-actually-produces)
> below. Do not read a lively filmstrip as a calibration check.

#### What real material actually produces

Measured 2026-07-27 through `--audio` on three local clips, none committed (see
the note above about `assets/test/`). All three were peak-normalized to −1 dBFS
first, because two of them arrived 20–26 dB under-levelled and every band read
zero — a level problem in the file, not a fact about the music.

> **These are raw magnitudes, so they now describe `bass_raw` / `mid_raw` /
> `treb_raw`** (ADR-0049). They used to be "the numbers to calibrate a gain
> against", and for the normalized `bass` / `mid` / `treb` they no longer are —
> that is the entire point of normalizing. Calibrate those against the `0–1` table
> in [presets.md](presets.md#set-the-threshold-from-a-measured-level-not-from---set)
> and they will hold on material like this without re-tuning. This section is now
> the reference for the **`*_raw`** variables, and for understanding *why* the
> normalized ones exist: look at how far apart the three rows below are.

| material | RMS | bass min / mean / max | mid | treb |
|---|---|---|---|---|
| electric-guitar loop, ~101 BPM, no drums | −17.7 dBFS | 0.000 / 0.000 / 0.004 | 0.000 / 0.002 / 0.013 | 0.000 / 0.000 / 0.000 |
| hi-hat percussion loop, ~102 BPM | −21.1 dBFS | 0.000 / 0.000 / 0.001 | 0.000 / 0.000 / 0.002 | 0.000 / 0.002 / 0.011 |
| trap with 808 sub, ~140 BPM | −19.9 dBFS | 0.000 / 0.007 / 0.190 | 0.000 / 0.001 / 0.026 | 0.000 / 0.001 / 0.006 |

**The shape of it matters more than any single number.** The 808's bass *peak*
(`0.190`) sits right on a full-scale 60 Hz sine (`0.187`) — the analyzer is not
quietly attenuating anything. Its *mean* is `0.007`, about 25× lower, because
real material is transient and spectrally sparse in a way no steady generator is.
Everything else here reads lower still: a guitar loop with no drums puts
essentially nothing in bass or treble, which is correct and is what most material
does in most bands most of the time.

So, in descending order of how far a stimulus is from real music — **on the raw
scale, i.e. what `bass_raw` sees**:

| stimulus | `bass_raw` it produces | vs. a real mean |
|---|---|---|
| `--set bass_raw=0.8` | `0.800` | **~100×** too hot |
| `--signal bass:60` (full-scale sine) | `0.187` | ~25× too hot |
| `--signal dynamic:110` | mean `0.040`, max `0.106` | ~6× too hot, right order for peaks |
| real music (above) | mean `0.000`–`0.007`, max up to `0.190` | — |

**This ladder is exactly what ADR-0049 abolished for the normalized variables.**
Its four rows span three orders of magnitude, and picking a threshold meant
knowing which rung you were standing on — which is why nine shipped mechanisms sat
dead for months. On the normalized scale every rung that carries real dynamics
lands in the same `0–1` range, so `bass > 0.8` means "near this material's own
peak" whether the material is an 808 at −1 dBFS or a quiet guitar loop. The ladder
survives here because `*_raw` still climbs it.

Two practical consequences, both of which still apply to `*_raw` and to `bin()`:

- **Calibrate against a mean, not a peak, for anything continuous** — a size, a
  zoom, a hue drift. Those spend their life near the mean, so a gain tuned to look
  right at `0.19` barely moves.
- **Calibrate against a peak for anything percussive** — a flash, a burst, a
  beat-latched accent. Those exist to fire on the hit, and the hit really does
  reach a full-scale tone's level.

The shipped library predates this measurement and is gained against the older
figures; whether it needs a re-gain pass is
[design-backlog 0020](design-backlog.md), not something to fix preset-by-preset.

> **Test audio is added manually and never committed.** Drop a 16-bit PCM WAV
> into [`assets/test/`](../assets/test/) — that folder is gitignored (only its
> README is tracked), so no licensed audio lands in the repo. Use your own or a
> royalty-free / CC0 clip; factory-library samples are fine to point at on disk
> but must not be committed. The `--signal` path needs no file, so the whole
> audio pipeline can be validated without adding anything.

The `--report --json` schema is a nested object of numbers keyed by
family/preset: per-band `reactivity` and `reactivity_low`, `animation`,
`coverage`, `transient` (`rise_frames` / `fall_frames` as integers plus their
`ratio`), `reachability`, the pairwise `pixel`/`shape` distinctness matrices, and
`near_duplicates`.

`reachability` carries `dead_branches` and `unapproached_ceilings` counts, the
full `gates` list (each with `param`, `source`, `kind`, and either `always` or
`peak_fraction_of_bound`), and a `probe` object naming the signal, BPM and
duration they were observed under. `kind` is `"select"`, `"compare"` or
`"clamp"` — matching the `GATE` / `COMP` / `CEIL` lines above — and
`dead_branches` counts the first two together. Keep the provenance when you
consume it: a flag only ever means *not observed under this stimulus*.

## The `core/tests/` harness

Most differential tests render on the **software adapter** (`prefer_software`) so
they hold on any GPU; the exceptions say so below. Run the whole suite:

```bash
cargo nextest run -p lmv-core     # what CI runs (per-test process isolation)
cargo test -p lmv-core            # single binaries only — see the two caveats below
```

> **Use `nextest` for the whole suite**, for two independent reasons.
>
> `preset`'s zero-allocation assertion
> counts allocations through a process-global allocator hook, so it is only
> reliable under nextest's per-test process isolation — under stock `cargo test`
> a concurrently-running test's allocations bleed into the count.
>
> And **stock `cargo test` runs a binary's tests as threads in one process, which
> the GPU tests do not survive.** Several of them build and drop a `Renderer` (and
> so a wgpu device) concurrently, and the driver aborts the process with
> `STATUS_ACCESS_VIOLATION` — `--test transition` every run on WARP, `--lib`
> intermittently at teardown, `--lib render::post` since Plan 0035. **This is a
> runner artifact, not a failing test**: the same binaries pass in full under
> `cargo nextest run`, which gives each test its own process. If a `cargo test`
> invocation aborts with `0xc0000005`, re-run it under nextest (or
> `-- --test-threads=1`) before concluding anything about coverage.
>
> Tests that need
> a real GPU (`background_composite`, and the in-crate dual-live dissolve check in
> `render/mod.rs`) **skip themselves** when only a software rasterizer is present,
> per [ADR-0016](adrs/0016-gpu-tests-opt-in-ci-scope.md). WARP mis-renders both:
> the fullscreen-scene + background pipeline set, and — once a dissolve allocates
> its blend targets mid-run — what the feedback `trails` stage resolves to.

Individual tests (add `-- --nocapture` to see the printed diagnostics):

| test | kind | asserts |
|------|------|---------|
| `reactivity` | HARD | every preset moves for at least one band (bass/mid/treb/onset); prints the per-band vector so a dead single binding — e.g. treble — is visible |
| `animation` | HARD | every preset changes between frame N and N+k at fixed audio (not frozen) |
| `sanity` | HARD | every preset lights a minimum coverage and spans ≥2 quadrants **against its own background** (not blank, not a dot) |
| `beat` | HARD | a 120 BPM click track through the **real** DSP makes a beat-accent preset render differently on-beat vs off-beat; a zeroed beat binding does not |
| `distinctness` | ADVISORY | prints per-family pixel + shape pairwise matrices and flags near-duplicate geometry; never asserts |
| `golden` | HARD (tolerance) | one **frozen fixture per system** matches its committed baseline PNG within a mean + max-outlier tolerance ([ADR-0023](adrs/0023-golden-drift-guard-uses-frozen-fixtures.md)) |
| `composite` | HARD (tolerance) | the **post stages**, one fixture each and never all at once — `trails`, `kaleido_*`, `bloom_*`, plus one that binds **no** stage and guards the composite's *arithmetic* (its assertion is that no channel of *that fixture* reaches 255 — a claim about the fixture, not a general property of the curve; see the re-bless note below). Captured at **160x100**, a size whose internal grid is *not* the target's shape, so an aspect error is visible ([ADR-0037](adrs/0037-internal-grid-is-a-resolution-not-a-shape.md)) |
| `bloom` | HARD (relative) | the bloom stage's behaviour, beside its baseline rather than in it: halo **energy** rises with `bloom_amount`, halo **extent** rises with `bloom_radius`, the rich tier's deeper pyramid reaches further than the floor's, and the halo is **round**. Captured at **256x256** — square, and load-bearing: the roundness guard is what catches a separable kernel whose two passes step in different units, and it reads 1.001 today against 7.05 under the defect it was written for. No magic numbers: every assertion compares two captures of one fixture differing in one bound param |
| `reaction_diffusion` | HARD | the first stateful-feedback scene: seed reproducibility, regime response ([ADR-0012](adrs/0012-stateful-feedback-render-system.md)) |
| `attractor` | HARD | the first compute-particle scene: seed reproducibility + beat perturbation ([ADR-0015](adrs/0015-gpu-compute-particle-idiom.md)) |
| `line_joints` | HARD (+ tolerance) | a **flagged joint stops leaving a hole** in the stroke ([ADR-0041](adrs/0041-line-joins-are-per-endpoint-on-the-segment-instance.md)): against a purpose-built zigzag `polyline`, a vertex is not a local luminance minimum relative to the segment interiors either side of it. Threshold-free, and captured at **512x512** because the wedge it measures is a fraction of a stroke-width across. The same capture is then pinned to a committed baseline (Plan 0040), since the reported defect had no pixel guard anywhere; the relative claim runs **first, even under `LMV_BLESS`**, so the notch cannot be blessed back in. Bless with `--test line_joints`, which cannot reach the golden roster |
| `ink` | HARD | the final tone-remap **inverts** tone, and `ink_amount = 0` is byte-identical to an unbound frame ([ADR-0028](adrs/0028-final-stage-ink-tone-remap.md)) |
| `background_composite` | HARD (**hardware only**) | RD / attractor presents alpha-blend over the `bg_*` backdrop; **skipped** on a software adapter, which mis-renders that pipeline set |
| `transition` | HARD | every switch path (cycle **and** select) renders intermediate blended frames as a ramp, reproducibly from the injected `dt`; each blend kind shows its own signature; a switch arriving mid-dissolve lands on the last index requested; a hot-reload mid-dissolve cancels cleanly; the heavy attractor ↔ reaction-diffusion pair dissolves on the freeze fallback (set `LMV_TRANSITION_STRIP=<dir>` to also dump filmstrips) |
| `easing` | HARD | `[smoothing]` is observable: a scalar entry measures symmetric and an `{ attack, release }` pair does not, against purpose-built near-linear fixtures ([ADR-0039](adrs/0039-verify-easing-with-a-transient-probe-not-a-committed-clip.md)). Also measures the `spectrum` `curve`↔easing **ordering** both ways round through one renderer — **every** frame count in the suite is gated on `segment_settled` first — the shared probe's window is 180 frames (3 s, 6 τ) because at 96 its own asymmetric arm was truncated, reading 61 where the settled answer is 69 |
| `preset` | HARD | the expression evaluator and TOML schema: exact values, rejection without panic, **zero allocation** per eval, and the `PARAMS` ↔ `set_param` drift guard |
| `dsp` / `ffi` / `hygiene` | HARD | known-signal analysis fixtures; the C ABI across the boundary; the hot-path panic pragma + exact dependency pinning |

**Golden baselines pin frozen fixtures, not shipped presets.** `core/tests/fixtures/*.toml`
is a deliberately minimal preset per `SystemKind`, committed alongside
`core/tests/golden/*.png`; the shipped presets in `presets/` are guarded
*behaviorally* (`sanity` / `reactivity` / `animation`) so the `preset-author` lane can
tune them freely without re-blessing pixels. A new `SystemKind` variant fails
`golden.rs` to **compile** until its fixture exists (exhaustive match, no wildcard arm).

`core::signal` (pure, zero-dep) synthesizes the test audio; `core::render::metrics`
(pure) provides `frame_diff`, `struct_diff`, `coverage`, and `quadrant_spread`,
shared by the tests and the CLI report, plus the step-response pair
`frames_to_settle` / `step_response` and the `segment_settled` gate that says
whether either of those two is worth reading.

### Golden baselines

Golden baselines live in `core/tests/golden/*.png` and are ordinary PNGs
(viewable in the repo / PR diffs). To regenerate them after an intended visual
change:

```bash
LMV_BLESS=1 cargo test -p lmv-core --test golden
LMV_BLESS=1 cargo test -p lmv-core --test composite     # the post-stage baselines
LMV_BLESS=1 cargo test -p lmv-core --test line_joints   # the joined-polyline baseline
```

Only the first of those owns the per-`SystemKind` roster. The four
`composite_*.png` belong to the `composite` test and `line_joint_zigzag.png` to
`line_joints`; blessing by binary is what keeps the scopes from rewriting each
other. `line_joints` additionally refuses to bless at all while its
local-minimum claim is failing, so a reopened notch cannot be baselined in.

**Eyeball the regenerated PNGs before committing** — the first baseline is easy
to enshrine wrong. The compare tolerates minor cross-GPU rasterization drift; a
genuine change exceeds it.

> **Eyeballing the baseline is not enough on its own, and Plan 0045 is the
> record of why.** The whole suite captures on WARP, which is documented to hand
> a pipeline another live pipeline's resources
> ([ADR-0021](adrs/0021-shared-palette-baked-lut.md) / Plan 0020, the tonemap in
> Phase 3, the bloom blur in Phase 4). A mis-rendered frame at these capture
> sizes can look entirely plausible: Phase 4's bloom halo was 2:1 elongated in
> one draft and smeared into a column of copies in another, and the 160x100
> baseline looked like a reasonable glow under both. **Render the same fixture on
> the hardware adapter at a size large enough to see it** (`shot` uses the
> default adapter, so `cargo run -p standalone --example shot -- --preset-file
> <fixture> --size 512x512` is the check) **and confirm the two adapters agree
> before blessing.** Where they disagree, the hardware one is right and the
> baseline is about to enshrine a driver bug.

### The tonemap and pixel-level assertions (Plan 0045)

Every baseline in the repository was re-blessed once at Plan 0045 Phase 3, when
the composite became linear-light `Rgba16Float` with a tonemap at the surface
boundary. Two things follow for anything that reads pixels here:

- **A capture is downstream of a compressive curve.** The curve is the identity
  below ~0.6 and rolls off above it, so a low- or mid-luminance assertion reads
  what it always read, and a bright one reads lower than the linear value that
  produced it. `composite_overlap` pins that a frame of stacked additive strokes
  rolls off instead of flattening: no channel of *that fixture* reaches 255.
  **Do not read that as "255 is unreachable".** The curve is bounded strictly
  below 1 for every finite input, but the surface write encodes to sRGB and
  *rounds*, so `f(x)` crosses the last byte's midpoint at a linear input of about
  36 at the shipped knee — and `attractor.toml` reaches it on the hardware
  adapter. A suite-wide no-255 gate would fail on correct frames.
- **A backdrop makes a bright pixel's dim channels darker, and that is the curve
  working.** The roll-off scales all three channels by `f(m)/m` off the
  *brightest* one, so adding a red-dominant `bg_*` under a magenta stroke raises
  `m`, drops the scale, and takes blue down with it — measured at up to 15 bytes
  on `composite_bloom` with every post stage off. Any assertion of the form
  "compositing something underneath may only add light" therefore has to be made
  **upstream of the tonemap**, on the linear composite, where it is exact; see
  `a_backdrop_under_an_active_halo_only_ever_adds_light` in
  `core/src/render/bloom.rs` (Plan 0045 Phase 4b).
- **`--report` moved, slightly and measurably.** Re-run over the library at that
  change: reactivity max 0.060 / mean 0.012, animation max 0.042 / mean 0.006,
  coverage max 0.187 / mean 0.010, distinctness max 0.12, reachability
  identical everywhere and every floor still passing. Read that as the scale of
  drift a luminance-model change produces in these columns — not as noise, and
  not as something to re-derive without measuring.

> **`LMV_BLESS=1` is not scoped to the scene you changed** — it rewrites **every**
> baseline the run touches. `git status` after blessing and `git checkout` the
> baselines your change had no business moving; committing an incidental re-bless
> silently retires the drift guard for that scene. (Learned the hard way in Plan
> 0027, where an over-broad bless moved `fragment_field` and `swarm`.)

## The habit for a new scene

When you add a new scene or preset:

1. **Eyeball it first** — `--preset <name> --out /tmp/new.png` and Read the PNG.
2. **Add the differential cases** — the `reactivity`, `animation`, and `sanity`
   tests iterate the embedded presets automatically, so a new *preset* is covered
   once it's in the default set; a new *system* may need its per-system floor in
   `sanity`. If it's beat-driven, extend `beat`.
3. **Check distinctness** — run `--report` (or the `distinctness` test) to see if
   the new preset is a near-duplicate of an existing one (advisory).
4. **A new *system* needs a golden fixture; a new *preset* does not.** Adding a
   `SystemKind` variant fails `golden.rs` to compile until you author
   `core/tests/fixtures/<system_name>.toml` and add its arm — then bless that one
   baseline after eyeballing it. Shipped presets are never pixel-pinned
   ([ADR-0023](adrs/0023-golden-drift-guard-uses-frozen-fixtures.md)).
