# Golden drift fixtures

These TOML files are **test-only frozen fixtures** for the golden drift guard
(`core/tests/golden.rs`), one per `SystemKind` — plus the `composite_*`,
`easing_*` and `line_joint_*` fixtures described at the bottom, which belong to
different guards. They exist to catch **unintended
engine rendering drift** — a shader or scene-math change that silently perturbs
output — by pinning each scene's pixels to a committed baseline PNG under
`core/tests/golden/`.

Decision and rationale: [ADR-0023](../../../docs/adrs/0023-golden-drift-guard-uses-frozen-fixtures.md)
(Plan 0022).

## Do not tune

**These are not shipped presets and must not be tuned for looks.** Editing a
fixture changes its render and invalidates the committed baseline, defeating the
drift guard. The shipped presets in `presets/` are the ones the `preset-author`
lane tunes; they are guarded *behaviorally* elsewhere (`sanity`, `reactivity`,
`animation` — all iterate `default_presets()`), never pixel-pinned here. Each
fixture is deliberately minimal and deterministic (constant or lightly-bound
params) so it draws a non-trivial frame that never needs content tuning.

## Adding a scene

A new `SystemKind` variant makes `golden.rs` fail to compile until you add its
fixture here — the fixture roster is an **exhaustive `match SystemKind`** with no
wildcard arm. To add one:

1. Author `<system_name>.toml` here (mirror the header comment of the others).
2. Add the variant's arm to `fixture()` in `golden.rs`. (There is no second list
   to update for a *new system*: the roster iterated here is `SystemKind::ALL`
   itself, since Plan 0030 Phase 3 retired this file's duplicate `SYSTEMS` list.
   `EXTRA_FIXTURES` — see below — is a different thing and is not part of adding
   a scene.)
3. Bless the baseline on Windows WARP:
   `LMV_BLESS=1 cargo test -p lmv-core --test golden`, then eyeball the new PNG
   under `core/tests/golden/` to confirm the scene actually drew.

Baselines are WARP-only (macOS skips per ADR-0016) and must be blessed on WARP or
they will drift. **`LMV_BLESS=1` rewrites every baseline, not just the one you
are adding** — check `git status` afterwards and restore any file you did not
mean to move, or you will silently re-baseline an unrelated scene's drift.

Note that `golden.rs`'s harness frame carries a **populated `spectrum` array**
(Plan 0034): a frame claiming `bass = 0.6` with 64 silent log-bands is not a
frame any audio could produce, and the spectrum fixture would pin a baseline of
nothing under it.

## `attractor_depth.toml` is a *second* fixture of a rostered system

It is read by `golden.rs` like the roster is, but it is not part of the roster —
it lives in `EXTRA_FIXTURES`, a one-entry list captured **after** the
`SystemKind::ALL` loop. That list is a narrow escape hatch, not a general
second roster, and it exists for one situation: the rostered fixture of a system
**structurally cannot reach** the code under test.

That is the case here by design rather than by accident. The roster's
`attractor.toml` is **De Jong**, and [ADR-0076](../../../docs/adrs/0076-the-attractor-keeps-the-depth-it-already-computes.md)
gives every 2-D family an inverse depth extent of exactly `0.0` — which is
precisely the mechanism that makes the perspective divide, the distance haze and
the depth tint the identity on a flat figure. No edit to `attractor.toml` could
execute a line of them. So the depth cues get a 3-D fixture (Lorenz) with all
four levers off their defaults.

Two properties of it are worth knowing before touching either file:

- **It is captured after the roster, deliberately.** Every pre-existing baseline
  is therefore rendered from the device state it always was, and adding this
  moved none of them — which matters on WARP, where building GPU resources
  mid-run is documented to change what a later capture resolves to.
- **Its sensitivity is measured, and the header records the numbers.** Each
  lever was neutralized in turn and the capture re-measured; all four fail the
  guard. The first draft's `depth_fade = 0.6` did **not** — it moved the capture
  by mean 0.0091 and an outlier of exactly 48, inside both tolerances — so a
  regression that killed the fade outright would have passed. If you weaken a
  value there, re-run that check.

