# Plan 0046 Phase 5 — the look, on the wall

Two **scratch presets** for the one thing a headless capture cannot answer: does
transformed feedback ([ADR-0048](../../../../docs/adrs/0048-transformed-feedback.md))
read as depth and flow, with music, fullscreen, on the target display — and does
frame time hold while it does.

They are **not shipped content and not test fixtures.** Nothing includes them,
`RLX_BLESS` never touches them, and `core/build.rs` does not see them (it globs
`presets/*.toml` only). Plan 0046's "What this plan does NOT do" is explicit that
the content pass belongs to a later roadmap item; these exist to be looked at and
then either superseded by real presets or deleted.

| File | Look | The gene it exercises |
|---|---|---|
| `tunnel_beat_zoom.toml` | a 12-fold star rosette streaming outward into a light tunnel, lunging on the kick | `fb_zoom` bound to `beat`, `max` deposit |
| `swirl_add_echo.toml` | filaments of the fragment field winding into a vortex, echoes summing into smoke | `[feedback] warp = "swirl"` + `blend = "add"`, `fb_warp` bound to `bass` |

## Running them

The standalone loads a **directory**, so point `RLX_PRESET_DIR` at this one — the
per-user preset cache is seeded write-if-absent and will not pick up an edit here
otherwise. From the repo root, with audio playing through the default output
device:

```powershell
$env:RLX_PRESET_DIR = "$PWD\core\tests\fixtures\scratch-0046"
cargo run --release -p standalone -- --tier rich
```

```bash
RLX_PRESET_DIR="$PWD/core/tests/fixtures/scratch-0046" \
  cargo run --release -p standalone -- --tier rich
```

Then: cycle between the two presets, go fullscreen on the target display, and
watch the frame-time readout. `--tier rich` pins the tier rather than letting the
governor demote it, which is what Phase 5 asks to judge.

To look at a single frame without audio (what was used while authoring these):

```bash
cargo run -p standalone --example shot -- \
  --preset-file core/tests/fixtures/scratch-0046/swirl_add_echo.toml \
  --size 960x600 --frames 150 --tier rich \
  --set bass=0.6,mid=0.5,treb=0.5 --out /tmp/echo.png
```

## What was already learned headlessly

Recorded because both are tuning traps that cost several rounds, and because a
disappointing *live* look should be judged against what is already known:

- **`fb_zoom` has a threshold below which a tunnel is just a blur.** Under about
  2/s the expanding copies overlap into one soft halo; at 3.2/s they separate into
  distinct rays. The first draft sat at 1.35 and read as a glow around a star.
- **`add` needs a sparse source.** The fragment field covers every pixel, and a
  fullscreen source under `add` sums into a flat wash at *every* exposure — three
  drafts of the echo preset were an even green rectangle. The narrow bright band
  in its `[palette]` is what turns the field into filaments on black, and that is
  structural rather than decorative.
- **`trails` alone can be an exact passthrough.** Over a static figure,
  `max(cur, prev * fade)` is exactly `cur`. If a feedback look appears not to
  respond, check the figure is actually moving before suspecting the stage.
