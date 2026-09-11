//! The analytic field's CPU-side contracts: the family roster, the family table
//! the generated reference prints, the quantizers that stand between a bound
//! value and the shader, and the escape-time numerics read through
//! [`mirror`](super::mirror).
// Test asserts panic freely; this is not the render path.
#![allow(
    clippy::panic,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used
)]

use super::mirror::{self, Escape, ITERATIONS_PER_PALETTE};
use super::*;

/// The pass's WGSL — vertex prelude and all — parses and validates through
/// naga, the frontend wgpu hands it to. GPU-free, so a shader error fails here
/// with its own diagnostic rather than as an invalid readback buffer three
/// calls downstream on a runner that has an adapter.
#[test]
fn the_shader_is_valid_wgsl() {
    let source = format!(
        "{}{}",
        crate::render::gpu::FULLSCREEN_VS_NDC,
        shader::SHADER
    );
    if let Err(e) = crate::milk::shader::validate_wgsl(&source) {
        panic!("the analytic field's shader does not validate:\n{e}");
    }
}

/// The pipeline builds on the adapter without a validation error — the half
/// naga cannot vouch for, since the backend recompiles the shader into its own
/// language and a construct naga accepts can still fail there. Inside an error
/// scope, so the failure is the backend's own message rather than a poisoned
/// resource surfacing later.
#[test]
fn the_pipeline_builds_on_the_adapter() {
    use crate::render::context::{RenderContext, RenderError};
    let ctx = match RenderContext::new_headless(64, 64, true) {
        Ok(ctx) => ctx,
        Err(RenderError::RequestAdapter(_)) => {
            eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
            return;
        }
        Err(e) => panic!("headless context build failed: {e}"),
    };
    let scope = ctx.device.push_error_scope(wgpu::ErrorFilter::Validation);
    let scene = AnalyticFieldScene::new(&ctx.device, crate::render::COMPOSITE_FORMAT);
    if let Some(error) = pollster::block_on(scope.pop()) {
        panic!("building the analytic field raised: {error}");
    }
    drop(scene);
}

/// Every family round-trips through its name, and the shader index is its
/// position in the roster — the two are numbered alike by construction only if
/// this holds.
#[test]
fn every_family_round_trips_and_indexes_by_roster_position() {
    for (position, family) in FieldFamily::ALL.into_iter().enumerate() {
        assert_eq!(FieldFamily::from_name(family.as_str()), Some(family));
        assert_eq!(family.index() as usize, position, "{family:?}");
    }
    assert_eq!(FieldFamily::from_name("Chladni"), None, "names are exact");
    assert_eq!(FieldFamily::from_name(""), None);
    assert_eq!(FieldConfig::default().family, FieldFamily::Chladni);
}

/// [`FAMILY_PARAMS`] is a statement about the engine, so it is held to it: each
/// row names a declared parameter once, lists every family by the name a preset
/// uses and in roster order, and carries a spec range some family reads.
#[test]
fn the_family_table_is_the_roster() {
    let families: Vec<&str> = FieldFamily::ALL.iter().map(|f| f.as_str()).collect();
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
        crate::render::scenes::family_params("analytic_field"),
        FAMILY_PARAMS,
        "the reference must reach this table under the system's own label"
    );
}

/// A mode number reaches the shader whole and inside the plate's range, and a
/// non-finite one never reaches it at all.
#[test]
fn a_bound_mode_is_clamped_and_rounded_before_the_shader() {
    for (value, applied) in [
        (3.0, 3.0),
        (3.4, 3.0),
        (4.6, 5.0),
        (0.0, 1.0),
        (-7.0, 1.0),
        (99.0, MAX_MODE),
    ] {
        assert_eq!(applied_mode(value, 3.0), applied, "{value}");
    }
    assert_eq!(applied_mode(f32::NAN, 3.0), 3.0);
    assert_eq!(applied_mode(f32::INFINITY, 5.0), 5.0);
    // And the Structural quantizer upstream composes with it to the identity,
    // which is the audit rule a `Structural` mark is held to (ADR-0180 Outcome).
    for i in -40..400 {
        let value = i as f32 * 0.07;
        let quantized = ParamKind::Structural.quantize(value);
        assert_eq!(
            applied_mode(quantized, 3.0),
            applied_mode(value, 3.0),
            "rounding upstream moved the plate at {value}"
        );
    }
}

