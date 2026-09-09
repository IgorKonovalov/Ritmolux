//! `shape_field`'s own contract (Plan 0091 Phase 3).
//!
//! Two of these are rendered rather than arithmetic, because the two claims
//! that matter are about what reaches the frame: that the aspect comes from the
//! render target, and that banding the coordinate draws **offsets of the shape**
//! rather than concentric circles. Both are wired-up claims, and both would pass
//! a CPU-side test of the arithmetic while the shader read the wrong thing.

#![allow(clippy::indexing_slicing, clippy::panic, clippy::expect_used)]

use super::{
    COORD_MODES, DEFAULT_COORD_MODE, DEFAULT_GAMMA, DEFAULT_ROTATION, DEFAULT_SCALE,
    MAX_COORD_MODE, MAX_GAMMA, MAX_SCALE, MIN_COORD_MODE, MIN_GAMMA, MIN_SCALE, PARAMS,
    applied_coord_mode, applied_gamma, applied_rotation, applied_scale, coord,
};
use crate::dsp::AnalysisFrame;
use crate::preset::Preset;
use crate::render::scenes::declares;
use crate::render::scenes::marks;
use crate::render::{CaptureImage, HeadlessOptions, RenderError, Renderer};

/// A preset driving this scene, with `extra` spliced into `[params]`.
fn preset(name: &str, extra: &str) -> Preset {
    let toml = format!("name = \"{name}\"\nsystem = \"shape_field\"\n[params]\n{extra}");
    Preset::from_toml_str(&toml).unwrap_or_else(|e| panic!("{name} failed to load: {e}"))
}

fn headless(width: u32, height: u32) -> Option<Renderer> {
    match Renderer::new_headless(HeadlessOptions {
        width,
        height,
        prefer_software: true,
    }) {
        Ok(r) => Some(r),
        Err(RenderError::RequestAdapter(_)) => {
            eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
            None
        }
        Err(e) => panic!("headless renderer build failed: {e}"),
    }
}

/// Rec. 601 luma of one pixel, `0..255`.
fn luma(img: &CaptureImage, x: u32, y: u32) -> f32 {
    let i = ((y * img.width + x) * 4) as usize;
    0.299 * f32::from(img.rgba[i])
        + 0.587 * f32::from(img.rgba[i + 1])
        + 0.114 * f32::from(img.rgba[i + 2])
}

/// `scale` is held inside the range the arithmetic needs, and a broken binding
/// lands on the default rather than on a bound.
#[test]
fn the_scale_is_clamped_and_falls_back() {
    assert_eq!(applied_scale(DEFAULT_SCALE), DEFAULT_SCALE);
    assert_eq!(applied_scale(0.0), MIN_SCALE);
    assert_eq!(applied_scale(-4.0), MIN_SCALE);
    assert_eq!(applied_scale(1e9), MAX_SCALE);
    assert_eq!(applied_scale(f32::NAN), DEFAULT_SCALE);
    assert_eq!(applied_scale(f32::INFINITY), DEFAULT_SCALE);
}

/// The declared vocabulary carries the shared roster's two names, so a preset
/// binding `shape` on this scene is not warned at as a typo.
#[test]
fn the_vocabulary_carries_the_shared_roster() {
    assert!(declares(PARAMS, "shape"));
    assert!(declares(PARAMS, "points"));
    assert!(declares(PARAMS, "scale"));
}

/// **The aspect comes from the render target, and this test bites**
/// (ADR-0037).
///
/// A `disc` is rendered at 2:1 and at 1:2 and its own extent is measured in
/// pixels. The figure must be **round** — the same number of frame-widths across
/// as it is frame-heights tall in absolute pixels — at both, which means the
/// measured pixel width equals the measured pixel height.
///
/// This is stated as a size test rather than a code-reading one because the
/// usual sizes cannot tell the two apart: 1920x1080 and this box's 2048x1152
/// both quantize to exactly 16:9, and at a square target every wrong aspect
/// source is right by accident. 2:1 and 1:2 are chosen so a dropped `aspect`
/// (i.e. 1.0) distorts the figure by a factor of two, and an *inverted* one by
/// four.
///
/// **Confirmed to bite, in the reverted direction**, which is the only way this
/// claim is worth anything. Substituting a literal `1.0` for the target's
/// aspect on the way to the uniform — the ADR-0037 defect, which has shipped
/// three times in this repo — renders the disc **29 px across and 14 px down**
/// at 240x120 (ratio 2.071) and 14 x 29 at 120x240, and this test fails on
/// both. As shipped it measures 14 x 14 and 29 x 29.
#[test]
fn the_figure_is_round_at_a_non_sixteen_by_nine_target() {
    // A hard-edged disc: `palette_steps = 2` puts a band boundary right at the
    // outline, so the extent below is a real edge rather than a ramp. The
    // palette runs dark-to-light so the interior is unambiguous.
    let params = "shape = \"0\"\nscale = \"0.5\"\ncolor_span = \"1\"\npalette_steps = \"2\"\n";

    let mut extents = Vec::new();
    for (w, h) in [(240u32, 120u32), (120, 240)] {
        let Some(mut renderer) = headless(w, h) else {
            return;
        };
        renderer.set_presets(vec![preset("round", params)]);
        let img = renderer
            .capture_preset("round", &AnalysisFrame::default(), 2)
            .expect("capture the disc");

        // The figure is centred, so walk out from the centre row and column to
        // the first pixel that differs from the centre's band.
        let (cx, cy) = (w / 2, h / 2);
        let centre = luma(&img, cx, cy);
        let differs = |v: f32| (v - centre).abs() > 8.0;
        let mut half_w = 0u32;
        while cx + half_w + 1 < w && !differs(luma(&img, cx + half_w + 1, cy)) {
            half_w += 1;
        }
        let mut half_h = 0u32;
        while cy + half_h + 1 < h && !differs(luma(&img, cx, cy + half_h + 1)) {
            half_h += 1;
        }
        println!("{w}x{h}: figure half-extent {half_w} px across, {half_h} px down");
        assert!(
            half_w > 4 && half_h > 4,
            "{w}x{h}: the figure has no measurable extent ({half_w} x {half_h}) — the \
             probe found no band edge, so it is measuring nothing"
        );
        extents.push((w, h, half_w, half_h));
    }

    for (w, h, half_w, half_h) in extents {
        let ratio = half_w as f32 / half_h as f32;
        assert!(
            (ratio - 1.0).abs() < 0.12,
            "at {w}x{h} the disc is {half_w} px across and {half_h} px down \
             (ratio {ratio:.3}) — it must be ROUND. An aspect taken from anywhere \
             but the render target distorts it by exactly the target's own \
             aspect, which at this size is a factor of two"
        );
    }
}

/// **The bands are offsets of the shape, not concentric circles** — the property
/// ADR-0105's whole argument rests on.
///
/// Checked on the **heart**, because it is the roster's one arm that is not
/// radially symmetric: a banding driven by a radius and a banding driven by a
/// distance are the same picture on a disc and different pictures here.
///
/// The measurement walks rays out from the figure's centre, finds where the
/// rendered band first changes, and asks two questions of those points:
///
/// - their **distance to the heart's own outline** must be near-constant — that
///   is what "offset curve" means, and it is the claim;
/// - their **radius** from the centre must *not* be, which is the control. It is
///   what makes the first assertion non-vacuous: if the figure were radially
///   symmetric both would be constant and the test could not tell the
///   constructions apart.
///
/// The outline is the numerically sampled one from `marks`' own Phase 2 harness,
/// not `mark_distance` — so this grades the render against the figure rather
/// than against the arithmetic that drew it.
#[test]
fn banding_the_distance_draws_offsets_of_the_shape() {
    const W: u32 = 320;
    const H: u32 = 320;
    // Square target on purpose: it takes the aspect out of the arithmetic below
    // so this test is about the banding alone. The aspect has its own test.
    let Some(mut renderer) = headless(W, H) else {
        return;
    };
    const SCALE: f32 = 0.5;
    renderer.set_presets(vec![preset(
        "offsets",
        "shape = \"4\"\nscale = \"0.5\"\ncolor_span = \"0.5\"\npalette_steps = \"8\"\n",
    )]);
    let img = renderer
        .capture_preset("offsets", &AnalysisFrame::default(), 2)
        .expect("capture the banded heart");

    let loops = marks::tests::boundary_loops(marks::tests::HEART, marks::DEFAULT_POINTS);
    let (cx, cy) = (W as f32 / 2.0, H as f32 / 2.0);

    // Walk rays and record the FIRST band change on each — one contour, sampled
    // all the way round.
    let mut distances = Vec::new();
    let mut radii = Vec::new();
    let rays = 64;
    for i in 0..rays {
        let theta = std::f32::consts::TAU * i as f32 / rays as f32;
        let (dx, dy) = (theta.cos(), theta.sin());
        let start = luma(&img, cx as u32, cy as u32);
        let mut hit = None;
        let mut r = 1.0f32;
        while r < (W as f32) * 0.48 {
            let (x, y) = (cx + dx * r, cy + dy * r);
            if (luma(&img, x as u32, y as u32) - start).abs() > 8.0 {
                hit = Some(r);
                break;
            }
            r += 0.5;
        }
        let Some(r) = hit else { continue };

        // Pixel -> the scene's square-unit space -> the figure's own frame. NDC
        // y is up, pixel y is down, and the target is square so `aspect` is 1.
        let ndc = [
            (cx + dx * r) / (W as f32) * 2.0 - 1.0,
            1.0 - (cy + dy * r) / (H as f32) * 2.0,
        ];
        let p = [ndc[0] / SCALE, ndc[1] / SCALE];
        distances.push(marks::tests::true_signed_distance(p, &loops));
        radii.push(r);
    }

    assert!(
        distances.len() > rays / 2,
        "only {} of {rays} rays found a band edge — the frame is not banded and \
         this test is measuring nothing",
        distances.len()
    );

    let spread = |v: &[f32]| -> (f32, f32) {
        let mean = v.iter().sum::<f32>() / v.len() as f32;
        let var = v.iter().map(|x| (x - mean) * (x - mean)).sum::<f32>() / v.len() as f32;
        (mean, var.sqrt())
    };
    let (d_mean, d_sd) = spread(&distances);
    let (r_mean, r_sd) = spread(&radii);
    println!(
        "first band edge over {} rays: distance-to-outline {d_mean:.4} +- {d_sd:.4} \
         (figure units); radius {r_mean:.1} +- {r_sd:.1} px",
        distances.len()
    );

    // The control first: on this figure the contour is emphatically NOT a
    // circle, so a radius-driven banding would have been distinguishable.
    assert!(
        r_sd / r_mean > 0.08,
        "the contour's radius barely varies ({r_sd:.2} of {r_mean:.2}) — on a \
         heart it must, and if it does not this test cannot tell an offset curve \
         from a circle"
    );
    // ...and the claim: it IS an offset of the outline.
    assert!(
        d_sd < 0.06,
        "the first band edge sits {d_mean:.4} +- {d_sd:.4} from the heart's own \
         outline — a band of the palette coordinate must be a band of constant \
         DISTANCE, which is what makes it an offset curve rather than a circle"
    );
}

