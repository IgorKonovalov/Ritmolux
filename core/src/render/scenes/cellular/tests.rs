//! The cellular system's contracts: the family roster, the quantizers between a
//! bound value and the shader, the generation clock, the reseed edge, and the
//! automaton itself — read back off the GPU field and held to
//! [`mirror`](super::mirror), the rule written out on the CPU.
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

use super::mirror;
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
fn scene_with(ctx: &RenderContext, config: CellularConfig) -> CellularScene {
    // `COMPOSITE_FORMAT`, not the surface format: a scene draws into the
    // composite chain's linear target (`scenes::create_all`).
    // Every radius the family declares, so a probe's radius is its own and
    // never the tier's; the tier's cap has its own test.
    let mut scene = CellularScene::new(
        &ctx.device,
        crate::render::COMPOSITE_FORMAT,
        MAX_RADIUS as u32,
    );
    scene.configure(&GeneratorConfig::Cellular(config));
    scene
}

fn ltl(grid: u32, wrap: bool, salt: u32) -> CellularConfig {
    CellularConfig {
        family: CellularFamily::LargerThanLife,
        grid,
        wrap,
        salt,
    }
}

fn life(grid: u32, wrap: bool, salt: u32) -> CellularConfig {
    CellularConfig {
        family: CellularFamily::LifeLike,
        grid,
        wrap,
        salt,
    }
}

/// Drives a scene through the renderer's per-frame order — `set_time`,
/// `advance`, `reset_params`, `set_param`, `update`, `render` — into a small
/// offscreen target.
struct Driver<'a> {
    ctx: &'a RenderContext,
    target: wgpu::Texture,
    view: wgpu::TextureView,
    time: f32,
}

const TARGET: u32 = 32;

impl<'a> Driver<'a> {
    fn new(ctx: &'a RenderContext) -> Self {
        let (target, view) =
            capture::create_target(&ctx.device, crate::render::COMPOSITE_FORMAT, TARGET, TARGET);
        Self {
            ctx,
            target,
            view,
            time: 0.0,
        }
    }

    /// The last frame's present, `TARGET` pixels a side, as linear RGBA.
    fn read_target(&self) -> Vec<[f32; 4]> {
        let (buffer, padded_bpr) =
            capture::create_linear_readback(&self.ctx.device, TARGET, TARGET);
        let mut encoder = self
            .ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("cellular-target-readback"),
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

    /// One frame of `dt` seconds with `params` bound; returns how many
    /// generations it ran.
    fn frame(&mut self, scene: &mut CellularScene, dt: f32, params: &[(&str, f32)]) -> u32 {
        scene.set_target_size(TARGET, TARGET);
        scene.set_time(self.time);
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
                label: Some("cellular-probe"),
            });
        capture::record_clear(&mut encoder, &self.view);
        scene.render(&self.ctx.queue, &mut encoder, &self.view, 1.0);
        self.ctx.queue.submit(std::iter::once(encoder.finish()));
        self.time += dt;
        generations
    }
}

/// The field's four channels, row-major from the top-left, read off the texture
/// the next pass would sample.
fn read_texels(ctx: &RenderContext, scene: &CellularScene) -> Vec<f32> {
    let res = scene
        .res
        .as_ref()
        .expect("the first render builds the resources");
    let n = res.grid;
    let (buffer, padded_bpr) = capture::create_linear_readback(&ctx.device, n, n);
    let mut encoder = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("cellular-readback"),
        });
    capture::record_copy(
        &mut encoder,
        res.field.read_texture(),
        &buffer,
        padded_bpr,
        n,
        n,
    );
    ctx.queue.submit(std::iter::once(encoder.finish()));
    capture::read_back_linear(&ctx.device, &buffer, n, n, padded_bpr).expect("the field reads back")
}

/// The state channel as `0`/`1` cells.
fn states(ctx: &RenderContext, scene: &CellularScene) -> Vec<u8> {
    read_texels(ctx, scene)
        .chunks_exact(4)
        .map(|t| u8::from(t[0] > 0.5))
        .collect()
}

/// Assert two fields are the same cell for cell, reporting how many cells and
/// how many live ones each has rather than printing a million-entry vector.
#[track_caller]
fn assert_same_field(gpu: &[u8], cpu: &[u8], what: &str) {
    assert_eq!(gpu.len(), cpu.len(), "{what}: the fields differ in size");
    let differing = gpu.iter().zip(cpu).filter(|(a, b)| a != b).count();
    let live = |f: &[u8]| f.iter().filter(|c| **c == 1).count();
    assert!(
        differing == 0,
        "{what}: {differing} of {} cells differ ({} live on the GPU, {} predicted)",
        gpu.len(),
        live(gpu),
        live(cpu)
    );
}

/// The live cells of an `n`-wide field as sorted `(x, y)` pairs.
fn live_cells(cells: &[u8], n: u32) -> Vec<(u32, u32)> {
    let mut out: Vec<(u32, u32)> = cells
        .iter()
        .enumerate()
        .filter(|(_, c)| **c == 1)
        .map(|(i, _)| (i as u32 % n, i as u32 / n))
        .collect();
    out.sort_unstable();
    out
}

