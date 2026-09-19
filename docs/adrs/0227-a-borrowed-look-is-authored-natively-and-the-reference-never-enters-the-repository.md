# ADR-0227 — A borrowed look is authored natively, and the reference never enters the repository

> **Status:** proposed
> **Date:** 2026-09-19
> **Related plan(s):** [0204](../plans/0204-the-library-learns-from-the-corpus-it-will-not-ship.md)
> **Relates to:** [ADR-0113](0113-milkdrop-presets-are-translated-ahead-of-time-onto-a-warp-mesh-idiom.md)
> (which deliberately left provenance undecided),
> [ADR-0081](0081-the-content-lane-lands-presets-and-architect-curates-the-set.md) (the landing
> route), [ADR-0089](0089-the-library-renews-by-replacement-cohorts.md) (how the set renews)

## Context

[ADR-0113](0113-milkdrop-presets-are-translated-ahead-of-time-onto-a-warp-mesh-idiom.md) built
`milkconv` and the `warp_mesh` idiom, and its Negative section named a cost it refused to pay
down: *"Preset provenance is unresolved and is not a technical problem. The large circulating
collections have no clear licensing. Shipping third-party presets in this repository is a decision
this ADR does not make."*
[Plan 0100](../plans/done/0100-the-engine-speaks-milkdrop.md) Phase 8 is the open `human` phase
that owns it, and its standing answer has been *nothing third-party in the repository or a
release*.

On 2026-09-19 the user converted the `cream-of-the-crop` corpus into a scratch directory outside
the checkout, browsed it theme by theme through `RLX_PRESET_DIR`, and picked twenty-one looks
worth having. The ask that followed was not an import. It was to author **native** presets in the
spirit of those looks.

That is a different question from Phase 8's, and it is the one this ADR answers: **on what terms
may the content lane use a third-party preset as a reference?** Three facts make it a real
decision rather than a formality.

**The corpus states an assumption, not a licence.** `cream-of-the-crop/LICENSE.md` says MilkDrop
presets were *"in almost all cases, not released under any specific license,"* that each author
theoretically holds full copyright, and that because they circulated widely for two decades *"it
is safe to assume them to be in the public domain."* An assumption of abandonment is not a grant,
and this repository is dual Apache-2.0/MIT.

**A render is not a neutral artifact.** The obvious way to give the content lane a target is to
render each pick and commit the stills beside the draft. But a render of a preset is a depiction
of that preset's output, and committing one puts third-party-derived content in the tree by a side
door that `presets/` was carefully kept clear of. The convenient path and the decided path point
opposite ways here, which is exactly why this needs writing down.

**The boundary inside "inspired by" is not self-evident.** Borrowing the *class* of motion — a
feedback field stirred by drifting sinusoids, a lobed contour, a two-tone ink — is how every
visual tradition works, and `warp_mesh` exists as a native scene, per ADR-0113, precisely so this
project can speak that vocabulary. Transcribing a specific preset's per-frame equations into our
expression grammar by hand is a different act with the same appearance from outside. ADR-0113's
Alternative B already rejected the *machine* version of that transcription on fidelity grounds;
the hand version is additionally a derivative work in substance, and retyping does not launder it.

## Decision

We will treat the converted corpus as a **visual reference for native authorship**, under three
constraints that travel together.

**Nothing from the corpus enters the repository — including renders of it.** Not a `.milk` file,
not a `milkconv` output, not a still or a clip of either. References live outside the checkout
(`WORK/milk-browse/`), are reproduced by re-running the converter over a corpus the repository
does not carry, and are disposable by design.

**A preset is authored from the look, not from the source.** The content lane works from the
rendered reference and its own reading of it. It does not open the converted `.toml`'s `[milk]`
bytecode or the EEL2 reproduced beside it, and it does not transcribe a source preset's equations,
constants or shader into our grammar. What is borrowed is the *gesture* — palette, cadence,
structure, the class of motion — which is the thing a person can carry away in their head.

