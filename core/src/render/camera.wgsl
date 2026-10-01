// The shared 3D camera (ADR-0257): prepended to every pipeline that draws a
// primitive through `Camera3d`. It declares the uniform's shape and the maths,
// and no binding - each pipeline binds `Camera` at its own slot, so this text
// never decides a layout.
//
// Mirrored on the CPU by `render::camera`: `CameraView::clip` is `project()`
// below, term for term, and a test pins the two equal on a fixed set of points.

struct Camera {
    // World to clip, column-major. Row 3 is the view depth, so `clip.w` is the
    // distance in front of the eye along the view axis; row 2 is zero because
    // no pipeline here has a depth attachment. The engine-wide pan is folded in
    // after the projection, so it moves the picture and not the eye.
    view_proj: mat4x4<f32>,
    // x: render-target width in pixels, y: its height, z: the reference depth
    // a pixel width is stated at - the focal plane - w: unused.
    viewport: vec4<f32>,
    // x: aperture in pixels, y: the focal depth, z: the largest circle of
    // confusion the tier draws, in pixels, w: unused.
    lens: vec4<f32>,
}

// A world point to clip space. `w` is the view depth; the caller guarantees it
// is positive, since every primitive is clipped against the near plane on the
// CPU before it is uploaded.
fn project(cam: Camera, p: vec3<f32>) -> vec4<f32> {
    return cam.view_proj * vec4<f32>(p, 1.0);
}

// A clip-space point to pixels from the target's centre, y up. Isotropic: one
// unit is one pixel on both axes, so a length measured here is a length on
// screen at every orientation.
fn clip_to_px(cam: Camera, clip: vec4<f32>) -> vec2<f32> {
    return clip.xy / clip.w * (0.5 * cam.viewport.xy);
}

// Pixels from the target's centre back to normalized device coordinates.
fn px_to_ndc(cam: Camera, px: vec2<f32>) -> vec2<f32> {
    return px / (0.5 * cam.viewport.xy);
}

// The thin-lens circle of confusion at view depth `depth`, as a radius in
// pixels: `aperture * |depth - focus| / depth`, clamped to the tier's cap.
// Exactly 0 at aperture 0, and at the focal depth whatever the aperture.
fn coc(cam: Camera, depth: f32) -> f32 {
    let blur = cam.lens.x * abs(depth - cam.lens.y) / depth;
    return clamp(blur, 0.0, cam.lens.z);
}

// A length stated in pixels at the reference depth, carried to `depth` by
// perspective: twice as far is half as wide.
fn at_depth(cam: Camera, px: f32, depth: f32) -> f32 {
    return px * cam.viewport.z / depth;
}
