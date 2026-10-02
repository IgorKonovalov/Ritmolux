// Tests index fixed arrays and panic on failure; allowed over the file's
// hot-path pragma — this is not the render path.
#![allow(clippy::indexing_slicing, clippy::panic, clippy::expect_used)]

use super::{
    CAMERA_WGSL, CULL_MARGIN, Camera3d, CameraParams, CameraUniform, CameraView, Lens, NEAR,
};
use crate::render::RenderError;
use crate::render::context::RenderContext;
use crate::render::scenes::{CapOverflow, OverflowContext};

/// The camera most tests look through: level, straight down `-z`, the origin
/// four units away.
fn level(fov: f32) -> Camera3d {
    Camera3d {
        yaw: 0.0,
        pitch: 0.0,
        distance: 4.0,
        fov,
        focus: 0.5,
        aperture: 0.0,
    }
}

fn ndc(view: &CameraView, p: [f32; 3]) -> [f32; 2] {
    let c = view.clip(p);
    [c[0] / c[3], c[1] / c[3]]
}

/// The points the mirror is pinned on: the orbit target, the corners of a box
/// around it, points off every axis and one well behind the target. Chosen to
/// exercise every column of the matrix, not to look like a scene.
const MIRROR_POINTS: [[f32; 3]; 12] = [
    [0.0, 0.0, 0.0],
    [1.0, 1.0, 1.0],
    [-1.0, 1.0, -1.0],
    [1.0, -1.0, -1.0],
    [-1.0, -1.0, 1.0],
    [0.7, 0.0, 0.0],
    [0.0, -0.4, 0.0],
    [0.0, 0.0, 1.9],
    [0.25, 0.5, -3.0],
    [-2.0, 0.3, 0.6],
    [0.01, 0.02, 0.03],
    [1.5, -1.25, 0.75],
];

/// The cameras the mirror is pinned under: an off-axis orbit with every framing
/// control engaged, a steep one, and the level default.
fn mirror_views() -> [CameraView; 3] {
    [
        Camera3d {
            yaw: 0.6,
            pitch: 0.35,
            distance: 3.5,
            fov: 0.8,
            focus: 0.5,
            aperture: 0.0,
        }
        .view(1.6, 1.3, [0.12, -0.07]),
        Camera3d {
            yaw: -2.2,
            pitch: -1.2,
            distance: 6.0,
            fov: 1.4,
            focus: 0.5,
            aperture: 0.0,
        }
        .view(0.75, 0.6, [0.0, 0.2]),
        level(0.9).view(16.0 / 9.0, 1.0, [0.0, 0.0]),
    ]
}

/// **The WGSL `project()` and [`CameraView::clip`] agree** — the camera exists
/// twice (ADR-0257) and this is what holds the two copies together.
///
/// Runs the snippet itself, prepended exactly as a pipeline prepends it, in a
/// compute pass over [`MIRROR_POINTS`] under each of [`mirror_views`], and
/// compares every component. The tolerance is a relative float epsilon: the GPU
/// may fuse a multiply-add the CPU rounds twice, and nothing else may differ.
///
/// Needs a GPU adapter, so it skips on a runner without one (ADR-0016).
#[test]
fn the_wgsl_projection_is_the_cpu_projection() {
    let ctx = match RenderContext::new_headless(16, 16, true) {
        Ok(ctx) => ctx,
        Err(RenderError::RequestAdapter(_)) => {
            eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
            return;
        }
        Err(e) => panic!("headless context build failed: {e}"),
    };
    for (v, view) in mirror_views().iter().enumerate() {
        let uniform = CameraUniform::new(view, 1920, 1080, Lens::new(12.0, 4.0, 20.0));
        let inputs: Vec<[f32; 4]> = MIRROR_POINTS
            .iter()
            .map(|p| [p[0], p[1], p[2], 1.0])
            .collect();
        let gpu = eval_on_gpu(&ctx, &uniform, &inputs, "project(cam, input[i].xyz)");
        for (p, (point, got)) in MIRROR_POINTS.iter().zip(&gpu).enumerate() {
            let want = view.clip(*point);
            for axis in 0..4 {
                let tol = 1e-5 * want[axis].abs().max(1.0);
                assert!(
                    (got[axis] - want[axis]).abs() <= tol,
                    "view {v}, point {p} {point:?}, component {axis}: WGSL {} vs CPU {}",
                    got[axis],
                    want[axis]
                );
            }
        }
    }
}

