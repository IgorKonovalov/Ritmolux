//! The mesh grid: its bounds, the per-vertex coordinate every program is
//! evaluated against, and the CPU-side state a frame's vertex buffer is
//! assembled from.
//!
//! No `wgpu` beyond the [`Vertex`](super::shaders::Vertex) it fills — this is
//! the arithmetic half of the scene, and it is the half the renderer also calls
//! (it sizes its per-vertex scratch off [`vertex_count`] and evaluates a
//! preset's bindings at [`vertex_position`]).

// Hot-path panic-denial pragma (Plan 0002 Phase 2; render/ is scanned by the
// hygiene guard).
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

// A continuation of one module split across five files, so it needs the names
// `warp_mesh/mod.rs` has in scope.
use super::*;

/// The smallest grid a `[mesh]` table may name, in cells. Below two the mesh is
/// a single quad and the per-vertex program has no interior to interpolate.
pub const MIN_MESH: u32 = 2;

/// The largest grid **any** tier may name, in cells — the `.milk` format's own
/// ceiling (`meshx <= 128`, `meshy <= 96`), so a converted preset's requested
/// grid is always representable.
pub const MAX_MESH: (u32, u32) = (128, 96);

/// The grid a `[mesh]` table's absent keys mean. Coarse enough to be free on any
/// machine and fine enough that a `rad`-driven program reads as a curve rather
/// than as facets.
pub const DEFAULT_MESH: (u32, u32) = (32, 24);

/// Clamp a preset's requested grid into what `tier` will carry.
///
/// **The one place the tier clamp happens.** Two consumers need the same answer
/// — the scene, which builds the vertex and index buffers, and the renderer,
/// which sizes the per-vertex evaluation scratch — and if they disagreed the
/// renderer would hand the scene a series of the wrong length every frame. Pure,
/// so both can call it and a test can hold them to the same value.
pub fn clamp_grid(requested: (u32, u32), tier: &crate::render::TierConfig) -> (u32, u32) {
    (
        requested
            .0
            .clamp(MIN_MESH, tier.mesh_grid.0.clamp(MIN_MESH, MAX_MESH.0)),
        requested
            .1
            .clamp(MIN_MESH, tier.mesh_grid.1.clamp(MIN_MESH, MAX_MESH.1)),
    )
}

/// How many vertices a grid of `mesh` cells has. One more than the cell count on
/// each axis — the fencepost the whole per-vertex path is sized by.
pub fn vertex_count(mesh: (u32, u32)) -> usize {
    (mesh.0 as usize + 1) * (mesh.1 as usize + 1)
}

/// The `(x, y, rad, ang)` a vertex's `[per_vertex]` bindings are evaluated
/// against, for the vertex at column `col`, row `row` of a `mesh` grid, on a
/// render target of aspect `aspect`.
///
/// `x` and `y` are the vertex's uv in `0..=1`, with `y = 0` at the **top** —
/// texture space, the space every sampler in this file addresses.
///
/// `rad` is the distance from the mesh centre and `ang` the angle there, both in
/// the **aspect-corrected** space of the render target (ADR-0037): `rad` reaches
/// `1.0` at the middle of the top and bottom edges on any display, and further
/// than that at the sides of a wide one. So a program written as
/// `zoom = 1 + rad * 0.2` makes a circular figure on a 16:9 monitor and the same
/// circular figure on a 5:4 one, which is the property the ADR exists for and the
/// reason this takes `aspect` rather than deriving one from `mesh`.
///
/// `ang` is in `0..tau`, measured counter-clockwise from the +x axis in *screen*
/// terms (y is flipped on the way in, so a positive angle turns the way an author
/// looking at the screen expects).
pub fn vertex_position(col: u32, row: u32, mesh: (u32, u32), aspect: f32) -> (f32, f32, f32, f32) {
    let x = col as f32 / mesh.0.max(1) as f32;
    let y = row as f32 / mesh.1.max(1) as f32;
    // Aspect correction on the *x* axis, so one unit of `rad` is one half-height
    // whatever the target's shape. A non-finite or non-positive aspect degrades
    // to square rather than poisoning every vertex with a NaN.
    let aspect = if aspect.is_finite() && aspect > 0.0 {
        aspect
    } else {
        1.0
    };
    let px = (x - 0.5) * 2.0 * aspect;
    // Flip y so +y is up, which is what makes `ang` read the way it looks.
    let py = (0.5 - y) * 2.0;
    let rad = (px * px + py * py).sqrt();
    let mut ang = py.atan2(px);
    if ang < 0.0 {
        ang += std::f32::consts::TAU;
    }
    (x, y, rad, ang)
}

