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
    // Every grid the loader takes, and a march at the target's own resolution.
    let mut scene = VoxelScene::new(
        &ctx.device,
        crate::render::COMPOSITE_FORMAT,
        MAX_GRID,
        GridScale::FULL,
        (4096, 4096),
    );
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
    /// Pixels a side.
    size: u32,
    target: wgpu::Texture,
    view: wgpu::TextureView,
    /// What each frame starts from: opaque black, as a capture does, or
    /// transparent, so the frame's alpha is the volume's own coverage.
    clear: wgpu::Color,
}

impl<'a> Driver<'a> {
    fn new(ctx: &'a RenderContext) -> Self {
        Self::sized(ctx, TARGET)
    }

    /// A driver into a square target `size` pixels a side.
    fn sized(ctx: &'a RenderContext, size: u32) -> Self {
        let (target, view) =
            capture::create_target(&ctx.device, crate::render::COMPOSITE_FORMAT, size, size);
        Self {
            ctx,
            size,
            target,
            view,
            clear: wgpu::Color::BLACK,
        }
    }

    /// One frame of `dt` seconds with `params` bound; returns how many
    /// generations it ran.
    fn frame(&mut self, scene: &mut VoxelScene, dt: f32, params: &[(&str, f32)]) -> u32 {
        scene.set_target_size(self.size, self.size);
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
        gpu::color_pass(
            &mut encoder,
            "voxel-probe-clear",
            &self.view,
            wgpu::LoadOp::Clear(self.clear),
        );
        scene.render(&self.ctx.queue, &mut encoder, &self.view, 1.0);
        self.ctx.queue.submit(std::iter::once(encoder.finish()));
        generations
    }

    /// The last frame's present, `size` pixels a side, as linear RGBA.
    fn read_target(&self) -> Vec<[f32; 4]> {
        let n = self.size;
        let (buffer, padded_bpr) = capture::create_linear_readback(&self.ctx.device, n, n);
        let mut encoder = self
            .ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("voxel-target-readback"),
            });
        capture::record_copy(&mut encoder, &self.target, &buffer, padded_bpr, n, n);
        self.ctx.queue.submit(std::iter::once(encoder.finish()));
        capture::read_back_linear(&self.ctx.device, &buffer, n, n, padded_bpr)
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

/// Overwrite the current generation: every cell `live` names is live at age
/// 0, every other dead. The bricks are re-marked, so the march jumps exactly
/// the bricks this structure leaves empty, and the light field is marked stale,
/// so the next frame with `bounce` on refills it.
fn write_cells(ctx: &RenderContext, scene: &mut VoxelScene, live: impl Fn([u32; 3]) -> bool) {
    let res = scene.res.as_mut().expect("configure builds the resources");
    let n = res.grid;
    let mut bytes = Vec::with_capacity((n * n * n * 4) as usize);
    for z in 0..n {
        for y in 0..n {
            for x in 0..n {
                bytes.extend_from_slice(&u32::from(live([x, y, z])).to_le_bytes());
            }
        }
    }
    ctx.queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: res.current(),
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &bytes,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(n * 4),
            rows_per_image: Some(n),
        },
        wgpu::Extent3d {
            width: n,
            height: n,
            depth_or_array_layers: n,
        },
    );
    let mut encoder = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("voxel-write-cells"),
        });
    res.encode_bricks(&mut encoder);
    res.light_fresh = false;
    ctx.queue.submit([encoder.finish()]);
}

/// A scene drawing `present` on a grid of `grid` whose cells are exactly
/// `live`, the clock frozen so nothing steps them, and a driver into a target
/// `size` pixels a side.
fn scene_holding(
    ctx: &RenderContext,
    size: u32,
    grid: u32,
    present: Present,
    live: impl Fn([u32; 3]) -> bool,
) -> (VoxelScene, Driver<'_>) {
    let mut scene = scene_with(
        ctx,
        VoxelConfig {
            present,
            ..config(grid, RosterRule::Clouds, 1)
        },
    );
    let mut driver = Driver::sized(ctx, size);
    // The first frame seeds the field; the structure replaces it after.
    driver.frame(&mut scene, 1.0 / 60.0, &[("step_rate", 0.0)]);
    write_cells(ctx, &mut scene, live);
    (scene, driver)
}

/// The pixel of a `size`-square frame that world point `p` lands on, under the
/// camera a frame with `yaw` and `pitch` bound and every other camera param at
/// rest looks through.
fn pixel_of(size: u32, yaw: f32, pitch: f32, p: [f32; 3]) -> usize {
    let camera = CameraParams {
        yaw,
        pitch,
        ..CameraParams::default()
    };
    pixel_through(size, camera, p)
}

/// The pixel of a `size`-square frame that world point `p` lands on, under
/// `camera` at rest zoom and pan.
fn pixel_through(size: u32, camera: CameraParams, p: [f32; 3]) -> usize {
    let view = camera.camera().view(1.0, DEFAULT_ZOOM, [0.0, 0.0]);
    let c = view.clip(p);
    let (x, y) = (c[0] / c[3], c[1] / c[3]);
    let n = size as usize;
    let col = (((x + 1.0) * 0.5 * size as f32) as usize).min(n - 1);
    let row = (((1.0 - y) * 0.5 * size as f32) as usize).min(n - 1);
    row * n + col
}

/// The camera every solid probe looks through.
const SOLID_YAW: f32 = 0.6;
const SOLID_PITCH: f32 = 0.4;

