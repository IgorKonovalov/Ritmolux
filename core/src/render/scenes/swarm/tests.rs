// Tests index fixed-size arrays and panic on failure; allowed over the
// file's hot-path pragma — this is not the render path.
#![allow(clippy::indexing_slicing, clippy::panic, clippy::expect_used)]

use super::{
    DEFAULT_DEPTH_FADE, DEFAULT_HUE, DEFAULT_HUE_CENTER, DEFAULT_HUE_SPREAD, DEFAULT_SPIN,
    FALLBACK_DT, MARGIN, PIVOT, Phase, REST_FOV, SEED, SIZE_DEPTH, Scene, SwarmInstance,
    SwarmScene, TWINKLE_FREQ_HI, TWINKLE_FREQ_LO, Z_FAR, Z_NEAR, Z_SPAN, channel, depth_light,
    half_extent, hue_coord, size_factor, slab_fade, sway_bound, twinkle_factor, unit,
};
use crate::render::palette::Palette;
use crate::render::scenes::SeededRng;

/// The particle count these tests run at — the floor tier's, which is the
/// number the seeded-scatter assertions below were written against and the one
/// every golden capture draws (Plan 0044).
const FLOOR_PARTICLES: usize = crate::render::TierConfig::FLOOR.swarm_particles;

/// The floor tier's swarm blur cap, which every scene these tests build is
/// held to.
const MAX_COC: f32 = crate::render::TierConfig::FLOOR.swarm_max_coc_px as f32;

/// Target aspects worth checking a domain against: 16:9, 16:10, 4:3, an
/// ultrawide, and a portrait.
const ASPECTS: [f32; 5] = [16.0 / 9.0, 16.0 / 10.0, 4.0 / 3.0, 21.0 / 9.0, 9.0 / 16.0];

/// ADR-0037's disagreeing pair: two targets whose aspects differ, so a shape
/// taken from anything but the target is wrong on at least one of them.
const TARGETS: [(u32, u32); 2] = [(1280, 800), (1920, 1080)];

/// A scene of `particles` on a software device, or `None` (a logged skip) when
/// the runner has none (ADR-0016). The renderer is returned beside it so the
/// device outlives the scene.
fn scene(particles: usize) -> Option<(crate::render::Renderer, SwarmScene)> {
    use crate::render::context::RenderError;
    use crate::render::{COMPOSITE_FORMAT, HeadlessOptions, Renderer};

    let renderer = match Renderer::new_headless(HeadlessOptions {
        width: 64,
        height: 64,
        prefer_software: true,
    }) {
        Ok(renderer) => renderer,
        Err(RenderError::RequestAdapter(_)) => {
            eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
            return None;
        }
        Err(e) => panic!("headless renderer build failed: {e}"),
    };
    let scene = SwarmScene::new(&renderer.ctx.device, COMPOSITE_FORMAT, particles, MAX_COC);
    Some((renderer, scene))
}

/// A world point's normalized-device position through `view`: the CPU mirror
/// of the shader's `project()` and divide.
fn ndc(view: &crate::render::camera::CameraView, p: [f32; 3]) -> [f32; 2] {
    let c = view.clip(p);
    [c[0] / c[3], c[1] / c[3]]
}

/// The torus has the **render target's** shape at every depth (ADR-0037,
/// ADR-0259): its cross-section is the rest frustum's times the margin, so its
/// aspect is the target's whatever the depth, and it grows linearly with depth.
#[test]
fn the_domain_takes_its_shape_from_the_target_at_every_depth() {
    for aspect in ASPECTS {
        for z in [Z_NEAR, PIVOT, Z_FAR] {
            let (hx, hy) = half_extent(z, aspect);
            assert!(
                (hx / hy - aspect).abs() < 1e-5,
                "cross-section shape {:.4} at depth {z} must equal the target's {aspect:.4}",
                hx / hy
            );
            // At rest the frame's half-height at `z` is `z * tan(fov / 2)`, so
            // the seam sits `MARGIN` times past it.
            let frame = z * (0.5 * REST_FOV).tan();
            assert!(
                (hy / frame - MARGIN).abs() < 1e-5,
                "the seam must sit MARGIN past the frame at depth {z}: {:.4}",
                hy / frame
            );
        }
    }
}

/// **Over 600 frames of the default flow, no particle within one sprite radius
/// of the wrap seam projects inside the frame**, at 1280x800 and at 1920x1080
/// (Plan 0239 Phase 1, ADR-0037's disagreeing pair) — at rest and with the
/// camera swayed to [`sway_bound`] in each of the four diagonal directions.
///
/// The projection is the camera's own CPU mirror of `project()`, through the
/// frame `render` would build, so this asserts on what the GPU draws.
#[test]
fn the_wrap_seam_stays_outside_the_frame_at_every_depth() {
    use crate::dsp::AnalysisFrame;

    for (width, height) in TARGETS {
        let Some((_renderer, mut scene)) = scene(FLOOR_PARTICLES) else {
            return;
        };
        let aspect = width as f32 / height as f32;
        scene.aspect = aspect;
        scene.target = (width, height);

        let [yaw, pitch] = sway_bound(REST_FOV, 1.0, aspect, [0.0, 0.0]);
        assert!(
            yaw > 0.02 && pitch > 0.02,
            "the margin must leave a usable sway at {width}x{height}: yaw {yaw:.4}, pitch {pitch:.4}"
        );
        let views: Vec<_> = [
            (0.0, 0.0),
            (yaw, pitch),
            (-yaw, pitch),
            (yaw, -pitch),
            (-yaw, -pitch),
        ]
        .into_iter()
        .map(|(y, p)| {
            // Bound past the limit, so the clamp is what is measured.
            scene.camera.yaw = y * 2.0;
            scene.camera.pitch = p * 2.0;
            scene.camera_frame(aspect, 0.0).view
        })
        .collect();

        let frame = AnalysisFrame::default();
        let (mut near_seam, mut inside, mut worst) = (0usize, 0usize, f32::INFINITY);
        for _ in 0..600 {
            scene.update(&frame);
            for (p, inst) in scene.particles.iter().zip(&scene.instance_data) {
                // The sprite's radius in frustum coordinates: its
                // normalized-device height at this depth over the margin, and
                // the width over the aspect too, because it is round on screen.
                let r_ndc = inst.radius * SIZE_DEPTH / p.z;
                let (ru, rv) = (r_ndc / (aspect * MARGIN), r_ndc / MARGIN);
                if p.pos[0].abs() < 1.0 - ru && p.pos[1].abs() < 1.0 - rv {
                    continue;
                }
                near_seam += 1;
                for view in &views {
                    let [x, y] = ndc(view, inst.center);
                    let past = x.abs().max(y.abs());
                    worst = worst.min(past);
                    if past < 1.0 {
                        inside += 1;
                    }
                }
            }
        }
        eprintln!(
            "{width}x{height}: {near_seam} particle-frames within a sprite radius of the seam, \
             {inside} projected inside the frame; nearest {worst:.4} ndc (sway bound \
             yaw {yaw:.4}, pitch {pitch:.4})"
        );
        assert!(
            near_seam > 1000,
            "too few particles reached the seam for this to measure anything: {near_seam}"
        );
        assert_eq!(
            inside, 0,
            "{inside} particle-frames within one sprite radius of the wrap seam projected \
             inside the frame at {width}x{height}"
        );
    }
}

/// **Far particles cross the screen more slowly than near ones under the same
/// flow** (Plan 0239 Phase 1): the mean screen speed of the far third of the
/// slab is lower than the near third's, measured on the projected centres over
/// the default flow. Perspective alone predicts the far third at about half the
/// near third's speed; the bound below is that with room for the field's own
/// depth dependence.
#[test]
fn far_particles_cross_the_screen_more_slowly_than_near_ones() {
    use crate::dsp::AnalysisFrame;

    let Some((_renderer, mut scene)) = scene(FLOOR_PARTICLES) else {
        return;
    };
    let aspect = 16.0 / 9.0;
    scene.aspect = aspect;
    let view = scene.camera_frame(aspect, 0.0).view;
    let frame = AnalysisFrame::default();
    for _ in 0..60 {
        scene.update(&frame);
    }

    let third = Z_SPAN / 3.0;
    let (mut near, mut far) = ((0.0f64, 0usize), (0.0f64, 0usize));
    let mut previous: Vec<([f32; 2], [f32; 2], f32)> = scene
        .particles
        .iter()
        .zip(&scene.instance_data)
        .map(|(p, inst)| (p.pos, ndc(&view, inst.center), p.z))
        .collect();
    for _ in 0..240 {
        scene.update(&frame);
        for ((p, inst), before) in scene
            .particles
            .iter()
            .zip(&scene.instance_data)
            .zip(previous.iter_mut())
        {
            let now = ndc(&view, inst.center);
            let wrapped = (p.pos[0] - before.0[0]).abs() > 1.0
                || (p.pos[1] - before.0[1]).abs() > 1.0
                || (p.z - before.2).abs() > 0.5 * Z_SPAN;
            if !wrapped {
                // Isotropic: an x step in normalized-device units is `aspect`
                // times as many pixels as a y step.
                let step = (((now[0] - before.1[0]) * aspect).powi(2)
                    + (now[1] - before.1[1]).powi(2))
                .sqrt() as f64;
                if p.z < Z_NEAR + third {
                    near.0 += step;
                    near.1 += 1;
                } else if p.z > Z_FAR - third {
                    far.0 += step;
                    far.1 += 1;
                }
            }
            *before = (p.pos, now, p.z);
        }
    }
    let near_speed = near.0 / near.1.max(1) as f64;
    let far_speed = far.0 / far.1.max(1) as f64;
    eprintln!(
        "screen speed: near third {near_speed:.6} over {} samples, far third {far_speed:.6} \
         over {} samples (ratio {:.3})",
        near.1,
        far.1,
        far_speed / near_speed
    );
    assert!(
        near.1 > 10_000 && far.1 > 10_000,
        "both thirds must be populated"
    );
    assert!(
        far_speed < near_speed * 0.8,
        "the far third must cross the screen more slowly: {far_speed:.6} vs {near_speed:.6}"
    );
}

