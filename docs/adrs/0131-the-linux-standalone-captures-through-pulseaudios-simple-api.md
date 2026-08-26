# ADR-0131 — The Linux standalone captures system audio through PulseAudio's simple API, on a third platform arm

> **Status:** proposed
> **Date:** 2026-08-26
> **Related plan(s):** [0120](../plans/0120-the-standalone-ships-on-ubuntu.md)
> **Related ADRs:** [0001](0001-rust-core-wgpu-cabi-foobar-shim.md) (the source-agnostic core),
> [0016](0016-gpu-tests-opt-in-ci-scope.md) (GPU stays out of the CI contract),
> [0038](0038-tag-driven-release-unsigned-universal-mac-app.md) (what a `v*` tag ships)

## Context

The standalone ships on Windows and macOS. The tree is already *shaped* for a third platform and has
never been compiled on one: `standalone/src/lib.rs` gives `preset_data_root()` an `XDG_DATA_HOME`
branch, `standalone/src/rss.rs` reads `/proc/self/statm`, and `start_capture` has a
`#[cfg(not(any(windows, target_os = "macos")))]` arm that returns `CaptureVerdict::Unsupported` and
renders silence-driven visuals. Nothing builds any of it — `ci.yml`'s `check` matrix is
`[windows-latest, macos-latest]`, and the three `ubuntu-latest` jobs run Node gates, `cargo deny`
and Miri, none of which compile `standalone`. So the Linux arm is untested code that has been
carried for its whole life.

The forcing request is parity, not a compile: an Ubuntu user should get the app a Windows user gets,
which means it hears what the machine is playing.

Four platform facts decide the shape.

1. **There is no single Linux system-audio API, and the kernel one cannot do this.** ALSA is the
   kernel interface and has no notion of a monitor or loopback source — it captures inputs.
   Recording what a machine is *playing* is a sound-server concept. PulseAudio introduced it: every
   sink carries a `.monitor` source yielding exactly what that sink received.
2. **PipeWire is the default on Ubuntu 22.04+, and it speaks PulseAudio.** `pipewire-pulse` is a
   PulseAudio protocol server, so a PulseAudio *client* reaches both the modern default and the
   older stack. Choosing the client protocol is therefore not choosing a sound server.
3. **Unlike WASAPI and ScreenCaptureKit, nothing hands us a callback.** Both shipped backends are
   callback-driven, which is why "the audio callback is sacred" is written the way it is. The
   PulseAudio *simple* API (`pa_simple_new` / `pa_simple_read`) is blocking and synchronous, and we
   own the thread it blocks on. The rule that matters there is the same one — no allocation, no
   lock, no logging in the loop — but a blocking read is the correct shape rather than a hazard.
4. **The simple API cannot enumerate.** It takes a device name and opens it. Listing sources with
   friendly names needs the asynchronous context API (`pa_mainloop` + `pa_context` +
   `pa_context_get_source_info_list`), which is a different and much larger program.

Against this, `docs/nfr.md` §4 makes every crate a cost, and §8 fixes the distribution posture:
unsigned, no installer.

## Decision

We will add a **third platform arm**, `standalone/src/capture_linux.rs`, gated
`#[cfg(target_os = "linux")]` and a structural sibling of `capture_win.rs` and `capture_mac.rs`: it
exports `CaptureError`, a `CaptureHandle` with `format()`, and
`start() -> Result<(CaptureHandle, SampleConsumer), CaptureError>`, so `capture_handle::Handle` and
`start_capture` gain a third branch and nothing else in the shell changes shape.

It binds PulseAudio through the **`libpulse-simple-binding` crate**, opens a record stream on the
special device name **`@DEFAULT_MONITOR@`** at a fixed 48 kHz stereo float format, and runs one
dedicated thread that blocks in `pa_simple_read` into a buffer allocated once at construction and
pushes frames into the existing SPSC ring. The verdict backend token is `"PulseAudio"`. Dropping the
handle stops the thread and closes the stream, as it does on both other platforms.

**Device enumeration is deliberately not part of this.** `--list-devices` and `config.input.device`
stay Windows-only; on Linux the config key exists and is inert — the same documented asymmetry
macOS already carries for now-playing metadata.

