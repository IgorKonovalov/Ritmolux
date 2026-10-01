# ADR-0262 — The studio renders a clip by piping three children, and transcodes what the player cannot read

> **Status:** proposed
> **Date:** 2026-10-01
> **Related plan(s):** [0247](../plans/0247-the-studio-renders-a-neural-clip.md); amends
> [ADR-0175](0175-the-studio-is-a-separate-application-that-never-draws-a-frame.md)'s *"spawned with
> progress read from its events"*

## Context

The owner wants to start a diffused music video from the studio: point at an MP3 or FLAC, pick any
library preset, and place one or several prompts across the song. Every piece of that pipeline already
exists, but none of it is reachable from the studio:

- **The renderer is a cargo example.** `shot --render` walks a WAV and writes Y4M to stdout, and
  `--bar-grid` writes the bars that walk crossed ([ADR-0114](0114-the-engine-renders-video-offline-and-delegates-encoding.md)).
  `shot` does not ship. The studio carries the *player* at `resources/player/`, and
  [ADR-0175](0175-the-studio-is-a-separate-application-that-never-draws-a-frame.md) recorded that clip
  rendering would be *"the player's `render` subcommand"*, and nothing has been built against that
  since.
- **The diffusion stage is a Python sidecar that never ships** ([ADR-0122](0122-a-sidecar-tool-documents-itself-in-one-place.md)).
  It already takes a prompt timeline on bars ([ADR-0236](0236-a-diffused-render-varies-by-prompt-on-bar-boundaries-and-the-seed-stays-fixed.md)).
- **The renderer reads 16-bit PCM WAV only**, through a hand-written reader with no decoder crate.
- **A diffused 4-minute render runs 1.4 to 15.5 hours** on the measured laptop (`docs/diffusion-filter.md`),
  so whatever starts it has to survive a night, keep the machine awake, and report progress that means
  something.

Four questions have more than one reasonable answer: who decodes MP3/FLAC, who connects the processes,
where progress comes from, and where the bars come from before a render exists. The studio needs the
bars first, because the owner places prompts on a bar strip.

## Decision

**The player gains two answer-and-exit modes, promoted from `shot`'s code:** `--render <wav>` writes
the same Y4M stream `shot --render` writes, byte for byte, and `--bars <wav>` writes the bar-grid JSON
from the analyzer walk alone, constructing no renderer and opening no GPU adapter. Both resolve a preset
against the same library the player rotates.

**The studio's main process composes the pipeline from three children**: the player's `--render`,
then the sidecar when the neural toggle is on, then `ffmpeg`. Each child's stdout is handed to the next
child's stdin as an OS pipe, so the kernel connects them directly and no frame byte passes through
JavaScript. The studio stays what ADR-0175 says it is: a control surface that never touches a frame.

**Progress is `ffmpeg -progress` read against the bar grid's `frames`.** The encoder is the end of the
chain, so its count is what has been written to disk, whichever stage is slow, and nothing new crosses
the control protocol. This amends ADR-0175's *"progress read from its events"* for clip rendering.

**MP3, FLAC and anything else ffmpeg reads are transcoded by the studio, with the `ffmpeg` the render
already requires**, into a 16-bit PCM WAV in a session cache. That one WAV feeds `--bars`, `--render`
and the encoder's audio input, so all three read identical samples. The player learns no format.

**The bars come from `--bars` and the waveform from the WAV.** The studio draws the strip from the
player's grid and its own min/max peaks of the transcoded samples. A peak envelope is a drawing of the
file, not a second estimate of anything the engine analyses, so
[ADR-0184](0184-the-player-reports-what-it-loaded-and-the-studio-re-derives-nothing.md) holds.

## Consequences

### Positive
- No new crate and no new protocol message. Nothing under `core/` moves.
- Plain and neural renders are one pipeline, with the sidecar spliced in or left out. The plain path
  and the sidecar's `--passthrough` exercise the whole of it on a machine with no CUDA.
- The player's `--render` makes clip rendering work from a packaged studio for the first time. Only
  the diffusion stage still needs a source checkout.

### Negative
- **The render dies with the studio.** A 15-hour job is a child of an Electron main process. A crash
  or a careless quit loses it, and the partial MP4 has no `moov` atom. The studio blocks app
  suspension and confirms a quit while a job runs, which does not help after a crash.
- **The shipped player grows** by the code `--render` and `--bars` pull out of `standalone::shot`,
  which today the binary does not link. NFR §4's cap has to absorb it, and the plan measures it.
- **The bar strip is mostly a fallback grid.** The downbeat estimator locks on about 3 % of audible
  time (backlog 0042), so most bars are a regular count of four that may not sit on the music's
  downbeats. The strip shows locked and fallback bars differently, which makes the problem visible
  without fixing it.
- **A transcode is a second copy of the track** on disk for the session: about 42 MB for four minutes
  of 44.1 kHz stereo.

## Alternatives considered

### Alternative A — the player decodes MP3 and FLAC itself
A decoder crate (`symphonia`, say) behind `--render`. It would make `--render` usable without ffmpeg,
except the render is never usable without ffmpeg, because ffmpeg encodes the output. It lost on that
alone: it adds a dependency and shipped bytes to the player for a need only a pipeline that already
requires ffmpeg has.

### Alternative B — a detached job that outlives the studio
The studio writes the job and a launcher, starts it under `setsid`, and polls a progress file, so a
night-long render survives a studio close and can be reattached. It would fix the worst negative above,
at the price of a launcher per platform, a progress file format, a reattach state machine and orphan
cleanup. It lost on proportion for the first version. It is the followup to reach for if a lost render
is ever reported.

### Alternative C — the player spawns the sidecar and the encoder
`--render` would grow a `--ffmpeg`-like flag that chains the sidecar too and reports progress as
events. It lost because the shipped player would learn about Python and a tool that never ships, and
the progress events would widen the control protocol to carry a number ffmpeg already reports.

### Alternative D — the studio estimates the bars itself
A beat tracker in TypeScript over the transcoded samples, so the strip appears without a player call.
It lost because it is a second estimate that disagrees with the bars the render and the sidecar actually
use, so prompts would land on bars that do not exist in the run.
