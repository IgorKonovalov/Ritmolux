//! Screen-space kaleidoscope (ADR-0018 composite stage 4, Plan 0018 Phase 7): a
//! post-pass that folds the composited frame into `N` mirrored wedges before
//! present — the general, engine-wide kaleidoscope (distinct from the line-only
//! *geometry* mirror of Phase 4, which replicates segments; this folds pixels).
//!
//! The fold is dihedral: each output pixel's angle is wrapped into one
//! `2*pi/order` wedge and mirrored within it, so the frame is invariant under a
//! `2*pi/order` rotation and carries a mirror line per wedge. `kaleido_angle`
//! rotates the whole fold, and `kaleido_center_x`/`_y` place its axis.
//!
//! # The fold folds a **disc** (ADR-0047)
//!
//! The operation is polar but the source is rectangular, so the two do not have
//! the same shape. Each output pixel keeps its radius and only changes its angle,
//! which means any pixel whose radius exceeds the source's extent *in the folded
//! direction* reconstructs a coordinate outside `[0, 1]`. Handing that to a
//! `ClampToEdge` sampler smears the border texel radially into hard streaks and
//! chevron debris — design-backlog 0010, user-reported three times, and
//! catastrophic in a portrait window, where most of the frame is out of range
//! rather than just the corners.
//!
//! So the **sample** radius is clamped to `r_max`, the largest disc around the
//! fold axis that the source contains (the nearest of the four edges, in
//! aspect-corrected space). Nothing is ever sampled outside the source, at any
//! aspect or fold centre. Past `r_max` the result fades out over
//! [`FALLOFF_BAND`], so the boundary is a designed vignette rather than a hard
//! ring. Content outside that disc is discarded (ADR-0047).
//!
//! # ...and what happens **outside** that disc is a choice (`kaleido_edge`, ADR-0061)
//!
//! That region is not a trim: with `r_max = 0.5` in aspect-corrected
//! space and a corner radius of `0.5 * sqrt(aspect^2 + 1)`, the corner
//! sits at **2.04x `r_max` at 16:9** and the inscribed disc covers only
//! `pi r_max^2` of an `aspect x 1` frame, so **55.8 % of the frame at
//! 16:9 lies outside it** (the same at 9:16, by symmetry). No single
//! treatment serves both of the fold's populations, so [`PARAMS`]'s
//! `kaleido_edge` selects one per preset, from a closed roster, inside
//! **one** pipeline — every arm is a uniform branch on how `r` maps to
//! a sample radius `rs` and an output weight `w` (`m = r / r_max`):
//!
//! | Value | Name | `rs` | `w` | Reads through |
//! |---|---|---|---|---|
//! | 0 | `falloff` | `min(r, r_max)` | `1 - smoothstep(r_max, r_max*(1+band), r)` | `ClampToEdge` |
//! | **1** | **`tile`** (default) | `r` | `1` | **`MirrorRepeat`** |
//! | 2 | `squash` | `r_max * tanh(m)` | `1` | `ClampToEdge` |
//!
//! The roster is closed and its numbering is historical — ADR-0061
//! swept five candidates and kept three in their original relative
//! order, so the values have no gap even though the roster does.
//!
//! **The default is `tile` (1), not `falloff` (0)** — the one place this stage's
//! numbering is deliberately not "0 is the default", because `0 = falloff` is
//! what ADR-0047 shipped and ADR-0061 then chose a different member as the
//! resting behaviour. A preset that binds no `kaleido_edge` **fills its frame**
//! rather than cropping to a disc.
//!
//! `falloff` and `squash` keep `rs` inside `[0, r_max]`, so they
//! inherit ADR-0047's guarantee: the design-backlog 0010 smear came
//! from *reconstructing a coordinate outside the source* and handing it
//! to `ClampToEdge`, and neither does that. **`tile` is the exception,
//! and it is the default** — the single most important thing to know
//! about this stage. Its coordinate is *meant* to leave `[0,1]`, and
//! that is safe **only** because a `MirrorRepeat` sampler defines the
//! read. Wired to the `ClampToEdge` sampler it is design-backlog 0010
//! under a new name, unguarded by the disc assertion (which `tile` is
//! supposed to break) — see `core/tests/kaleidoscope.rs`, where the
//! guard that does catch it is the ray-variance property.
//!
//! `squash` is **not** the identity inside the disc the way a clamp is: `tanh(m) <
//! m` for every `m > 0`, so it compresses the whole interior, 1:1 only in the limit
//! at the fold axis.
//!
//! `kaleido_edge` is the stage's **second stepped param**. Like `kaleido_order` it
//! is clamped and rounded on the CPU ([`fold_edge`]), for the [`fold_order`]
//! reason: `[smoothing]` and preset dissolves both sweep a param *continuously*
//! between two settings, and a selector swept through 1.5 is not a fourth treatment
//! — it is an undefined one. Rounding in Rust keeps the shader's precondition
//! visible in Rust.
//!
//! **Identity passthrough when every term is at its identity** — `kaleido_order <
//! 2` *and* `kaleido_radial <= 1` *and* `kaleido_tile <= 1` — so the
//! [`PostChain`](super::post::PostChain) skips this stage entirely: no offscreen,
//! no pipeline, the NFR §1 iGPU floor pays nothing, and (like the
//! background/trails passes) the DX12 WARP software adapter never sees a
//! coexisting fold pipeline during the no-kaleidoscope captures. When active the
//! pipeline builds lazily and is dropped on the capture scene-rebuild.
//!
//! Runs at an internal resolution that **follows the render target** (ADR-0034),
//! quantized and capped by
//! [`internal_grid_size`](super::post::internal_grid_size), rather than a fixed
//! 1280x720 with the fold's aspect correction baked to match.
//!
//! **The fold's aspect is the render target's, never that grid's**
//! (ADR-0037). The grid is quantized to a 256 px step, so its ratio is
//! only approximately the window's, and folding about the grid's axis
//! skews every wedge wherever the two disagree — which is most window
//! sizes, and not the 16:9 ones.
//!
//! On a line scene, prefer the **geometry** mirror (`mirror_order` /
//! `mirror_reflect`) over this fold when either would do: that one replicates real
//! segments *before* rasterization, so it costs nothing in resolution, while this
//! one folds finished pixels at the stage's internal grid.
//!
//! # This is now **the symmetry stage** (ADR-0077)
//!
//! The fold is one term of a **composed destination-to-source coordinate map**,
//! evaluated before the stage's *single* texture read. Map the plane to
//! `(log r, θ)`: periodicity in `θ` is the dihedral fold above, and periodicity in
//! **`log r`** is scale self-similarity — concentric rings, each a shrunk copy of
//! the one outside it. The second half is what turns a flat rosette into a mandala.
//!
//! The composed order is **tile → fold → radial → spiral**, expressed
//! destination-to-source, and it is **fixed, not author-selectable** —
//! one pipeline and one resample however many terms are live
//! (ADR-0077). Read forwards it means the polar rosette is the motif
//! the tile replicates.
//!
//! | Param | Identity | Default | What it is |
//! |---|---|---|---|
//! | `kaleido_tile` | `1` | `1` | mirrored wallpaper cells across the frame |
//! | `kaleido_order` | `1` | `1` | the dihedral fold above |
//! | `kaleido_radial` | `1` | `1` | the **scale ratio between successive rings** (2 = each ring half the last) |
//! | `kaleido_spiral` | `0` | `0` | an **integer winding number** — the Droste shear |
//! | `kaleido_zoom` | `0` | `0` | travel along `log r`, **in rings** — 1 is exactly one |
//! | `kaleido_inner` | `0` | **`0.06`** | where the repeat stops, as a fraction of `r_max` |
//!
//! The two columns agree on every row but the last, and that one disagreement is
//! deliberate — see [`DEFAULT_INNER`]. It costs nothing while the repeat is off,
//! since the whole radial group is skipped then.
//!
//! Three facts about the composition are worth having before reading the shader:
//!
//! - **The repeat subsumes the edge treatment.** Every destination radius wraps
//!   into the canonical band `(r_max/radial, r_max]`, so with `kaleido_radial > 1`
//!   nothing is ever outside the disc, nothing is clamped, nothing fades, and
//!   `kaleido_edge` is inert — ADR-0077's "one radius policy".
//! - **The zoom's period is exact, and its unit is the ring.** The map is periodic
//!   in `log r` with period `L = ln(radial)`, so an offset of exactly `L` is the
//!   identity map rather than an approximation of it — an audio- or time-driven
//!   `kaleido_zoom` is an endless tunnel with no reset and no crossfade.
//!   `kaleido_zoom` is authored in **rings**, not in `log r`: the shader multiplies
//!   it by `L` itself ([`fold_zoom`]), so `kaleido_zoom = 1` advances exactly one
//!   ring at **every** `kaleido_radial`. The unit is the whole difference between
//!   `"bar_phase * 1.0"` and asking an author for the logarithm of a ratio they
//!   chose by eye — the same map, but only one spelling survives re-tuning
//!   `kaleido_radial`.
//! - **The spiral's winding number is quantized CPU-side, and must be.** Shearing
//!   `log r` by `k·θ` shifts the radius by `2πk` over one revolution, so the image
//!   closes only when `2πk` is a whole multiple of `L` — that is, `k = m·L/(2π)`
//!   for integer `m`. `kaleido_spiral` *is* that `m`, and [`fold_spiral`] rounds it
//!   for [`fold_order`]'s reason. A fractional winding draws a visible seam along
//!   `atan2`'s branch cut, exactly as a fractional order tears along it.
//!
//! **The inner rings alias, and [`fold_inner`] is a workaround, not a fix.** The
//! repeat *minifies* toward the centre: at `radial = 2`, after five repeats a
//! destination annulus at 0.0125 displays the source's canonical annulus at 0.4 — a
//! linear compression of 32, so roughly a thousand source texels land under one
//! destination pixel against a bilinear sampler's four. `kaleido_inner` freezes the
//! repeat below a radius, which makes that region *radially constant* (continuous
//! at the cutoff, since `r_eff = max(r, inner·r_max)` agrees with `r` there) and
//! therefore alias-free.
//!
//! **So its default is [`DEFAULT_INNER`] = 0.06 rather than 0**, which is the one
//! place on this stage where the resting value is not the identity — chosen for
//! costing nothing rather than for fixing anything visible, since no aliasing
//! onset was observed at any cutoff (ADR-0077's Outcome). A preset that wants the
//! repeat all the way to the axis writes `kaleido_inner = "0"` and gets it;
//! nothing clamps it up.
//!
//! **With the fold inactive the stage can still be active** — a preset binding only
//! `kaleido_radial` or only `kaleido_tile` gets those terms and no fold. The
//! uniform then carries `order = 1` and the shader skips the wrap outright rather
//! than folding into one wedge, because an order-1 wrap-and-mirror is a *mirror
//! about the x-axis*, not the identity. `kaleido_angle` still applies in that
//! configuration, where it reads as a plain rotation of the source.

