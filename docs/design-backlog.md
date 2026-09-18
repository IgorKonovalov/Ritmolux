# Design backlog — captured feedback, not yet promoted

Short, durable notes for design gaps surfaced during work but **not yet** decided into an ADR or
plan. Chiefly the `preset-author → architect` feedback handoff (a look wanting something the
preset grammar or engine can't express), plus any other "worth remembering, not worth acting on
yet" finding.

An entry here is **not** a commitment to build — it is a captured signal so the friction isn't
lost between sessions. Verify every entry against the code before acting on it — these are dated
snapshots, and the surface moves (same rule the lanes apply to their own references).

<!-- toc:begin depth=2 -->
- [Every live entry carries a probe, and something re-runs it](#every-live-entry-carries-a-probe-and-something-re-runs-it)
- [Where the promoted and closed entries went](#where-the-promoted-and-closed-entries-went)
- [Open entries](#open-entries)
- [Entry 0021 — from the Plan 0038 / ADR-0040 ruling](#entry-0021--from-the-plan-0038--adr-0040-ruling)
- [0021 — an "even fall" is not reachable with a one-pole, in any ordering](#0021--an-even-fall-is-not-reachable-with-a-one-pole-in-any-ordering)
- [Entry 0032 — from the Plan 0049 Phase 5 sample-rate sweep](#entry-0032--from-the-plan-0049-phase-5-sample-rate-sweep)
- [0032 — both analysis windows are sized in **samples**, so a third of the band axis loses resolution at 96 kHz](#0032--both-analysis-windows-are-sized-in-samples-so-a-third-of-the-band-axis-loses-resolution-at-96-khz)
- [0038 — mid-tone-dominated presets lost ~8 % luminance to the tonemap knee, and the library has not been retuned](#0038--mid-tone-dominated-presets-lost-8--luminance-to-the-tonemap-knee-and-the-library-has-not-been-retuned)
- [0042 — the downbeat estimator locks on ~3 % of audible time, so the gated bar variables are almost always fallback](#0042--the-downbeat-estimator-locks-on-3--of-audible-time-so-the-gated-bar-variables-are-almost-always-fallback)
- [Entry 0069 — from Plan 0070's close (2026-08-05)](#entry-0069--from-plan-0070s-close-2026-08-05)
- [0069 — there is no way to draw a two-tone object (a fill with a contrasting outline), because the composite is additive](#0069--there-is-no-way-to-draw-a-two-tone-object-a-fill-with-a-contrasting-outline-because-the-composite-is-additive)
- [Entries 0075-0076 — from the Plan 0074 Phase 6 content pass (2026-08-08), binding the root channel](#entries-0075-0076--from-the-plan-0074-phase-6-content-pass-2026-08-08-binding-the-root-channel)
- [0075 — `root_tint` earned no binding on either shipped IFS preset, and `root_hue` earned both](#0075--root_tint-earned-no-binding-on-either-shipped-ifs-preset-and-root_hue-earned-both)
- [Entries 0078-0079 — from Plan 0064's Phase 4 and Phase 6 (2026-08-09), the symmetry stage](#entries-0078-0079--from-plan-0064s-phase-4-and-phase-6-2026-08-09-the-symmetry-stage)
- [0079 — an accumulating figure rendered with `trails = 0` is not a sparse source, it is a blank one, and a whole third of a decision grid was unreadable because of it](#0079--an-accumulating-figure-rendered-with-trails--0-is-not-a-sparse-source-it-is-a-blank-one-and-a-whole-third-of-a-decision-grid-was-unreadable-because-of-it)
- [Entries 0084-0089 — from the Plan 0075 cohorts 1-5 handoff (2026-08-11)](#entries-0084-0089--from-the-plan-0075-cohorts-1-5-handoff-2026-08-11)
- [0087 — `reaction_diffusion` has no glow of its own, and the engine bloom's threshold sits above where its output lives](#0087--reaction_diffusion-has-no-glow-of-its-own-and-the-engine-blooms-threshold-sits-above-where-its-output-lives)
- [0092 — every figure this engine draws is unlit, and two reference images ask for a shaded one](#0092--every-figure-this-engine-draws-is-unlit-and-two-reference-images-ask-for-a-shaded-one)
- [0094 — the `frame_ms_p99` tail is not switch-correlated, so the steady-state column does not remove it](#0094--the-frame_ms_p99-tail-is-not-switch-correlated-so-the-steady-state-column-does-not-remove-it)
- [0095 — the backdrop ramp makes parallel stripes only, and a converging fan cannot be lit *and* darkened](#0095--the-backdrop-ramp-makes-parallel-stripes-only-and-a-converging-fan-cannot-be-lit-and-darkened)
- [0100 — "hand-drawn" is edge wobble, not spike-length variation, and the star arm has no lever for it](#0100--hand-drawn-is-edge-wobble-not-spike-length-variation-and-the-star-arm-has-no-lever-for-it)
- [0101 — the mark roster cannot morph between silhouettes, and two other rosters in this engine already do](#0101--the-mark-roster-cannot-morph-between-silhouettes-and-two-other-rosters-in-this-engine-already-do)
- [0108 — the conversion tail: HLSL arrays (~71 files) and 218 MD2 presets that convert but render blank](#0108--the-conversion-tail-hlsl-arrays-71-files-and-218-md2-presets-that-convert-but-render-blank)
- [0109 — disk textures are 88.7 % of every MilkDrop conversion failure, and the exclusion's trigger condition is already met](#0109--disk-textures-are-887--of-every-milkdrop-conversion-failure-and-the-exclusions-trigger-condition-is-already-met)
- [0125 — every diffused frame is an upscale: both profiles diffuse well below the stream's own resolution](#0125--every-diffused-frame-is-an-upscale-both-profiles-diffuse-well-below-the-streams-own-resolution)
- [0126 — a render is one prompt, one seed and one preset from first frame to last, so nothing varies across a track](#0126--a-render-is-one-prompt-one-seed-and-one-preset-from-first-frame-to-last-so-nothing-varies-across-a-track)
- [0154 — a swap spawns a thread that creates a COM object, and one activation in 22 failed with `REGDB_E_CLASSNOTREG` where the retry budget cannot tell that from a dead device](#0154--a-swap-spawns-a-thread-that-creates-a-com-object-and-one-activation-in-22-failed-with-regdb_e_classnotreg-where-the-retry-budget-cannot-tell-that-from-a-dead-device)
- [0165 - the windowed app cannot ask for the discrete GPU, so every windowed frame-time figure this project has quoted is an integrated-GPU figure](#0165---the-windowed-app-cannot-ask-for-the-discrete-gpu-so-every-windowed-frame-time-figure-this-project-has-quoted-is-an-integrated-gpu-figure)
- [0187 — two measurements of the same console on the same adapter class disagree by 2x, and nothing explains which one the machine actually does](#0187--two-measurements-of-the-same-console-on-the-same-adapter-class-disagree-by-2x-and-nothing-explains-which-one-the-machine-actually-does)
- [0219 — a `ctl/preset` datagram on loopback never reached the listener's queue, in 3 of 79 loaded runs, and nothing counted it](#0219--a-ctlpreset-datagram-on-loopback-never-reached-the-listeners-queue-in-3-of-79-loaded-runs-and-nothing-counted-it)
- [0220 — a headless walk of the system roster stalls at `emitter`: the ping sent with the ask is answered and the preset never reaches the screen](#0220--a-headless-walk-of-the-system-roster-stalls-at-emitter-the-ping-sent-with-the-ask-is-answered-and-the-preset-never-reaches-the-screen)
- [0221 — the run-alone override costs `-P fast` 165 s, twice its tests' serial time, because each of its 18 testcases drains the machine separately](#0221--the-run-alone-override-costs--p-fast-165-s-twice-its-tests-serial-time-because-each-of-its-18-testcases-drains-the-machine-separately)
- [Entries 0227-0235 — from the Plan 0189 Phase 8 watched runs (2026-09-15), all archived](#entries-0227-0235--from-the-plan-0189-phase-8-watched-runs-2026-09-15-all-archived)
- [0236 — the `.claude/` park reads a phase's declared `Files touched`, and Plan 0190's own Phase 9 declared its three `.claude/` files in prose](#0236--the-claude-park-reads-a-phases-declared-files-touched-and-plan-0190s-own-phase-9-declared-its-three-claude-files-in-prose)
- [0237 — the session allowlist bounds a deletion by four literal path shapes, so a path the shell expands escapes the lane](#0237--the-session-allowlist-bounds-a-deletion-by-four-literal-path-shapes-so-a-path-the-shell-expands-escapes-the-lane)
- [0239 — three per-preset suites are 54 % of the workspace suite and grow with every shipped preset, because each rebuilds its own headless renderer](#0239--three-per-preset-suites-are-54--of-the-workspace-suite-and-grow-with-every-shipped-preset-because-each-rebuilds-its-own-headless-renderer)
- [0240 — a merged plan never leaves `queue.json`, and the exemption that tolerates it is keyed on a gitignored file](#0240--a-merged-plan-never-leaves-queuejson-and-the-exemption-that-tolerates-it-is-keyed-on-a-gitignored-file)
- [0241 — the session allowlist is asserted against a model of the CLI's matcher, and the first unattended run falsified the model on a case it asserts](#0241--the-session-allowlist-is-asserted-against-a-model-of-the-clis-matcher-and-the-first-unattended-run-falsified-the-model-on-a-case-it-asserts)
- [0242 — the conductor's gate skips a check whose precondition a lane never has, and says nothing, while the pre-push hook doing the identical skip prints a notice](#0242--the-conductors-gate-skips-a-check-whose-precondition-a-lane-never-has-and-says-nothing-while-the-pre-push-hook-doing-the-identical-skip-prints-a-notice)
- [0243 — the served version-line rule is anchored to a column and a basename, not to the workspace section, so a dependency table edited in place would serve a code change to `-P fast`](#0243--the-served-version-line-rule-is-anchored-to-a-column-and-a-basename-not-to-the-workspace-section-so-a-dependency-table-edited-in-place-would-serve-a-code-change-to--p-fast)
- [0244 — a custom wave is drawn from the source's traces but is neither smoothed nor scaled like the eight built-in figures](#0244--a-custom-wave-is-drawn-from-the-sources-traces-but-is-neither-smoothed-nor-scaled-like-the-eight-built-in-figures)
- [0245 — the converted warp space has no pixel baseline, because every golden fixture is square and the two chains are identical there](#0245--the-converted-warp-space-has-no-pixel-baseline-because-every-golden-fixture-is-square-and-the-two-chains-are-identical-there)
- [0246 — the local `cargo doc` mirror covers one crate of five, and the four it leaves out are still unreachable until after a push](#0246--the-local-cargo-doc-mirror-covers-one-crate-of-five-and-the-four-it-leaves-out-are-still-unreachable-until-after-a-push)
- [0247 — a suite run by hand inside a lane records into that lane's own ledger, which is the one place no gate reads](#0247--a-suite-run-by-hand-inside-a-lane-records-into-that-lanes-own-ledger-which-is-the-one-place-no-gate-reads)
- [0248 — nothing in this repo asks whether a groundless luminous field is a composition or a fill, and four shipped presets are the open cases](#0248--nothing-in-this-repo-asks-whether-a-groundless-luminous-field-is-a-composition-or-a-fill-and-four-shipped-presets-are-the-open-cases)
- [0249 — `warp_mesh`'s `zoom` doc says the opposite of what the shader does, and four generated surfaces carry it](#0249--warp_meshs-zoom-doc-says-the-opposite-of-what-the-shader-does-and-four-generated-surfaces-carry-it)
- [0250 — a conductor session cannot run a command that carries an environment assignment, and two documented repairs need one](#0250--a-conductor-session-cannot-run-a-command-that-carries-an-environment-assignment-and-two-documented-repairs-need-one)
- [0251 — `warp_mesh`'s level mode draws bands but not an ink class, because coverage is a continuum nothing thresholds](#0251--warp_meshs-level-mode-draws-bands-but-not-an-ink-class-because-coverage-is-a-continuum-nothing-thresholds)
- [0252 — the conductor's gate is a hand-maintained copy of the Node gate list, and it has fallen behind twice](#0252--the-conductors-gate-is-a-hand-maintained-copy-of-the-node-gate-list-and-it-has-fallen-behind-twice)
<!-- toc:end -->

## Every live entry carries a probe, and something re-runs it

Since [ADR-0108](adrs/0108-a-backlog-claim-about-the-repo-carries-an-executable-probe.md) the
mechanical half of a verification is a dated bullet a script re-runs, at the three call sites the
doc-link checker already occupies (`pre-push`, the architect close ceremony, and the CI `links` job,
which is the un-bypassable one). **`scripts/check-backlog-claims.mjs` is the authority on the
grammar** — it is the parser, so a second copy here could only drift away from it. Its header
carries the three forms, the tracked-path rule and the regex-escaping caveat, and the advisory half.
Enforced, not advised: a live `##` entry with no dated bullet under it reds the gate at its own line.

Three things the parser cannot tell you, and they are why the probe sits beside the claim rather
than in a manifest:

- **Green means the stated reduction still holds, never that the entry is true.** A probe checks
  only the reduction its author chose; whether that reduction covers the claim is for a reader to
  see. Entry 0081's stamp was dated, recent, accurate — and verified the half of the entry that
  survived rather than the half in its own title.
- **Prefer the narrowest path that carries the claim** (`core/src/render/tier.rs`, not `core/src`).
  It keeps the staleness advisory quiet, and it is the better probe for the same reason.
- **`absent:` on a common word is a probe that can never fail**, and it reads as verification while
  checking nothing. A claim with no honest reduction says so — `unprobeable: <why>`, which the
  checker accepts and rosters, and that visible count is what keeps the opt-out from becoming a
  blanket.

**The lifecycle, in one line:** raised here → **PROMOTED** (the plan that takes it is approved; the
body moves to the archive and a `Promoted` ledger row points at the plan, per
[ADR-0206](adrs/0206-a-promoted-backlog-entry-leaves-the-live-file.md)) → **CLOSED** (the plan
landed; the close appends the marker to the archived body and moves the row to `Closed`). An entry
only *partly* promoted stays here, with a dated bullet naming which half a plan took.

## Where the promoted and closed entries went

**[`design-backlog-archive.md`](design-backlog-archive.md)**, whose `## The ledger` indexes it in two
tables — promoted and closed. Nothing was ever deleted — every body moved across verbatim. So this
file holds only asks **no approved plan owns**; a `**Closes:** design-backlog NNNN` line in an active
plan names an entry that is already in the archive.

**Read the archive rather than the ledger whenever you are about to act on the same surface.** The
bodies are kept for the corrections they carry, not for the outcomes: five entries had their causal
claim *inverted* under verification, and a ledger row cannot express one of them. The archive's own
head names all five. How that archive came to exist — three sweeps in ten days, and a lifecycle rule
that had no carrier until the close ceremony gained step 3c — is the last section of it.

## Open entries

---

## Entry 0021 — from the Plan 0038 / ADR-0040 ruling

Not from the content lane. Raised by an `architect` ruling that had to falsify a claim in order to
answer a `dev` finding, and left a real want with nowhere to live.

---

---

## 0021 — an "even fall" is not reachable with a one-pole, in any ordering

- **Raised:** 2026-07-28, from
  [ADR-0040's Outcome](adrs/0040-spectrum-level-curve-applies-before-the-easing.md#outcome-2026-07-28-after-plan-0038-phase-3s-measurement).
  ADR-0040 chose the spectrum level curve's position in the pipeline partly to buy "a perceptually
  even fall". Plan 0038 Phase 3 measured it, and the closed form settles it: **no ordering can deliver
  that**, because every `[smoothing]` response in this engine is a one-pole exponential and a power of
  an exponential is an exponential.
- **Verified against code:** yes. `Easing::step` (`core/src/preset/schema.rs:223`) is
  `held + (1 - exp(-dt/tau)) * (raw - held)`, one constant per direction (ADR-0035), no shape.
  `Smoother::smooth` (`core/src/render/mod.rs:317`) is the same arithmetic for bindings, and the
  spectrum scene's per-element easing calls the same method.
- **Verified 2026-08-15** — the mechanism citation still resolves, and the rate-limited release
  this entry proposes as the cheap shape is still unbuilt:
  `present: Easing in: core/src/preset/schema/easing.rs`, `absent: slew in: core/src`. The evenness
  arithmetic itself is not a repo claim and is not reduced here.

**The measurement, for the record.** An exponential spends **30 %** of its settling time covering the
first half of its travel (`ln2 / ln10` = 0.301); a linear ramp spends **56 %**. Both curve orderings
measure 0.301 when measured to settlement. So "even" is a ~1.8x gap from what the engine can currently
produce, in either ordering, at any exponent.

**The want is legitimate and has been asked for twice.** This is the half of
[0006](design-backlog-archive.md) that
[ADR-0035](adrs/0035-asymmetric-attack-release-easing.md) deliberately did not take — 0006's origin
ask was literally "use some qubic bezziere function or something", and the asymmetric one-pole
answered the *symmetry* half of that defect while leaving the *shape* half untouched. A meter that
falls at a constant rate is the classic look this cannot make.

**The cheap shape, if it is wanted:** a **rate-limited (slew) release** rather than a curve —
`held += clamp(raw - held, -rate * dt, +rate * dt)` — which is a third `[smoothing]` form beside
today's scalar and `{ attack, release }`, needs **no** new per-binding state (the slot exists), stays
stateless from the author's side, and is frame-rate-independent for the same reason the one-pole is
(ADR-0019's injected real `dt`). A constant-rate fall is exactly evenness 0.556. The nameable rejected
alternative is a full parametric ease curve, which needs a notion of "a transition in progress" and a
rule for a target that moves mid-ease — the same reason it lost in 0006.

**Not the thing ADR-0035 already rejected.** Its Alternative C was a `slew(x, up, down)` **function in
the grammar**, refused outright because expressions are pure and stateless by hard invariant. The
proposal here is the opposite location: a `[smoothing]`-table *form*, where the state already lives and
where the asymmetric one-pole itself landed. That distinction is the whole reason this is a fresh entry
rather than a re-litigation, and any ADR must say so explicitly or it will read as reopening 0035.

**ADR-worthy** as a short supplement to [ADR-0019](adrs/0019-eased-parameters.md) /
[ADR-0035](adrs/0035-asymmetric-attack-release-easing.md) if acted on. **Not urgent**: nothing shipped
is broken, and unlike most entries here this one is a *new capability* rather than a wall the content
lane has already hit. It wants a preset-author "I want this look and cannot get it" before it wants a
plan — the evidence so far is an architect's arithmetic, not a frustrated author.

---

---

## Entry 0032 — from the Plan 0049 Phase 5 sample-rate sweep

---

---

## 0032 — both analysis windows are sized in **samples**, so a third of the band axis loses resolution at 96 kHz

- **Raised:** 2026-07-30, by `dev` implementing Plan 0049 Phase 5 item 3 (the sample-rate coverage
  gap Plan 0048's Mode 4 review named), and confirmed at that plan's close review.
- **Verified against code:** yes — measured, and the measurement is pinned by
  `core/src/dsp/fft.rs::the_axis_holds_at_the_rates_we_do_not_develop_at`.
- **Verified 2026-08-15** — both windows are still sized in samples, and the sweep that measured
  the consequence is still pinned: `present: WINDOW_SIZE: usize = 2048 in: core/src/dsp/mod.rs`,
  `present: LOW_WINDOW_SIZE: usize = 8192 in: core/src/dsp/mod.rs`,
  `present: the_axis_holds_at_the_rates_we_do_not_develop_at in: core/src/dsp/fft.rs`. A literal
  sample count is the claim, so the day either becomes a duration this goes red — which is the
  fix this entry asks for and therefore the re-read it wants.

Every band-layout test was at 48 kHz until Plan 0049. `AudioFormat` accepts 8 kHz-384 kHz, and
WASAPI loopback runs at whatever the device mix format is — 96 kHz is an ordinary setting on a
discrete DAC or an audio interface. The sweep measured:

| rate | crossover band | crossover | bin-starved bands |
|------|----------------|-----------|-------------------|
| 44.1 kHz | 19 | ~223 Hz | 8 |
| 48 kHz | 20 | ~246 Hz | 8 |
| 96 kHz | 27 | ~487 Hz | **21** |

**44.1 kHz found nothing, which is the good outcome** — one band lower, same starved count — and
that is the rate foobar hands the plugin for CD material, so the plugin path is unaffected.

**The mechanism at 96 kHz.** `WINDOW_SIZE` and the long window are both fixed in **samples**, not
seconds, so at twice the rate each spans half the time and resolves half the frequency detail. The
crossover rides `sample_rate / WINDOW_SIZE`, and the region the long window still cannot resolve
grows from 8 bands to 21 — a third of the axis — because the widening cascades through `fill`'s
`prev_hi` chain, each widened band pushing the next one's floor up.

**This is not an ADR-0049 regression.** That ADR's claim is about band **edges in Hz** below the
crossover, and those do not move at any rate. It is physics working as specified: a higher sample
rate buys time resolution and spends frequency resolution. But a third of the axis reading at
one-bin resolution is a real difference in what a preset's `bin()` sees on a 96 kHz device, and it
was invisible before the test existed.

### What a fix would be

Size both windows in **seconds** rather than samples, so the analysis time-span — and therefore the
frequency resolution behind every band — is the same on every device. That has real consequences to
weigh (a rate-dependent FFT size, its cost at 192/384 kHz, whether `HOP_SIZE` follows, and what it
does to the onset envelope's cadence and to `docs/nfr.md`'s window budget), and it re-opens a
decision ADR-0049 made. **So it is ADR territory, not a patch** — which is exactly why Plan 0049
recorded it rather than acting on it.

### Priority

Low and honest about it. Nobody has reported it, the two rates that dominate (44.1 / 48 kHz) are
clean, and the failure is a coarser low end rather than anything broken. Worth taking the day
someone runs the standalone on a 96 kHz interface and says the sub-bass reads mushy — at which
point this entry is the starting measurement rather than a fresh investigation.

---

---

## 0038 — mid-tone-dominated presets lost ~8 % luminance to the tonemap knee, and the library has not been retuned

- **STILL OPEN 2026-08-13, and a document that says otherwise is wrong.**
  [`docs/plans/README.md`](plans/README.md)'s Plan 0080 Phase 7 write-up states that *"the
  **tonemap-knee** half of that pairing is now measured away."* **It is not.** What Plan 0080 Phase 7
  retired is a *different* suspicion raised at its own close — that `bg_bright = 0.85` was reaching
  the tonemap's shoulder on the **backdrop ramp** — settled by finding 0 % of the scanned column
  rail-pinned on any channel in any of the three probes. That measurement is about a backdrop
  gradient. **This entry is about mid-tone figure luminance on attractor presets**, measured as
  `attractor_clifford` 82.54 → 75.91 mean luma, and no backdrop measurement speaks to it. The two
  were conflated because both mention the tonemap.
- **Verified 2026-08-29** at the Plan 0104 close — **the "lever is unused" half of this entry is
  falsified, and the retune it asks for is not.** The reduction that now stands for this entry is
  that its own measured subject is still untouched:
  `absent: ^exposure in: presets/attractor_clifford.toml` — red the day someone retunes the
  preset whose −8.0 % opened this entry, which is exactly when it should be re-read. This entry said twice (2026-08-13, 2026-08-15)
  that exactly **one** shipped preset binds `exposure` (`lsystem_vellum.toml:60`). **Sixteen do**,
  and fifteen of them landed in [Plan 0104](plans/done/0104-the-library-stops-being-lopsided.md):
  its Phase 2 found that a branching or line figure has too little area for a level term to
  register on the stroke, and moved the level response to a whole-frame stage — `exposure` or
  `bg_bright` — on cohort after cohort. So `exposure` is now a routine authoring lever rather
  than an unused one, and any argument here resting on its rarity is void.
- **What survives that correction is the whole of the ask.** None of the fifteen new binders is in
  the population this entry names — *the attractor family, the softer `fragment_*`, `swarm_drift`*
  — which is the set of presets with no over-range peak, and not one of them was touched by
  Plan 0104. The measured −8.0 % on `attractor_clifford` is unaddressed. **The entry stays live.**
- **Why no gate caught this, which is the reusable part.** The claim is carried as
  `unprobeable: ... the grammar deliberately has no count verb (ADR-0108, Notes)`, so
  `scripts/check-backlog-claims.mjs` reported green across every run of the plan that falsified it.
  This is the case the close ceremony prints the `unprobeable:` roster for: the roster is the set of
  claims nothing checks, and a claim in it decays silently until a human reads it against the tree.
- **ROUTED, and now scheduled:** it is §4 of [`content-brief.md`](content-brief.md), paired with Plan
  0071's standing `occlude` retune as one pass over the shipped set. That brief also records the
  other correction this entry's routing carries — the plan text says to run it "with 0038 and 0058",
  but **0058 closed by content on 2026-08-04**, five days before Plan 0071 reached Phase 5, so the
  three-way pass is a two-way pass.
- **Raised:** 2026-07-31, from `architect`, at Plan 0045's Mode 4 review.
- **Verified against code:** yes — measured, not inferred (numbers below).
- **Verified 2026-08-15, and its headline claim is superseded above — sixteen presets bind
  `exposure`, not one.** What that dated check still establishes stands: the original binding is
  present — `present: ^exposure in: presets/lsystem_vellum.toml`. The count around it never
  reduced, and that is why the falsification went unseen for a whole plan:
  `unprobeable: exactly one shipped preset binds exposure is a claim about how many files match,
  and the grammar deliberately has no count verb (ADR-0108, Notes)`. The document this
  entry corrects still carries the sentence it corrects: `present: tonemap-knee in: docs/plans/README.md`
  — which goes red when that paragraph is next rewritten, and that is the moment to re-read whether
  the correction is still owed.
- **For:** `preset-author`. This is genuinely content-lane work; the engine behaved as designed.
- **ROUTED 2026-08-01 → `preset-author`, as a content pass rather than a plan.** The user's
  call at the Plan 0051 close: this needs no engine change and no ADR, so it goes to the lane
  directly. It pairs naturally with [0040](design-backlog-archive.md) (**closed 2026-08-09**; its retune half is Plan 0071 Phase 5, which this should run with) — both are retunes of the same shipped set
  against a composite whose behaviour has changed under them.

The user's report was "clifford is really dim". Rendering `attractor_clifford` at an identical
stimulus on `main` and on the Plan 0045 branch (640x360, 90 frames, hardware adapter):

| preset | main | branch | |
|---|---|---|---|
| `attractor_clifford` | mean luma 82.54 | 75.91 | **-8.0 %** |
| `attractor_leviathan` | mean luma 63.98 | 67.70 | **+5.8 %** |

That is not drift. It is the tonemap knee's documented price, to the decimal: `tonemap.rs`'s
`KNEE` docstring says a linear 0.8 mid-tone now presents at 0.733, which is -8.4 %. **The split is
the whole story.** Clifford is a diffuse particle cloud living almost entirely in the mid range, so
it pays the knee and collects none of the headroom above 1.0. Leviathan has genuinely over-range
cores, so it gains. Plan 0045 chose to pay this on mid-tones rather than on highlights, deliberately
and in writing — the consequence is simply that every preset shaped like Clifford now reads dimmer.

**The lever already exists and is one line:** `exposure` (default 1.0) is a linear multiplier ahead
of the tonemap, added by this same plan for exactly this. `exposure = "1.1"` restores Clifford's
level without re-balancing a single element against its own background, which is what raising
per-element `brightness` would force. The population to check is presets with no over-range peak —
the attractor family, the softer `fragment_*`, `swarm_drift`.

**A related record correction, since this is the entry about the luminance model.** This file's own
`0034` section (the "why it works, mechanically" passage under the Supernova table, around line
1561) still says "the frame clips per channel" in the present tense, and reasons from it. That
premise retired with Plan 0045. The *conclusion* stands and is if anything stronger — geometry
still has somewhere to go when luminance does not — but the mechanism is now a roll-off, not a
clip. Per this file's append-only rule the passage is left standing; this paragraph is the
correction.

---

---

## 0042 — the downbeat estimator locks on ~3 % of audible time, so the gated bar variables are almost always fallback

- **The instrument half is now built, 2026-08-25 — the entry stays live and its headline number is
  untouched.** [Plan 0117](plans/done/0117-the-downbeat-log-sees-the-counter-it-folds-over.md)
  appended `fold_beat` and `grid_bar_phase` to `--downbeat-log`, so the sentence closing the bullet
  below — *"an instrument that logs `beat_in_bar` / `bar_index` ... is the cheapest next step and is
  not yet filed"* — is discharged. **Nothing was measured with it.** The two readings that bullet
  says are inseparable are still inseparable, because no capture carries the columns; what changed
  is that spending a capture is now a `human` call rather than an unbuilt prerequisite.
- **Verified 2026-08-25** — the columns exist: `present: fold_beat in: standalone/src/downbeatlog.rs`
- **HALF-DISCHARGED 2026-08-25, and the entry stays live** — [Plan 0095](plans/done/0095-the-downbeat-fold-gets-a-musical-beat.md)
  closed, and it built the repair the 2026-08-15 bullet said was still unbuilt: the fold now buckets
  over a tempo-driven bar grid (`core/src/dsp/grid.rs`) instead of over `beat_index`. **The cause
  that bullet corrected is fixed; this entry's headline number is not.** Measured against a
  reconstruction of the pre-0095 fold on the same captures, the share of hops over the gate moved
  `0.00 → 2.36 %` on rock/pop, `0.79 → 3.67 %` on hip-hop and `4.16 → 0.42 %` on techno — so the
  trio is still counter-derived the overwhelming majority of the time, which is exactly what this
  entry's title says. What changed is *why*: it is no longer a fold indexed by a unit that is not a
  beat. Two things now bound the remaining shortfall, and either could be the successor. **The
  accent feature** is still 70 % bass band (ADR-0082's `Outcome`), and **the gate is now binding
  where it never was** — rock/pop's corrected effect size reached a p90 of 0.2060 against
  `CONFIDENCE_THRESHOLD = 0.25`, i.e. the distribution moved up *under* the gate rather than through
  it, and ADR-0082's reason for that threshold was argued when the estimator had no signal at all.
  That is not licence to lower it; it is the first evidence a decision about it could be made
  against. **Nothing in this repo can separate the two**, because no log column carries the grid —
  `--downbeat-log` predates it. An instrument that logs `beat_in_bar` / `bar_index` beside the
  existing accent decomposition is the cheapest next step and is not yet filed.
- **PROMOTED A THIRD TIME 2026-08-15 → [ADR-0109](adrs/0109-the-beat-clock-counts-onsets-not-beats.md) +
  [Plan 0095](plans/done/0095-the-downbeat-fold-gets-a-musical-beat.md)**, and **this entry's stated cause
  is corrected here rather than left standing.** Plan 0086 ran its measurement and closed at Phase 2:
  the cue was never changed, so the repair this entry has now been promoted for three times is still
  unbuilt and the entry stays **live**. What the measurement found is upstream of the cue. `beat_index`
  counts **onset-detector events, not musical beats** — 1.73x / 1.35-2.10x / 1.76x detections per beat
  on three genres, wandering across 1x, 2x and 4x inside a single track, against a synthesized control
  that reads exactly 1.00 — so the 4/4 fold is indexed by a unit that is not a beat and a bar-locked
  accent precesses across all four alignments. The bass-weighted accent named below (Plan 0068 Phase 3,
  and repeated in the 2026-08-15 verification bullet) is therefore **at best secondary**: it is a real
  property of the feature, but it is not what holds the publish rate down, and a second accent band
  would not have moved it. The measured rates in this entry's body remain accurate as *outcomes*; the
  mechanism sentence attached to them does not.
- **PROMOTED AGAIN 2026-08-13 → [ADR-0097](adrs/0097-the-downbeat-cue-is-chosen-against-per-beat-evidence.md) +
  [Plan 0086](plans/done/0086-the-downbeat-finds-a-cue-that-is-not-the-kick.md)** — the **repair**, which
  the 2026-08-09 answer below explicitly left unwritten. The plan does not open by building the cue:
  ADR-0082's own `Outcome` records that the 1 Hz log carries **band levels, not per-beat accents**,
  so "the accent feature is the cause" is a ladder match plus a construction argument. Phase 1
  therefore builds the per-beat `DownbeatTerms` capture — the decomposition Plan 0068 wrote and
  nothing outside its tests has ever called — and the cue is chosen at a `human` gate from a ranked
  shortlist, because at least three failures fit the same evidence (a narrow accent, a 2-periodic
  degeneracy where alignments 0 and 2 tie, or a thin history window) and they want different
  repairs. `CONFIDENCE_THRESHOLD` still does not move.
- **PROMOTED 2026-08-04 → [ADR-0082](adrs/0082-the-downbeat-gate-holds-and-the-estimator-is-diagnosed-first.md) +
  [Plan 0068](plans/done/0068-why-the-downbeat-rarely-locks.md)** — as a **diagnosis**, not a fix. The
  ADR records the one thing this entry insists on: `CONFIDENCE_THRESHOLD` does not move to buy lock
  rate, because adjusting a safety gate using data collected while the gate was closed is circular.
  The plan builds the decomposition the 1 Hz column cannot give (four alignment scores, raw and
  corrected effect size), degrades a known-good pattern along three axes to find which term
  collapses first, and ends with a named cause. The repair is a follow-on plan written against the
  diagnosis.
- **Raised:** 2026-08-02, from `architect`, running [Plan 0048](plans/done/0048-analysis-v2-and-the-retune.md)
  Phase 6 (`human`) with the user.
- **Measured, not impressionistic:** 8.8 minutes through the live app on the `v0.28.1` release
  build, 517 log rows at 1 Hz, **458 with signal**, roughly half beat-driven 4/4 (the Plan 0037
  Phase 4 trap/808 material) and half sparse.
- **Verified 2026-08-15** — the gate has not moved, and the accent is still bass-weighted, which
  is the cause Plan 0068 Phase 3 named: `present: CONFIDENCE_THRESHOLD: f32 = 0\.25 in: core/src/dsp/downbeat.rs`,
  `present: BASS_WEIGHT: f32 = 0\.7 in: core/src/dsp/downbeat.rs`. Both are written to go red when
  [Plan 0086](plans/done/0086-the-downbeat-finds-a-cue-that-is-not-the-kick.md) changes the cue, which
  is exactly when this entry's measured rates stop describing the engine. The authoring-doc
  qualification this entry called for is also still in place, checked rather than assumed:
  `present: 70 % bass band in: presets/README.md`.

`downbeat_locked` was true in **14 of 458 audible rows — 3.1 %**, which over the beat-driven half
is roughly **6 %**. `downbeat_confidence` sat at **mean 0.030, median 0.000** against
`CONFIDENCE_THRESHOLD = 0.25` (`core/src/dsp/downbeat.rs:55`), clearing the gate in **two of
eighteen** 30-second windows and peaking at **0.516** — twice the gate. So the estimator is
capable of locking and rarely does.

**This is a shortfall, not a defect, and the distinction is load-bearing.**
[ADR-0050](adrs/0050-downbeat-and-phrase-tracking-with-confidence-fallback.md) designed the gate so
that failing to lock degrades to the counters-only option the interview declined — the safe floor,
working exactly as specified. Nothing is broken. What is true is that `beat_in_bar`, `bar_index` and
`bar_phase` were counter-derived for essentially the whole session, so a preset binding them today
is binding the fallback.

**The stopping condition did not fire, and the record should not be read as it passing.** No
confidently-wrong bar line was observed — but with the gate shut 97 % of the time there was little
opportunity for one. The mis-accent question is *untested*, not *answered*.

**Do not read this as an argument for lowering `CONFIDENCE_THRESHOLD`.** That is the one change the
measurement must not be taken to recommend: ADR-0050 exists because a confidently wrong beat 1 is
the failure an author cannot work around, and buying lock rate with the gate inverts the trade the
ADR was written to make. If the gate moves at all it moves *after* the estimator improves, not
instead.

**What a design here would weigh** (ADR-0050 supplement territory, and it wants an interview):

- **Improve the accent model.** The estimator folds accents into a 4/4 hypothesis; whether the
  weakness is the accent feature, the fold, or the confidence measure itself is unknown and is the
  first thing to find out. Nothing here has been diagnosed — only the outcome measured.
- **Re-price the confidence measure without moving the gate.** If confidence is systematically
  under-reading a correct alignment, the fix is the measure, not the threshold, and the gate keeps
  its meaning.
- **Accept it and say so in the authoring docs.** Cheapest, and honest: layer 1 (`beat_index`,
  `time_since_beat`) is unconditional and reliable; layer 2 is decorative until further notice.
  `presets/README.md` currently offers both without distinguishing their availability.

**Blocks nothing, qualifies one thing.** Plan 0048 Phase 7's retune should lean on layer 1 and treat
layer 2 as decorative — recorded in that plan's Phase 6 results. **Do not re-measure by ear**: the
1 Hz `downbeat_locked` column is the instrument, and a targeted pass on known-4/4 material only
would sharpen the 6 % figure the half-and-half split leaves approximate.

**ANSWERED 2026-08-09 by Plan 0068 Phase 3 (the targeted pass this entry asked for).** 98 minutes of
unambiguous 4/4 through the live app on `v0.48.0`, 5900 audible rows: **352 locked — 6.0 %**. Split by
genre it is **6.79 %** on four-on-the-floor techno (5173 rows) and **0.14 %** on backbeat rock/pop
(727 rows, one single locked row, peak confidence 0.2664 against the 0.25 gate). Two things this
settles:

- **The ~6 % estimate was right, and it is a ceiling rather than a floor.** Restricting to clear 4/4
  does not rescue the rate — the material was never the problem.
- **Backbeat material is 48x *worse* than four-on-the-floor**, which is the opposite of the intuition
  and is what names the cause. The accent is 70 % bass band (`BASS_WEIGHT`); the kick marks every
  beat in four-on-the-floor and the half-bar in a backbeat, so it hardly ever marks the *bar*. The
  named cause is the accent feature, not the fold and not the confidence measure — full reasoning,
  the ladder placement and the limits are in
  [ADR-0082](adrs/0082-the-downbeat-gate-holds-and-the-estimator-is-diagnosed-first.md)'s `Outcome`.

The authoring-doc qualification this entry called for is done (`presets/README.md`, `docs/presets.md`,
both now stating the measured rate). **The repair — a downbeat cue that is not bass energy — is not
written and has no plan**; it is the open work this entry now points at. The mis-accent question
ADR-0050 guards is *still* untested, for the same reason as before: the gate was shut ~94 % of the
time here too.

- **Updated 2026-09-15** - the diagnosis and instrument halves are closed (Plan 0068, Plan 0095's
  bar grid, Plan 0117's `fold_beat` / `grid_bar_phase` log columns). The headline stays live: the lock
  rate and the mis-accent question are untested, `CONFIDENCE_THRESHOLD` and `BASS_WEIGHT` in
  `core/src/dsp/downbeat.rs` are unchanged, and no capture has used the new columns yet.

---

---

## Entry 0069 — from Plan 0070's close (2026-08-05)

Its sibling
**[0068](design-backlog-archive.md)
closed 2026-08-15** at [Plan 0090](plans/done/0090-the-emitters-source-moves.md)'s close — both
options delivered — and its body is in the archive.

---

---

## 0069 — there is no way to draw a two-tone object (a fill with a contrasting outline), because the composite is additive

> **CORRECTED 2026-08-13 — the title and the mechanism below are FALSE for field scenes, and have
> been since 2026-08-11. This entry stays live for its other half only; do not act on the paragraph
> that follows without reading this box first.**
>
> The claim "black adds zero, so a dark edge cannot exist inside the composite" was written
> **2026-08-05**. The layer system landed **2026-08-11** ([ADR-0090](adrs/0090-a-preset-composes-two-scene-layers.md)),
> six days later, and three closes ran in between without anyone revisiting this. `layer_blend.rs`
> gives `multiply`, which **strictly darkens**, and `fragment_field.rs:168` shows a fullscreen field
> emitting alpha = `occlude` — **1 by default on every pixel, including black ones** — which is the
> coverage a darkening blend needs.
>
> **Measured 2026-08-13**, one preset, `blend` the only variable, 640x360 on this box's hardware
> adapter: `multiply` reaches **min luma 18.5** with **61.9 %** of pixels below 64; the `add`
> control **cannot get below 181.6** anywhere, with **0.0 %** below 64. Three tones coexist in the
> multiply frame. Full derivation and costs in
> [ADR-0106](adrs/0106-two-tone-graphics-come-from-a-multiply-layer.md).
>
> **MEASURED AGAIN 2026-08-15**, at [Plan 0091](plans/done/0091-the-figure-fills-the-frame.md) Phase 1,
> which settled the path ADR-0106 left open and falsified one of its own consequences in the
> process. Both runs are `core/tests/layer.rs`, so the numbers are re-derivable rather than
> recalled:
>
> - **Multiply does NOT reach the backdrop, and the answer is negative in the clean way.** The
>   backdrop sits outside the chain's input (`post.rs:33`) and is composited *underneath* the
>   junction, so no blend mode can operate on it. At the default `occlude = 1` the frame is
>   **byte-identical** over a lit backdrop and over a black one — the backdrop is not darkened, it
>   is **absent**, held out by coverage. With it visible (`occlude = 0`) it is added after the
>   blend and floors the frame: the same multiply layer reaching **18.9** over black reaches only
>   **171.3** over a lit sky, where the sky alone reads 196.9. **Consequence for authoring: a light
>   ground must come from the CHAIN, not from `bg_*`.**
> - **A particle layer CAN darken, and ADR-0106's Negative consequence saying otherwise is
>   wrong.** Measured: a frozen swarm at `brightness = 0` in a `multiply` slot takes a light chain
>   from luma **174.1** to **0.9** — *darker* than the field route's 18.9. The mechanism the ADR
>   states ("a particle's alpha *is* its brightness") is not what the code does: `swarm.rs` emits
>   `vec4(color * g, g)` where `g` is the mark's **geometric** falloff, independent of its colour,
>   and `layer_blend.rs:138` un-premultiplies (`straight = b.rgb / max(b.a, 1e-4)`) before the mode
>   runs. So a black particle has full coverage and a zero operand — the darkening condition met,
>   not failed. **The real difference between the two routes is footprint, not capability**: a field
>   darkens every pixel, a particle darkens only inside its marks.
>
> **What survives, and it is why this entry is corrected rather than archived:** multiply darkens in
> proportion to *coverage*, and **nothing in this engine still decides what is in front of what**. A
> shaped object that occludes another figure is unbuilt.

- **Raised:** 2026-08-05, at [Plan 0070](plans/done/0070-shaped-marks.md)'s close. **Re-filed from
  [0033](design-backlog-archive.md), at that entry's own instruction** — 0033 carried two asks, Plan
  0070 answered one of them, and leaving the other inside a closed entry is how the two get confused
  again.
- **Verified by measurement:** yes, and the measurement is the point. The cardioid
  `r = 1 - sin(theta)` drawn through `parametric_curve` at `ink_amount = 1` on white paper renders
  its outline **grey**, not black: a thin anti-aliased stroke averages to mid luminance and lands
  halfway down the ink ramp.
- **Verified 2026-08-15** — the correction box above, not the title: the darkening blend and the
  fullscreen coverage it needs both exist:
  `present: multiply in: core/src/render/layer_blend.rs`,
  `present: occlude in: core/src/render/scenes/fragment_field.rs`. The half this entry stays live
  for does not reduce — `unprobeable: nothing in this engine decides what is in front of what is
  the absence of a whole mechanism rather than of a symbol, and every narrow spelling of it (depth,
  sort, order) is a common word in this tree, so any probe on it could never fail and would read as
  verification while checking nothing`

The original ask was a Solitaire-style cascade of **hearts — red fill, black outline**. Plan 0070
delivered the silhouette: `shape = heart` on `swarm`/`emitter` draws a heart-shaped *glow*,
brightest in its middle, fading to nothing at its boundary. That is as far as an additive pipeline
reaches. **Black adds zero**, so a dark edge cannot exist inside the composite, and the only
dark-on-light route in the engine is the ink stage, which is structurally two-poled
(`mix(paper, ink, luminance)`) and therefore cannot hold three tones either.

### Why this is its own question and not a follow-on

It reopens [ADR-0018](adrs/0018-engine-wide-scene-compositing.md)'s composite and
[ADR-0056](adrs/0056-additive-scenes-emit-premultiplied-alpha.md)'s alpha model, and it needs an
ordering or sorting story the additive pipeline has never required — a filled object *occludes*, and
nothing in this engine has ever had to decide what is in front. That is why
[ADR-0084](adrs/0084-a-particle-marks-silhouette-is-a-signed-distance-function.md) rejected it as a
bundled decision (Alternative B) rather than on its merits. It also sits adjacent to
[0040](design-backlog-archive.md)
and its plan, which is the *other* place the additive model's occlusion behaviour is being
questioned — anyone taking this should read that first.

### Updated 2026-08-26 at [Plan 0113](plans/done/0113-the-engine-paints-a-canvas.md)'s close — half the ask now exists, and this entry stays live for the other half

**Occlusion exists inside one scene.** `shape_collage` (twelfth system,
[ADR-0123](adrs/0123-a-flat-graphic-scene-paints-its-own-paper-and-composites-opaque-elements-in-one-pass.md))
paints flat opaque elements on its own paper in painter order, so a black bar genuinely sits in
front of a red one and a fill with a contrasting outline is drawable today — two elements, the
smaller later in the array. `the_later_element_wins_the_overlap` renders the pair in **both** array
orders and asserts the overlap takes the later element's colour each time, which is the mechanism
rather than an example of it: `present: SystemKind::ShapeCollage in: core/src/preset/schema/system.rs`

**And it cost no composite change**, which is the half of this entry's own pricing that was wrong.
This entry has sat at **Low** since 2026-08-05 because it was priced as a composite redesign.
ADR-0123 found that price mistaken for the in-scene case: a fullscreen scene emitting `alpha = 1`
already holds the backdrop out (measured, Plan 0091 Phase 1), and the tonemap is the identity below
`KNEE`, so the capability landed as a **scene** with the composite untouched.

**What is still missing is the cross-scene half, and it is the harder one.** A collage element and a
`swarm` particle still have no ordering relationship, because nothing in the engine decides what is
in front of what across scenes — which is this entry's title claim and remains true. ADR-0090's two
layers compose by blend, not by depth, and ADR-0018 / ADR-0031 rejected a render graph twice. So the
entry stays **live**: the ask survives, the in-scene route is now a shipped answer for anyone who
only needs one world at a time, and the remaining work is engine-wide depth rather than a fill model.

### Priority

**Low, and deliberately so.** The user has asked for it once, in a form Plan 0070 partially answered
and Plan 0113 half-answered, and the cost of the remaining half is still a composite redesign. It is
here so the ask survives, not because it is next.

- **Updated 2026-09-15** - still half open: fields multiply (ADR-0106) and one scene occludes within
  itself (`shape_collage`, ADR-0123), but nothing orders *between* scenes; the two layers of ADR-0090
  still combine by blend alone, and no ADR or plan takes depth across layers.

---

---

## Entries 0075-0076 — from the Plan 0074 Phase 6 content pass (2026-08-08), binding the root channel

---

## 0075 — `root_tint` earned no binding on either shipped IFS preset, and `root_hue` earned both

- **ITEM 2 DECIDED 2026-08-13 → [ADR-0102](adrs/0102-a-palette-coordinates-edge-is-a-per-preset-choice.md),
  proposed, and deliberately with no plan.** The entry asked whether to clamp the palette coordinate
  rather than repeat it. The ADR's finding is that **no per-param answer exists**: there is exactly one
  coordinate, it is a *sum* of contributors of two kinds — angles, where wrapping is correct, and
  distances, where it is a discontinuity at the quantity's own floor — and a sum cannot carry two
  addressing behaviours. So the edge becomes a **per-preset `[palette]` choice**, wrap by default
  (zero pixels move), implemented as a shader `select` rather than a second sampler, clamped to the
  **texel-centre range** because linear filtering blends texel 255 into texel 0 at exactly 0 and 1.
  **Verified against code 2026-08-13:** one sampler, `AddressMode::Repeat` on `u`
  (`core/src/render/palette.rs:332`), and its docstring gives the cyclic justification.
- **No plan, by the user's call** — the want is real, no shipped content is wrong, and both presets
  bind the route that works. This entry stays live because the design has not landed, which is the
  lifecycle working rather than a miss. **Item 1 remains closed** (the authoring half landed at Plan
  0074's close) and **item 3 is not a defect**.

- **Raised:** 2026-08-08, at [Plan 0074](plans/done/0074-the-figure-colours-by-how-far-it-has-come.md)
  Phase 6 — the `preset-author` pass, filed under that phase's own "any route that could not be made
  to read is written up here rather than quietly left bound to nothing".
- **Verified by measurement:** yes — both routes rendered against each other on both presets, at a
  quiet frame and a typical one, and on `attractor_dissolve` at three points across its morph.
- **Verified 2026-08-15** — item 2's engine fact is unchanged, and it is the only half of this
  entry still open: one sampler, repeating on the palette coordinate:
  `present: address_mode_u: wgpu::AddressMode::Repeat in: core/src/render/palette.rs`.
  [ADR-0102](adrs/0102-a-palette-coordinates-edge-is-a-per-preset-choice.md) is proposed and has no
  plan, so this is expected to hold until a look asks for the clamp.
- **Nothing here argues the channel was a mistake.** `root` reads, exactly as the Phase 2 gate
  found. This entry is about *which of its two routes* a real preset can afford, and the answer was
  the same on both looks for two **different** reasons — which is what makes it a property of the
  tint route rather than a fact about one palette.

**What shipped.** `attractor_fern` binds `root_hue = 0.21 + sin(time * 0.047) * 0.05` with
`map_tint` **left at its full `0.46`**; `attractor_dissolve` binds
`root_hue = 0.17 + sin(time * 0.1200 + 0.35) * 0.05`, in phase with its `palette_mix` so the crystal
end is nearly pure and the living end carries the depth. Neither binds `root_tint`.

**The fern: the coordinate was already spent, and the hue route made the budget question moot.**
Phase 2's gate had found a tuning that beats stock — `map_tint` `0.46 -> 0.22` with
`root_tint = 0.85`, the budget *split* rather than stacked — and it recorded that it had judged that
split before `root_hue` existed. Rendered against each other now: the split reads *flatter* than
stock at rest, because cutting `map_tint` in half is exactly the part-separation the fern's Plan
0073 pass paid `hue_spread` for, and the anchored `root_tint` returns a wash rather than a
separation. `root_hue` at full `map_tint` keeps both — the body cools to jade while the frond
origins stay warm, and nothing was given up. **The hue route is not the fallback the gate assumed;
on this preset it is the answer.**

**The dissolve: a different reason, and the general one.** Its palette coordinate is not
contested — the problem is that its mineral end already runs its densest crossings into near-white.
`root_tint` is **anchored**, so it only ever pushes *up* the ramp: at `0.75` it whitens exactly the
regions that were already brightest and the frost structure flattens. The hue route does not touch
the coordinate, so it buys the same depth with none of the headroom.

Stated generally, and this is the part worth keeping: **an anchored coordinate term spends the
ramp's bright end by construction.** So `root_tint` is structurally disadvantaged on any preset
whose palette *ends* bright — which, under the additive composite this library authors for, is most
of them. `map_tint` does not have this problem because it is centred and spends both directions.

**Negative `root_tint` is expressible, is the obvious escape, and has an unflagged edge.** Nothing
stops a preset writing `root_tint = -0.30`, which ramps the figure *down* the palette — spending the
dark end, which is empty. It reads (subtly) on the fern and costs no headroom. But at `-0.55` a
**bright cream speckle appears mid-figure**, in the region that should be darkest: the coordinate
crosses zero and the LUT sampler *repeats*, wrapping the darkest points to the ramp's brightest
stop. Arithmetic confirms it — the fern's coordinate floor is `hue_center` at its sine trough
(`0.20`) minus `hue_spread/2`, so a `root01` of `0.46` goes negative at about `root_tint = -0.38`.
The centred params can find the same edge, but they are far less likely to: they push half as far in
either direction, where an anchored term walks monotonically toward one edge and will find it if
driven. **`presets/README.md` documents neither the negative direction nor the wrap.**

### What a fix would be

Nothing engine-side is *required* — the two-route design already provides the escape, and it worked.
Three things are cheap and would save the next author the same session:

1. ~~**Say in `presets/README.md` that `root_tint` may be negative, and what happens at the edge.**~~
   **Done 2026-08-08 at Plan 0074's close**, with the anchored-term-spends-the-bright-end property, the
   negative escape, the wrap, and the fern's `-0.38` arithmetic. **That is the authoring half of this
   entry closed; what remains open is item 2, which is the engine question.**
2. **Consider clamping the palette coordinate rather than repeating it** — or documenting the repeat
   as deliberate. A wrap that turns the darkest region of a figure into its brightest speckle is a
   surprising default for a *coordinate*, whatever it is for a texture sampler. This is an engine
   question and an ADR-sized one; it touches every scene that samples the LUT, not just the IFS.
3. **Nothing about the anchoring.** It is correct — [ADR-0088](adrs/0088-the-ifs-colours-by-distance-from-its-own-skeleton.md)'s
   *Anchoring* section reasoned it from the measured distribution and this pass agrees with it. The
   consequence above is a cost of a right decision, not evidence against it.

### Priority

**Low.** No shipped content is wrong and no author is blocked — the route that works is bound in
both presets and documented. It is a documentation gap with one genuine engine question behind it.

---

## Entries 0078-0079 — from Plan 0064's Phase 4 and Phase 6 (2026-08-09), the symmetry stage

---

## 0079 — an accumulating figure rendered with `trails = 0` is not a sparse source, it is a blank one, and a whole third of a decision grid was unreadable because of it

**Raised by:** `architect`, reading [Plan 0064](plans/done/0064-the-symmetry-stage-and-the-banded-palette.md)
Phase 3's sample set at Phase 4. **Owner if taken:** whoever next builds a capture grid — this is a
methodology note, not a code change.

- **Verified 2026-08-15** — `unprobeable: this is a capture-hygiene rule for whoever next builds a
  sample grid, and its own What a fix would be section is nothing in code, so it makes no claim
  about this repository's contents at all`

### The finding

Phase 3 rendered its grid with `trails = 0` in every cell, on sound reasoning that is written into
the set's own index: a trail averages frames, and each cell is a judgement about **one frame's**
coordinate map. That is correct for `fragment_field` and for `star_pattern`.

It is wrong for `attractor_lorenz`, and the six attractor sheets came out **near-black**. An
attractor *is* its accumulation — the figure exists only as the deposit of many frames — so removing
the trail does not make it sparse, it removes the picture. Four of the twelve cells in each attractor
sheet have no legible content at all.

### Why it matters beyond one grid

The attractor was in the set **for a specific reason** the plan states: a coordinate map behaves
completely differently on a texture that fills the frame and on one that is mostly empty. So the
sparse-source question is exactly the one those sheets were rendered to answer, and they are the only
cells that could have answered it. Phase 6 then asks the same question again in its own words — does
the mandala hold up on a sparse source, or only on a full-frame field — and had to be answered live
because the grid could not.

The general form: **a capture-hygiene rule that is right for most scenes can be wrong for one
family**, and the failure is silent, because a near-black cell looks like a preset that renders dark
rather than like a broken measurement.

### What a fix would be

Nothing in code. When a grid spans scene families, set the accumulation **per source** rather than
globally — trails off for scenes whose figure is present in one frame, trails at the preset's own
value for scenes whose figure *is* the accumulation — and state the difference in the index, so a
reader knows the cells are not directly comparable and why.

### Priority

**Low**, and it is a note for the next person building a grid rather than work to schedule. It cost
this plan one unanswered question, which Phase 6 absorbed.

[0048]: plans/done/0048-analysis-v2-and-the-retune.md

---

---

## Entries 0084-0089 — from the Plan 0075 cohorts 1-5 handoff (2026-08-11)

The renaissance's first five cohorts (28 worlds, cohort 5 judged live 2026-08-11) handed back
one assembled feedback note. Three of its items are **re-raises** and are recorded as dated
updates inside 0009, 0055 and 0068, all three now in
[`design-backlog-archive.md`](design-backlog-archive.md),
rather than as new entries; the two doc drifts it carried went to
[Plan 0075](plans/done/0075-the-content-renaissance.md) Phase 6's sweep list, not here. Each entry
below carries a **handoff verdict** — promote or park — per that plan's Decision (promotion on
demonstrated want, each through its own ADR/plan, never absorbed into 0075). Promoted items
queue **behind Plan 0076 and cohort 6**; none of them gates the collage. All measurements below
are the lane's renders, reported at the handoff — not independently re-verified here; the
file's standing verify-before-acting rule applies.

---

## 0087 — `reaction_diffusion` has no glow of its own, and the engine bloom's threshold sits above where its output lives

**Raised by:** `preset-author`, Plan 0075 cohort 3 (the Verdigris/Mitosis register).
**Owner if taken:** `architect` then `dev`, if a second want arrives.

- **Verified 2026-08-15** — the absence is still an absence: the RD scene has no glow or threshold
  of its own, so the engine-wide `bloom_*` is still the only instrument pointed at it:
  `absent: bloom in: core/src/render/scenes/reaction_diffusion.rs`. That is the whole of this
  entry's repo claim; whether a second want has arrived is not a fact about the tree.

### The finding

The want: a glow accent on the RD field. Engine `bloom_*` acts on the composited frame, and its
threshold sits where the RD field's mapped output rarely reaches — driving the field bright
enough to cross it blows out the pattern first. So one cohort's RD looks were tuned around the
absence.

The route already exists as precedent:
[ADR-0080](adrs/0080-the-attractor-owns-its-level-and-bloom-thresholds-exposed-light.md) gave
the attractor **its own** level and bloom thresholds when the engine-wide ones proved the wrong
instrument for one scene's dynamic range. RD asking for the same shape is not a new argument —
it is the same argument on a second scene.

### Handoff verdict (2026-08-11): park

One cohort's demonstrated want. The ADR-0080 shape is the named route when the second arrives.

---

## 0092 — every figure this engine draws is unlit, and two reference images ask for a shaded one

> **ITS TRIGGER FIRED AND RESOLVED NEGATIVELY — 2026-08-16. Do not take this entry off it.**
> The entry says to take the lighting work *"if the Phase 6 look gate says the flat sparkle is the
> disappointing one in the set"*. That gate ran at
> [Plan 0091](plans/done/0091-the-figure-fills-the-frame.md)'s close and the user rejected the star
> silhouettes — but the stated reason was that they looked **"dirty and upscaled"**, which is
> neither silhouette nor shading. It was [0099](design-backlog-archive.md): the probes drew their figures
> through 8 to 32 of the palette's 256 LUT texels, with edge transitions of 1.3 texels, because a
> sharp star's tiny inradius had forced `color_span` down to `0.037`. Re-rendered with
> `palette_steps` bound, the same five silhouettes come back **crisp**.
>
> **So the trigger is answered and the answer is no.** The re-judge ran on a fair probe the same
> day and the verdict was *"objectively good, yes to all"* on all five silhouettes
> ([backlog 0100](design-backlog.md) carries the one soft edge, and it is about edge wobble rather
> than shading). Nothing in that gate says the flat sparkle was the disappointing one. **This entry
> stays filed on its original evidence — two reference images — and the Phase 6 trigger is spent.**
> A future lighting plan needs a fresh want, not this one.

- **Raised:** 2026-08-13, from the second of two user reference batches, alongside
  [Plan 0091](plans/done/0091-the-figure-fills-the-frame.md). Filed separately **at the point of raising**
  rather than absorbed into that plan, because it is a lighting decision and the plan is a silhouette
  one, and bundling them would have made a shading register arrive as a side effect of a shape phase.
- **Verified by measurement:** no, and it does not need one — the claim is an absence. Nothing in
  `core/src/render/` computes a surface normal or evaluates a light. Every family is emissive: the
  particle scenes add glow, the field scenes map a scalar through a LUT, the line renderer strokes,
  and the terminal stages remap what those produced. Brightness in this engine is *authored colour*,
  never *illumination*.
- **Verified 2026-08-15** — nothing shades: `absent: matcap in: core/src`. Deliberately narrow
  rather than probing for a normal or a light — `normalize` is everywhere in this tree, so a probe
  spelled that way could never fail, and a probe that cannot fail reads as verification while
  checking nothing (ADR-0108, Negative). `matcap` is the name this entry's own proposed fix
  carries, so the probe goes red exactly when the entry is delivered.

Two of the six star references are chrome — a four-pointed sparkle with concave edges, rendered as
polished metal with specular highlights, a horizon reflection and self-shadowing. They read as
**objects with a surface**, and the engine has no vocabulary for that at all. The rest of the batch
is flat graphic work that [Plan 0091](plans/done/0091-the-figure-fills-the-frame.md) Phase 5 reaches with
three parameters; these two are a different question wearing the same silhouette.

### Why it is worth an entry rather than a comment

**The prerequisite is about to exist, and that is the whole reason to file this now.** ADR-0105 puts
a signed distance field on screen, and the gradient of a distance field *is* a surface normal —
`normalize(vec3(dFdx(d), dFdy(d), k))` for a 2.5D bevel, or the analytic gradient where an arm has
one. So the expensive precondition for shading (knowing which way a surface faces) arrives as a free
by-product of a plan already written for other reasons. A matcap or a small analytic environment then
turns that normal into the chrome look, and neither needs a light rig, a depth buffer, or a second
pass.

**It would also be the engine's first non-emissive register**, which is why it is an ADR-worthy
decision and not a parameter. Every consequence downstream assumes emission: ADR-0056's alpha *is*
the falloff, bloom's bright-pass reads emitted light, ADR-0046's linear-light ordering is built for
additive accumulation, and [ADR-0106](adrs/0106-two-tone-graphics-come-from-a-multiply-layer.md) has
just established that darkening requires a `multiply` layer. A shaded object has dark regions that
are *shape information* rather than absent light, and nothing in that chain currently distinguishes
the two.

### What a fix would be

A `shade` amount on the shape field, off by default, deriving a normal from the distance gradient
and reading a small built-in matcap. Off is an exact identity, so nothing that ships moves. The open
questions are which matcaps ship (a closed roster, on ADR-0084's precedent), whether the bevel
profile is authorable or fixed, and — the load-bearing one — whether a shaded figure should bloom,
since its highlight is the brightest thing on screen and is *not* an emitter.

### Priority

**Low, and gated on Plan 0091.** There is no route to this before the distance field exists, and one
user batch that mixed it with five flat references is a want expressed once rather than a demonstrated
gap. Take it if the Phase 6 look gate says the flat sparkle is the disappointing one in the set — that
verdict is the trigger, and it is scheduled.

---

## 0094 — the `frame_ms_p99` tail is not switch-correlated, so the steady-state column does not remove it

**Raised by:** `architect`, at [Plan 0085](plans/done/0085-the-show-length-horizon-gets-an-instrument.md)
Phase 5, from the three paired runs that closed
[0083](design-backlog-archive.md).
**Owner if taken:** `architect`, and only if the governor is revisited.

- **Verified 2026-08-15** — the governor reads the raw series at a miss fraction, not `p99`:
  `present: MISS_FRACTION in: core/src/render/tier.rs`. Written in
  [ADR-0108](adrs/0108-a-backlog-claim-about-the-repo-carries-an-executable-probe.md)'s grammar
  while [Plan 0093](plans/done/0093-the-backlog-stops-asserting-things-about-a-repo-it-has-not-read.md)
  is still in flight, because this entry's whole reason to exist is that its *predecessor's*
  repo-claim rotted. The probe covers the load-bearing half: if `sustained_miss` is ever rewritten
  to read `p99`, that constant is what goes with it, and this entry should go red rather than keep
  telling a reader the tail is unremovable by an exclusion nobody is applying.

**This is a new entry rather than an edit to
[0082](design-backlog-archive.md)**,
which is closed and archived. It cites it; it does not amend it. 0082 already carries two
corrections — the governor was built, and it never reads p99. This is a **third**, and it is about
the half of 0082 that survived both.

### The finding

0082 states, as the observation the whole entry rests on: *"The spikes coincide with preset switches
and the fullscreen toggle — GPU resource rebuilds, not steady-state cost."* Plan 0085 Phase 5's
**run 3** was 1,797 s of feedback presets with auto-rotate **off** — three surface reconfigures at
startup and nothing after. It reached:

| | value |
|---|---|
| `frame_ms_p99` max | **23.960 ms** at t = 355 s |
| second peak | 18.681 ms at t = 1761 s |
| switches after startup | **0** |
| rows where `frame_ms_p99_steady` diverged from raw | **0 of 359** |
| frames dropped (`diagnostics.log`, whole window) | **0 of 200,667** |

The steady column never diverged because there was no switch anywhere near those spikes — the
exclusion window had nothing to exclude. **A ~24 ms p99 excursion happened in a session with no
preset switching at all.**

Both mechanisms are real and they are different sizes. In **run 1** (62 switches) the exclusion
fired on **58 of 239** rows and separated them cleanly — ~11.9 ms raw against ~6.5-7.0 ms steady —
so switches *do* elevate p99 and the new column *does* remove that. But the largest excursions in
both runs (17.7 ms in run 1, 24.0 ms in run 3) landed on rows the exclusion did not touch.

### Why it is worth an entry

**0082's first candidate response — "exclude the frames following a preset switch or a surface
reconfigure from the governor's window" — would not have solved the problem 0082 raised.** The
spikes it was written about survive the exclusion. Anyone revisiting the governor from that entry
would implement the exclusion, watch the tail persist, and have to rediscover this.

It is **not** currently a defect: nothing demotes on p99 (the shipped `sustained_miss` reads the raw
series at a 75 %-of-180 miss fraction, which no excursion of this size approaches), zero frames
dropped across 200,667, and fps never left the 146-165 band against a 60 fps floor. The value is
entirely in *not* designing against a false model later.

### What a fix would be

Nothing, until the governor is revisited. What is missing is a **cause** — the tail is unexplained,
and three candidates are untested: OS/driver scheduling on a 165 Hz vsync, a genuinely expensive
frame in one of the four feedback presets, or another process on a developer box. The cheap
discriminator is a run with the per-frame series retained rather than a p99 summary, which
`--soak`'s coarse tick cannot give — `diagnostics.log`'s 1 Hz rows are the nearest existing
instrument and were not read for this.

**Whoever takes this should re-measure rather than trust the table above.** These runs were
windowed, with no audio, on one box — and **the fullscreen toggle, which is the one event 0082
named that this run could not reproduce, remains the strongest untested candidate for the original
25.037 ms.**

### Priority

**Low.** It blocks nothing, nothing ships broken, and it becomes load-bearing only when someone
opens the governor. It is filed because that is exactly the moment the correction would otherwise
be missing.

### Update 2026-08-30 — a show-length run exists now, and it neither discharges nor contradicts this

The 2026-08-29 live set ran **8h08m / 3,505,083 frames with zero dropped**, at 120.0 fps flat, on
the show notebook with real audio and a real rotation. That is **17x** run 3's 200,667 frames and
the first data this project has at show length rather than in minutes, so it is worth naming here:
**the tail never became a drop.** The entry's "not currently a defect" reading survives a horizon
two orders of magnitude past the one it was written on.

**It does not close the entry, because the one column that would is missing.** The summary that
survives the night records fps, frames, dropped, `rss_bytes`, `gpu_bytes`, handles and threads —
and **not** `frame_ms_p99`. No soak or `diagnostics.log` from the set is findable on this machine
as of 2026-08-30. So the cheap discriminator "What a fix would be" asks for — a run with the
per-frame series retained — was within reach on the night and was not captured. The practical
lesson is for the *next* show rather than for the governor: a set that runs this long is the
cheapest instrument this project will ever get for the p99 question, and it costs one `--soak`
path on the command line.

---

## 0095 — the backdrop ramp makes parallel stripes only, and a converging fan cannot be lit *and* darkened

**Raised by:** `architect`, at [Plan 0091](plans/done/0091-the-figure-fills-the-frame.md)'s close, from
that plan's **Phase 7, which was designed as a cut point and was cut**. **Owner if taken:**
`architect` (it owes its own ADR) then `dev`.

- **Verified 2026-08-16** — the ramp's coordinate is linear along `bg_angle` and the swept
  coordinate is repeat-addressed, so `bg_hue_span > 1` gives repeating **parallel** stripes and
  nothing gives an angular one: `present: bg_angle in: core/src/render/background.rs`
- **Verified 2026-08-16** — the backdrop is outside the chain's input, which is why no blend mode
  and no fold can reach it: `present: backdrop is not in the chain's input in: core/src/render/post.rs`

### The want

The third of the user's Plan 0091 reference images is a collage whose floor is **white with black
stripes converging to a vanishing point**. Everything else in that collage is now reachable: the
flat red figure is `shape_field` (ADR-0105), and the dark-on-light treatment is a `multiply` layer
(ADR-0106). The floor is not.

### What is actually missing, and it is small

**Only the coordinate.** The backdrop's ramp is already swept and already repeat-addressed, so
repeating stripes ship today at no cost. An **angular** coordinate about a movable point turns the
same stripes into a fan. Folding the existing ramp is not available — `post.rs` puts the backdrop
outside the chain the kaleidoscope reads — so this is a backdrop *mode*, not a reuse.

### The constraint Plan 0091 Phase 1 added, and the reason this entry exists rather than a phase

Phase 1 measured what a `multiply` layer does to a **lit backdrop**, and the answer changes what a
backdrop fan can be *for*:

- at the default `occlude = 1` the backdrop is **absent** — held out by coverage, not darkened, and
  the frame is byte-identical to the same preset over a black backdrop;
- at `occlude = 0` the backdrop is added *after* the junction and becomes a **floor**: a multiply
  layer reaching display luma 18.9 over black reaches only **171.3** over a lit sky.

So **a fan drawn on the backdrop and a dark figure drawn over it are mutually exclusive at the
tones the reference has.** The collage's black stripes would have to come from the backdrop's own
palette stops — which the ramp can express — and the figure over them cannot then be darkened into
that floor.

### What a fix would be, and the alternative that Phase 1 promoted

Two routes, and the second is no longer the weaker one:

1. **A backdrop coordinate mode** (`bg_coord = "linear" | "radial"` plus a centre). Cheapest, and it
   is the third decision on that pass after ADR-0094 and ADR-0095 — so it owes an ADR, with every
   default an arithmetic identity and the existing baselines moving zero pixels.
2. **Draw the floor as a scene in the chain.** Plan 0091 listed this as the rejected alternative
   because it costs the preset's one `[layer]` slot (ADR-0090). Phase 1's measurement argues *for*
   it: a floor inside the chain is reachable by every blend mode, and a floor on the backdrop is
   not. The slot cost is real and it is the honest trade to weigh, not a reason to dismiss it.

The collage's floor is also **bounded by a horizon**, which the ramp expresses only through stop
placement — so "a fan" may not be the whole of what the image is doing.

### Priority

**Low.** Nothing is blocked and nothing ships broken. The heart is the part of those references the
user asked for twice and it shipped; the floor was asked for once, as one element of a collage, and
this engine is not a collage tool. It becomes worth taking when someone authors a world that wants
a vanishing point — at which point the two routes above should be judged by rendering, not by
argument, which is what Phase 7's own first done-when said.

---

## 0100 — "hand-drawn" is edge wobble, not spike-length variation, and the star arm has no lever for it

**Raised by:** `architect`, from Plan 0091 Phase 6's star look gate (2026-08-16).
**Owner if taken:** `architect` then `dev`. **Low priority and the user said so in the same breath
as passing it** — this entry exists because the *reason* it fell slightly short is specific and
worth not rediscovering.

- **Verified 2026-08-16** — the only per-spike variation the arm has is a tip-radius scale, drawn
  from an index hash: `present: rt = 1\.0 \+ jitter in: core/src/render/scenes/marks.rs`
- **Verified 2026-08-16** — and there is no seed or phase input to re-scatter it:
  `absent: star_seed in: core/src/render/scenes/marks.rs`

### The finding

Plan 0091 Phase 5 shipped three `star` shape params against a batch of six reference images. Phase 6
judged them, and the verdict was **yes on all five silhouettes**, with one soft exception: the
hand-drawn six-pointer *"maybe"* reads as hand-drawn, *"but it's fine also"*.

That soft edge has a specific cause. `star_jitter` varies each spike's **tip radius** — one scalar
per spike, drawn from `mark_spike_hash01(index)`. A hand-drawn figure does not vary that way: its
spikes are roughly the right length and its **edges wander** — the line between tip and valley is
not straight or cleanly bowed, it wobbles along its length. That is **displacement along the edge**,
a different quantity from the one the arm exposes, and `star_curve` cannot supply it either because
it bows the whole edge as one smooth quadratic.

Two smaller consequences of the same shape, both real and neither urgent:

- **There is no lever to re-scatter the jitter while keeping its amount.** The pattern is a pure
  function of the spike index, so a preset that wants *this much* irregularity in a *different*
  arrangement cannot ask for it. That is the price of the determinism rule the arm correctly follows
  (a `sin`-based hash would differ between GPUs), but a `star_seed` input would keep determinism and
  restore the choice.
- **A jittered star's exterior is the roster's least accurate field** — up to 0.54 out, because the
  angular fold measures against a point's own spike when a longer neighbour may be nearer
  (Plan 0091 Phase 5 measured it). Edge displacement would make that worse, so anything built here
  should be judged against that number rather than assumed free.

### A record defect, worth one line

[Plan 0091](plans/done/0091-the-figure-fills-the-frame.md) line 124 says this item was **"separately
filed rather than absorbed"**. It was not — no entry existed until this one, five days later, and
the claim was never checked. It is the same class
[ADR-0108](adrs/0108-a-backlog-claim-about-the-repo-carries-an-executable-probe.md) exists for, one
level up: a plan asserting something about the backlog rather than about the code.

### What a fix would be

Not obvious, and that is why this is filed rather than planned. A displacement term needs a
coordinate along the edge and a noise source that is cheap in a fragment shader and stable across
GPUs — the same constraints that made the tip jitter an integer hash. It is plausibly a
`star_wobble` amplitude plus a frequency, evaluated on the folded edge parameter the arm already
computes.

### Priority

**Low.** The look gate passed the silhouette. Take it when someone wants a *deliberately* rough
figure rather than a slightly irregular one, and read the exterior-accuracy number first.

---

## 0101 — the mark roster cannot morph between silhouettes, and two other rosters in this engine already do

**Raised by:** `preset-author`, from the user's "can we morph the shape with music" at
`presets/shape_facet.toml`'s authoring (2026-08-16). **Owner if taken:** `architect` — it owes an
ADR, and it is a real design question rather than a missing knob.

- **Verified 2026-08-16** — the selector is rounded, so a fractional index selects no arm:
  `present: clamp\(MIN_SHAPE, MAX_SHAPE\)\.round\(\) in: core/src/render/scenes/marks.rs`
- **Verified 2026-08-16** — the same file states the roster is a list of identities rather than a
  quantity: `present: values are \*identities\* rather than a quantity in: core/src/render/scenes/marks.rs`

### The finding

**Within an arm, morphing already works and needs nothing** — `star_valley`, `star_curve` and
`star_jitter` are clamp-only, so a binding drives them continuously and the silhouette genuinely
deforms with the music. That was demonstrated and it is the answer to the question as asked.

**Between arms it is a cut.** `mark_shape` rounds, deliberately: ADR-0084's roster is five
identities and a fractional index selects no arm at all. So a preset can switch `star` to `heart`
on a beat but cannot travel between them, and an eased binding steps rather than morphs.

**What makes this worth an entry rather than a shrug is that this engine has twice decided the
opposite for other rosters.** [ADR-0060](adrs/0060-star-pattern-variants-interpolate.md) makes the
star-pattern variants **interpolate**; [ADR-0075](adrs/0075-ifs-family-morphs-in-singular-value-space.md)
morphs the IFS family in **singular-value space** — a considered answer to exactly the question
"these are discrete entries, so how do you travel between them". So "roster entries interpolate" is
an established idea here, and the newest roster is the one without it.

### Why it is a design question and not a knob

A naive lerp between two signed-distance functions is **not** a shape in any principled sense — the
result is a field whose zero set is some blend that neither arm describes, and whose interior may
not even be connected. It is nonetheless a well-known technique that often looks good, which is
precisely the kind of trade an ADR exists to record rather than to have someone discover in a
preset.

Two constraints any answer has to meet:

- **The particle path shares this chunk.** `swarm` and `emitter` read the same `mark_distance`, so a
  morph capability arrives there too, and its default must be an exact identity or every shipped
  `shape`-bearing preset moves (ADR-0105's shared-chunk consequence).
- **It collides with [ADR-0111](adrs/0111-the-shape-field-gains-a-scaled-copy-coordinate.md).** That
  decision adds a second per-arm scalar (`r_boundary`), so a morph would have to blend *both* the
  distance and the boundary radius, or be defined only for the distance coordinate. Whoever takes
  this should read 0111 first — and if both are wanted, they are probably one plan rather than two.

### Priority

**Low, and it is genuinely optional.** The question that prompted it is already answered by the
three continuous star params. This is filed so that if someone later wants a figure that travels
from a heart to a star across a phrase, the precedents and the constraints are in one place instead
of being re-derived.

## 0108 — the conversion tail: HLSL arrays (~71 files) and 218 MD2 presets that convert but render blank

**Raised by:** `dev` (Plan 0100 Phase 6 log, "followup noticed, not acted on"), filed by
`architect` at the close (2026-08-16). **Owner if taken:** `dev`.

- **Verified 2026-08-16** — arrays are a named rejection class, not a silent drop:
  `present: array declaration in: milkconv/src/shader/parse.rs`
- `unprobeable:` the counts (71, 218, 80.1 %) are a measurement of one corpus run (2026-08-16,
  dev box), reproducible with `milkconv --report`/`--render` over `WORK/milkdrop-corpus`, not a
  property of this tree; both eras' tables are in `docs/milkdrop-conversion.md`.

### The finding

After Phase 6, the corpus converts at 80.1 % (8 289 of 10 347) and renders non-blank at 77.9 %.
The residual worth work, in order: **218 MD2 presets convert but render blank** — with their
shaders now running, these are fidelity findings (most plausibly warp shaders that supply no light
of their own whose source was a refused disk texture), and backlog 0106/0107's fixes should be
re-measured against them before any new mechanism is hunted. **~71 files use HLSL arrays**, today
a named `unsupported` rejection; a bounded-size array lowering in the frontend would recover them.
The `emitter-invalid` class (naga refusing our own emission) ended Phase 6 at zero and should stay
there.

### Priority

*(Updated 2026-09-15: 0106/0107 landed with Plan 0108, so the gate below has expired; the re-rank
now waits on [Plan 0142](plans/done/0142-the-milkdrop-import-earns-its-verdict.md) Phase 6, which names
this entry and does not take it.)*

**Low until 0106/0107 land** — the blank list is contaminated by both, so counting it again first
is wasted; re-run `--render` after they land and re-rank.

**Re-ranked 2026-09-18**, by [Plan 0142](plans/done/0142-the-milkdrop-import-earns-its-verdict.md) Phase 6,
which names this entry and does not take it. The line above pointed at a gate that had already
expired, so this replaces it as the live priority.

**Low, and behind [0109](#0109--disk-textures-are-887--of-every-milkdrop-conversion-failure-and-the-exclusions-trigger-condition-is-already-met).**
Both are conversion-rate work and both ride on the same trigger — a verdict that converted presets
are worth having more of — which Plan 0142's look gate read as **still not better** for the third
time (that entry's dated no-go, and
[ADR-0113](adrs/0113-milkdrop-presets-are-translated-ahead-of-time-onto-a-warp-mesh-idiom.md)'s third
`Outcome`). So neither is buyable today, and if that changes, 0109 goes first by 25x on its own
arithmetic.

**The 218 count above is un-recounted, and should not be restated as current.** It measures the
2026-08-16 corpus run, before Plan 0108, Plan 0109, Plan 0111, Plan 0180 and Plan 0142 Phase 3 each
changed what a converted preset renders. Whoever takes this re-runs `milkconv --report`/`--render`
over the corpus before touching anything; the numbers in *The finding* are what was true then, not a
present-day residual.

---

## 0109 — disk textures are 88.7 % of every MilkDrop conversion failure, and the exclusion's trigger condition is already met

**Raised by:** `architect`, at Plan 0108's planning sweep (2026-08-17), reading
[ADR-0113](adrs/0113-milkdrop-presets-are-translated-ahead-of-time-onto-a-warp-mesh-idiom.md)'s
Outcome and [Plan 0100](plans/done/0100-the-engine-speaks-milkdrop.md)'s followup list against
`docs/milkdrop-conversion.md`'s measured corpus tables. **Owner if taken:** `architect` (it reopens a scoped
exclusion and is ADR territory) then `dev`.

- **Verified 2026-08-17** — the exclusion is a named rejection class, deliberately, and says so in
  its own message: `present: fn disk_texture in: milkconv/src/shader/emit.rs`,
  `present: deliberately out of scope in: milkconv/src/shader/emit.rs`
- **Verified 2026-08-17** — the corpus tables this entry re-reads are in the operator doc, both
  eras: `present: WHY A FILE DID NOT CONVERT, ranked in: docs/milkdrop-conversion.md`
- `unprobeable:` the counts (1 217 / 609 / 2 058 / 88.7 %) are a measurement of one corpus run
  (2026-08-16, dev box, `WORK/milkdrop-corpus`), reproducible with `milkconv --report`, not a
  property of this tree

### The finding

**This entry claims nothing new about the mechanism. It claims the ranking was already decided and
nobody carried it.** Plan 0100's followup list says *"MilkDrop's `textures/` support, if Phase 5's
failure ranking says it is a large class."* Phase 5 ran, Phase 6 ran after it, and the ranking is
in `docs/milkdrop-conversion.md`:

| Rejection reason | Files | Share of corpus |
|---|---|---|
| warp shader `disk-texture` | 1 217 | 11.8 % |
| comp shader `disk-texture` | 609 | 5.9 % |
| **every other cause combined** | **232** | **2.2 %** |

Total conversion failures are `10 347 - 8 289 = 2 058`. Disk textures are **1 826 of them —
88.7 %**. Every other named cause in the whole corpus — HLSL arrays, computed conditions, parse
failures, unknown names, the EEL program classes — sums to 232 files.

So the conditional in Plan 0100's followup is **satisfied**, and by a margin that is not close.
The 19 % figure ADR-0113's Outcome prices the exclusion at was the *census grep*; the converter
sees a slightly wider class (it also flags `sampler_pc`) and measured **21.8 %** of the corpus
reading a disk texture.

### Why this is an entry rather than a line in Plan 0108

Plan 0108 is fidelity work on presets that already convert. This is the **conversion rate**, and it
is a different question with a different owner: shipping or sourcing texture files reopens
[ADR-0113](adrs/0113-milkdrop-presets-are-translated-ahead-of-time-onto-a-warp-mesh-idiom.md)'s
scope and collides directly with the provenance question Plan 0100 Phase 8 deferred (*decide
later*, nothing third-party in the repository or a release). **Those two are the same decision seen
from two sides** — a texture is third-party content exactly as a preset is — which is why this
wants an ADR and an interview rather than a phase.

It also sits *above* [0108](#0108--the-conversion-tail-hlsl-arrays-71-files-and-218-md2-presets-that-convert-but-render-blank)
in value by its own arithmetic: that entry's HLSL-array lowering recovers ~71 files, and this
recovers ~1 826. Both are conversion-rate work; only one of them is 25x the other.

### What a fix would be, and the shape is genuinely open

At least four routes, and they differ on the question this project has already deferred once rather
than on mechanism:

1. **Ship nothing, load from the user's own `textures/` directory** — the same shape as
   `RLX_PRESET_DIR` today. No provenance question at all, because nothing third-party enters the
   repository; the user who has the preset pack already has its textures. Cheapest, and the most
   consistent with Phase 8's standing answer.
2. **Substitute procedurally.** The six built-in noise textures already exist and 51 % of the corpus
   samples one. A missing disk texture could resolve to a procedural stand-in rather than a
   rejection — the preset renders *something its author did not draw*, which Phase 6 explicitly
   moved away from, so this trades fidelity for conversion rate and needs a judged look call.
3. **Ship a small curated texture set.** Highest fidelity, and it walks straight into the licensing
   question Phase 8 deferred.
4. **Keep the exclusion and stop calling it a corner.** Legitimate, and it is the null option that
   should be named: the cost is stated, the 8 289 that convert are the product, and the entry closes
   as a decision rather than as work.

### Priority

**Medium, and it is the largest single lever on the import's reach.** It blocks nothing — the
import ships and works on four fifths of the corpus. Take it when the fidelity work
([Plan 0108](plans/done/0108-the-milkdrop-import-gets-its-tone-back.md)) has settled whether converted
presets are worth having more of, which is the honest ordering: reach is only worth buying after
quality is judged. **Do not take it before Plan 0108's Phase 2**, whose verdict on whether these
presets read as better or merely different is exactly the evidence for how much reach is worth.

### The go/no-go, third time — **no-go, 2026-09-18**

[Plan 0142](plans/done/0142-the-milkdrop-import-earns-its-verdict.md) Phase 4 re-ran the seven pairs
against `foo_vis_milk2` 0.2.0.0 (DX11), and
[ADR-0113](adrs/0113-milkdrop-presets-are-translated-ahead-of-time-onto-a-warp-mesh-idiom.md)'s third
`Outcome` records what the user read: **one pair better, two good, one fixed, two still washed at the
ground, one wrong on structure.** The precondition this entry sets for itself — that the fidelity
work has settled whether converted presets are *worth having more of* — is therefore **still unmet**,
after Plan 0100 Phase 7, Plan 0108 Phase 2 and now Plan 0142 Phase 4. This entry is unbought for the
third time by a verdict, not un-picked-up.

**What is different about this "no", and why it is not the same answer a third time.** The first two
were *merely different* with one defect dominating and unnamed. This one has the wash **named at the
reference's own source** (this file's archived 0113 body, `### Update 2026-09-17`: the reference
truncates its per-frame decay factor and multiplies it in an encoded domain) and **partly repaired**
— Plan 0142 Phase 3 — and the plan's own subject moved from washed to fixed. Each of the three pairs
that still read wrong now names a **different** mechanism: a per-frame deposit against a per-second
transform rate at 164-165 fps, an echo that is bound and is not nesting (*Songflower*, which sets
`fDecay = 1.000` so no wash repair reaches it), and a waveform scale confirmed on one mode only. So a
fourth gate has a route rather than a re-ask, and this entry's trigger is that verdict and nothing
else.

Nothing in this entry's own arithmetic moved: 1 826 files, 88.7 % of every conversion failure, 25x
the ~71 of
[0108](#0108--the-conversion-tail-hlsl-arrays-71-files-and-218-md2-presets-that-convert-but-render-blank).
**If reach is ever bought, this is still the one to buy first**, and it still wants an ADR and an
interview rather than a phase.

---

## 0125 — every diffused frame is an upscale: both profiles diffuse well below the stream's own resolution

**Raised by:** the user, at Plan 0106's Phase 6 human gate (2026-08-25), on a full-track render of
`star_rosewindow` — *"it would obviously be great if resolution would be higher"*. **Owner if
taken:** `architect` — it reopens a clause ADR-0121 recorded as deliberately rejected, so it is an
ADR question before it is a code one.

- **Verified 2026-08-25** — the shipping `quality` profile diffuses at a 589,824 px budget, which is
  28 % of a 1920x1080 frame, so every output pixel is resampled up:
  `present: "size": "589824" in: tools/sd-filter/sd_filter.py`

### The finding

The clip that drew the verdict was rendered at **`fast`** — a 262,144 px budget, **680x384** at
16:9 — and resampled to 1920x1080. `quality` is 1024x576, **2.25x the pixels**, and *has never been
rendered on a real track*. So an unknown and possibly large share of this complaint is a profile
choice rather than a wall, and **the cheap first move is a side-by-side still at both budgets**, not
a design.

What is genuinely walled, and why this is not simply "raise the budget":

- **SD1.5 duplicates or mirrors content above roughly 768²** — its native-resolution artifact, named
  in Plan 0106 Phase 1's traps. Raising the budget does not scale smoothly into it.
- **SDXL plus ControlNet is ~7.5 GB against an 8 GB card**, and the spike already peaks at 5.68 GB
  with two ControlNets loaded. Offloading fixes the memory and ruins the throughput over thousands
  of frames, which Phase 1 also measured.
- **Cost scales with pixels.** Phase 2b measured 2.721 s/frame at 589,824 px against roughly a third
  of that at 262,144. A 4-minute track at `quality` already measures ~5.9 h *before* the 1.406x
  scope correction Plan 0106 Phase 7d applies to that figure.

**The tension worth surfacing before anyone designs.**
[ADR-0121](adrs/0121-the-diffusion-filter-is-an-offline-stage-with-profiles-and-it-interpolates-its-own-stride.md)'s
Alternative C is *diffuse at a smaller budget and upscale*, measured as the cheaper route and
**rejected by this same user in the design interview**, on the ground that generated detail is worth
its price against inferred detail. This verdict does not obviously overturn that — the ask is for
*more* detail, and an upscaler infers rather than generates — but it does mean the rejection was
made before anyone had watched five minutes of output. A tiled or multi-pass approach that
*generates* at higher resolution is the option neither the ADR nor the plan has costed.

## 0126 — a render is one prompt, one seed and one preset from first frame to last, so nothing varies across a track

**Raised by:** the user, at Plan 0106's Phase 6 human gate (2026-08-25), on a 5:15 render —
*"...and with more variety"*. **Owner if taken:** `architect` — it is squarely inside Plan 0106's
stated non-scope, so taking it is a scope decision, not an implementation one.

- **Verified 2026-08-25** — one seed is fixed for the entire render, by construction and on purpose:
  `present: manual_seed\(cfg in: tools/sd-filter/sd_filter.py`

### The finding

This is **not a defect** — it is Plan 0106's *What this plan does NOT do*, in as many words:
*"No timeline, cuts, or prompt automation across a track. One prompt per render, matching 0101's one
preset per render."* The fixed seed is load-bearing for the thing the gate approved: Phase 1 records
that a per-frame seed *"guarantees boiling whatever else is tuned"*. So variety cannot be bought by
simply unfixing it, and that is the first thing a designer would reach for.

The plan already carries the shape of the answer in its own Followups — **audio-conditioned
diffusion**, *"denoise from the onset envelope, prompt blend on bar boundaries"* — which was filed as
a nice-to-have and is now a user ask with a watched render behind it. Levers worth costing, roughly
in increasing order of what they disturb:

- **Prompt interpolation on bar or section boundaries**, which the analyzer can already supply. The
  smallest change that produces real variation, and it keeps one seed and one preset.
- **Preset changes across a track** — 0101 renders one preset per render, so this is a `shot`
  question before it is a filter question.
- **Denoise strength driven by the onset envelope**, which is the one lever that **reopens Plan
  0106's no-audio-conditioning decision**: it carries real audio data across a seam the plan
  deliberately kept image-only. Phase 2 named exactly this as the repair if the music stopped
  reading — it did not, so this would be taken for variety rather than for reactivity, which is a
  different justification and wants its own ADR.

**Do not fold this into a resolution plan.** It shares a verdict with backlog 0125 and nothing else:
one is a pixel budget against a VRAM wall, the other is a timeline the pipeline does not have.

## 0154 — a swap spawns a thread that creates a COM object, and one activation in 22 failed with `REGDB_E_CLASSNOTREG` where the retry budget cannot tell that from a dead device

> **Filed 2026-08-28** at the Plan 0130 Mode 4 review, from that plan's own Phase 5 log — an
> observation the plan reported honestly, claimed no mechanism for, and had nowhere to leave.

Before Plan 0130 `capture_win::start` ran **once per process**. It now runs on every input swap and
on every recovery attempt, and each run spawns a thread that does `CoInitializeEx(MULTITHREADED)`,
`CoCreateInstance(MMDeviceEnumerator)`, `CoUninitialize`, and exits. Under menu-speed churn on the
development box, **one swap in 22 failed** at that `CoCreateInstance` with
`REGDB_E_CLASSNOTREG (0x80040154)`.

**What is and is not claimed.** The shell degraded exactly as designed — the reason printed, the
verdict read `failed WASAPI … Class not registered`, rendering continued, and the next swap came
back live. No mechanism is claimed: the apartment-churn reading is untested, one run on one box is
not evidence for a cause, and 1-in-22 is a single sample, not a rate.

**Why it is worth an entry rather than a note.** `poll_input_lost` reopens up to
`INPUT_RECOVERY_ATTEMPTS` times on **consecutive frames** — the fastest thread-spawn churn the design
can produce, and faster than the churn that drew the error. The budget exists to bound COM
activations against a device that is not coming back; it cannot distinguish an activation that
failed *for its own reasons* from one that failed *because the endpoint is gone*, so a real loss
whose reopens drew this error would spend all three attempts on the wrong failure and write a
`lost …` verdict about a device that was fine. That is the one path where this turns a transient
into a wrong answer, and it is also the path
[`docs/on-device-validation.md`](on-device-validation.md)'s unplug item says has never run.

**Impact:** low frequency, narrow blast radius, and invisible while swaps stay operator-paced. It
matters only where the design already churns fastest, which is the recovery path.

**What a fix looks like** — three shapes, cheapest first, and picking between them wants the unplug
evidence rather than more reasoning:

1. **Retry the activation once, in place**, before charging the attempt to the recovery budget. A
   class-registration failure is not a statement about the endpoint, so it should not spend a
   budget that is counting statements about the endpoint.
2. **Keep one long-lived enumerator on the render thread** rather than creating one per stream
   start. `MMDeviceEnumerator` is a Both-model object and `ComScope` already handles the STA the
   render thread lives in, so `endpoints()` has the pattern; `setup_stream` creates its own because
   it runs on the capture thread.
3. **Separate the two failure classes in the verdict**, so `failed … Class not registered` and
   `lost …` never read as the same conclusion about the device.

- **Verified 2026-08-28** — a fresh COM object is still created per stream start, on the capture
  thread: `present: CoCreateInstance in: standalone/src/capture_win.rs`
- **Verified 2026-09-06** — discharging the REGDB-absence bullet this entry
  carried until Plan 0147 Phase 2 landed the third shape. The two failure classes are now separate
  verdicts about separate subjects: `present: LossCause in: standalone/src/capture_verdict.rs`
- **Verified 2026-08-28** — the budget that would be spent on it is still the only bound:
  `present: INPUT_RECOVERY_ATTEMPTS in: standalone/src/capture_start.rs`

**Update 2026-08-30, at Plan 0135's close — still live, still unevidenced.** That plan gathered the
three fixes whose shape was settled and left this one deliberately unfixed: its Phase 5 was a
`human` unplug gate whose whole deliverable was the evidence this entry asks for, and **it did not
run**, for the same reason Plan 0130's Phase 5 did not — there is no removable audio interface on
the box. Nothing about the three candidate shapes has changed and none is preferred; the entry is
carried, not stalled.

**Two things Plan 0135 did change under it**, both worth knowing before anyone re-reads the reasoning
above. The reopen budget is unchanged — still `INPUT_RECOVERY_ATTEMPTS = 3` spent on **consecutive
frames**, so the *fastest churn the design can produce* is still what this entry says it is, and it
is still refresh-rate-dependent even though the *settle* window beside it is now in seconds
(`INPUT_RECOVERY_SETTLE_SECS`, backlog 0155, archived). And an operator-initiated restart now resets
the policy (backlog 0156, archived), so the swap that drew the original 1-in-22 observation returns
a full budget where it used to inherit a spent one — which makes a repeat observation *more*
likely to be visible, not less.

**Where the evidence is now owed from:** the unplug checkbox in
[`docs/on-device-validation.md`](on-device-validation.md), which carries Plan 0135's three extra
questions (**(d)** does `REGDB_E_CLASSNOTREG` appear during a *real* loss, **(e)** how many attempts
a real unplug consumes, **(f)** does the verdict name the right cause) alongside Plan 0130's
original three, and a Standing bullet in [the plans index](plans/README.md). Run against v0.95.0 or
later, or the policy under test is not the repaired one.
- **PARTLY PROMOTED 2026-09-01 -> [Plan 0147](plans/done/0147-what-the-show-costs-and-what-its-numbers-mean.md) Phase 2**, which takes **only the third shape** - the verdict
  stops reading the same about an activation and about a device. **The mechanism halves stay filed and
  this entry stays live**, because choosing between retry-in-place and a long-lived enumerator wants the
  unplug evidence, and the box still has no removable interface.

**Update 2026-09-06, at the Plan 0147 Phases 1-3 review — the third shape landed, and it is wired to
the wrong incident.** `CaptureVerdict::Lost` now carries a `LossCause`, and the two tokens are
distinguishable and tested. What is not yet right is which one the shell picks. The evidence flag
`reopen_reached_endpoint` is cleared on the **lost-flag rising edge**, but a reopen that *succeeds*
clears `input_lost`, so the next re-loss inside the same budget presents as a fresh incident and
wipes the flag. The recovery budget's incident is wider — `RecoveryPolicy` keeps `attempts` until
`INPUT_RECOVERY_SETTLE_SECS` of unbroken delivery — and the give-up verdict is a statement about
*that* incident. So in the flap path `capture_start.rs`'s own
`a_stream_that_dies_as_fast_as_it_opens_still_gives_up` models, every attempt reaches an endpoint
and the verdict still reads *"none of which reached an endpoint"*. The evidence has to be scoped to
the budget's incident — cleared where `RecoveryPolicy` resets itself, not on the lost flag. Carried
as Plan 0147 Phase 3c; **this half of the third shape is not discharged until that lands.**

- **Updated 2026-09-15** - the whole third shape has landed (`LossCause`, and `forget_if_restored`
  clearing on the budget's reset in `standalone/src/capture_start.rs`). What stays live is the fix
  itself, shape 1 or 2: `capture_win.rs` still creates a COM object per stream start, with no
  in-place retry, and it waits on the unchecked unplug row of `docs/on-device-validation.md`.

## 0165 - the windowed app cannot ask for the discrete GPU, so every windowed frame-time figure this project has quoted is an integrated-GPU figure

> **Filed 2026-08-30** at Plan 0131's close. Found by Phase 6, which requires a frame-rate figure to
> name the GPU that produced it (ADR-0071) - and the window had no equivalent of the stream mode's
> startup print, so the phase could not have been satisfied without adding one.

The windowed path builds its adapter with `request_adapter` against a `compatible_surface` and a
**default power preference**, which on a hybrid laptop hands back the power-saving GPU.
`RenderContext::new` takes no adapter choice at all: [ADR-0146](adrs/0146-one-name-selects-the-gpu-and-each-side-matches-its-own-roster.md)
gave `--gpu` to `--stream` and to the Spout sender, and there is no windowed equivalent.

Measured on the dev box, which has both an RTX 3080 and integrated Radeon graphics:

```
# renderer adapter: AMD Radeon(TM) Graphics (Dx12, IntegratedGpu), driver 30.0.13002.1001
```

**Every windowed frame-time number this project has ever recorded on this machine is therefore an
iGPU number** - the NFR 1 checks, the soak runs, the tier calibration readings, Plan 0131's own
console measurement - and until this note was added nothing said so. The numbers are not wrong; they
are attributed to a machine rather than to the adapter inside it, which is exactly what ADR-0071
exists to stop.

**This is also why Plan 0131's dual-GPU degrade path has still never been exercised.** `open_console`
treats an `attach_aux` failure as non-fatal, logs it and leaves the show untouched, and that branch
is unreachable on a single-adapter run - which every run here is, because the window cannot be put on
the other GPU.

### What a fix looks like

Extend `--gpu` to the windowed path: `RenderContext::new` takes the same `AdapterChoice` the stream
mode already resolves, matched against the same roster, with the same
[ADR-0146](adrs/0146-one-name-selects-the-gpu-and-each-side-matches-its-own-roster.md) name rule.
That is a small change with one real question attached - whether a windowed surface can be created
on an adapter that does not drive the display it is on, which is the same dual-GPU question
0131 Phase 6 is still owed - so it wants measuring before it is promised. The startup note that names
the running adapter has already landed and is what makes any of this attributable.

> **Updated 2026-08-31, at Plan 0144's close. Half discharged: the lever landed, the measurement did
> not.** [ADR-0155](adrs/0155-the-window-takes-the-adapter-and-the-preset-the-operator-names.md) gave
> `--gpu` to the windowed path exactly as the fix above describes, and it was observed working —
> `ritmolux --gpu 1` put this box's window on `NVIDIA GeForce RTX 3080 Laptop GPU (Dx12, DiscreteGpu)`
> with the startup line reading `(pinned by --gpu)`. The dual-GPU question this entry said *"wants
> measuring before it is promised"* is answered for the surface-creation half: a named adapter that
> cannot present is refused by name rather than silently swapped.
>
> **What is left is this entry's actual title.** No windowed frame-time figure has been re-taken on
> the discrete adapter, so every published one is still an iGPU figure — and that is now true *by
> choice* rather than by impossibility, because Plan 0144 deliberately left the unflagged request at
> `AdapterChoice::Default` so no existing number would move underneath a CLI change. The remaining
> work is a measurement pass producing a **new row** beside the iGPU numbers, not a correction to
> them. The first probe below is rewritten because Phase 2 falsified its reduction, not its claim.

> **Updated 2026-09-06, at Plan 0147 Phase 6. The measurement half is discharged; the degrade half is
> not, and this entry stays live for it.** A windowed frame-time row now names
> `NVIDIA GeForce RTX 3080 Laptop GPU (Dx12, DiscreteGpu)` and sits **beside** the iGPU figures in
> [nfr.md](nfr.md), which were not edited. It is a matched pair taken on one build minutes apart —
> 1080p windowed, `Rich` tier, rotation on — so the two rows are comparable to each other rather than
> to a cross-build figure: the discrete part reads 165.0 fps median with a worst p99 sample of
> 8.936 ms, the unflagged integrated part 112.8 median with 21 of 171 samples under the 60 fps floor.
> Neither drops a frame. **This entry's title is now satisfied**: a published windowed figure names
> the discrete adapter.
>
> **The console's dual-GPU degrade path still has not executed, and this box cannot make it.** A
> window pinned to the adapter that does not drive the display was the configuration in which it
> could first have fired, and it did not — the console opened normally on the RTX 3080 (`Mailbox`,
> frame latency 1) and presented 9,834 times with 0 skips. On a single-display Optimus laptop the
> discrete adapter presents to a window the integrated part composites, so nothing refuses. Reaching
> that branch needs a genuinely multi-adapter display topology; the last probe below therefore still
> holds, and the entry keeps this half.

- **Verified 2026-08-31** - re-written: the constructor now takes the choice, so the old reduction is dead; what stands is that the window still *asks* for the default when unflagged: `present: None => AdapterChoice::Default in: standalone/src/gpu.rs`
- **Verified 2026-08-30** - and the code's own doc says what the default yields on a hybrid box: `present: the power-saving GPU for a console process in: core/src/render/context.rs`
- **Verified 2026-08-31** - the two unflagged arms are held apart, which is what keeps the published figures comparable: `present: fn the_window_and_the_stream_disagree_when_unflagged in: standalone/src/gpu.rs`
- **Verified 2026-08-30** - the startup note that makes a figure attributable exists: `present: renderer adapter in: standalone/src/app_state.rs`
- **Verified 2026-08-30** - the console's degrade branch is still built and still unreachable here: `present: console surface unavailable on this adapter in: standalone/src/app_state.rs`
- **PARTLY PROMOTED 2026-09-01 -> [Plan 0147](plans/done/0147-what-the-show-costs-and-what-its-numbers-mean.md) Phase 6**, which takes the measurement half: a new windowed
  frame-time row naming the discrete adapter, beside the iGPU figures rather than replacing them. The
  phase also records whether the console's dual-GPU degrade path became reachable; **if it stays
  unexercised this entry keeps that half and stays live.**
- **Updated 2026-09-15** - it stayed unexercised. The title ask is discharged (`--gpu` reaches the
  window, and `docs/nfr.md` carries a discrete-adapter windowed row); the live half is the console's
  dual-GPU degrade path in `standalone/src/app_state.rs`, which needs a multi-adapter display setup.

## 0187 — two measurements of the same console on the same adapter class disagree by 2x, and nothing explains which one the machine actually does

**Raised by:** `architect`, at [Plan 0147](plans/done/0147-what-the-show-costs-and-what-its-numbers-mean.md)'s
close review (2026-09-06), carrying the residue of archived 0164. **Owner if taken:** `human` first
— it is a measurement question before it is a design one, and the configuration that would settle it
is hardware this box does not have.

On 2026-08-30 a 95 s hands-off pair differing only by `--console` read **61.7 fps closed against
33.1 open**, `frame_ms_p99_steady` 18.6 ms against 47.3 ms — landing within 3 % of exactly half,
which is the shape of two presents serialising. On 2026-09-06 the same protocol, on the same
integrated Radeon and the same single 165 Hz display, read **53.5 closed against 53.1 open**, with
four further arms spanning 52.3 to 54.5 and a witness attached: every open arm presented ~4,700
times with **zero skips**, and at the vsync cap 14,797 presents cost the output 0.0 fps. The second
reading is the better-instrumented one by a wide margin. It is not a refutation of the first — the
two are different builds, and a cross-build comparison is one Plan 0147 explicitly forbade itself —
so what stands is that **a 2x cost was measured once, has never been reproduced, and has no named
cause.**

Three hypotheses, none tested: a build difference somewhere in the 71 versions between them; a
configuration difference the first window did not record (the console's size, its position, whether
another window overlapped it); or an uncontrolled variable in the first reading, which had no present
count and therefore cannot distinguish a console that cost 29 ms from one whose surface was in a
state the second window never entered.

**What would settle it, and why it is not free.** Both windows put both surfaces on **one display at
one refresh rate**, which archived 0164 and
[ADR-0143](adrs/0143-the-operator-console-is-a-second-surface-and-the-shell-owns-its-meaning.md)'s
first `Outcome` both name as precisely the configuration that cannot separate the two pacing
sources. The cross-refresh, two-display run is still owed and needs a second monitor at a different
refresh rate. Until then the honest statement in the operator docs is the one Plan 0147 Phase 5
shipped: the cost is a measurement, quoted with its adapter and its present count, not a guarantee.

- **Verified 2026-09-06** — the instrument that makes any re-run readable now exists and reconciles
  against the frames the loop ran:
  `present: pub struct AuxCounts in: core/src/render/aux_target.rs`
- **Verified 2026-09-06** — and its totals reach the log rather than dying with the process:
  `present: console \{label\}: in: standalone/src/app_state.rs`
- **Verified 2026-09-06** — both levers stay reachable, so a re-run costs a config edit rather than
  a build:
  `present: pub present_every_n in: standalone/src/config.rs`
- `unprobeable:` that no run anywhere has put the two surfaces on displays at different refresh
  rates is a negative about measurement history, not a match countable in any file

---

## 0219 — a `ctl/preset` datagram on loopback never reached the listener's queue, in 3 of 79 loaded runs, and nothing counted it

`a_preset_datagram_selects_by_name` in `standalone/tests/control_loopback.rs` binds a `Control` on
`127.0.0.1:0`, builds a headless renderer, sends one `ctl/preset second` datagram and waits for
`has_pending()`. On 2026-09-14, on the reference machine, with `golden`, `attractor` and
`reaction_diffusion` running in a second nextest process beside it, the test failed 3 times in 79
back-to-back runs of the `stream_show` and `control_loopback` binaries. All three failed at that
first wait, and all three said the same thing:

```text
ctl/preset `second`: nothing reached the listener within 5s (gave up after 5.00 s); still nothing
10.00 s after the send; listener counters: rejected 0, dropped 0
```

So the datagram was neither late, refused by the decoder, nor dropped on a full queue. It never
reached the queue. The `send_to` had returned `Ok`. The other three tests in the binary, which
also send on loopback, passed in all 79 runs. Of the four, this is the only one that builds the
renderer after binding the listener and before sending. That is an observation, not a cause.

**What the evidence cannot say:**

- Whether the listener thread was still running. `Control` exposes no liveness, and the test was
  not given one (ADR-0193 kept `Control`'s public surface fixed for the diagnosis).
- Whether `recv_from` was failing. `listen` in `standalone/src/control.rs` swallows every receive
  error with a bare `continue` and counts nothing, so a listener whose receive keeps failing looks
  exactly like an idle one.

Those two gaps are the first thing to close. The studio drives the player through this listener,
and a lost `ctl/preset` there shows up as a click that does nothing.

**Seen again 2026-09-15**, in a conductor gate with no other lane running: `gate 0175-post-close`
on tree `05d1639` failed `a_preset_datagram_selects_by_name` (1696 passed, 1 failed, exit 100). A
hand run on the same tree 20 minutes earlier and the same gate after `resume` both passed. That is one
red in about five full suites that day, and it cost a park, a resume and an 11.4-minute re-run. The
gate does not retry (ADR-0193), so while this entry is open every full suite carries that chance.

- **Raised:** 2026-09-14, by `dev` during the ADR-0193 diagnosis. **Owner if taken:** `dev`; making
  the swallowed receive error observable comes before any fix.
- **Verified 2026-09-14** — a receive error is swallowed without a count:
  `present: let Ok\(\(len, _from\)\) = socket\.recv_from\(&mut buf\) else \{ in: standalone/src/control.rs`
- **Verified 2026-09-14** — the test's delivery failure reports the listener counters and a late check:
  `present: still nothing \{:\.2\} s after the send in: standalone/tests/control_loopback.rs`

### Priority

**Medium.** It is a loopback datagram lost on the control path the studio uses. It is intermittent
and has so far been seen only under heavy concurrent GPU load.

## 0220 — a headless walk of the system roster stalls at `emitter`: the ping sent with the ask is answered and the preset never reaches the screen

`every_system_is_reported_by_the_key_the_schema_labels_its_roster_with` in
`standalone/tests/stream_show.rs` spawns the player with `--stream --events --control`, holds
rotation, then sends one `ctl/preset` per `SystemKind::ALL` entry. After each one it waits for the
`preset` event naming it. Each ask is followed by a `ctl/ping`. The ping is answered by the same
`apply_control_rest` drain that applies the preset — but the two are **separate datagrams**, so a
`pong` proves that the *ping* was drained, not that the preset was.

> **Corrected 2026-09-14 at Plan 0174's close.** The entry as raised read the pong as proof the
> ask's frame was drained, and concluded *"the ask was not lost"*. That does not follow: 0219 shows a
> single loopback datagram vanishing with `rejected 0, dropped 0`, and the same thing happening to
> the preset datagram while the ping behind it arrived would print exactly the report below. A lost
> datagram is therefore a **fourth candidate**, and 0219 and 0220 may be one defect. Against it:
> both failures stopped on the same ask, which a random loss over fifteen datagrams would do about
> one time in fourteen. The test's `Ask::ping` doc comment carries the same overclaim.

On 2026-09-14, under the load described in 0219, the walk failed twice in 79 runs. Both times it
stopped at **ask 9 of 14, `emitter`**, with 8 systems already reported:

```text
ctl/preset `emitter` (ask 9 of 14), awaiting its `preset` event: nothing after 60.0 s (Deadline,
bound 60s); still absent 120.0 s after the ask
child: still running
ctl/ping 424250 sent with the ask WAS answered
stdout: 576000 bytes at the ask, 208857600 bytes now
  | {"v":1,"ev":"pong","nonce":424250}
  | {"v":1,"ev":"health","fps":29.9807,...,"ctl_rejected":0,"ctl_dropped":0,"ctl_refused":0,...}
```

The ping behind the ask was drained, the child kept drawing at 30 fps, and no `roster`, `preset` or
`preset_error` line followed in two minutes. `report_active_preset` in `standalone/src/show.rs`
emits only when `renderer.preset_name()` changes, so the preset on screen never became `emitter`.
There are four candidates, none yet distinguished: the preset datagram never reached the queue (see
0219); `select_preset_by_name` returned `false`; it returned `true` and the dissolve never completed;
or the selection was overridden. The walk's ninth step does not follow
on from anything earlier in the walk: the same ask succeeded in 77 other runs.

The earlier red run recorded in ADR-0193 stopped at ask 2 of 12, with nothing kept that could say
why. Whether it was the same defect is unknown.

- **Raised:** 2026-09-14, by `dev` during the ADR-0193 diagnosis. **Owner if taken:** `dev`.
- **Verified 2026-09-14, re-pointed 2026-09-16** — the `preset` event fires only on a change of what
  is on screen. [Plan 0179](plans/done/0179-a-parameters-range-belongs-to-its-family.md) Phase 4 moved
  the comparison into `Show::preset_report` and widened the key from the name alone to name, system
  and family, so the probe names the new shape. **The claim this entry rests on is unchanged** — a
  repeat of the same preset still emits nothing:
  `present: seen == name && \*was == system && \*drew == family in: standalone/src/show.rs`
- **Verified 2026-09-14** — the pong is emitted by the drain that applies the preset:
  `present: events\.emit\(&Event::Pong \{ nonce: \*nonce \}\); in: standalone/src/show.rs`
- **Verified 2026-09-14** — the walk sends a ping with every ask:
  `present: Evidence only, never asserted: see .Ask::ping. in: standalone/tests/stream_show.rs`
- **Verified 2026-09-14** (the close's correction) — the ping goes out as its own datagram after the
  preset's: `present: Action::Ping\(nonce\)\.encode\(&mut buf\); in: standalone/tests/stream_show.rs`

### Priority

**Medium.** A `ctl/preset` the player drains and does not show is the studio's library click doing
nothing. It is intermittent and has so far been seen only under heavy concurrent GPU load.

## 0221 — the run-alone override costs `-P fast` 165 s, twice its tests' serial time, because each of its 18 testcases drains the machine separately

ADR-0193's override gives each selected testcase every nextest slot. On 2026-09-14, at Plan 0174's
close, one run per arm on the same tree, back to back on the reference machine: `-P fast` took
**244.6 s without it and 409.5 s with it**. The selected set takes about 85 s when its tests run
one at a time. The rest is idle slots, which are paid once per selected testcase and not once per
binary. There are 18 selected testcases, and 11 of them are `help_cli` (8 tests, 0.3 s of work
between them) and `stream_pipe` (3). That lands on every pre-push, every `dev` phase gate and CI's
`check` job. The close accepted the cost and routed the cheaper shape here.

Shapes worth measuring, none yet measured. Each keeps ADR-0193's guard honest:

- **Fewer, larger exclusive testcases.** Fold `help_cli`'s eight into one test per timing-sensitive
  property. That is a test edit, and the guard reads binaries, so it does not notice.
- **Take `help_cli` out of the class.** Its 1 s bound sits around a process that exits in
  milliseconds. Listing it in `CLOCK_ALONE_EXEMPT` with a reason is the guard's own escape. It
  weakens ADR-0193's Decision point 2, and it needs a sentence there, not only an entry.
- **Fewer than every slot.** `threads-required` at a fraction of the machine removes most of the
  load for less drain. That is ADR-0193 Alternative B's question in a new shape, and it needs an
  ADR amendment.

Whether nextest's queue holds unrelated tests behind a waiting exclusive one, or lets them fill the
slots while it waits, has not been checked. That decides how much any of these buys, so check it
first.

- **Raised:** 2026-09-14, by `architect` at Plan 0174's close. **Owner if taken:** `dev`, with an
  `architect` amendment to ADR-0193 for the third shape.
- **Verified 2026-09-14** — the override claims every slot for each selected test:
  `present: threads-required = "num-test-threads" in: .config/nextest.toml`
- **Verified 2026-09-14** — `help_cli` is selected whole:
  `present: binary\(help_cli\) in: .config/nextest.toml`

### Priority

**Medium.** No test is wrong. The cost is 165 s on every push, and a gate that hurts gets bypassed
(ADR-0033 Alternative F).

---

## Entries 0227-0235 — from the Plan 0189 Phase 8 watched runs (2026-09-15), all archived

Raised by the owner and a human-started session watching four conductor runs that merged 0175 and
carried 0177 to its close. The owner's complaint was speed; 0227 is that, and the rest are what cost
the runs their parks. **Every one of the nine has left this file.** 0228-0235 were closed by
[Plan 0190](plans/done/0190-the-conductor-survives-a-run-nobody-is-watching.md) on 2026-09-16, and
0227 was promoted the same day to
[Plan 0191](plans/done/0191-a-green-tree-is-not-tested-four-times.md) - its skip half only, the
cheaper-suite half living on as 0239 below. The bodies and the verdicts are in
[the archive](design-backlog-archive.md); this heading stays because the group is how they were
raised and the next reader of one will look for the other eight.

## 0236 — the `.claude/` park reads a phase's declared `Files touched`, and Plan 0190's own Phase 9 declared its three `.claude/` files in prose

[ADR-0210](adrs/0210-a-claude-repair-is-the-owners-and-a-session-that-needs-one-parks-with-the-edit.md) parks a plan in front of a phase whose files include a path
under `.claude/`, because the CLI refuses a headless session an `Edit` there. `claudePaths` (
`tools/conductor/lib/plan.mjs`) implements exactly what the ADR says: it scans the phase's
`**Files touched:**` bullet for a literal `.claude/…` path. `nextStep` parks on a non-empty result.

The guarantee is therefore only as strong as a plan's prose, and **the plan that built the mechanism
is itself the counterexample**. Run over
[Plan 0190](plans/done/0190-the-conductor-survives-a-run-nobody-is-watching.md) at its close:

```
1 dev [".claude/hooks/conductor-no-background.js", ".claude/settings.json",
       ".claude/skills/dev/SKILL.md", ".claude/skills/architect/SKILL.md",
       ".claude/skills/studio-builder/SKILL.md"]
9 dev []
```

Phase 9 edited three `.claude/skills/*/SKILL.md` files. Its `Files touched` names them as *"`lib/lane.mjs`,
`lib/outcome.mjs`, `prompts/*.md` and the three conductor-mode sections"* — a true sentence with no
literal path in it. Under the conductor that phase would have been handed to a session, run, hit the
denial, and parked `check_red` on its own done-when: precisely the late failure ADR-0210 exists to
move to the front. A whole-body scan would not have caught it either; the paths are not written
anywhere in the phase.

Shapes, none decided:

- **A gate rather than a parser.** `check-index-rows.mjs`-style: a plan whose phase body names a
  conductor-mode section, a skill or a hook by description, without a path in `Files touched`, is a
  drafting error the architect fixes before approval. Cheap, and it fails at the right time.
- **Widen the scan** to the whole phase body, which catches a `.claude/` path written in a Done-when
  and still misses this one. Strictly weaker than the above.
- **Accept it as bounded.** ADR-0210's Negative already says a plan that touches `.claude/` cannot
  run under the conductor at all; the residual is only that an *undeclared* one fails late rather
  than early, which is today's behaviour and no worse.

- **Raised:** 2026-09-16, at Plan 0190's close review, by running `claudePaths` over that plan's own
  phases. **Owner if taken:** `architect` (whether this is a gate or a parser), then `dev`.
- **Verified 2026-09-16** — only the `Files touched` bullet is read:
  `present: String\(phase\?\.filesText \?\? ""\) in: tools/conductor/lib/plan.mjs`
- **Verified 2026-09-16** — and that bullet is the only thing `nextStep` consults for the park:
  `present: claudePaths\(byId.get\(id\)\).length > 0 in: tools/conductor/lib/plan.mjs`

### Priority

**Low.** It degrades to the behaviour that existed before ADR-0210 — a late park instead of an early
one — and the conductor is stood down. It matters the first time a conductor-run plan touches a skill
file without naming it, which is a normal thing for a plan to do.

## 0237 — the session allowlist bounds a deletion by four literal path shapes, so a path the shell expands escapes the lane

[Plan 0190](plans/done/0190-the-conductor-survives-a-run-nobody-is-watching.md) Phase 2 allowed
`Bash(rm *)` and `PowerShell(Remove-Item *)` and bounded them with deny rules for the four ways a
written path leaves the worktree: `..`, `~`, a leading `/`, and a drive letter (`*:/*`, `*:\*`).
Inside the lane, a relative path with no `..` cannot escape, so the bound holds for a path a session
*writes out*.

It does not hold for a path the shell *produces*. None of these match a deny rule, and all are
allowed:

```
rm -rf $HOME/.cargo
rm -rf "$(git rev-parse --show-toplevel)/../rlx-plan-0180"
Remove-Item -Recurse $env:USERPROFILE\WORK
```

`test/settings.test.mjs` cannot see this either, and says so in its own header: it models the CLI's
rule matching over the command *text*, which is the same level the CLI matches at. The gap is not in
the model — it is that a glob over text cannot bound a path that does not exist until the shell runs.

Shapes, none decided:

- **Deny the expansion syntax**, not the path: `rm *$*`, `rm *%*`, `Remove-Item *$*`. Crude, cheap,
  and it costs a session nothing it needs — a phase deleting its own scratch writes a literal path.
- **Allowlist the scratch roots instead of the verb**: `Bash(rm -rf target/*)`, `Bash(rm target/*)`,
  and nothing else. Narrower than what Phase 2 was asked for, and backlog 0231 lists deletions
  outside `target/` (`studio/shared/seed-target.ts`).
- **Accept it.** The lane is a worktree with nothing irreplaceable in it, a sibling lane is
  recoverable from its branch, and the blast radius of `$HOME` is the machine rather than the repo —
  which is exactly the argument against accepting it.

- **Raised:** 2026-09-16, at Plan 0190's close review. **Owner if taken:** `dev`.
- **Verified 2026-09-16** — the verb is allowed wholesale:
  `present: "Bash\(rm \*\)" in: tools/conductor/settings.conductor.json`
- **Verified 2026-09-16** — and no rule mentions an expansion:
  `absent: HOME in: tools/conductor/settings.conductor.json`

### Priority

**Medium.** Nothing has triggered it and no session has reason to write one, but it is the one rule
in that file whose stated bound — *"a path that leaves the lane is refused, whatever it is for"* — is
not the bound the rules actually enforce, and the README repeats the claim.

---

## 0239 — three per-preset suites are 54 % of the workspace suite and grow with every shipped preset, because each rebuilds its own headless renderer

[ADR-0211](adrs/0211-a-green-suite-record-serves-a-later-tree-when-no-deferred-suite-can-read-the-diff.md)
stops a green tree being tested four times. It deliberately does not touch what one test costs, and
records this as its Alternative A. This is that alternative.

Measured from `0175-remerge-18-cargo_nextest.log` on 2026-09-15 — 7378 test-seconds over 73 binaries:

| Suite | Test-seconds | Share |
|---|---|---|
| `reactivity` | 1566 | 21 % |
| `animation` | 1291 | 18 % |
| `sanity` | 1099 | 15 % |
| core unit tests | 1065 | 14 % |
| `distinctness` | 299 | 4 % |
| `reaction_diffusion_contract` | 216 | 3 % |

The first three are **54 % together**, and they are the three that iterate the shipped preset library
one preset at a time ([ADR-0157](adrs/0157-the-preset-sweeps-split-per-preset-and-the-phase-tier-samples-a-declared-representative.md)
split them per preset so a phase tier could sample them). Splitting per preset is what makes the
per-preset cost visible, and it is also what multiplies the fixed cost: each testcase stands up its
own adapter, device and pipeline set. **The cost therefore grows with the library**, and the library is
the thing this project exists to grow.

Shapes, none decided:

- **One renderer per binary, not per testcase.** A `OnceLock`-held context the per-preset cases share,
  so the adapter and pipeline build is paid once per binary rather than once per preset. The question
  is whether a shared device leaks state between cases in a way a golden would catch — and whether
  that is a property a test can assert rather than a hope.
- **Sample at the gate, cover whole nightly.** The gate runs ADR-0157's declared representatives; a
  scheduled run covers the library entire. Cheapest to build, and it moves a class of failure from
  "before the merge" to "the next morning", which is the trade to argue.
- **Retire the split.** Go back to one testcase per suite iterating the library in-process. Undoes
  ADR-0157's sampling, which the phase tier now depends on.

Note the interaction: **the cheaper this gets, the less ADR-0211 buys**, because ADR-0211's saving is
the difference between `-P fast` and the full suite. They are not additive and the second one to land
should be re-measured rather than assumed.

- **Raised:** 2026-09-16, when ADR-0211 was written and routed this half out of Plan 0191.
  **Owner if taken:** `architect` (a shared device is a testing-contract question), then `dev`.
- **Verified 2026-09-16** — the three suites exist as separate per-preset binaries:
  `present: reactivity in: .config/nextest.toml`
- **Verified 2026-09-16** — and ADR-0211 did not touch what a test costs:
  `absent: OnceLock in: tools/conductor/lib/gate.mjs`

### Priority

**Medium.** It is the larger number of the two halves of backlog 0227, but ADR-0211 takes the
cheap part of that cost first, and this half needs a measurement and a testing-contract decision
before it needs code.

## 0240 — a merged plan never leaves `queue.json`, and the exemption that tolerates it is keyed on a gitignored file

`queue.json` is accumulate-only. Nothing in the conductor removes a plan from a lane list, and no
gate asks: a merged plan stays listed forever and is carried by a **special case in the validator**.
`lib/queue.mjs` says so in its own doc comment — *"they sit under done/ and stay listed in the
committed queue, so they are accepted there rather than rejected as closed — otherwise every run
after the first merge would refuse to start."*

Two mechanisms keep that harmless, and they do **not** agree about what they read:

- `lane.mjs`'s `merged(ctx, plan)` skips a merged plan when picking the next one, and asks two
  sources: `ctx.state.plans[plan]?.status === "merged"` **or** `findPlan(ctx.repo, plan)?.done ===
  true`. The second needs no state — the file being under `done/` is enough.
- `validateQueue` runs **first**, and has only the `mergedPlans` set the caller passes, which
  `stateSets` builds out of `state/conductor.json`. It has no `done/` fallback of its own.

`tools/conductor/state/` is gitignored (`.gitignore:79`), so it never travels. A clone, a wiped
`state/`, or a second machine therefore turns every merged plan still listed into a **fatal**
preflight error, and the run refuses to start at all rather than skipping them.

Demonstrated against this repository, `0190` being closed and `0191` approved:

```
q = { lanes: { a: ['0190', '0191'] } }
validateQueue(q, repo, new Set(['0190']), new Set(['0190']))   -> errors: []
validateQueue(q, repo, new Set(),         new Set())           -> errors:
  ["plan 0190: already closed (0190-...-watching.md is under docs/plans/done/)"]
```

The queue is portable only as long as a gitignored directory travels with it, which it never does.
Today this is latent — the five plans merged before 2026-09-16 were pruned by hand when both lanes
were filled, so the committed queue happens to hold no merged plan at all.

Shapes, none decided, and the tension is real:

- **Give `validateQueue` the fallback `merged()` already has.** One condition: a plan under `done/`
  is skipped rather than fatal, whatever the state says. Makes the queue stand alone. **It also
  loses a check** — queueing a plan that was closed by hand is a typo, and that error is what
  catches it.
- **Prune at the close, as a ceremony step.** No code; the architect drops merged plans from the
  lane list beside the roster refresh. Keeps the typo check. **This project's own record argues
  against it**: the backlog-archive step went undone through three sweeps (2026-08-04, 2026-08-13
  and again hours later) until a carrier existed for it, which is the same shape.
- **A `conductor.mjs prune` command** that rewrites `queue.json` from the state, so the discipline
  has a carrier without the validator losing its check. More code than either half above.

- **Raised:** 2026-09-16, by the owner asking when a plan leaves the queue, during the first
  two-lane run. **Owner if taken:** `architect` (whether the check is worth keeping), then `dev`.
- **Verified 2026-09-16** — the fatal path is in the validator:
  `present: already closed \(\$\{found\.file\} in: tools/conductor/lib/queue.mjs`
- **Verified 2026-09-16** — and the tolerant predicate the validator does not share is in the lane:
  `present: findPlan\(ctx\.repo, plan\)\?\.done === true in: tools/conductor/lib/lane.mjs`
- **Verified 2026-09-16** — the state the exemption rests on does not travel:
  `present: tools/conductor/state/ in: .gitignore`

### Priority

**Low.** It costs nothing while `state/` is intact, and the failure is loud rather than silent: the
preflight names the plan and refuses, which is recoverable in one edit. It is filed because the
reason it has never fired is that the queue was pruned by hand twice, and nothing records that as
something anyone must keep doing.

## 0241 — the session allowlist is asserted against a model of the CLI's matcher, and the first unattended run falsified the model on a case it asserts

`test/settings.test.mjs` decides every case through a `decide()` it implements itself, and says so in
its own header: *"`decide` below models the CLI's documented rule matching … It is a model of the
CLI, not the CLI: it pins what this file means, and a CLI that changed its matcher would not turn it
red."* One clause of that model is that **a compound command is split at `&&`, `||`, `;`, `|`, `&`
and newlines and every part must be allowed on its own**, from which two cases follow:

```js
{ tool: "PowerShell", command: "cd studio; npm run typecheck", allowed: false, why: "`cd` is not a command a session may run" },
{ tool: "Bash",       command: "cd studio && npm run typecheck", allowed: false, why: "same, in the other shell" },
```

**Step `0191-01-implement`, 2026-09-16, falsified that.** Under `settings.conductor.json` exactly as
the conductor passes it, on CLI 2.1.273, these **ran**:

```
RAN     cd "C:/.../rlx-plan-0191" && node --test tools/conductor/test/ledger.test.mjs
RAN     cd "C:/.../rlx-plan-0191" && git status --porcelain
RAN     cd "C:/.../rlx-plan-0191" && rm argvcheck.cjs && git status --porcelain
DENIED  cd /tmp && printf "..." > argvcheck.cjs && node argvcheck.cjs -P fast
DENIED  cd "C:/.../rlx-plan-0191" && node --test ... | sed -n '/not ok 16/,/^  \.\.\./p' | head -40
```

26 of that step's 36 shell calls carried a `cd`, and **4 were denied**. Under the model all 26 should
have been: `cd` matches no allow rule. The CLI is evidently doing something the model does not
describe — the denials correlate with `/tmp` (a path outside the lane) and with `sed` / `head` (verbs
on no rule), not with `cd` itself. **What the real rule is has not been established**, and this entry
does not guess.

Two consequences, and the second is the one that matters:

- **Plan 0190 Phase 2's prompt half is not working, and now we know why.** `prompts/implement.md` says
  *"Shell calls run one command per call … No `cd`"*. The session ignored it 26 times in one step
  because ignoring it **works** — nothing teaches otherwise. Compound rates across the 15 recorded
  sessions run 12 % (`0182-01`) to 89 % (`0177-04`), and 0191's 81 % sits inside that spread rather
  than below it, so the rule moved nothing measurable. One post-rule session is not a trend; it is
  enough to say the rule is not self-enforcing.
- **The same model backs the safety claims.** `README.md`'s *"a path that leaves the lane is refused,
  whatever it is for"*, and every `rm`/`Remove-Item` deny case, are asserted through `decide()` and
  not against the CLI. A model already known to be wrong about one compound shape is a weak floor
  under *"a deletion whose path leaves the worktree is refused"*. This is a different gap from
  backlog 0237, which is about shell **expansion**; this one is about whether
  the matcher splits compounds at all. **Neither is measured.**

Shapes, none decided:

- **Probe the matcher.** `spike/probe.mjs` already runs sessions under `settings.conductor.json` and
  records what the CLI did; a session that attempts a fixed list of compound and escaping shapes
  turns `decide()` from a model into a transcript. This is the one that answers the question rather
  than working around it, and it is the same evidence discipline ADR-0208 applies to the version
  table.
- **Assert the deny half only, and assert it hard.** Keep `decide()` for the allow cases, where being
  wrong costs a refused command, and move every negative case behind a probe, where being wrong costs
  a deleted directory.
- **Stop saying it in the prompt and let the allowlist be the whole contract.** If `cd <lane> && …`
  is in fact safe, the prompt rule is noise the session correctly ignores; drop it and keep the
  bound where it is enforced.

- **Raised:** 2026-09-16, from the first unattended two-lane run, by reading `0191-01-implement`'s
  transcript after the phase landed. **Owner if taken:** `architect` (what the negative cases must be
  asserted against), then `dev`.
- **Verified 2026-09-16** — the model asserts the shape production ran:
  `present: cd studio && npm run typecheck in: tools/conductor/test/settings.test.mjs`
- **Verified 2026-09-16** — and says of itself that it is not the CLI:
  `present: It is a model of the CLI, not in: tools/conductor/test/settings.test.mjs`
- **Verified 2026-09-16** — the prompt rule the sessions do not follow:
  `present: one command per call in: tools/conductor/prompts/implement.md`

### Priority

**Medium**, and it would be Low but for the second consequence. Nothing has gone wrong: the denials
cost turns, the sessions recovered, all three phases of 0191 committed. It is filed because the
negative cases are the ones worth being right about, and the run just demonstrated that the thing
asserting them can be wrong about a case it states outright.

## 0242 — the conductor's gate skips a check whose precondition a lane never has, and says nothing, while the pre-push hook doing the identical skip prints a notice

`defaultGate()` guards the three studio steps on a path:

```js
{ name: "studio typecheck", cmd: ["npm", "--prefix", "studio", "run", "typecheck"], onlyIf: "studio/node_modules" },
{ name: "studio lint",      cmd: [...], onlyIf: "studio/node_modules" },
{ name: "studio test",      cmd: [...], onlyIf: "studio/node_modules" },
```

`studio/node_modules/` is gitignored (`.gitignore:115`), so **`git worktree add` never creates it and
no lane ever has one.** Verified on the live `plan-0191` lane on 2026-09-16: the directory does not
exist. Every conductor lane therefore runs the gate with all three studio checks skipped, always.

**And the skip leaves no trace.** `runGate` drops the step with a bare `continue` before anything is
recorded — it is not in `ran`, not in `timed`, and reaches neither `onCommandSkipped` nor the run
terminal. The only signal is the step count in `gate <stage> checks ok (16, 1m41s)`, and a reader
would need to know that 19 was the number to expect.

**The repository already solved this one layer up.** `.githooks/pre-push:209-214` guards on the same
directory and, when it is absent, prints:

```
pre-push: skipping the studio's typecheck, lint and tests (no studio/node_modules; run: npm --prefix studio ci)
```

That is [ADR-0016](adrs/0016-gpu-tests-opt-in-ci-scope.md)'s shape — skip, but say so, and name the
command that would un-skip it. The gate does the same skip and does not.

**Why it matters now rather than in the abstract.**
[Plan 0179](plans/done/0179-a-parameters-range-belongs-to-its-family.md) is next in lane `b` after 0191,
and its Phase 5 is the first `studio-builder` phase ever handed to the conductor. As things stand it
would run four `dev` phases, hand Phase 5 a session that edits `studio/`, and gate that work with
**zero** studio checks — typecheck, lint and test all skipped, silently, at both `pre-review` and
`post-close`. CI has a studio job and would convict after the push, which is the position the
conductor exists to get ahead of.

`onlyIfCommand` is the same class: the sd-filter step skips when `python3` is not on PATH, equally
silently. It happens to run on this machine (3.11.9), so only the studio half is live today.

Shapes, none decided:

- **Report the skip, as the hook does.** The cheapest, and it makes the hole visible without deciding
  anything: one `onCommandSkipped`-style line naming the step and the command that would enable it.
  It does not make the check run.
- **Install the lane's dependencies when it opens.** `openLane` runs `npm --prefix <lane>/studio ci`
  when `studio/` is in the plan's declared files. Makes the check real; costs an install per lane and
  puts an npm dependency inside worktree creation.
- **Make it a precondition rather than a guard.** A plan whose phases touch `studio/` fails the
  preflight unless its lane can run the studio checks — the same shape as the `.claude/` park
  ([ADR-0210](adrs/0210-a-claude-repair-is-the-owners-and-a-session-that-needs-one-parks-with-the-edit.md)),
  refusing before the work rather than skipping after it.

- **Raised:** 2026-09-16, reading what lane `b` would do after 0191 closes. **Owner if taken:**
  `architect` (whether a missing precondition is a skip or a refusal), then `dev`.
- **Verified 2026-09-16** — the gate guards the studio steps on a gitignored path:
  `present: onlyIf: "studio/node_modules" in: tools/conductor/lib/gate.mjs`
- **Verified 2026-09-16** — and drops the step before anything records it:
  `present: if \(c\.onlyIf && !existsSync\(join\(cwd, c\.onlyIf\)\)\) continue; in: tools/conductor/lib/gate.mjs`
- **Verified 2026-09-16** — while the hook doing the identical skip announces it:
  `present: skipping the studio's typecheck, lint and tests in: .githooks/pre-push`
- **Verified 2026-09-16** — the precondition a lane cannot inherit:
  `present: studio/node_modules/ in: .gitignore`

### Priority

**Medium-high, and time-bounded.** It is inert for a plan that does not touch `studio/`, which is
every plan the conductor has run so far. 0179 is the first that does, and it is next in the queue —
so the cheap mitigation (`npm --prefix <lane>/studio ci` once the lane opens) is worth doing by hand
before this is decided properly.

## 0243 — the served version-line rule is anchored to a column and a basename, not to the workspace section, so a dependency table edited in place would serve a code change to `-P fast`

[ADR-0211](adrs/0211-a-green-suite-record-serves-a-later-tree-when-no-deferred-suite-can-read-the-diff.md)
serves `Cargo.toml` and `Cargo.lock` forward *"only when their diff touches nothing but the version
line — the shape a `cargo release` bump produces, and no more."* `lib/ledger.mjs` implements that as
two anchors, and neither is the workspace section:

```js
const VERSION_LINE = /^[+-]version = "\d+\.\d+\.\d+"$/;       // column 0, any section
...
if (SERVED_PATHS.versionLineOnly.includes(basename(path)))     // any Cargo.toml in the tree
  return versionLineOnly(path, cwd, a, b);
```

`versionLineOnly` diffs at `--unified=0` and serves when **every** `+`/`-` line matches. So a
dependency written in table form and edited **in place**:

```toml
[workspace.dependencies.wgpu]
version = "27.0.1"        # -> "27.1.0", the only changed line
```

produces a diff whose every line matches, the tree is served, and the nine deferred GPU suites do not
run on what is a dependency change. The `basename` match widens it past the root file: **all six**
`Cargo.toml` in this workspace are under the rule, not only `[workspace.package]`'s.

**Latent today, and the entry says exactly why rather than leaving it to trust.** Verified
2026-09-16: of the six tracked `Cargo.toml`, only the root carries a column-0 three-part
`version = "…"` — `Cargo.toml:23`, the workspace version itself. Every member uses
`version.workspace = true`, and no file uses the `[*.dependencies.*]` table form. Two further
accidents stand between this and a real miss, and **neither is a guard**: a registry bump also
rewrites `Cargo.lock`, whose `checksum = "…"` line matches nothing and re-arms the suite; and a
two-part requirement (`version = "0.20"`) is refused by the three-part pattern, which is deliberate
and commented.

**Where it came from.** Raised as the one `minor` of
[Plan 0191](plans/done/0191-a-green-tree-is-not-tested-four-times.md)'s conductor close review, and
**left open there correctly** — [ADR-0209](adrs/0209-a-conductor-close-repairs-the-prose-and-comments-its-findings-name.md)
lets a close repair prose and comments, and this repair is code. It is filed here because an open
finding that lives only inside a closed plan's `## Close review` section has no carrier: nothing
re-reads it, and no probe re-checks it.

Shapes, none decided:

- **Anchor to the section.** Track the preceding `[...]` header while walking the hunk and serve only
  a `version` line under `[workspace.package]`. Exact, and it makes the rule say what ADR-0211's
  prose already says.
- **Anchor to the file and the line number.** Serve only the root `Cargo.toml`, and only the line the
  workspace version is on. Cruder, and it breaks the day the file is re-ordered.
- **Stop serving `Cargo.toml` at all.** A version bump then always re-arms the full suite, which
  costs one suite per close — the exact cost ADR-0211 exists to remove, so this is the shape that
  trades the feature away and is listed to be argued against rather than adopted quietly.

- **Raised:** 2026-09-16, at Plan 0191's close, by the review; re-read and widened the same day
  (the `basename` half is not in the review's text). **Owner if taken:** `dev`.
- **Verified 2026-09-16** — the pattern is anchored to a column, not a section:
  `present: const VERSION_LINE in: tools/conductor/lib/ledger.mjs`
- **Verified 2026-09-16** — and the file test is a basename, so every crate's manifest is in scope:
  `present: versionLineOnly\.includes\(basename\(path\)\) in: tools/conductor/lib/ledger.mjs`
- **Verified 2026-09-16** — latency rests on no manifest using the table form, which is a fact about
  today's tree and not a rule; this probe goes red the day one does:
  `absent: ^\[workspace\.dependencies\. in: Cargo.toml`

### Priority

**Low, and it should stay Low only while the probe above is green.** Nothing can reach it in this
tree, the failure is a weaker gate rather than a wrong result, and `-P fast` still runs. It matters
the first time anyone writes a dependency as a table — a normal thing to do when pinning a git
source or adding `features` — because the rule would then quietly stop testing dependency changes on
the GPU suites, and nothing would report it.

## 0244 — a custom wave is drawn from the source's traces but is neither smoothed nor scaled like the eight built-in figures

Plan 0180 Phase 6 rebuilt the eight `wave_mode` figures as `CPlugin::DrawWave` builds them: each
passes through `SmoothWave` (`milkdropfs.cpp` l.2549-2577, applied at l.3319-3335) and each carries
`HOST_SAMPLE_FACTOR` on its sample term ([ADR-0199](adrs/0199-a-converted-waveform-draws-the-sources-figure-at-the-hosts-scale.md)
clause 2). **A custom wave gets neither.**

It reads the same smoothed, `wave_scale`d traces the built-in figures read — `custom_waves` in
`core/src/render/scenes/warp_mesh/draw.rs` hands `value1`/`value2` from the analyzer's levelled pair —
so the two channels are right since that plan. What it does not get is the midpoint insertion or the
host factor, and the source applies `SmoothWave` to a custom wave too, unless it draws dots
(l.2722).

That was **deliberate and correctly scoped**: backlog 0216's figure contract covered the eight
built-in modes, and Plan 0180 says so in Phase 6's notes and again in its close block. So this is not
a defect found; it is the question that scope left standing, raised so it is not lost.

**What a decision needs.** Whether the figure contract reaches custom waves is a real question rather
than an oversight, because the two cases differ: a built-in figure is the source's construction and a
custom wave is the preset author's own per-point program, which may already place its points where it
wants them. Smoothing it inserts points the program did not compute. The host factor is the weaker
half of the question — it is a property of how a sample reaches the draw, which a custom wave shares.

- **Raised:** 2026-09-16, at Plan 0180's close, by the plan's own **Not done here** section.
  **Owner if taken:** `architect` to decide the contract, then `dev`.
- **Verified 2026-09-16** — the host factor is applied in the built-in figure's sample readers and
  nowhere else:
  `present: HOST_SAMPLE_FACTOR in: core/src/render/scenes/warp_mesh/draw.rs`
- **Verified 2026-09-16** — `custom_waves` passes the pair straight through with no factor and no
  smoothing pass:
  `present: let value1 = left\.get\(at\)\.copied\(\)\.unwrap_or\(0\.0\); in: core/src/render/scenes/warp_mesh/draw.rs`

### Priority

**Low.** Nothing is wrong on screen: a custom wave draws where its program puts it, which is a
defensible reading of the reference either way. It matters only when someone compares a converted
preset's custom wave against `foo_vis_milk2` side by side and finds it thinner or smaller — which is
[Plan 0142](plans/done/0142-the-milkdrop-import-earns-its-verdict.md) Phase 4's session, and the reason to
have this written down before that session runs.

## 0245 — the converted warp space has no pixel baseline, because every golden fixture is square and the two chains are identical there

[ADR-0212](adrs/0212-a-converted-preset-gets-its-own-vertex-module-and-the-pipeline-is-chosen-not-branched.md)
gives a converted preset its own vertex module, and Plan 0180 Phase 3 landed the arithmetic: four
warp stages run in MilkDrop's aspect-corrected space, each differing from the native chain by `1/A`
on the shorter axis — **1.78 on y at 16:9**. The plan's Risks section expected "a larger re-bless than
its title suggests".

**Nothing was re-blessed. No baseline moved at all.** `core/tests/golden.rs` renders every fixture at
`SIZE = 128` on both axes, and at a square target MilkDrop's aspect pair is `(1, 1)`, so both chains
are the identity — the same agreement `at_a_square_target_the_two_warp_chains_agree` asserts on
purpose. So `warp_mesh_milk.png`, `warp_mesh_shader.png` and `warp_mesh_stroke.png` are structurally
incapable of guarding the corrected-space chain.

This is a **drift-guard gap, not missing coverage.** The behaviour is asserted well: four per-stage
tests run at 128x72 and 128x96 against a converted scene and a native one, mutation-checked, plus the
square-target agreement test. What no baseline can catch is incidental pixel drift in the converted
chain — the class a golden exists for, and the class ADR-0037 was written about after it shipped
twice invisible at 16:9.

Shapes, none decided:

- **One converted fixture at a non-square size**, the way `attractor_trails` is captured at 160x100 in
  its own module with its own baseline. Smallest change; adds one baseline and one bless scope.
- **A second size for the whole golden roster.** Catches the same class everywhere rather than in one
  fixture, at the cost of doubling the roster and every bless.
- **Nothing, and say so.** The per-stage tests may simply be better evidence than a baseline, in which
  case this entry closes with that reasoning recorded rather than left implicit.

- **Raised:** 2026-09-16, at Plan 0180's close, by the review. **Owner if taken:** `architect` then
  `dev`.
- **Verified 2026-09-16** — every golden fixture is square:
  `present: const SIZE: u32 = 128; in: core/tests/golden.rs`
- **Verified 2026-09-16** — the converted chain exists and is selected per preset:
  `present: warp_pipeline_converted in: core/src/render/scenes/warp_mesh/resources.rs`
- **Verified 2026-09-16** — the non-square probes that are the only coverage:
  `present: const SPACE_TARGETS: \[\(u32, u32\); 2\] = \[\(128, 72\), \(128, 96\)\]; in: core/src/render/scenes/warp_mesh/tests.rs`

### Priority

**Low.** The arithmetic is asserted directly and at two aspects, so a regression in what the chain
computes is caught. The exposure is narrow: an incidental pixel change in a converted preset at a
non-square target, which today means a test fixture and no shipped preset — `presets/` carries no
`[milk]` bundle. It rises the day one ships.


## 0246 — the local `cargo doc` mirror covers one crate of five, and the four it leaves out are still unreachable until after a push

[Backlog 0179](design-backlog-archive.md) established that `cargo doc` was the one CI gate no local
step mirrored, and [Plan 0177](plans/done/0177-the-test-tree-stops-costing-disk-and-touching-the-machine.md)
Phase 7 closed it: `.githooks/pre-push` now runs `cargo doc -p rlx-core --no-deps --features text`
under `RUSTDOCFLAGS=-D warnings`, after clippy, for a measured 4.5-5.9 s. **That entry is closed and
this one does not reopen it** — the step it asked for exists and works.

What is left is the **scope**. The workspace has five members and the hook documents one of them, so
`core-cabi`, `rlx-ring`, `milkconv` and `standalone` — each a `lib` target with its own public
surface — are documented in CI and nowhere else. A rustdoc error in any of the four is still
**structurally unreachable** before a push, which is 0179's own finding surviving its own repair at
four-fifths strength.

**It has already fired, and this is the second instance of the class.** At Plan 0180's close,
`cargo doc --workspace` was red on `main`: `milkconv/src/convert.rs`'s module header made the private
`deposit_block` an intra-doc link, which is an error for a public item under `-D warnings`. Repaired
in `262a4c03`, after a `chore: Release` tag had already been written on top of the red. The first
instance was Plan 0137 and is what raised 0179. Both shipped a red `main` under a release tag; the
trigger in both is a **visibility or link change in a crate no local step documents**, not a doc edit.

**The gap was written down at the moment it was created, in the one place a reader of this file
would never look.** `.github/workflows/ci.yml` carries it in a comment beside the job — *"What it
does NOT cover is `standalone`'s public surface (and `milkconv`'s and `rlx-core-cabi`'s); this job is
the only place those are documented"* — and `.githooks/pre-push` says the same in fewer words. Both
were accurate when written and neither is a backlog entry, so the residual had no name. That is why
four documents now cite **backlog 0179** — a closed entry — as the home of a live gap
(`.claude/skills/architect/SKILL.md` twice, `docs/plans/README-archive.md` twice, and `262a4c03`'s
own commit message, which cannot be corrected). Note also that the `ci.yml` comment names **three**
uncovered crates and the answer is four: `rlx-ring` is missing from it.

Shapes, none decided:

- **Widen the hook step to `--workspace`.** One word. Measured once on the development machine at
  `0233a119`, warm, on an already-built tree: `RUSTDOCFLAGS="-D warnings" cargo doc --workspace
  --no-deps` finished in **10.29 s**, green — against the scoped step's 4.5-5.9 s and the clippy step's
  6.3-7.8 s from Plan 0177 Phase 7. **That is one warm run on one machine and not a budget ruling**;
  ADR-0033's ~28 s hook budget is what a decision has to weigh it against, and a cold or
  dependency-touching run was not measured. One simplification comes free: `--features text` is
  needed only to document `rlx-core` alone, because `standalone/Cargo.toml:46` turns that feature on
  and `--workspace` unifies it.
- **Name the four crates instead of the workspace,** if the measurement above turns out to have been
  flattered by the warm tree. Costs the same words to maintain as the comment already does.
- **Leave the scope and move the class's home.** The hook's budget is real and ADR-0033 is entitled
  to win; then the residual still needs a name that is not a closed entry, and the close ceremony's
  own `cargo doc --workspace` line (Mode 4 lens 1) becomes the only carrier.

- **Raised:** 2026-09-17, after Plan 0180's close, by `architect`. **Owner if taken:** `architect`
  then `dev`.
- **Verified 2026-09-17** — the hook's doc step is scoped to one crate:
  `present: run_step "cargo doc -p rlx-core --no-deps --features text in: .githooks/pre-push`
- **Verified 2026-09-17** — and no step in it documents the workspace (the bare string appears in the
  hook's prose, which is why this probe reduces to the invocation):
  `absent: run_step "cargo doc --workspace in: .githooks/pre-push`
- **Verified 2026-09-17** — CI is the only place the other four are documented:
  `present: this job is the only place those are documented in: .github/workflows/ci.yml`
- **Verified 2026-09-17** — five members, one of them covered locally:
  `present: members = \["core", "core-cabi", "rlx-ring", "milkconv", "standalone"\] in: Cargo.toml`
- **Verified 2026-09-17** — the feature the scoped step needs and `--workspace` would not:
  `present: rlx-core = \{ path = "\.\./core", features = \["text"\] \} in: standalone/Cargo.toml`

### Priority

**Medium.** It has shipped a red `main` under a release tag twice in sixteen days, and the cost each
time was a close ceremony discovering after the fact what a hook step could have said before. It is
not higher because the blast radius stops at CI — no user-visible behaviour is involved — and the
repair may be a single word, which is also the reason it should not sit here long.

## 0247 — a suite run by hand inside a lane records into that lane's own ledger, which is the one place no gate reads

[ADR-0207](adrs/0207-a-suite-run-the-conductor-observed-green-is-not-run-again-on-the-same-tree.md)
gave the operator an affordance and
`tools/conductor/README.md` states it: *"A suite you run by hand counts. Run one through the wrapper
— `node tools/conductor/with-lock.mjs suite -- cargo nextest run --workspace` — and, because the
wrapper can see it is running inside this repository or one of its worktrees, it records the result
in `state/suite-ledger.jsonl` as `hand`. The conductor's next gate on that same tree finds the
record, prints it and does not run the suite again."*

**The last sentence does not hold when the hand run happens in a lane, and a lane is where the
operator is.** `suiteLedger()` resolves the destination as `join(selfDir, "state",
"suite-ledger.jsonl")` — beside *the copy of the script that was invoked*. `tools/conductor/state/`
is gitignored, so every worktree has its own, and `node tools/conductor/with-lock.mjs` run from
`WORK/rlx-plan-NNNN` writes that worktree's file. The guard in front of it compares
`gitCommonDir(cwd)` with `gitCommonDir(selfDir)`, which is the **same directory** for a checkout and
all its worktrees — so a lane run passes the "inside this repository" test exactly as the comment
above it intends (*"the main checkout or any of its worktrees"*), records itself `hand`, and lands
where the conductor will never look. The conductor runs from the main checkout and reads the ledger
beside *its* copy.

**Observed 2026-09-17**, settling Plan 0178's `claude_dir` park. The wrapped
`cargo nextest run --workspace` in `WORK/rlx-plan-0178` finished green — 1985 passed, 6 skipped, 640 s
— and wrote `{"tree":"e6634cd2…","by":"hand",…}` into the lane's `state/suite-ledger.jsonl`. The main
checkout's ledger has no line for that date at all, so the pre-review gate on that same tree will run
the full suite a second time.

**The cost is redundant work, never a wrong answer**, and that bound is worth stating: nothing reads
a lane's ledger, so no run can be skipped on the strength of a record the gate cannot see. A second
hand run in the same lane on the same tree does skip, correctly, off the lane's own file.

**What makes it worth an entry rather than a note is where it fires.** The park table sends the
operator *into the lane* for every reason it lists — `human_phase`, `claude_dir`, `gate_red`,
`review_failed` — so the affordance works in the checkout where a repair does not happen and fails in
the worktree where it does. Nothing is red: `test/with-lock.test.mjs` asserts the `hand` case only
with `selfDir` and `cwd` in the same tree, which is the case that works.

Shapes, none decided:

- **Resolve the ledger through the common dir.** One file per repository rather than per worktree:
  derive the main checkout from `git rev-parse --git-common-dir` and write beside it. Needs a rule for
  a bare or relocated `.git`, and it changes where a hand run in the main checkout writes (nowhere, if
  the derivation is right — it is already that file).
- **Have the gate read the lane's ledger too** when it gates that lane's tree. Leaves the resolution
  alone and puts the knowledge in one place, at the cost of a second lookup path and a second file
  that can disagree.
- **Sweep on `resume`.** The conductor already reads a lane at resume; folding its ledger into the
  main one there is small and touches nothing that runs during a step.
- **Document the override and stop promising.** `RLX_SUITE_LEDGER` already wins over the resolution,
  so the operator can point a hand run at the main checkout's file. Weakest: it is a per-run manual
  step, and the README sentence would have to be narrowed to the main checkout, which is the half of
  the affordance nobody needs.

- **Raised:** 2026-09-17, from the Plan 0178 Phase 4 park repair, by the owner's session.
  **Owner if taken:** `dev`.
- **Verified 2026-09-17** — the README makes the promise:
  `present: A suite you run by hand counts in: tools/conductor/README.md`
- **Verified 2026-09-17** — the destination is beside the invoked script:
  `present: join\(selfDir, "state", "suite-ledger\.jsonl"\) in: tools/conductor/with-lock.mjs`
- **Verified 2026-09-17** — and the comment above it means to cover a worktree:
  `present: the main checkout or any of its worktrees in: tools/conductor/with-lock.mjs`
- **Verified 2026-09-17** — `state/` is gitignored, so each worktree carries its own:
  `present: tools/conductor/state/ in: .gitignore`
- **Verified 2026-09-17** — the test's `hand` case shares one directory, so this is unasserted rather
  than asserted-and-broken:
  `present: suiteLedger\(s\.repo, \{\}, s\.selfDir\) in: tools/conductor/test/with-lock.test.mjs`

### Priority

**Low.** It costs one redundant full suite — about 11 minutes — per lane the operator repairs by
hand, and it cannot produce a wrong skip. It is not lower because the repair is small and the
affordance is documented as working, which is the shape that wastes someone's afternoon before they
think to check the file it wrote.

---

## 0248 — nothing in this repo asks whether a groundless luminous field is a composition or a fill, and four shipped presets are the open cases

The surviving half of [backlog 0128](design-backlog-archive.md), carved out by
[Plan 0186](plans/done/0186-the-flatness-gate-tells-a-figure-from-its-ground.md)'s
`## What this plan does NOT do` and refiled here at its close, because the archive is append-only
and closed. Read the archived body for the full history; this is the question it left standing.

`Sumi`, `Whorl`, `Supernova` and `Neon Tunnel` are `fragment_field` presets that fill the frame with
luminous tone and have no ground worth the name. The `sanity` harness marks them in its printed
table and has done since Plan 0116 — the `NOTE` line above the candidate rows names all four — but
**no statistic in this repository decides between the two readings**:

- a **composition** that happens to cover the frame, which is legitimate content and must pass; or
- a **fill**, a wash with no figure in it, which is the defect `sanity` exists to catch and which
  today's conjunction cannot reach because term one clears it.

Their readings at the suite's 96x96 capture, 2026-09-17, after ADR-0200:

| preset | `coverage` | `tonal_flatness` (max 0.90) | term two | `role_ratio` (cut 1.17) |
|---|---|---|---|---|
| `Sumi` | 0.9253 | 0.2085 | 0.1001 | 1.0807 → ground |
| `Whorl` | 0.9504 | 0.2552 | 0.1182 | 1.0522 → ground |
| `Supernova` | 0.9934 | 0.4241 | 0.0644 | 1.0067 → ground |
| `Neon Tunnel` | 0.9969 | 0.1690 | 0.0540 | 1.0032 → ground |

All four are under the `0.23` default boundary floor and all four are far under the flatness
ceiling, so each is among the 62 of 112 presets held out of conviction by term one alone. **Nothing
here says whether that is correct.** The tonal term reports plenty of tonal structure, which is true
of a beautiful wash and of a broken one alike; `coverage`, `quadrant_spread` and
`radial_shell_occupancy` are all near-degenerate at this density, which is the failure the archived
0128 body already diagnosed for the light-ground case.

ADR-0200 did not touch this and could not: the role classifier puts all four on the *ground* side,
which is the correct call for a luminous field and leaves term two reading their ink — where they
genuinely have little perimeter. The missing instrument is a third question, not a different
reference for the second.

- **Raised:** 2026-09-17, at Plan 0186's close, carried over from backlog 0128's 2026-09-02 re-open.
  **Owner if taken:** `architect` first — like its parent, this is a question about what the sanity
  lens *means* before it is a threshold.
- **Verified 2026-09-17** — the harness still marks the four and still asks nothing about them:
  `present: the four groundless luminous in: core/tests/sanity.rs`
- **Verified 2026-09-17** — the conjunction that cannot reach them:
  `present: flat > MAX_TONAL_FLATNESS && boundary < b_floor in: core/tests/sanity.rs`
- **Verified 2026-09-17** — and the question itself has no instrument, which is the absence of a
  mechanism rather than a fact about the tree:
  `unprobeable: whether Sumi, Whorl, Supernova and Neon Tunnel are compositions or fills is a
  question no statistic in this repo asks, so there is nothing to match on`

### Priority

**Low, and honestly so.** All four ship, none is suspected broken, and the cost of the gap is that
nothing would notice if one became a wash. It is not lower because it is the last live piece of a
diagnosis three ADRs and three plans have now worked on, and because the instrument it wants — a
statistic that reads a full frame's *internal* organization rather than its departure from a ground —
is the one shape this line has never tabled.

## 0249 — `warp_mesh`'s `zoom` doc says the opposite of what the shader does, and four generated surfaces carry it

The `ParamSpec` doc for `warp_mesh`'s `zoom` reads *"Scale the previous frame is resampled at, per
vertex; above 1 the image tunnels inward."* The shader says the reverse, in a comment written to
explain exactly this trap: *"The INVERSE of the motion the outputs name throughout: a destination
vertex asks where its content came from, so a `zoom` above 1 shrinks the source window and the past
appears to grow."* The plan's own fixture agrees with the shader — `warp_mesh_ladder.toml` uses
`1.04` to make content travel outward.

**The string is declared twice and rendered four times.** `mod.rs:116` (`PER_VERTEX_PARAMS`) and
`mod.rs:397` (`PARAMS`) carry identical copies, and ADR-0170's generation pipes them into
`presets/README.md`'s parameter table, `presets/schema/warp_mesh.schema.json` (twice — `description`
and `markdownDescription`) and `docs/specs/player-schema.json`. So an author who checks the reference
before binding the parameter is told the wrong direction by every surface this project offers.

**It has already produced a false claim in shipped content.** Plan 0184's `presets/warp_ladder.toml`
was authored with a header paragraph explaining that `zoom` below 1 makes the field creep outward —
written from the reference, not from the shader — and the close review of that plan convicted it
(minor 2) and traced it here (nit 6). The preset's prose is repaired; the source of it is not.

**The repair is small and is `dev`'s**, because it is an engine edit that moves three generated
artifacts: correct both declarations, then regenerate with `RLX_UPDATE_PARAM_REFERENCE=1` and
`RLX_UPDATE_PRESET_SCHEMA=1`. The review wrote the replacement text: *"Scale the previous frame is
resampled at, per vertex; above 1 the past is magnified and the image travels outward."* That is
outside what a conductor close may repair (ADR-0209), which is why it is here rather than fixed.

- **Raised:** 2026-09-17, from Plan 0184's close review, by the owner's session. **Owner if taken:**
  `dev`.
- **Verified 2026-09-17** — the declaration says inward:
  `present: above 1 the image tunnels inward in: core/src/render/scenes/warp_mesh/mod.rs`
- **Verified 2026-09-17** — and the shader beside it says the opposite:
  `present: above 1 shrinks the source in: core/src/render/scenes/warp_mesh/shaders.rs`
- **Verified 2026-09-17** — the generated parameter table carries the wrong one:
  `present: above 1 the image tunnels inward in: presets/README.md`
- **Verified 2026-09-17** — and so does the editor schema:
  `present: above 1 the image tunnels inward in: presets/schema/warp_mesh.schema.json`

### Priority

**Medium.** It is two lines of text and a regeneration, and it has already cost one shipped header a
false paragraph and a close-review finding. It is not higher because nothing it touches is executable:
every picture the engine draws is correct, and only the prose about it is wrong.

## 0250 — a conductor session cannot run a command that carries an environment assignment, and two documented repairs need one

`tools/conductor/settings.conductor.json` allows commands by **prefix** — `Bash(cargo *)`,
`Bash(node *)`, `Bash(npx *)` and so on. A command written `RLX_UPDATE_PRESET_SCHEMA=1 cargo nextest
run …` does not begin with `cargo`, so it matches no rule and is denied. The same holds for
`RLX_UPDATE_PARAM_REFERENCE=1`, and for any other `VAR=value cmd` form.

**Two repairs the project documents are therefore unreachable from inside the conductor**, and both
have now been met:

- **Regenerating a generated file to resolve a merge.** Plan 0184's close hit a conflict in
  `docs/specs/player-schema.json`, which both branches had rewritten whole. The session diagnosed it
  correctly, named the command `docs/developing.md` prescribes, and parked `merge_conflict` because it
  could not run it. The owner resolved it by hand in one step.
- **Any `RLX_UPDATE_*` regeneration a phase needs.** A phase that renames a parameter or moves a
  default must regenerate the reference and the schemas; the same denial applies.

**The park is honest and the lane is left clean, so this is a cost rather than a hazard** — but it is
a cost paid in a full review session each time, and the class will recur for as long as generated
files are committed.

Shapes, none decided:

- **Allow the two spellings by name.** `Bash(RLX_UPDATE_PRESET_SCHEMA=1 cargo *)` and the parameter
  one, which keeps the allowlist a list of things rather than a pattern. Narrow, and it needs a case
  in `test/settings.test.mjs` like every other rule.
- **Teach the repairs the `--config` form.** `cargo test --config 'env.RLX_UPDATE_PRESET_SCHEMA="1"'`
  begins with `cargo` and is already allowed; a session tried exactly this at Plan 0183 and it was
  denied for a different reason, so the form needs checking before it is documented.
- **Let the conductor resolve a generated-file conflict itself**, from a declared list of
  regenerate-don't-merge paths. The largest change, and the one that removes the class rather than
  the two instances.

- **Raised:** 2026-09-17, from Plan 0184's `merge_conflict` park, by the owner's session.
  **Owner if taken:** `dev`.
- **Verified 2026-09-17** — the allowlist admits `cargo` only as a prefix:
  `present: "Bash\(cargo \*\)" in: tools/conductor/settings.conductor.json`
- **Verified 2026-09-17** — and carries no environment-assignment rule of any kind:
  `absent: Bash\(RLX_ in: tools/conductor/settings.conductor.json`
- **Verified 2026-09-17** — while the documented regeneration is written in exactly that form:
  `present: RLX_UPDATE_PRESET_SCHEMA=1 cargo nextest in: docs/developing.md`

### Priority

**Low.** It costs one parked plan and one hand resolution per occurrence, it never produces a wrong
result, and the park names the command to run. It rises if a plan lands that regenerates a parameter
surface per phase, because then every phase meets it.

## 0251 — `warp_mesh`'s level mode draws bands but not an ink class, because coverage is a continuum nothing thresholds

[ADR-0197](adrs/0197-the-contour-can-be-an-ink-and-the-warp-field-can-be-coloured-by-its-level.md)
gave `warp_mesh` a second colour path — `color_source = "1"` deposits uncoloured light and the present
pass colours the field by its own accumulated level — and it does exactly what backlog 0146 asked for
as *structure*: the bands are the feedback loop's own decay contours, and nothing else in this engine
makes one. **What it does not produce is a limited-ink frame**, and that is a property of the last
line in the pass rather than of the coordinate.

The present writes `ink * coverage`. `ink` is one of the palette's own values, but coverage is the
field's alpha — a continuum — so every pixel short of full coverage is a fresh value the palette never
named. Measured on the shipped `presets/warp_ladder.toml` at 640x360 loud: **a two-ink palette renders
851 exact colours** with `palette_steps = "0"`. Quantizing recovers most of it, because quantizing the
palette *coordinate* quantizes the level the coverage is computed from as well — twelve bands bring the
same frame to **60** — and the preset ships at twelve for that reason, stating the count in its header
instead of claiming a class it does not have.

**Sixty is not two, and no switch in the engine gets there.** `palette_contour` does not help: it draws
at a *band* edge, so at `palette_steps = "0"` there is no edge and it is inert (851 colours with the key
at full strength and 851 without, measured both ways), and with the bands on it lands inside a black rung
on a duotone.

**The candidate the plan named, undecided.** Plan 0184 Phase 4's stop condition says that if the fringe
reads as shading rather than as the ladder dissolving, the verdict comes back here rather than being
tuned around — and it reads as shading. The obvious shape is a **coverage threshold in level mode**: a
parameter above which coverage snaps to 1 and below which it snaps to 0, so the frame holds only the
palette's inks and the paper. Everything about that is a design question and none of it is decided:

- **Whether the edge then aliases**, since it would be a hard alpha cutoff with no derivative behind it —
  the same trade the hard contour style already makes, but on a silhouette rather than a hairline.
- **Whether it belongs to `warp_mesh` or to ADR-0138's draw seam.** ADR-0138 defines the limited-ink
  guarantee *at the draw seam*, and this is a present-time composite; a threshold here may be a
  `warp_mesh` parameter or may be the general repair for every scene whose output is premultiplied light.
- **Whether a soft outer edge is worth keeping as the default.** `warp_ladder`'s header argues the fade
  reads as the ladder running out of ink at the edge of the sheet, which is a look rather than a defect.
  Two shipped worlds would want opposite defaults.

- **Raised:** 2026-09-17, from Plan 0184 Phase 4's look gate, routed by its own stop condition and
  recorded in ADR-0197's `Outcome`. **Owner if taken:** `architect` (an ADR) then `dev`.
- **Verified 2026-09-17** — the present pass multiplies the ink by a continuous coverage, with nothing
  between them:
  `present: ink \* clamp\(c\.a, 0\.0, 1\.0\) in: core/src/render/scenes/warp_mesh/shaders.rs`
- **Verified 2026-09-17** — and `warp_mesh` declares no parameter that touches coverage at all:
  `absent: coverage in: core/src/render/scenes/warp_mesh/mod.rs`
- **Verified 2026-09-17** — the shipped world states the measured count rather than an ink class:
  `present: 851 exact colours in: presets/warp_ladder.toml`
- **Verified 2026-09-17** — and the reader document says the same thing in its own words:
  `present: The fringe is not two-ink in: docs/preset-palettes.md`

### Priority

**Low.** Nothing is broken: the mechanism does what ADR-0197 decided, the world that wanted it ships,
and the residue is a class the frame does not join rather than a picture that is wrong. It rises if a
second author asks `warp_mesh` for a print, or if the same question arrives from another
premultiplied-light scene — at which point it is ADR-0138's boundary being asked to move, not this
scene's.

## 0252 — the conductor's gate is a hand-maintained copy of the Node gate list, and it has fallen behind twice

The pre-push hook and CI's `links` job are the two carriers every Node gate is wired into, and a
plan that adds one edits both. There is a **third** carrier nothing points a plan at:
`defaultGate()` in `tools/conductor/lib/gate.mjs`, the list the conductor runs at `pre-review` and
`post-close` for a plan it runs itself. It is a literal roster, maintained by hand, and it is
missing two gates: `check-translations.mjs` (Plan 0166) and `check-system-counts.mjs` (Plan 0178).

**What that costs is the whole point of running a gate before the close rather than after the
push.** For a conductor-run plan the first machine that executes either checker is CI, after the
owner pushes — which is the shape backlog 0246 already records for `cargo doc`, arriving here by a
different route. Neither plan did anything wrong: ADR-0202 scopes its gate to the hook and the
`links` job, Plan 0166 the same, and both wired it exactly there. The defect is that a third list
exists and no rule, gate or ceremony step names it.

**What a fix looks like, and it is a decision rather than an edit.** The obvious repair — add the
two names — restores the invariant for a day and rebuilds the same trap. The shapes worth weighing:

- **Derive the list from the hook.** `.githooks/pre-push` already runs every Node gate in order;
  a conductor gate that parsed its `run_step "node scripts/…"` lines could not fall behind, at the
  price of reading a shell script as data.
- **Derive both from one manifest** — a committed list of gates that the hook, the `links` job and
  `defaultGate()` all read. The largest change, and the only one that also covers CI.
- **Add a gate that asserts the three carriers agree.** The same substitution this repository has
  made three times (ADR-0116, ADR-0149, ADR-0202): when a convention keeps failing, it takes a
  checker. It is also the only option that catches the next one without changing how any of them
  runs.

- **Raised:** 2026-09-18, at [Plan 0178](plans/done/0178-what-the-operator-reads-is-true.md)'s close
  review, by `architect`. **Owner if taken:** `architect` then `dev`.
- **Verified 2026-09-18** — the conductor's gate does not run the count gate:
  `absent: check-system-counts in: tools/conductor/lib/gate.mjs`
- **Verified 2026-09-18** — nor the translation gate, which has been missing a plan longer:
  `absent: check-translations in: tools/conductor/lib/gate.mjs`
- **Verified 2026-09-18** — it does carry the gates either side of them in the hook's own order, so
  this is an omission rather than a scope decision:
  `present: check-reader-prose.mjs in: tools/conductor/lib/gate.mjs`
- **Verified 2026-09-18** — the two carriers a plan is told to edit both have the count gate:
  `present: node scripts/check-system-counts.mjs in: .githooks/pre-push`
- **Verified 2026-09-18** — and:
  `present: node scripts/check-system-counts.mjs in: .github/workflows/ci.yml`

### Priority

**Low-medium.** Nothing user-visible is involved and both gates do run — on every push, by CI, and
on every local push by anyone with the hook installed. What is lost is the pre-close reading for
conductor-run plans, which is exactly where a red gate is cheapest to repair. It rises with each
further gate added, because the gap is not one checker but a list that nobody is told to update.
