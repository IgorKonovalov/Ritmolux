# Roadmap: visual richness — where the engine caps beauty, and the way out

**Written 2026-07-30 (architect), from a full-repo review requested by the user.** The mandate,
in the user's words: the shipped output is "dull and not really interesting except a couple";
the target is imagery that is "moving, alive, smooth, sometimes crazy" — marbled fluid strata,
neon kaleidoscopic tunnels, fractal spirals, interference meshes, layered translucent collages,
molten cellular fields, dense organic line-growth, lit 3D relief, particle shatter — **"even at
the cost of performance if needed."**

This document is the strategic layer: the diagnosis and the ordered themes. Each numbered item
below still goes through the normal loop (interview → ADR → plan) before `dev` touches it —
nothing here is a plan yet. The tactical roster stays in [`plans/README.md`](plans/README.md).

---

## Verdict

The foundation is sound and none of it needs reopening: the Rust core / wgpu / C ABI split
(ADR-0001), the source-agnostic core, deterministic DSP, the `PostStage` seam (ADR-0031), the
palette LUT (ADR-0021), the preset grammar's purity, and the capture/QA harness are exactly the
chassis this roadmap builds on.

"Dull" traces to **five specific architectural turns plus one content-side habit** — all
recoverable, none requiring a rewrite. The pattern across all five: a v1 constraint chosen for
the iGPU floor or for schedule was never revisited once it stopped being necessary, and in two
cases the seam for the richer capability was built and then never used.

### Wrong turn 1 — the iGPU floor became a universal ceiling (the root cause) — **fixed**

> **Corrected by R0** (Plan 0044, 2026-07-30). The diagnosis below is kept as written, as the
> record of why. Its `file:line` citations are now stale on purpose: those five constants moved
> into `TierConfig` in `core/src/render/tier.rs` and no longer exist at the sites named. `GRID`
> and `STAGE_COUNT` are the two that did **not** move — see R0's note.

NFR §1 always specified quality **tiers** — a reduced tier holding 60 fps on the ~2015 iGPU
baseline, a rich tier for capable hardware. The tier system and frame-time governor were
declared "their own follow-up plan" and then deferred by Plans 0001, 0003, 0009, 0011 and 0012.
It was never built. Consequence: there is **one fixed quality, tuned to the weakest supported
machine, on every machine**. Every constant below cites the iGPU floor as its reason:
`PARTICLES = 10_000` (swarm.rs:28), `PARTICLE_COUNT = 50_000` (particles/mod.rs:66),
`MAX_SEGMENTS = 20_000` (lines/mod.rs:51), `GRID = 256` (reaction_diffusion.rs:48),
`POST_MAX 1920x1080` (post.rs:88), `STAGE_COUNT = 2` (post.rs:72), and the standing pressure
against adding pipelines at all (background.rs:12-19 et al.). The user's "even at the cost of
performance" is a direct instruction to finally split the tiers.

### Wrong turn 2 — feedback shipped as decay, never as motion

MilkDrop-class visuals are built on one gene: **the previous frame resampled through a
transform** — a small zoom, rotation, or warp — every frame, so light echoes into tunnels,
spirals, and radial streaks. Our trails stage is `accum = max(cur, prev * fade)` sampled at the
**identical uv** (trails.rs:64-68): the past can only sit still and dim. No preset, however
clever, can make it move. The attractor's trail is the same (`prev * k`, same uv). The
`PingPongField` seam says in its own doc comment that "future warp/feedback variants reuse it
(ADR-0002 named it a deferred follow-up)" (feedback.rs:6-8) — the skeleton was built; the soul
was never installed. Reference looks 2, 3, and 5 (tunnel, spiral, layered echo) live entirely
in this gap.

### Wrong turn 3 — the 8-bit additive composite was never decided at all

