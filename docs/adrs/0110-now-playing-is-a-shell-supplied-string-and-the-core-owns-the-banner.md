# ADR-0110 — Now-playing metadata is a shell-supplied string, and the core owns the banner

> **Status:** proposed
> **Date:** 2026-08-16
> **Related plan(s):** [0097](../plans/0097-the-track-announces-itself.md)

## Context

The user asked whether the visualizer can show what is playing right now. Both frontends can
answer that question, and they answer it from completely different places.

**The plugin already has the metadata and cannot draw it.** foobar2000 hands a component the
track directly — `play_callback` fires on every track change and `titleformat` renders
`%artist% - %title%` from the `metadb_handle`. No guessing, no extra dependency, exact strings.
But the plugin has no text path at all: it is a C++ shim over the C ABI, and the C ABI
([`docs/specs/0001-c-abi.md`](../specs/0001-c-abi.md)) has no text entry point.

**The standalone can draw it and has to guess at the metadata.** It already owns a text layer
(glyphon, [ADR-0009](0009-glyphon-text-rendering.md)) and paints the preset name every frame.
What it lacks is a source. On Windows the supported one is WinRT's
`GlobalSystemMediaTransportControlsSessionManager` (SMTC) — the same feed that drives the OS
media flyout — which reports whatever app is publishing now-playing, foobar2000 included. That
is a **feature flag on the `windows` crate the shell already depends on**, not a new crate, so
NFR §4's dependency gate is not tripped by the standalone half. On macOS there is no supported
equivalent: `MediaRemote` is a private framework, and third-party access to it was restricted in
recent macOS releases (*unverified — confirm before any Mac phase is scheduled*).

**The core must not learn what either source is.** A WASAPI, WinRT, or foobar type inside
`core/` is the one violation the whole two-frontend split exists to prevent. Whatever shape this
takes, the core has to receive metadata the same way it receives audio: as a value that carries
no evidence of where it came from.

That leaves one genuine tradeoff, and it is about **fonts**. The core has *two* text
capabilities, and they are not interchangeable:

| | `overlay_font.rs` (quad font) | glyphon (`text` feature) |
|---|---|---|
| In the plugin build? | **Yes** — every build | No — standalone only |
| Dependency cost | Zero | cosmic-text tree; NFR §4 gate |
| Coverage | **31 glyphs**: digits, `. / : *`, and the 16 uppercase letters the diagnostics readout happens to use | System font shaping, full Unicode |
| Failure mode | **Silent blank** (documented, by design) | Font fallback |

The quad font covers `F P S M B L O R I C H A D E K N T` — it has no `G`, `J`, `Q`, `U`, `V`,
`W`, `X`, `Y`, `Z`, no lowercase, and no punctuation, because it was built to spell `FPS`, `MS`,
`MB`, `BASS`, `ONSET` and `LOCK`. Its own module doc states the consequence: *"an uncovered
character renders blank, not an error … a confident gap and nothing in the engine notices."*
That is the correct design for a fixed debug readout and the wrong one for arbitrary input.

## Decision

We will treat now-playing as a **UTF-8 string the shell pushes into the core**, and the **core
owns the banner** — the string, its `dt`-driven fade envelope, and its placement — drawn through
the **glyphon** text seam, which `core-cabi` enables for the plugin build. The standalone calls
the Rust method directly; the plugin calls a new `lmv_set_now_playing` entry point, which is a C
ABI shape change and therefore bumps `LMV_ABI_VERSION` per
[ADR-0003](0003-c-abi-v1-surface.md). The core copies the string on receipt and never retains
the caller's pointer. The banner is **transient**: it appears on a track change, holds a few
seconds, and fades out, leaving a clean canvas.

The core learns nothing about the source. `lmv_set_now_playing(handle, utf8, len)` is the same
kind of boundary `lmv_push_samples` already is — SMTC and foobar's `titleformat` are
indistinguishable to everything downstream of it.

**The plugin's glyphon cost is not assumed, it is measured.** Plan 0097 Phase 4 records the DLL
size before and after enabling the `text` feature and stops against NFR §4's ~10 MB soft cap; if
it blows the cap, the plugin half falls back to Alternative A below or ships without the banner,
and the standalone half — which is complete on its own — is unaffected either way.

## Consequences

### Positive

- **One banner, not two.** The fade envelope, the layout, and the truncation rule live in one
  place, so the two frontends cannot drift on what a track change looks like. A shell's whole
  job is to supply a string.
- **The standalone half needs no ABI change and no new crate** — a `windows` feature flag and a
  core method. It is shippable before the plugin work starts, which is why Plan 0097 orders it
  first.
