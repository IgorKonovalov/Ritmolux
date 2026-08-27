//! The attractor's four WGSL programs (Plan 0061 Phase 6).
//!
//! Text only — every constant here is shader source, not Rust. They live in
//! their own file because they are the single largest thing in `particles/` and
//! they change for entirely different reasons than the scene that dispatches
//! them: Plans 0073 and 0074 nearly doubled `STEP_SHADER` and `DRAW_SHADER`
//! between two re-measures of the module.
//!
//! `projection_mirror` — the CPU transcription that pins `DRAW_SHADER`'s
//! projection — stays a child of `particles`, so it still reaches these
//! constants through `super`.

// No pragma here: this file declares no Rust code path, only `&str` shader
// source. The hygiene guard's sentinel is carried below so the guard, which
// greps every file under render/, still recognises it as covered.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

/// Compute step: iterate every particle through the selected attractor map once.
/// Discrete maps (De Jong, Clifford) iterate directly; continuous flows (Thomas,
/// Lorenz) Euler-integrate a few sub-steps of the fixed frame `dt`. Writes the
/// storage buffer in place; the draw pass then reads it as a vertex buffer.
pub(super) const STEP_SHADER: &str = r#"
struct Particle {
    pos: vec3<f32>,
    seed: f32,
    prev: vec3<f32>,
    pad: f32,
    // ADR-0087's two channels. `age` counts steps since this particle last
    // respawned; `map` is the index of the map applied on the most recent step.
    // Both are written by the IFS arm alone and stay at their seeded 0.0
    // everywhere else.
    age: f32,
    map: f32,
    // ADR-0088's third, at offset 40: the distance from this point to the
    // nearest of the drawn maps' fixed points, normalised by the skeleton's own
    // diameter. Written by the IFS arm alone, like the two above.
    root: f32,
    // The LAST spare word. Named, not implicit: WGSL's 16-byte round-up bought
    // it, and it is the next channel's budget rather than slack.
    spare1: f32,
}
@group(0) @binding(0) var<storage, read_write> particles: array<Particle>;

struct Step {
    coeffs: vec4<f32>, // discrete: a,b,c,d; Lorenz: sigma,rho,beta; Thomas: b
                       //   family 5 (jitter): xyz half-extent of the kick,
                       //   w != 0 draws the kick as a streak (ADR-0069)
    dt: f32,           // fixed sub-step seconds (for continuous families)
    family: u32,       // 0 De Jong, 1 Clifford, 2 Thomas, 3 Lorenz, 4 IFS, 5 jitter
    count: u32,        // active particle count
    salt: u32,         // jitter only: which reseed this is
    // Monotonic fixed-step counter, incremented once per compute step (ADR-0075).
    // The IFS's map choice is drawn from it and the particle's own fixed seed, so
    // the draw stays a pure function of the seed and the step sequence — and the
    // step sequence is a pure function of accumulated injected dt, which captures
    // pin at 1/60 s. Zero and unread on every other family.
    step_index: u32,
    // The reciprocal of the fixed-point set's floored diameter (ADR-0088). It
    // spends the FIRST of the three padding words below, which the vec4
    // alignment had already paid for - so the struct stays 192 bytes and the
    // bind-group layout gains no binding. Zero (and unread) on every other
    // family and on the jitter dispatch.
    root_recip: f32,
    // The rest of the vec4 alignment the affine table below requires. SCALARS,
    // not a `vec3<u32>`: a WGSL vec3 aligns to 16, which would push the table to
    // offset 64 and the struct to 176 while the Rust side laid it out at 48 and
    // 160.
    _pad1: u32,
    _pad2: u32,
    // The IFS's resolved affine table (family 4), CPU-side output of
    // `ifs::resolve`. Four linear parts (a,b,c,d), four (e,f) translations packed
    // two per row, and the cumulative probabilities a unit draw is compared
    // against. Four named rows rather than an array: the map choice is an
    // unrolled branch, for the reason `Basis::masks` uses one-hot selectors.
    m0: vec4<f32>,
    m1: vec4<f32>,
    m2: vec4<f32>,
    m3: vec4<f32>,
    t01: vec4<f32>,
    t23: vec4<f32>,
    cumulative_p: vec4<f32>,
    // The four respawn targets (ADR-0087), two (x, y) per row exactly as the
    // translations above are. Every slot is a DRAWN map's fixed point - the CPU
    // duplicates into the pads - so a pick of one of four needs no branch and no
    // knowledge of the probability table.
    fixed01: vec4<f32>,
    fixed23: vec4<f32>,
}
@group(0) @binding(1) var<uniform> step: Step;

// ADR-0087's churn constants. MIRRORED FROM THE RUST SIDE, which is the source:
// `CHURN_LIFETIME`, `CHURN_LIFETIME_SPREAD` and `LIFETIME_SALT`, held to these
// literals by `the_churn_constants_agree_between_rust_and_wgsl`. The CPU needs
// the same numbers because `seed()` places each particle at a point in a life
// this shader measures.
const LIFETIME_STEPS: f32 = 180.0;
const LIFETIME_LO: f32 = 0.5;
const LIFETIME_HI: f32 = 1.5;
const LIFETIME_SALT: u32 = 0x9E3779B1u;
// A separate salt for the respawn target, so which point a particle restarts at
// does not correlate with how long it lived.
const RESPAWN_SALT: u32 = 0x85EBCA6Bu;

// Euler sub-steps per frame for the continuous (ODE) families, so a stiff flow
// (Lorenz) stays stable at the frame dt without a per-family clock.
const ODE_SUBSTEPS: i32 = 4;