/// Overwrite the field with exactly `cells` live and every other cell dead —
/// at age 0 and at the age cap respectively, as a seed leaves them.
///
/// A test-only pass with **no bind group at all** — the cells are constants in
/// its source — so it adds no layout for ADR-0058's guard to weigh against the
/// scene's own. Call after a first frame has built the resources.
fn plant(ctx: &RenderContext, scene: &mut CellularScene, cells: &[(u32, u32)]) {
    let tests: String = cells
        .iter()
        .map(|(x, y)| {
            format!(
                "    if (c.x == {x}u && c.y == {y}u) {{ v = vec4<f32>(1.0, 0.0, 0.0, 1.0); }}\n"
            )
        })
        .collect();
    let body = format!(
        "@fragment\nfn fs_main(in: VsOut) -> @location(0) vec4<f32> {{\n    \
         let c = vec2<u32>(in.pos.xy);\n    var v = vec4<f32>(0.0, {AGE_CAP:?}, 0.0, 1.0);\n{tests}    \
         return v;\n}}\n"
    );
    let shader = gpu::fullscreen_shader(
        &ctx.device,
        "cellular-plant",
        gpu::FULLSCREEN_VS_UV_FLIPPED,
        &body,
    );
    let pipeline = gpu::fullscreen_pipeline(
        &ctx.device,
        &shader,
        &[],
        PingPongField::FORMAT,
        wgpu::BlendState::REPLACE,
        "cellular-plant",
    );
    let res = scene.res.as_ref().expect("plant after the first render");
    let mut encoder = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("cellular-plant"),
        });
    {
        let mut pass = gpu::color_pass(
            &mut encoder,
            "cellular-plant",
            res.field.read_view(),
            wgpu::LoadOp::Clear(wgpu::Color::BLACK),
        );
        pass.set_pipeline(&pipeline);
        pass.draw(0..3, 0..1);
    }
    ctx.queue.submit(std::iter::once(encoder.finish()));
}

/// `dt` and `step_rate` that run exactly one generation a frame: both exact
/// binary fractions, so no slack is spent.
const ONE_GEN: (f32, f32) = (0.25, 4.0);

// ---------------------------------------------------------------------------
// The shaders and the roster
// ---------------------------------------------------------------------------

/// Both passes' WGSL — preludes and all — parse and validate through naga, so a
/// shader error fails here with its own diagnostic.
#[test]
fn the_shaders_are_valid_wgsl() {
    for pass in [StepPass::Seed, StepPass::Stamp, StepPass::Step] {
        let step = format!("{}{}", gpu::FULLSCREEN_VS_UV_FLIPPED, pass.source());
        if let Err(e) = crate::milk::shader::validate_wgsl(&step) {
            panic!("the {} shader does not validate:\n{e}", pass.label());
        }
    }
    let present = format!(
        "{}{}",
        gpu::FULLSCREEN_VS_UV_FLIPPED,
        shader::PRESENT_SHADER
    );
    if let Err(e) = crate::milk::shader::validate_wgsl(&present) {
        panic!("the present shader does not validate:\n{e}");
    }
}

/// Every pipeline and bind group builds on the adapter without a validation
/// error — the half naga cannot vouch for.
#[test]
fn the_resources_build_on_the_adapter() {
    let Some(ctx) = context() else {
        return;
    };
    let scope = ctx.device.push_error_scope(wgpu::ErrorFilter::Validation);
    let res = Resources::build(&ctx.device, crate::render::COMPOSITE_FORMAT, 64);
    if let Some(error) = pollster::block_on(scope.pop()) {
        panic!("building the cellular resources raised: {error}");
    }
    drop(res);
}

/// Every family round-trips through its name, and the shader index is its
/// position in the roster.
#[test]
fn every_family_round_trips_and_indexes_by_roster_position() {
    for (position, family) in CellularFamily::ALL.into_iter().enumerate() {
        assert_eq!(CellularFamily::from_name(family.as_str()), Some(family));
        assert_eq!(family.index() as usize, position, "{family:?}");
    }
    assert_eq!(
        CellularFamily::from_name("Life_Like"),
        None,
        "names are exact"
    );
    assert_eq!(CellularFamily::from_name(""), None);
    assert_eq!(CellularConfig::default().family, CellularFamily::LifeLike);
    assert_eq!(CellularConfig::default().grid, DEFAULT_GRID);
    assert!(CellularConfig::default().wrap);
}

/// A bound rule reaches the shader as a whole mask inside the nine-bit range,
/// and the Structural quantizer upstream composes with it to the identity.
#[test]
fn a_bound_rule_is_clamped_and_rounded_before_the_shader() {
    for (value, applied) in [
        (8.0, 8),
        (12.4, 12),
        (11.6, 12),
        (-3.0, 0),
        (511.0, 511),
        (9000.0, 511),
    ] {
        assert_eq!(applied_rule(value, 8.0), applied, "{value}");
    }
    assert_eq!(applied_rule(f32::NAN, 8.0), 8);
    assert_eq!(applied_rule(f32::INFINITY, 12.0), 12);
    for i in -40..600 {
        let v = i as f32 * 1.37;
        assert_eq!(
            applied_rule(ParamKind::Structural.quantize(v), 8.0),
            applied_rule(v, 8.0),
            "quantizing {v} before the scene must change nothing it applies"
        );
    }
}

// ---------------------------------------------------------------------------
// The generation clock
// ---------------------------------------------------------------------------

/// **`step_rate` decouples the automaton from the frame rate**: the same wall
/// time runs the same number of generations at every refresh rate, for every
/// rate — including rates and spans that are not whole multiples of a frame.
#[test]
fn the_same_wall_time_runs_the_same_generations_at_any_frame_rate() {
    let total = |rate: f32, fps: u32, seconds: u32| -> u32 {
        let mut clock = GenerationClock::default();
        let dt = 1.0 / fps as f32;
        (0..fps * seconds).map(|_| clock.advance(rate, dt)).sum()
    };
    for rate in [1.0_f32, 5.0, 10.0, 12.0, 24.0, 30.0, 60.0] {
        let expected = (rate * 5.0).round() as u32;
        for fps in [24, 30, 50, 60, 75, 120, 144, 165, 240] {
            // Above `MAX_GENERATIONS_PER_FRAME * fps` the cap binds, which is a
            // stall's behaviour and not this claim's.
            if rate > (MAX_GENERATIONS_PER_FRAME * fps) as f32 {
                continue;
            }
            assert_eq!(
                total(rate, fps, 5),
                expected,
                "{rate} generations/s for 5 s at {fps} fps"
            );
        }
    }
    // The done-when's own pair, stated plainly.
    assert_eq!(total(12.0, 30, 1), 12);
    assert_eq!(total(12.0, 144, 1), 12);
}