- **`dt`-driven fade is frame-rate independent for free**, since Plan 0014 already injects real
  `dt` (`lmv_render_dt`). The envelope is a pure function of elapsed time, so it is unit-testable
  without a GPU and identical on a 60 Hz and a 165 Hz display.
- **The core stays source-agnostic**, and demonstrably so: the entry point's signature contains
  no type that names an audio source or a platform.

### Negative

- **The plugin DLL grows by the cosmic-text tree.** This is the real price. NFR §4 sets a ~10 MB
  soft cap and gates any crate pulling >~20 transitive deps; glyphon clears that bar comfortably.
  The measurement and its stop condition are Plan 0097 Phase 4, not an assumption made here.
- **The C ABI grows a thirteenth function and `LMV_ABI_VERSION` moves.** The C++ shim is compiled
  separately, so a mismatch fails at link time or at runtime — the plugin must be rebuilt in
  lockstep, and [`docs/specs/0001-c-abi.md`](../specs/0001-c-abi.md) is updated in the same phase.
- **macOS standalone gets no metadata source.** The banner will exist on Mac and simply never be
  fed, exactly as the Mac loopback path is the asterisked one. The plugin path is the answer
  there — the same argument that already makes plugin parity valuable on Mac.
- **A string crossing the ABI is a lifetime hazard.** The rule is stated once and defended by the
  spec: the core copies on receipt, the caller may free immediately, and the core never retains
  the pointer. Getting this wrong is a use-after-free that will not reproduce reliably.
- **Metadata arrives on a callback that is not the audio callback**, and the distinction has to
  hold. `play_callback` / the SMTC event thread may allocate; `lmv_set_now_playing` must never be
  called from the `visualisation_stream` or capture thread, where the copy would be an allocation
  on the sacred path.

### Neutral

- **The preset name stays shell-drawn while now-playing is core-drawn.** An asymmetry, and an
  accepted one: the preset name is a roster concept the standalone owns (the plugin has no
  browse overlay), while now-playing is the thing both frontends must render identically. If the
  plugin ever grows a preset HUD, this ADR is the precedent to follow rather than a rule to fix.
- **Not every player publishes SMTC.** The standalone shows a banner for those that do and stays
  silent for those that do not, which is the correct degradation (NFR §10) rather than an error
  state. Whether foobar2000 v2 publishes SMTC out of the box or needs a component is
  *unverified*; the plugin path does not depend on the answer.

## Alternatives considered

### Alternative A — Extend the core's quad font and draw the banner with it

Zero new dependency, works in the plugin today, and the migration note in `overlay_font.rs`
anticipates growth. It loses on **input**: track metadata is human-authored text in arbitrary
scripts, and a real library routinely contains `Björk`, `Sigur Rós`, `Édith Piaf`, Cyrillic and
CJK titles. Growing the table from 31 to a full ASCII ~96 glyphs is a day of bitmap authoring
that still renders `Björk` as `Bj rk` — silently, because a blank is this font's documented
failure mode. Spending real effort to arrive at a wrong answer that nothing reports is worse
than the size cost. It survives as the **fallback** if Phase 4's measurement rejects glyphon,
where "ASCII titles render, others show gaps" beats "the plugin has no banner at all".

### Alternative B — Each shell draws its own banner

The standalone would need nothing new at all — it already queues `TextRun`s and could paint this
in an afternoon. It loses decisively on the plugin: the shim is C++ with no text path, so
"each shell draws its own" means the plugin never gets one. That is precisely the outcome this
ADR exists to avoid, and the user asked for both frontends.

### Alternative C — The core reads the metadata itself

Fewest moving parts at the seam: no ABI string, no shell wiring. Rejected outright — it would
put WinRT and foobar SDK types inside `core/`, breaking the source-agnostic rule that
[ADR-0001](0001-rust-core-wgpu-cabi-foobar-shim.md) rests on, and it would not even work, since
the plugin's metadata comes from a host the core cannot query.

## Notes

- The SMTC surface the standalone needs:
  `GlobalSystemMediaTransportControlsSessionManager::RequestAsync()` →`GetCurrentSession()` →
  `TryGetMediaPropertiesAsync()` for `Artist()` / `Title()`, plus the `MediaPropertiesChanged`
  and `CurrentSessionChanged` events so the banner is driven by track changes rather than polled.
  It is a WinRT namespace, so the `windows` crate needs the `Media_Control` feature and a live
  COM apartment — the shell already initializes COM for WASAPI, but whether that apartment
  satisfies WinRT is a Plan 0097 Phase 2 risk, not a settled fact.
- The transient presentation was chosen against a persistent corner line in the design interview:
  it keeps the canvas clean for a live show, and it matches how a track change actually feels —
  an announcement, not a status bar.
</content>
</invoke>
