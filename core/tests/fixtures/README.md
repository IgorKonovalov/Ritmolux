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

## `swarm_shaped.toml` is a *third* off-roster fixture, on the same argument

It is in `EXTRA_FIXTURES` for the reason the two above are (Plan 0070 Phase 4,
[ADR-0084](../../../docs/adrs/0084-a-particle-marks-silhouette-is-a-signed-distance-function.md)):
the rostered fixture **structurally cannot reach** the code under test. The mark
silhouette's default is `disc`, and `disc` is *exactly* `length(local)` — the
arithmetic the sprite drew before the roster existed, which is what keeps every
other baseline in this directory byte-identical — so `swarm.toml` takes that arm
and no edit to it that kept its own baseline could execute a line of the ring,
polygon, star or heart arms.

It draws seven-pointed stars, which is the figure the plan came from. Two of its
values are load-bearing and its header says so: `size = 9.0` (at 128 px a sprite
has to be several pixels across before a silhouette is anything but its own
anti-aliasing) and `zoom = 8.0` (the swarm's population is fixed at the tier's
10 000, so a sprite that large saturates the frame — the first draft at zoom 1
was an even confetti mush with no figure in it at all, and the zoom is what puts
a few hundred separated marks in frame instead).

Appended at the **end** of `EXTRA_FIXTURES`, for the reason that list gives.
Blessing it moved no other baseline: `LMV_BLESS` rewrites all of them, and the
three line-scene PNGs it rewrote (`lsystem`, `parametric_curve`,
`star_pattern` — the three that read `max_outlier 1` against their committed
baselines, i.e. WARP's own rasterization noise) were restored before committing.
Check `git status` after any bless here and do the same.

## `attractor_trails.toml` has its own test binary, and that is the point of it

Plan 0053 Phase 1, [ADR-0058](../../../docs/adrs/0058-bind-group-layout-collisions-carry-evidence.md).
It belongs to `core/tests/attractor_trails.rs` and to nothing else, so
`LMV_BLESS=1 cargo test -p lmv-core --test attractor_trails` rewrites **one**
file. That is deliberate: `LMV_BLESS` is not scoped to a fixture, so adding this
to `golden.rs`'s `EXTRA_FIXTURES` would have meant rewriting all 12 of that
binary's baselines to add one — and three of them (`lsystem`, `parametric_curve`,
`star_pattern`) re-encode differently on this repository's dev box from a clean
tree, so the diff would name files the change never touched. Same posture
`line_joints.rs` documents.

**What it covers is a pipeline coexistence no other capture produced.**
`attractor_clifford` and `attractor_leviathan` bind the engine `trails` stage on
the attractor, putting that scene's four pipelines and the stage's two in one
command buffer. `attractor.toml` binds no trails and every `composite_*` fixture
is a line scene, so the densest coexistence any shipped preset creates was pinned
by nothing. Its own binary also keeps those six pipelines off the devices the
other two capture binaries build, which is the rule `composite.rs` states.

It is **coverage, not evidence of correctness** — the baseline is blessed on WARP
like every other one here, so if this configuration aliases, the PNG is a picture
of the wrong thing. ADR-0058's hardware-vs-WARP comparison is the check; this is
the drift guard.

`fade = 0.6` under `trails = 0.98` is load-bearing for the reason the
`attractor_*_fb_*` family's header gives at the bottom of this file, and the
binary asserts the relation rather than trusting it.

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

`composite_bloom_exposed.toml` (Plan 0066 Phase 3, ADR-0080) is the **only
fixture in this directory that binds `exposure`** — before it, `grep -l exposure
*.toml` was empty across all 23, which is exactly why nothing guarded the
relationship between a preset's stop and its `bloom_threshold`. It pairs an
extreme stop with a scene level that compensates for it, so the tonemap sees the
frame it always saw and only the bright-pass's input has moved. Its header
carries the measurement that says the guard bites.

`composite_symmetry.toml` (Plan 0064 Phase 5, ADR-0077 + ADR-0078) is the **third**
kaleidoscope fixture, and it is not a third opinion about the fold. The stage grew
five coordinate terms — `kaleido_tile`, `_radial`, `_spiral`, `_zoom`, `_inner` —
and the palette grew `palette_steps` and `palette_contour`, and **not one fixture in
this directory bound any of them**: `composite_kaleido` binds the order and the
angle, its squash sibling adds the edge, and every other baseline in the suite
leaves the whole radial group at its identity. So the log-radius wrap, the spiral's
closure across `atan2`'s branch cut, the inner freeze and the zoom's scaling by the
log period could all have been broken with the suite green. It binds all eight, each
off its default, over a border-filling `fragment_field`.

Two of its values are measured rather than picked, and its header carries the
reasoning: `kaleido_zoom = 0.35` (at `0` the ring-unit parameterization is
indistinguishable from the raw `log r` one it replaced, so a zero would pin nothing
about it) and `color_span = 1.6` (at `fragment_field`'s default `0.6` the field
walks half the gradient and six bands collapse to three neighbouring olives — a
faint texture rather than a pin on `band_coord`). It runs **no lit backdrop**,
unlike `composite_kaleido`: `fragment_field` over one is a WARP mis-render that
pre-exists this plan on `main`.

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

## The `*_over_scaled.toml` pair is a seventh guard, and neither file is a preset

