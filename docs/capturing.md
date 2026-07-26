# Headless capture & visual QA

The renderer can draw a scene with **no window** — a surface-less wgpu context
draws into an offscreen texture and hands back raw RGBA pixels. Two things are
built on that (Plan 0013):

- a **`shot` CLI** (`standalone/examples/shot.rs`) that writes PNGs an agent can
  read and a metrics report it can parse, and
- a **differential visual-QA harness** in `core/tests/` that hard-tests every
  preset for reactivity, animation, shape sanity, and beat response, with an
  advisory distinctness report and golden-image regression.

A headless render is a **pure function** of `(preset, input, frame-count)` —
scenes are reseeded per capture, every frame steps at the fixed
`scenes::FALLBACK_DT` (the live app injects its real `dt` instead; Plan 0014),
and the DSP is deterministic — so renders are reproducible and diff-able.

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
| `--set k=v,...` | constant stimulus frame: `bass,mid,treb,onset,bar` (0..1) and `beat` (non-zero = true) |
| `--frames <N>` | frames to advance before capture (default 120) |
| `--size <WxH>` | render size (default 1280x720) |
| `--out <path>` | output PNG (single shot) or dir/file (`--all`) |
| `--all` | contact sheet of every preset, labeled (needs `--out`) |
| `--report [family=<sys>]` | per-family metrics table; `family=` takes any `system` name (`fragment_field`, `swarm`, `parametric_curve`, `lsystem`, `star_pattern`, `reaction_diffusion`, `attractor`) |
| `--json` | emit the report as JSON instead of a text table |
| `--signal <kind:param>` | synth-audio filmstrip (see below) |
| `--audio <clip.wav>` | filmstrip from a 16-bit PCM WAV |
| `--strip <N>` | frames tiled along the audio (default 8) |

Bad arguments and unknown presets exit non-zero with a message.

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

# Labeled contact sheet of the whole library
cargo run -p standalone --example shot -- --all --out gallery/

# Metrics report as a text table, or JSON for parsing
cargo run -p standalone --example shot -- --report
cargo run -p standalone --example shot -- --report --json > report.json

# Beat filmstrip from a synthesized click track (no asset needed)
cargo run -p standalone --example shot -- --preset "Pulse Field" \
  --signal click:120 --strip 8 --out click.png

# Filmstrip from a real clip (16-bit PCM WAV)
cargo run -p standalone --example shot -- --preset "Burst" \
  --audio assets/test/clip.wav --strip 8 --out clip.png
```

`--signal` kinds: `click:<bpm>`, `bass:<hz>`, `treble:<hz>`, `noise:<seed>`,
`chord`. The synth path needs no committed asset. `--audio` reads uncompressed
16-bit PCM WAV only (a hand-rolled reader — no decoder dependency); other
encodings are a followup.

> **Test audio is added manually and never committed.** Drop a 16-bit PCM WAV
> into [`assets/test/`](../assets/test/) — that folder is gitignored (only its
> README is tracked), so no licensed audio lands in the repo. Use your own or a
> royalty-free / CC0 clip; factory-library samples are fine to point at on disk
> but must not be committed. The `--signal` path needs no file, so the whole
> audio pipeline can be validated without adding anything.

The `--report --json` schema is a nested object of numbers keyed by
family/preset: per-band `reactivity`, `animation`, `coverage`, the pairwise
`pixel`/`shape` distinctness matrices, and `near_duplicates`.

## The `core/tests/` harness

Most differential tests render on the **software adapter** (`prefer_software`) so
they hold on any GPU; the exceptions say so below. Run the whole suite:

```bash
cargo nextest run -p lmv-core     # what CI runs (per-test process isolation)
cargo test -p lmv-core            # also fine, except where noted below
```

> **Use `nextest` for the whole suite.** `preset`'s zero-allocation assertion
> counts allocations through a process-global allocator hook, so it is only
> reliable under nextest's per-test process isolation — under stock `cargo test`
> a concurrently-running test's allocations bleed into the count. Tests that need
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
| `reaction_diffusion` | HARD | the first stateful-feedback scene: seed reproducibility, regime response ([ADR-0012](adrs/0012-stateful-feedback-render-system.md)) |
| `attractor` | HARD | the first compute-particle scene: seed reproducibility + beat perturbation ([ADR-0015](adrs/0015-gpu-compute-particle-idiom.md)) |
| `ink` | HARD | the final tone-remap **inverts** tone, and `ink_amount = 0` is byte-identical to an unbound frame ([ADR-0028](adrs/0028-final-stage-ink-tone-remap.md)) |
| `background_composite` | HARD (**hardware only**) | RD / attractor presents alpha-blend over the `bg_*` backdrop; **skipped** on a software adapter, which mis-renders that pipeline set |
| `transition` | HARD | every switch path (cycle **and** select) renders intermediate blended frames as a ramp, reproducibly from the injected `dt`; each blend kind shows its own signature; a switch arriving mid-dissolve lands on the last index requested; a hot-reload mid-dissolve cancels cleanly; the heavy attractor ↔ reaction-diffusion pair dissolves on the freeze fallback (set `LMV_TRANSITION_STRIP=<dir>` to also dump filmstrips) |
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
shared by the tests and the CLI report.

### Golden baselines

Golden baselines live in `core/tests/golden/*.png` and are ordinary PNGs
(viewable in the repo / PR diffs). To regenerate them after an intended visual
change:

```bash
LMV_BLESS=1 cargo test -p lmv-core --test golden
```

**Eyeball the regenerated PNGs before committing** — the first baseline is easy
to enshrine wrong. The compare tolerates minor cross-GPU rasterization drift; a
genuine change exceeds it.

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