// --- Phase 4: the figure responds ---------------------------------------------
//
// Most of this phase is **verification that the three levers the user asked for
// are already free**, which is why there is more measurement here than code. The
// one thing built is the response exponent.

/// The response exponent is conditioned CPU-side, and **`1.0` is the exact
/// identity** (ADR-0092's care, because `pow(x, 1.0)` is not bit-exact).
#[test]
fn the_response_exponent_is_an_exact_identity_at_one() {
    assert_eq!(applied_gamma(DEFAULT_GAMMA), 1.0);
    assert_eq!(applied_gamma(0.0), MIN_GAMMA);
    assert_eq!(applied_gamma(-3.0), MIN_GAMMA);
    assert_eq!(applied_gamma(1e9), MAX_GAMMA);
    assert_eq!(applied_gamma(f32::NAN), DEFAULT_GAMMA);

    // Bit equality, not a tolerance: at the default the coordinate must be the
    // distance itself, so an unbound preset never goes through `pow`.
    for i in 0..=40 {
        let d = i as f32 / 10.0;
        let through = coord(d, 1.0, 1.0, 0.0);
        assert_eq!(
            through.to_bits(),
            d.to_bits(),
            "gamma = 1 must pass the distance through untouched: {through} vs {d}"
        );
    }
}

/// **The exponent moves where the contours crowd, and BELOW 1 is the direction
/// the reference images want.**
///
/// Stated as a property of where the *band boundaries* land rather than as a
/// curve shape, because that is the thing an author sees. Bands are evenly
/// spaced in the palette coordinate and the coordinate is `d^gamma`, so a
/// boundary at `k/n` sits at distance `(k/n)^(1/gamma)`:
///
/// | `gamma` | boundary distances (8 bands) | reads as |
/// |---|---|---|
/// | `0.4` | 0.006 0.031 0.086 0.177 0.309 0.487 0.716 1.0 | **crowded toward the centre** |
/// | `1.0` | 0.125 0.25 0.375 0.5 0.625 0.75 0.875 1.0 | evenly spaced |
/// | `2.5` | 0.435 0.574 0.675 0.758 0.829 0.891 0.948 1.0 | crowded toward the outline |
///
/// **The direction is worth stating because it is the opposite of the intuition
/// `ink_gamma` builds.** There a higher exponent darkens, so "more" reads as
/// "more effect toward the low end"; here the exponent is inverted on its way to
/// a boundary position, so it is `gamma` **below** 1 that tightens the rings
/// toward the middle the way the user's reference does. Nothing warns; the docs
/// carry the table.
#[test]
fn the_exponent_moves_where_the_contours_crowd() {
    // Where does band boundary `k / n` sit, in distance? Solve
    // `coord(d) = k/n` for `d` at `color_span = 1`, `color_center = 0`:
    // `d^gamma = k/n`, so `d = (k/n)^(1/gamma)`.
    let boundary = |k: u32, n: u32, gamma: f32| -> f32 { (k as f32 / n as f32).powf(1.0 / gamma) };
    let gaps = |gamma: f32| -> Vec<f32> {
        (1..8)
            .map(|k| boundary(k + 1, 8, gamma) - boundary(k, 8, gamma))
            .collect()
    };

    // The boundary helper must agree with the shader's own arithmetic, or this
    // whole test is about a formula rather than about the scene.
    for gamma in [0.4f32, 1.0, 2.5] {
        for k in 1..8 {
            let d = boundary(k, 8, gamma);
            let want = k as f32 / 8.0;
            let got = coord(d, gamma, 1.0, 0.0);
            assert!(
                (got - want).abs() < 1e-4,
                "the boundary solver disagrees with `coord` at gamma {gamma}, \
                 band {k}: {got} vs {want}"
            );
        }
    }

    let flat = gaps(1.0);
    for w in flat.windows(2) {
        assert!(
            (w[1] - w[0]).abs() < 1e-5,
            "at gamma = 1 the contours must be EVENLY spaced — that is the \
             baseline this param exists to move away from ({flat:?})"
        );
    }

    // Below 1: the gaps GROW outward, so the rings tighten toward the centre.
    // This is the reference's direction.
    let toward_centre = gaps(0.4);
    for w in toward_centre.windows(2) {
        assert!(
            w[1] > w[0],
            "below 1 the gaps must grow outward — the contours tighten toward the \
             CENTRE, which is what the reference images do ({toward_centre:?})"
        );
    }
    // Above 1: the reverse, and it is a real look rather than a mistake.
    let toward_edge = gaps(2.5);
    for w in toward_edge.windows(2) {
        assert!(
            w[1] < w[0],
            "above 1 the contours must crowd toward the OUTLINE ({toward_edge:?})"
        );
    }
    println!(
        "contour gaps  gamma 0.4: {toward_centre:?}\n              gamma 1.0: {flat:?}\n              gamma 2.5: {toward_edge:?}"
    );
}

/// **The figure breathes**: `scale` takes a binding and the response is
/// **monotone** in it.
///
/// Rendered rather than argued, and measured as the figure's own pixel extent
/// at three scales — a param that reached the uniform but was, say, inverted or
/// clamped flat would still load and still warn about nothing.
#[test]
fn the_figure_breathes_monotonically_with_scale() {
    const SIZE: u32 = 240;
    let Some(mut renderer) = headless(SIZE, SIZE) else {
        return;
    };
    let scales = [0.25f32, 0.45, 0.7];
    renderer.set_presets(
        scales
            .iter()
            .map(|s| {
                preset(
                    &format!("s{}", (s * 100.0) as u32),
                    &format!(
                        "shape = \"0\"\nscale = \"{s}\"\ncolor_span = \"1\"\npalette_steps = \"2\"\n"
                    ),
                )
            })
            .collect(),
    );

    let mut extents = Vec::new();
    for s in scales {
        let name = format!("s{}", (s * 100.0) as u32);
        let img = renderer
            .capture_preset(&name, &AnalysisFrame::default(), 2)
            .unwrap_or_else(|e| panic!("capture {name}: {e}"));
        let (cx, cy) = (SIZE / 2, SIZE / 2);
        let centre = luma(&img, cx, cy);
        let mut half = 0u32;
        while cx + half + 1 < SIZE && (luma(&img, cx + half + 1, cy) - centre).abs() <= 8.0 {
            half += 1;
        }
        println!("scale {s}: figure half-extent {half} px");
        extents.push(half);
    }

    assert!(
        extents[0] < extents[1] && extents[1] < extents[2],
        "the figure's extent must grow monotonically with `scale` — measured \
         {extents:?} at scales {scales:?}"
    );
    assert!(
        extents[0] > 4,
        "the smallest figure has no measurable extent ({}) — this test is \
         measuring nothing",
        extents[0]
    );
}

