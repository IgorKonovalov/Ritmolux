# 0237 — The L-system turtle turns in space, and can grow without end

> **Status:** in-progress (2026-10-05). Runs after Plan 0236 closes.
> **Created:** 2026-10-01
> **Owner skill(s):** dev, human
> **Related ADRs:** [ADR-0258](../adrs/0258-a-system-takes-depth-through-one-shared-camera-block-and-its-3d-mode-forgoes-what-seg3d-does-not-draw.md) (proposed), [ADR-0257](../adrs/0257-a-shared-camera-projects-3d-primitives-and-depth-of-field-is-a-per-endpoint-circle-of-confusion.md), [ADR-0059](../adrs/0059-line-scenes-colour-along-their-generator-axis.md), [ADR-0019](../adrs/0019-eased-parameters.md)

## TL;DR

Two structural keys on `[generator]`, each defaulting to today's behaviour, so the six shipped
L-system presets and the golden keep their bytes.

- **`turtle = "space"`** makes the `lsystem` walk a **3D turtle**. Beside today's `+` and `-`, `&`
  and `^` pitch, `\` and `/` roll, and `|` turns around. A branching grammar grows a tree in depth,
  seen through the shared camera, with branches far from the focal plane going soft.
- **`growth = "endless"`** makes the figure a **vine that never ends**, in `flat` or `space` mode.
  The turtle walks an effectively infinite derivation at a bindable rate, so it can surge on the beat.
  A fixed ring keeps the most recent segments and fades the oldest. The camera follows the growing
  tip on a damped spring.

## Context & problem

The L-system turtle (`turtle.rs`) holds `x`, `y` and one `heading` angle. It knows `F`, `G`, `f`,
`+`, `-`, `[` and `]`, and every other character is an inert grammar variable. Geometry is expanded,
walked and fitted once per depth at `configure`, up to `MAX_LSYSTEM_DEPTH = 7`, and cached
(`lsystem.rs`). A frame only picks a depth, colours it by generation, transforms it in 2D and keeps
the `draw_progress` prefix.

None of the shipped grammars (`bower`, `coral`, `icecrystal`, `rime`, `sumimono`, `vellum`) or the
golden fixture uses `&`, `^`, `\`, `/` or `|`. So the classic 3D turtle vocabulary (Prusinkiewicz and
Lindenmayer, *The Algorithmic Beauty of Plants*, ch. 1.5) collides with nothing. The fit
(`normalize_fit`) is a 2D bounding box. A figure that turns under a camera needs a fit that does not
depend on the view.

The owner asked for a figure that **never ends**, with the camera following its growth. The cached
model cannot give that. A branching grammar multiplies its segment count every generation, so
"deeper" runs into the segment cap within a few depths. Two endless designs were weighed:

- **A walking vine**, which streams the derivation and keeps a window of it. This plan builds it.
- **A dive along a growing lineage**, where every tip grows at once and the camera zooms forever.
  It is a followup, built only if the vine proves the idea.

## Decision

**Space.** A structural `turtle` key opts in (ADR-0258). In `space` mode the turtle carries a
position and an orthonormal frame (heading, left, up):

- `+` and `-` yaw about up.
- `&` and `^` pitch about left.
- `\` and `/` roll about heading.
- `|` yaws by 180 degrees.

All of them turn by `angle_deg`. The cache holds `Segment3dInstance`s per depth, fitted by
**bounding sphere**. The scene draws them through its own `new_3d` renderer, sized by
`seg3d_segments` (Plan 0236). In `flat` mode the new symbols stay inert. We rejected switching to 3D
whenever a grammar contains a 3D symbol: a structural mode that turns on silently is one a reader
cannot see in the file.

**Endless.** A structural `growth` key opts in. In `endless` mode the scene caches nothing.

- **The stream.** A **lazy depth-first expansion** of the grammar at a fixed internal derivation
  depth produces one turtle symbol at a time. Its stack is preallocated from a bound computed at load.
- **The rate.** The stream advances by an integrated phase of the bindable `grow` (draw steps per
  second), the way the attractor integrates `spin`. The figure grows at the same speed at any display
  rate (ADR-0019), and a binding to onset makes it surge.
- **The ring.** Each draw step lands in a fixed **ring** of `trail` segments. The oldest fade out by
  age over the `tail` fraction.
- **The follow point.** It is the centroid of the newest `follow_window` segments, smoothed by a
  critically damped spring with time constant `follow`. The centroid of a window rather than the tip
  itself, so a `]` that jumps the tip back is averaged rather than chased.
- **Upload.** Every segment is uploaded **relative to the follow point**. In `space` the camera's
  orbit centre is the follow point, with `Camera3d` unchanged. In `flat` the 2D transform pans by it,
  composed with the preset's own `pan`.
- **Re-basing.** When the tip passes a re-base radius, an offset is subtracted from the turtle, its
  stack, the ring and the follow state together. Positions stay small however long the vine runs, and
  every step stays deterministic.

We rejected the dive for now (above), and **deeper cached depths**, which run into the cap within a
few generations. We also rejected a **camera locked to the tip**, which whips at every `]`.

## Architecture diagram

```mermaid
flowchart LR
    subgraph fixed["growth = fixed (today, plus space)"]
        G[grammar.rs expand] --> T{turtle}
        T -- flat --> W2[walk 2D + box fit<br/>SegmentInstance cache]
        T -- space --> W3[walk 3D frame + sphere fit<br/>Segment3dInstance cache]
    end
    subgraph endless["growth = endless, per frame"]
        PH["phase += grow * dt"] --> ST["lazy DFS stream<br/>preallocated stack"]
        ST --> TU[turtle step, flat or space]
        TU --> RING["ring of trail segments<br/>age fade over tail"]
        RING --> FP["follow point: centroid of newest<br/>follow_window, damped spring"]
        RING --> REL[upload relative to follow point<br/>re-base past radius]
        FP --> REL
    end
    W2 --> D2[shared LineRenderer]
    W3 --> D3["own new_3d renderer<br/>CameraParams + coc()"]
    REL --> D2 & D3
