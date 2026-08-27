//! `shape_field`'s own contract (Plan 0091 Phase 3).
//!
//! Two of these are rendered rather than arithmetic, because the two claims
//! that matter are about what reaches the frame: that the aspect comes from the
//! render target, and that banding the coordinate draws **offsets of the shape**
//! rather than concentric circles. Both are wired-up claims, and both would pass
//! a CPU-side test of the arithmetic while the shader read the wrong thing.

#![allow(clippy::indexing_slicing, clippy::panic, clippy::expect_used)]

use super::{
    DEFAULT_GAMMA, DEFAULT_SCALE, MAX_GAMMA, MAX_SCALE, MIN_GAMMA, MIN_SCALE, PARAMS,
    applied_gamma, applied_scale, coord,
};
use crate::dsp::AnalysisFrame;
use crate::preset::Preset;
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
    assert!(PARAMS.contains(&"shape"));
    assert!(PARAMS.contains(&"points"));
    assert!(PARAMS.contains(&"scale"));
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
                  { at = 0.0, color = \"#101030\" },\n\
                  { at = 0.5, color = \"#ff6040\" },\n\
                  { at = 1.0, color = \"#101030\" },\n]\n";
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
