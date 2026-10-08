//! The voxel system's contracts: the clock against the frame rate, the march's
//! ray against the camera it is recovered from, and a seeded volume reaching
//! the screen.
//!
//! **Software adapter**, like the rest of the GPU suites (ADR-0016); a test
//! needing one skips on a runner without it.
// Test asserts panic freely; this is not the render path.
#![allow(
    clippy::panic,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used
)]

use super::*;
use crate::render::capture;
use crate::render::context::{RenderContext, RenderError};

// ---------------------------------------------------------------------------
// The harness
// ---------------------------------------------------------------------------

/// A headless context, or `None` on a runner with no adapter.
fn context() -> Option<RenderContext> {
    match RenderContext::new_headless(64, 64, true) {
        Ok(ctx) => Some(ctx),
        Err(RenderError::RequestAdapter(_)) => {
            eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
            None
        }
        Err(e) => panic!("headless context build failed: {e}"),
    }
}

/// A scene configured with `config`, as the renderer would hand it a preset.
fn scene_with(ctx: &RenderContext, config: VoxelConfig) -> VoxelScene {
    // `COMPOSITE_FORMAT`, not the surface format: a scene draws into the
    // composite chain's linear target (`scenes::create_all`).
    let mut scene = VoxelScene::new(&ctx.device, crate::render::COMPOSITE_FORMAT, MAX_GRID);
    scene.configure(&GeneratorConfig::Voxel(config));
    scene
}

/// Roster rule `rule` on a grid of `grid`, from its own seed.
fn config(grid: u32, rule: RosterRule, salt: u32) -> VoxelConfig {
    let (seed_radius, seed_fill) = rule.seed();
    VoxelConfig {
        grid,
        rules: RuleList::one(rule.rule()),
        seed_radius,
        seed_fill,
        salt,
        ..VoxelConfig::default()
    }
}

const TARGET: u32 = 48;

/// Drives a scene through the renderer's per-frame order — `advance`,
/// `reset_params`, `set_param`, `update`, `render` — into a small offscreen
/// target.
struct Driver<'a> {
    ctx: &'a RenderContext,
    target: wgpu::Texture,
    view: wgpu::TextureView,
}

impl<'a> Driver<'a> {
    fn new(ctx: &'a RenderContext) -> Self {
        let (target, view) =
            capture::create_target(&ctx.device, crate::render::COMPOSITE_FORMAT, TARGET, TARGET);
        Self { ctx, target, view }
    }

    /// One frame of `dt` seconds with `params` bound; returns how many
    /// generations it ran.
    fn frame(&mut self, scene: &mut VoxelScene, dt: f32, params: &[(&str, f32)]) -> u32 {
        scene.set_target_size(TARGET, TARGET);
        scene.advance(dt);
        scene.reset_params();
        for (name, value) in params {
            assert!(
                crate::render::scenes::declares(PARAMS, name),
                "the probe set an unknown param `{name}`"
            );
            scene.set_param(name, *value);
        }
        scene.update(&AnalysisFrame::default());
        let generations = scene.pending_generations;
        let mut encoder = self
            .ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("voxel-probe"),
            });
        capture::record_clear(&mut encoder, &self.view);
        scene.render(&self.ctx.queue, &mut encoder, &self.view, 1.0);
        self.ctx.queue.submit(std::iter::once(encoder.finish()));
        generations
    }

    /// The last frame's present, `TARGET` pixels a side, as linear RGBA.
    fn read_target(&self) -> Vec<[f32; 4]> {
        let (buffer, padded_bpr) =
            capture::create_linear_readback(&self.ctx.device, TARGET, TARGET);
        let mut encoder = self
            .ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("voxel-target-readback"),
            });
        capture::record_copy(
            &mut encoder,
            &self.target,
            &buffer,
            padded_bpr,
            TARGET,
            TARGET,
        );
        self.ctx.queue.submit(std::iter::once(encoder.finish()));
        capture::read_back_linear(&self.ctx.device, &buffer, TARGET, TARGET, padded_bpr)
            .expect("the target reads back")
            .chunks_exact(4)
            .map(|p| [p[0], p[1], p[2], p[3]])
            .collect()
    }
}