**The reference is recorded in the plan, not in the shipped preset.** A preset's header carries
its mechanism, per `CLAUDE.md`'s comment rule, and names no third-party work. Which look a world
was authored toward is a dated record, and dated records live in `docs/plans/`. This keeps the
provenance trail honest and complete without writing a claim about someone else's copyright into
an artifact we ship.

Phase 8 stays open. This ADR narrows what the content lane may do **today** without it; it does
not decide, prejudge or quietly answer the question of shipping converted presets.

## Consequences

### Positive

- **The campaign starts without a licensing decision.** The one route that needs no answer from
  Phase 8 is the one that imports nothing, and this is it.
- **The content lane gets a real target instead of a description.** Judging a preset against a
  rendered reference is a different quality of work from authoring against prose, and the looks in
  question are motion-dominated — a sentence about one is nearly content-free.
- **The engine's own advantages apply.** A native world authored toward a MilkDrop look renders in
  linear light with a real tonemap and binds this project's grammar — tempo, bar phase, spectral
  novelty — none of which the source could reach. The result is not a copy competing with an
  original; it is a different instrument playing a borrowed phrase.

### Negative

- **The references are not reproducible from a checkout.** A future session reading Plan 0204
  cannot see what any preset was authored toward without obtaining the corpus and re-running
  `milkconv`. The plan's prose is the only durable record, and prose about a moving image is thin.
  This is the direct price of the first constraint and there is no version of it that is cheap.
- **No gate can enforce any of this.** Nothing distinguishes a native world from a careful
  transcription by inspection, and no script will ever try. This ADR is a discipline the content
  lane keeps, backed by review and nothing else — which is weaker than every other rule in this
  project that has a checker behind it.
- **The second constraint forbids the most useful debugging move.** When a native attempt falls
  short of its reference, the fastest way to learn why is to read what the source actually did.
  That is exactly what is ruled out, so the lane re-derives by eye and will sometimes fail to.
- **"The spirit of" is unfalsifiable.** There is no done-when that says a look was reached. The
  verdict is a human look call, which is why the plan that uses this ADR ends in one.

### Neutral

- Phase 8's standing answer is untouched, and the `RLX_PRESET_DIR` browsing route this rests on
  works on a user-supplied directory exactly as ADR-0113 said the import path would.
- The corpus's own themed folders (`Reaction`, `Dancer`, `Hypnotic`, …) are a useful vocabulary for
  talking about the picks and carry no weight beyond that — they are one curator's sort, not a
  taxonomy this project adopts.

## Alternatives considered

### Alternative A — answer Phase 8 and ship the converted presets

Decide provenance now in the permissive direction, on the strength of the corpus's own
public-domain assumption, and put converted `.toml` files in `presets/`. **Rejected because it is
not this decision's to make and the evidence does not support it.** Phase 8 is a `human` phase
owned by the user, the assumption in `LICENSE.md` is not a grant, and
[Plan 0142](../plans/done/0142-the-milkdrop-import-earns-its-verdict.md) Phase 4's third go/no-go
read the converted output as *one pair better, two good, one fixed, two still washed at the
ground, one wrong on structure* — so even setting licensing aside, the quality case for wanting
more of them is unmet for the third time. Nothing here forecloses it; a later ADR can still say
yes.

### Alternative B — transcribe the source equations into our grammar by hand

Read each pick's converted EEL2 and rewrite it as expression bindings and structural tables, which
would reach the look far faster than re-deriving it. **Rejected because retyping does not change
what the work is.** A hand transcription of a specific preset's per-frame program is a derivative
of that program whatever language it lands in, which walks straight back into the question this
ADR exists to route around. The fidelity objection from ADR-0113's Alternative B also still
stands — the expression layer is pure and total, so per-frame state has nowhere to go — but that
is the lesser reason here.

### Alternative C — commit the reference renders to `docs/images/`

Render each pick once, commit the stills, and let the plan point at them. **Rejected because a
render of a preset is derived from that preset**, so this puts third-party-derived content in the
tree under a different file extension. It would also be the first thing anyone cites as precedent
the next time the question comes up, which makes a convenience into a policy without an argument.