/// **Rings travel outward from `color_center`, and the wrap does not stutter** —
/// the first of the three asks, and it costs no code. This is the evidence, plus
/// the check on the one seam that could have spoiled it.
///
/// `color_center` offsets the palette coordinate, which is now a distance, so
/// sliding it slides every contour outward together. The risk is the LUT's
/// **repeat addressing**: the coordinate wraps at 1, and if the gradient's two
/// ends differ the wrap is a visible seam crossing the figure. So this walks a
/// full cycle of `color_center` and asserts two things — that the picture
/// actually moves at every step, and that no single step is an outlier against
/// the rest, which is what a stutter at the wrap would look like.
#[test]
fn rings_travel_outward_with_color_center_and_the_wrap_does_not_stutter() {
    use crate::render::metrics::frame_diff;

    const SIZE: u32 = 160;
    const STEPS: usize = 12;
    let Some(mut renderer) = headless(SIZE, SIZE) else {
        return;
    };
    // A CYCLIC gradient, which is what the wrap needs to be seamless: the last
    // stop is the first colour again. `presets/README.md` says so at the
    // parameter; this test is where the claim is checked.
    let cyclic = "[palette]\nstops = [\n\
                  { at = 0.0, color = \"#474778\" },\n\
                  { at = 0.5, color = \"#ffa589\" },\n\
                  { at = 1.0, color = \"#474778\" },\n]\n";
    let presets: Vec<_> = (0..STEPS)
        .map(|i| {
            let c = i as f32 / STEPS as f32;
            let toml = format!(
                "name = \"c{i}\"\nsystem = \"shape_field\"\n{cyclic}\
                 [params]\nshape = \"4\"\nscale = \"0.45\"\ncolor_span = \"0.5\"\n\
                 palette_steps = \"6\"\ncolor_center = \"{c}\"\n"
            );
            Preset::from_toml_str(&toml).unwrap_or_else(|e| panic!("c{i}: {e}"))
        })
        .collect();
    renderer.set_presets(presets);

    let frames: Vec<_> = (0..STEPS)
        .map(|i| {
            renderer
                .capture_preset(&format!("c{i}"), &AnalysisFrame::default(), 2)
                .unwrap_or_else(|e| panic!("capture c{i}: {e}"))
        })
        .collect();

    // Consecutive steps, wrapping the last back to the first — so the step
    // ACROSS the seam is in the list and is graded like every other.
    let steps: Vec<f32> = (0..STEPS)
        .map(|i| frame_diff(&frames[i], &frames[(i + 1) % STEPS]))
        .collect();
    let mean = steps.iter().sum::<f32>() / steps.len() as f32;
    let worst = steps.iter().cloned().fold(0.0f32, f32::max);
    println!(
        "color_center walk, {STEPS} steps round a full cycle: mean {mean:.5}, \
         worst {worst:.5}, wrap step {:.5}",
        steps[STEPS - 1]
    );

    assert!(
        steps.iter().all(|d| *d > 0.001),
        "every step of `color_center` must move the picture — the rings travel \
         because the coordinate they band is a distance ({steps:?})"
    );
    assert!(
        worst < 4.0 * mean,
        "one step is {worst:.5} against a mean of {mean:.5} — on a cyclic \
         gradient the wrap must be no more of a jump than any other step, and an \
         outlier here IS the stutter the plan asked about ({steps:?})"
    );
}

/// **Ring count on the beat**: `palette_steps` is quantized CPU-side, so an
/// eased binding visits whole counts and never a fractional one.
///
/// The open question is not whether it works but whether it *reads* — a band
/// count is a global change to every pixel at once, which is exactly the
/// shape a strobe has. That is Phase 6's judgement, in the running app. What
/// this pins is the half a test can settle: each count is a distinct picture,
/// and the distinctness is not an artifact of a fractional value crawling.
#[test]
fn the_ring_count_steps_between_whole_figures() {
    use crate::render::metrics::frame_diff;
    use crate::render::palette::band_steps;

    // The CPU-side quantizer is the mechanism, and it is shared — this asserts
    // the property this scene depends on rather than re-implementing it.
    for i in 0..=400 {
        let raw = 3.0 + 6.0 * i as f32 / 400.0;
        let q = band_steps(raw);
        assert_eq!(
            q,
            q.round(),
            "palette_steps reached the shader at {q}, from {raw}"
        );
    }

    const SIZE: u32 = 160;
    let Some(mut renderer) = headless(SIZE, SIZE) else {
        return;
    };
    let counts = [4u32, 5, 6, 7];
    renderer.set_presets(
        counts
            .iter()
            .map(|n| {
                preset(
                    &format!("n{n}"),
                    &format!(
                        "shape = \"4\"\nscale = \"0.45\"\ncolor_span = \"0.5\"\n\
                         palette_steps = \"{n}\"\n"
                    ),
                )
            })
            .collect(),
    );
    let frames: Vec<_> = counts
        .iter()
        .map(|n| {
            renderer
                .capture_preset(&format!("n{n}"), &AnalysisFrame::default(), 2)
                .unwrap_or_else(|e| panic!("capture n{n}: {e}"))
        })
        .collect();
    for (i, a) in frames.iter().enumerate() {
        for (j, b) in frames.iter().enumerate().skip(i + 1) {
            let diff = frame_diff(a, b);
            assert!(
                diff > 0.002,
                "ring counts {} and {} render indistinguishably (diff {diff:.5})",
                counts[i],
                counts[j]
            );
        }
    }
}

// --- Phase 2: the scaled-copy coordinate ---------------------------------------
//
// ADR-0111's second coordinate. The claims that matter are about what reaches
// the frame, so two of the three below are rendered: a mode that reached the
// uniform but was never read would pass any CPU-side test of the arithmetic.

/// The mode selector is closed at both ends, **rounds**, and a broken binding
/// lands on the default rather than on a bound (`kaleido_edge`'s rule, which
/// `marks::mark_shape` already follows for the same reason).
///
/// The rounding is the load-bearing half: `[smoothing]` and preset dissolves
/// interpolate a binding continuously, so easing the distance to the radius
/// passes through 0.4 and 0.6 — and there is nothing halfway between an offset
/// curve and a scaled copy for the shader to draw there.
#[test]
fn the_coordinate_mode_clamps_rounds_and_falls_back() {
    // Any arm but `ring`, which refuses the radius mode outright and has
    // its own test below.
    const NOT_RING: f32 = marks::DEFAULT_SHAPE;
    assert_eq!(applied_coord_mode(DEFAULT_COORD_MODE, NOT_RING), 0.0);
    assert_eq!(applied_coord_mode(-3.0, NOT_RING), MIN_COORD_MODE);
    assert_eq!(applied_coord_mode(99.0, NOT_RING), MAX_COORD_MODE);
    assert_eq!(applied_coord_mode(0.4, NOT_RING), 0.0);
    assert_eq!(applied_coord_mode(0.6, NOT_RING), 1.0);
    assert_eq!(applied_coord_mode(f32::NAN, NOT_RING), DEFAULT_COORD_MODE);
    assert_eq!(
        applied_coord_mode(f32::INFINITY, NOT_RING),
        DEFAULT_COORD_MODE
    );
    for (i, _) in COORD_MODES.iter().enumerate() {
        assert_eq!(applied_coord_mode(i as f32, NOT_RING), i as f32);
    }

    // ...and nothing the quantizer emits is ever fractional, anywhere in range.
    for i in 0..=400 {
        let raw = -1.0 + 3.0 * i as f32 / 400.0;
        let q = applied_coord_mode(raw, NOT_RING);
        assert_eq!(
            q,
            q.round(),
            "coord_mode reached the shader at {q}, from {raw}"
        );
    }
    assert_eq!(COORD_MODES, ["distance", "radius"]);
    assert!(declares(PARAMS, "coord_mode"));
}