/// The three faces of a cube centred on the origin that a camera at
/// [`SOLID_YAW`], [`SOLID_PITCH`] sees — `+x`, `+y`, `+z` — each as its
/// outward normal.
const VISIBLE_FACES: [[f32; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

/// An 8³ block in the middle of a 16³ grid: cells 4 to 11 on every axis, the
/// world cube `[-0.5, 0.5]^3`.
fn block(c: [u32; 3]) -> bool {
    c.iter().all(|&v| (4..12).contains(&v))
}

/// The colour of each [`VISIBLE_FACES`] face's centre, on the 8³ [`block`]
/// drawn solid with `params` bound over the probe camera.
fn face_centres(ctx: &RenderContext, params: &[(&str, f32)]) -> [[f32; 4]; 3] {
    let (mut scene, mut driver) = scene_holding(ctx, TARGET, 16, Present::Solid, block);
    let mut bound = vec![
        ("step_rate", 0.0),
        ("yaw", SOLID_YAW),
        ("pitch", SOLID_PITCH),
        // One colour over the whole block, so a face's brightness is its light.
        ("hue_spread", 0.0),
        ("age_tint", 0.0),
    ];
    bound.extend_from_slice(params);
    driver.frame(&mut scene, 1.0 / 60.0, &bound);
    let pixels = driver.read_target();
    VISIBLE_FACES.map(|n| pixels[pixel_of(TARGET, SOLID_YAW, SOLID_PITCH, n.map(|v| v * 0.5))])
}

fn luminance(p: [f32; 4]) -> f32 {
    0.2126 * p[0] + 0.7152 * p[1] + 0.0722 * p[2]
}

// ---------------------------------------------------------------------------
// Solid
// ---------------------------------------------------------------------------

/// **Three faces of a solid block read at three brightnesses, ordered by
/// `n . l`**: an 8³ block seen from `yaw = 0.6, pitch = 0.4` under the
/// default key light shows its `+x`, `+y` and `+z` faces, each covering its
/// centre pixel, and their luminances sort exactly as their normals' dot
/// products with the light do.
#[test]
fn a_solid_blocks_three_faces_are_ordered_by_n_dot_l() {
    let Some(ctx) = context() else {
        return;
    };
    let view = CameraParams {
        yaw: SOLID_YAW,
        pitch: SOLID_PITCH,
        ..CameraParams::default()
    }
    .camera()
    .view(1.0, DEFAULT_ZOOM, [0.0, 0.0]);
    let l = light_dir(&view, DEFAULT_LIGHT_YAW, DEFAULT_LIGHT_PITCH);
    let n_dot_l = VISIBLE_FACES.map(|n| dot(n, l).max(0.0));
    let faces = face_centres(&ctx, &[]);
    let lum = faces.map(luminance);
    println!("n . l {n_dot_l:?}, luminance {lum:?}, faces {faces:?}");
    for (i, face) in faces.iter().enumerate() {
        assert!(
            face[3] > 0.99 * crate::render::post::DEFAULT_OCCLUDE,
            "face {i}'s centre is not covered: {face:?}"
        );
    }
    let mut by_light = [0, 1, 2];
    by_light.sort_by(|&a, &b| n_dot_l[a].total_cmp(&n_dot_l[b]));
    for pair in by_light.windows(2) {
        let (lo, hi) = (pair[0], pair[1]);
        assert!(
            n_dot_l[hi] - n_dot_l[lo] > 0.05,
            "the probe's light does not separate faces {lo} and {hi}: {n_dot_l:?}"
        );
        assert!(
            lum[hi] > lum[lo] * 1.02,
            "face {hi} faces the light more than face {lo} but reads no brighter: \
             n . l {n_dot_l:?}, luminance {lum:?}"
        );
    }
}

/// **With no key light and full ambient, every face reads the albedo**: the
/// three face centres are one colour, the palette's at `hue` times
/// `brightness`.
#[test]
fn with_key_0_and_ambient_1_every_face_reads_the_albedo() {
    let Some(ctx) = context() else {
        return;
    };
    let hue = 0.3;
    let faces = face_centres(&ctx, &[("key", 0.0), ("ambient", 1.0), ("hue", hue)]);
    let albedo = Palette::default_spectrum().sample(hue, 0.0);
    println!("faces {faces:?}, albedo {albedo:?}");
    for face in &faces {
        for c in 0..3 {
            assert!(
                (face[c] - faces[0][c]).abs() < 1e-3,
                "two faces differ with no key light: {faces:?}"
            );
            assert!(
                (face[c] - albedo[c]).abs() < 0.02 + 0.02 * albedo[c],
                "a face reads {face:?}, not the albedo {albedo:?}"
            );
        }
    }
}

/// **`glow` ignores the light**: a frame drawn under `glow` with the light
/// params far from rest is byte-identical to one with them at rest.
#[test]
fn glow_ignores_the_light_params() {
    let Some(ctx) = context() else {
        return;
    };
    let frame = |params: &[(&str, f32)]| {
        let (mut scene, mut driver) = scene_holding(&ctx, TARGET, 16, Present::Glow, block);
        let mut bound = vec![
            ("step_rate", 0.0),
            ("yaw", SOLID_YAW),
            ("pitch", SOLID_PITCH),
        ];
        bound.extend_from_slice(params);
        driver.frame(&mut scene, 1.0 / 60.0, &bound);
        driver.read_target()
    };
    let rest = frame(&[]);
    let moved = frame(&[
        ("key", 3.0),
        ("ambient", 0.9),
        ("ao", 0.0),
        ("light_yaw", 2.0),
        ("light_pitch", -1.0),
    ]);
    assert!(rest.iter().any(|p| p[3] > 0.0), "the block drew nothing");
    assert!(rest == moved, "a light param moved a glow frame");
}

/// The AO probes' target, pixels a side: fine enough that a point a few
/// tenths of a cell from an edge lands on a pixel of its own.
const AO_TARGET: u32 = 384;

/// An L of two blocks on a 16³ grid, extruded along z over cells 4 to 11: a
/// base, cells x 2-11 and y 2-5, and an upright on its `-x` end, cells x 2-5
/// and y 6-13. The inner edge runs along z at `x = y = 6` cells.
fn l_shape(c: [u32; 3]) -> bool {
    let [x, y, z] = c;
    let depth = (4..12).contains(&z);
    let base = (2..12).contains(&x) && (2..6).contains(&y);
    let upright = (2..6).contains(&x) && (6..14).contains(&y);
    depth && (base || upright)
}

/// A point `(x, y, z)` in cells of the 16³ grid, in world units.
fn cells_to_world(c: [f32; 3]) -> [f32; 3] {
    c.map(|v| v / 8.0 - 1.0)
}

/// The luminance at each of `points` (in cells) on the [`l_shape`] drawn solid
/// with `ao` bound, every point required to be covered.
fn l_luminance(ctx: &RenderContext, ao: f32, points: &[[f32; 3]]) -> Vec<f32> {
    let (mut scene, mut driver) = scene_holding(ctx, AO_TARGET, 16, Present::Solid, l_shape);
    driver.frame(
        &mut scene,
        1.0 / 60.0,
        &[
            ("step_rate", 0.0),
            ("yaw", SOLID_YAW),
            ("pitch", SOLID_PITCH),
            ("hue_spread", 0.0),
            ("age_tint", 0.0),
            ("ao", ao),
        ],
    );
    let pixels = driver.read_target();
    points
        .iter()
        .map(|&c| {
            let p = pixels[pixel_of(AO_TARGET, SOLID_YAW, SOLID_PITCH, cells_to_world(c))];
            assert!(p[3] > 0.0, "the point {c:?} is not on the L");
            luminance(p)
        })
        .collect()
}

/// **The ambient occlusion darkens the crease and nothing else**: at `ao = 1`
/// the two faces meeting at the L's inner edge — the base's top and the
/// upright's `+x` face — each read strictly darker 0.4 cells from that edge
/// than at their own centres, while the upright's top, which has no
/// neighbours, reads the same near its edge as at its centre. The control: at
/// `ao = 0` the crease reads as its centre does.
#[test]
fn ambient_occlusion_darkens_the_inner_edge_of_an_l() {
    let Some(ctx) = context() else {
        return;
    };
    let points = [
        // The base's top, at y = 6: near the inner edge, then its centre.
        [6.4, 6.0, 8.0],
        [9.0, 6.0, 8.0],
        // The upright's +x face, at x = 6: near the inner edge, then its centre.
        [6.0, 6.4, 8.0],
        [6.0, 10.0, 8.0],
        // The upright's top, at y = 14: near its +x edge, then its centre.
        [5.6, 14.0, 8.0],
        [4.0, 14.0, 8.0],
    ];
    let on = l_luminance(&ctx, 1.0, &points);
    let off = l_luminance(&ctx, 0.0, &points);
    println!("ao 1: {on:?}\nao 0: {off:?}");
    for (face, pair) in [(0, "the base's top"), (2, "the upright's +x face")] {
        assert!(
            on[face] < on[face + 1] * 0.98,
            "{pair} reads {} at the crease, no darker than {} at its centre",
            on[face],
            on[face + 1]
        );
        assert!(
            (off[face] - off[face + 1]).abs() < 1e-3,
            "with ao 0, {pair} still darkens at the crease: {} against {}",
            off[face],
            off[face + 1]
        );
    }
    assert!(
        (on[4] - on[5]).abs() < 1e-3,
        "the upright's open top reads {} at its edge and {} at its centre",
        on[4],
        on[5]
    );
}

// ---------------------------------------------------------------------------
// Bounce
// ---------------------------------------------------------------------------

/// **An unbound `bounce` builds no light field**: a solid volume stepped
/// through a run of generations at `bounce = 0` refills the field zero times
/// and never builds it, and a `glow` volume with `bounce = 1` bound, which
/// reads no light, builds none either.
#[test]
fn bounce_0_builds_no_light_field() {
    let Some(ctx) = context() else {
        return;
    };
    for (present, bounce) in [(Present::Solid, 0.0), (Present::Glow, 1.0)] {
        let mut scene = scene_with(
            &ctx,
            VoxelConfig {
                present,
                ..config(32, RosterRule::Clouds, 3)
            },
        );
        let mut driver = Driver::new(&ctx);
        let generations: u32 = (0..30)
            .map(|_| {
                driver.frame(
                    &mut scene,
                    1.0 / 30.0,
                    &[("step_rate", 15.0), ("bounce", bounce)],
                )
            })
            .sum();
        assert!(
            generations >= 10,
            "the run stepped {generations} generations"
        );
        assert_eq!(
            scene.light_build_count(),
            0,
            "{present:?} at bounce {bounce} refilled the light field"
        );
        assert!(
            scene.res.as_ref().is_some_and(|res| res.light.is_none()),
            "{present:?} at bounce {bounce} built the light field"
        );
    }
}

/// The probe for the bounce test, on a 16³ grid: one live cell at x 4, and
/// beside it on `+x`, three empty cells away, the 4³ block x 8-11 that is
/// exactly one light-field texel. Both sit at y and z 8-11.
const LONE_CELL: [u32; 3] = [4, 9, 9];

fn lone_cell(c: [u32; 3]) -> bool {
    c == LONE_CELL
}

fn cell_and_block(c: [u32; 3]) -> bool {
    lone_cell(c) || c.iter().all(|&v| (8..12).contains(&v))
}

/// The pixel at the centre of [`LONE_CELL`]'s `+x` face, drawn solid with
/// `bounce` bound over the probe camera, `live` naming every live cell.
fn lone_face(ctx: &RenderContext, live: fn([u32; 3]) -> bool, bounce: f32) -> [f32; 4] {
    let (mut scene, mut driver) = scene_holding(ctx, AO_TARGET, 16, Present::Solid, live);
    driver.frame(
        &mut scene,
        1.0 / 60.0,
        &[
            ("step_rate", 0.0),
            ("yaw", SOLID_YAW),
            ("pitch", SOLID_PITCH),
            ("hue_spread", 0.0),
            ("age_tint", 0.0),
            ("bounce", bounce),
        ],
    );
    let [x, y, z] = LONE_CELL.map(|v| v as f32);
    let face = cells_to_world([x + 1.0, y + 0.5, z + 0.5]);
    let p = driver.read_target()[pixel_of(AO_TARGET, SOLID_YAW, SOLID_PITCH, face)];
    assert!(p[3] > 0.0, "the lone cell's +x face is not covered: {p:?}");
    p
}

/// **A face brightens toward a block of live cells**: the `+x` face of one
/// live cell, three cells from a 4³ block of live cells, reads brighter at
/// `bounce = 1` than at 0, and gains more between the two than the same face
/// gains when the cell is drawn alone. At `bounce = 0` the block moves the face
/// not at all, so the gain is the field's alone: the block neither hides the
/// face nor closes its occlusion.
#[test]
fn a_face_brightens_toward_a_block_of_live_cells() {
    let Some(ctx) = context() else {
        return;
    };
    let with_0 = lone_face(&ctx, cell_and_block, 0.0);
    let with_1 = lone_face(&ctx, cell_and_block, 1.0);
    let alone_0 = lone_face(&ctx, lone_cell, 0.0);
    let alone_1 = lone_face(&ctx, lone_cell, 1.0);
    println!(
        "with the block: {with_0:?} at 0, {with_1:?} at 1; alone: {alone_0:?} at 0, {alone_1:?} \
         at 1"
    );
    assert!(
        with_0 == alone_0,
        "at bounce 0 the block moved the face: {with_0:?} against {alone_0:?}"
    );
    let gain = luminance(with_1) - luminance(with_0);
    let alone_gain = luminance(alone_1) - luminance(alone_0);
    assert!(
        gain > 0.0,
        "the face toward the block reads no brighter at bounce 1"
    );
    assert!(
        gain > 2.0 * alone_gain.max(0.0) + 1e-3,
        "the face gains {gain} beside the block and {alone_gain} alone"
    );
}

/// **A light field first built mid-run reads as one built from the start**:
/// a stepping solid volume switched to `bounce = 1` after ten frames at 0
/// draws the switch frame, and the frame after it, byte-identical to a run
/// bound to 1 throughout — and bounce visibly moves that frame.
#[test]
fn a_light_field_built_mid_run_matches_one_built_from_the_start() {
    let Some(ctx) = context() else {
        return;
    };
    let switch = 10;
    let run = |bounce_at: &dyn Fn(usize) -> f32| {
        let mut scene = scene_with(
            &ctx,
            VoxelConfig {
                present: Present::Solid,
                ..config(32, RosterRule::Clouds, 3)
            },
        );
        let mut driver = Driver::new(&ctx);
        let mut frames = Vec::new();
        for i in 0..switch + 2 {
            driver.frame(
                &mut scene,
                1.0 / 60.0,
                &[
                    ("step_rate", 20.0),
                    ("yaw", SOLID_YAW),
                    ("pitch", SOLID_PITCH),
                    ("bounce", bounce_at(i)),
                ],
            );
            frames.push(driver.read_target());
        }
        (frames, scene.light_build_count())
    };
    let (always, always_builds) = run(&|_| 1.0);
    let (late, late_builds) = run(&|i| if i < switch { 0.0 } else { 1.0 });
    let (never, _) = run(&|_| 0.0);
    println!("refills: {always_builds} from the start, {late_builds} from frame {switch}");
    assert!(late_builds > 0 && late_builds < always_builds);
    for i in [switch, switch + 1] {
        assert!(
            always[i] == late[i],
            "frame {i} differs between a field built at frame 0 and at frame {switch}"
        );
        assert!(
            always[i] != never[i],
            "bounce 1 does not move frame {i}, so the comparison is empty"
        );
    }
}

// ---------------------------------------------------------------------------
// The section plane
// ---------------------------------------------------------------------------

/// `core/tests/fixtures/voxel_section.toml`: a solid 16³ block cut through its
/// centre, seen from an eye inside the cut plane.
const SECTION_FIXTURE: &str = include_str!("../../../../tests/fixtures/voxel_section.toml");

/// A voxel fixture's `[voxel]` table, and its `[params]` each evaluated once at
/// rest: every binding a fixture here writes is a constant.
fn fixture(src: &str) -> (VoxelConfig, Vec<(String, f32)>) {
    let preset = crate::preset::Preset::from_toml_str(src).expect("the fixture parses");
    let Some(GeneratorConfig::Voxel(config)) = preset.config else {
        panic!("the fixture carries no [voxel] table");
    };
    let vars = crate::preset::expr::Variables::default();
    let params = preset
        .params
        .iter()
        .map(|b| (b.name.clone(), b.expr.eval(&vars)))
        .collect();
    (config, params)
}

/// One frame of the fixture `src` into a `size`-square target cleared
/// transparent, with `overrides` replacing (or adding to) its params; the frame
/// as linear RGBA, its alpha the volume's coverage.
fn render_fixture(
    ctx: &RenderContext,
    src: &str,
    size: u32,
    overrides: &[(&str, f32)],
) -> Vec<[f32; 4]> {
    let (config, params) = fixture(src);
    let mut bound: Vec<(&str, f32)> = params.iter().map(|(n, v)| (n.as_str(), *v)).collect();
    for &(name, value) in overrides {
        match bound.iter_mut().find(|(n, _)| *n == name) {
            Some(slot) => slot.1 = value,
            None => bound.push((name, value)),
        }
    }
    let mut scene = scene_with(ctx, config);
    let mut driver = Driver::sized(ctx, size);
    driver.clear = wgpu::Color::TRANSPARENT;
    driver.frame(&mut scene, 1.0 / 60.0, &bound);
    driver.read_target()
}

/// The value the fixture `src` binds `name` to.
fn fixture_param(src: &str, name: &str) -> f32 {
    fixture(src)
        .1
        .into_iter()
        .find(|(n, _)| n == name)
        .map(|(_, v)| v)
        .unwrap_or_else(|| panic!("the fixture binds no `{name}`"))
}

/// **The section plane hides the side its normal points to and leaves the
/// other untouched**: the fixture's eye lies in the plane `z = 0`, which
/// projects to the frame's vertical centre line. At `cut = 0.5` every pixel on
/// the `+z` side has alpha 0, and every pixel on the other side is
/// byte-identical to the `cut = 0` frame, since no ray there enters a cut cell.
/// At `cut = 1` the whole frame is the backdrop.
#[test]
fn the_section_plane_hides_one_side_and_leaves_the_other() {
    let Some(ctx) = context() else {
        return;
    };
    let size = 64;
    assert_eq!(
        fixture_param(SECTION_FIXTURE, "cut"),
        0.5,
        "the fixture no longer cuts through the centre"
    );
    let yaw = fixture_param(SECTION_FIXTURE, "yaw");
    let pitch = fixture_param(SECTION_FIXTURE, "pitch");
    let side = size as usize;
    let half = side / 2;
    // The column a point on the normal's side lands in names that side.
    let normal_left = pixel_of(size, yaw, pitch, [0.0, 0.0, 0.5]) % side < half;
    let on_normal_side = |i: usize| (i % side < half) == normal_left;

    let uncut = render_fixture(&ctx, SECTION_FIXTURE, size, &[("cut", 0.0)]);
    let cut = render_fixture(&ctx, SECTION_FIXTURE, size, &[]);
    let gone = render_fixture(&ctx, SECTION_FIXTURE, size, &[("cut", 1.0)]);

    let covered = |side: bool| {
        (0..uncut.len())
            .filter(|&i| on_normal_side(i) == side && uncut[i][3] > 0.0)
            .count()
    };
    let (near, far) = (covered(true), covered(false));
    println!("uncut: {near} covered pixels on the normal's side, {far} on the other");
    assert!(
        near > 100 && far > 100,
        "the uncut block does not cover both halves, so the comparison is empty"
    );
    for (i, (c, u)) in cut.iter().zip(&uncut).enumerate() {
        if on_normal_side(i) {
            assert_eq!(c[3], 0.0, "pixel {i} on the cut side is covered: {c:?}");
        } else {
            assert!(
                c.map(f32::to_bits) == u.map(f32::to_bits),
                "pixel {i} on the kept side moved: {c:?} against {u:?}"
            );
        }
    }
    assert!(
        gone.iter().all(|p| p[3] == 0.0),
        "at cut = 1 something is still drawn"
    );
}

/// The plane's offset runs from past the cube's farthest corner at `cut` just
/// above 0 to the far corner's opposite at 1, through the centre at 0.5, and
/// `cut = 0` packs no plane.
#[test]
fn the_cut_plane_spans_the_cube_and_is_off_at_zero() {
    assert_eq!(cut_plane(0.0, 1.0, 0.3), None);
    assert_eq!(cut_plane(-0.2, 0.0, 0.0), None);
    let [nx, ny, nz, d] = cut_plane(0.5, 0.0, 0.0).expect("a plane");
    assert_eq!([nx, ny, nz, d], [0.0, 0.0, 1.0, 0.0]);
    let d1 = cut_plane(1.0, 0.0, 0.0).expect("a plane")[3];
    assert!((d1 + CUBE_RADIUS).abs() < 1e-6);
    assert_eq!(cut_plane(4.0, 0.0, 0.0), cut_plane(1.0, 0.0, 0.0));
    let [x, y, z, _] = cut_plane(0.3, 2.0, -0.7).expect("a plane");
    assert!(
        (x * x + y * y + z * z - 1.0).abs() < 1e-6,
        "the normal is unit"
    );
    let up = cut_plane(0.3, 0.0, FRAC_PI_2).expect("a plane");
    assert!((up[1] - 1.0).abs() < 1e-6, "a quarter-turn pitch faces up");
}

/// `core/tests/fixtures/voxel_section_lit.toml`: a solid 32³ block cut through
/// its centre on a plane tilted toward `+y`, with occlusion on.
const SECTION_LIT_FIXTURE: &str = include_str!("../../../../tests/fixtures/voxel_section_lit.toml");

/// Whether `plane` ([`cut_plane`]) hides cell `c` of a grid of `n`: the CPU
/// mirror of `cut_away` in `shader::MARCH_SHADER`.
fn is_cut(plane: [f32; 4], n: u32, c: [i32; 3]) -> bool {
    let centre = c.map(|v| (v as f32 + 0.5) / n as f32 * 2.0 - 1.0);
    dot(centre, xyz(plane)) > plane[3]
}

/// **A tilted section is lit as a surface, and no cut cell darkens it**: on
/// the fixture's stepped section, a tread — the `+z` face of a live cell on
/// the block's `+x` side, every cell in front of it cut or off the grid —
/// reads the same 0.3 cells from its edge as at its centre with `ao = 1`, the
/// same as with `ao = 0`, and the colour a `+z` face takes under the key light
/// and the full ambient. The section is interior: at the tread's centre the
/// uncut block shows its outer `+x` face instead, and reads differently.
#[test]
fn a_tilted_section_is_lit_as_a_surface_and_occluded_by_no_cut_cell() {
    let Some(ctx) = context() else {
        return;
    };
    let size = 768;
    let src = SECTION_LIT_FIXTURE;
    let p = |name| fixture_param(src, name);
    let (config, _) = fixture(src);
    let n = config.grid as i32;
    assert_eq!(p("ao"), 1.0, "the fixture no longer turns the occlusion on");
    let plane = cut_plane(p("cut"), p("cut_yaw"), p("cut_pitch")).expect("the fixture cuts");
    // What `solid_at` reads as open: off the grid, or cut.
    let open = |c: [i32; 3]| c.iter().any(|&v| v < 0 || v >= n) || is_cut(plane, config.grid, c);
    let x = n - 1;
    let (j, k) = (0..n)
        .flat_map(|j| (0..n).map(move |k| (j, k)))
        .filter(|&(j, k)| {
            let front_open = (-1..=1)
                .flat_map(|du| (-1..=1).map(move |dv| (du, dv)))
                .all(|(du, dv)| open([x + du, j + dv, k + 1]));
            // Live, flat toward -y, and nothing in front of it.
            !open([x, j, k]) && !open([x, j - 1, k]) && front_open
        })
        .min_by_key(|&(j, k)| (j - n / 2).abs() + (k - n / 2).abs())
        .expect("the section has a tread on the block's +x side");
    let world = |c: [f32; 3]| c.map(|v| v * 2.0 / n as f32 - 1.0);
    let (xf, jf, kf) = (x as f32, j as f32, k as f32);
    let centre_point = world([xf + 0.5, jf + 0.5, kf + 1.0]);
    let edge_point = world([xf + 0.7, jf + 0.5, kf + 1.0]);

    let camera = CameraParams {
        yaw: p("yaw"),
        pitch: p("pitch"),
        distance: p("distance"),
        fov: p("fov"),
        ..CameraParams::default()
    };
    let centre = pixel_through(size, camera, centre_point);
    let edge = pixel_through(size, camera, edge_point);
    assert_ne!(centre, edge, "the edge probe landed on the centre's pixel");

    let lit = render_fixture(&ctx, src, size, &[]);
    let flat = render_fixture(&ctx, src, size, &[("ao", 0.0)]);
    let uncut = render_fixture(&ctx, src, size, &[("cut", 0.0)]);
    let view = camera.camera().view(1.0, DEFAULT_ZOOM, [0.0, 0.0]);
    let l = light_dir(&view, p("light_yaw"), p("light_pitch"));
    let albedo = Palette::default_spectrum().sample(p("hue"), 0.0);
    let lighting = p("ambient") + p("key") * l[2].max(0.0);
    println!(
        "tread at cell ({x}, {j}, {k}): centre {:?}, edge {:?}, ao 0 {:?}, uncut {:?}, \
         expected {:?}",
        lit[centre],
        lit[edge],
        flat[centre],
        uncut[centre],
        albedo.map(|c| c * lighting)
    );
    for (name, px) in [("centre", lit[centre]), ("edge", lit[edge])] {
        assert!(px[3] > 0.0, "the tread's {name} is not covered: {px:?}");
    }
    assert!(
        (luminance(lit[edge]) - luminance(lit[centre])).abs() < 1e-3,
        "the tread reads {} near its edge and {} at its centre: a cut cell closed a corner",
        luminance(lit[edge]),
        luminance(lit[centre])
    );
    for c in 0..3 {
        assert!(
            (lit[centre][c] - flat[centre][c]).abs() < 1e-3,
            "ao darkens the tread: {:?} against {:?} at ao 0",
            lit[centre],
            flat[centre]
        );
        let want = albedo[c] * lighting;
        assert!(
            (lit[centre][c] - want).abs() < 0.02 + 0.02 * want,
            "the tread reads {:?}, not a +z face's {want} in channel {c}",
            lit[centre]
        );
    }
    assert!(
        uncut[centre][3] > 0.0,
        "the uncut block does not cover the tread's pixel"
    );
    assert!(
        (0..3).any(|c| (uncut[centre][c] - lit[centre][c]).abs() > 1e-3),
        "the uncut block reads the tread's colour at its pixel, so the section is not interior: \
         {:?} against {:?}",
        uncut[centre],
        lit[centre]
    );
}

/// The camera-space light: at `(0, 0)` it shines from the eye along the view
/// axis whatever the orbit, a quarter turn of `light_yaw` puts it on screen
/// right, and a positive `light_pitch` puts it above.
#[test]
fn the_key_light_is_given_in_camera_space() {
    for (yaw, pitch) in [(0.0, 0.0), (0.6, 0.4), (-2.0, -1.0)] {
        let view = CameraParams {
            yaw,
            pitch,
            ..CameraParams::default()
        }
        .camera()
        .view(16.0 / 9.0, 1.3, [0.2, -0.1]);
        let [r0, _, r3] = rows(&view);
        let forward = xyz(r3);
        let l = light_dir(&view, 0.0, 0.0);
        assert!(
            (dot(l, forward) + 1.0).abs() < 1e-5,
            "at (0, 0) the light shines along the view axis: {l:?} against {forward:?}"
        );
        // A quarter turn right puts it on screen right.
        let right = light_dir(&view, FRAC_PI_2, 0.0);
        assert!(dot(right, xyz(r0)) > 0.0 && dot(right, forward).abs() < 1e-5);
        // A pitch up puts it above.
        let up = light_dir(&view, 0.0, 1.0);
        assert!(dot(up, cross(xyz(r0), forward)) > 0.0);
    }
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
// The roster and the rule index
// ---------------------------------------------------------------------------

/// The live fraction of `rule` at generations 100 and 400 from `seed` on a
/// 32-cell cube, run 4 generations a frame.
fn live_fractions(ctx: &RenderContext, rule: Rule, seed: (f32, f32), salt: u32) -> [f64; 2] {
    let mut scene = scene_with(
        ctx,
        VoxelConfig {
            grid: 32,
            rules: RuleList::one(rule),
            seed_radius: seed.0,
            seed_fill: seed.1,
            salt,
            ..VoxelConfig::default()
        },
    );
    let mut driver = Driver::new(ctx);
    let mut out = [0.0; 2];
    let mut generations = 0;
    for (slot, at) in out.iter_mut().zip([100, 400]) {
        while generations < at {
            // 120 generations a second for 1/30 s: exactly 4 a frame.
            generations += driver.frame(&mut scene, 1.0 / 30.0, &[("step_rate", MAX_STEP_RATE)]);
        }
        assert_eq!(generations, at, "the run overshot generation {at}");
        let cells = read_cells(ctx, &scene);
        *slot = live(&cells) as f64 / cells.len() as f64;
    }
    out
}

/// **Every roster rule lives from its own seed**: at 32³, from the standard
/// seed (the rule's own ball, the preset salt 7), its live fraction is
/// strictly between 0 and 0.9 at generations 100 and 400 — neither dead nor
/// saturated. A structural statistic, not a pixel comparison (ADR-0180 rule 3).
#[test]
fn every_roster_rule_lives_from_its_own_seed() {
    let Some(ctx) = context() else {
        return;
    };
    for rule in RosterRule::ALL {
        let [at_100, at_400] = live_fractions(&ctx, rule.rule(), rule.seed(), 7);
        println!(
            "{:<12} seed {:?}: live {at_100:.4} at 100, {at_400:.4} at 400",
            rule.as_str(),
            rule.seed()
        );
        for (at, fraction) in [(100, at_100), (400, at_400)] {
            assert!(
                fraction > 0.0 && fraction < 0.9,
                "{} is at live fraction {fraction:.4} at generation {at}, outside (0, 0.9)",
                rule.as_str()
            );
        }
    }
}

/// **The bound `rule` index picks from the list**: rounded, held inside it, and
/// the first rule where it is not finite.
#[test]
fn the_rule_index_picks_from_the_list_and_holds_inside_it() {
    let rules: Vec<Rule> = [RosterRule::Clouds, RosterRule::Coral, RosterRule::Crystal]
        .iter()
        .map(|r| r.rule())
        .collect();
    let list = RuleList::new(&rules).expect("three rules make a list");
    assert_eq!(list.pick(0.0), rules[0]);
    assert_eq!(list.pick(1.0), rules[1]);
    assert_eq!(list.pick(2.0), rules[2]);
    assert_eq!(list.pick(7.0), rules[2], "past the end runs the last");
    assert_eq!(list.pick(-3.0), rules[0]);
    assert_eq!(list.pick(1.4), rules[1]);
    assert_eq!(list.pick(f32::NAN), rules[0]);
    assert!(RuleList::new(&[]).is_none());
    assert!(RuleList::new(&[rules[0]; rules::MAX_RULES + 1]).is_none());
}

/// **A cell in a decay stage past the new rule's states falls to dead** at the
/// first generation of the new rule: a long-decaying rule switched to a
/// two-state one leaves no decaying cell behind.
#[test]
fn a_rule_change_drops_decay_stages_the_new_rule_does_not_have() {
    let Some(ctx) = context() else {
        return;
    };
    let rules = [RosterRule::Pyroclastic.rule(), RosterRule::Clouds.rule()];
    let (seed_radius, seed_fill) = RosterRule::Pyroclastic.seed();
    let mut scene = scene_with(
        &ctx,
        VoxelConfig {
            grid: 32,
            rules: RuleList::new(&rules).expect("a list"),
            seed_radius,
            seed_fill,
            salt: 7,
            ..VoxelConfig::default()
        },
    );
    let mut driver = Driver::new(&ctx);
    for _ in 0..10 {
        driver.frame(&mut scene, 1.0 / 30.0, &[("step_rate", MAX_STEP_RATE)]);
    }
    let decaying = |cells: &[u32]| cells.iter().filter(|c| **c & 0xFF >= 2).count();
    let before = decaying(&read_cells(&ctx, &scene));
    // One generation under the two-state rule.
    driver.frame(
        &mut scene,
        1.0 / 30.0,
        &[("step_rate", 30.0), ("rule", 1.0)],
    );
    let after = decaying(&read_cells(&ctx, &scene));
    println!("{before} decaying cells under pyroclastic, {after} after one clouds generation");
    assert!(before > 0, "pyroclastic left no decay to drop");
    assert_eq!(
        after, 0,
        "a decay stage outlived the switch to a two-state rule"
    );
}

// ---------------------------------------------------------------------------
// The reseed ball
// ---------------------------------------------------------------------------

/// **A `reseed` rise changes cells only inside the ball**: with the clock
/// frozen, the frame a rise lands on changes some cells, and every cell it
/// changes lies inside the ball the rise count and the salt hash to.
#[test]
fn a_reseed_rise_changes_cells_only_inside_its_ball() {
    let Some(ctx) = context() else {
        return;
    };
    let salt = 5;
    let mut scene = scene_with(&ctx, config(32, RosterRule::Crystal, salt));
    let mut driver = Driver::new(&ctx);
    let frozen = [("step_rate", 0.0), ("reseed_radius", 0.5)];
    driver.frame(&mut scene, 1.0 / 60.0, &frozen);
    let before = read_cells(&ctx, &scene);
    driver.frame(
        &mut scene,
        1.0 / 60.0,
        &[("step_rate", 0.0), ("reseed_radius", 0.5), ("reseed", 1.0)],
    );
    let after = read_cells(&ctx, &scene);
    let ball = stamp(salt, 0, 32, 0.5);
    let mut changed = 0;
    for (i, (a, b)) in before.iter().zip(&after).enumerate() {
        if a == b {
            continue;
        }
        changed += 1;
        let c = [i % 32, (i / 32) % 32, i / (32 * 32)];
        let d2: u32 = c
            .iter()
            .zip(ball.centre)
            .map(|(&c, centre)| {
                let d = (c as i64 - i64::from(centre)).unsigned_abs() as u32;
                d * d
            })
            .sum();
        assert!(
            d2 <= ball.radius_sq,
            "cell {c:?} changed {d2} cells² from the ball's centre {:?}, past its radius² {}",
            ball.centre,
            ball.radius_sq
        );
    }
    println!(
        "the reseed changed {changed} cells inside a ball at {:?}, radius² {}",
        ball.centre, ball.radius_sq
    );
    assert!(changed > 0, "the reseed changed nothing");

    // Held high, it does not fire again.
    driver.frame(
        &mut scene,
        1.0 / 60.0,
        &[("step_rate", 0.0), ("reseed_radius", 0.5), ("reseed", 1.0)],
    );
    assert_eq!(
        read_cells(&ctx, &scene),
        after,
        "a held reseed refilled again"
    );
}

/// The balls are a pure function of the salt and the rise count, and differ
/// from one rise to the next.
#[test]
fn reseed_balls_are_hashed_from_the_rise_count_and_the_salt() {
    assert_eq!(stamp(3, 0, 64, 0.25), stamp(3, 0, 64, 0.25));
    assert_ne!(stamp(3, 0, 64, 0.25).centre, stamp(3, 1, 64, 0.25).centre);
    assert_ne!(stamp(3, 0, 64, 0.25).centre, stamp(4, 0, 64, 0.25).centre);
    let ball = stamp(3, 0, 64, 0.25);
    assert!(ball.centre.iter().all(|&c| c < 64));
    assert_eq!(
        ball.radius_sq, 64,
        "a quarter of the half-side of 64 is 8 cells"
    );
}

// ---------------------------------------------------------------------------
// The shells
// ---------------------------------------------------------------------------

/// **The spectrum is laid across the shells bass first**, each shell the mean of
/// its share of the bins, and a shell count of zero lights none.
#[test]
fn the_spectrum_is_laid_across_the_shells_bass_first() {
    let mut spectrum = [0.0_f32; crate::dsp::SPECTRUM_BINS];
    for (i, bin) in spectrum.iter_mut().enumerate() {
        *bin = i as f32;
    }
    let levels = shell_levels(&spectrum, 4);
    // 64 bins over 4 shells: 0..16, 16..32, ...
    assert_eq!(&levels[..4], &[7.5, 23.5, 39.5, 55.5]);
    assert!(levels[4..].iter().all(|l| *l == 0.0));
    assert!(shell_levels(&spectrum, 0).iter().all(|l| *l == 0.0));
    spectrum[0] = f32::NAN;
    assert_eq!(
        shell_levels(&spectrum, 64)[0],
        0.0,
        "a NaN bin lights nothing"
    );
}

// ---------------------------------------------------------------------------
// Cost: the cap, the bricks and the march target
// ---------------------------------------------------------------------------

/// **A grid past the cap is clamped with a notice**: `configure` runs the cube
/// at the cap and returns the clamp, naming what was asked and what it runs
/// at; a grid at the cap is no clamp.
#[test]
fn a_grid_past_the_cap_is_clamped_with_a_notice() {
    let Some(ctx) = context() else {
        return;
    };
    let floor = crate::render::TierConfig::FLOOR;
    let mut scene = VoxelScene::new(
        &ctx.device,
        crate::render::COMPOSITE_FORMAT,
        floor.voxel_grid,
        floor.voxel_march_scale,
        floor.voxel_march_cap,
    );
    let notice = scene
        .configure(&GeneratorConfig::Voxel(config(128, RosterRule::Clouds, 1)))
        .expect("a grid past the cap must be announced, not silent");
    println!("{notice}");
    assert_eq!(
        scene.config.grid, floor.voxel_grid,
        "the cube does not run at the cap"
    );
    assert_eq!(
        notice.context,
        crate::render::scenes::OverflowContext::Voxels(128)
    );
    let text = notice.to_string();
    assert!(text.contains("grid 128") && text.contains("64"), "{text}");
    assert!(
        text.contains("pin --tier rich"),
        "Rich runs 128, so it is the remedy: {text}"
    );
    assert_eq!(
        scene.configure(&GeneratorConfig::Voxel(config(64, RosterRule::Clouds, 1))),
        None,
        "asking for exactly the cap is not a clamp"
    );
}

/// **Jumping empty bricks moves no pixel**: the sparse cost fixture's volume
/// — a thin scatter through a ball, so most of every ray crosses empty bricks
/// — marched with the jump and without it, through several orbits, draws
/// byte-identical frames.
#[test]
fn jumping_empty_bricks_moves_no_pixel() {
    let Some(ctx) = context() else {
        return;
    };
    let sparse = VoxelConfig {
        grid: 64,
        rules: RuleList::one(RosterRule::Crystal.rule()),
        seed_radius: 0.9,
        seed_fill: 0.05,
        salt: 11,
        ..VoxelConfig::default()
    };
    for (yaw, pitch) in [(0.6, 0.4), (0.0, 0.0), (2.3, -0.7), (-1.1, 1.2)] {
        let mut frames = Vec::new();
        for skip in [true, false] {
            let mut scene = scene_with(&ctx, sparse);
            scene.skip_bricks = skip;
            let mut driver = Driver::new(&ctx);
            driver.frame(
                &mut scene,
                1.0 / 60.0,
                &[
                    ("step_rate", 0.0),
                    ("yaw", yaw),
                    ("pitch", pitch),
                    ("density", 1.0),
                ],
            );
            frames.push(driver.read_target());
        }
        let lit = frames[0]
            .iter()
            .filter(|p| p[0] + p[1] + p[2] > 0.0)
            .count();
        let differing = frames[0]
            .iter()
            .zip(&frames[1])
            .filter(|(a, b)| a != b)
            .count();
        println!("yaw {yaw}, pitch {pitch}: {lit} pixels lit, {differing} differ");
        assert!(
            lit > 0,
            "the sparse volume drew nothing, so the comparison is empty"
        );
        assert_eq!(
            differing, 0,
            "jumping empty bricks moved {differing} pixels"
        );
    }
}

/// **The march target is the tier's fraction of the render target, held to its
/// cap by one factor**, so its aspect follows the target's (ADR-0037) and a
/// scale below 1 draws fewer pixels.
#[test]
fn the_march_target_is_a_fraction_of_the_render_target_held_to_its_cap() {
    assert_eq!(march_size((1920, 1080), 1.0, (1920, 1080)), (1920, 1080));
    // 540 rounds to the nearest step of 8.
    assert_eq!(march_size((1920, 1080), 0.5, (1920, 1080)), (960, 544));
    // A 4K target held to a 1080p cap keeps its 16:9.
    assert_eq!(march_size((3840, 2160), 1.0, (1920, 1080)), (1920, 1080));
    // The renderer's own scale composes with the tier's.
    assert_eq!(
        march_size((1920, 1080), 0.5 * 0.75, (1920, 1080)),
        march_size((1920, 1080), 0.375, (1920, 1080))
    );
    // A product under the smallest grid scale is held there, never zero.
    let (w, h) = march_size((1920, 1080), 0.01, (1920, 1080));
    assert!(w > 0 && h > 0);
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

/// No expression in the march indexes a vector by a runtime value. FXC, the
/// DX12 compiler, unrolls a loop whose body does, and fails the pipeline when
/// the loop is too long to unroll; this holds that rule on a runner with no
/// DX12. An array indexed at run time is not a vector and stays allowed.
#[test]
fn the_march_never_indexes_a_vector_at_run_time() {
    use wgpu::naga;
    let source = format!("{}{}", gpu::FULLSCREEN_VS_NDC, march_body());
    let module = naga::front::wgsl::parse_str(&source).expect("the march parses");
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::default(),
    )
    .validate(&module)
    .expect("the march validates");

    let mut offenders = Vec::new();
    let functions = module.functions.iter().map(|(h, f)| (f, &info[h])).chain(
        module
            .entry_points
            .iter()
            .enumerate()
            .map(|(i, ep)| (&ep.function, info.get_entry_point(i))),
    );
    for (function, fn_info) in functions {
        for (_, expr) in function.expressions.iter() {
            let naga::Expression::Access { base, .. } = *expr else {
                continue;
            };
            let vector = match *fn_info[base].ty.inner_with(&module.types) {
                naga::TypeInner::Vector { .. } => true,
                naga::TypeInner::ValuePointer { size, .. } => size.is_some(),
                naga::TypeInner::Pointer { base, .. } => {
                    matches!(module.types[base].inner, naga::TypeInner::Vector { .. })
                }
                _ => false,
            };
            if vector {
                offenders.push(function.name.clone().unwrap_or_default());
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "a vector indexed at run time in: {offenders:?}"
    );
}
