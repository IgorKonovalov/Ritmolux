//! The shared 3D camera (ADR-0257): an orbit camera that projects world points
//! through a perspective lens, for every pipeline that draws a 3D primitive.
//!
//! Two halves, one function. `CAMERA_WGSL` is prepended to each 3D pipeline's
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
//! no 3D pipeline has a depth attachment. Additive light needs no occlusion
//! (ADR-0044), and a `solid` frame occludes by drawing its strokes far to near
//! rather than by a depth test (ADR-0263).
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

use crate::render::scenes::{
    CapOverflow, OverflowContext, ParamGroup, ParamKind, ParamSpec, default_of,
};

/// The WGSL half: the `Camera` uniform's shape and `project()`, prepended to
/// each 3D pipeline's own shader. It declares no binding.
pub(crate) const CAMERA_WGSL: &str = include_str!("camera.wgsl");

/// How close to the eye a primitive may reach before it is clipped, in world
/// units. A point nearer than this projects through a divide by nearly zero.
pub const NEAR: f32 = 0.05;

/// The widest field of view the lens opens to, in radians, after `zoom` has
/// divided it. Past about 170 degrees the tangent grows without bound and the
/// picture is all edge.
pub(crate) const MAX_FOV: f32 = 3.0;

/// The narrowest, in radians. `zoom` divides the field of view, so a large
/// `zoom` would otherwise reach a lens that magnifies without limit.
pub(crate) const MIN_FOV: f32 = 0.01;

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

/// One frame's projection: the matrix `CAMERA_WGSL` multiplies by, and what
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
    /// `p` in clip space. **The CPU mirror of `project()` in `CAMERA_WGSL`**,
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

/// A lens: the CPU half of `coc()` in `CAMERA_WGSL`, with its inputs made
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
    /// **Mirrors `coc()` in `CAMERA_WGSL`**, term for term.
    pub fn coc(&self, depth: f32) -> f32 {
        let blur = self.aperture * (depth - self.focal_depth).abs() / depth;
        blur.clamp(0.0, self.max_coc)
    }
}

/// The `Camera` uniform `CAMERA_WGSL` declares, as it is uploaded.
///
/// **Field order is the WGSL struct's**, and both are `vec4`-aligned, so the
/// Rust layout is the uniform layout with no padding to keep in step.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    /// [`CameraView::view_proj`].
    pub view_proj: [[f32; 4]; 4],
    /// `[target width px, target height px, reference depth, volume's nearest
    /// view depth]`.
    pub viewport: [f32; 4],
    /// `[aperture px, focal depth, max circle of confusion px, volume's depth
    /// span]`.
    pub lens: [f32; 4],
}

impl CameraUniform {
    /// The uniform for `view` on a `width` x `height` target through `lens`,
    /// with pixel widths stated at the lens's focal plane. The volume is
    /// unset, so `volume_depth()` in `CAMERA_WGSL` reads `0` everywhere until
    /// [`with_volume`](Self::with_volume) names one.
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

    /// This uniform with the scene's volume spanning view depths
    /// `near_extent` to `near_extent + span`: the 0-nearest, 1-farthest scale
    /// `focus` is stated on, which `fog` reads (ADR-0263).
    pub fn with_volume(mut self, near_extent: f32, span: f32) -> Self {
        self.viewport[3] = near_extent;
        self.lens[3] = span;
        self
    }
}

/// How far past the frame a primitive may lie before it is culled, in
/// normalized device units — room for the stroke's own width, so a line whose
/// centre has left the frame does not take its visible edge with it.
pub(crate) const CULL_MARGIN: f32 = 0.05;

/// `yaw`, shared: the orbit's turn about the volume.
pub(crate) const YAW: ParamSpec = ParamSpec {
    name: "yaw",
    default: 0.0,
    range: Some([-std::f32::consts::PI, std::f32::consts::PI]),
    doc: "Turns the camera around the scene's volume, in radians; bind it to a slow clock to orbit.",
    kind: ParamKind::Modal,
    group: ParamGroup::Motion,
    main: false,
};

/// `pitch`, shared: the orbit's elevation.
pub(crate) const PITCH: ParamSpec = ParamSpec {
    name: "pitch",
    default: 0.25,
    range: Some([-MAX_PITCH, MAX_PITCH]),
    doc: "Raises the camera above the scene's volume, in radians; negative looks up from below.",
    kind: ParamKind::Modal,
    group: ParamGroup::Motion,
    main: false,
};

/// `distance`, shared: eye to orbit target.
pub(crate) const DISTANCE: ParamSpec = ParamSpec {
    name: "distance",
    default: 3.5,
    range: Some([1.5, 8.0]),
    doc: "How far the camera sits from the centre of the scene's volume; nearer exaggerates the perspective.",
    kind: ParamKind::Modal,
    group: ParamGroup::Motion,
    main: false,
};

