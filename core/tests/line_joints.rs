//! Line joins at the pixel level (Plan 0039 Phase 2, ADR-0041; the baseline
//! added by Plan 0040 Phase 1).
//!
//! The unit tests beside the producers assert the **flag pattern** — which
//! endpoints a producer declares as joined. This file asserts the thing no unit
//! test can: that a flagged joint actually stops leaving a hole in the rendered
//! stroke.
//!
//! The claim is deliberately threshold-free. A joinless stroke leaves a wedge on
//! the outside of every turn, so a point just outside the vertex falls in empty
//! space while the same offset over a segment's interior falls inside the
//! stroke. That makes the joint a **local luminance minimum** along an
//! outward-offset path — and "is not a local minimum" is a property, not a tuned
//! constant. It was verified to fail against Phase 1's code (flags implemented,
//! no producer flagging) before the polyline began flagging its joints.
//!
//! Software adapter (`prefer_software`) so it holds on any CI GPU.
//!
//! # Three duties, one capture, in this order
//!
//! Plan 0040 Phase 2 added the middle one: the figure's two **outer** ends are
//! free, so the stroke must not reach past them. That is the only place a swap
//! of `JOINED_A` and `JOINED_B` is visible — an interior joint carries both bits
//! and renders identically either way — and it is why the pair is now generated
//! into the shader from the Rust constants rather than restated as `1u`/`2u`.
//!
//! Plan 0040 Phase 1 added a **committed baseline** beside the relative claim,
//! because the reported defect — the polyline's notch — was pinned by no pixels
//! anywhere: `fixtures/spectrum.toml` takes the default `bars` layout, and
//! `spectrum_ridge` is a shipped preset guarded behaviorally (ADR-0023). A
//! shader edit could reopen the notch on a gentler figure than this deliberately
//! hostile zigzag and move no file.
//!
//! The two are **not** redundant, and neither replaces the other. The baseline
//! catches a silent change; the relative assertion says *why* the change
//! matters. So the relative assertion runs **first, including under `LMV_BLESS`**
//! — the notch therefore cannot be blessed back in by someone who reads the diff
//! as drift, because the bless never runs. That ordering is the whole answer to
//! "a baseline can always be re-blessed".
//!
//! They share **one** capture. A second `Renderer::new_headless` in this binary
//! would be a second GPU resource build mid-run, which `composite.rs` documents
//! as changing what the software adapter resolves — a risk taken for nothing
//! when the pixels the pin wants are the pixels the probe already measured.
//!
//! The pin follows `composite.rs` rather than joining `golden.rs`: that roster is
//! one fixture per `SystemKind`, enforced by `systems_rosters_every_variant`, and
//! a second `spectrum` entry would break the invariant ADR-0023 rests on. Its own
//! binary also means `LMV_BLESS=1 cargo test -p lmv-core --test line_joints`
//! rewrites **this** baseline and cannot reach the roster.

use lmv_core::dsp::{AnalysisFrame, SPECTRUM_BINS};
use lmv_core::preset::Preset;
use lmv_core::render::{CaptureImage, metrics::frame_diff};

mod common;

/// Capture size. Larger than the other suites' 96–128 on purpose: the feature
/// under test is a wedge a fraction of a stroke-width across, and at 128 px a
/// `thickness = 24` stroke is under 5 px wide with nothing left to measure.
/// Square, so the renderer's aspect divide is the identity and world
/// coordinates *are* NDC.
const SIZE: u32 = 512;
/// The fixture binds no `time` term and takes no `[spectrum] smoothing`, so its
/// geometry is settled on the first frame; a few more only cost warm-up.
const FRAMES: u32 = 4;

// The fixture's geometry, restated. These are not free choices — each mirrors a
// value in `fixtures/line_joint_zigzag.toml`, and the test locates the joints by
// recomputing the scene's own layout arithmetic from them.
const ELEMENTS: usize = 6;
const SPAN: f32 = 0.9;
const BASELINE: f32 = -0.45;
const THICKNESS: f32 = 24.0;
/// `spectrum.rs`'s `WIDTH_SCALE`: the private constant mapping `thickness` to an
/// NDC-y **half**-width, which is also the distance a flagged end extends by.
const WIDTH_SCALE: f32 = 0.003;
/// The two element levels the zigzag alternates between.
const LOW: f32 = 0.05;
const HIGH: f32 = 0.75;

/// Baseline file stem under `tests/golden/` (Plan 0040 Phase 1).
const BASELINE_STEM: &str = "line_joint_zigzag";
/// Mean per-channel difference (0..1) a fresh render may drift from baseline.
/// `composite.rs`'s tolerance, unchanged — no measurement here asked for another.
const MEAN_TOL: f32 = 0.02;
/// Largest single-channel byte difference tolerated at any pixel.
const MAX_OUTLIER: u8 = 48;

