//! The plexus point set and its proximity graph: pure CPU arithmetic, no GPU.
//!
//! A point set — the `cloud`'s or the `sheet`'s — is seeded once per preset load and then advanced by `dt`; the
//! graph is rebuilt from it every frame. Both are pure functions of the seed,
//! the layout and the sequence of `(dt, params)` they were handed, so a capture
//! is reproducible and a determinism test can compare two runs exactly.

// Hot-path panic-denial pragma (the hygiene guard scans `render/`): `step` and
// `link` run every displayed frame.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use crate::render::scenes::{Phase, SeededRng};

/// Half the side of the `cloud` layout's cube, in world units. The camera
/// orbits its centre.
pub(crate) const CLOUD_HALF_EXTENT: f32 = 1.0;

/// How far inside a face of the cube a point starts to fade, in world units.
///
/// A point that drifts through a face reappears at the opposite one. Without a
/// fade that is a jump, and every edge attached to it would jump with it; with
/// one, the point has faded to nothing by the time it crosses, and fades back in
/// on the far side, so no edge ever pops.
pub(crate) const FACE_FADE: f32 = 0.2;

/// The spatial frequency of the drift flow, in radians per world unit: about
/// one full swirl across the cube.
const FLOW_FREQUENCY: f32 = 2.2;

/// The flow's speed at `drift = 1`, in world units a second.
const FLOW_SPEED: f32 = 0.5;

/// How fast the flow itself evolves, in radians of phase per second at
/// `drift = 1`: the currents turn over in a few seconds rather than standing
/// still while the points ride them. Applied where the phase is read, never
/// inside its sum (ADR-0135).
const FLOW_EVOLUTION: f32 = 0.35;

/// One edge of the proximity graph: the indices of its two points and how
/// present it is, `0..=1` before `link_alpha`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Edge {
    pub(crate) a: u32,
    pub(crate) b: u32,
    pub(crate) presence: f32,
}

/// The `cloud` layout: points in a cube, riding a seeded divergence-free flow
/// and wrapping at its faces.
#[derive(Debug, Clone)]
pub(crate) struct Cloud {
    /// Positions, world units, inside `[-CLOUD_HALF_EXTENT, CLOUD_HALF_EXTENT]^3`.
    pub(crate) pos: Vec<[f32; 3]>,
    /// Each point's face fade, `0..=1`, from its current position.
    pub(crate) fade: Vec<f32>,
    /// The flow's six seeded phase offsets.
    phases: [f32; 6],
    /// The flow's own clock, integrated at `drift` (ADR-0135): never the
    /// shared clock times a field.
    flow_phase: Phase,
}

impl Cloud {
    /// `count` points placed uniformly in the cube, and a flow, all drawn from
    /// `seed`. `capacity` is the tier's cap, reserved once so a later preset
    /// asking for more never grows the buffers mid-show.
    pub(crate) fn seeded(seed: u64, count: usize, capacity: usize) -> Self {
        let mut rng = SeededRng::new(seed);
        let mut phases = [0.0f32; 6];
        for phase in &mut phases {
            *phase = rng.range(0.0, std::f32::consts::TAU);
        }
        let mut pos = Vec::with_capacity(capacity.max(count));
        let mut fade = Vec::with_capacity(capacity.max(count));
        for _ in 0..count {
            let p = [
                rng.range(-CLOUD_HALF_EXTENT, CLOUD_HALF_EXTENT),
                rng.range(-CLOUD_HALF_EXTENT, CLOUD_HALF_EXTENT),
                rng.range(-CLOUD_HALF_EXTENT, CLOUD_HALF_EXTENT),
            ];
            pos.push(p);
            fade.push(face_fade(p));
        }
        Self {
            pos,
            fade,
            phases,
            flow_phase: Phase::default(),
        }
    }

    /// The radius of the sphere that bounds the layout, about the orbit target:
    /// what the depth of the volume is measured against.
    pub(crate) fn bounding_radius(&self) -> f32 {
        CLOUD_HALF_EXTENT * 3.0f32.sqrt()
    }