// A reseed's per-particle offset (ADR-0066). Deterministic: a pure function of
// the particle's own fixed seed and the reseed counter, so the cloud stays a pure
// function of its seed and step sequence, and two runs from the same seed produce
// identical positions after a reseed.
//
// Salted by the counter rather than by the seed alone, so successive reseeds kick
// a given particle in different directions. With the seed alone every reseed
// would apply the same displacement field, which over a session is a rigid
// pattern rather than a disturbance.
//
// `mix32` and `unit01` are NOT declared here: they moved to `gpu::HASH_WGSL` when
// the tonemap's dither became their second caller (Plan 0082 Phase 1), and
// `resources.rs` concatenates that text in front of this one.

// The same 24 bits as a SIGNED unit fraction in [-1, 1) — the reseed kick, which
// needs a direction as well as a magnitude. The unsigned spelling this is built
// from is the shared one.
fn unit(h: u32) -> f32 {
    return unit01(h) * 2.0 - 1.0;
}

// Unrolled rather than looped over a dynamically-indexed vector: WGSL permits
// that only for addressable storage and the backends disagree about the rest, so
// three named rounds is the portable spelling.
// This particle's lifetime in steps (ADR-0087). A pure function of its own fixed
// seed, so the phases are spread across the buffer once and stay spread: a small
// flat fraction of the population restarts each step rather than the whole of it
// restarting together every three seconds.
fn ifs_lifetime(seed: f32) -> f32 {
    let u = unit01(mix32(bitcast<u32>(seed) ^ LIFETIME_SALT));
    return LIFETIME_STEPS * (LIFETIME_LO + u * (LIFETIME_HI - LIFETIME_LO));
}

// Which of the four targets this particle restarts at. Salted by the step index
// as well as the seed, so a particle does not return to the same point every
// time it recycles - the same reason ADR-0066's kick is salted by the reseed
// counter rather than by the seed alone.
fn ifs_respawn_slot(seed: f32, step_index: u32) -> u32 {
    let u = unit01(mix32(bitcast<u32>(seed) ^ (step_index * RESPAWN_SALT)));
    return min(u32(u * 4.0), 3u);
}

// Unrolled for the reason the map choice above is: WGSL will not dynamically
// index a uniform, and the backends disagree about the rest.
fn ifs_fixed_point(slot: u32) -> vec2<f32> {
    if (slot == 0u) {
        return step.fixed01.xy;
    } else if (slot == 1u) {
        return step.fixed01.zw;
    } else if (slot == 2u) {
        return step.fixed23.xy;
    }
    return step.fixed23.zw;
}

// ADR-0088's channel: how far this point is from the figure's own skeleton,
// normalised by the skeleton's diameter (the CPU ships the reciprocal).
//
// A `min` over all four slots with no branch and no knowledge of the probability
// table, for the reason `ifs_fixed_point` above needs neither: every slot is a
// DRAWN map's fixed point, because the CPU duplicates into the pads.
//
// **This is the SOURCE**; `root_distance` in the Rust test body transcribes it,
// the discipline `projection_mirror` follows against the draw shader.
//
// Normalised at the write and clamped at the READ, so the stored value stays a
// faithful measurement: the skeleton's diameter is not an upper bound on how far
// the attractor reaches, and a point past 1 is a real point rather than an error.
fn ifs_root_distance(q: vec2<f32>) -> f32 {
    let d0 = distance(q, step.fixed01.xy);
    let d1 = distance(q, step.fixed01.zw);
    let d2 = distance(q, step.fixed23.xy);
    let d3 = distance(q, step.fixed23.zw);
    return min(min(d0, d1), min(d2, d3)) * step.root_recip;
}