/// A zero rate freezes, a non-finite one falls back to the default, and a stall
/// runs at most the cap and then drops its backlog rather than racing.
#[test]
fn the_clock_freezes_at_zero_and_drops_a_stalls_backlog() {
    let mut clock = GenerationClock::default();
    assert_eq!(clock.advance(0.0, 10.0), 0);
    assert_eq!(clock.advance(-5.0, 1.0), 0, "a negative rate is zero");

    let mut clock = GenerationClock::default();
    let n: u32 = (0..60).map(|_| clock.advance(f32::NAN, 1.0 / 60.0)).sum();
    assert_eq!(n, DEFAULT_STEP_RATE as u32, "a NaN rate runs the default");

    // A 2 s stall at 30 generations/s owes 60; one frame runs the cap, and the
    // next ordinary frame runs what an ordinary frame owes, not the rest.
    let mut clock = GenerationClock::default();
    assert_eq!(clock.advance(30.0, 2.0), MAX_GENERATIONS_PER_FRAME);
    assert_eq!(clock.advance(30.0, 0.1), 3);
    // A non-finite dt runs nothing rather than poisoning the sum.
    assert_eq!(clock.advance(30.0, f32::INFINITY), 0);
    assert_eq!(clock.advance(30.0, 0.1), 3);
}

// ---------------------------------------------------------------------------
// The reseed edge
// ---------------------------------------------------------------------------

/// **`reseed` fires once per rising edge, not once per frame above
/// threshold** — and a NaN neither fires nor arms a spurious edge.
#[test]
fn reseed_fires_once_per_rising_edge() {
    let Some(ctx) = context() else {
        return;
    };
    let mut scene = scene_with(&ctx, life(64, true, 9));
    let sequence = [
        0.0,
        0.7,
        0.9,
        1.0,
        0.2,
        0.6,
        0.6,
        0.4,
        0.5,
        f32::NAN,
        1.0,
        1.0,
    ];
    let mut fired = Vec::new();
    for (i, value) in sequence.into_iter().enumerate() {
        scene.advance(1.0 / 60.0);
        scene.reset_params();
        scene.set_param("reseed", value);
        scene.update(&AnalysisFrame::default());
        if scene.pending_stamp.take().is_some() {
            fired.push(i);
        }
    }
    assert_eq!(
        fired,
        vec![1, 5, 8, 10],
        "fires on 0->0.7, 0.2->0.6, 0.4->0.5 and NaN->1.0, never while held"
    );

    // The discs come off the preset's own stream: the same salt replays them.
    let mut again = scene_with(&ctx, life(64, true, 9));
    let mut rng = stamp_rng(9);
    for _ in 0..3 {
        again.advance(1.0 / 60.0);
        again.reset_params();
        again.set_param("reseed", 0.0);
        again.update(&AnalysisFrame::default());
        again.reset_params();
        again.set_param("reseed", 1.0);
        again.update(&AnalysisFrame::default());
        assert_eq!(again.pending_stamp.take(), Some(next_stamp(&mut rng, 64)));
    }
}

/// A reseed changes exactly the disc it names — cell for cell what the CPU
/// statement of the disc predicts — and a value held high for several frames
/// refills it once, exactly as a one-frame pulse does.
///
/// Each run builds, drives and drops its own scene before the next begins.
/// Several instances of this scene live at once on the WARP adapter are handed
/// one another's resources — four interleaved, one run's disc landed on
/// another's field — which is ADR-0058's hazard between instances of one
/// layout rather than between two layouts, and the renderer never builds more
/// than one of this scene per preset and its layer.
#[test]
fn a_reseed_refills_exactly_one_disc() {
    let Some(ctx) = context() else {
        return;
    };
    const N: u32 = 64;
    let config = life(N, true, 21);
    let mut driver = Driver::new(&ctx);
    let mut run = |reseed: &dyn Fn(i32) -> f32| -> Vec<u8> {
        let mut scene = scene_with(&ctx, config);
        for i in 0..8 {
            driver.frame(
                &mut scene,
                0.1,
                &[("step_rate", 0.0), ("reseed", reseed(i))],
            );
        }
        states(&ctx, &scene)
    };
    let before = run(&|_| 0.0);
    let after = run(&|i| if (2..7).contains(&i) { 1.0 } else { 0.0 });
    let pulsed = run(&|i| if i == 2 { 1.0 } else { 0.0 });
    let disc = next_stamp(&mut stamp_rng(21), N);
    let predicted = mirror::stamp(&before, N, true, disc, live_threshold(LIFE_DENSITY));
    let changed = before.iter().zip(&after).filter(|(a, b)| a != b).count();
    println!("reseed disc at {:?}: {changed} cells changed", disc.centre);
    assert!(changed > 50, "the disc changed only {changed} cells");
    assert_same_field(
        &after,
        &predicted,
        "the refilled field against the predicted disc",
    );
    assert_same_field(
        &pulsed,
        &after,
        "five frames held high against a one-frame pulse",
    );
}

// ---------------------------------------------------------------------------
// The automaton
// ---------------------------------------------------------------------------