/// `fov`, shared: the vertical field of view.
pub(crate) const FOV: ParamSpec = ParamSpec {
    name: "fov",
    default: 0.8,
    range: Some([0.2, 2.0]),
    doc: "The camera's vertical field of view in radians; zoom divides it.",
    kind: ParamKind::Modal,
    group: ParamGroup::Motion,
    main: false,
};

/// `focus`, shared: the focal plane, normalized across the volume's depth.
pub(crate) const FOCUS: ParamSpec = ParamSpec {
    name: "focus",
    default: 0.5,
    range: Some([0.0, 1.0]),
    doc: "Where the focal plane sits in the depth of the scene's volume: 0 at its nearest point, 1 at its farthest.",
    kind: ParamKind::Modal,
    group: ParamGroup::Light,
    main: true,
};

/// `aperture`, shared: the far field's circle of confusion, in pixels. Its
/// range tops out at the highest tier's cap; a lower tier draws at its own cap
/// and announces it.
pub(crate) const APERTURE: ParamSpec = ParamSpec {
    name: "aperture",
    default: 0.0,
    range: Some([0.0, crate::render::TierConfig::RICH.max_coc_px as f32]),
    doc: "The blur of the far background, in pixels; strokes nearer than the focal plane blur more, up to the tier's cap. 0 keeps every stroke sharp, and wider costs fill.",
    kind: ParamKind::Modal,
    group: ParamGroup::Light,
    main: true,
};

/// `fog`, shared: light falling toward black with depth across the volume
/// (ADR-0263). `0` is off, and every stroke keeps the light it had.
pub(crate) const FOG: ParamSpec = ParamSpec {
    name: "fog",
    default: 0.0,
    range: Some([0.0, 1.0]),
    doc: "Fades strokes toward black with depth: at 1 the farthest point of the scene's volume is black and the nearest keeps its light. 0 is off.",
    kind: ParamKind::Modal,
    group: ParamGroup::Light,
    main: false,
};

/// `solid`, shared: whether the frame's strokes occlude (ADR-0263). Read per
/// frame, and solid at [`SOLID_AT`] and above; `0` is the additive glow.
pub(crate) const SOLID: ParamSpec = ParamSpec {
    name: "solid",
    default: 0.0,
    range: Some([0.0, 1.0]),
    doc: "1 paints near strokes over far ones, so the scene reads as an object and crossings stop brightening; 0 is the additive glow. Solid sorts every stroke by depth each frame.",
    kind: ParamKind::Modal,
    group: ParamGroup::Light,
    main: false,
};

/// The `solid` value at and above which a frame is drawn solid: the switch is
/// two-valued, and the threshold sits between its two values so a bound
/// `select(...)` or an eased value flips once, half-way.
pub(crate) const SOLID_AT: f32 = 0.5;

/// The camera params a 3D system binds, as one block (ADR-0258), in the
/// shape of [`PanParams`](crate::render::scenes::common::PanParams): the
/// `ParamSpec`s declared once here and spliced into each system's `PARAMS`,
/// and a setter each scene's `set_param` delegates to.
///
/// **Nothing here clamps.** [`Camera3d::view`] and [`Lens::new`] make every
/// raw bound value safe where it is read.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CameraParams {
    /// `yaw`, in radians.
    pub yaw: f32,
    /// `pitch`, in radians.
    pub pitch: f32,
    /// `distance`, in world units.
    pub distance: f32,
    /// `fov`, in radians.
    pub fov: f32,
    /// `focus`, `0` the volume's nearest extent and `1` its farthest.
    pub focus: f32,
    /// `aperture`, in pixels.
    pub aperture: f32,
    /// `fog`, `0` off and `1` the farthest extent black.
    pub fog: f32,
    /// `solid`, solid at [`SOLID_AT`] and above.
    pub solid: f32,
}

impl Default for CameraParams {
    fn default() -> Self {
        let rest = |name| default_of(&Self::SPECS, name);
        Self {
            yaw: rest("yaw"),
            pitch: rest("pitch"),
            distance: rest("distance"),
            fov: rest("fov"),
            focus: rest("focus"),
            aperture: rest("aperture"),
            fog: rest("fog"),
            solid: rest("solid"),
        }
    }
}