/// **Under the radius mode a contour is a SCALED COPY of the outline, and under
/// the distance mode it is not** — the property this whole plan exists for,
/// measured on the one arm of Phase 2's pair that can show the difference.
///
/// # Why a polygon, and why the contour measured is OUTSIDE it
///
/// A scaled polygon keeps its corners and an offset one rounds them — but that
/// is only true **outside** the figure, and the difference is worth stating
/// because the obvious reading of this test is vacuous.
///
/// *Inside* a **regular** polygon the two coordinates are not merely similar,
/// they are the same expression. The interior arm is `r cos(f) / apothem`, whose
/// level set is the line `x = c * apothem` in folded coordinates — a regular
/// polygon of apothem `c * apothem`, i.e. a scaled copy. Eroding a regular
/// polygon moves every edge inward by the same amount and rounds nothing,
/// because erosion rounds **reflex** corners and a convex polygon has none. So
/// an interior contour measures 0 spread under both modes and proves nothing.
/// (Measured, on a pentagon: mean 0.2513 and relative spread 0.0177 under both,
/// to four figures.)
///
/// Outside, the arm measures to the edge as a **segment**, so its level set
/// carries a circular arc around each vertex while the radius mode's stays a
/// sharp scaled triangle. `palette_steps * color_span` is set below 1 on
/// purpose, which puts the **first** band boundary past the outline — so the
/// first contour each ray meets is already the exterior one, with no band
/// counting to get wrong.
///
/// # The measurement, and the control that makes it non-vacuous
///
/// Rays are walked out from the figure's centre until the rendered band changes.
/// Each hit is converted into the figure's own frame and divided by
/// `r_boundary(theta)` — the closed form the shader used, mirrored in
/// `marks::tests`. Under the radius mode that ratio is **constant in theta**,
/// which is the definition of a scaled copy.
///
/// The same measurement under the **distance** mode is the control, and both
/// numbers are printed. Without it the first assertion would pass on a disc, on
/// a bug, and on a coordinate that was never wired up: a spread of zero proves
/// nothing unless something in the same harness produces a spread that is not.
#[test]
fn the_radius_mode_bands_scaled_copies_where_the_distance_bands_offsets() {
    // Large on purpose: the band edge is located to the nearest pixel, so the
    // radius mode's residual spread IS the pixel grid. At this size the contour
    // sits 96..192 px out, which puts that residual near 0.003 — two orders
    // below the effect being measured rather than one.
    const W: u32 = 640;
    const H: u32 = 640;
    // Square target on purpose: it takes the aspect out of the arithmetic below
    // so this test is about the coordinate alone. The aspect has its own test.
    let Some(mut renderer) = headless(W, H) else {
        return;
    };
    // A TRIANGLE, because the corner is where the two constructions differ and
    // three corners are the sharpest the roster allows. `palette_steps *
    // color_span = 0.5`, below 1, so the first band boundary sits at a
    // coordinate of **2.0** — well outside the outline, where the difference
    // lives and where it is largest.
    const SCALE: f32 = 0.3;
    const POINTS: f32 = 3.0;
    let params = |mode: &str| {
        format!(
            "shape = \"2\"\npoints = \"3\"\nscale = \"0.3\"\ncolor_span = \"0.125\"\n\
             palette_steps = \"4\"\ncoord_mode = \"{mode}\"\n"
        )
    };
    renderer.set_presets(vec![
        preset("offsets", &params("0")),
        preset("copies", &params("1")),
    ]);

    let (cx, cy) = (W as f32 / 2.0, H as f32 / 2.0);
    let mut spread_of = |name: &str| -> (f32, f32, usize) {
        let img = renderer
            .capture_preset(name, &AnalysisFrame::default(), 2)
            .unwrap_or_else(|e| panic!("capture {name}: {e}"));
        let start = luma(&img, cx as u32, cy as u32);
        let mut ratios = Vec::new();
        let rays = 90;
        for i in 0..rays {
            let theta = std::f32::consts::TAU * i as f32 / rays as f32;
            let (dx, dy) = (theta.cos(), theta.sin());
            let mut hit = None;
            let mut r = 1.0f32;
            while r < (W as f32) * 0.48 {
                if (luma(&img, (cx + dx * r) as u32, (cy + dy * r) as u32) - start).abs() > 8.0 {
                    hit = Some(r);
                    break;
                }
                r += 0.25;
            }
            let Some(r) = hit else { continue };
            // Pixel -> the scene's square-unit space -> the figure's own frame.
            // NDC y is up, pixel y is down, and the target is square so the
            // aspect is 1.
            let ndc = [
                (cx + dx * r) / (W as f32) * 2.0 - 1.0,
                1.0 - (cy + dy * r) / (H as f32) * 2.0,
            ];
            let p = [ndc[0] / SCALE, ndc[1] / SCALE];
            let boundary = marks::tests::mark_boundary_radius(
                p,
                marks::tests::POLYGON,
                POINTS,
                marks::tests::NEUTRAL_STAR,
            );
            ratios.push((p[0] * p[0] + p[1] * p[1]).sqrt() / boundary);
        }
        let n = ratios.len();
        let mean = ratios.iter().sum::<f32>() / n as f32;
        let sd = (ratios.iter().map(|x| (x - mean) * (x - mean)).sum::<f32>() / n as f32).sqrt();
        (mean, sd / mean, n)
    };

    let (d_mean, d_spread, d_n) = spread_of("offsets");
    let (r_mean, r_spread, r_n) = spread_of("copies");
    println!(
        "first contour, r / r_boundary(theta) over 90 rays on a triangle:\n  \
         distance mode: mean {d_mean:.4}, relative spread {d_spread:.4} ({d_n} rays)\n  \
         radius   mode: mean {r_mean:.4}, relative spread {r_spread:.4} ({r_n} rays)"
    );

    assert!(
        d_n > 60 && r_n > 60,
        "only {d_n} / {r_n} of 90 rays found a band edge — the frame is not \
         banded and this test is measuring nothing"
    );
    // The claim: the contour IS a scaled copy, so the ratio does not vary.
    assert!(
        r_spread < 0.02,
        "under the radius mode the first contour's `r / r_boundary` varies by \
         {r_spread:.4} of its mean — a band of THAT coordinate is a band of \
         constant scaling, so the ratio must be constant in theta or the level \
         set is not a scaled copy of the outline"
    );
    // The control: on the same figure, in the same run, the offset coordinate's
    // does. A dimensionless comparison, so the adapter cancels (ADR-0071).
    assert!(
        d_spread > 4.0 * r_spread,
        "the distance mode's spread ({d_spread:.4}) is not meaningfully larger \
         than the radius mode's ({r_spread:.4}) — on a triangle an offset curve \
         rounds the corners and a scaled copy keeps them, so if these agree the \
         harness cannot tell the two constructions apart and the assertion above \
         proves nothing"
    );
}

/// **The two modes agree on a `disc`** — the harness check, and the reason the
/// disc arm returns a literal `1.0`.
///
/// For a circle the two constructions coincide exactly: `mark_distance` is
/// `length(p)` and the ratio is `length(p) / 1`. So a disagreement here convicts
/// the wiring rather than the shape, and it is the one place in this pair where
/// "no difference" is the claim rather than the failure.
#[test]
fn the_two_modes_coincide_on_a_disc() {
    use crate::render::metrics::frame_diff;

    const SIZE: u32 = 200;
    let Some(mut renderer) = headless(SIZE, SIZE) else {
        return;
    };
    let params = |mode: &str| {
        format!(
            "shape = \"0\"\nscale = \"0.55\"\ncolor_span = \"0.5\"\n\
             palette_steps = \"7\"\npalette_contour = \"0.6\"\ncoord_mode = \"{mode}\"\n"
        )
    };
    renderer.set_presets(vec![
        preset("disc_d", &params("0")),
        preset("disc_r", &params("1")),
    ]);
    let a = renderer
        .capture_preset("disc_d", &AnalysisFrame::default(), 2)
        .expect("capture the disc under the distance mode");
    let b = renderer
        .capture_preset("disc_r", &AnalysisFrame::default(), 2)
        .expect("capture the disc under the radius mode");

    let differing = a
        .rgba
        .chunks_exact(4)
        .zip(b.rgba.chunks_exact(4))
        .filter(|(x, y)| x[..3] != y[..3])
        .count();
    println!(
        "disc under both modes: frame_diff {:.6}, {differing} of {} pixels differ",
        frame_diff(&a, &b),
        (SIZE * SIZE) as usize
    );
    assert_eq!(
        differing, 0,
        "the two coordinates must draw the SAME disc — `mark_distance` is \
         `length(p)` there and the ratio is `length(p) / 1`, so a difference is \
         the wiring rather than the shape"
    );
}

// --- Phase 4: `ring` gets an honest answer -------------------------------------
//
// ADR-0111 names this as the one behavioural choice it leaves open, with
// three defensible answers. All three were rendered before one was chosen, and
// the rendering is what settled it rather than the argument:
//
// | answer | what it renders | verdict |
// |---|---|---|
// | silent fallback to the distance | the annulus, banded about its mid-radius | right picture, no way to know why |
// | **warn, then the distance** | the same picture | **chosen** |
// | define it against the outer rim | **byte-identical to a `disc`** | the hole stops existing |
//
// The third is the one that had to be seen. `r / r_outer` collapses to
// `length(p)`, so the annulus renders as a plain radial ramp — the same file,
// to the byte, as `shape = "0"` at the same settings. A preset would name one
// roster entry and be shown another. That is the negative ADR-0111 predicted,
// reached in practice, and it is why the arm is refused rather than defined.
//
// Between the first two the picture is identical and only the telling differs,
// which is the whole of ADR-0020's argument for a load warning.

/// **A `ring` never reaches the shader with the radius mode selected**, whatever
/// the binding says.
#[test]
fn the_ring_arm_falls_back_to_the_distance() {
    // Every mode, every way of spelling it, on a ring: always the distance.
    for raw in [0.0f32, 0.6, 1.0, 4.0, -2.0, f32::NAN, f32::INFINITY] {
        assert_eq!(
            applied_coord_mode(raw, marks::RING_SHAPE),
            DEFAULT_COORD_MODE,
            "a ring must take the distance whatever `coord_mode` says ({raw})"
        );
    }
    // ...and the refusal is scoped to that one arm. Every other shape still
    // gets the mode it asked for.
    for shape in [0.0f32, 2.0, 3.0, 4.0] {
        assert_eq!(applied_coord_mode(1.0, shape), 1.0, "shape {shape}");
        assert_eq!(applied_coord_mode(0.0, shape), 0.0, "shape {shape}");
    }
}

