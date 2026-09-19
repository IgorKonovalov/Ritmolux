# ADR-0231 — The standalone's size cap is re-derived from what it carries, and the build reports it

> **Status:** proposed
> **Date:** 2026-09-19
> **Related plan(s):** [0207](../plans/0207-the-commitments-get-their-instruments.md)
> **Amends:** [NFR §4](../nfr.md#4-size-and-dependencies)'s standalone figure
> **Relates to:** [ADR-0159](0159-the-component-gets-its-own-size-cap-and-the-recipe-carries-it.md)
> (the precedent, one artifact over), [ADR-0038](0038-tag-driven-release-unsigned-universal-mac-app.md)
> (what a tag ships), [ADR-0071](0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md)
> (a measurement names its machine)

## Context

[NFR §4](../nfr.md#4-size-and-dependencies) sets a **10,000,000 B soft cap** on the standalone
release exe and says of it, in its own text, that *"the **value** is the inherited one, and it has
never been measured against what the exe actually contains."*

On 2026-09-19, on this project's development box (Windows 10, `cargo build --release`, default
features), it was measured: **10,971,648 B**, or **9.7 % over**. The measurement was incidental —
taken to price embedding thumbnails for [Plan 0206](../plans/0206-the-browser-shows-the-look.md) —
and is recorded as [backlog 0257](../design-backlog.md).

**The asymmetry is what makes this a decision rather than a correction.** The foobar component has
exactly the machinery this is missing:
[ADR-0159](0159-the-component-gets-its-own-size-cap-and-the-recipe-carries-it.md) derived that
artifact's cap from what it actually carries — 9,789,952 B measured, plus one more step the size of
the largest feature class the project had shipped, rounded up to the next binary boundary, giving
12,582,912 B — and put the measurement in `packaging/foobar/build-component.ps1`, which prints the
length on every build and warns above 90 % of the cap. **Nothing does this for the standalone.** No
build step, no CI job and no gate reports its size, which is how a 9.7 % breach sat unremarked in
the artifact NFR §4 names first.

**And the cap is answering a question nobody is asking of it.** In the interview that produced this
ADR the owner stated the real goal as *"keep calculations lean and fast, so no matter the
complexity of preset it would still be runnable on a mid range laptop"* — a **runtime** commitment,
which is [NFR §1](../nfr.md#1-performance--adaptive-quality)'s Floor tier and the frame-time
governor, not a binary size. A 10 MB exe and a 16 MB exe render at the same rate. What §4's cap
genuinely protects is download size, cold-start time and dependency discipline, and it should be
argued on those terms rather than borrowed as a performance proxy.
[ADR-0232](0232-a-presets-frame-cost-is-measured-and-reported-never-asserted.md) takes the runtime
half.

## Decision

We will **re-derive the standalone's cap from what the artifact carries**, by
[ADR-0159](0159-the-component-gets-its-own-size-cap-and-the-recipe-carries-it.md)'s rule applied to
this artifact — measured size, plus headroom for one more feature of the largest class this project
has shipped, rounded up to the next binary boundary — and **give the recipe a carrier**: the
packaging step prints the exe's length on every build and warns near the cap, exactly as the
component's does.

**The constant is not set in this ADR, and that is deliberate.** The rule needs one input this ADR
does not have: what a feature of the largest shipped class actually costs *in this binary*. The two
candidate boundaries differ by a factor that matters — 12,582,912 B leaves 14.7 % headroom over
today's measurement, 16,777,216 B leaves 53 % — and choosing between them without the step size
would be inventing a number, which is the failure this whole ADR exists to correct rather than
repeat. [Plan 0207](../plans/0207-the-commitments-get-their-instruments.md) Phase 1 takes that
measurement and **its stop condition sets the constant**; this ADR gains a dated `Outcome` at the
plan's close recording what it came out as.

**The cap stays soft.** ADR-0159 records that these caps *"never fail a release over a size"*, and
that is not narrowed here: the carrier warns, and a release is never blocked by a byte count.

## Consequences

### Positive

- **The number stops being inherited.** Whatever it becomes, it will have been derived from this
  artifact rather than carried over from a context nobody can name.
- **A breach becomes visible the day it happens**, on the developer's own machine, rather than
  years later and by accident.
- **The two artifacts are finally symmetric.** One rule, one carrier shape, two caps — which is
  also the cheapest thing to keep true, because the component's recipe is the worked example.
- **Future feature arguments start from a real number.** Every "does this fit?" — thumbnails being
  the immediate one — currently begins from a figure that is wrong in an unknown direction.

### Negative

- **It will almost certainly raise the cap, and a raised cap reads as moving the goalposts.** The
  honest answer is that the goalpost was never placed; but the appearance is real, and the
  derivation has to be written down well enough to carry the argument later.
- **A soft cap with a warning is still only a warning.** Nothing prevents the binary growing past
  it, and this ADR deliberately declines to make it fatal — so the instrument's whole value rests
  on somebody reading the build output.
- **The measurement is a property of a build, not of the tree.** It moves with the toolchain, with
  the profile and with `debug = 0` on dependencies; a number taken on one machine is
  [ADR-0071](0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md)'s measurement
  rather than a property, and the carrier must print what it saw rather than assert what it
  expected.
- **Two caps mean two things to re-derive** when the project's shape changes again.

### Neutral

- Nothing about the runtime commitment changes here. NFR §1's Floor tier, §2's baseline and the
  frame-time governor are untouched by this ADR.

## Alternatives considered

### Alternative A — keep 10,000,000 B and shrink the binary to fit

Treat the 9.7 % as a defect and pay it down. **Rejected because it spends real work to satisfy a
number nobody derived.** NFR §4 states plainly that the value is inherited and was never checked
against the artifact; optimising toward it would give an arbitrary figure the authority of a
requirement, and the first question anyone asks afterwards — *why ten million?* — would still have
no answer.

### Alternative B — retire the numeric cap and keep only the dependency discipline

Drop the byte figure; keep *"every new crate is a cost"* as the operative rule. **Rejected because
it removes the only thing that could notice a dependency that doubles the binary.** The discipline
is a habit and habits are not instruments; the whole finding here is that a commitment without a
carrier goes unchecked for years, and the repair for that is not to delete the commitment.

### Alternative C — make the cap fatal, like the component's other checks

Fail the build above the cap. **Rejected because ADR-0159 already decided the opposite for the
sibling artifact**, on the grounds that a size is not a correctness property, and splitting the two
artifacts' severity would leave the project unable to say what its size caps mean. If soft turns
out to be too weak, that is one decision for both, taken once.