Distribution follows [ADR-0038](0038-tag-driven-release-unsigned-universal-mac-app.md) unchanged in spirit: a
fourth job in `release.yml` stages a folder and writes a **`.tar.gz`**, built on **`ubuntu-latest`**,
which sets the glibc floor at Ubuntu 24.04. CI gains an `ubuntu-latest` arm on the `check` matrix
running the same five steps as the other two, with the GPU suites skipping through
[ADR-0016](0016-gpu-tests-opt-in-ci-scope.md)'s existing mechanism.

## Consequences

### Positive

- The third `cfg` arm stops being untested code. Every push compiles it, lints it, and runs its
  non-GPU tests — worth having on its own, independently of capture.
- One client protocol covers both sound servers a current Ubuntu can be running, so the backend does
  not branch on PipeWire versus PulseAudio and has no runtime detection to get wrong.
- The blocking simple API is the smallest program that does the job: no mainloop, no callback
  lifetime to manage across an FFI boundary, no `UnsafeCell` producer smuggled into an Objective-C
  class as `capture_mac.rs` needs.
- `core/` is untouched. This is a shell concern by ADR-0001, and the decision exercises that split
  rather than testing it.
- The tarball mirrors the Windows zip's contents and its four verification checks, so the packaging
  bar is the existing one rather than a new one.

### Negative

- **`libpulse.so.0` is a hard link dependency.** On a machine with no PulseAudio and no
  `pipewire-pulse`, the binary does not start *at all* — it fails at the dynamic loader, before any
  of our code runs, so the shell never gets to render the `Unsupported` verdict it is perfectly
  capable of rendering. That graceful degradation is exactly what Alternative A would have bought,
  and we are giving it up. It is a real loss, and cheap only because the target is Ubuntu desktop,
  where the library is always present.
- **Building needs `libpulse-dev` on every builder**, including CI's ubuntu arm and the release job.
  A missing package is a build failure, not a degraded build.
- **No device selection on Linux.** A user with several sinks gets the default one and no way to say
  otherwise short of changing the system default. Adding the async context later is additive and
  does not disturb this arm's shape, but until then the asymmetry is real.
- **`@DEFAULT_MONITOR@` under `pipewire-pulse` is unverified from this dev box.** It is a documented
  PulseAudio special name; whether PipeWire's protocol server resolves it is a claim we have not
  tested. Plan 0120's first phase is a human probe on the target machine that answers it *before*
  any code is written, and a negative answer is a scope change — the fallback is reading the default
  sink name through the async API and appending `.monitor` — not a silent workaround.
- **The glibc floor is Ubuntu 24.04.** A 22.04 user gets nothing from this. Building on the older
  runner would have covered both, but that image is being retired and the only machine available to
  validate the result runs 24.04, so the wider reach would be reach into a configuration nobody ever
  launches the binary on.
- **A fourth release artifact is a fourth thing that can break a tag**, and the publish guard does
  not currently notice: it counts `assets/*.zip` and asserts exactly 3, so a silently-skipped Linux
  job leaves that count correct and ships a short release. The guard has to learn about the second
  archive kind.
- **Widening `deny.toml`'s `[graph].targets` pulls in the Linux-only dependency tree** — winit's
  Wayland and X11 backends among others — which the supply-chain gate has never evaluated. It may
  surface advisories or licences needing reasoned entries.
- **Wayland is Ubuntu 24.04's default session, and winit cannot position windows there.** The `D`
  hotkey (move to the next monitor) may be a no-op. That is a parity gap, not a capture one, and this
  ADR does not solve it.

### Neutral

- The `CaptureVerdict::Unsupported` variant survives and keeps its meaning; its `dead_code` allow
  list simply gains a third platform. The arm that constructs it becomes
  `not(any(windows, target_os = "macos", target_os = "linux"))` — still reachable in principle, still
  dead on every platform we ship.
- Nothing here touches the C ABI or the foobar component. A Linux media-player plugin (DeaDBeeF,
  Audacious) is a separate decision nobody has asked for.

## Alternatives considered

