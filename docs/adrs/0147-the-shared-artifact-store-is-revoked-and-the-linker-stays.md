# ADR-0147 — The shared artifact store is revoked, and the linker stays

> **Status:** proposed
> **Date:** 2026-08-29
> **Supersedes in part:** [ADR-0141](0141-one-artifact-store-serves-every-lane.md)
> **Related plan(s):** [0134](../plans/0134-the-lanes-stop-sharing-a-store.md)

## Context

[ADR-0141](0141-one-artifact-store-serves-every-lane.md) pointed every worktree of this repository at
one artifact store — `WORK/.lmv-target` — through a machine-local `WORK/.cargo/config.toml`, and in
the same file pointed the MSVC linker at the toolchain's bundled `rust-lld`. It was accepted on
2026-08-29 with a measured `Outcome`: a never-built worktree compiles 3 workspace crates in 15-24 s
with zero dependencies recompiled, against 129 crates in 105 s cold, and the linker took the cold
path to every test binary from 171 s to 145 s while moving no golden.

It named three hazards, all of them costs: `cargo clean` in one lane wipes the store for every lane,
the store grows monotonically, and two lanes building at once serialize on cargo's lock. **There is a
fourth, it was found the same day, and it is not a cost.** Plan 0115's lane hit
`no method named open_tap found for struct Renderer` in `standalone/tests/frame_tap_memory.rs` while
`core/tests/frame_tap.rs` passed in the same worktree against the same committed source. The verbose
build showed the test linking `liblmv_core-3841a2a685bea7be.rlib` and cargo treating it as fresh;
that archive contains neither `open_tap` nor `render_tapped`, while the one the lane built does. The
lane was compiling against source it does not contain.

**The mechanism is that the worktree path is not in cargo's fingerprint at all**, and that is read
from the store rather than inferred. Across all eight `lmv-core` library fingerprints in
`.lmv-target/debug/.fingerprint/`, the `path` field holds exactly **one** value
(`2995440285629810167`); across all six build-script compile units, exactly one
(`12501519184093212959`) — and one of those units is provably from the Plan 0115 worktree, since its
build-script `output` records rerun paths under `WORK/lmv-plan-0115`. Cargo hashes the package's
path *within* the workspace, so two worktrees with the same layout, the same features and the same
resolved dependency graph produce **the same unit hash**, address the same fingerprint directory and
the same output filename. Freshness is then decided by resolving the dep-info's **relative** source
paths against whichever package root is building and comparing mtimes — so a lane whose files are
older than the recorded build time is told its neighbour's artifact is up to date.

**Whether the build script collides in the same way is unresolved, and this ADR does not rest on
it.** The `debug` tree held seven build-script output directories, all carrying the same 57
generated entries, none containing `lsystem_bower` — a preset that exists only in the Plan 0104
worktree, which holds **72 presets against main's 54**. That was first read as the content half of
the same collision. **It does not support that reading.** The Plan 0104 lane reports that the
`release` tree held **two distinct** generated preset lists at the same moment — one of 72 presets
with `warp_wellhead`, `lsystem_coral` and `star_zellij` in it, one of 54 — so the build script did
run per lane there and its output directory was keyed per lane. Seven identical `debug` outputs are
equally explained by the fact that the only two checkouts that built in `debug` were `main` and the
Plan 0115 lane, which carry the *same* 54 presets; an absence is not a collision. The distinction
the counter-evidence turns on is real and was elided: **compiling `build.rs` and running it are
separate units with separate fingerprints**, and it is the run's output that decides the embedded
set. Neither reading can now be re-checked, because the store was deleted before the question was
settled. **Nothing below depends on the answer** — the library collision is directly evidenced and
is sufficient on its own.

The distinction that decides this ADR is between the two failures. Here it failed **loudly**,
because a method was missing and the compile broke. The dangerous case is two lanes whose cores
differ only in behaviour: the suite goes green, the goldens hold, and nothing anywhere says which
source produced the result. This project's entire verification story — 40 integration binaries, a
golden suite, a coverage ratchet, ADR-0071's insistence that a number name its machine — rests on
the premise that a test ran the code in front of you. That premise is what the store removes.

Against that, the store's measured benefit is the one ADR-0141's own `Outcome` reports rather than
the one its Context argued: not a 3x multiplier and not *"a small change is taking hours"* — the
warm one-file rebuild was 28 s — but **105 s of cold build once per new lane**, and roughly one
worktree's worth of disk per live lane.

**The linker is a separate change that happened to ship in the same file.** It has no bearing on
fingerprinting, no cross-lane surface at all, and it delivered 171 s → 145 s while moving no golden.
Nothing about it is implicated.

## Decision

**Every worktree compiles into its own `target/` again.** The `[build] target-dir` redirect is
removed from the machine-local `WORK/.cargo/config.toml`; the
`[target.x86_64-pc-windows-msvc] linker = "rust-lld.exe"` block **stays**. The file remains
machine-local, uncommitted, above the worktrees, and inert when absent, so a lane still needs no
setup of its own and the property ADR-0141 valued there survives.

This supersedes ADR-0141's Decision **in part**: the shared store is revoked and the linker is kept.
Everything ADR-0141 says about `rust-lld` — including the `Outcome` that measured it — stands, as
does its amendment of [ADR-0053](0053-plan-lanes-run-in-git-worktrees.md) on the linker's account.
The disk Negative it discharged from ADR-0053 comes back with it: **ADR-0053's *"Disk cost is severe
and recurring"* is live again**, and a finished lane's worktree must be removed, as ADR-0053 already
prescribes.