/// The iteration budget reaches the shader whole, inside `1..=cap`, and a
/// non-finite one never reaches it — and the Structural rounding upstream
/// composes with the scene's to the identity.
#[test]
fn a_bound_iteration_budget_is_clamped_to_its_cap_and_rounded() {
    for (value, cap, applied) in [
        (64.0, MAX_ITERATIONS, 64.0),
        (63.6, MAX_ITERATIONS, 64.0),
        (0.0, MAX_ITERATIONS, 1.0),
        (9000.0, MAX_ITERATIONS, MAX_ITERATIONS),
        (300.0, 96.0, 96.0),
        (12.0, 96.0, 12.0),
        // A cap past the declared top is held to it: the shader's loop bound
        // is never larger than the range says.
        (9000.0, 1e9, MAX_ITERATIONS),
    ] {
        assert_eq!(applied_iterations(value, cap), applied, "{value} at {cap}");
    }
    assert_eq!(applied_iterations(f32::NAN, 32.0), 32.0);
    assert_eq!(
        applied_iterations(f32::NAN, MAX_ITERATIONS),
        DEFAULT_ITERATIONS
    );
    for i in 0..2000 {
        let value = i as f32 * 0.37;
        assert_eq!(
            applied_iterations(ParamKind::Structural.quantize(value), 96.0),
            applied_iterations(value, 96.0),
            "rounding upstream moved the budget at {value}"
        );
    }
}

/// The Julia constant that the tests call *inside* the Mandelbrot set: the
/// main cardioid's `mu / 2 - mu^2 / 4` at `mu = 0.7 e^(2i)`, well inside
/// `|mu| < 1`, so its filled Julia set is a single quasi-disk.
const C_INSIDE: [f32; 2] = [-0.066, 0.411];

/// A packed escape-time uniform at the defaults the tests vary from.
fn julia(c: [f32; 2]) -> Escape {
    Escape {
        c,
        iterations: 64.0,
        radius: 16.0,
        power: 2.0,
        interior: 0.0,
        mandelbrot: false,
    }
    .packed()
}

/// **No pixel is non-finite anywhere in the declared ranges** — nor at the
/// binding extremes past them, which the scene's clamps are there to absorb.
///
/// Read through the mirror, because a capture cannot show a NaN: the 8-bit
/// write turns it into a colour. What makes the mirror's verdict the shader's is
/// `the_gpu_draws_the_set_the_mirror_computes` below.
#[test]
fn no_escape_time_sample_is_non_finite_across_the_declared_ranges() {
    let spec = |name: &str| {
        PARAMS
            .iter()
            .find(|s| s.name == name)
            .and_then(|s| s.range)
            .unwrap_or_else(|| panic!("`{name}` declares a range"))
    };
    // Each range's two ends and its middle, plus values a binding could reach
    // past it.
    let across = |name: &str, beyond: &[f32]| -> Vec<f32> {
        let [lo, hi] = spec(name);
        let mut v = vec![lo, 0.5 * (lo + hi), hi];
        v.extend_from_slice(beyond);
        v
    };
    let c_re = across("c_re", &[-1e30, 1e30]);
    let c_im = across("c_im", &[-1e30, 1e30]);
    let power = across("power", &[1.0, 2.5, 0.0, 1e9]);
    let radius = across("escape_radius", &[0.0, 1e12]);
    let iterations = [1.0, 64.0, MAX_ITERATIONS];

    // The field coordinates a frame can reach: the widest aspect and the
    // smallest zoom in range, plus a pan far past the frame.
    let mut points = Vec::new();
    for i in 0..9 {
        for j in 0..9 {
            points.push([(i as f32 - 4.0) * 4.0, (j as f32 - 4.0) * 4.0]);
        }
    }
    points.extend([[0.0, 0.0], [1e6, -1e6], [-3e38, 3e38]]);

    let mut checked = 0usize;
    for &re in &c_re {
        for &im in &c_im {
            for &pw in &power {
                for &r in &radius {
                    for &it in &iterations {
                        for mandelbrot in [false, true] {
                            let e = Escape {
                                c: [re, im],
                                iterations: it,
                                radius: r,
                                power: pw,
                                interior: 1.0,
                                mandelbrot,
                            }
                            .packed();
                            for &p in &points {
                                let out = mirror::escape(p, &e);
                                checked += 1;
                                assert!(
                                    out.coord.is_finite() && out.light.is_finite(),
                                    "non-finite sample {out:?} at p {p:?}, {e:?}"
                                );
                                assert!(
                                    (0.0..=1.0).contains(&out.light),
                                    "light {} outside [0, 1]",
                                    out.light
                                );
                            }
                        }
                    }
                }
            }
        }
    }
    println!("{checked} escape-time samples, every one finite");
    assert!(checked > 100_000, "the sweep shrank to {checked} samples");
}

