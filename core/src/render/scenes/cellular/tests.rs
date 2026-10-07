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
        MAX_GRID,
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
        "{}{}{}",
        gpu::FULLSCREEN_VS_UV_FLIPPED,
        shader::PRESENT_SHADER,
        shader::PRESENT_MAIN
    );
    if let Err(e) = crate::milk::shader::validate_wgsl(&present) {
        panic!("the present shader does not validate:\n{e}");
    }
    for (what, source) in [
        ("route grid", route::grid_source()),
        ("route control", route::control_source()),
        (
            "route present",
            format!(
                "{}{}",
                gpu::FULLSCREEN_VS_UV_FLIPPED,
                route::present_source()
            ),
        ),
    ] {
        if let Err(e) = crate::milk::shader::validate_wgsl(&source) {
            panic!("the {what} shader does not validate:\n{e}");
        }
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
    let route = route::RouteResources::build(
        &ctx.device,
        crate::render::COMPOSITE_FORMAT,
        64,
        &res.field,
        &res.present_layout,
    );
    if let Some(error) = pollster::block_on(scope.pop()) {
        panic!("building the cellular resources raised: {error}");
    }
    drop(route);
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
/// than one of this scene per preset and its layer. (A WARP reading,
/// unverified on lavapipe as of 2026-10-06.)
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
            tier.cellular_grid,
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

/// **The default rule, on a 128-cell torus from salts 12 and 13, is still moving
/// at generation 2,000** (Plan 0164 Phase 3): the declared default (radius 5,
/// births at 34..=46 of 120 neighbours, survival at 32..=56) from those two
/// seeded soups, compared at generation 2,000 with itself 60 generations on — a
/// span every oscillator of period 1 to 6, 10, 12, 15, 20 and 30 returns to
/// itself across, so a field that had settled into still lifes and those
/// oscillators would compare equal.
///
/// **This is a claim about one grid and two seeds, not about the rule or the
/// family.** Whether a `larger_than_life` field keeps moving depends on the grid
/// and the soup: the same default rule on a 256-cell grid from generator seed 23
/// settles into still rings within about 720 generations (design-backlog 0211),
/// which is why an unreseeded world on this family is documented as settling. A
/// green run here does not say a preset on the default rule stays alive.
///
/// The control is a radius-4 rule (births at 26..=34 of 80 neighbours,
/// survival at 24..=42) that from the same soup does settle, into a few hundred
/// still cells: without it, "moving" could be a property of the measurement
/// rather than of the rule.
#[test]
fn the_default_rule_is_still_moving_at_generation_2000_on_a_128_torus_from_salts_12_and_13() {
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

// ---------------------------------------------------------------------------
// cyclic
// ---------------------------------------------------------------------------

fn cyclic(grid: u32, wrap: bool, salt: u32) -> CellularConfig {
    CellularConfig {
        family: CellularFamily::Cyclic,
        grid,
        wrap,
        salt,
    }
}

/// The state channel as colour indices.
fn colours(ctx: &RenderContext, scene: &CellularScene) -> Vec<u8> {
    read_texels(ctx, scene)
        .chunks_exact(4)
        .map(|t| (t[0] + 0.5) as u8)
        .collect()
}

/// The cyclic field matches the rule written out on the CPU, generation by
/// generation, from its seeded noise — on a torus and inside a dead border, at
/// two cycles.
#[test]
fn cyclic_matches_the_cpu_rule() {
    let Some(ctx) = context() else {
        return;
    };
    const N: u32 = 40;
    let mut driver = Driver::new(&ctx);
    for (states, threshold, wrap) in [(3u32, 3u32, true), (5, 2, false), (7, 1, true)] {
        let rule = [
            ("step_rate", ONE_GEN.1),
            ("states", states as f32),
            ("threshold", threshold as f32),
        ];
        let mut scene = scene_with(&ctx, cyclic(N, wrap, 2));
        driver.frame(
            &mut scene,
            ONE_GEN.0,
            &[("step_rate", 0.0), ("states", states as f32)],
        );
        let mut cpu = mirror::seed_colours(N, field_seed(2), states);
        assert_same_field(&colours(&ctx, &scene), &cpu, "the cyclic seed");
        for generation in 1..=24 {
            driver.frame(&mut scene, ONE_GEN.0, &rule);
            cpu = mirror::cyclic_step(&cpu, N, wrap, states, threshold);
            if generation % 6 == 0 {
                assert_same_field(
                    &colours(&ctx, &scene),
                    &cpu,
                    &format!("{states}/{threshold}, wrap {wrap}, generation {generation}"),
                );
            }
        }
        drop(scene);
    }
}

/// **A seeded noise field organizes into rotating spirals within a bounded
/// number of generations**, measured by topology rather than by eye: noise is
/// full of plaquettes whose colours wind a full turn round the cycle, a spiral
/// is exactly one such plaquette at its core, and a field of spirals has few of
/// them, holds them still while its arms turn, and never stops turning.
///
/// At the declared default — 3 colours, threshold 3 — by generation 296: the
/// defects fall to under a twentieth of the noise's, some remain (the cores),
/// most of them at 360 sit within three cells of where one was 64 generations
/// earlier, and a tenth of the field is still changing every generation.
#[test]
fn noise_organizes_into_rotating_spirals() {
    let Some(ctx) = context() else {
        return;
    };
    const N: u32 = 128;
    let states = DEFAULT_STATES as u32;
    let mut driver = Driver::new(&ctx);
    let mut scene = scene_with(&ctx, cyclic(N, true, 3));
    let fast = [("step_rate", 240.0)];
    driver.frame(&mut scene, 0.1, &[("step_rate", 0.0)]);
    let noise = mirror::defects(&colours(&ctx, &scene), N, states).len();
    let mut generation = 0;
    let mut run_to = |scene: &mut CellularScene, driver: &mut Driver<'_>, stop: u32| {
        while generation < stop {
            generation += driver.frame(scene, 1.0 / 30.0, &fast);
        }
        assert_eq!(generation, stop);
    };
    // Checkpoints on multiples of the eight generations one frame runs.
    run_to(&mut scene, &mut driver, 296);
    let at_300 = colours(&ctx, &scene);
    let cores_300 = mirror::defects(&at_300, N, states);
    run_to(&mut scene, &mut driver, 360);
    let at_360 = colours(&ctx, &scene);
    let cores_360 = mirror::defects(&at_360, N, states);
    // One more generation, for how much of the field is turning.
    driver.frame(&mut scene, 1.0 / 240.0, &fast);
    let turning = at_360
        .iter()
        .zip(&colours(&ctx, &scene))
        .filter(|(a, b)| a != b)
        .count();
    let within = |reach: u32| {
        let near = |p: &(u32, u32), q: &(u32, u32)| {
            let d = |a: u32, b: u32| a.abs_diff(b).min(N - a.abs_diff(b));
            d(p.0, q.0) <= reach && d(p.1, q.1) <= reach
        };
        cores_360
            .iter()
            .filter(|p| cores_300.iter().any(|q| near(p, q)))
            .count()
    };
    println!(
        "cores held within 1/2/3/5 cells: {}/{}/{}/{}",
        within(1),
        within(2),
        within(3),
        within(5)
    );
    // A core in this rule drifts a cell or two as its arms turn rather than
    // sitting on one plaquette, so "held still" is within three cells.
    let still = within(3);
    println!(
        "defects: {noise} in the noise, {} at 296, {} at 360 ({still} within three cells of a \
         296 core); {turning} of {} cells turning at 360",
        cores_300.len(),
        cores_360.len(),
        N * N
    );
    assert!(noise > 1000, "the seeded noise has only {noise} defects");
    assert!(
        cores_300.len() * 20 < noise,
        "{} defects at 296 of the noise's {noise}: the field has not organized",
        cores_300.len()
    );
    assert!(!cores_360.is_empty(), "no spiral cores remain");
    assert!(
        still * 2 >= cores_360.len(),
        "only {still} of {} cores held still for 64 generations",
        cores_360.len()
    );
    assert!(
        turning * 10 >= (N * N) as usize,
        "only {turning} cells are turning: the spirals have stopped"
    );
}

/// **The palette coordinate is the state index directly, with no remap**: at
/// `hue = 0` every pixel is the palette at `state / states`, whatever `trail`
/// and `age_tint` ask — they are inert on this family — and every cell is lit.
#[test]
fn a_cyclic_cell_is_the_palette_at_its_state_index() {
    let Some(ctx) = context() else {
        return;
    };
    let states = 5u32;
    let mut driver = Driver::new(&ctx);
    let mut scene = scene_with(&ctx, cyclic(TARGET, true, 4));
    for _ in 0..6 {
        driver.frame(
            &mut scene,
            ONE_GEN.0,
            &[
                ("step_rate", ONE_GEN.1),
                ("states", states as f32),
                ("threshold", 2.0),
            ],
        );
    }
    driver.frame(
        &mut scene,
        ONE_GEN.0,
        &[
            ("step_rate", 0.0),
            ("states", states as f32),
            ("trail", 3.0),
            ("age_tint", 0.9),
        ],
    );
    let field = colours(&ctx, &scene);
    let image = driver.read_target();
    let spectrum = Palette::default_spectrum();
    let mut worst = 0.0_f32;
    for (pixel, state) in image.iter().zip(&field) {
        let rgb = spectrum.sample(f32::from(*state) / states as f32, 0.0);
        for c in 0..3 {
            worst = worst.max((pixel[c] - rgb[c]).abs());
        }
    }
    let used: std::collections::BTreeSet<u8> = field.iter().copied().collect();
    println!("worst channel error {worst:.4}; states in use {used:?}");
    assert_eq!(
        used.len(),
        states as usize,
        "not every colour is on the field"
    );
    assert!(
        worst < 0.01,
        "a pixel is {worst} off the palette at its state index"
    );
}

/// **`states` and `threshold` are Structural and hold cleanly under
/// `[hold]`**: both round before the scene sees them, a preset may hold both on
/// a musical edge, and a held step of `states` downward leaves no cell outside
/// the new cycle — the field is read modulo the new count and written back
/// inside it on the very next generation, which the CPU statement predicts cell
/// for cell.
#[test]
fn states_and_threshold_are_structural_and_hold_cleanly() {
    use crate::preset::{HoldEdge, Preset};
    for name in ["states", "threshold"] {
        let spec = PARAMS.iter().find(|s| s.name == name).expect("declared");
        assert_eq!(spec.kind, ParamKind::Structural, "{name}");
    }
    for i in -10..300 {
        let v = i as f32 * 0.13;
        assert_eq!(
            applied_states(ParamKind::Structural.quantize(v)),
            applied_states(v)
        );
        assert_eq!(
            applied_threshold(ParamKind::Structural.quantize(v)),
            applied_threshold(v)
        );
    }
    assert_eq!(applied_states(1.0), 2, "a cycle of one never turns");
    assert_eq!(applied_states(99.0), MAX_STATES as u32);
    assert_eq!(applied_threshold(0.0), 1);
    assert_eq!(applied_threshold(12.0), 8);

    let preset = Preset::from_toml_str(
        "system = \"cellular\"\n[cellular]\nfamily = \"cyclic\"\n\
         [params]\nstates = \"3 + floor(bass * 5)\"\nthreshold = \"1 + floor(mid * 3)\"\n\
         [hold]\nstates = \"bar\"\nthreshold = \"beat\"\n",
    )
    .expect("a held cyclic preset loads");
    assert!(preset.warnings.is_empty(), "{:?}", preset.warnings);
    for (name, edge) in [("states", HoldEdge::Bar), ("threshold", HoldEdge::Beat)] {
        let binding = preset
            .params
            .iter()
            .find(|b| b.name == name)
            .expect("bound");
        assert_eq!(binding.kind, ParamKind::Structural, "{name}");
        assert_eq!(binding.hold, Some(edge), "{name}");
    }

    let Some(ctx) = context() else {
        return;
    };
    const N: u32 = 32;
    let mut driver = Driver::new(&ctx);
    let mut scene = scene_with(&ctx, cyclic(N, true, 5));
    let seven = [
        ("step_rate", ONE_GEN.1),
        ("states", 7.0),
        ("threshold", 1.0),
    ];
    driver.frame(
        &mut scene,
        ONE_GEN.0,
        &[("step_rate", 0.0), ("states", 7.0)],
    );
    let mut cpu = mirror::seed_colours(N, field_seed(5), 7);
    for _ in 0..5 {
        driver.frame(&mut scene, ONE_GEN.0, &seven);
        cpu = mirror::cyclic_step(&cpu, N, true, 7, 1);
    }
    let before = colours(&ctx, &scene);
    assert!(
        before.iter().any(|s| *s >= 3),
        "no cell sits past the smaller cycle, so the step down tests nothing"
    );
    // The held step: 7 colours to 3.
    driver.frame(
        &mut scene,
        ONE_GEN.0,
        &[
            ("step_rate", ONE_GEN.1),
            ("states", 3.0),
            ("threshold", 1.0),
        ],
    );
    cpu = mirror::cyclic_step(&cpu, N, true, 3, 1);
    let after = colours(&ctx, &scene);
    assert!(
        after.iter().all(|s| *s < 3),
        "a cell was left outside the 3-colour cycle"
    );
    assert_same_field(&after, &cpu, "the generation after states stepped 7 -> 3");
}

// ---------------------------------------------------------------------------
// The determinism proof, in the field's own units
// ---------------------------------------------------------------------------

/// **Identical seed and identical frames yield an identical field after 1,000
/// generations**, for every family — state and age, every texel, read off the
/// field rather than off a picture of it, with reseeds on a beat pattern and a
/// generation rate that moves. The integration twin in `core/tests/suite/cellular.rs`
/// makes the same claim through the whole renderer.
#[test]
fn the_field_is_identical_after_a_thousand_generations_for_every_family() {
    let Some(ctx) = context() else {
        return;
    };
    let mut driver = Driver::new(&ctx);
    for family in CellularFamily::ALL {
        let mut run = |salt: u32| -> (u32, Vec<f32>) {
            let config = CellularConfig {
                family,
                grid: 64,
                wrap: true,
                salt,
            };
            let mut scene = scene_with(&ctx, config);
            let mut generations = 0;
            for i in 0..300u32 {
                let rate = 200.0 + 40.0 * ((i * 37) % 100) as f32 / 100.0;
                let reseed = if i % 23 == 0 { 1.0 } else { 0.0 };
                generations += driver.frame(
                    &mut scene,
                    1.0 / 60.0,
                    &[("step_rate", rate), ("reseed", reseed), ("radius", 3.0)],
                );
            }
            (generations, read_texels(&ctx, &scene))
        };
        let (g1, one) = run(3);
        let (g2, two) = run(3);
        assert_eq!(g1, g2);
        assert!(g1 >= 1000, "{family:?} ran only {g1} generations");
        assert!(one == two, "{family:?}: two runs from one seed diverged");
        let (_, other) = run(4);
        assert!(
            one != other,
            "{family:?}: a different seed reached the same field"
        );
    }
}

// ---------------------------------------------------------------------------
// The route (ADR-0266)
// ---------------------------------------------------------------------------

/// A rule under which no cell changes: no count gives birth, every count
/// survives. The maze it holds is exactly the one it started with.
const FROZEN: [(&str, f32); 3] = [("step_rate", ONE_GEN.1), ("birth", 0.0), ("survive", 511.0)];

/// `params` with `route` switched on.
fn with_route(params: &[(&'static str, f32)]) -> Vec<(&'static str, f32)> {
    let mut out = params.to_vec();
    out.push(("route", 1.0));
    out
}

/// The route's buffers, built by a frame with `route > 0`.
fn route_of(scene: &CellularScene) -> &route::RouteResources {
    scene
        .res
        .as_ref()
        .and_then(|res| res.route.as_ref())
        .expect("a frame with route on builds the route")
}

/// Every word of `buffer`, read back.
fn read_words(ctx: &RenderContext, buffer: &wgpu::Buffer) -> Vec<u32> {
    let bytes = buffer.size();
    let read = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("cellular-route-readback"),
        size: bytes,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("cellular-route-readback"),
        });
    encoder.copy_buffer_to_buffer(buffer, 0, &read, 0, bytes);
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
        .expect("poll the route readback");
    rx.recv()
        .expect("map callback ran")
        .expect("map the route readback");
    let words: Vec<u32> = bytemuck::cast_slice(&slice.get_mapped_range().expect("mapped")).to_vec();
    read.unmap();
    words
}

/// The control words.
fn control(ctx: &RenderContext, scene: &CellularScene) -> Vec<u32> {
    read_words(ctx, &route_of(scene).control)
}

/// Word `at` of `words`.
fn word(words: &[u32], at: u32) -> u32 {
    words[at as usize]
}

/// One region of the route's per-cell buffer.
fn region(ctx: &RenderContext, scene: &CellularScene, region: u32) -> Vec<u32> {
    let cells = read_words(ctx, &route_of(scene).cells);
    let n = (scene.config.grid * scene.config.grid) as usize;
    let at = region as usize * n;
    cells[at..at + n].to_vec()
}

/// The distance field the last relax pass wrote: the half of the pair the
/// control words name.
fn distance_field(ctx: &RenderContext, scene: &CellularScene) -> Vec<u32> {
    let parity = word(&control(ctx, scene), route::C_PARITY);
    region(ctx, scene, route::R_DIST + parity)
}

/// The committed route: each cell's distance from B on it, the sentinel off it.
fn committed(ctx: &RenderContext, scene: &CellularScene) -> Vec<u32> {
    region(ctx, scene, route::R_ROUTE)
}

/// Drive `params` until `done` holds of the control words, at most `frames`
/// frames of `dt`; returns the words it held on.
#[track_caller]
fn drive_until(
    ctx: &RenderContext,
    driver: &mut Driver<'_>,
    scene: &mut CellularScene,
    dt: f32,
    params: &[(&str, f32)],
    frames: u32,
    done: impl Fn(&[u32]) -> bool,
) -> Vec<u32> {
    for _ in 0..frames {
        driver.frame(scene, dt, params);
        let words = control(ctx, scene);
        if done(&words) {
            return words;
        }
    }
    panic!("the route did not get there in {frames} frames");
}

/// A tree maze on an `n` grid as a `0`/`1` field, walls live: rooms on the odd
/// coordinates below `n - 1` carved together by a seeded depth-first walk, so
/// the open cells are one tree, and every cell on the border is a wall.
fn tree_maze(n: u32, seed: u64) -> Vec<u8> {
    let rooms = (n - 1) / 2;
    let mut cells = vec![1u8; (n * n) as usize];
    let at = |x: u32, y: u32| (y * n + x) as usize;
    let mut rng = SeededRng::new(seed);
    let mut visited = vec![false; (rooms * rooms) as usize];
    let mut stack = vec![(0u32, 0u32)];
    visited[0] = true;
    cells[at(1, 1)] = 0;
    while let Some(&(rx, ry)) = stack.last() {
        let options: Vec<(u32, u32)> = [(1i64, 0i64), (-1, 0), (0, 1), (0, -1)]
            .iter()
            .map(|(dx, dy)| (i64::from(rx) + dx, i64::from(ry) + dy))
            .filter(|(x, y)| (0..i64::from(rooms)).contains(x) && (0..i64::from(rooms)).contains(y))
            .map(|(x, y)| (x as u32, y as u32))
            .filter(|(x, y)| !visited[(y * rooms + x) as usize])
            .collect();
        if options.is_empty() {
            stack.pop();
            continue;
        }
        let (nx, ny) = options[(rng.next_f32() * options.len() as f32) as usize % options.len()];
        visited[(ny * rooms + nx) as usize] = true;
        cells[at(2 * nx + 1, 2 * ny + 1)] = 0;
        cells[at(rx + nx + 1, ry + ny + 1)] = 0;
        stack.push((nx, ny));
    }
    cells
}

/// A maze whose longest route runs through a loop of two equal branches, on a
/// 32-cell grid: a corridor along row 16 from column 1 to 30 that splits at
/// column 12 into an upper branch on row 14 and a lower one on row 18, joining
/// again at column 20, plus a three-cell dead-end spur up column 5. Walls
/// live.
///
/// From the centre source (15, 14) the double sweep reaches B = (30, 16) at 17
/// and then C = (1, 16) at 33, the spur's tip lying at 32: the corridor is the
/// route, and both branches, each 12 long, lie on it.
fn loop_maze() -> Vec<u8> {
    const N: u32 = 32;
    let mut cells = vec![1u8; (N * N) as usize];
    let mut open = |x: u32, y: u32| cells[(y * N + x) as usize] = 0;
    for x in (1..=12).chain(20..=30) {
        open(x, 16);
    }
    for y in 14..=18 {
        open(12, y);
        open(20, y);
    }
    for x in 12..=20 {
        open(x, 14);
        open(x, 18);
    }
    for y in 13..=15 {
        open(5, y);
    }
    cells
}

/// A scene on `config` with `walls` planted after a first frame of `first`,
/// which runs with the route off so nothing is found on the seeded field the
/// maze replaces.
fn planted(
    ctx: &RenderContext,
    driver: &mut Driver<'_>,
    config: CellularConfig,
    walls: &[u8],
    first: &[(&str, f32)],
) -> CellularScene {
    let first: Vec<(&str, f32)> = first
        .iter()
        .filter(|(name, _)| *name != "route")
        .copied()
        .collect();
    let mut scene = scene_with(ctx, config);
    driver.frame(&mut scene, ONE_GEN.0, &first);
    plant(ctx, &mut scene, &live_cells(walls, config.grid));
    scene
}

/// Whether the control words hold at least one committed route.
fn has_committed(words: &[u32]) -> bool {
    word(words, route::C_COMMITS) >= 1
}

/// A seeded `life_like` field of `n` cells held frozen with the route on,
/// driven until its first route commits. Returns the scene and the seeded
/// states.
fn committed_on_seed(
    ctx: &RenderContext,
    driver: &mut Driver<'_>,
    n: u32,
    wrap: bool,
    salt: u32,
) -> (CellularScene, Vec<u8>) {
    let mut scene = scene_with(ctx, life(n, wrap, salt));
    driver.frame(&mut scene, ONE_GEN.0, &[("step_rate", 0.0), ("route", 1.0)]);
    drive_until(
        ctx,
        driver,
        &mut scene,
        ONE_GEN.0,
        &with_route(&FROZEN),
        40,
        has_committed,
    );
    let seeded = mirror::seed_field(n, field_seed(salt), live_threshold(LIFE_DENSITY));
    assert_same_field(&states(ctx, &scene), &seeded, "the frozen maze");
    (scene, seeded)
}

/// The frame rate of the timeline: generations and route passes come out in
/// time order, a generation first on a tie, each stamped in milliseconds, and
/// nothing is dropped or invented.
#[test]
fn the_timeline_merges_both_clocks_in_time_order() {
    let generations = route::Ticks {
        owed_before: 0.0,
        rate: 4.0,
        count: 1,
    };
    let passes = route::Ticks {
        owed_before: 0.0,
        rate: 16.0,
        count: 4,
    };
    let events: Vec<(route::RouteEvent, u32)> =
        route::Timeline::new(generations, passes, 2.0, 0.25).collect();
    use route::RouteEvent::{Generation, Pass};
    assert_eq!(
        events,
        vec![
            (Pass, 2062),
            (Pass, 2125),
            (Pass, 2187),
            (Generation, 2250),
            (Pass, 2250),
        ]
    );
    // A clock with nothing owed adds nothing; a stall's capped backlog stays
    // inside the frame.
    let mut clock = route::RouteClock::default();
    assert_eq!(clock.advance(0.0, 1.0).count, 0);
    assert_eq!(clock.advance(f32::NAN, 1.0).count, 0);
    let stalled = clock.advance(240.0, 2.0);
    assert_eq!(stalled.count, route::MAX_ROUTE_PASSES_PER_FRAME);
    assert!(stalled.offset(stalled.count, 2.0) <= 2.0);
    assert_eq!(
        clock.advance(240.0, 0.1).count,
        24,
        "the backlog is dropped"
    );
}

/// **Converged sweeps are the breadth-first searches, cell for cell**: on a
/// seeded maze held still, the epoch's kept field is the CPU's BFS from B and
/// its last field the BFS from C, where B and C are the double sweep's ends
/// from the centre source — on a torus and inside a dead border — and every
/// wall and every open cell the search cannot reach holds the sentinel.
#[test]
fn converged_sweeps_are_the_cpu_breadth_first_searches() {
    let Some(ctx) = context() else {
        return;
    };
    const N: u32 = 48;
    let mut driver = Driver::new(&ctx);
    for wrap in [true, false] {
        let (scene, seeded) = committed_on_seed(&ctx, &mut driver, N, wrap, 7);
        let words = control(&ctx, &scene);
        let open = mirror::open_mask(&seeded);
        let source = mirror::centre_source(&open, N).expect("the maze has open cells");
        let cpu = mirror::double_sweep(&open, N, wrap, source);
        assert_eq!(word(&words, route::C_END), cpu.c + 1, "wrap {wrap}: C");
        assert_eq!(word(&words, route::C_LENGTH), cpu.length, "wrap {wrap}");
        for (what, gpu, cpu) in [
            ("d_B", region(&ctx, &scene, route::R_KEPT), &cpu.d_b),
            ("d_C", distance_field(&ctx, &scene), &cpu.d_c),
        ] {
            let differing = gpu.iter().zip(cpu).filter(|(a, b)| a != b).count();
            assert_eq!(differing, 0, "wrap {wrap}: {differing} of {what} differ");
            assert!(
                gpu.iter()
                    .zip(&open)
                    .all(|(d, o)| *o == 1 || *d == route::INF),
                "wrap {wrap}: a wall holds a distance in {what}"
            );
        }
        let unreachable = open
            .iter()
            .zip(&cpu.d_c)
            .filter(|(o, d)| **o == 1 && **d == route::INF)
            .count();
        let reached = cpu.d_c.iter().filter(|d| **d != route::INF).count();
        println!(
            "wrap {wrap}: length {}, epoch {} passes; {reached} reached, {unreachable} open \
             cells cut off",
            cpu.length,
            word(&words, route::C_LAST_EPOCH)
        );
        assert!(unreachable > 0, "no cut-off pocket to hold the sentinel");
        assert!(
            reached > (N * N / 3) as usize,
            "only {reached} cells reached"
        );
        assert!(cpu.length > N, "the route is only {} long", cpu.length);
        drop(scene);
    }
}

/// **A standing maze relaxes nothing after its commit**: the GPU's count of
/// relax passes run stands still while frames go on owing route passes, the
/// route stays as committed, and no second epoch starts.
#[test]
fn a_standing_maze_relaxes_nothing_after_its_commit() {
    let Some(ctx) = context() else {
        return;
    };
    let mut driver = Driver::new(&ctx);
    let (mut scene, _) = committed_on_seed(&ctx, &mut driver, 48, true, 7);
    let route = committed(&ctx, &scene);
    let before = control(&ctx, &scene);
    for _ in 0..6 {
        driver.frame(&mut scene, ONE_GEN.0, &with_route(&FROZEN));
    }
    let after = control(&ctx, &scene);
    assert!(word(&before, route::C_RELAX_PASSES) > 0);
    assert_eq!(
        word(&after, route::C_RELAX_PASSES),
        word(&before, route::C_RELAX_PASSES),
        "relax passes ran after the commit"
    );
    assert_eq!(
        word(&after, route::C_COMMITS),
        1,
        "a second epoch committed"
    );
    assert_eq!(
        word(&after, route::C_ARGS),
        0,
        "the work pass still dispatches"
    );
    assert!(
        committed(&ctx, &scene) == route,
        "the route moved after it committed"
    );
}

/// **On a tree maze the route is the diameter**: a planted tree held still
/// commits exactly the CPU's double-sweep path, one cell wide, and its length
/// is the maze's diameter found by brute force over every pair of open cells.
#[test]
fn on_a_tree_maze_the_route_is_the_diameter() {
    let Some(ctx) = context() else {
        return;
    };
    const N: u32 = 32;
    let maze = tree_maze(N, 5);
    let open = mirror::open_mask(&maze);
    let mut driver = Driver::new(&ctx);
    let mut scene = planted(
        &ctx,
        &mut driver,
        life(N, false, 1),
        &maze,
        &[("step_rate", 0.0), ("route", 1.0)],
    );
    let words = drive_until(
        &ctx,
        &mut driver,
        &mut scene,
        ONE_GEN.0,
        &with_route(&FROZEN),
        60,
        has_committed,
    );
    assert_same_field(&states(&ctx, &scene), &maze, "the planted tree");
    let source = mirror::centre_source(&open, N).expect("open cells");
    let sweep = mirror::double_sweep(&open, N, false, source);
    let expected = mirror::commit(&open, &sweep);
    let gpu = committed(&ctx, &scene);
    let differing = gpu.iter().zip(&expected).filter(|(a, b)| a != b).count();
    assert_eq!(
        differing, 0,
        "{differing} route cells differ from the mirror"
    );

    let cells: Vec<u32> = (0..N * N).filter(|i| open[*i as usize] == 1).collect();
    let diameter = cells
        .iter()
        .flat_map(|s| mirror::bfs(&open, N, false, *s))
        .filter(|d| *d != route::INF)
        .max()
        .expect("a distance");
    let on_route = gpu.iter().filter(|d| **d != route::INF).count() as u32;
    println!(
        "{} open cells; diameter {diameter}; route {} long over {on_route} cells; epoch {} passes",
        cells.len(),
        word(&words, route::C_LENGTH),
        word(&words, route::C_LAST_EPOCH)
    );
    assert_eq!(word(&words, route::C_LENGTH), diameter);
    assert_eq!(on_route, diameter + 1, "a tree's route is one path");
    assert!(diameter > 100, "a diameter of {diameter} tests little");
}

/// **On a loop of two equal branches the route takes both**: the committed
/// route equals the CPU's commit cell for cell, holding every cell of both
/// branches and none of the spur.
#[test]
fn on_a_loop_the_route_takes_both_equal_branches() {
    let Some(ctx) = context() else {
        return;
    };
    const N: u32 = 32;
    let maze = loop_maze();
    let open = mirror::open_mask(&maze);
    let mut driver = Driver::new(&ctx);
    let mut scene = planted(
        &ctx,
        &mut driver,
        life(N, true, 1),
        &maze,
        &[("step_rate", 0.0), ("route", 1.0)],
    );
    drive_until(
        &ctx,
        &mut driver,
        &mut scene,
        ONE_GEN.0,
        &with_route(&FROZEN),
        60,
        has_committed,
    );
    let source = mirror::centre_source(&open, N).expect("open cells");
    let sweep = mirror::double_sweep(&open, N, true, source);
    let expected = mirror::commit(&open, &sweep);
    let gpu = committed(&ctx, &scene);
    assert!(gpu == expected, "the route differs from the mirror's");
    let on = |x: u32, y: u32| gpu[(y * N + x) as usize] != route::INF;
    assert_eq!(
        sweep.length,
        33,
        "B {:?}, C {:?}, source {:?}",
        (sweep.b % N, sweep.b / N),
        (sweep.c % N, sweep.c / N),
        (source % N, source / N)
    );
    assert!(
        (12..=20).all(|x| on(x, 14) && on(x, 18)),
        "a branch is missing"
    );
    assert!((13..=15).all(|y| !on(5, y)), "the spur is on the route");
    assert!(on(1, 16) && on(30, 16), "an end is missing");
}

/// **A maze that never stands still never commits a route**: a seeded Life
/// field, every generation of which changes more than the tolerance, runs
/// sixty generations with the route on and commits nothing.
#[test]
fn a_maze_that_never_stands_still_never_commits() {
    let Some(ctx) = context() else {
        return;
    };
    let mut driver = Driver::new(&ctx);
    let mut scene = scene_with(&ctx, life(64, true, 3));
    for generation in 0..60 {
        driver.frame(
            &mut scene,
            ONE_GEN.0,
            &[("step_rate", ONE_GEN.1), ("route", 1.0)],
        );
        let words = control(&ctx, &scene);
        assert!(
            !mirror::is_still(
                word(&words, route::C_LAST_MOVED),
                word(&words, route::C_LAST_OPEN)
            ),
            "generation {generation} was still, so this field cannot test the claim"
        );
        assert_eq!(word(&words, route::C_COMMITS), 0, "a route committed");
        assert!(word(&words, route::C_STILL_RUN) < route::QUIET_HOLD);
    }
}

/// **A stamp landing mid-epoch abandons the epoch**: nothing commits from the
/// snapshot it interrupted, and the next quiet stretch commits the route of
/// the stamped maze, exactly as the CPU finds it.
#[test]
fn a_stamp_mid_epoch_abandons_it_and_the_next_quiet_commits_the_new_maze() {
    let Some(ctx) = context() else {
        return;
    };
    const N: u32 = 32;
    let maze = tree_maze(N, 9);
    let mut driver = Driver::new(&ctx);
    let mut scene = planted(
        &ctx,
        &mut driver,
        life(N, true, 4),
        &maze,
        &[("step_rate", 0.0), ("route", 1.0)],
    );
    // Slow passes, so the epoch is still in flight a few frames in.
    scene.route_rate = 8.0;
    let frozen = with_route(&FROZEN);
    let words = drive_until(&ctx, &mut driver, &mut scene, ONE_GEN.0, &frozen, 40, |w| {
        word(w, route::C_SEARCH) >= 2
    });
    assert_eq!(
        word(&words, route::C_COMMITS),
        0,
        "it committed before the stamp"
    );

    let mut stamped = frozen.clone();
    stamped.push(("reseed", 1.0));
    driver.frame(&mut scene, ONE_GEN.0, &stamped);
    let words = control(&ctx, &scene);
    assert_eq!(
        word(&words, route::C_ABANDONED),
        1,
        "the epoch was not abandoned"
    );
    assert_eq!(word(&words, route::C_SEARCH), 0);
    let new_maze = states(&ctx, &scene);
    assert!(new_maze != maze, "the stamp changed nothing");

    scene.route_rate = route::ROUTE_RATE;
    let mut frames = 0;
    let words = loop {
        driver.frame(&mut scene, ONE_GEN.0, &frozen);
        let words = control(&ctx, &scene);
        frames += 1;
        if has_committed(&words) || frames > 60 {
            break words;
        }
        assert!(word(&words, route::C_SHOWN) == route::SHOWN_NONE);
    };
    assert_eq!(word(&words, route::C_COMMITS), 1);
    assert_eq!(word(&words, route::C_ABANDONED), 1);
    let open = mirror::open_mask(&new_maze);
    let source = mirror::centre_source(&open, N).expect("open cells");
    let expected = mirror::commit(&open, &mirror::double_sweep(&open, N, true, source));
    assert!(
        committed(&ctx, &scene) == expected,
        "the committed route is not the stamped maze's"
    );
}

/// The colour a live cell presents at `trail = 0` and `hue = 0`: the palette's
/// origin at full light.
fn live_colour() -> [f32; 3] {
    let rgb = Palette::default_spectrum().sample(0.0, 0.0);
    let b = common::DEFAULT_BRIGHTNESS;
    [rgb[0] * b, rgb[1] * b, rgb[2] * b]
}

/// Whether `pixel` is the driver's untouched clear: no light, no coverage.
fn dark(pixel: &[f32; 4]) -> bool {
    *pixel == [0.0, 0.0, 0.0, 1.0]
}

/// A tree maze of `TARGET` cells drawn one to a pixel, committed with the route
/// on at `trail = 0`, one generation a frame of 1/60 s. Returns the scene, the
/// maze and the parameters it was driven with.
fn committed_tree(
    ctx: &RenderContext,
    driver: &mut Driver<'_>,
    reveal: f32,
    extra: &[(&'static str, f32)],
) -> (CellularScene, Vec<u8>, Vec<(&'static str, f32)>) {
    let maze = tree_maze(TARGET, 11);
    let mut params = vec![
        ("step_rate", 60.0),
        ("birth", 0.0),
        ("survive", 511.0),
        ("route", 1.0),
        ("trail", 0.0),
        ("age_tint", 0.0),
        ("route_reveal", reveal),
    ];
    params.extend_from_slice(extra);
    let mut scene = planted(ctx, driver, life(TARGET, true, 2), &maze, &params);
    drive_until(
        ctx,
        driver,
        &mut scene,
        1.0 / 60.0,
        &params,
        400,
        has_committed,
    );
    (scene, maze, params)
}

/// **A broken route fades, and never paints a wall**: after a stamp breaks the
/// quiet over a drawn route, every live cell in every frame is the live colour
/// and nothing else, the route still draws on the first frame after the break,
/// and once `route_reveal / 4` seconds of scene time have passed no frame
/// paints route colour at all.
#[test]
fn a_broken_route_fades_and_never_paints_a_wall() {
    let Some(ctx) = context() else {
        return;
    };
    let mut driver = Driver::new(&ctx);
    let reveal = 0.4;
    let (mut scene, _, params) = committed_tree(&ctx, &mut driver, reveal, &[]);
    // Past the reveal, with no further epoch to start a new route.
    for _ in 0..30 {
        driver.frame(&mut scene, 1.0 / 60.0, &params);
    }
    scene.route_rate = 0.0;
    let lit = driver.read_target().iter().filter(|p| !dark(p)).count();
    let field = states(&ctx, &scene);
    let walls = field.iter().filter(|s| **s == 1).count();
    assert!(lit > walls, "the route is not drawn before the break");

    let mut stamped = params.clone();
    stamped.push(("reseed", 1.0));
    let live = live_colour();
    let mut drew_after_break = false;
    let mut faded_frames = 0;
    for frame in 0..30 {
        let p = if frame == 0 { &stamped } else { &params };
        driver.frame(&mut scene, 1.0 / 60.0, p);
        let words = control(&ctx, &scene);
        assert_ne!(
            word(&words, route::C_SHOWN),
            route::SHOWN_ON,
            "frame {frame}"
        );
        let image = driver.read_target();
        let field = states(&ctx, &scene);
        let mut route_lit = 0;
        for (pixel, state) in image.iter().zip(&field) {
            if *state == 1 {
                for c in 0..3 {
                    assert!(
                        (pixel[c] - live[c]).abs() < 0.01,
                        "frame {frame}: a live cell is {pixel:?}, not the live colour"
                    );
                }
            } else if !dark(pixel) {
                route_lit += 1;
            }
        }
        let gone = route::scene_ms(scene.scene_time) - word(&words, route::C_BREAK_MS);
        if gone >= route::fade_ms(reveal) {
            assert_eq!(
                route_lit, 0,
                "frame {frame}: route colour {gone} ms after the break"
            );
            faded_frames += 1;
        } else if route_lit > 0 {
            drew_after_break = true;
        }
    }
    assert!(
        drew_after_break,
        "the route vanished at the break instead of fading"
    );
    assert!(
        faded_frames > 10,
        "only {faded_frames} frames came after the fade"
    );
}

/// **The route reveals along its length**: with `route_reveal = 2`, a frame
/// one second after the commit paints the cells with `t` up to the leading
/// edge and none past it, and a frame two seconds after paints all of them.
#[test]
fn the_route_reveals_along_its_length() {
    let Some(ctx) = context() else {
        return;
    };
    let mut driver = Driver::new(&ctx);
    let (mut scene, _, params) = committed_tree(&ctx, &mut driver, 2.0, &[]);
    let words = control(&ctx, &scene);
    let commit = f64::from(word(&words, route::C_COMMIT_MS)) / 1000.0;
    let length = word(&words, route::C_LENGTH) as f32;
    let route = committed(&ctx, &scene);
    let to = |driver: &mut Driver<'_>, scene: &mut CellularScene, at: f64| {
        let dt = (at - scene.scene_time) as f32;
        assert!(dt > 0.0);
        driver.frame(scene, dt, &params);
        driver.read_target()
    };
    let half = to(&mut driver, &mut scene, commit + 1.0005);
    let front = 0.5 * (1.0 + route::REVEAL_EDGE);
    let (mut inside, mut beyond) = (0, 0);
    for (pixel, d) in half.iter().zip(&route) {
        if *d == route::INF {
            continue;
        }
        let t = *d as f32 / length;
        if t <= front - route::REVEAL_EDGE - 0.01 {
            assert!(!dark(pixel), "t {t} is behind the front and not drawn");
            inside += 1;
        } else if t >= front + 0.01 {
            assert!(dark(pixel), "t {t} is past the front and drawn");
            beyond += 1;
        }
    }
    assert!(
        inside > 10 && beyond > 10,
        "{inside} behind, {beyond} beyond"
    );
    let whole = to(&mut driver, &mut scene, commit + 2.0005);
    let field = states(&ctx, &scene);
    for (i, pixel) in whole.iter().enumerate() {
        if field[i] == 0 {
            assert_eq!(
                !dark(pixel),
                route[i] != route::INF,
                "cell {i}: drawn and on-route disagree two seconds in"
            );
        }
    }
}

/// **`route_grade` grades the route along `t`**: at 0 every route cell is one
/// colour, the palette at `route_coord`; at 0.6 the end at B is the palette at
/// `route_coord` and the end at C at `route_coord + 0.6`.
#[test]
fn route_grade_paints_the_ends_at_their_coordinates() {
    let Some(ctx) = context() else {
        return;
    };
    let mut driver = Driver::new(&ctx);
    let (mut scene, _, params) = committed_tree(&ctx, &mut driver, 0.0, &[]);
    let route = committed(&ctx, &scene);
    let length = word(&control(&ctx, &scene), route::C_LENGTH);
    let b = route.iter().position(|d| *d == 0).expect("B");
    let c = route.iter().position(|d| *d == length).expect("C");
    let spectrum = Palette::default_spectrum();
    let expect = |coord: f32| {
        let rgb = spectrum.sample(coord, 0.0);
        rgb.map(|v| v * common::DEFAULT_BRIGHTNESS)
    };
    let close = |pixel: &[f32; 4], rgb: [f32; 3]| (0..3).all(|k| (pixel[k] - rgb[k]).abs() < 0.02);

    let mut solid = params.clone();
    solid.extend([("route_coord", 0.2), ("route_grade", 0.0)]);
    driver.frame(&mut scene, 1.0 / 60.0, &solid);
    let image = driver.read_target();
    let colours: std::collections::BTreeSet<[u32; 3]> = route
        .iter()
        .zip(&image)
        .filter(|(d, _)| **d != route::INF)
        .map(|(_, p)| [p[0].to_bits(), p[1].to_bits(), p[2].to_bits()])
        .collect();
    let shown: Vec<(usize, [f32; 4])> = route
        .iter()
        .zip(&image)
        .enumerate()
        .filter(|(_, (d, _))| **d != route::INF)
        .map(|(i, (_, p))| (i, *p))
        .collect();
    assert_eq!(
        colours.len(),
        1,
        "a solid route drew {} colours: {shown:?}",
        colours.len()
    );
    assert!(
        close(&image[b], expect(0.2)),
        "the solid route is not the palette at 0.2"
    );

    let mut graded = params.clone();
    graded.extend([("route_coord", 0.2), ("route_grade", 0.6)]);
    driver.frame(&mut scene, 1.0 / 60.0, &graded);
    let image = driver.read_target();
    assert!(close(&image[b], expect(0.2)), "B is {:?}", image[b]);
    assert!(close(&image[c], expect(0.8)), "C is {:?}", image[c]);
    assert!(!close(&image[b], expect(0.8)), "the ends drew one colour");
}

/// **A `cyclic` preset runs no route at any `route`**: nothing of it is built.
#[test]
fn cyclic_runs_no_route() {
    let Some(ctx) = context() else {
        return;
    };
    let mut driver = Driver::new(&ctx);
    let mut scene = scene_with(&ctx, cyclic(32, true, 1));
    for _ in 0..4 {
        driver.frame(
            &mut scene,
            ONE_GEN.0,
            &[("step_rate", ONE_GEN.1), ("route", 1.0)],
        );
    }
    assert!(
        scene.res.as_ref().is_some_and(|res| res.route.is_none()),
        "cyclic built the route"
    );
}

/// **The compare pass counts exactly the cells whose open bit changed**, against
/// the CPU's count over the same pair of generations read off the field — and
/// the open cells too.
#[test]
fn the_compare_count_is_the_cpu_change_count() {
    let Some(ctx) = context() else {
        return;
    };
    const N: u32 = 48;
    let mut driver = Driver::new(&ctx);
    let mut scene = scene_with(&ctx, life(N, true, 5));
    driver.frame(&mut scene, ONE_GEN.0, &[("step_rate", 0.0), ("route", 1.0)]);
    // The route was built on that frame, with no mask recorded: its first
    // compare is against all walls.
    let mut previous = vec![0u8; (N * N) as usize];
    let mut counts = Vec::new();
    for generation in 1..=20 {
        driver.frame(
            &mut scene,
            ONE_GEN.0,
            &[("step_rate", ONE_GEN.1), ("route", 1.0)],
        );
        let open = mirror::open_mask(&states(&ctx, &scene));
        let words = control(&ctx, &scene);
        let moved = word(&words, route::C_LAST_MOVED);
        assert_eq!(
            moved,
            mirror::changed(&previous, &open),
            "generation {generation}: the compare count"
        );
        assert_eq!(
            word(&words, route::C_LAST_OPEN),
            open.iter().filter(|o| **o == 1).count() as u32,
            "generation {generation}: the open count"
        );
        assert_eq!(word(&words, route::C_GENERATIONS), generation);
        counts.push(moved);
        previous = open;
    }
    println!("changed open bits per generation: {counts:?}");
    let distinct: std::collections::BTreeSet<u32> = counts.iter().copied().collect();
    assert!(
        counts.iter().all(|c| *c > 0) && distinct.len() > 5,
        "a field that does not move tests nothing: {counts:?}"
    );
}

/// **30 and 144 fps reach the identical route at the same wall time**: a tree
/// maze held still commits, reveals, is broken by a stamp landing at 2.25 s and
/// fades, and at every wall time sampled the two rates draw the same bytes, and
/// end on the same route words and control words. The first frame, which plants
/// the maze, is a quarter second at both rates.
#[test]
fn thirty_and_one_hundred_forty_four_fps_reach_the_same_route() {
    let Some(ctx) = context() else {
        return;
    };
    let maze = tree_maze(TARGET, 13);
    let mut driver = Driver::new(&ctx);
    // Wall times past the first quarter second, in sixths of a second, at
    // which both rates end a frame.
    let samples = [3u32, 6, 9, 12, 13, 15, 18];
    let mut run = |fps: u32| -> (Vec<Vec<[f32; 4]>>, Vec<u32>, Vec<u32>) {
        let params = [
            ("step_rate", 12.0),
            ("birth", 0.0),
            ("survive", 511.0),
            ("route", 1.0),
            ("trail", 0.0),
            ("route_reveal", 0.5),
        ];
        let mut stamped = params.to_vec();
        stamped.push(("reseed", 1.0));
        let mut scene = planted(&ctx, &mut driver, life(TARGET, true, 6), &maze, &params);
        let mut images = Vec::new();
        for frame in 1..=fps * 3 {
            // Frame `f` ends at 0.25 + f / fps s; the one starting at 2.25 s
            // carries the stamp.
            let p: &[(&str, f32)] = if frame == fps * 2 + 1 {
                &stamped
            } else {
                &params
            };
            driver.frame(&mut scene, 1.0 / fps as f32, p);
            if samples.iter().any(|s| s * fps / 6 == frame) {
                images.push(driver.read_target());
            }
        }
        let out = (images, control(&ctx, &scene), committed(&ctx, &scene));
        drop(scene);
        out
    };
    let (i30, c30, r30) = run(30);
    let (i144, c144, r144) = run(144);
    println!(
        "commit at {} ms, break at {} ms, {} commits, {} relax passes",
        word(&c30, route::C_COMMIT_MS),
        word(&c30, route::C_BREAK_MS),
        word(&c30, route::C_COMMITS),
        word(&c30, route::C_RELAX_PASSES)
    );
    assert!(word(&c30, route::C_COMMITS) >= 1, "no route committed");
    assert!(
        (2250..2450).contains(&word(&c30, route::C_BREAK_MS)),
        "the stamp did not break the quiet"
    );
    assert_eq!(c30, c144, "the control words differ");
    assert!(r30 == r144, "the committed routes differ");
    assert_eq!(i30.len(), samples.len());
    for (k, (a, b)) in i30.iter().zip(&i144).enumerate() {
        assert!(
            a == b,
            "at {}/6 s the two rates drew different frames",
            samples[k]
        );
    }
    assert!(
        i30.windows(2).filter(|w| w[0] != w[1]).count() >= 3,
        "the sampled frames barely change, so their equality tests little"
    );
}

/// **`route = 0` changes nothing**: a scene that never turns the route on
/// builds none of it, and a scene that turns it on and off again draws the very
/// bytes the first one does once it is off — while drawing something else
/// while it is on, so the equality is the switch's doing.
#[test]
fn route_zero_draws_the_frame_unchanged() {
    let Some(ctx) = context() else {
        return;
    };
    let maze = tree_maze(TARGET, 3);
    let mut driver = Driver::new(&ctx);
    let on = 2..14u32;
    let mut run = |on: &dyn Fn(u32) -> bool| -> (Vec<Vec<[f32; 4]>>, bool) {
        let params = |route: f32| {
            [
                ("step_rate", ONE_GEN.1),
                ("birth", 0.0),
                ("survive", 511.0),
                ("route", route),
                ("route_coord", 0.7),
                ("route_reveal", 0.0),
            ]
        };
        let mut scene = planted(
            &ctx,
            &mut driver,
            life(TARGET, true, 2),
            &maze,
            &params(0.0),
        );
        let frames = (0..20)
            .map(|i| {
                let route = if on(i) { 1.0 } else { 0.0 };
                driver.frame(&mut scene, ONE_GEN.0, &params(route));
                driver.read_target()
            })
            .collect();
        let built = scene.res.as_ref().is_some_and(|res| res.route.is_some());
        (frames, built)
    };
    let (plain, built) = run(&|_| false);
    assert!(!built, "route = 0 built the route's resources");
    let (toggled, _) = run(&|i| on.contains(&i));
    for i in 0..20 {
        if !on.contains(&(i as u32)) {
            assert!(
                plain[i] == toggled[i],
                "frame {i}: route = 0 drew a different frame"
            );
        }
    }
    assert!(
        on.clone().any(|i| plain[i as usize] != toggled[i as usize]),
        "the route never drew, so the equality tests nothing"
    );
}

// ---------------------------------------------------------------------------
// The route's readings (Plan 0253 Phase 1). Measurements, not gates.
// ---------------------------------------------------------------------------

/// `cellular_labyrinth`'s own table and salt.
fn labyrinth_config() -> CellularConfig {
    let preset = crate::preset::Preset::from_toml_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../presets/cellular_labyrinth.toml"
    )))
    .expect("the labyrinth loads");
    match preset.config {
        Some(GeneratorConfig::Cellular(config)) => config,
        other => panic!("the labyrinth is not a cellular preset: {other:?}"),
    }
}