/// The sway bound comes from the margin **and** the field of view: a wider
/// `fov` shows more of the margin and leaves less to sway into, a `zoom` that
/// narrows it leaves more, and a pan eats into it. Past the point where the
/// seam already shows, the bound is zero rather than negative.
#[test]
fn the_sway_bound_follows_the_lens_and_the_pan() {
    let aspect = 16.0 / 9.0;
    let [rest_yaw, rest_pitch] = sway_bound(REST_FOV, 1.0, aspect, [0.0, 0.0]);
    let [wide_yaw, wide_pitch] = sway_bound(0.9, 1.0, aspect, [0.0, 0.0]);
    let [zoomed_yaw, zoomed_pitch] = sway_bound(REST_FOV, 1.3, aspect, [0.0, 0.0]);
    let [panned_yaw, panned_pitch] = sway_bound(REST_FOV, 1.0, aspect, [0.1, 0.1]);
    let [open_yaw, open_pitch] = sway_bound(1.2, 1.0, aspect, [0.0, 0.0]);
    eprintln!(
        "sway bound (yaw, pitch): rest {rest_yaw:.4} {rest_pitch:.4}, fov 0.9 {wide_yaw:.4} \
         {wide_pitch:.4}, zoom 1.3 {zoomed_yaw:.4} {zoomed_pitch:.4}, pan 0.1 {panned_yaw:.4} \
         {panned_pitch:.4}"
    );
    assert!(wide_yaw < rest_yaw && wide_pitch < rest_pitch);
    assert!(zoomed_yaw > rest_yaw && zoomed_pitch > rest_pitch);
    assert!(panned_yaw < rest_yaw && panned_pitch < rest_pitch);
    assert_eq!([open_yaw, open_pitch], [0.0, 0.0]);
    for value in sway_bound(f32::NAN, f32::INFINITY, f32::NAN, [f32::NAN; 2]) {
        assert!(
            value.is_finite() && value >= 0.0,
            "a non-finite input must give a usable bound"
        );
    }
}

/// `depth_fade`'s default reproduces ADR-0044's ramp — 1.05 at the near bound,
/// 0.45 at the far — and `0` lights every depth alike. The slab fade is 1 in
/// the interior and 0 at both bounds, where a particle wraps.
#[test]
fn the_depth_light_reproduces_the_ramp_and_the_slab_bounds_are_dark() {
    assert!((depth_light(Z_NEAR, DEFAULT_DEPTH_FADE) - 1.05).abs() < 1e-5);
    assert!((depth_light(Z_FAR, DEFAULT_DEPTH_FADE) - 0.45).abs() < 1e-5);
    assert_eq!(depth_light(Z_NEAR, 0.0), depth_light(Z_FAR, 0.0));
    assert!(depth_light(Z_FAR, 3.0) >= 0.0);
    assert_eq!(slab_fade(Z_NEAR), 0.0);
    assert_eq!(slab_fade(Z_FAR), 0.0);
    assert_eq!(slab_fade(PIVOT), 1.0);
}

/// **Two frames a second apart differ** (Plan 0239 Phase 1): the swarm golden
/// fixture rendered through the headless capture path `shot` drives, at frames
/// 60 and 120.
#[test]
fn the_swarm_fixture_moves_between_two_frames_a_second_apart() {
    use crate::dsp::AnalysisFrame;
    use crate::preset::Preset;
    use crate::render::context::RenderError;
    use crate::render::{HeadlessOptions, Renderer};

    const FIXTURE: &str = include_str!("../../../../tests/fixtures/swarm.toml");
    let mut renderer = match Renderer::new_headless(HeadlessOptions {
        width: 160,
        height: 100,
        prefer_software: true,
    }) {
        Ok(renderer) => renderer,
        Err(RenderError::RequestAdapter(_)) => {
            eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
            return;
        }
        Err(e) => panic!("headless renderer build failed: {e}"),
    };
    let preset = Preset::from_toml_str(FIXTURE).expect("the swarm fixture parses");
    let name = preset.name.clone();
    renderer.set_presets(vec![preset]);
    let frame = AnalysisFrame::default();
    let a = renderer
        .capture_preset(&name, &frame, 60)
        .expect("capture frame 60");
    let b = renderer
        .capture_preset(&name, &frame, 120)
        .expect("capture frame 120");
    let lit = a
        .rgba
        .chunks_exact(4)
        .filter(|px| px[..3] != [0, 0, 0])
        .count();
    let differing = a
        .rgba
        .chunks_exact(4)
        .zip(b.rgba.chunks_exact(4))
        .filter(|(x, y)| x[..3] != y[..3])
        .count();
    eprintln!("swarm fixture: {lit} lit pixels at frame 60, {differing} differ at frame 120");
    assert!(lit > 100, "the fixture must draw: {lit} lit pixels");
    assert!(
        differing > 100,
        "two frames a second apart must differ: {differing} pixels"
    );
}

/// The depth axis is **seeded**, so a capture is reproducible run-to-run
/// (NFR §6) — and it genuinely spans the slab, which is what makes perspective,
/// the depth light and motion parallax do anything.
#[test]
fn the_seeded_scatter_reproduces_the_same_depth_sequence() {
    let depths = || {
        let mut rng = SeededRng::new(SEED);
        (0..FLOOR_PARTICLES)
            .map(|_| SwarmScene::spawn(&mut rng).z)
            .collect::<Vec<f32>>()
    };
    let (a, b) = (depths(), depths());
    assert_eq!(a, b, "the same seed must give the same depth sequence");

    let lo = (a.iter().copied().fold(f32::INFINITY, f32::min) - Z_NEAR) / Z_SPAN;
    let hi = (a.iter().copied().fold(f32::NEG_INFINITY, f32::max) - Z_NEAR) / Z_SPAN;
    let mean = (a.iter().sum::<f32>() / a.len() as f32 - Z_NEAR) / Z_SPAN;
    assert!(
        (0.0..0.02).contains(&lo) && (0.98..=1.0).contains(&hi),
        "depth must span the whole slab, got {lo:.4}..{hi:.4} of it"
    );
    assert!(
        (0.45..0.55).contains(&mean),
        "depth must populate the slab evenly, mean was {mean:.4} of it"
    );
}

/// A resize rescales the field instead of teleporting it (ADR-0044's
/// consequence: "the wrap must stay stable across one rather than teleporting
/// every particle at once").
///
/// Frustum-coordinate storage is what buys this, and the test says so by
/// measuring the alternative alongside: with world-space positions, shrinking
/// the domain re-wraps everything outside the new bounds, and those particles
/// jump by a full domain width. Frustum coordinates move continuously with the
/// change. Measured at the slab's centre; the half-extents are linear in depth,
/// so every depth scales the same way.
#[test]
fn a_resize_rescales_the_field_rather_than_wrapping_it() {
    let (before, after) = (16.0 / 9.0, 16.0 / 10.0);
    let (bx0, by0) = half_extent(SIZE_DEPTH, before);
    let (bx1, by1) = half_extent(SIZE_DEPTH, after);

    // A fan of normalized positions spanning the torus, including both seams.
    let samples: Vec<[f32; 2]> = (0..64)
        .map(|i| {
            let u = i as f32 / 63.0 * 2.0 - 1.0;
            [u, -u]
        })
        .collect();

    let mut worst_normalized = 0.0f32;
    let mut worst_world_space = 0.0f32;
    for s in &samples {
        // What this scene does: the normalized position is untouched, so the
        // world position moves by exactly the change in the half-extents.
        let moved = ((s[0] * bx1 - s[0] * bx0).powi(2) + (s[1] * by1 - s[1] * by0).powi(2)).sqrt();
        worst_normalized = worst_normalized.max(moved);

        // What a world-space store would do: keep the world position and
        // re-wrap it into the new domain.
        let (mut wx, wy) = (s[0] * bx0, s[1] * by0);
        if wx > bx1 {
            wx -= 2.0 * bx1;
        } else if wx < -bx1 {
            wx += 2.0 * bx1;
        }
        let jump = ((wx - s[0] * bx0).powi(2) + (wy - s[1] * by0).powi(2)).sqrt();
        worst_world_space = worst_world_space.max(jump);
    }

    // 16:9 -> 16:10 narrows the x half-extent by ~0.22 world units; nothing
    // moves further than that, and the y axis does not move at all.
    assert!(
        worst_normalized < 0.3,
        "a resize must move particles continuously, worst was {worst_normalized:.3}"
    );
    assert!(
        worst_world_space > 2.0,
        "the world-space alternative must genuinely teleport, or this test proves \
         nothing: worst jump was {worst_world_space:.3}"
    );
}

/// The default hue band (`center = 0.5`, `spread = 1`, `hue = 0`) reduces to
/// `particle_hue`, so the band leaves the swarm's colour alone (Plan 0020).
#[test]
fn default_hue_band_is_the_prior_full_wheel() {
    for &ph in &[0.0, 0.2, 0.5, 0.73, 0.99] {
        let coord = hue_coord(DEFAULT_HUE_CENTER, DEFAULT_HUE_SPREAD, ph, DEFAULT_HUE);
        assert!(
            (coord - ph).abs() < 1e-6,
            "default band maps particle_hue to itself: {coord} vs {ph}"
        );
    }
}

