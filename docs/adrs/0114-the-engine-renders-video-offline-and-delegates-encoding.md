# ADR-0114 — The engine renders video offline and delegates encoding to a pipe

> **Status:** proposed
> **Date:** 2026-08-16
> **Related plan(s):** [0101](../plans/0101-the-engine-renders-a-music-video.md)

## Context

Every competitor that monetizes in this space sells the same thing: **a file**. Specterr, SYQEL
and ProVisHD render a music video from a track. The live visualizers — projectM, MilkDrop,
Plane9 — do not, and the reason is structural: their render loop is coupled to real time and to a
live audio device, so "export" means screen-capturing the window and accepting whatever the
machine could keep up with.

**This engine is not coupled that way, and it is the only one in the open-source segment that
isn't.** Three decisions already shipped make an offline render a small feature rather than a
rewrite: `dt` is **injected** by the caller rather than read from a clock
([ADR-0013](0013-c-abi-v4-render-dt.md), C ABI v4); DSP output is a **pure function of its input
window** with no wall-clock read ([NFR §6](../nfr.md#6-determinism)); and visual randomness is
**explicitly seeded**, with every capture path forcing the declared number
([ADR-0051](0051-seeded-grammar-randomness-with-per-run-opt-in.md)). The headless path already
exists and is exercised on every push — `Renderer::new_headless` draws into an offscreen texture
and returns raw RGBA, and `shot` drives it over a WAV clip today.

So the engine can already walk a track at a fixed 1/60 s step, faster or slower than real time,
and produce **byte-identical output on two runs**. That is a capability, not an aspiration; the
golden suite depends on it. What is missing is only the part that turns a frame sequence into a
file.

**The constraint that decides this is arithmetic.** At 1920x1080 a frame is 8.29 MB of RGBA
(`1920 x 1080 x 4`). Sixty of those a second is **498 MB/s**, and a four-minute track is 14,400
frames — **119 GB** if the frames are written to disk before encoding. Chroma-subsampled or
PNG-compressed the number falls by a factor of three or four and stays in the tens of gigabytes.
There is no version of "write the frames out, encode them afterwards" that is not absurd.

The other constraint is [NFR §4](../nfr.md#4-size-and-dependencies): a **~10 MB soft cap** on the
shipped executable, with wgpu named as the one accepted fixed cost. Even a minimal single-codec
static build of a general encoder exceeds that cap on its own, and a general-purpose one is
several times it. Bundling an encoder would make the export feature cost more binary than the
entire application.

## Decision

We will add an **offline render mode to `shot`** that walks a WAV clip at a fixed frame step and
**streams frames over a pipe to an external encoder**, and we will ship **no encoder**.

- The render loop reads the clip, advances the analyzer hop by hop and the renderer by a fixed
  injected `dt`, and writes each frame to **stdout** (or a named pipe) in a **self-describing
  stream format** — one that carries its own dimensions, frame rate and colour range, so a
  mis-typed geometry argument cannot silently produce garbage. `ffmpeg` reads it natively.
- The source WAV is passed through untouched; muxing audio to video is the encoder's job.
- A convenience flag **spawns a user-supplied `ffmpeg`** and wires the pipe, so the common case is
  one command. Its absence is a named error, never a bundled fallback.
- The mode is **`shot`-only** — dev and creator tooling, not a button in the app. `image` is
  already a dev-dependency for exactly this reason ([ADR-0011](0011-image-crate-for-capture-tooling.md)),
  and the shipped `lmv.exe` gains nothing.

The property this mode must hold, and the one worth testing, is **reproducibility**: the same
command over the same clip produces the same bytes. That is what distinguishes this from a screen
capture, and it is the whole reason the feature is cheap here and expensive everywhere else.

## Consequences

### Positive

- **A market segment opens that no live visualizer serves.** "Render a music video from a track"
  is what the paying half of this field sells, and this engine can do it deterministically, at
  any resolution, faster or slower than real time, with no dropped frames — because nothing in the
  path is racing a display.
- **Quality stops being bounded by the machine.** An offline render at 2560x1440 on the `Rich`
  tier is not competing with a 60 Hz deadline, so the frame-time governor never fires and the
  heaviest presets render at full cost.
- **It costs the shipped binary nothing.** No new runtime dependency, no size movement, no code on
  a hot path.
- **The instrument is reusable.** The same loop produces the repository's own demo material, which
  [Plan 0103](../plans/0103-the-project-gets-an-audience.md) needs and currently has no way to
  make: every image in this repo is a still, because there was no way to record a moving one.

### Negative

- **`ffmpeg` becomes a documented prerequisite for one feature.** A user who wants an MP4 must
  install it. This is a real friction cost, accepted deliberately over a 10x binary-size increase,
  and it is the standard posture for tools of this size.
- **Colour is ours to get wrong.** The tonemap hands the pipe display-referred sRGB; the stream
  header must declare range and primaries correctly or the exported file is visibly washed out
  against the app, in a way that looks like an engine bug and is not. This is a whole phase, and
  it is the part most likely to ship subtly broken.
- **Long renders inherit an unfixed defect.** `shot --horizon` currently dies at ~3,601 frames at
  ~2.9 GB resident ([Plan 0099](../plans/0099-the-horizon-reaches-its-own-length.md), design-backlog
  0093). A four-minute video is 14,400 frames — **four times past where the existing long-run path
  already fails**. This feature is blocked on that repair, and saying so is the point of naming it
  here.
- **Nothing validates the output file.** We test that the frames are right and that the stream is
  well-formed; whether `ffmpeg` produced a good MP4 is outside the harness.

### Neutral

- The mode is inherently slower than real time on heavy presets, and that is fine — it is the
  first path in this project where wall-clock cost is not a correctness property.

## Alternatives considered

### Alternative A — bundle a static encoder

Link libx264/libvpx, or ship a vendored `ffmpeg`, so export is self-contained. **Rejected on
[NFR §4](../nfr.md#4-size-and-dependencies).** Even a minimal single-codec static build is larger
than this application's entire soft cap; a general one is several times it. Paying that on every
download so that a minority feature needs no prerequisite inverts the project's stated priority
that lightweight is a feature.

### Alternative B — a pure-Rust encoder crate

Encode in-process with a Rust codec crate. **Rejected because the mature options are bindings to
the same native libraries Alternative A was rejected for**, and the genuinely pure-Rust ones are
young enough that a broken export would be a dependency bug we could not fix. Either way it is a
large new dependency for a peripheral feature, which is precisely what the NFR §4 gate exists to
stop.

### Alternative C — write a frame sequence to disk, encode afterwards

Emit PNGs or raw frames, then run an encoder over the directory. **Rejected on the arithmetic
above**: 8.29 MB a frame at 1080p is 119 GB for four minutes raw, and tens of gigabytes
compressed. It also doubles the wall-clock cost by writing and re-reading every frame. A pipe is
not an optimization here; it is the only viable shape.

### Alternative D — screen-capture the running window

Point OBS at `lmv.exe`. **Rejected because it discards the one advantage this engine has.** A
capture is coupled to real time, so it drops frames under load, cannot exceed the display's
refresh, cannot exceed the display's resolution, and is not reproducible. That is what every
competitor is stuck with; choosing it deliberately would be choosing to be indistinguishable from
them.

## Notes

- The colour question is not hypothetical: the composite is linear-light `Rgba16Float` end to end
  and only becomes display-referred at the tonemap ([ADR-0046](0046-linear-light-hdr-composite-bloom-tonemap.md)),
  and the display write dithers in the **encoded** domain
  ([ADR-0096](0096-the-display-write-dithers.md)). The export tap must sit **after** both, or the
  file will not match what the app shows.
- [ADR-0099](0099-the-show-length-horizon-is-a-spot-check-and-it-splits-in-two.md) already
  established that this project's long-run instruments are spot checks rather than gates. An
  export render is a third long-run path with a fourth failure mode (encoder backpressure on the
  pipe), and it is not a gate either.