```

## Implementation phases

### Phase 1 — Walking skeleton: a tree in depth
- **Owner skill:** dev
- **What:** `[generator] turtle = "flat" | "space"`, default `flat`, rejected with both names if
  unknown. A 3D walk with the frame and the five new symbols, emitting `Segment3dInstance`s and the
  per-segment generation depth, index-aligned as `walk_with_depths` does now. A bounding-sphere fit
  per cached depth. The scene owns a `new_3d` renderer sized by `seg3d_segments`, splices
  `CameraParams`, and in `space` mode draws the cached depth through it, with colour by generation
  and the `draw_progress` prefix as in `flat`.
- **Files touched:** `core/src/render/scenes/lines/turtle.rs`, `core/src/render/scenes/lines/lsystem.rs`,
  `core/src/preset/schema/raw/generator.rs`, `core/src/preset/schema/load.rs`,
  `core/tests/fixtures/`, `core/tests/`.
- **Done when:** a `space` fixture with a branching grammar that uses `&` and `/` renders a visible
  tree through `shot`, and two frames at different `yaw` differ. The `lsystem` golden and the six
  shipped presets render byte-identically. A grammar of `F&F&F&F` at 90 degrees in `space` mode walks
  a closed square in a vertical plane, and the same string in `flat` mode walks a straight line of
  four steps.

### Phase 2 — What is inert in space, and the frame's drift
- **Owner skill:** dev
- **What:** declare `rotation`, `mirror_order`, `mirror_reflect`, `stroke_blend` and `scale` inert in
  `space` mode (ADR-0258), and the camera block inert in `flat`. `thickness` is in pixels at the focal
  plane in `space`. After each turn, re-orthonormalize the frame, so a long walk does not let the frame
  drift off orthonormal. This matters most in `endless` mode, which turns without limit.
- **Files touched:** `core/src/render/scenes/lines/lsystem.rs`, `core/src/render/scenes/lines/turtle.rs`,
  `core/tests/`.
- **Done when:** after 100,000 random turns, the frame's three vectors are unit length and mutually
  orthogonal to within `1e-4`. The generated reference names `space` or `flat` against each inert
  param.

### Phase 3 — Caps and the space golden
- **Owner skill:** dev
- **What:** in `space` mode, segments over `seg3d_segments` are counted and reported through
  `CapOverflow::Depth`, as in `flat`. One golden of a `space` tree with a fixed camera and a non-zero
  `aperture`.
- **Files touched:** `core/src/render/scenes/lines/lsystem.rs`, `core/tests/golden.rs`,
  `core/tests/golden/`, `core/tests/fixtures/`.
- **Done when:** the new golden holds on the software adapter. A depth whose walk exceeds the cap
  reports it rather than drawing a truncated tree without notice.

### Phase 4 — Endless: the lazy stream and the ring
- **Owner skill:** dev
- **What:**
  - `[generator] growth = "fixed" | "endless"`, default `fixed`, rejected with both names if unknown.
    In `endless` mode, a lazy depth-first expansion at a fixed internal derivation depth
    (`STREAM_DEPTH`, a named constant whose comment gives the stream length it buys). Its stack is
    preallocated from a bound computed at load: `STREAM_DEPTH + 1` expansion frames, and a bracket
    stack of `STREAM_DEPTH` times the deepest bracket nesting in any successor.
  - The loader computes the stream's length by saturating dynamic programming over the rules. It
    rejects `endless` on a grammar whose stream is shorter than `trail`, naming both numbers, since
    such a grammar would restart before the ring fills.
  - When the stream ends anyway, it restarts from the axiom, and the ring fades on without a pop.
  - The stream advances by `phase += grow * dt`, emitting `floor(phase) - emitted` draw steps, clamped
    to a per-frame ceiling so a huge `grow` cannot stall a frame.
  - Draw steps land in a ring of `trail` segments. `trail` is structural and capped by
    `seg3d_segments` in `space` and by `max_segments` in `flat`, clamped and announced. Each segment's
    alpha falls by age over the oldest `tail` fraction of the ring.
  - Colour stays by generation: the stream's bracket depth, as in `fixed`.
  - `max_depth`, `visible_depth`, `draw_progress`, `mirror_order` and `mirror_reflect` are declared
    inert in `endless`.
  - No `normalize_fit` runs, since the figure has no extent. One draw step is one unit, and `scale`
    sets its size in `flat`.
- **Files touched:** `core/src/render/scenes/lines/grammar.rs`, `core/src/render/scenes/lines/turtle.rs`,
  `core/src/render/scenes/lines/lsystem.rs`, `core/src/preset/schema/raw/generator.rs`,
  `core/src/preset/schema/load.rs`, `core/tests/fixtures/`, `core/tests/`.
- **Done when:**
  - The `lsystem` golden and the six shipped presets are byte-identical.
  - The lazy stream's first 10,000 symbols equal the eager `expand` at depth 7, for every shipped
    grammar and the golden's.
  - After 6,000 frames of `F=F[+F]F[-F]F` at `grow = 20`, the ring holds exactly `trail` segments.
    The capacity of every buffer the scene owns is unchanged since `configure`.
  - The emitted count after 600 frames at `dt = 1/60` and after 1,440 frames at `dt = 1/144` differs
    by at most one.
  - A grammar with no growing rule is rejected under `endless` with its stream length in the message.

### Phase 5 — The camera follows the growth
- **Owner skill:** dev
- **What:**
  - The follow point: the centroid of the newest `follow_window` segments (structural, default 32),
    through a critically damped spring with time constant `follow` (bindable, seconds).
  - Segments are uploaded relative to the follow point, in `flat` through the 2D transform composed
    with the preset's `pan`, and in `space` with the camera orbiting it. `focus` resolves against the
    ring's bounding radius about the follow point.
  - Re-basing: when the tip passes `REBASE_RADIUS` (a named constant, in draw steps), subtract the
    tip's position, rounded to whole steps, from the turtle, the stack, the ring and the spring state
    in one step.
  - In `fixed` mode none of this runs, and the camera orbits the fitted figure as in Phase 1.
- **Files touched:** `core/src/render/scenes/lines/lsystem.rs`, `core/src/render/scenes/lines/turtle.rs`,
  `core/tests/fixtures/`, `core/tests/`.
- **Done when:**
  - Across a `]` that moves the tip by `d`, the follow point never moves more per frame than the
    spring allows at that `follow`, so there is no jump.
  - For a non-branching vine grammar over 6,000 frames at the default `follow`, the tip's projected
    position stays inside the frame, in `flat` at 1280x800 and in `space` at 1920x1080.
  - After a fast-forward of 1,000,000 draw steps, the newest segment's length is `1 ± 1e-5` units,
    so re-basing has kept the precision of step 0.
  - A re-base changes no rendered pixel: the frames before and after it are identical.

### Phase 6 — Endless cost and the endless golden
- **Owner skill:** dev
- **What:**
  - Measure an `endless` `space` fixture at `trail` equal to `seg3d_segments` on `Floor`, at
    `aperture` 0 and past the cap, against the budget Plan 0238 and Plan 0239 use: the blurred frame
    at most twice the sharp one, and inside NFR section 1.
  - Record the CPU cost of `update` at the per-frame emit ceiling too, since the stream and the ring
    upload are CPU work.
  - The whole ring is re-uploaded every frame, because its positions are relative to a moving follow
    point. If that upload is the cost that misses the budget, the log says so, and the followup below
    is the answer.
  - One golden per mode (`flat` and `space`), taken after a fixed number of frames of an `endless`
    vine under a fixed analysis frame.
- **Files touched:** `core/src/render/scenes/lines/lsystem.rs`, `core/src/render/tier.rs`,
  `core/tests/golden.rs`, `core/tests/golden/`, `core/tests/fixtures/`, `core/tests/`.
- **Done when:** both goldens hold on the software adapter. The same analysis frames give the same ring
  after 600 frames. The log carries the measured sharp and blurred frame times, the ratio and the
  `update` time, and the ratio is at most 2 with the blurred time inside NFR section 1, or `trail`'s cap
  is lowered until it is.

### Phase 7 — Documentation and the references
- **Owner skill:** dev
- **What:**
  - The turtle vocabulary paragraph in `presets/README.md`'s `[generator]` section gains the five
    symbols and the `turtle` and `growth` keys, and what each mode makes inert.
  - The generated params block and schemas are regenerated.
  - `turtle.rs`'s module doc lists the space vocabulary.
  - `docs/preset-guide.md` gets one picture of a tree in space and one of an endless vine.
  - The log carries the text for the preset-author reference's `## lsystem` section, for the owner
    to apply in Phase 9: both modes, their inert params, the facet at a right-angle turn, and which
    grammars suit `endless` (vines and short-branched coral, not deep trees, whose `]` sends the tip
    back a long way). A headless session cannot edit `.claude/` (ADR-0210).