/// A band array whose downsampled elements alternate hard between [`LOW`] and
/// [`HIGH`].
///
/// `downsample` gives element `i` the mean of bands `[i*B/N, (i+1)*B/N)`, so
/// filling each element's own slice with a single value makes the element level
/// exactly that value — which is what lets the test compute the vertex
/// coordinates rather than discover them.
fn zigzag_frame() -> AnalysisFrame {
    let mut frame = AnalysisFrame {
        bass: 0.6,
        mid: 0.5,
        treb: 0.6,
        ..Default::default()
    };
    for i in 0..ELEMENTS {
        let lo = i * SPECTRUM_BINS / ELEMENTS;
        let hi = (i + 1) * SPECTRUM_BINS / ELEMENTS;
        for band in frame.spectrum.iter_mut().take(hi).skip(lo) {
            *band = level(i);
        }
    }
    frame
}

/// Element `i`'s level — the alternation the whole fixture is built around.
fn level(i: usize) -> f32 {
    if i.is_multiple_of(2) { LOW } else { HIGH }
}

/// Polyline point `i` in world space, which on a square capture is also NDC.
/// This is `spectrum.rs`'s `Polyline` arm, recomputed: `n` points spanning edge
/// to edge at `baseline + length`, with `length = base + scale * level` at
/// `base = 0`, `scale = 1`.
fn point(i: usize) -> [f32; 2] {
    let step = 2.0 * SPAN / (ELEMENTS - 1) as f32;
    [-SPAN + step * i as f32, BASELINE + level(i)]
}

fn midpoint(a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    [0.5 * (a[0] + b[0]), 0.5 * (a[1] + b[1])]
}

/// Mean RGB luminance (0..1) in a 3x3 box around the pixel `world` lands on, so
/// half a pixel of rasterization slop cannot decide the test. Square capture, no
/// zoom and no pan, so world coordinates go straight to NDC and then to pixels.
fn luma_at(img: &CaptureImage, world: [f32; 2]) -> f32 {
    let cx = ((world[0] * 0.5 + 0.5) * img.width as f32).round() as i32;
    let cy = ((0.5 - world[1] * 0.5) * img.height as f32).round() as i32;
    let (w, h) = (img.width as i32, img.height as i32);
    let mut sum = 0.0f32;
    let mut n = 0.0f32;
    for dy in -1..=1 {
        for dx in -1..=1 {
            let (x, y) = (cx + dx, cy + dy);
            if x < 0 || y < 0 || x >= w || y >= h {
                continue;
            }
            let o = (y as usize * img.width as usize + x as usize) * 4;
            let px = img.rgba.get(o..o + 3).unwrap_or(&[0, 0, 0]);
            sum += px.iter().map(|&c| c as f32).sum::<f32>() / (3.0 * 255.0);
            n += 1.0;
        }
    }
    sum / n.max(1.0)
}

/// Largest absolute single-channel (RGB) byte difference across the two images.
fn max_channel_outlier(a: &CaptureImage, b: &CaptureImage) -> u8 {
    a.rgba
        .chunks_exact(4)
        .zip(b.rgba.chunks_exact(4))
        .flat_map(|(pa, pb)| {
            pa.iter()
                .zip(pb.iter())
                .take(3)
                .map(|(x, y)| x.abs_diff(*y))
        })
        .max()
        .unwrap_or(0)
}

