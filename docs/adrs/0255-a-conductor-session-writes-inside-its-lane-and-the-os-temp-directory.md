# ADR-0255 — A conductor session writes inside its lane and the OS temp directory

> **Status:** accepted 2026-09-29 (Plan 0234), with an `Outcome`
> **Date:** 2026-09-29
> **Related plan(s):** [0234](../plans/done/0234-a-conductor-session-writes-only-where-it-works.md)
> **Extends:** [ADR-0233](0233-a-session-allowlist-safety-claim-is-asserted-against-a-transcript.md)
> (a refusal is asserted against a transcript)

## Context

A conductor session runs headless under `tools/conductor/settings.conductor.json` with
`--permission-mode dontAsk`. The allowlist grants the bare `Write` and `Edit` tools, with no path.
[Plan 0208](../plans/done/0208-the-conductors-safety-claims-get-their-evidence.md) bounded
**deletion** to the lane and asserted that bound against a recorded CLI transcript (ADR-0233).
Nothing bounds **writing**.

On 2026-09-29 the CLI re-verification found a file the 2.1.282 probe had written in `~/Work`, the
directory above its worktree, under the conductor's own settings (backlog 0273). A session can
create or overwrite any file its user can. The README's statement that the lane is a session's bound
holds for `rm` and `Remove-Item`, and not for the tool a session uses most.

The owner chose the bound on 2026-09-29: the lane, plus the OS temp directory, which the build tools
and the sidecar stage files in.

## Decision

We will replace the bare `Write` and `Edit` grants with path-scoped ones that cover the session's
working directory, which the conductor sets to the lane, and the OS temp directory, and we will
leave every other path to dontAsk's default refusal. **Which rule spellings the CLI honours is
decided by a probe, not by this ADR**: ADR-0233's rule that a refusal is asserted against a recorded
transcript applies to writes exactly as it applied to deletions. `spike/matcher-probe.mjs` gains write
shapes (inside the lane, relative and absolute; the temp directory; the lane's parent; the home
directory) and runs under the candidate file. The spellings that the transcript shows grant the
lane and the temp directory, and nothing else, are the ones that ship.

## Consequences

**Positive.**

- The README's "the lane is the bound" becomes true for writing as well as for deletion, and it is
  asserted against the real CLI rather than a model.
- A mangled path, like the backslash that produced backlog 0273's file, is refused instead of
  landing beside the worktree.

**Negative.**

- A session that has a real reason to write elsewhere is refused and parks. None is known today.
  The conductor's own state lives in the main checkout, but the conductor process writes it, not a
  session.
- The rule spellings rest on CLI behaviour that a later version can change. The probe row carries
  its version, and ADR-0208's version check is the prompt to re-run it.
- If no spelling honours a relative working-directory path, the fallback is a per-lane settings file
  the conductor writes at session start, which is more machinery. The plan says so, and the probe
  decides.

## Alternatives considered

### Alternative A — the lane only

This is the tightest bound. It lost because the Rust and Node toolchains and the sd-filter sidecar
stage files under the OS temp directory, and the owner judged that refusing those buys nothing a
lane bound does not already give.

### Alternative B — record the gap and do not bound it

This costs nothing. The README would say plainly that `Write` is unbounded. It lost because the gap
already produced a write outside a lane, through an ordinary malformed path rather than anything
adversarial, and the deletion bound beside it makes the asymmetry look like an oversight.

### Alternative C — a hook that checks every write's path

A `PreToolUse` hook could resolve each `Write` or `Edit` path and deny any outside the lane. It would
not depend on the permission matcher's spellings. It lost as the first move because the settings file
is where every other bound lives and is what the transcript tests. It stays the fallback if the probe
shows the matcher cannot express the bound.

## Outcome (2026-09-29, Plan 0234)

**The bound that shipped has a third grant, and the Decision's "and nothing else" is not what
shipped.** `settings.conductor.json` grants `Write` and `Edit` on `./**` (the lane), `//tmp/**` (the
Linux temp directory) and `/state/reviews/**`, which the CLI resolves against the settings file's own
directory, so it names the running checkout's `tools/conductor/state/reviews/`.

**The Negative bullet "None is known today" was falsified by the probe.** A review session writes its
review into that directory, and a close session reads it there; neither is the conductor process. On
2.1.283 the probe's Candidate A row (`./**` plus `//tmp/**`) refused the reviews write even though the
session was handed the directory with `--add-dir`: `--add-dir` grants nothing on its own. Candidate B
added `/state/reviews/**` and wrote it. The table is in `tools/conductor/spike/README.md`. **Do not
delete the reviews grant**: without it every review and close session is refused its own review file.

**The price of that grant.** One settings file serves every session kind, so a `dev` implement or fix
session may also create or overwrite any plan's review file under `state/reviews/`, including an
earlier round's review that the next review reads as prior findings. It is accepted: the directory
holds only reviews, and the conductor, not a review file, decides a verdict. A per-kind settings file
is the only structural repair, and it is not worth one today.

**The temp-directory grant is Linux-only.** macOS's `$TMPDIR` and Windows' `%TEMP%` are unprobed and
owed, beside backlog 0267.
