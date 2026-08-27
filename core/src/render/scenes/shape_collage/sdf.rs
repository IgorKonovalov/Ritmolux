//! **The element roster's signed distance functions** (Plan 0113 Phases 1 and 7).
//!
//! Eight kinds, as one WGSL chunk spliced into the painter's shader — the same
//! arrangement [`marks`](super::super::marks) uses, and for the same reason: the
//! chunk declares no bindings and no entry points, so splicing it in changes
//! nothing about the pipeline's layout.
//!
//! | kind | `p0` | `p1` | half extents mean |
//! |---|---|---|---|
//! | `quad` 0 | — | — | half width, half height |
//! | `circle` 1 | — | — | the two semi-axes (an ellipse) |
//! | `triangle` 2 | — | — | half width, half height of the equilateral figure |
//! | `bar` 3 | — | — | half **length**, and the stroke's **radius** |
//! | `ring` 4 | — | — | outer **radius**, and the annulus **thickness** |
//! | `segment` 5 | half-aperture / π | — | **radius** (`hy` unused) |
//! | `arc` 6 | half-aperture / π | — | outer **radius**, and the **thickness** |
//! | `checker` 7 | — | cells per axis | half width, half height |
//!
//! # The circle family is circular, not elliptical, and that is a decision
//!
//! `ring`, `segment` and `arc` take a **radius** from `hx` and ignore `hy` as a
//! shape (it is their thickness instead). An elliptical annulus or sector has no
//! closed-form distance and — worse for this scene — no closed-form *rotated
//! bounding box*, and a loose box is a silent cost regression against Plan 0113
//! Phase 2's measurement. A circular one has both, exactly. The references
//! (ADR-0123's reading of *On White II*) want concentric rings and arcs, which
//! are circular anyway.
//!
//! # `checker`'s distance is approximate, and it is the only one that is
//!
//! A checkerboard is not one connected figure, so what this returns is the
//! distance to the **nearest cell boundary**, signed by cell parity, intersected
//! with the patch's own box. That is exact in sign and exact in magnitude within
//! a cell of the boundary — which is all the one-pixel coverage ramp reads — and
//! wrong further in, where nothing looks. Every other kind here is an exact
//! Euclidean distance.
//!
//! The cell count is forced **even** (see `checker_cells`) so both diagonals'
//! corner cells are filled and the patch's bounding box is the box it claims.

// Hot-path panic-denial pragma, as everywhere under `scenes/`. Nothing here but
// a string constant today, and the pragma stays anyway: the guard's value is
// that it is unconditional, and a helper added to this file later would
// otherwise arrive unprotected.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

/// The distance functions, as WGSL. Spliced ahead of the painter's own body.
pub(crate) fn wgsl() -> &'static str {
    SDF_WGSL
}

const SDF_WGSL: &str = r#"
// One flat element, 64 bytes. **The array is the painter's order, so the index
// IS the depth.** Declared here rather than in the painter's own body because
// `element_distance` below takes one — the struct travels with the functions
// that read it, and a chunk that referred forward to it would not parse.
struct Element {
    // cx, cy, half_x, half_y — canvas space
    center_size: vec4<f32>,
    // cos(angle), sin(angle), kind, p0 (kind-specific)
    shape: vec4<f32>,
    // palette coordinate, alpha, birth, p1 (kind-specific)
    tint: vec4<f32>,
    // x0, y0, x1, y1 — the precomputed TIGHT reject box, canvas space
    aabb: vec4<f32>,
}

// Exact signed distance to an axis-aligned box of half extents h.
fn sd_box(p: vec2<f32>, h: vec2<f32>) -> f32 {
    let q = abs(p) - h;
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0);
}

// An ellipse's distance, approximated by the unit-circle distance scaled back by
// the smaller half axis. **Exact when hx == hy**, which is the circle case the
// aspect test measures; elliptical elements get a distance correct in sign and
// slightly conservative in magnitude, which moves an edge by well under the one
// pixel the coverage ramp spans. The exact ellipse distance is iterative and
// this runs per pixel per element.
fn sd_ellipse(p: vec2<f32>, h: vec2<f32>) -> f32 {
    return (length(p / h) - 1.0) * min(h.x, h.y);
}

// Exact signed distance to the triangle with the three given vertices
// (Inigo Quilez's `sdTriangle`). Exact rather than a unit-space approximation
// because the vertices are also what the CPU-side bounding box is built from,
// so the box is tight by construction.
fn sd_triangle(p: vec2<f32>, v0: vec2<f32>, v1: vec2<f32>, v2: vec2<f32>) -> f32 {
    let e0 = v1 - v0;
    let e1 = v2 - v1;
    let e2 = v0 - v2;
    let w0 = p - v0;
    let w1 = p - v1;
    let w2 = p - v2;
    let q0 = w0 - e0 * clamp(dot(w0, e0) / dot(e0, e0), 0.0, 1.0);
    let q1 = w1 - e1 * clamp(dot(w1, e1) / dot(e1, e1), 0.0, 1.0);
    let q2 = w2 - e2 * clamp(dot(w2, e2) / dot(e2, e2), 0.0, 1.0);
    let s = sign(e0.x * e2.y - e0.y * e2.x);
    var d = min(
        vec2<f32>(dot(q0, q0), s * (w0.x * e0.y - w0.y * e0.x)),
        vec2<f32>(dot(q1, q1), s * (w1.x * e1.y - w1.y * e1.x)),
    );
    d = min(d, vec2<f32>(dot(q2, q2), s * (w2.x * e2.y - w2.y * e2.x)));
    return -sqrt(d.x) * sign(d.y);
}