/// **A `ring` under either mode renders the same frame, to the byte** — the
/// fallback, proved where it is visible rather than at the quantizer.
///
/// This is the assertion that would have failed under the outer-rim definition:
/// there the two frames differ completely, and the radius one matches a `disc`.
#[test]
fn a_ring_renders_identically_under_both_modes_and_is_not_a_disc() {
    const SIZE: u32 = 220;
    let Some(mut renderer) = headless(SIZE, SIZE) else {
        return;
    };
    let body = |shape: &str, mode: &str| {
        format!(
            "shape = \"{shape}\"\nscale = \"0.55\"\ncolor_span = \"0.5\"\n\
             palette_steps = \"7\"\npalette_contour = \"0.55\"\ncoord_mode = \"{mode}\"\n"
        )
    };
    renderer.set_presets(vec![
        preset("ring_d", &body("1", "0")),
        preset("ring_r", &body("1", "1")),
        preset("disc_r", &body("0", "1")),
    ]);
    let shot = |renderer: &mut Renderer, name: &str| {
        renderer
            .capture_preset(name, &AnalysisFrame::default(), 2)
            .unwrap_or_else(|e| panic!("capture {name}: {e}"))
    };
    let ring_d = shot(&mut renderer, "ring_d");
    let ring_r = shot(&mut renderer, "ring_r");
    let disc_r = shot(&mut renderer, "disc_r");

    let differing = |a: &CaptureImage, b: &CaptureImage| -> usize {
        a.rgba
            .chunks_exact(4)
            .zip(b.rgba.chunks_exact(4))
            .filter(|(x, y)| x[..3] != y[..3])
            .count()
    };
    let total = (SIZE * SIZE) as usize;
    let same_mode = differing(&ring_d, &ring_r);
    let vs_disc = differing(&ring_r, &disc_r);
    println!(
        "ring: {same_mode} of {total} pixels differ between the modes; \
         {vs_disc} differ from a disc under the radius mode"
    );

    assert_eq!(
        same_mode, 0,
        "a ring must render the SAME figure under either mode — the scene \
         refuses the scaled-copy coordinate there and hands the palette the \
         distance"
    );
    // The control, and the reason the refusal exists: under the rejected
    // outer-rim definition this number is 0 and the one above is not.
    assert!(
        vs_disc * 20 > total,
        "the ring renders like a disc ({vs_disc} of {total} pixels differ) — \
         that is exactly the collapse Phase 4 refused, where the hole stops \
         existing and a preset is shown a shape it did not name"
    );
}

// --- Phase 4b: the figure can turn ---------------------------------------------
//
// This scene had no rotation lever at all, while every other figure-drawing
// scene has one — `lines/star.rs` and `lines/lsystem.rs` carry `rotation`,
// `lines/parametric.rs` carries `spin`. So a star here could breathe, drift and
// morph, and could not turn, which for a star is the most obvious motion there
// is.

/// `rotation` is passed through, and **0 is an exact identity** — the shader
/// tests for it and never enters `cos`/`sin`, which is what keeps every shipped
/// preset and every golden baseline on the arithmetic it has today.
///
/// The finiteness guard is the load-bearing half of the rest: `cos(NaN)` is
/// `NaN`, and one `NaN` in the figure's own frame takes every pixel with it.
#[test]
fn the_rotation_passes_through_and_zero_is_the_identity() {
    assert_eq!(
        applied_rotation(DEFAULT_ROTATION).to_bits(),
        0.0f32.to_bits()
    );
    for v in [-9.0f32, -0.25, 0.5, 1.0, std::f32::consts::TAU, 400.0] {
        assert_eq!(
            applied_rotation(v).to_bits(),
            v.to_bits(),
            "an angle wraps, so `rotation` is unclamped: {v}"
        );
    }
    assert_eq!(applied_rotation(f32::NAN), DEFAULT_ROTATION);
    assert_eq!(applied_rotation(f32::INFINITY), DEFAULT_ROTATION);
    assert_eq!(applied_rotation(f32::NEG_INFINITY), DEFAULT_ROTATION);
    assert!(declares(PARAMS, "rotation"));
}

/// **A quarter turn does not shear the figure at a 2:1 target**
/// (ADR-0037).
///
/// The rotation happens in `uv`, which has already had x stretched by the
/// **render target's** aspect — so one unit is the same length on both axes and
/// the two lines are a rotation. Done in raw NDC the identical two lines would
/// **shear**, and this is the exact configuration where that is invisible at
/// 16:9 (the stretch is nearly 1 there) and glaring at 2:1.
///
/// The subject is a **square**, because a square is carried onto itself by a
/// quarter turn: its extent is unchanged, and so is every pixel. A sheared
/// square is a rectangle, and at 2:1 it is a 4:1 one.
///
/// The non-vacuity is the eighth turn, in the same run: 45 degrees genuinely
/// moves the picture, so "the frames match" is a statement about the quarter
/// turn rather than about a parameter that never reached the shader.
#[test]
fn a_quarter_turn_neither_shears_nor_resizes_the_figure() {
    use crate::render::metrics::frame_diff;

    const W: u32 = 240;
    const H: u32 = 120;
    let Some(mut renderer) = headless(W, H) else {
        return;
    };
    let square = |name: &str, rotation: &str| {
        preset(
            name,
            &format!(
                "shape = \"2\"\npoints = \"4\"\nscale = \"0.45\"\ncolor_span = \"1\"\n\
                 palette_steps = \"2\"\nrotation = \"{rotation}\"\n"
            ),
        )
    };
    renderer.set_presets(vec![
        square("rot0", "0"),
        square("rot90", "1.5707963"),
        square("rot45", "0.7853982"),
    ]);

    // Half-extent of the figure from the centre, across and down, in pixels —
    // the same probe the aspect test uses.
    let extent = |img: &CaptureImage| -> (u32, u32) {
        let (cx, cy) = (W / 2, H / 2);
        let centre = luma(img, cx, cy);
        let differs = |v: f32| (v - centre).abs() > 8.0;
        let mut half_w = 0u32;
        while cx + half_w + 1 < W && !differs(luma(img, cx + half_w + 1, cy)) {
            half_w += 1;
        }
        let mut half_h = 0u32;
        while cy + half_h + 1 < H && !differs(luma(img, cx, cy + half_h + 1)) {
            half_h += 1;
        }
        (half_w, half_h)
    };

    let shot = |renderer: &mut Renderer, name: &str| {
        renderer
            .capture_preset(name, &AnalysisFrame::default(), 2)
            .unwrap_or_else(|e| panic!("capture {name}: {e}"))
    };
    let rot0 = shot(&mut renderer, "rot0");
    let rot90 = shot(&mut renderer, "rot90");
    let rot45 = shot(&mut renderer, "rot45");

    let (w0, h0) = extent(&rot0);
    let (w90, h90) = extent(&rot90);
    let (w45, h45) = extent(&rot45);
    let quarter = frame_diff(&rot0, &rot90);
    let eighth = frame_diff(&rot0, &rot45);
    println!(
        "square at {W}x{H}: extent {w0}x{h0} at 0, {w90}x{h90} at a quarter turn, \
         {w45}x{h45} at an eighth; frame_diff {quarter:.5} / {eighth:.5}"
    );

    assert!(
        w0 > 4 && h0 > 4,
        "the figure has no measurable extent ({w0} x {h0}) — this test is \
         measuring nothing"
    );
    // The claim: a quarter turn carries a square onto itself, in square units.
    assert!(
        w0.abs_diff(w90) <= 1 && h0.abs_diff(h90) <= 1,
        "a quarter turn changed the square's extent from {w0}x{h0} to {w90}x{h90} \
         — a rotation applied in a frame whose x has been stretched by the aspect \
         SHEARS rather than rotates, and at 2:1 it does so by a factor of two"
    );
    assert!(
        quarter < 0.02,
        "the quarter-turn frame differs from the unrotated one by {quarter:.5} — a \
         square is carried onto itself by 90 degrees, so the picture must not move"
    );
    // The control: the parameter really does reach the shader.
    assert!(
        eighth > 10.0 * quarter.max(1e-6),
        "an eighth turn moved the picture {eighth:.5} against the quarter turn's \
         {quarter:.5} — if these are comparable, `rotation` never reached the \
         shader and the assertion above is about nothing"
    );
}