fn hash3(seed: f32, salt: u32) -> vec3<f32> {
    let h0 = mix32(bitcast<u32>(seed) ^ (salt * 0x9E3779B9u));
    let h1 = mix32(h0);
    let h2 = mix32(h1);
    return vec3<f32>(unit(h0), unit(h1), unit(h2));
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= step.count) {
        return;
    }
    let a = step.coeffs.x;
    let b = step.coeffs.y;
    let c = step.coeffs.z;
    let d = step.coeffs.w;
    var p = particles[i].pos;
    // Captured before any branch mutates `p`. The storage slot still holds this
    // value until the write at the end, so reading it there would work too —
    // but only by knowing that, which is exactly the kind of thing that breaks
    // when someone reorders the writes.
    let origin = p;

    if (step.family == 5u) {
        // A reseed: disturb the cloud where it is, rather than replacing it with
        // a uniform fill of the seed box (ADR-0066). The points stay on the
        // attractor, so no axis-aligned rectangle exists at any moment, and the
        // map's own mixing spreads the kick within a few iterations.
        let kicked = p + hash3(particles[i].seed, step.salt) * step.coeffs.xyz;
        // Whether the kick is *drawn* as a streak is Phase 4's call, not this
        // phase's: a jitter displaces a particle by far more than a step does
        // (ADR-0069 measures ~15x a frame's travel), so the segment would be a
        // long stroke along a path the particle never traversed. `w` selects it,
        // so an A/B is a constant flip and no shader edit.
        //   w != 0 -> prev stays pre-kick, the streak is drawn
        //   w == 0 -> prev follows the kick, so the segment has zero length
        particles[i].prev = select(kicked, origin, step.coeffs.w != 0.0);
        particles[i].pos = kicked;
        return;
    }

    if (step.family == 0u) {
        // De Jong: x' = sin(a*y) - cos(b*x), y' = sin(c*x) - cos(d*y).
        p = vec3<f32>(sin(a * p.y) - cos(b * p.x), sin(c * p.x) - cos(d * p.y), 0.0);
    } else if (step.family == 1u) {
        // Clifford: x' = sin(a*y) + c*cos(a*x), y' = sin(b*x) + d*cos(b*y).
        p = vec3<f32>(sin(a * p.y) + c * cos(a * p.x), sin(b * p.x) + d * cos(b * p.y), 0.0);
    } else if (step.family == 4u) {
        // Iterated function system (ADR-0075): draw one of four affine maps and
        // apply it. The draw is salted by the step counter rather than by the
        // reseed counter, so a particle picks a different map each step while
        // staying a pure function of its own fixed seed and the step index.
        let r = unit01(mix32(bitcast<u32>(particles[i].seed) ^ (step.step_index * 0x9E3779B9u)));
        // Unrolled rather than a dynamically-indexed uniform array — WGSL permits
        // that only for addressable storage and the backends disagree about the
        // rest, the same reason `hash3` above is three named rounds.
        var m = step.m3;
        var t = step.t23.zw;
        // Which of the four was chosen, carried as an f32 because that is what
        // the channel is and because the draw reads it through a vertex
        // attribute, which has no integer path here (ADR-0087).
        var k = 3.0;
        if (r < step.cumulative_p.x) {
            m = step.m0;
            t = step.t01.xy;
            k = 0.0;
        } else if (r < step.cumulative_p.y) {
            m = step.m1;
            t = step.t01.zw;
            k = 1.0;
        } else if (r < step.cumulative_p.z) {
            m = step.m2;
            t = step.t23.xy;
            k = 2.0;
        }
        // x' = a*x + b*y + e,  y' = c*x + d*y + f. Two dimensional: z stays 0.
        p = vec3<f32>(m.x * p.x + m.y * p.y + t.x, m.z * p.x + m.w * p.y + t.y, 0.0);

        // ADR-0087's churn. The particle ages one step, and at the end of its own
        // lifetime restarts at one of the drawn maps' fixed points - which are ON
        // the attractor, so it is drawing the figure again from its first step
        // rather than travelling to it.
        //
        // Continuous rather than a one-time unfurl, and that is the load-bearing
        // half: under a one-time unfurl every age saturates within ~0.4 s and the
        // age channel is a uniform value thereafter. Under churn the population
        // always holds every age, so Phase 4's gradient is permanent.
        var age = particles[i].age + 1.0;
        if (age >= ifs_lifetime(particles[i].seed)) {
            let slot = ifs_respawn_slot(particles[i].seed, step.step_index);
            p = vec3<f32>(ifs_fixed_point(slot), 0.0);
            // It now sits AT map `slot`'s fixed point, which is inside that map's
            // sub-copy - so `map` is the slot, not the map that was applied above
            // and then discarded.
            k = f32(slot);
            age = 0.0;
        }
        particles[i].age = age;
        // Unconditional within this arm: the value names where the particle now
        // IS, so it is only meaningful when written every step. The jitter
        // dispatch returns above without touching it, which is right — a reseed
        // displaces a particle without changing which sub-copy it belongs to.
        particles[i].map = k;
        // ...and so is ADR-0088's distance, but NOT for the same reason, and the
        // difference is worth spelling out because the obvious reading has it
        // backwards. It is a PURE FUNCTION OF POSITION, recomputed from where the
        // particle now sits rather than accumulated — which is the whole
        // difference from `age`, and the reason this gradient does not decay: a
        // particle five hundred steps old sitting near a fixed point reads the
        // same near-zero a freshly restarted one does.
        //
        // That makes the jitter dispatch's early return WEAKER here than it is
        // for `map`, not stronger. A reseed kick leaves sub-copy membership
        // alone, so `map` is still correct after it; it moves the particle, so
        // `root` is not. The kicked particle carries the distance it had before
        // the kick until the next fixed step overwrites it — one step, ~1/60 s,
        // and the emergence ramp is not involved because nothing respawned.
        // Do NOT "fix" it by calling `ifs_root_distance` in the jitter arm. That
        // dispatch is handed `StepUniform::NO_IFS`, so `step.fixed01`/`fixed23`
        // and `step.root_recip` are all zero there — the call would return an
        // exact 0 for every particle and flash the whole figure to the palette's
        // anchor colour on every reseed. Fixing it properly means uploading the
        // table to the jitter slot, which costs more than one stale step is
        // worth.
        //
        // After the respawn branch, so a just-restarted particle reads EXACTLY
        // 0 — it *is* at a fixed point. That is one end of the ramp rather than a
        // special case.
        particles[i].root = ifs_root_distance(p.xy);
    } else if (step.family == 2u) {
        // Thomas cyclically-symmetric flow (b = dissipation). Lively speed-up so
        // the slow flow visibly moves each frame.
        let h = step.dt * 3.0 / f32(ODE_SUBSTEPS);
        for (var s = 0; s < ODE_SUBSTEPS; s = s + 1) {
            let dp = vec3<f32>(sin(p.y) - a * p.x, sin(p.z) - a * p.y, sin(p.x) - a * p.z);
            p = p + dp * h;
        }
    } else {
        // Lorenz (sigma, rho, beta). Euler-integrated in sub-steps for stability.
        let h = step.dt / f32(ODE_SUBSTEPS);
        for (var s = 0; s < ODE_SUBSTEPS; s = s + 1) {
            let dp = vec3<f32>(a * (p.y - p.x), p.x * (b - p.z) - p.y, p.x * p.y - c * p.z);
            p = p + dp * h;
        }
    }

    // The position this particle came from, for the continuous families' segment
    // (ADR-0069). Written for every family — the *draw* decides whether to use
    // it, so the buffer's contents stay one shape and a discrete map that took
    // the branch by mistake is a visible chord rather than stale data.
    //
    // This is the position before the whole step, not before the last Euler
    // sub-step: ADR-0069 rejected the sub-step polyline by measurement, and the
    // segment is meant to span the frame's travel.
    particles[i].prev = origin;
    particles[i].pos = p;
}
"#;