/// A narrow `hue_spread` collapses the full particle-hue range into a tight
/// LUT band, so the sampled colours cluster (a coherent single-family swarm)
/// where `spread = 1` samples the whole wheel (rainbow confetti). Measured as
/// the spread of sampled RGB.
#[test]
fn narrow_spread_makes_colour_coherent() {
    let pal = Palette::default_spectrum();
    // Total variance of the sampled colours across a fan of particle hues.
    let colour_spread = |spread: f32| -> f32 {
        let hues: Vec<f32> = (0..64).map(|i| i as f32 / 64.0).collect();
        let cols: Vec<[f32; 3]> = hues
            .iter()
            .map(|&h| pal.sample(hue_coord(0.5, spread, h, 0.0), 0.0))
            .collect();
        let n = cols.len() as f32;
        let mut mean = [0.0f32; 3];
        for c in &cols {
            for k in 0..3 {
                mean[k] += c[k] / n;
            }
        }
        let mut var = 0.0f32;
        for c in &cols {
            for k in 0..3 {
                var += (c[k] - mean[k]).powi(2);
            }
        }
        var / n
    };
    let narrow = colour_spread(0.1);
    let full = colour_spread(1.0);
    assert!(
        narrow < full * 0.25,
        "narrow band ({narrow:.4}) is far more coherent than the full wheel ({full:.4})"
    );
}

// -----------------------------------------------------------------------
// Per-mark individuation (Plan 0077 Phase 2, backlog 0068)
// -----------------------------------------------------------------------

/// The pure-function half of the individuation contract, in the emitter's
/// test's shape: both factors collapse to **exactly** `1.0` at their defaults
/// — the arithmetic every untouched golden baseline rests on — and both
/// genuinely vary across the population when opened. The third block is the
/// property backlog 0068 measured the emitter for and the swarm lacked: with
/// rate AND phase drawn per particle, the population's mean stays nearly flat
/// while every member swings — a shared-rate field would swing its mean as one
/// sheet however the phases scattered.
#[test]
fn per_mark_individuation_collapses_at_zero_and_shimmers_without_breathing() {
    let n = 512u32;
    let freqs: Vec<f32> = (0..n)
        .map(|i| {
            TWINKLE_FREQ_LO + unit(i, channel::TWINKLE_FREQ) * (TWINKLE_FREQ_HI - TWINKLE_FREQ_LO)
        })
        .collect();
    let phases: Vec<f32> = (0..n).map(|i| unit(i, channel::TWINKLE_PHASE)).collect();

    // Exact identity at the defaults, falsifiable in both directions.
    for i in 0..n as usize {
        assert_eq!(twinkle_factor(freqs[i], phases[i], 3.7, 0.0), 1.0);
        assert_eq!(size_factor(unit(i as u32, channel::SIZE), 0.0), 1.0);
    }

    // Opened, both vary across the population.
    let sizes: Vec<f32> = (0..n)
        .map(|i| size_factor(unit(i, channel::SIZE), 0.7))
        .collect();
    let twinkles: Vec<f32> = (0..n as usize)
        .map(|i| twinkle_factor(freqs[i], phases[i], 3.7, 0.8))
        .collect();
    for (label, series) in [("size", &sizes), ("twinkle", &twinkles)] {
        let lo = series.iter().copied().fold(f32::INFINITY, f32::min);
        let hi = series.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        assert!(
            hi - lo > 1e-3,
            "{label} must vary across the population: {lo} .. {hi}"
        );
    }

    // Shimmer without breathing, at the factor level: the field mean's swing
    // over a window against the mean member swing over the same window. The
    // emitter asserted 8x on the same statistic; the swarm uses the emitter's
    // band and hash, so the same margin holds.
    const TWINKLE: f32 = 0.8;
    let times: Vec<f32> = (0..240).map(|f| f as f32 / 60.0).collect();
    let swing = |series: &[f32]| {
        let lo = series.iter().copied().fold(f32::INFINITY, f32::min);
        let hi = series.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        hi - lo
    };
    let field_mean: Vec<f32> = times
        .iter()
        .map(|&t| {
            (0..n as usize)
                .map(|i| twinkle_factor(freqs[i], phases[i], t, TWINKLE))
                .sum::<f32>()
                / n as f32
        })
        .collect();
    let member_swing: f32 = (0..n as usize)
        .map(|i| {
            let series: Vec<f32> = times
                .iter()
                .map(|&t| twinkle_factor(freqs[i], phases[i], t, TWINKLE))
                .collect();
            swing(&series)
        })
        .sum::<f32>()
        / n as f32;
    let field_swing = swing(&field_mean);
    assert!(
        member_swing > TWINKLE,
        "members must actually twinkle (mean swing {member_swing:.3}), or the \
         flat field mean below is vacuous"
    );
    assert!(
        member_swing > field_swing * 8.0,
        "the field must not flash as one sheet: member swing {member_swing:.3} \
         vs field swing {field_swing:.3}"
    );
}

/// **The captured pixels say the same thing** — Phase 2's done-when, on the
/// real pipeline: with `twinkle` bound at fixed (silent) audio, two captures
/// at different scene times differ per mark while the whole-frame mean stays
/// within a bound derived from the twinkle depth; and the whole path is
/// deterministic and byte-identical at the explicit defaults.
///
/// **The bound's derivation, stated as the done-when requires.** Each mark's
/// brightness factor swings up to `1 ± TWINKLE`; if every mark shared one
/// oscillator the whole-frame mean would swing by up to `TWINKLE` relatively
/// (the sheet flash — the failure this gate names). With rates and phases
/// drawn independently per particle, the mean of `N` independent unit-variance
/// oscillators scales as `1/sqrt(N)`, so the expected relative swing is
/// `~TWINKLE / sqrt(N_visible)` where `N_visible = FLOOR_PARTICLES / MARGIN^2`
/// (the tier's pool times the domain's visible fraction) — 0.6/80 = 0.75 %
/// here. The asserted bound takes **8x** that for the tonemap's nonlinearity
/// and 8-bit quantization: 6 %, still 16x under the sheet's 100 %-of-TWINKLE
/// signature, so the assertion separates the two designs by an order of
/// magnitude in both directions.
#[test]
fn a_twinkling_swarm_shimmers_without_breathing_on_the_pixels() {
    use crate::dsp::AnalysisFrame;
    use crate::preset::Preset;
    use crate::render::context::RenderError;
    use crate::render::{HeadlessOptions, Renderer};

    const SIZE: u32 = 128;
    /// Two capture points 0.5 s apart: the slowest rate in the band moves
    /// 0.175 cycles between them, so every mark's factor moves visibly.
    const FRAME_A: u32 = 40;
    const FRAME_B: u32 = 70;
    const TWINKLE: f32 = 0.6;

    let mut renderer = match Renderer::new_headless(HeadlessOptions {
        width: SIZE,
        height: SIZE,
        prefer_software: true,
    }) {
        Ok(renderer) => renderer,
        Err(RenderError::RequestAdapter(_)) => {
            eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
            return;
        }
        Err(e) => panic!("headless renderer build failed: {e}"),
    };
    let frame = AnalysisFrame::default();
    // `force`/`spin` at 0 so the seeded velocities damp out and a pixel
    // difference between the two capture points is the marks' brightness, not
    // their motion.
    let mut capture = |name: &str, extra: &str, at: u32| {
        let toml = format!(
            "system = \"swarm\"\nname = \"{name}\"\n[params]\nforce = \"0\"\n\
             spin = \"0\"\nbrightness = \"0.9\"\nsize = \"3.0\"\n{extra}"
        );
        let preset = Preset::from_toml_str(&toml).expect("the probe preset parses");
        renderer.set_presets(vec![preset]);
        renderer
            .capture_preset(name, &frame, at)
            .expect("capture the probe preset")
    };
    let mean = |img: &crate::render::CaptureImage| -> f32 {
        let sum: u64 = img
            .rgba
            .chunks_exact(4)
            .map(|px| px[0] as u64 + px[1] as u64 + px[2] as u64)
            .sum();
        sum as f32 / (img.rgba.len() as f32 / 4.0 * 3.0)
    };

    let tw = format!("twinkle = \"{TWINKLE}\"\n");
    let a = capture("tw", &tw, FRAME_A);
    let b = capture("tw", &tw, FRAME_B);

    // Determinism first: the same preset at the same scene time, byte for byte.
    let a2 = capture("tw", &tw, FRAME_A);
    assert_eq!(
        a.rgba, a2.rgba,
        "two captures at the same scene time must be byte-identical — the \
         twinkle is a pure function of (seed, scene time)"
    );

    // Per-mark life: the two capture points differ across a real share of the
    // frame...
    let differing = a
        .rgba
        .chunks_exact(4)
        .zip(b.rgba.chunks_exact(4))
        .filter(|(x, y)| x[..3] != y[..3])
        .count();
    let total = (SIZE * SIZE) as usize;
    eprintln!("twinkle: {differing} of {total} pixels differ between the two times");
    assert!(
        differing * 50 > total,
        "a bound twinkle must move individual marks between two capture points: \
         only {differing} of {total} pixels differ"
    );

    // ...while the frame's total light sits still, within the derived bound.
    let (mean_a, mean_b) = (mean(&a), mean(&b));
    let n_visible = FLOOR_PARTICLES as f32 / (MARGIN * MARGIN);
    let bound = 8.0 * TWINKLE / n_visible.sqrt();
    let rel = (mean_a - mean_b).abs() / mean_a.max(1e-6);
    eprintln!(
        "twinkle: whole-frame mean {mean_a:.3} vs {mean_b:.3} (relative {rel:.4}, \
         bound {bound:.4})"
    );
    assert!(
        rel < bound,
        "the whole-frame mean must not breathe with the twinkle: relative swing \
         {rel:.4} over the derived bound {bound:.4} — the field is flashing as \
         one sheet"
    );

    // The explicit defaults are the unbound preset, byte for byte — the
    // identity claim through the whole preset path.
    let unbound = capture("plain", "", FRAME_A);
    let explicit = capture("zeroed", "twinkle = \"0\"\nsize_spread = \"0\"\n", FRAME_A);
    assert_eq!(
        unbound.rgba, explicit.rgba,
        "explicit twinkle = 0 / size_spread = 0 must render byte-identically to \
         no binding at all"
    );

    // And `size_spread` genuinely reaches the marks (its non-vacuity arm).
    let spread = capture("spread", "size_spread = \"1.2\"\n", FRAME_A);
    let spread_differs = spread
        .rgba
        .chunks_exact(4)
        .zip(unbound.rgba.chunks_exact(4))
        .filter(|(x, y)| x[..3] != y[..3])
        .count();
    eprintln!("size_spread: {spread_differs} of {total} pixels differ from unbound");
    assert!(
        spread_differs * 50 > total,
        "a bound size_spread must move the frame: only {spread_differs} of \
         {total} pixels differ"
    );
}

