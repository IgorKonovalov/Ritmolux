// Tests index slices and panic on failure; allowed over the file's hot-path
// pragma — this is not the render path.
#![allow(clippy::indexing_slicing, clippy::panic, clippy::expect_used)]

// -----------------------------------------------------------------------
// The in-frame geometry fraction (Plan 0069 Phase 1, ADR-0083)
// -----------------------------------------------------------------------

use super::super::ViewTransform;
use super::{DrawExtent, SegmentInstance, measure_extent};

/// A segment with everything but its endpoints held constant — colour, width
/// and the join flags are not part of this measurement (it is length, not
/// area).
fn seg(a: [f32; 2], b: [f32; 2]) -> SegmentInstance {
    SegmentInstance {
        a,
        b,
        color: [1.0, 1.0, 1.0],
        width: 0.01,
        joined: 0,
    }
}

/// Slack for the *clipped* cases only. The two endpoints of the property —
/// wholly inside and wholly outside — are asserted **exactly** below, which
/// is the point: they are the same sum and the empty sum respectively, not
/// two nearly-equal ones.
const EPS: f32 = 1e-6;

/// A figure entirely inside the frame is **exactly** 1.0: no segment is
/// clipped, so `len * 1.0` adds the same value to both sums.
#[test]
fn a_figure_inside_the_frame_measures_exactly_one() {
    let segments = [
        seg([-0.5, -0.5], [0.5, 0.5]),
        seg([0.2, -0.9], [1.5, 0.9]),
        seg([-1.55, 0.0], [1.55, 0.1]),
    ];
    let extent = measure_extent(&segments, 1.6, ViewTransform::default());
    assert!(extent.total_len > 0.0, "the fixture must draw something");
    assert_eq!(
        extent.in_frame_len, extent.total_len,
        "an unclipped figure must accumulate the identical sum twice"
    );
    assert_eq!(extent.fraction(), Some(1.0));
}

/// A figure entirely outside is **exactly** 0.0 — a real denominator over an
/// empty numerator, which is what distinguishes it from "nothing drawn".
#[test]
fn a_figure_outside_the_frame_measures_exactly_zero() {
    let segments = [
        seg([2.0, 2.0], [3.0, 3.0]),
        seg([-4.0, 1.5], [4.0, 1.5]), // spans the width, above the top edge
    ];
    let extent = measure_extent(&segments, 1.0, ViewTransform::default());
    assert!(extent.total_len > 0.0, "the fixture must draw something");
    assert_eq!(extent.in_frame_len, 0.0);
    assert_eq!(extent.fraction(), Some(0.0));
}

/// A segment crossing an edge contributes its **clipped share**, not
/// all-or-nothing — including the case both endpoints are outside, which is
/// the one an endpoint test gets wrong and exactly what an over-scaled figure
/// is made of.
///
/// Every case is hand-computed against the unit frame (`aspect = 1.0`, so the
/// rectangle is `[-1, 1]^2`):
///
/// | segment                   | length | inside | fraction |
/// |---------------------------|--------|--------|----------|
/// | `(0,0) -> (4,0)`          | 4      | 1      | 0.25     |
/// | `(-2,0) -> (2,0)`         | 4      | 2      | 0.5      |
/// | `(-2,-2) -> (2,2)`        | √32    | √8     | 0.5      |
/// | `(-2,0.5) -> (0.5,-2)`    | √12.5  | ·0.2   | 0.2      |
/// | `(-2,1.5) -> (2,1.5)`     | 4      | 0      | 0        |
#[test]
fn a_crossing_segment_contributes_its_clipped_share() {
    let cases: [([f32; 2], [f32; 2], f32); 5] = [
        ([0.0, 0.0], [4.0, 0.0], 0.25),
        ([-2.0, 0.0], [2.0, 0.0], 0.5),
        ([-2.0, -2.0], [2.0, 2.0], 0.5),
        ([-2.0, 0.5], [0.5, -2.0], 0.2),
        ([-2.0, 1.5], [2.0, 1.5], 0.0),
    ];
    for (a, b, want) in cases {
        let extent = measure_extent(&[seg(a, b)], 1.0, ViewTransform::default());
        let got = extent.fraction().expect("the segment has a length");
        assert!(
            (got - want).abs() < EPS,
            "{a:?} -> {b:?} measured {got}, hand-computed {want}"
        );
    }
}

