# ADR-0144 — The lighting feed is a resolved NDI sender and a fixed OSC telemetry set

> **Status:** accepted 2026-08-29 — see the dated `Outcome` below; superseded in part by
> [0145](0145-the-engine-drives-the-fixtures-directly-over-art-net.md)
> **Date:** 2026-08-28
> **Related plan(s):** [0132](../plans/done/0132-the-lighting-rig-follows-the-visuals.md),
> [0133](../plans/0133-the-engine-drives-the-lights.md)

## Context

The user wants this engine to drive **real stage lighting**, through **Resolume Arena**, which
already runs on the rig and already outputs Art-Net to the fixtures. Arena patches the fixtures,
owns the zoning and owns the dimming; what it lacks is a signal that follows the music the way this
engine's presets already do.

**The rig's shape decides most of this, and it was established by interview rather than assumed.**
The music lives on the visualizer box — loopback capture from the DJ software — so the app cannot
move to the Arena machine. Arena is on a **separate** machine, and the two are joined by **switched
Ethernet**. Arena's use of our picture is **lighting only**: it samples the video onto fixtures and
nobody ever watches those pixels. Fixture mapping stays Arena's job by the user's explicit choice.

**Two things have to cross, and one transport cannot carry both.** The picture is what makes the
lamps inherit a preset's actual colour and motion — the whole reason to involve a visualizer rather
than a lighting console's own generators. Discrete musical events — an onset, a beat count, a tempo,
a preset change — are things a pixel expresses badly and an operator wants to bind to a strobe, a
bump or a cue. So: video **and** control, deliberately, as two sinks.

**[ADR-0125](0125-the-live-video-out-is-a-spout-sender-fed-by-a-frame-tap.md) already anticipated
this exact case and deferred it.** That ADR chose Spout for TouchDesigner **on the same machine**,
and said so in as many words: its Alternative B records NDI as *"the alternative to revisit if
remote becomes the primary case rather than a follow-on."* It also made the frame tap
transport-agnostic on purpose, *"which is what makes the remote case a later sink rather than a
later rewrite."* This ADR is the first exercise of that extension point, and it does not reopen
ADR-0125 — Spout remains right for the local TouchDesigner case, because **Spout is shared memory on
one GPU and simply cannot cross a machine boundary.**

What is left is a transport question with an unusually hard constraint: **the receiver is fixed.**
Whatever we emit, Arena must ingest it natively, because putting a translation helper in a live show
path is a third thing to fail at 1 a.m. Across machines, on Windows, that set is small.

## Decision

We will add **two independent sinks to `standalone`**, both off by default, both fed by read-only
taps on machinery `core` already has:

1. **An NDI sender** carrying the rendered scene, **resolved down** to a small lighting-appropriate
   size, fed by ADR-0125's frame tap with a **resolve stage added before readback**.
2. **An OSC sender** publishing a **fixed, versioned telemetry set** derived from `AnalysisFrame` at
   frame cadence to a configured `host:port`.

`core` gains **exactly one thing: the resolve stage on the tap.** The OSC half touches no `core`
code whatsoever — the standalone already holds the `AnalysisFrame` it hands to `Renderer::render`
and to the `Director`, so the telemetry sink reads a value the shell has in hand and needs no new
accessor. **No NDI type, no OSC type and no socket appears in `core`** — the shell owns the sink
exactly as it owns the audio source, which is the same split ADR-0125 drew and the same one
[ADR-0142](0142-the-audio-input-is-switched-live-and-the-shell-owns-the-policy.md) drew for input
policy. **The C ABI is untouched:** this is standalone-only, so `LMV_ABI_VERSION` does not move and
[ADR-0003](0003-c-abi-v1-surface.md)'s contract is not engaged.

### The lighting picture is a downsample of a show-size render, never a small render

This is the non-obvious half of the decision and it is the one that would otherwise be got wrong,
because rendering directly at 128x72 looks like the obvious economy and is not the same picture.

