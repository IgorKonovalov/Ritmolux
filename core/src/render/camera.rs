//! The shared 3D camera (ADR-0257): an orbit camera that projects world points
//! through a perspective lens, for every pipeline that draws a 3D primitive.
//!
//! Two halves, one function. [`CAMERA_WGSL`] is prepended to each 3D pipeline's
//! shader and projects on the GPU; [`CameraView`] is the same matrix on the CPU,
//! where a scene clips its primitives against the near plane and culls what
//! lies outside the frustum before anything is uploaded. The two are held equal
//! by a test that runs the WGSL `project()` in a compute pass and compares it
//! with [`CameraView::clip`] on a fixed set of points.
//!
//! # The space
//!
//! The camera orbits the world origin. `yaw` turns it about the world's `+y`
//! axis, `pitch` raises it above the `xz` plane, and `distance` is how far the
//! eye sits from the origin. At `yaw = 0, pitch = 0` the eye is on `+z` looking
//! down `-z`, with world `+x` to the right and `+y` up on screen.
//!
//! Clip space puts the view depth in `w`, so the perspective divide is a divide
//! by the distance in front of the eye along the view axis. The `z` row is zero:
//! no 3D pipeline has a depth attachment, because additive light needs no
//! occlusion (ADR-0044).
//!
//! # Depth of field is per primitive, not per pixel
//!
//! [`Lens::coc`] is the thin-lens circle of confusion at a view depth,
//! `aperture * |depth - focus| / depth`, in pixels and clamped to the tier's
//! cap. A pipeline evaluates it at each endpoint of what it draws and widens the
//! primitive by it, so one line can be sharp at the focal plane and soft at both
//! ends (ADR-0257). The additive scenes this serves have no single depth per
//! pixel for a post-process blur to read.
//!
//! # The framing controls keep their meaning
//!
//! The aspect is the **render target's**, handed in by the caller, never an
//! internal grid's (ADR-0037). The engine-wide `zoom` divides the field of
//! view, and `pan_x` / `pan_y` shift after the projection, in the same units
//! the 2D [`ViewTransform`](crate::render::scenes::lines::ViewTransform) pans
//! in: `pan_x` is divided by the aspect on the way to clip space, so one unit of
//! pan moves the picture the same number of pixels on both axes.

// Hot-path panic-denial pragma (the hygiene guard scans `render/`). A view is
// built once per frame and every primitive is projected through it.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

/// The WGSL half: the `Camera` uniform's shape and `project()`, prepended to
/// each 3D pipeline's own shader. It declares no binding.
pub(crate) const CAMERA_WGSL: &str = include_str!("camera.wgsl");

/// How close to the eye a primitive may reach before it is clipped, in world
/// units. A point nearer than this projects through a divide by nearly zero.
pub const NEAR: f32 = 0.05;

/// The widest field of view the lens opens to, in radians, after `zoom` has
/// divided it. Past about 170 degrees the tangent grows without bound and the
/// picture is all edge.
const MAX_FOV: f32 = 3.0;

/// The narrowest, in radians. `zoom` divides the field of view, so a large
/// `zoom` would otherwise reach a lens that magnifies without limit.
const MIN_FOV: f32 = 0.01;

/// The steepest `pitch` the orbit reaches, in radians, just short of straight
/// up or down. At a quarter turn the view axis is parallel to world `+y` and
/// the screen's right-hand direction is undefined.
const MAX_PITCH: f32 = 1.55;

/// An orbit camera, as a preset binds it (ADR-0257).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera3d {
    /// Turn about world `+y`, in radians. A preset writing `time * 0.05` here
    /// orbits slowly.
    pub yaw: f32,
    /// Elevation above the `xz` plane, in radians; clamped short of a quarter
    /// turn either way.
    pub pitch: f32,
    /// Eye to orbit target, in world units.
    pub distance: f32,
    /// The vertical field of view, in radians, before `zoom` divides it.
    pub fov: f32,
    /// Where the focal plane sits in the scene's volume: `0` at its nearest
    /// extent, `1` at its farthest. Normalized so an author never deals in world
    /// units; [`CameraView::focal_depth`] resolves it against the volume.
    pub focus: f32,
    /// The lens's aperture, as the circle of confusion in pixels a point would
    /// have at infinite depth. `0` is a pinhole and blurs nothing.
    pub aperture: f32,
}

