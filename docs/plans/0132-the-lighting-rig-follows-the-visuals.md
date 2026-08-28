# 0132 — The lighting rig follows the visuals

> **Status:** in-progress
> **Created:** 2026-08-28
> **Owner skill(s):** dev, human
> **Related ADRs:** [0144](../adrs/0144-the-lighting-feed-is-a-resolved-ndi-sender-and-a-fixed-osc-telemetry-set.md) (proposed),
> [0125](../adrs/0125-the-live-video-out-is-a-spout-sender-fed-by-a-frame-tap.md) (proposed),
> [0046](../adrs/0046-linear-light-hdr-composite-bloom-tonemap.md),
> [0140](../adrs/0140-a-sample-budget-is-a-density-against-the-render-target.md) (proposed),
> [0142](../adrs/0142-the-audio-input-is-switched-live-and-the-shell-owns-the-policy.md) (proposed)

## TL;DR

The standalone grows **two optional sinks** so that real stage lighting follows what the visualizer
is doing. An **OSC telemetry stream** publishes the analyzer's fixed signal set to Resolume Arena on
the lighting machine; an **NDI sender** publishes the rendered scene, resolved down in linear light,
so Arena can sample it onto fixtures and the room inherits the preset's actual colour.

**The OSC half ships first and stands alone.** It touches no `core` code, needs no GPU, no SDK and
no Plan 0115, and its first user-visible behavior is **a lamp changing on the beat**. The NDI half
is behind two gates that can each kill it — Plan 0115's unbuilt frame tap and an unread SDK licence
— and the plan is ordered so that neither takes the OSC half down with it. **Under the 2026-08-29
show date that ordering is the whole point:** Phases 1a through 4 are the deliverable, and nothing
on the show-night path waits on the video half.

## Context & problem

The user runs a live rig: DJ audio on the visualizer box, **Resolume Arena on a separate machine**
joined by **switched Ethernet**, and Art-Net out of Arena to real fixtures. Arena already owns the
patch, the zoning and the dimming. What it has no source for is a signal that moves the way the
music does — which is exactly what this engine already computes and already draws.

[ADR-0144](../adrs/0144-the-lighting-feed-is-a-resolved-ndi-sender-and-a-fixed-osc-telemetry-set.md)
is the decision: two sinks, Arena owns the mapping, the transport set is constrained by what Arena
can natively ingest across machines. It also lists **six facts it rests on that are unverified in
this repo** — the NDI licence chief among them. Phases 1a and 1b establish them, split so the
NDI ones gate only the NDI phases.

**The two halves have very different dependency weight, and the phase order is built on that.** The
OSC half is a UDP socket and a message encoder in the shell. The NDI half needs
[Plan 0115](0115-the-engine-becomes-a-live-video-source.md)'s frame tap, which is **approved and not
started**, plus a third-party SDK whose licence terms nobody here has read. Interleaving them would
put the cheap, certain half behind the expensive, uncertain one.

## Decision

Take ADR-0144 as written, and **order the plan so value lands before risk**: confirm the rig half we
do not own, then the entire OSC path through to a lamp moving on the beat, then — behind its own
stop gate and its own external dependency — the video path.

## Architecture diagram

```mermaid
flowchart LR
    subgraph vis["visualizer machine — lmv.exe"]
        direction TB
        wasapi["WASAPI loopback"]
        ring[["SPSC ring"]]
        subgraph core["core/ — GPU-abstract, source-agnostic"]
            an["Analyzer"]
            df["draw_frame"]
            tap["FrameTap (Plan 0115)<br/>+ linear resolve — this plan"]
        end
        loop["shell frame loop<br/>holds the AnalysisFrame"]
        osc["osc sink<br/>UDP, no core change"]
        ndi["ndi sink<br/>feature-gated, staged SDK"]
    end
    subgraph show["lighting machine"]
        arena["Resolume Arena"]
    end
    lamps(["fixtures"])

    wasapi --> ring --> an --> loop
    loop --> df --> tap -->|"resolved RGBA8"| ndi
    loop --> osc
    osc -->|"OSC / UDP<br/>gigabit ethernet"| arena
    ndi -->|"NDI"| arena
    arena -->|"Art-Net"| lamps
```

## Implementation phases