/// **The WGSL `coc()` and [`Lens::coc`] agree**, and neither ever exceeds the
/// tier's cap — at any aperture, including absurd ones, and at any depth in
/// front of the eye.
///
/// Needs a GPU adapter, so it skips on a runner without one (ADR-0016).
#[test]
fn the_circle_of_confusion_is_mirrored_and_never_exceeds_its_cap() {
    let ctx = match RenderContext::new_headless(16, 16, true) {
        Ok(ctx) => ctx,
        Err(RenderError::RequestAdapter(_)) => {
            eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
            return;
        }
        Err(e) => panic!("headless context build failed: {e}"),
    };
    let view = level(0.8).view(1.6, 1.0, [0.0, 0.0]);
    let depths: Vec<f32> = (0..64).map(|k| NEAR + k as f32 * 0.25).collect();
    let inputs: Vec<[f32; 4]> = depths.iter().map(|d| [*d, 0.0, 0.0, 0.0]).collect();
    for aperture in [0.0, 1.0, 12.0, 40.0, 1.0e6] {
        for focal in [NEAR, 1.0, 4.0, 9.0] {
            let lens = Lens::new(aperture, focal, 12.0);
            let uniform = CameraUniform::new(&view, 1920, 1080, lens);
            let gpu = eval_on_gpu(&ctx, &uniform, &inputs, "vec4<f32>(coc(cam, input[i].x))");
            for (depth, got) in depths.iter().zip(&gpu) {
                let want = lens.coc(*depth);
                assert!(
                    (got[0] - want).abs() <= 1e-4 * want.max(1.0),
                    "aperture {aperture}, focus {focal}, depth {depth}: WGSL {} vs CPU {want}",
                    got[0]
                );
                assert!(
                    want <= 12.0 && got[0] <= 12.0,
                    "past the cap: {want} / {}",
                    got[0]
                );
                assert!(want >= 0.0, "a negative circle at depth {depth}");
            }
            if aperture == 0.0 {
                assert!(gpu.iter().all(|c| c[0] == 0.0), "a pinhole blurs nothing");
            }
        }
    }
    // Exactly sharp at the focal plane, whatever the aperture.
    assert_eq!(Lens::new(30.0, 4.0, 12.0).coc(4.0), 0.0);
}

/// `expr`, a WGSL expression of `cam` and `input[i]` evaluating to a `vec4`,
/// run on the GPU once per input with [`CAMERA_WGSL`] prepended exactly as a
/// pipeline prepends it.
fn eval_on_gpu(
    ctx: &RenderContext,
    uniform: &CameraUniform,
    points: &[[f32; 4]],
    expr: &str,
) -> Vec<[f32; 4]> {
    use wgpu::util::DeviceExt;

    let device = &ctx.device;
    let source = format!(
        "{CAMERA_WGSL}
@group(0) @binding(0) var<uniform> cam: Camera;
@group(0) @binding(1) var<storage, read> input: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read_write> projected: array<vec4<f32>>;

@compute @workgroup_size(1)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {{
    let i = id.x;
    projected[i] = {expr};
}}
"
    );
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("camera-mirror-shader"),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    let uniform_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("camera-mirror-uniform"),
        contents: bytemuck::bytes_of(uniform),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let points_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("camera-mirror-points"),
        contents: bytemuck::cast_slice(points),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let bytes = std::mem::size_of_val(points) as u64;
    let out_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("camera-mirror-out"),
        size: bytes,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let read_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("camera-mirror-read"),
        size: bytes,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("camera-mirror-layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("camera-mirror-bind-group"),
        layout: &layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: points_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: out_buf.as_entire_binding(),
            },
        ],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("camera-mirror-pipeline-layout"),
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("camera-mirror-pipeline"),
        layout: Some(&pipeline_layout),
        module: &module,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("camera-mirror-encoder"),
    });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("camera-mirror-pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(points.len() as u32, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&out_buf, 0, &read_buf, 0, bytes);
    ctx.queue.submit([encoder.finish()]);

    let slice = read_buf.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |res| {
        let _ = tx.send(res);
    });
    device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            // Bounded: a test readback has no frame deadline, but a hung
            // device should fail the test rather than hang the suite.
            timeout: Some(std::time::Duration::from_secs(30)),
        })
        .expect("poll the mirror readback");
    rx.recv()
        .expect("map callback ran")
        .expect("map the mirror readback");
    let out: Vec<[f32; 4]> =
        bytemuck::cast_slice(&slice.get_mapped_range().expect("mapped")).to_vec();
    read_buf.unmap();
    out
}