The scenes' drawn sample counts **do not currently scale with the render target** — that is the
defect [ADR-0140](0140-a-sample-budget-is-a-density-against-the-render-target.md) and Plan 0128 are
taking on, measured there as 0.651 particles/px at 640x360 against 0.072 at 1080p. So a tiny render
today is a **saturated blob**: the full 1080p sample count crushed into 9,216 pixels. And once
ADR-0140 lands and the count becomes a density, a tiny render becomes the opposite — a handful of
samples on an empty field. **Neither is the look**, and the difference is not a quality setting; it
is a different image. The lighting feed must therefore be an **average of the picture the show would
have shown**, which means rendering at show size and resolving down.

### The resolve happens in linear light

Averaging display-encoded sRGB bytes **underestimates** mean radiance, and it errs in precisely the
direction that makes a room too dark — the failure mode this feature exists to avoid.
[ADR-0046](0046-linear-light-hdr-composite-bloom-tonemap.md) gives the engine a linear HDR
composite, and Plan 0045 Phase 3 already built and proved an `Rgba16Float` readback (today
`#[cfg(test)]`-scoped, in `core/src/render/capture.rs`), so the format is known to work here.

The resolve takes the **tonemapped** result — what the screen would show, so the lamps match the
look — and averages it **before the display encode**, encoding once at the end. Whether the
tonemap's output offscreen is already display-encoded is **unverified in this repo**; if it is, the
resolve decodes to linear, averages, and re-encodes, and the plan's tap phase establishes which.

### The OSC set is fixed by the engine, not declared by presets

The address space is flat, versioned, and identical under every preset: per-band levels, `rms`,
onset, the beat counter, beat phase, tempo, and the current preset name. An authored `[osc]` table
in preset TOML was considered and rejected below — a preset swap must not silently rewrite the
contract an operator has mapped Arena against, least of all mid-show.

### The send path is decided by measurement, and both exits are stated

The OSC datagram is first sent **inline in the frame loop**, and the p99 `sendto` cost is measured
on the real rig against a LAN target — not localhost, which is the cheap case and the wrong one.
Against [NFR section 3](../nfr.md)'s 60 ms end-to-end budget and
[NFR section 1](../nfr.md)'s frame budget, if it fits, it ships inline. If
it does not, **a dedicated sender thread behind a lock-free handoff is the known fix**, it is the
same discipline as the audio/render seam, and nothing in this decision moves to adopt it. What is
**not** negotiable either way: this never touches the audio callback
([NFR section 5](../nfr.md)).

### The link requirement, stated rather than assumed

Switched **gigabit Ethernet**. NDI's own published figure is ~100-130 Mbps at 1080p60; at the
lighting resolutions here it is a rounding error, so the requirement is set by headroom and jitter
rather than throughput. **Art-Net shares this LAN**, and it is the latency-sensitive traffic in the
building — the video and the lighting control should not contend on one uplink. WiFi is out of
scope by construction: this ADR's arithmetic assumes a wired link, and ADR-0125's Alternative D (the
h.264 pipe) remains the only option here that survives one.

## Consequences

### Positive

- **The lamps inherit the preset, not a re-authoring of it.** Colour, motion and energy arrive as
  the picture itself. Nothing about a preset's look has to be reproduced by hand in Arena, which is
  the entire cost this design exists to avoid.
- **The readback that ADR-0125 flagged as its main unmeasured cost is essentially free here.** At
  128x72 a frame is `128 x 72 x 4` = **36,864 bytes**, ~2.2 MB/s at 60 fps — against the 8.29 MB
  per frame and ~498 MB/s each way that ADR-0125 accepted at 1080p, a **225x** reduction by
  arithmetic. The resolve is a GPU-side reduction of a texture that was rendered anyway.
- **It exercises ADR-0125's extension point instead of contradicting it.** The tap stays
  transport-agnostic; a second sink attaches. This is the shape that ADR predicted, arriving on
  schedule.