/// **The figure spins in place rather than orbiting the frame centre** — a
/// stated choice, asserted where it is visible.
///
/// `rotation` is applied to `uv - pan`, i.e. **after** the pan. Applied before
/// it, the same two lines would swing the figure around the frame's middle on a
/// circle of radius `|pan|`. Both are defensible and they look completely
/// different, so this pins which one shipped.
///
/// Measured as the position of the figure's own centre, which is where the
/// coordinate is `0` and therefore the darkest point of a dark-to-light palette.
/// With `pan_x` well off centre, an orbit moves it by hundreds of pixels.
#[test]
fn the_figure_turns_about_its_own_centre_and_does_not_orbit() {
    const SIZE: u32 = 240;
    const PAN_X: f32 = 0.5;
    let Some(mut renderer) = headless(SIZE, SIZE) else {
        return;
    };
    // A heart, so the turn is unmistakable, and a monotone palette so the
    // darkest pixels are the figure's middle.
    //
    // **`color_span` is small on purpose.** The coordinate keeps growing outside
    // the figure and the LUT repeat-addresses, so at the default span the frame
    // corners wrap past 1 and come back as BLACK — darker than the figure's own
    // centre, which would put this centroid on a wrap ring rather than on the
    // figure. At 0.08 the whole frame stays inside 0.1..0.82 and never wraps.
    let turned = |name: &str, rotation: &str| -> Preset {
        let toml = format!(
            "name = \"{name}\"\nsystem = \"shape_field\"\n\
             [palette]\nstops = [\n\
             {{ at = 0.0, color = \"#000000\" }},\n\
             {{ at = 1.0, color = \"#ffffff\" }},\n]\n\
             [params]\nshape = \"4\"\nscale = \"0.3\"\ncolor_span = \"0.08\"\n\
             color_center = \"0.1\"\npan_x = \"{PAN_X}\"\nrotation = \"{rotation}\"\n"
        );
        Preset::from_toml_str(&toml).unwrap_or_else(|e| panic!("{name}: {e}"))
    };
    let angles = ["0", "0.9", "2.4", "-1.7"];
    renderer.set_presets(
        angles
            .iter()
            .enumerate()
            .map(|(i, a)| turned(&format!("r{i}"), a))
            .collect(),
    );

    // The centroid of the darkest region, weighted by how dark it is.
    let dark_centroid = |img: &CaptureImage| -> (f32, f32) {
        let mut lo = f32::INFINITY;
        for y in 0..SIZE {
            for x in 0..SIZE {
                lo = lo.min(luma(img, x, y));
            }
        }
        let cut = lo + 6.0;
        let (mut sx, mut sy, mut sw) = (0.0f32, 0.0f32, 0.0f32);
        for y in 0..SIZE {
            for x in 0..SIZE {
                let v = luma(img, x, y);
                if v <= cut {
                    let w = cut - v + 1.0;
                    sx += w * x as f32;
                    sy += w * y as f32;
                    sw += w;
                }
            }
        }
        (sx / sw, sy / sw)
    };

    // Where the figure's centre must be: `pan` in square units, and the target
    // is square so `uv` is NDC.
    let want_x = (PAN_X * 0.5 + 0.5) * SIZE as f32;
    let want_y = 0.5 * SIZE as f32;

    let mut frames = Vec::new();
    for (i, a) in angles.iter().enumerate() {
        let img = renderer
            .capture_preset(&format!("r{i}"), &AnalysisFrame::default(), 2)
            .unwrap_or_else(|e| panic!("capture r{i}: {e}"));
        let (cx, cy) = dark_centroid(&img);
        println!(
            "rotation {a:>5}: figure centre at ({cx:.1}, {cy:.1}) px, want ({want_x:.1}, {want_y:.1})"
        );
        assert!(
            (cx - want_x).abs() < 6.0 && (cy - want_y).abs() < 6.0,
            "at rotation {a} the figure's centre sits at ({cx:.1}, {cy:.1}) rather \
             than at ({want_x:.1}, {want_y:.1}). `rotation` is applied AFTER the \
             pan so the figure spins in place; applied before it, the figure \
             orbits the frame centre on a circle of radius |pan|"
        );
        frames.push(img);
    }

    // ...and it really did turn: an asymmetric figure at four angles is four
    // pictures, or `rotation` reached nothing.
    use crate::render::metrics::frame_diff;
    for (i, a) in frames.iter().enumerate() {
        for (j, b) in frames.iter().enumerate().skip(i + 1) {
            let diff = frame_diff(a, b);
            assert!(
                diff > 0.01,
                "rotations {} and {} render indistinguishably (diff {diff:.5})",
                angles[i],
                angles[j]
            );
        }
    }
}

// -----------------------------------------------------------------------
// The authored contour (Plan 0092 Phase 2 / ADR-0107)
// -----------------------------------------------------------------------

/// A leaf: two cubics meeting at a cusp top and bottom. **No combination of
/// `shape` and `points` draws it** — the roster's arms are a disc, a ring, a
/// polygon, a star and a heart — which is what makes it the right figure to
/// assert on. Its bounding box is 1.35 wide by 2 tall, so after normalization it
/// spans the full `[-1, 1]` in y and ±0.675 in x.
const LEAF: &str = "M 0,-1 C 0.9,-0.4 0.9,0.4 0,1 C -0.9,0.4 -0.9,-0.4 0,-1 Z";

/// The leaf's own width as a fraction of its height, from the path data: the
/// cubic's extreme is at `t = 0.5`, where `x = 3/8 * (0.9 + 0.9)`.
const LEAF_ASPECT: f32 = 0.675;

/// The two-band look every path test here renders through, spliced into both
/// the `[path]` and roster presets so the two are comparable.
///
/// **The band boundary sits exactly on the outline**: `color_span = 1/16` maps
/// the coordinate's `1` — the outline, by the scene's contract — onto `0.0625`,
/// and `palette_steps = 16` puts a band edge there. So the interior and the
/// exterior are two flat colours with a hard seam on the figure itself, and the
/// seam is *exact* rather than a ramp — which is what lets a walk out from the
/// centre measure the figure's own extent.
///
/// **The span is small because the palette LUT repeat-addresses.** The
/// coordinate keeps growing outward — past `d = 5` in this frame's corners — so
/// a span that let `coord` reach 1 anywhere would wrap the LUT and paint the far
/// exterior in the interior's colour, putting a second seam out near the frame
/// edge. At `1/16` the whole frame stays inside one wrap.
const TWO_BAND_PARAMS: &str =
    "scale = \"0.55\"\ncolor_span = \"0.0625\"\ncolor_center = \"0.0\"\npalette_steps = \"16\"\n";

/// The palette those two bands read from, **flat above the seam on purpose**.
/// Sixteen bands means fifteen of them sit outside the figure; holding the stops
/// constant across everything above the first makes them all one colour, so the
/// only seam in the frame is the figure's own outline.
const TWO_BAND_PALETTE: &str = "[palette]\nstops = [\
    { at = 0.0, color = \"#8a8ac0\" }, { at = 0.06, color = \"#8a8ac0\" }, \
    { at = 0.07, color = \"#ffffff\" }, { at = 1.0, color = \"#ffffff\" }]\n";

/// A `shape_field` preset with a `[path]` table, under the two-band look above.
fn path_preset(name: &str, d: &str, extra: &str) -> Preset {
    let toml = format!(
        "name = \"{name}\"\nsystem = \"shape_field\"\n\
         [path]\nd = \"{d}\"\n{TWO_BAND_PALETTE}\
         [params]\n{TWO_BAND_PARAMS}{extra}"
    );
    Preset::from_toml_str(&toml).unwrap_or_else(|e| panic!("{name} failed to load: {e}"))
}

/// A morphing `[path]` preset pinned at one point along its travel.
fn morph_preset(name: &str, from: &str, to: &str, morph: f32) -> Preset {
    let toml = format!(
        "name = \"{name}\"\nsystem = \"shape_field\"\n\
         [path]\nd = \"{from}\"\nmorph_to = \"{to}\"\n{TWO_BAND_PALETTE}\
         [params]\n{TWO_BAND_PARAMS}morph = \"{morph}\"\n"
    );
    Preset::from_toml_str(&toml).unwrap_or_else(|e| panic!("{name} failed to load: {e}"))
}

/// The same look with no `[path]` table, so the scene draws its rostered
/// silhouette instead.
fn roster_preset(name: &str) -> Preset {
    let toml = format!(
        "name = \"{name}\"\nsystem = \"shape_field\"\n{TWO_BAND_PALETTE}\
         [params]\n{TWO_BAND_PARAMS}"
    );
    Preset::from_toml_str(&toml).unwrap_or_else(|e| panic!("{name} failed to load: {e}"))
}