/// A point on the camera's axis — the orbit target — lands on the centre of
/// the target **at every aspect**, and a view taken off-axis agrees.
#[test]
fn the_orbit_target_projects_to_the_target_centre_at_every_aspect() {
    for aspect in [0.5, 1.0, 1.6, 16.0 / 9.0, 2.4] {
        for cam in [
            level(0.8),
            Camera3d {
                yaw: 1.1,
                pitch: -0.4,
                distance: 2.5,
                fov: 1.2,
                focus: 0.5,
                aperture: 0.0,
            },
        ] {
            let [x, y] = ndc(&cam.view(aspect, 1.0, [0.0, 0.0]), [0.0, 0.0, 0.0]);
            assert!(
                x.abs() < 1e-6 && y.abs() < 1e-6,
                "aspect {aspect}, {cam:?}: the target landed at ({x}, {y})"
            );
        }
    }
}

/// **A unit square at the orbit distance keeps its aspect on screen** — at
/// 1920x1080 and at 1280x800, the configuration where an internal grid's
/// aspect and the target's disagree (ADR-0037).
///
/// The square faces the camera through the orbit target, so it spans the same
/// number of pixels horizontally as vertically only if the projection takes
/// its aspect from the target it is handed.
#[test]
fn a_square_at_the_orbit_distance_stays_square_on_screen() {
    for (w, h) in [(1920u32, 1080u32), (1280, 800)] {
        let aspect = w as f32 / h as f32;
        let view = level(0.8).view(aspect, 1.0, [0.0, 0.0]);
        let px = |p: [f32; 3]| {
            let [x, y] = ndc(&view, p);
            [x * w as f32 * 0.5, y * h as f32 * 0.5]
        };
        let lo = px([-0.5, -0.5, 0.0]);
        let hi = px([0.5, 0.5, 0.0]);
        let (width_px, height_px) = (hi[0] - lo[0], hi[1] - lo[1]);
        assert!(width_px > 1.0, "the square has to be visible at {w}x{h}");
        assert!(
            (width_px / height_px - 1.0).abs() < 1e-4,
            "{w}x{h}: the square is {width_px} x {height_px} px"
        );
    }
}

/// `pan_x` / `pan_y` shift the picture by the same amount at **every depth**,
/// and one unit of `pan_x` moves as many pixels as one of `pan_y` — the 2D
/// view transform's meaning, carried into the 3D camera.
#[test]
fn pan_shifts_after_the_projection() {
    let aspect = 1.6;
    let still = level(0.8).view(aspect, 1.0, [0.0, 0.0]);
    let panned = level(0.8).view(aspect, 1.0, [0.2, -0.1]);
    for p in [[0.0, 0.0, 0.0], [0.5, 0.2, 1.5], [-0.3, 0.4, -2.0]] {
        let [x0, y0] = ndc(&still, p);
        let [x1, y1] = ndc(&panned, p);
        assert!(
            ((x1 - x0) - 0.2 / aspect).abs() < 1e-5,
            "{p:?}: x moved {}",
            x1 - x0
        );
        assert!(((y1 - y0) + 0.1).abs() < 1e-5, "{p:?}: y moved {}", y1 - y0);
    }
}

/// `zoom` divides the field of view, so a point off the axis moves outward
/// and the target stays put.
#[test]
fn zoom_narrows_the_field_of_view() {
    let near = level(0.8).view(1.0, 1.0, [0.0, 0.0]);
    let zoomed = level(0.8).view(1.0, 2.0, [0.0, 0.0]);
    let p = [0.3, 0.2, 0.0];
    let (a, b) = (ndc(&near, p), ndc(&zoomed, p));
    assert!(b[0] > a[0] * 1.9 && b[1] > a[1] * 1.9, "{a:?} -> {b:?}");
    assert_eq!(ndc(&zoomed, [0.0; 3]), [0.0, 0.0]);
}

/// A segment crossing the near plane is **clipped, not culled**: the part in
/// front of the eye survives and its new end lies on the plane. One wholly
/// behind the eye is dropped, and one wholly in front is untouched.
#[test]
fn a_segment_through_the_near_plane_is_clipped_not_culled() {
    let view = level(0.8).view(1.0, 1.0, [0.0, 0.0]);
    // The eye is at z = 4 looking down -z: depth is 4 - z.
    let front = [0.1, 0.0, 0.0];
    let behind = [0.1, 0.0, 5.0];
    let (a, b) = view
        .clip_near(front, behind)
        .expect("the part in front of the eye is kept");
    assert_eq!(a, front);
    assert!(
        (view.depth(b) - NEAR).abs() < 1e-5,
        "the cut lands on the plane"
    );

    let (a, b) = view
        .clip_near(behind, front)
        .expect("the order of the ends does not matter");
    assert!((view.depth(a) - NEAR).abs() < 1e-5);
    assert_eq!(b, front);

    assert_eq!(view.clip_near([0.0, 0.0, 4.5], [0.3, 0.0, 6.0]), None);
    assert_eq!(
        view.clip_near(front, [0.0, 0.2, -1.0]),
        Some((front, [0.0, 0.2, -1.0]))
    );
}