- **The OSC half is independent of everything.** It needs no tap, no NDI, no Plan 0115, no GPU and
  no Windows — so it can ship first and alone, it is the walking skeleton that makes a lamp move,
  and it works on macOS and Linux for free.
- **We do not become a lighting console.** Fixture profiles, patching, zoning, dimmer curves and
  DMX timing all stay in Arena, which does them well and which the operator already knows.
- **NDI is less platform-restricted than Spout**, which has no macOS form at all. This particular
  decision does not inherit ADR-0125's platform asymmetry, even though our capture path still has
  its own.

### Negative

- **NDI's licence is not Spout's, and this is the one finding that can kill the video half.**
  ADR-0125 could say "BSD, vendored by a pinned fetch" and be done. The NDI SDK is proprietary,
  gated behind an accepted EULA, and carries redistribution and attribution conditions on its
  runtime. **The terms are unverified in this repo** and are a blocking gate before any NDI code is
  written. If redistribution is not permissible on terms we can meet, the fallback is that the
  operator installs the NDI runtime on both boxes — acceptable on a show machine, unacceptable as a
  silent assumption.
- **A show-size render is paid for a 128x72 output.** The resolve saves the readback, not the
  render. Nobody looks at those pixels and we draw all of them anyway. This is inherent to the
  "downsample, don't render small" decision and is accepted as the price of the lamps showing the
  real look.
- **Two clocks, and they are not phase-locked.** The NDI picture and the OSC events traverse
  different paths with different latencies, so an OSC-triggered strobe and a pixel-driven wash can
  disagree about where the beat was. Lighting tolerates more skew than video does, but the amount is
  **unverified** and only the rig can say.
- **A second network dependency lands in a live show path.** A dropped NDI stream is a dark room,
  and the failure is silent from our side. Whatever the sinks do on failure has to be a deliberate
  choice, not a `?`.
- **Nothing in the golden suite can cover any of this.** Both sinks are wall-clock paced and their
  output leaves the process. Every claim about them is either a byte-identity claim against a
  deterministic path or a human reading on the rig — the same position ADR-0125 accepted.
- **Nobody art-directs the lighting.** The engine was designed to make a picture for a projector:
  sparse bright marks on black, high contrast, wide dynamic range. Averaged onto lamps that is a
  dim room with occasional flashes, which may read as broken rather than as intended. This is a
  **real and expected** finding, its instrument is a human looking at fixtures, and the deliberate
  choice here is to discover it on the rig rather than pre-emptively build a lighting-specific
  render path nobody has yet shown is needed.

### Neutral

- No audio crosses either sink. Arena takes audio from its own source; neither NDI-as-configured
  here nor the OSC set carries any.
- Both sinks are feature-gated / config-gated off, so an ordinary build, CI, and every user who does
  not run a lighting rig are untouched — the [NFR section 4](../nfr.md) posture
  ADR-0125 established for Spout, applied again.
- Arena's Art-Net output, its fixture sampling model, its OSC input port and its NDI input support
  are all **its** behaviour, not ours. We verify them on the rig; we do not specify them.

## Alternatives considered

### Alternative A — Spout, as ADR-0125 chose

**Rejected because it is physically impossible here.** Spout is a shared D3D11 texture in shared
memory on one GPU. There is no network form. This is listed not because it was competitive but
because it is the obvious first thought given ADR-0125, and the reason it fails is worth recording
once so nobody re-proposes it.

### Alternative B — the h.264 pipe into `ffmpeg`, from ADR-0114's shipped machinery

Render headless, write Y4M into a spawned `ffmpeg`, serve SRT or RTSP over the LAN.
**Rejected because the receiver cannot read it.** Arena has no native SRT or RTSP input, so this
needs a translation process on the show machine — the exact third thing to fail at 1 a.m. that the
"receiver is fixed" constraint exists to prevent. Its 150-400 ms encode-transport-decode latency is
a second, independent disqualification: on lighting that is visible lag against the beat, not a
buffering detail. It remains the right answer for a **WiFi** link, and this ADR's gigabit
requirement is what retires it.

