# ADR-0232 — A preset's frame cost is measured and reported, never asserted

> **Status:** accepted 2026-09-23, implemented by
> [Plan 0207](../plans/done/0207-the-commitments-get-their-instruments.md) Phase 2 — carries an
> `Outcome`
> **Date:** 2026-09-19
> **Related plan(s):** [0207](../plans/done/0207-the-commitments-get-their-instruments.md)
> **Relates to:** [ADR-0045](0045-quality-tiers-floor-and-rich.md) (the tiers and the governor),
> [ADR-0071](0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md) (a measurement
> names its machine), [ADR-0016](0016-gpu-tests-opt-in-ci-scope.md) (CI has no GPU contract),
> [NFR §1](../nfr.md#1-performance--adaptive-quality) (the Floor commitment)

## Context

The owner's stated aim is that the app *"would still be runnable on a mid range laptop"* whatever
the preset. [NFR §1](../nfr.md#1-performance--adaptive-quality) already commits to more than that —
**≥ 60 fps at 1080p at the `Floor` tier on [§2](../nfr.md#2-platform-baseline)'s baseline**, which
is *any DX12-capable GPU including integrated, ~2015+*. A mid-range laptop sits well above that
line, so the commitment is not in question.

**Whether it holds is.** The behavioural suite has a long roster of gates — `sanity`, `reactivity`,
`animation`, `beat`, `distinctness`, `golden`, `composite`, `bloom`, `geometry_extent` and the
per-system ones — and **not one of them measures time**. Every gate asks what a frame looks like.
Nothing asks what it cost. So the Floor commitment is asserted in a document and checked by
nothing, which is the same shape as [backlog 0256](../design-backlog.md)'s quality gap one level
over.

**There is direct evidence the commitment can be missed.** While browsing the converted MilkDrop
corpus on the development box on 2026-09-19, two presets reported, in the window title's own
readout: `2009 4th of July with AdamFX n Martin - into the fireworks E` at 110 fps with a **p99 of
114.2 ms**, and `$$$ Royal - Mashup (114)` at 81 fps with a **p99 of 42.7 ms**. Those are
*converted* presets rather than shipped ones, so they say nothing about the shipped library — but
they are a working demonstration that preset complexity alone can blow a 16.7 ms budget on hardware
far above the baseline.

**The governor does not close this.** [ADR-0045](0045-quality-tiers-floor-and-rich.md)'s frame-time
governor demotes `Rich` to `Floor` once per session, one way. That rescues a mispredicted *tier
budget*; it does nothing for a preset that is expensive at `Floor` too, because there is nowhere
further to demote.

The question is therefore not whether to care, but **what kind of instrument to build** — and this
project has a strong opinion about numbers already.
[ADR-0071](0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md) exists because
two frozen frame-time-shaped numbers asserted universally produced five consecutive red CI pushes,
and [ADR-0016](0016-gpu-tests-opt-in-ci-scope.md) keeps GPU out of the CI contract entirely, so a
frame-time assertion has no reliable machine to run on in the first place.

## Decision

We will **measure a preset's frame cost and report it, and assert nothing**.

The preset report (`shot --presets … --report`) gains a per-preset frame-cost reading, printed
beside the reactivity and distinctness columns it already carries, and **naming the machine and the
tier it was taken on** per ADR-0071. It is **advisory** in exactly the sense `distinctness` already
is: it prints, it flags, it never fails.

**The Floor claim gets a named machine rather than a gate.** [NFR §9](../nfr.md#9-test-hardware-matrix-what-the-user-has)'s
matrix already lists an *"Older Windows PC (iGPU)"* against *"the performance floor (§1) on
baseline hardware (§2)"*. That row is a machine class, not a machine: it is named in this ADR's
plan, and the Floor reading is taken on it, so the number in NFR §1 stops being asserted and starts
being a measurement with a configuration attached.

**Evidence before mechanism, deliberately.** Whether any *shipped* preset misses the budget is
currently unknown. Designing a gate before knowing that would mean designing it around a guess —
the same reasoning the owner applied to preset curation in backlog 0256, and the same order. If the
report shows real offenders, the gate that follows is argued from them and gets its own ADR.

## Consequences

### Positive

- **The commitment becomes checkable at all**, which it is not today, at the cost of one report
  column.
- **It cannot go red for the wrong reason.** An advisory reading on a machine that names itself is
  immune to the failure mode ADR-0071 was written after — a frame-time number asserted on whatever
  hardware happened to run it.
- **It fits the harness as it stands.** `distinctness` is already ADVISORY and already prints a
  matrix nobody's build depends on, so the shape is precedented rather than novel.
- **It prices content honestly.** A preset author gets to see what a look costs, which is
  information the lane has never had.

### Negative

- **An advisory nobody reads is worth nothing**, and this project has already found that shape
  expensive — `distinctness`'s own family roster went stale precisely because nothing fails when it
  is wrong (backlog 0256). This ADR knowingly accepts that risk a second time.
- **It does not make the Floor commitment true.** If a shipped preset misses the budget on baseline
  hardware, this decision produces a number and no consequence. That is the point, and it is also
  the cost.
- **A frame cost measured under `shot` is not a frame cost in the app.** The headless path has no
  window, no present, no compositor and no audio thread competing with it, so the reading is a
  comparative instrument between presets rather than an absolute prediction of the show.
- **One more column in a report that is already dense.**

### Neutral

- Nothing about the tiers or the governor changes. ADR-0045 stands in full.
- The size cap is a separate question, decided in
  [ADR-0231](0231-the-standalone-size-cap-is-re-derived-from-what-it-carries-and-the-build-reports-it.md).

## Outcome (2026-09-23)

**The advisory half shipped; the named-machine half did not, and moved.**
[Plan 0207](../plans/done/0207-the-commitments-get-their-instruments.md) Phase 2 gave
`shot --presets … --report` a per-preset frame cost in ms/frame at 1920x1080 — the size NFR §1
states its budget at — taken fully driven on the report's tier, as the slope between a 48- and an
8-frame capture, best of two repeats, with the presets interleaved inside the repeat loop so none of
them inherits a GPU that had finished ramping its clocks. The text report's header names the adapter
and the build profile, the JSON carries the same as a top-level `machine` object, and a `!` marks a
reading past 16.67 ms and does nothing else. On a software adapter **no reading is taken at all**:
the cells print `-`, the JSON omits the key, and the header says why — a frame time on a CPU
rasterizer is a fact about the rasterizer, and CI has no GPU contract to take one on (ADR-0016).

**This Decision's second paragraph said NFR §9's machine class *"is named in this ADR's plan"*. It
was not.** The box is not in hand, and both machines that are are faster than the baseline by
construction, so Plan 0207's Phase 3 was deferred at its close and the walk extracted verbatim into
`docs/on-device-validation.md` as a dated `iGPU-gated` section. What landed instead is the honest
statement: NFR §1's Floor bullet now says in its own text, dated, that the number has never been read
on the hardware it names, and §9's row says the machine is still a class. **So the Floor claim is not
yet a measurement with a configuration attached** — it is an assertion that admits it is one, which
is the nearest true thing, and the instrument for the reading now exists.

**The negative this ADR knowingly accepted is unchanged**: an advisory nobody reads is worth
nothing, and the one occasion that guarantees this column gets read at least once is the deferred
walk.

## Alternatives considered

### Alternative A — a hard per-preset frame-cost gate

Fail the suite when a preset misses the budget on the reference machine. **Rejected on two counts.**
It is exactly the machine-dependent frozen number ADR-0071 was written to stop, and CI has no GPU
contract to run it on (ADR-0016), so the gate would either skip in the place it most needs to run
or assert a number measured somewhere else. And it would be designed against a guess: nobody yet
knows whether any shipped preset is a problem. If the advisory finds offenders, this becomes the
obvious next ADR — argued from evidence rather than from fear.

### Alternative B — nothing; the governor is the answer

Rely on adaptive tiers and the demotion. **Rejected because the governor cannot reach the case that
matters.** It demotes once, one way, from `Rich` to `Floor`; a preset that misses at `Floor` has
nowhere to go, and NFR §1's floor commitment is precisely a claim about `Floor`. The governor
protects against a mispredicted tier budget, not against expensive content.

### Alternative C — restate the hardware target as a mid-range laptop

Loosen §2's baseline to match the stated goal, making the commitment easier to meet. **Rejected
because it weakens a promise in order to pass it.** The current baseline — a 2015-era integrated
GPU — is stricter than the goal the owner articulated, so meeting it meets the goal automatically;
restating it downward would trade a real commitment for a comfortable one and would quietly drop
every machine between the two.