/// Draw pass: one additive glowing point-sprite per particle, into the trail
/// field. The particle storage buffer is bound as an instance vertex buffer; the
/// shader expands each into a screen-facing quad, projects the (2D or 3D)
/// attractor state to the screen with a slow spin, and tints it from the seeded
/// per-particle offset.
pub(super) const DRAW_SHADER: &str = r#"
struct Draw {
    // v: x aspect, y point half-size (world), z hue offset, w spin (radians)
    // w: x world scale, y dim (2 or 3), z unused (was the z-center; Plan 0062
    //    made the centre three components and moved it to `ctr`),
    //    w deposit scale (ADR-0065: FLOOR_PARTICLES / active_count)
    // u: x hue_spread, y hue_center, z palette_mix, w saturation
    // x: x zoom, yz pan (view transform, ADR-0018), w streak (ADR-0069:
    //    non-zero on a continuous family, so the quad spans prev -> pos)
    // bh: xyz the axis the spin rotates x against (ADR-0068), w unused
    // bv: xyz the vertical axis (ADR-0068), w unused
    // d: x perspective, y depth_fade, z depth_hue, w the family's INVERSE depth
    //    half-extent (ADR-0076) - exactly 0 for a 2D family, which is what
    //    collapses every depth cue below to the identity with no branch
    // ctr: xyz the world centre subtracted before projection, w unused. The four
    //    map families pass [0,0,0] or [0,0,25] - exactly what they passed when
    //    this was the scalar `w.z` - and subtracting a zero is exact.
    // ch: the two per-particle colour channels, two routes each -
    //    x map_tint, y map_hue (ADR-0087), z root_tint, w root_hue (ADR-0088).
    //    The row SWAPPED rather than grew at Plan 0074 Phase 3: `age_tint` and
    //    `age_hue` held z and w and were retired, because `age` proxied
    //    distance-from-the-fixed-points and the proxy decayed. Every one defaults
    //    to 0 and 0 is the arithmetic identity on all four routes, so a preset
    //    that binds none of them renders exactly what it rendered before they
    //    existed. The CPU zeroes the WHOLE ROW on a non-IFS family, where both
    //    channels are identically 0 and `channel_shift` being centred would
    //    otherwise turn a bound value into a uniform tint over a family it means
    //    nothing on.
    //    THE TWO HALVES ARE NOT THE SAME SHAPE. `map_*` is centred, because
    //    `map01` genuinely spans [0, 1]; `root_*` is ANCHORED at 0, because
    //    `root01` does not (ADR-0088's Anchoring section). The zeroing above is
    //    therefore load-bearing for x/y and merely belt-and-braces for z/w.
    // em: ADR-0087's emergence ramp - x the per-step brightness increment, y the
    //    floor. The IFS passes (1/emergence, 0); every other family passes
    //    (0, 1), which makes the ramp EXACTLY 1.0 there rather than exactly 0 -
    //    their `age` is identically zero, so a bare `age * rate` would black them
    //    out. Two numbers rather than a branch, and the multiply by a literal 1.0
    //    is the identity in IEEE-754, so no existing capture moves.
    //    z is `palette_steps` (ADR-0078) - it and w were FREE since Plan 0074
    //    Phase 3, when z stopped carrying the reciprocal of the longest
    //    reachable lifetime that only the retired age colour channel read.
    v: vec4<f32>,
    w: vec4<f32>,
    u: vec4<f32>,
    x: vec4<f32>,
    bh: vec4<f32>,
    bv: vec4<f32>,
    d: vec4<f32>,
    ctr: vec4<f32>,
    ch: vec4<f32>,
    em: vec4<f32>,
}
@group(0) @binding(0) var<uniform> draw: Draw;
// Shared gradient LUTs (ADR-0021): sampled per-particle in the vertex shader
// (VERTEX visibility). A/B for the `palette_mix` crossfade, one repeat sampler.
@group(0) @binding(1) var lut_a: texture_2d<f32>;
@group(0) @binding(2) var lut_b: texture_2d<f32>;
@group(0) @binding(3) var lut_samp: sampler;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    // Position within the sprite, in units of the point radius. For a point this
    // is the corner itself; for a segment the quad is stretched along its axis
    // and this is the coordinate in the segment's own frame.
    @location(0) local: vec2<f32>,
    @location(1) color: vec3<f32>,
    // Half-length of the segment in the same units, so the fragment measures
    // distance to a *capsule* rather than to a disc. Exactly 0 for a point,
    // which makes the two cases one expression (ADR-0069).
    @location(2) @interpolate(flat) half_len: f32,
}

// Shared `saturation` (mirrors core/src/render/palette.rs::desaturate verbatim).
fn apply_saturation(c: vec3<f32>, s: f32) -> vec3<f32> {
    let luma = dot(c, vec3<f32>(0.299, 0.587, 0.114));
    return vec3<f32>(luma) + (c - vec3<f32>(luma)) * s;
}

