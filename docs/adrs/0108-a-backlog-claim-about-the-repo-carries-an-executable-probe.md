# ADR-0108 — a backlog claim about the repo carries an executable probe

> **Status:** accepted (2026-08-15, user approval)
> **Date:** 2026-08-15
> **Related plan(s):** [0093 — the backlog stops asserting things about a repo it has not read](../plans/done/0093-the-backlog-stops-asserting-things-about-a-repo-it-has-not-read.md)
> **Supplements:** [ADR-0033](0033-testing-strategy-coverage-ratchet-and-pre-push-gate.md), [ADR-0073](0073-the-windows-ci-critical-path.md)

## Context

**Four entries in `docs/design-backlog.md` have now been falsified, and all four failed the same
way.** Each made a claim about *the repository* — what it contains, what it documents, what is
built — and each claim was wrong at the moment it was written or became wrong shortly after. The
symptom halves were fine every time; what rotted was the half asserting an absence.

| Entry | The claim | What was actually there |
|---|---|---|
| 0052 | `Spectrum Ridge` has no tonal structure | It was never flat; the statistic convicted the right preset for the wrong reason |
| 0078 | `kaleido_tile` is not quantized *(and should be)* | `kaleidoscope.rs:458` carried an explicit *"Deliberately **not** rounded"* comment, landed five phases earlier in the same plan |
| 0081 | The house gain rule is written down nowhere | `presets/README.md:203` had carried `G = C / 0.85` for six days |
| 0082 | The quality governor is not yet built, and is specified to read `p99` | [Plan 0044](../plans/done/0044-quality-tiers.md) shipped it ten days earlier, and `sustained_miss` reads the raw frame-time series |

The file's own header already carries the rule — *"Verify every entry against the code before acting
on it"* — and the convention it implies is **already widespread**: `- **Verified against code:**`
appears **52 times** across the live file and the archive, and the good ones already cite `file:line`
(`core/src/preset/schema.rs:223`, `core/src/render/palette.rs:332`). So this is not a project that
forgot to have a rule. It is a project whose rule is prose, optional, and re-read by nobody.

**Three of the four falsified entries carried no stamp at all.** That is the ordinary half, and a
required stamp would have caught it.

**The fourth is the one that shapes this decision.** [0081](../design-backlog-archive.md) *did* carry
a stamp — dated, recent, and **true**: *"Re-verified against code 2026-08-13 — neither
`presets/README.md` nor `docs/presets.md` contains any 'failure state' language."* That sentence was
accurate. It verified the half of the entry that **survived** (the exception class) and never touched
the half in the entry's own title (the gain rule). A prose stamp records that *somebody looked*; it
cannot record *at what*, and it cannot be re-run when the subject moves. It is therefore necessary
and demonstrably not sufficient.

The last force is what a script can honestly do. **No checker can verify "nothing documents the gain
rule"** — that is a semantic claim over prose, and a project that pretended otherwise would trade a
known gap for a false green. What a checker *can* do is re-run a mechanical reduction the author
already committed to, and refuse to let that reduction quietly stop matching the world.

## Decision

**Every live entry in `docs/design-backlog.md` that makes a claim about the repository carries a
dated verification bullet with at least one machine-runnable probe, and a checked-in script re-runs
every probe on every push.** A probe is a restricted two-verb grammar — `absent: <regex> in: <path>`
and `present: <regex> in: <path>` — written as an inline-code span beside the claim it reduces:

```markdown
- **Verified 2026-08-15** — the governor does not exist: `absent: sustained_miss in: core/src`
```

The script resolves probes with **its own file walker, never a shell**, so a document can describe a
check without a document being able to execute code in CI. It is a **gate**, at the three call sites
[`check-doc-links.mjs`](../../scripts/check-doc-links.mjs) already occupies: `pre-push` (opt-in per
clone), the architect close ceremony, and the CI `links` job, which is the un-bypassable backstop.

Two deliberate softenings, each answering a specific way this could go wrong:

- **An entry that genuinely cannot be reduced to a probe carries a reasoned opt-out** —
  `unprobeable: <why>` — which the script accepts and *prints in a summary*. The set of unchecked
  claims stays visible and small rather than invisible and unbounded. This is the pressure valve
  that keeps the other probes honest: without it, the gate would push authors to write a weak probe
  that passes, and a weak probe reads as verification.
- **Staleness is an advisory, not a failure.** The script also compares each stamp's date against
  `git log` for the paths its probes name, and prints the entries whose subject has moved since
  anyone last read them. It does not fail on it, because a probe scoped at `core/src` would re-red
  on every commit — and the advisory's low noise is exactly what rewards narrow probe paths, which
  are also better probes.

## Consequences

### Positive

- **The historical cases would all have been caught, and cheaply.** 0082's claim reduces to
  `absent: sustained_miss in: core/src`, which goes red on 2026-07-30 — the day the governor landed,
  ten days *before* the entry was written. 0081's reduces to `absent: C / 0.85 in: presets/README.md`,
  red at the moment of writing. 0078's to `absent: not.*round in: core/src/render/kaleidoscope.rs`,
  likewise. The check that would have prevented three plan-closes' worth of correction is one grep.
- **It upgrades a convention rather than replacing one.** The 52 existing stamps keep their shape and
  their prose; what changes is that the mechanical part moves out of the sentence and into something
  re-runnable. Nobody has to learn a new document.