// -----------------------------------------------------------------------
// The reseed disturbance (Plan 0077 Phase 3, ADR-0066 semantics)
// -----------------------------------------------------------------------

/// **A `reseed` pulse disperses the population and the flow re-gathers it** —
/// Phase 3's done-when, at fixed (silent) audio. The pulse is a pure-time
/// gate, so the edge fires at a known frame with no audio in the loop.
///
/// Four claims: shortly after the edge the frame visibly differs from the
/// no-pulse control (the disturbance is real); a few seconds later the two
/// populations paint statistically similar coverage again (the kick disturbs,
/// it does not restructure); `reseed` unbound is byte-identical
/// to an explicit `reseed = "0"` (the default is the absence); and the whole
/// path is deterministic. What is deliberately **not** claimed here is the
/// minutes-horizon rescue of a piled-up swarm — no test at this suite's
/// horizon can see it (backlog 0086); that is Plan 0077 Phase 5's bounded
/// check, run once by the content lane and recorded in the world's header.
#[test]
fn a_reseed_pulse_disperses_the_population_and_it_reconverges() {
    use crate::dsp::AnalysisFrame;
    use crate::preset::Preset;
    use crate::render::context::RenderError;
    use crate::render::metrics::{coverage, frame_diff};
    use crate::render::{HeadlessOptions, Renderer};

    const SIZE: u32 = 128;
    /// The pulse gate opens at t = 1 s (frame 60 at the fixed capture step).
    const PULSE: &str = "reseed = \"time > 1.0\"\n";
    /// Six frames after the edge: the kick has landed, the flow has not yet
    /// re-gathered it.
    const DISPERSED: u32 = 66;
    /// Four seconds in (three after the edge). Measured: the single kick's
    /// coverage gap against the control closes to **0.30 %** here — the
    /// re-gathering is fast because a ±6 % kick leaves every particle near
    /// the streamline it left. The margin below (15 %) is not tuned to that;
    /// it is the bound under which the *defect class* cannot hide: while the
    /// edge detector re-fired every frame (the `reset_params` bug caught
    /// building this — see the omission comment there), the gap read 19 % at
    /// this frame and **105 % and diverging** at ten seconds. The two
    /// behaviours sit two orders of magnitude apart at this horizon.
    const RECONVERGED: u32 = 240;
    const BLACK: [u8; 4] = [0, 0, 0, 255];
    const EPS: u8 = 10;

    let mut renderer = match Renderer::new_headless(HeadlessOptions {
        width: SIZE,
        height: SIZE,
        prefer_software: true,
    }) {
        Ok(renderer) => renderer,
        Err(RenderError::RequestAdapter(_)) => {
            eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
            return;
        }
        Err(e) => panic!("headless renderer build failed: {e}"),
    };
    let frame = AnalysisFrame::default();
    // The family-default flow (`force`/`spin` unbound), so "re-converges" means
    // the scene's own gathering, not a stilled field.
    let mut capture = |name: &str, extra: &str, at: u32| {
        let toml = format!(
            "system = \"swarm\"\nname = \"{name}\"\n[params]\n\
             brightness = \"0.9\"\nsize = \"3.0\"\n{extra}"
        );
        let preset = Preset::from_toml_str(&toml).expect("the probe preset parses");
        renderer.set_presets(vec![preset]);
        renderer
            .capture_preset(name, &frame, at)
            .expect("capture the probe preset")
    };

    // The disturbance is real: pulsed vs control, six frames after the edge.
    let control_d = capture("ctl", "reseed = \"0\"\n", DISPERSED);
    let pulsed_d = capture("pulse", PULSE, DISPERSED);
    let disperse_diff = frame_diff(&control_d, &pulsed_d);
    eprintln!("reseed: frame-diff vs control at frame {DISPERSED}: {disperse_diff:.4}");
    assert!(
        disperse_diff > 0.005,
        "a reseed pulse must visibly move the frame against the no-pulse \
         control: frame-diff {disperse_diff:.4}"
    );

    // Determinism: the same pulsed capture twice, byte for byte — the kick is
    // a pure function of (particle index, reseed ordinal).
    let pulsed_d2 = capture("pulse", PULSE, DISPERSED);
    assert_eq!(
        pulsed_d.rgba, pulsed_d2.rgba,
        "two captures of the same reseed pulse must be byte-identical"
    );

    // The population re-converges: statistically similar coverage afterwards.
    let control_r = capture("ctl", "reseed = \"0\"\n", RECONVERGED);
    let pulsed_r = capture("pulse", PULSE, RECONVERGED);
    let (cov_c, cov_p) = (
        coverage(&control_r, BLACK, EPS),
        coverage(&pulsed_r, BLACK, EPS),
    );
    let rel = (cov_c - cov_p).abs() / cov_c.max(1e-6);
    eprintln!(
        "reseed: coverage at frame {RECONVERGED}: control {cov_c:.4}, pulsed {cov_p:.4} \
         (relative gap {rel:.4})"
    );
    assert!(
        rel < 0.15,
        "three seconds after the pulse the population must paint statistically \
         similar coverage again: control {cov_c:.4} vs pulsed {cov_p:.4}"
    );

    // Unbound is the explicit zero, byte for byte.
    let unbound = capture("plain", "", DISPERSED);
    assert_eq!(
        unbound.rgba, control_d.rgba,
        "`reseed` unbound must render byte-identically to `reseed = \"0\"`"
    );
}

// -----------------------------------------------------------------------
// The mark silhouette (Plan 0070 Phase 1, ADR-0084)
// -----------------------------------------------------------------------

/// The square capture the single-mark probe below draws into. Large enough
/// that a seven-pointed star's valleys are tens of pixels from its tips —
/// the whole point of the count is that the profile has structure, and at
/// `golden.rs`'s 128 there is not enough of it to bin cleanly.
const MARK_CAPTURE: u32 = 256;

/// The mark's half-size in normalized-device units. The frame is `|ndc| <= 1`
/// on a square target, so this leaves a tenth of the frame outside the sprite
/// quad.
const MARK_HALF: f32 = 0.9;

/// **One mark, drawn large and centred, through the real swarm pipeline** —
/// the linear composite it wrote, RGBA, row-major.
///
/// A swarm normally draws thousands of sprites and no single silhouette is
/// legible in the sum, so this builds the scene with a pool of **one**, runs
/// its `update` so the silhouette uniform is the bound one, and then replaces
/// that one sprite with a known one: on the view axis at [`SIZE_DEPTH`], where
/// the shader draws `radius` unscaled, at a half-size of [`MARK_HALF`], and
/// grey, so the profile below thresholds on luminance with no palette sample
/// putting notches in it that have nothing to do with the shape.
fn capture_one_mark(shape: f32, points: f32) -> Option<Vec<f32>> {
    capture_sprites(
        &[("shape", shape), ("points", points)],
        &[SwarmInstance {
            center: [0.0, 0.0, PIVOT - SIZE_DEPTH],
            radius: MARK_HALF,
            color: [0.5; 3],
            presence: 1.0,
        }],
    )
}

/// `sprites`, drawn through the real swarm pipeline under `params` onto a clear
/// [`MARK_CAPTURE`]-square target — the linear composite, RGBA, row-major.
///
/// The scene is built with a pool of exactly `sprites.len()`, updated once so
/// its uniforms are the bound ones, and its sprites then replaced by these.
fn capture_sprites(params: &[(&str, f32)], sprites: &[SwarmInstance]) -> Option<Vec<f32>> {
    use crate::dsp::AnalysisFrame;
    use crate::render::context::RenderError;
    use crate::render::{COMPOSITE_FORMAT, HeadlessOptions, Renderer, capture};

    let renderer = match Renderer::new_headless(HeadlessOptions {
        width: MARK_CAPTURE,
        height: MARK_CAPTURE,
        prefer_software: true,
    }) {
        Ok(renderer) => renderer,
        Err(RenderError::RequestAdapter(_)) => {
            eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
            return None;
        }
        Err(e) => panic!("headless renderer build failed: {e}"),
    };
    let device = renderer.ctx.device.clone();
    let queue = renderer.ctx.queue.clone();

    let mut scene = SwarmScene::new(&device, COMPOSITE_FORMAT, sprites.len(), MAX_COC);
    for &(name, value) in params {
        scene.set_param(name, value);
    }
    scene.set_time(0.0);
    scene.update(&AnalysisFrame::default());
    scene.set_target_size(MARK_CAPTURE, MARK_CAPTURE);
    scene.instance_data.copy_from_slice(sprites);

    let (texture, view) =
        capture::create_target(&device, COMPOSITE_FORMAT, MARK_CAPTURE, MARK_CAPTURE);
    let (buffer, padded_bpr) = capture::create_linear_readback(&device, MARK_CAPTURE, MARK_CAPTURE);
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("swarm-one-mark"),
    });
    capture::record_clear(&mut encoder, &view);
    // A square target, so the shader's aspect divide is the identity and a
    // world unit is a normalized-device unit on both axes.
    scene.render(&queue, &mut encoder, &view, 1.0);
    capture::record_copy(
        &mut encoder,
        &texture,
        &buffer,
        padded_bpr,
        MARK_CAPTURE,
        MARK_CAPTURE,
    );
    queue.submit(std::iter::once(encoder.finish()));
    Some(
        capture::read_back_linear(&device, &buffer, MARK_CAPTURE, MARK_CAPTURE, padded_bpr)
            .expect("read back the one-mark composite"),
    )
}