// Project one attractor position to the pre-aspect "world" plane, **keeping the
// depth** the rotation produces: `xy` is the screen position, `z` is the view
// depth (ADR-0076). Factored out of the vertex body so a segment can project
// **both** its endpoints through the identical path — two call sites that must
// not be allowed to drift apart.
//
// The depth used to be computed and thrown away, which is exactly why the 3D
// families rendered flat: an orthographic projection of a rotating transparent
// structure carries no information about the direction of rotation, because the
// image at rotation pi is the exact x-mirror of the image at 0.
//
// **This function and the two below are the SOURCE**; `projection_mirror` in the
// Rust body transcribes them for the property test, the same discipline
// `apply_saturation` follows against `palette.rs::desaturate`. Edit here, then
// edit there.
fn project(q: vec3<f32>, dim: f32, ctr: vec3<f32>, cs: f32, sn: f32) -> vec3<f32> {
    if (dim < 2.5) {
        // 2D map: centre, then in-plane rotation. There is no third coordinate,
        // so the depth is zero here as well as via `draw.d.w` - belt and braces,
        // and neither is load-bearing alone.
        //
        // The centre is what lets a 2D figure sit off the origin (the fern spans
        // y in [0, 10]); De Jong and Clifford pass [0,0,0], so this subtraction
        // is exact and their captures are unchanged.
        let c = q - ctr;
        return vec3<f32>(c.x * cs - c.y * sn, c.x * sn + c.y * cs, 0.0);
    }
    // 3D flow: centre, pick the viewing plane, rotate around the vertical
    // axis, project. The plane is the family's own (ADR-0068), arriving as two
    // one-hot axis selectors rather than as a second pipeline: `bh` is the axis
    // the spin rotates `x` against, `bv` is the vertical. `bh = z, bv = y`
    // reproduces the shared convention this replaced exactly; Lorenz ships
    // `bh = y, bv = z`.
    let p = q - ctr;
    let h = dot(p, draw.bh.xyz);
    // The third term is the rotation's OTHER output - the exact partner of the
    // horizontal one - so it costs a multiply-add rather than a second rotation.
    return vec3<f32>(p.x * cs + h * sn, dot(p, draw.bv.xyz), -p.x * sn + h * cs);
}

// Depth in units of the family's own half-extent, clamped to [-1, 1].
//
// `draw.d.w` is an INVERSE extent and is exactly 0 for a 2D family, so this is
// identically 0 there - no branch, no division, no NaN.
//
// **The clamp is not decoration.** A family's converged figure overruns its
// `seed_box` (Lorenz reaches y = 25.4 against a 26 half-extent while its x
// reaches 19.2, so the rotated depth reaches ~1.22), and an unclamped value at
// the `perspective` ceiling would magnify by ~50x rather than the 5x ADR-0076
// documents. Clamping is what makes the stated (1 + p) / (1 - p) ratio true and
// keeps the divisor below bounded away from zero.
fn depth_norm(depth: f32) -> f32 {
    return clamp(depth * draw.d.w, -1.0, 1.0);
}

// The perspective magnification: near material grows, far material shrinks.
// `perspective` is the figure's depth half-extent as a fraction of the camera
// distance, clamped CPU-side to [0, 0.8], so the divisor stays in [0.2, 1.8].
// At `perspective = 0` this is exactly 1.0 and every use of it is a no-op.
fn magnify(dn: f32) -> f32 {
    return 1.0 / (1.0 - draw.d.x * dn);
}

// Depth remapped to [0, 1] with **1 nearest**, which is the sense both
// atmospheric cues below are written in. `dn` is already clamped, so this needs
// no clamp of its own.
fn depth01(dn: f32) -> f32 {
    return (dn + 1.0) * 0.5;
}

// Distance haze: brightness attenuated with distance, so `depth_fade = 1` takes
// the far end to black and `0` is exactly 1.0 everywhere (ADR-0076).
// `depth_fade` is clamped CPU-side to [0, 1] - past 1 this would go NEGATIVE,
// and a negative deposit in an additive accumulation subtracts light.
//
// The fade term is also multiplied by whether the family HAS depth at all
// (Plan 0075 Phase 2, design-backlog 0067). `dn` is identically 0 on a flat
// family, which lands `depth01` on 0.5 - arithmetically "mid depth", so the
// multiplier was a uniform `1 - depth_fade/2`: a 45% whole-figure dimmer at
// `depth_fade = 0.9`, on the one cue that was NOT the identity at zero extent.
// `f32(bool)` is 1.0 or 0.0 - one extra multiply, no branch, the same style
// as the zero-extent trick in `depth_norm`.
fn haze(dn: f32) -> f32 {
    let has_depth = f32(draw.d.w != 0.0);
    return 1.0 - draw.d.y * (1.0 - depth01(dn)) * has_depth;
}

// Distance tint: a shift of +/- depth_hue/2 in the palette coordinate across the
// depth range, centred so the mid-depth colour is the one the preset asked for.
// Rides the existing LUT sample and needs no new machinery.
fn depth_tint(dn: f32) -> f32 {
    return draw.d.z * (depth01(dn) - 0.5);
}

// The largest map index, so `map / MAP_SPAN` puts the fern's stem at 0 and its
// right frond at 1. Mirrors `ifs::MAPS - 1`; the count is structural (the step
// shader's choice is an unrolled four-way branch), so this is a constant rather
// than a uniform field.
const MAP_SPAN: f32 = 3.0;

// A per-particle channel's contribution, **centred**: a shift of +/- amount/2
// across the channel's [0, 1] range, so the mid-channel colour is the one the
// preset asked for and raising the amount opens a spread rather than sliding the
// whole figure. Exactly `depth_tint`'s shape, for exactly its reason.
//
// At `amount = 0` this is an exact 0 whatever `unit` is, which is what makes
// both palette-coordinate routes the arithmetic identity at their defaults.
fn channel_shift(unit: f32, amount: f32) -> f32 {
    return amount * (unit - 0.5);
}

