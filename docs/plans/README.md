# Plans index

The one-minute "what's in flight" view. Read this first each session instead of
re-deriving state from `git log`. Completed plans move to `done/`; their full
close write-ups move to [README-archive.md](README-archive.md).

**Next free number: 0077** (ADRs are a separate sequence — next free there is **0091**.)

## Active roster

Only plans still in `docs/plans/`. A closed plan leaves this table entirely —
`Recently closed` below and `done/` both already record it. Each row carries at
most two sentences of **live constraint**: what a reader needs to decide whether
to pick this plan up. Anything longer belongs in the plan file, which is where
someone who picked it up is reading.

| Plan | Title | Status | Owner | Live constraint |
|------|-------|--------|-------|-----------------|
| [0075](0075-the-content-renaissance.md) | The content renaissance: the library is rebuilt as worlds, by replacement cohorts | **in progress** (cohorts 1-5 landed) | dev, human | The R6 rebuild ([ADR-0089](../adrs/0089-the-library-renews-by-replacement-cohorts.md): replacement cohorts under a fresh-slate rule). **Runs in the worktree lane `WORK/lmv-plan-0075`** (branch `plan-0075-content-renaissance`, unpushed): cohorts 1-5 committed and suite-green through `767d7eb`, the library is 28 worlds, cohort 5 judged live by the user 2026-08-11 with no retunes requested. **What remains: cohort 6 (the collage), gated on [0076]**, then the Phase 6 doc sweep — do not close the lane before both. The cohorts' engine-feedback handoff was filed 2026-08-11: [backlog 0084-0089](../design-backlog.md) plus re-raises inside 0009/0055/0068, each carrying a promote/park verdict; **promoted items queue behind [0076] and cohort 6, nothing new was absorbed into this plan** (its own Decision). Phase 6 carries three handoff-recorded sweep items (the `fragment_kaleido` exemplar drift, the `vigor` space-filling caveat, the `444600d` title note). The [0046] Phase 5 content finding (`blend = "add"` vs a rich palette) stands until a feedback cohort resolves it — check the cohort records before assuming it was met. |
| [0076](0076-the-second-layer.md) | The second layer: a preset composes two scenes (R3) | **approved 2026-08-09** | dev, human | [ADR-0090](../adrs/0090-a-preset-composes-two-scene-layers.md), all four open choices user-decided by interview (incl. **per-layer scene instances from the start** — same-system pairs are in scope, and Phase 2's registry migration is the plan's real risk). Touches `render/mod.rs`/`post.rs` — **[0071] and [0046] have both landed, so that block is lifted and its "wants [0046] first" preference is discharged**. Read what the two left there first: `post.rs` now owns a chain-level param vocabulary (`CHAIN_PARAMS`) routed as `ParamRoute::Composite`, `Fold::Over` carries a payload, and `composite_into` decides per frame whether the seam belongs to the chain's last stage or to the scene. A second layer has to answer what `occlude` means when two scenes share one backdrop — and now also what `fb_*` means when two scenes share one trails accumulation, since [0046] routed those names through `ParamRoute::StageAndScene` and gave `PostStage` a `set_dt` / `set_feedback` pair on the same one-way route as `set_exposure`. |

## Recommended execution sequence

**Rewritten 2026-08-09, at [0046]'s close, because the shape of this roster changed rather than
its order.** The previous sequence was written when [0075] had four open engine prerequisites, and
it opened by naming [0046] "the critical path". **All four have now landed** — [0071], [0064]
and [0067] within a week of each other, and [0046] at this close. [0068] landed the same day and
changes nothing downstream by construction (it shipped a diagnosis and no fix, on purpose).

**So there is no critical path any more, and that is the headline.** What is left is three plans
that no longer gate each other on capability — only on *file contention* and on one deliberate
preference. Two consequences worth stating plainly, because a reader skimming rows would miss both:

- **[0075]'s Phases 4+ are unblocked.** The renaissance was waiting on this and nothing else. Its
  Phases 1-3 were already ungated; the whole plan is now takeable.
- **The [0053]/[0046] concurrency collision is discharged**, not deferred. [0046] landed first,
  [0053] ran second and has now **closed** (2026-08-09) — so the sequence below is two plans, and
  the bind-group-layout guard [0076] was told to wait for is in place.

**No plan here closes in one session** — every remaining one carries a `human` phase, so plan on
bringing the user in rather than picking on that criterion.

