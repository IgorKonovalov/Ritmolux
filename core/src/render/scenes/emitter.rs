//! Emitter scene: objects that **spawn**, follow an **analytic** ballistic path,
//! age, and are **retired** — the first scene in the engine whose population is
//! not fixed (ADR-0057).
//!
//! It exists beside the swarm rather than inside it. The swarm's world is a
//! **torus** (ADR-0044): `bounds(aspect)` wraps every particle back into frame,
//! deliberately, so the field stays populated with no respawn hitches. A cascade
//! is the opposite requirement — a thing that falls out of shot and does not come
//! back — so the two worlds cannot share one scene without a mode switch that
//! changes the world topology.
//!
//! # Position is a closed form, not an accumulator
//!
//! Each object stores its spawn time, spawn position, launch velocity and the
//! gravity it was launched under; its position at scene time `t` is
//!
//! ```text
//! p(t) = p0 + v0 * (t - t0) + 0.5 * a * (t - t0)^2
//! ```
//!
//! There is **no `dt` in the position at all**, so the trajectory is exactly
//! frame-rate independent by construction rather than by tuning — the class of
//! divergence Plan 0014 removed when it retired `SCENE_DT` cannot reappear here.
//! It also makes the arithmetic checkable: an object launched with vertical speed
//! `v0` against gravity `g` reaches its apex at `t = v0 / g` and at height
//! `v0^2 / (2 g)`, on any cadence. See
//! [`an_object_follows_the_closed_form_parabola`](tests::an_object_follows_the_closed_form_parabola).
//!
//! **Retirement is a closed form too, and that is not decoration.** Sampling "is
//! this object outside the frame?" once per frame would make the *population* a
//! function of where the frames happened to land — an object that arcs above the
//! top bound and falls back would be culled by a cadence that sampled while it was
//! out and survive one that did not. So each object's death time is solved at
//! spawn: the earliest of its lifetime, the time it leaves through a side (linear
//! in `t`), and the last time it is above the bottom bound (the larger root of the
//! quadratic). Retirement is then `time >= death_time`, monotone in scene time,
//! and the whole scene is a pure function of `(seed, scene time)`.
//!
//! # The pool is fixed and spawning is clamped to it
//!
//! Spawn/die is the one place this scene could allocate on the hot path, so it
//! cannot: the object array and its free list are sized once from
//! [`TierConfig::emitter_objects`](crate::render::TierConfig::emitter_objects) and
//! never grow. When the pool is full a spawn is **dropped** — not queued, not
//! allocated for — and the spawn loop is capped at one pool's worth of spawns per
//! frame so an absurd `spawn_rate` costs bounded work rather than a stall.
//!
//! Objects draw through the swarm's sprite idiom — `vec4(colour * g, g)` on
//! [`gpu::ADDITIVE_LIGHT_SATURATING_COVERAGE`] — so this is the **third** pipeline
//! that writes directly into the post chain's input and it owes the third
//! lit-backdrop guard (ADR-0056).

// Hot-path panic-denial pragma (Plan 0002 Phase 2, extended to scenes by Plan
// 0003 Phase 0). Runs every displayed frame.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use super::marks;
use super::{Scene, SeededRng};
use crate::dsp::AnalysisFrame;
use crate::render::gpu;
use crate::render::palette::{self, Palette};

/// The scene's spawn seed — the only randomness it has, and it is explicit
/// (NFR §6). "LMV_EMIT".
const SEED: u64 = 0x4C4D_565F_454D_4954;

/// Domain aspect before the first [`Scene::render`] hands one over. Only reached
/// on the very first `update` of a fresh scene.
const FALLBACK_ASPECT: f32 = 16.0 / 9.0;

/// How far past the visible frame an object may travel before it is gone for
/// good, as a multiple of the frame's half-extents.
///
/// The visible frame is `|world.y| <= 1` by `|world.x| <= aspect` (the shader
/// divides x by the aspect on its way to NDC), so this is the visible rectangle
/// scaled outward. It is deliberately generous: a sprite is retired when its
/// *centre* passes the bound, and a mark still half in frame that vanished would
/// read as a pop rather than an exit.
const RETIRE_MARGIN: f32 = 1.6;

/// The sprite's world half-size at `size = 1`, before the per-object size draw.
const BASE_SIZE: f32 = 0.019;

/// The mark's minor axis as a fraction of its major one — **this scene's `disc`**.
///
/// It exists because a perfect disc is rotationally symmetric, so rotating its
/// quad would change nothing at all and `spin` would join the list of parameters
/// that are documented and do nothing (the swarm's `hue` was one for four plans).
/// One constant on one radial falloff makes the mark a soft elongated *glint*
/// instead, which a rotation can be seen in.
///
/// Sized so the elongation reads at a few pixels across without the mark becoming
/// a streak: at 0.55 the long axis is not quite twice the short one.
///
/// **Plan 0070 answered the shape question this used to defer** (ADR-0084), and
/// deliberately left this arm alone: `shape = disc` on the emitter is *this*
/// figure, not a circle, so every existing preset and the golden baseline are
/// untouched. The roster's other four silhouettes are evaluated on the
/// un-squashed sprite frame, so a star is a star rather than a squashed one — and
/// `spin` turns it, which is what makes a shaped mark read as an object.
const GLINT_ANISO: f32 = 0.55;

/// The per-object twinkle rate, in Hz, at the two ends of the seeded draw.
///
/// **The spread across objects is the point, not the values.** A field of
/// oscillators that all share a rate flashes as one sheet however their phases
/// are scattered — the sum of N sinusoids at one frequency is a sinusoid at that
/// frequency. Drawing the *rate* per object as well is what makes the whole-frame
/// mean steady while every member of it is not, which is the property Phase 2's
/// last done-when asserts. The range is a little under two octaves: wide enough
/// to decorrelate, narrow enough that no object reads as either frozen or
/// strobing.
const TWINKLE_FREQ_LO: f32 = 0.35;
const TWINKLE_FREQ_HI: f32 = 1.6;

/// Fraction of an object's life spent fading in. Short — the mark should be lit
/// by the time it reaches the frame — but not zero, because a sprite switched on
/// at full brightness inside the frame is a pop.
const ATTACK_FRAC: f32 = 0.08;

/// Hard ceiling on `spawn_rate`, in objects per second.
///
/// Not a look value: it bounds the per-frame spawn loop's *arithmetic* alongside
/// the pool cap that bounds its *effect*. A preset binding `spawn_rate` to an
/// unclamped expression is the realistic way this is reached, and the answer is a
/// saturated pool, not a stall.
const MAX_SPAWN_RATE: f32 = 20_000.0;

