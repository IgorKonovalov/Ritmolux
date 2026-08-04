//! Emitter scene: objects that **spawn**, follow an **analytic** ballistic path,
//! age, and are **retired** — the first scene in the engine whose population is
//! not fixed ([ADR-0057](../../../../docs/adrs/0057-emitter-scene-analytic-ballistics-seeded-individuation.md)).
//!
//! It exists beside the swarm rather than inside it. The swarm's world is a
//! **torus** ([ADR-0044](../../../../docs/adrs/0044-swarm-world-is-a-25d-torus-sized-from-the-target.md)):
//! `bounds(aspect)` wraps every particle back into frame, deliberately, so the
//! field stays populated with no respawn hitches. A cascade is the opposite
//! requirement — a thing that falls out of shot and does not come back — so the
//! two worlds cannot share one scene without a mode switch that changes the world
//! topology.
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
//! lit-backdrop guard
//! ([ADR-0056](../../../../docs/adrs/0056-additive-scenes-emit-premultiplied-alpha.md)).

// Hot-path panic-denial pragma (Plan 0002 Phase 2, extended to scenes by Plan
// 0003 Phase 0). Runs every displayed frame.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

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

/// Where the source line sits, in world units. Just **below** the visible frame,
/// so an upward-launched object rises into shot rather than appearing in it.
const SOURCE_Y: f32 = -1.12;

/// The sprite's world half-size at `size = 1`, before the per-object size draw.
const BASE_SIZE: f32 = 0.019;

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
// Shared palette colour knobs (ADR-0021), same meaning as the swarm's.
const DEFAULT_HUE: f32 = 0.0;
const DEFAULT_HUE_SPREAD: f32 = 1.0;
const DEFAULT_HUE_CENTER: f32 = 0.5;
const DEFAULT_SATURATION: f32 = 1.0;
const DEFAULT_PALETTE_MIX: f32 = 0.0;
// Shared view transform (ADR-0018): identity by default.
const DEFAULT_ZOOM: f32 = 1.0;
const DEFAULT_PAN: f32 = 0.0;

const SHADER: &str = r#"
struct Misc {
    // x: aspect, y: zoom, zw: pan (the shared ViewTransform, ADR-0018)
    v: vec4<f32>,
}

@group(0) @binding(0) var<uniform> misc: Misc;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) local: vec2<f32>,
    @location(1) color: vec3<f32>,
}

@vertex
fn vs_main(
    @builtin(vertex_index) vi: u32,
    @location(0) center: vec2<f32>,
    @location(1) size: f32,
    @location(2) color: vec3<f32>,
) -> VsOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
    );
    let c = corners[vi] * 2.0 - vec2<f32>(1.0, 1.0);
    // Shared ViewTransform (ADR-0018): zoom about the frame centre, then pan;
    // the sprite quad (c * size) keeps its on-screen size.
    let zoom = misc.v.y;
    let pan = misc.v.zw;
    let world = center * zoom + pan + c * size;
    var out: VsOut;
    out.pos = vec4<f32>(world.x / misc.v.x, world.y, 0.0, 1.0);
    out.local = c;
    out.color = color;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let d = length(in.local);
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
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Misc {
    v: [f32; 4],
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
    /// Half-extents of the source line and of the retirement bound, world units.
    source_half_width: f32,
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
        }
        self.retire(time);
        self.spawn_due(time, cfg);
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
    pub(super) const LIFETIME: u32 = 3;
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
        SOURCE_Y,
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