| # | Plan | Why here |
|---|------|----------|
| 1 | [0076] | Unblocked on every axis — [0071], [0046] and now [0053] have all landed (shared files: `render/mod.rs`, `post.rs`). **[0053]'s guard is why this is takeable rather than risky**: this plan adds a blend pass and duplicated stateful pipelines, which is exactly the bind-group-layout pressure that guard now asserts on, so a colliding layout fails at the commit instead of being blessed into a baseline. Read the two `min_binding_size` fixes in `background.rs` / `transition.rs` before adding a layout — an explicit size is the cheap separation, and `blend-bind-layout` is one of the two it already rescued. Before [0075]'s cohorts by preference, so the brief can assign layered worlds — a preference, not a gate. |
| 2 | [0075] | **Last by design** (ADR-0089): the fresh library is authored once, against a finished engine. Its four prerequisites are all landed, so this is now a scheduling choice rather than a wait. Its Phases 1-3 remain the exception — ungated instrument/doc fixes, take them whenever a session has room. |

**Confirmed 2026-08-11, at the cohorts 1-5 handoff: [0076] remains the next execution step.**
Cohort 6 (the collage) is gated on it, and none of the handoff's promoted backlog items
(0084, 0088, the 0009+0068 sparse-idiom pair with 0085, the 0055 tuple-roster re-raise) queues
ahead of it — each waits for its own ADR/plan after the renaissance, per [0075]'s Decision.

### The baseline-drift control any pixel-touching plan inherits

Kept here after [0053]'s close because it is not that plan's property — it applies to every plan
that could move a render. **Do not `git diff` the committed baselines.** On this box **8 of 20
drift from their committed bytes under `LMV_BLESS`** (`composite_bloom`, `composite_kaleido`,
`composite_overlap`, `composite_trails`, `line_joint_zigzag`, `lsystem`, `parametric_curve`,
`star_pattern`), so a naive diff convicts eight files the change never touched. Bless every binary
in scope (`--test golden --test composite --test line_joints --test attractor_trails`) and compare
**bless-to-bless**, then `git checkout -- core/tests/golden`.

[0053]'s close used a tighter form of this than the clean-`main` control it was handed, and it is
the one to reuse: bless twice **on the same branch**, differing only by reverting the change under
test. Bless output is deterministic run-to-run (it is bless-vs-*committed* that drifts), so the two
hash sets are directly comparable and everything except the change is held fixed. All 20 came back
identical, which is how "the two WARP fixes moved zero pixels" was established rather than argued.

The other recorded collision, "**[0046] should precede [0075] Phase 3**", is **discharged**: that
phase documents [backlog 0063](../design-backlog.md)'s `spin`x`fade` smear ceiling in *frames*, and
[0046] has now made the decay time-based, so the paragraph gets written once against final
semantics. The honest phrasing there is seconds rather than frames — and note the shipped
exponent is `fade^(dt / FALLBACK_DT)`, exactly `1.0` at the capture step, so every number measured
at 60 Hz or at capture `dt` stays correct as written.

[0045]: done/0045-linear-light-and-bloom.md
[0046]: done/0046-transformed-feedback.md
[0052]: done/0052-the-emitter-objects-that-spawn-fall-and-die.md
[0053]: done/0053-the-suite-stops-blessing-what-warp-gets-wrong.md
[0055]: done/0055-the-fold-edge-becomes-a-choice.md
[0062]: done/0062-the-chaos-game-grows-a-fern.md
[0065]: done/0065-the-mandala-interior.md
[0066]: done/0066-the-level-lever.md
[0069]: done/0069-the-instrument-that-sees-a-figure-leave-the-frame.md
[0061]: done/0061-the-build-stops-paying-for-what-it-is-not-building.md
[0064]: done/0064-the-symmetry-stage-and-the-banded-palette.md
[0067]: done/0067-the-curation-route.md
[0068]: done/0068-why-the-downbeat-rarely-locks.md
[0071]: done/0071-light-that-adds-without-covering.md
[0072]: done/0072-the-backdrop-joins-the-palette.md
[0075]: 0075-the-content-renaissance.md
[0076]: 0076-the-second-layer.md
[backlog 0038]: ../design-backlog.md
[backlog 0058]: ../design-backlog.md

### The six plans added 2026-08-04, and why they exist

They came from a **backlog sweep**, not from six separate requests: the user asked for the backlog
to be checked, the stale entries retired, and plans made from whatever was left that nothing else
already covered. What that produced is worth stating, because the shape of it is not obvious from
the rows above.