/// `cellular_labyrinth`'s bindings driven by a fixed beat at 120 bpm and every
/// band at zero, on frame `frame` of `fps`: beat `k` lands on the frame holding
/// `k / 2` seconds and `beat_index` reads `k` until the next. So `step_rate` is
/// 14, a disc lands on every fourth beat, and `survive` drops to 30 for beats
/// 0-5 of every 32.
fn labyrinth_params(frame: u32, fps: u32) -> (Vec<(&'static str, f32)>, u32, bool) {
    let per_beat = fps / 2;
    let beat = frame / per_beat;
    let on_beat = frame.is_multiple_of(per_beat);
    let survive = if beat % 32 < 6 { 30.0 } else { 62.0 };
    let reseed = if on_beat && beat.is_multiple_of(4) {
        1.0
    } else {
        0.0
    };
    (
        vec![
            ("step_rate", 14.0),
            ("birth", 8.0),
            ("survive", survive),
            ("reseed", reseed),
        ],
        beat,
        on_beat,
    )
}

/// **The readings Plan 0253 Phase 1 owes**, printed with the adapter they were
/// taken on: what one relax pass and one step pass cost at grids 192, 512 and
/// 1024 on every hardware adapter, how many passes an epoch takes on the
/// labyrinth's grown maze, the labyrinth's per-generation change counts over a
/// driven run, and how often that run draws a route at three pass rates.
///
/// ```text
/// cargo nextest run -p rlx-core --lib --run-ignored only route_readings --no-capture
/// ```
#[test]
#[ignore = "a measurement, not a gate — ADR-0071"]
fn route_readings() {
    use crate::render::context::{AdapterChoice, list_adapters};
    // --- What a pass costs, per hardware adapter ---
    for (index, adapter) in list_adapters().iter().enumerate() {
        let Ok(ctx) = RenderContext::new_headless_on(64, 64, &AdapterChoice::Index(index)) else {
            continue;
        };
        if ctx.is_software() {
            continue;
        }
        println!("adapter {index}: {}", ctx.adapter());
        let _ = adapter;
        let mut driver = Driver::new(&ctx);
        for grid in [192u32, 512, 1024] {
            let mut scene = scene_with(&ctx, life(grid, true, 17));
            let fps = 60;
            let dt = 1.0 / fps as f32;
            driver.frame(&mut scene, dt, &[("step_rate", 0.0)]);
            // Few enough that a sweep from the centre of the seeded field is
            // still relaxing at the frame's end on the smallest grid.
            const PASSES: u32 = 8;
            let route = [
                ("step_rate", 0.0),
                ("birth", 0.0),
                ("survive", 511.0),
                ("route", 1.0),
            ];
            // Held still until the quiet takes a snapshot.
            for _ in 0..6 {
                driver.frame(&mut scene, ONE_GEN.0, &with_route(&FROZEN));
            }
            scene.route_rate = (PASSES * fps) as f32;
            let seeded = mirror::seed_field(grid, field_seed(17), live_threshold(LIFE_DENSITY));
            let source = mirror::centre_source(&mirror::open_mask(&seeded), grid);
            let before = word(&control(&ctx, &scene), route::C_RELAX_PASSES);
            let costs = timed_frames(&ctx, &mut driver, &mut scene, dt, &route, 60, source);
            let relaxed = word(&control(&ctx, &scene), route::C_RELAX_PASSES) - before;
            assert_eq!(
                relaxed,
                PASSES * 60,
                "grid {grid}: a timed pass did not relax"
            );
            let relax = costs.iter().find(|(l, _)| l == "cellular-route-relax");
            let control = costs.iter().find(|(l, _)| l == "cellular-route-control");
            let steps = [("step_rate", 8.0 * fps as f32), ("route", 0.0)];
            let step_costs = timed_frames(&ctx, &mut driver, &mut scene, dt, &steps, 60, None);
            let step = step_costs.iter().find(|(l, _)| l == "cellular-step");
            println!(
                "  grid {grid}: relax {:.4} ms/pass, its control {:.4} ms/pass, step {:.4} \
                 ms/generation",
                relax.map_or(f64::NAN, |r| r.1 / f64::from(PASSES)),
                control.map_or(f64::NAN, |r| r.1 / f64::from(PASSES)),
                step.map_or(f64::NAN, |r| r.1 / f64::from(MAX_GENERATIONS_PER_FRAME)),
            );
            drop(scene);
        }
    }

    let Some(ctx) = context() else {
        return;
    };
    println!("labyrinth readings on {}", ctx.adapter());
    let config = labyrinth_config();
    let fps = 30;
    let dt = 1.0 / fps as f32;
    let mut driver = Driver::new(&ctx);

    // --- Sweep 1 on the grown maze at generation 600 ---
    let mut scene = scene_with(&ctx, config);
    let (mut frame, mut generations) = (0u32, 0u32);
    while generations < 600 {
        let (params, ..) = labyrinth_params(frame, fps);
        generations += driver.frame(&mut scene, dt, &params);
        frame += 1;
    }
    assert_eq!(generations, 600);
    // Held still from here: the quiet starts an epoch from the centre rule.
    let words = drive_until(
        &ctx,
        &mut driver,
        &mut scene,
        ONE_GEN.0,
        &with_route(&FROZEN),
        2000,
        has_committed,
    );
    let open = mirror::open_mask(&states(&ctx, &scene));
    let on_route = committed(&ctx, &scene)
        .iter()
        .filter(|d| **d != route::INF)
        .count();
    println!(
        "epoch at generation 600 from the centre rule: {} work passes, route length {} over \
         {on_route} cells; {} of {} cells open",
        word(&words, route::C_LAST_EPOCH),
        word(&words, route::C_LENGTH),
        open.iter().filter(|o| **o == 1).count(),
        open.len()
    );
    // The open cells' four-connected components, and a sweep from inside the
    // largest: the centre source may fall in a pocket.
    let n = config.grid;
    let mut label = vec![u32::MAX; open.len()];
    let mut sizes: Vec<(usize, u32)> = Vec::new();
    for start in 0..open.len() {
        if open[start] != 1 || label[start] != u32::MAX {
            continue;
        }
        let id = sizes.len() as u32;
        let mut stack = vec![start];
        label[start] = id;
        let mut size = 0;
        while let Some(i) = stack.pop() {
            size += 1;
            let (x, y) = ((i as u32 % n) as i64, (i as u32 / n) as i64);
            for (dx, dy) in [(-1i64, 0i64), (1, 0), (0, -1), (0, 1)] {
                let j = ((y + dy).rem_euclid(n as i64) * n as i64 + (x + dx).rem_euclid(n as i64))
                    as usize;
                if open[j] == 1 && label[j] == u32::MAX {
                    label[j] = id;
                    stack.push(j);
                }
            }
        }
        sizes.push((size, start as u32));
    }
    sizes.sort_unstable_by(|a, b| b.cmp(a));
    let histogram =
        |lo: usize, hi: usize| sizes.iter().filter(|(s, _)| (lo..hi).contains(s)).count();
    println!(
        "{} open components; largest {:?}; sizes 1-9: {}, 10-99: {}, 100-999: {}, 1000+: {}",
        sizes.len(),
        sizes.iter().take(6).map(|(s, _)| *s).collect::<Vec<_>>(),
        histogram(1, 10),
        histogram(10, 100),
        histogram(100, 1000),
        histogram(1000, usize::MAX)
    );
    let (largest, cell) = sizes[0];
    let source = (0..open.len() as u32)
        .filter(|i| label[*i as usize] == label[cell as usize])
        .min_by_key(|i| {
            let m = i64::from(n);
            let dx = 2 * i64::from(i % n) + 1 - m;
            let dy = 2 * i64::from(i / n) + 1 - m;
            (dx * dx + dy * dy, *i)
        })
        .expect("the largest component has a cell");
    force_sweep(&ctx, &scene, source);
    let words = drive_until(
        &ctx,
        &mut driver,
        &mut scene,
        ONE_GEN.0,
        &with_route(&FROZEN),
        2000,
        |w| word(w, route::C_COMMITS) >= 2,
    );
    let cpu = mirror::double_sweep(&open, n, true, source);
    println!(
        "epoch from the largest component ({largest} cells, source {source}): {} work passes, \
         route length {} (CPU {}); the route matches the CPU: {}",
        word(&words, route::C_LAST_EPOCH),
        word(&words, route::C_LENGTH),
        cpu.length,
        committed(&ctx, &scene) == mirror::commit(&open, &cpu)
    );
    drop(scene);

    // --- How often the route is drawn on a driven run, by pass rate ---
    for rate in [60.0, 120.0, 240.0] {
        let mut scene = scene_with(&ctx, config);
        scene.route_rate = rate;
        let (mut drawn, mut measured) = (0u32, 0u32);
        for frame in 0..104 * fps / 2 {
            let (mut params, beat, _) = labyrinth_params(frame, fps);
            params.push(("route", 1.0));
            driver.frame(&mut scene, dt, &params);
            if beat >= 32 {
                measured += 1;
                if word(&control(&ctx, &scene), route::C_SHOWN) == route::SHOWN_ON {
                    drawn += 1;
                }
            }
        }
        let words = control(&ctx, &scene);
        println!(
            "{rate} passes/s over 72 beats after 32 of warm-up: {} commits, {} abandoned, last \
             epoch {} passes, a route drawn (not fading) in {drawn} of {measured} frames",
            word(&words, route::C_COMMITS),
            word(&words, route::C_ABANDONED),
            word(&words, route::C_LAST_EPOCH),
        );
        drop(scene);
    }

    // --- Change counts over a driven run ---
    let mut scene = scene_with(&ctx, config);
    let warm_beats = 32;
    let beats = 72;
    let mut last_generations = 0;
    let mut log: Vec<(f32, u32, u32, u32)> = Vec::new();
    let mut last_bite = None;
    for frame in 0..(warm_beats + beats) * fps / 2 {
        let (mut params, beat, on_beat) = labyrinth_params(frame, fps);
        if beat >= warm_beats {
            params.push(("route", 1.0));
        }
        if on_beat && beat.is_multiple_of(4) {
            last_bite = Some(frame);
        }
        driver.frame(&mut scene, dt, &params);
        if beat < warm_beats {
            continue;
        }
        let words = control(&ctx, &scene);
        let g = word(&words, route::C_GENERATIONS);
        if g != last_generations {
            last_generations = g;
            let since_bite = last_bite.map_or(u32::MAX, |b| frame - b);
            log.push((
                frame as f32 * dt,
                beat,
                since_bite,
                word(&words, route::C_LAST_MOVED),
            ));
        }
    }
    // The first compare after the route turns on is against no mask.
    log.remove(0);
    println!("time  beat  frames-since-bite  changed  (rule loosened on beats 0-5 of 32)");
    for (t, beat, since, moved) in &log {
        println!(
            "{t:6.2} {beat:4} {since:4} {moved:6}{}",
            if beat % 32 < 6 { "  loose" } else { "" }
        );
    }
    let mut runs: Vec<(usize, f32)> = Vec::new();
    let mut run = 0usize;
    for (t, _, _, moved) in &log {
        if *moved == 0 {
            run += 1;
        } else if run > 0 {
            runs.push((run, *t));
            run = 0;
        }
    }
    println!(
        "still stretches (generations, ending at s): {runs:?}; at 14 generations/s a \
         generation is {:.3} s",
        1.0 / 14.0
    );
}

/// Start a fresh sweep from `source` on the snapshot already taken, by writing
/// the control words a measurement wants rather than the ones the centre rule
/// would reach.
fn force_sweep(ctx: &RenderContext, scene: &CellularScene, source: u32) {
    let route = route_of(scene);
    let tiles = route::tiles(scene.config.grid);
    let mut words = control(ctx, scene);
    for (at, value) in [
        (route::C_ARGS, tiles),
        (route::C_ARGS + 1, tiles),
        (route::C_ARGS + 2, 1),
        (route::C_ACTION, route::A_RELAX),
        (route::C_SEARCH, 1),
        (route::C_FRESH, 1),
        (route::C_SOURCE, source),
        (route::C_SWEEP_PASSES, 0),
        (route::C_CONVERGED, 0),
        (route::C_EPOCH_PASSES, 0),
    ] {
        words[at as usize] = value;
    }
    ctx.queue
        .write_buffer(&route.control, 0, bytemuck::cast_slice(&words));
}

/// Render `frames` frames of `params` with the pass timer armed, and return each
/// label's mean milliseconds per frame. With `restart`, every frame first
/// starts a fresh sweep from that cell, so its route passes all relax.
fn timed_frames(
    ctx: &RenderContext,
    driver: &mut Driver<'_>,
    scene: &mut CellularScene,
    dt: f32,
    params: &[(&str, f32)],
    frames: u32,
    restart: Option<u32>,
) -> Vec<(String, f64)> {
    let mut timer = gpu::PassTimer::new(&ctx.device, &ctx.queue);
    if timer.is_none() {
        println!("  this adapter has no timestamp queries");
        return Vec::new();
    }
    let mut costs = crate::render::capture::PassCosts::default();
    for _ in 0..frames {
        if let Some(source) = restart {
            force_sweep(ctx, scene, source);
        }
        scene.set_target_size(TARGET, TARGET);
        scene.advance(dt);
        scene.reset_params();
        for (name, value) in params {
            scene.set_param(name, *value);
        }
        scene.update(&AnalysisFrame::default());
        gpu::arm_pass_timer(timer.take());
        let mut encoder = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("cellular-timed"),
            });
        scene.render(&ctx.queue, &mut encoder, &driver.view, 1.0);
        timer = gpu::disarm_pass_timer(&mut encoder);
        ctx.queue.submit([encoder.finish()]);
        if let Some(t) = timer.as_mut() {
            t.map();
        }
        ctx.device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: Some(std::time::Duration::from_secs(30)),
            })
            .expect("poll the timed frame");
        if let Some(t) = timer.as_mut() {
            t.collect(&mut costs);
        }
    }
    costs
        .rows()
        .into_iter()
        .map(|(label, ms)| (label.to_owned(), ms))
        .collect()
}
