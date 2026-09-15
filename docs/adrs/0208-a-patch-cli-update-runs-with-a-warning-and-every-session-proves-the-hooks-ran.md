# ADR-0208 — A patch CLI update runs with a warning, and every session proves the hooks ran

> **Status:** proposed
> **Date:** 2026-09-15
> **Related plan(s):** [0189](../plans/0189-the-conductor-can-be-watched-and-stops-re-proving-a-green-tree.md)
> **Amends:** [0205](0205-an-approved-plan-runs-under-a-conductor-and-every-judgement-it-cannot-make-parks-the-plan.md)
> (its Negative: "refuses one it has not been verified on")

## Context

`preflight` refuses any `claude --version` missing from `VERIFIED_CLI`. On 2026-09-15 the CLI had
updated itself from 2.1.270 to 2.1.272, and the run stopped before opening a lane. Clearing it meant
a probe run, reading its JSON against a prose table by eye, a constant edit and a commit. No lane can
do any of that, and an auto-updating CLI brings it back on no schedule (backlog 0224).

The guard exists because the conductor is built on observed headless behaviour, and the owner has
decided to accept patch-level moves with a warning. **The price has to be stated plainly: this CLI
numbers almost every release as a patch.** The verified versions are 2.1.270 and 2.1.272, so "warn
on patch, refuse on minor" turns the guard into a warning for nearly every update. Backlog 0222
records the risk arriving exactly there: between those two patches, `rate_limit_info.utilization`
moved under `unifiedWindows.seven_day`.

So the question is which silent changes would go unnoticed without the probe. Most would not. A
missing `result` event parks as `api`, a missing outcome block as `no_outcome`, a malformed one as
`bad_outcome`: the stream reader already fails closed. **The dangerous class is the one that fails
open**: the project `PreToolUse` hooks not running. Those hooks are the suite-lock rule, the push and
history-rewrite deny, the broad-staging deny and the attribution deny. A session without them still
ends with a well-formed outcome, and nothing the conductor reads today would show it.

## Decision

**A CLI version sharing major and minor with a verified entry, at a higher patch, runs with a
warning. Any other unlisted version is still refused.** The warning is printed at the start of `run`,
recorded on the run, and carried in the digest's **Needs you** until a verification lists the
version. The clearing procedure itself is unchanged.

**Every conductor-run session, verified CLI or not, must prove the project hooks ran in it.**
`conductor-suite-lock.js`, which already runs only when `RLX_CONDUCTOR=1`, appends one line per call
to a per-step hook log whose path the conductor passes in the environment. When a session ends, the
conductor checks two things. If its transcript contains at least one `Bash` or `PowerShell` tool call,
the hook log must exist. And the stream's `system/init` event must list the skill the prompt invoked.
Either failure parks the plan with the new reason `cli_contract`, before any verification of the
session's outcome. So a CLI that stopped loading hooks or skills is caught on its first session,
whatever its version.

## Consequences

### Positive

- **A CLI update stops blocking the queue.** The interruption becomes a digest line.
- **The fail-open class gets a check that runs every session**, which the probe never did. The probe
  runs once per version and only when a human remembers to.

### Negative

- **The guard is now mostly a warning**, for the numbering reason above. A change to anything besides
  hooks, skill loading and the fields the stream reader already demands can reach a run. That covers
  permission-mode semantics, `--max-budget-usd` enforcement and `--append-system-prompt-file`. The
  budget cap is the costly one: under subscription auth it was observed to overrun by one turn, and a
  CLI that stopped enforcing it would spend until the session ended.
- **The hook tripwire proves one hook ran, not that each did.** It is carried by the suite-lock hook
  because that hook is the conductor's own. If the CLI kept one hook and dropped another, the check
  would not see it.
- **A session that makes no shell call cannot be checked for hooks.** Every implement, fix and review
  session so far has made one, so this is theoretical today.

## Alternatives considered

### Alternative A — Keep refusing, and automate the comparison
`probe.mjs --compare` against a recorded baseline, clearing the version when every row matches. This
keeps the strongest guard and turns the eye-reading into a diff. Rejected by the owner's call: it
still blocks the queue until someone runs it, which is the friction 0224 names.

### Alternative B — Warn on every version, with no tripwire
Rejected. It gives up the one class that fails open, in exchange for a check that costs a file append
per shell call.

### Alternative C — Pin the CLI and disable auto-update
Removes the drift at the source. Rejected because the owner's interactive sessions share the
installation, and pinning them to the conductor's verified version trades one interruption for a
standing one.