// Hot-path panic-denial pragma (Plan 0002 Phase 2; render/ is scanned by the
// hygiene guard). The fold pass encodes every displayed frame it is active.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use crate::render::gpu;

use super::post::{Fold, PostStage, internal_grid_size};

/// `kaleido_order` default — 1 = identity, so an unbound preset is unaffected.
const DEFAULT_ORDER: f32 = 1.0;
/// `kaleido_angle` default — no rotation.
const DEFAULT_ANGLE: f32 = 0.0;
/// `kaleido_center_x` / `kaleido_center_y` default — the screen centre, which is
/// where the fold axis was hardcoded before it became bindable (ADR-0047).
const DEFAULT_CENTER: f32 = 0.5;

/// How far past the inscribed disc the vignette takes to fade out, as a fraction
/// of `r_max` (ADR-0047).
///
/// The out-of-disc region is not small and its size is an aspect fact, not a
/// tuning one: the fold keeps each output pixel's radius, so what matters is the
/// ratio between the frame's corner radius and its shortest half-extent, which is
/// `sqrt(1 + (long/short)^2)` — about **2.04x `r_max` at 16:9**, and larger the
/// further the window is from square. A band of 0.35 therefore reaches the
/// backdrop well before the corners at every aspect, which is what makes the edge
/// read as a vignette rather than as a disc pasted on a rectangle.
const FALLOFF_BAND: f32 = 0.35;

/// The first value the `kaleido_edge` roster defines (0 = `falloff`).
const MIN_EDGE: f32 = 0.0;
/// The last value the `kaleido_edge` roster defines (2 = `squash`). Values past it
/// clamp here rather than selecting the shader's fall-through arm by accident.
const MAX_EDGE: f32 = 2.0;
/// `kaleido_edge` default — **1 = `tile`**, the treatment Plan 0055's live A/B
/// chose as the resting behaviour.
///
/// Deliberately not [`MIN_EDGE`]. `0 = falloff` keeps the roster's numbering tied
/// to ADR-0047's shipped treatment, which is what this file's history and the
/// preset comments refer to; which member of the roster is the *default* is a
/// separate question and the A/B answered it differently. Reordering the roster to
/// force the default to 0 would trade a readable history for a tidier constant.
const DEFAULT_EDGE: f32 = 1.0;

/// Below this order the fold term is the identity (no angular wrap at all).
const MIN_ACTIVE_ORDER: f32 = 2.0;
/// Ceiling on the fold order — beyond a couple dozen wedges the fold is a blur.
const MAX_ORDER: f32 = 48.0;
/// The order the uniform carries when the fold term is **off** but another term
/// keeps the stage active (ADR-0077).
///
/// The shader skips its wrap below 1.5 rather than folding into a single wedge:
/// `seg = 2*pi` makes `abs(a - seg/2)` a **mirror about the x-axis**, which is not
/// the identity and is not what a preset binding only `kaleido_radial` asked for.
const IDENTITY_ORDER: f32 = 1.0;

// --- The composed coordinate map (ADR-0077) ---------------------------------

/// `kaleido_radial` default — 1 = no repeat, the unmapped path.
const DEFAULT_RADIAL: f32 = 1.0;
/// At or below this ring ratio the log-radius repeat is **off**, and off is the
/// unmapped path rather than a degenerate case of the mapped one.
const MIN_ACTIVE_RADIAL: f32 = 1.0;
/// The smallest ratio an *active* repeat is allowed, so `L = ln(radial)` cannot
/// approach zero.
///
/// At 1.02 a 10:1 radius span already holds `ln(10)/ln(1.02)` ≈ **116** rings —
/// past that the repeat is a grey wash, and the arithmetic starts losing the band
/// to f32 precision (the wrap divides by `L`). An eased `kaleido_radial` crossing 1
/// therefore snaps from off to 1.02, which costs nothing visually because 1.02 is
/// already indistinguishable from a wash.
const MIN_RADIAL: f32 = 1.02;
/// Ceiling on the ring ratio — at 8 a 10:1 span holds barely more than one ring,
/// so past it the repeat has nothing left to repeat.
const MAX_RADIAL: f32 = 8.0;

/// `kaleido_spiral` default — 0 = no shear.
const DEFAULT_SPIRAL: f32 = 0.0;
/// Ceiling on `|kaleido_spiral|`. The shear is `m` whole turns of `log r` per
/// revolution; past a handful the rings are a vortex with no readable motif.
const MAX_SPIRAL: f32 = 8.0;

/// `kaleido_zoom` default — 0 rings travelled, so no offset along `log r`.
const DEFAULT_ZOOM: f32 = 0.0;

/// `kaleido_tile` default — 1 cell across, the identity.
const DEFAULT_TILE: f32 = 1.0;
/// At or below this cell count the wallpaper tile is **off**. Not a degenerate
/// case: `abs(fract(x/2)*2 - 1)` at one cell is `1 - x`, a flip, not the identity.
const MIN_ACTIVE_TILE: f32 = 1.0;
/// Ceiling on the cell count — 16 cells across a 1280-wide grid is 80 px a cell,
/// which is already less than the motif needs.
const MAX_TILE: f32 = 16.0;

/// `kaleido_inner` default — **0.06**, not 0, and the only resting value on this
/// stage that is not the identity (Plan 0064 Phase 4).
///
/// The repeat minifies toward the axis without bound, so the identity here is the
/// one setting guaranteed to alias. Phase 4's sweep — the ratio × winding grid on a
/// full-frame field, an accumulating attractor and a sparse line figure, at two
/// aspects — could not tell 0.06 from 0 on any source, while it caps the worst
/// minification: the frozen interior covers `0.06^2` ≈ 0.4 % of the disc's area and
/// bounds the compression there at `1/0.06` ≈ 17x instead of unbounded.
///
/// It costs an author nothing to opt out — `kaleido_inner = "0"` is honoured
/// exactly ([`fold_inner`] clamps but does not floor) — and it costs the stage
/// nothing when the repeat is off, since the whole radial group is skipped then.
const DEFAULT_INNER: f32 = 0.06;
/// Ceiling on the inner cutoff: `r_max` itself, at which the whole disc is the
/// frozen innermost ring and the repeat shows nothing.
const MAX_INNER: f32 = 1.0;

/// The wedge count the shader is handed: clamped to the active range, then
/// **rounded to an integer**.
///
/// The shader wraps with `a - seg * floor(a / seg)` where `seg = 2*pi/order`, then
/// mirrors within the wedge — a function periodic in `seg`. `atan2`'s branch cut
/// lies on the **-x ray**: crossing it, `a` jumps by exactly `2*pi`, and a
/// `seg`-periodic function absorbs that jump only when `2*pi` is a whole multiple
/// of `seg` — that is, only when `order` is an integer. At any fractional order
/// the frame tears along one horizontal ray from the centre to the left edge.
///
/// Two things make that constant rather than rare. `kaleido_order` sits under
/// `[smoothing]` in nearly every shipped kaleido preset, so each ladder step eases
/// through a second or more of fractional orders and preset dissolves interpolate
/// it too; and the fold's mirror is *even*, so the jump cancels exactly at
/// `kaleido_angle = 0` and only at 0 — which 10 of the 12 shipped presets with an
/// active fold leave behind immediately, driving the angle off `time`.
///
/// Rounding **here** rather than in WGSL keeps the shader's precondition visible
/// in Rust: the uniform never carries a fractional order. The cost is that
/// `kaleido_order` becomes a **stepped** parameter (a 12.5-wedge kaleidoscope is
/// not a thing); `presets/README.md` says so beside the param.
fn fold_order(order: f32) -> f32 {
    order.clamp(MIN_ACTIVE_ORDER, MAX_ORDER).round()
}

/// One axis of the fold centre, in uv: clamped into the frame, with a non-finite
/// binding falling back to the screen centre.
///
/// Off-frame is not a useful fold: the inscribed disc is the distance to the
/// *nearest* source edge, so a centre outside `[0, 1]` has no disc at all and the
/// falloff would take the whole frame to the backdrop. Clamping keeps an
/// over-driven binding (an eased `pan`-like sweep that overshoots) at the frame
/// edge instead of blanking the picture.
fn fold_center(v: f32) -> f32 {
    if v.is_finite() {
        v.clamp(0.0, 1.0)
    } else {
        DEFAULT_CENTER
    }
}

/// The edge treatment the shader is handed: clamped into the roster, then
/// **rounded to an integer**, with a non-finite binding falling back to the
/// default.
///
/// This is [`fold_order`]'s treatment for [`fold_order`]'s reason, on a param
/// whose values are *identities* rather than a quantity. `kaleido_edge` sits in
/// the same `[smoothing]` and preset-dissolve machinery as everything else, and
/// both of those interpolate a binding **continuously** from one setting to
/// another: easing from `falloff` (0) to `squash` (2) passes through 0.5, 1.0,
/// 1.5, so it visits `tile` on the way. Rounding here means the sweep *snaps* at
/// each midpoint — `kaleido_order`'s documented cost, taken again knowingly —
/// instead of the shader receiving a value no arm defines. Doing it in Rust rather
/// than WGSL keeps that precondition visible on the CPU side, where the roster's
/// bounds live.
///
/// Non-finite falls back to the default rather than clamping (which is what
/// `fold_order` does with an infinity): a selector has no "as far as you can go"
/// reading, so a broken binding should land on the resting treatment. Note that
/// since the default is not the low bound, a clamp and the fallback are genuinely
/// different answers here — an under-driven binding lands on `falloff` while a
/// broken one lands on `tile`.
fn fold_edge(v: f32) -> f32 {
    if v.is_finite() {
        v.clamp(MIN_EDGE, MAX_EDGE).round()
    } else {
        DEFAULT_EDGE
    }
}