/// Bounds on `lifetime`, in seconds. The lower bound keeps `age / lifetime`
/// finite; the upper one keeps an object launched into a gravity-free sky from
/// occupying a pool slot forever.
const MIN_LIFETIME: f32 = 0.05;
const MAX_LIFETIME: f32 = 60.0;

// Parameter defaults — an unbound emitter is a calm upward shower.
const DEFAULT_SPAWN_RATE: f32 = 120.0;
const DEFAULT_GRAVITY: f32 = 1.5;
const DEFAULT_LAUNCH_SPEED: f32 = 1.75;
const DEFAULT_LAUNCH_ANGLE: f32 = 0.0;
const DEFAULT_LIFETIME: f32 = 3.0;
const DEFAULT_SIZE: f32 = 1.0;
const DEFAULT_BRIGHTNESS: f32 = 1.0;
// The distribution params (Phase 2). Each says how *wide* a per-object draw is;
// the seed picks within it. `spread` and the two `*_spread` widths default
// non-zero because a population with no variation is the defect this phase
// exists to fix — a shower launched on one angle is a column, not a shower.
// `spin` and `twinkle` default off: both are motion a preset asks for.
/// Full width of the launch-angle cone, radians (~31 degrees).
const DEFAULT_SPREAD: f32 = 0.55;
const DEFAULT_SIZE_SPREAD: f32 = 0.6;
const DEFAULT_LIFETIME_SPREAD: f32 = 0.45;
const DEFAULT_SPIN: f32 = 0.0;
const DEFAULT_TWINKLE: f32 = 0.0;
// The source geometry (Plan 0090). Both defaults are the geometry this scene
// shipped with, stated as values rather than as constants at the spawn site
// (ADR-0104).
/// Where the source line sits, in world units. Just **below** the visible frame,
/// so an upward-launched object rises into shot rather than appearing in it.
///
/// A preset may move it, **including inside the frame** — that is the only route
/// to a slow look the behavioral gates can see, and the object then switches on
/// where the eye is unless the preset also asks for a `spawn_fade`. It is still
/// clamped to the retirement bound, by correctness rather than by taste: a source
/// outside it spawns objects whose exit time has already passed.
const DEFAULT_SOURCE_Y: f32 = -1.12;
/// The source line's half-width **as a fraction of the frame's**, so the default
/// resolves to `aspect * 1.0` — bit for bit the full-frame line this scene has
/// always drawn. `0` collapses the line to a point source.
const DEFAULT_SOURCE_WIDTH: f32 = 1.0;
/// Fraction of an object's life over which its brightness ramps up from zero.
/// Off by default, which is exactly today: an object arrives at the brightness
/// [`ATTACK_FRAC`] gives it. It is the answer to an inside-frame `source_y`,
/// where a mark switched on at full brightness is a pop.
const DEFAULT_SPAWN_FADE: f32 = 0.0;
/// Lifetimes of spawns to back-date at scene start. Off by default, because a
/// prewarmed world is *full* on its first frame — right for a sky, wrong for a
/// cascade, and the two readings live one number apart.
const DEFAULT_PREWARM: f32 = 0.0;

/// Ceiling on `prewarm`, in lifetimes. Past one nothing new survives to be
/// added — an object older than its own life is dead by definition — and the
/// widest `lifetime_spread` stretches that to one and a half, so two is already
/// generous. It bounds the back-dated spawn loop's arithmetic the way
/// [`MAX_SPAWN_RATE`] bounds the live one's.
const MAX_PREWARM: f32 = 2.0;
// Shared palette colour knobs (ADR-0021), same meaning as the swarm's.
const DEFAULT_HUE: f32 = 0.0;
const DEFAULT_HUE_SPREAD: f32 = 1.0;
const DEFAULT_HUE_CENTER: f32 = 0.5;
const DEFAULT_SATURATION: f32 = 1.0;
const DEFAULT_PALETTE_MIX: f32 = 0.0;
// Shared view transform (ADR-0018): identity by default.
const DEFAULT_ZOOM: f32 = 1.0;
const DEFAULT_PAN: f32 = 0.0;
// The shared mark silhouette (ADR-0084). `disc` is this scene's glint, exactly
// as it was, so an unbound emitter is unchanged.
const DEFAULT_SHAPE: f32 = marks::DEFAULT_SHAPE;
const DEFAULT_POINTS: f32 = marks::DEFAULT_POINTS;
/// The `star` arm's three shape params (Plan 0091 Phase 5), aliased beside the
/// other two mark defaults so this scene states its whole vocabulary locally.
const DEFAULT_STAR_VALLEY: f32 = marks::DEFAULT_STAR_VALLEY;
const DEFAULT_STAR_CURVE: f32 = marks::DEFAULT_STAR_CURVE;
const DEFAULT_STAR_JITTER: f32 = marks::DEFAULT_STAR_JITTER;

/// The WGSL, with `%ANISO%` substituted from [`GLINT_ANISO`] at module creation
/// so the elongation exists in exactly one place — a second copy in the shader
/// string is a constant that drifts silently the first time the Rust one moves.
/// The shared mark-silhouette chunk ([`marks::sdf_wgsl`]) is prepended, so
/// `mark_distance` here is the same function the swarm evaluates.
///
/// `shape` / `points` travel vertex -> fragment as flat varyings, as they do on
/// the swarm — see that scene's shader comment for why a per-draw value goes
/// through the varyings rather than through a wider bind-layout visibility.
const SHADER: &str = r#"
const ANISO: f32 = %ANISO%;

struct Misc {
    // x: aspect, y: zoom, zw: pan (the shared ViewTransform, ADR-0018)
    v: vec4<f32>,
    // x: mark shape index, y: quantized point count (ADR-0084). Per draw, not
    // per instance.
    m: vec4<f32>,
    // xyz: the star arm's shape params (valley, curve, jitter), conditioned
    // CPU-side (Plan 0091 Phase 5). Per draw, like `m`. Inert on every other
    // shape, and at their defaults the arm takes its original closed form.
    s: vec4<f32>,
}

@group(0) @binding(0) var<uniform> misc: Misc;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) local: vec2<f32>,
    @location(1) color: vec3<f32>,
    @location(2) @interpolate(flat) shape: f32,
    @location(3) @interpolate(flat) points: f32,
    @location(4) @interpolate(flat) star: vec3<f32>,
}

