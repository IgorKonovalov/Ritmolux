# ADR-0100 — Documentation images are committed headless renders

> **Status:** proposed
> **Date:** 2026-08-13
> **Related plan(s):** [0088](../plans/0088-the-docs-get-pictures.md)

## Context

This repository has shipped for eighty-eight plans with **no committed image of any kind**. Every
document describes a visual system in prose, and the one instrument that renders it —
`shot` — writes into `target/` or a scratch path that nobody keeps. A reader arriving at
`README.md` cannot see what the app draws; a preset author reading
[`presets/README.md`](../../presets/README.md)'s 2,943 lines of parameter tables cannot see what any
of those parameters look like.

Adding pictures is therefore not a formatting question. It is a decision about **where pixels come
from, whether they enter git history, and what keeps them true** — and all three could reasonably go
the other way.

Three concrete facts shape it:

- **The renderer is already headless and deterministic.** A capture is a pure function of
  `(preset, input, frame-count, size)` ([`docs/capturing.md`](../capturing.md), NFR §6), so a render
  can be *regenerated* rather than archived. A photograph of a window cannot.
- **PNGs are not small, and git history is not prunable here.** Measured 2026-08-13 across six
  families at 1280x720: **1.0–2.0 MB per image, mean ~1.4 MB** (`attractor_leviathan` 2.0 MB,
  `spectrum_halo` 1.0 MB). Committing ~20 of them adds ~28 MB to every clone forever, in a project
  whose stated value is that lightweight is a feature — and this project **never rewrites history**,
  so the decision is one-way.
- **Renders are not byte-reproducible across machines.** The golden suite calls a `0.02` mean
  channel difference rasterizer drift, and eight of twenty baselines rewrite on a clean bless
  locally. So "re-run the script and diff the bytes" is not available as a freshness gate.

## Decision

We will generate every documentation image with the **`shot` CLI**, commit the results as
**full-resolution 1280x720 PNGs under `docs/images/`**, and drive them from a **committed
regeneration script** (`scripts/docs-shots.mjs`) whose manifest is the single record of which preset,
stimulus, hop, size and tier produced each file. Images are captured **under real audio** — a named
hop of `--signal dynamic:110`, through the real analyzer — and at the **`Rich` tier**, because that
is what the app starts on and a picture in the README should be what a user sees.

The regeneration script is **not a CI gate and never becomes one.** Adapter drift makes byte
comparison meaningless, so freshness is maintained by re-running the script when the thing it depicts
changes — a close-ceremony sweep duty, in the same table as the other operator docs.

Two boundaries that follow from this and are part of the decision. Documentation images show **what
the engine draws, not the application's window**: there is no screenshot of the preset browser, the
settings menu or the `F3` overlay, because `shot` has no chrome to render and a live grab would come
from a human and would not regenerate. And a **`Rich` documentation image is still not a baseline** —
[ADR-0064](0064-a-capture-may-pin-the-rich-tier.md)'s prohibition is about `core/tests/golden/`, which
this decision does not touch.

## Consequences

### Positive

- **Every image is reproducible from a command in the repo.** The manifest names the preset file, the
  stimulus, the hop, the size and the tier, so "how was this made" is never a guess and re-shooting
  the whole set after an engine change is one command with no arguments.
- **The existing link checker covers them for free.** `scripts/check-doc-links.mjs` matches
  `](target)`, which is exactly what `![alt](target)` contains, so a deleted or renamed image is
  caught by the pre-push hook and by CI's `links` job — verified against the regex, not assumed.
- **The docs work offline and in an editor.** A relative path renders in GitHub, in VS Code, and in a
  clone with no network, which an externally-hosted image does not.
- **No new dependency.** `image` is already a dev-dependency (ADR-0011); the shipped binary is
  untouched.

### Negative

- **~28 MB enters git history permanently.** At the measured ~1.4 MB mean, a ~20-image set is the
  single largest thing in the repository, and it cannot be removed later without a history rewrite
  this project forbids. The mitigation is a stated budget — **≤ 22 images, ≤ 32 MB** — and the rule
  that a new documentation image *replaces* one rather than adding to the set.
- **Images go stale silently, and nothing detects it.** A preset retuned by the content lane leaves
  its gallery image showing the old look, and no gate can see the difference. This is the same class
  of rot as [the preset-header workaround sweep](../plans/README.md) and has the same mitigation: a
  human duty at a named cadence, plus a script cheap enough that re-running it is never the reason it
  was skipped.
- **`Rich` is uncalibrated.** ADR-0045's Phase 4 calibration has never run, so the tier's values are
  provisional; when it runs, every documentation image owes a re-render. That is a script re-run, and
  it is the cost of showing what a user sees rather than what a baseline pins.
- **No picture of the application.** The README will show nine renders and no window, so a reader
  learns what the visuals look like and nothing about the preset browser or the settings menu.

### Neutral

- A capture at `Rich` is not measurably larger on disk than the same capture at `Floor` — measured
  1,899 KB against 2,010 KB on `attractor_leviathan`, marginally *smaller*, which is
  [ADR-0065](0065-the-attractor-deposit-is-normalized-by-particle-count.md)'s deposit normalization
  visible in the file size.

## Alternatives considered

### Alternative A — Host the images externally (release assets, a `gh-pages` branch, a CDN)

Zero repository weight, which is the one real cost of the chosen option. Rejected because it breaks
three things at once: the link checker cannot verify an `https:` target (it deliberately skips them
to avoid a network flake), the docs stop rendering offline and in an editor, and the preset
documentation stops being self-contained in a clone. The 28 MB buys all three back.

### Alternative B — Downscale, or use a lossy format (WebP/JPEG at ~960px)

Roughly a fifth of the weight for images that still read fine at README width. Rejected on the user's
explicit call at the interview, with the measured per-image cost in front of them. The technical
argument against it is real but secondary: these renders are smooth gradients and fine particle
noise, which is exactly the content that palettizes and blocks badly, and a documentation image of a
*renderer* that shows compression artifacts is documenting the wrong thing.

### Alternative C — Screenshot the running application instead

The only way to show the preset browser, the settings menu, the `F3` overlay and the app in situ, and
the only way to show the visuals reacting to the user's own music. Rejected as the *primary* source
because nothing about it regenerates: every image would be a `human` phase, re-shooting after a
preset retune would be a manual sitting, and the pictures would drift from the code with no command
that fixes them. It remains the right answer for chrome specifically, and is recorded as a followup
rather than a rejection on the merits.

### Alternative D — Capture under `--set` held stimuli instead of real audio

Simpler: one flag, no hop arithmetic, and it already produces a full-size frame today. Rejected
because [`docs/capturing.md`](../capturing.md#the-three-calibration-traps) names three traps that all
land on documentation images specifically. `--set beat=1` holds every beat accent at full deflection,
so a working preset photographs as a blown-out one. `--set` band magnitudes are "a fraction of peak,
held forever", which no music does. And `--set` cannot reach the 64-band array at all, so `bin(x)`
reads `0` and the entire `spectrum` system renders as its inert resting comb — the family's
documentation image would show a preset that looks broken and is not.

## Notes

Measurements in this ADR were taken 2026-08-13 on the development box's hardware adapter with the
release `shot` example, at 1280x720, before any of Plan 0088's phases landed. The filmstrip-tile
dimension quoted in that plan (363x208) is from the same session.