    /// Advance every point by `dt` seconds at `drift`.
    pub(crate) fn step(&mut self, dt: f32, drift: f32) {
        let drift = if drift.is_finite() {
            drift.max(0.0)
        } else {
            0.0
        };
        self.flow_phase.step(drift, dt);
        let step = drift * FLOW_SPEED * dt;
        let (t, ph) = (self.flow_phase.get() * FLOW_EVOLUTION, self.phases);
        for (p, fade) in self.pos.iter_mut().zip(self.fade.iter_mut()) {
            let v = flow(*p, t, &ph);
            for (x, dx) in p.iter_mut().zip(v) {
                *x = wrap(*x + dx * step);
            }
            *fade = face_fade(*p);
        }
    }
}

/// Half the side of the `sheet` layout's square, in world units, in the `xz`
/// plane. Wider than the cube, because a sheet seen at a grazing angle is
/// foreshortened to a band.
pub(crate) const SHEET_HALF_EXTENT: f32 = 1.6;

/// How far a sheet point is jittered off its grid cell's centre, as a fraction
/// of the cell. Under a half, so two neighbouring points never swap places and
/// the grid's order is the sheet's order.
const SHEET_JITTER: f32 = 0.35;

/// How far inside the sheet's rim its points fade out, in world units — the
/// sheet's counterpart of [`FACE_FADE`], so its edge is a fade into the dark
/// rather than a ruled line.
const RIM_FADE: f32 = 0.35;

/// How fast the height field's phase advances, in radians per second at
/// `drift = 1`. Applied where the phase is read (ADR-0135).
const WAVE_EVOLUTION: f32 = 0.9;

/// The fewest world units a wave's scale is read as, so a bound `wave_scale`
/// of zero cannot divide the field into noise.
const MIN_WAVE_SCALE: f32 = 0.05;

/// One component of the sheet's height field.
#[derive(Debug, Clone, Copy)]
struct Wave {
    /// Unit direction of travel in the `xz` plane.
    dir: [f32; 2],
    /// Spatial frequency, radians per world unit at `wave_scale = 1`.
    freq: f32,
    /// Seeded phase offset, radians.
    phase: f32,
    /// How fast this component travels relative to the shared phase.
    speed: f32,
    /// Its share of the amplitude.
    weight: f32,
}

/// The `sheet` layout: a seeded jittered grid on the `xz` plane, displaced
/// along its normal by a seeded smooth height field.
///
/// **Only the displacement moves.** Every point keeps its grid position in
/// `xz` for the life of the preset, so neighbours stay neighbours and the mesh
/// ripples rather than rewiring — while its edges still come from the
/// proximity rule, and still fade as a ripple stretches a pair apart.
#[derive(Debug, Clone)]
pub(crate) struct Sheet {
    /// Each point's fixed position in the plane, `(x, z)`.
    base: Vec<[f32; 2]>,
    /// Positions, world units: `(x, height, z)`.
    pub(crate) pos: Vec<[f32; 3]>,
    /// Each point's rim fade, `0..=1`; fixed, since the plane position is.
    pub(crate) fade: Vec<f32>,
    waves: [Wave; 3],
    /// The height field's own clock, integrated at `drift` (ADR-0135).
    phase: Phase,
}

impl Sheet {
    /// `count` points on a jittered grid and a height field, all drawn from
    /// `seed`, with buffers reserved for `capacity`.
    pub(crate) fn seeded(seed: u64, count: usize, capacity: usize) -> Self {
        let mut rng = SeededRng::new(seed);
        let weights = [1.0, 0.6, 0.35];
        let mut waves = [Wave {
            dir: [1.0, 0.0],
            freq: 1.0,
            phase: 0.0,
            speed: 1.0,
            weight: 1.0,
        }; 3];
        for (k, (wave, weight)) in waves.iter_mut().zip(weights).enumerate() {
            let angle = rng.range(0.0, std::f32::consts::TAU);
            *wave = Wave {
                dir: [angle.cos(), angle.sin()],
                // Each component a little finer than the last, so the field
                // reads as a swell with chop on it rather than one ripple.
                freq: rng.range(1.2, 1.8) * (1.0 + k as f32 * 0.9),
                phase: rng.range(0.0, std::f32::consts::TAU),
                speed: rng.range(0.6, 1.4),
                weight,
            };
        }
        let cols = (count as f32).sqrt().ceil().max(1.0) as usize;
        let rows = count.div_ceil(cols).max(1);
        let (cell_x, cell_z) = (
            2.0 * SHEET_HALF_EXTENT / cols as f32,
            2.0 * SHEET_HALF_EXTENT / rows as f32,
        );
        let mut base = Vec::with_capacity(capacity.max(count));
        let mut fade = Vec::with_capacity(capacity.max(count));
        for i in 0..count {
            let (c, r) = ((i % cols) as f32, (i / cols) as f32);
            let x = -SHEET_HALF_EXTENT + (c + 0.5 + SHEET_JITTER * rng.range(-1.0, 1.0)) * cell_x;
            let z = -SHEET_HALF_EXTENT + (r + 0.5 + SHEET_JITTER * rng.range(-1.0, 1.0)) * cell_z;
            base.push([x, z]);
            fade.push(rim_fade(x, z));
        }
        let pos = base.iter().map(|&[x, z]| [x, 0.0, z]).collect();
        Self {
            base,
            pos,
            fade,
            waves,
            phase: Phase::default(),
        }
    }