/// **`birth = 8`, `survive = 12` is Conway's Life**: a glider planted at a
/// known cell travels one cell diagonally per four generations, for ten
/// periods — and a different rule in the same space does not carry it, so the
/// bound rule is what the shader runs.
#[test]
fn a_planted_glider_travels_one_cell_diagonally_every_four_generations() {
    let Some(ctx) = context() else {
        return;
    };
    const N: u32 = 32;
    // The glider that travels toward +x, +y (down-right, rows counting down).
    const SHAPE: [(u32, u32); 5] = [(1, 0), (2, 1), (0, 2), (1, 2), (2, 2)];
    let at = |dx: u32, dy: u32| -> Vec<(u32, u32)> {
        let mut out: Vec<(u32, u32)> = SHAPE
            .iter()
            .map(|(x, y)| ((x + 10 + dx) % N, (y + 10 + dy) % N))
            .collect();
        out.sort_unstable();
        out
    };
    let conway = [("step_rate", ONE_GEN.1), ("birth", 8.0), ("survive", 12.0)];

    let mut driver = Driver::new(&ctx);
    let mut scene = scene_with(&ctx, life(N, true, 1));
    driver.frame(&mut scene, ONE_GEN.0, &[("step_rate", 0.0)]);
    plant(&ctx, &mut scene, &at(0, 0));
    assert_eq!(live_cells(&states(&ctx, &scene), N), at(0, 0), "planted");

    for period in 1..=10u32 {
        for _ in 0..4 {
            assert_eq!(driver.frame(&mut scene, ONE_GEN.0, &conway), 1);
        }
        assert_eq!(
            live_cells(&states(&ctx, &scene), N),
            at(period, period),
            "after {} generations the glider is not where Conway's rule puts it",
            4 * period
        );
    }

    // The control: B3/S2 — one survival bit removed — does not carry a glider,
    // so the pass above was reading `survive` and not a hardcoded rule. One
    // instance at a time (see `a_reseed_refills_exactly_one_disc`).
    drop(scene);
    let mut control = scene_with(&ctx, life(N, true, 1));
    driver.frame(&mut control, ONE_GEN.0, &[("step_rate", 0.0)]);
    plant(&ctx, &mut control, &at(0, 0));
    for _ in 0..4 {
        driver.frame(
            &mut control,
            ONE_GEN.0,
            &[("step_rate", ONE_GEN.1), ("birth", 8.0), ("survive", 4.0)],
        );
    }
    assert_ne!(live_cells(&states(&ctx, &control), N), at(1, 1));
}

/// The whole field matches the rule written out on the CPU, generation by
/// generation, from a seeded field — on a torus and inside a dead border, for
/// Conway and for HighLife (B36/S23), so every bit of both masks and both edge
/// rules is exercised.
#[test]
fn the_field_matches_the_cpu_rule_generation_by_generation() {
    let Some(ctx) = context() else {
        return;
    };
    const N: u32 = 48;
    let mut driver = Driver::new(&ctx);
    for wrap in [true, false] {
        for (birth, survive) in [(8u32, 12u32), (72, 12)] {
            let mut scene = scene_with(&ctx, life(N, wrap, 3));
            driver.frame(&mut scene, ONE_GEN.0, &[("step_rate", 0.0)]);
            let mut cpu = mirror::seed_field(N, field_seed(3), live_threshold(LIFE_DENSITY));
            assert_same_field(
                &states(&ctx, &scene),
                &cpu,
                &format!("the seeded field (wrap {wrap})"),
            );

            let rule = [
                ("step_rate", ONE_GEN.1),
                ("birth", birth as f32),
                ("survive", survive as f32),
            ];
            for generation in 1..=40 {
                driver.frame(&mut scene, ONE_GEN.0, &rule);
                cpu = mirror::life_step(&cpu, N, wrap, birth, survive);
                if generation % 8 == 0 {
                    assert_same_field(
                        &states(&ctx, &scene),
                        &cpu,
                        &format!("B{birth}/S{survive}, wrap {wrap}, generation {generation}"),
                    );
                }
            }
            let live = cpu.iter().filter(|c| **c == 1).count();
            assert!(
                live > 20 && live < (N * N) as usize / 2,
                "{live} live cells after 40 generations is too degenerate a field to \
                 have tested the rule on"
            );
        }
    }
}

/// **Two runs with the same seed and the same analysis frames produce an
/// identical field**, reseeds and all — and a different salt does not, so the
/// equality is not two blank fields agreeing.
#[test]
fn the_same_seed_and_frames_produce_an_identical_field() {
    let Some(ctx) = context() else {
        return;
    };
    let mut driver = Driver::new(&ctx);
    let run = |driver: &mut Driver<'_>, salt: u32| -> Vec<f32> {
        let mut scene = scene_with(&ctx, life(96, true, salt));
        for i in 0..120 {
            let reseed = if i == 30 || (60..64).contains(&i) {
                1.0
            } else {
                0.0
            };
            driver.frame(
                &mut scene,
                1.0 / 60.0,
                &[("step_rate", 20.0), ("reseed", reseed)],
            );
        }
        read_texels(&ctx, &scene)
    };
    let one = run(&mut driver, 5);
    let two = run(&mut driver, 5);
    assert!(
        one == two,
        "two runs from one seed diverged — the field is not a pure function of its inputs"
    );
    let other = run(&mut driver, 6);
    let differing = one
        .chunks_exact(4)
        .zip(other.chunks_exact(4))
        .filter(|(a, b)| a[0] != b[0])
        .count();
    assert!(
        differing > 96 * 96 / 10,
        "salts 5 and 6 differ in only {differing} cells"
    );
}

/// The same wall time at 30 fps and at 144 fps lands on the **same field**,
/// not merely the same count — and half that time at 60 fps does not, so the
/// equality is the rate's doing.
#[test]
fn thirty_and_one_hundred_forty_four_fps_reach_the_same_field() {
    let Some(ctx) = context() else {
        return;
    };
    let mut driver = Driver::new(&ctx);
    let run = |driver: &mut Driver<'_>, fps: u32, frames: u32| -> (u32, Vec<u8>) {
        let mut scene = scene_with(&ctx, life(64, true, 11));
        let generations = (0..frames)
            .map(|_| driver.frame(&mut scene, 1.0 / fps as f32, &[("step_rate", 12.0)]))
            .sum();
        (generations, states(&ctx, &scene))
    };
    let (g30, f30) = run(&mut driver, 30, 30);
    let (g144, f144) = run(&mut driver, 144, 144);
    assert_eq!((g30, g144), (12, 12));
    assert_same_field(&f30, &f144, "one second at 30 fps against one at 144 fps");
    let (g_half, f_half) = run(&mut driver, 60, 30);
    assert_eq!(g_half, 6);
    assert_ne!(f_half, f30, "half the wall time reached the same field");
}