// Shared `palette_steps` (mirrors core/src/render/palette.rs::band_coord
// verbatim, ADR-0078): snap the palette coordinate to a band centre before the
// LUT read. Below 1.5 steps it is the exact identity, not a one-band degenerate.
fn band_coord(t: f32, steps: f32) -> f32 {
    if (steps < 1.5) {
        return t;
    }
    return (floor(t * steps) + 0.5) / steps;
}

// Standard RGB->HSV, the inverse of `hsv2rgb` below (both are the iq forms; the
// forward one is transcribed from `render/ink.rs`, which is where this project
// already spells HSV). The `1e-10` guards the two divisions on a greyscale or
// black input, where hue is undefined and any value is as good as another.
fn rgb2hsv(c: vec3<f32>) -> vec3<f32> {
    let k = vec4<f32>(0.0, -1.0 / 3.0, 2.0 / 3.0, -1.0);
    let p = mix(vec4<f32>(c.bg, k.wz), vec4<f32>(c.gb, k.xy), step(c.b, c.g));
    let q = mix(vec4<f32>(p.xyw, c.r), vec4<f32>(c.r, p.yzx), step(p.x, c.r));
    let d = q.x - min(q.w, q.y);
    let e = 1.0e-10;
    return vec3<f32>(abs(q.z + (q.w - q.y) / (6.0 * d + e)), d / (q.x + e), q.x);
}

// Standard HSV->RGB (iq form), transcribed from `render/ink.rs::hsv2rgb`.
// `fract` normalizes an arbitrary hue into [0, 1), so a shift may sweep freely.
fn hsv2rgb(c: vec3<f32>) -> vec3<f32> {
    let h = fract(c.x);
    let rgb = clamp(
        abs(((h * 6.0 + vec3<f32>(0.0, 4.0, 2.0)) % vec3<f32>(6.0)) - vec3<f32>(3.0)) - vec3<f32>(1.0),
        vec3<f32>(0.0),
        vec3<f32>(1.0),
    );
    return c.z * mix(vec3<f32>(1.0), rgb, c.y);
}

// The **second** route a channel reaches colour by (ADR-0087): rotate the hue of
// the colour the palette already produced, leaving the palette coordinate alone.
// That is the route for a preset that wants its fronds nudged off its body
// without editing its ramp; `*_tint` is the route for one whose colour should be
// the author's gradient.
//
// **The zero early-out is load-bearing, not an optimization.** An RGB -> HSV ->
// RGB round trip is not bit-exact, and sixteen golden baselines assert that a
// preset binding none of these params renders byte-identically. Comparing
// against literal 0.0 is exact, and 0.0 is what every unbound preset carries.
fn shift_hue(c: vec3<f32>, turns: f32) -> vec3<f32> {
    if (turns == 0.0) {
        return c;
    }
    var hsv = rgb2hsv(c);
    hsv.x = fract(hsv.x + turns);
    return hsv2rgb(hsv);
}