impl Camera3d {
    /// The view this camera gives onto a render target of `aspect` (width over
    /// height), under the engine-wide `zoom` and `pan` (ADR-0018).
    ///
    /// Every input is taken as a raw bound value and made safe here, once: a
    /// non-finite or out-of-range yaw, pitch, distance, field of view, zoom or
    /// aspect resolves to the nearest usable camera rather than a NaN matrix.
    pub fn view(&self, aspect: f32, zoom: f32, pan: [f32; 2]) -> CameraView {
        let finite = |v: f32, fallback: f32| if v.is_finite() { v } else { fallback };
        let yaw = finite(self.yaw, 0.0);
        let pitch = finite(self.pitch, 0.0).clamp(-MAX_PITCH, MAX_PITCH);
        let distance = finite(self.distance, 1.0).max(NEAR * 2.0);
        let zoom = finite(zoom, 1.0).max(1e-3);
        let fov = (finite(self.fov, 1.0) / zoom).clamp(MIN_FOV, MAX_FOV);
        let aspect = finite(aspect, 1.0).max(0.1);
        let [pan_x, pan_y] = [finite(pan[0], 0.0), finite(pan[1], 0.0)];

        let (sy, cy) = yaw.sin_cos();
        let (sp, cp) = pitch.sin_cos();
        let eye = [distance * cp * sy, distance * sp, distance * cp * cy];
        // The view axis, from the eye toward the origin: `-eye / distance`,
        // written from the angles so it is unit length without a square root.
        let forward = [-cp * sy, -sp, -cp * cy];
        // Screen right: `forward x up`, which for up = +y is the horizontal
        // (cos yaw, 0, -sin yaw) whatever the pitch.
        let right = [cy, 0.0, -sy];
        // Screen up: `right x forward`, unit because the two are orthonormal.
        let up = cross(right, forward);

        let s = 1.0 / (0.5 * fov).tan();
        let sx = s / aspect;
        let (re, ue, fe) = (dot(right, eye), dot(up, eye), dot(forward, eye));
        // Rows of the world-to-clip matrix acting on (p, 1):
        //   clip.x = (right . (p - eye)) * s / aspect + (pan_x / aspect) * depth
        //   clip.y = (up    . (p - eye)) * s          +  pan_y           * depth
        //   clip.z = 0
        //   clip.w = forward . (p - eye)                            -- the depth
        // so the divide by `w` lands the pan as a constant shift in NDC.
        let px = pan_x / aspect;
        let row0 = [
            right[0] * sx + forward[0] * px,
            right[1] * sx + forward[1] * px,
            right[2] * sx + forward[2] * px,
            -re * sx - fe * px,
        ];
        let row1 = [
            up[0] * s + forward[0] * pan_y,
            up[1] * s + forward[1] * pan_y,
            up[2] * s + forward[2] * pan_y,
            -ue * s - fe * pan_y,
        ];
        let row3 = [forward[0], forward[1], forward[2], -fe];
        let rows = [row0, row1, [0.0; 4], row3];
        // Column-major, as WGSL's `mat4x4` is laid out.
        let mut view_proj = [[0.0f32; 4]; 4];
        for (c, column) in view_proj.iter_mut().enumerate() {
            for (r, cell) in column.iter_mut().enumerate() {
                *cell = rows
                    .get(r)
                    .and_then(|row| row.get(c))
                    .copied()
                    .unwrap_or(0.0);
            }
        }
        CameraView {
            view_proj,
            aspect,
            distance,
        }
    }
}

/// One frame's projection: the matrix [`CAMERA_WGSL`] multiplies by, and what
/// the CPU needs to clip against it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraView {
    /// World to clip, column-major.
    pub view_proj: [[f32; 4]; 4],
    /// The aspect the view was built for, after its clamp.
    pub aspect: f32,
    /// Eye to orbit target, after its clamp.
    pub distance: f32,
}

impl CameraView {
    /// `p` in clip space. **The CPU mirror of `project()` in [`CAMERA_WGSL`]**,
    /// the same four dot products in the same order: a column-major matrix times
    /// `(p, 1)`.
    pub fn clip(&self, p: [f32; 3]) -> [f32; 4] {
        let [c0, c1, c2, c3] = self.view_proj;
        let mut out = [0.0f32; 4];
        for (r, cell) in out.iter_mut().enumerate() {
            let at = |c: &[f32; 4]| c.get(r).copied().unwrap_or(0.0);
            *cell = at(&c0) * p[0] + at(&c1) * p[1] + at(&c2) * p[2] + at(&c3);
        }
        out
    }

    /// The view depth of the focal plane, for a volume of bounding radius
    /// `radius` about the orbit target and a normalized `focus`: `0` the
    /// volume's nearest extent, `1` its farthest. Never nearer than [`NEAR`].
    ///
    /// Written as `near + focus * (far - near)` and not as
    /// `near + 2 * focus * radius`: the two differ in the last bit, and a
    /// reference depth that moves by a bit moves every stroke width with it.
    pub fn focal_depth(&self, focus: f32, radius: f32) -> f32 {
        let near = self.distance - radius;
        let far = self.distance + radius;
        let focus = if focus.is_finite() { focus } else { 0.5 };
        (near + focus * (far - near)).max(NEAR)
    }