### Alternative C — HDMI out of the visualizer box into a capture card on the Arena box

Arena reads capture devices natively, so this needs **zero software from us**. **Rejected on what it
undoes.** It forces a window and an attached display back onto the visualizer box, discarding the
headless mode this whole line of work is built on; it adds a capture card and a cable run to a rig
that already has Ethernet; and it moves a 128x72 lighting signal over a 1080p wire to get there. It
is the honest fallback if the NDI licence gate fails **and** the runtime-install fallback is also
refused, and it is recorded here so that path is already argued.

### Alternative D — OSC only, no video at all

Drop the pixel path; let Arena's own clips and generators make the light, modulated by our
telemetry. Cheapest by a wide margin, no SDK, no licence, no second render, no colour question, and
cross-platform. **Rejected because the coupling is the point.** Without the picture, the room no
longer looks like the visuals — an operator would rebuild each preset's palette and motion by hand
in Arena, per preset, forever, which is the work this feature exists to delete. Notably this
alternative is **not discarded**: it is precisely the OSC half, shipping first and standing alone,
so if the video half dies at the licence gate this remains a working feature rather than a
cancelled plan.

### Alternative E — Art-Net or sACN emitted directly by this app

**Rejected because it makes us a lighting console.** Fixture profiles, patching, zoning, dimmer
curves, channel layouts and DMX refresh timing are all real work, all already done by Arena, and all
work the operator already knows how to drive. It also discards the one piece of the rig that is
already there and already trusted. The user chose Arena-owns-the-mapping explicitly, and this is
what that choice rejects.

### Alternative F — preset-declared OSC channels (an `[osc]` table in preset TOML)

Bind named OSC addresses to expressions in the preset grammar, so the content lane authors lighting
cues alongside the look. Genuinely attractive: it reuses the evaluator, the `--report` tooling and
the whole authoring toolchain, and a new lighting signal would cost no engine work.
**Rejected on the show-floor failure it creates.** The operator maps Arena against an address space;
a preset rotation would then silently rewrite that contract mid-show, and the symptom is a fixture
that stops responding for reasons no log explains. A fixed set is stable across rotation, which is
the property a live rig needs more than it needs expressiveness. It also hands the `preset-author`
lane a responsibility it has no rig to test against. Revisit only if the fixed set proves too narrow
in practice, and then as a **supplement** to it rather than a replacement.

## Notes

**Six facts this decision rests on are unverified in this repo, and every one of them is checkable
on the rig in a single session before any code is written.** They are listed so that a wrong belief
costs an hour rather than a plan — the same posture ADR-0125's notes took, for the same reason.

1. **That Arena ingests NDI**, on the operator's edition and version. The operator has confirmed
   Arena "consumes video as an input"; which transports that covers is not yet established.
2. **The NDI SDK licence and its runtime redistribution terms.** Blocking. See the first negative.
3. **Arena's DMX/Art-Net output** — its feature name, whether it is Arena-only, and the model by
   which it samples a video region onto a fixture. This ADR deliberately asserts none of it from
   memory.
4. **Arena's OSC input** — the port, the address matching rules, and whether a value arriving at
   frame rate is throttled or acted on per message.
5. **Whether the tonemap's output offscreen is linear float or already display-encoded**, which
   decides whether the resolve stage decodes before averaging.
6. **End-to-end latency on both paths, and the skew between them.** Estimated to be within
   lighting's tolerance; measured by nobody yet.

