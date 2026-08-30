# ADR-0125 — The live video-out is a Spout sender fed by a headless frame tap

> **Status:** accepted 2026-08-30
> **Date:** 2026-08-25
> **Related plan(s):** [0115](../plans/done/0115-the-engine-becomes-a-live-video-source.md)

## Context

The user wants this engine's picture as a live input in **TouchDesigner on the same Windows
machine**, with **no window** on our side — the app becomes a headless video source feeding
somebody else's composite. That is one step further out than
[NFR §10](../nfr.md#10-live-performance-added-in-the-2026-07-21-follow-up-interview)'s "renders to
a projector", and it is the same live-show use.

**Same machine is the fact that decides this, and it was not the first fact we had.** The question
was first asked with TouchDesigner on another machine over WiFi, which makes the transport a
bandwidth problem — NDI's intra-frame SpeedHQ is roughly **100–130 Mbps at 1080p60** by its own
published figures, against an estimated 10–20 Mbps for h.264, and a shared jitter-prone link has no
answer for the former. On one box that entire argument is void: there is no link. What is left is a
different question — how do two GPU applications on one machine hand each other pixels — and it has
a standard answer in this exact community.

**Spout is that answer.** It is the Windows app-to-app video path the TouchDesigner world already
uses, TouchDesigner ships a native **Spout In TOP**, and Spout is **BSD-licensed**, so it can be
vendored and pinned the way [ADR-0115](0115-the-foobar-component-is-a-released-artifact-with-a-parameterized-sdk.md)
already stages the foobar2000 SDK. Nothing is installed at runtime, nothing is negotiated with a
vendor, and no codec sits between our tonemap and TouchDesigner's input. NDI on localhost would
still work — but it would encode and decode SpeedHQ to cross a machine boundary that is not there,
paying CPU and a generation of quality for nothing, and it would add an SDK that is **not**
redistributable on the same terms.

**The one hard part is the seam to wgpu, and it has a cheap door.** Spout shares a **D3D11**
texture; this engine renders through wgpu on DX12 or Vulkan
([ADR-0001](0001-rust-core-wgpu-cabi-foobar-shim.md)). Handing our texture to Spout directly means
creating a shared D3D12 resource, importing it through `wgpu-hal`, and wrapping it for Spout via
`D3D11On12` — unsafe interop at precisely the seam ADR-0001 exists to keep abstract. But Spout's
`SpoutDX` class also takes **pixels**: it owns its own D3D11 device, accepts a CPU frame buffer,
and does the upload and the sharing itself. The engine already reads frames back on every capture
path, so the CPU door costs one readback and one upload per frame — measurable, not structural —
and it costs **zero** unsafe GPU interop.

## Decision

We will add a **frame tap** to `core`'s render API — a persistent offscreen target plus readback
buffer, drawn through the same `draw_frame` the window presents through, at a caller-supplied
per-frame `dt` — and a **headless `stream` mode** in the standalone that drives that tap from the
live loopback ring at wall-clock cadence and publishes each frame as a **Spout sender** through
`SpoutDX`'s CPU pixel entry point. TouchDesigner receives it with a Spout In TOP.

**The tap lives in `core`; the Spout sender lives in `standalone`, and that split is the point.**
Spout is Windows, D3D11 and third-party — everything the source-agnostic, GPU-abstract core rule
forbids. `core` produces frames and knows nothing about where they go; the shell owns the sink,
exactly as it owns the audio source. The tap is therefore **transport-agnostic by construction**,
which is what makes the remote case a later sink rather than a later rewrite.

The Spout dependency is **feature-gated off by default** and Windows-only, so an ordinary
`cargo build`, CI, and the macOS target are untouched by it; the SDK is **fetched against a pinned
hash and never committed**, per ADR-0115. The release build turns the feature on after staging it,
the way the foobar component's release job already does.

We take the **CPU pixel path, not zero-copy texture sharing** — and this is a decision with a
stated exit: if measurement shows the readback-plus-upload is what limits the frame rate, the
zero-copy path is the known fix and the tap's shape does not change to accommodate it.

## Consequences

### Positive

- **No codec anywhere.** TouchDesigner receives the exact 8-bit frame the app would have presented,
  with no encode, no decode, and no generation loss. NDI and the h.264 pipe both spend quality and
  CPU to cross a boundary that does not exist here.
- **Latency is a frame or two**, not the 150–400 ms an encode-transport-decode round trip costs.
- **No runtime install and no vendor relationship.** Spout is BSD, vendored by a pinned fetch. The
  user runs `lmv.exe` and opens a TOP.
- **No unsafe GPU interop, and no backend pinning.** The wgpu seam stays abstract; Spout's D3D11
  device is entirely its own and never meets ours. ADR-0001's abstraction survives intact.
- **The tap serves the case we do not build.** A remote sink — the h.264 pipe into `ffmpeg`, which
  [ADR-0114](0114-the-engine-renders-video-offline-and-delegates-encoding.md)'s machinery already
  most of — attaches to the same tap. The user has said remote matters later; this makes later
  cheap.
- **The binary does not grow for anyone who does not ask.** Feature off by default, ~hundreds of KB
  when on, against the ~10 MB soft cap in [NFR §4](../nfr.md#4-size-and-dependencies).

### Negative

- **Windows only, and that is not a gap we can paper over.** Spout has no macOS form; the Mac
  analogue is Syphon, a different SDK with a Metal/IOSurface seam. A Mac user gets no video-out
  from this decision. The platform asymmetry is the same shape as loopback capture's and is
  accepted for the same reason.
- **Two copies of every frame.** GPU→CPU readback then CPU→GPU upload, ~8.29 MB each way at 1080p
  (`1920 x 1080 x 4`), ~498 MB/s each way at 60 fps. Zero-copy sharing would be neither. This is the
  price of not writing `wgpu-hal` interop, it is **unmeasured**, and Plan 0115 measures it before
  anything is optimized.
- **A C++ build step enters the standalone's release path.** Today `lmv.exe` is pure `cargo`; with
  the feature on it needs MSVC and a staged SDK. The foobar component already carries exactly this
  cost, so the pattern is proven — but it now reaches a second artifact.
- **Colour interpretation is a real hazard and nothing in this repo can catch it.** The tap reads
  back `Rgba8UnormSrgb` — display-referred bytes, dithered in the encoded domain
  ([ADR-0096](0096-the-display-write-dithers.md)) — and Spout publishes an
  unlabelled 8-bit texture. Whether TouchDesigner treats those bytes as sRGB or as linear is a
  TouchDesigner-side setting, and getting it wrong yields a washed-out or crushed picture that
  looks like an engine bug and is not one. This project has shipped a wash-out class of defect
  before ([Plan 0111](../plans/done/0111-the-milkdrop-import-stops-washing-out.md)); here the only
  instrument is a human comparing the TOP against a `shot` PNG, and Plan 0115 makes that a phase.
- **Nothing in the golden suite can cover this mode.** It is wall-clock paced, so its output is not
  reproducible and no test will assert on it. Every claim about the live path is either a
  byte-identity claim against a deterministic path or a human reading.
- **A second render loop exists** — the window's `render` and the stream's — walking the same
  `draw_frame` but separately paced. Only one runs per process, but a change made to one and not
  the other is now possible in a way it was not before.

### Neutral

- The stream carries no audio. TouchDesigner takes audio from its own source; Spout is a video
  transport and has nothing to carry it in.
- Headless costs no swapchain and no window, so this mode's GPU footprint is smaller than the
  app's, not larger.
- Spout's own D3D11 device holds one or two extra frame-sized textures (~8–17 MB at 1080p) in a
  process that, per [ADR-0010](0010-accept-gpu-driver-memory-floor.md), is already
  driver-dominated.

## Alternatives considered

### Alternative A — zero-copy: share the render target with Spout directly

Create the render target as a shared D3D12 resource, import it through `wgpu-hal`, wrap it for
Spout with `D3D11On12`, and publish the handle — no readback, no upload, nothing per frame.
**Rejected for now on risk, not on merit.** It is strictly the better transport and it is also
unsafe interop against a raw D3D12 device, at the one seam ADR-0001 abstracts, on a backend wgpu is
free to choose; it would pin the backend and put `unsafe` in the shell for a feature whose cost has
not yet been shown to matter. The CPU path is one readback the engine already performs on every
capture. If measurement convicts it, this is the fix, and nothing in the chosen design has to move
to adopt it.

### Alternative B — NDI

Link an NDI binding and appear in TouchDesigner's NDI In TOP by name. **Rejected because on one
machine it pays for a boundary that is not there**: SpeedHQ encode and decode, CPU on both sides,
and a generation of 8-bit quality loss, to move pixels between two processes on one GPU. Its real
advantages — auto-discovery and working across machines — are advantages for the *remote* case, and
that case is deferred. It also brings an SDK required at build time, a runtime the user must
install on both ends, and a redistribution question Spout's BSD licence simply does not have. This
is the alternative to revisit if remote becomes the primary case rather than a follow-on.

### Alternative C — TouchDesigner's Shared Mem In TOP

Write frames into a named shared-memory block in TouchDesigner's documented layout and read them
with a Shared Mem In TOP. **Rejected on lock-in and fragility, having been genuinely competitive on
cost** — it needs no third-party code at all, since the `windows` crate the standalone already
links provides the file mapping. But it works with TouchDesigner and nothing else, it makes us
responsible for matching a header layout and a synchronisation protocol we do not own and cannot
test against, and it is not what this community's tooling speaks. Spout costs a vendored BSD
library and buys a transport every VJ application on Windows already reads.

### Alternative D — the h.264 pipe into `ffmpeg`, pointed at localhost

Reuse ADR-0114's shipped machinery: render headless, write Y4M into a spawned `ffmpeg`, serve SRT
or RTSP, read it in a Video Stream In TOP. **Rejected for the local case because it is the same
objection as NDI, larger**: encode plus decode plus a network stack, 150–400 ms of latency, and a
pipeline the user assembles by hand — all to cross one process boundary. It remains the right
answer for the **remote** case precisely because it is the only option here that survives a WiFi
link, and it is recorded as the follow-on rather than as a rejection.

## Notes

- Two figures here are of different quality and are labelled as such. NDI's ~100–130 Mbps at
  1080p60 is **vendor-published**. The readback/upload bandwidth is **arithmetic** from the frame
  size. The claim that this costs one to two frames of latency is an **estimate**, and Plan 0115's
  human gate measures it rather than inheriting it.
- Spout2's BSD licence, the exact `SpoutDX` CPU entry point and its channel-order and row-order
  flags, and TouchDesigner's colour handling of an unlabelled 8-bit Spout texture are all
  **unverified in this repo**. Plan 0115's first phase establishes all four with the SDK's own
  demo sender and no engine code written, so a wrong belief here costs an hour rather than a plan.

## Outcome — 2026-08-30, at Plan 0115's close

The decision stands and shipped. Five things this ADR left open are now answered, and one thing it
asserted is wrong.

- **The stated exit was measured and is not taken.** Over a 30-minute run at 1280x720/60 the two
  stages read **3.67–7.82 ms render+readback against 0.27–0.58 ms Spout send**. The sink is an
  order of magnitude off being the limit, so Alternative A's zero-copy path is not convicted and
  stays unbuilt. That measurement is on this machine's RTX 3080 and does not travel (ADR-0071).
- **The CPU path costs the two copies budgeted, not three.** A pixel-format hypothesis — that Spout
  wanted BGRA, needing a per-frame `rgba2bgra` — was tried, published, and **refused by the
  receiver anyway**; the real cause was the adapter (ADR-0146). `R8G8B8A8_UNORM` with no conversion
  is correct.
- **Colour needs no setting on either side.** The receiving TOP and a `Movie File In` TOP of the
  same reference PNG are indistinguishable, and so is the streamed picture against a
  `shot --frame-at` PNG of the same preset and size. The hazard this ADR said nothing in the repo
  could catch did not materialize.
- **The licence is Simplified BSD**, and clause 2 binds the release archive because Spout links
  **statically** — no DLL travels, so the notice does. The release zip carries and verifies
  `spout-license.txt`.
- **Four `extern "C"` entry points became six.** `resize` is unnecessary — `SendImage` carries the
  dimensions and resizes in place under the same sender name — while `name`, and the two adapter
  functions ADR-0146 needs, are not.
- **WRONG IN THIS ADR: the receiving operator is `Syphon Spout In`, not `Spout In`.** No TOP by the
  latter name exists; Derivative ships Spout and Syphon as one operator. The name appears
  throughout this document and is left as written, per the append-only rule — the docs a reader
  follows (`README.md`, `docs/capturing.md`) say `Syphon Spout In`.

Two figures this ADR states at 1920x1080 — 8.29 MB/frame and ~498 MB/s — were **never confirmed end
to end**, because the receiving install is TouchDesigner Non-Commercial and caps the TOP at
1280x1280. Phase 6's "largest size and rate" sweep was deferred for time and is bounded above by
that cap regardless, so the size ceiling of this transport remains unmeasured on this machine.