- **Files touched:** `presets/README.md`, `presets/schema/`, `.taplo.toml`,
  `core/src/render/scenes/lines/turtle.rs`, `docs/preset-guide.md`, `docs/images/`,
  `docs/specs/player-schema.json`.
- **Done when:** the schema and param-reference tests pass on the regenerated files.
  `node scripts/toc.mjs --check`, `node scripts/check-doc-links.mjs` and
  `node scripts/check-reader-prose.mjs` pass.

### Phase 8 — The look, judged
- **Owner skill:** human
- **Blocks merge:** no
- **What:** the owner runs a `space` tree, and an `endless` vine in `flat` and in `space` mode, live
  with the music track. They judge:
  - whether depth reads, and whether the facets at the grammar's turns show;
  - whether the endless vine reads as growth the camera is travelling with, rather than a scroll;
  - whether `grow` bound to onset surges well;
  - whether the vine proves the idea enough to plan the dive.

  Then they hand a brief to `preset-author`.
- **Files touched:** none.
- **Done when:** the owner records a keep, or a list of what is off, and a yes or no on the dive, in
  this plan's log.

### Phase 9 — The preset-author reference
- **Owner skill:** human
- **Blocks merge:** no
- **What:** the owner applies, in an interactive session, the text Phase 7 left in the log to
  `.claude/skills/preset-author/references/systems.md`'s `## lsystem` section.