> **RETIRED 2026-08-29 — Phases 1b, 5, 6 and 7 will not run, and the architecture they serve is
> superseded.** A live set on 2026-08-29 ran the whole chain with **no Arena, no second machine and
> no NDI**: `lmv --osc 127.0.0.1` into a bridge that emitted Art-Net straight to the fixtures. The
> user has since rejected the second machine outright and for all future work, which removes the
> receiver every one of those phases was built to feed.
>
> [ADR-0145](../adrs/0145-the-engine-drives-the-fixtures-directly-over-art-net.md) and
> [Plan 0133](0133-the-engine-drives-the-lights.md) carry the replacement. **The NDI licence gate is
> moot rather than resolved** — it was never read and no longer needs to be.
>
> **Phases 2 and 3 stand and their work ships.** The `/lmv/v1` OSC sink is what the live set ran on;
> ADR-0145 retains it. Phase 1a's questions are answered by the rig probes, and Phase 4 was met in a
> form the plan did not anticipate — a fixture moved on the beat, driven by this telemetry, through
> a bridge rather than through Arena. This is ADR-0144's own Alternative D surviving alone, exactly
> as that ADR said it might.
>
> **What this plan still owes at close:** ADR-0144 accepted with a dated `Outcome` recording that
> its premise was rejected rather than falsified, and Phase 4's *"which addresses were useful"*
> finding, which is real and is filed as design-backlog 0157.


> **Amended 2026-08-28, on a show date of 2026-08-29.** Phase 1 was originally one human stop gate
> covering both transports and running before `dev` started. That is wrong under a deadline: it puts
> the cheap, certain OSC half behind an NDI licence read that cannot help tomorrow's show. It is
> split into **1a (OSC)** and **1b (NDI)**, and **only 1b is a stop gate** — for Phases 5 to 7 alone.

### Phase 1a — Confirm Arena reaches the lamps at all, with no engine code

- **Owner skill:** human
- **What:** establish that the half of the chain **we do not own** works, before anything we build
  is pointed at it. This is not a formality: if Arena's Art-Net output is not reaching a fixture,
  every signal we send is irrelevant and the failure will look like ours.
- **Files touched:** none in the repo. Findings go in this plan's `## Implementation log`.
- **How** — at the venue, in roughly this order, none of it needing our code:
  - Move a parameter **by hand** in Arena and watch a fixture change. Record Arena's DMX/Art-Net
    output feature name and how it samples a source onto a fixture.
  - Turn on Arena's **OSC input** and record the port it listens on, and how a parameter is bound
    to an incoming address — Arena defines the address, so record the exact string it expects.
  - Record both machines' IP addresses and confirm they reach each other across the Ethernet link.
  - Record Arena's **edition and version**.
- **Done when** the log records the above, and in particular **whether a hand-moved parameter in
  Arena visibly changes a fixture.** If it does not, this plan is blocked on the rig, not on us, and
  that is the finding.
- **Under the deadline this phase may run *concurrently with* Phase 2 rather than before it**, and
  the OSC probe may be performed with the Phase 2 binary itself instead of a generic sender. The
  risk that accepts is small and named: our sink emits standard OSC 1.0 over UDP, so the only thing
  a generic sender would have de-risked is our own encoder, which Phase 2 unit-tests against the
  spec's padding rules anyway.

### Phase 1b — The NDI probe and the licence read

> **RETIRED 2026-08-29.** No NDI receiver exists in the architecture. The licence was never read.

- **Owner skill:** human
- **What:** settle ADR-0144's remaining unverified facts for the video half. **This is a stop gate
  for Phases 5 to 7 only.** It does not gate Phases 2 to 4 and it is not on the show-night path.
