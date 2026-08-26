# 0120 — The standalone ships on Ubuntu

> **Status:** draft
> **Created:** 2026-08-26
> **Owner skill(s):** dev, human
> **Related ADRs:** [0131](../adrs/0131-the-linux-standalone-captures-through-pulseaudios-simple-api.md) (proposed),
> [0038](../adrs/0038-tag-driven-release-unsigned-universal-mac-app.md),
> [0016](../adrs/0016-gpu-tests-opt-in-ci-scope.md),
> [0025](../adrs/0025-foobar-component-version-single-sourced.md),
> [0001](../adrs/0001-rust-core-wgpu-cabi-foobar-shim.md)

## TL;DR

The standalone gets a third platform: **Ubuntu 24.04 x86_64**. A new
`standalone/src/capture_linux.rs` opens the default sink's monitor source through PulseAudio's
simple API — which `pipewire-pulse` also serves — so an Ubuntu user hears what the machine is
playing, exactly as a Windows user does. CI gains an `ubuntu-latest` arm, and a `v*` tag gains a
fourth artifact: a `.tar.gz` staged the same way the Windows zip already is.

The first thing that happens is not code. **Phase 1 is a human probe on the target machine**, run
before `dev` starts, because ADR-0131's central premise — that `@DEFAULT_MONITOR@` resolves under
PipeWire's PulseAudio compat server — has never been tested from this dev box.

## Context & problem

The tree is already shaped for Linux and has never been compiled on it. `preset_data_root()` has an
`XDG_DATA_HOME` branch (`standalone/src/lib.rs:186`), `current_rss_bytes()` reads
`/proc/self/statm` (`standalone/src/rss.rs:78`), `capture_handle::Handle` has a `()` third arm
(`standalone/src/main.rs:191`), and `start_capture` has an arm returning `CaptureVerdict::Unsupported`
that renders silence-driven visuals (`standalone/src/main.rs:1220`). None of it is built by anything:
`ci.yml`'s `check` matrix is `[windows-latest, macos-latest]`, and the `links`, `deny` and `miri`
jobs that do run on `ubuntu-latest` never invoke `cargo build`. So this is code that has been carried
for the project's whole life without a compiler ever seeing it.

What the user asked for is not that compile — it is **parity**: the Ubuntu build should do what the
Windows and macOS builds do, which means capturing system audio. On Linux that is a sound-server
concept rather than a kernel one, and the decision of which client protocol to speak is
[ADR-0131](../adrs/0131-the-linux-standalone-captures-through-pulseaudios-simple-api.md).

Three things are true about this work that shape the phasing:

- **The capture seam already exists and is the right one.** `capture_win.rs` and `capture_mac.rs`
  are siblings that each hand back a `CaptureHandle` and a `SampleConsumer`; the shell branches on
  `cfg` in exactly three places. A third backend is an addition, not a refactor, and `core/` is not
  touched at all.
- **The riskiest claim is cheap to test and expensive to be wrong about.** If `@DEFAULT_MONITOR@`
  does not resolve, the backend needs the asynchronous context API to find the default sink name —
  a different and larger program. One `parec` command on the target box settles it, so that command
  runs first.