@vertex
fn vs_main(
    @builtin(vertex_index) vi: u32,
    @location(0) center: vec2<f32>,
    @location(1) size: f32,
    @location(2) color: vec3<f32>,
    @location(3) angle: f32,
) -> VsOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
    );
    let c = corners[vi] * 2.0 - vec2<f32>(1.0, 1.0);
    // The quad is rotated in world space and `local` is left un-rotated, so the
    // elongated falloff below is written in the sprite's own frame and turns
    // with it. `angle` is the CPU-resolved orientation: a seeded base plus
    // `spin` times age.
    let s = sin(angle);
    let k = cos(angle);
    let r = vec2<f32>(c.x * k - c.y * s, c.x * s + c.y * k);
    // Shared ViewTransform (ADR-0018): zoom about the frame centre, then pan;
    // the sprite quad (r * size) keeps its on-screen size.
    let zoom = misc.v.y;
    let pan = misc.v.zw;
    let world = center * zoom + pan + r * size;
    var out: VsOut;
    out.pos = vec4<f32>(world.x / misc.v.x, world.y, 0.0, 1.0);
    out.local = c;
    out.color = color;
    out.shape = misc.m.x;
    out.points = misc.m.y;
    out.star = misc.s.xyz;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // This scene's `disc` is the glint, not a circle: one radial falloff with one
    // axis scaled, which is what makes a rotation visible at all. Left exactly as
    // it was so every shipped emitter preset is untouched (ADR-0084). The
    // roster's other silhouettes read the un-squashed sprite frame, so a star is
    // a star rather than a squashed one.
    var d: f32;
    if (in.shape < 0.5) {
        d = length(vec2<f32>(in.local.x, in.local.y / ANISO));
    } else {
        d = mark_distance(in.local, in.shape, in.points, in.star);
    }
    let falloff = max(0.0, 1.0 - d);
    let g = falloff * falloff;
    // Premultiplied: colour AND alpha carry the same coverage `g`, so the four
    // corners outside the inscribed disc write nothing at all rather than
    // opaque black (ADR-0056). See `gpu::ADDITIVE_LIGHT_SATURATING_COVERAGE`.
    return vec4<f32>(in.color * g, g);
}
"#;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Instance {
    center: [f32; 2],
    size: f32,
    color: [f32; 3],
    /// The sprite's orientation in radians, resolved on the CPU from the
    /// object's seeded base angle and `spin` times its age (Plan 0052 Phase 2),
    /// so the shader needs no per-object state and no spin constants.
    angle: f32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Misc {
    v: [f32; 4],
    /// `[shape, points, 0, 0]` — the mark silhouette, quantized on the way in
    /// (ADR-0084), padded to a second `vec4` by WGSL's uniform layout rule.
    m: [f32; 4],
    /// `[star_valley, star_curve, star_jitter, 0]` — the star arm's three shape
    /// params, conditioned on the way in (Plan 0091 Phase 5). A third `vec4`
    /// rather than a wider `m` because WGSL's uniform layout rule packs in
    /// 16-byte rows; the layout SHAPE is unchanged, so ADR-0058 is untouched.
    s: [f32; 4],
}

/// One thrown object. Everything here is fixed at spawn — the path is decided
/// once and never re-steered (ADR-0057: no drag, no flow field, no collision).
#[derive(Clone, Copy, Debug)]
struct Object {
    /// Spawn position, world units.
    p0: [f32; 2],
    /// Launch velocity, world units per second.
    v0: [f32; 2],
    /// Scene time at spawn.
    t0: f32,
    /// Seconds this object lives, after the per-object draw.
    lifetime: f32,
    /// The gravity it was launched under, **carried per object** rather than read
    /// from the scene each frame. `gravity` is bindable, so a scene-level read
    /// would teleport every object in flight the moment a preset moved it; the
    /// path is fixed at spawn, so the acceleration is part of the path.
    gravity: f32,
    /// Scene time at which this object is retired: the earliest of its lifetime
    /// and the moment it leaves the bound for good, solved at spawn so
    /// retirement is monotone in scene time (see the module docs).
    death_time: f32,
    /// Drawn once at spawn. **Every** individuating quantity is a pure function
    /// of this and a preset distribution param (ADR-0057).
    seed: u32,
    /// Whether this slot holds a live object. The free list holds the rest.
    alive: bool,
}

impl Object {
    /// A dead slot — what the pool is filled with at construction.
    const DEAD: Self = Self {
        p0: [0.0, 0.0],
        v0: [0.0, 0.0],
        t0: 0.0,
        lifetime: 1.0,
        gravity: 0.0,
        death_time: 0.0,
        seed: 0,
        alive: false,
    };

    /// Position at scene time `time` — the closed form, and the whole of this
    /// scene's motion.
    fn position(&self, time: f32) -> [f32; 2] {
        let age = time - self.t0;
        [
            self.p0[0] + self.v0[0] * age,
            self.p0[1] + self.v0[1] * age - 0.5 * self.gravity * age * age,
        ]
    }
}

/// The per-frame spawn configuration, resolved from the bound params once per
/// frame and validated there (validate at the boundary, trust inside).
#[derive(Clone, Copy, Debug)]
struct Spawn {
    /// Objects per second, in `0..=`[`MAX_SPAWN_RATE`].
    rate: f32,
    gravity: f32,
    speed: f32,
    /// Radians clockwise from straight up.
    angle: f32,
    /// Full width of the launch-angle cone, radians.
    spread: f32,
    /// Seconds, in [`MIN_LIFETIME`]`..=`[`MAX_LIFETIME`].
    lifetime: f32,
    /// Fractional width of the per-object lifetime draw (`0` = every object
    /// lives exactly `lifetime`).
    lifetime_spread: f32,
    /// Half-extent of the source line, world units: `aspect * source_width`,
    /// clamped to the retirement bound. `0` is a point source.
    source_half_width: f32,
    /// The source line's world `y`, clamped to the retirement bound.
    source_y: f32,
    /// Lifetimes of spawns to back-date at scene start, in `0..=`[`MAX_PREWARM`].
    /// Read once, on the field's first [`step`](Field::step), and never again.
    prewarm: f32,
    /// Half-extents of the retirement bound, world units.
    bound: [f32; 2],
}

/// The fixed-capacity object pool: spawn, retire, and nothing else. **GPU-free
/// on purpose** — the properties ADR-0057 claims (the closed-form path, cadence
/// independence, that objects genuinely leave, that the pool cannot be overrun)
/// are properties of this struct, so they are asserted against it directly
/// rather than inferred from pixels.
struct Field {
    /// Fixed length: one slot per unit of pool capacity. Never resized.
    objects: Vec<Object>,
    /// Indices of the dead slots. Never grows past the pool.
    free: Vec<u32>,
    live: usize,
    rng: SeededRng,
    /// Scene time of the **next** spawn instant. Advanced by the spawn period,
    /// not by `dt`, so the sequence of `t0` values a run produces does not depend
    /// on where its frames landed.
    next_spawn: f32,
    /// Whether [`step`](Self::step) has run once, so `next_spawn` can be seeded
    /// from the first scene time this field ever sees rather than from 0 (a
    /// mid-session tier change rebuilds scenes at a non-zero clock).
    started: bool,
}

