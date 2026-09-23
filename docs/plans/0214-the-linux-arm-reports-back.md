# 0214 — The Linux arm reports back

> **Status:** approved
> **Created:** 2026-09-20
> **Approved:** 2026-09-20 (user)
> **Owner skill(s):** human, dev
> **Related ADRs:** [0131](../adrs/0131-the-linux-standalone-captures-through-pulseaudios-simple-api.md),
> [0016](../adrs/0016-gpu-tests-opt-in-ci-scope.md),
> [0038](../adrs/0038-tag-driven-release-unsigned-universal-mac-app.md),
> [0203](../adrs/0203-a-release-tag-is-annotated-and-origin-is-what-is-checked.md)
> **Depends on:** [0120](done/0120-the-standalone-ships-on-ubuntu.md) — every phase landed and on `main`

> **Amended 2026-09-22 (architect).** [Plan 0219](done/0219-the-arch-box-builds-tests-and-runs-every-lane.md)
> runs 0120 on an Arch box, so the **local** compile and test readings are taken there and live in
> 0120's log. Phases 1-3 of this plan are unchanged. They read GitHub's runners, which no local box
> can stand in for. Phase 4 still runs the tarball on the **Ubuntu** box, because that is the
> distribution the tarball is built for. The Arch box's live run is 0219 Phase 7 and 0218 Phase 6.

## TL;DR

Plan 0120 writes all the Linux code and **witnesses none of it**. This plan is the witnessing: the
owner pushes, and the three readings that only exist after a push get read and recorded — the
`ubuntu-latest` arm's six steps, the wgpu adapter it resolves, and a `workflow_dispatch` dry run's
artifacts — then the tarball runs on the real machine.

It is a short plan with an unusual shape: **three of its four phases are `human`**, because the
evidence lives on GitHub's runners and on a box in the room, and neither is reachable from a
session. `dev` appears once, between two of them, to repair what the first reading reports.

## Context & problem

The split is [0120](done/0120-the-standalone-ships-on-ubuntu.md)'s 2026-09-20 amendment, and the reason
is mechanical rather than editorial: **the conductor never pushes, and a Windows session cannot run
a Linux CI arm.** Every remaining 0120 done-when phrased as *"the arm runs green"* would therefore
stop a lane that had already written the code correctly — the work would be done and the phase
unclosable.

So 0120 was re-cut along *who can witness the evidence*. It now ends at the last line of code it
writes. What came here is every clause that needs a push or the machine, quoted rather than
paraphrased, so nothing was softened on the way across.

**The consequence this plan exists to handle is that 0120 closes with Linux code nobody has
compiled.** `#[cfg(target_os = "linux")]` code is not type-checked on Windows, and the one partial
substitute — `cargo check --target x86_64-unknown-linux-gnu` — stops reaching once `libpulse-sys`
enters the graph, because `cargo check` runs build scripts and that one needs `pkg-config` and
libpulse headers. This repository already carries one standing finding of exactly that shape
(`plugin-foobar/viz_session.cpp`, first compiled by a release job), so the class is known and the
cost is known. **Phase 1 is where that first compile happens**, and it should be expected to fail
the first time rather than treated as a surprise.

## Decision

**Read, repair, read again, then run it on the box.** The phases are ordered by what each one can
see, and each hands the next a reading rather than a belief:

```mermaid
flowchart TD
  subgraph off["Off this machine"]
    ci["CI: ubuntu-latest arm<br/>six steps + the adapter it resolves"]
    disp["release.yml workflow_dispatch<br/>six artifacts, publishes nothing"]
    box["The Ubuntu box<br/>26.04, RTX 3060, PipeWire"]
  end
  subgraph here["What a session can do"]
    fix["dev: repair what the log reports"]
    log["The plan's Implementation log<br/>every reading, pass or fail"]
  end
  push["The owner pushes"] --> ci
  ci -->|"Phase 1"| log
  log --> fix
  fix -->|"Phase 2, then another push"| ci
  push --> disp
  disp -->|"Phase 3"| log
  disp --> box
  box -->|"Phase 4"| log
```