// ---------------------------------------------------------------------------
// The age channel
// ---------------------------------------------------------------------------

/// The field read as the mirror's [`mirror::Aged`]: state and age per cell.
fn aged(ctx: &RenderContext, scene: &CellularScene) -> mirror::Aged {
    let texels = read_texels(ctx, scene);
    mirror::Aged {
        state: texels
            .chunks_exact(4)
            .map(|t| u8::from(t[0] > 0.5))
            .collect(),
        age: texels.chunks_exact(4).map(|t| t[1]).collect(),
    }
}

/// **The age channel is generations since each cell last changed**, cell for
/// cell what the CPU statement predicts: through a seed, forty generations, a
/// reseed disc that is not a generation, and past it.
#[test]
fn the_age_channel_counts_generations_since_each_cell_changed() {
    let Some(ctx) = context() else {
        return;
    };
    const N: u32 = 48;
    let threshold = live_threshold(LIFE_DENSITY);
    let mut driver = Driver::new(&ctx);
    let mut scene = scene_with(&ctx, life(N, true, 4));
    driver.frame(&mut scene, ONE_GEN.0, &[("step_rate", 0.0)]);
    let mut cpu = mirror::Aged::seeded(mirror::seed_field(N, field_seed(4), threshold));
    assert!(aged(&ctx, &scene) == cpu, "the seeded field's ages");

    let conway = [("step_rate", ONE_GEN.1)];
    for generation in 1..=40 {
        driver.frame(&mut scene, ONE_GEN.0, &conway);
        cpu = cpu.then(mirror::life_step(&cpu.state, N, true, 8, 12), true);
        if generation % 10 == 0 {
            let gpu = aged(&ctx, &scene);
            let differing = gpu.age.iter().zip(&cpu.age).filter(|(a, b)| a != b).count();
            assert_same_field(&gpu.state, &cpu.state, &format!("generation {generation}"));
            assert_eq!(
                differing, 0,
                "generation {generation}: {differing} ages differ"
            );
        }
    }
    let young = cpu.age.iter().filter(|a| **a > 0.0 && **a < 40.0).count();
    assert!(
        young > 100,
        "only {young} cells carry a history to have tested"
    );

    // A disc on a frozen frame: its changed cells restart, the rest keep their
    // age exactly — a disc is not a generation.
    driver.frame(
        &mut scene,
        ONE_GEN.0,
        &[("step_rate", 0.0), ("reseed", 1.0)],
    );
    let disc = next_stamp(&mut stamp_rng(4), N);
    cpu = cpu.then(mirror::stamp(&cpu.state, N, true, disc, threshold), false);
    assert!(aged(&ctx, &scene) == cpu, "the ages after a reseed disc");
    driver.frame(&mut scene, ONE_GEN.0, &conway);
    cpu = cpu.then(mirror::life_step(&cpu.state, N, true, 8, 12), true);
    assert!(aged(&ctx, &scene) == cpu, "the generation after the disc");
}

/// Rec. 709 luminance of a linear pixel.
fn luminance(p: [f32; 4]) -> f32 {
    0.2126 * p[0] + 0.7152 * p[1] + 0.0722 * p[2]
}

/// **A moving glider leaves a fading wake whose length is `trail`**: behind it,
/// exactly the dead cells younger than `trail` light, each dimmer the older it
/// is, and a longer trail lights strictly more of them. `trail = 0` lights
/// none. The grid is drawn one cell to a pixel, so a pixel is a cell.
#[test]
fn a_moving_glider_leaves_a_wake_as_long_as_trail() {
    let Some(ctx) = context() else {
        return;
    };
    const SHAPE: [(u32, u32); 5] = [(1, 0), (2, 1), (0, 2), (1, 2), (2, 2)];
    let mut driver = Driver::new(&ctx);
    let mut scene = scene_with(&ctx, life(TARGET, true, 1));
    driver.frame(&mut scene, ONE_GEN.0, &[("step_rate", 0.0)]);
    let glider: Vec<(u32, u32)> = SHAPE.iter().map(|(x, y)| (x + 4, y + 4)).collect();
    plant(&ctx, &mut scene, &glider);
    for _ in 0..40 {
        driver.frame(&mut scene, ONE_GEN.0, &[("step_rate", ONE_GEN.1)]);
    }
    let field = aged(&ctx, &scene);

    let mut wakes = Vec::new();
    for trail in [0.0_f32, 3.0, 8.0, 20.0] {
        driver.frame(
            &mut scene,
            ONE_GEN.0,
            &[("step_rate", 0.0), ("trail", trail), ("age_tint", 0.0)],
        );
        let image = driver.read_target();
        let mut wake: Vec<(f32, f32)> = Vec::new();
        for (i, pixel) in image.iter().enumerate() {
            let lit = luminance(*pixel) > 1e-4;
            if field.state[i] == 1 {
                assert!(lit, "trail {trail}: a live cell is dark");
                continue;
            }
            let age = field.age[i];
            assert_eq!(
                lit,
                age < trail,
                "trail {trail}: a dead cell of age {age} is {}",
                if lit { "lit" } else { "dark" }
            );
            if lit {
                wake.push((age, luminance(*pixel)));
            }
        }
        // Dimmer with age: any two wake cells of different ages are ordered.
        for (a, la) in &wake {
            for (b, lb) in &wake {
                if a < b {
                    assert!(
                        la > lb,
                        "trail {trail}: age {a} at {la} is not brighter than age {b} at {lb}"
                    );
                }
            }
        }
        println!("trail {trail}: {} wake cells", wake.len());
        wakes.push(wake.len());
    }
    assert_eq!(wakes[0], 0, "trail 0 drew a wake");
    assert!(
        wakes[1] > 0 && wakes[1] < wakes[2] && wakes[2] < wakes[3],
        "the wake does not lengthen with trail: {wakes:?}"
    );
}