impl Field {
    fn new(capacity: usize) -> Self {
        let mut free = Vec::with_capacity(capacity);
        // Highest index first, so the pool fills from slot 0 upward — the draw
        // order is then stable and readable rather than reversed.
        for i in (0..capacity).rev() {
            free.push(i as u32);
        }
        Self {
            objects: vec![Object::DEAD; capacity],
            free,
            live: 0,
            rng: SeededRng::new(SEED),
            next_spawn: 0.0,
            started: false,
        }
    }

    fn capacity(&self) -> usize {
        self.objects.len()
    }

    /// Retire everything whose death time has passed, then spawn everything due
    /// at or before `time`.
    fn step(&mut self, time: f32, cfg: &Spawn) {
        if !self.started {
            self.started = true;
            self.next_spawn = time;
            self.prewarm(time, cfg);
        }
        self.retire(time);
        self.spawn_due(time, cfg);
    }

    /// Fill the pool as if this field had already been running for
    /// `prewarm * lifetime` seconds, so the population **begins** at its steady
    /// state instead of ramping toward it over a lifetime (ADR-0104).
    ///
    /// This is the second warm-up, and moving the source does not touch it:
    /// wherever the source sits, the population climbs toward `rate * lifetime`
    /// at `rate` a second, and a behavioral gate captures 30 frames — half a
    /// second. A world whose lifetime is measured in seconds is therefore
    /// scored on a fraction of the picture it is actually about.
    ///
    /// **Back-dating is exact, not approximated**, and that is a property of the
    /// scene rather than of this function: a path is closed-form in `t - t0` and
    /// a death time is derived from `t0`, so an object built with a back-dated
    /// `t0` is indistinguishable from one that genuinely spawned then. The RNG
    /// advances once per back-dated spawn exactly as a real run's would, so the
    /// seeds match too, and nothing here reads a clock (NFR §6).
    ///
    /// Two bounds keep the work finite and the pool holding the right end of
    /// the history. An object spawned more than a longest-possible life ago
    /// cannot still be alive, so the window is clipped there rather than at
    /// whatever `prewarm` asked for; and a back-dated object whose life has
    /// already ended is **dropped rather than stored**, because it is invisible
    /// either way (its envelope is zero) but a stored one would hold a slot the
    /// live object behind it needs. A world whose steady state exceeds the pool
    /// starts full of the oldest survivors, which is a saturated pool either
    /// way.
    fn prewarm(&mut self, time: f32, cfg: &Spawn) {
        if cfg.rate <= 0.0 || cfg.prewarm <= 0.0 {
            return;
        }
        let longest_life = cfg.lifetime * (1.0 + cfg.lifetime_spread * 0.5);
        let seconds = (cfg.prewarm * cfg.lifetime).min(longest_life);
        let period = 1.0 / cfg.rate;
        let cap = self.capacity();
        let mut t0 = time - seconds;
        let mut spawned = 0usize;
        while t0 <= time && spawned < cap {
            let seed = (self.rng.next_u64() >> 32) as u32;
            let object = build(seed, t0, cfg);
            if object.death_time > time
                && let Some(index) = self.free.pop()
                && let Some(slot) = self.objects.get_mut(index as usize)
            {
                *slot = object;
                self.live += 1;
            }
            t0 += period;
            spawned += 1;
        }
        // The live schedule resumes where the back-dated one left off, so the
        // seam is a spawn instant like any other rather than a gap or a burst.
        self.next_spawn = t0;
    }

    fn retire(&mut self, time: f32) {
        for (index, object) in self.objects.iter_mut().enumerate() {
            if object.alive && time >= object.death_time {
                object.alive = false;
                self.free.push(index as u32);
                self.live -= 1;
            }
        }
    }

    fn spawn_due(&mut self, time: f32, cfg: &Spawn) {
        if cfg.rate <= 0.0 {
            // No backlog accrues while the source is off: an emitter switched on
            // after ten silent seconds must not fire ten seconds of sparks.
            self.next_spawn = time;
            return;
        }
        let period = 1.0 / cfg.rate;
        // Capped at one pool's worth per frame. Past that the pool is full by
        // definition, so further spawns are drops — and a drop still costs the
        // loop an iteration, which is what this bounds.
        let cap = self.capacity();
        let mut spawned = 0usize;
        while self.next_spawn <= time && spawned < cap {
            let t0 = self.next_spawn;
            self.spawn_at(t0, cfg);
            self.next_spawn += period;
            spawned += 1;
        }
        if self.next_spawn < time {
            self.next_spawn = time;
        }
    }

    /// Spawn one object at scene time `t0`, **or drop it** when the pool is full.
    ///
    /// The RNG advances either way, so the seed sequence is a function of the
    /// spawn schedule alone and not of how full the pool happened to be.
    fn spawn_at(&mut self, t0: f32, cfg: &Spawn) {
        let seed = (self.rng.next_u64() >> 32) as u32;
        let Some(index) = self.free.pop() else {
            return;
        };
        let object = build(seed, t0, cfg);
        if let Some(slot) = self.objects.get_mut(index as usize) {
            *slot = object;
            self.live += 1;
        }
    }
}

/// A uniform in `[0, 1)` derived from an object's seed and a channel index `k`.
///
/// The individuation contract in one function: a per-object quantity is a pure
/// function of `(seed, k)`, so it is stable for the object's whole life, needs no
/// per-object state, and costs no RNG draw beyond the single one taken at spawn.
fn unit(seed: u32, k: u32) -> f32 {
    // splitmix64's finalizer over the (seed, channel) pair — the same mixer
    // `SeededRng` uses, applied as a hash rather than as a stream.
    let mut z = ((seed as u64) << 32 | k as u64).wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 40) as f32 / (1u64 << 24) as f32
}

/// Seed channels. Named so a later quantity cannot silently reuse one and
/// correlate itself with an existing draw.
mod channel {
    pub(super) const SOURCE_X: u32 = 0;
    pub(super) const ANGLE: u32 = 1;
    pub(super) const SIZE: u32 = 2;
    pub(super) const LIFETIME: u32 = 3;
    pub(super) const ORIENT: u32 = 4;
    pub(super) const SPIN: u32 = 5;
    pub(super) const TWINKLE_FREQ: u32 = 6;
    pub(super) const TWINKLE_PHASE: u32 = 7;
    pub(super) const HUE: u32 = 8;
}