/// The lit radius of a capture, per direction: for each of `rays` angles out
/// of the frame centre, the furthest sample whose luminance still clears a
/// fraction of the frame's brightest.
///
/// Marched along the ray rather than binned by pixel angle, because binning
/// leaves *holes*: at a hundred directions the angular width of a bin is
/// under a pixel of arc at small radii, so a valley direction can contain no
/// pixel centre at all and read as a lit radius of zero. Marching asks each
/// direction directly.
///
/// The falloff is radial along every ray by construction — `d` scales
/// linearly with radius for the disc, polygon and star arms — so this profile
/// is the silhouette's own boundary radius times a constant, and its maxima
/// are the shape's points.
fn lit_radius_profile(pixels: &[f32], rays: usize) -> Vec<f32> {
    let size = MARK_CAPTURE as usize;
    let centre = MARK_CAPTURE as f32 * 0.5;
    let lum = |x: f32, y: f32| -> f32 {
        // The capture is row-major top-to-bottom; the sprite's `local` frame
        // has +y up, so the row index counts down from the centre.
        let col = (centre + x).floor();
        let row = (centre - y).floor();
        if col < 0.0 || row < 0.0 || col >= size as f32 || row >= size as f32 {
            return 0.0;
        }
        let base = (row as usize * size + col as usize) * 4;
        pixels
            .get(base..base + 3)
            .map_or(0.0, |px| px[0] + px[1] + px[2])
    };
    let peak = pixels
        .chunks_exact(4)
        .map(|px| px[0] + px[1] + px[2])
        .fold(0.0f32, f32::max);
    assert!(peak > 0.0, "the one-mark capture is empty");
    // A fifth of the brightest sample: well clear of the half-float floor,
    // and — because `g = (1 - d)^2` — a contour at a fixed fraction of the
    // shape's own radius, so it has the silhouette's outline.
    let threshold = peak * 0.2;

    (0..rays)
        .map(|i| {
            let a = std::f32::consts::TAU * i as f32 / rays as f32;
            let (dx, dy) = (a.cos(), a.sin());
            let steps = (centre * 2.0) as usize;
            let mut furthest = 0.0f32;
            for s in 0..steps {
                let r = s as f32 * 0.5;
                if r > centre - 1.0 {
                    break;
                }
                if lum(dx * r, dy * r) > threshold {
                    furthest = r;
                }
            }
            furthest
        })
        .collect()
}

/// How many separated angular maxima a circular profile has — the point
/// count, counted rather than eyeballed.
///
/// A **Schmitt trigger against the mark's own outer radius**, not a
/// derivative test and not the midpoint of the profile's own range. Both of
/// those count rasterization noise: a disc's profile here spans 62.8 to 64.0
/// px, and the midpoint of *that* range is crossed 88 times by a circle.
/// Here a lobe is an angular run reaching within 20 % of the furthest lit
/// radius, separated from the next by a dip below 70 % of it — so a figure
/// that never dips (a disc, a many-sided polygon) is one lobe, and a
/// seven-pointed star, whose valleys sit at 45 % of its tips, is seven.
fn angular_lobes(profile: &[f32]) -> usize {
    let hi = profile.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let (on, off) = (hi * 0.8, hi * 0.7);
    let n = profile.len();
    // Latched state, walked twice so the wrap-around is settled before the
    // count starts.
    let mut lit = profile.iter().copied().fold(true, |acc, r| {
        if r >= on {
            true
        } else if r <= off {
            false
        } else {
            acc
        }
    });
    if profile.iter().all(|&r| r > on) {
        return 1;
    }
    let mut lobes = 0usize;
    for i in 0..n {
        let r = profile.get(i).copied().unwrap_or(0.0);
        let next = if r >= on {
            true
        } else if r <= off {
            false
        } else {
            lit
        };
        if next && !lit {
            lobes += 1;
        }
        lit = next;
    }
    lobes
}

/// **A seven-pointed star has exactly seven angular maxima** (Plan 0070
/// Phase 1's second done-when), counted off a real capture of the real
/// pipeline rather than asserted by eye.
///
/// Three counts, because one would not separate "the shape is a star" from
/// "the profile is noisy": the same probe at 5, 7 and 9 points must return 5,
/// 7 and 9. And a disc must return **one** — a circle's lit radius is
/// constant, so a lobe count above 1 on it would mean this whole measurement
/// is reading rasterization noise and the star counts prove nothing.
#[test]
fn a_seven_pointed_star_has_seven_angular_maxima() {
    const BINS: usize = 360;
    const STAR: f32 = 3.0;
    const DISC: f32 = 0.0;

    let Some(disc) = capture_one_mark(DISC, 7.0) else {
        return;
    };
    let disc_profile = lit_radius_profile(&disc, BINS);
    let disc_lobes = angular_lobes(&disc_profile);
    let (lo, hi) = (
        disc_profile.iter().copied().fold(f32::INFINITY, f32::min),
        disc_profile
            .iter()
            .copied()
            .fold(f32::NEG_INFINITY, f32::max),
    );
    eprintln!("disc lit radius {lo:.1}..{hi:.1} px over {BINS} bins, {disc_lobes} lobe(s)");
    assert!(
        hi < lo * 1.15,
        "a disc's lit radius must be constant to within rasterization, got \
         {lo:.1}..{hi:.1} px — the profile is measuring something other than \
         the silhouette"
    );
    assert_eq!(
        disc_lobes, 1,
        "a disc has no points; a count above 1 means this measurement reads noise"
    );

    for points in [5.0f32, 7.0, 9.0] {
        let Some(star) = capture_one_mark(STAR, points) else {
            return;
        };
        let profile = lit_radius_profile(&star, BINS);
        let lobes = angular_lobes(&profile);
        let lo = profile.iter().copied().fold(f32::INFINITY, f32::min);
        let hi = profile.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        eprintln!("star points={points}: lit radius {lo:.1}..{hi:.1} px, {lobes} angular maxima");
        assert!(
            hi > lo * 1.5,
            "a star's tips must reach well past its valleys, got {lo:.1}..{hi:.1} px"
        );
        assert_eq!(
            lobes, points as usize,
            "a {points}-pointed star must show exactly {points} angular maxima, \
             counted {lobes}"
        );
    }
}

/// **An eased `points` renders only whole figures — never a partial lobe**
/// (Plan 0070 Phase 3's done-when), asserted on the pixels rather than on the
/// quantizer.
///
/// `marks::mark_points` already pins the arithmetic. This pins the
/// *behaviour* the arithmetic exists for, because the two are separable: a
/// count could be rounded on the way into the uniform and still reach an
/// angular fold fractionally if some later hand re-derived it. So a sweep
/// from 7 to 9 — the fractional values an ease actually visits — is rendered,
/// and the frames are grouped by **exact** equality.
///
/// The claim, stated as the test checks it: the seven captures fall into
/// exactly **three** groups; the group boundaries sit at the half-integers
/// (7.4 draws the same frame as 7.0, 7.6 the same as 8.0); and the three
/// groups have 7, 8 and 9 angular maxima. No frame between them exists to be
/// found.
///
/// The first pair is the determinism control: the same request captured twice
/// must produce the same bytes, or grouping by equality would be measuring
/// the adapter.
#[test]
fn an_eased_points_sweep_renders_only_whole_figures() {
    const BINS: usize = 360;
    const STAR: f32 = 3.0;
    /// The sweep, straddling both half-integer steps between 7 and 9.
    const SWEEP: [f32; 7] = [7.0, 7.4, 7.6, 8.0, 8.4, 8.6, 9.0];

    let Some(control_a) = capture_one_mark(STAR, 7.0) else {
        return;
    };
    let Some(control_b) = capture_one_mark(STAR, 7.0) else {
        return;
    };
    assert_eq!(
        control_a, control_b,
        "the same request must render the same bytes, or grouping frames by \
         equality measures the adapter rather than the point count"
    );

    let mut frames = Vec::new();
    for points in SWEEP {
        let Some(frame) = capture_one_mark(STAR, points) else {
            return;
        };
        frames.push((points, frame));
    }

    // Group by exact frame equality, keeping first-seen order.
    let mut groups: Vec<(Vec<f32>, Vec<f32>)> = Vec::new();
    for (points, frame) in &frames {
        match groups.iter_mut().find(|(pixels, _)| pixels == frame) {
            Some((_, members)) => members.push(*points),
            None => groups.push((frame.clone(), vec![*points])),
        }
    }
    let members: Vec<Vec<f32>> = groups.iter().map(|(_, m)| m.clone()).collect();
    let lobes: Vec<usize> = groups
        .iter()
        .map(|(pixels, _)| angular_lobes(&lit_radius_profile(pixels, BINS)))
        .collect();
    eprintln!("points sweep {SWEEP:?} -> groups {members:?}, angular maxima {lobes:?}");

    assert_eq!(
        members,
        vec![vec![7.0, 7.4], vec![7.6, 8.0, 8.4], vec![8.6, 9.0]],
        "an eased 7 -> 9 sweep must render exactly three figures, switching at \
         the half-integers"
    );
    assert_eq!(
        lobes,
        vec![7, 8, 9],
        "the three figures must be the 7-, 8- and 9-pointed stars"
    );
}