`systems_rosters_every_variant` holds it to the same conditions as the roster
(the TOML parses, and its stem cannot collide with a rostered baseline's).

## The `composite_*` fixtures are a different guard

`composite_trails.toml` and `composite_kaleido.toml` are **not** part of the
per-`SystemKind` roster and `golden.rs` never reads them. They belong to
`core/tests/composite.rs` (Plan 0035 Phase 2), and they exist because **no
fixture bound `trails` or `kaleido_*`** — so the entire post-composite path was
covered by no capture in the suite, which is how a defect that stretched the
whole frame shipped green (ADR-0037).

Two things about them differ from the rest of this directory, both deliberate:

- **They are captured at 160x100, not at `golden.rs`'s square 128.** The post
  stages round each grid axis up to a 256 px step, so 160x100 takes a 256x256
  grid — aspect 1.0 against the target's 1.6. A square or 16:9 size is returned
  aspect-exact by the policy and would make the guard blind, which is exactly why
  the defect survived at 1920x1080. **Do not "tidy" that size.**
- **`composite_kaleido.png` pins a known defect on purpose** (design-backlog
  0010, a Plan 0018 Phase 7 clamp artifact). Its header says what will and will
  not be visible and why. Fixing 0010 moves that baseline; re-bless it then.

They are otherwise governed by everything above: do not tune, bless on WARP,
eyeball before committing.

## The `easing_*` fixtures are a third guard, and pin no pixels

`easing_scalar.toml` and `easing_asymmetric.toml` belong to `core/tests/easing.rs`
(Plan 0037 Phase 1, ADR-0039), the transient probe. They are **twins**: the same
`[curve]` family and the same `[params]` bindings, differing only in their `name`
and their `[smoothing]` table — one scalar, one an `{ attack, release }` pair. The
test asserts that twinship, because the probe's whole claim is that the table is
the only thing that differs.

They have **no committed baseline**. Nothing here is blessed and `LMV_BLESS` does
not touch them: the probe measures a *relative* property (how many frames the
frame takes to settle after a step, up against down), so there is no PNG to drift.

"Do not tune" applies to them for a different reason than to the golden roster. A
tuned `easing_*` fixture does not fail loudly — it quietly starts measuring the
scene instead of the easing, because the probe reads the **frame** rather than the
parameter and can only see through a near-linear visual response. Their headers
say exactly which choices keep that response linear (a static figure, one
directly-multiplying param, an amplitude below the additive-blend clamp, no
composite stage). Read them before editing either file.

## `line_joint_zigzag.toml` is a fourth guard, and it pins pixels *as well*

It belongs to `core/tests/line_joints.rs` (Plan 0039 Phase 2, ADR-0041), which
asserts that a flagged joint stops leaving a hole in the stroke — a *relative*
property: a vertex is not a local luminance minimum against the segment interiors
either side of it.

**Since Plan 0040 Phase 1 it also carries a committed baseline**,
`golden/line_joint_zigzag.png`, blessed with
`LMV_BLESS=1 cargo test -p lmv-core --test line_joints`. It exists because the
defect that motivated ADR-0041 — the polyline's notch — was pinned by no pixels
anywhere: `spectrum.toml` below takes the default `bars` layout, and
`spectrum_ridge` is a shipped preset guarded behaviorally. A shader edit could
have reopened the notch on a gentler figure than this deliberately hostile zigzag
and moved no file.

The two claims are not redundant and the order matters: the relative assertion
runs **first, including under `LMV_BLESS`**, so the notch cannot be blessed back
in by someone reading the diff as drift — the bless never runs. A baseline alone
only says "something moved"; the relative claim fails loudly and says why.

The pin lives here rather than in the `golden.rs` roster because that roster is
one fixture per `SystemKind` (enforced by `systems_rosters_every_variant`) and a
second `spectrum` entry would break the invariant ADR-0023 rests on. Blessing by
`--test line_joints` therefore rewrites this one PNG and **cannot** reach the
roster — verified, not assumed.

It is captured at **512x512**, not the golden roster's 128. The feature under
test is a wedge a fraction of a stroke-width across; at 128 px there is nothing
left of it to measure. Square, so the aspect divide is the identity and the test
can turn world coordinates into pixels directly.

"Do not tune" bites hardest here: the test **recomputes the vertex positions**
from `elements`, `span`, `baseline` and `thickness`, so changing any of them
moves the probes off the geometry and the test starts comparing two pieces of
background (which it fails on, deliberately, rather than passing vacuously). The
header says what each value is holding.

## The `*_lit_backdrop.toml` trio is a fifth guard, at a configuration nothing else here tests

`swarm_lit_backdrop.toml` belongs to
`a_lit_backdrop_survives_where_the_swarm_drew_nothing` in
`core/src/render/scenes/swarm.rs`, `lines_lit_backdrop.toml` to
`a_lit_backdrop_survives_where_the_strokes_drew_nothing` in
`core/src/render/scenes/lines/renderer.rs` (Plan 0051, ADR-0056), and
`emitter_lit_backdrop.toml` to
`a_lit_backdrop_survives_where_the_emitter_drew_nothing` in
`core/src/render/scenes/emitter.rs` (Plan 0052) — one per **draw seam**, since
those are the three pipelines that render directly into the post chain's input
rather than presenting through an alpha-aware pass. The line one covers all four
line scenes at once; they share the renderer.

**They are per-seam rather than global because nothing structurally forces a
shader's colour and alpha to stay in step**, which is also why adding a fourth
seam means adding a fourth fixture here. Each has been demonstrated in **both**
directions against a deliberately reverted constant-alpha shader; the emitter's
test records its two numbers (0.3345 against 0.0002 on the same capture) in its
doc comment.

They pin **no pixels** and have no committed baselines; `LMV_BLESS` does not
touch them. Each test captures its own fixture three ways — lit backdrop, black
backdrop, and backdrop with the scene contributing nothing — and asserts that
wherever the scene wrote no light, the backdrop arrives intact in the **linear**
composite, upstream of the tonemap, where the bound is 0 rather than a tolerance.

**`bg_bright > 0` is the whole point, and it is why these are separate files
rather than re-parameterized existing ones.** Nearly every other fixture here
runs `bg_bright = 0`, which is the right call *for a baseline* — see
`composite_bloom.toml`'s own header for the reasoning. It is also why a scene
emitting a constant alpha 1, holding the backdrop out of every pixel its quads
covered, stayed invisible to this whole suite for as long as it shipped: on
black, covering the backdrop and compositing over it are the same picture.

The exception is instructive. `composite_kaleido.toml` **does** run a lit
backdrop, and it is a line scene, so it was the one baseline positioned to see
the defect — and it still did not, because at its `thickness = 2.0` the dark rim
is a hairline and a mean-drift gate cannot see one. It moved when the fix landed;
its header records the numbers. A guard that asserts the property directly is
what these two files are for.

"Do not tune" applies for a third distinct reason here. Each header says what
every value is holding — a lit backdrop, an **active post stage** (without one
the scene draws straight onto the backdrop and the defect cannot exist at all),
geometry sparse enough to leave untouched backdrop to check, a colour that is
never black in all three channels at once, and — on the line fixture — a
deliberately **fat stroke**, because the rim scales with `thickness` and a
shipped width leaves the test green and blind. The emitter's differs on one
point and its header says so: it **cannot be frozen** the way the swarm's is,
because an emitter whose objects do not move has no picture at all — its source
line sits below the frame. It costs nothing, since the three captures vary only
`bg_bright` and `size` and neither touches spawning or the path. Each test reads those
preconditions back out of its own file before it touches the GPU and reports the
pixel counts either side, so an edit that quietly empties the region under test
fails rather than passing on nothing.

## `emitter_onset.toml` is a sixth guard, and it is driven by a *changing* stimulus

It belongs to `a_spawn_rate_on_onset_bursts_and_then_idles` in
`core/src/render/scenes/emitter.rs` (Plan 0052 Phase 3). It pins no pixels and
has no baseline.

Every other fixture here is captured through `capture_preset`, which holds **one**
analysis frame for every warm-up step — the right primitive for a baseline, and
structurally unable to ask this question. The emitter is the first scene whose
*population* is not fixed, so "reacts to a transient" is only half the claim: the
other half is that the frame **empties again** when the transient passes, which a
sustained stimulus can never show. This fixture is therefore driven through
`capture_preset_over` with a short silent lead, a six-frame hit, and a second of
silence, and the test reads the whole response.

Two of its values are load-bearing and neither is a look choice. `spawn_rate` has
**no constant term** — every shipped preset carries a base rate so the frame is
never empty, and with one here "idles between transients" would be a statement
about that floor rather than about the scene. And it binds **no `trails`**: with a
feedback stage on, the tail would decay the same way whether objects were retired
or not, so the measurement would say nothing about lifetimes. `lifetime = 0.55`
is 33 frames, comfortably inside the silent tail.