The existing `WORK/.lmv-target` is deleted rather than left in place. It holds artifacts of unknown
provenance and nothing can attribute them after the fact.

## Consequences

### Positive
- **A lane compiles what it contains.** The property is restored by construction rather than by a
  check, which matters because no check was possible: cargo offers no hook that could have caught
  this, and the loud instance was found only because a method happened to be missing.
- **The question of what a lane embeds stops being open.** Whether the build script ever served one
  lane another's preset set is unresolved above and is now unanswerable; with no shared store the
  question cannot arise again. Plan 0104's rebuild after the revocation embeds its own 72.
- **Measurements become attributable again.** Plan 0115's `dev` session discarded a full-suite run
  rather than report it, because it could not be known which core it ran against. That is the right
  call and it is not a cost anyone should have to pay.
- **`cargo clean` stops being a shared destructive act**, and the store stops growing monotonically.
  Two of ADR-0141's three named hazards go with the fourth.
- **Lanes build in parallel again** without serializing on one cargo lock — ADR-0053's positive,
  which ADR-0141 knowingly sold.
- **The linker win is kept in full**, at no risk, because it never had a cross-lane surface.

### Negative
- **The cold build comes back, at 105 s per new lane**, measured by Plan 0129 Phase 1. That is the
  whole of what this gives up, and it is the correct price for the property above.
- **Disk multiplies again**, at roughly 7-15 GB per live lane by the same measurements. ADR-0053's
  *"the disk reached zero bytes mid-session, breaking a build"* becomes reachable once more, and the
  defence is its own prescription — remove a finished lane's worktree — which is discipline and not
  a gate.
- **ADR-0141 was accepted this morning and is superseded in part this evening.** The record shows a
  decision reversed inside a day. That is the honest shape of it: the store was argued and measured
  on build time, correctness was never in the frame, and the fourth hazard was found by the next
  lane to use it.
- **Two things in one config file were coupled, and one of them was wrong.** The linker had to be
  disentangled from the store after the fact, which is only cheap because it happened to be
  separable.

### Neutral
- CI is untouched, as it was under ADR-0141, because the file is not committed.
- The committed scripts that resolve cargo output under `<repo>/target` —
  `plugin-foobar/build.ps1`, `packaging/macos/bundle.sh`, the two `renders/plan-0106-p*/run.sh`, and
  `standalone/tests/shot_cli.rs`'s `scratch()` — become correct again by accident rather than by
  repair. Backlog **0160** and **0161** record them and stay live: they are wrong about a redirect
  that could return, and asking `cargo metadata` is right either way.

## Alternatives considered

### Alternative A — keep the store and add a detector
Have `core/build.rs` record its own `CARGO_MANIFEST_DIR` into the generated source, and a test assert
it matches the tree the test is running from. It would have caught this instance and it is cheap.
Rejected because it is a detector for a defect the build should not be able to have, it only fires
when the build script re-runs, and the test binary is subject to the identical collision — so the
instrument shares the failure mode of the thing it measures. A gate that can be served a stale copy
of itself is not a gate.

### Alternative B — per-lane `target/` with `sccache` underneath
ADR-0141's own Alternative A, revisited on changed grounds. `sccache` is content-addressed, so two
lanes with different source get different cache entries and correctness is structural rather than
disciplinary — and it recovers most of the warm start this ADR gives up. It lost on scope rather
than on merit: it is a tool to install, pin and keep working, it wants `CARGO_INCREMENTAL=0` (trading
the inner-loop speed the user also named), and the cost it would recover is 105 s per new lane. It
stays the answer if the cold build turns out to hurt, and it is the *only* alternative here that
could give back the warm start without giving back the defect.

### Alternative C — a lane-unique value in `RUSTFLAGS`
A `--cfg lane="0115"` per worktree would make every unit hash distinct and the collision impossible.
Rejected because `RUSTFLAGS` applies to dependencies as well, so every lane rebuilds all 129 crates
anyway — it is Alternative-free: the same cold build as this decision, plus a per-lane setup step
and a non-obvious mechanism. It buys nothing over simply removing the redirect.

### Alternative D — keep the store and serialize lanes by discipline
The collision needs two lanes; work one lane at a time and it cannot happen. Rejected because it is
not what went wrong. Plan 0115's lane was served an artifact built by a session that was not running
concurrently with it — the store persists, so the hazard is *sequential*, not concurrent, and no
discipline about when lanes run touches it.

## Notes

The fingerprint evidence in Context was read directly out of `WORK/.lmv-target/debug/.fingerprint/`
and `debug/build/*/output` on 2026-08-29, before the store was deleted — and the deletion is why the
build-script question above stays open. **Destroying the artifact was the same act as destroying the
evidence about it**, which is worth one line here: the store was removed for being untrustworthy,
and that removed the only material that could have characterised how untrustworthy it was. The single-valued `path`
field is the whole finding in one number, and it is worth repeating that this is a property of cargo
and not a misconfiguration: nothing in `WORK/.cargo/config.toml` could have made two worktrees
distinguishable to it.

Nothing here reopens ADR-0053. Worktrees stay, the merge direction stays, the five-step close stays,
and the shared-stash and `git worktree remove` hazards stay. This ADR changes only what a worktree
compiles into, which is the same scope ADR-0141 claimed.