/// The **log period** `L` the shader is handed, from the authored ring ratio:
/// `ln(radial)` when the repeat is active, and exactly `0` when it is not.
///
/// Zero is the shader's off switch for the whole radial group, which is why this
/// returns a period rather than the ratio: `radial <= 1` (and any non-finite
/// binding) takes the **unmapped** path, not a degenerate case of the mapped one.
/// A ratio of exactly 1 has no period at all — `ln(1) = 0` would make the wrap
/// divide by zero — so the two readings coincide and the constant does double duty.
///
/// The ratio is the authorable parameterization ADR-0077 chose: `2.0` means each
/// ring is half the size of the one outside it, and across a 10:1 radius range
/// `1.3` gives `ln(10)/ln(1.3)` ≈ 9 rings against `2.0`'s ≈ 3. Active values are
/// clamped into `[MIN_RADIAL, MAX_RADIAL]` — see [`MIN_RADIAL`] for why the low
/// bound is not 1.
fn fold_radial(radial: f32) -> f32 {
    if radial.is_finite() && radial > MIN_ACTIVE_RADIAL {
        radial.clamp(MIN_RADIAL, MAX_RADIAL).ln()
    } else {
        0.0
    }
}

/// The winding number the shear is built from: clamped into `±MAX_SPIRAL`, then
/// **rounded to an integer**, with a non-finite binding falling back to no shear.
///
/// This is [`fold_order`]'s treatment for a sharper version of [`fold_order`]'s
/// reason. Shearing `log r` by `k·θ` shifts the radius by `2πk` over one
/// revolution, and the map is periodic in `log r` with period `L`, so the image
/// closes across `atan2`'s branch cut only when `2πk = m·L` for a whole `m`. The
/// authored parameter *is* that `m`; [`spiral_shear`] turns it into `k`. A
/// fractional `m` leaves a seam along the -x ray — the same defect a fractional
/// order produces, from the same branch cut — and `[smoothing]` and preset
/// dissolves both sweep a binding continuously between two settings, so the
/// fractional values are the common case rather than the exotic one.
fn fold_spiral(spiral: f32) -> f32 {
    if spiral.is_finite() {
        spiral.clamp(-MAX_SPIRAL, MAX_SPIRAL).round()
    } else {
        DEFAULT_SPIRAL
    }
}

/// The shear coefficient `k` for a winding number `m` and log period `L`:
/// `k = m·L/(2π)`, the value that makes one revolution shift `log r` by exactly
/// `m` whole periods. Zero whenever the repeat is off, because without a period
/// there is no winding to be a whole number of.
fn spiral_shear(spiral: f32, log_period: f32) -> f32 {
    spiral * log_period / std::f32::consts::TAU
}

/// The zoom the shader is handed, **in rings**, wrapped into one ring.
///
/// The unit is the decision Plan 0064 Phase 4 made and the reason this function
/// does not see `log_period` at all: the shader multiplies by `L` itself, so the
/// authored value is a **ring count** and `kaleido_zoom = 1` advances exactly one
/// ring at every `kaleido_radial`. Authored in raw `log r` it would have been
/// `ln(kaleido_radial)` — a number an author has to recompute every time they
/// re-tune the ratio, and one that silently stops looping when they forget.
///
/// The wrap is not the periodicity — the map is periodic in `log r` whatever
/// offset it is given, and that is asserted on the map itself. It is precision
/// hygiene for the common binding: `kaleido_zoom = "time * k"` grows without
/// bound, and by the time it reaches a few thousand an f32's ulp is a visible
/// fraction of a ring, so the tunnel would step. Reducing it here keeps the
/// uniform in `[0, 1)` and the shader's arithmetic at full precision indefinitely
/// — and reducing *before* the multiply by `L`, rather than after it as the raw
/// `log r` parameterization had to, is strictly the more precise order.
///
/// No `log_period` guard either: with the repeat off the shader never reaches the
/// arm that reads this, so there is nothing to protect against.
fn fold_zoom(zoom: f32) -> f32 {
    if zoom.is_finite() {
        zoom.rem_euclid(1.0)
    } else {
        DEFAULT_ZOOM
    }
}

/// The wallpaper cell count the shader is handed: clamped into the active range,
/// with anything at or below one cell — and any non-finite binding — meaning off.
///
/// Deliberately **not** rounded, unlike the fold order and the winding number.
/// Those two are integral because a fractional value is undefined or torn; a
/// fractional cell count is neither. `abs(fract(x·n/2)·2 − 1)` at `n = 2.5` is a
/// perfectly continuous mirrored grid whose last cell is cut off at the frame
/// edge, so a smoothed `kaleido_tile` can ease between cell counts instead of
/// snapping — the one param on this stage where that is true.
fn fold_tile(tile: f32) -> f32 {
    if tile.is_finite() && tile > MIN_ACTIVE_TILE {
        tile.min(MAX_TILE)
    } else {
        DEFAULT_TILE
    }
}

/// The inner cutoff the shader is handed, as a fraction of `r_max`: clamped into
/// `[0, 1]`, with a non-finite binding falling back to [`DEFAULT_INNER`].
///
/// Clamped, never floored: the default is 0.06 rather than 0 (Plan 0064 Phase 4),
/// and an author who writes `kaleido_inner = "0"` gets exactly that. A default is a
/// resting value, not a minimum — the same distinction `fold_edge` draws between
/// clamping an out-of-range binding and falling back on a broken one.
///
/// Below `inner · r_max` the repeat is *frozen* rather than skipped — the shader
/// takes `r_eff = max(r, inner·r_max)`, which agrees with `r` at the cutoff, so the
/// map stays continuous and the interior becomes radially constant. That constant
/// interior is both the reference images' bright central disc and the reason the
/// cutoff works as an anti-aliasing control: a map with zero radial derivative
/// cannot minify.
fn fold_inner(inner: f32) -> f32 {
    if inner.is_finite() {
        inner.clamp(0.0, MAX_INNER)
    } else {
        DEFAULT_INNER
    }
}

/// The roster's radius map, normalized: given a treatment and `m = r / r_max`,
/// the **sample** radius as a fraction of `r_max`.
///
/// **The shader below is the implementation; this is its CPU mirror**, and it
/// exists so the properties that make each treatment what it claims can be
/// asserted arithmetically rather than argued — that `falloff` and `squash` never
/// reconstruct a coordinate outside the source, and that `tile` is the one arm
/// that does. The two are kept identical by inspection. The *pixel-level* guards
/// on the shader itself live in `core/tests/kaleidoscope.rs`, which is where
/// `tile`'s real safety property is asserted: this function cannot see which
/// sampler an arm reads through, and for `tile` the sampler is the whole
/// guarantee.
///
/// Weight `w` is not mirrored here: it is a plain `smoothstep` on the one arm that
/// uses it and carries no such property.
#[cfg(test)]
fn edge_sample_radius(edge: f32, m: f32) -> f32 {
    // Half-step comparisons and the same arm order as the shader, so the two are
    // one function written twice rather than two functions that agree on the
    // roster's three values.
    if edge < 0.5 {
        m.min(1.0)
    } else if edge < 1.5 {
        m
    } else {
        m.tanh()
    }
}

/// The log-radius repeat's radius map — **the shader below is the
/// implementation; this is its CPU mirror**, kept identical by inspection the way
/// [`edge_sample_radius`] is.
///
/// It exists because the two properties that make the repeat *the* repeat are
/// exact arithmetic rather than pixels, and asserting them here says what is
/// guaranteed instead of measuring what a rasterizer happened to produce:
///
/// - offsetting `zoom` by exactly **1** — one ring, since Plan 0064 Phase 4 made
///   the ring the authored unit — is the **identity**, which is why a driven
///   `kaleido_zoom` is an endless tunnel with no reset;
/// - the map agrees at `θ` and `θ + 2π` for every **integer** winding number and
///   disagrees for a fractional one, which is why [`fold_spiral`] rounds.
///
/// `theta` is the **unfolded** angle (`atan2 + kaleido_angle`), because the seam
/// condition is about a full revolution of it — the folded angle never makes one.
///
/// Guard on the radius the logarithm is taken of, so an exact centre pixel with no
/// inner cutoff yields a finite `log r` instead of `-inf`. Mirrors the shader's
/// literal; it lives here rather than beside the other constants because the
/// shader is a `&'static str` that cannot interpolate one.
#[cfg(test)]
const MIN_LOG_RADIUS: f32 = 1e-8;

#[cfg(test)]
fn repeat_sample_radius(
    r: f32,
    r_max: f32,
    log_period: f32,
    shear: f32,
    zoom: f32,
    inner: f32,
    theta: f32,
) -> f32 {
    // Freeze the repeat below the cutoff. `max` rather than a branch: it agrees
    // with `r` at the cutoff, so the map is continuous there.
    let r_eff = r.max(inner * r_max);
    // `zoom` is in RINGS and is scaled by the period here, mirroring the shader —
    // see `fold_zoom`. That is what makes an offset of exactly 1 the identity.
    let lr = r_eff.max(MIN_LOG_RADIUS).ln() + zoom * log_period + shear * theta;
    let lm = r_max.ln();
    // Wrap into the canonical band (r_max/radial, r_max] — the annulus of the
    // source the whole plane is a self-similar copy of.
    let n = ((lm - lr) / log_period).floor();
    (lr + log_period * n).exp().min(r_max)
}