/// **An authored contour reaches the frame, and it is a figure the roster
/// cannot select** (ADR-0107).
///
/// Two claims in one render, because either alone would pass while the feature
/// did nothing: the path preset must differ from the same preset without a
/// `[path]` table (or the table was ignored), and the figure it draws must have
/// the **leaf's** proportions rather than the disc's (or something drew a
/// fallback shape that merely happens to differ).
#[test]
fn an_authored_contour_draws_a_figure_the_roster_cannot_select() {
    const SIZE: u32 = 320;
    let Some(mut renderer) = headless(SIZE, SIZE) else {
        return;
    };
    renderer.set_presets(vec![path_preset("leaf", LEAF, ""), roster_preset("roster")]);
    let leaf = renderer
        .capture_preset("leaf", &AnalysisFrame::default(), 2)
        .expect("capture the authored contour");
    let roster = renderer
        .capture_preset("roster", &AnalysisFrame::default(), 2)
        .expect("capture the roster default");

    let differing = leaf
        .rgba
        .chunks_exact(4)
        .zip(roster.rgba.chunks_exact(4))
        .filter(|(a, b)| a[..3] != b[..3])
        .count();
    let total = (SIZE * SIZE) as usize;
    assert!(
        differing * 20 > total,
        "the `[path]` preset drew the roster's figure: only {differing} of \
         {total} pixels differ, so the table reached nothing"
    );

    // The figure's own extent, walked out from the centre to the band seam —
    // the same measurement `the_figure_is_round_at_a_non_sixteen_by_nine_target`
    // makes, and it works here for the same reason: the seam is a real edge.
    let (cx, cy) = (SIZE / 2, SIZE / 2);
    let centre = luma(&leaf, cx, cy);
    let differs = |v: f32| (v - centre).abs() > 8.0;
    let mut half_w = 0u32;
    while cx + half_w + 1 < SIZE && !differs(luma(&leaf, cx + half_w + 1, cy)) {
        half_w += 1;
    }
    let mut half_h = 0u32;
    while cy + half_h + 1 < SIZE && !differs(luma(&leaf, cx, cy + half_h + 1)) {
        half_h += 1;
    }
    let ratio = half_w as f32 / half_h as f32;
    println!("leaf extent {half_w} x {half_h} px, ratio {ratio:.3}, want {LEAF_ASPECT:.3}");
    assert!(
        (ratio - LEAF_ASPECT).abs() < 0.08,
        "the figure is {half_w} x {half_h} px (ratio {ratio:.3}), not the leaf's \
         {LEAF_ASPECT:.3}. A disc or any rostered arm at this scale would read 1.0"
    );
}

/// **Fill and stroke come from one field, so the stroke lies on the fill's own
/// boundary** (ADR-0107).
///
/// The claim a second rendering route could not make. Two captures differ only
/// in `stroke`; the filled one gives the interior/exterior partition and the
/// stroked one gives the lit band, and the property is asserted **both ways** —
/// every lit pixel sits near the partition's seam, and every seam pixel has a
/// lit pixel near it. One direction alone is satisfied by a stroke that draws
/// nothing, or by one that lights the whole frame.
///
/// **Confirmed to bite.** Testing the band at `abs(d - 0.6)` instead — a stroke
/// on the same field but not on the fill's boundary — leaves 81 of 3285 lit
/// pixels more than 22 px from the seam and fails here.
#[test]
fn the_stroke_lies_on_the_fill_boundary_because_both_come_from_one_field() {
    const SIZE: u32 = 320;
    /// Coordinate half-width of the stroke. The coordinate runs 0 at the
    /// figure's centre to 1 on its outline, so this is a band of about 8 % of
    /// the figure's own half-extent either side of the outline.
    const STROKE: &str = "0.08";
    /// How far a lit pixel may be from the seam, in pixels. The figure's
    /// interior spans about 88 px here (`scale` 0.55 of a 320 px half-frame), so
    /// a 0.08 coordinate band is roughly 7 px and this is a generous bound on
    /// it rather than a tight fit.
    const NEAR_SEAM: i32 = 22;
    /// How far a seam pixel may be from the nearest lit one. The stroke covers
    /// the seam, so this is small on purpose: a stroke drawn somewhere else
    /// entirely would fail here while passing the other direction.
    const COVERED: i32 = 3;

    let Some(mut renderer) = headless(SIZE, SIZE) else {
        return;
    };
    renderer.set_presets(vec![
        path_preset("filled", LEAF, ""),
        path_preset("stroked", LEAF, &format!("stroke = \"{STROKE}\"\n")),
    ]);
    let filled = renderer
        .capture_preset("filled", &AnalysisFrame::default(), 2)
        .expect("capture the filled figure");
    let stroked = renderer
        .capture_preset("stroked", &AnalysisFrame::default(), 2)
        .expect("capture the stroked figure");

    // The two band colours, read off the image rather than predicted from the
    // palette: the centre pixel is interior and the corner is exterior by
    // construction, whatever the LUT bake did to the stops.
    let inside_luma = luma(&filled, SIZE / 2, SIZE / 2);
    let outside_luma = luma(&filled, 2, 2);
    assert!(
        (inside_luma - outside_luma).abs() > 20.0,
        "the two bands are not distinguishable ({inside_luma:.1} inside, \
         {outside_luma:.1} outside), so this test cannot see a seam"
    );
    assert!(
        inside_luma.min(outside_luma) > 40.0,
        "a band is too dark to tell from the stroke's masked-out black \
         ({inside_luma:.1}, {outside_luma:.1})"
    );

    let is_fill = |x: u32, y: u32| {
        let v = luma(&filled, x, y);
        (v - inside_luma).abs() < (v - outside_luma).abs()
    };
    let is_lit = |x: u32, y: u32| luma(&stroked, x, y) > 25.0;

    // The seam: a filled pixel with a neighbour on the other side of it.
    let mut seam = Vec::new();
    for y in 1..SIZE - 1 {
        for x in 1..SIZE - 1 {
            let here = is_fill(x, y);
            if here != is_fill(x + 1, y) || here != is_fill(x, y + 1) {
                seam.push((x, y));
            }
        }
    }
    let lit: Vec<(u32, u32)> = (0..SIZE)
        .flat_map(|y| (0..SIZE).map(move |x| (x, y)))
        .filter(|&(x, y)| is_lit(x, y))
        .collect();
    println!(
        "{} seam px, {} lit px of {} total",
        seam.len(),
        lit.len(),
        SIZE * SIZE
    );

    // Non-vacuity on both sides before the property itself.
    assert!(
        seam.len() > 200,
        "the filled capture has no seam to be on ({} px)",
        seam.len()
    );
    let total = (SIZE * SIZE) as usize;
    assert!(
        !lit.is_empty() && lit.len() * 4 < total,
        "the stroke lit {} of {total} pixels, which is either nothing or the \
         whole frame — neither is an outline",
        lit.len()
    );

    // A `near` test that walks a window rather than every pair: the frame is
    // 320x320 and the two sets are thousands of pixels each, so the quadratic
    // form would be minutes.
    let within = |set: &[(u32, u32)], x: u32, y: u32, r: i32| -> bool {
        set.iter()
            .any(|&(sx, sy)| (sx as i32 - x as i32).abs() <= r && (sy as i32 - y as i32).abs() <= r)
    };
    let mut grid_seam = vec![false; total];
    for &(x, y) in &seam {
        if let Some(slot) = grid_seam.get_mut((y * SIZE + x) as usize) {
            *slot = true;
        }
    }
    let near_seam = |x: u32, y: u32, r: i32| -> bool {
        for dy in -r..=r {
            for dx in -r..=r {
                let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                if nx < 0 || ny < 0 || nx >= SIZE as i32 || ny >= SIZE as i32 {
                    continue;
                }
                if grid_seam
                    .get((ny as u32 * SIZE + nx as u32) as usize)
                    .copied()
                    .unwrap_or(false)
                {
                    return true;
                }
            }
        }
        false
    };

    // Direction one: every lit pixel is on the fill's boundary.
    let stray: Vec<(u32, u32)> = lit
        .iter()
        .copied()
        .filter(|&(x, y)| !near_seam(x, y, NEAR_SEAM))
        .collect();
    assert!(
        stray.is_empty(),
        "{} of {} lit pixels are more than {NEAR_SEAM} px from the fill's own \
         boundary (first at {:?}). The stroke is `abs(d - 1) < w` on the SAME \
         `d` the fill tests, so a lit pixel away from the seam means the two \
         are reading different fields",
        stray.len(),
        lit.len(),
        stray.first()
    );

    // Direction two: the whole boundary is stroked, so the outline is closed.
    let uncovered: Vec<(u32, u32)> = seam
        .iter()
        .copied()
        .filter(|&(x, y)| !within(&lit, x, y, COVERED))
        .collect();
    assert!(
        uncovered.is_empty(),
        "{} of {} seam pixels have no lit pixel within {COVERED} px (first at \
         {:?}), so the stroke does not close around the fill",
        uncovered.len(),
        seam.len(),
        uncovered.first()
    );
}

/// **A `shape_field` preset that declares no `[path]` clears the outgoing
/// preset's contour**, which is why the loader hands this scene a config on
/// every switch rather than only when a table is present.
///
/// Rendered rather than reasoned about: the failure is a *stale* contour, and
/// nothing about the second preset's own state can see it.
#[test]
fn switching_away_from_a_path_preset_clears_the_contour() {
    const SIZE: u32 = 256;
    let Some(mut renderer) = headless(SIZE, SIZE) else {
        return;
    };
    renderer.set_presets(vec![
        roster_preset("roster_first"),
        path_preset("leaf", LEAF, ""),
        roster_preset("roster_after"),
    ]);

    let first = renderer
        .capture_preset("roster_first", &AnalysisFrame::default(), 2)
        .expect("capture the roster before any path");
    let _leaf = renderer
        .capture_preset("leaf", &AnalysisFrame::default(), 2)
        .expect("capture the path");
    let after = renderer
        .capture_preset("roster_after", &AnalysisFrame::default(), 2)
        .expect("capture the roster after the path");

    let differing = first
        .rgba
        .chunks_exact(4)
        .zip(after.rgba.chunks_exact(4))
        .filter(|(a, b)| a[..3] != b[..3])
        .count();
    assert_eq!(
        differing, 0,
        "the same roster preset rendered differently before and after a path \
         preset ({differing} px), so the contour survived the switch"
    );
}