/// One frame of the shared lens logic, for a scene to clip, cull and draw by.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CameraFrame {
    /// The projection, for clipping and culling on the CPU.
    pub view: CameraView,
    /// The uniform every 3D pipeline binds.
    pub uniform: CameraUniform,
    /// The cull margin, in normalized device units, widened by the blur cap
    /// when the lens blurs at all.
    pub margin: f32,
    /// The view depth of the volume's nearest extent.
    pub near_extent: f32,
    /// The view depth from the volume's nearest extent to its farthest.
    pub span: f32,
    /// The blur clamp to announce when `aperture` is past the tier's cap.
    pub blur: Option<CapOverflow>,
    /// `fog`, made safe: finite and in `[0, 1]`.
    pub fog: f32,
    /// Whether this frame is drawn solid: near strokes painted over far ones
    /// in a back-to-front order rather than added (ADR-0263).
    pub solid: bool,
}

impl CameraFrame {
    /// Where view depth `depth` lies across the volume: `0` at its nearest
    /// extent, `1` at its farthest, clamped. **Mirrors `volume_depth()` in
    /// `CAMERA_WGSL`**, so a scene that colours or fogs on the CPU reads the
    /// scale the `seg3d` pipeline fogs on.
    pub fn volume_depth(&self, depth: f32) -> f32 {
        if self.span > 0.0 {
            ((depth - self.near_extent) / self.span).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    /// The share of its light a point at view depth `depth` keeps under this
    /// frame's `fog`: `1 - fog * volume_depth`. **Mirrors `fog_light()` in
    /// `CAMERA_WGSL`**, and is exactly `1.0` at `fog = 0`.
    pub fn fog_light(&self, depth: f32) -> f32 {
        (1.0 - self.fog * self.volume_depth(depth)).clamp(0.0, 1.0)
    }
}

impl CameraParams {
    /// The specs, in the order a system splices them.
    pub(crate) const SPECS: [ParamSpec; 8] =
        [YAW, PITCH, DISTANCE, FOV, FOCUS, APERTURE, FOG, SOLID];

    /// Store `value` if `name` is one of the camera's, and say whether it
    /// was.
    pub(crate) fn set(&mut self, name: &str, value: f32) -> bool {
        match name {
            "yaw" => self.yaw = value,
            "pitch" => self.pitch = value,
            "distance" => self.distance = value,
            "fov" => self.fov = value,
            "focus" => self.focus = value,
            "aperture" => self.aperture = value,
            "fog" => self.fog = value,
            "solid" => self.solid = value,
            _ => return false,
        }
        true
    }

    /// Back to the declared defaults.
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }

    /// The orbit camera these params describe.
    pub(crate) fn camera(&self) -> Camera3d {
        Camera3d {
            yaw: self.yaw,
            pitch: self.pitch,
            distance: self.distance,
            fov: self.fov,
            focus: self.focus,
            aperture: self.aperture,
        }
    }

    /// This frame's view, lens and cull margin onto a `target` of pixels at
    /// `aspect` (the render target's, ADR-0037), for a volume of bounding
    /// `radius` about the orbit target, with blur held to the tier's `max_coc`.
    ///
    /// The aperture is the far field's blur, the one the lens approaches and
    /// never passes behind focus; past the cap that is what draws shallower,
    /// and [`CameraFrame::blur`] carries the clamp. The near side is unbounded
    /// and saturates at the cap by design, so it is never judged (ADR-0257).
    pub(crate) fn frame(
        &self,
        aspect: f32,
        zoom: f32,
        pan: [f32; 2],
        target: (u32, u32),
        radius: f32,
        max_coc: f32,
    ) -> CameraFrame {
        let camera = self.camera();
        let view = camera.view(aspect, zoom, pan);
        let near_extent = view.distance - radius;
        let far_extent = view.distance + radius;
        let lens = Lens::new(
            camera.aperture,
            view.focal_depth(camera.focus, radius),
            max_coc,
        );
        let blur = (lens.aperture > lens.max_coc).then(|| CapOverflow {
            dropped: 0,
            context: OverflowContext::Blur(lens.aperture.ceil() as u32),
            cap: lens.max_coc as usize,
        });
        // A blurred stroke reaches past its centreline by up to the cap, so
        // the cull keeps that much more of the frame's surround.
        let margin = if lens.aperture > 0.0 {
            CULL_MARGIN + 2.0 * lens.max_coc / target.1.max(1) as f32
        } else {
            CULL_MARGIN
        };
        let span = far_extent - near_extent;
        let fog = if self.fog.is_finite() {
            self.fog.clamp(0.0, 1.0)
        } else {
            0.0
        };
        CameraFrame {
            view,
            uniform: CameraUniform::new(&view, target.0, target.1, lens)
                .with_volume(near_extent, span),
            margin,
            near_extent,
            span,
            blur,
            fog,
            // `NaN >= 0.5` is false, so a non-finite binding draws the glow.
            solid: self.solid >= SOLID_AT,
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
