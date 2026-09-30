# ADR-0254 — The release artifact count runs on every release run, and proves it refuses

> **Status:** proposed
> **Date:** 2026-09-28
> **Related plan(s):** [0214](../plans/0214-the-linux-arm-reports-back.md)
> **Amends:** [ADR-0038](0038-tag-driven-release-unsigned-universal-mac-app.md) (where the release's
> per-kind count runs)

## Context

`release.yml` publishes a prerelease only when all six build jobs finish, and before it publishes it
counts what it is about to ship: **exactly 5 `.zip` and 1 `.tar.gz`**. Plan 0120 made that count
per kind, because counting zips alone would pass a release that carries nothing for Linux. That count
is the only thing that stops a silently skipped job from shipping a short release.

The count is a bash block inside the `release` job's publish step, and that job is gated
`if: github.event_name == 'push' && startsWith(github.ref, 'refs/tags/v')`, so that a
`workflow_dispatch` never publishes (backlog 0194). The consequence nobody wrote down is that **a dry
run never runs the count either.** [Plan 0214](../plans/0214-the-linux-arm-reports-back.md) Phase 3
asked a dispatch to exercise it, and dispatch run 36471874865 on 2026-09-28 built all six artifacts,
published nothing, and skipped the job the count lives in. That done-when clause cannot be met.

What the count has shown so far is only that it **passes** a correct release: every tag since
`v0.148.0` went through it with six artifacts. No run has ever shown it **refusing** a short one,
which is the only property it exists for. A guard seen passing and never seen refusing is
indistinguishable from `true`.

## Decision

We will move the per-kind count out of the publish step into `scripts/check-release-assets.mjs`,
run it in a new `verify` job that needs the six builds and runs on **both** a tag push and a
dispatch, and make `release` need `verify`. The script carries `--self-test`, which runs it over
seeded asset directories: one short a tarball, one short a zip, one correct. It must refuse the
first two and pass the third. The self-test joins the gate roster in `scripts/gates.manifest.mjs`,
so every push and every pre-push proves the refusal, independently of any release run.

A dry run then exercises the same count a tag push does, on the real artifacts. The refusal is
proven by the self-test, and the pass is proven by every release run.

## Consequences

**Positive.**

- A dispatch rehearsal checks everything a tag push checks except the publish itself, which is
  what "rehearsal" was always taken to mean.
- The refusal half is asserted on every push, where before it had never been observed at all.
- The count lives in one script both events call, rather than in shell text inside a YAML string
  that no test can reach.

**Negative.**

- One more job per release run: an `ubuntu-latest` runner that downloads about 430 MB of artifacts.
  It adds a minute or two to a run that already takes about twenty.
- One more entry in the gate roster, and a script whose only production caller is a CI job. The
  pre-push hook gains a step that runs in well under a second.
- `release` now depends on `verify` as well as on the six builds, so a bug in the script blocks a
  release. That is the intended direction: a count that cannot run should stop a publish.
- Changing `.github/workflows/` needs the `workflow` OAuth scope on the pusher's credential, as
  every edit to that directory does.

## Alternatives considered

### Alternative A — reword Plan 0214's clause and leave the count where it is

The done-when would accept that the count runs only on tag pushes, and cite `v0.148.0`-`v0.151.0`
as evidence. It is free. It lost because it certifies the half that was never in doubt. Six-artifact
releases show the count passes a correct release, and nothing would ever show it refusing a wrong
one.

### Alternative B — a `verify` job on both events, with the count left as inline bash

A dry run would then exercise the count on real artifacts. It lost for the same reason as A, one
step later: a dry run with six correct artifacts shows a pass. Showing a refusal needs a short
release, and the only way to get one from a real run is to break a build job on purpose. A seeded
fixture gets the same evidence on every push, at no cost.

### Alternative C — seed a short release in a real dispatch

A dispatch input could drop one artifact before the count, so a rehearsal proves the refusal on the
real job. It lost because it adds a code path to the release workflow whose only purpose is to make
the release fail, and it proves the property only when someone remembers to run it that way.
