# Grammar notes for authoring

> **The complete expression reference is `docs/presets.md`** (variables, constants, functions,
> comparisons, idioms) and **the table reference is `presets/README.md`**. Both are maintained at
> plan close. This file is the authoring layer on top: what to reach for when, what bites, and
> what the error surface actually does to you mid-session. Source of truth stays the code —
> `VAR_NAMES` / `Func::from_name` in `core/src/preset/expr.rs`, `RawPreset` in
> `core/src/preset/schema.rs`.

## File shape

```toml
system = "attractor"        # required — one of the seven; unknown rejects the file
name   = "Lorenz Drift"     # optional — display name, and what `--preset` matches

[particles]                 # structural config: [curve] | [generator] | [particles]
family = "lorenz"

[generator]                 # any system may carry one just for the seed
seed = 12                   # salts hash()/noise(); "random" varies per launch

[palette]                   # colour: a built-in `name` OR custom `stops`, never both
name = "ice"

[palette_b]                 # optional crossfade target for a bindable `palette_mix`
name = "ember"

[smoothing]                 # per-param easing: SECONDS, a bare number, NOT an expression
zoom = 0.12

[params]                    # every value is a quoted STRING, even a bare number
zoom        = "1 + clamp(bass * 3, 0, 0.6)"
palette_mix = "bar"
```

`[params]` values are strings — `warp = 0.4` is a TOML type error, `warp = "0.4"` is right.
`[smoothing]` values are the opposite: bare numbers, and an expression there is an error.
Bindings apply name-sorted, so file order is irrelevant (group them for a human reader anyway).

## Reaching for the right tool

| You want | Write |
|----------|-------|
| a small band to move a param usefully | `clamp(band * gain, lo, hi)` — gain-then-bound, over a baseline |
| a threshold that doesn't snap | `smoothstep(0.15, 0.45, bass)` — the easing primitive |
| a hard either/or | `select(cond, a, b)` — **only the taken branch evaluates**, so it also guards partial functions: `select(x >= 0, sqrt(x), 0)` |
| a punchier or gentler response curve | `pow(bass * 8, 2)` / `pow(x, 0.5)` |
| a value that wraps cleanly | `mod(time * 0.2, 1.0)` — floored, so it never returns negative |
| a drift that wanders instead of ramping | `noise(time * 0.3)` — seeded value noise, `[0, 1]`. One call replaces the sum-of-detuned-sines idiom the older files use |
| something *different* per beat / per element | `hash(floor(time * 2))` / `hash(index * 64)` — a scatter in `[0, 1)`, unrelated between neighbouring arguments |
| a level in dB | `20 * log(max(x, 0.0001)) / 2.302585` — `log` is natural, and `log(0)` is `-inf` |
| "loud AND fast" / "beat OR strong hit" | `min(bass > 0.3, tempo > 120)` / `max(beat, onset > 0.6)` — no booleans exist; comparisons give clean `1`/`0` |
| behaviour that changes with tempo | `select(tempo > 128, fast, calm)` — never use `tempo` raw, it's BPM |
| a driver that stops jittering | move it to `[smoothing]`, don't fake it with arithmetic |
| a value held until the next beat | **not expressible** — the evaluator is pure and stateless. API feedback. |

Chained comparisons (`a > b > c`) parse left-associatively and compare a `0`/`1` against `c` —
write `min(a > b, b > c)`.

## What the engine does with your mistake

**Hard error — the whole file is rejected and the app keeps its last good set (never crashes):**
malformed TOML; unknown `system`; an expression that fails to compile (unknown identifier, wrong
arity, unbalanced parens, stray character, `1 2` trailing tokens); an invalid `[curve]` /
`[generator]` / `[particles]` / `[palette]` / `[smoothing]` value.

**Warning — the preset loads and everything else applies:** a binding whose param name no system or
engine stage consumes. The message names the param and the system.

> **The warning is where this lane gets bitten.** The standalone prints warnings on load and on
> every hot-reload; **`shot` prints errors only**. So a typo'd param is invisible in the tool you
> verify through, and the render looks "fine, just not doing what I asked". Check names against
> `presets/README.md`, or run the app against the folder when a binding seems inert.

**Not validated at all:** the numeric *range* an expression produces. Output is written straight
into the scene param — no clamping, no NaN check. `NaN`/`inf` (a zero denominator, `sqrt` of a
negative outside a `select`) becomes broken geometry, not an error. You clamp; the engine won't.

## Structural tables — the parts with rules

- **`[curve]`** — `parametric_curve` only. `family = "maurer_rose"` is the **only** value; absent
  means the family default. A second family is engine work.
- **`[generator]` as L-system** — required for `lsystem`. Non-empty `axiom`; ≥1 `rules` entry with a
  **single-character** key; `angle_deg` finite (default 25); `max_depth` in `1..=7` (default 4).
  Turtle vocabulary: `F`/`G` draw, `f` moves without drawing, `+`/`-` turn, `[`/`]` push/pop, any
  other char is an inert variable.
- **`[generator]` as star** — required for `star_pattern`. `tiling` ∈
  `square|4|4.4.4.4`, `hexagon|6|6.6.6`, `octagon|8|4.8.8`, `dodecagon|12|3.12.12`;
  `contact_angle_deg` finite (default 30).
- **`[particles]`** — `attractor` only. `family` ∈ `de_jong|clifford|thomas|lorenz` (default
  `de_jong`).
- **`[palette]` / `[palette_b]`** — a built-in `name` (`spectrum|ember|ice|mono|aurora`) **or**
  custom `stops` (≥2, each `at` in `0..=1` and ascending, colour `#rrggbb` or `[r,g,b]` floats).
  Setting both, or neither, is a load error. **Silently inert on the three line scenes.**
- **`[smoothing]`** — `param = seconds`, non-negative and finite; `0` means instant. Runs on real
  elapsed time (identical at any refresh rate) and resets on a preset switch.

**Geometry cap:** the line scenes share `MAX_SEGMENTS = 20_000`. A too-dense figure (high `samples`,
`max_depth`, or `mirror_order`) is truncated and the drop is surfaced — lower the density rather
than living with a clipped figure.