const SHADER: &str = r#"
struct K {
    v: vec4<f32>, // x: order, y: angle, z: aspect, w: occlude (ADR-0085)
    c: vec4<f32>, // x,y: fold centre (uv), z: falloff band (fraction of r_max),
                  //   w: edge treatment (ADR-0061; integral, quantized CPU-side)
    m: vec4<f32>, // ADR-0077's composed map. x: log period L (0 = repeat OFF),
                  //   y: spiral shear k (= m*L/2pi, m integral CPU-side),
                  //   z: zoom in RINGS (scaled by L below; pre-wrapped into [0,1)),
                  //   w: tile cells across the frame (1 = OFF)
    n: vec4<f32>, // x: inner cutoff (fraction of r_max), yzw: unused
}

@group(0) @binding(0) var<uniform> u: K;
@group(0) @binding(1) var t_src: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;
// `MirrorRepeat`, and used by the `tile` arm alone — the one treatment whose
// sample coordinate is MEANT to leave [0,1]. That is safe only because this
// sampler defines the out-of-range read; wiring `tile` to `samp` above would be
// design-backlog 0010 with a new name (ADR-0061).
@group(0) @binding(3) var samp_tile: sampler;

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let order = max(u.v.x, 1.0);
    let angle = u.v.y;
    let aspect = max(u.v.z, 0.001);
    let centre = u.c.xy;
    let log_period = u.m.x;
    let shear = u.m.y;
    let zoom = u.m.z;
    let tile = u.m.w;
    let inner = u.n.x;

    // --- tile, FIRST in the destination-to-source chain (ADR-0077) ------------
    // Read forwards that means the polar rosette below is the motif this
    // replicates, which is what the reference images show. `n` mirrored cells per
    // axis: `fract` has period 2/n, `*2 - 1` makes it a signed ramp and `abs`
    // reflects it, so alternate cells are mirrored and the grid is continuous
    // across every cell boundary. BOTH axes take the same cell count, so a cell
    // carries the frame's own aspect and the motif inside it is not distorted.
    //
    // PRECONDITION: `tile > 1` (the CPU side sends exactly 1 when off — see
    // `fold_tile`). At one cell the expression is `1 - x`, a flip, not the
    // identity, so "off" cannot be a value of `n`.
    var duv = in.uv;
    if (tile > 1.0) {
        duv = abs(fract(duv * (tile * 0.5)) * 2.0 - 1.0);
    }

    // Centre and aspect-correct so the wedges are radially symmetric.
    var p = duv - centre;
    p.x = p.x * aspect;

    let r = length(p);
    let seg = 6.28318530 / order;
    var a = atan2(p.y, p.x) + angle;
    // The UNFOLDED angle, kept for the spiral shear below: the seam condition is
    // about a full revolution of this one, and the folded angle never makes one.
    let a_raw = a;
    // Wrap into one wedge, then mirror within it (dihedral fold).
    //
    // PRECONDITION: `order` is integral (the CPU side rounds it — see
    // `fold_order`). `atan2` jumps by 2*pi across the -x ray, and this wrap only
    // absorbs that jump when 2*pi is a whole multiple of `seg`. A fractional
    // order tears the frame along that ray.
    //
    // Skipped outright below 1.5, which is the value the CPU sends when the fold
    // term is off but another term keeps the stage alive (`IDENTITY_ORDER`): at
    // `order = 1`, `seg = 2*pi` and `abs(a - seg*0.5)` is a MIRROR about the
    // x-axis, not the identity a preset binding only `kaleido_radial` asked for.
    if (order >= 1.5) {
        a = a - seg * floor(a / seg);
        a = abs(a - seg * 0.5);
    }

    // The largest disc centred on the fold axis that the source rectangle
    // contains, in this same aspect-corrected space: the nearest of the four
    // edges. An off-centre fold shrinks it on one side by construction.
    let r_max = max(min(min(centre.x, 1.0 - centre.x) * aspect,
                        min(centre.y, 1.0 - centre.y)), 0.001);

    let band = max(u.c.z, 0.001);
    let m = r / r_max;

    // What happens outside the disc is the preset's choice (ADR-0061). Every arm
    // is a map from `r` to a SAMPLE radius `rs` and an output weight `w`; the
    // branch is on a uniform-buffer value, so this is one pipeline, one bind
    // layout, one pass, one fetch.
    //
    // PRECONDITION: `u.c.w` is an integer in [0, 2] (`fold_edge` on the CPU side).
    // The comparisons are half-step so a quantized value can only land on its own
    // arm; the fall-through is `squash`. Note the DEFAULT is 1 (`tile`), which is
    // not the fall-through and not the first arm — see `DEFAULT_EDGE`.
    let edge = u.c.w;
    var rs = min(r, r_max);
    var w = 1.0;
    if (log_period > 0.0) {
        // --- the log-radius repeat, and the spiral and zoom that live on it ---
        //
        // Periodicity in log r is scale self-similarity: every destination radius
        // wraps into the canonical band (r_max/radial, r_max], so the frame becomes
        // concentric shrinking copies of that one source annulus.
        //
        // THIS ARM SUBSUMES THE EDGE TREATMENT (ADR-0077's one radius policy).
        // The wrap lands every radius inside the disc by construction, so there is
        // no out-of-disc region left for `kaleido_edge` to treat, nothing to clamp
        // and nothing to fade: `w` stays 1 and `rs <= r_max` holds for every
        // finite `r`. Two radius policies here would fight rather than compose.
        //
        // `inner` freezes the repeat near the axis. `max` rather than a branch
        // because it agrees with `r` at the cutoff, so the map stays continuous
        // and the frozen interior is radially CONSTANT — which is both the
        // reference tunnel's bright central disc and the anti-aliasing control: a
        // map with no radial derivative cannot minify (module docs).
        let r_eff = max(r, inner * r_max);
        // `zoom` arrives in RINGS and is scaled by the period HERE, which is what
        // makes `kaleido_zoom = 1` exactly one ring at every `kaleido_radial`
        // (Plan 0064 Phase 4). The CPU side pre-wraps it into [0,1), so this
        // product stays inside one period however long a `time` binding has run.
        let lr = log(max(r_eff, 1e-8)) + zoom * log_period + shear * a_raw;
        let lm = log(r_max);
        // PRECONDITION: `log_period > 0` selects this arm, so the divide is safe;
        // and `shear = m * log_period / 2pi` for an INTEGER m (`fold_spiral`), so
        // the 2*pi jump `a_raw` takes across the -x ray moves `lr` by a whole
        // number of periods and the wrap absorbs it. A fractional winding seams.
        rs = min(exp(lr + log_period * floor((lm - lr) / log_period)), r_max);
    } else if (edge < 0.5) {
        // 0 `falloff` — ADR-0047's treatment. Clamping the SAMPLE radius, not the
        // output pixel's, is what keeps every reconstructed coordinate inside
        // [0,1]: beyond r_max the polar reconstruction used to land outside the
        // source and `ClampToEdge` smeared the border texel radially into the
        // streaks and chevrons of design-backlog 0010. Past the disc it fades out
        // rather than leaving a flat ring — a plain clamp does NOT leave one, since
        // the clamped sample still varies with angle, so the rim replicates outward
        // as a sunburst of rays (ADR-0047's Outcome). This is what fades those.
        w = 1.0 - smoothstep(r_max, r_max * (1.0 + band), r);
    } else if (edge < 1.5) {
        // 1 `tile`, THE DEFAULT — leave the radius alone and let the MirrorRepeat
        // sampler define the read. The only arm that samples outside [0,1]; safe
        // only because of that sampler, and the original defect if ever wired to
        // `samp`.
        rs = r;
    } else {
        // 2 `squash` — compress the radius asymptotically into the disc. 1:1 at
        // the fold axis (tanh'(0) = 1) and approaching r_max at the corners, so it
        // crops nothing and draws no ray, at the cost of bending geometry. NOT the
        // identity inside the disc: tanh(m) < m everywhere past the axis, so the
        // whole interior is pulled inward.
        rs = r_max * tanh(m);
    }

    // Reconstruct the sample coordinate from the folded angle + sample radius.
    var q = vec2<f32>(cos(a), sin(a)) * rs;
    q.x = q.x / aspect;
    let s_uv = q + centre;
    // Two `textureSample` calls, one per address mode, each in UNIFORM control
    // flow — the branch is on a uniform-buffer value, which is what `textureSample`
    // requires. Only one executes per fragment. With the repeat arm live the two
    // are interchangeable — `rs <= r_max` puts `s_uv` inside `[0,1]`, where no
    // address mode is reachable — so the selection stays exactly as ADR-0061 left
    // it rather than growing a third case that could not be observed.
    var col: vec4<f32>;
    if (edge > 0.5 && edge < 1.5) {
        col = textureSample(t_src, samp_tile, s_uv);
    } else {
        col = textureSample(t_src, samp, s_uv);
    }
    // `w` scales COLOUR AND ALPHA together (ADR-0055). The values are
    // premultiplied, so this fades to *transparent* and the backdrop composited
    // underneath the chain shows through. Multiplying only `.rgb` and forcing
    // alpha to 1 is what made the falloff fade to black and fight `bg_*` instead
    // of landing on it. The three fill arms leave `w = 1`, so they carry the
    // source's own alpha out to the frame edge.
    //
    // Alpha is scaled once more by `occlude` — how much of the fold's coverage the
    // backdrop underneath resolves against (ADR-0085). 1.0 folding into a scratch
    // offscreen and by default, where the multiply is exact.
    let out = col * w;
    return vec4<f32>(out.rgb, out.a * u.v.w);
}
"#;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct K {
    v: [f32; 4],
    c: [f32; 4],
    /// ADR-0077's composed map: log period, spiral shear, zoom, tile cells.
    m: [f32; 4],
    /// `x` is the inner cutoff; the rest is padding to the `vec4` the WGSL side
    /// declares.
    n: [f32; 4],
}

struct Resources {
    // The offscreen the scene (or the trails output) renders into.
    /// The grid these were built for, so `begin` can compare before rebuilding.
    size: (u32, u32),
    // Kept alive so `src_view` stays valid; not read after construction.
    _src: wgpu::Texture,
    src_view: wgpu::TextureView,
    uniform: wgpu::Buffer,
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
}

