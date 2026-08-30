# ADR-0145 — The engine drives the fixtures directly over Art-Net

> **Status:** proposed
> **Date:** 2026-08-29
> **Related plan(s):** [0133](../plans/0133-the-engine-drives-the-lights.md)
> **Supersedes:** [ADR-0144](0144-the-lighting-feed-is-a-resolved-ndi-sender-and-a-fixed-osc-telemetry-set.md)'s
> rejection of its own Alternative E, and its NDI half. ADR-0144's OSC half stands and ships.

## Context

**ADR-0144's central premise did not survive contact with the rig.** That decision put Resolume
Arena on a second machine in the middle of the path: Arena would own the fixture patch, the zoning
and the dimming, we would feed it a picture over NDI and a telemetry stream over OSC, and it would
emit the Art-Net. Its Alternative E — *"Art-Net or sACN emitted directly by this app"* — was
rejected in one line: *"it makes us a lighting console."*

**On 2026-08-29 a live set ran on the rejected path, and it worked.** The chain was
`lmv --osc 127.0.0.1` into a Python bridge on the same machine, emitting Art-Net straight to the
fixtures. No Arena, no NDI, no second machine. The user's verdict on the result was *"looks
amazing"*, and the user has since rejected the second machine outright and for all future work.

**Alternative E's decisive reason is not merely inconvenient here — it is false.** It rejected
direct output because *"fixture profiles, patching, zoning, dimmer curves, channel layouts and DMX
refresh timing are all real work, all already done by Arena."* This rig has none of them. It is two
**BinColor Pixel-16** Ethernet-to-SPI controllers at `192.168.1.159` and `192.168.1.160`, speaking
Art-Net on UDP 6454, net/sub `0/0`, plain RGB, universes 0-23 patched and 24-31 not. There is no
console to inherit work from and no profile to respect. What the controllers want is pixels.

**The rig is a raster, and that is the fact the whole design turns on.** Universe index tracks
height — 0-5 the bottom band, 6-11 the middle, 12-17 the upper diamond, 18-23 the apex — and one
universe drives a chain of two to three sticks at 170 pixels each. So the addressable surface is a
**24 x 170 image**, and `universe / 23` is a usable vertical coordinate. That was established by
probe on the rig, not assumed.

Two consequences of the hardware are load-bearing and were each discovered by watching something
fail:

- **The nodes latch.** They hold the last frame indefinitely. Anything that drives them and exits
  without sending black leaves the room lit until someone finds the power.
- **A frame must fill a whole universe** — 170 pixels, 510 channels. At 60 pixels the later sticks
  in each chain keep stale data, which presents as *"half the rig is broken"*.

## Decision

We will **emit Art-Net directly from `standalone`**, to a rig described by configuration, driven by
looks that are data rather than code.

**Four parts, each of which was a real choice:**

1. **The sink lives in `standalone`** — a new `standalone/src/artnet.rs`, config-gated off by
   default, beside the OSC sink ADR-0144 already shipped. `core` gains nothing: no socket, no DMX
   type, no fixture concept. This is the same shell-owns-the-policy split
   [ADR-0142](0142-the-audio-input-is-switched-live-and-the-shell-owns-the-policy.md) drew for audio
   input and ADR-0144 drew for OSC, applied a third time.
2. **The fixture map is configuration, not constants.** Node addresses, universe range, pixels per
   universe, and the mapping from universe index to a spatial coordinate all live in `config.toml`.
   The rig we have is one instance of that description, not the definition of it.
3. **A look is a TOML file in the preset expression grammar**, not Rust. The engine already
   evaluates expressions over `bass`, `mid`, `treb`, `onset`, `beat`, `tempo` and `time`; a lighting
   look is a function of those plus a spatial coordinate. Authoring one is then a file edit at 2 a.m.
   rather than a rebuild and a release.
4. **The look is selected by the operator and held across a preset rotation**, with a preset able to
   *request* a look only where the operator has allowed it. Independent is the default because that
   is the property a live rig needs; coupling is available because the room changing character with
   the screen is the more dramatic result when someone wants it.

**The OSC sink stays, and the external bridge stays a supported path.** ADR-0144's telemetry set is
shipped, tested and now dogfooded — it is what tonight's show ran on. Keeping it costs nothing, it
is the escape hatch when the in-process sink misbehaves mid-set, and it is the fast loop for trying
a look in Python before committing one to the library. This is the one half of the rejected
companion-binary alternative worth keeping.

### The picture path is why the sink is in-process, and it is the decisive argument

The lamps should eventually show **the actual rendered preset**, resolved down — that was ADR-0144's
best idea and it survives the loss of Arena and NDI completely intact. The rig is a 24 x 170 raster;
the resolve is a downsample and a copy.

