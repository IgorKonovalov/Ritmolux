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
    let mut scene = CellularScene::new(&ctx.device, crate::render::COMPOSITE_FORMAT);
    scene.configure(&GeneratorConfig::Cellular(config));
    scene
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
    _target: wgpu::Texture,
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
            _target: target,
            view,
            time: 0.0,
        }
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

/// Overwrite the field with exactly `cells` live and every other cell dead.
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
         let c = vec2<u32>(in.pos.xy);\n    var v = vec4<f32>(0.0, 0.0, 0.0, 1.0);\n{tests}    \
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
        let step = format!(
            "{}const MODE: u32 = {}u;\n{}{}",
            gpu::FULLSCREEN_VS_UV_FLIPPED,
            pass.mode(),
            gpu::HASH_WGSL,
            shader::STEP_SHADER
        );
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