impl Resources {
    fn build(device: &wgpu::Device, surface_format: wgpu::TextureFormat, size: (u32, u32)) -> Self {
        let src = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("kaleido-src"),
            size: wgpu::Extent3d {
                width: size.0,
                height: size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: surface_format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let src_view = src.create_view(&wgpu::TextureViewDescriptor::default());
        // `falloff` and `squash` keep their sample radius inside the inscribed
        // disc, so they never sample outside [0,1] and this address mode is
        // unreachable for them — it stays `ClampToEdge` as the defined fallback.
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("kaleido-sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        // The third — `tile` (ADR-0061) — is the one whose coordinate leaves
        // `[0,1]` on purpose, and it is safe *only* because this sampler defines
        // what a read out there means. Reflecting rather than repeating is what
        // keeps the continuation continuous at the source border instead of
        // wrapping the far edge in. Built unconditionally so the layout shape does
        // not depend on a param value.
        //
        // Since Plan 0055 Phase 3 this is the sampler the DEFAULT reads through, so
        // it is on the path of every fold-bearing preset rather than an opt-in.
        let sampler_tile = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("kaleido-sampler-tile"),
            address_mode_u: wgpu::AddressMode::MirrorRepeat,
            address_mode_v: wgpu::AddressMode::MirrorRepeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("kaleido-uniform"),
            size: std::mem::size_of::<K>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let shader = gpu::fullscreen_shader(
            device,
            "kaleido-shader",
            gpu::FULLSCREEN_VS_UV_FLIPPED,
            SHADER,
        );
        // `[Uniform, Texture, Sampler, Sampler]` since ADR-0061 added `tile`'s
        // second address mode. Re-derived after Plan 0055 Phase 3 deleted `mirror`
        // and `vignette` rather than carried over: neither of those needed a
        // sampler, `tile` survived, so the shape is unchanged from Phase 1.
        //
        // That is one entry longer than the `[Uniform, Texture, Sampler]` shape
        // ADR-0058 records this layout under — a shape it shared with
        // `ink-bind-layout` — so the fold is now the more distinctive of the two
        // and that particular collision is off the list.
        let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("kaleido-bind-layout"),
            entries: &[
                gpu::uniform(0, wgpu::ShaderStages::FRAGMENT),
                gpu::texture(1, true),
                gpu::sampler(2),
                gpu::sampler(3),
            ],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("kaleido-bind-group"),
            layout: &bind_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&src_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&sampler_tile),
                },
            ],
        });
        // Premultiplied-alpha OVER, not REPLACE (ADR-0055). Into the chain's
        // destination this composites the fold over the backdrop painted there.
        // Into an intermediate stage's input — which `Fold::Own` has just cleared
        // to transparent — it is *bit-identical* to REPLACE, since
        // `src + dst * (1 - src.a)` with `dst = 0` is `src` in every channel. One
        // pipeline covers both, so the stage's pipeline count is unchanged and the
        // WARP sensitivity documented in `post.rs` is not disturbed.
        let pipeline = gpu::fullscreen_pipeline(
            device,
            &shader,
            &[&bind_layout],
            surface_format,
            wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING,
            "kaleido",
        );

        Self {
            size,
            _src: src,
            src_view,
            uniform,
            pipeline,
            bind_group,
        }
    }
}

/// The engine screen-space kaleidoscope stage — a [`PostStage`], not a
/// [`Scene`](super::scenes::Scene): it consumes an already-rendered frame rather
/// than an `AnalysisFrame`. Driven by the `kaleido_*` named params, it folds
/// whatever the chain hands it — the trails output when that stage is active,
/// otherwise the scene alone — before the next stage or present (ADR-0018). The
/// backdrop is **not** in that input: it is composited underneath the chain
/// (ADR-0055), so the fold never folds `bg_*`.
pub struct Kaleidoscope {
    device: wgpu::Device,
    surface_format: wgpu::TextureFormat,
    res: Option<Resources>,
    order: f32,
    angle: f32,
    center_x: f32,
    center_y: f32,
    /// The out-of-disc treatment (ADR-0061), raw as the preset bound it —
    /// [`fold_edge`] quantizes it on the way to the uniform.
    edge: f32,
    /// The composed map's terms (ADR-0077), each raw as the preset bound it —
    /// [`fold_radial`], [`fold_spiral`], [`fold_zoom`], [`fold_tile`] and
    /// [`fold_inner`] condition them on the way to the uniform.
    radial: f32,
    spiral: f32,
    zoom: f32,
    tile: f32,
    inner: f32,
    /// The active tier's cap on this stage's internal grid — see
    /// [`Trails::post_cap`](super::trails::Trails).
    post_cap: (u32, u32),
    /// How many times [`Resources::build`] has run — see
    /// [`Trails::builds`](super::trails::Trails).
    builds: u32,
}

/// Global parameter vocabulary — see [`background::PARAMS`](super::background::PARAMS).
/// **Keep in sync with `set_param` below.**
pub const PARAMS: &[&str] = &[
    "kaleido_order",
    "kaleido_angle",
    "kaleido_center_x",
    "kaleido_center_y",
    "kaleido_edge",
    // ADR-0077's composed map, in the order it is applied
    // (destination-to-source): tile -> fold -> radial -> spiral, with `zoom` and
    // `inner` riding on the radial term.
    "kaleido_tile",
    "kaleido_radial",
    "kaleido_spiral",
    "kaleido_zoom",
    "kaleido_inner",
];

impl Kaleidoscope {
    /// Store the device/format for a lazy build; no GPU resources yet.
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        post_cap: (u32, u32),
    ) -> Self {
        Self {
            device: device.clone(),
            surface_format,
            res: None,
            order: DEFAULT_ORDER,
            angle: DEFAULT_ANGLE,
            center_x: DEFAULT_CENTER,
            center_y: DEFAULT_CENTER,
            edge: DEFAULT_EDGE,
            radial: DEFAULT_RADIAL,
            spiral: DEFAULT_SPIRAL,
            zoom: DEFAULT_ZOOM,
            tile: DEFAULT_TILE,
            inner: DEFAULT_INNER,
            post_cap,
            builds: 0,
        }
    }

    /// How many times this stage has built its GPU resources.
    #[cfg(test)]
    pub(crate) fn build_count(&self) -> u32 {
        self.builds
    }

    /// Whether the **fold term** is live — order at least 2.
    ///
    /// Not the same question as whether the *stage* is live (ADR-0077):
    /// another term can keep it running with the fold off, and the
    /// uniform then carries [`IDENTITY_ORDER`].
    fn fold_active(&self) -> bool {
        self.order >= MIN_ACTIVE_ORDER && self.order.is_finite()
    }
}