/// **The smooth count is continuous where the integer count steps** — which is
/// what keeps the palette free of banding at `palette_steps = 0`.
///
/// Walks a fine ray in from the frame's edge toward the set. Wherever two
/// neighbouring samples escaped at different integer steps, the integer count
/// jumps by a whole iteration — a band edge 1/32 of the palette tall — however
/// gently the field slopes there. The smooth count must instead move across
/// that pair by about what it moves across the pairs either side of it: a jump
/// with no excess over the local slope is no edge at all.
///
/// Judged only where the local slope is gentle, because near the set the count
/// climbs steeply whatever its colouring and a jump there says nothing about
/// banding. A control asserts enough integer steps were judged.
#[test]
fn the_smooth_count_is_continuous_across_every_integer_step() {
    let e = julia(C_INSIDE);
    let samples = 20_000;
    let walk: Vec<mirror::Out> = (0..samples)
        .map(|k| {
            // From x = 1.8 in toward the set along a line just off the real
            // axis.
            let x = 1.8 - 1.4 * k as f32 / samples as f32;
            mirror::escape([x, 0.07], &e)
        })
        .collect();
    let same_step_jump = |a: usize, b: usize| -> Option<f32> {
        let (a, b) = (walk.get(a)?, walk.get(b)?);
        (a.escaped && b.escaped && a.steps == b.steps).then(|| (b.nu - a.nu).abs())
    };

    let (mut judged, mut worst_excess) = (0usize, 0.0f32);
    for k in 2..samples - 1 {
        let (prev, here) = (walk[k - 1], walk[k]);
        if !(prev.escaped && here.escaped) || prev.steps == here.steps {
            continue;
        }
        let (Some(before), Some(after)) = (same_step_jump(k - 2, k - 1), same_step_jump(k, k + 1))
        else {
            continue;
        };
        let slope = before.max(after);
        if slope > 0.01 {
            continue;
        }
        judged += 1;
        let jump = (here.nu - prev.nu).abs();
        worst_excess = worst_excess.max(jump - slope);
    }
    println!(
        "{judged} integer steps judged on a gentle slope; the worst jump across one exceeds \
         the local slope by {worst_excess:.5} (an integer count would exceed it by ~1)"
    );
    assert!(
        judged >= 4,
        "only {judged} integer steps fell on a gentle slope, so this cannot show banding"
    );
    assert!(
        worst_excess < 0.02,
        "across an integer step the smooth count jumps {worst_excess:.5} past its local \
         slope — that is a band edge, not a glow"
    );
}