- **Files touched:** `.claude/skills/preset-author/references/systems.md`.
- **Done when:** the edit is committed on `main` and `node scripts/check-doc-links.mjs` exits 0.

## Data shapes

```rust
// illustrative, not the final interface
struct Turtle3d {
    pos: [f32; 3],
    heading: [f32; 3], // H
    left: [f32; 3],    // L
    up: [f32; 3],      // U
}
// + / -  : rotate (H, L) about U by +/- angle
// & / ^  : rotate (H, U) about L by +/- angle
// \ / /  : rotate (L, U) about H by +/- angle
// |      : rotate (H, L) about U by pi

struct Stream {                 // growth = "endless"
    frames: Vec<(u16, u32)>,    // (rule index, position in successor); len <= STREAM_DEPTH + 1
    phase: f64,                 // integrated grow * dt; f64 so a long run keeps whole steps exact
    emitted: u64,
}
struct Follow {
    point: [f32; 3],            // spring position, relative to the current re-base origin
    velocity: [f32; 3],
}
```

Bindable params added in `endless` mode, all Modal: `grow` (draw steps per second), `tail` (fraction
of the ring that fades) and `follow` (spring time constant, seconds). Structural: `growth`, `trail`
and `follow_window`.

## Risks & open questions

- **Facets at turns.** L-system grammars turn by 20 to 90 degrees at every joint, so the missing join
  extension (ADR-0258, Negative) shows at any width above a hairline. If Phase 8 finds it ugly, the
  answer is an ADR on joins for `seg3d`, not a workaround here.