/// **`trail = 0` is the binary field exactly**: every pixel is either the live
/// colour or untouched — no light and no coverage — whatever `age_tint` asks.
/// The rostered golden fixture binds `trail = 0` and its baseline predates the
/// age channel, which holds the same claim against the whole present.
#[test]
fn trail_zero_draws_the_binary_field() {
    let Some(ctx) = context() else {
        return;
    };
    let mut driver = Driver::new(&ctx);
    let mut scene = scene_with(&ctx, life(TARGET, true, 2));
    for _ in 0..12 {
        driver.frame(
            &mut scene,
            ONE_GEN.0,
            &[("step_rate", ONE_GEN.1), ("trail", 0.0)],
        );
    }
    driver.frame(
        &mut scene,
        ONE_GEN.0,
        &[("step_rate", 0.0), ("trail", 0.0), ("age_tint", 0.8)],
    );
    let field = aged(&ctx, &scene);
    let image = driver.read_target();
    let live_colour = image
        .iter()
        .zip(&field.state)
        .find(|(_, s)| **s == 1)
        .map(|(p, _)| *p)
        .expect("some cell is live");
    let recent = field.age.iter().filter(|a| **a < 4.0).count();
    assert!(
        recent > 20,
        "only {recent} cells died recently enough to tempt a wake"
    );
    for (pixel, state) in image.iter().zip(&field.state) {
        if *state == 1 {
            assert_eq!(*pixel, live_colour, "a live cell is not the live colour");
        } else {
            // The driver clears to opaque black, and a premultiplied draw of no
            // light and no coverage leaves that exactly as it was.
            assert_eq!(
                *pixel,
                [0.0, 0.0, 0.0, 1.0],
                "a dead cell drew something at trail 0"
            );
        }
    }
}

/// **The palette's A/B crossfade and `palette_steps` act on this coordinate as
/// they do on every other scene's**: every pixel is the palette pair sampled
/// at its cell's coordinate — `hue`, plus `age_tint` times how far its wake has
/// faded — banded by `palette::band_coord` and crossfaded by `palette_mix`,
/// the CPU statements the shared WGSL mirrors, times its light.
#[test]
fn every_pixel_is_the_palette_at_its_age_coordinate() {
    use crate::render::palette::{NamedPalette, PaletteConfig};
    let Some(ctx) = context() else {
        return;
    };
    let pair = Palette::bake_pair(
        &PaletteConfig::default_spectrum(),
        &PaletteConfig::Named(NamedPalette::from_name("ice").expect("ice is a palette")),
    );
    let mut driver = Driver::new(&ctx);
    let mut scene = scene_with(&ctx, life(TARGET, true, 6));
    scene.set_palette(&pair);
    for _ in 0..16 {
        driver.frame(&mut scene, ONE_GEN.0, &[("step_rate", ONE_GEN.1)]);
    }
    let field = aged(&ctx, &scene);
    let (trail, tint, hue) = (10.0_f32, 0.6_f32, 0.15_f32);

    let mut worst = 0.0_f32;
    let mut banded_colours = std::collections::BTreeSet::new();
    let mut smooth_colours = std::collections::BTreeSet::new();
    for (mix, steps) in [
        (0.0_f32, 0.0_f32),
        (1.0, 0.0),
        (0.4, 0.0),
        (0.0, 4.0),
        (0.7, 4.0),
    ] {
        driver.frame(
            &mut scene,
            ONE_GEN.0,
            &[
                ("step_rate", 0.0),
                ("trail", trail),
                ("age_tint", tint),
                ("hue", hue),
                ("palette_mix", mix),
                ("palette_steps", steps),
            ],
        );
        let image = driver.read_target();
        for (i, pixel) in image.iter().enumerate() {
            let (coord, light) = if field.state[i] == 1 {
                (0.0, 1.0)
            } else {
                let fade = (1.0 - (field.age[i] + 1.0) / (trail + 1.0)).clamp(0.0, 1.0);
                (tint * (1.0 - fade), fade)
            };
            let t = palette::band_coord(coord + hue, palette::band_steps(steps));
            let rgb = pair.sample(t, mix);
            for c in 0..3 {
                worst = worst.max((pixel[c] - rgb[c] * light).abs());
            }
            if light > 0.0 && mix == 0.0 {
                let key = [
                    (pixel[0] / light * 64.0).round() as i32,
                    (pixel[1] / light * 64.0).round() as i32,
                    (pixel[2] / light * 64.0).round() as i32,
                ];
                if steps > 0.0 {
                    banded_colours.insert(key);
                } else {
                    smooth_colours.insert(key);
                }
            }
        }
    }
    println!(
        "worst channel error {worst:.4}; {} colours smooth, {} banded",
        smooth_colours.len(),
        banded_colours.len()
    );
    assert!(
        worst < 0.02,
        "a pixel is {worst} off the palette at its coordinate"
    );
    // Non-vacuity: the wake spans many colours when smooth and at most the
    // four bands (plus the live colour's own band) when stepped.
    assert!(
        smooth_colours.len() > 6,
        "the wake spans only {} colours",
        smooth_colours.len()
    );
    assert!(
        banded_colours.len() <= 4,
        "four bands drew {} colours",
        banded_colours.len()
    );
}

// ---------------------------------------------------------------------------
// larger_than_life
// ---------------------------------------------------------------------------