/// **The default mark is byte-identical to the one this scene drew before it
/// had a shape at all** (Plan 0070 Phase 1's first done-when), end to end
/// through the preset path.
///
/// Exact equality, not a tolerance: the `disc` arm is `length(p)` — the same
/// expression the fragment shader held — so binding `shape = "0"` and binding
/// nothing must produce the same bytes on the same adapter in the same run.
/// That is the property every untouched golden baseline rests on, asserted
/// here rather than inferred from the goldens passing.
///
/// The third capture is the non-vacuity arm: a star through the same path
/// must genuinely move the frame, or the first assertion would also pass on a
/// `shape` binding that reached nothing.
#[test]
fn a_disc_shaped_swarm_is_byte_identical_to_the_unshaped_one() {
    use crate::dsp::AnalysisFrame;
    use crate::preset::Preset;
    use crate::render::context::RenderError;
    use crate::render::{HeadlessOptions, Renderer};

    const SIZE: u32 = 128;
    const FRAMES: u32 = 30;

    let mut renderer = match Renderer::new_headless(HeadlessOptions {
        width: SIZE,
        height: SIZE,
        prefer_software: true,
    }) {
        Ok(renderer) => renderer,
        Err(RenderError::RequestAdapter(_)) => {
            eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
            return;
        }
        Err(e) => panic!("headless renderer build failed: {e}"),
    };
    let frame = AnalysisFrame::default();
    let mut capture = |name: &str, extra: &str| {
        let toml = format!(
            "system = \"swarm\"\nname = \"{name}\"\n[params]\nforce = \"0.8\"\n\
             spin = \"0.1\"\nbrightness = \"0.9\"\nsize = \"3.0\"\n{extra}"
        );
        let preset = Preset::from_toml_str(&toml).expect("the probe preset parses");
        renderer.set_presets(vec![preset]);
        renderer
            .capture_preset(name, &frame, FRAMES)
            .expect("capture the probe preset")
    };

    let unshaped = capture("unshaped", "");
    let disc = capture("disc", "shape = \"0\"\npoints = \"7\"\n");
    let star = capture("star", "shape = \"3\"\npoints = \"7\"\n");

    assert_eq!(
        unshaped.rgba, disc.rgba,
        "an explicit `shape = disc` must render byte-identically to no shape \
         binding at all — the disc arm is the length() it replaced"
    );
    let differing = star
        .rgba
        .chunks_exact(4)
        .zip(disc.rgba.chunks_exact(4))
        .filter(|(a, b)| a[..3] != b[..3])
        .count();
    eprintln!(
        "shaped swarm: {differing} of {} pixels differ between disc and star",
        (SIZE * SIZE) as usize
    );
    assert!(
        differing * 50 > (SIZE * SIZE) as usize,
        "a star must genuinely move the frame, or the equality above is a \
         statement about a binding that reached nothing: {differing} pixels"
    );
}

// -----------------------------------------------------------------------
// Depth of field on the sprites (Plan 0239 Phase 2, ADR-0257)
// -----------------------------------------------------------------------

/// A sprite at view depth `depth`, `x` normalized-device units across a square
/// target, whose **sharp** size on screen is `half` normalized-device units at
/// any depth: its radius is scaled up by the depth so perspective cancels, and
/// the only thing left to differ between two of them is their blur.
fn sprite_at(depth: f32, x: f32, half: f32) -> SwarmInstance {
    SwarmInstance {
        center: [x * depth * (0.5 * REST_FOV).tan(), 0.0, PIVOT - depth],
        radius: half * depth / SIZE_DEPTH,
        color: [0.5; 3],
        presence: 1.0,
    }
}

/// The lit-pixel count and the peak luminance of one half of a square linear
/// capture: the left half when `left`, else the right.
fn half_stats(pixels: &[f32], left: bool) -> (usize, f32) {
    let size = MARK_CAPTURE as usize;
    let (mut lit, mut peak) = (0usize, 0.0f32);
    for (i, px) in pixels.chunks_exact(4).enumerate() {
        if (i % size < size / 2) != left {
            continue;
        }
        let lum = px[0] + px[1] + px[2];
        peak = peak.max(lum);
        if lum > 1e-3 {
            lit += 1;
        }
    }
    (lit, peak)
}

/// **`aperture = 0` draws exactly the sharp sprites**: binding it, and moving
/// `focus` while it is 0, renders the same bytes as leaving both unbound. At
/// `coc = 0` the shader's growth and area factors are exactly 1, so this is
/// equality, not a tolerance.
#[test]
fn a_zero_aperture_renders_the_sharp_swarm_byte_for_byte() {
    use crate::dsp::AnalysisFrame;
    use crate::preset::Preset;
    use crate::render::context::RenderError;
    use crate::render::{HeadlessOptions, Renderer};

    const FIXTURE: &str = include_str!("../../../../tests/fixtures/swarm.toml");
    let mut renderer = match Renderer::new_headless(HeadlessOptions {
        width: 160,
        height: 100,
        prefer_software: true,
    }) {
        Ok(renderer) => renderer,
        Err(RenderError::RequestAdapter(_)) => {
            eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
            return;
        }
        Err(e) => panic!("headless renderer build failed: {e}"),
    };
    let frame = AnalysisFrame::default();
    let mut capture = |extra: &str| {
        let preset = Preset::from_toml_str(&format!("{FIXTURE}{extra}"))
            .expect("the swarm fixture parses with overrides");
        let name = preset.name.clone();
        renderer.set_presets(vec![preset]);
        renderer
            .capture_preset(&name, &frame, 60)
            .expect("capture the swarm fixture")
    };
    let unbound = capture("");
    let zero = capture("aperture = \"0\"\nfocus = \"0.1\"\n");
    let blurred = capture("aperture = \"10\"\nfocus = \"0.1\"\n");
    assert_eq!(
        unbound.rgba, zero.rgba,
        "aperture 0 must draw the sharp swarm whatever the focus"
    );
    let differing = blurred
        .rgba
        .chunks_exact(4)
        .zip(unbound.rgba.chunks_exact(4))
        .filter(|(a, b)| a[..3] != b[..3])
        .count();
    eprintln!("aperture 10: {differing} pixels differ from the sharp swarm");
    assert!(
        differing * 10 > unbound.rgba.len() / 4,
        "a bound aperture must reach the sprites: {differing} pixels differ"
    );
}

/// **A sprite's circle of confusion never exceeds the tier's
/// `swarm_max_coc_px`, at any aperture** (Plan 0239 Phase 3), on either tier,
/// at every depth across the slab and every focus; and an aperture past the cap
/// is announced as a blur clamp. The swarm's cap never sits above the shared
/// one.
#[test]
fn the_circle_of_confusion_never_exceeds_the_swarm_cap() {
    use crate::render::TierConfig;
    use crate::render::camera::Lens;

    let Some((_renderer, mut scene)) = scene(1) else {
        return;
    };
    for tier in [TierConfig::FLOOR, TierConfig::RICH] {
        assert!(
            tier.swarm_max_coc_px <= tier.max_coc_px,
            "{:?}: the swarm's cap must not exceed the shared one",
            tier.tier
        );
        let cap = tier.swarm_max_coc_px as f32;
        scene.max_coc = cap;
        let mut worst = 0.0f32;
        for aperture in [0.0, 1.0, cap, cap + 0.5, 40.0, 1e6, f32::INFINITY, f32::NAN] {
            for focus in [0.0, 0.5, 1.0] {
                scene.set_param("aperture", aperture);
                scene.set_param("focus", focus);
                let frame = scene.camera_frame(16.0 / 9.0, scene.max_coc);
                let [a, focal, max_coc, _] = frame.uniform.lens;
                let lens = Lens::new(a, focal, max_coc);
                for step in 0..=48 {
                    let depth = Z_NEAR + Z_SPAN * step as f32 / 48.0;
                    let coc = lens.coc(depth);
                    worst = worst.max(coc);
                    assert!(
                        coc <= cap,
                        "{:?}: coc {coc} at depth {depth}, aperture {aperture}, focus {focus} \
                         exceeds the cap {cap}",
                        tier.tier
                    );
                }
                assert_eq!(
                    frame.blur.is_some(),
                    aperture.is_finite() && aperture > cap,
                    "{:?}: an aperture of {aperture} against the cap {cap} must be announced \
                     exactly when it is past it",
                    tier.tier
                );
            }
        }
        assert_eq!(worst, cap, "{:?}: the sweep must reach the cap", tier.tier);
    }
}

/// **A sprite far from the focal depth covers more pixels and has a lower
/// peak than one at it, in one rendered frame** (Plan 0239 Phase 2): two
/// sprites of the same sharp size on screen and the same light, one at the far
/// slab bound with the focal plane on it, one at the near bound.
#[test]
fn a_defocused_sprite_spreads_wider_and_dimmer_than_a_focused_one() {
    const HALF: f32 = 0.05;
    let Some(pixels) = capture_sprites(
        &[("aperture", 24.0), ("focus", 1.0)],
        &[
            // Left: at the focal plane.
            sprite_at(Z_FAR, -0.5, HALF),
            // Right: at the near bound, as far from focus as the slab allows.
            sprite_at(Z_NEAR, 0.5, HALF),
        ],
    ) else {
        return;
    };
    let (sharp_lit, sharp_peak) = half_stats(&pixels, true);
    let (soft_lit, soft_peak) = half_stats(&pixels, false);
    eprintln!(
        "focused: {sharp_lit} px, peak {sharp_peak:.4}; defocused: {soft_lit} px, peak {soft_peak:.4}"
    );
    assert!(
        sharp_lit > 20,
        "the focused sprite must draw: {sharp_lit} px"
    );
    assert!(
        soft_lit > sharp_lit * 2,
        "the defocused sprite must cover more pixels: {soft_lit} vs {sharp_lit}"
    );
    assert!(
        soft_peak < sharp_peak * 0.7,
        "the defocused sprite must peak lower: {soft_peak:.4} vs {sharp_peak:.4}"
    );
}