/// Build the object a `seed` spawned at `t0` under `cfg` describes.
///
/// Free-standing and pure, so the closed-form path and the death-time solve can
/// be exercised without a pool around them.
fn build(seed: u32, t0: f32, cfg: &Spawn) -> Object {
    let angle = cfg.angle + (unit(seed, channel::ANGLE) - 0.5) * cfg.spread;
    let lifetime = (cfg.lifetime
        * (1.0 + (unit(seed, channel::LIFETIME) - 0.5) * cfg.lifetime_spread))
        .clamp(MIN_LIFETIME, MAX_LIFETIME);
    let p0 = [
        (unit(seed, channel::SOURCE_X) * 2.0 - 1.0) * cfg.source_half_width,
        cfg.source_y,
    ];
    // Angle is measured clockwise from straight up, so zero launches along +y.
    let v0 = [angle.sin() * cfg.speed, angle.cos() * cfg.speed];
    let exit = exit_time(p0, v0, cfg.gravity, cfg.bound);
    Object {
        p0,
        v0,
        t0,
        lifetime,
        gravity: cfg.gravity,
        death_time: t0 + lifetime.min(exit),
        seed,
        alive: true,
    }
}

/// **When this path leaves the bound for good** — elapsed seconds from spawn, or
/// [`f32::INFINITY`] if it never does.
///
/// Solved rather than sampled. The horizontal component is linear in `t` (gravity
/// is vertical), so a side exit is one division and is permanent — `v0.x` never
/// changes. The vertical one is the *larger* root of `p0.y + v0.y t - g t^2 / 2 =
/// -bound.y`: the object may arc above the top bound and fall back, so only the
/// bottom is an exit at all, and only its last crossing counts.
///
/// A non-positive gravity is the case with no bottom exit: the path either rises
/// forever (`g < 0`, an upward accelerator) or is a straight line, and a straight
/// line only leaves downward when it is already heading that way. Both are then
/// bounded by lifetime alone, which is why lifetime has a ceiling.
fn exit_time(p0: [f32; 2], v0: [f32; 2], gravity: f32, bound: [f32; 2]) -> f32 {
    let (bx, by) = (bound[0], bound[1]);
    let side = if v0[0] > 0.0 {
        (bx - p0[0]) / v0[0]
    } else if v0[0] < 0.0 {
        (-bx - p0[0]) / v0[0]
    } else {
        f32::INFINITY
    };
    let below = if gravity > 0.0 {
        let disc = v0[1] * v0[1] + 2.0 * gravity * (p0[1] + by);
        if disc < 0.0 {
            // Already below the bound with too little upward speed to return.
            0.0
        } else {
            (v0[1] + disc.sqrt()) / gravity
        }
    } else if gravity == 0.0 && v0[1] < 0.0 {
        (p0[1] + by) / -v0[1]
    } else {
        f32::INFINITY
    };
    side.max(0.0).min(below.max(0.0))
}

/// The retirement bound for a render target of this aspect: the visible frame
/// scaled by [`RETIRE_MARGIN`].
///
/// The **render target's** aspect, never an internal grid's (ADR-0037) — this is
/// screen-destined geometry, and the quantized post-chain grid is a resolution,
/// not a shape.
fn bounds(aspect: f32) -> [f32; 2] {
    [aspect * RETIRE_MARGIN, RETIRE_MARGIN]
}

/// The source line's half-extent on a target of this aspect, from `source_width`
/// **as the preset bound it** (ADR-0104).
///
/// Fractional rather than absolute, which is what makes the default an exact
/// identity: `aspect * 1.0` is bit for bit `aspect`, the full-frame line this
/// scene has always drawn, so nothing shipped moves on the way in. It is also
/// where the aspect belongs (ADR-0037) — an absolute width would be a different
/// fraction of the frame on every display and would hand the author the
/// reconciliation.
///
/// Clamped as a magnitude, the way `lifetime_spread` is, and at the retirement
/// margin: a line wider than the bound puts its ends where the side exit has
/// already happened, which is a pool churning against itself rather than
/// anything visible.
fn source_half_width(aspect: f32, source_width: f32) -> f32 {
    aspect * finite(source_width, DEFAULT_SOURCE_WIDTH).clamp(0.0, RETIRE_MARGIN)
}

/// The source line's world `y`, from `source_y` as the preset bound it.
///
/// Clamped to the retirement bound and deliberately **not** to the visible frame:
/// a source inside the frame is legal (ADR-0104) and is the only route to a look
/// slow enough to read as a sky, at the price of a spawn pop that `spawn_fade`
/// is there to answer. Outside the *bound* is a different matter and is a
/// correctness clamp: an object spawned there is born with its exit time already
/// past.
fn source_line_y(source_y: f32, bound: [f32; 2]) -> f32 {
    finite(source_y, DEFAULT_SOURCE_Y).clamp(-bound[1], bound[1])
}

/// The LUT sample coordinate for one object (ADR-0021), identical in meaning to
/// the swarm's: the per-object hue occupies the band
/// `hue_center + (object_hue - 0.5) * hue_spread`, plus the shared rotation.
fn hue_coord(hue_center: f32, hue_spread: f32, object_hue: f32, hue: f32) -> f32 {
    hue_center + (object_hue - 0.5) * hue_spread + hue
}

/// The age envelope: a short fade in, then a fade toward the end of life.
///
/// An object that vanished at full brightness would pop; this is what makes a
/// retirement read as a spark burning out. `u` is `age / lifetime`.
fn envelope(u: f32) -> f32 {
    let attack = (u / ATTACK_FRAC).clamp(0.0, 1.0);
    let remaining = (1.0 - u).clamp(0.0, 1.0);
    attack * remaining.sqrt()
}

/// **The spawn ramp** (Plan 0090 Phase 2): a second, preset-owned fade-in over
/// the first `spawn_fade` of an object's life, multiplying [`envelope`]'s own
/// short attack rather than replacing it. `u` is `age / lifetime`.
///
/// It exists for the source that sits *inside* the frame (ADR-0104), where the
/// engine's 8 % attack is far too short to hide a mark switching on where the
/// eye already is. It is also a soft spark on its own terms — a ramp this scene
/// could not express at any `brightness`.
///
/// **Exactly `1.0` when the fade is off, by an equality branch and not by
/// arithmetic.** The natural form divides by the fade and is `0/0` at age zero,
/// and the house precedent is that the obviously-equivalent arithmetic is not
/// bit-exact: ADR-0092's `ink_gamma` and ADR-0094's ramp exponent both take the
/// same branch. That exactness is what keeps the default free of the one
/// committed emitter baseline.
fn spawn_ramp(u: f32, spawn_fade: f32) -> f32 {
    if spawn_fade <= 0.0 {
        return 1.0;
    }
    (u / spawn_fade).clamp(0.0, 1.0)
}

