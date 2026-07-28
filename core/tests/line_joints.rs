//! Line joins at the pixel level (Plan 0039 Phase 2, ADR-0041).
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

use lmv_core::dsp::{AnalysisFrame, SPECTRUM_BINS};
use lmv_core::preset::Preset;
use lmv_core::render::{CaptureImage, HeadlessOptions, RenderError, Renderer};

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

/// Build a headless `Renderer`, or `None` (a logged skip) when the runner
/// exposes no GPU adapter — macOS has no software Metal fallback (ADR-0016).
fn headless() -> Option<Renderer> {
    match Renderer::new_headless(HeadlessOptions {
        width: SIZE,
        height: SIZE,
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
#[test]
fn a_polyline_joint_is_not_a_notch() {
    let Some(mut renderer) = headless() else {
        return;
    };
    let preset = Preset::from_toml_str(include_str!("fixtures/line_joint_zigzag.toml"))
        .expect("the zigzag fixture is a valid preset");
    let name = preset.name.clone();
    renderer.set_presets(vec![preset]);

    let img = renderer
        .capture_preset(&name, &zigzag_frame(), FRAMES)
        .expect("capture the zigzag fixture");

    let half_width = THICKNESS * WIDTH_SCALE;

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

        let left = luma_at(&img, lift(midpoint(before, vertex)));
        let joint = luma_at(&img, lift(vertex));
        let right = luma_at(&img, lift(midpoint(vertex, after)));
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
    }
}