/// **A `shape = "2"` sprite at the focal plane keeps its polygon edge**
/// (Plan 0239 Phase 2): with a wide aperture bound, a triangle drawn at the
/// focal depth still shows three angular maxima and matches the aperture-0
/// frame to within rounding, while the same triangle at the near bound softens.
#[test]
fn a_polygon_at_the_focal_plane_keeps_its_edge() {
    const BINS: usize = 360;
    let triangle = |aperture: f32, depth: f32| {
        capture_sprites(
            &[
                ("shape", 2.0),
                ("points", 3.0),
                ("aperture", aperture),
                ("focus", (depth - Z_NEAR) / Z_SPAN),
            ],
            &[sprite_at(depth, 0.0, 0.6)],
        )
    };
    let Some(sharp) = triangle(0.0, PIVOT) else {
        return;
    };
    let Some(focused) = triangle(24.0, PIVOT) else {
        return;
    };
    let worst = sharp
        .iter()
        .zip(&focused)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0f32, f32::max);
    let lobes = angular_lobes(&lit_radius_profile(&focused, BINS));
    eprintln!(
        "triangle at the focal plane: {lobes} angular maxima, worst |diff| vs aperture 0 {worst:.5}"
    );
    assert_eq!(lobes, 3, "a triangle in focus must keep its three corners");
    assert!(
        worst < 1e-3,
        "a sprite at the focal plane must draw as the sharp one: worst {worst:.5}"
    );

    // Non-vacuity: the same aperture does blur a triangle away from focus.
    let Some(soft) = capture_sprites(
        &[
            ("shape", 2.0),
            ("points", 3.0),
            ("aperture", 24.0),
            ("focus", 1.0),
        ],
        &[sprite_at(PIVOT, 0.0, 0.6)],
    ) else {
        return;
    };
    let moved = soft
        .iter()
        .zip(&sharp)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0f32, f32::max);
    assert!(
        moved > 0.01,
        "the triangle away from focus must soften: worst |diff| {moved:.5}"
    );
}

// -----------------------------------------------------------------------
// The sprite seam does not punch holes in the backdrop (Plan 0051 Phase 1)
// -----------------------------------------------------------------------

/// The lit-backdrop fixture this guard captures three ways. Its `bg_bright`
/// and `size` lines are **stripped and rewritten** per capture — one scene at
/// three configurations — so the numbers are read back out of the file rather
/// than restated here, and editing the fixture moves the test with it.
const LIT_FIXTURE: &str = include_str!("../../../../tests/fixtures/swarm_lit_backdrop.toml");

/// The square capture size. Modest, because this reads back three whole float
/// frames; and an exact multiple of the post chain's 256 px grid step, so the
/// trails stage runs at the target size and its present is a 1:1 sample rather
/// than a resample that would blur the property being asserted.
const CAPTURE_SIZE: u32 = 256;

/// Frames per capture. `force`/`spin`/`burst` are all 0 in the fixture, so
/// this is long enough for the seeded initial velocities to damp out and the
/// trail history to settle onto a static field.
const CAPTURE_FRAMES: u32 = 40;

/// A backdrop channel this bright counts as *present* for the non-vacuity arm
/// below — well above the half-precision floor, well below the fixture's own
/// `bg_bright`.
const BACKDROP_PRESENT: f32 = 0.05;

/// The value of a top-level `key = "<number>"` line in [`LIT_FIXTURE`], or
/// `NaN` when it is absent. Used so the fixture stays the single statement of
/// what this test captures.
fn fixture_value(key: &str) -> f32 {
    LIT_FIXTURE
        .lines()
        .find_map(|line| {
            let rest = line.trim_start().strip_prefix(key)?;
            let rest = rest.trim_start().strip_prefix('=')?;
            rest.trim().trim_matches('"').parse::<f32>().ok()
        })
        .unwrap_or(f32::NAN)
}

/// Slack for half-precision rounding, the same shape `bloom.rs`'s guard uses.
/// The composite is `Rgba16Float`, so a value of magnitude `m` is stored to
/// roughly `m / 1024`, and the lit capture quantizes a different sum than the
/// backdrop-only one does.
///
/// It is slack, not a tolerance: the property below is **exact** in real
/// arithmetic. Upstream of the tonemap the composite is a plain premultiplied
/// OVER, so where the scene wrote nothing the backdrop must arrive unchanged.
/// Measured on this fixture, the fixed shader's worst `|L - B|` is **0.0002**
/// and the pre-fix one's is **0.3467** — the backdrop's own brightness,
/// discarded outright — across 9594 channels. This sits ~1700x below the
/// defect and ~20x above the noise.
fn half_slack(value: f32) -> f32 {
    (4.0 / 1024.0) * value.abs().max(1.0)
}