/// **The per-object brightness multiplier** — the answer to the report this plan
/// came from (ADR-0057 Notes: the user asked for stars that *blink* and got a
/// field-wide flash, because a binding is evaluated once per frame for the whole
/// scene).
///
/// Both the rate and the phase come off the object's seed, so no two objects
/// share an oscillator and the field never flashes as one sheet. Exactly `1.0`
/// at `twinkle <= 0`, which is what makes the population-varies assertion
/// falsifiable in both directions.
///
/// Clamped at zero: `twinkle` is a preset expression and may exceed 1, and a
/// negative multiplier would subtract light rather than removing it.
fn twinkle_factor(seed: u32, time: f32, twinkle: f32) -> f32 {
    if twinkle <= 0.0 {
        return 1.0;
    }
    let freq =
        TWINKLE_FREQ_LO + unit(seed, channel::TWINKLE_FREQ) * (TWINKLE_FREQ_HI - TWINKLE_FREQ_LO);
    let phase = unit(seed, channel::TWINKLE_PHASE);
    let wave = (std::f32::consts::TAU * (freq * time + phase)).sin();
    (1.0 + twinkle * wave).max(0.0)
}

/// The sprite's orientation: a seeded base angle plus `spin` radians a second,
/// signed per object so the field turns both ways.
///
/// The base exists at `spin = 0` too — a population of identically-oriented
/// glints is the sheet this phase is about, and it costs nothing to scatter.
fn sprite_angle(seed: u32, age: f32, spin: f32) -> f32 {
    let base = unit(seed, channel::ORIENT) * std::f32::consts::TAU;
    let rate = (unit(seed, channel::SPIN) * 2.0 - 1.0) * spin;
    base + rate * age
}

/// The object's size multiplier within `size_spread`. `1.0` exactly at zero
/// spread.
fn size_factor(seed: u32, size_spread: f32) -> f32 {
    (1.0 + (unit(seed, channel::SIZE) - 0.5) * size_spread).max(0.0)
}

/// Parameter vocabulary — see [`fragment_field::PARAMS`](super::fragment_field::PARAMS).
/// **Keep in sync with `set_param` below.**
pub const PARAMS: &[&str] = &[
    "spawn_rate",
    "gravity",
    "launch_speed",
    "launch_angle",
    "spread",
    "lifetime",
    "lifetime_spread",
    // The source geometry, and the ramp that makes an inside-frame one usable
    // (Plan 0090).
    "source_y",
    "source_width",
    "spawn_fade",
    "prewarm",
    "size",
    "size_spread",
    "spin",
    "twinkle",
    "brightness",
    "hue",
    "hue_spread",
    "hue_center",
    "saturation",
    "palette_mix",
    "palette_steps",
    "palette_contour",
    "zoom",
    "pan_x",
    "pan_y",
    // The shared mark silhouette (ADR-0084) — the same two names the swarm
    // carries; `marks::PARAMS` is the single statement of the pair.
    "shape",
    "points",
    "star_valley",
    "star_curve",
    "star_jitter",
];

/// Objects that spawn, fall on a parabola, and die (ADR-0057).
pub struct EmitterScene {
    pipeline: wgpu::RenderPipeline,
    instances: wgpu::Buffer,
    uniforms: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    field: Field,
    instance_data: Vec<Instance>,
    /// How many of `instance_data`'s slots this frame's draw uses.
    draw_count: usize,
    /// Shared scene clock (seconds), set by the renderer each frame.
    time: f32,
    /// The **render target's** aspect, recorded by `render` for the next `update`
    /// to size the source line and the retirement bound from (ADR-0037). One
    /// frame behind by construction, which is harmless: it moves a bound that
    /// already sits well off-screen.
    aspect: f32,
    spawn_rate: f32,
    gravity: f32,
    launch_speed: f32,
    launch_angle: f32,
    spread: f32,
    lifetime: f32,
    lifetime_spread: f32,
    /// The source line's world `y` and its half-width as a fraction of the
    /// frame's, **as bound** (ADR-0104). Both are conditioned in
    /// [`spawn_config`](Self::spawn_config), which is the one place a binding's
    /// arbitrary arithmetic is allowed to reach the pool.
    source_y: f32,
    source_width: f32,
    /// Fraction of a life spent ramping up from black, **as bound**; conditioned
    /// at the draw site beside the other appearance params.
    spawn_fade: f32,
    /// Lifetimes of spawns to back-date at scene start, **as bound**. Read once,
    /// on the pool's first step — a preset easing it afterwards changes nothing,
    /// which is why it is the one param here that is not a per-frame quantity.
    prewarm: f32,
    size: f32,
    size_spread: f32,
    spin: f32,
    twinkle: f32,
    brightness: f32,
    /// The active baked palette (ADR-0021), sampled per object on the CPU.
    palette: Palette,
    hue: f32,
    hue_spread: f32,
    hue_center: f32,
    saturation: f32,
    palette_mix: f32,
    /// Hard palette bands and their contour (ADR-0078), raw as the preset
    /// bound them -- `palette::band_steps` / `band_contour` condition them on
    /// the way to the sample site.
    palette_steps: f32,
    palette_contour: f32,
    zoom: f32,
    pan_x: f32,
    pan_y: f32,
    /// The mark silhouette and its point count, **as bound** (ADR-0084). Both
    /// are quantized on the way to the uniform, not here, so a `[smoothing]`-eased
    /// binding still eases — it just steps at the midpoints
    /// (see [`marks::mark_points`]).
    shape: f32,
    points: f32,
    /// The `star` arm's three shape params, raw as the preset bound them
    /// (Plan 0091 Phase 5). `marks::star_*` condition them on the way to the
    /// uniform. Inert on every other silhouette, and nothing warns —
    /// `presets/README.md` carries that.
    star_valley: f32,
    star_curve: f32,
    star_jitter: f32,
}

