# ADR-0241 — Linux leads, and Windows stays a peer

> **Status:** accepted
> **Date:** 2026-09-20
> **Related plan(s):** [0218](../plans/0218-the-reference-machine-becomes-arch.md),
> [0120](../plans/done/0120-the-standalone-ships-on-ubuntu.md),
> [0214](../plans/0214-the-linux-arm-reports-back.md)
> **Related ADRs:** [0131](0131-the-linux-standalone-captures-through-pulseaudios-simple-api.md)
> (the PulseAudio capture arm), [0038](0038-tag-driven-release-unsigned-universal-mac-app.md)
> (what a `v*` tag ships), [0001](0001-rust-core-wgpu-cabi-foobar-shim.md) (the founding split)

## Context

This project was built on Windows and validated there. `docs/nfr.md` §9 says so in as many words —
its hardware matrix lists a primary Windows dev box, an older Windows PC for the performance floor,
and foobar2000 — and §2's platform baseline names two platforms. Every golden baseline in
`core/tests/golden/` was blessed on **WARP**, the DX12 software rasterizer, which exists only on
Windows. The release ships three zips and, once
[Plan 0120](../plans/done/0120-the-standalone-ships-on-ubuntu.md) lands, a Linux tarball beside them.

The owner is migrating the development machine to **Arch**. That is not the same event as "Linux
becomes a supported target", which 0120 and [0214](../plans/0214-the-linux-arm-reports-back.md)
already deliver. It is a change of *reference*: the machine that judges a picture, runs the gate
before a push, hears real audio through a real sound server, and drives a real projector stops
being a Windows box.

Two facts make this a decision rather than a consequence.

**A platform's standing here is a documentary fact, not a code fact.** Nothing in `core/` branches
on an operating system — that is ADR-0001's whole point — so "which platform leads" is carried
entirely by `docs/nfr.md`, `docs/on-device-validation.md`, `docs/developing.md`, the CI matrices and
this repository's habits. Those are the things that change, and if they do not change, nothing
does: the tree would ship on Linux while every document still told a reader to check it on Windows.

**Windows cannot simply be dropped.** The foobar2000 component is a released artifact
([ADR-0115](0115-the-foobar-component-is-a-released-artifact-with-a-parameterized-sdk.md)) and
foobar2000 has no Linux build; the Windows standalone zip is the most-downloaded artifact this
project has; and the release tarball must keep being built on `ubuntu-latest` whatever the dev box
runs, because glibc is forward-compatible only — a binary linked against Arch's glibc runs on
nothing older.

## Decision

**Linux leads and Windows stays a peer.** Linux — specifically an Arch box on the PipeWire stack —
becomes the machine the app is developed on, judged on, and hand-validated on. Windows keeps its CI
arm, its release zip, the foobar2000 component and its place in the platform baseline; what it
loses is its standing as *the* machine a claim is measured on.

Concretely, this ADR binds four things and nothing else:

1. `docs/nfr.md` §2 gains a Linux baseline row, and §9's hardware matrix names the Arch box as the
   primary and the Windows box as the peer it is checked against.
2. `docs/on-device-validation.md` gains a Linux column, and the checks only a real machine can run
   — live capture, a projector, a second display — are expected to be run there first.
3. `docs/developing.md` and `CLAUDE.md`'s machine-setup section carry the Arch dev loop beside the
   Windows one, including the fact that the MSVC linker override is Windows-only and has a
   different answer on Linux.
4. The software rasterizer the goldens are blessed on moves, which is its own decision and its own
   cost: [ADR-0242](0242-the-software-reference-rasterizer-is-lavapipe-and-a-warp-claim-is-re-measured.md).

**Release artifacts do not move.** The Linux tarball is still built on `ubuntu-latest` and the
Windows zip on `windows-latest`; a dev box is not a build host. Whether Arch gets a native package
— a `PKGBUILD`, an AUR entry — is deliberately left open, because `docs/nfr.md` §8 says no
installer and reversing that is a separate decision with its own rejected alternatives.

## Consequences

### Positive
- Every reading this project takes — a frame time, a picture, a capture verdict, a live-show
  rehearsal — starts being taken on the machine the owner actually uses, rather than inferred
  across a platform boundary.
- The Linux path stops being code nobody runs. ADR-0131 records that the third `cfg` arm was
  carried for the project's whole life without a compiler seeing it; a leading platform cannot
  decay that way, because the everyday loop compiles it.
- The macOS asymmetry gets a name. macOS has always been validated by a recipient rather than
  in-house (`docs/nfr.md` §9); with Linux leading and Windows a peer, that is one of three clearly
  ranked positions rather than an embarrassment in a footnote.

### Negative
- **Windows becomes the platform that decays**, and it is the one with the foobar2000 component
  behind it — the artifact CI already cannot test at all. The peer position is a promise to keep
  checking a machine that is no longer under the owner's hands every day, and promises of that
  shape are what `on-device-validation.md` exists to record precisely because they are not kept by
  themselves.
- **The four Windows-only surfaces become gaps rather than asymmetries.** Now-playing (SMTC),
  device enumeration, the Spout video-out and the foobar host all have no Linux counterpart today.
  Each is wanted, each is its own ADR, and until they land the leading platform is the one missing
  features the peer has.
- **A documentary change with no gate behind it is the kind this repository has already watched
  drift.** Nothing fails when `nfr.md` §9 goes stale. This ADR names the files rather than the
  habit for that reason, and even so, the files are prose.

### Neutral
- Wayland is now the display server that matters most, and it permits less than Win32 does — a
  client cannot place its own window on a chosen output. Whether that costs the operator console
  and the `D` display-cycle anything is unmeasured, and Plan 0218's first phase is a probe rather
  than a design.

## Alternatives considered

### Alternative A — Linux only, Windows dropped except the plugin
Rejected because the Windows standalone zip is a shipped artifact with users, and the marginal cost
of keeping its CI arm green is one matrix row. Dropping a working platform to simplify a table is
not a saving.

### Alternative B — Keep Windows as the reference and treat Linux as a supported target
This is what Plans 0120 and 0214 already deliver, and it is coherent: the goldens stay put, no
prose moves, and Linux is one more platform CI builds. Rejected because it would leave every
judgement — a blessed picture, a frame-time reading, a live rehearsal — on a machine the owner is
migrating away from, which is how a reference machine becomes a machine nobody has.

### Alternative C — Declare no reference platform at all
Per-platform baselines everywhere, every claim carried twice. Rejected as an answer to *this*
question: it is a real option for the goldens specifically, where it is considered and rejected on
its own terms in ADR-0242, but as a project stance it doubles the hand-validation surface while
naming no machine as the one that must be right.