/// Plan 0039 Phase 2 done-when 3, and the reported defect itself.
///
/// Every odd element of the fixture is a **peak**, so the outside of each turn
/// is straight up and one vertical offset serves the joint and both the segment
/// interiors either side of it. The offset is half the wedge's own height at the
/// vertex column (`half_width * cos(theta)`), which puts the sample squarely
/// inside the hole a joinless stroke leaves while staying well inside the stroke
/// over a segment's interior.
///
/// **Proven to fail first.** Against Phase 1's code — the flag and the shader
/// extension in place, no producer flagging anything — the joint samples measure
/// `0.0000` against interiors of `0.4885` and `0.4588`, and the assertion fires.
/// A test that passes before the fix is not testing the fix.
///
/// With the joins on, the joint reads *brighter* than either interior (`0.6431`
/// against `0.4885`/`0.4588`): the two extended quads overlap and the additive
/// blend sums them. That is the near-180-degree overshoot ADR-0041 accepts in
/// place of a miter limit, and this fixture is sharp enough to show it — which is
/// the point. It is recorded here rather than discovered later.
///
/// This is the **first** of the capture's three duties;
/// [`compare_against_baseline`] is the last, and the module docs say why it runs
/// after rather than before.
///
/// Returns the **dimmest interior sample** it measured, which
/// [`assert_the_outer_ends_are_free`] uses as its comparator (Plan 0040
/// Phase 2). These are samples of a stroke lit at an *off-centre* offset, so
/// anything genuinely on the stroke's centre line reads brighter — which is what
/// makes "darker than this" a statement about background rather than a tuned
/// threshold.
fn assert_no_notch(img: &CaptureImage) -> f32 {
    let half_width = THICKNESS * WIDTH_SCALE;
    let mut dimmest_interior = f32::INFINITY;

    // The last point is the figure's free end, never a joint — so the interior
    // peaks are the odd elements below it.
    let joints: Vec<usize> = (1..ELEMENTS - 1).step_by(2).collect();
    assert!(
        !joints.is_empty(),
        "the fixture must contain at least one interior peak to test"
    );

    for j in joints {
        let (before, vertex, after) = (point(j - 1), point(j), point(j + 1));

        // How far the wedge reaches above the vertex is set by the turn: the
        // outer corners of the two quads sit at `half_width` from the vertex
        // along each segment's own normal, whose vertical component is
        // `cos(theta)`. Sample halfway up that, derived rather than tuned.
        let d = [vertex[0] - before[0], vertex[1] - before[1]];
        let len = (d[0] * d[0] + d[1] * d[1]).sqrt();
        let cos_theta = (d[0] / len).abs();
        let offset = half_width * cos_theta * 0.5;
        let lift = |p: [f32; 2]| [p[0], p[1] + offset];

        let left = luma_at(img, lift(midpoint(before, vertex)));
        let joint = luma_at(img, lift(vertex));
        let right = luma_at(img, lift(midpoint(vertex, after)));
        println!(
            "element {j}: interior {left:.4} | joint {joint:.4} | interior {right:.4} \
             (offset {offset:.4} NDC above the path)"
        );

        assert!(
            left > 0.0 && right > 0.0,
            "element {j}: the interior probes must land on the stroke, or the \
             comparison is between two pieces of background — got {left:.4} and \
             {right:.4}"
        );
        assert!(
            joint >= left.min(right),
            "element {j}: the joint is a local luminance minimum ({joint:.4} \
             against interiors {left:.4} and {right:.4}) — the stroke is coming \
             apart at the vertex"
        );
        dimmest_interior = dimmest_interior.min(left).min(right);
    }
    dimmest_interior
}

/// Plan 0040 Phase 2: **a swap of [`JOINED_A`] and [`JOINED_B`] must fail a
/// test**, which before this nothing did.
///
/// `line_joints.rs` probed only *interior* joints, where a chained segment
/// carries both bits and the two are indistinguishable — swap them and every
/// interior vertex renders identically. The bits are only separable where a
/// segment carries **one** of them, and the fixture already has exactly that
/// shape: six elements, five segments, `joined[0] = JOINED_B` (its `a` free) and
/// `joined[4] = JOINED_A` (its `b` free). No new fixture needed.
///
/// The property, stated rather than thresholded: **the stroke must not extend
/// past the figure's own first and last points.** Those two ends are free, so a
/// probe just beyond them along the stroke direction falls in background. Swap
/// the bits and each grows by a half-width, putting that probe on the stroke's
/// **centre line** — where the shader's falloff is 1, so it reads brighter than
/// any of the off-centre interior samples [`assert_no_notch`] measured. Hence
/// the ordinal comparison: dimmer than the dimmest stroke sample separates
/// "background" from "stroke" without inventing a constant.
///
/// The probe sits **half** an extension out, not a full one: at a full
/// half-width it would land exactly on the flat end-cut a swapped quad would
/// draw, and a boundary sample decides nothing. Halfway is unambiguous either
/// way — 9 px at this capture size, against `luma_at`'s 1 px box.
///
/// "Lit or not" is then decided by **classification between two measured
/// regimes**, not by a constant: unlit background sampled from the frame, lit
/// stroke from `dimmest_interior`, and the probe must fall on the background
/// side of the midpoint. Both regimes come from this same capture, so nothing
/// here is tuned — and both margins are wide, where comparing straight against
/// `dimmest_interior` left only 0.03 on the failing side.
fn assert_the_outer_ends_are_free(img: &CaptureImage, dimmest_interior: f32) {
    let half_width = THICKNESS * WIDTH_SCALE;

    // Unlit reference, well clear of the figure: the polyline's highest point is
    // `BASELINE + HIGH` and it can only reach a half-width beyond that.
    let background = luma_at(img, [0.0, 0.9]);
    assert!(
        background < dimmest_interior,
        "the background probe landed on the figure ({background:.4} against a stroke \
         interior of {dimmest_interior:.4}) — the fixture's geometry has moved and \
         the classification below has no two regimes to separate"
    );
    let unlit_side_of = 0.5 * (background + dimmest_interior);

    // (label, the free endpoint, the interior point the segment runs from/to).
    // Segment 0 runs point(0) -> point(1) with `a` free, so its outward
    // direction is away from point(1); segment 4 runs point(4) -> point(5) with
    // `b` free, so its outward direction is away from point(4).
    let ends = [
        ("first", point(0), point(1)),
        ("last", point(ELEMENTS - 1), point(ELEMENTS - 2)),
    ];

    for (label, end, inward) in ends {
        let d = [end[0] - inward[0], end[1] - inward[1]];
        let len = (d[0] * d[0] + d[1] * d[1]).sqrt();
        let outward = [d[0] / len, d[1] / len];
        let reach = half_width * 0.5;
        let probe = [end[0] + outward[0] * reach, end[1] + outward[1] * reach];

        let beyond = luma_at(img, probe);
        println!(
            "{label} end: beyond {beyond:.4} (background {background:.4}, dimmest \
             stroke interior {dimmest_interior:.4}, unlit below {unlit_side_of:.4}; \
             probe {reach:.4} NDC past the endpoint)"
        );

        assert!(
            beyond < unlit_side_of,
            "the {label} end of the figure is free, but the stroke reaches past it: \
             {beyond:.4} sits on the lit side of {unlit_side_of:.4} (background \
             {background:.4}, stroke {dimmest_interior:.4}). The most likely cause \
             is JOINED_A and JOINED_B being swapped between the Rust constants and \
             the shader — the outer ends are the only endpoints carrying a single \
             bit, so they are the only place a swap is fully visible."
        );
    }
}