- **Seven entries were retired**, none of which carried a marker saying it was dead — 0015 (the
  half-linear band axis, landed with Plan 0048's second analysis window), 0020's content half (Plan
  0048 Phase 7's 368-gain retune), 0030 (landed in the content lane's own `craft.md`), 0036 (retired
  unfired), 0049 (carried into Plan 0055's judged A/B), 0051 (both `star_*` presets now ship triangle
  waves) and 0007's interior half (specified at last by [0065]). The backlog had been accumulating
  answered questions faster than it was closing them.
- **Five entries stay parked deliberately** — 0009 (informational), 0021 (the slew release, awaiting
  an author who wants it), 0032 (96 kHz, awaiting a report), 0038 and 0058 (content-lane retunes,
  routed not planned), and 0055 (attractor variety, which [0062] partly covers and whose own
  re-check condition just landed). **0058 has since closed** by content on 2026-08-04 (`859ec66`),
  so the parked content-lane retune is 0038 alone — worth knowing because two later documents kept
  pairing them.
- **Two of the six plans ship no capability at all.** [0068] ships a diagnosis and explicitly no
  fix; [0069] replaces a measure that was proved not to work. Both are here because the alternative
  — tuning a threshold, or calibrating a statistic that cannot separate the cases — is the move each
  one's ADR exists to refuse.
- **[0066] and [0071] each turned out to move zero pixels**, in both cases by *arithmetic* rather
  than by a chosen default: no golden fixture binds `exposure`, and `occlude` defaults to literal
  `1.0`. Neither was designed for that outcome; both plans check it as a phase failure rather than
  claiming it. **[0066] has since closed and the claim held exactly** — zero baselines modified
  across the whole plan, and it left behind the fixture that makes the premise false going forward
  (`composite_bloom_exposed.toml` is now the suite's only `exposure`-binding fixture), so the same
  reasoning cannot be reused unchecked. **[0071] has now closed and its claim held too, but it was
  the check rather than the arithmetic that established it**: the default at literal `1.0` was the
  argument, and what was run was an `LMV_BLESS` on the change re-encoding all 19 baselines
  hash-identical to an `LMV_BLESS` on clean `main`. That is the form to reuse — a bless-against-a-
  control, not a diff against the committed files, because three baselines on this machine
  (`lsystem`, `parametric_curve`, `star_pattern`) drift from their committed bytes under `LMV_BLESS`
  on clean `main` too, and a naive diff would have convicted the change of moving them.


## Standing (not a plan)

- **Plan [0061] Phase 9 — the one verification still outstanding** (2026-08-08). The plan is
  `done` and every `dev` phase landed. **Phase 8 ran and passed the same day**: the foobar plugin
  builds against the extracted `lmv-core-cabi` and `foo_lmv.dll` loads in foobar2000 v2 and
  renders. That closed the one risk [ADR-0072](../adrs/0072-the-c-abi-ships-from-its-own-crate.md)
  carried into C++ link time — the linked artifact renamed to `lmv_core_c.lib`, and CI has no
  plugin job that would have caught a stale path. What remains needs a CI run rather than this
  machine:
  - **Phase 9 — read a cache-warm CI run** (the *second* after the push; the first is a cold
    build, because a `[profile.dev]` edit and a new workspace member each invalidate `rust-cache`
    wholesale). It re-derives `COVERAGE_FLOOR` — currently **91**, measured on a hardware-GPU box
    where CI has WARP — and checks the one property
    [ADR-0073](../adrs/0073-the-windows-ci-critical-path.md) committed to: **`coverage` is the
    longest job**. If it is not, `check (windows-latest)` is build-dominated, and that is the
    single measurement that flips ADR-0073's Alternative A (merge the two Windows jobs) from
    rejected to worth taking — route it back to `architect` as a supplement rather than editing
    the job.
- **Plan [0071] Phase 5 — the retune `occlude` unblocked** (2026-08-09). The plan is `done` and
  every `dev` phase landed; this phase is `human` and was left undone **on purpose**, not missed.
  It is a `preset-author` pass raising the floors that were floored for the black rim, now that the
  ceiling above them is adjustable. **Run it as one pass with [backlog 0038]** — both are retunes of
  the same shipped set against a composite that moved underneath it, both are judged in motion over
  a lit backdrop, and doing them separately means walking the same presets twice. **The plan's own
  text says "0038 and 0058"; that is one entry out of date** — backlog 0058 closed by content on
  2026-08-04 (`859ec66`, all thirteen fold-binding presets now name a `kaleido_edge`), five days
  before this plan reached Phase 5. The three-way pass is a two-way pass. Two things the close
  measured that it should start from: **no shipped
  preset binds `occlude` today**, and at shipped brightnesses the default's effect is almost
  negligible — the ceiling binds where the figure is *dim*, so the presets worth walking first are
  the ones with a dimming depth cue (`swarm_storm`'s `depth_fade`, `lsystem_fern`'s `glow`-dimmed
  outer stems). `lsystem_fern.toml`'s header now says so in place.
- **[On-device validation — low-end Windows iGPU smoke](../on-device-validation.md)** — a
  hardware-gated checklist, **not** a phased plan and **not** in the roster above: it never blocks a
  plan from closing. Holds the low-end / older Windows iGPU checks (fps floor ≥ 60 @ 1080p; footprint
  on a second GPU vendor) the user can only run once that box is in hand. Ticked when run; deleted when
  empty. Currently home to the extracted Plan 0012 Phase 3 (also covers the identical Plan 0003 Phase 3
  iGPU-fps carry-forward).


## Recently closed

One line per plan. **The full close write-ups — review verdicts, the findings
each close recorded, the properties that outlived the plan — moved verbatim to
[README-archive.md](README-archive.md)** (Plan 0061 Phase 7b). Nothing was
deleted; it simply stopped being loaded into every session's context.

- [0053 — The suite stops blessing what WARP gets wrong](done/0053-the-suite-stops-blessing-what-warp-gets-wrong.md) — closed 2026-08-09. Review: **no blockers, two majors, four minors**. **The plan was written to prove the collisions benign and instead found two live mis-renders and fixed them.** `background-bind-layout` collided with the fullscreen scenes' single uniforms and rendered the fragment field a **flat grey** on WARP (`142.712` on all three channels against hardware's `131.010 170.559 141.381`); `blend-bind-layout` collided with `trails-bind-layout` and rendered a dissolve between two `trails`-binding presets wrong (`08.999 38.682 45.794` against `17.572 48.211 50.116`). Both fixed by an explicit `min_binding_size`, each isolated against a no-collision control that agrees to **0.02 of one 8-bit level**. That makes ADR-0058's "nothing is observed to be wrong" and "it fixes nothing that is currently broken" **false**, recorded in its [Outcome](../adrs/0058-bind-group-layout-collisions-carry-evidence.md#outcome--2026-08-09-at-plan-0053s-close). Nine colliding pairs became **four** allowlisted, each carrying a measurement; the shape the test keys on widened from binding *kinds* to kinds + visibility + whether a `min_binding_size` is declared, the first forced by ADR-0058's own boxed note and the second **measured here, twice independently**. Guard confirmed in the reverted direction at the review: dropping either fix back to `gpu::uniform` re-collides its pair and fails. **Zero baselines moved** — a bless-to-bless control differing only by reverting the two fixes came back hash-identical across all 20 PNGs. The line seam's lit-backdrop guard went from **15 channels** of reach to the whole stroke footprint (779 of 28 173 post-fix against 28 178 pre-fix) via a fourth capture at zero emitted light, and the swarm took the same arm. **Two majors left as findings, both consequences of the fix rather than defects in it:** the allowlist's `RIG` names neither adapter, in a repo whose ADR-0074 `Outcome` established five days earlier that this box's WARP 10.0.19041 is the outlier and CI's 10.0.26100 is not — so four `AGREES` entries grant permission on a build never measured; and `core/tests/background_composite.rs` still skips on every software adapter citing the quirk this plan **fixed**, so a check CI has never run may now be liftable (unmeasured — the attractor half is a different layout group). **Phase 3 is `human` and was run by `dev` under the user's explicit authorization at the gate.**
- [0046 — Transformed feedback: the past learns to move](done/0046-transformed-feedback.md) — closed 2026-08-09. Review: **no blockers, no majors, four minors, two nits**. **R2 lands, and with it the last engine gate on [0075]** — both accumulation buffers now resample their past through one shared transform, so a zoom is a tunnel and a rotation a spiral, and the standing `trails` frame-rate defect (a `fade` applied once per *frame*, a third as long at 144 Hz as at 48) is retired en route. **Zero pixels moved without opt-in**: all 20 pre-existing baselines hash-identical to a clean-`main` bless, three added, re-measured at this close after the merge. **[ADR-0037] was caught for the third time here** — `Trails::resolve` had been ignoring its `surface` argument on a premise a rotation falsifies — and the fix carries the negative control this rule has always needed: a spun ring boxes **45x46** at a portrait 100x160 target, against **44x71** with the aspect forced to `1.0`. **One contract narrowed:** `[feedback] blend` reaches the trails stage only, because the attractor deposits additively in one pass and a `max` there would cost a second pipeline — the exact WARP hazard the one-shader warp family avoids. The plan sentence changed, not the code. **Alternative D was overcharged**: including the attractor cost one shared `Transform` and one WGSL snippet concatenated into both shaders, not "a second shader and test surface". Two Phase 5 observations became [backlog 0082](../design-backlog.md) (`frame_ms_p99` spikes to 25.0 ms on preset switches, and the not-yet-built quality governor is specified to read that column) and [backlog 0083](../design-backlog.md) (RSS 385 → 663 MB over three minutes, with no no-feedback control beside it).
- [0068 — Why the downbeat rarely locks](done/0068-why-the-downbeat-rarely-locks.md) — closed 2026-08-09. Review: **no blockers, no majors, two minors, one nit**. **It shipped the diagnosis and no fix, as designed, and that was verified rather than assumed** — `CONFIDENCE_THRESHOLD` is still `0.25`, `BASS_WEIGHT` still `0.7`, the fold and the confidence measure are untouched, and the `effect_size` split is arithmetic-identical on every branch. The probe reads the gate rather than second-guessing it: `terms()` is `&self`, allocation-free, clock-free, and its `to_bits()` equality against the published `BarClock::confidence` is asserted on all six cases. **The named cause is the accent feature's bass weighting**, and the finding that names it is that backbeat rock/pop locks **48x worse** than four-on-the-floor — 0.14 % against 6.79 % over 98 minutes — because a bass accent marks every beat in one and the half-bar in the other, so it hardly ever marks the *bar*. [ADR-0082](../adrs/0082-the-downbeat-gate-holds-and-the-estimator-is-diagnosed-first.md) is **accepted** carrying that dated `Outcome`, including the limit that matters most: the 1 Hz log records **band levels, not per-beat accents**, so this is a ladder match plus a construction argument and *not* a direct measurement of the four alignment scores on real audio. **The repair has no ADR and no plan**, and that is a decision taken at this close rather than an omission: the route is named ([design-backlog 0042](../design-backlog.md), answered in place) but the fork an ADR would decide — a stronger accent feature versus a longer history window — is not yet a real fork, because ADR-0082's `Outcome` exonerates the fold and the confidence measure. It stays a backlog pointer until someone takes it.
- [0067 — The curation route](done/0067-the-curation-route.md) — closed 2026-08-09. Review: **no blockers and no code findings**. The one substantive item was a **factual error in the plan** — its claim that `bar` "stopped being a variable at ADR-0050" is false (`bar` is `VAR_NAMES[5]`, the beat phase in `[0, 1)`; ADR-0050 *added* `bar_phase` alongside it) — struck in the plan and in [ADR-0081](../adrs/0081-the-content-lane-lands-presets-and-architect-curates-the-set.md)'s `Outcome`, which repeated it. **The gate is now worth leaning on**: `reactivity` drives PCM through the real analyzer, with a non-vacuity test that has a positive control, and ADR-0081's gate-strength Negative is discharged. **Phase 1d is a recorded negative result** — the resolution ladder is flat because `frame_diff` scores occupancy and occupancy is scale-invariant, so `ANIM_FLOOR` and `SIZE` did not move and CI pays nothing; [backlog 0009](../design-backlog.md) now needs a coverage-aware statistic, which is the earned question. Two costs the lane refused to absorb went to [backlog 0080](../design-backlog.md) (reactivity 1.8x, ~85 % of it warm-up hops that are rendered and thrown away) and the Coral Oracle's gain exception to [backlog 0081](../design-backlog.md) (the house gain rule is written down nowhere).
- [0064 — The symmetry stage and the banded palette](done/0064-the-symmetry-stage-and-the-banded-palette.md) — closed 2026-08-09. Review: **no blockers and no code findings**; the three items raised were all stale text in the plan itself, corrected at the close (its "fourteen" baselines are **19**, its five LUT sample sites are **seven**, and its "duplicated at five sites" risk is **three** WGSL copies plus four calls into the one Rust function). ADR-0037 verified clean at `kaleidoscope.rs:1116` — the aspect comes from the render target, in the stage that has shipped that bug twice. All 19 pre-existing baselines byte-identical. **[ADR-0077](../adrs/0077-the-symmetry-stage-owns-one-coordinate-map.md) was accepted with an [Outcome](../adrs/0077-the-symmetry-stage-owns-one-coordinate-map.md#outcome--2026-08-09-at-plan-0064s-close): its "the inner rings alias severely" was inferred from a texel ratio and never observed** — six cutoffs, three sources, no visible onset — so `kaleido_inner` ships as styling with a protective side effect, not as the rescue. **First run of close-ceremony step 3b**: no near-duplicate geometry in any of the nine families, and nothing in `presets/` still pays for a fixed defect.
- [0071 — Light that adds without covering (`occlude`)](done/0071-light-that-adds-without-covering.md) — closed 2026-08-09. Review: no blockers, two majors, three minors, two nits. **Phase 5 is `human` and deliberately outstanding — see Standing.** The default stayed `1.0` by the user's look, no preset binds it, and **no golden baseline moved** (measured as a bless-against-a-clean-`main`-bless, all 19 hash-identical). Three ADR-0085 claims were falsified and are in its [Outcome](../adrs/0085-how-much-a-scene-occludes-the-backdrop-is-one-number.md#outcome-2026-08-09-after-plan-0071): there was no "one seam" (six sites, four shaders), the `Scene` trait *did* widen (`set_occlude`, its fourth optional method — it meets [ADR-0030](../adrs/0030-scene-target-size-hot-path-hook.md)'s three conditions, checked at the close), and the families *did* drift (additive scenes with an empty chain are unoccluded whatever they bind). It also emptied [ADR-0058](../adrs/0058-bind-group-layout-collisions-carry-evidence.md)'s `[Texture, Sampler]` group — see [0053]'s row.
- [0072 — The backdrop joins the palette](done/0072-the-backdrop-joins-the-palette.md) — closed 2026-08-09. Review: no blockers, no majors, four minors, three nits. **The last surface outside `[palette]` joined it**: no cosine copy remains in `background.rs`, and `saturation` / `palette_mix` fan out to the sky through one binding. Two of the plan's own claims were falsified and are recorded in [ADR-0086](../adrs/0086-the-backdrop-colours-through-the-preset-palette.md)'s `Outcome` — **the two fixtures it ordered re-blessed pin no pixels**, and its "fifteen" is 18 by its own grep (16 in scope, 3 moved). Zero golden baselines changed.
- [0061 — The build stops paying for what it is not building](done/0061-the-build-stops-paying-for-what-it-is-not-building.md) — closed 2026-08-08. Review: no blockers, two majors, three minors (all five doc drift, all repaired at the close). **Phases 8 and 9 are `human` and carried forward — see Standing.**
- [0074 — The figure colours by how far it has come](done/0074-the-figure-colours-by-how-far-it-has-come.md) — closed 2026-08-08. Review: no blockers, four minor items (three repaired at the close)
- [0073 — The fern unfurls and colours by what made it](done/0073-the-fern-unfurls-and-colours-by-what-made-it.md) — closed 2026-08-06. Review: no blockers, two minor doc items repaired at the close
- [0065 — The mandala interior: `star_pattern` stops being hollow](done/0065-the-mandala-interior.md) — closed 2026-08-06. Review: no blockers
- [0069 — The instrument that sees a figure leave the frame](done/0069-the-instrument-that-sees-a-figure-leave-the-frame.md) — closed 2026-08-06. Review: no blockers, three minor, one nit
- [0070 — Shaped marks](done/0070-shaped-marks.md) — closed 2026-08-05. Review: no blockers, one minor
- [0066 — The level lever](done/0066-the-level-lever.md) — closed 2026-08-05. Review: no blockers, one minor
- [0062 — The chaos game grows a fern](done/0062-the-chaos-game-grows-a-fern.md) — closed 2026-08-05. Review: no blockers, one major, four minor
- [0063 — The attractor keeps its depth](done/0063-the-attractor-keeps-its-depth.md) — closed 2026-08-04. Review: no blockers, one major, two minor
- [0036 — macOS and Windows release artifacts](done/0036-macos-and-windows-release-artifacts.md) — closed 2026-08-04. Review: no blockers, one minor, one nit
- [0055 — The fold edge becomes a choice](done/0055-the-fold-edge-becomes-a-choice.md) — closed 2026-08-04. Review: no blockers
- [0051 — the scene seam emits premultiplied alpha](done/0051-the-scene-seam-emits-premultiplied-alpha.md) — closed 2026-08-01. Review: no blockers, one major (an operator-doc gap, fixed at close), four minor
- [0039 — Line joins: the stroke stops coming apart at every vertex](done/0039-line-joins.md) — closed 2026-07-28. d Mode 4 review (no blockers; one major, three minors, two nits — nothing fixed in the close commit, the major is deliberately…)
- [0023 — Cross-preset visual transitions: MilkDrop-style dissolves between presets](done/0023-cross-preset-transitions.md) — closed 2026-07-26. Review: no blockers, no majors; five minors, one nit
- [0019 — Preset expression grammar v2: branching, math functions, tempo, typo warnings](done/0019-preset-grammar-v2.md) — closed 2026-07-25. Review: no blockers; one major, three minors, one nit
- [0015 — Preset-directory override + live iteration](done/0015-preset-dir-override-and-live-iteration.md) — closed 2026-07-25. Review: no blockers; one major, three minors, two nits
- [0029 — Attractor resize cost + ink-stage followups](done/0029-attractor-resize-cost-and-ink-followups.md) — closed 2026-07-25. Review: no blockers, no majors; two minors, two nits
- [0027 — Attractor ink-on-paper (engine-wide final tone-remap) + crisp trails](done/0027-attractor-ink-and-crisp-trails.md) — closed 2026-07-25. Plan 0029 or ADR-0030 rather than reworked
- [0025 — Full composite coverage: background + view transform for reaction-diffusion and attractor](done/0025-full-composite-coverage.md) — closed 2026-07-24. Review: no blockers, no majors; one minor, one nit
- [0028 — Parametric-curve shape params: radial offset + phase (audio-morphable rose geometry)](done/0028-parametric-curve-shape-params.md) — closed 2026-07-24. Review: no blockers, no majors; one minor, one nit
- [0020 — Shared palette system: gradient LUT, named + custom palettes, bindable color (all four scenes)](done/0020-shared-palette-system.md) — closed 2026-07-24. Review: no blockers, no majors; two minor, two nits
- [0026 — Calmer scene rotation: hold one scene by default, longer dwell, softened drop bias](done/0026-calmer-scene-rotation.md) — closed 2026-07-24. Review: no blockers, no majors; one minor, one nit
- [0018 — Engine-wide visual enrichment: zoom, atmosphere, easing, mirrors](done/0018-engine-wide-visual-enrichment.md) — closed 2026-07-23. Review: no blockers, no majors; three minor, two nits
- [0024 — Single-source the foobar component version + refresh stale plugin descriptions](done/0024-foobar-component-version-single-source.md) — closed 2026-07-23. passed Mode 4 review cold (no blockers, no majors, no minors — one nit)
- [0021 — Decouple preset content from code: build-time embedding + single-source system names](done/0021-decouple-preset-content-from-code.md) — closed 2026-07-23. Review: no blockers, no majors, no minors, no nits — a clean landing
- [0016 — GPU compute-particle scenes: strange attractors](done/0016-gpu-compute-particle-scenes.md) — closed 2026-07-23. Review: no blockers, no majors; two minor, three nits
- [0022 — Decouple the golden drift guard from shipped presets (per-system frozen fixtures)](done/0022-golden-fixtures-decouple-content.md) — closed 2026-07-23. Review: no blockers, no majors; one minor, one nit
- [0014 — Reaction-diffusion feedback scene + frame-rate-independent render clock](done/0014-reaction-diffusion-feedback-scene.md) — closed 2026-07-23. Review: no blockers, no majors; two minor, two nits
- [0009 — Live performance features (standalone)](done/0009-live-performance-features.md) — closed 2026-07-23. Review: no blockers, no majors; two minor deviations, both pre-flagged and reconciled
- [0017 — Green CI: reasoned ttf-parser advisory ignore + adapter-skip for headless GPU tests](done/0017-ci-green-advisory-and-gpu-tests.md) — closed 2026-07-23. Review: no blockers, no majors, no minors; two non-actionable nits
- [0010 — Line-geometry scenes: parametric curves, L-systems, star patterns](done/0010-line-geometry-scenes.md) — closed 2026-07-23. Review: no blockers, no majors; three minor, two nits
- [0013 — Headless scene capture + differential visual QA + golden images + shot CLI](done/0013-headless-scene-capture.md) — closed 2026-07-22. Review: no blockers, no majors; one minor, one nit
- [0008 — In-app preset browse overlay (standalone)](done/0008-preset-browse-overlay.md) — closed 2026-07-22. Review: no blockers, no majors
- [0012 — Measure the driver-memory floor + cull dead scenes](done/0012-memory-floor-measure-and-scene-cull.md) — closed 2026-07-22. Review: no blockers, no majors
- [0011 — Diagnostics harness + quick-win memory/perf trim](done/0011-diagnostics-and-memory-trim.md) — closed 2026-07-22. Review: no blockers, no majors; two nits
- [0007 — Curated preset library: robust loading + seed-on-first-run + C ABI v2](done/0007-curated-preset-library.md) — closed 2026-07-22. Review: no blockers, no majors
- [0004 — foo_lmv as an embeddable Default UI panel](done/0004-foobar-ui-element-panel.md) — closed 2026-07-21. Review: no blockers, no majors
- [0005 — Extract the lock-free ring into a wgpu-free crate for Miri](done/0005-miri-ring-extraction.md) — closed 2026-07-21. Review: no blockers, no majors
- [0003 — Generative scenes + data-driven presets](done/0003-generative-scenes-and-presets.md) — closed 2026-07-21. Review: no blockers
- [0006 — Versioning: single source of truth + cargo-release + surfacing](done/0006-versioning-wiring.md) — closed 2026-07-21. Review: no blockers, no majors
- [0002 — Rust enforcement tooling](done/0002-rust-enforcement-tooling.md) — closed 2026-07-21. Review: no blockers
- [0001 — Core + standalone MVP, then foobar parity](done/0001-core-and-standalone-mvp.md) — closed 2026-07-21. Review: no blockers; C ABI recorded in ADR-0003

## Roadmap (agreed 2026-07-21, revised same day for the live-show use case; numbers assigned when drafted)

> **2026-07-30: a second, strategic roadmap now exists —
> [docs/roadmap-visual-richness.md](../roadmap-visual-richness.md)** — from the user-requested
> "why is everything dull" architecture review. It diagnoses the five capability caps (single
> quality tier, decay-only feedback, 8-bit additive composite, one-scene/fixed-chain
> composition, starved grammar) and orders the themes R0-R6 that answer them. Item 3's
> remaining half below (quality tiers + governor) is that roadmap's R0. New visual-capability
> plans should cite it.

Execution order after Plan 0001, per the NFR interviews ([docs/nfr.md](../nfr.md)):

1. **Preset / scripting engine** — layered presets per
   [ADR-0002](../adrs/0002-layered-preset-architecture.md): TOML data + expression language
   driving built-in systems (feedback/warp, boids, walkers/growth, 3D scene), with an
   optional budgeted Rhai script for staged per-track arcs (NFR §10). Replaces "scenes are
   Rust code" — Plan 0001's Scene trait becomes the rendering vocabulary presets drive, so
   keep it thin. **Delivered by [Plan 0003](done/0003-generative-scenes-and-presets.md)** (layers
   1-2: fragment-field + swarm systems, data + expression presets); Rhai (layer 3), blending, and
   compute-scale particles remain follow-ups tracked in 0003.
2. **Live performance features** — line-in/audio-interface capture, scene triggers
   (auto-rotate + hotkey/MIDI + experimental track-change detection), fullscreen on a
   chosen display/projector, 4-hour soak stability (NFR §10).
   **Delivered by [Plan 0009](done/0009-live-performance-features.md)** (standalone borderless-
   fullscreen on a chosen display, line-in capture selection, drop-biased scene director +
   hotkeys, spectral track-change novelty nudge on the native `Frame`, `--soak` instrumentation;
   C ABI frozen). **MIDI triggers and the ≥4-hour projector-rig soak run remain** — MIDI is its
   own ADR-backed follow-up; the soak run is a `human` on-device carry-forward.
3. **Adaptive quality + runtime-memory trim** — quality tiers + frame-time governor for the
   60 fps iGPU floor (NFR §1), plus cutting the standalone's ~200 MB working set (NFR §12).
   The memory trim's primary lever — compiling wgpu with only the per-OS backend feature
   (DX12/Metal), dropping the dead Vulkan/GL paths — is a cheap, low-risk win that can
   front-run the full tier system. Both validated on the older iGPU test PC (footprint stated
   before/after; the backend trim must not regress the §1 floor).
   **Front-run by [Plan 0011](done/0011-diagnostics-and-memory-trim.md)** (diagnostics harness +
   the cheap NFR §12 levers, all-three-frontend, C ABI v3 / [ADR-0008](../adrs/0008-c-abi-v3-diagnostics.md)):
   it builds the before/after measuring stick and lands the wgpu-backend + swapchain trims. The
   **adaptive-quality tiers + frame-time governor remain** for a later plan — 0011 explicitly
   does not do them.
4. **Remaining v1 UX** — always-on-top / mini mode, settings persistence (NFR §11;
   fullscreen/multi-monitor land earlier with live features).
5. **Packaging & release** — GitHub release zip: unsigned standalone exe +
   `.fb2k-component` (NFR §8).

Later, unordered: better tempo tracking, preset sharing/library, signed installer.


## Conventions

- **Numbering:** sequential, zero-padded 4 digits. Take the next free number above, then
  bump it here in the same session.
- **Phases:** ordered, each one commit, each tagged `**Owner skill:**` with one value from the
  vocabulary `dev` (all code) or `human` (a task only the user can do). The `dev` skill reads
  this tag at the start of each phase; a missing tag is a Mode 4 review blocker. An optional
  `**Area:**` note (`core` / `standalone` / `plugin`) orients the reader but is not the tag.
- **Skills:** `architect` designs and owns `docs/`; `dev` implements all code. `architect`
  writes and closes plans; `dev` flips `draft → in-progress` at "go" and nothing else in the file.
- **Lifecycle:** `draft` → `approved` (user/architect validated it; ready for `dev`) →
  `in-progress` → `done` (then `git mv` to `done/` and drop from this roster). Review
  happens at plan end, in a fresh `/architect` session — not by the session that wrote
  the code.