/// **The GPU draws the set the mirror computes** — which is what lets the two
/// mirror-read tests above speak for the shader.
///
/// Interior black and exterior white over a flat palette, so a capture is the
/// escape mask and nothing else. Pixels whose orbit escapes within a couple of
/// steps of the budget are left out: whether the last `f32` rounding carries
/// them over the radius is the adapter's business.
#[test]
fn the_gpu_draws_the_set_the_mirror_computes() {
    use crate::dsp::AnalysisFrame;
    use crate::preset::Preset;
    use crate::render::{HeadlessOptions, RenderError, Renderer};

    const SIZE: u32 = 96;
    const ZOOM: f32 = 0.6;
    let mut renderer = match Renderer::new_headless(HeadlessOptions {
        width: SIZE,
        height: SIZE,
        prefer_software: true,
    }) {
        Ok(r) => r,
        Err(RenderError::RequestAdapter(_)) => {
            eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
            return;
        }
        Err(e) => panic!("headless renderer build failed: {e}"),
    };
    let toml = format!(
        "system = \"analytic_field\"\nname = \"mirror\"\n\
         [palette]\nstops = [{{ at = 0.0, color = \"#ffffff\" }}, {{ at = 1.0, color = \"#ffffff\" }}]\n\
         [field]\nfamily = \"escape_time\"\n\
         [params]\nc_re = \"{}\"\nc_im = \"{}\"\niterations = \"64\"\nzoom = \"{ZOOM}\"\n",
        C_INSIDE[0], C_INSIDE[1]
    );
    let preset = Preset::from_toml_str(&toml).expect("the mirror probe loads");
    renderer.set_presets(vec![preset]);
    let img = renderer
        .capture_preset("mirror", &AnalysisFrame::default(), 2)
        .expect("capture the mirror probe");

    let e = julia(C_INSIDE);
    let (mut decided, mut agree, mut interior) = (0usize, 0usize, 0usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let px = ((x as f32 + 0.5) / SIZE as f32 * 2.0 - 1.0) / ZOOM;
            let py = (1.0 - (y as f32 + 0.5) / SIZE as f32 * 2.0) / ZOOM;
            let out = mirror::escape([px, py], &e);
            if out.escaped && out.steps + 2 >= e.iterations as u32 {
                continue;
            }
            decided += 1;
            let i = ((y * SIZE + x) * 4) as usize;
            let lit = img.rgba[i] > 127;
            if !out.escaped {
                interior += 1;
            }
            if lit == out.escaped {
                agree += 1;
            }
        }
    }
    let agreement = agree as f32 / decided as f32;
    println!(
        "GPU vs mirror: {agreement:.4} of {decided} decided pixels agree; {interior} interior"
    );
    assert!(
        interior > decided / 20,
        "the probe's set covers only {interior} of {decided} pixels, too little to compare"
    );
    assert!(
        agreement > 0.99,
        "the GPU and the mirror disagree on {:.2}% of the escape mask — the mirror has \
         drifted from the shader, and the tests that read through it no longer speak for it",
        100.0 * (1.0 - agreement)
    );
}

/// The palette coordinate the escape arm hands on is the smooth count over a
/// FIXED scale, so an early-escaping pixel's colour does not depend on the
/// budget — capping `iterations` recolours only the pixels past the cap.
#[test]
fn an_early_escape_keeps_its_colour_whatever_the_budget() {
    let p = [1.2, 0.3];
    let shallow = mirror::escape(
        p,
        &Escape {
            iterations: 16.0,
            ..julia(C_INSIDE)
        }
        .packed(),
    );
    let deep = mirror::escape(
        p,
        &Escape {
            iterations: 512.0,
            ..julia(C_INSIDE)
        }
        .packed(),
    );
    assert!(shallow.escaped && shallow.steps < 16, "{shallow:?}");
    assert_eq!(shallow.coord, deep.coord);
    assert_eq!(shallow.coord, shallow.nu / ITERATIONS_PER_PALETTE);
}