/// The other silhouettes the morph sweep travels between. Chosen so the four
/// pairs below each price a different alignment hazard rather than four
/// variations of one.
const SQUARE: &str = "M -1,-1 L 1,-1 L 1,1 L -1,1 Z";
const TRIANGLE: &str = "M 0,-1 L 0.866,0.5 L -0.866,0.5 Z";
const STAR5: &str = "M 0,-1 L 0.2245,-0.309 L 0.9511,-0.309 L 0.3633,0.118 \
                     L 0.5878,0.809 L 0,0.382 L -0.5878,0.809 L -0.3633,0.118 \
                     L -0.9511,-0.309 L -0.2245,-0.309 Z";
/// The leaf turned a quarter turn — the same outline, so any change in the
/// figure's area across this morph is the correspondence and nothing else.
const LEAF_TURNED: &str = "M -1,0 C -0.4,0.9 0.4,0.9 1,0 C 0.4,-0.9 -0.4,-0.9 -1,0 Z";

/// **Mid-morph states are inspected, not assumed** (ADR-0107).
///
/// Plan 0079 swept twenty tuple pairs and refused *four* by measurement, because
/// their intermediate states collapsed to zero extent; ADR-0075 exists because
/// naive interpolation of the obvious representation was wrong. There is no
/// reason to expect this feature to be the one that escapes that class, so this
/// renders a strip across each pair and **reports the figure's own area at every
/// step**.
///
/// A collapse is a finding to write down rather than a bug to tune away, so the
/// gate is deliberately loose — a quarter of the smaller endpoint. What it
/// catches is the failure mode that class actually has: an intermediate frame
/// with nothing in it.
#[test]
fn the_morph_strip_is_rendered_and_its_extent_recorded() {
    const SIZE: u32 = 200;
    /// The strip: both endpoints and three steps between them.
    const STEPS: [f32; 5] = [0.0, 0.25, 0.5, 0.75, 1.0];
    const PAIRS: [(&str, &str, &str); 4] = [
        ("square -> leaf   ", SQUARE, LEAF),
        ("leaf -> star(5)  ", LEAF, STAR5),
        ("triangle -> squar", TRIANGLE, SQUARE),
        ("leaf -> leaf turn", LEAF, LEAF_TURNED),
    ];

    let Some(mut renderer) = headless(SIZE, SIZE) else {
        return;
    };
    let mut presets = Vec::new();
    for (index, (_, from, to)) in PAIRS.iter().enumerate() {
        for (step, t) in STEPS.iter().enumerate() {
            presets.push(morph_preset(&format!("m{index}_{step}"), from, to, *t));
        }
    }
    renderer.set_presets(presets);

    let mut report = String::from("shape_field morph strip: figure area in px, across the travel");
    report.push_str(&format!("\n  {:<20}", "pair"));
    for t in STEPS {
        report.push_str(&format!("{:>10}", format!("t={t}")));
    }

    let mut findings: Vec<String> = Vec::new();
    for (index, (label, _, _)) in PAIRS.iter().enumerate() {
        let mut areas = Vec::new();
        for step in 0..STEPS.len() {
            let name = format!("m{index}_{step}");
            let img = renderer
                .capture_preset(&name, &AnalysisFrame::default(), 2)
                .unwrap_or_else(|e| panic!("capture {name}: {e}"));
            // The interior's own pixel count: the centre is inside every figure
            // here and the corner is outside every one, so "closer to the centre
            // than to the corner" is the fill without knowing the palette bake.
            let inside = luma(&img, SIZE / 2, SIZE / 2);
            let outside = luma(&img, 2, 2);
            let count = (0..SIZE)
                .flat_map(|y| (0..SIZE).map(move |x| (x, y)))
                .filter(|&(x, y)| {
                    let v = luma(&img, x, y);
                    (v - inside).abs() < (v - outside).abs()
                })
                .count();
            areas.push(count);
        }
        report.push_str(&format!("\n  {label:<20}"));
        for a in &areas {
            report.push_str(&format!("{a:>10}"));
        }

        let ends = areas
            .first()
            .copied()
            .unwrap_or(0)
            .min(areas.last().copied().unwrap_or(0));
        let dip = areas.iter().copied().min().unwrap_or(0);
        if dip * 4 < ends {
            findings.push(format!(
                "{}: dips to {dip} px against endpoints of {ends}",
                label.trim()
            ));
        }
    }
    println!("{report}");

    assert!(
        findings.is_empty(),
        "a morph pair collapses through the middle:\n  {}\n\nThat is ADR-0075's \
         class, and the recorded response is to change the interpolated \
         REPRESENTATION rather than to tune the endpoints until one still frame \
         looks acceptable",
        findings.join("\n  ")
    );
}

/// `morph` is an ordinary bindable param under the existing grammar, and
/// `[smoothing]` reaches it like any other — there is no second vocabulary for
/// travelling between two silhouettes.
#[test]
fn morph_is_an_ordinary_binding_that_smoothing_reaches() {
    assert!(declares(PARAMS, "morph"));
    let toml = format!(
        "name = \"m\"\nsystem = \"shape_field\"\n\
         [path]\nd = \"{LEAF}\"\nmorph_to = \"{SQUARE}\"\n\
         [params]\nmorph = \"beat\"\n[smoothing]\nmorph = 0.35\n"
    );
    let preset = Preset::from_toml_str(&toml).expect("a bound, smoothed morph loads");
    assert!(
        preset.warnings.is_empty(),
        "binding morph should not warn, got {:?}",
        preset.warnings
    );
    let binding = preset
        .params
        .iter()
        .find(|b| b.name == "morph")
        .expect("the morph binding survives the load");
    assert!(
        binding.tau.attack > 0.0,
        "the [smoothing] entry should have folded into the binding, got {:?}",
        binding.tau
    );
}

/// **The two adapters agree on the authored-contour fixture** — the ADR-0058
/// standing rule, run before its golden baseline is blessed.
///
/// The whole golden suite captures on the DX12 WARP software adapter, and two
/// bind-group layouts of one shape alias there: a pass is handed another pass's
/// resources, and a mis-render is blessed rather than caught. This scene's
/// layout is unchanged by the `[path]` work — the contour rides the existing
/// uniform precisely so it stays a shape nothing else holds — so this is
/// confirming that rather than discovering it.
///
/// Ignored by default and skipped on a one-adapter machine, which is the CI
/// case; the recorded reading is what the fixture's own comment points at.
///
/// **Measured on the development box (Windows 10, DX12), 160x160 over 2 frames,
/// before the baseline was blessed: hardware mean rgb
/// `102.535 150.984 102.304`, WARP `102.573 150.998 102.286`, `frame_diff`
/// `0.000231`.** Agreement to well under one 8-bit level.
#[test]
#[ignore = "needs both a hardware and a software adapter; run locally before blessing"]
fn the_adapters_agree_on_the_authored_contour() {
    const SIZE: u32 = 160;
    let build = |prefer_software: bool| -> Option<Renderer> {
        match Renderer::new_headless(HeadlessOptions {
            width: SIZE,
            height: SIZE,
            prefer_software,
        }) {
            Ok(r) => Some(r),
            Err(RenderError::RequestAdapter(_)) => None,
            Err(e) => panic!("headless renderer build failed: {e}"),
        }
    };
    let (Some(mut hardware), Some(mut software)) = (build(false), build(true)) else {
        eprintln!("skipped: this machine does not expose both adapters");
        return;
    };

    let capture = |renderer: &mut Renderer| -> CaptureImage {
        let preset = Preset::from_toml_str(include_str!(
            "../../../../tests/fixtures/shape_field_path.toml"
        ))
        .expect("the path fixture parses");
        let name = preset.name.clone();
        renderer.set_presets(vec![preset]);
        renderer
            .capture_preset(&name, &AnalysisFrame::default(), 2)
            .expect("capture the path fixture")
    };
    let hw = capture(&mut hardware);
    let sw = capture(&mut software);

    let mean = |img: &CaptureImage| -> [f64; 3] {
        let mut sums = [0f64; 3];
        for px in img.rgba.chunks_exact(4) {
            for (sum, c) in sums.iter_mut().zip(px) {
                *sum += f64::from(*c);
            }
        }
        let n = (img.rgba.len() / 4) as f64;
        [sums[0] / n, sums[1] / n, sums[2] / n]
    };
    let difference = crate::render::metrics::frame_diff(&hw, &sw);
    println!(
        "[shape_field_path] hardware mean rgb {:?}, WARP mean rgb {:?}, frame_diff {difference:.6}",
        mean(&hw),
        mean(&sw)
    );
    assert!(
        difference < 0.05,
        "the two adapters disagree by {difference:.4} on the authored-contour \
         fixture. That is the shape of an ADR-0058 layout collision — check \
         `shape-field-bind-layout` against the crate's enumeration before \
         blessing anything"
    );
}