/// [`FAMILY_PARAMS`] is a statement about the engine, so it is held to it: each
/// row names a declared parameter once, lists every family by the name a preset
/// uses and in roster order, and carries a spec range some family reads.
#[test]
fn the_family_table_is_the_roster() {
    let families: Vec<&str> = CellularFamily::ALL.iter().map(|f| f.as_str()).collect();
    let mut seen = Vec::new();
    for row in FAMILY_PARAMS {
        assert!(!seen.contains(&row.name), "`{}` has two rows", row.name);
        seen.push(row.name);
        let spec = PARAMS
            .iter()
            .find(|spec| spec.name == row.name)
            .unwrap_or_else(|| panic!("`{}` is not a declared parameter", row.name));
        let listed: Vec<&str> = row.ranges.iter().map(|r| r.family).collect();
        assert_eq!(listed, families, "`{}` must list every family", row.name);
        assert!(
            row.ranges.iter().any(|r| r.range == spec.range),
            "`{}`'s spec range {:?} is no family's range",
            row.name,
            spec.range
        );
    }
    assert_eq!(
        crate::render::scenes::family_params("cellular"),
        FAMILY_PARAMS,
        "the reference must reach this table under the system's own label"
    );
}

/// A bound radius reaches the shader whole, inside `1..=cap`, and never past
/// the family's own top; the Structural quantizer composes with it.
#[test]
fn a_bound_radius_is_clamped_to_the_cap_and_rounded() {
    for (value, cap, applied) in [
        (5.0, 10.0, 5),
        (5.4, 10.0, 5),
        (5.6, 10.0, 6),
        (0.0, 10.0, 1),
        (-3.0, 10.0, 1),
        (9.0, 6.0, 6),
        (40.0, 99.0, MAX_RADIUS as u32),
    ] {
        assert_eq!(applied_radius(value, cap), applied, "{value} under {cap}");
    }
    assert_eq!(applied_radius(f32::NAN, 10.0), DEFAULT_RADIUS as u32);
    assert_eq!(
        applied_radius(f32::NAN, 3.0),
        3,
        "the fallback is capped too"
    );
    for i in -20..200 {
        let v = i as f32 * 0.11;
        assert_eq!(
            applied_radius(ParamKind::Structural.quantize(v), 10.0),
            applied_radius(v, 10.0)
        );
    }
}

/// **The tier's radius cap reaches the shader**: a scene built at `Floor`
/// hands a bound radius of 10 to the step pass as `Floor`'s cap, and one built
/// at `Rich` hands it through — read off the uniform the pass is given, not
/// recomputed from the law. No frame is rendered.
#[test]
fn the_tier_caps_the_radius_the_shader_is_given() {
    use crate::render::TierConfig;
    let Some(ctx) = context() else {
        return;
    };
    for tier in [TierConfig::FLOOR, TierConfig::RICH] {
        let mut scene = CellularScene::new(
            &ctx.device,
            crate::render::COMPOSITE_FORMAT,
            tier.cellular_radius,
        );
        scene.configure(&GeneratorConfig::Cellular(ltl(64, true, 1)));
        scene.reset_params();
        scene.set_param("radius", 10.0);
        assert_eq!(
            scene.step_params(None).b[2],
            tier.cellular_radius.min(10),
            "{:?}",
            tier.tier
        );
        scene.set_param("radius", 3.0);
        assert_eq!(scene.step_params(None).b[2], 3, "a radius inside the cap");
    }
    const {
        assert!(
            TierConfig::FLOOR.cellular_radius < 10,
            "the Floor cap binds at 10"
        );
        assert!(
            DEFAULT_RADIUS as u32 <= TierConfig::FLOOR.cellular_radius,
            "the default rule runs at its own radius on every tier"
        );
    }
}

/// An interval of fractions becomes the whole counts it admits, inclusive at
/// both ends — so an exact fraction keeps its count — and an interval holding no
/// whole count admits nothing. The defaults land on the counts they are
/// declared for at radius 5.
#[test]
fn an_interval_becomes_the_whole_counts_it_admits() {
    assert_eq!(neighbourhood(1), 8);
    assert_eq!(neighbourhood(5), 120);
    let none = [8.0, 8.0];
    // Conway at radius 1: B3, S23.
    assert_eq!(interval_counts(3.0 / 8.0, 3.0 / 8.0, 1, none), [3, 3]);
    assert_eq!(interval_counts(2.0 / 8.0, 3.0 / 8.0, 1, none), [2, 3]);
    // The declared defaults: births at 34..=46 — Bosco's B34..45 widened by one
    // count — and Bosco's S33..57 with the centre excluded, 32..=56.
    let birth = interval_counts(DEFAULT_BIRTH_LO, DEFAULT_BIRTH_HI, 5, none);
    let survive = interval_counts(DEFAULT_SURVIVE_LO, DEFAULT_SURVIVE_HI, 5, none);
    assert_eq!((birth, survive), ([34, 46], [32, 56]));
    // Empty, reversed, and past the ends.
    let [first, last] = interval_counts(0.30, 0.31, 1, none);
    assert!(first > last, "0.30..0.31 of 8 holds no whole count");
    let [first, last] = interval_counts(0.6, 0.4, 5, none);
    assert!(first > last, "a reversed interval admits nothing");
    assert_eq!(interval_counts(-1.0, 2.0, 2, none), [0, 24]);
    assert_eq!(interval_counts(f32::NAN, 0.5, 1, [0.25, 0.9]), [2, 4]);
}