**A failure at any phase is a recorded finding, not an improvisation.** Phases 1, 3 and 4 report;
only Phase 2 changes code.

## Implementation phases

### Phase 1 — The arm reports what it sees

- **Owner skill:** human

Push `main` so CI runs the arm 0120 Phase 2 declared, then read the `check (ubuntu-latest)` job and
write what it says into this plan's `## Implementation log`. **This is the first time any machine
compiles the Linux-only code**, so a red arm here is the expected case and the phase's product
either way.

Three of 0120's retired clauses land here verbatim:

> The `ubuntu-latest` arm runs all six existing steps green — `cargo build`,
> `cargo nextest run --workspace -P fast`, `cargo test --workspace --doc`,
> `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`, and
> `cargo doc --workspace --no-deps` under `RUSTDOCFLAGS: -D warnings`.

> The implementation log records **which wgpu adapter, if any, resolves on the runner**, and whether
> any GPU-touching test actually executed there rather than skipping. `ubuntu-latest` may ship
> Mesa's software Vulkan, in which case tests that skip on macOS for want of an adapter will *run*
> on Linux — against a rasterizer nothing in this repo has ever compared against. If any did run,
> name them.

> A run of `help_cli` and `stream_pipe` on the Ubuntu arm leaves `$HOME/.local/share` untouched.

**Done when** each of these is in the log, pass or fail:

- The six steps, one line each, with the failing step's log excerpt where one is red.
- **The adapter, named**, and the list of GPU-touching tests that ran rather than skipped. An empty
  list is a reading and must be written as one; silence is not.
- Whether the runner's `$HOME/.local/share` was left untouched, and how that was determined.
- The runner's own Ubuntu version, because `ubuntu-latest` moves and the artifact's glibc floor
  moves with it.

### Phase 2 — The arm goes green

- **Owner skill:** dev

Repair what Phase 1 reported, reasoning from the exact log lines rather than from a guess about
Linux. Expect the classes 0120 Phase 2 predicted — the `capture_handle::Handle = ()` arm,
unused-import warnings under `-D warnings`, a `#[cfg]` that assumed exactly two platforms — plus
whatever the first real compilation of `capture_linux.rs` turns up.

**This phase loops with the owner**, and that is stated rather than hidden: a session writes the
repair, the owner pushes, the next run either goes green or hands back a shorter list. Each pass
appends to the log; nothing is re-litigated.

**Done when:**

- The arm is green, evidenced by a run the log names — run id and date, not "it passed".
- **No test's platform gate was widened or loosened to get there.** A test that should not run on
  Linux gets a gate that says so in ADR-0016's shape, argued from the gate's own reasoning. A gate
  edited *because* CI was red, with no such reasoning, is the failure this clause exists to catch.
- If a GPU-touching test ran on the runner's software rasterizer and its result is not trustworthy
  there, the repair is that gate — and the log says which test and why, since it is the first time
  this project has had to answer that question.

### Phase 3 — The release dry run

- **Owner skill:** human

Run `release.yml` under `workflow_dispatch` and read what it produced. 0120 Phase 4 wrote every
assertion into `stage.sh` and into the `release` job's count guard, so **this phase reads a verdict
rather than inspecting an archive** — if the guard is wrong, that is itself the finding.

0120's two retired clauses land here verbatim:

> `stage.sh` produces `target/dist/ritmolux-v<version>-linux-x64.tar.gz`, whose single top-level
> entry is a folder of that name holding `ritmolux`, `presets/*.toml` and `READ-ME-FIRST.txt`.

> A `workflow_dispatch` dry run produces six artifacts and publishes nothing.

**Done when:**

- The dry run is green and the artifact list is in the log: **six**, named.
- The count guard's assertion of **5 `.zip` and exactly 1 `.tar.gz`** was exercised by that run —
  not read in the diff. A guard that would pass a Linux-less release is the thing most worth
  catching here, and only a run catches it.