**Where the Art-Net code lives decides whether that is free or whether it is a transport problem.**
In `standalone`, the resolved raster is already in the process. Anywhere else, the picture has to be
shipped across a boundary by shared memory or a second stream — which is ADR-0144's NDI question
resurrected in a smaller form, and that ADR is a record of how expensive that question is. Placing
the sink in-process now costs nothing and makes the later step a downsample instead of a protocol.

**Telemetry-driven looks ship first and the picture is the follow-on**, because
[Plan 0115](../plans/done/0115-the-engine-becomes-a-live-video-source.md)'s frame tap is approved and
unstarted, and because the telemetry look that ran tonight is already good.

### The resolve, when it comes, happens in linear light

Unchanged from ADR-0144 and restated because it is the half that would otherwise be lost with the
NDI half. Averaging display-encoded sRGB bytes underestimates mean radiance, and it errs in the
direction that makes a room too dark — the exact failure this feature exists to avoid.
[ADR-0046](0046-linear-light-hdr-composite-bloom-tonemap.md) gives the engine a linear HDR
composite, and the resolve averages the tonemapped result *before* the display encode.

### Blackout is a correctness requirement, not a courtesy

Because the nodes latch, **every** exit path must end in black: normal exit, operator quit, a
panic, and a kill. This is stated here rather than left to the plan because it is the one behaviour
whose absence is visible to an audience, and because putting the emitter in-process is what makes a
panic in the render loop able to leave a room lit. A sink that cannot guarantee it is not finished.

### Art-Net, not sACN

The rig speaks Art-Net — established by `dmxprobe.py`'s ArtPoll discovery and passive listen on both
6454 and 5568, not assumed. sACN is the better-specified protocol and this decision does not dispute
that; it is simply not what these controllers answer.

## Consequences

### Positive

- **One process runs a show.** No second machine, no second binary, no version skew between an app
  and a bridge, no OSC round trip in which the app encodes what the bridge immediately decodes.
- **The picture path becomes a downsample rather than a protocol.** See above; this is the reason
  the placement was chosen and it is worth the whole decision on its own.
- **The rig knowledge enters the project.** Latching nodes, full-universe frames, the height
  coordinate, the WireGuard trap — all of it currently lives in a folder outside version control,
  and all of it is the kind of fact that costs an evening to rediscover.
- **A look is content, and content has a lane.** Expressed in the existing grammar, a lighting look
  is `preset-author`-shaped work, testable headlessly, and it needs no engine change to add one.
- **Nothing about the standalone's existing surfaces moves.** The OSC sink, the audio input policy
  and the console are untouched; this is a third sink beside the first.
- **Cross-platform for free.** Art-Net is UDP. Unlike the capture path and unlike Spout, this half
  carries no platform asymmetry.

### Negative

- **We become a lighting console, and this ADR is where that is admitted.** ADR-0144 refused the
  title for a reason that was correct in the presence of Arena. In its absence the work does not
  disappear — patching, geometry, refresh cadence and blackout discipline all become ours. This
  decision buys directness by taking on exactly that.
- **A crash in the app is now a dark room, or worse, a frozen bright one.** Tonight's architecture
  had a genuinely valuable property that this gives up: the bridge survived the app restarting, and
  looks were iterated all evening without the visuals ever going down. Keeping the OSC path
  supported is a partial mitigation and not a full one.
- **The frame loop gains per-frame pixel work whose cost is unmeasured.** Building 24 x 170 pixels
  and sending 24 datagrams every frame is real work next to a `sendto` of 392 bytes. The Python
  bridge achieves ~32 Hz doing it; Rust will be far faster and that is not the same as free. This is
  measured in the plan, against the same property-not-threshold criterion ADR-0144's send path used.
- **The fixture geometry model is generalized from exactly one rig.** "Universe index is a spatial
  coordinate" is true of this structure and may be an accident of how it was patched. A config
  format designed against a single instance is a format that will be revised.
- **A second network path enters the show, and this one is the show.** An Art-Net outage is not a
  degraded feature, it is a dark room, and it is silent from our side.
- **Opt-in coupling reintroduces a shape ADR-0144 rejected outright.** Its Alternative F was refused
  because a preset rotation could silently rewrite an operator's mapping mid-show. Making it opt-in
  and off by default narrows that to a case someone chose; it does not delete it.

### Neutral

- The C ABI is untouched. Standalone only, so `LMV_ABI_VERSION` does not move and
  [ADR-0003](0003-c-abi-v1-surface.md)'s contract is not engaged.
- No audio crosses the sink.
- Nothing in the golden suite can cover it. The sink is wall-clock paced and its output leaves the
  process — the same position ADR-0144 and ADR-0125 both accepted.

## Alternatives considered

### Alternative A — ADR-0144 as written: Arena on a second machine, fed by NDI and OSC