/// **The rectangle follows the aspect the caller hands in, and only that**
/// ([ADR-0037](../../../../../docs/adrs/0037-internal-grid-is-a-resolution-not-a-shape.md)).
///
/// `measure_extent` is a free function over the endpoints: there is no
/// `self`, no texture and no internal grid in scope, so the aspect parameter
/// is the *only* source of one — and `draw` passes the value it was handed,
/// which is the render target's. This asserts the consequence, which is what
/// a reader can check: the same segment measures 0.5 in a square frame and
/// 1.0 in one twice as wide, while the **vertical** half-extent stays at 1.0
/// whatever the aspect (the renderer maps two world units to the frame
/// *height*).
#[test]
fn the_rectangle_follows_the_aspect_the_draw_is_handed() {
    let across = [seg([0.0, 0.0], [2.0, 0.0])];
    let up = [seg([0.0, 0.0], [0.0, 2.0])];
    let square = measure_extent(&across, 1.0, ViewTransform::default());
    let wide = measure_extent(&across, 2.0, ViewTransform::default());
    assert!((square.fraction().expect("length") - 0.5).abs() < EPS);
    assert_eq!(
        wide.fraction(),
        Some(1.0),
        "at aspect 2 the same horizontal segment fits exactly"
    );
    for aspect in [1.0, 2.0, 3.5] {
        let vertical = measure_extent(&up, aspect, ViewTransform::default());
        assert!(
            (vertical.fraction().expect("length") - 0.5).abs() < EPS,
            "the frame's half-height is 1.0 at every aspect, got {vertical:?} at {aspect}"
        );
    }
}

/// The view transform is part of what the frame shows, so it is part of the
/// measurement: the shader moves endpoints by `a * zoom + pan` before the
/// aspect divide, and a figure pushed off the top by `pan_y` has overshot
/// just as surely as one scaled off it.
#[test]
fn the_view_transform_moves_the_figure_against_the_frame() {
    let segments = [seg([0.0, 0.0], [0.0, 0.5])];
    let zoomed = ViewTransform {
        zoom: 4.0,
        ..Default::default()
    };
    let panned = ViewTransform {
        pan: [0.0, 1.0],
        ..Default::default()
    };
    assert_eq!(
        measure_extent(&segments, 1.0, ViewTransform::default()).fraction(),
        Some(1.0)
    );
    let zoomed = measure_extent(&segments, 1.0, zoomed);
    assert!((zoomed.fraction().expect("length") - 0.5).abs() < EPS);
    assert_eq!(
        measure_extent(&segments, 1.0, panned).fraction(),
        Some(0.0),
        "panned up by a full half-height, the segment sits on and above the edge"
    );
}

/// Nothing drawn is **not** a fraction of zero. Plan 0058's table printed
/// `inf` for a preset that drew nothing; this reports `None` and leaves the
/// total case to `sanity.rs`.
#[test]
fn nothing_drawn_reports_no_fraction() {
    assert_eq!(DrawExtent::default().fraction(), None);
    assert_eq!(
        measure_extent(&[], 1.6, ViewTransform::default()).fraction(),
        None
    );
    // A degenerate segment is length zero wherever it sits, so it moves
    // neither sum — a figure collapsed to a point is `sanity.rs`'s question.
    let degenerate = [seg([9.0, 9.0], [9.0, 9.0])];
    assert_eq!(
        measure_extent(&degenerate, 1.6, ViewTransform::default()).fraction(),
        None
    );
}

// -----------------------------------------------------------------------
// The stroke seam does not punch holes in the backdrop (Plan 0051 Phase 2)
// -----------------------------------------------------------------------

/// The lit-backdrop fixture this guard captures three ways. Its `bg_bright`
/// and `draw_progress` lines are **stripped and rewritten** per capture — one
/// scene at three configurations — so the numbers are read back out of the
/// file rather than restated here, and editing the fixture moves the test
/// with it. Read its header before touching it: `thickness = 9` is what makes
/// the defect measurable at all.
const LIT_FIXTURE: &str = include_str!("../../../../../tests/fixtures/lines_lit_backdrop.toml");

/// The square capture size — twice the swarm guard's, and an exact multiple
/// of the post chain's 256 px grid step so the trails stage runs 1:1 with the
/// target.
///
/// The feature under test here is a **rim a fraction of a stroke width
/// across**, not a sprite corner, so it needs the pixels: at 256 the fat
/// stroke is only 7 px wide and there is little left of its edge to measure.
/// Same argument `line_joint_zigzag.toml` makes for its own 512.
const CAPTURE_SIZE: u32 = 512;

/// Frames per capture. The fixture is frozen (`spin = 0`), so this only
/// clears the draw-in and lets the trail history settle.
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

/// Slack for half-precision rounding, the same shape the swarm guard and
/// `bloom.rs`'s use. The composite is `Rgba16Float`, so a value of magnitude
/// `m` is stored to roughly `m / 1024`, and the lit capture quantizes a
/// different sum than the backdrop-only one does.
///
/// It is slack, not a tolerance: the property below is **exact** in real
/// arithmetic. Measured on this fixture, the fixed shader's worst `|L - B|`
/// is **0.0000** across 0 channels; the pre-fix one's is **0.4944** — very
/// nearly the backdrop's own `bg_bright`, i.e. discarded outright — across 15
/// channels. The magnitude is unambiguous; the *count* is small, and the
/// comment on `lit_mask` below says why that is geometry rather than a weak
/// fixture.
fn half_slack(value: f32) -> f32 {
    (4.0 / 1024.0) * value.abs().max(1.0)
}