/// **Where the swarm drew no light, the backdrop arrives intact** — the guard
/// the scene→chain seam shipped without.
///
/// Returning `vec4(in.color * g, 1.0)` from `fs_main` puts the radial
/// falloff in colour and a literal constant in alpha. With the alpha blend
/// at `BlendComponent::OVER` and a source alpha of exactly 1, destination
/// alpha saturates to 1 across every sprite's **square** quad — including
/// the four corners outside the inscribed disc, about 21 % of each sprite,
/// where the shader wrote nothing at all. The chain's resolve computes
/// `src.rgb + backdrop * (1 - src.a)` (ADR-0055), so those corners discarded
/// the backdrop and rendered as black rectangular notches, dozens per frame.
/// See `gpu::ADDITIVE_LIGHT_SATURATING_COVERAGE`.
///
/// # Why this reads the linear composite and not the capture
///
/// Same reason `bloom.rs`'s guard does: the capture's bytes are downstream of
/// the tonemap, which scales all three channels off the brightest one
/// (ADR-0046), so adding a backdrop under a stroke changes every channel by
/// design and no byte-level tolerance separates that from the defect.
/// Upstream of the tonemap there is no confound — it is a plain premultiplied
/// OVER — so the bound is **0** rather than a tolerance. That readback is
/// `pub(crate)`, which is why this test lives here and not in `core/tests/`.
///
/// # Why it needed writing at all
///
/// Every swarm fixture and every golden baseline runs `bg_bright = 0`, where
/// a black backdrop times any alpha is still black. The whole regression
/// suite was blind to this by construction, and so was the contact sheet.
/// That is verbatim the blind spot ADR-0055's first Negative bullet names —
/// the third instance of it, after the fold (Plan 0045 Phase 2b) and the
/// bloom recombine (Phase 4b), each of which got a guard of this shape.
#[test]
fn a_lit_backdrop_survives_where_the_swarm_drew_nothing() {
    use crate::dsp::AnalysisFrame;
    use crate::preset::Preset;
    use crate::render::capture;
    use crate::render::context::RenderError;
    use crate::render::{HeadlessOptions, Renderer};

    // --- Non-vacuity, before any GPU work: the fixture must still describe
    // the configuration this guard exists for. ---
    let backdrop = fixture_value("bg_bright");
    let sprite = fixture_value("size");
    let trails = fixture_value("trails");
    assert!(
        backdrop > 0.0,
        "swarm_lit_backdrop.toml no longer ships a lit backdrop (bg_bright = \
         {backdrop}); on black this whole comparison is black against black"
    );
    assert!(
        sprite > 0.0,
        "swarm_lit_backdrop.toml no longer draws sprites (size = {sprite})"
    );
    assert!(
        trails > 0.0,
        "swarm_lit_backdrop.toml no longer binds `trails` (= {trails}), so no \
         post stage is active. With an empty chain the scene draws straight \
         onto the backdrop and its additive colour cannot remove light — the \
         defect is unrepresentable and this test proves nothing"
    );

    /// The linear composite the tonemap is about to map, at a given backdrop
    /// brightness and sprite size.
    ///
    /// Builds and drops **one** renderer per call rather than holding three:
    /// a second live device in a binary is what the software adapter falls
    /// over on, and building GPU resources mid-run shifts what the trails
    /// stage resolves to on WARP (a WARP reading, unverified on lavapipe as of
    /// 2026-10-06).
    fn linear_composite(bg_bright: f32, size: f32, brightness: Option<f32>) -> Option<Vec<f32>> {
        let mut renderer = match Renderer::new_headless(HeadlessOptions {
            width: CAPTURE_SIZE,
            height: CAPTURE_SIZE,
            prefer_software: true,
        }) {
            Ok(renderer) => renderer,
            Err(RenderError::RequestAdapter(_)) => {
                eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
                return None;
            }
            Err(e) => panic!("headless renderer build failed: {e}"),
        };
        // All three keys live in `[params]`, which is the fixture's last table,
        // so stripping them and appending the overrides keeps them in it.
        let base: String = LIT_FIXTURE
            .lines()
            .filter(|line| {
                let line = line.trim_start();
                !line.starts_with("bg_bright")
                    && !line.starts_with("size")
                    && !line.starts_with("brightness")
            })
            .collect::<Vec<_>>()
            .join("\n");
        let mut toml = format!("{base}\nbg_bright = \"{bg_bright}\"\nsize = \"{size}\"\n");
        if let Some(brightness) = brightness {
            toml.push_str(&format!("brightness = \"{brightness}\"\n"));
        }
        let preset = Preset::from_toml_str(&toml)
            .expect("the lit-backdrop swarm fixture parses with overrides");
        let name = preset.name.clone();
        renderer.set_presets(vec![preset]);

        // Every binding is a constant, so the analysis frame only has to be
        // well-formed — the swarm's `update` ignores it entirely.
        let frame = AnalysisFrame::default();
        renderer
            .capture_preset(&name, &frame, CAPTURE_FRAMES)
            .expect("capture the lit-backdrop swarm fixture");

        let device = renderer.ctx.device.clone();
        let queue = renderer.ctx.queue.clone();
        let src = renderer
            .tonemap
            .src_texture()
            .expect("the tonemap built its input while capturing")
            .clone();
        let (buffer, padded_bpr) =
            capture::create_linear_readback(&device, CAPTURE_SIZE, CAPTURE_SIZE);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("swarm-backdrop-readback"),
        });
        capture::record_copy(
            &mut encoder,
            &src,
            &buffer,
            padded_bpr,
            CAPTURE_SIZE,
            CAPTURE_SIZE,
        );
        queue.submit(std::iter::once(encoder.finish()));
        Some(
            capture::read_back_linear(&device, &buffer, CAPTURE_SIZE, CAPTURE_SIZE, padded_bpr)
                .expect("read back the linear composite"),
        )
    }

    // `L`: the frame as shipped. `K`: the same scene over a black backdrop,
    // which is what "the scene wrote no light here" is read off. `B`: the
    // backdrop with the scene contributing nothing — zero-area sprite quads
    // rasterize no fragments, so the chain resolves fully transparent and
    // this is the backdrop alone, through the same pipeline as `L`.
    let Some(lit) = linear_composite(backdrop, sprite, None) else {
        return;
    };
    let Some(dark) = linear_composite(0.0, sprite, None) else {
        return;
    };
    let Some(backdrop_only) = linear_composite(backdrop, 0.0, None) else {
        return;
    };
    // `U`: the fourth capture (Plan 0053 Phase 4), the same one the line guard
    // takes. At `brightness = 0` the per-particle colour is zero, so `in.color *
    // g` is zero everywhere and the frame is exactly `backdrop * (1 - a)` — a
    // direct readout of alpha, which is what this guard is actually about.
    let Some(unlit_sprites) = linear_composite(backdrop, sprite, Some(0.0)) else {
        return;
    };
    assert_eq!(dark.len(), lit.len(), "the captures differ in size");
    assert_eq!(
        dark.len(),
        backdrop_only.len(),
        "the captures differ in size"
    );

    let total = dark.len() / 4;
    let (mut untouched, mut drawn, mut over_backdrop) = (0usize, 0usize, 0usize);
    let (mut violations, mut worst) = (0usize, 0.0f32);
    for (pixel, texel) in dark.chunks_exact(4).enumerate() {
        if texel[0] != 0.0 || texel[1] != 0.0 || texel[2] != 0.0 {
            drawn += 1;
            continue; // the scene put light here; the property says nothing
        }
        untouched += 1;
        let base = pixel * 4;
        if backdrop_only[base..base + 3]
            .iter()
            .any(|&c| c > BACKDROP_PRESENT)
        {
            over_backdrop += 1;
        }
        for channel in 0..3 {
            let l = lit[base + channel];
            let b = backdrop_only[base + channel];
            let diff = (l - b).abs();
            if diff > worst {
                worst = diff;
            }
            if diff > half_slack(b) {
                violations += 1;
            }
        }
    }
    eprintln!(
        "swarm lit backdrop at {CAPTURE_SIZE}x{CAPTURE_SIZE}: {untouched} of \
         {total} pixels untouched by the scene ({over_backdrop} of those over \
         a lit backdrop), {drawn} lit by it; worst |L - B| {worst:.4}"
    );

    // --- Non-vacuity: the region the property speaks about is a substantial
    // part of the frame, the scene genuinely drew into the rest, and the
    // backdrop genuinely reached the frame underneath. A fixture edit that
    // quietly empties any of the three shows up here rather than passing. ---
    assert!(
        untouched * 4 > total,
        "only {untouched} of {total} pixels are untouched by the scene — the \
         fixture has filled the frame and the property covers almost nothing"
    );
    assert!(
        drawn * 20 > total,
        "only {drawn} of {total} pixels carry any scene light — the fixture \
         has stopped drawing, so the sprite corners this guards are not in \
         the frame"
    );
    assert!(
        over_backdrop * 2 > untouched,
        "only {over_backdrop} of the {untouched} untouched pixels sit over a \
         backdrop brighter than {BACKDROP_PRESENT} — comparing black against \
         black, which any alpha would pass"
    );

    // --- The second, wider property (Plan 0053 Phase 4). ---
    //
    // The swarm's exact arm is not the thin one — its zero-colour region is
    // four hard-edged corners per sprite and the revert moves 9 594 channels —
    // so this arm costs one more call to a harness that already exists and buys
    // the *same* property on both seams rather than two different ones. Where
    // the sprites emit nothing the frame is `backdrop * (1 - a)`, so a fully
    // extinguished pixel is one where alpha reached 1.
    //
    // Measured before the assertions, like the line guard's, so a failing run
    // prints both counts instead of short-circuiting on the first.
    let extinguished: usize = (0..total)
        .filter(|&pixel| {
            let base = pixel * 4;
            backdrop_only[base..base + 3]
                .iter()
                .any(|&c| c > BACKDROP_PRESENT)
                && (0..3).all(|c| unlit_sprites[base + c] <= backdrop_only[base + c] * 0.02)
        })
        .count();
    eprintln!(
        "swarm brightness = 0: {extinguished} of {drawn} footprint pixels fully \
         extinguished ({:.2} %); the exact arm moved {violations} channels",
        extinguished as f32 / drawn.max(1) as f32 * 100.0
    );

    // --- The first property, unchanged and still exact. ---
    assert_eq!(
        violations, 0,
        "{violations} channels differ between the lit frame and the backdrop \
         alone at pixels where the scene wrote NO light (worst {worst:.4}). \
         Upstream of the tonemap this is a plain premultiplied OVER, so where \
         nothing was drawn the backdrop must arrive intact — a difference \
         here is a sprite emitting coverage it does not have, holding the \
         backdrop out of pixels it never painted"
    );

    // Non-vacuity: the fourth capture has to be an alpha readout. If
    // `brightness = 0` stopped zeroing the emitted colour, this would be
    // measuring something else.
    let sprite_light: usize = (0..total)
        .filter(|&pixel| {
            let base = pixel * 4;
            let drew = dark[base] != 0.0 || dark[base + 1] != 0.0 || dark[base + 2] != 0.0;
            drew && (0..3).any(|c| unlit_sprites[base + c] > backdrop_only[base + c])
        })
        .count();
    assert_eq!(
        sprite_light, 0,
        "{sprite_light} pixels are BRIGHTER than the backdrop alone in the \
         `brightness = 0` capture, so the sprites are still emitting light \
         there — that capture's whole purpose is to make the frame \
         `backdrop * (1 - a)` and nothing else"
    );

    // A tenth of the footprint, the same ceiling the line guard uses. Measured
    // on this fixture: the fixed shader gives **1 of 12 880 (0.01 %)** and the
    // pre-fix one gives **16 052 (124.63 %)**. Over 100 % is expected here and
    // is the defect's own signature — the footprint is counted from where the
    // sprites put *colour*, and the corners outside the inscribed disc (~21 % of
    // every quad) are exactly the region that draws nothing and, pre-fix,
    // extinguished the backdrop anyway.
    assert!(
        extinguished * 10 < drawn,
        "{extinguished} of the sprites' {drawn} footprint pixels are fully \
         extinguished with `brightness = 0` — the frame there is \
         `backdrop * (1 - a)`, so that is alpha at 1 across the sprite quads \
         rather than only at their centres. A premultiplied sprite carries its \
         coverage in alpha (ADR-0056); a constant alpha 1 punches the backdrop \
         out of every quad's corners. The exact arm moved {violations} channels \
         on the same run"
    );
}

/// The field clock integrates at `spin` rather than multiplying the shared clock
/// (ADR-0135), and at a constant rate the two agree — which is why the two
/// `swarm` golden fixtures, both of which bind a constant, do not move.
#[test]
fn a_constant_spin_integrates_to_the_multiply_it_replaced() {
    let dt = FALLBACK_DT;
    for rate in [DEFAULT_SPIN, 0.0, 0.1, 1.15] {
        let mut phase = Phase::default();
        let mut time = 0.0f32;
        for _ in 0..600 {
            phase.step(rate, dt);
            time += dt;
        }
        assert!(
            (phase.get() - rate * time).abs() < 1e-3,
            "rate {rate}: integrated {} against the multiply's {}",
            phase.get(),
            rate * time
        );
    }
}

/// ...and the property the multiply failed, which on this scene is the whole
/// point of the correction: a world binding `spin` to `mid` through a
/// `tau = 0.3` one-pole moves the rate ~0.04 in a frame across a 0.75 swing. Integrated, the field clock advances one frame's worth whatever the
/// elapsed time; multiplied, it advanced ~4 s at t = 100 s against a nominal
/// 0.019 s and the flow re-rolled on every loud passage.
#[test]
fn a_spin_change_bends_the_field_clock_instead_of_re_rolling_it() {
    let dt = FALLBACK_DT;
    let mut phase = Phase::default();
    let mut time = 0.0f32;
    for _ in 0..6_000 {
        phase.step(1.15, dt);
        time += dt;
    }
    assert!(time > 99.0, "the fixture must be far from t = 0: {time}");

    let before = phase.get();
    phase.step(1.19, dt);
    let step = phase.get() - before;
    assert!(
        (step - 1.19 * dt).abs() < 1e-4,
        "the field clock advanced {step}, not {}",
        1.19 * dt
    );
    // The size of the defect, computed rather than described: what `time * spin`
    // would have delivered for the same 0.04 swing, against one nominal frame.
    let multiplied_jump = 0.04 * time;
    let nominal = 1.19 * dt;
    assert!(
        multiplied_jump > 100.0 * nominal,
        "the multiply's one-frame jump was {multiplied_jump} s against a nominal {nominal} s"
    );
}