Every intermediate in the composite runs at the surface format — 8-bit — and scenes accumulate
additively into it with no tone mapping, no bloom, and no gamma management (palette.rs:54
defers it explicitly). **No ADR chose this; it is an undecided default.** Its measured costs
run through the whole record: the "additive ceiling" (glow above 1 saturates the core and only
widens the skirt — backlog 0019's measurement), Cathedral first rendering as a solid white
disc, mirrored halos summing to blowout on the quietest part of a spectrum readout (Plan 0039
Phase 5 note), the ink pass flattening any mid-luminance field to slate grey (backlog 0027),
and the twice-user-requested bloom stage (backlog 0005: "still feels too naked") having
nothing correct to bloom *from*. Every neon reference look requires HDR accumulation → bloom →
tonemap; that is the standard pipeline for this genre and we have none of the three stages.

### Wrong turn 4 — one scene per preset, one fixed two-stage chain

A preset names exactly one `system` (schema.rs:280-313); the post chain is a compile-time
array of exactly `[Trails, Kaleidoscope]` (post.rs:72,345); blend modes are hardcoded per
pipeline with nothing selectable. A render graph was rejected twice (ADR-0018 Alt A, ADR-0031
Alt B) — correctly, at the time, on YAGNI grounds. But the consequence today is that a layered
composition (a particle figure **over** a warped field, a fold **before** trails, a second
trails pass) is not expressible by any preset, and the reference collage look is out of reach.
The evidence that this is affordable already exists in-tree: the cross-preset dissolve runs
**two full composites with independent PostChains simultaneously** (Plan 0030 proved it with a
test) — layering is a generalization of shipped machinery, not new invention.

### Wrong turn 5 — determinism was over-read as "no randomness, no state, no phrase time"

NFR §6 requires *seeded* randomness, not *no* randomness. Yet the grammar has no `noise()` or
`hash()` — a seeded hash is a pure function and perfectly deterministic — so authors fake
aliveness with sums of four incommensurate sines (attractor_dejong: periods 170/217/275/331 s).
There is no bar index, no phrase counter, no `time_since(beat)`, and `novelty` is experimental
and used by zero presets — so no preset can build an 8-bar arc, land a drop, or do anything
"every 4th beat". And the stateless-expression rule (right in itself) left the one-pole
smoother as the *only* trajectory shape, which produces exactly mush or twitch (backlog 0006,
0021). The determinism invariant never required this starvation; the vocabulary was simply
never grown.

### Wrong turn 6 — content: the library was tuned blind and bound to brightness

Not architecture, but roughly half of the felt dullness, and the record is unambiguous:

- **The library was gained against stimuli 6-100x hotter than real music** (backlog 0020).
  Six presets shipped with their *headline mechanism* dead for months — `fragment_kaleido`
  never changed fold order, `reaction_reef` never folded, `lsystem_arrowhead` never
  subdivided, every attractor never reseeded. On real material much of the set barely moves.
  Plans 0041/0042 fixed the instrument; the re-gain pass is underway.
- **Audio is bound to luminance, not geometry** (backlog 0030). The user's words: "too safe…
  I should see wonders and other worlds." The four presets authored geometry-first (Supernova,
  Leviathan, Cathedral, Reliquary) measure 2-4x better on the animation metric than the set
  they joined. The principle is proven; the library predates it.
