# ADR-0121 — The diffusion filter is an offline stage with profiles, and it interpolates its own stride

> **Status:** proposed
> **Date:** 2026-08-20
> **Related plan(s):** [0106](../plans/0106-the-frame-stream-passes-through-a-diffusion-model.md)

**On the number.** This is 0121 and 0120 is skipped deliberately.
[Plan 0111](../plans/0111-the-milkdrop-import-stops-washing-out.md) Phase 3 names **ADR-0120** as
the ADR it writes if its bisect names a stage whose semantics are a decision, and that lane is live
in a parallel worktree. A gap in the sequence costs nothing; two documents claiming one number costs
a citation that silently resolves to the wrong decision. If 0111 takes its stop branch, 0120 stays
free for the next taker.

## Context

[Plan 0106](../plans/0106-the-frame-stream-passes-through-a-diffusion-model.md) deferred this ADR on
purpose: it is written **between Phases 2 and 3**, against the spike's evidence rather than against
a guess about what a diffusion pass would look like. That was the right call, because the evidence
overturned the plan's own cost model and changed what the filter has to be.

**The gate passed, and it asked for something the plan excludes.** Phase 2's look gate returned
*usable with feedback* and *the music reads*, so the no-audio-conditioning decision stands — the
image is still the whole signal. But the ask that came back with the verdict was a **realtime path
and higher resolution**, and the plan's own *What this plan does NOT do* names both: *"No real-time
or live path"* and *"near-real-time preview ... is a followup, not a scope."*

**Three measurements decide most of what follows.** All are from the dev box (RTX 3080 Laptop 8 GB,
torch 2.6.0+cu124, Python 3.12) and all name that configuration rather than claiming to be
properties ([ADR-0071](0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md)):

1. **The look rides on classifier-free guidance, not on step count.** LCM-LoRA at the schedule it is
   sold on — 4 steps, `cfg 1.0` — returns a smoothed render with no material. **8 steps at
   `cfg 1.0` is still flat at every strength tried; the same 8 steps at `cfg 2.0` is the picture the
   gate approved.** A dedicated guidance-free finetune (`Lykon/dreamshaper-8-lcm`, `cfg 1.0`) fails
   identically. Guidance costs a second UNet evaluation per step, and **two independent `cfg 1`
   configurations agree that the cost is not removable by model choice.**
2. **The banked speedup is 3.1x with the look intact**: 3.60 -> **1.164 s/frame** at 512x512 over
   120 frames, at boil 1.09 against the 20-step cell's 1.03. Locked 30 fps needs 0.033 s/frame, so
   **~35x remains**, against remaining named levers worth ~2x (TensorRT) and ~1.3x (a T2I-Adapter in
   place of ControlNet).
3. **Coherence is bought by the feedback blend and costs throughput.** Feedback 0.40 / 0.60 / 0.75
   measure boil 1.09 / 0.93 / 0.81 at 1.164 / 1.349 / **1.611** s/frame. Below 1.0 the output moves
   *less* than the render driving it. So **feedback is a quality lever and structurally never a
   realtime one** — the thing that makes it look right makes it slower.

**And one thing the gate accepted breaks a contract the plan states as absolute.** The user judged a
**strided** render — diffuse every 3rd frame, interpolate back to 30 fps — *"actually fine"*. The
plan's Data shapes says: *"Frame count in equals frame count out, always. A filter that drops or
duplicates a frame desynchronizes the audio mux downstream, and that failure is silent in the
file."* Stride consumes N frames and produces one. That is a real decision, not an oversight to
route around, because the failure it guards against is **silent in the output file** — nothing
downstream reports it and no test in this repo would see it.

Stride is also the only rate lever that is simultaneously a speed lever: per-diffused-frame cost is
unchanged, so stride N is exactly N x fewer frames. Its ceiling is **musical rather than
computational** — at ~118 bpm a beat is ~15 frames at 30 fps, so N=8 leaves under two diffused
frames per beat and the geometry stops tracking the music before the picture stops looking smooth.

## Decision

**The diffusion filter stays a single offline stdio stage, it exposes its cell as flags plus named
profiles, and it owns its own temporal interpolation so that frame count in still equals frame count
out.** Concretely, and each clause is load-bearing:

- **One program, not two.** `tools/sd-filter/` keeps every knob as a flag — `--prompt --negative
  --strength --cn-scale --feedback --stride --steps --size --seed --control --model --controlnet` —
  and adds **named profiles** (`--profile <name>`) that set a known-good combination. The profile is
  what a user types; the flags are what a profile is made of, and any flag passed explicitly
  overrides the profile so a render is never trapped inside a preset. The profile actually used is
  **echoed on stderr as its expanded flag list**, so a result can always be reproduced from what ran
  rather than from a name whose meaning moved.
- **The frame-count contract is preserved, and the filter pays for it.** With `--stride N` the
  filter consumes N frames, diffuses one, and **emits N frames**, interpolating between successive
  diffused outputs. The canonical `ffmpeg` invocation in `docs/capturing.md` does not change, and
  no A/V desynchronization is representable. The interpolator is the filter's dependency to carry.
- **Resolution is bought natively, not by upscaling.** The quality profile diffuses at **768x768 or
  above**, at the measured 2.7x per-frame cost of 512, because the detail is then generated rather
  than inferred. No upscaler dependency is taken.
- **Realtime is out of scope for Plan 0106 and becomes its own plan with its own ADR.** This ADR
  records the measurements that plan will start from, and records that it reopens two of Plan
  0106's rejected alternatives, but decides nothing about its architecture.

## Consequences

### Positive