/// Frustum culling drops a segment beyond one edge and keeps one that crosses
/// the frame with both ends outside it.
#[test]
fn a_segment_crossing_the_frame_is_not_culled() {
    let view = level(0.8).view(1.0, 1.0, [0.0, 0.0]);
    assert!(view.outside([5.0, 0.0, 0.0], [6.0, 1.0, 0.0], 0.0));
    assert!(!view.outside([-5.0, 0.0, 0.0], [5.0, 0.0, 0.0], 0.0));
    assert!(!view.outside([0.0, 0.0, 0.0], [0.2, 0.1, 0.0], 0.0));
}

/// A non-finite input resolves to a usable camera rather than a NaN matrix:
/// the parameters are bound expressions and nothing upstream has clamped them.
#[test]
fn a_non_finite_camera_still_projects() {
    let cam = Camera3d {
        yaw: f32::NAN,
        pitch: f32::INFINITY,
        distance: f32::NAN,
        fov: f32::NEG_INFINITY,
        focus: f32::NAN,
        aperture: f32::NAN,
    };
    let view = cam.view(f32::NAN, f32::NAN, [f32::NAN, 0.0]);
    for column in view.view_proj {
        for cell in column {
            assert!(cell.is_finite(), "{:?}", view.view_proj);
        }
    }
}

/// **The block answers exactly its six specs**, rests at their defaults, and
/// `reset` returns there.
#[test]
fn the_camera_block_answers_exactly_its_specs() {
    let mut params = CameraParams::default();
    for spec in CameraParams::SPECS {
        assert!(params.set(spec.name, 0.75), "`{}` is dropped", spec.name);
    }
    assert_eq!(
        params,
        CameraParams {
            yaw: 0.75,
            pitch: 0.75,
            distance: 0.75,
            fov: 0.75,
            focus: 0.75,
            aperture: 0.75,
        }
    );
    for name in ["zoom", "pan_x", "line_width", "", "Yaw"] {
        assert!(!params.set(name, 0.0), "`{name}` is not the camera's");
    }
    params.reset();
    let rest = CameraParams::default();
    for (spec, value) in CameraParams::SPECS.iter().zip([
        rest.yaw,
        rest.pitch,
        rest.distance,
        rest.fov,
        rest.focus,
        rest.aperture,
    ]) {
        assert_eq!(spec.default, value, "`{}` rests off its spec", spec.name);
    }
    assert_eq!(params, rest);
}

/// **The helper resolves the lens as each piece does on its own**: the view
/// and uniform are `Camera3d::view` and `CameraUniform::new` through
/// `Lens::new`, the margin widens only when the lens blurs, and the blur
/// clamp is reported only past the cap.
#[test]
fn the_lens_helper_is_the_pieces_composed() {
    let params = CameraParams {
        yaw: 0.4,
        pitch: 0.2,
        distance: 3.0,
        fov: 0.9,
        focus: 0.3,
        aperture: 10.0,
    };
    let (target, radius, max_coc) = ((1280, 800), 1.2, 16.0);
    let frame = params.frame(1.6, 1.25, [0.1, -0.05], target, radius, max_coc);
    let view = params.camera().view(1.6, 1.25, [0.1, -0.05]);
    let lens = Lens::new(10.0, view.focal_depth(0.3, radius), max_coc);
    assert_eq!(frame.view, view);
    assert_eq!(
        frame.uniform,
        CameraUniform::new(&view, target.0, target.1, lens)
    );
    assert_eq!(frame.near_extent, view.distance - radius);
    assert_eq!(
        frame.span,
        (view.distance + radius) - (view.distance - radius)
    );
    assert_eq!(frame.margin, CULL_MARGIN + 2.0 * max_coc / 800.0);
    assert_eq!(frame.blur, None);

    let sharp = CameraParams {
        aperture: 0.0,
        ..params
    };
    let frame = sharp.frame(1.6, 1.25, [0.1, -0.05], target, radius, max_coc);
    assert_eq!(frame.margin, CULL_MARGIN);
    assert_eq!(frame.blur, None);

    let wide = CameraParams {
        aperture: 40.5,
        ..params
    };
    let frame = wide.frame(1.6, 1.25, [0.1, -0.05], target, radius, max_coc);
    assert_eq!(
        frame.blur,
        Some(CapOverflow {
            dropped: 0,
            context: OverflowContext::Blur(41),
            cap: 16,
        })
    );
}
