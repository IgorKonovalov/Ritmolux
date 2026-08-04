# ADR-0074 — A ratio against an in-run control is not automatically portable

> **Status:** **accepted** (2026-08-04, at Plan 0060's close) — with an **Outcome** section at the
> end, because the hardware measurement this ADR called for came back the same day and inverted one
> of the premises recorded below. The decision is unchanged; see Outcome for what is not.
> **Date:** 2026-08-04
> **Corrected 2026-08-04, same day, before acceptance.** This ADR was written believing the
> hardware its deferral depends on was not available on this side. **It is.** `dev` box, verified
> by running `a_dual_live_dissolve_carries_the_outgoing_trail` — the hardware-only sibling — with
> `--no-capture`: it **executes** (7.68 s of real GPU work) and prints no skip notice, so
> `Renderer::adapter_is_software()` is `false` here and every hardware-gated check in this suite has
> been runnable on this box all along. The **decision below is unchanged** — it rests on the 7.3x
> ratio spread, not on where a machine is — but the **disposition changes**: the magnitude claim is
> measurable *today*, not deferred indefinitely to a `human` phase waiting on a box. The two
> passages this affects are marked inline.
> **Related plan(s):** [0060](../plans/0060-a-test-number-states-a-property-or-names-its-machine.md)
> (Phase 3, which routed this decision here by its own instruction)
> **Supplements:** [0071](0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md)
> (whose "property" form this narrows, and one of whose Positive consequences it withdraws),
> [0016](0016-gpu-tests-opt-in-ci-scope.md) (the skip-with-notice shape the rejected fallback would
> have used), [0023](0023-golden-drift-guard-uses-frozen-fixtures.md) (`MEAN_TOL`, the declared
> noise floor).

## Context

[ADR-0071](0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md) says a numeric
assertion states a property or names its machine, and it offered the **ratio against a control
taken in the same run on the same adapter** as one of the three property forms — "so the machine
cancels". `dual_live_keeps_the_outgoing_side_animating` was rebuilt on that form in Plan 0060
Phase 1, deliberately without a magnitude floor, and Phase 2 pushed it to read the ratio off the
runners. Those numbers are now in, and they falsify the premise.

| statistic | local WARP (10.0.19041) | CI WARP (10.0.26100) | spread |
|---|---|---|---|
| peak signal — `max frame_diff(frozen[i], live[i])` | 0.109573 | **0.009683** | **11.3x** |
| peak control — `max frame_diff(frozen[0], frozen[i])` | 0.407826 | 0.264177 | 1.54x |
| **ratio** — peak signal / peak control | **0.268675** | **0.036654** | **7.3x** |
| `golden.rs` `MEAN_TOL`, for scale | 0.02 | 0.02 | — |

The CI reading is **stable, not noise**: `check (windows-latest)` and the instrumented `coverage`
job produced `0.036654` identically, in the same push (run 30903871856).

Three things follow, and the third is the decision.

**The ratio does not cancel the machine.** If it did, the two readings would agree; they differ by
7.3x. Signal and control move by different factors between the two builds — 11.3x against 1.54x —
because they are not the same kind of quantity. The control measures the dissolve's own
progression (the incoming side fading in). The signal measures the outgoing side continuing to
render *through* trails accumulation. An in-run control on the same adapter is **necessary** for a
portable ratio and is not **sufficient**; the two terms must also respond to the machine the same
way, and these do not.

**The numerator is dominated by a driver artifact, and that is not arguable.** The code is
identical and deterministic on both machines — the test's own opening-frame `assert_eq!` on raw
RGBA is the demonstration that two runs reproduce each other byte for byte. An 11.3x spread in the
signal between two builds of the same software rasterizer therefore cannot be the dissolve's
behaviour. It is `dissolve_at`'s documented WARP quirk: allocating the dissolve's GPU resources
mid-run resets what the trails feedback resolves to, so the outgoing side comes back at a single
stroke's brightness. How much history that costs is a driver behaviour, and the newer WARP evidently
costs more of it. Plan 0060's Risks section predicted this hazard in the abstract; the measurement
is what makes it a number.

**And the CI signal sits under the declared noise floor.** `0.009683` is below `MEAN_TOL = 0.02`.
Same-machine determinism means it is not literally noise — but ADR-0071's own corollary says a
claim of that magnitude on this statistic is not worth defending, and it applies to a claim built
*on* the quantity as much as to one asserted *about* it.

Plan 0060 named the fallback condition in advance — a CI ratio below `0.05` — and routed the
decision here rather than to `dev`. `0.036654` is below it.

## Decision

**`dual_live_keeps_the_outgoing_side_animating` keeps its property form on WARP and gains no
magnitude floor, in CI or anywhere else.** The magnitude claim is not written from these numbers;
it is deferred to hardware, where the allocation quirk that crippled the numerator does not exist.
Its sibling `a_dual_live_dissolve_carries_the_outgoing_trail` is already hardware-only and already
asserts a magnitude there (`CARRIES = 1.5`), so the shape is established.

**Corrected — the hardware is this box, and the claim is owed now rather than someday.** This
paragraph originally sent the measurement to Plan
[0053](../plans/0053-the-suite-stops-blessing-what-warp-gets-wrong.md) Phase 3, on the belief that
a `human` phase gated on a machine we lacked was the next time real hardware would see this suite.
That belief was wrong and cheap to check: the gate in the code is
`Renderer::adapter_is_software()` — `device_type == DeviceType::Cpu`
(`core/src/render/context.rs:148`) — not "discrete", and the sibling **runs and passes on the dev
box today**. So the measurement is one `software: false` away in `dissolve_at`, and it is the
*clean* one: on hardware the dissolve's opening frame is byte-identical to the ordinary frame it
replaces, so the numerator is the outgoing side animating and nothing else. Plan 0060 Phase 3
carries it. What such an assertion buys is bounded and should not be oversold — a hardware-only
test **skips in CI on both runners** (WARP on Windows, no software Metal on macOS, ADR-0016), so it
is enforced by the local gate and the pre-push hook, never by CI. That is the same bargain the
sibling already makes.

The test's doc comment is **demoted to what it actually proves on a software adapter**: that
dual-live runs, that it produces a different picture from freeze somewhere in the window, and that
the dissolve is dissolving. It stops reading as a guard on the defect, because on WARP it is not
one — the allocation quirk makes the two modes differ for its own reasons, so a genuinely held
outgoing side could still satisfy it. That was already true when Phase 1 landed and was recorded
only in the plan; it now lives in the test.

ADR-0071's rule gains one corollary: **a ratio is a property only when numerator and denominator
are the same kind of quantity.** Same run and same adapter are the entry requirements, not the
proof.

## Consequences

### Positive

- No number enters the suite that would break on the next runner image. Setting a floor from these
  two readings would have been a measurement asserted universally — the exact shape ADR-0071
  exists to forbid, at a different magnitude.
- CI keeps a green, non-flaky, cheap exercise of the dual-live code path, and keeps printing the
  full signal/control series through the `.config/nextest.toml` override — so the next time these
  numbers move, the log says so.
- The magnitude claim lands where it can be trusted, on the machine that already owes this suite a
  visit, next to the sibling that already makes a magnitude claim about the same dissolve.
- One ADR-0071 Positive is withdrawn honestly rather than left standing: *"the ratio form
  generalizes — any future differential pixel test has a control available"* is now qualified by
  the corollary above.

### Negative

- **Nothing in CI defends the dual-live magnitude, and the check that remains is weaker than it
  reads.** This is the cost, it is larger than the fallback's headline suggests, and it is written
  here rather than inferred: on WARP the surviving assertions can pass with the outgoing side held.
  CI has smoke coverage of the dissolve, not a guard on the defect.
- ~~**The magnitude claim has no date.** It is deferred to a `human` phase gated on hardware this
  side does not have, and Plan 0053 Phase 3 has been waiting since 2026-08-02. If that hardware
  never materializes, the claim is never restored.~~ **Withdrawn the same day: the hardware is the
  dev box.** The claim has a date and an owner — Plan 0060 Phase 3, `dev`. What survives of this
  cost is smaller and different: the restored assertion will be **hardware-only, so CI never runs
  it**. Between pushes, the dual-live magnitude is defended by the local gate alone.
- **A hardware ratio measured on one box is a measurement, not a property**, and ADR-0071 obliges
  it to say so and skip elsewhere — which is exactly the sibling's shape, and exactly what this ADR
  refused to let the *WARP* number pretend otherwise about. The one thing it must not become is a
  floor written from this box and asserted everywhere.
- The two WARP readings are now a known, unexplained 11x apart on a statistic nothing asserts.
  That is recorded and not investigated; if the newer WARP is losing *more* trail history than the
  older one, the same artifact is silently present in every other WARP capture this suite takes.

### Neutral

- No production code changes; no golden baseline moves; no CI workflow change. The whole decision
  is a test's doc comment, an assertion not added, and this record.
- The DSP half of Plan 0060 Phase 3 is untouched by this — the arm64 values and the determinism
  spec's scoping sentence proceed as written.

## Alternatives considered

### Alternative A — Set the floor anyway, at half the lower reading

`0.036654 / 2 = 0.018`, keeping the stated 2x headroom on the weaker machine. Rejected because the
headroom is measured against the wrong thing. The ratio varies 7.3x between two builds of one
rasterizer; 2x of headroom under the *lower of two samples* is not headroom, it is a coin flip on
the next runner image. It would also put the floor at `0.018` on a numerator whose CI value is
`0.0097` — a threshold calibrated against a quantity beneath the project's own declared noise floor.

### Alternative B — The documented fallback: make the whole test hardware-only

Plan 0060 named this in advance: skip on software alongside
`a_dual_live_dissolve_carries_the_outgoing_trail`, accepting that CI stops checking the dissolve.
Rejected because it gives up more than the decision requires. What failed to travel is the
**magnitude**, not the property: the exact-difference assertion and the non-trivial-control
assertion are green on both WARP builds, cannot fail spuriously, cost nothing, were verified
non-vacuous by mutation (freeze against freeze reads exactly `0.000000` at all 40 frames and
fails), and carry the printed series. Deleting a working instrument to settle a question about a
number that is not being added is a trade with no upside. The substance of the fallback — no ratio
floor — is taken; its mechanism is declined.

### Alternative C — Re-pose the control so the two terms are the same kind of quantity

Compare against the outgoing preset's *own* frame-to-frame motion rendered without a dissolve, so
signal/control reads as "how much of the ordinary motion survives into the dissolve" — near 1.0
when live, near 0 when held, and dimensionally the claim. Better-posed, and rejected for now on
mechanism rather than taste: on WARP the numerator is crippled by the allocation quirk before any
denominator is chosen, so no choice of control repairs it, and confirming that would cost another
push-and-read round trip to learn something the hardware session answers directly. Named as the
successor if the hardware never arrives.

### Alternative D — Delete the test

Rejected on ADR-0071's own grounds: the claim is worth defending. What is wrong is where it can be
proved, not whether.

## Notes

Readings taken 2026-08-04. Local: commit `1d56600`, `Microsoft Basic Render Driver`, DX12, driver
10.0.19041.5794, Windows 10 10.0.19045. CI: run **30903871856**, `windows-latest`, WARP
10.0.26100, read through the `.config/nextest.toml` `success-output` override added in `31073f6`
(without which both readings would have been invisible on exactly the green run that produced
them).

Two of Plan 0060's other predictions resolved in the same run and are recorded with the plan rather
than here: `onset_raw`'s arm64 divergence is `1.86e-5` relative — 2.2x `bass_raw`'s, not the orders
of magnitude the plan flagged as possible — and the coverage ratchet evaluated for the first time
since 2026-07-30 at **93.34 %** against `COVERAGE_FLOOR = 88`.

## Outcome (added at Plan 0060's close, 2026-08-04)

The hardware reading this ADR called for was taken hours later, on the same day, in `ae4c215`
(`a_dual_live_dissolve_moves_the_picture_against_its_own_progression`, floor `0.018`). **The
decision above stands and no assertion changes.** But the third reading is not the confirmation the
ADR expected — it falsifies the mechanism recorded in Context, and the correction matters more than
the number.

| statistic | hardware (AMD Radeon integrated, DX12) | CI WARP 10.0.26100 | local WARP 10.0.19041 |
|---|---|---|---|
| peak signal | 0.009653 | 0.009683 | 0.109573 |
| peak control | 0.264172 | 0.264177 | 0.407826 |
| **ratio** | **0.036542** | **0.036654** | **0.268675** |

**The hardware adapter agrees with CI WARP, not with the local one** — to three significant figures
on the ratio and **five on the control**. Two independent statistics reproducing that closely across
a software rasterizer and a hardware DX12 adapter is not coincidence.

**So the Context section has the direction backwards.** It attributes the 11.3x signal spread to
`dissolve_at`'s allocation quirk "costing more history on the newer build". The evidence says the
opposite: WARP 10.0.26100 behaves like hardware, and **WARP 10.0.19041 — this box's build, the one
every local capture is taken on — is the outlier**, exhibiting the quirk in the *inflating*
direction. The conclusion that survives is narrower and still sufficient: the numerator is
contaminated on at least one rasterizer build, so the ratio is not portable across builds. What does
not survive is the claim about *which* build, and that claim was doing work.

**Two things follow, both for Plan [0053](../plans/0053-the-suite-stops-blessing-what-warp-gets-wrong.md),
which owns whether WARP's output should be trusted at all.**

- The Negative bullet above — *"if the newer WARP is losing more trail history than the older one,
  the same artifact is silently present in every other WARP capture this suite takes"* — points at
  the wrong build, and the corrected version is sharper rather than milder. The golden suite blesses
  on **this box**, on 10.0.19041, the build that disagrees with hardware by 1.54x on a statistic
  (the control) that involves no dual-live asymmetry at all. That is a `frozen`-only sequence
  rendering measurably differently here than on either of the other two configurations, and nothing
  has looked at what else it moves.
- `dissolve_at`'s doc comment (`core/src/render/mod.rs`) states the quirk as a property of *"the DX12
  WARP rasterizer"*, and it is load-bearing — it is the stated reason
  `a_dual_live_dissolve_carries_the_outgoing_trail` is hardware-only and the reason
  `core/tests/background_composite.rs` skips. On this evidence it is a property of one WARP build,
  written as a property of WARP: [ADR-0071](0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md)'s
  own error, one level down, in prose instead of an assertion. Left standing deliberately rather
  than edited from a single contrary reading — a second hardware or newer-WARP configuration would
  settle it, and Plan 0053 Phase 3 is where that happens.

**Alternative A is weaker than it reads, and this is the honest place to say so.** It rejected the
`0.018` floor as "a coin flip on the next runner image" and as calibrated against a numerator
beneath `MEAN_TOL`. The shipped hardware test takes **exactly that floor**, `0.018`, against a
numerator of `0.0097` — answering the second objection with a determinism argument (two runs of the
same code on the same adapter in one process, opening at exactly `0.0000` and climbing
monotonically) that is *available on hardware because the opening frame is byte-identical there*.
All three readings clear `0.018`. The rejection still stands on its remaining leg — a quantity that
moves 7.3x across two builds of one rasterizer should not carry a floor asserted everywhere — but
**"would `0.018` in fact hold in CI" is now an open question with three data points behind it, not a
settled no.** It is not reopened here: doing so on one further sample would repeat the mistake this
ADR exists to record. It is reopened by a fourth configuration, or by Plan 0053 establishing which
WARP build to believe.

**And one of this ADR's Positive bullets is now false as written.** *"The magnitude claim lands
where it can be trusted, on the machine that already owes this suite a visit"* — it landed on the
dev box the same day, per the correction in the header. Plan 0053 Phase 3 no longer owes it.
