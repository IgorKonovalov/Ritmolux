//! `shape_field`'s own contract (Plan 0091 Phase 3).
//!
//! Two of these are rendered rather than arithmetic, because the two claims
//! that matter are about what reaches the frame: that the aspect comes from the
//! render target, and that banding the coordinate draws **offsets of the shape**
//! rather than concentric circles. Both are wired-up claims, and both would pass
//! a CPU-side test of the arithmetic while the shader read the wrong thing.

#![allow(clippy::indexing_slicing, clippy::panic, clippy::expect_used)]

use super::{DEFAULT_SCALE, MAX_SCALE, MIN_SCALE, PARAMS, applied_scale};
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
/// ([ADR-0037](../../../../../docs/adrs/0037-internal-grid-is-a-resolution-not-a-shape.md)).
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