/// Every cell of the current generation, x fastest then y then z, as the
/// packed `state | age << 8` texel.
fn read_cells(ctx: &RenderContext, scene: &VoxelScene) -> Vec<u32> {
    let res = scene.res.as_ref().expect("configure builds the resources");
    let n = res.grid;
    let row = n * 4;
    let padded =
        row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let read = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("voxel-cells-readback"),
        size: u64::from(padded * n * n),
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("voxel-cells-readback"),
        });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: res.current(),
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &read,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded),
                rows_per_image: Some(n),
            },
        },
        wgpu::Extent3d {
            width: n,
            height: n,
            depth_or_array_layers: n,
        },
    );
    ctx.queue.submit([encoder.finish()]);
    let slice = read.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |res| {
        let _ = tx.send(res);
    });
    ctx.device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            // Bounded: a hung device fails the test rather than the suite.
            timeout: Some(std::time::Duration::from_secs(30)),
        })
        .expect("poll the cells readback");
    rx.recv()
        .expect("map callback ran")
        .expect("map the cells readback");
    let bytes = slice.get_mapped_range().expect("mapped").to_vec();
    read.unmap();
    let mut cells = Vec::with_capacity((n * n * n) as usize);
    for line in bytes.chunks_exact(padded as usize) {
        cells.extend(
            line[..row as usize]
                .chunks_exact(4)
                .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])),
        );
    }
    cells
}

/// FNV-1a over the cells: one number that moves when any cell's state or age
/// does.
fn hash(cells: &[u32]) -> u64 {
    cells.iter().fold(0xcbf2_9ce4_8422_2325_u64, |h, cell| {
        cell.to_le_bytes()
            .iter()
            .fold(h, |h, b| (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3))
    })
}

/// How many cells are live.
fn live(cells: &[u32]) -> usize {
    cells.iter().filter(|c| **c & 0xFF == 1).count()
}

// ---------------------------------------------------------------------------
// The clock
// ---------------------------------------------------------------------------

/// **The same wall time runs the same generations, and so the same volume, at
/// any refresh rate**: two seconds at 60 Hz and at 144 Hz run the same count,
/// and leave state textures with the same hash.
#[test]
fn two_seconds_at_60_and_144_hz_run_the_same_generations_and_volume() {
    let Some(ctx) = context() else {
        return;
    };
    let run = |fps: u32| -> (u32, u64, usize) {
        let mut scene = scene_with(&ctx, config(32, RosterRule::Clouds, 7));
        let mut driver = Driver::new(&ctx);
        let dt = 1.0 / fps as f32;
        let generations: u32 = (0..2 * fps)
            .map(|_| driver.frame(&mut scene, dt, &[("step_rate", 10.0)]))
            .sum();
        let cells = read_cells(&ctx, &scene);
        (generations, hash(&cells), live(&cells))
    };
    let (g60, h60, live60) = run(60);
    let (g144, h144, live144) = run(144);
    println!("60 Hz: {g60} generations, {live60} live; 144 Hz: {g144} generations, {live144} live");
    assert_eq!(g60, 20, "10 generations a second for 2 s at 60 Hz");
    assert_eq!(g144, g60, "the same wall time at 144 Hz");
    assert!(live60 > 0, "the volume died, so the hash compares nothing");
    assert_eq!(h144, h60, "the same generations leave the same volume");
}

/// The shared clock under this system's rate: zero freezes, a non-finite rate
/// runs the declared default, and a rate past the ceiling is held to it.
#[test]
fn the_applied_step_rate_freezes_falls_back_and_caps() {
    assert_eq!(applied_step_rate(0.0), 0.0);
    assert_eq!(applied_step_rate(-4.0), 0.0);
    assert_eq!(applied_step_rate(f32::NAN), DEFAULT_STEP_RATE);
    assert_eq!(applied_step_rate(1e9), MAX_STEP_RATE);
}

// ---------------------------------------------------------------------------
// The march's ray
// ---------------------------------------------------------------------------