- **Branchy grammars jump.** In a depth-first walk, a `]` at a coarse level returns the tip to a
  branch point that may be far behind, beyond the ring. The camera then glides across empty space at
  spring speed. That is the vine's nature, not a bug. `follow_window` and `follow` soften it, and the
  reference steers authors to vine-like grammars. The dive followup is the design for deep trees.
- **The stream visits the tree in depth-first order, not growth order.** A tree drawn endlessly reads
  as a pen tracing it, not as a plant sprouting. That is the honest difference between the vine and
  the dive. Phase 8 judges whether the vine is enough.
- **The sphere fit makes small trees small** in `fixed` `space` mode. Default `distance` is the lever,
  and the reference says so.
- **`|` in flat mode stays inert**, even though ABOP gives it a meaning in 2D too. Changing it would
  break the "flat means today" rule for any user grammar that uses `|` as a variable.
- **A whole-ring upload per frame** is about 0.9 MB at 20,000 segments, which is fine on the iGPU
  floor. If Phase 6 shows otherwise, write only the new segments into a GPU ring and move the
  follow offset into the camera uniform.

## What this plan does NOT do

- The dive: every tip growing at once, with the camera diving along a lineage forever. That is a
  followup, gated on Phase 8's yes.
- Stochastic, parametric or context-sensitive grammars.
- Tropism, leaf polygons or any non-line primitive.
- Ship presets. That belongs to `preset-author` after Phase 8.

## Implementation log

**Lane:** branch `plan-0237-the-l-system-turtle-turns-in-space`, worktree `/home/igor/Work/rlx-plan-0237`

| phase | owner | state | commit |
|---|---|---|---|
| 1 — Walking skeleton: a tree in depth | dev | done | aaf32332 |
| 2 — What is inert in space, and the frame's drift | dev | done | 6d5c4129 |
| 3 — Caps and the space golden | dev | done | 927564c3 |
| 4 — Endless: the lazy stream and the ring | dev | done | 263296bd |
| 5 — The camera follows the growth | dev | done | 6222ae74 |
| 6 — Endless cost and the endless golden | dev | done | d50a77fb |
| 7 — Documentation and the references | dev | done | b8d2cd95 |
| 8 — The look, judged | human | owed | |
| 9 — The preset-author reference | human | owed | |

### Notes

- The shipped set holds four `lsystem` presets, not six: `lsystem_icecrystal`, `lsystem_rime`,
  `lsystem_sumimono` and `lsystem_thicket`. `bower`, `coral` and `vellum` are not in `presets/`.
  Every "six shipped presets" done-when was checked against these four.
- Phase 1 byte-identity: `shot --size 640x400 --frames 60` of `core/tests/fixtures/lsystem.toml`
  and the four presets, before and after the phase on the same machine (llvmpipe), compared with
  `cmp`. All five identical. The `lsystem` golden itself is WARP-only and was not read.
- Phase 1 touched files outside its list. `GeneratorConfig::LSystem` gains `turtle` in
  `core/src/render/scenes/mod.rs`, which also builds `LSystemScene` with the device and the tier's
  `seg3d_segments` and `max_coc_px`. `Roster::Turtle` is in `core/src/preset/schema/export.rs`,
  with its parser check in `core/src/preset/schema/tests.rs`. The test module is registered in
  `core/tests/suite/main.rs`.
- Phase 1 regenerated `presets/README.md`'s params block, `presets/preset.schema.json`,
  `presets/schema/*.json` and `docs/specs/player-schema.json`, which Phase 7 lists. The new key
  and the spliced camera params made their drift tests red otherwise.