- **~55 % of the library is one template per family with different numbers** (the
  preset-surface survey's clustering count), and the scene vocabulary underneath is thin:
  eight scenes, exactly one curve family (`maurer_rose` is the *only* legal `[curve] family`),
  and a 5-iteration sine fold as the entire fragment-field vocabulary.

---

## What the reference looks demand, against what exists

| Reference look | Needs | Have today | Gap lives in |
|---|---|---|---|
| Marbled fluid strata, terraced contours | multi-octave domain warp, terraced palettes | 5-iteration sine fold, LUT palettes | R4 (fragment vocabulary) |
| Neon kaleidoscopic tunnel, light streaks | transformed feedback, bloom, layering | same-uv decay, no bloom, one scene | R1 + R2 + R3 |
| Fractal glowing spiral | escape-time/IFS or feedback spiral, HDR glow | neither | R1 + R2 or R4 |
| Waveform interference mesh | time-domain waveform access, dense polylines | no waveform tap; 64-band spectrum only | R4 + R5 |
| Layered translucent collage | 2+ layers, blend modes | one scene, additive only | R3 |
| Molten cellular field, dark cell walls | Voronoi/cellular technique, edge emphasis | no cellular scene, no edge pass | R4 |
| Dense organic line-growth | growth/flow-line technique, per-segment colour, big segment budgets | 20k segment cap, whole-figure hue on 3 of 4 line scenes | R0 + R4 + R5 |
| Lit 3D relief / heightfield | a lit 3D idiom | nothing 3D-lit (attractor's 3D is unlit points) | R4 (last) |
| Particle shatter on light ground | sprite shapes, dark-on-light composition | round sprites; ink pass can invert line art | R4 (cheap variant) |
| Recursive subdivision line art on paper | subdivision geometry + paper mode | lsystem + ink mode — **nearly reachable today** | content |

---

## The roadmap

Ordered by leverage. R0 and R1 are the hinge — most of what follows either needs them or is
multiplied by them. Every item is its own ADR + plan; costs below are order-of-magnitude
honesty, not commitments.

### R0 — the license: quality tiers (finally building NFR §1's second half) — **DONE 2026-07-30**

> **Delivered by [Plan 0044](plans/done/0044-quality-tiers.md) /
> [ADR-0045](adrs/0045-quality-tiers-floor-and-rich.md)** (accepted). `TierConfig` resolves once at
> renderer construction; unpinned starts `Rich` and a one-way frame-time governor demotes to
> `Floor` on a sustained miss, announced not silent; `--tier` / `LMV_TIER` / `[quality] tier` pin
> it; captures and goldens pin `Floor` **by construction**, so every baseline stayed
> byte-identical through the change. `Floor` is the pre-tier engine field for field.
>
> **Two deltas from the sketch below.** The **RD grid stays 256² in both tiers** — it is a
> *content*-changing constant (pattern scale moves with resolution, ADR-0034), so it was
> deliberately left out of `TierConfig`; changing it is its own future decision. **`STAGE_COUNT`
> is likewise untouched** — it lands in R1, where the bloom stage is what needs it. And the
> **`Rich` values are still provisional multipliers, not measurements**: Plan 0044's Phase 4
> calibration did not run, and is carried in
> [`on-device-validation.md`](on-device-validation.md#runnable-now--the-rich-tier-calibration-plan-0044-phase-4).
> So the *license* is granted; the *budget* is still a guess.

An ADR that ends the single-tier era: a `rich` tier as the default on capable hardware, the
existing constants becoming the `floor` tier, and a frame-time governor that demotes gracefully
(the dual-live governor at mod.rs:88-96 is the in-tree precedent). Every capped constant —
particle counts, segment budget, RD grid, post resolution, stage count — becomes a per-tier
value instead of a universal one. This is roadmap item 3's remaining half from 2026-07-21, and
it is what makes "at the cost of performance" an engineering parameter instead of a vibe.
*Cheap to decide, moderate to build; unblocks everything below politically and budget-wise.*

### R1 — the luminous pipeline: linear-light HDR composite, bloom, tonemap

Scene and post intermediates move to `Rgba16Float` (the format `PingPongField` already uses),
accumulation happens in linear light with real headroom, a bloom stage (bright-pass +
separable blur + additive recombine) joins the chain as a third `PostStage` — ADR-0031 priced
that at "an array element and a `STAGE_COUNT` bump" — and a tonemap/exposure pass sits at
present. This dissolves the additive ceiling, makes `glow` mean something, gives every neon
reference look its engine support, and **lifts all 36 existing presets without touching one
of them**. Backlog 0005 (bloom, user-requested twice) closes inside it. Prerequisite: decide
the kaleidoscope fold defect (backlog 0010/0011) first, so bloom builds against settled
resampling — the backlog already says so. *The single highest paint-per-effort item on this
list. Cost: ~2x composite bandwidth plus 2 passes per blur level — a rich-tier cost, which is
why R0 goes first.*

### R2 — transformed feedback: the MilkDrop gene

The trails accumulation gains a bindable per-frame transform: `fb_zoom`, `fb_rotate`,
`fb_dx`/`fb_dy`, and a small procedural warp — the previous frame is sampled through the
inverse transform before decay, exactly the "warp/feedback variant" feedback.rs:6-8 was built
to host (ADR-0012 named it a cheap future variant on the same seam). Deposit blending becomes
selectable (max-decay stays the default; additive-decay joins it). Tunnels, spirals, radial
streaks, echo-into-depth — the whole family of looks the current engine structurally cannot
produce — arrive in one stage. Bundle the kaleidoscope fold fix (0010) and fold centre (0011)
into the same "resampling stages done right" effort. *Cost: one extra sampled fullscreen pass;
trivial next to R1.*

### R3 — composition: a second scene layer with blend modes

A preset may declare an optional second system plus a blend mode (`add`, `screen`,
`multiply`, `overlay` — in linear light, which is why R3 follows R1) and a coarse routing
choice (which layer the trails/kaleidoscope apply to). This generalizes machinery that
already exists and is already tested: the dissolve engine runs two full composites with
independent PostChains today. It is ADR-0018's rejected render-graph alternative returning at
minimum viable scope with new evidence — so it gets a superseding ADR, not a quiet widening.
The layered-collage reference look is the acceptance test. *Cost: up to 2x frame cost when a
preset uses two layers — rich-tier, governed.*

### R4 — the scene-vocabulary wave

The generative-techniques catalogue (refreshed 2026-07-25) says the unit of work is now "a
technique on an existing idiom — mostly content plus a shader or a sampler". Cheapest first:

1. **Curve families** beyond the lone Maurer rose: Lissajous, harmonograph, epicycloid,
   superformula — all listed Trivial; ADR-0029 rejected the superformula only as "more than
   the routed need", a premise this roadmap replaces.
2. **Cellular/Voronoi fragment technique** with edge-line emphasis (the molten-cells look).
3. **Fractal flame** on the attractor idiom — density histogram + log tonemap, which is
   *exactly* the HDR machinery R1 installs; the catalogue already lists it.
4. **Waveform/oscilloscope scene** — needs a small DSP addition (a time-domain tap on
   `AnalysisFrame`; the PCM already flows through the ring) and reuses `LineRenderer`.
   Unlocks the interference-mesh look and is the most-requested visualizer form we lack.
5. **Lit 3D heightfield/relief** (spectrum- or sim-displaced surface with lighting) — the one
   genuinely new render idiom on the list, deliberately last.

*Each is its own small plan; none blocks the others; 1-4 are independent of R1-R3 and can
interleave.*

### R5 — the grammar of aliveness

Small, separable, each closing a named wall:

- ~~**Seeded `noise(x)` / `hash(x)`** in the expression grammar — pure, deterministic,
  NFR §6-compliant; retires the four-incommensurate-sines workaround.~~ **Landed** —
  Plan [0047](plans/done/0047-expression-randomness.md) / ADR-0051, 2026-07-30, with
  `seed = "random"` on top (per-run variety in the live app, pinned on every capture path).
- **Normalized band variables** (auto-gain-controlled `bass_n`/`mid_n`/`treb_n` or a
  `norm(x)` form) — structurally kills the 6-100x mis-gain class of defect instead of
  re-tuning it away preset by preset (the largest observed source of dead mechanisms).
- **Phrase time**: `beat_index`, a bar/phrase counter, `time_since_beat` — what "make the
  drop land" and "every 4th beat" need; decide `novelty`'s graduation or replacement here.
- **Slew-release smoothing form** (backlog 0021's shape) — the even fall, no new state.
- **Per-segment colour on the line family** (backlog 0026) + `[palette]` adoption on the
  three scenes that still ignore it — ends whole-figure-hue.
- **Bindable structural tables where cheap**: `elements`, `angle_deg`, `contact_angle_deg`,
  a real geometry lerp for `star_pattern` variants (backlog 0007, user-requested).
- The half-linear band axis (backlog 0015, already flagged as the next ADR) rides in this
  theme.

### R6 — the content renaissance

After R1-R5 land in any substantial part: a full `preset-author` pass armed with the new
vocabulary and the two proven craft principles (geometry-first bindings per backlog 0030;
realistic-level gains per 0020/0041). Retire the template clones — the goal is that each
shipped preset is a distinct *world*, not a palette swap. This is where "sometimes crazy"
gets authored: feedback spirals that bloom on the drop, layered collages that restructure per
phrase, cellular fields that crack open on the bass.

---

## Sequencing and the first three moves

1. ~~**Run the approved roster first**: Plans 0042 → 0043~~ — **both closed 2026-07-30.**
2. ~~**R0 + R1 next** — one interview covering both~~ — the interview ran, both ADRs and plans
   landed, and **R0 is done** (Plan 0044). **R1 ([Plan 0045](plans/done/0045-linear-light-and-bloom.md))
   is now the one to run** — the largest single visible change available, and its bloom levels and
   `Floor` bandwidth relief hang off the `TierConfig` R0 just built.
3. **R2 immediately after** ([Plan 0046](plans/0046-transformed-feedback.md) — it renders *into*
   the R1 pipeline), with 0010/0011 folded in.

R4 items 1-2 and the R5 grammar items are small enough to interleave whenever a gap opens.

**What this roadmap deliberately does not do:** reopen ADR-0001 (the Rust/wgpu/C-ABI chassis
is an asset); take ADR-0002's author-WGSL escape hatch (revisit only if R1-R5 prove
insufficient — it trades the whole curation/QA model for authoring power we may not need);
chase `.milk`/projectM compatibility (rejected in ADR-0002 for reasons that still hold); or
relax determinism, the audio-callback rules, or the source-agnostic core (they cost nothing
visually).

## Risks

- **R1 touches every golden baseline** — the linear-light switch changes every rendered
  pixel. Budget a deliberate re-bless with eyes on every scene, and remember `LMV_BLESS`
  rewrites all baselines, not just the targeted one.
- **The WARP software adapter's pipeline-count sensitivity** (the documented mis-render
  pressure against adding pipelines) is a live constraint on R1/R2/R3's test strategy —
  the suite may need per-stage gating rather than one mega-composite test.
- **Tier bifurcation doubles the visual-QA surface** (floor and rich renders differ).
  ~~Decide in R0's ADR which tier the golden suite pins, and what the other tier gets.~~
  **Decided and shipped:** the suite pins `Floor` by construction, and `Rich` gets `shot --tier
  rich` spot checks plus the on-device checklist (ADR-0045, `docs/capturing.md`). The risk is not
  retired — it is *named and accepted*: rich-tier regressions have no baseline to catch them.
- **Scope gravity**: every item above is interview-sized on its own. The failure mode is
  bundling them; the loop (one ADR, one plan, one review) is the antidote.