/// Plan 0040 Phase 1: the reported defect, pinned in pixels.
///
/// [`assert_no_notch`] measures one property at six probe points. This measures
/// **the whole frame**, so a shader edit that reopens the notch somewhere the
/// probes do not look — a gentler turn, a free end, the falloff — moves a file
/// instead of passing quietly. It is the coarse net under the sharp claim.
///
/// `LMV_BLESS=1 cargo test -p lmv-core --test line_joints` rewrites this one
/// baseline. Scoped to this binary it **cannot** reach the `golden.rs` roster:
/// that suite renders `SystemKind::ALL` and is not built by this invocation. Run
/// against the whole suite (`cargo test` with `LMV_BLESS` set) it would rewrite
/// every baseline in the repository — bless by `--test line_joints` and check
/// `git status`, the trap that cost Plan 0039 two manual restores.
fn compare_against_baseline(img: &CaptureImage) {
    std::fs::create_dir_all(common::golden_dir()).expect("create tests/golden");
    let path = common::golden_dir().join(format!("{BASELINE_STEM}.png"));

    if std::env::var_os("LMV_BLESS").is_some() {
        common::encode(img, &path);
        println!("blessed {}", path.display());
        return;
    }

    assert!(
        path.exists(),
        "missing baseline {} — run `LMV_BLESS=1 cargo test -p lmv-core --test line_joints`",
        path.display()
    );
    let baseline = common::decode(&path);
    let mean = frame_diff(&baseline, img);
    let outlier = max_channel_outlier(&baseline, img);
    println!(
        "{BASELINE_STEM:<18} mean {mean:.4} (tol {MEAN_TOL}) max_outlier {outlier} (tol {MAX_OUTLIER})"
    );
    assert!(
        mean <= MEAN_TOL && outlier <= MAX_OUTLIER,
        "the joined polyline has drifted from its baseline: mean {mean:.4} / outlier \
         {outlier} exceeds tolerance. The stroke still passes the local-minimum claim, \
         so this is a change in *how* the joint renders rather than the notch \
         reopening. Bless with LMV_BLESS=1 only if that change was intended \
         (ADR-0041)."
    );
}

/// The suite: one capture, all three duties, in the order the module docs argue
/// for.
#[test]
fn the_joined_polyline_holds_its_shape_and_its_pixels() {
    let Some(mut renderer) = common::headless(SIZE, SIZE) else {
        return;
    };
    let preset = Preset::from_toml_str(include_str!("fixtures/line_joint_zigzag.toml"))
        .expect("the zigzag fixture is a valid preset");
    let name = preset.name.clone();
    renderer.set_presets(vec![preset]);

    let img = renderer
        .capture_preset(&name, &zigzag_frame(), FRAMES)
        .expect("capture the zigzag fixture");

    let dimmest_interior = assert_no_notch(&img);
    assert_the_outer_ends_are_free(&img, dimmest_interior);
    compare_against_baseline(&img);
}