- **Phase 3's exact-bytes property survives untouched.** The stub is still `frames in == frames
  out`, byte-identical, no GPU and no weights — the one part of this feature that can be a real CI
  gate, and the decision above deliberately refuses to spend it.
- **A render is reproducible from its own stderr.** Profile expansion is echoed, so the argument
  list that produced a file is recoverable without knowing which version of a profile was current.
- **Stride is available in the quality path, not only in a hypothetical realtime one.** A 4-minute
  track at 30 fps is 7 200 frames; at the LCM cell's 768 cost (~3.1 s/frame) that is **~6.3 hours**,
  and at `--stride 3` it is **~2.1 hours**. The lever the gate liked for its *look* is also what
  makes an overnight render an afternoon one.
- **Plan 0106 stays closeable.** Phases 3-5 are unblocked and small; nothing waits on the ~35x
  problem.

### Negative

- **The filter takes an interpolation dependency it would not otherwise need**, and interpolation
  quality is now the filter's problem. `ffmpeg`'s `minterpolate` — what the judged clip used — is
  **CPU-bound and cannot be the in-filter implementation for anything fast**; a GPU interpolator
  (RIFE class) is a further dependency, and the plan ships *no* weights today. The honest first
  implementation may be a **held frame** (repeat rather than interpolate), which preserves the
  contract with no new dependency and produces a deliberately stepped 30/N look. **This has not
  been compared against the interpolated version by anyone** — the gate saw only the interpolated
  clip — so the implementing phase must render both and ask.
- **Native 768 is 2.7x the per-frame cost, and it does not buy stability** — boil measures 1.03 at
  both 512 and 768, so the extra spend is detail only. It also forecloses SDXL on this box: SDXL
  plus ControlNet is ~7.5 GB against 8 GB, and the spike already peaks at 5.68 GB with two
  ControlNets loaded.
- **Profiles are a second surface that can rot.** A profile whose meaning drifts silently
  invalidates every render that named it. Mitigated by the stderr echo, not eliminated by it.
- **Realtime remains unsolved and now has a named wall.** Anyone reading this should not expect the
  next increment to be small: the remaining bounded levers do not reach 35x, and the unbounded one
  (residual CFG) is engineering rather than configuration.

### Neutral

- The `boil` statistic — `mean|out[i]-out[i-1]| / mean|src[i]-src[i-1]|` — is a **spike instrument
  and does not become a repository test.** It is dimensionless and same-kind
  ([ADR-0074](0074-a-ratio-against-an-in-run-control-is-not-automatically-portable.md)) but it is
  only interpretable when the source is moving: a near-static preset reads 5.26 while looking calm,
  because the denominator collapses. Nothing here is reproducible across machines
  (fp16 reduction order, cuDNN autotuning), so no diffused frame may enter `core/tests/golden/`.

## Alternatives considered

### Alternative A — emit at 30/N fps and teach `ffmpeg` the rate

The simplest filter: it diffuses every Nth frame and writes what it has. Rejected because it trades
a **silent** failure for a saved dependency. The plan's own contract states the reason — a frame
count mismatch desynchronizes the audio mux and *the failure is invisible in the file* — and the
canonical `ffmpeg` command would have to grow a rate that must agree with a flag on another process.
Every other option here fails loudly; this one fails quietly, which is the wrong direction for a
pipeline whose output is judged by eye hours after it ran.

### Alternative B — separate programs for quality and preview

Two entry points, `sd-filter` and `sd-preview`. Rejected because the two share everything that is
hard — stream framing, the control/base split, model loading, the feedback carry — and differ only
in the values of flags that already exist. Two programs means two places for one fix. The
distinction that *is* real (offline versus live) is not a flag difference at all; it is a different
source and a different plan, which is what this ADR decides below rather than papering over with a
second binary.

### Alternative C — diffuse at 512 and upscale to 768/1080p

Measured to be the cheaper route: 512 costs 1/2.7 of 768 per frame at identical coherence, and a
Real-ESRGAN pass adds apparent detail for tens of milliseconds without destabilizing a frame that is
already coherent. **Rejected by the user in the design interview, deliberately**, on the ground that
generated detail is worth its price against inferred detail. Recorded here with its measurement
intact because it is the obvious lever to reach for if the render cost becomes the binding
constraint — reversing this clause costs one flag, not a redesign.

### Alternative D — grow Plan 0106 to cover realtime

Rejected on sequencing rather than on merit. A live filter cannot sit downstream of `shot --render`,
which walks a WAV offline, so realtime is not an extension of the pipe — it needs the live loop, and
that **reopens two alternatives Plan 0106 rejected explicitly**: in-process inference (rejected for
immature wgpu-native diffusion and a very large dependency against *lightweight is a feature*) and
publishing frames to another process (rejected for giving away ownership of the loop). Folding it in
would hold the four things the gate already approved hostage to a problem measured at ~35x. It gets
its own ADR and its own plan, starting from the evidence recorded here.

## Notes

- Every number above comes from Plan 0106's **Implementation log**, which records the runs, the
  retracted readings, and two environment traps that produce a plausible-looking environment failing
  at the first frame (`pip` replacing a CUDA torch build with a CPU one; `torchvision` version-locked
  to `torch`).
- **Phase 2's third question — "is this something they would publish?" — was never asked in those
  terms and is recorded as open.** The request for realtime and higher resolution is a stronger
  signal than a yes, but it is not the same answer, and this ADR does not treat it as one.
- The spike's artifacts (contact sheets, ten clips, per-run `cell.json`) live untracked in the lane
  at `WORK/lmv-plan-0106/spike/` and are the only record of what the gate actually judged.
