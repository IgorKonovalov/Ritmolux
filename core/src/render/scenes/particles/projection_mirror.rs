use super::AttractorFamily;

/// One projected particle: the pre-aspect "world" plane position, and the
/// view depth the rotation produces alongside it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Projected {
    pub(super) screen: [f32; 2],
    pub(super) depth: f32,
}

/// Mirrors `project()` in [`DRAW_SHADER`](super::DRAW_SHADER).
///
/// Takes `cs`/`sn` rather than an angle, exactly as the WGSL does — which is
/// also what lets a test state the mirror identity *exactly*: `cos` of an
/// `f32` `π` is not `−1` to the last bit, and the property ADR-0076 names is
/// about `cs = −1, sn = 0`.
pub(super) fn project(q: [f32; 3], family: AttractorFamily, cs: f32, sn: f32) -> Projected {
    let (_, dim, [cx, cy, cz]) = family.projection();
    let ([hx, hy, hz], [vx, vy, vz]) = family.basis().masks();
    let [qx, qy, qz] = q;
    let [px, py, pz] = [qx - cx, qy - cy, qz - cz];
    if dim < 2.5 {
        return Projected {
            screen: [px * cs - py * sn, px * sn + py * cs],
            depth: 0.0,
        };
    }
    let h = px * hx + py * hy + pz * hz;
    let v = px * vx + py * vy + pz * vz;
    Projected {
        screen: [px * cs + h * sn, v],
        depth: -px * sn + h * cs,
    }
}

/// Mirrors `depth_norm()` in [`DRAW_SHADER`](super::DRAW_SHADER).
pub(super) fn depth_norm(depth: f32, inv_extent: f32) -> f32 {
    (depth * inv_extent).clamp(-1.0, 1.0)
}

/// Mirrors `magnify()` in [`DRAW_SHADER`](super::DRAW_SHADER).
pub(super) fn magnify(dn: f32, perspective: f32) -> f32 {
    1.0 / (1.0 - perspective * dn)
}

/// Mirrors `depth01()` in [`DRAW_SHADER`](super::DRAW_SHADER).
pub(super) fn depth01(dn: f32) -> f32 {
    (dn + 1.0) * 0.5
}

/// Mirrors `haze()` in [`DRAW_SHADER`](super::DRAW_SHADER) — the per-particle
/// brightness multiplier.
///
/// This is where the fade is measurable at all. Which *screen region* holds
/// the far material depends on the spin phase, so a pixel-side assertion
/// would be measuring the clock; the multiplier is the thing the decision is
/// about.
pub(super) fn haze(dn: f32, depth_fade: f32) -> f32 {
    1.0 - depth_fade * (1.0 - depth01(dn))
}

/// Mirrors `depth_tint()` in [`DRAW_SHADER`](super::DRAW_SHADER) — the shift
/// added to the per-particle palette coordinate.
pub(super) fn depth_tint(dn: f32, depth_hue: f32) -> f32 {
    depth_hue * (depth01(dn) - 0.5)
}

/// Mirrors `channel_shift()` in [`DRAW_SHADER`](super::DRAW_SHADER) —
/// ADR-0087's centred contribution from a `[0, 1]` per-particle channel.
pub(super) fn channel_shift(unit: f32, amount: f32) -> f32 {
    amount * (unit - 0.5)
}

/// Mirrors the root channel's palette term in
/// [`DRAW_SHADER`](super::DRAW_SHADER) — ADR-0088's **anchored**
/// contribution, and deliberately not [`channel_shift`].
///
/// A separate function rather than a call with an offset, because the
/// difference between the two is the decision: `channel_shift` is centred on
/// the assumption that its channel spans `[0, 1]`, which `root01` does not.
/// Spelling them apart is what stops a later edit from "tidying" this into
/// the shared helper and silently reintroducing the slide.
pub(super) fn root_shift(unit: f32, amount: f32) -> f32 {
    amount * unit
}

/// Mirrors `rgb2hsv()` in [`DRAW_SHADER`](super::DRAW_SHADER).
///
/// The WGSL is branchless (two `mix`es driven by `step`); this spells the
/// same selects as the branches they are, which is the only difference. The
/// `1e-10` guards the two divisions on a greyscale or black input, where hue
/// is undefined and any value is as good as another.
pub(super) fn rgb2hsv(c: [f32; 3]) -> [f32; 3] {
    let [r, g, b] = c;
    // `p = mix(vec4(c.bg, k.wz), vec4(c.gb, k.xy), step(c.b, c.g))`, where
    // `step(edge, x)` is 1.0 when `x >= edge`.
    let (p0, p1, p2, p3) = if g >= b {
        (g, b, 0.0, -1.0 / 3.0)
    } else {
        (b, g, -1.0, 2.0 / 3.0)
    };
    // `q = mix(vec4(p.xyw, c.r), vec4(c.r, p.yzx), step(p.x, c.r))`.
    let (q0, q1, q2, q3) = if r >= p0 {
        (r, p1, p2, p0)
    } else {
        (p0, p1, p3, r)
    };
    let d = q0 - q1.min(q3);
    let e = 1.0e-10;
    [(q2 + (q3 - q1) / (6.0 * d + e)).abs(), d / (q0 + e), q0]
}

/// Mirrors `hsv2rgb()` in [`DRAW_SHADER`](super::DRAW_SHADER), which is
/// itself transcribed from `render/ink.rs::hsv2rgb`.
pub(super) fn hsv2rgb(c: [f32; 3]) -> [f32; 3] {
    let [hue, sat, val] = c;
    let h = hue - hue.floor();
    let mut out = [0.0f32; 3];
    for (slot, offset) in out.iter_mut().zip([0.0f32, 4.0, 2.0]) {
        let k = (h * 6.0 + offset) % 6.0;
        let ramp = ((k - 3.0).abs() - 1.0).clamp(0.0, 1.0);
        // `c.z * mix(vec3(1.0), rgb, c.y)`, multiplied out.
        *slot = val * (1.0 + (ramp - 1.0) * sat);
    }
    out
}

/// Mirrors `shift_hue()` in [`DRAW_SHADER`](super::DRAW_SHADER) — the
/// channels' second route, **including the exact-zero early return**.
///
/// That early return is load-bearing rather than an optimization: an
/// RGB → HSV → RGB round trip is not bit-exact, and sixteen golden baselines
/// assert that a preset binding none of these params renders byte-identically.
pub(super) fn shift_hue(c: [f32; 3], turns: f32) -> [f32; 3] {
    if turns == 0.0 {
        return c;
    }
    let mut hsv = rgb2hsv(c);
    if let Some(h) = hsv.first_mut() {
        *h += turns;
    }
    hsv2rgb(hsv)
}

/// The magnified world-space position of one particle — `project` composed
/// with the two above and the family's world scale, which is the composition
/// the vertex shader performs before the aspect division and the view
/// transform.
///
/// The sprite's own corner offset is left off: it is a fixed square about
/// this point, so it cannot affect whether two projections are mirror images.
pub(super) fn world(
    q: [f32; 3],
    family: AttractorFamily,
    cs: f32,
    sn: f32,
    perspective: f32,
) -> [f32; 2] {
    let (scl, _, _) = family.projection();
    let p = project(q, family, cs, sn);
    let m = magnify(depth_norm(p.depth, family.inv_depth_extent()), perspective);
    let [sx, sy] = p.screen;
    [sx * scl * m, sy * scl * m]
}