    /// The view depth of `p`: its distance in front of the eye along the view
    /// axis. Negative behind the eye.
    pub fn depth(&self, p: [f32; 3]) -> f32 {
        self.clip(p)[3]
    }

    /// The segment `a -> b` with any part nearer than [`NEAR`] cut away, or
    /// `None` when all of it is.
    ///
    /// **Clipped, not culled**: a segment crossing the near plane keeps the
    /// part in front of it, so an edge the camera orbits through shortens
    /// instead of blinking out. The cut is taken in world space, where depth is
    /// linear along the segment, so the new endpoint lies exactly on the plane.
    pub fn clip_near(&self, a: [f32; 3], b: [f32; 3]) -> Option<([f32; 3], [f32; 3])> {
        let (da, db) = (self.depth(a), self.depth(b));
        match (da >= NEAR, db >= NEAR) {
            (true, true) => Some((a, b)),
            (false, false) => None,
            (a_in, _) => {
                let t = (NEAR - da) / (db - da);
                let cut = lerp3(a, b, t);
                Some(if a_in { (a, cut) } else { (cut, b) })
            }
        }
    }

    /// Whether the segment `a -> b`, both ends at or past [`NEAR`], lies wholly
    /// outside one edge of the frame, allowing `margin` in normalized device
    /// units for the stroke's own width.
    ///
    /// Conservative on purpose: a segment both of whose ends are outside
    /// **different** edges may still cross the frame, and is kept.
    pub fn outside(&self, a: [f32; 3], b: [f32; 3], margin: f32) -> bool {
        let ndc = |p: [f32; 3]| {
            let c = self.clip(p);
            [c[0] / c[3], c[1] / c[3]]
        };
        let ([ax, ay], [bx, by]) = (ndc(a), ndc(b));
        let edge = 1.0 + margin;
        (ax > edge && bx > edge)
            || (ax < -edge && bx < -edge)
            || (ay > edge && by > edge)
            || (ay < -edge && by < -edge)
    }
}

/// A lens: the CPU half of `coc()` in [`CAMERA_WGSL`], with its inputs made
/// safe once.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Lens {
    /// Aperture in pixels, finite and non-negative.
    pub aperture: f32,
    /// The focal plane's view depth.
    pub focal_depth: f32,
    /// The largest circle of confusion drawn, in pixels — the tier's cap.
    pub max_coc: f32,
}

impl Lens {
    /// A lens with `aperture` and `max_coc` held finite and non-negative.
    pub fn new(aperture: f32, focal_depth: f32, max_coc: f32) -> Self {
        let safe = |v: f32| if v.is_finite() { v.max(0.0) } else { 0.0 };
        Self {
            aperture: safe(aperture),
            focal_depth: focal_depth.max(NEAR),
            max_coc: safe(max_coc),
        }
    }

    /// The circle of confusion at view depth `depth`, as a radius in pixels.
    /// **Mirrors `coc()` in [`CAMERA_WGSL`]**, term for term.
    pub fn coc(&self, depth: f32) -> f32 {
        let blur = self.aperture * (depth - self.focal_depth).abs() / depth;
        blur.clamp(0.0, self.max_coc)
    }
}

/// The `Camera` uniform [`CAMERA_WGSL`] declares, as it is uploaded.
///
/// **Field order is the WGSL struct's**, and both are `vec4`-aligned, so the
/// Rust layout is the uniform layout with no padding to keep in step.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    /// [`CameraView::view_proj`].
    pub view_proj: [[f32; 4]; 4],
    /// `[target width px, target height px, reference depth, unused]`.
    pub viewport: [f32; 4],
    /// `[aperture px, focal depth, max circle of confusion px, unused]`.
    pub lens: [f32; 4],
}

impl CameraUniform {
    /// The uniform for `view` on a `width` x `height` target through `lens`,
    /// with pixel widths stated at the lens's focal plane.
    pub fn new(view: &CameraView, width: u32, height: u32, lens: Lens) -> Self {
        Self {
            view_proj: view.view_proj,
            viewport: [
                width.max(1) as f32,
                height.max(1) as f32,
                lens.focal_depth,
                0.0,
            ],
            lens: [lens.aperture, lens.focal_depth, lens.max_coc, 0.0],
        }
    }
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

#[cfg(test)]
mod tests;