@vertex
fn vs_main(
    @builtin(vertex_index) vi: u32,
    @location(0) center: vec3<f32>,
    @location(1) seed: f32,
    @location(2) previous: vec3<f32>,
    // ADR-0087's last-map channel, at byte offset 36 of the particle. The
    // attribute offsets are spelled out in `PARTICLE_ATTRIBUTES` rather than
    // taken from `vertex_attr_array!`, which lays attributes out consecutively
    // and would fetch this from the padding word.
    @location(3) map: f32,
    // ADR-0087's age channel, at byte offset 32. Steps since this particle last
    // respawned; identically 0 on every family but the IFS.
    @location(4) age: f32,
    // ADR-0088's root channel, at byte offset 40. Distance to the nearest of the
    // drawn maps' fixed points, already normalised by the skeleton's diameter;
    // identically 0 on every family but the IFS.
    @location(5) root: f32,
) -> VsOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
    );
    let corner = corners[vi] * 2.0 - vec2<f32>(1.0, 1.0);
    let aspect = draw.v.x;
    let psize = draw.v.y;
    let hue = draw.v.z;
    let rot = draw.v.w;
    let scl = draw.w.x;
    let dim = draw.w.y;
    let ctr = draw.ctr.xyz;
    let hue_spread = draw.u.x;
    let hue_center = draw.u.y;
    let palette_mix = draw.u.z;
    let saturation = draw.u.w;

    let cs = cos(rot);
    let sn = sin(rot);
    let streak = draw.x.w;
    let projected = project(center, dim, ctr, cs, sn);
    let screen = projected.xy;
    // This particle's normalized depth, and the magnification it earns
    // (ADR-0076). Both are exactly 0 and exactly 1 for a 2D family.
    let dn = depth_norm(projected.z);
    let mag = magnify(dn);
    // Position AND sprite size take the same magnification, which is what makes
    // size grading and parallax one mutually-consistent term rather than two
    // hand-tuned constants (the swarm needed two; ADR-0076 Alternative B).
    let sprite = psize * mag;

    // The sprite. A point is a `sprite` square about the projected position; a
    // segment is that square swept from `prev` to `pos` — a capsule (ADR-0069).
    //
    // Both are built in **world** space, before the single aspect division below.
    // That is deliberate and it is what keeps the stroke an even width: world `x`
    // is what becomes NDC `x / aspect`, so equal world distances are equal
    // *pixels* on both axes, and a capsule built here is round-ended on screen
    // rather than sheared by the target's aspect (ADR-0037).
    var world: vec2<f32>;
    var local: vec2<f32>;
    var half_len = 0.0;
    if (streak != 0.0) {
        // **Both endpoints are magnified independently**, so a trace receding
        // into the distance is drawn genuinely shorter - the strongest depth cue
        // a curve has, and free, because the capsule already projects both ends.
        let pp = project(previous, dim, ctr, cs, sn);
        let dn_prev = depth_norm(pp.z);
        let a = pp.xy * scl * magnify(dn_prev);
        let b = screen * scl * mag;
        let mid = (a + b) * 0.5;
        let axis = (b - a) * 0.5;
        let len = length(axis);
        // A stationary particle has no direction to orient by, and `normalize`
        // of a zero vector is undefined — so fall back to the point's own frame,
        // which is what a zero-length capsule is anyway.
        var dir = vec2<f32>(1.0, 0.0);
        if (len > 1e-9) {
            dir = axis / len;
        }
        let nrm = vec2<f32>(-dir.y, dir.x);
        // The capsule's WIDTH is uniform and takes the midpoint's magnification.
        // A tapered stroke would mean interpolating a radius in the fragment's
        // distance function, which reworks ADR-0069's one-expression
        // point/segment unification - deliberately out of scope (ADR-0076).
        let wid = psize * magnify((dn + dn_prev) * 0.5);
        half_len = len / wid;
        // Extended by `wid` past each end so the round caps have room.
        world = mid + dir * (corner.x * (len + wid)) + nrm * (corner.y * wid);
        local = vec2<f32>(corner.x * (half_len + 1.0), corner.y);
    } else {
        world = screen * scl * mag + corner * sprite;
        local = corner;
    }

    // View transform (ADR-0018): project to NDC, then scale about the screen centre
    // by `zoom` and offset by `pan`. Default zoom = 1, pan = 0 is the identity, so an
    // unbound preset is byte-unchanged. Applied post-projection so it moves the whole
    // attractor (position and apparent point size) as one.
    let zoom = draw.x.x;
    let pan = draw.x.yz;
    let ndc = vec2<f32>(world.x / aspect, world.y) * zoom + pan;

    // Per-particle colour through the shared LUT: the seeded jitter occupies the
    // band `hue_center + (seed - 0.5)*hue_spread` (was a hardcoded `seed*0.15`),
    // plus the shared `hue`; both LUTs crossfade by `palette_mix` before
    // `saturation`. `textureSampleLevel` (LOD 0) — vertex stage has no derivatives.
    //
    // The depth tint rides the same coordinate (ADR-0076). Taken from the
    // particle's own `dn` - for a segment that is the head's, not the midpoint's:
    // the colour follows where the particle IS, and the trail behind it already
    // carries the shade it had when it was there.
    //
    // The last-map channel rides it too (ADR-0087), by the same centred shift:
    // `map` names which sub-copy of the figure this point sits in, so on the
    // fern this is what makes stem, body and the two fronds separate colours.
    // It is identically 0 on every family but the IFS, where nothing writes it.
    //
    // ...and ADR-0088's root channel, which is what the age channel was trying
    // to be. `age` proxied distance-from-the-fixed-points and the proxy decayed
    // after ~10 steps, so `age_tint`/`age_hue` never produced a gradient and were
    // retired here (Plan 0074 Phase 3). This IS that distance, recomputed every
    // step, so it is permanent. Clamped at the READ rather than at the write: the
    // skeleton's diameter is not an upper bound on the attractor's reach, so a
    // stored value past 1 is a faithful measurement, and the palette coordinate
    // is where it has to become a unit.
    //
    // **ANCHORED AT ZERO, and deliberately not `channel_shift`** (ADR-0088's
    // Anchoring section). The other terms are centred because `map01` and
    // `depth01` genuinely span [0, 1], so their midpoint means *typical* and
    // raising the amount opens a spread about the preset's colour. `root01` does
    // NOT span [0, 1] - measured, it tops out at 0.41 on the spiral and 1.05 on
    // the dragon - so a centred shift would be negative almost everywhere and
    // would slide the figure as well as spread it. Zero is the anchor that is
    // both meaningful and exactly reachable here: it is the respawn state, a
    // particle sitting on a fixed point. So the contraction points keep the
    // preset's chosen colour and the figure ramps away from them.
    let map01 = map / MAP_SPAN;
    let root01 = clamp(root, 0.0, 1.0);
    let coord = hue + hue_center + (seed - 0.5) * hue_spread + depth_tint(dn)
        + channel_shift(map01, draw.ch.x)
        + draw.ch.z * root01;
    // Hard bands (ADR-0078). NO contour here and that is the honest scoping,
    // not an omission: this is the VERTEX stage, `fwidth` does not exist in it,
    // and a point sprite carries a single palette coordinate - so there is no
    // gradient across it for a contour to sit in. `palette_contour` is inert on
    // this scene and `presets/README.md` says so.
    let banded = band_coord(coord, draw.em.z);
    let ca = textureSampleLevel(lut_a, lut_samp, vec2<f32>(banded, 0.5), 0.0).rgb;
    let cb = textureSampleLevel(lut_b, lut_samp, vec2<f32>(banded, 0.5), 0.0).rgb;
    // ...and each channel's OTHER route, which shifts the hue of whatever colour
    // the palette produced instead of moving where it was sampled. Before
    // `apply_saturation`, so `saturation` stays the last word on colour.
    //
    // `root_hue` is anchored for the same reason `root_tint` is: a particle on a
    // fixed point takes NO rotation, and the figure rotates away from the colour
    // the ramp gave it. Centring would rotate the skeleton itself by
    // `-root_hue/2`, which is the slide the anchoring exists to avoid - and it
    // matters more on this route than on the tint one, because a hue rotation
    // has no `hue_center` to absorb it.
    let shifted = shift_hue(
        mix(ca, cb, clamp(palette_mix, 0.0, 1.0)),
        channel_shift(map01, draw.ch.y) + draw.ch.w * root01,
    );
    let col = apply_saturation(shifted, saturation);

    // Normalize the additive deposit by the particle count (ADR-0065), so total
    // light per frame is invariant to the tier, times the preset's `brightness`
    // (ADR-0080). Applied here rather than in the fragment shader because the draw
    // uniform is bound VERTEX-only; the fragment multiplies this by its own radial
    // falloff, and both are linear, so the result is identical to scaling the
    // emitted fragment.
    let deposit = draw.w.w;

    var out: VsOut;
    out.pos = vec4<f32>(ndc, 0.0, 1.0);
    out.local = local;
    out.half_len = half_len;
    // ...and the distance haze, which is the stand-in for the occlusion this
    // scene deliberately does not do (ADR-0076). Applied to the emitted light,
    // so the trail inherits the grading: a particle that was far and is now near
    // leaves a dim streak behind a bright head.
    //
    // Times the emergence ramp (ADR-0087), which is what makes the churn
    // invisible: a just-respawned particle sits on one of exactly four points, so
    // a thousand of them per frame would integrate into four bright dots in the
    // trail field. Ramped from zero it deposits almost nothing until it has been
    // iterated enough to have spread. Exactly 1.0 on every non-IFS family.
    let emergence = min(1.0, age * draw.em.x + draw.em.y);
    out.color = col * deposit * haze(dn) * emergence;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // Distance to the segment from (-half_len, 0) to (+half_len, 0), in units of
    // the point radius. At `half_len = 0` this is `length(in.local)` exactly —
    // the point's own radial falloff, unchanged, which is what lets the discrete
    // families keep byte-identical captures through one shader.
    let d = length(vec2<f32>(max(abs(in.local.x) - in.half_len, 0.0), in.local.y));
    let falloff = max(0.0, 1.0 - d);
    let g = falloff * falloff;
    return vec4<f32>(in.color * g, 1.0);
}
"#;