- **The reduction is visible next to the claim.** 0081's failure was a verification that did not
  cover its own title, and nothing in the entry showed that. A probe sitting beside the claim is
  readable *as* a reduction — a reviewer can see whether `absent: C / 0.85` covers "the gain rule is
  written down nowhere" in a way they could not see it in a paragraph.
- **Cost is bounded and already measured.** There are **14** live entries. The CI `links` job is
  `ubuntu-latest` plus one `node` line with no setup step, so this is one more `- run:` on a job
  nowhere near [ADR-0073](0073-the-windows-ci-critical-path.md)'s Windows critical path.
- **It gives `check-doc-links.mjs`'s orphaned `root` argument a caller.** Plan 0084 built that
  argument for a fixture tree, ran it ad hoc, and never committed the tree — recorded at that close
  as the one thing left undone. Both checkers can share one committed fixture, so both bite checks
  become repeatable.

### Negative

- **A passing probe is not a correct entry, and this must not be read as one.** The probe verifies
  the reduction the author chose. An entry can be wrong in a way its own probe does not cover — which
  is precisely 0081, one level down. The mitigation is visibility, not proof: the probe is printed
  beside the claim so a mismatch is legible. **Green means "the stated reduction still holds", never
  "this entry is true."**
- **A rename turns the gate red for a reason that is not the entry's fault.** `absent: sustained_miss`
  goes red if someone renames the function to something else and reintroduces the old name elsewhere,
  and `present:` probes break on any rename at all. This is accepted, and arguably wanted: an entry
  naming a symbol that no longer means what it meant needs re-reading regardless of why.
- **It adds a gate, and gates decay when they hurt** ([ADR-0033](0033-testing-strategy-coverage-ratchet-and-pre-push-gate.md)'s
  own argument for its fast subset). This one is a filesystem walk over `docs/` plus a bounded number
  of regex scans, so it belongs in the fast subset — but it is one more thing that can stand between
  an author and a push.
- **The opt-out can be abused into a blanket.** Nothing stops `unprobeable:` on an entry that could
  perfectly well carry a probe. The only defence is that the summary prints every one of them, so the
  abuse is visible at each close rather than silent.
- **It does nothing for ADRs or plans**, which rot the same way — ADR-0099 named
  `metrics::peak_to_mean` an existing instrument that did not exist, and Plan 0085's own Phase 3
  done-when asked two frame-time columns to diverge in a log that had only one. That scope was
  considered and deliberately declined below.

### Neutral

- The archive is out of scope and stays out. An archived entry is a closed record whose value is the
  correction it carries; re-probing it would be checking history against the present.

## Alternatives considered

### Alternative A — require a dated prose stamp, no probe

Keep the existing `- **Verified against code:**` convention and simply make it mandatory, with the
script asserting only that a live entry has one and that its date is recent. Zero new syntax, and it
upgrades all 52 existing stamps for free. **Rejected because 0081 is a worked counter-example**: its
stamp was present, dated, recent and *true*, and the entry was false anyway, because prose cannot
record which claim was checked and cannot be re-run when the subject moves. This alternative would
have caught three of four; the probe catches four of four, and the fourth is the instructive one.

### Alternative B — staleness only, computed from `git log`

Skip probes entirely: flag any live entry whose stamp predates the last commit touching the files it
cites. Cheapest possible change, no new grammar at all. **Rejected as too noisy to be a gate and too
weak to be a report.** It cannot distinguish a commit that invalidates the claim from one that does
not, so scoped at `core/src` it fires constantly and scoped narrowly it says nothing — and critically
it is blind to the *birth defect* case, where the entry was already false on the day it was written.
0082 and 0081 were both wrong at birth. Kept as the **advisory half** of the decision, which is the
role it can actually hold.

### Alternative C — extend the gate to ADRs and plans

Both make repo claims and both have been falsified (ADR-0099, Plan 0085 Phase 3). **Rejected on a
difference in kind, not in cost.** An accepted ADR is deliberately a *snapshot of what was believed
then* — that is why it is append-only and why falsification is recorded as a dated `Outcome` rather
than an edit. Gating it would fight that design and would red the build over a document that is
correctly frozen. A live backlog entry is the opposite: it exists to be acted on *now*, which is
exactly what makes a stale one dangerous rather than merely historical. Plans sit between the two
and are the natural second scope if this earns its place; that is a followup, not this decision.

### Alternative D — require a probe on every entry, no opt-out

Strongest guarantee, simplest rule to state. **Rejected because it manufactures the failure it is
trying to prevent.** Some claims are honestly unprobeable — a judgement about rendered output, a
want expressed by a user, "the docs never explain *why* X". Forcing those to carry a probe produces a
weak one written to satisfy a gate, and a weak probe is worse than an honest gap because it *reads*
as verification. The reasoned opt-out keeps the unchecked set explicit and countable.

## Notes

**The probe grammar is deliberately two verbs and no more.** `absent:` is the one that matters — every
falsified entry made an absence claim. `present:` exists for the citation-rot case, where an entry
says "`sustained_miss` is at `core/src/render/tier.rs:375`" and the symbol later moves. A count verb
(`exactly: 13 in: presets/`) was considered and left out: no falsified entry needed one, and a third
verb is a grammar to learn rather than a check to run.

**Why the gate cannot exec a shell.** The natural spelling of a probe is a backticked
`grep -rn "sustained_miss" core/src/`, and re-running it would be one `execSync`. That would make
every markdown file in the repo a script CI executes on push — an unacceptable surface for a
convenience. The restricted grammar buys the same expressiveness for the checks that actually occur,
with a parser instead of an interpreter.
