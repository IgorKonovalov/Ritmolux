# Developing

How to build the project from a checkout and what the local gate runs before a push. This is the
contributor's page; if you are looking for what the application does, start at
[Running the app](running.md).

## Building

A recent stable **Rust** toolchain (the workspace is edition 2024 — Rust 1.85+) and, for the
documentation gates, **Node**. From the repo root:

```sh
cargo build                                          # the everyday build
cargo run -p standalone --release                    # the app
cargo nextest run --workspace                        # the whole suite
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

`--workspace` is load-bearing on the test and lint rows rather than a flourish: the C ABI crate sits
outside the workspace's default members, so a bare `cargo nextest run` skips the ABI conformance
suite and a bare `cargo clippy` stops linting the C ABI, and both come back green while covering
nothing.

The headless capture CLI and the visual-QA harness have their own page,
[Headless capture and video](capturing.md).

## The pre-push gate

A checked-in `.githooks/pre-push` runs the fast subset of CI before a push, so a
broken push costs seconds locally instead of minutes in CI. It is **opt-in per
clone** — enable it with:

```sh
git config core.hooksPath .githooks
```

> **An uninstalled clone has no gate.** Git will not run a hook from a tracked
> directory without that config, so until you set it nothing below happens. There
> is deliberately no auto-install ([ADR-0033](adrs/0033-testing-strategy-coverage-ratchet-and-pre-push-gate.md)
> Alternative H).

What it runs, stopping at the first failure and naming the step that failed:

| Step | Command |
|------|---------|
| Doc links | `node scripts/check-doc-links.mjs` |
| Index rows | `node scripts/check-index-rows.mjs` |
| Index rows (self-test) | `node scripts/check-index-rows.mjs --self-test` |
| Backlog claims | `node scripts/check-backlog-claims.mjs` |
| Filter figures | `node scripts/check-filter-figures.mjs` |
| Comment hygiene | `node scripts/check-comment-hygiene.mjs` |
| Contents blocks | `node scripts/toc.mjs --check` |
| Contents blocks (self-test) | `node scripts/toc.mjs --self-test` |
| Reader prose | `node scripts/check-reader-prose.mjs` |
| Format | `cargo fmt --all --check` |
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` |
| Tests | `cargo nextest run --workspace -P fast` (narrowed — see below) |

The Node steps come first because they are the cheapest (tens of milliseconds
between them): every relative markdown link in the repo must resolve, every row
inside a marked roster region must stay under 320 bytes
([ADR-0116](adrs/0116-an-index-row-is-a-pointer-and-a-gate-holds-it-to-one.md)),
every live `design-backlog.md` entry must carry a probe that still holds
([ADR-0108](adrs/0108-a-backlog-claim-about-the-repo-carries-an-executable-probe.md)),
the diffusion filter's cost figures must live in one file
([ADR-0122](adrs/0122-a-sidecar-tool-documents-itself-in-one-place.md)), no `.rs`
comment may carry a relative link or plan-relative narration
([ADR-0127](adrs/0127-a-comment-carries-the-mechanism-and-the-decision-record-stays-in-docs.md)),
every citation in a reader document must sit inside a link
([ADR-0168](adrs/0168-the-reader-documents-address-a-reader-and-the-record-stays-a-link.md)),
and every generated contents block must still match the headings beneath it
([ADR-0163](adrs/0163-a-long-document-carries-a-generated-contents-block.md)).
The two that could go green on a rule that had quietly stopped working — a roster
detector matching nothing, an anchor rule that is merely plausible — carry a
`--self-test` beside their check.
If `node` is not on your `PATH` they all **skip with a notice** rather than
failing the push; nothing else here needs Node. That skip is about the hook only
— CI's `links` job runs the same checks on `ubuntu-latest`, where they cannot
skip and are not bypassable.

**Measured warm wall time: ~48.6 s** (2026-08-08; dominated by the tests — `fmt`
and `clippy` are under two seconds between them). The hook excludes the nine
GPU-heavy suites that iterate every shipped preset or scene through a real
adapter. **Which nine is not written here** — since
[ADR-0156](adrs/0156-the-per-phase-gate-is-scoped-and-the-suite-is-owed-once-per-plan.md)
the list is the `fast` profile's `default-filter` in `.config/nextest.toml`, and the hook and CI's
`check` job both cite `-P fast` rather than restating it.
The test runner **names the skipped binaries itself** on every run, on the profile's
authority, so the narrowing is never silent, and **CI runs
all of them regardless** — though since [ADR-0073](adrs/0073-the-windows-ci-critical-path.md)
it runs those nine in the `coverage` job alone rather than in two Windows jobs, so
the promise is now underwritten by one job instead of a redundancy between two.

`cargo deny`, doctests, Miri, and the coverage job are deliberately *not* in the
hook — they push it into minutes, and a gate that hurts gets disabled
([ADR-0033](adrs/0033-testing-strategy-coverage-ratchet-and-pre-push-gate.md) Alternative F).
They are also the checks least likely to break from a local edit.

Bypass once with `git push --no-verify`.

## What else there is to read

- [Testing and visual QA](testing.md) — the `core/tests/` harness that hard-tests every preset for
  reactivity, animation, shape sanity and beat response.
- [Headless capture and video](capturing.md) — the `shot` CLI, `--render` and the live video-out.
- [MilkDrop conversion](milkdrop-conversion.md) — reading what `milkconv` produced.
- [Embedding the core](embedding.md) — putting the engine inside another application.
- [On-device validation](on-device-validation.md) — the manual checklist for what CI cannot run:
  real GPUs, live loopback, installing the foobar2000 component.
- [Releasing](releasing.md) — how the version moves and what a `v*` tag builds.
- [Non-functional requirements](nfr.md) — the quantified budgets behind every "lightweight" and
  "real-time" claim.
- [The architecture decisions](adrs/) — start with
  [ADR-0001](adrs/0001-rust-core-wgpu-cabi-foobar-shim.md), the founding decision, with the rejected
  alternatives recorded.