/// Parameter vocabulary — see [`fragment_field::PARAMS`](super::fragment_field::PARAMS).
/// **Keep in sync with `set_param` below.**
pub const PARAMS: &[&str] = &[
    "spawn_rate",
    "gravity",
    "launch_speed",
    "launch_angle",
    "lifetime",
    "size",
    "brightness",
    "hue",
    "hue_spread",
    "hue_center",
    "saturation",
    "palette_mix",
    "zoom",
    "pan_x",
    "pan_y",
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
    lifetime: f32,
    size: f32,
    brightness: f32,
    /// The active baked palette (ADR-0021), sampled per object on the CPU.
    palette: Palette,
    hue: f32,
    hue_spread: f32,
    hue_center: f32,
    saturation: f32,
    palette_mix: f32,
    zoom: f32,
    pan_x: f32,
    pan_y: f32,
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
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
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
            lifetime: DEFAULT_LIFETIME,
            size: DEFAULT_SIZE,
            brightness: DEFAULT_BRIGHTNESS,
            palette: Palette::default_spectrum(),
            hue: DEFAULT_HUE,
            hue_spread: DEFAULT_HUE_SPREAD,
            hue_center: DEFAULT_HUE_CENTER,
            saturation: DEFAULT_SATURATION,
            palette_mix: DEFAULT_PALETTE_MIX,
            zoom: DEFAULT_ZOOM,
            pan_x: DEFAULT_PAN,
            pan_y: DEFAULT_PAN,
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
            spread: 0.0,
            lifetime: finite(self.lifetime, DEFAULT_LIFETIME).clamp(MIN_LIFETIME, MAX_LIFETIME),
            lifetime_spread: 0.0,
            source_half_width: self.aspect,
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
        self.lifetime = DEFAULT_LIFETIME;
        self.size = DEFAULT_SIZE;
        self.brightness = DEFAULT_BRIGHTNESS;
        self.hue = DEFAULT_HUE;
        self.hue_spread = DEFAULT_HUE_SPREAD;
        self.hue_center = DEFAULT_HUE_CENTER;
        self.saturation = DEFAULT_SATURATION;
        self.palette_mix = DEFAULT_PALETTE_MIX;
        self.zoom = DEFAULT_ZOOM;
        self.pan_x = DEFAULT_PAN;
        self.pan_y = DEFAULT_PAN;
    }

    fn set_param(&mut self, name: &str, value: f32) {
        match name {
            "spawn_rate" => self.spawn_rate = value,
            "gravity" => self.gravity = value,
            "launch_speed" => self.launch_speed = value,
            "launch_angle" => self.launch_angle = value,
            "lifetime" => self.lifetime = value,
            "size" => self.size = value,
            "brightness" => self.brightness = value,
            "hue" => self.hue = value,
            "hue_spread" => self.hue_spread = value,
            "hue_center" => self.hue_center = value,
            "saturation" => self.saturation = value,
            "palette_mix" => self.palette_mix = value,
            "zoom" => self.zoom = value,
            "pan_x" => self.pan_x = value,
            "pan_y" => self.pan_y = value,
            _ => {}
        }
    }

    fn update(&mut self, _frame: &AnalysisFrame) {
        let cfg = self.spawn_config();
        let time = self.time;
        self.field.step(time, &cfg);

        let size = finite(self.size, DEFAULT_SIZE) * BASE_SIZE;
        let brightness = finite(self.brightness, DEFAULT_BRIGHTNESS);
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
            let u = (time - object.t0) / object.lifetime;
            let coord = hue_coord(
                self.hue_center,
                self.hue_spread,
                unit(object.seed, channel::HUE),
                self.hue,
            );
            let base = palette::desaturate(
                self.palette.sample(coord, self.palette_mix),
                self.saturation,
            );
            let bright = brightness * envelope(u);
            *slot = Instance {
                center: pos,
                size,
                color: [base[0] * bright, base[1] * bright, base[2] * bright],
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
mod tests {
    // Tests index fixed-size arrays and panic on failure; allowed over the
    // file's hot-path pragma — this is not the render path.
    #![allow(clippy::indexing_slicing, clippy::panic, clippy::expect_used)]

    use super::{Field, Object, RETIRE_MARGIN, SOURCE_Y, Spawn, bounds, build, exit_time};

    /// A test aspect, and the retirement bound it gives.
    const ASPECT: f32 = 16.0 / 9.0;

    /// A spawn config with everything named, so each test says exactly what it
    /// varies.
    fn cfg(rate: f32, gravity: f32, speed: f32) -> Spawn {
        Spawn {
            rate,
            gravity,
            speed,
            angle: 0.0,
            spread: 0.0,
            lifetime: 4.0,
            lifetime_spread: 0.0,
            source_half_width: ASPECT,
            bound: bounds(ASPECT),
        }
    }

    /// **The closed form is the parabola it claims to be** (Phase 1's first
    /// done-when).
    ///
    /// An object launched straight up with speed `v0` against gravity `g`
    /// reaches its apex at `t = v0 / g`, at height `v0^2 / (2 g)` above its spawn
    /// point. Both are *derived*, not fitted: there is no integrator here, so the
    /// only error is f32 rounding of the two multiply-adds the position costs,
    /// and the tolerance below is a handful of ulps of the apex height rather
    /// than a number chosen to make a run pass.
    ///
    /// Two `(v0, g)` pairs, because one pair could coincide with an arithmetic
    /// slip (a swapped factor of two agrees with the truth at `g = 2`).
    #[test]
    fn an_object_follows_the_closed_form_parabola() {
        for (v0, g) in [(1.75f32, 1.5f32), (3.2, 9.81)] {
            let object = Object {
                p0: [0.0, 0.0],
                v0: [0.0, v0],
                t0: 0.0,
                lifetime: 100.0,
                gravity: g,
                death_time: f32::INFINITY,
                seed: 0,
                alive: true,
            };
            let apex_t = v0 / g;
            let apex_h = v0 * v0 / (2.0 * g);
            // A few ulps of the height, which is the magnitude the arithmetic
            // rounds at. Not a tolerance in the fitted sense — the closed form is
            // exact in real arithmetic.
            let slack = 8.0 * f32::EPSILON * apex_h;

            let at_apex = object.position(apex_t)[1];
            assert!(
                (at_apex - apex_h).abs() <= slack,
                "apex height at v0={v0} g={g}: got {at_apex}, want {apex_h} (slack {slack:e})"
            );

            // ...and it really is the maximum: nothing on a fine sweep either
            // side of it goes higher.
            for i in 0..=2000 {
                let t = apex_t * 2.0 * (i as f32 / 2000.0);
                let y = object.position(t)[1];
                assert!(
                    y <= apex_h + slack,
                    "t={t} rises to {y}, above the apex {apex_h}"
                );
            }
            // The launch point and the symmetric return, which pin the other two
            // roots of the same quadratic.
            assert!(object.position(0.0)[1].abs() <= slack);
            assert!(
                object.position(2.0 * apex_t)[1].abs() <= 4.0 * slack,
                "a symmetric flight returns to its launch height"
            );
        }
    }

    /// Drive a field through an explicit list of **scene times** and report the
    /// live population at the last one, keyed by spawn time so two runs can be
    /// compared object for object.
    fn population(times: &[f32], cfg: &Spawn, capacity: usize) -> Vec<(f32, [f32; 2])> {
        let mut field = Field::new(capacity);
        let mut last = 0.0;
        for &t in times {
            field.step(t, cfg);
            last = t;
        }
        let mut out: Vec<(f32, [f32; 2])> = field
            .objects
            .iter()
            .filter(|o| o.alive)
            .map(|o| (o.t0, o.position(last)))
            .collect();
        out.sort_by(|a, b| a.0.total_cmp(&b.0));
        out
    }

    /// **The trajectory is identical at different frame cadences** (Phase 1's
    /// second done-when), and so is the population.
    ///
    /// The claim ADR-0057 makes is structural rather than tuned, so it is
    /// asserted as **exact equality** and not within a tolerance: position holds
    /// no `dt`, spawn instants advance by the spawn period rather than by the
    /// frame step, and retirement compares against a death time solved at spawn.
    /// Nothing in the scene can see how the elapsed time was divided up.
    ///
    /// A steady 60 Hz cadence against a deliberately ragged one — long frames,
    /// short frames, a stall — both starting and ending at the same scene time.
    #[test]
    fn the_trajectory_is_the_same_under_any_frame_cadence() {
        const START: f32 = 0.02;
        const END: f32 = 4.0;
        let cfg = cfg(90.0, 1.5, 1.9);

        let steady: Vec<f32> = (0..=240)
            .map(|i| START + (END - START) * (i as f32 / 240.0))
            .collect();
        // Ragged: frame lengths swinging between a third and three times the
        // steady one, plus one long stall, landing on exactly the same end time.
        let ragged: Vec<f32> = {
            let mut times = vec![START];
            let mut t = START;
            let mut i = 0u32;
            while t < END {
                let step = match i % 5 {
                    0 => 0.004,
                    1 => 0.050,
                    2 => 0.011,
                    3 => 0.120,
                    _ => 0.007,
                };
                t += step;
                if t < END {
                    times.push(t);
                }
                i += 1;
            }
            times.push(END);
            times
        };
        assert!(
            ragged.len() * 2 < steady.len(),
            "the ragged cadence must genuinely differ from the steady one: \
             {} frames against {}",
            ragged.len(),
            steady.len()
        );

        let a = population(&steady, &cfg, 4096);
        let b = population(&ragged, &cfg, 4096);
        assert!(
            a.len() > 100,
            "the comparison must have a population to compare: {} objects",
            a.len()
        );
        assert_eq!(
            a, b,
            "the same scene time under two cadences must give the same objects \
             at the same places — the position is a closed form, so this is \
             exact, not approximate"
        );
    }

    /// **Objects genuinely leave** (Phase 1's third done-when) — the property the
    /// swarm fails by construction, since ADR-0044's toroidal `bounds` wraps every
    /// particle back into frame and its population is constant forever.
    ///
    /// Spawn for a second, then switch the source off and run far past the
    /// longest possible flight. A cascade empties; a torus does not. The
    /// lower-half count is read at both ends because that is where a wrapped
    /// object would have re-entered from the top and landed.
    #[test]
    fn objects_leave_the_frame_and_do_not_come_back() {
        let mut field = Field::new(4096);
        let on = cfg(200.0, 2.0, 1.4);
        let off = Spawn { rate: 0.0, ..on };

        let mut t = 0.0f32;
        let dt = 1.0 / 60.0;
        while t < 1.0 {
            t += dt;
            field.step(t, &on);
        }
        let populated = field.live;
        let lower_before = field
            .objects
            .iter()
            .filter(|o| o.alive && o.position(t)[1] < 0.0)
            .count();
        assert!(
            populated > 50 && lower_before > 5,
            "the field must fill before it drains, got {populated} live \
             ({lower_before} in the lower half)"
        );

        // Source off. Every object's lifetime is 4 s and the fastest flight is
        // far shorter, so ten seconds is well past the last possible retirement.
        let mut previous = field.live;
        while t < 11.0 {
            t += dt;
            field.step(t, &off);
            assert!(
                field.live <= previous,
                "with the source off the population may only fall; it went \
                 {previous} -> {} at t={t}",
                field.live
            );
            previous = field.live;
        }

        assert_eq!(
            field.live, 0,
            "every object must have been retired — a toroidal world would still \
             hold all {populated} of them"
        );
        let lower_after = field
            .objects
            .iter()
            .filter(|o| o.alive && o.position(t)[1] < 0.0)
            .count();
        assert_eq!(
            lower_after, 0,
            "the frame's lower half was replenished from the top: {lower_after} \
             objects, from {lower_before}"
        );
    }

    /// **The pool cannot be overrun** (Phase 1's fourth done-when) — the phase's
    /// real-time hazard, made unrepresentable rather than merely unlikely.
    ///
    /// A `spawn_rate` demanding two orders of magnitude more objects per second
    /// than the pool holds. The live count saturates at capacity, the vectors
    /// behind it never reallocate (their capacities are read before and after),
    /// and nothing panics.
    #[test]
    fn the_pool_saturates_instead_of_overrunning() {
        const CAPACITY: usize = 256;
        let mut field = Field::new(CAPACITY);
        let objects_capacity = field.objects.capacity();
        let free_capacity = field.free.capacity();

        // 100 000 objects a second against a 256-slot pool with a 4 s lifetime:
        // the demand is ~1560x what the pool can hold.
        let cfg = cfg(100_000.0, 1.5, 1.9);
        let mut t = 0.0f32;
        for _ in 0..60 {
            t += 1.0 / 60.0;
            field.step(t, &cfg);
            assert!(
                field.live <= CAPACITY,
                "the pool went over capacity: {} live in {CAPACITY} slots",
                field.live
            );
        }

        assert_eq!(
            field.live, CAPACITY,
            "an unbounded spawn rate must saturate the pool, not fall short of it"
        );
        assert_eq!(field.objects.len(), CAPACITY, "the pool must not grow");
        assert!(field.free.is_empty(), "a saturated pool has no free slots");
        assert_eq!(
            (field.objects.capacity(), field.free.capacity()),
            (objects_capacity, free_capacity),
            "neither the pool nor its free list may reallocate — this is the \
             hot path, and a spawn is the one place it could"
        );

        // And it drains again rather than latching: the dropped spawns left no
        // state behind.
        let off = Spawn { rate: 0.0, ..cfg };
        while t < 12.0 {
            t += 1.0 / 60.0;
            field.step(t, &off);
        }
        assert_eq!(field.live, 0, "a saturated pool must still drain");
        assert_eq!(field.free.len(), CAPACITY);
    }

    /// The retirement bound is the **render target's** shape, scaled outward —
    /// never a square, and never an internal grid's aspect (ADR-0037).
    #[test]
    fn the_retirement_bound_takes_its_shape_from_the_target() {
        for aspect in [16.0 / 9.0, 16.0 / 10.0, 4.0 / 3.0, 21.0 / 9.0, 9.0 / 16.0] {
            let b = bounds(aspect);
            assert!(
                (b[0] / b[1] - aspect).abs() < 1e-5,
                "bound shape {:.4} must equal the target's {aspect:.4}",
                b[0] / b[1]
            );
            assert!(b[1] > 1.0, "the bound must sit outside the visible frame");
        }
    }

    /// The death-time solve, against the three shapes of path it has to answer
    /// for. This is what makes retirement cadence-independent, so it is asserted
    /// directly rather than only through the population comparison above.
    #[test]
    fn the_exit_time_is_the_last_crossing_not_the_first() {
        let b = bounds(ASPECT);

        // Up and over: the object arcs *above* the top bound and comes back. A
        // first-crossing answer would retire it in mid-air; the true exit is when
        // it finally falls past the bottom.
        let g = 1.0f32;
        let v0 = [0.0f32, 3.0f32];
        let p0 = [0.0f32, SOURCE_Y];
        let t = exit_time(p0, v0, g, b);
        let apex = v0[1] / g;
        assert!(
            t > apex,
            "the exit must come after the apex at {apex}, got {t}"
        );
        let y_at_exit = p0[1] + v0[1] * t - 0.5 * g * t * t;
        assert!(
            (y_at_exit + b[1]).abs() < 1e-3,
            "the exit must land on the bottom bound {}, got {y_at_exit}",
            -b[1]
        );
        // It really did leave the frame at the top on the way, which is the case
        // a sampled bound would have got wrong.
        assert!(
            p0[1] + v0[1] * apex - 0.5 * g * apex * apex > RETIRE_MARGIN,
            "this path must genuinely go above the bound, or it proves nothing"
        );

        // Sideways: linear in t, so the side exit is exact and permanent.
        let t_side = exit_time([0.0, 0.0], [2.0, 0.0], 0.0, b);
        assert!(
            (t_side - b[0] / 2.0).abs() < 1e-5,
            "a horizontal path leaves at bound_x / speed: {t_side}"
        );

        // No gravity, no downward speed, no side speed: it never leaves, and only
        // the lifetime retires it.
        assert_eq!(exit_time([0.0, 0.0], [0.0, 1.0], 0.0, b), f32::INFINITY);
    }

    /// A spawn draws its source position from its **seed**, so an object's whole
    /// description is a pure function of `(seed, spawn time, config)` — and the
    /// source line spans the target's width rather than a square.
    #[test]
    fn a_spawn_is_a_pure_function_of_its_seed() {
        let cfg = cfg(60.0, 1.5, 1.9);
        for seed in [0u32, 1, 7, 0xDEAD_BEEF, u32::MAX] {
            let a = build(seed, 0.25, &cfg);
            let b = build(seed, 0.25, &cfg);
            assert_eq!(a.p0, b.p0);
            assert_eq!(a.v0, b.v0);
            assert_eq!(a.death_time, b.death_time);
            assert!(
                a.p0[0].abs() <= cfg.source_half_width,
                "the source line spans the frame width, got x={}",
                a.p0[0]
            );
            assert_eq!(a.p0[1], SOURCE_Y);
        }
    }
}