/// **The separated sum is the box**: a `larger_than_life` field matches the
/// rule summed cell by cell on the CPU, generation by generation, at three
/// radii, on a torus and inside a dead border.
#[test]
fn larger_than_life_matches_the_cpu_rule() {
    let Some(ctx) = context() else {
        return;
    };
    const N: u32 = 40;
    let mut driver = Driver::new(&ctx);
    for (radius, wrap) in [(2u32, true), (3, false), (5, true)] {
        let bounds = (0.28_f32, 0.385_f32, 0.26_f32, 0.47_f32);
        let birth = interval_counts(bounds.0, bounds.1, radius, [0.0; 2]);
        let survive = interval_counts(bounds.2, bounds.3, radius, [0.0; 2]);
        let params = [
            ("step_rate", ONE_GEN.1),
            ("radius", radius as f32),
            ("birth_lo", bounds.0),
            ("birth_hi", bounds.1),
            ("survive_lo", bounds.2),
            ("survive_hi", bounds.3),
        ];
        let mut scene = scene_with(&ctx, ltl(N, wrap, 8));
        driver.frame(&mut scene, ONE_GEN.0, &[("step_rate", 0.0)]);
        let mut cpu = mirror::seed_field(N, field_seed(8), live_threshold(LTL_DENSITY));
        assert_same_field(&states(&ctx, &scene), &cpu, "the larger_than_life seed");
        for generation in 1..=20 {
            driver.frame(&mut scene, ONE_GEN.0, &params);
            cpu = mirror::ltl_step(&cpu, N, wrap, radius, birth, survive);
            if generation % 5 == 0 {
                assert_same_field(
                    &states(&ctx, &scene),
                    &cpu,
                    &format!("radius {radius}, wrap {wrap}, generation {generation}"),
                );
            }
        }
        let live = cpu.iter().filter(|c| **c == 1).count();
        assert!(
            live > 20 && live < (N * N) as usize * 3 / 4,
            "radius {radius}: {live} live cells is too degenerate a field to have tested on"
        );
        drop(scene);
    }
}

/// **At radius 1 with whole-count thresholds `larger_than_life` is
/// `life_like`**: Conway written as intervals (`B3/8..3/8`, `S2/8..3/8`) and as
/// masks (`birth = 8`, `survive = 12`) grow the same field from the same
/// planted soup, generation by generation.
#[test]
fn radius_one_with_whole_thresholds_is_life_like() {
    let Some(ctx) = context() else {
        return;
    };
    const N: u32 = 24;
    let soup: Vec<(u32, u32)> = mirror::seed_field(N, 99, live_threshold(0.4))
        .iter()
        .enumerate()
        .filter(|(_, c)| **c == 1)
        .map(|(i, _)| (i as u32 % N, i as u32 / N))
        .collect();
    let mut driver = Driver::new(&ctx);
    let mut run = |config: CellularConfig, rule: &[(&str, f32)]| -> Vec<Vec<u8>> {
        let mut scene = scene_with(&ctx, config);
        driver.frame(&mut scene, ONE_GEN.0, &[("step_rate", 0.0)]);
        plant(&ctx, &mut scene, &soup);
        (0..24)
            .map(|_| {
                driver.frame(&mut scene, ONE_GEN.0, rule);
                states(&ctx, &scene)
            })
            .collect()
    };
    let masks = run(
        life(N, true, 1),
        &[("step_rate", ONE_GEN.1), ("birth", 8.0), ("survive", 12.0)],
    );
    let intervals = run(
        ltl(N, true, 1),
        &[
            ("step_rate", ONE_GEN.1),
            ("radius", 1.0),
            ("birth_lo", 3.0 / 8.0),
            ("birth_hi", 3.0 / 8.0),
            ("survive_lo", 2.0 / 8.0),
            ("survive_hi", 3.0 / 8.0),
        ],
    );
    for (generation, (a, b)) in masks.iter().zip(&intervals).enumerate() {
        assert_same_field(b, a, &format!("generation {}", generation + 1));
    }
    assert!(
        masks[0] != masks[23],
        "the soup froze at once, so the comparison tested nothing"
    );
}

/// **A radius above 1 is still moving after 2,000 generations**, the property
/// the plan asks of the family: the declared default rule (radius 5, births at
/// 34..=46 of 120 neighbours, survival at 32..=56) from two seeded soups,
/// compared at generation 2,000 with itself 60 generations on — a span every
/// oscillator of period 1 to 6, 10, 12, 15, 20 and 30 returns to itself
/// across, so a field that had settled into still lifes and those oscillators
/// would compare equal.
///
/// The control is a radius-4 rule (births at 26..=34 of 80 neighbours,
/// survival at 24..=42) that from the same soup does settle, into a few hundred
/// still cells: without it, "moving" could be a property of the measurement
/// rather than of the rule.
#[test]
fn larger_than_life_is_still_moving_after_two_thousand_generations() {
    let Some(ctx) = context() else {
        return;
    };
    const N: u32 = 128;
    let mut driver = Driver::new(&ctx);
    // Eight generations a frame, the most one frame encodes.
    let mut run = |salt: u32, rule: &[(&str, f32)]| -> (usize, usize) {
        let dt = 1.0 / 30.0;
        let mut scene = scene_with(&ctx, ltl(N, true, salt));
        let mut generations = 0;
        while generations < 2000 {
            generations += driver.frame(&mut scene, dt, rule);
        }
        assert_eq!(generations, 2000);
        let at_2000 = states(&ctx, &scene);
        let mut later = 0;
        while later < 60 {
            later += driver.frame(&mut scene, dt, rule);
        }
        let at_2060 = states(&ctx, &scene);
        let live = at_2000.iter().filter(|c| **c == 1).count();
        let moved = at_2000.iter().zip(&at_2060).filter(|(a, b)| a != b).count();
        (live, moved)
    };
    for salt in [12, 13] {
        let (live, moved) = run(salt, &[("step_rate", 240.0)]);
        println!("default rule, salt {salt}: {live} live at 2000, {moved} differ at 2060");
        assert!(live > 50, "salt {salt}: the soup died out ({live} live)");
        assert!(
            moved > 50,
            "salt {salt}: only {moved} cells differ across 60 generations — it settled"
        );
    }
    let control = [
        ("step_rate", 240.0),
        ("radius", 4.0),
        ("birth_lo", 26.0 / 80.0),
        ("birth_hi", 34.0 / 80.0),
        ("survive_lo", 24.0 / 80.0),
        ("survive_hi", 42.0 / 80.0),
    ];
    let (live, moved) = run(12, &control);
    println!("radius-4 control, salt 12: {live} live at 2000, {moved} differ at 2060");
    assert!(
        live > 50,
        "the control died out rather than settling ({live} live)"
    );
    assert_eq!(
        moved, 0,
        "the control moved {moved} cells, so the measurement cannot tell settled from moving"
    );
}