- Phase 1's `lsystem` scene now builds a `seg3d` renderer at construction, so every golden
  captured after `lsystem` in the roster runs on a device with one more set of GPU resources.
  Windows CI's golden job is the first WARP reading of that.
- Phase 2 declares the inert params as `lsystem`'s own family table, with the modes `flat` and
  `space` as its families, registered in `family_params` in `core/src/render/scenes/mod.rs`
  (outside the phase list). The 13 mode-dependent params are re-declared with `lsystem` doc lines,
  because the schema keys a family row by declaration. `rotation`'s doc line changed with it.
  Phase 2 regenerated the same generated files as Phase 1.
- Phase 2's inert claim is also asserted on pixels, in `core/tests/suite/lsystem_space.rs`: the
  flat-only params leave the space fixture byte-identical, and the camera block leaves the flat
  fixture byte-identical.
- Phase 3 adds no code to `lsystem.rs`: Phase 1's `configure` already reports a space depth's
  drop through `OverflowContext::Depth` with `seg3d_segments` as the cap. Phase 3's test of it is
  in `core/tests/suite/lsystem_space.rs`, outside the phase list. `top_tier_lifts` still reads
  `max_segments` for `Depth`; `Depth`'s notice prints no tier remedy, so nothing reads it there.
- Phase 3's golden `lsystem_space.png` was written on llvmpipe through a reverted local
  missing-baseline-only change to `golden.rs`, as Plans 0235, 0236 and 0238 did. It reads mean
  0.0000, outlier 0 on llvmpipe; Windows CI's golden job is its first WARP reading. The same run
  reads `waterfall` (0.0017 / 213), `parametric_torus_knot` (0.0010 / 208) and
  `parametric_lissajous_3d` (0.0010 / 224) past WARP's outlier tolerance on llvmpipe; those were
  0.0000 / 0 on llvmpipe when blessed. This lane did not establish whether its own `seg3d`
  renderer contributes.
- Phase 3 ran `-P fast` over `rlx-core` only (1268 passed): the phase changes core tests and a
  baseline and nothing another crate builds.
- Phase 4: one draw step is `ENDLESS_STEP` = 0.025 world units, not one unit. A whole unit is most
  of the view: the flat view spans `[-1, 1]` vertically, and the camera at its default distance
  shows about three units. On the plane `scale` multiplies it.
- Phase 4: `growth` and `trail` are `[generator]` keys, because the loader checks `trail` against
  the stream. `grow` and `tail` are `[params]`. `STREAM_DEPTH` is 32. The stream-equals-`expand`
  test walks the stream at depth 7, the depth the done-when names. The clamp of `trail` to the
  tier's cap is announced through a new `OverflowContext::Trail`, and `Roster::Growth` lists the
  modes. Both are outside the phase list, in `core/src/render/scenes/mod.rs` and
  `core/src/preset/schema/export.rs`, with `Roster::Growth`'s parser check in
  `core/src/preset/schema/tests.rs`.
- Phase 4 inert declarations: the family table's modes are `flat`, `space`, `flat endless` and
  `space endless`. `rotation`, `scale` and `stroke_blend` read on `flat endless`. `max_depth` is a
  `[generator]` key, so its inertness under `endless` is in its key doc, not the table. The
  generated files were regenerated again.
- Phase 4 adds a per-frame budget of 65,536 stream symbols beside the plan's emit ceiling of 1024
  draw steps: a draw step can sit behind any number of symbols that draw nothing. A frame that
  cannot pay what it owes drops the backlog. The pen's branch stack refuses a push past the
  bracket bound, which only an unbalanced grammar reaches.
- Phase 4's colour divisor for an endless figure is the bracket bound, 32 times the deepest
  successor nesting. On `F=F[+F]F[-F]F` the first 1,200 frames drew in generations far below it,
  so `hue_spread` spans little of the palette there.
- Phase 4 byte-identity re-checked with `shot` as in Phase 1: identical.
- Phase 5 holds the pen, the ring and the follow point in `f64`, snapped to a grid of `2^-30`
  draw steps (`turtle::on_grid`), rather than the `f32` of the plan's data shapes. The turtle's
  frame still turns in `f32`. On the grid a re-base by whole steps is exact, so every uploaded
  position is bit-identical across it. `REBASE_RADIUS` is 1024 draw steps. Phase 4's ring and pen
  changed type with it.
