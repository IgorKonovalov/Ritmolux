# ADR-0203 — A release tag is annotated, and what gets checked is whether origin has it

> **Status:** accepted 2026-09-14 (Plan 0176)
> **Date:** 2026-09-14
> **Related plan(s):** [0176](../plans/done/0176-a-release-tag-reaches-origin.md)
> **Extends:** [0038](0038-tag-driven-release-unsigned-universal-mac-app.md) (tag-driven release),
> [0005](0005-versioning-and-release-cadence.md) (one version bump per plan, at the close)

## Context

A release exists only if its `vX.Y.Z` tag reaches `origin`: pushing the tag is the one event that
starts `release.yml` (ADR-0038). Plan 0165's review counted the gap as the release path's main way of
failing, and backlog 0196 could not explain it: of 24 tags in the `v0.93.0`-`v0.113.0` window, 19
produced no Release run and no error.

**The 2026-09-14 plan sweep measured two causes. Neither is on GitHub's side.**

1. **The tags never left the machine when they were made.** `ci.yml` runs on `push` with no filter,
   so every tag push that GitHub accepts as an event starts a CI run named after the tag. The CI run
   list (`gh run list --workflow=ci.yml`, full history back to 2026-07-21) and the Release run list
   match exactly. Every tag with a Release run has a CI run started in the same second, and **none of
   the silent tags has a CI run**. They were not suppressed and did not fail. No push event carried
   them. They reached `origin` only when the 2026-09-10 history rewrite force-pushed 131 tags in
   one command, and bulk pushes like that fire no workflows (Plan 0165 Phase 3).
2. **The close ceremony writes lightweight tags, and `git push --follow-tags` skips a lightweight
   tag.** `cargo release` writes an annotated tag (`chore: Release vX.Y.Z`). The studio version-sync
   step then moves it with `git tag -d vX.Y.Z && git tag vX.Y.Z`. The manual bump used when a
   parallel lane blocks `cargo release` ends in `git tag vX.Y.Z` too. Both write lightweight tags. On
   2026-09-14, ten `v*` tags existed locally and not on `origin`: `v0.115.0`-`v0.120.0` and
   `v0.122.0`-`v0.123.0`. Eight are lightweight.

**The two causes are not the same, and the second does not explain all of the first.** `v0.118.0`
and `v0.119.0` are annotated, their commits are on `origin/main`, and they are still missing. In the
older window, most of the silent tags are annotated today as well (15 of 19). So annotating tags is
necessary, because the documented push command cannot carry a lightweight tag. It is not sufficient,
because the push that was actually run did not carry annotated tags either. One caveat: the older
window's tag objects went through the rewrite, and nobody has checked that the rewrite kept each
tag's type. The recent ten did not go through it.

What fails is one property: **the version `main` declares has a tag on `origin`.** Every recorded
failure breaks that property silently. Nothing reads it, and nobody watches for something being
absent.

## Decision

We will make every release tag the close ceremony writes an **annotated** tag, and check the property
itself, not a habit that is supposed to produce it:

1. **Annotated everywhere.** Both places in the ceremony that write a tag by hand write
   `git tag -a [-f] vX.Y.Z -m "chore: Release vX.Y.Z"`, which is the message `cargo release` writes.
   The documented push is `git push --follow-tags origin main`.
2. **One gate script, two readings.** `scripts/check-release-tag.mjs` reads
   `[workspace.package].version` from the root `Cargo.toml`.
   - **Offline**, in the pre-push hook: `refs/tags/v<version>` must exist locally, be an annotated
     tag object, and point at an ancestor of `HEAD`.
   - **`--remote`**, in CI's `links` job, only on a push to `refs/heads/main`: `origin` must advertise
     the tag and its peeled `^{}` line, which exists only for an annotated tag. It polls for a
     bounded time, so a tag pushed in a second command straight after `main` is not a false red.
3. **The gate proves it can fail.** `--self-test` builds a throwaway repository and asserts
   exit 1 on a missing tag, exit 1 on a lightweight tag, and exit 0 on an annotated one. Its sibling
   gates already do the same (backlog 0104's lesson).

A published release for the tag is **not** checked. The release job takes minutes, so any check of it
races, and a missing tag already explains every failure recorded so far.

## Consequences

### Positive

- **The failure stops being silent where it cannot be bypassed.** A push of `main` whose version has
  no annotated tag on `origin` turns `main`'s CI red, and the message names the command that fixes
  it. Before this, 105 of 132 tags went unpublished and nobody read it as a fault.
- **Caught before the push as well.** The offline reading catches a lightweight or missing tag on the
  developer's machine, which is where both causes happen.
- **One property covers both causes.** The gate does not need to know whether a tag was lightweight
  or the push command left it behind. It checks the result.

### Negative

- **Only the version at the tip is checked.** Suppose two plans close and bump before one push, and
  only the newer tag is pushed. The older version is never checked and ships no release. It is the
  same class of failure at a smaller scale, and nothing catches it.
- **`main`'s CI can go red for a reason unrelated to code.** A maintainer who pushes `main` and holds
  the tag back on purpose ("If a close should *not* publish, do not push the tag", in
  `docs/releasing.md`) gets a red run. That choice now needs a documented escape: push the tag, or
  accept the red run until the next bump.
- **Tags must be repaired before any push.** From the day the offline reading lands, a lightweight
  tag at the current version refuses every push until it is re-created as annotated. Plan 0176
  orders its phases so the repair comes first, but a lane that bumps in a worktree meets the gate at
  its first push.
- **A network call in CI.** `git ls-remote` against `origin` is one more dependency in the `links`
  job, which has none today.

## Alternatives considered

### Alternative A — Set `push.followTags` and document it

**Rejected because the setting is per clone and still skips lightweight tags**, and because
`v0.118.0`/`v0.119.0` show that an annotated tag was also left behind by whatever push was run. It is
a convenience layered on the gate, not the defence.

### Alternative B — A scheduled workflow that reconciles tags against releases

**Rejected as the defence because it can only see tags that reached `origin`.** All ten stranded tags
on 2026-09-14 never left the machine, and every silent tag in the older window produced no push event
at all. A reconciler would have reported nothing for either set.

### Alternative C — Hand the user an explicit refspec at every close (`git push origin main vX.Y.Z`)

**Rejected because it is the same discipline, done by hand, that already failed.** The commands handed
over at a close were never recorded, and the evidence shows they did not reliably include the tag.
The gate does not depend on which command is run.

### Alternative D — CI creates the tag when `main`'s version moves

**Rejected because it moves the tag's authority out of the close ceremony**, where ADR-0005 places the
version bump. It needs `contents: write` on every `main` push, and it would tag whatever commit
happened to be pushed rather than the studio-sync commit the tag must sit on. That is a supersession
of ADR-0038's tag-driven shape, and nothing here justifies it.

## Notes

The two instruments behind the Context, runnable again:

```sh
git for-each-ref refs/tags/v* --format='%(refname:short) %(objecttype)'        # tag types
git ls-remote --tags origin                                                   # what origin carries
gh run list --workflow=ci.yml --limit 1000 --json headBranch,createdAt \
  --jq '.[] | select(.headBranch | startswith("v")) | "\(.headBranch) \(.createdAt)"'
```