- Nothing was published. The dispatch path's whole point is that it cannot.

### Phase 4 — Run it on the Ubuntu box

- **Owner skill:** human

0120 Phase 6, carried across whole. Validate **the artifact that ships**, not a `cargo run`:
extract the tarball from the Phase 3 dispatch run (or from a local `stage.sh`) on the Ubuntu
machine.

**The box is Ubuntu 26.04 and the build floor is 24.04** (0120 Phase 1's reading), so this run also
answers the compatibility direction: an artifact built against the older glibc running on the newer
system. That is the safe direction, and it is still a reading rather than an assumption.

**Done when** each of these is reported, pass or fail:

- It extracts and launches; `chmod +x` was or was not needed.
- Music plays and the visuals react to it.
- **F3's audio line reads `live PulseAudio 48000/2 <endpoint>`**, and the endpoint names a monitor
  source. A `failed PulseAudio …` line is a finding with its reason attached; `unsupported` means
  the wrong arm compiled.
- `~/.local/share/Ritmolux/` appears and holds `config.toml`, a preset copy and `diagnostics.log`;
  `diagnostics.log`'s `capture` column carries the same verdict token.
- `F` (fullscreen) works. `D` (next monitor) works, or is a no-op — **expected under Wayland**, per
  ADR-0131's Negative and confirmed as the box's session type by 0120 Phase 1, and recorded either
  way rather than treated as a bug.
- The frame rate F3 shows, and whether the quality tier was auto-dropped. The box has a discrete
  RTX 3060 on the proprietary driver, so a dropped tier here would be a finding rather than a
  weak-hardware result.
- Capture works on a **Bluetooth** default sink, which is what 0120 Phase 1 probed against. If the
  box's default sink is something else that day, say which.

Anything that fails becomes a **backlog entry**, not a silent fix — the code phases are finished by
this point and a repair belongs in its own scope.

## Risks & open questions

- **`ubuntu-latest` is a moving target, and the artifact's glibc floor moves with it.** 0120's
  release notes name Ubuntu 24.04 as the floor; if the runner image has moved to 26.04, the built
  binary will not run on 24.04 and the notes are wrong. Phase 1 records the runner's version for
  exactly this reason. The repair, if needed, is pinning the image in `release.yml` — an edit to
  0120's work, not a new decision.
- **The software rasterizer may make previously-skipped tests run.** ADR-0016 scopes GPU tests by
  adapter availability, and every such gate in this repository was written when "no adapter" meant
  macOS CI. Mesa's llvmpipe on the runner turns that assumption over. This is the reading most
  likely to produce a follow-on decision, and Phase 2 is deliberately allowed to gate rather than
  to chase a green.
- **Phase 2's loop is bounded by the owner's pushes, not by a session.** If it takes more than two
  or three passes, the cause is likely a class of failure the repo cannot see from Windows at all,
  and the honest answer is a local Linux checkout rather than a fourth round trip.
- **Nothing here is gated.** Every phase's product is prose in the log. That is the nature of
  evidence that lives off the machine, and it is why each done-when asks for a named run id, an
  adapter name or an artifact list rather than an adjective.

## What this plan does NOT do

- **It writes no Linux features.** Device enumeration, MPRIS metadata, AppImage/`.deb`/Flatpak,
  ARM64 — all still out, all still ADR-0131's deferred alternatives.
- **It does not re-open ADR-0131.** Phase 1 of 0120 confirmed the premise; this plan reads results,
  and a result that contradicts the ADR is an Outcome section on the ADR, written by the architect.
- **It does not push or tag.** The owner pushes, here as everywhere.
- **It does not absorb its own findings.** A failure becomes a backlog entry or a repair phase; it
  does not become a quietly loosened assertion.

## Implementation log

> Written as the phases land. **The phases above are the contract; everything here is what
> happened.**

### Phase 1 readings, partial (2026-09-22, recorded by `architect` at the owner's request)

**Run [35753544726](https://github.com/IgorKonovalov/Ritmolux/actions/runs/35753544726)** on
`144b137c`, and before it run 35731385874 on `3e7cb237`, the first push carrying 0120's Linux code.
Both failed in the same place. Every other job was green, including the `windows-latest` and
`macos-latest` arms of `check`.

- **`check (ubuntu-latest)`:** `cargo build` green. `cargo nextest run --workspace -P fast` red:
  `885/1695 tests run: 884 passed (1 slow), 1 failed, 86 skipped`. nextest then cancelled, so the
  remaining 810 tests and the job's later steps (doctests, clippy, fmt, doc) **did not run** and
  are still unread. The adapter line and the release dry run are also still owed.
- **The one failure:** `rlx-core render::tonemap::tests::the_scan_reads_the_visibility_the_helpers_actually_set`,
  at `core/src/render/tonemap/tests.rs:1431`: *"`pub(crate) fn texture(` now sets ShaderStages::,
  but MARKERS says its entries are FRAGMENT-visible"*, `left: ""`, `right: "FRAGMENT"`.
- **Cause (read from source, not yet run):** this is not a Linux defect. The test concatenates every
  `core/src` file in `rs_files`'s `read_dir` order, which is unsorted and filesystem-dependent, and
  then takes the **first** `find("pub(crate) fn texture(")`. That string matches three places:
  `render/gpu.rs:66` (the helper meant), `render/preview.rs:245` (an unrelated accessor with no
  `ShaderStages`), and the test's own literal at `tonemap/tests.rs:1421`. On NTFS and on this Arch
  box `gpu.rs` came first; on the runner's ext4 another match did. The repair is `dev`'s in Phase 2:
  make the file order deterministic, **and** make each needle identify its one definition (for
  example `pub(crate) fn texture(binding`), since sorting alone keeps a match that depends on
  filenames sorting in a particular order.

### Phase 2, first pass (2026-09-23)

**Lane:** `main` directly. **Commit:** `b47dc0f1`.

The one failure Phase 1 recorded is repaired in `core/src/render/tonemap/tests.rs`. The walk is
sorted through a new `sorted_rs_files`, which both scans in that file now call; the needle scan
leaves its own file out of the concatenation; three needles are narrowed to `...(binding`; and each
needle is asserted to match exactly one place in `core/src`.

**Two readings that correct the cause recorded above**, both taken here rather than read from
source:

- **The wrong hit is the test's own literal, not `preview.rs:245`.** When a needle matches itself,
  the next `ShaderStages::` is the one in its own assertion message, `ShaderStages::{found}`, where
  `::` is followed by `{`; the visibility parser reads an empty alphanumeric run and returns `""`.
  That is the `left: ""` both runs reported. `preview.rs:245` is a second match and a real hazard,
  but it is not what fired.
- **All four needles fail under the runner's order, not one.** Running the scan's logic over the
  real tree in both directions: the old logic in sorted order reads all four correctly; the old
  logic in reversed order reads `""` for all four; the new logic reads all four correctly in both
  orders, each with exactly one match. nextest reported the first assertion to fire, so the arm was
  never one match away from green.

**The failure was reproduced on this machine, not only on the runners.** Under the conductor on
2026-09-23, Plan 0223's lane failed this test at 18:31 on the `pub(crate) fn sampler(` needle
(`left: ""`, `right: "FRAGMENT"`), while Plan 0215's lane ran `-P fast` green at 18:45. One box, one
filesystem, two worktrees, opposite results.

**No test's platform gate was widened or loosened**, and none was touched: the repair is confined to
how one scan enumerates and searches `core/src`, and the assertion it now makes is strictly stronger
than the one it replaces.

**Checks:** `cargo fmt --all --check` clean; `cargo clippy --workspace --all-targets -- -D warnings`
clean; `cargo nextest run --workspace -P fast` 1718 passed, 0 failed, 86 skipped.

**Still owed on this phase, and only the owner can take it.** nextest cancelled on this failure, so
the 810 tests behind it and the job's later steps - doctests, clippy, fmt, doc - plus the adapter
line and the GPU-test list Phase 1's done-when names, have still never run. They arrive on the next
push, and this phase stays open until that reading is green or hands back a shorter list.

**One deviation, flagged rather than taken:** `Status:` is left at `approved` rather than flipped to
`in-progress`. No plan on `main` carries `in-progress`, the roster row in `docs/plans/README.md`
states `approved`, and this plan alternates between `human` pushes and `dev` repairs across many
sessions, so the flip would sit desynced from that index indefinitely.

### Phase 2, second pass (2026-09-23)

**Lane:** `main` directly. **Commit:** `f6bd0bf5`.

**The first pass worked, read from run
[35916291035](https://github.com/IgorKonovalov/Ritmolux/actions/runs/35916291035) on `6ae4af8c`.**
The arm went from `885/1695 tests run` to `1441/1718`, so 556 tests that had never executed on Linux
now do, and every other job on that run is green - `check (windows-latest)`, `check (macos-latest)`,
`coverage`, `spout`, `miri`, `deny`, `studio`, `links`. nextest still cancelled, on a different and
genuinely Linux-only failure, which is this pass.

**The second failure:** `standalone::help_cli
an_unhonourable_command_line_is_refused_before_anything_is_built` at `standalone/tests/help_cli.rs:218`.
`--preset a-name-no-preset-has` exited 101, not 2, because the process panicked at
`standalone/src/run.rs:670`: *"failed to create event loop: neither WAYLAND_DISPLAY nor
WAYLAND_SOCKET nor DISPLAY is set."*

**Cause, and it is narrower than it first looks.** The six comments in `run.rs` that say a flag is
judged *before the window exists* are all accurate: the window is created later, in `resumed`. What
was wrong is that `EventLoop::new()` itself needs a display server on Linux, and it was constructed
177 lines before the validation it is supposed to follow. On Windows and macOS that call cannot
fail, so the ordering was invisible there and the guarantee appeared to hold. The local binding it
produced was read in exactly one place, `run_app`, so the repair is to construct it there.

**Reproduced here before it was repaired, and the other cases it was hiding.** With `DISPLAY`,
`WAYLAND_DISPLAY` and `WAYLAND_SOCKET` all unset on the Arch box:

| headless command line | before | after | expected |
|---|---|---|---|
| `--preset a-name-no-preset-has` | 101 | 2 | 2 |
| `--preview syphon` | 101 | 2 | 2 |
| `--tier nonsense` | 101 | 1 | 1 |
| `--definitely-not-a-flag` | 2 | 2 | 2 |

The last row is the control: it is parsed in `stream::parse`, which already ran before the event
loop, and it was correct throughout. The second row is the case the same test asserts a few lines
below the one that fired, so the arm had at least one more failure queued behind it.

**No test's platform gate was widened or loosened**, and none was touched. No test changed at all:
the repair is one statement moved inside `run`.

**Checks, every one with no display server set:** `standalone::help_cli` 3 passed, 0 failed;
`cargo nextest run --workspace -P fast` 1722 passed, 0 failed, 86 skipped; `cargo fmt --all --check`
clean; `cargo clippy --workspace --all-targets -- -D warnings` clean.

**A finding for `architect`, not acted on.** This class is invisible on Windows and macOS by
construction - `EventLoop::new()` cannot fail there - so the ubuntu arm is the only thing that can
catch it, and it catches it as a panic in an unrelated assertion rather than as itself. Whether the
ordering deserves a guard of its own is a design question this pass did not answer.

**Still owed on this phase.** The arm cancelled before its later steps again, so `cargo test --doc`,
`clippy`, `fmt` and `doc` on Linux, the adapter Phase 1's done-when names, and the list of
GPU-touching tests that ran rather than skipped are all still unread. They arrive on the next push.