### Alternative A — Hand-rolled `dlopen` of `libpulse-simple.so.0`, no new crates

Declare the dozen C entry points ourselves and load them at runtime. Zero dependencies against NFR
§4, no `libpulse-dev` on any builder, and — the real prize — the binary starts on a machine without
PulseAudio and degrades to the `Unsupported` verdict the shell already renders.

**Lost on ownership.** It is roughly two hundred lines of `unsafe` C ABI declarations that no
compiler checks against the real header; a struct layout or enum discriminant that drifts is silent
memory corruption on a hot audio thread, and it would be *our* bug. The project already accepts
binding crates for exactly this job — the macOS path links seven `objc2` crates — so refusing them
here would be an inconsistency bought with a graceful-degradation path that matters only off the
target platform.

### Alternative B — The full asynchronous PulseAudio context

`pa_mainloop` plus `pa_context`, enumerating sources with friendly names, supporting monitor and
line-in modes, matching the Windows selector exactly.

**Lost on scope.** It is the largest of the three programs, and it buys a capability no Linux user
has asked for. It is also strictly additive later: the arm's public surface is `start()` returning a
handle and a consumer, and swapping what sits behind it changes nothing in the shell.

### Alternative C — Native PipeWire (`libpipewire`)

The modern API, lower latency, the direction the ecosystem is going.

**Lost on reach for a benefit we do not need.** It excludes any system still on PulseAudio, and our
latency budget is a visualizer's, not a DAW's — the ring buffer already decouples audio from render
cadence by design. The pulse-compat layer is universal on the target and costs us nothing.

### Alternative D — `cpal`

One cross-platform crate, already familiar to the ecosystem, ALSA backend on Linux.

**Lost on capability.** ALSA has no monitor source, so `cpal` on Linux captures input devices. It
would give us a microphone tap and call it done, requiring every user to hand-configure an ALSA
loopback to get what the other two platforms give for free.

### Alternative E — JACK

Real system-audio routing, and the serious-audio Linux answer.

**Lost on presence.** It is not running on a default Ubuntu desktop, so it would make the common
case the configured case.

### Alternative F — AppImage, `.deb`, or Flatpak

A friendlier artifact: self-contained, or dependency-declaring, or sandboxed.

**Lost on cost against reach.** The `.tar.gz` mirrors what already ships on Windows, and every
library the binary needs is present on the target. A second packaging recipe is a second thing to
keep verified, for a tester audience that is currently one person. Revisit if the audience grows.

### Alternative G — Build on the `ubuntu-22.04` runner for a lower glibc floor

Covers 22.04, 24.04 and later from one build.

**Lost on validation.** GitHub is retiring that image, so the pin would need a revisit date it would
not get, and the only machine available to validate the artifact runs 24.04 — so the wider reach
would be reach into a configuration nobody ever launches the binary on.

### Alternative H — Run the GPU suites on Linux CI through software Vulkan (lavapipe)

Mesa's `lavapipe` would make headless rendering executable on the ubuntu arm, the way DX12 WARP does
on Windows.

**Lost on what the goldens mean.** The baselines are blessed against WARP; a third rasterizer either
needs its own baseline set or reports its differences as failures. This repo has twice blessed
garbage produced by a software adapter — a pass whose bind-group layout matched a live pipeline's
took that pipeline's uniform, and building GPU resources mid-run shifted what the trails stage
resolved to — both invisible until someone compared adapters. Adding a third one to be trusted is
the opposite of the lesson.

## Notes

**Unverified claims in this ADR**, each one a thing Plan 0120 Phase 1 or Phase 2 establishes rather
than something measured here:

- That `pipewire-pulse` resolves `@DEFAULT_MONITOR@`. Documented for PulseAudio; assumed for the
  compat server.
- That `ubuntu-latest`'s image needs an explicit `apt-get` step for the winit/wgpu/pulse development
  headers, and which packages exactly.
- Which wgpu adapter, if any, resolves on a GitHub `ubuntu-latest` runner, and therefore which
  GPU-touching tests actually execute there rather than skipping.
- That widening `deny.toml` to `x86_64-unknown-linux-gnu` leaves the supply-chain gate green.