- Phase 5's "a re-base changes no rendered pixel" is asserted on what the GPU is handed, not on a
  capture. The flat draw buffer, the 3D instances and the focus radius are compared byte for byte
  before and after a forced re-base, in `an_endless_scene_grows_no_buffer_it_owns`. The positions
  they are laid out from are compared in `a_rebase_leaves_every_drawn_position_bit_for_bit`.
- Phase 5's spring step is the critically damped spring's closed-form solution over the frame,
  with the target held. The no-jump test bounds each frame's move by `dt / follow` times the
  farthest the point has been from its target. On `F=F[+F]F[-F]F` over 6,000 frames, the largest
  tip jump was 80.1 steps, and the follow point moved 0.031 steps on that frame.
- Phase 5's in-frame test drives `Endless` on the CPU and projects the tip, through the flat
  view's mapping at 1280x800 and the default camera at 1920x1080. It renders nothing. The tip
  reached 0.237 (flat, `F+F-F-F+F`) and 0.157 (space, `F+F-F&F^F`) of the half-extent. `follow`
  defaults to 0.3 s, and `follow_window` is a `[generator]` key (default 32), which adds a field
  to `GeneratorConfig::LSystem` in `core/src/render/scenes/mod.rs`.
- Phase 5 dropped Phase 4's sanitizing of `dt`: `hygiene::a_frame_delta_is_checked_for_finiteness_in_exactly_one_place`
  allows only the renderer's (ADR-0191). `grow` and `follow` are still sanitized.