    /// The radius of the sphere that bounds the layout about the orbit target:
    /// the square's half-diagonal. The height field is a small addition to it
    /// and is left out, so the volume `focus` is measured in does not breathe
    /// with `wave`.
    pub(crate) fn bounding_radius(&self) -> f32 {
        SHEET_HALF_EXTENT * std::f32::consts::SQRT_2
    }

    /// Advance the height field by `dt` seconds at `drift`, and displace every
    /// point by it at amplitude `wave` and spatial scale `wave_scale`.
    pub(crate) fn step(&mut self, dt: f32, drift: f32, wave: f32, wave_scale: f32) {
        let finite = |v: f32, fallback: f32| if v.is_finite() { v } else { fallback };
        self.phase.step(finite(drift, 0.0).max(0.0), dt);
        let t = self.phase.get() * WAVE_EVOLUTION;
        let amplitude = finite(wave, 0.0).max(0.0);
        let scale = finite(wave_scale, 1.0).max(MIN_WAVE_SCALE);
        for (p, &[x, z]) in self.pos.iter_mut().zip(&self.base) {
            p[1] = amplitude * height(&self.waves, x, z, t, scale);
        }
    }
}

/// The unit-amplitude height field at `(x, z)`: the weighted mean of the
/// seeded travelling sinusoids, in `-1..=1`.
fn height(waves: &[Wave; 3], x: f32, z: f32, t: f32, scale: f32) -> f32 {
    let mut sum = 0.0;
    let mut total = 0.0;
    for w in waves {
        let along = (w.dir[0] * x + w.dir[1] * z) * w.freq / scale;
        sum += w.weight * (along + w.phase + w.speed * t).sin();
        total += w.weight;
    }
    sum / total
}

/// How present a sheet point at `(x, z)` is: `1` inside, falling smoothly to
/// `0` at the rim over [`RIM_FADE`].
fn rim_fade(x: f32, z: f32) -> f32 {
    smoothstep((SHEET_HALF_EXTENT - x.abs()) / RIM_FADE)
        * smoothstep((SHEET_HALF_EXTENT - z.abs()) / RIM_FADE)
}

/// Either layout's point set, as the scene holds it.
#[derive(Debug, Clone)]
pub(crate) enum Points {
    Cloud(Cloud),
    Sheet(Sheet),
}

impl Points {
    /// `count` points of `layout`, seeded from `seed`, with buffers reserved
    /// for `capacity`.
    pub(crate) fn seeded(
        layout: super::PlexusLayout,
        seed: u64,
        count: usize,
        capacity: usize,
    ) -> Self {
        match layout {
            super::PlexusLayout::Cloud => Points::Cloud(Cloud::seeded(seed, count, capacity)),
            super::PlexusLayout::Sheet => Points::Sheet(Sheet::seeded(seed, count, capacity)),
        }
    }

    pub(crate) fn pos(&self) -> &[[f32; 3]] {
        match self {
            Points::Cloud(c) => &c.pos,
            Points::Sheet(s) => &s.pos,
        }
    }

    pub(crate) fn fade(&self) -> &[f32] {
        match self {
            Points::Cloud(c) => &c.fade,
            Points::Sheet(s) => &s.fade,
        }
    }

    pub(crate) fn bounding_radius(&self) -> f32 {
        match self {
            Points::Cloud(c) => c.bounding_radius(),
            Points::Sheet(s) => s.bounding_radius(),
        }
    }