**Rejected because the user rejected the second machine, and because the premise was false here.**
Arena's claim on the design was that it already owned the patch, the zoning and the dimming; this
rig has no console and no such work to inherit. The NDI licence gate that ADR-0144 identified as its
largest risk is now moot rather than resolved — it was never read, and no longer needs to be.
ADR-0144's Alternative D (OSC only) is what actually survived, exactly as that ADR predicted it
might.

### Alternative B — a companion binary in this repo consuming the OSC telemetry

An `lmv-lighting` crate reading `/lmv/v1` and emitting Art-Net. Genuinely attractive: it is what ran
tonight, it preserves the seam in which the visualizer never speaks DMX, it makes the OSC contract a
dogfooded interface, and it keeps the show-floor property that lighting can be restarted without
touching the visuals. **Rejected on the picture path** — the resolved raster would have to cross a
process boundary, which is ADR-0144's transport problem in miniature and the single most expensive
question that ADR had to answer. **Partially adopted:** the OSC sink and the external bridge both
remain supported, which keeps the escape hatch and the fast authoring loop without making the
picture path pay for them.

### Alternative C — the probes stay external, tidied into their own repository

Zero risk to the shipped app, and Python is genuinely the right language for trying a look — the
edit-run loop is seconds where a Rust rebuild is minutes. **Rejected because it answers none of the
question that was asked.** The rig facts and the engine gaps stay outside the project's memory, the
looks never get tests, CI, versioning or a release, and the picture path is impossible. The fast
loop it protects is preserved by keeping the OSC sink instead.

### Alternative D — the sink lives in `standalone`, but looks are Rust

Simpler to build, easier to make fast, and no grammar to extend. **Rejected because every new look
becomes a rebuild and a release**, which is the wrong cost curve for the thing that will change most
often, and because it puts lighting content in the `dev` lane where no one owns judging how a room
looks. The engine already has an expression evaluator and a content lane; declining to use them here
would be building a second, worse authoring surface.

### Alternative E — sACN (E1.31) instead of Art-Net

Better specified, multicast, and the modern default for new installations. **Rejected because these
controllers do not speak it.** `dmxprobe.py` listened on both 6454 and 5568 and the rig answered
Art-Net. This is a fact about the hardware, not a preference, and a rig that wants sACN later is a
second transport under the same fixture map rather than a different decision.

### Alternative F — the lighting look follows the visual preset rotation by default

The room changes character with the screen, which is the more dramatic result and needs no operator
attention. **Rejected as the default on ADR-0144 Alternative F's own reasoning:** a preset rotation
would silently change what the lights are doing mid-set, and the operator has no way to hold a look
through a section. Available opt-in, where it is a choice someone made rather than a surprise.

## Notes

**Facts this decision rests on that are established, and by what.** Unusually for a lighting ADR in
this repo, most of the rig is measured rather than assumed — the probes did that work on 2026-08-28:

1. **The controllers, addresses, protocol and port** — `dmxprobe.py`, ArtPoll plus passive listen.
2. **Universes 0-23 patched, 24-31 not; colour order plain RGB** — `artnet_test.py`, `artnet_map.py`.
3. **One universe drives a 2-3 stick chain, so a frame must fill 170 pixels** — observed as stale
   tail sticks at 60 pixels.
4. **Universe index tracks height** — `artnet_pass.py`, six universes in six colours per pass.
5. **The nodes latch** — observed by leaving without a blackout.
6. **The telemetry set is sufficient to drive a room** — `artnet_rise2.py`, and a live set.

**Facts it rests on that are NOT established, and which the plan must settle:**

1. **The per-frame cost of building and sending the rig from the render thread.** Unmeasured. The
   plan measures it against the same property ADR-0144's send path used — inside the run-to-run
   variance the same configuration shows against itself — and the dedicated-thread exit is stated.
2. **The Art-Net refresh rate these nodes want.** 40 Hz was chosen by the probes and ~32 Hz was
   achieved; neither is a measurement of what the hardware prefers, and nothing establishes whether
   a higher rate helps, is ignored, or floods them.
3. **Whether the nodes honour ArtSync**, and therefore whether 24 universes tear against each other.
   Not looked at. The probes send 24 unsynchronized `ArtDmx` packets per frame and it looked fine.
4. **Whether blackout survives a panic.** A Rust panic in the render loop unwinding to a guard that
   sends black is plausible and untested, and an abort would skip it entirely.
5. **Whether "universe index is a spatial coordinate" generalizes** past this one structure.
6. **The corner connectors carry a few pixels of their own**, showing as slivers of a neighbouring
   chain's colour at the joints. Whether that is patch, wiring or geometry is unresolved, and it
   bounds how exact any spatial mapping can be.
