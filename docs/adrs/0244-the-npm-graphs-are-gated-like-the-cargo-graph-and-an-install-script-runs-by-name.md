# ADR-0244 — The npm graphs are gated like the cargo graph, and an install script runs by name

> **Status:** proposed
> **Date:** 2026-09-22
> **Related plan(s):** [0220](../plans/0220-the-dependencies-catch-up-and-npm-gets-its-gate.md)

## Context

The repository has three dependency graphs, and only one of them is watched. `Cargo.lock` has
`cargo deny check` in CI's `deny` job, where `deny.toml` holds every exception as an id with a written
reason. It is green today. The two npm projects, `studio/` and `site/`, have no gate at all, and no
bot either: nothing in `.github/` runs `npm audit`, and there is no Dependabot or Renovate
configuration. ADR-0178 pinned every studio dependency exactly and stopped there.

The result was measured on 2026-09-22. `npm audit` in `studio/` reported 20 advisories, 2 of them
critical: a vitest RCE, and `tar` path traversal pulled in by `electron-builder` 25. `electron` was
at 32.1.2, twelve majors behind 44.4.3, **inside a release artifact**, because the studio zip ships
Electron itself (ADR-0178, "Packaging, diverged"). No commit made this worse. Advisories were
published against pins that stayed still, and nothing was listening.

The same day showed a second, unrelated npm fact. npm 11 (11.19.1 on the Arch box) **blocks
dependency install scripts by default**, and runs one only when the project's `package.json` lists it
under `allowScripts`. `electron` fetches its binary in `postinstall`. So `npm ci` on that box
completed without error and left no Electron, and two studio test files failed with *"Electron failed
to install correctly"*. CI runs Node 22, whose npm 10 still runs every script, so CI did not see it.
Node 24, the current LTS that Plan 0220 moves CI to, ships npm 11.

Two questions here could each go either way. The first is **what watches the npm graphs**: a gate
that fails, a bot that proposes, or a person who remembers. The second is **which install scripts
run**: every one, as npm 10 did, or each by name.

## Decision

**CI gates both npm graphs on a schedule of its own, and it holds exceptions the way `deny.toml`
does.** A zero-dependency Node gate, `scripts/check-npm-audit.mjs`, runs `npm audit --json` in
`studio/` and in `site/` and applies two thresholds:

- **The shipped graph fails at `high`.** That is `studio/` with `--omit=dev`, the packages that
  reach a tester inside the studio zip.
- **The full graph fails at `critical`.** That is every package of both projects, dev and build
  tooling included, since a critical in a test runner or a packager still runs on a developer's
  machine and in CI.

Anything below a threshold is printed, never failed on. An exception lives in `npm-audit.allow.json`
at the repository root as an advisory's GHSA id plus a written reason, and the gate rejects an entry
without a reason. It also reports an entry whose advisory no longer appears, so the list cannot rot
silently the way an unread ignore does. The gate runs in its own CI job, beside `deny`. It stays out
of `.githooks/pre-push` and out of `scripts/gates.manifest.mjs`, because it needs the network and its
answer changes without a commit. ADR-0033 kept `cargo deny` out of the hook for the same reason.

**An install script runs only when its package is named.** Each npm project's `package.json` carries
an `allowScripts` field listing exactly the packages whose install scripts must run. For the studio
that is `electron` (its binary) and `esbuild` (its platform binary). `npm install-scripts approve
<pkg>` maintains the list. A new entry is a reviewed edit to a committed file, like a new
`deny.toml` ignore, and never `approve --all`.

## Consequences

### Positive
- The drift that reached 20 advisories and a twelve-major Electron gap can no longer grow unobserved.
  The first high advisory against a shipped package turns CI red.
- The two graphs are held to one rule: an id, a reason, reviewed in a diff. Anyone who can read
  `deny.toml` can read the allow file.
- A fresh clone on npm 11 installs a working Electron with no local step, and the list of code that
  runs at install time is written down rather than implied.
- Moving CI to Node 24, which ships npm 11, does not reproduce the Arch box's missing-binary failure.

### Negative
- **`main` can go red with no commit**, when an advisory is published against a pinned version.
  `cargo deny` already has this property, and it is the price of a gate that watches the world rather
  than the diff. The repair is always either a bump or an allow entry with a reason, and the job
  failing on its own names which package.
- `npm audit` depends on the registry's advisory endpoint. An outage fails the job. The gate treats a
  failed request as a failure and never as a pass.
- The `high` line on the shipped graph is a judgement, not a measurement. It is set where a
  high-severity finding in shipped Electron code is something a tester should not receive, and it
  will produce exceptions for advisories whose vector the studio does not expose. Each costs a
  reason.
- `allowScripts` is an npm 11 field. npm 10 ignores it and runs everything, so on Node 22 the list is
  documentation rather than enforcement. It becomes enforcement once every environment is on npm 11.

### Neutral
- Nothing about `site/`'s shape changes. Its whole graph is build tooling, so only the `critical` line
  applies to it.

## Alternatives considered

### Alternative A — A scheduled bump bot (Dependabot or Renovate)
A bot opens a pull request per outdated package, on a schedule. It lost because it answers a
different question, "what is newer", where the gap here was "what is dangerous". It also generates
pushes and pull requests from outside the session, in a project where only the owner pushes and every
change passes through a lane. A bot's majors would arrive as dozens of unordered PRs, where this
project plans an upgrade the way Plan 0220 does. Nothing prevents adding one later on top of the gate.

### Alternative B — `npm audit --audit-level=high` as a bare CI step, no script and no allow file
One line of YAML. It lost because `npm audit` has no exception mechanism. The first advisory the
studio cannot fix, such as one with no patched release or one that needs a vector the studio never
opens, would force a choice between disabling the step and lowering the level for everything. The
allow file with a mandatory reason is what keeps the gate standing after its first real exception.
The same argument is why `deny.toml` has `ignore` entries rather than `cargo deny` being run with
checks switched off.

### Alternative C — Run every install script (`--allow-scripts` / `approve --all`)
That restores npm 10's behaviour with one flag. It lost because npm 11's default is the one supply
chain control the studio gets for free: a compromised transitive dependency's `postinstall` is the
standard delivery route, and the studio needs exactly two scripts.

## Notes

The 2026-09-22 audit, the partial patch bump (`4581c6b5`) and the eslint/typescript-eslint
incompatibility that bump ran into are recorded in Plan 0220's Context.
