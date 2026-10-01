//! The plexus point set and its proximity graph: pure CPU arithmetic, no GPU.
//!
//! A point set is seeded once per preset load and then advanced by `dt`; the
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