    /// Advance by `dt`. The cloud reads only `drift`; the sheet reads all three.
    pub(crate) fn step(&mut self, dt: f32, drift: f32, wave: f32, wave_scale: f32) {
        match self {
            Points::Cloud(c) => c.step(dt, drift),
            Points::Sheet(s) => s.step(dt, drift, wave, wave_scale),
        }
    }
}

/// The drift velocity at `p`: an Arnold–Beltrami–Childress flow with seeded
/// phases, `0..=2` in each component before [`FLOW_SPEED`].
///
/// Each component depends only on the other two coordinates, so the flow is
/// **divergence-free**: it carries the points around without bunching them
/// into sinks or tearing holes, which is what keeps the density — and so the
/// number of links — steady over a long run.
fn flow(p: [f32; 3], t: f32, ph: &[f32; 6]) -> [f32; 3] {
    let k = FLOW_FREQUENCY;
    let [x, y, z] = p;
    [
        (k * z + ph[0] + t).sin() + (k * y + ph[1] - 0.7 * t).cos(),
        (k * x + ph[2] + 0.8 * t).sin() + (k * z + ph[3] - t).cos(),
        (k * y + ph[4] - 0.9 * t).sin() + (k * x + ph[5] + 1.1 * t).cos(),
    ]
}

/// `x` wrapped into the cube's extent along one axis.
fn wrap(x: f32) -> f32 {
    let side = 2.0 * CLOUD_HALF_EXTENT;
    if x > CLOUD_HALF_EXTENT {
        x - side
    } else if x < -CLOUD_HALF_EXTENT {
        x + side
    } else {
        x
    }
}

/// How present a point at `p` is: `1` in the interior, falling smoothly to `0`
/// at every face over [`FACE_FADE`].
pub(crate) fn face_fade(p: [f32; 3]) -> f32 {
    p.iter()
        .map(|x| smoothstep((CLOUD_HALF_EXTENT - x.abs()) / FACE_FADE))
        .product()
}

/// `t` clamped to `0..=1` and eased: `t^2 (3 - 2t)`. Zero slope at both ends,
/// which is what makes a fade's arrival and departure read as one motion.
pub(crate) fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// How present the edge between two points `d` apart is, at link distance
/// `link`: `smoothstep(1 - d / link)`, and exactly `0` at and past `link`.
///
/// **Continuous in `d`**, so an edge fades in as its pair nears and out as it
/// parts, and never pops.
pub(crate) fn link_presence(d: f32, link: f32) -> f32 {
    if link <= 0.0 || d >= link {
        return 0.0;
    }
    smoothstep(1.0 - d / link)
}

/// Every pair of points closer than `link`, with its presence, into `edges`
/// (cleared first) — at most `cap` of them, in index order.
///
/// **Brute force, on purpose.** Every pair is tested, `N (N - 1) / 2` distance
/// checks: at the tier's point cap of a few hundred that is under a hundred
/// thousand subtractions a frame, well inside the budget, and a uniform grid
/// would pay its own bookkeeping to save it. The squared distance is compared
/// first so the square root is only taken for a pair that links.
///
/// Returns how many linking pairs were found, which exceeds `edges.len()` when
/// the cap truncated the graph.
pub(crate) fn link(
    pos: &[[f32; 3]],
    fade: &[f32],
    link: f32,
    cap: usize,
    edges: &mut Vec<Edge>,
) -> usize {
    edges.clear();
    let link = if link.is_finite() { link.max(0.0) } else { 0.0 };
    let link2 = link * link;
    let mut found = 0usize;
    for (i, (pi, fi)) in pos.iter().zip(fade).enumerate() {
        if *fi <= 0.0 {
            continue;
        }
        let rest_pos = pos.get(i + 1..).unwrap_or(&[]);
        let rest_fade = fade.get(i + 1..).unwrap_or(&[]);
        for (j, (pj, fj)) in rest_pos.iter().zip(rest_fade).enumerate() {
            let dx = pj[0] - pi[0];
            let dy = pj[1] - pi[1];
            let dz = pj[2] - pi[2];
            let d2 = dx * dx + dy * dy + dz * dz;
            if d2 >= link2 || *fj <= 0.0 {
                continue;
            }
            let presence = link_presence(d2.sqrt(), link) * fi * fj;
            if presence <= 0.0 {
                continue;
            }
            found += 1;
            if edges.len() < cap {
                edges.push(Edge {
                    a: i as u32,
                    b: (i + 1 + j) as u32,
                    presence,
                });
            }
        }
    }
    found
}