- **There is a real Ubuntu 24.04 machine to validate on.** Unlike the macOS path, which shipped
  having never executed on Apple hardware (ADR-0038's Context), this one can be run before the tag.

## Decision

Take ADR-0131: a `libpulse-simple-binding` backend on `@DEFAULT_MONITOR@`, a `.tar.gz` built on
`ubuntu-latest`, and an `ubuntu-latest` CI arm whose GPU suites skip through ADR-0016's existing
mechanism. The plan runs the human probe first, then a contiguous `dev` block, then a human
validation of the artifact that actually ships.

## Architecture

```mermaid
flowchart TD
    subgraph os["OS audio, per platform"]
        wasapi["WASAPI loopback<br/>(Windows)"]
        sck["ScreenCaptureKit<br/>(macOS 13+)"]
        pulse["PulseAudio protocol<br/>@DEFAULT_MONITOR@<br/>served by pipewire-pulse<br/>(Ubuntu 24.04)"]
    end

    subgraph shell["standalone/ — the shell (platform code lives here)"]
        cw["capture_win.rs<br/>cfg(windows)"]
        cm["capture_mac.rs<br/>cfg(target_os = macos)"]
        cl["capture_linux.rs<br/>cfg(target_os = linux)<br/>NEW"]
        verdict["capture_verdict.rs<br/>live PulseAudio 48000/2"]
        ring["SPSC ring<br/>audio::intake"]
    end

    subgraph core["core/ — untouched by this plan"]
        dsp["DSP + scenes + wgpu"]
    end

    wasapi --> cw
    sck --> cm
    pulse --> cl
    cw --> ring
    cm --> ring
    cl --> ring
    cw -.-> verdict
    cm -.-> verdict
    cl -.-> verdict
    ring --> dsp
```

The new box is `capture_linux.rs`, and it attaches to the two seams that already exist. Nothing
crosses into `core/`, which is what ADR-0001's split is for.

## Implementation phases

### Phase 1 — Probe the Ubuntu box before any code is written

- **Owner skill:** human

This phase runs **before `dev` starts**, on the real Ubuntu 24.04 machine, and it exists because
ADR-0131 rests on a premise this repo cannot test from Windows. It writes no code. The user runs
these and reports the output:

```sh
echo "$XDG_SESSION_TYPE"                       # wayland or x11 — decides what Phase 6 can check
pactl info                                     # server flavour and the default sink name
pactl list short sources | grep -i monitor     # the monitor sources that exist
timeout 3 parec -d @DEFAULT_MONITOR@ --raw > /tmp/mon.raw   # WITH MUSIC PLAYING
ls -l /tmp/mon.raw && xxd /tmp/mon.raw | head  # non-zero size, and not all zero bytes
apt-cache policy libpulse-dev libpulse0        # is the dev package installable
vulkaninfo --summary 2>/dev/null | head -20    # which Vulkan device, if any
```

**Done when** three questions have answers, written into this plan's `## Implementation log`:

1. **Does `@DEFAULT_MONITOR@` resolve, and does it carry audio?** `/tmp/mon.raw` is non-empty **and
   its bytes are not all zero while music is playing.** A non-empty file of silence is a failure
   here, not a pass — a monitor source that resolves and delivers nothing is the exact failure mode
   this probe exists to catch.
2. **What is the session type and the GPU?** Recorded, not acted on — Phase 6 needs to know whether
   `D` (move to next monitor) is expected to work.
3. **Is `libpulse-dev` installable?**

**If question 1 answers no**, stop and route back to the architect. The fallback is the asynchronous
context API reading the server's default sink name and appending `.monitor`, which is a scope change
to ADR-0131 and a re-plan — not something `dev` improvises. This is a stop gate on purpose, on the
Plan 0115 Phase 1 precedent.

### Phase 2 — The tree compiles, lints and tests on Ubuntu

- **Owner skill:** dev

Add `ubuntu-latest` to `ci.yml`'s `check` matrix and make the third `cfg` arm compile. Expect to fix
whatever a compiler has never seen: the `capture_handle::Handle = ()` arm, unused-import warnings
under `-D warnings`, and any `#[cfg]` that assumed exactly two platforms.

The runner needs system packages for winit, wgpu and (from Phase 3) PulseAudio. Start from
`libxkbcommon-dev libwayland-dev libpulse-dev pkg-config` and converge — the exact list is one of
ADR-0131's Notes, not a settled fact.

Update the `check` job's own comment: it currently says the `-E` filter is *"Applied on BOTH matrix
arms"*, which stops being true here.

**Done when:**

- The `ubuntu-latest` arm runs all five existing steps green — `cargo build`, the filtered
  `cargo nextest run --workspace`, `cargo test --workspace --doc`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check` — with the same
  `-E` filter the other two arms use, not a loosened one.
- **The implementation log records which wgpu adapter, if any, resolves on the runner, and whether
  any GPU-touching test actually executed there rather than skipping.** This is the phase's real
  finding and it must not be absorbed silently: `ubuntu-latest` may ship Mesa's software Vulkan, in
  which case tests that skip on macOS for want of an adapter will *run* on Linux — against a
  rasterizer nothing in this repo has ever compared against. If any did run, name them.
- No test's platform gate is widened or loosened to make the arm green. A test that should not run
  on Linux gets a gate that says so, in ADR-0016's shape.

### Phase 3 — The PulseAudio capture backend

- **Owner skill:** dev

Write `standalone/src/capture_linux.rs` as a structural sibling of `capture_mac.rs`, and wire the
three `cfg` sites in `main.rs` (`mod` declaration, `capture_handle::Handle`, `start_capture`).

Pin both pulse crates exact in a new `[target.'cfg(target_os = "linux")'.dependencies]` block, per
NFR §4 and the hygiene guard. Add `x86_64-unknown-linux-gnu` to `deny.toml`'s `[graph].targets` and
update that block's comment, which currently says the gate evaluates only Windows and macOS *"and
prunes crates pulled in solely by other targets — e.g. winit's Linux Wayland backend"*. That
sentence becomes false in this phase.

**Done when:**

- `capture_linux.rs` exports `CaptureError`, `CaptureHandle` with `format()`, and
  `start() -> Result<(CaptureHandle, SampleConsumer), CaptureError>` — the same three items
  `capture_mac.rs` exports, so nothing else in `main.rs` changes shape.
- The format is 48 kHz stereo and goes through `AudioFormat::validate()` at the intake boundary,
  once, like both other backends. The ring is built with the same `RING_CAPACITY_FRAMES = 16_384`
  the other two use.
- **The read loop allocates nothing.** The staging buffer is sized and allocated once, before the
  loop; the loop body performs no allocation, takes no lock, opens no file and logs nothing. This is
  a property, checkable by reading the loop body — there is no number attached to it.
- A **partial read is not a partial frame.** A `pa_simple_read` returning a byte count that is not a
  whole number of interleaved frames must neither push a truncated frame nor discard the remainder.
  A unit test asserts this on a synthetic buffer — it is a pure function of a byte slice and a frame
  size, so it needs no PulseAudio server and runs on every CI arm.
- Dropping the `CaptureHandle` stops the thread and closes the stream, and the shell's shutdown path
  does not hang waiting for a blocked read.
- `CaptureVerdict::Unsupported`'s `cfg_attr(..., allow(dead_code))` list gains `target_os = "linux"`,
  and the arm constructing it becomes
  `not(any(windows, target_os = "macos", target_os = "linux"))`. The doc comment on that variant
  names three platforms, not two.
- `cargo deny check` is green with the widened target list. If it is not, the fix is a **reasoned
  `ignore` entry naming the advisory id and why**, never a narrowed target list — narrowing would
  put the shipped Linux artifact back outside the gate.
- The `check` arm from Phase 2 stays green with the new dependency present.

### Phase 4 — The release tarball

- **Owner skill:** dev

Add `packaging/linux/stage.sh` and a `linux` job in `release.yml` that calls it, following the
`packaging/macos/bundle.sh` precedent: **the build, the staging, the archiving and the verification
all live in the script**, so a developer running it on their own Ubuntu box is held to the CI job's
bar rather than a looser one. Invoke it as `bash packaging/linux/stage.sh` rather than as an
executable, for the same reason the macOS job does — this repo is developed on Windows with
`core.filemode=false`.

Write `packaging/linux/READ-ME-FIRST.md` mirroring the Windows one: `chmod +x lmv` then run it, the
same Controls table, F3's audio line reading `live PulseAudio 48000/2` when capture works, and the
per-user directory at `~/.local/share/light-music-visualizer/`.

**Done when:**

- `stage.sh` produces `target/dist/light-music-visualizer-v<version>-linux-x64.tar.gz`, whose single
  top-level entry is a folder of that name holding `lmv`, `presets/*.toml` and `READ-ME-FIRST.txt`.
- The version is parsed **section-anchored** from `[workspace.package]` in root `Cargo.toml` per
  [ADR-0025](../adrs/0025-foobar-component-version-single-sourced.md), not by a first-match
  `version =` — a naive match reads a member crate's line or a `[profile]` key.
- The script **verifies from the archive it wrote**, not from the staging directory, and makes the
  same four assertions the Windows job makes: `lmv` and `READ-ME-FIRST.txt` are at the top level, the
  `.toml` count equals `presets/*.toml` in the repo, and no `.md` file is present.
- The `release` job's `needs:` gains `linux`, and **its count guard learns about the second archive
  kind**: it asserts exactly **3 `.zip` and exactly 1 `.tar.gz`** (macOS, Windows and foobar being
  the three zips). Today it counts `assets/*.zip` and asserts 3, which a silently-skipped Linux job
  would satisfy — so leaving that check alone would ship a short release without a red job anywhere.
- The release notes gain a Linux bullet naming the floor (Ubuntu 24.04 or newer, x86_64) and the
  runtime requirement (PipeWire or PulseAudio — the binary will not start without `libpulse.so.0`).
- A `workflow_dispatch` dry run produces four artifacts and publishes nothing.

### Phase 5 — The docs say Linux

- **Owner skill:** dev

The operator-doc sweep, done in this phase rather than left to the close. Every place that says the
project ships on two platforms now says three, and every place that describes the loopback asymmetry
gains its third entry.

**Done when** each of these has been opened and either updated or deliberately left alone:

- `README.md` — the platform statement and anything naming the shipped artifacts.
- `CLAUDE.md` — the "Architecture at a glance" diagram's frontend list, the `standalone/` and
  `packaging/` entries under "Where things live", and the **"Loopback capture is not symmetric"**
  bullet under "Platform realities", which currently contrasts Windows against macOS only.
- `docs/nfr.md` — §7's CI platform statement and §8's distribution posture.
- `docs/releasing.md` — a tag now builds and publishes four artifacts, not two.
- `docs/on-device-validation.md` — a Linux section, which Phase 6 then executes.
- `docs/capturing.md` — only if a `shot` flag or harness behaviour differs on Linux; say so in the
  log if nothing changed.

And: `node scripts/check-doc-links.mjs`, `node scripts/check-index-rows.mjs` and
`node scripts/check-backlog-claims.mjs` all exit 0.

### Phase 6 — Run it on the Ubuntu box

- **Owner skill:** human

Validate the artifact that actually ships, not a `cargo run`. Extract the tarball produced by
Phase 4 (from a `workflow_dispatch` run, or from a local `stage.sh`) on the Ubuntu 24.04 machine.

**Done when** each of these is reported, pass or fail:

- It extracts and launches; `chmod +x` was or was not needed.
- Music plays and the visuals react to it.
- **F3's audio line reads `live PulseAudio 48000/2`.** A `failed PulseAudio …` line is a finding
  with its reason attached; `unsupported` means the wrong arm compiled.
- `~/.local/share/light-music-visualizer/` appears and holds `config.toml`, a preset copy and
  `diagnostics.log`; `diagnostics.log`'s `capture` column carries the same verdict token.
- `F` (fullscreen) works. `D` (next monitor) works, or is a no-op — expected under Wayland, per
  ADR-0131's Negative, and recorded either way rather than treated as a bug.
- The frame rate F3 shows, and whether the quality tier was auto-dropped.

Anything that fails here becomes a **backlog entry**, not a silent fix — the plan's `dev` phases are
finished by this point and a repair belongs in its own scope.

## Risks & open questions

- **`@DEFAULT_MONITOR@` may not resolve under `pipewire-pulse`.** The whole backend rests on it.
  Phase 1 answers it before a line is written, and a negative answer stops the plan rather than
  redirecting it mid-flight.
- **The `ubuntu-latest` runner may resolve a software Vulkan adapter.** Then tests that skip on
  macOS for want of one will execute on Linux against a rasterizer this repo has never compared
  against, and any difference will look like a code bug. Phase 2 is required to *report* this rather
  than absorb it. The repo has twice blessed garbage off a software adapter; the mitigation is
  knowing which tests ran, not trusting them.
- **Widening `deny.toml` may go red.** The Linux dependency tree has never been through the
  supply-chain gate. The repair is a reasoned `ignore` entry, never a narrowed target list.
- **Wayland limits winit.** Window positioning and monitor moves are not available the way they are
  on X11; the `D` hotkey may be inert. Not solved here, and Phase 6 records it as a known gap.
- **The glibc floor is 24.04.** If a 22.04 user turns up, that is a new decision (ADR-0131
  Alternative G), not a tweak.
- **`libpulse-dev` on the runner image is unconfirmed.** If it is absent from `ubuntu-latest`, every
  ubuntu job that compiles `standalone` needs the `apt-get` step, and forgetting it in a new job is
  a build failure rather than a degraded build.

## What this plan does NOT do

- **No device enumeration or line-in on Linux.** `--list-devices` and `config.input.device` stay
  Windows-only; the config key exists and is inert. That is ADR-0131's Alternative B, deferred and
  additive.
- **No now-playing metadata on Linux.** Windows has SMTC (ADR-0110); the Linux equivalent is MPRIS
  over D-Bus and it is not in scope. The banner exists and is simply never fed, the same asymmetry
  macOS already carries.
- **No AppImage, `.deb` or Flatpak.** ADR-0131 Alternative F.
- **No GPU test coverage on Linux CI.** ADR-0131 Alternative H.
- **No 22.04 or ARM64 Linux build.** One target: `x86_64-unknown-linux-gnu`.
- **No Linux media-player plugin.** The foobar component is Windows-only and stays so; a DeaDBeeF or
  Audacious equivalent is a separate decision nobody has asked for.
- **No change to `core/`.** Capture is a shell concern by ADR-0001. A diff touching `core/` in this
  plan is a finding.