Two figures used above are of different quality and are labelled as such. NDI's ~100-130 Mbps at
1080p60 is **vendor-published** and is inherited from ADR-0125. The 36,864-byte frame, the 2.2 MB/s
and the 225x readback reduction are **arithmetic** from the frame size. The particle densities
quoted for the small-render argument are **measurements from Plan 0128 Phase 1**, and that plan is
draft — if its numbers move, the direction of the argument does not, because it holds under both
the pre- and post-ADR-0140 behaviour.

## Outcome (added at Plan 0132's close, 2026-08-29)

**The OSC half shipped and stands. The NDI half was never built, and its premise was rejected
rather than falsified** — which is a different verdict from the one this ADR prepared for, and the
distinction is the whole of what this section records.

### What shipped

`standalone/src/osc.rs` publishes the fixed `/lmv/v1` set over UDP, off by default, behind `--osc`
and an `[osc]` config section. The encoder is hand-rolled, as this ADR argued, and adds no crate.
Two shapes this ADR left open were decided in the plan and are now the contract:

- **`rms` had no source.** This ADR names it in the fixed set and the engine computes no such value.
  It is computed in the shell from `AnalysisFrame::waveform` and is therefore **un-normalized**,
  unlike the four levels beside it — an operator mapping it needs a console-side gain.
- **"The beat counter" is two addresses**, `beat/trigger` and `beat/index`, because `/lmv/v1/beat`
  would have been a *prefix* of `/lmv/v1/beat/phase` and an OSC pattern match treats a prefix and a
  sibling differently.

**The send path was decided by measurement and the inline exit was taken**, per this ADR's own
"both exits are stated" clause. Plan 0132's Phase 3 log carries the distributions, the machine and
the honest reading: sink-on sits inside sink-off's own run-to-run spread on every metric, but the
send itself costs ~0.169 ms mean against a 10.875 ms frame, so the inline exit is earned by GPU-wait
slack that happened to exist on that machine rather than by the send being free. A CPU-bound
configuration would show more of it. That is a re-measure trigger, not a defect.

### What was rejected, and why that is not a falsification

A live set on 2026-08-29 ran the whole chain with **no Arena, no second machine and no NDI**:
`lmv --osc 127.0.0.1` into a Python bridge on the same host, emitting Art-Net straight to the
fixtures. The user then rejected the second machine outright and for all future work.

That removes the **receiver** every NDI argument here was built to feed. Nothing in the video
half was measured and found wrong; the room it was designed for stopped existing. So:

- **The NDI licence gate is moot, not resolved.** It was never read. Plan 0132 Phase 1b did not run.
  Anyone reaching for NDI later starts that question from zero — this ADR's silence on it is an
  absence of evidence, not evidence.
- **The resolve-in-linear-light argument survives its transport.** It was never about NDI; it is
  about what a lighting sample of an HDR composite must average, and it moves to
  [Plan 0133](../plans/0133-the-engine-drives-the-lights.md) Phase 8 to resolve onto the rig's own
  24 x 170 raster instead of onto an NDI frame.
- **Alternative D — OSC only, no video at all — is what actually ran**, alone, exactly as this ADR
  said it might. It was listed as the degenerate fallback and it turned out to be the product.
- **Alternative E — Art-Net emitted directly by this app — was refused here on a premise that is
  now false.** This ADR rejected it because *"it makes us a lighting console"* and Arena already was
  one. The rig has no console. [ADR-0145](0145-the-engine-drives-the-fixtures-directly-over-art-net.md)
  reopens it on that corrected premise and supersedes this ADR's transport half.
- **Alternative F — preset-declared OSC channels — is still open and now has evidence pointing at
  it.** The fixed set's first real consumer wanted the bar grid the engine already computes and did
  not publish, filed as design-backlog 0157, and hit the unsettled tempo octave, filed as 0158.
  Both are additive under `/lmv/v1` and neither requires F.

**What is *not* superseded:** the fixed-vocabulary decision, the `/lmv/v1` versioning-in-the-address
scheme, the drop-and-report-the-edge failure behaviour, and the off-by-default posture. ADR-0145
retains the sink as built.