- **Files touched:** none in the repo. Findings go in this plan's `## Implementation log`.
- **How:** publish a generic NDI source (a test pattern or screen capture from NDI's own tools) from
  the visualizer machine across the Ethernet link, and receive it in Arena. Separately, open the NDI
  SDK's own licence and record what it says about accepting terms, redistributing the runtime, and
  attribution.
- **Done when** the log records whether NDI appears as a source in that Arena at all, the licence
  terms, and **which of these outcomes occurred:**
  - **(a) NDI arrives and the licence permits our distribution model.** Phases 5 to 7 proceed.
  - **(b) NDI is unsupported there, or the licence refuses.** Phases 5 to 7 do not run. ADR-0144 is
    accepted with a dated `Outcome` section recording which, and its **Alternative C** (HDMI plus a
    capture card) becomes the live question for a separate plan. **Phases 2 to 4 are unaffected and
    the plan still delivers a working feature.**
- **The fallback that is not a failure:** if the licence forbids *redistribution* but permits the
  operator installing the NDI runtime on both machines, that is outcome (a) with a documented
  install step, not outcome (b). Say which one it is.

### Phase 2 — The OSC telemetry sink, through to values on the wire

- **Owner skill:** dev
- **What:** a new `standalone` module publishing ADR-0144's fixed telemetry set over UDP. **No
  `core` change at all** — the shell already holds the `AnalysisFrame` it passes to
  `Renderer::render` and to the `Director`.
- **Files touched:** new `standalone/src/osc.rs` (+ `osc/tests.rs`), `standalone/src/main.rs`,
  `standalone/src/config.rs`, `standalone/src/lib.rs`.
- **How:**
  - **Hand-roll the OSC 1.0 encoder; add no crate.** A message is an address string, a type-tag
    string and the arguments, each padded to a 4-byte boundary — on the order of a hundred lines for
    the `f`, `i` and `s` types this needs. [NFR section 4](../nfr.md)'s dependency gate asks for a
    justification for any crate pulling a large transitive graph, and the honest answer here is that
    the encoder is smaller than the justification would be. `std::net::UdpSocket` is the transport.
  - **The address space is versioned in the addresses themselves** — `/lmv/v1/...` — so a later
    change is additive and an operator's Arena mapping keeps working. Publish the four normalized
    levels, the raw levels, onset, the beat counter, beat phase, tempo, and the preset name.
    `AnalysisFrame`'s `bar` field is beat phase under a documented misnomer (ADR-0050); **publish it
    under its true name**, since nothing outside this repo inherits that naming debt.
  - **Config and CLI:** `[osc]` in `config.toml` (enabled, target `host:port`, send rate) plus an
    `--osc <host:port>` override, following the `[input]` precedent
    ([ADR-0142](../adrs/0142-the-audio-input-is-switched-live-and-the-shell-owns-the-policy.md)).
    **Off by default.**
  - **Decide the failure behaviour deliberately.** A send error must not propagate with `?` into the
    frame loop and must not spam a log every frame. Whatever it does — drop silently, back off,
    latch a status line — is a stated choice, in a comment, with the mechanism.
- **Done when:**
  - With the app running against real audio and an OSC monitor listening on another host, every
    published address carries a value, and the level and onset addresses **change with the music**.
  - `standalone`'s tests assert the encoder against **the OSC 1.0 spec's own padding rules**: an
    address of length 4 still takes a full 4 bytes of padding, a 3-character string pads to 4, and
    every message length is a multiple of 4. These are exact, dimensionless properties — no
    tolerances and no machine dependence.
  - `cargo nextest run --workspace` is green and **no golden baseline moves**, blessed or otherwise.
    This phase adds no render path; a baseline that moves is a finding.

### Phase 3 — Decide the send path by measurement, and record the number

- **Owner skill:** dev
- **What:** establish whether the inline `sendto` belongs in the frame loop, and take the dedicated
  sender thread only if the measurement says so. ADR-0144 states both exits; this phase picks one.
- **Files touched:** `standalone/src/osc.rs`, `standalone/src/main.rs`.
- **How:** measure the frame-time distribution with the sink **off** and with it **on**, against a
  **LAN** target rather than localhost — localhost is the cheap case and the wrong one. Repeat each
  configuration enough times to characterize its own run-to-run spread.
- **Done when** the log records the measured distributions and which exit was taken, against this
  criterion:
  - **The property, not a threshold:** enabling the sink must not move the frame-time distribution
    outside the run-to-run variance the *same* configuration shows against itself. If sink-on sits
    inside sink-off's own spread, the inline send ships. If it sits outside it, the sender thread
    lands in this phase.
  - This is stated as a property on purpose. A fixed microsecond budget would be a number invented
    here rather than earned, and it would not survive a different machine — whereas "smaller than
    this configuration's own noise" is checkable anywhere and means the same thing everywhere.
  - The log names the machine and the link the measurement was taken on, per
    [ADR-0071](../adrs/0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md).

### Phase 4 — Human: the OSC half against real fixtures

- **Owner skill:** human
- **What:** run the app into Arena on the rig, bind the telemetry to fixtures, and play a set.
- **Files touched:** none. Findings go in the log.
- **Done when** the log records:
  - **A fixture visibly changes on the beat**, driven by our telemetry. This is the plan's walking
    skeleton and the first thing anyone can point at.
  - **Which addresses turned out useful and which were noise.** This is the real feedback the fixed
    set needs: ADR-0144 chose a fixed vocabulary over preset-declared channels, so the only way it
    gets better is an operator saying which signals earned their place.
  - Anything that behaved unlike Phase 1's probe predicted.

### Phase 5 — The frame tap grows a linear resolve stage

> **RETIRED 2026-08-29 as written; the idea survives.** The linear resolve is the right thing and
> moves to [Plan 0133](0133-the-engine-drives-the-lights.md) Phase 8, which resolves onto the rig's
> own 24 x 170 raster instead of onto an NDI frame.

- **Owner skill:** dev
- **Depends on:** [Plan 0115](0115-the-engine-becomes-a-live-video-source.md) Phase 2 having landed
  the frame tap. **This plan does not build that tap.** If 0115 has not reached Phase 2 when this
  plan is taken up, this phase and the two after it wait; Phases 1 to 4 do not.
- **What:** add a GPU-side reduction to the tap that resolves the show-size render down to the
  lighting size **in linear light**, so the readback is the small buffer rather than the large one.
- **Files touched:** `core/src/render/` (the tap module 0115 Phase 2 creates), plus its tests.
- **How:** establish first — by reading the code, and recording it in the log — **whether the
  tonemap's output offscreen is linear float or already display-encoded**, which is ADR-0144's
  unverified fact 5. If it is already encoded, the resolve decodes to linear before averaging and
  re-encodes once at the end.
- **Done when:**
  - **The discriminating test passes, and it is exact.** Resolve a frame that is half full-white and
    half full-black. A correct linear average is 0.5 in linear light, which sRGB-encodes to roughly
    **0.735** (about byte 188). Averaging the *encoded* bytes instead gives 127.5 (byte 128). The
    test asserts the linear answer. The two are far enough apart that no tolerance argument is
    needed — this is a property of the transfer function, derivable on paper, and it fails loudly
    if the resolve ever silently moves back into encoded space.
  - A constant-colour frame resolves to exactly that colour, at every supported lighting size.
  - **No golden baseline moves.** The resolve is a new path off the tap; it changes nothing the
    window or the capture paths render.
  - `cargo nextest run --workspace` green, `cargo clippy --workspace --all-targets` clean.

### Phase 6 — The NDI sender in the standalone

> **RETIRED 2026-08-29.** No NDI, no SDK staging, no licence obligation.

- **Owner skill:** dev
- **Depends on:** Phase 1b outcome (a), and Phase 5.
- **What:** publish each resolved frame as an NDI source, behind a cargo feature that is **off by
  default**.
- **Files touched:** new `standalone/src/ndi.rs`, `standalone/Cargo.toml`, `standalone/src/main.rs`,
  the SDK staging script, CI's release job.
- **How:** stage the SDK by **pinned fetch, never committed**, exactly as
  [ADR-0115](../adrs/0115-the-foobar-component-is-a-released-artifact-with-a-parameterized-sdk.md)
  already stages the foobar2000 SDK and as ADR-0125 plans to stage Spout — one pattern, third use.
  The feature being off by default is what keeps an ordinary `cargo build`, CI's default job and the
  macOS target untouched.
- **Done when:**
  - An NDI monitor on **the other machine** shows the stream, at the configured size and rate, with
    the picture recognisably the scene.
  - A default `cargo build` and the default CI job are byte-unaffected — the feature-off build links
    no NDI symbol.
  - The staged SDK's licence obligations (attribution, notices) are discharged wherever the release
    artifacts carry licence text, per Phase 1's reading.
  - Sender lifetime is defined across a **resolution change** and across the receiver disappearing
    and returning; the log states which behaviour was chosen.

### Phase 7 — Human: the whole rig, and the question this plan cannot answer

> **RETIRED 2026-08-29.** Its legibility question is real and moves to [Plan 0133](0133-the-engine-drives-the-lights.md) Phase 8.

- **Owner skill:** human
- **What:** run the full path — audio to visuals to NDI to Arena to Art-Net to fixtures — for a real
  set, and judge whether the lamps **read**.
- **Files touched:** none. Findings go in the log.
- **Done when** the log records:
  - **The verdict on legibility.** ADR-0144 predicts this openly: the engine draws sparse bright
    marks on black for a projector, and averaged onto lamps that may be a dim room with occasional
    flashes. This phase is the only instrument that exists for it.
  - **If the room is too dim, which remedy is wanted** — Arena-side gain, a gain or lift inside our
    resolve, or a lighting-specific render path. **None of the three is built here.** The choice is
    routed to the design backlog, and the third is ADR-worthy.
  - **The observed skew between the two paths** — whether an OSC-triggered effect and the
    pixel-driven wash agree about where the beat was, which ADR-0144 lists as unmeasured.
  - Session behaviour over the set: whether either sink degraded, stalled or dropped.

### Phase 8 — The operator documentation

> **RETIRED 2026-08-29.** Documentation moves to [Plan 0133](0133-the-engine-drives-the-lights.md) Phase 9, which
> also brings the rig facts in from the out-of-repo probe folder.

- **Owner skill:** dev
- **What:** write the operator guide and sweep the docs a new surface makes stale.
- **Files touched:** new `docs/lighting.md`, `README.md`, `docs/nfr.md`.
- **How:** `docs/lighting.md` carries the rig topology, the Arena-side setup Phase 1 established, the
  **full `/lmv/v1/` address table**, the config keys and CLI flags, and the gigabit-Ethernet link
  requirement including the note that Art-Net shares that LAN. `README.md` gains the new flags and
  config keys. `docs/nfr.md` section 10 (live performance) gains the lighting path, since it already
  names the live-show use this serves.
- **Done when** `node scripts/check-doc-links.mjs` exits 0, the address table matches what Phase 2
  actually emits (checked against the code, not against this plan), and the README's flag list
  matches `--help`.

## Risks & open questions

- **The NDI licence is the single largest risk and it is unquantified.** Spout was BSD and this is
  not. Phase 1 reads it before a line of NDI code exists, which is the cheapest possible ordering,
  and outcome (b) is a real and survivable end.
- **Plan 0115 is approved and unstarted**, so Phases 5 to 7 have a dependency outside this plan's
  control. That is why the OSC half is complete by Phase 4 rather than interleaved.
- **Legibility may be the real finding.** It is plausible that this whole path works perfectly and
  the lamps still look wrong, because the source material was never designed for them. Phase 7 is
  built to surface that as a *routed decision* rather than an improvised fix at 2 a.m.
- **The fixed OSC set may prove too narrow**, which is ADR-0144's Alternative F waiting to be
  revisited. Phase 4's "which addresses were useful" note is the evidence that question needs.
- **Two networked sinks enter a live show path.** Failure behaviour is a stated choice in Phase 2,
  but nothing here makes the show robust to a switch dying.
- **Open:** whether the lighting resolution should be operator-configurable or fixed. The plan
  assumes configurable with a small default; Phase 7 may argue it should be pinned.

## What this plan does NOT do

- **No Art-Net, sACN or DMX leaves this application.** Arena owns fixture profiles, patching,
  zoning, dimmer curves and DMX timing. ADR-0144 Alternative E records why.
- **No zone extraction and no lighting-specific render path.** We resolve the picture we already
  draw. If Phase 7 says that is not enough, the remedy is routed, not built.
- **No preset-declared OSC channels.** ADR-0144 Alternative F, rejected on the mid-show contract
  break.
- **No foobar plugin support.** Standalone only, so `LMV_ABI_VERSION` does not move and the C ABI
  contract is not engaged.
- **This plan does not build Plan 0115's frame tap.** It adds a resolve stage to it.
- **No WiFi and no remote-over-internet path.** ADR-0144's arithmetic assumes a wired gigabit link.
- **No audio crosses either sink.**
- **No golden coverage is claimed for either sink.** Both are wall-clock paced and their output
  leaves the process; every claim about them is a human reading or a property test on a
  deterministic sub-part.

## Implementation log

> Written by `dev` — one row per phase as that phase's commit lands, and the close block after the
> last one. **The phases above are the contract; everything here is what happened.**

**Lane:** `plan-0132-the-lighting-rig-follows-the-visuals`, worktree
`C:/Users/Igor Konovalov/WORK/lmv-plan-0132`, branched from `main` at `1ec19f2`
(after Plan 0127's close).

| phase | owner | state | commit |
|-------|-------|-------|--------|
| 1a | human | partly answered by Phase 2's probe — see Notes | — |
| 1b | human | RETIRED — no NDI receiver | — |
| 2 | dev | done | c8bdcd5 |
| 3 | dev | done — inline send ships, no code change | committed with this row |
| 4 | human | not started | — |
| 5 | dev | RETIRED — moves to Plan 0133 Phase 8 | — |
| 6 | dev | RETIRED — no NDI | — |
| 7 | human | RETIRED — moves to Plan 0133 Phase 8 | — |
| 8 | dev | RETIRED — moves to Plan 0133 Phase 9 | — |

### Notes

**Phase 2 — the rig is not reachable from the dev machine, and the "another
host" done-when was met on this one instead.** The user gave the Arena machine
as `192.168.1.101/24`. `ping` returns 100% loss, and three independent causes
were established before any code ran:

- the dev machine is `192.168.0.171/24` on Wi-Fi — `192.168.1.0/24` is a
  different subnet and is not on-link;
- its Ethernet adapter holds `169.254.118.59`, an APIPA address, so the switched
  gigabit link ADR-0144 requires is not up;
- `Find-NetRoute 192.168.1.101` resolves to a VPN tunnel (`10.32.5.126`), so
  traffic for that address leaves through the tunnel rather than the LAN.

`192.168.0.1` (the local gateway) answers in 1-5 ms, so the NIC and the local
LAN are fine; the path to Arena does not yet exist. **These are Phase 1a's
findings, arrived at from this end of the link rather than at the venue**, and
Phase 1a's own questions — whether a hand-moved Arena parameter changes a
fixture, Arena's OSC port and address strings, its edition — are all still
open.

So the first done-when bullet was run with the monitor on **this** host
(`127.0.0.1:9000`), not another one. What it recorded, over ~13 s of loopback
capture against a synthesized 120 BPM signal: **7,281 datagrams, all 14
addresses present, every datagram a multiple of 4 bytes.** `level/bass`,
`level/mid`, `level/treb` and `level/onset` each swept the full `0.0`-`1.0`
range; `level/rms` `0.0`-`0.505`; `beat/index` ratcheted 0 to 23; `tempo`
settled at 59.84; `preset` carried `Clifford`. The cross-host half of that
bullet is unmet and needs the rig.

**Two shapes the plan left open, decided here.**

- **`rms` has no source in `AnalysisFrame`.** ADR-0144 names it in the fixed set
  and the engine computes no such value. It is computed in the shell from
  `AnalysisFrame::waveform` (`osc::rms_of`) and published at
  `/lmv/v1/level/rms`. It is therefore **un-normalized**, unlike the four levels
  beside it, because the waveform it comes from deliberately is (ADR-0049) —
  an operator mapping it needs a console-side gain where the other four need
  none. The observed range above is the evidence for what that gain faces.
- **The failure behaviour** (the plan asks for a stated choice): a send error
  drops the datagram, increments a counter, and prints **only on the edge**
  between working and failing, and again on recovery — the transition shape
  `reported_overflow` and `reported_demotion` already use. Nothing propagates
  with `?`. The socket is **non-blocking**, so a `sendto` stalling on ARP
  resolution cannot cost a frame; `WouldBlock` counts as a drop like any other.

**The address the plan calls "the beat counter" is published as
`/lmv/v1/beat/trigger` + `/lmv/v1/beat/index`,** two addresses rather than one:
`/lmv/v1/beat` would have been a prefix of `/lmv/v1/beat/phase`, and an OSC
pattern match against `/lmv/v1/beat/*` behaves differently for a prefix than for
a sibling.

**One observation outside this phase's scope, and its retraction.** The tempo
estimate settled at **59.84 BPM against a signal built at 120 BPM** in the
13-second probe above, which read as a half-tempo lock. A later 45-second run on
the same signal settled at **127.84 BPM**, so the low reading was the estimator
still warming rather than a standing defect. Recorded because the first number
was written here before the second existed; **there is no tempo finding.** The
sink publishes what the analyzer reports either way.

**Phase 3 — the inline send ships; `standalone/src/osc.rs` and
`standalone/src/main.rs` were not modified.** The criterion is met on all three
metrics, so the phase's conditional work (the dedicated sender thread) did not
land and this row's commit carries only the measurement.

*Machine and link* (ADR-0071): AMD Ryzen 9 5900HS, NVIDIA RTX 3080 Laptop +
Radeon iGPU, Windows 10 19045, display 2560x1440 @ **165 Hz**, app windowed at
1920x1080. Release build. Link: **Wi-Fi 6 (MediaTek MT7921), 144.4 Mbps
negotiated** — *not* the gigabit Ethernet ADR-0144 specifies, and *not* the
Arena machine; see the deviation below.

*Method.* Twelve runs, **alternating off/on** so drift falls on both
configurations equally: `--soak` sampling every 5 s over 24 s of playback, the
preset pinned to a single-preset `LMV_PRESET_DIR` (`attractor_clifford`) so
every run draws the same scene, the same synthesized 120 BPM signal played
through loopback each time, and each run's first soak sample dropped as startup.
Sink-on target `192.168.0.1:9000`.

*Frame-time distributions*, per-run means across 6 runs each:

| metric | sink off (mean, sd, range) | sink on (mean) | verdict |
|---|---|---|---|
| fps | 92.008, sd 0.330, [91.600, 92.400] | 91.850 | inside |
| mean frame ms | 10.8753, sd 0.0395, [10.8289, 10.9259] | 10.8941 | inside |
| `frame_ms_p99_steady` | 14.0096, sd 0.489, [13.4150, 14.5328] | 13.7199 | inside |

**Sink-on's mean sits inside sink-off's own run-to-run range on every metric.**
The mean-frame-time delta is **+0.0188 ms (+0.2%)**, under half of sink-off's own
standard deviation against itself; the p99 delta is *negative* (-0.29 ms), which
is noise rather than an improvement.

*The send cost itself was measured separately*, because a distribution that does
not move says nothing about how much headroom is left. A throwaway instrument
(scratchpad, not in the repo) replicated the per-frame work exactly — fourteen
non-blocking `send_to` calls totalling 392 bytes — over 1,200 paced frames
after a 200-frame warmup:

| target | mean | p50 | p90 | p99 | max |
|---|---|---|---|---|---|
| `192.168.0.1` (LAN) | 0.1691 ms | 0.1531 | 0.2256 | 0.3003 | 0.6794 |
| `127.0.0.1` (control) | 0.1052 ms | 0.0898 | 0.1580 | 0.2224 | 0.3104 |

The LAN target costs **~1.6x** localhost, which is the plan's reason for
refusing the localhost measurement.

**The two numbers disagree, and the reason matters more than either.** A 0.169 ms
mean send against a 10.875 ms frame should have moved the mean frame time by
~1.5%; it moved it by 0.2%. The app ran at ~92 fps against a **165 Hz** display,
so it was **not** vsync-clamped — it was bound waiting on the GPU, and the
send overlaps that wait. So the inline exit is earned on this machine by
**slack that happens to exist**, not by the send being free. A CPU-bound
configuration would show more of the 0.169 ms, and the p99 send of 0.30 ms is
4.9% of a 165 Hz frame budget and 1.8% of a 60 Hz one.

**Two deviations from the phase as written.**

- **The measurement is against the local gateway over Wi-Fi, not the Arena
  machine over gigabit Ethernet** (user's instruction, 2026-08-28, after the
  rig was found unreachable — see the Phase 2 note). It satisfies "a LAN
  target rather than localhost": the datagrams leave the NIC and traverse the
  switch. It does not measure the rig's own path, and the 0.169 ms figure is
  the one to re-take on the show network.
- **The soak log has no mean-frame-time column.** Its columns are `fps`,
  `frame_ms_p99` and `frame_ms_p99_steady`, so the mean frame time above is
  `1000 / fps` rather than a directly logged statistic.