/// The ray the march casts through NDC `(x, y)`, as the shader recovers it from
/// the uniform: the eye, and the unit direction both screen rows of `view`
/// project to `(x, y)`. **The CPU mirror of the ray in `shader::MARCH_SHADER`.**
fn ray(view: &camera::CameraView, x: f32, y: f32) -> ([f32; 3], [f32; 3]) {
    let sub = |a: [f32; 3], b: [f32; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let scale = |a: [f32; 3], s: f32| [a[0] * s, a[1] * s, a[2] * s];
    let [r0, r1, r3] = rows(view);
    let a = sub(xyz(r0), scale(xyz(r3), x));
    let b = sub(xyz(r1), scale(xyz(r3), y));
    let c = cross(a, b);
    let mut d = scale(c, 1.0 / dot(c, c).sqrt());
    if dot(d, xyz(r3)) < 0.0 {
        d = scale(d, -1.0);
    }
    (eye(view), d)
}

/// **The ray the march casts through a pixel projects back onto that pixel.**
/// Every point along it lands at the NDC it was cast through and in front of
/// the eye, whatever the orbit, the aspect, the zoom or the pan — so the march
/// draws the volume where the camera puts it (ADR-0037: the aspect is the
/// target's).
#[test]
fn the_march_ray_projects_back_onto_its_pixel() {
    for (yaw, pitch, aspect, zoom, pan) in [
        (0.0, 0.0, 1.0, 1.0, [0.0, 0.0]),
        (0.7, 0.3, 16.0 / 9.0, 1.0, [0.0, 0.0]),
        (-2.1, -0.9, 0.6, 2.0, [0.2, -0.1]),
    ] {
        let view = CameraParams {
            yaw,
            pitch,
            ..CameraParams::default()
        }
        .camera()
        .view(aspect, zoom, pan);
        for (x, y) in [(0.0, 0.0), (0.5, -0.25), (-0.9, 0.8)] {
            let (eye, dir) = ray(&view, x, y);
            for t in [0.5_f32, 2.0, 5.0] {
                let p = [
                    eye[0] + dir[0] * t,
                    eye[1] + dir[1] * t,
                    eye[2] + dir[2] * t,
                ];
                let c = view.clip(p);
                assert!(c[3] > 0.0, "a point along the ray is behind the eye");
                assert!(
                    (c[0] / c[3] - x).abs() < 1e-4 && (c[1] / c[3] - y).abs() < 1e-4,
                    "yaw {yaw} aspect {aspect}: the ray through ({x}, {y}) projects to ({}, {})",
                    c[0] / c[3],
                    c[1] / c[3]
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// The volume on screen
// ---------------------------------------------------------------------------

/// **A seeded volume reaches the screen, and reads as a volume**: through a
/// turned camera the frame is lit, but not everywhere and not uniformly.
#[test]
fn a_seeded_volume_draws_a_lit_non_uniform_frame() {
    let Some(ctx) = context() else {
        return;
    };
    let mut scene = scene_with(&ctx, config(32, RosterRule::Clouds, 3));
    let mut driver = Driver::new(&ctx);
    for _ in 0..30 {
        driver.frame(&mut scene, 1.0 / 60.0, &[("yaw", 0.6), ("pitch", 0.4)]);
    }
    let pixels = driver.read_target();
    let lum: Vec<f32> = pixels.iter().map(|p| p[0] + p[1] + p[2]).collect();
    let lit = lum.iter().filter(|l| **l > 0.01).count();
    let max = lum.iter().copied().fold(0.0, f32::max);
    let mean = lum.iter().sum::<f32>() / lum.len() as f32;
    println!(
        "{lit} of {} pixels lit, mean {mean:.4}, max {max:.4}",
        lum.len()
    );
    assert!(
        lit > lum.len() / 20,
        "the volume lights too little of the frame"
    );
    assert!(
        lit < lum.len(),
        "the volume lights every pixel, so the cube's silhouette is gone"
    );
    assert!(
        max > 2.0 * mean,
        "the frame is uniform rather than a structure"
    );
}