// Exact distance to a capsule along x: a segment of half-length `hx - r`
// swept by a disc of radius `r`. The element's total half extent along x is
// therefore `hx`, which is what keeps `bar` interchangeable with `quad` in a
// layout that does not care which it drew.
fn sd_bar(p: vec2<f32>, h: vec2<f32>) -> f32 {
    let r = min(h.y, h.x);
    let half = max(h.x - r, 0.0);
    let q = vec2<f32>(max(abs(p.x) - half, 0.0), p.y);
    return length(q) - r;
}

// Exact distance to an annulus: outer radius h.x, thickness h.y.
fn sd_ring(p: vec2<f32>, h: vec2<f32>) -> f32 {
    let t = min(h.y, h.x);
    // The band is centred on the circle of radius `h.x - t/2`, so the OUTER
    // boundary sits exactly at h.x — the radius the bounding box is built from.
    return abs(length(p) - (h.x - t * 0.5)) - t * 0.5;
}

// Exact distance to a circular sector of radius h.x, opening along +x with
// half-aperture `a` radians (Inigo Quilez's `sdPie`, rotated a quarter turn so
// the aperture is centred on +x like every other kind's long axis).
fn sd_segment(p: vec2<f32>, h: vec2<f32>, a: f32) -> f32 {
    // Into sdPie's frame, which opens along +y.
    let q = vec2<f32>(abs(p.y), p.x);
    let c = vec2<f32>(sin(a), cos(a));
    let l = length(q) - h.x;
    let m = length(q - c * clamp(dot(q, c), 0.0, h.x));
    return max(l, m * sign(c.y * q.x - c.x * q.y));
}

// Exact distance to an annular sector — an arc of thickness h.y on the circle of
// outer radius h.x, opening along +x with half-aperture `a`. The intersection of
// a ring and a sector, which for these two convex-in-the-radial-sense figures is
// the max of their distances.
fn sd_arc(p: vec2<f32>, h: vec2<f32>, a: f32) -> f32 {
    return max(sd_ring(p, h), sd_segment(p, vec2<f32>(h.x * 2.0, h.y), a));
}

// A checkerboard patch: `cells` squares per axis over a box of half extents h.
// **Approximate** — see the module docs. The sign is the cell parity and the
// magnitude is the distance to the nearest cell edge, which is what the coverage
// ramp reads; further inside a cell it is not a Euclidean distance and nothing
// looks there.
fn sd_checker(p: vec2<f32>, h: vec2<f32>, cells: f32) -> f32 {
    let n = max(cells, 2.0);
    let size = 2.0 * h / n;
    let g = (p + h) / size;
    let cell = floor(g);
    let f = g - cell;
    let d = min(min(f.x, 1.0 - f.x) * size.x, min(f.y, 1.0 - f.y) * size.y);
    let filled = abs(cell.x + cell.y - 2.0 * floor((cell.x + cell.y) * 0.5)) < 0.5;
    let inside = select(d, -d, filled);
    return max(sd_box(p, h), inside);
}

// One element's signed distance at canvas point p.
fn element_distance(e: Element, p: vec2<f32>) -> f32 {
    let h = max(e.center_size.zw, vec2<f32>(1e-5));
    let ca = e.shape.x;
    let sa = e.shape.y;
    let d = p - e.center_size.xy;
    // Into the element's own frame: rotate by -angle, with the pair the CPU
    // precomputed (see the scene's module docs).
    let q = vec2<f32>(ca * d.x + sa * d.y, -sa * d.x + ca * d.y);
    let kind = e.shape.z;
    let p0 = e.shape.w;
    let p1 = e.tint.w;
    if (kind < 0.5) {
        return sd_box(q, h);
    }
    if (kind < 1.5) {
        return sd_ellipse(q, h);
    }
    if (kind < 2.5) {
        let s3 = 0.8660254;
        return sd_triangle(
            q,
            vec2<f32>(0.0, h.y),
            vec2<f32>(-s3 * h.x, -0.5 * h.y),
            vec2<f32>(s3 * h.x, -0.5 * h.y),
        );
    }
    if (kind < 3.5) {
        return sd_bar(q, h);
    }
    if (kind < 4.5) {
        return sd_ring(q, h);
    }
    if (kind < 5.5) {
        return sd_segment(q, h, p0);
    }
    if (kind < 6.5) {
        return sd_arc(q, h, p0);
    }
    return sd_checker(q, h, p1);
}
"#;