`spectrum_comb_over_scaled.toml` and `spectrum_corona_over_scaled.toml` belong to
`core/tests/geometry_extent.rs` (Plan 0069 Phase 3,
[ADR-0083](../../../docs/adrs/0083-in-frame-geometry-is-measured-at-the-line-renderers-draw-seam.md)).
They pin no pixels, have no baselines, and `LMV_BLESS` does not touch them.

They are **frozen defects** rather than fixtures authored for a look: each is a
shipped preset exactly as it shipped over-scaled, recovered from
`git show 2efb80e^:presets/<name>.toml` with the comments stripped and the `name`
suffixed. Both were tuned before ADR-0049 normalized the bands to `0..1` and
afterwards multiplied a value roughly five times larger, so the comb's bars stood
more than two frame-heights above the top edge and the corona's spokes ran off
all four.

The point of keeping them is that **pixel coverage scored both of them above the
legitimate content**: a comb roots every bar on a shared baseline and a corona
roots every spoke at a centre, so clipping the tips costs almost no lit pixels.
They are what makes "the new measure convicts what the old one could not" a claim
with evidence behind it, and the gate compares each against **the shipped preset
it was recovered from**, by name — a paired comparison, because two shipped
presets (`Rose Zoom`, `Rose Overflow`) deliberately leave the frame and sit right
beside the defect on an absolute scale.

"Do not tune" bites in an unusual direction here: a fixture quietly brought back
inside the frame would leave the gate's assertion true of nothing at all. The one
binding that matters in each is `scale` (`3.80` and `5.20`), and each header
records the arithmetic that makes it wrong.

## The `feedback_*` / `composite_warp_*` / `attractor_fb_*` families pin a *motion*

Eleven files, Plan 0046 / [ADR-0048](../../../docs/adrs/0048-transformed-feedback.md),
belonging to `core/tests/feedback.rs` and (for the three `composite_warp_*`) to
`core/tests/composite.rs` as well. What they exist to test is that an accumulation
**moves** — which is the one thing a baseline cannot say, since a baseline asks
whether a picture is the picture it was last time.

So most of them pin no pixels. They are read in **matched pairs and quartets**
where the members differ in exactly one key, and the guards compare a run against
itself or one fixture against its control:

- `feedback_still` / `feedback_identity` — the same preset binding no `fb_*` and
  binding all six to their documented defaults. Asserted **byte-identical**, which
  is ADR-0048's whole identity claim in one comparison.
- `feedback_zoom` / `feedback_rotate` — `feedback_still` plus one rate each, read
  as radial against tangential displacement.
- `feedback_ring` — the rotation, spun into a closed ring, whose pixel bounding
  box states an aspect claim (ADR-0037).
- `feedback_add` / `feedback_max` — one `[feedback] blend` key apart.
- `composite_warp_swirl` / `_ripple` / `_fisheye` — `composite_trails.toml` param
  for param plus one `[feedback] warp` key. These three **do** carry baselines,
  because a drift guard on each is worth having; that the three are *distinct from
  each other* is asserted in `feedback.rs` instead, since three identical
  baselines would pin perfectly happily.
- `attractor_fb_control` / `_rotate` and `attractor_trails_control` / `_fb_both` —
  the second accumulation sink, alone and alongside the engine stage.

"Do not tune" bites in a specific direction here, and it has already bitten:
**several of these values exist to stop a comparison being vacuous, not to make a
picture.** Two pairs measured `0.000000` apart during authoring, for two reasons
that both look exactly like a passing test — `max(cur, prev * fade)` over a
stationary figure is exactly `cur`, and so is a `trails` stage whose tail is
shorter than the scene's own `fade`. That is why the attractors carry a `spin` and
why `trails = 0.98` sits above their `fade = 0.6`, and each header records the
arithmetic. A "simplification" that removes either restores a green test that
measures nothing.

## `scratch-NNNN/` directories are not fixture directories at all

A `scratch-NNNN/` holds presets a **human** phase of Plan NNNN needs to look at,
plus a README. Nothing includes them, no test names them, `LMV_BLESS` does not
touch them, and `core/build.rs` cannot see them (it globs `presets/*.toml` only).
They live here because a phase that needs a preset needs somewhere to keep one,
and a session's temporary directory does not survive the session.

- **`scratch-0046/`** — two presets for Plan 0046 Phase 5, whose whole content is
  looking at transformed feedback fullscreen, with music, and saying whether it
  reads. Its README has the run command and the tuning traps found while
  authoring them.
- **`scratch-0082/`** — the **banding reference frame**. One preset, the dusk
  ground at `bg_ramp_gamma = 0.4`: the darkest of the Plan 0080 probes (mean RGB
  `34.0 / 42.7 / 69.5`) and the worst banding case, both because a fast-dropping
  ramp leaves a long dim tail and a flat tail is where one 8-bit level lasts
  longest. It is kept so the **same frame** can be re-measured after
  [Plan 0082](../../../docs/plans/done/0082-the-gradient-stops-banding.md)'s dither
  and again after [Plan 0081](../../../docs/plans/0081-the-sky-gets-a-galaxy.md)
  adds a second overlapping gradient — a before/after on two different pictures
  would prove nothing. Its README carries the run command, the 2026-08-12
  pre-dither measurements (widest mid-range plateau **58 px at value 11**, 0 %
  rail-pinned) and what to check at each of those two points.

**A `scratch-NNNN/` preset must not be tuned**, for a different reason from the
fixtures above: no baseline depends on it, but its whole value is being *the same
frame* as the measurement or observation recorded beside it. To explore a
variant, copy it.