impl PostStage for Kaleidoscope {
    fn name(&self) -> &'static str {
        "kaleidoscope"
    }

    /// Reset the fold params to their defaults (each frame, before routing).
    fn reset_params(&mut self) {
        self.order = DEFAULT_ORDER;
        self.angle = DEFAULT_ANGLE;
        self.center_x = DEFAULT_CENTER;
        self.center_y = DEFAULT_CENTER;
        self.edge = DEFAULT_EDGE;
        self.radial = DEFAULT_RADIAL;
        self.spiral = DEFAULT_SPIRAL;
        self.zoom = DEFAULT_ZOOM;
        self.tile = DEFAULT_TILE;
        self.inner = DEFAULT_INNER;
    }

    /// Apply one named parameter, returning whether it was a `kaleido_*` param.
    fn set_param(&mut self, name: &str, value: f32) -> bool {
        match name {
            "kaleido_order" => self.order = value,
            "kaleido_angle" => self.angle = value,
            "kaleido_center_x" => self.center_x = value,
            "kaleido_center_y" => self.center_y = value,
            "kaleido_edge" => self.edge = value,
            "kaleido_tile" => self.tile = value,
            "kaleido_radial" => self.radial = value,
            "kaleido_spiral" => self.spiral = value,
            "kaleido_zoom" => self.zoom = value,
            "kaleido_inner" => self.inner = value,
            _ => return false,
        }
        true
    }

    fn params(&self) -> &'static [&'static str] {
        PARAMS
    }

    /// Whether **any** term of the composed map is active this frame — the fold
    /// (order at least 2), the log-radius repeat (ratio above 1), or the wallpaper
    /// tile (more than one cell). With none of them the stage is the identity and
    /// the [`PostChain`](super::post::PostChain) skips it entirely.
    ///
    /// `kaleido_spiral`, `kaleido_zoom` and `kaleido_inner` deliberately do **not**
    /// appear: all three are properties *of* the repeat — a winding number with no
    /// period to wind, an offset along a `log r` nothing wraps, and a cutoff on a
    /// repeat that is not happening are each a no-op — so none of them can wake the
    /// stage on its own.
    fn active(&self) -> bool {
        self.fold_active()
            || (self.radial.is_finite() && self.radial > MIN_ACTIVE_RADIAL)
            || (self.tile.is_finite() && self.tile > MIN_ACTIVE_TILE)
    }

    /// The fold-input size, following the render target under the shared policy
    /// (ADR-0034) — reported to a scene that sizes an internal field, as the trails
    /// stage's is. A **texel count only**: the aspect the composite renders at, and
    /// the one [`resolve`](PostStage::resolve) folds about, is the render target's
    /// (ADR-0037).
    fn internal_size(&self, surface: (u32, u32)) -> (u32, u32) {
        internal_grid_size(surface, self.post_cap)
    }

    /// Build the resources if needed and return the offscreen view the scene (or
    /// the trails output) renders into this frame. `None` only if the resources are
    /// absent (never, after the build) — the caller falls back to the surface.
    /// Called when [`active`](PostStage::active).
    fn begin(
        &mut self,
        _encoder: &mut wgpu::CommandEncoder,
        surface: (u32, u32),
    ) -> Option<wgpu::TextureView> {
        // Compare-first (ADR-0030): build once, then only when the grid changes.
        let wanted = self.internal_size(surface);
        if self.res.as_ref().is_none_or(|res| res.size != wanted) {
            self.res = Some(Resources::build(&self.device, self.surface_format, wanted));
            self.builds += 1;
        }
        self.res.as_ref().map(|res| res.src_view.clone())
    }

    /// Fold the input offscreen into `out` — the next active stage's input, or the
    /// chain's destination. Called after the scene has rendered into the
    /// [`begin`](PostStage::begin) target, when [`active`](PostStage::active).
    fn resolve(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        out: &wgpu::TextureView,
        surface: (u32, u32),
        fold: Fold,
    ) -> u32 {
        let Some(res) = self.res.as_ref() else {
            return 0;
        };
        // `IDENTITY_ORDER` when the fold term is off: the stage may be running for
        // the repeat or the tile alone, and `fold_order` would clamp an unbound
        // order UP into the active range and fold a frame nobody asked to fold.
        let order = if self.fold_active() {
            fold_order(self.order)
        } else {
            IDENTITY_ORDER
        };
        // ADR-0077's composed map. `log_period` is the whole radial group's off
        // switch (0 = off), which is why the shear and the zoom are derived from
        // it rather than sent raw: neither means anything without a period.
        let log_period = fold_radial(self.radial);
        let shear = spiral_shear(fold_spiral(self.spiral), log_period);
        // In rings; the shader scales by `log_period` (`fold_zoom`).
        let zoom = fold_zoom(self.zoom);
        // The **render target's** ratio, not this stage's input grid's (ADR-0037).
        // The fold happens in the destination's space and the frame it samples was
        // drawn pre-squashed at this same aspect, so both the output geometry and
        // the reconstructed sample coordinate want the shape the frame is finally
        // seen at. The grid's own aspect is a resolution artefact — quantized to a
        // 256 px step — and correcting by it skewed every wedge on any window the
        // step did not divide evenly.
        let aspect = surface.0 as f32 / surface.1.max(1) as f32;
        queue.write_buffer(
            &res.uniform,
            0,
            bytemuck::bytes_of(&K {
                v: [order, self.angle, aspect, fold.alpha_scale()],
                c: [
                    fold_center(self.center_x),
                    fold_center(self.center_y),
                    FALLOFF_BAND,
                    fold_edge(self.edge),
                ],
                m: [log_period, shear, zoom, fold_tile(self.tile)],
                n: [fold_inner(self.inner), 0.0, 0.0, 0.0],
            }),
        );
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("kaleido-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: out,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: fold.load_op(),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&res.pipeline);
        pass.set_bind_group(0, &res.bind_group, &[]);
        pass.draw(0..3, 0..1);

        1 // the fold pass
    }

    /// Drop the lazily-built resources — used on the capture scene-rebuild so a
    /// stale fold pipeline never lingers to mis-render the next capture's scene on
    /// the WARP adapter (module docs).
    fn reset_resources(&mut self) {
        self.res = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The uniform never carries a fractional wedge count, whatever a preset (or
    /// the smoothing that eases between two ladder steps) hands the stage. The
    /// pixel-level consequence — no tear across the -x ray — is
    /// `core/tests/kaleidoscope.rs`; this pins the arithmetic that guarantees it.
    #[test]
    fn fold_order_is_always_integral() {
        for &raw in &[2.0f32, 2.4, 6.0, 12.5, 12.4999, 13.5, 30.7, 47.999] {
            let order = fold_order(raw);
            assert_eq!(
                order,
                order.round(),
                "fold_order({raw}) = {order} is not an integer"
            );
        }
        // Nearest-integer, not truncation: an eased sweep must land on the step
        // it is closest to rather than always the one below it.
        assert_eq!(fold_order(12.4), 12.0);
        assert_eq!(fold_order(12.6), 13.0);
    }

    /// Rounding happens inside the active range, so it can never hand the shader
    /// an order the stage would have skipped (`< MIN_ACTIVE_ORDER`) or one past
    /// the blur ceiling.
    #[test]
    fn fold_order_stays_within_the_active_range() {
        assert_eq!(fold_order(0.0), MIN_ACTIVE_ORDER);
        assert_eq!(fold_order(1.9), MIN_ACTIVE_ORDER);
        assert_eq!(fold_order(1e9), MAX_ORDER);
        assert_eq!(fold_order(f32::NEG_INFINITY), MIN_ACTIVE_ORDER);
    }

    /// The fold axis stays inside the frame whatever a binding drives it to. A
    /// centre outside `[0, 1]` has no inscribed disc — `r_max` would be negative
    /// — so this is what keeps an overshooting sweep at the edge rather than
    /// blanking the picture to the backdrop.
    #[test]
    fn fold_center_stays_inside_the_frame() {
        assert_eq!(fold_center(0.5), 0.5);
        assert_eq!(fold_center(0.2), 0.2);
        assert_eq!(fold_center(-3.0), 0.0);
        assert_eq!(fold_center(1.4), 1.0);
        // Not merely finite-checked away: NaN would otherwise reach the uniform
        // and every comparison in the shader's `min` chain would go false.
        assert_eq!(fold_center(f32::NAN), DEFAULT_CENTER);
        assert_eq!(fold_center(f32::INFINITY), DEFAULT_CENTER);
    }

    // --- The edge treatment (ADR-0061) -------------------------------------
    //
    // The shader is the implementation; `edge_sample_radius` is its CPU mirror
    // and what these assert against (see its docs). Each test states the property
    // that makes a treatment *that* treatment, so a future edit to the map has to
    // break a named claim rather than merely a pixel.

    /// 16:9, the aspect the whole out-of-disc question is sized at.
    const ASPECT_16_9: f32 = 16.0 / 9.0;

    /// Where the frame's corner sits, as a multiple of `r_max`.
    ///
    /// A centred fold in the shader's aspect-corrected space has `r_max = 0.5` and
    /// a corner at `0.5 * sqrt(aspect^2 + 1)`, so the ratio is `sqrt(aspect^2 + 1)`
    /// — independent of the frame's size, and the reason this is arithmetic rather
    /// than a measurement.
    fn corner_m(aspect: f32) -> f32 {
        (aspect * aspect + 1.0).sqrt()
    }

    /// The uniform never carries a fractional treatment or one outside the roster,
    /// whatever a binding — or the `[smoothing]` easing between two settings —
    /// hands the stage. `kaleido_order`'s guard, on `kaleido_order`'s seam, for the
    /// reason in [`fold_edge`]'s docs.
    #[test]
    fn fold_edge_is_always_a_roster_value() {
        for &raw in &[-9.0f32, 0.0, 0.4, 1.0, 1.6, 2.0, 2.4, 400.0] {
            let e = fold_edge(raw);
            assert_eq!(e, e.round(), "fold_edge({raw}) = {e} is not an integer");
            assert!(
                (MIN_EDGE..=MAX_EDGE).contains(&e),
                "fold_edge({raw}) = {e} is outside the roster [{MIN_EDGE}, {MAX_EDGE}]"
            );
        }

        // Nearest-integer, not truncation: an eased sweep from one treatment to
        // another must land on the step it is closest to, so it snaps at the
        // midpoint rather than lagging a whole treatment behind.
        assert_eq!(fold_edge(0.4), MIN_EDGE);
        assert_eq!(fold_edge(0.6), 1.0);
        assert_eq!(fold_edge(1.4), 1.0);
        assert_eq!(fold_edge(1.6), MAX_EDGE);

        // Out of range clamps to the nearer BOUND, which is not the default —
        // the distinction that only exists because the default is the roster's
        // middle value. An under-driven binding lands on `falloff`, not on `tile`.
        assert_eq!(fold_edge(-1.0), MIN_EDGE);
        assert_eq!(fold_edge(99.0), MAX_EDGE);
        assert_ne!(
            fold_edge(-1.0),
            DEFAULT_EDGE,
            "clamping and the non-finite fallback must stay distinguishable"
        );

        // Non-finite falls back to the DEFAULT, not to a clamp bound: a selector
        // has no "as far as you can go" reading, so a broken binding lands on the
        // resting treatment.
        assert_eq!(fold_edge(f32::NAN), DEFAULT_EDGE);
        assert_eq!(fold_edge(f32::INFINITY), DEFAULT_EDGE);
        assert_eq!(fold_edge(f32::NEG_INFINITY), DEFAULT_EDGE);
    }

    /// `falloff` and `squash` keep their **sample** radius inside the disc, so
    /// neither can reconstruct a coordinate outside the source — ADR-0047's real
    /// guarantee, and the mechanism behind design-backlog 0010's smear. `tile` is
    /// the deliberate exception, and the reason it needs its own sampler.
    ///
    /// The sweep runs well past the 16:9 corner because a mis-implementation that
    /// only escapes far out would read correctly at every `m` below the rim.
    #[test]
    fn only_tile_lets_the_sample_radius_leave_the_disc() {
        let mut m = 0.0f32;
        while m <= 3.0 {
            for edge in [MIN_EDGE, MAX_EDGE] {
                let rs = edge_sample_radius(edge, m);
                assert!(
                    (0.0..=1.0).contains(&rs),
                    "treatment {edge} maps m = {m} to rs = {rs} r_max, outside the source"
                );
            }
            m += 0.01;
        }

        // Non-vacuity, and the roster's one deliberate exception — which is also
        // the DEFAULT, so this is the arm every unbound fold-bearing preset takes.
        // Its safety is the MirrorRepeat sampler, which this function cannot see;
        // the guard that can is the ray-variance property in
        // `core/tests/kaleidoscope.rs`.
        let corner = corner_m(ASPECT_16_9);
        assert!(
            edge_sample_radius(DEFAULT_EDGE, corner) > 1.0,
            "tile no longer leaves the disc, so the check above distinguishes nothing"
        );
    }

    /// `squash` is 1:1 **at the axis** and asymptotic to the rim — it never crops
    /// and never leaves the disc, at the cost of compressing the whole interior.
    ///
    /// Note this is *not* the identity below `r_max` the way a clamp is: Plan 0055
    /// Phase 1's done-when grouped `squash` with the (since-deleted) `mirror` as
    /// leaving the disc interior untouched, but `tanh(m) < m` for every `m > 0`, so
    /// `squash` pulls the whole interior inward. The formula is the one both the
    /// plan's roster table and ADR-0061's give; the grouping in the prose is what
    /// does not hold, and ADR-0061's Outcome carries the correction.
    #[test]
    fn squash_is_one_to_one_at_the_axis_and_asymptotic_to_the_rim() {
        // 1:1 at the fold axis: tanh'(0) = 1, so the ratio tends to 1. At m = 1e-4
        // the series error is ~m^2/3, some four orders below this bound.
        let tiny = 1e-4f32;
        assert!(
            (edge_sample_radius(MAX_EDGE, tiny) / tiny - 1.0).abs() < 1e-5,
            "squash is not 1:1 at the fold axis"
        );

        // Asymptotic, never reaching: no crop, no ray, and nothing sampled outside
        // the source however far out the pixel is. `m` runs to 8 because a fold
        // centre clamped to the frame edge shrinks `r_max` without bound, so large
        // ratios are reachable and not merely hypothetical.
        //
        // Only non-decreasing out here, deliberately: past `m ~ 7.6` consecutive
        // steps of `tanh` land within one f32 ulp of each other, so "asymptotic"
        // and "constant" stop being distinguishable in the type. That costs
        // nothing — the guarantee is that the radius stays inside the disc, and it
        // does — but asserting strict growth there would be asserting a property of
        // f32 rather than of the map.
        let mut prev = 0.0f32;
        let mut m = 0.05f32;
        while m <= 8.0 {
            let rs = edge_sample_radius(MAX_EDGE, m);
            assert!(rs < 1.0, "squash reached the rim at m = {m} (rs = {rs})");
            assert!(rs >= prev, "squash went backwards at m = {m}");
            prev = rs;
            m += 0.05;
        }

        // Strictly increasing across every ratio a frame actually presents: the
        // corner sits at `sqrt(aspect^2 + 1)` — 2.04 at 16:9, 2.28 at the portrait
        // shape the disc guard captures at — so 4 is comfortably past both. This is
        // the range in which "compresses without cropping" has to mean that
        // distinct radii stay distinct, or the corners flatten into a ring.
        let mut prev = 0.0f32;
        let mut m = 0.05f32;
        while m <= 4.0 {
            let rs = edge_sample_radius(MAX_EDGE, m);
            assert!(
                rs > prev,
                "squash is not strictly monotone at m = {m}, which is inside the range \
                 a real frame reaches — distinct radii must stay distinct there"
            );
            prev = rs;
            m += 0.05;
        }

        // ...and it is a compression of the interior, not the identity there.
        assert!(
            edge_sample_radius(MAX_EDGE, 0.5) < 0.5,
            "squash left the disc interior untouched — it is tanh, which compresses \
             everywhere past the axis"
        );
    }

    // --- The composed coordinate map (ADR-0077) -----------------------------
    //
    // The shader is the implementation; `repeat_sample_radius` is its CPU mirror
    // and what the two exact properties are asserted against. Both are arithmetic
    // — a periodicity and a seam condition — so they are stated here rather than
    // measured on pixels, where a rasterizer's tolerance would blunt them.

    /// The disc radius a centred fold has in the shader's aspect-corrected space.
    const R_MAX: f32 = 0.5;

    /// Destination radii the properties are sampled over: from well inside the
    /// disc out past the 16:9 corner (2.04x `r_max`), which is the range a real
    /// frame presents.
    const SAMPLE_RADII: [f32; 9] = [0.004, 0.02, 0.06, 0.12, 0.25, 0.4, 0.5, 0.75, 1.05];

    /// Relative agreement required of two maps the arithmetic says are the same
    /// map. Not bit-equality: the two reach the identical value by different
    /// roundings — `(lr + L) + L*(n-1)` against `lr + L*n` — so f32 associativity
    /// is the whole gap, and it is a few ulps.
    const MAP_REL_TOL: f32 = 1e-5;

    fn assert_close_tol(a: f32, b: f32, tol: f32, what: &str) {
        let rel = (a - b).abs() / a.abs().max(b.abs()).max(f32::MIN_POSITIVE);
        assert!(
            rel <= tol,
            "{what}: {a} vs {b} (relative {rel:e}, tolerance {tol:e})"
        );
    }

    fn assert_close(a: f32, b: f32, what: &str) {
        assert_close_tol(a, b, MAP_REL_TOL, what);
    }

    /// The ring ratio maps to a log period only when the repeat is on, and `off`
    /// is the unmapped path rather than a degenerate ratio.
    #[test]
    fn fold_radial_is_off_at_or_below_one_and_a_log_period_above_it() {
        // Off — and exactly 0, which is the shader's off switch for the entire
        // radial group (the spiral and the zoom hang off the same value).
        assert_eq!(fold_radial(1.0), 0.0);
        assert_eq!(fold_radial(0.4), 0.0);
        assert_eq!(fold_radial(-3.0), 0.0);
        assert_eq!(fold_radial(f32::NAN), 0.0);
        assert_eq!(fold_radial(f32::INFINITY), 0.0);

        // On — the period is the log of the authored ratio, so the ring count over
        // a decade of radius is ln(10)/L: about 9 rings at 1.3 and 3 at 2.0, the
        // two figures ADR-0077 sizes the parameterization by.
        assert_close(fold_radial(2.0), std::f32::consts::LN_2, "ln(2)");
        let rings = |ratio: f32| 10.0f32.ln() / fold_radial(ratio);
        assert!(
            (rings(1.3) - 9.0).abs() < 0.5,
            "a decade of radius holds {} rings at ratio 1.3, not ~9",
            rings(1.3)
        );
        assert!(
            (rings(2.0) - 3.0).abs() < 0.5,
            "a decade of radius holds {} rings at ratio 2.0, not ~3",
            rings(2.0)
        );

        // An active ratio never approaches 1, where L would approach 0 and the
        // wrap's divide would lose the band to f32 precision.
        assert_eq!(fold_radial(1.000001), MIN_RADIAL.ln());
        assert_eq!(fold_radial(1e9), MAX_RADIAL.ln());
    }

    /// **The seamless-loop property.** Offsetting `kaleido_zoom` by exactly
    /// **1.0** is the identity map, at every destination coordinate and at every
    /// ring ratio — which is what makes a driven zoom an endless tunnel with no
    /// reset and no crossfade, rather than a loop with a hidden cut.
    ///
    /// The identity offset was `L = ln(kaleido_radial)` when Phase 1 landed this,
    /// because the parameter was a raw `log r` offset. Plan 0064 Phase 4 made the
    /// **ring** the authored unit, and the assertion got strictly stronger for it:
    /// `1.0` is a constant rather than a function of the ratio, so the loop closes
    /// at the *same* authored number however the ratio is re-tuned — which is the
    /// property an author binding `"bar_phase * 1.0"` is actually relying on. A
    /// regression that reintroduced the raw-`log r` parameterization would fail
    /// here at every ratio except the one where `ln(ratio)` happens to be 1.
    #[test]
    fn a_zoom_offset_of_one_period_is_the_identity_map() {
        for ratio in [1.15f32, 1.3, 2.0, 3.5] {
            let l = fold_radial(ratio);
            for &r in &SAMPLE_RADII {
                for i in 0..12 {
                    let theta = -std::f32::consts::PI + std::f32::consts::TAU * i as f32 / 12.0;
                    // Sampled with the spiral and the inner cutoff live, so the
                    // property is of the whole map and not of a stripped one.
                    let shear = spiral_shear(2.0, l);
                    let base = repeat_sample_radius(r, R_MAX, l, shear, 0.0, 0.05, theta);
                    let shifted = repeat_sample_radius(r, R_MAX, l, shear, 1.0, 0.05, theta);
                    assert_close(
                        base,
                        shifted,
                        &format!(
                            "ratio {ratio}, r {r}, theta {theta}: zoom + 1 ring is not the identity"
                        ),
                    );
                }
            }
        }

        // Non-vacuity, and the guard against the offset being ignored outright: a
        // HALF ring must move the map. Checked at a radius the inner cutoff does
        // not freeze, and away from the exact powers of the ratio where the
        // self-similarity maps a shifted radius back onto itself.
        let l = fold_radial(2.0);
        let base = repeat_sample_radius(0.3, R_MAX, l, 0.0, 0.0, 0.05, 0.4);
        let half = repeat_sample_radius(0.3, R_MAX, l, 0.0, 0.5, 0.05, 0.4);
        assert!(
            (base / half - 1.0).abs() > 0.05,
            "half a ring left the map unchanged ({base} vs {half}) — the zoom term \
             is not reaching the map at all"
        );
    }

    /// **The seam condition, and the reason [`fold_spiral`] rounds.** Across
    /// `atan2`'s branch cut the unfolded angle jumps by exactly `2*pi`; the map
    /// absorbs that jump only when the shear moves `log r` by a whole number of
    /// periods, which is exactly the integer winding numbers. A fractional one
    /// leaves a discontinuity — a visible seam along the -x ray.
    #[test]
    fn only_an_integer_winding_number_closes_across_the_branch_cut() {
        let l = fold_radial(1.3);
        let theta = -std::f32::consts::PI + 0.017;

        // Integral windings close, at every radius.
        for m in [-3.0f32, -1.0, 0.0, 1.0, 2.0, 5.0] {
            let shear = spiral_shear(m, l);
            for &r in &SAMPLE_RADII {
                let here = repeat_sample_radius(r, R_MAX, l, shear, 0.0, 0.0, theta);
                let round = repeat_sample_radius(
                    r,
                    R_MAX,
                    l,
                    shear,
                    0.0,
                    0.0,
                    theta + std::f32::consts::TAU,
                );
                assert_close(
                    here,
                    round,
                    &format!("winding {m} at r {r} does not close across the branch cut"),
                );
            }
        }

        // ...and a fractional one does not, or the rounding guards nothing. Half a
        // winding shifts `log r` by half a period, which is the worst case and the
        // one a `[smoothing]` sweep sits on for as long as it sits anywhere.
        let mut seamed = 0usize;
        for m in [0.5f32, 1.5, -2.5] {
            let shear = spiral_shear(m, l);
            for &r in &SAMPLE_RADII {
                let here = repeat_sample_radius(r, R_MAX, l, shear, 0.0, 0.0, theta);
                let round = repeat_sample_radius(
                    r,
                    R_MAX,
                    l,
                    shear,
                    0.0,
                    0.0,
                    theta + std::f32::consts::TAU,
                );
                // A half-period jump in log r is a ratio of sqrt(1.3) = 1.14, so
                // "differs" here is a 14 % step in the sampled radius, not noise.
                if (here / round - 1.0).abs() > 0.05 {
                    seamed += 1;
                }
            }
        }
        assert_eq!(
            seamed,
            3 * SAMPLE_RADII.len(),
            "a fractional winding number closed across the branch cut at some radius — \
             then the seam it is supposed to draw is not there, and `fold_spiral`'s \
             rounding is guarding nothing"
        );

        // And the guard itself: the uniform never carries a fractional winding.
        for &raw in &[-99.0f32, -2.4, 0.0, 0.5, 1.4, 1.6, 99.0] {
            let m = fold_spiral(raw);
            assert_eq!(m, m.round(), "fold_spiral({raw}) = {m} is not an integer");
            assert!((-MAX_SPIRAL..=MAX_SPIRAL).contains(&m));
        }
        // Non-finite means NO shear rather than the far end of the range, for
        // `fold_edge`'s reason: a broken binding should land on the resting
        // behaviour, and "as far as you can wind" is not a reading of infinity
        // anyone authored.
        assert_eq!(fold_spiral(f32::NAN), DEFAULT_SPIRAL);
        assert_eq!(fold_spiral(f32::INFINITY), DEFAULT_SPIRAL);
        assert_eq!(fold_spiral(f32::NEG_INFINITY), DEFAULT_SPIRAL);
    }

    /// The repeat lands **every** radius inside the disc — that is what lets this
    /// arm subsume the edge treatment (ADR-0077's one radius policy) instead of
    /// composing with it, and it must hold out past the frame corner.
    #[test]
    fn the_repeat_keeps_every_radius_inside_the_disc() {
        let l = fold_radial(2.0);
        let shear = spiral_shear(3.0, l);
        let mut r = 0.0f32;
        while r <= 1.2 {
            let rs = repeat_sample_radius(r, R_MAX, l, shear, 0.31, 0.0, 1.1);
            assert!(
                rs.is_finite() && rs > 0.0 && rs <= R_MAX,
                "the repeat mapped r = {r} to rs = {rs}, outside (0, r_max]"
            );
            // ...and into the canonical band, not merely inside the disc: the
            // whole plane is a self-similar copy of ONE source annulus.
            assert!(
                rs > R_MAX / 2.0 * (1.0 - MAP_REL_TOL),
                "the repeat mapped r = {r} to rs = {rs}, below the canonical band \
                 (r_max/radial, r_max]"
            );
            r += 0.001;
        }

        // The exact fold axis included: with no inner cutoff, log(0) would be
        // -inf, and the guard is what keeps the centre pixel a colour.
        let at_axis = repeat_sample_radius(0.0, R_MAX, l, shear, 0.0, 0.0, 0.0);
        assert!(
            at_axis.is_finite() && at_axis > 0.0,
            "the fold axis mapped to {at_axis} — the log guard is gone"
        );
    }

    /// The inner cutoff **freezes** the repeat rather than skipping it: the map is
    /// continuous at the cutoff and radially constant inside it. The constancy is
    /// the anti-aliasing property — a map with no radial derivative cannot minify
    /// — and the continuity is why it does not draw a ring.
    #[test]
    fn the_inner_cutoff_is_continuous_and_freezes_the_interior() {
        let l = fold_radial(2.0);
        let inner = 0.1f32;
        let cutoff = inner * R_MAX;
        let map = |r: f32| repeat_sample_radius(r, R_MAX, l, 0.0, 0.0, inner, 0.4);

        // Constant inside.
        let at_cutoff = map(cutoff);
        for f in [0.0f32, 0.01, 0.25, 0.5, 0.9, 0.999] {
            assert_close(
                map(cutoff * f),
                at_cutoff,
                &format!("the interior is not frozen at {f} of the cutoff"),
            );
        }

        // Continuous across it — no ring. A *skipping* cutoff (take the plain
        // radius inside, the wrapped one outside) jumps by an arbitrary fraction
        // of the whole band here; the frozen one moves only as far as the repeat
        // itself does over the same 0.1 % step in `r`, which at ratio 2 is 0.1 %.
        // The bound is 30x that and still two orders below a skip's jump.
        assert_close_tol(
            map(cutoff * 1.001),
            at_cutoff,
            0.03,
            "the map jumps at the inner cutoff",
        );

        // Non-vacuity: outside the cutoff the repeat is doing something, or
        // "frozen inside" would be a statement about a constant map.
        //
        // The multiplier is deliberately NOT a whole power of the ring ratio.
        // 4 = 2^2 at ratio 2 is exactly two periods, so the repeat maps it to the
        // *same* sample radius — the self-similarity working perfectly reads as a
        // constant map, and a non-vacuity check written that way passes only when
        // the feature is broken.
        assert!(
            (map(cutoff * 1.5) - at_cutoff).abs() > 1e-3,
            "the map is constant outside the cutoff too — the repeat is not running"
        );

        // And a cutoff of 0 leaves the repeat running to the axis (the default).
        // Same trap, same reason for 0.3 rather than 0.25.
        let uncut = |r: f32| repeat_sample_radius(r, R_MAX, l, 0.0, 0.0, 0.0, 0.4);
        assert!(
            (uncut(cutoff * 0.3) - uncut(cutoff)).abs() > 1e-3,
            "with kaleido_inner = 0 the interior is still frozen"
        );
    }

    /// `kaleido_zoom` reaches the shader reduced into **one ring** — `[0, 1)` —
    /// and the reduction is exact enough to be invisible: it is precision hygiene
    /// for a `time`-driven binding, not a change to the map.
    ///
    /// The unit is what changed at Plan 0064 Phase 4, and the reduction got
    /// cheaper for it: reducing in rings happens *before* the multiply by `L`, so
    /// the truncation is against a period of exactly 1 rather than against
    /// whatever `ln(ratio)` came out to.
    #[test]
    fn the_zoom_offset_is_reduced_into_one_ring() {
        let l = fold_radial(1.3);
        for raw in [0.0f32, 0.1, 1.0, 7.5, 1000.0, -3.25] {
            let z = fold_zoom(raw);
            assert!(
                (0.0..1.0).contains(&z),
                "fold_zoom({raw}) = {z} is outside [0, 1)"
            );
            // Same map, whatever whole number of rings was handed in. The
            // tolerance is looser than [`MAP_REL_TOL`] on purpose and in the
            // direction the reduction exists to fix: at raw = 1000 an f32's ulp is
            // 6e-5, a fifteenth of a per-mille of a ring, so the *unreduced* side
            // is the imprecise one. That drift is the whole argument for reducing,
            // and asserting it away would delete the motive.
            assert_close_tol(
                repeat_sample_radius(0.2, R_MAX, l, 0.0, raw, 0.0, 0.3),
                repeat_sample_radius(0.2, R_MAX, l, 0.0, z, 0.0, 0.3),
                2e-4,
                &format!("reducing zoom {raw} changed the map"),
            );
        }
        // The reduction does not consult the ring ratio at all — the shader owns
        // the multiply — so a whole number of rings reduces to nothing whatever
        // the repeat is set to, which is exactly what makes 1.0 the identity
        // offset at every ratio.
        assert_eq!(fold_zoom(3.0), 0.0);
        assert_eq!(fold_zoom(-2.0), 0.0);
        assert_eq!(fold_zoom(f32::NAN), DEFAULT_ZOOM);
        assert_eq!(fold_zoom(f32::INFINITY), DEFAULT_ZOOM);
    }

    /// The wallpaper tile is off at one cell and capped above, and — unlike the
    /// fold order and the winding number — it is deliberately **not** rounded.
    #[test]
    fn fold_tile_is_off_at_one_cell_and_is_not_stepped() {
        assert_eq!(fold_tile(1.0), DEFAULT_TILE);
        assert_eq!(fold_tile(0.2), DEFAULT_TILE);
        assert_eq!(fold_tile(f32::NAN), DEFAULT_TILE);
        assert_eq!(fold_tile(1e9), MAX_TILE);
        // A fractional cell count is a perfectly good mirrored grid whose last
        // cell is cut off at the frame edge, so an eased binding travels rather
        // than snapping. This is the one param on this stage where that is true.
        assert_eq!(fold_tile(2.5), 2.5);
    }

    /// The inner cutoff is a fraction of `r_max`, clamped, with a broken binding
    /// falling back to the **default** — which since Plan 0064 Phase 4 is 0.06 and
    /// not the identity, so "clamped" and "defaulted" are two different answers
    /// here the way they already are for `kaleido_edge`.
    #[test]
    fn fold_inner_is_a_clamped_fraction_of_r_max() {
        // An explicit 0 is honoured exactly: the default is a resting value, not a
        // floor. A preset that wants the repeat running to the fold axis says so
        // and gets it.
        assert_eq!(fold_inner(0.0), 0.0);
        assert_eq!(fold_inner(0.35), 0.35);
        assert_eq!(fold_inner(-1.0), 0.0);
        assert_eq!(fold_inner(4.0), MAX_INNER);
        assert_eq!(fold_inner(f32::NAN), DEFAULT_INNER);
        assert_eq!(fold_inner(f32::INFINITY), DEFAULT_INNER);

        // ...and the default is the non-identity Phase 4 chose. Asserted rather
        // than assumed, because every other term on this stage rests at its
        // identity and a future tidy-up would "fix" this one by reflex. A `const`
        // block, so it fails at compile time rather than at run time — the claim is
        // about a constant and nothing at run time can change the answer.
        const {
            assert!(
                DEFAULT_INNER > 0.0,
                "kaleido_inner's default is back at the identity — that is the one \
                 setting the repeat is guaranteed to alias at (module docs)"
            );
        }
        assert_ne!(
            fold_inner(0.0),
            DEFAULT_INNER,
            "clamping and the non-finite fallback must stay distinguishable"
        );
    }
}
