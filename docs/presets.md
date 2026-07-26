# Preset authoring guide & expression reference

A **preset** is a small text file that describes one visual: it names a built-in
rendering **system** and binds each of that system's **parameters** to a short
**expression** over the live audio analysis. Editing a preset needs no Rust, no
rebuild, and no shader knowledge — you change a line, save, and the running app
picks it up.

This document is the reference for the **expression language** — the vocabulary
you write on the right-hand side of every binding — plus how presets load, where
they live, and how a mistake is reported. It is the authoring counterpart to
[ADR-0002](adrs/0002-layered-preset-architecture.md) (the layered preset
architecture) and [ADR-0020](adrs/0020-preset-grammar-v2-branching-functions-tempo.md)
(the v2 grammar). Only **layers 1-2** of ADR-0002 exist today — TOML data presets
over a pure expression language. Layer 3 (Rhai scripting) and cross-preset
blending are deferred.

**The per-system parameter tables live in [`presets/README.md`](../presets/README.md)**,
next to the preset files themselves. That is the one place they are maintained;
this document does not duplicate them.

> **Accurate as of 2026-07-25**, against the 35-preset curated set across seven
> systems and the v2 expression grammar (Plan 0019).

---

## Quickstart: your first preset

1. **Find your preset directory** (see [Where preset files live](#where-preset-files-live)).
   On Windows that is `%APPDATA%\light-music-visualizer\presets`. Both the
   standalone app and the foobar2000 plugin read this same folder — it is seeded
   with the curated set on first run.

2. **Copy an existing preset** as a starting point. `swarm_flow.toml` (a calm
   particle swarm) and `fragment_aurora.toml` (a slow warp field) are the
   friendliest bases:

   ```
   copy swarm_flow.toml   my_first.toml
   ```

3. **Edit the bindings.** Open `my_first.toml` and change an expression — for
   example make the beat kick harder:

   ```toml
   system = "swarm"
   name   = "My First"

   [params]
   force      = "1.2 + clamp(bass * 20, 0, 4)"
   spin       = "0.3 + clamp(mid * 6, 0, 1.2)"
   burst      = "beat * 9"          # was beat * 5 — a bigger blast on each beat
   hue        = "time * 0.02 + clamp(treb * 4, 0, 1)"
   brightness = "0.6 + clamp(bass * 6, 0, 0.8)"
   size       = "1.0 + beat * 0.6"
   ```

4. **Save and watch.** The standalone app polls the folder every ~150 ms and
   hot-reloads on any change (no restart). Press **Space** to cycle to your new
   preset; the window title shows the active preset name and system. If the file
   has a typo, the app reports it and keeps the last good set — it never crashes
   on a bad preset.

That is the whole loop: copy, edit an expression, save, cycle.

---

## Anatomy of a preset file

A preset is a TOML file with a two-line header and a `[params]` table:

```toml
system = "fragment_field"   # required — which built-in system to drive
name   = "Aurora"           # optional — display name (defaults to the system name)

[params]                    # each key is a system parameter; each value is an expression string
warp  = "0.3 + clamp(bass * 14, 0, 1.8)"
hue   = "time * 0.03 + clamp(treb * 5, 0, 1)"
zoom  = "1.0 + bar * 0.25"
glow  = "0.4 + clamp((bass + mid) * 8, 0, 1.1)"
flash = "clamp(onset * 3, 0, 1)"
```

Rules:

- **`system`** must be one of the seven known system names (below). An unknown
  system rejects the whole file.
- **`name`** is free text shown in the standalone title bar. If omitted, the
  system name is used.
- **`[params]`** binds parameters by name to expression strings. Every value is
  a **string** (quote it), even a bare number: `warp = "0.4"`, not `warp = 0.4`.
- **Unbound parameters** fall back to the system's default, so you only need to
  write the parameters you want to drive. Order does not matter — bindings are
  sorted by name at load for determinism.
- **An unknown parameter name is a load-time warning**, not an error — see
  [When a preset is wrong](#when-a-preset-is-wrong).

Beyond `[params]`, a preset may carry optional tables — `[curve]` / `[generator]`
(structural config for the line systems), `[particles]` (attractor family),
`[smoothing]` (per-parameter easing), and `[palette]` / `[palette_b]` (colour).
All are documented in [`presets/README.md`](../presets/README.md) and
[`docs/preset-palettes.md`](preset-palettes.md).

Every `[params]` value is evaluated **once per frame** and applied to the system
before it renders. There is no per-frame state you can accumulate in a preset —
an expression is a pure function of the current analysis frame plus the clock.

---

## The built-in systems

Seven systems are addressable from a preset. Their **named parameters, defaults,
and per-system notes are tabulated in [`presets/README.md`](../presets/README.md#systems-and-their-named-parameters)** —
that table is maintained alongside the presets and is the authoritative list.

| `system = ` | What it draws | Curated presets |
|-------------|---------------|-----------------|
| `fragment_field` | A fullscreen domain-warped light field (fragment shader). | 7 |
| `swarm` | ~10k CPU-simulated particles on an evolving flow field. | 5 |
| `parametric_curve` | A sampled line curve — the Maurer rose (ADR-0007). | 11 |
| `lsystem` | An L-system turtle figure, precomputed per depth (ADR-0007). | 2 |
| `star_pattern` | A Hankin star pattern over a regular tiling (ADR-0007). | 1 |
| `reaction_diffusion` | A Gray-Scott reaction-diffusion field (ADR-0012). | 4 |
| `attractor` | GPU compute particles iterating a strange attractor (ADR-0015). | 5 |

Beyond a system's own parameters, **every** preset may also bind the engine-wide
compositing controls — the shared view transform (`zoom`, `pan_x`, `pan_y`), the
background pass (`bg_*`), feedback `trails`, the screen-space kaleidoscope
(`kaleido_*`), and the final ink-on-paper remap (`ink_*` / `paper_*`). Those are
documented under [Engine-wide controls](../presets/README.md#engine-wide-controls-plan-0018).

They run in a fixed order, which is worth knowing when a look does not compose the
way you expect:

```
background -> scene -> post chain (trails -> kaleidoscope) -> [transition blend] -> ink -> present
```

Everything up to and including the post chain is **per preset** — during a dissolve
each side composites its own, independently. The blend and the ink remap are
**engine-wide**: one pass each, over the frame both presets produced.

### Transitions between presets

A preset switch dissolves rather than cuts (ADR-0024 / ADR-0032). Nothing about it
is preset-authored today — duration and blend kind are engine policy in code — but
two consequences reach your file:

- **Your preset renders live from the dissolve's second frame.** It is composited
  through its own background + post chain the whole way, so a `trails` accumulation
  starts from empty at the switch, not from the outgoing preset's history.
- **`ink_*` / `paper_*` crossfade.** There is one ink pass for the blended frame, so
  its params travel from the outgoing preset's values to yours across the dissolve.
  A preset whose look *depends* on ink reaches it about a second after the switch.

Per-preset `[transition]` declarations are a deliberate follow-up, not a gap you can
work around from the TOML.

---

## The expression language

Each parameter value is a tiny arithmetic expression, compiled once when the
preset loads and evaluated every frame. It is deliberately small: pure,
allocation-free, and **total** (evaluation never panics), so it is safe to run
per parameter per frame during a live show.

### Grammar

```text
expr   := sum  (('>' | '<' | '>=' | '<=' | '==' | '!=') sum)*
sum    := term  (('+' | '-') term)*
term   := unary (('*' | '/') unary)*
unary  := ('-' | '+')? primary
primary:= number | ident | ident '(' expr (',' expr)* ')' | '(' expr ')'
```

- **Arithmetic:** `+` `-` `*` `/`, unary `-` / `+`, and parentheses `( )`, with
  the usual precedence.
- **Comparisons** sit at the **lowest** precedence, below `+`/`-`, so
  `1 + 1 > 3 - 2` reads as `(1 + 1) > (3 - 2)`.
- **Numbers:** decimal `f32` literals (`0.3`, `14`, `1.8`).

There is no randomness and no way to read wall-clock time — the only clock is
`time`, the renderer's shared scene clock, so a preset is reproducible given the
same audio.

### Variables

Nine read-only variables carry the live audio analysis into your expressions:

| Variable | Meaning | Notes |
|----------|---------|-------|
| `bass` | Mean magnitude in the bass band (~20–250 Hz). | **Raw and small** — multiply up (e.g. `bass * 14`) and clamp. |
| `mid`  | Mean magnitude in the mid band (~250–4000 Hz). | Same scale caveat as `bass`. |
| `treb` | Mean magnitude in the treble band (~4–18 kHz). | Same scale caveat; treble reads smallest of the three. |
| `onset` | Spectral-flux onset envelope for this hop. | A transient/attack strength, not a level — spikes on hits. |
| `beat` | `1.0` on a hop where a beat fired, else `0.0`. | A gate: `beat * k` adds `k` only on beat frames. |
| `bar` | Beat phase in `[0, 1)`: `0` on each beat, ramping to the next. | A sawtooth that "breathes" between beats. |
| `time` | The scene clock in seconds (monotonic). | Use `time * k` for slow drift; `k` sets the speed. |
| `tempo` | Tracked tempo in **BPM**. | **Not a `0–1` band** — see the warning below. |
| `novelty` | Spectral-change transient: ~`0` within a steady segment, spiking at a track/section boundary. | **Experimental** — see below. |

The band values (`bass`/`mid`/`treb`) are raw mean magnitudes normalized so a
full-scale sine reads near `1.0`, but real program material reads far lower — so
curated presets consistently apply their own gain and then clamp to a bounded
range. That is the central idiom (below).

> **`tempo` is the one variable that is not roughly `0–1`.** It is `0` until the
> tempo tracker warms up (the first several seconds of audio), then jumps to a
> real BPM in the ~`60–200` range. Using it raw will blow out any parameter.
> Either **scale** it (`tempo / 180`) or — better — **compare** it:
> ```
> select(tempo > 128, 2.5, 0.8)     # a fast look above 128 BPM, a calm one below
> ```

> **`novelty` is experimental.** It is a transient that spikes when the spectrum
> changes character — useful for gating an accent on a track or section change
> (`beat * (novelty > 0.5)`). Its DSP shape may change in a later release, or it
> may be withdrawn; do not build a preset that only works with today's exact
> values.

### Constants

| Constant | Value |
|----------|-------|
| `pi` | 3.14159265… |
| `tau` | 2π — a full turn, the natural unit for a cyclic `hue` or `rotation`. |

Constants resolve before the variable lookup, so they cannot be shadowed. A bare
identifier that is neither a constant nor a variable is a compile error.

### Functions

| Function | Args | Result |
|----------|------|--------|
| `sin(x)` | 1 | Sine of `x` (radians). |
| `cos(x)` | 1 | Cosine of `x` (radians). |
| `abs(x)` | 1 | Absolute value. |
| `floor(x)` | 1 | Largest integer ≤ `x`. |
| `sqrt(x)` | 1 | Square root. `sqrt` of a negative is `NaN` — guard it with `select` or `max(x, 0)`. |
| `min(a, b)` | 2 | Smaller of `a`, `b`. |
| `max(a, b)` | 2 | Larger of `a`, `b`. |
| `pow(base, exp)` | 2 | `base` raised to `exp`. Shape a response curve: `pow(bass * 8, 2)` is punchier, `pow(x, 0.5)` gentler. |
| `mod(a, b)` | 2 | **Floored** modulo, `a - b * floor(a / b)`. Divisor-signed, so `mod(-0.2, 1.0)` is `0.8` — it wraps cleanly for a cyclic hue or phase. |
| `clamp(x, lo, hi)` | 3 | `x` bounded to `[lo, hi]`. Total even if `lo > hi`. |
| `lerp(a, b, t)` | 3 | Linear blend `a + (b - a) * t`. |
| `smoothstep(e0, e1, x)` | 3 | Eased `0 → 1` ramp as `x` crosses `e0 → e1` (`0` below, `1` above). The easing primitive — smoother than `clamp` for a threshold. |
| `select(cond, x, y)` | 3 | `x` if `cond != 0.0`, else `y`. **Only the taken branch is evaluated.** |

Calling a function with the wrong number of arguments, or referencing an unknown
name, is a **compile error** — the preset is rejected at load and the app keeps
the previous good set (it does not crash). Division by zero yields `inf`/`NaN`
rather than panicking, but you should avoid it — a `NaN` parameter produces
undefined-looking visuals.

### Comparisons and branching

The six comparison operators — `>` `<` `>=` `<=` `==` `!=` — each yield exactly
`1.0` (true) or `0.0` (false), so they compose with arithmetic:

```
0.4 + (bass > 0.2) * 0.3        # 0.4 normally, 0.7 once the bass crosses 0.2
```

`select(cond, x, y)` picks between two whole expressions. Because it evaluates
**only** the branch it takes, it is also the way to guard a partial function:

```
select(bass > 0.5, 3.0, 0.8)          # a threshold switch
select(x >= 0, sqrt(x), 0)            # safe — the untaken sqrt never runs
```

That last property is why `select` exists rather than a `lerp`-based blend: a
blend evaluates both sides, so an out-of-domain `sqrt` would poison the result
with `NaN` even on the branch you did not want.

**There are no boolean operators** (`&&`, `||`, `!`) — with clean `0`/`1`
comparison results they add nothing:

| You want | Write |
|----------|-------|
| `a AND b` | `min(a, b)` |
| `a OR b` | `max(a, b)` |
| `NOT c` | `1 - c` |

```
min(bass > 0.3, tempo > 120)          # loud AND fast
max(beat, onset > 0.6)                # on a beat OR a strong transient
```

**Chained comparisons are legal but rarely what you mean.** `a > b > c` parses
left-associatively as `(a > b) > c`, comparing a `0`/`1` against `c`. Write
`min(a > b, b > c)` instead.

### Idioms (patterns from the curated set)

- **Gain-then-bound** — turn a small raw band into a usable range:
  ```
  clamp(bass * 14, 0, 1.8)
  ```
  Multiply the raw band up, then clamp so a loud passage can't blow the parameter
  out. Nearly every reactive binding is a variant of this.

- **Baseline + reactive** — a resting value plus an audio-driven add:
  ```
  0.4 + clamp((bass + mid) * 8, 0, 1.1)
  ```
  The constant is what you see in silence; the clamped term is the reaction.

- **Slow drift** — a parameter that wanders on its own:
  ```
  time * 0.03
  ```
  Small coefficients (`0.008`–`0.08` in the library) set how fast a hue rotates.

- **Beat gate** — add something only on beat frames:
  ```
  0.5 + beat * 0.8
  ```
  `beat` is `0` most frames and `1` on a beat, so this jumps by `0.8` on each beat.

- **Beat-phase breathing** — a smooth ramp between beats:
  ```
  1.0 + bar * 0.25
  ```
  `bar` sweeps `0 → 1` between beats, so `zoom` eases up and resets each beat.

- **Soft threshold** — `smoothstep` where `clamp` would snap:
  ```
  smoothstep(0.15, 0.45, bass)
  ```
  A `0 → 1` ramp that eases in and out instead of cornering.

- **Cyclic wrap** — keep a rotating value in one turn:
  ```
  mod(time * 0.2, 1.0)
  ```
  Floored `mod` never returns a negative, so a hue driven this way never jumps.

- **Two-mode preset** — one file that behaves differently by energy or tempo:
  ```
  select(tempo > 130, 1 + bass * 3, 1 + bass * 0.6)
  ```

### What an expression cannot do: shape a value *over time*

`smoothstep` eases a **value** as its input crosses a threshold. It cannot ease a
**trajectory**, because expressions are pure and stateless by hard invariant —
there is no previous frame to ease away from. Anything time-shaped therefore
lives in `[smoothing]`, not in the expression:

```toml
[params]
thickness = "0.9 + beat * 0.6"           # the target: snaps on every beat

[smoothing]
thickness = { attack = 0.02, release = 0.7 }   # the trajectory: hit, then glide
```

`attack` is the time constant while the value is **rising**, `release` while it is
falling or level; a plain `thickness = 0.3` means the same constant both ways.
That is what makes a percussive accent land in a frame or two and then decay over
most of a second — a single constant slows the rise exactly as much as the fall,
so there is no value that gives both.

> **A two-constant entry stops being a low-pass and becomes a rectifier.**
> Because rise and fall are treated differently, a symmetric input comes out with
> a DC offset: the parameter **rides above its input's mean** under sustained
> material. On a percussive accent that is the point. On a continuous parameter
> it is a surprise — a fast-attack `hue` drifts upward instead of tracking. Use
> the pair where you want a hit; leave anything continuous symmetric. Full table
> reference: [`presets/README.md`](../presets/README.md).

---

## When a preset is wrong

The engine distinguishes mistakes that make a preset meaningless from mistakes
that merely waste a line. Neither ever crashes a running visual (NFR 10).

**Hard errors — the file is rejected, the last good set is kept:**

- Malformed TOML.
- An unknown `system` name.
- An expression that fails to compile — an unknown identifier, a bad number, a
  wrong argument count, an unbalanced parenthesis, a stray character.
- An invalid structural table (`[curve]`, `[generator]`, `[particles]`,
  `[palette]`, `[smoothing]`).

**Warnings — the preset still loads and renders:**

- **An unknown parameter name.** A binding whose name no system or engine stage
  consumes is reported at load, naming the parameter and the system, and the rest
  of the preset applies normally (ADR-0020):

  ```
  preset ...\my_first.toml: warning: unknown parameter 'wrap' for system 'fragment_field' (binding kept, but nothing reads it)
  ```

  This is why a typo no longer fails silently. It is a warning rather than a
  rejection on purpose: one mistyped character should not blank a scene
  mid-show, so the good bindings keep working while the mistake is surfaced.
  The standalone prints both errors and warnings to stderr on every load and
  hot-reload.

---

## Where preset files live

There are three copies of the curated set, and understanding the flow explains
why "edit once, both frontends see it" works.

```
  presets/*.toml                 core/build.rs -> EMBEDDED             per-user preset dir
  (repo, source of truth)  ──>   (globbed + include_str!'d      ──>    seeded on first run,
                                  into the binary)                     then loaded + watched
```

1. **`presets/` at the repo root — the source of truth.** These `.toml` files are
   what a contributor edits. Nothing reads them at runtime directly.

2. **`EMBEDDED` — generated at build time.** `core/build.rs` globs `presets/*.toml`
   and emits an `EMBEDDED` slice of `(filename, contents)` tuples, each
   `include_str!`'d, so the compiled binary always carries the curated set
   ([ADR-0022](adrs/0022-build-time-preset-embedding.md)). `default_presets()`
   parses these as the fallback the C-ABI / foobar path renders even with no
   preset directory present. **Adding a preset is dropping a file** — there is no
   list to edit and no count to bump.

3. **The per-user directory — what actually gets loaded.** On first run each
   frontend **seeds** this directory (writes every embedded preset that isn't
   already there — **never overwriting** your edits) and then loads and watches
   it. The standalone and the foobar plugin resolve the **same** path, so a
   preset you edit shows up in both.

   | OS | Preset directory |
   |----|------------------|
   | Windows | `%APPDATA%\light-music-visualizer\presets` |
   | macOS | `~/Library/Application Support/light-music-visualizer/presets` |
   | Linux/other | `$XDG_DATA_HOME/light-music-visualizer/presets` (or `~/.local/share/light-music-visualizer/presets`) |

### A custom preset folder: `LMV_PRESET_DIR`

Set the **`LMV_PRESET_DIR`** environment variable to point the Rust frontends at
any folder instead of the per-user directory above
([ADR-0014](adrs/0014-preset-dir-override-for-dev-iteration.md)):

```bash
# Windows (PowerShell) — run the app against a folder you keep elsewhere
$env:LMV_PRESET_DIR = "D:\my-presets"; cargo run -p standalone --release

# macOS / Linux
LMV_PRESET_DIR=~/my-presets cargo run -p standalone --release
```

- Both the **standalone app** and the headless **`shot` CLI** honor it, through
  one shared resolver — they cannot disagree about which folder an edit lands in.
- The override folder is **yours**: the app loads and hot-reloads it but **never
  seeds** the curated set into it. An empty or missing folder simply falls back
  to the presets compiled into the binary, exactly like an empty per-user
  directory.
- Only the presets move. `diagnostics.log` and `config.toml` stay under the
  per-user app directory.
- The **foobar2000 plugin does not read it** — the C++ side keeps resolving the
  per-user directory (a followup, not a current behavior).

Pointing it at the repo's own `presets/` is the preset-authoring loop: edit a
version-controlled `.toml` and the running window follows within ~150 ms, with no
rebuild. See [`capturing.md`](capturing.md#editing-presets-live) for that loop and
for `shot`'s equivalent `--presets` / `--preset-file` flags.

### Loading, cycling, and hot-reload

- **Seeding is write-if-absent.** Your edits to a seeded preset survive
  re-seeding. The flip side: a curated preset changed in a **new release** does
  **not** replace the copy already on your disk — delete that file and relaunch
  to get the updated version (there is no "refresh curated" button yet).
- **Hot-reload (standalone).** The app polls the directory ~every 150 ms and
  reloads on any change. Errors and warnings are printed and the last good set is
  kept — a bad edit never crashes a running visual.
- **Cycling.** Standalone: the app **holds one scene by default** (ADR-0027) —
  **Space** cycles to the next preset (title bar shows the name), and **`A`**
  toggles auto-rotate on/off (auto is off out of the box; enable it per-run with
  `A` or persistently via `auto = true` under `[rotate]` in `config.toml`).
  foobar2000: **Space**, or right-click the visualization → **Next scene**.
- **foobar loads on init.** The plugin calls the core's `lmv_load_presets` (C ABI
  v2, [ADR-0006](adrs/0006-c-abi-v2-preset-loading.md)) against the shared
  directory when it starts, so it seeds and renders the same library — no
  loopback capture needed on that path.

---

## Keeping this current

**Adding, renaming, or retiring a preset touches two places:**

1. **`presets/<name>.toml`** — the preset file itself. That is all the *shipping*
   takes: `core/build.rs` globs the directory, so the embedded list and the
   preset-count test follow automatically (ADR-0022). There is no array to extend
   and no count to bump.
2. **[`presets/README.md`](../presets/README.md)** — if the preset showcases a
   control worth pointing an author at.

**Adding a parameter to a system** touches the scene's `set_param` match, the
`PARAMS` const beside it (the two are guarded against drift by
`declared_params_match_set_param` in `core/tests/preset.rs`), and the table in
[`presets/README.md`](../presets/README.md).

**Adding an expression variable, function, or operator** touches
`core/src/preset/expr.rs` and [The expression language](#the-expression-language)
in this file. A change to the grammar is **ADR territory** (ADR-0002 fixed the
model; ADR-0020 grew it to v2) — flag it rather than quietly widening the
vocabulary here.

> **Format stability:** the app is pre-1.0 and in active development, so the
> preset format may still change between releases. Preset-format stability begins
> at 1.0.0.

---

## Related documents

- [ADR-0002 — Layered preset architecture](adrs/0002-layered-preset-architecture.md):
  the data/expression/script model and why it is layered.
- [ADR-0020 — Preset expression grammar v2](adrs/0020-preset-grammar-v2-branching-functions-tempo.md):
  the math functions, branching, `tempo`/`novelty`, and warn-but-load typo handling.
- [`presets/README.md`](../presets/README.md): the per-system parameter tables,
  engine-wide controls, structural config, and `[smoothing]`.
- [`docs/preset-palettes.md`](preset-palettes.md): the palette surface — built-in
  names, custom stops, and the A/B crossfade.
- [ADR-0006 — C ABI v2 preset loading](adrs/0006-c-abi-v2-preset-loading.md):
  how the foobar plugin reaches the shared library.
- [`docs/capturing.md`](capturing.md): the headless `shot` CLI for rendering a
  preset to a PNG without launching the app.