- Phase 6 frame cost: `shot --report family=lsystem --tier floor`, release profile, 1920x1080, on
  AMD Radeon Graphics (RADV RENOIR) iGPU with Mesa 26.2.2, one run. The presets were scratch
  files under `target/0237/cost/`: `growth = "endless"`, `turtle = "space"`,
  `F=F[&+F]F[^-F]/F` at 25 degrees, `trail 8000` (Floor's `seg3d_segments`), `thickness 12`,
  `distance 1.5`, `fov 1.2`, `focus 0`. `grow 60000` fills the ring inside the report's 8-frame
  short leg, so each timed frame emits about 1,000 steps, near the 1,024 ceiling, over a full
  ring. `follow 0.001` keeps that ring in view; at the default 0.3 s the spring lags hundreds of
  units at that rate and the frame is black. Sharp (`aperture 0`): 1.218 ms. Blurred
  (`aperture 40`, past Floor's 12 px cap): 1.523 ms. Ratio 1.25. NFR section 1's budget is
  16.67 ms. `trail`'s cap was not lowered, and `core/src/render/tier.rs` is untouched.
- Phase 6 `update` cost: a scratch release-profile test, not committed, drove `LSystemScene`
  with `grow 1e6` (the 1,024 ceiling every frame), `trail 8000` and the same grammar, for 600
  frames after 60 to warm. It timed `reset_params`, `set_param`, `advance` and `update`, plus
  `lay_endless_space` on the space turtle (the CPU half of `render`). Flat: 104.5 us a frame.
  Space: 326.0 us a frame. Neither reading includes the GPU upload.
- Phase 6's goldens `lsystem_endless_flat.png` and `lsystem_endless_space.png` were written on
  llvmpipe through the reverted missing-baseline-only change, as in Phase 3; both read 0.0000 / 0.
  Their fixtures overrun `trail 48` within the harness's 60 frames (`grow 72`). The 600-frame
  determinism check is `lsystem_endless::the_same_frames_grow_the_same_ring`, which compares two
  captures of each fixture byte for byte.
- Phase 7's regeneration changed nothing: `presets/schema/`, `.taplo.toml`,
  `docs/specs/player-schema.json` and the params block were already current from Phases 1, 2
  and 4. The schema tests and `the_parameter_reference_block_is_current` pass on them.
- Phase 7's two guide pictures render teaching presets, `docs/examples/lsystem/space_tree.toml`
  and `docs/examples/lsystem/endless_vine.toml`, through two new entries in
  `scripts/docs-shots.mjs`, which is every committed image's provenance record. Both paths are
  outside the phase list. The images were rendered on llvmpipe; the `docs/images/lsystem/` stems
  are outside `docs/images/gallery/`, so the gallery hygiene tests do not read them.
- The lane's tip builds `rlx-core` with a `dead_code` warning on `PreviewService::target` in
  `core/src/render/preview.rs`, which arrived with the merge of `main` (fdac6e6e, a8eab67d), not
  from this plan. Not touched.
- Phase 7's text for the preset-author reference's `## lsystem` section in
  `.claude/skills/preset-author/references/systems.md`, for the owner to apply in Phase 9:

  > **Two structural modes on `[generator]`, both defaulting to the shipped behaviour.**
  >
  > - **`turtle = "space"`** walks the grammar in depth. Beside `+`/`-` (yaw), `&`/`^` pitch,
  >   `\`/`/` roll and `|` turns around, all by `angle_deg`; under `flat` those five are inert
  >   variables. The figure is drawn through the camera block (`yaw`, `pitch`, `distance`, `fov`,
  >   `focus`, `aperture`, `fog`, `solid`), and `thickness` is pixels at the focal plane. Inert in
  >   `space`: `rotation`, `scale`, `stroke_blend`, `mirror_order`, `mirror_reflect`. The fit is
  >   a bounding sphere, so a small tree reads small: `distance` is the lever, not `scale`.
  > - **`growth = "endless"`** grows a vine that never ends, in either turtle mode. `grow` is draw
  >   steps a second (bind it to `onset` to surge), `trail` (a `[generator]` key, default 2000,
  >   held to the tier's cap) is how many segments stay, `tail` is the fading oldest fraction, and
  >   the view follows the newest `follow_window` segments on a spring of `follow` seconds. Inert
  >   in `endless`: `max_depth`, `visible_depth`, `draw_progress`, `mirror_order`,
  >   `mirror_reflect`. In `flat endless`, `scale` sets the step's size and `rotation` still turns
  >   the figure. `grow`, `tail` and `follow` are inert under `fixed`. A grammar whose stream is
  >   shorter than `trail` is a load error.
  > - **The facet at a turn.** A segment is a straight stroke with no join, so every turn of
  >   `angle_deg` shows a facet at any width above a hairline, more at 60 to 90 degrees than at
  >   20. Keep `thickness` low on a sharp-angled grammar, or accept the faceted look.
  > - **Which grammars suit `endless`.** The walk is depth-first, so a `]` sends the tip back to
  >   its branch point and the camera glides after it. Vines and short-branched coral
  >   (`F=F[&+F]F[^-F]/F`, `F=F+F-F&F^F`) read as growth. A deep tree (`X=F[+X][-X]FX`,
  >   `F=FF`) jumps a long way back at every coarse `]` and reads as a pen tracing a tree; draw
  >   it `fixed` instead. A wider `follow_window` or a longer `follow` softens a jump.

### Close triggers

- **`presets/` touched:** yes. `presets/README.md` (the generated params block and the
  `[generator]` section), `presets/preset.schema.json` and every `presets/schema/*.schema.json`,
  all regenerated or documentation. No preset `.toml` added, changed or removed.
- **Plan header `Closes:`** none
- **What shipped:** feature. The `turtle` and `growth` `[generator]` keys on `lsystem`, with
  `trail`, `follow_window` and the `grow`, `tail` and `follow` params.
- **Operator docs touched:** `presets/README.md`, `docs/preset-guide.md` (with
  `docs/examples/lsystem/` and `docs/images/lsystem/`), `docs/specs/player-schema.json`.
- **Backlog probes (`node scripts/check-backlog-claims.mjs`):** exit 0. 58 stated reductions
  hold across 29 live entries, 4 unprobeable.
- **Full suite:** owed to the conductor's pre-review gate (ADR-0207).
- **Outstanding `human` phases:** Phase 8 (the look, judged) and Phase 9 (the preset-author
  reference), both `Blocks merge: no`.

## Followups (after this lands)

- `preset-author`: a launch pair of space trees, and an endless vine in each mode.
- **The dive, if Phase 8 says yes.** It is its own plan:
  - Every tip grows at once, the L-system's own parallel semantics, with fractional depth so new
    segments extend smoothly.
  - The camera dives along a seeded lineage of tips.
  - Only the subtrees whose bounds meet the view are expanded. Each symbol's bound at depth `n` is
    precomputed from the rules and contracts geometrically with depth.
  - The world re-roots at the lineage node in view, so precision never runs out.
- An incremental GPU ring with the follow offset in the camera uniform, if Phase 6 shows the
  whole-ring upload is what costs.