/// Decay pass: draw the previous accumulation back into the fresh target scaled
/// by the per-frame retention factor `k`, laying down the faded trail before the
/// new points are added on top.
pub(super) const DECAY_SHADER: &str = r#"
struct Decay {
    // x: per-frame retention factor, y: occlude (present pass only),
    // z: transform-active flag
    k:  vec4<f32>,
    // The ADR-0048 transform, exactly as `feedback::Transform::pack` returns it.
    xf: vec4<f32>,
    tr: vec4<f32>,
    wp: vec4<f32>,
}
@group(0) @binding(0) var prev: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var<uniform> decay: Decay;

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // The past, resampled through this frame's `fb_*` transform (ADR-0048) — the
    // second of the engine's two accumulation sinks, running the same
    // `lmv_source_uv` the trails stage does.
    //
    // **Only the past.** The fresh points are drawn additively over this bed in
    // the same render pass, at their own projected positions, so the transform
    // reaches the trail and never the head. `select` on a uniform flag, so an
    // untransformed preset samples the LITERAL `in.uv` and every attractor golden
    // is byte-identical.
    let moved = decay.k.z != 0.0;
    let suv = select(in.uv, lmv_source_uv(in.uv, decay.xf, decay.tr, decay.wp), moved);
    let inside = select(1.0, lmv_inside(suv), moved);
    let c = textureSampleLevel(prev, samp, suv, 0.0).rgb * (decay.k.x * inside);
    return vec4<f32>(c, 1.0);
}
"#;

/// Present pass: composite the accumulation field to the surface (linear sample,
/// stretched to fill; aspect ignored as in the reaction-diffusion present).
pub(super) const PRESENT_SHADER: &str = r#"
struct Decay { k: vec4<f32> } // x: retention (unread here), y: occlude

// FOUR entries for three resources, with the sampler bound twice. That is not an
// oversight: `occlude` needed a uniform in a pass that had none, and two
// bind-group layouts of the same shape mis-render when they coexist on the DX12
// WARP software adapter (ADR-0058, which is where that hazard is recorded) —
// measured on this very change, where a `[uniform]` group read the backdrop's
// buffer on WARP while working on hardware. All six three-entry arrangements of {texture, sampler, uniform} are
// already spoken for (`attractor-decay`, `ink`, `tonemap`, `bloom-up`,
// `bloom-bright`, and the trails present, which took the last one). A duplicate
// sampler binding is the cheapest way to a fourth shape — no second texture view,
// no new binding type. Pinned by
// `the_two_present_layouts_added_for_occlude_are_shapes_nothing_else_has`.
@group(0) @binding(0) var field: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var<uniform> u: Decay;
@group(0) @binding(3) var samp_unused: sampler;

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let c = textureSampleLevel(field, samp, in.uv, 0.0).rgb;
    // Alpha from the accumulated luminance so empty space (no points, no trail) is
    // transparent and reveals the bg_* backdrop (ADR-0026), while bright cloud cores
    // (luma -> 1) occlude it. The present pipeline blends premultiplied-OVER: `c` is
    // emitted as-is (added over the backdrop), so over the default black backdrop
    // this is byte-identical to the prior opaque present.
    // …and `occlude` scales that coverage — how much of the backdrop the cloud
    // holds out (ADR-0085). Reached only when no post stage is active; the chain
    // owns the seam otherwise and the renderer hands a literal 1.0 here.
    let a = clamp(dot(c, vec3<f32>(0.299, 0.587, 0.114)), 0.0, 1.0);
    return vec4<f32>(c, a * u.k.y);
}
"#;