/// The per-frame values the CPU assembles a vertex buffer from.
pub(super) struct MeshState {
    /// The clamped grid this state is sized for.
    pub(super) mesh: (u32, u32),
    /// One value per vertex for each of the nine outputs. Only the entries whose
    /// `bound` flag is set this frame are read; the rest fall back to the scalar
    /// param, which is what makes a `[per_vertex]` binding an override.
    pub(super) values: [Vec<f32>; OUTPUTS],
    pub(super) bound: [bool; OUTPUTS],
    /// The assembled vertex buffer, resized only when the grid changes.
    pub(super) vertices: Vec<Vertex>,
}

impl MeshState {
    pub(super) fn new(mesh: (u32, u32)) -> Self {
        let n = vertex_count(mesh);
        Self {
            mesh,
            values: std::array::from_fn(|_| vec![0.0; n]),
            bound: [false; OUTPUTS],
            vertices: vec![
                Vertex {
                    clip: [0.0; 2],
                    t0: [0.0; 4],
                    t1: [0.0; 4],
                    t2: [0.0; 4],
                };
                n
            ],
        }
    }

    /// Resize to `mesh` if it differs — off the hot path (a preset switch), so
    /// the allocation here is not a per-frame one.
    pub(super) fn resize(&mut self, mesh: (u32, u32)) {
        if self.mesh == mesh {
            return;
        }
        *self = Self::new(mesh);
    }

    /// Fill `out` with this frame's vertices. `scalars` supplies the fallback for
    /// every output with no `[per_vertex]` binding this frame.
    pub(super) fn assemble(&mut self, scalars: &[f32; OUTPUTS]) {
        let (mx, my) = self.mesh;
        let mut v = 0usize;
        for row in 0..=my {
            for col in 0..=mx {
                let clip = [
                    (col as f32 / mx.max(1) as f32) * 2.0 - 1.0,
                    1.0 - (row as f32 / my.max(1) as f32) * 2.0,
                ];
                let mut out = [0.0f32; OUTPUTS];
                for (i, slot) in out.iter_mut().enumerate() {
                    *slot = match (self.bound.get(i), self.values.get(i)) {
                        (Some(true), Some(series)) => series.get(v).copied().unwrap_or(0.0),
                        _ => scalars.get(i).copied().unwrap_or(0.0),
                    };
                }
                if let Some(slot) = self.vertices.get_mut(v) {
                    *slot = Vertex {
                        clip,
                        t0: [out[0], out[1], out[2], out[3]],
                        t1: [out[4], out[5], out[6], out[7]],
                        t2: [out[8], 0.0, 0.0, 0.0],
                    };
                }
                v += 1;
            }
        }
    }
}

/// The triangle indices for a `mesh` grid, two triangles per cell.
pub(super) fn build_indices(mesh: (u32, u32)) -> Vec<u32> {
    let (mx, my) = mesh;
    let stride = mx + 1;
    let mut out = Vec::with_capacity((mx as usize) * (my as usize) * 6);
    for row in 0..my {
        for col in 0..mx {
            let a = row * stride + col;
            let b = a + 1;
            let c = a + stride;
            let d = c + 1;
            out.extend_from_slice(&[a, c, b, b, c, d]);
        }
    }
    out
}