/// **Where the strokes drew no light, the backdrop arrives intact** — the
/// same guard `swarm.rs` installs, on the other draw seam.
///
/// `fs_main` used to return `vec4(in.color * g * u.v.y, 1.0)`: colour carried
/// the across-the-stroke falloff, alpha was a literal constant. With the
/// alpha blend at `BlendComponent::OVER` and a source alpha of exactly 1,
/// destination alpha saturated across the whole stroke quad — including the
/// two long edges where the falloff reaches zero and the shader wrote
/// nothing. The chain resolves `src.rgb + backdrop * (1 - src.a)`
/// (ADR-0055), so those edges discarded the backdrop and rendered as black
/// rims and wedges over the figure.
///
/// **This is the quiet seam of the two, and that is a geometric fact rather
/// than a difference in kind.** The swarm's falloff is radial over a *square*
/// quad, so its zero-colour region is four large hard-edged corners; the
/// line's is one-dimensional across the stroke, so its zero-colour region is
/// a rim whose width scales with `thickness`. At shipped widths that rim is
/// nearly a hairline — which is why the swarm was reported and this was not,
/// and why the fixture uses a deliberately fat stroke.
///
/// One edit and one guard cover all four line scenes: `parametric_curve`,
/// `lsystem`, `star_pattern` and `spectrum` all stroke through this renderer.
///
/// See the swarm guard for why this reads the **linear** composite rather
/// than the capture (the tonemap scales every channel off the brightest one,
/// so no byte-level tolerance separates the defect from the curve behaving as
/// designed) and why it therefore lives in the render module rather than in
/// `core/tests/`.
#[test]
fn a_lit_backdrop_survives_where_the_strokes_drew_nothing() {
    use crate::dsp::AnalysisFrame;
    use crate::preset::Preset;
    use crate::render::capture;
    use crate::render::context::RenderError;
    use crate::render::{HeadlessOptions, Renderer};

    // --- Non-vacuity, before any GPU work: the fixture must still describe
    // the configuration this guard exists for. ---
    let backdrop = fixture_value("bg_bright");
    let progress = fixture_value("draw_progress");
    let thickness = fixture_value("thickness");
    let trails = fixture_value("trails");
    assert!(
        backdrop > 0.0,
        "lines_lit_backdrop.toml no longer ships a lit backdrop (bg_bright = \
         {backdrop}); on black this whole comparison is black against black"
    );
    assert!(
        progress > 0.0,
        "lines_lit_backdrop.toml no longer draws the curve (draw_progress = \
         {progress})"
    );
    assert!(
        thickness >= 6.0,
        "lines_lit_backdrop.toml is down to thickness = {thickness}. The dark \
         region this guards is a RIM whose width scales with the stroke, and \
         at shipped widths (2 to 3) it is close to a hairline that a capture \
         cannot discriminate — a thin fixture leaves this test green and \
         blind. See the file's header"
    );
    assert!(
        trails > 0.0,
        "lines_lit_backdrop.toml no longer binds `trails` (= {trails}), so no \
         post stage is active. With an empty chain the scene draws straight \
         onto the backdrop and its additive colour cannot remove light — the \
         defect is unrepresentable and this test proves nothing"
    );

    /// The linear composite the tonemap is about to map, at a given backdrop
    /// brightness and reveal fraction.
    ///
    /// Builds and drops **one** renderer per call rather than holding three:
    /// a second live device in a binary is what the software adapter falls
    /// over on, and building GPU resources mid-run shifts what the trails
    /// stage resolves to on WARP.
    fn linear_composite(bg_bright: f32, draw_progress: f32) -> Option<Vec<f32>> {
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
        // Both keys live in `[params]`, which is the fixture's last table, so
        // stripping them and appending the overrides keeps them in it.
        let base: String = LIT_FIXTURE
            .lines()
            .filter(|line| {
                let line = line.trim_start();
                !line.starts_with("bg_bright") && !line.starts_with("draw_progress")
            })
            .collect::<Vec<_>>()
            .join("\n");
        let toml =
            format!("{base}\nbg_bright = \"{bg_bright}\"\ndraw_progress = \"{draw_progress}\"\n");
        let preset = Preset::from_toml_str(&toml)
            .expect("the lit-backdrop line fixture parses with overrides");
        let name = preset.name.clone();
        renderer.set_presets(vec![preset]);

        // Every binding is a constant, so the analysis frame only has to be
        // well-formed — nothing in this fixture reads it.
        let frame = AnalysisFrame::default();
        renderer
            .capture_preset(&name, &frame, CAPTURE_FRAMES)
            .expect("capture the lit-backdrop line fixture");

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
            label: Some("lines-backdrop-readback"),
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
    // backdrop with the scene contributing nothing — at `draw_progress = 0`
    // the curve yields zero segments and the renderer returns without a draw
    // call, so this is the backdrop alone through the same pipeline as `L`.
    let Some(lit) = linear_composite(backdrop, progress) else {
        return;
    };
    let Some(dark) = linear_composite(0.0, progress) else {
        return;
    };
    let Some(backdrop_only) = linear_composite(backdrop, 0.0) else {
        return;
    };
    assert_eq!(dark.len(), lit.len(), "the captures differ in size");
    assert_eq!(
        dark.len(),
        backdrop_only.len(),
        "the captures differ in size"
    );

    let total = dark.len() / 4;
    // Which pixels the scene put light into, so the report below can say how
    // much of the untouched region actually **borders** the geometry. That
    // sub-count is the part of the domain a broken alpha can reach: the rest
    // is open backdrop no quad ever covered, which passes either way.
    //
    // It is deliberately reported and not asserted on. Unlike the swarm's
    // corners — a 2-D region where the radial falloff is identically zero
    // over ~21 % of every sprite — the line falloff is one-dimensional and
    // *quadratic*, so its exactly-zero band is the outermost sub-pixel sliver
    // of the quad and only a handful of sample points land in it. The rim the
    // eye sees is the much wider band where coverage is near zero rather than
    // zero. That is a property of the geometry, not of this fixture, and no
    // choice of `samples`/`scale`/`thickness` widens it.
    let lit_mask: Vec<bool> = dark
        .chunks_exact(4)
        .map(|texel| texel[0] != 0.0 || texel[1] != 0.0 || texel[2] != 0.0)
        .collect();
    let borders_geometry = |pixel: usize| -> bool {
        let (w, h) = (CAPTURE_SIZE as usize, CAPTURE_SIZE as usize);
        let (x, y) = (pixel % w, pixel / w);
        let mut neighbours = Vec::with_capacity(4);
        if x > 0 {
            neighbours.push(pixel - 1);
        }
        if x + 1 < w {
            neighbours.push(pixel + 1);
        }
        if y > 0 {
            neighbours.push(pixel - w);
        }
        if y + 1 < h {
            neighbours.push(pixel + w);
        }
        neighbours.iter().any(|&n| lit_mask[n])
    };

    let (mut untouched, mut drawn, mut over_backdrop, mut on_the_rim) =
        (0usize, 0usize, 0usize, 0usize);
    let (mut violations, mut worst) = (0usize, 0.0f32);
    for (pixel, texel) in dark.chunks_exact(4).enumerate() {
        if texel[0] != 0.0 || texel[1] != 0.0 || texel[2] != 0.0 {
            drawn += 1;
            continue; // the scene put light here; the property says nothing
        }
        untouched += 1;
        if borders_geometry(pixel) {
            on_the_rim += 1;
        }
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
        "lines lit backdrop at {CAPTURE_SIZE}x{CAPTURE_SIZE}: {untouched} of \
         {total} pixels untouched by the scene ({over_backdrop} of those over \
         a lit backdrop, {on_the_rim} of them bordering geometry), {drawn} lit \
         by it; worst |L - B| {worst:.4}"
    );

    // --- Non-vacuity: the region the property speaks about is a substantial
    // part of the frame, the strokes genuinely drew into the rest, and the
    // backdrop genuinely reached the frame underneath. A fixture edit that
    // quietly empties any of the three shows up here rather than passing. ---
    assert!(
        untouched * 4 > total,
        "only {untouched} of {total} pixels are untouched by the scene — the \
         fixture has filled the frame and the property covers almost nothing"
    );
    assert!(
        drawn * 50 > total,
        "only {drawn} of {total} pixels carry any stroke light — the fixture \
         has stopped drawing, so the stroke rims this guards are not in the \
         frame"
    );
    assert!(
        over_backdrop * 2 > untouched,
        "only {over_backdrop} of the {untouched} untouched pixels sit over a \
         backdrop brighter than {BACKDROP_PRESENT} — comparing black against \
         black, which any alpha would pass"
    );

    // --- The property. ---
    assert_eq!(
        violations, 0,
        "{violations} channels differ between the lit frame and the backdrop \
         alone at pixels where the strokes wrote NO light (worst {worst:.4}). \
         Upstream of the tonemap this is a plain premultiplied OVER, so where \
         nothing was drawn the backdrop must arrive intact — a difference \
         here is a stroke emitting coverage it does not have, rimming itself \
         in backdrop it never painted over"
    );
}