impl EmitterScene {
    /// Build the pipeline, buffers and object pool on `device`. `capacity` is the
    /// active tier's
    /// [`emitter_objects`](crate::render::TierConfig::emitter_objects); it is
    /// fixed for the life of the scene, so the per-frame path never allocates,
    /// and a tier change rebuilds the scene rather than resizing it.
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        capacity: usize,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("emitter-shader"),
            source: wgpu::ShaderSource::Wgsl(
                // The shared silhouette chunk first, then this scene's own
                // source — one `mark_distance`, two scenes (ADR-0084).
                format!(
                    "{}{}",
                    marks::sdf_wgsl(),
                    SHADER.replace("%ANISO%", &format!("{GLINT_ANISO:?}"))
                )
                .into(),
            ),
        });
        let instances = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("emitter-instances"),
            size: (capacity * std::mem::size_of::<Instance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("emitter-misc"),
            size: std::mem::size_of::<Misc>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        // **This layout is deliberately not the swarm's, and that is load-bearing
        // on the software adapter** (design-backlog 0039, the surface Plan 0053
        // is about).
        //
        // Written first as the swarm's exactly — one `[Uniform]` entry, `VERTEX`
        // visibility, `min_binding_size: None` — which is a byte-identical
        // descriptor to the pipeline this scene sits beside. On DX12 WARP that
        // made the **swarm** read this scene's uniform: `golden` came back with
        // every other fixture at mean 0.0000 and `swarm` at **0.1803** with a
        // max outlier of **175**, and `sanity` gave the three swarm presets a
        // different set of numbers on each run (Storm 0.0000 then 0.1667 against
        // its documented 0.8407). Nothing about the swarm had changed; merely
        // *constructing* a seventh pipeline with the same layout shape was
        // enough. Hardware renders both correctly, which is exactly why this
        // could only be caught by looking — a bless here would have committed
        // garbage as the swarm's baseline (the failure mode ADR-0074 and Plan
        // 0053 exist for).
        //
        // Distinguishing the descriptor — a wider visibility mask and an
        // explicit `min_binding_size` — restored `swarm` to mean 0.0000 with a
        // zero outlier. The two changes are cheap and honest on their own terms
        // (the size *is* known; the mask is a superset, so it forbids nothing),
        // but the reason they are here is the collision. **Do not "tidy" this
        // back into the swarm's shape.**
        let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("emitter-bind-layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: std::num::NonZeroU64::new(std::mem::size_of::<Misc>() as u64),
                },
                count: None,
            }],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("emitter-bind-group"),
            layout: &bind_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniforms.as_entire_binding(),
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("emitter-pipeline-layout"),
            bind_group_layouts: &[Some(&bind_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("emitter-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Instance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x2,
                        1 => Float32,
                        2 => Float32x3,
                        3 => Float32,
                    ],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    // Additive light, saturating coverage (ADR-0056) — shared
                    // with the swarm and the line renderer so the three cannot
                    // drift.
                    blend: Some(gpu::ADDITIVE_LIGHT_SATURATING_COVERAGE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        Self {
            pipeline,
            instances,
            uniforms,
            bind_group,
            field: Field::new(capacity),
            instance_data: vec![
                Instance {
                    center: [0.0, 0.0],
                    size: 0.0,
                    color: [0.0, 0.0, 0.0],
                    angle: 0.0,
                };
                capacity
            ],
            draw_count: 0,
            time: 0.0,
            aspect: FALLBACK_ASPECT,
            spawn_rate: DEFAULT_SPAWN_RATE,
            gravity: DEFAULT_GRAVITY,
            launch_speed: DEFAULT_LAUNCH_SPEED,
            launch_angle: DEFAULT_LAUNCH_ANGLE,
            spread: DEFAULT_SPREAD,
            lifetime: DEFAULT_LIFETIME,
            lifetime_spread: DEFAULT_LIFETIME_SPREAD,
            source_y: DEFAULT_SOURCE_Y,
            source_width: DEFAULT_SOURCE_WIDTH,
            spawn_fade: DEFAULT_SPAWN_FADE,
            prewarm: DEFAULT_PREWARM,
            size: DEFAULT_SIZE,
            size_spread: DEFAULT_SIZE_SPREAD,
            spin: DEFAULT_SPIN,
            twinkle: DEFAULT_TWINKLE,
            brightness: DEFAULT_BRIGHTNESS,
            palette: Palette::default_spectrum(),
            hue: DEFAULT_HUE,
            hue_spread: DEFAULT_HUE_SPREAD,
            hue_center: DEFAULT_HUE_CENTER,
            saturation: DEFAULT_SATURATION,
            palette_mix: DEFAULT_PALETTE_MIX,
            palette_steps: palette::DEFAULT_PALETTE_STEPS,
            palette_contour: palette::DEFAULT_PALETTE_CONTOUR,
            zoom: DEFAULT_ZOOM,
            pan_x: DEFAULT_PAN,
            pan_y: DEFAULT_PAN,
            shape: DEFAULT_SHAPE,
            points: DEFAULT_POINTS,
            star_valley: DEFAULT_STAR_VALLEY,
            star_curve: DEFAULT_STAR_CURVE,
            star_jitter: DEFAULT_STAR_JITTER,
        }
    }

    /// This frame's spawn configuration — the one place the bound params are
    /// validated. A binding may produce anything at all (an expression is
    /// arbitrary arithmetic over the analysis frame), so every value that reaches
    /// the pool is clamped and de-NaN'd here and trusted below.
    fn spawn_config(&self) -> Spawn {
        let bound = bounds(self.aspect);
        Spawn {
            rate: finite(self.spawn_rate, DEFAULT_SPAWN_RATE).clamp(0.0, MAX_SPAWN_RATE),
            gravity: finite(self.gravity, DEFAULT_GRAVITY),
            speed: finite(self.launch_speed, DEFAULT_LAUNCH_SPEED),
            angle: finite(self.launch_angle, DEFAULT_LAUNCH_ANGLE),
            spread: finite(self.spread, DEFAULT_SPREAD),
            lifetime: finite(self.lifetime, DEFAULT_LIFETIME).clamp(MIN_LIFETIME, MAX_LIFETIME),
            // A width, so it is only meaningful as a magnitude; clamped at 1 so
            // a preset cannot draw a negative lifetime out of the distribution.
            lifetime_spread: finite(self.lifetime_spread, DEFAULT_LIFETIME_SPREAD).clamp(0.0, 1.0),
            source_half_width: source_half_width(self.aspect, self.source_width),
            source_y: source_line_y(self.source_y, bound),
            prewarm: finite(self.prewarm, DEFAULT_PREWARM).clamp(0.0, MAX_PREWARM),
            bound,
        }
    }
}

/// `value`, or `fallback` when a binding produced something that is not a
/// number. NaN would propagate into a death time and pin a pool slot forever.
fn finite(value: f32, fallback: f32) -> f32 {
    if value.is_finite() { value } else { fallback }
}

impl Scene for EmitterScene {
    fn name(&self) -> &'static str {
        "emitter"
    }

    fn set_time(&mut self, time: f32) {
        self.time = time;
    }

    fn set_palette(&mut self, palette: &Palette) {
        // CPU-sampled per object in `update`; a cheap array copy, off the hot
        // path (once per preset switch).
        self.palette = palette.clone();
    }

    fn reset_params(&mut self) {
        self.spawn_rate = DEFAULT_SPAWN_RATE;
        self.gravity = DEFAULT_GRAVITY;
        self.launch_speed = DEFAULT_LAUNCH_SPEED;
        self.launch_angle = DEFAULT_LAUNCH_ANGLE;
        self.spread = DEFAULT_SPREAD;
        self.lifetime = DEFAULT_LIFETIME;
        self.lifetime_spread = DEFAULT_LIFETIME_SPREAD;
        self.source_y = DEFAULT_SOURCE_Y;
        self.source_width = DEFAULT_SOURCE_WIDTH;
        self.spawn_fade = DEFAULT_SPAWN_FADE;
        self.prewarm = DEFAULT_PREWARM;
        self.size = DEFAULT_SIZE;
        self.size_spread = DEFAULT_SIZE_SPREAD;
        self.spin = DEFAULT_SPIN;
        self.twinkle = DEFAULT_TWINKLE;
        self.brightness = DEFAULT_BRIGHTNESS;
        self.hue = DEFAULT_HUE;
        self.hue_spread = DEFAULT_HUE_SPREAD;
        self.hue_center = DEFAULT_HUE_CENTER;
        self.saturation = DEFAULT_SATURATION;
        self.palette_mix = DEFAULT_PALETTE_MIX;
        self.palette_steps = palette::DEFAULT_PALETTE_STEPS;
        self.palette_contour = palette::DEFAULT_PALETTE_CONTOUR;
        self.zoom = DEFAULT_ZOOM;
        self.pan_x = DEFAULT_PAN;
        self.pan_y = DEFAULT_PAN;
        self.shape = DEFAULT_SHAPE;
        self.points = DEFAULT_POINTS;
        self.star_valley = DEFAULT_STAR_VALLEY;
        self.star_curve = DEFAULT_STAR_CURVE;
        self.star_jitter = DEFAULT_STAR_JITTER;
    }

    fn set_param(&mut self, name: &str, value: f32) {
        match name {
            "spawn_rate" => self.spawn_rate = value,
            "gravity" => self.gravity = value,
            "launch_speed" => self.launch_speed = value,
            "launch_angle" => self.launch_angle = value,
            "spread" => self.spread = value,
            "lifetime" => self.lifetime = value,
            "lifetime_spread" => self.lifetime_spread = value,
            "source_y" => self.source_y = value,
            "source_width" => self.source_width = value,
            "spawn_fade" => self.spawn_fade = value,
            "prewarm" => self.prewarm = value,
            "size" => self.size = value,
            "size_spread" => self.size_spread = value,
            "spin" => self.spin = value,
            "twinkle" => self.twinkle = value,
            "brightness" => self.brightness = value,
            "hue" => self.hue = value,
            "hue_spread" => self.hue_spread = value,
            "hue_center" => self.hue_center = value,
            "saturation" => self.saturation = value,
            "palette_mix" => self.palette_mix = value,
            "palette_steps" => self.palette_steps = value,
            "palette_contour" => self.palette_contour = value,
            "zoom" => self.zoom = value,
            "pan_x" => self.pan_x = value,
            "pan_y" => self.pan_y = value,
            "shape" => self.shape = value,
            "points" => self.points = value,
            "star_valley" => self.star_valley = value,
            "star_curve" => self.star_curve = value,
            "star_jitter" => self.star_jitter = value,
            _ => {}
        }
    }

    fn update(&mut self, _frame: &AnalysisFrame) {
        let cfg = self.spawn_config();
        let time = self.time;
        self.field.step(time, &cfg);

        let size = finite(self.size, DEFAULT_SIZE) * BASE_SIZE;
        let brightness = finite(self.brightness, DEFAULT_BRIGHTNESS);
        // The three appearance distributions, hoisted: read once, used for every
        // live object. Unlike `spread` and `lifetime_spread` these are resolved
        // at *draw* rather than at spawn, because they describe how an object
        // looks rather than where it goes — so a preset easing one of them moves
        // the whole population continuously instead of only the objects spawned
        // since the change.
        let size_spread = finite(self.size_spread, DEFAULT_SIZE_SPREAD).clamp(0.0, 2.0);
        let spin = finite(self.spin, DEFAULT_SPIN);
        let twinkle = finite(self.twinkle, DEFAULT_TWINKLE);
        // A fraction of a life, so past 1 there is no more life to ramp over.
        // Resolved here rather than at spawn for the same reason as the three
        // above: it says how an object *looks*, so easing it moves the whole
        // population and not only the marks thrown since the change.
        let spawn_fade = finite(self.spawn_fade, DEFAULT_SPAWN_FADE).clamp(0.0, 1.0);
        let mut count = 0usize;
        // One pass over the pool, writing the live objects into the front of the
        // instance buffer. Iterating dead slots costs a branch; compacting is
        // what keeps the draw proportional to the population rather than to the
        // pool.
        for object in self.field.objects.iter() {
            if !object.alive {
                continue;
            }
            let Some(slot) = self.instance_data.get_mut(count) else {
                break;
            };
            let pos = object.position(time);
            let age = time - object.t0;
            let u = age / object.lifetime;
            let coord = hue_coord(
                self.hue_center,
                self.hue_spread,
                unit(object.seed, channel::HUE),
                self.hue,
            );
            // Hard bands on the palette coordinate (ADR-0078), the canonical
            // `palette::band_coord` called rather than copied. `palette_steps <= 1`
            // returns it untouched, so an unbound preset is byte-unchanged.
            let base = palette::desaturate(
                self.palette.sample(
                    palette::band_coord(coord, self.palette_steps),
                    self.palette_mix,
                ),
                self.saturation,
            );
            let bright = brightness
                * envelope(u)
                * spawn_ramp(u, spawn_fade)
                * twinkle_factor(object.seed, time, twinkle);
            *slot = Instance {
                center: pos,
                size: size * size_factor(object.seed, size_spread),
                color: [base[0] * bright, base[1] * bright, base[2] * bright],
                angle: sprite_angle(object.seed, age, spin),
            };
            count += 1;
        }
        self.draw_count = count;
    }

    fn render(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        aspect: f32,
    ) {
        // The bound the *next* `update` retires against. This argument is the
        // render target's aspect — the only correct source for a shape
        // (ADR-0037).
        self.aspect = aspect.max(0.1);
        queue.write_buffer(
            &self.uniforms,
            0,
            bytemuck::bytes_of(&Misc {
                v: [self.aspect, self.zoom, self.pan_x, self.pan_y],
                // Quantized here, on the way into the uniform, so the shader's
                // precondition stays visible on the CPU side: no fractional
                // point count ever reaches an angular fold (ADR-0084).
                m: [
                    marks::mark_shape(self.shape),
                    marks::mark_points(self.points),
                    0.0,
                    0.0,
                ],
                s: [
                    marks::star_valley(self.star_valley),
                    marks::star_curve(self.star_curve),
                    marks::star_jitter(self.star_jitter),
                    0.0,
                ],
            }),
        );
        if let Some(live) = self.instance_data.get(..self.draw_count)
            && !live.is_empty()
        {
            queue.write_buffer(&self.instances, 0, bytemuck::cast_slice(live));
        }

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("emitter-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    // Load over the engine backdrop (ADR-0018).
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        if self.draw_count == 0 {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, self.instances.slice(..));
        pass.draw(0..6, 0..self.draw_count as u32);
    }
}

#[cfg(test)]
mod tests;
