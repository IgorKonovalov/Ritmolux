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
        alpha: 1.0,
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
    let extent = measure_extent(&segments, &[], 1.6, ViewTransform::default());
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
    let extent = measure_extent(&segments, &[], 1.0, ViewTransform::default());
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
        let extent = measure_extent(&[seg(a, b)], &[], 1.0, ViewTransform::default());
        let got = extent.fraction().expect("the segment has a length");
        assert!(
            (got - want).abs() < EPS,
            "{a:?} -> {b:?} measured {got}, hand-computed {want}"
        );
    }
}

/// **The rectangle follows the aspect the caller hands in, and only that**
/// (ADR-0037).
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
    let square = measure_extent(&across, &[], 1.0, ViewTransform::default());
    let wide = measure_extent(&across, &[], 2.0, ViewTransform::default());
    assert!((square.fraction().expect("length") - 0.5).abs() < EPS);
    assert_eq!(
        wide.fraction(),
        Some(1.0),
        "at aspect 2 the same horizontal segment fits exactly"
    );
    for aspect in [1.0, 2.0, 3.5] {
        let vertical = measure_extent(&up, &[], aspect, ViewTransform::default());
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
        measure_extent(&segments, &[], 1.0, ViewTransform::default()).fraction(),
        Some(1.0)
    );
    let zoomed = measure_extent(&segments, &[], 1.0, zoomed);
    assert!((zoomed.fraction().expect("length") - 0.5).abs() < EPS);
    assert_eq!(
        measure_extent(&segments, &[], 1.0, panned).fraction(),
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
        measure_extent(&[], &[], 1.6, ViewTransform::default()).fraction(),
        None
    );
    // A degenerate segment is length zero wherever it sits, so it moves
    // neither sum — a figure collapsed to a point is `sanity.rs`'s question.
    let degenerate = [seg([9.0, 9.0], [9.0, 9.0])];
    assert_eq!(
        measure_extent(&degenerate, &[], 1.6, ViewTransform::default()).fraction(),
        None
    );
}

// -----------------------------------------------------------------------
// The in-frame measure counts arcs too (Plan 0087 Phase 2, ADR-0083)
// -----------------------------------------------------------------------

/// An arc with everything but its geometry held constant — colour and width
/// are not part of this measurement, which is length and not area.
fn marc(centre: [f32; 2], radius: f32, start: f32, sweep: f32) -> super::ArcInstance {
    super::ArcInstance {
        centre,
        radius,
        angle_start: start,
        angle_sweep: sweep,
        color: [1.0, 1.0, 1.0],
        width: 0.01,
    }
}

/// **An arc's own length is what it contributes**, and an arc inside the frame
/// measures exactly 1.0 — the same exactness the segment path has, and for the
/// same reason: both sums are taken from `|sweep| * radius`, so a sub-arc that
/// is not clipped adds the identical value to each.
#[test]
fn an_arc_inside_the_frame_measures_exactly_one_and_its_own_length() {
    let quarter = std::f32::consts::FRAC_PI_2;
    let arc = marc([0.0, 0.0], 0.5, 0.0, quarter);
    let extent = measure_extent(&[], &[arc], 1.6, ViewTransform::default());

    // The length is the arc's, not its chord's — a quarter of a circle of
    // radius 0.5 is `0.5 * pi/2`, where the chord would be `0.5 * sqrt(2)`.
    let want = 0.5 * quarter;
    assert!(
        (extent.total_len - want).abs() < EPS,
        "a quarter arc of radius 0.5 measured {} long, not {want} — a chord \
         sum would give {}",
        extent.total_len,
        0.5 * std::f32::consts::SQRT_2
    );
    assert_eq!(
        extent.in_frame_len, extent.total_len,
        "an unclipped arc must accumulate the identical sum twice"
    );
    assert_eq!(extent.fraction(), Some(1.0));
}

/// **An arc half outside the frame reports half**, and the expected value is
/// derived from the arc's own geometry rather than recorded off a run.
///
/// A full circle centred on the top edge of a square frame: exactly half of it
/// — the half below `y = 1` — is inside, by symmetry, whatever the radius. So
/// the number this must report is `0.5` because of where the circle sits, not
/// because a previous run said so.
///
/// Without this, an arc-drawing preset would read *better*-framed than it is:
/// the arc would contribute nothing to either sum, the denominator would shrink
/// to whatever segments remained, and a figure half off the frame would report
/// the fraction of the part that is still on it.
#[test]
fn an_arc_half_outside_the_frame_reports_half_of_its_own_length() {
    // Centred on the top edge (y = 1) of the square frame `[-1, 1]^2`, and
    // small enough not to reach the left or right edges.
    let arc = marc([0.0, 1.0], 0.4, 0.0, std::f32::consts::TAU);
    let extent = measure_extent(&[], &[arc], 1.0, ViewTransform::default());

    let want_len = 0.4 * std::f32::consts::TAU;
    assert!(
        (extent.total_len - want_len).abs() < EPS,
        "the circle's length is {want_len}, measured {}",
        extent.total_len
    );
    let got = extent.fraction().expect("the arc has a length");
    // Slack for the sub-chord sampling: each of the `ARC_STEPS` sub-arcs is
    // judged by its chord, and only the two straddling the edge can disagree
    // with the arc, by at most their own sagitta.
    let slack = 4.0 / super::ARC_STEPS as f32;
    assert!(
        (got - 0.5).abs() < slack,
        "a circle centred on the frame's top edge measured {got}, and by \
         symmetry exactly half of it is inside"
    );
}

/// The fraction **moves in the right direction** as the same arc is pushed off
/// the frame: monotonically down, exactly 1.0 while it is wholly inside, and
/// exactly 0.0 once it is wholly outside.
///
/// A direction rather than a table of values, so the check survives a change to
/// how the sub-arcs are counted.
#[test]
fn pushing_an_arc_off_the_frame_lowers_its_fraction_monotonically() {
    let mut previous = f32::INFINITY;
    let mut readings = Vec::new();
    for step in 0..=8 {
        let y = step as f32 * 0.35;
        let arc = marc([0.0, y], 0.4, 0.0, std::f32::consts::TAU);
        let got = measure_extent(&[], &[arc], 1.0, ViewTransform::default())
            .fraction()
            .expect("the arc has a length");
        assert!(
            got <= previous + EPS,
            "the fraction rose from {previous} to {got} as the arc moved \
             further off the frame"
        );
        previous = got;
        readings.push(got);
    }
    eprintln!("arc pushed off the top edge, fraction per 0.35 step: {readings:?}");
    assert_eq!(
        readings.first().copied(),
        Some(1.0),
        "the arc starts wholly inside the frame and must measure exactly 1.0"
    );
    assert_eq!(
        readings.last().copied(),
        Some(0.0),
        "at y = 2.8 with radius 0.4 the arc is wholly outside and must measure \
         exactly 0.0"
    );
}

/// The view transform reaches arcs exactly as it reaches segments: `zoom`
/// scales the radius with the centre and `pan` moves both, in world space
/// before the aspect divide — which is what the vertex shader does.
#[test]
fn the_view_transform_moves_an_arc_against_the_frame() {
    let arc = marc([0.0, 0.0], 0.4, 0.0, std::f32::consts::TAU);
    assert_eq!(
        measure_extent(&[], &[arc], 1.0, ViewTransform::default()).fraction(),
        Some(1.0)
    );
    // Zoomed by 4 the radius is 1.6, well past the frame's half-height, so most
    // of the circle is out; panned up by 2 it is gone entirely.
    let zoomed = measure_extent(
        &[],
        &[arc],
        1.0,
        ViewTransform {
            zoom: 4.0,
            ..Default::default()
        },
    );
    let fraction = zoomed.fraction().expect("the arc has a length");
    assert!(
        (0.0..1.0).contains(&fraction),
        "a 4x zoom must push the circle past the frame, got {fraction}"
    );
    assert!(
        (zoomed.total_len - 4.0 * 0.4 * std::f32::consts::TAU).abs() < 1e-4,
        "zoom scales the arc's length with its radius, got {}",
        zoomed.total_len
    );
    assert_eq!(
        measure_extent(
            &[],
            &[arc],
            1.0,
            ViewTransform {
                pan: [0.0, 2.0],
                ..Default::default()
            }
        )
        .fraction(),
        Some(0.0),
        "panned two half-heights up, the whole circle is above the frame"
    );
}

/// Segments and arcs in one batch share **one denominator** — the fraction is
/// over everything drawn, not over one kind of it.
#[test]
fn a_mixed_batch_measures_over_both_kinds() {
    // A segment wholly inside, and a circle wholly outside above it.
    let inside = seg([-0.5, 0.0], [0.5, 0.0]);
    let outside = marc([0.0, 3.0], 0.4, 0.0, std::f32::consts::TAU);
    let both = measure_extent(&[inside], &[outside], 1.0, ViewTransform::default());
    let alone = measure_extent(&[inside], &[], 1.0, ViewTransform::default());

    assert_eq!(
        alone.fraction(),
        Some(1.0),
        "the segment alone is wholly inside"
    );
    assert_eq!(
        both.in_frame_len, alone.in_frame_len,
        "the off-frame arc adds nothing to the numerator"
    );
    assert!(
        both.total_len > alone.total_len,
        "the off-frame arc must widen the denominator — that is the whole \
         obligation: an arc contributing nothing would make an arc-drawing \
         preset read better-framed than it is"
    );
    let got = both.fraction().expect("the batch has length");
    assert!(
        got < 1.0,
        "the mixed batch measured {got}; half its drawn length is off the frame"
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
/// Returning `vec4(in.color * g * u.v.y, 1.0)` from `fs_main` puts the
/// across-the-stroke falloff in colour and a literal constant in alpha.
/// With the alpha blend at `BlendComponent::OVER` and a source alpha of
/// exactly 1, destination alpha saturates across the whole stroke quad —
/// including the two long edges where the falloff reaches zero and the
/// shader wrote nothing. The chain resolves `src.rgb + backdrop * (1 -
/// src.a)` (ADR-0055), so those edges discarded the backdrop and rendered
/// as black rims and wedges over the figure.
///
/// **This is the quiet seam of the two, and that is a geometric fact rather
/// than a difference in kind.** The swarm's falloff is radial over a *square*
/// quad, so its zero-colour region is four large hard-edged corners; the
/// line's is one-dimensional across the stroke, so its zero-colour region is
/// a rim whose width scales with `thickness`. At shipped widths that rim is
/// nearly a hairline — which is why the swarm was reported and this was not,
/// and why the fixture uses a deliberately fat stroke.
///
/// # Two properties, on four captures (Plan 0053 Phase 4)
///
/// The consequence of that geometry was a guard that *worked* and barely
/// discriminated: reverting the shader moved the exact arm on **15 channels**,
/// about five pixels (design-backlog 0041). The magnitude was unambiguous and
/// the region was a thread.
///
/// The fourth capture fixes that by changing the property rather than the
/// fixture. At `glow = 0` the stroke draws its whole geometry and emits no
/// light, so the frame is exactly `bg * (1 - a)` and the composite becomes a
/// **direct readout of alpha**. Pre-fix the fully-extinguished set is the whole
/// quad footprint (28 178 pixels); post-fix it is the centreline (779). Both
/// arms stay: the first is exact and says the backdrop arrives *intact*, the
/// second is wide and says alpha is *coverage*. Neither replaces the other.
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
    let softness = fixture_value("softness");
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
        softness == 1.0,
        "lines_lit_backdrop.toml is at softness = {softness}, not the pinned \
         1.0. The wide arm below reads alpha off the FULLY EXTINGUISHED set, \
         and only the pure quadratic profile makes that set a curve: coverage \
         reaches 1 on the centreline alone (2.77 % of the footprint) against \
         the defect's whole quad (100 %). At the shipped 0.25 default a \
         plateau reaches coverage 1 over a REGION legitimately, so the same \
         reading no longer tells that from the constant alpha the arm exists \
         to catch. Being the shipped default is what makes this the easiest \
         line in the file to normalise away. See the file's header"
    );
    assert!(
        trails > 0.0,
        "lines_lit_backdrop.toml no longer binds `trails` (= {trails}), so no \
         post stage is active. With an empty chain the scene draws straight \
         onto the backdrop and its additive colour cannot remove light — the \
         defect is unrepresentable and this test proves nothing"
    );

    /// The linear composite the tonemap is about to map, at a given backdrop
    /// brightness and reveal fraction, optionally overriding `glow`.
    ///
    /// `glow: None` appends nothing, so the three original captures render
    /// exactly the configuration they always did — the existing arm below is a
    /// proven assertion and this must not move it. `Some(0.0)` is the fourth
    /// capture (Plan 0053 Phase 4): the stroke draws its whole geometry and
    /// emits no light.
    ///
    /// Builds and drops **one** renderer per call rather than holding four:
    /// a second live device in a binary is what the software adapter falls
    /// over on, and building GPU resources mid-run shifts what the trails
    /// stage resolves to on WARP.
    fn linear_composite(bg_bright: f32, draw_progress: f32, glow: Option<f32>) -> Option<Vec<f32>> {
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
                    && !line.starts_with("draw_progress")
                    && !line.starts_with("glow")
            })
            .collect::<Vec<_>>()
            .join("\n");
        let mut toml =
            format!("{base}\nbg_bright = \"{bg_bright}\"\ndraw_progress = \"{draw_progress}\"\n");
        if let Some(glow) = glow {
            toml.push_str(&format!("glow = \"{glow}\"\n"));
        }
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
    let Some(lit) = linear_composite(backdrop, progress, None) else {
        return;
    };
    let Some(dark) = linear_composite(0.0, progress, None) else {
        return;
    };
    let Some(backdrop_only) = linear_composite(backdrop, 0.0, None) else {
        return;
    };
    // `U`: the fourth capture (Plan 0053 Phase 4). Same scene, same backdrop,
    // `glow = 0` — the stroke rasterizes its whole quad and multiplies its
    // colour by zero, so `src.rgb` is 0 everywhere and the frame is exactly
    // `backdrop * (1 - a)`. That turns the composite into a **direct readout of
    // alpha**, which is the quantity this guard is actually about and the one
    // the lit capture can only see indirectly.
    let Some(unlit_stroke) = linear_composite(backdrop, progress, Some(0.0)) else {
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

    // --- The second, wider property (Plan 0053 Phase 4). ---
    //
    // **Measured before either assertion**, so a run that trips the exact arm
    // below still prints both counts. The whole point of this arm is the size
    // of the region it reaches against the size of the region the exact one
    // does, and a short-circuit that hid the comparison would retire the
    // evidence while keeping the code.
    //
    // The arm above is exact and it is *narrow*: it speaks only where the
    // falloff is identically zero, which for a 1-D quadratic falloff is the
    // outermost sub-pixel sliver of the quad. Reverting the shader at Plan
    // 0051's close moved it on **15 channels**. The magnitude was unambiguous
    // (0.4944, very nearly the whole backdrop) but five pixels is a thin thread
    // to hang a regression guard on, and no choice of `samples` / `scale` /
    // `thickness` widens it (design-backlog 0041).
    //
    // The `glow = 0` capture changes the *property* rather than the fixture.
    // With the stroke emitting nothing the frame is exactly `backdrop * (1 - a)`,
    // so a pixel is fully extinguished exactly where `a = 1`:
    //
    // - **Pre-fix**, alpha was the literal `1.0` over the whole quad, so the
    //   extinguished set is the entire stroke footprint — a 2-D region.
    // - **Post-fix**, alpha is the coverage `g`, which reaches 1 only on the
    //   centreline, so the extinguished set is a curve through it.
    //
    // That is three orders of magnitude of margin instead of five pixels, and
    // it needs no shader change and no new fixture.
    let extinguished = |limit: f32| -> usize {
        (0..total)
            .filter(|&pixel| {
                let base = pixel * 4;
                // Only where there was backdrop to lose; elsewhere "dark" is
                // not evidence of anything.
                backdrop_only[base..base + 3]
                    .iter()
                    .any(|&c| c > BACKDROP_PRESENT)
                    && (0..3).all(|c| unlit_stroke[base + c] <= backdrop_only[base + c] * limit)
            })
            .count()
    };
    // "Fully extinguished" = at most 2 % of the backdrop survives, i.e. `a` is
    // 0.98 or above. A hard `== 0` would be an empty set post-fix for a reason
    // that has nothing to do with the defect: `g` reaches exactly 1 only at
    // `d = 0`, which no sample point need land on.
    const EXTINGUISHED: f32 = 0.02;
    let killed = extinguished(EXTINGUISHED);
    // The stroke's own footprint, from the capture that shows where it drew.
    let footprint = drawn;
    eprintln!(
        "lines glow = 0: {killed} of {footprint} footprint pixels fully \
         extinguished ({:.2} %); the exact arm above moved {violations} channels",
        killed as f32 / footprint.max(1) as f32 * 100.0
    );

    // --- The first property, unchanged and still exact. ---
    assert_eq!(
        violations, 0,
        "{violations} channels differ between the lit frame and the backdrop \
         alone at pixels where the strokes wrote NO light (worst {worst:.4}). \
         Upstream of the tonemap this is a plain premultiplied OVER, so where \
         nothing was drawn the backdrop must arrive intact — a difference \
         here is a stroke emitting coverage it does not have, rimming itself \
         in backdrop it never painted over"
    );

    // Non-vacuity: the fourth capture must actually be an alpha readout. If
    // `glow = 0` stopped zeroing the emitted light the frame would carry stroke
    // colour again and this arm would be measuring something else.
    let stroke_light: usize = (0..total)
        .filter(|&pixel| {
            let base = pixel * 4;
            lit_mask[pixel] && (0..3).any(|c| unlit_stroke[base + c] > backdrop_only[base + c])
        })
        .count();
    assert_eq!(
        stroke_light, 0,
        "{stroke_light} pixels are BRIGHTER than the backdrop alone in the \
         `glow = 0` capture, so the stroke is still emitting light there. That \
         capture's whole purpose is to make the frame `backdrop * (1 - a)` and \
         nothing else — check that `glow` still scales the emitted colour and \
         not the coverage"
    );

    // A tenth of the footprint. Measured on this fixture: the fixed shader gives
    // **779 of 28 173 (2.77 %)** — the centreline — and the pre-fix one gives
    // **28 178 (100.02 %)**, the whole quad. So this arm reaches ~84 500
    // channels on the defect against the exact arm's **15**, which is the
    // three-orders-of-magnitude widening design-backlog 0041 asked for.
    //
    // Over 100 % is not a bug in the ratio: the footprint is counted from the
    // `dark` capture's strictly-non-zero colour, and a few anti-aliased edge
    // pixels round their colour to zero while still carrying alpha. They belong
    // to the quad; they just do not register as "drawn".
    //
    // The ceiling sits an order of magnitude below the defect and ~3.6x above
    // the passing value, so it is a floor on the *margin* rather than a tuned
    // constant.
    assert!(
        killed * 10 < footprint,
        "{killed} of the stroke's {footprint} footprint pixels are fully \
         extinguished with `glow = 0` — the frame there is `backdrop * (1 - a)`, \
         so that is alpha at 1 over a 2-D region rather than on the centreline. \
         A premultiplied stroke carries its coverage in alpha (ADR-0056); a \
         constant alpha 1 discards the backdrop across the whole quad and rims \
         the figure in black. The exact arm above moved {violations} channels on \
         the same run — it reaches 15 on this defect, which is why this wider \
         arm exists"
    );
}

// -----------------------------------------------------------------------
// The arc instance draws the same curve as the polyline (Plan 0087 Phase 1,
// ADR-0098)
// -----------------------------------------------------------------------

/// The control's render target. **Deliberately 4:3, not 16:9**: the aspect
/// ADR-0037 asks about is only observable where a grid-derived aspect and the
/// target's *disagree*, and at 1920x1080 (and at this box's 2048x1152) they
/// coincide exactly. Twice the golden suite's 128 so a stroke a few pixels
/// across still has an interior.
const ARC_W: u32 = 320;
const ARC_H: u32 = 240;
/// The aspect the draw is handed — the render target's, and the only one in
/// scope here.
const ARC_ASPECT: f32 = ARC_W as f32 / ARC_H as f32;

/// Samples in the polyline the arc is compared against. Dense enough that the
/// chord sagitta — `r * (1 - cos(pi / N))`, about 1e-5 world units at this N
/// and radius — is four orders below the stroke it is drawn with, so any
/// difference the comparison finds is the primitive's and not the sampling's.
const ARC_SAMPLES: usize = 512;

/// The stroke the comparison is drawn at, in NDC-y half-width — `thickness`
/// around 2, which is what the line presets actually ship (`presets/README.md`
/// gives the working range as 1.5 to 3.2).
const ARC_WIDTH: f32 = 0.006;

/// Render `segments` and `arcs` through one `LineRenderer` into a linear
/// `Rgba16Float` target and read the light back, unclamped and untonemapped.
///
/// `arc_capacity` is a parameter rather than derived from `arcs.len()` so a
/// caller can build a renderer *without* the arc pipeline and confirm it draws
/// no arcs. `softness` is a parameter because the two fragments share one
/// profile (ADR-0124) and the comparison below has to hold across its range,
/// not only at the default.
fn arc_capture(
    segments: &[SegmentInstance],
    arcs: &[super::ArcInstance],
    arc_capacity: usize,
    softness: f32,
) -> Option<Vec<f32>> {
    use crate::render::capture;
    use crate::render::context::RenderContext;

    const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

    let ctx = match RenderContext::new_headless(ARC_W, ARC_H, true) {
        Ok(ctx) => ctx,
        Err(_) => {
            eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
            return None;
        }
    };
    let texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("arc-control-target"),
        size: wgpu::Extent3d {
            width: ARC_W,
            height: ARC_H,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    // The pass loads rather than clears (the background pass owns the clear in
    // the shipped chain), so clear this target once here.
    let mut encoder = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("arc-control"),
        });
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("arc-control-clear"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });

    let mut renderer = if arc_capacity > 0 {
        super::LineRenderer::new_with_arcs(
            &ctx.device,
            FORMAT,
            segments.len().max(1),
            arc_capacity,
            "arc-control",
        )
    } else {
        super::LineRenderer::new(&ctx.device, FORMAT, segments.len().max(1), "arc-control")
    };
    renderer.draw_arcs(
        &ctx.queue,
        &mut encoder,
        &view,
        ARC_ASPECT,
        1.0,
        softness,
        ViewTransform::default(),
        segments,
        arcs,
    );

    let (buffer, padded_bpr) = capture::create_linear_readback(&ctx.device, ARC_W, ARC_H);
    capture::record_copy(&mut encoder, &texture, &buffer, padded_bpr, ARC_W, ARC_H);
    ctx.queue.submit(std::iter::once(encoder.finish()));
    Some(
        capture::read_back_linear(&ctx.device, &buffer, ARC_W, ARC_H, padded_bpr)
            .expect("read back the arc control"),
    )
}

/// A closed circle of `ARC_SAMPLES` **unjoined** segments about `centre`.
///
/// Unjoined on purpose: ADR-0041's join extends each flagged end by a half
/// width, so at this sampling density every vertex would overlap its
/// neighbours by most of a stroke and the additive composite would sum to
/// something far brighter than either primitive draws. That bead is the defect
/// ADR-0098 exists to remove, not the baseline the arc should match — the
/// question here is whether the two draw the same *curve*.
fn sampled_circle(centre: [f32; 2], radius: f32, width: f32) -> Vec<SegmentInstance> {
    let point = |k: usize| {
        let t = std::f32::consts::TAU * k as f32 / ARC_SAMPLES as f32;
        [centre[0] + radius * t.cos(), centre[1] + radius * t.sin()]
    };
    (0..ARC_SAMPLES)
        .map(|k| SegmentInstance {
            a: point(k),
            b: point(k + 1),
            color: [1.0, 1.0, 1.0],
            width,
            joined: 0,
            alpha: 1.0,
        })
        .collect()
}

/// Mean per-channel difference and the largest single-channel byte outlier
/// between two linear captures — the golden suite's own two statistics
/// (`core/tests/golden.rs`), computed the same way so its tolerances mean here
/// what they mean there.
fn arc_diff(a: &[f32], b: &[f32]) -> (f32, u8) {
    let (mut sum, mut worst) = (0.0f64, 0.0f32);
    let mut n = 0usize;
    for (x, y) in a.chunks_exact(4).zip(b.chunks_exact(4)) {
        for c in 0..3 {
            let (x, y) = (x[c].clamp(0.0, 1.0), y[c].clamp(0.0, 1.0));
            let d = (x - y).abs();
            sum += d as f64;
            if d > worst {
                worst = d;
            }
            n += 1;
        }
    }
    (
        (sum / n.max(1) as f64) as f32,
        (worst * 255.0).round().min(255.0) as u8,
    )
}

/// The golden suite's tolerances, quoted rather than re-invented — Phase 1's
/// done-when is "within the golden suite's own drift tolerance", and these are
/// the two numbers `core/tests/golden.rs` holds every baseline to.
/// The `softness` at which the profile is the **pre-Plan-0114 fragment, term
/// for term** — `g = u²`, the shape every closed form in this file is written
/// against.
///
/// Named rather than spelled `1.0` inline, and deliberately **not**
/// [`DEFAULT_SOFTNESS`](super::super::DEFAULT_SOFTNESS): Plan 0114 Phase 5
/// moved the default to 0.25 on a look gate, and it will move again. What
/// these assertions are about is the profile, not the library's current
/// taste.
const SOFT_PROFILE: f32 = 1.0;

const ARC_MEAN_TOL: f32 = 0.02;
const ARC_OUTLIER_TOL: u8 = 48;

/// **An arc draws the same curve a densely-sampled polyline of it does**, at a
/// non-16:9 target, to within the golden suite's own drift tolerance.
///
/// This is the primitive's whole claim: one instance with no vertices at any
/// resolution is a *drawing of the same circle*, not a different look. The two
/// captures differ only in how the stroke's distance is found — the polyline
/// interpolates a coordinate across a quad per chord, the arc evaluates
/// `abs(length(p - c) - r)` per pixel — so where they agree, the per-pixel
/// distance field is the same picture.
///
/// # Why the aspect check bites here and nowhere else
///
/// The arc is authored in world space and the target is 4:3, so the circle
/// reaches NDC x of `r / aspect` and NDC y of `r`. Take the aspect from
/// anything but the render target and the two primitives stop agreeing:
/// the polyline's squash comes from the vertex shader's divide (which is
/// handed the target's aspect by `draw`), and a wrongly-sourced aspect in the
/// arc shader squashes its ellipse by a different factor. **Verified to bite**
/// by temporarily replacing the arc fragment's `u.v.x` with a fixed 1.0: this
/// comparison goes from mean 0.0000 / outlier 1 to mean **0.0044** / outlier
/// **255**, and the lit-pixel count falls from 628 to 408.
///
/// **It is the outlier arm that convicts, and that is worth knowing before
/// anyone tunes these two numbers.** A wrongly-sourced aspect moves a thin
/// closed curve, so it is wrong by everything on a few hundred pixels and
/// right on the other seventy-six thousand — the mean lands at 0.0044 and
/// stays comfortably *inside* the 0.02 the golden suite allows. Dropping the
/// max-outlier arm would leave this test green on the bug it exists for.
///
/// # The `softness` sweep (Plan 0114 Phase 2)
///
/// Once the profile became a parameter, this comparison became the guard on
/// **both fragments sharing it**: the arc's distance field and the segment's
/// interpolated coordinate are different expressions, so the two pictures can
/// only keep agreeing across the range if the profile they feed is one
/// definition. That is why [`arc_shader_source`](super::arc_shader_source)
/// prepends it rather than the fragment restating it.
///
/// It is run at three `softness` values, and **at this width all three draw the
/// same picture** — deliberately. 0.006 NDC-y is 0.72 px of half-width at a
/// 240-row target, inside the regime where the edge term reaches its cap and the
/// ramp is the whole half-width whatever `softness` asks for. So what this
/// asserts across the sweep is that the arc keeps drawing the *same curve* at
/// every value, including the sub-pixel case where the parameter is inert; the
/// proof that the two fragments share one **profile** is
/// [`the_arc_stroke_falls_off_quadratically_like_a_segment`], which resolves it
/// against a single straight segment at a width that has an interior.
///
/// A fatter stroke here would not do that job: at 512 samples the chords are
/// shorter than a pixel, and a polyline of sub-pixel slivers seams — measured at
/// 0.04 NDC-y the two disagree by a **whole pixel of the bright core** (outlier
/// 243) at the unchanged default profile. The dense polyline is a reference for
/// the *curve*, and only while the stroke is thin.
///
/// The aspect does not enter the new term. The arc's `d` is an NDC distance and
/// its `width` is flat-interpolated, so `fwidth(d / width)` is
/// `fwidth(d) / width` — the aspect divides out with the distance it was already
/// applied to, and the edge stays one pixel of this 4:3 target.
#[test]
fn an_arc_draws_the_same_curve_as_a_dense_polyline() {
    const CENTRE: [f32; 2] = [0.1, -0.05];
    const RADIUS: f32 = 0.55;

    let segments = sampled_circle(CENTRE, RADIUS, ARC_WIDTH);
    let arc = super::ArcInstance {
        centre: CENTRE,
        radius: RADIUS,
        angle_start: 0.0,
        angle_sweep: std::f32::consts::TAU,
        color: [1.0, 1.0, 1.0],
        width: ARC_WIDTH,
    };

    for softness in [SOFT_PROFILE, 0.5, 0.0] {
        let Some(polyline) = arc_capture(&segments, &[], 0, softness) else {
            return;
        };
        let Some(drawn) = arc_capture(&[], &[arc], 4, softness) else {
            return;
        };

        // Non-vacuity first: two black frames agree perfectly and prove nothing.
        let lit = |px: &[f32]| px.chunks_exact(4).filter(|t| t[0] > 0.01).count();
        let (lit_polyline, lit_arc) = (lit(&polyline), lit(&drawn));
        let total = (ARC_W * ARC_H) as usize;
        let (mean, outlier) = arc_diff(&polyline, &drawn);
        eprintln!(
            "arc vs {ARC_SAMPLES}-segment polyline at {ARC_W}x{ARC_H} (aspect \
             {ARC_ASPECT:.4}), softness {softness}: {lit_polyline} lit pixels \
             vs {lit_arc} of {total}; mean {mean:.4} (tol {ARC_MEAN_TOL}) \
             outlier {outlier} (tol {ARC_OUTLIER_TOL})"
        );
        assert!(
            lit_polyline * 200 > total && lit_arc * 200 > total,
            "at softness {softness} one of the two captures is nearly empty \
             ({lit_polyline} and {lit_arc} of {total} pixels lit) — the \
             comparison below would pass on black against black"
        );

        assert!(
            mean <= ARC_MEAN_TOL && outlier <= ARC_OUTLIER_TOL,
            "at softness {softness} the arc and a {ARC_SAMPLES}-segment \
             polyline of the same circle draw different pictures: mean \
             {mean:.4} (tol {ARC_MEAN_TOL}), worst single-channel byte \
             {outlier} (tol {ARC_OUTLIER_TOL})"
        );
    }
}

/// **The stroke profile is the segment path's**: at the default a bright core
/// falling quadratically to zero at the stroke edge, not a flat bar and not a
/// linear ramp — and **at every `softness`, whatever shape that is, the same
/// shape on both primitives**.
///
/// Read off the column through the circle's centre, where the arc runs
/// horizontally and its distance is measured along y — so the profile is
/// sampled in the very axis the half-width is quoted in and no aspect
/// conversion enters. That makes the expected value a **closed form** rather
/// than a shape to match: a pixel whose centre sits at NDC `y` is exactly
/// `|y - radius|` from the circle there, so at `softness = 1` the fragment must
/// emit `(1 - d / width)^2` and nothing else.
///
/// # This is where the two fragments are held to one profile (Plan 0114 Phase 2)
///
/// The arc's distance field and the segment's interpolated coordinate are
/// different expressions reaching the same `stroke_coverage`, prepended to both
/// modules by [`arc_shader_source`](super::arc_shader_source). Two hand-kept
/// copies would compile and render, and the symptom would be a mandala whose
/// circles and interlace stop matching. So the cross-sections are compared at
/// **both ends of the `softness` range and one value between**, against a single
/// straight segment rather than against a polyline — one quad has no seams and
/// no unjoined corners, which is what makes the comparison survive a hard edge.
///
/// The last assertion is the one that keeps the rest from passing vacuously: the
/// three cuts must actually differ from each other. Both fragments ignoring
/// `softness` would satisfy every equality above.
#[test]
fn the_arc_stroke_falls_off_quadratically_like_a_segment() {
    const CENTRE: [f32; 2] = [0.0, 0.0];
    const RADIUS: f32 = 0.5;
    // Fat enough that a dozen rows land across the half-width; the property
    // does not depend on the width, only the resolution of the check does.
    const WIDTH: f32 = 0.08;
    // The composite is `Rgba16Float`, so a value near 1 is stored to about
    // 1/1024; the rest is the rasterizer agreeing with the pixel-centre
    // convention this reconstructs `d` from. It is slack, not a tolerance: the
    // properties below are exact equalities in real arithmetic.
    const SLACK: f32 = 0.01;

    let arc = super::ArcInstance {
        centre: CENTRE,
        radius: RADIUS,
        angle_start: 0.0,
        angle_sweep: std::f32::consts::TAU,
        color: [1.0, 1.0, 1.0],
        width: WIDTH,
    };
    // The other half of "like a segment": the same cut through the segment
    // path's own stroke must land on the same numbers. Without it the closed
    // form below pins the arc to a formula rather than to its sibling.
    let flat = [SegmentInstance {
        a: [-1.0, RADIUS],
        b: [1.0, RADIUS],
        color: [1.0, 1.0, 1.0],
        width: WIDTH,
        joined: 0,
        alpha: 1.0,
    }];

    // `color` and the glow multiplier are both 1 and the target was cleared to
    // black, so the red channel *is* the fragment's coverage `g`.
    let column = (ARC_W / 2) as usize;
    // Pixel centres sit at +0.5, and row 0 is NDC y = +1.
    let ndc_y = |row: usize| 1.0 - 2.0 * (row as f32 + 0.5) / ARC_H as f32;
    let rows: Vec<usize> = (0..(ARC_H / 2) as usize)
        .filter(|row| (ndc_y(*row) - RADIUS).abs() <= WIDTH)
        .collect();

    let mut cuts: Vec<(f32, Vec<f32>)> = Vec::new();
    for softness in [SOFT_PROFILE, 0.5, 0.0] {
        let Some(drawn) = arc_capture(&[], &[arc], 4, softness) else {
            return;
        };
        let Some(segment) = arc_capture(&flat, &[], 0, softness) else {
            return;
        };
        let at = |px: &[f32], row: usize| px[(row * ARC_W as usize + column) * 4];

        let cut: Vec<f32> = rows.iter().map(|row| at(&drawn, *row)).collect();
        let peak = cut.iter().copied().fold(0.0f32, f32::max);
        let worst_pair = rows.iter().fold(0.0f32, |acc, row| {
            acc.max((at(&drawn, *row) - at(&segment, *row)).abs())
        });
        eprintln!(
            "softness {softness} at {ARC_W}x{ARC_H}: {} rows across the \
             stroke, peak {peak:.4}; worst arc-vs-segment per-row difference \
             {worst_pair:.4}",
            rows.len()
        );
        assert!(
            rows.len() >= 16 && peak > 0.8,
            "only {} rows fell across the stroke and the brightest reached \
             {peak:.4} — the profile is not resolved and the assertions below \
             would be vacuous",
            rows.len()
        );
        assert!(
            worst_pair <= SLACK,
            "at softness {softness} the arc's stroke and a straight segment's \
             differ by {worst_pair:.4} across their own cross-sections — the \
             two primitives are not drawing the same stroke"
        );

        if softness == SOFT_PROFILE {
            // `1.0` is still the pre-Plan-0114 fragment, term for term — that is
            // what the golden corpus rests on, and it is a value a preset can
            // still bind. It is NOT the default — that is 0.25 (Plan 0114
            // Phase 5) — so this arm names the number it is about.
            let worst = rows.iter().fold(0.0f32, |acc, row| {
                let d = (ndc_y(*row) - RADIUS).abs();
                let falloff = 1.0 - d / WIDTH;
                acc.max((at(&drawn, *row) - falloff * falloff).abs())
            });
            eprintln!(
                "arc stroke profile at softness {SOFT_PROFILE}: worst \
                 |measured - (1 - d/w)^2| {worst:.4}"
            );
            assert!(
                worst <= SLACK,
                "the arc's across-the-stroke profile departs from \
                 `(1 - d/w)^2` by {worst:.4} (slack {SLACK}) — a flat bar, a \
                 linear ramp or a stroke of the wrong width here would all be \
                 a different look drawn on the same curve"
            );
        }
        cuts.push((softness, cut));
    }

    // **The sweep has to move something.** A `softness` both fragments ignored
    // would satisfy every equality above, on three identical pictures.
    for pair in cuts.windows(2) {
        let ((soft_hi, hi), (soft_lo, lo)) = (&pair[0], &pair[1]);
        let moved = hi
            .iter()
            .zip(lo)
            .fold(0.0f32, |acc, (a, b)| acc.max((a - b).abs()));
        eprintln!("softness {soft_hi} vs {soft_lo}: cross-sections differ by {moved:.4}");
        assert!(
            moved > 0.1,
            "softness {soft_hi} and {soft_lo} draw the same cross-section \
             (worst row difference {moved:.4}) — the parameter is reaching the \
             fragments as a no-op, so the agreement asserted above proves \
             nothing about the profile they share"
        );
    }
}

/// A renderer built **without** the arc pipeline draws no arcs, rather than
/// panicking or silently binding the segment pipeline to arc instances.
///
/// The capability is opt-in for the reason `over_pipeline` records — an unused
/// pipeline still allocates, and on WARP a changed allocation order changes a
/// later pass — so "did not ask for it" has to be a defined state and not an
/// accident.
#[test]
fn a_renderer_without_the_arc_pipeline_draws_no_arcs() {
    let arc = super::ArcInstance {
        centre: [0.0, 0.0],
        radius: 0.5,
        angle_start: 0.0,
        angle_sweep: std::f32::consts::TAU,
        color: [1.0, 1.0, 1.0],
        width: 0.02,
    };
    let Some(drawn) = arc_capture(&[], &[arc], 0, super::super::DEFAULT_SOFTNESS) else {
        return;
    };
    let lit = drawn.chunks_exact(4).filter(|t| t[0] > 0.001).count();
    assert_eq!(
        lit, 0,
        "{lit} pixels are lit by a renderer that was never given an arc \
         pipeline"
    );
}

/// An arc's **angular span** is honoured: a quarter turn lights roughly a
/// quarter of what the full circle does, and it lights it on the side the span
/// names.
///
/// The count is a ratio against the arc's own full circle rather than an
/// absolute pixel number, so it stays a property of the geometry rather than a
/// number recorded off this capture size (ADR-0071).
#[test]
fn an_arcs_angular_span_is_what_it_draws() {
    const RADIUS: f32 = 0.6;
    const WIDTH: f32 = 0.02;
    let arc = |start: f32, sweep: f32| super::ArcInstance {
        centre: [0.0, 0.0],
        radius: RADIUS,
        angle_start: start,
        angle_sweep: sweep,
        color: [1.0, 1.0, 1.0],
        width: WIDTH,
    };
    let quarter = std::f32::consts::FRAC_PI_2;

    let Some(full) = arc_capture(
        &[],
        &[arc(0.0, std::f32::consts::TAU)],
        4,
        super::super::DEFAULT_SOFTNESS,
    ) else {
        return;
    };
    let Some(first) = arc_capture(&[], &[arc(0.0, quarter)], 4, super::super::DEFAULT_SOFTNESS)
    else {
        return;
    };
    let lit = |px: &[f32]| px.chunks_exact(4).filter(|t| t[0] > 0.01).count();
    // Which half of the frame the light is in. Rows count down from NDC y = +1,
    // so the first quadrant (angles 0..pi/2) is the TOP right.
    let quadrant = |px: &[f32]| {
        let mut counts = [0usize; 4];
        for (i, t) in px.chunks_exact(4).enumerate() {
            if t[0] <= 0.01 {
                continue;
            }
            let (x, y) = (i % ARC_W as usize, i / ARC_W as usize);
            let right = x >= ARC_W as usize / 2;
            let top = y < ARC_H as usize / 2;
            counts[usize::from(right) + 2 * usize::from(top)] += 1;
        }
        counts
    };

    let (lit_full, lit_first) = (lit(&full), lit(&first));
    let counts = quadrant(&first);
    eprintln!(
        "arc span: full circle {lit_full} lit, quarter {lit_first} lit \
         (ratio {:.3}); the quarter's quadrant counts (bl, br, tl, tr) are \
         {counts:?}",
        lit_first as f32 / lit_full.max(1) as f32
    );
    assert!(
        lit_full > 0 && lit_first > 0,
        "nothing drew ({lit_full} and {lit_first} lit)"
    );
    // A quarter of the circle plus two endpoint caps, each a half-disc of the
    // stroke's own radius — so a little over a quarter, never a half.
    let ratio = lit_first as f32 / lit_full as f32;
    assert!(
        (0.2..0.35).contains(&ratio),
        "a quarter-turn arc lit {ratio:.3} of what the full circle lit, which \
         is not a quarter plus its two endpoint caps"
    );
    // Top-right is index 3 by the packing above. Everything else is a cap
    // spilling a pixel or two over an axis, so the quadrant the span names must
    // dominate rather than merely lead.
    let elsewhere: usize = counts[0] + counts[1] + counts[2];
    assert!(
        counts[3] > elsewhere * 4,
        "the 0..pi/2 arc put {} pixels in the top-right quadrant against \
         {elsewhere} everywhere else — the span is not selecting the side it \
         names",
        counts[3]
    );
}

// -----------------------------------------------------------------------
// The opacity-preserving seam (Plan 0123 Phase 7, ADR-0138)
// -----------------------------------------------------------------------

/// Two crossing strokes of two different inks, drawn either additively or over,
/// read back as linear light. The **second** stroke in the batch is the later
/// one, so it is the one that must survive in the overlap.
///
/// A sibling of [`profile_capture`] for the reason that one is a sibling of
/// [`arc_capture`]: the property here is about the colour of specific pixels, so
/// the target's size and the batch's composition both belong to this fixture.
/// One context per call, as everything else in this file does.
fn overlap_capture(opaque: bool) -> Option<Vec<f32>> {
    use crate::render::capture;
    use crate::render::context::RenderContext;

    const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
    const SIZE: u32 = 64;

    let ctx = match RenderContext::new_headless(SIZE, SIZE, true) {
        Ok(ctx) => ctx,
        Err(_) => {
            eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
            return None;
        }
    };
    let texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("overlap-target"),
        size: wgpu::Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    let mut encoder = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("overlap-capture"),
        });
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("overlap-clear"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });

    // Two fat strokes crossing at the centre, wide enough that the overlap has a
    // real interior rather than only the coverage ramps of two edges. Red first,
    // then white: the property names the LATER one.
    let segments = [
        SegmentInstance {
            a: [-4.0, 0.0],
            b: [4.0, 0.0],
            color: [1.0, 0.0, 0.0],
            width: 0.30,
            joined: 0,
            alpha: 1.0,
        },
        SegmentInstance {
            a: [0.0, -4.0],
            b: [0.0, 4.0],
            color: [1.0, 1.0, 1.0],
            width: 0.30,
            joined: 0,
            alpha: 1.0,
        },
    ];

    let mut renderer =
        super::LineRenderer::new_split_with_arcs(&ctx.device, FORMAT, segments.len(), 0, "overlap");
    // `softness = 0`: a solid stroke with a one-pixel edge, so the centre of the
    // overlap is at full coverage and the property is read where it is stated —
    // in the INTERIOR, not on a ramp.
    if opaque {
        renderer.draw_opaque(
            &ctx.queue,
            &mut encoder,
            &view,
            1.0,
            1.0,
            0.0,
            ViewTransform::default(),
            &segments,
            &[],
        );
    } else {
        renderer.draw(
            &ctx.queue,
            &mut encoder,
            &view,
            1.0,
            1.0,
            0.0,
            ViewTransform::default(),
            &segments,
        );
    }

    let (buffer, padded_bpr) = capture::create_linear_readback(&ctx.device, SIZE, SIZE);
    capture::record_copy(&mut encoder, &texture, &buffer, padded_bpr, SIZE, SIZE);
    ctx.queue.submit(std::iter::once(encoder.finish()));
    Some(
        capture::read_back_linear(&ctx.device, &buffer, SIZE, SIZE, padded_bpr)
            .expect("read back the overlap capture"),
    )
}

/// The `(r, g, b)` of the pixel at the centre of a `SIZE`-square linear capture.
fn centre_rgb(pixels: &[f32], size: u32) -> (f32, f32, f32) {
    let mid = size / 2;
    let i = ((mid * size + mid) * 4) as usize;
    (pixels[i], pixels[i + 1], pixels[i + 2])
}

/// **The property ADR-0138's first delivery is for**: in the *interior* of an
/// overlap, two strokes of two different inks give the colour of the **later**
/// stroke, not the sum of the two.
///
/// Stated for the interior because a stroke's edge is a coverage ramp and the
/// property there is a blend by construction — a different claim, and one this
/// fixture deliberately does not make. The centre pixel is interior to both
/// strokes at these widths.
///
/// The additive control is what makes this non-vacuous: the same two strokes
/// through the additive seam **do** sum, which is the behaviour the limited-ink
/// class exists to opt out of, and the reason a quantized palette's plateaus
/// vanish on every line scene that cannot.
#[test]
fn the_opaque_seam_replaces_the_interior_of_an_overlap_where_additive_sums_it() {
    const SIZE: u32 = 64;
    // Slack for the half-precision target and the profile's one-pixel edge term
    // at the centre. Both properties below are exact in real arithmetic.
    const SLACK: f32 = 0.02;

    let Some(additive) = overlap_capture(false) else {
        return;
    };
    let Some(opaque) = overlap_capture(true) else {
        return;
    };

    let (ar, ag, ab) = centre_rgb(&additive, SIZE);
    let (or_, og, ob) = centre_rgb(&opaque, SIZE);

    // The later stroke is white, so every channel is up. Green and blue are the
    // discriminating pair: the earlier stroke is pure red and contributes
    // nothing to them, so they cannot separate the two seams — but red can.
    assert!(
        og > 1.0 - SLACK && ob > 1.0 - SLACK,
        "the later (white) stroke must own the overlap interior under OVER, \
         got ({or_}, {og}, {ob})"
    );
    assert!(
        (or_ - 1.0).abs() < SLACK,
        "under OVER the interior is the later stroke's red, 1.0 — not the sum of \
         two reds; got {or_}"
    );

    // The control. Additive light sums the two reds, so the interior is over
    // range where the OVER seam holds it at the ink's own value.
    assert!(
        ar > 1.0 + 4.0 * SLACK,
        "THE ADDITIVE CONTROL must sum the two reds past the ink's own 1.0, or \
         this test is not separating two seams; got {ar} against {or_}"
    );
    assert!(
        ag > 1.0 - SLACK && ab > 1.0 - SLACK,
        "sanity: the additive control's white stroke lights every channel, \
         got ({ar}, {ag}, {ab})"
    );
}

// -----------------------------------------------------------------------
// The across-the-stroke profile (Plan 0114 Phase 1, ADR-0124)
// -----------------------------------------------------------------------

/// Render `segments` at `softness` into a linear `Rgba16Float` target of the
/// given size and read the light back, unclamped and untonemapped.
///
/// A sibling of [`arc_capture`] rather than a parameter on it: these fixtures
/// are about **pixels of the render target**, so the size has to be theirs and
/// the aspect has to be derived from it (ADR-0037). Colour is white and `glow`
/// is 1 in every caller, so the red channel *is* the fragment's coverage `g`.
///
/// Builds and drops one context per call, for the reason `linear_composite`
/// records: a second live device in a binary is what the software adapter falls
/// over on.
fn profile_capture(
    width: u32,
    height: u32,
    softness: f32,
    segments: &[SegmentInstance],
) -> Option<Vec<f32>> {
    use crate::render::capture;
    use crate::render::context::RenderContext;

    const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

    let ctx = match RenderContext::new_headless(width, height, true) {
        Ok(ctx) => ctx,
        Err(_) => {
            eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
            return None;
        }
    };
    let texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("profile-target"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    let mut encoder = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("profile-capture"),
        });
    // The line pass loads rather than clears (the background pass owns the clear
    // in the shipped chain), so clear this target once here.
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("profile-clear"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });

    let mut renderer =
        super::LineRenderer::new(&ctx.device, FORMAT, segments.len().max(1), "profile");
    renderer.draw(
        &ctx.queue,
        &mut encoder,
        &view,
        width as f32 / height as f32,
        1.0,
        softness,
        ViewTransform::default(),
        segments,
    );

    let (buffer, padded_bpr) = capture::create_linear_readback(&ctx.device, width, height);
    capture::record_copy(&mut encoder, &texture, &buffer, padded_bpr, width, height);
    ctx.queue.submit(std::iter::once(encoder.finish()));
    Some(
        capture::read_back_linear(&ctx.device, &buffer, width, height, padded_bpr)
            .expect("read back the profile capture"),
    )
}

/// Half-precision slack for the pixel-coordinate closed forms below. It is
/// slack, not a tolerance: both properties are exact equalities in real
/// arithmetic.
const PIXEL_PROFILE_SLACK: f32 = 0.02;

/// A stroke spanning the whole frame horizontally at NDC y = 0. World x reaches
/// well past the frame at either edge, so every column is interior and the
/// column read below never lands on an end cap.
fn flat_stroke(half_width: f32) -> [SegmentInstance; 1] {
    [SegmentInstance {
        a: [-4.0, 0.0],
        b: [4.0, 0.0],
        color: [1.0, 1.0, 1.0],
        width: half_width,
        joined: 0,
        alpha: 1.0,
    }]
}

/// A stroke crossing the frame at a **deliberately unremarkable slope**, so the
/// perpendicular offsets of the pixel centres it covers equidistribute rather
/// than repeating on the lattice. Every one of its pixels is interior at the
/// sizes below, so the peak it reaches is the profile's and not an end cap's.
fn slanted_stroke(half_width: f32) -> [SegmentInstance; 1] {
    [SegmentInstance {
        a: [-1.55, -0.71],
        b: [1.55, 0.69],
        color: [1.0, 1.0, 1.0],
        width: half_width,
        joined: 0,
        alpha: 1.0,
    }]
}

/// NDC y of the centre of `row`, at a target `height` rows tall. Row 0 is
/// NDC y = +1 and pixel centres sit at +0.5.
fn row_ndc_y(row: usize, height: u32) -> f32 {
    1.0 - 2.0 * (row as f32 + 0.5) / height as f32
}

/// **The edge term can never exceed the softness term** — the cap in the shared
/// profile, and the single fact `softness = 1.0` being byte-identical rests on.
///
/// `d` is normalized across the half-width, so `fwidth(d)` is roughly
/// `1 / half-width-in-pixels`: about 0.41 at `thickness = 1.5` and 0.19 at
/// `3.2`, the range the shipped line presets span. A **sub-pixel** stroke drives
/// it the other way, above 1.0, and there an uncapped `max(softness, edge)`
/// divides `u` down — dimming the line instead of sharpening it, and breaking
/// byte-identity at the default.
///
/// # Why these three configurations
///
/// The deep arm is synthetic — `thickness = 0.1`, which is inside the dead zone
/// [`MIN_USEFUL_THICKNESS`](super::super::MIN_USEFUL_THICKNESS) describes, so it
/// floors to [`MIN_HALF_WIDTH`](super::super::MIN_HALF_WIDTH) and lands at about
/// a quarter pixel. The other two are **real shipped geometry**:
/// `warp_mesh::draw`'s `THIN = 0.0025` NDC-y, the width every MilkDrop waveform,
/// motion vector and thin border is stroked at, at the two target sizes ADR-0124
/// reasons about. It is 1.35 px of half-width at 1080p and **1.0 px** at
/// 1280x800, and byte-identity for `warp_mesh` — which is pinned at `1.0` and
/// has no golden baseline shading a stroke — depends on the cap holding there.
///
/// At `softness = 1.0` the capped profile puts `g = u²`, so a pixel centre
/// landing on the centreline reads **1.0** whatever the stroke's width. Uncapped
/// it would read `min(1, 1/fwidth)²`, which at these three widths is about
/// **0.51**, **0.037** and **0.93** respectively — so the 1280x800 arm and the
/// deep arm each convict on their own, and the 1080p arm is the control just
/// inside the cap, where the two expressions very nearly agree.
#[test]
fn the_edge_term_never_exceeds_the_softness_term() {
    /// `warp_mesh::draw`'s `THIN`, the half-width every MilkDrop waveform,
    /// motion vector and thin border is stroked at. Private there; restated
    /// here because this fixture's whole point is that it is *that* geometry.
    const MILKDROP_THIN: f32 = 0.0025;

    let cases: [(&str, u32, u32, f32); 3] = [
        ("warp_mesh THIN at 1280x800", 1280, 800, MILKDROP_THIN),
        (
            "thickness = 0.1 at 1080p",
            1920,
            1080,
            super::super::half_width(0.1),
        ),
        ("warp_mesh THIN at 1080p", 1920, 1080, MILKDROP_THIN),
    ];

    for (label, width, height, half_width) in cases {
        let Some(drawn) = profile_capture(
            width,
            height,
            super::super::DEFAULT_SOFTNESS,
            &slanted_stroke(half_width),
        ) else {
            return;
        };
        let half_px = half_width * height as f32 / 2.0;
        let lit = drawn.chunks_exact(4).filter(|t| t[0] > 0.01).count();
        let peak = drawn.chunks_exact(4).fold(0.0f32, |acc, t| acc.max(t[0]));
        // The ramp an uncapped `max(softness, edge)` would divide by, and the
        // peak it would leave. A diagonal stroke's `fwidth` is
        // `(|sin| + |cos|) / half_px`, about `1.4 / half_px` at this slope.
        let uncapped_ramp = 1.4 / half_px;
        let uncapped_peak = (1.0f32 / uncapped_ramp.max(1.0)).powi(2);
        eprintln!(
            "{label}: half-width {half_px:.2} px, {lit} lit pixels, peak \
             coverage {peak:.4}; uncapped this would peak at about \
             {uncapped_peak:.3}"
        );
        assert!(
            lit > 200,
            "{label}: only {lit} pixels lit — the peak below would be read off \
             almost nothing"
        );
        assert!(
            peak >= 0.95,
            "{label}: a stroke {half_px:.2} px of half-width peaks at \
             {peak:.4} at softness = 1.0, where the profile is `g = u²` and a \
             pixel centre on the centreline must read 1.0. The edge term has \
             escaped its cap and is dimming the stroke instead of sharpening \
             it (uncapped, this configuration peaks at about {uncapped_peak:.3})"
        );
    }
}

/// **The edge is a width in pixels of the render target, not a fraction of the
/// stroke** (ADR-0124, the other side of ADR-0037).
///
/// At `softness = 0` the ramp is the edge term alone, so the coverage a pixel
/// carries is a function of **how many pixels inside the stroke edge it sits**
/// and of nothing else:
///
/// ```text
/// g(p) = min(p, 1)²    p = pixels inward from the stroke edge
/// ```
///
/// There is no resolution in that expression, which is the property. It is
/// asserted at 1920x1080 and at 1280x800 against the *same* NDC half-width, so
/// the stroke itself is 27 px across at one and 20 px at the other and the one
/// pixel of ramp is a visibly different **share** of it — 3.7 % against 5 %.
///
/// Stated as the closed form rather than as a ramp-width count because a
/// one-pixel ramp spans at most one row of pixel centres per side: counting rows
/// would be a comparison of small integers that a broken implementation could
/// pass, where matching `min(p, 1)²` across forty rows cannot be.
#[test]
fn the_edge_is_a_width_in_pixels_not_a_fraction_of_the_stroke() {
    /// Fat enough that a couple of dozen rows land across the half-width, so
    /// the plateau the closed form describes is resolved rather than inferred.
    const HALF_WIDTH: f32 = 0.05;

    let mut half_px_seen: Vec<f32> = Vec::new();
    for (width, height) in [(1920u32, 1080u32), (1280, 800)] {
        let Some(drawn) = profile_capture(width, height, 0.0, &flat_stroke(HALF_WIDTH)) else {
            return;
        };
        let half_px = HALF_WIDTH * height as f32 / 2.0;
        half_px_seen.push(half_px);

        let column = (width / 2) as usize;
        let (mut worst, mut checked, mut peak) = (0.0f32, 0usize, 0.0f32);
        for row in 0..height as usize {
            let d = row_ndc_y(row, height).abs();
            if d > HALF_WIDTH {
                continue; // outside the stroke; the fragment writes nothing
            }
            // Pixels inward from the stroke edge — the coordinate the property
            // is stated in, derived from the geometry and the target height.
            let p = (HALF_WIDTH - d) * height as f32 / 2.0;
            let want = p.min(1.0).powi(2);
            let got = drawn[(row * width as usize + column) * 4];
            worst = worst.max((got - want).abs());
            peak = peak.max(got);
            checked += 1;
        }
        eprintln!(
            "{width}x{height}: half-width {half_px:.1} px, {checked} rows \
             across the stroke, peak {peak:.4}, worst |measured - min(p,1)^2| \
             {worst:.4}; the one-pixel ramp is {:.1} % of the half-width",
            100.0 / half_px
        );
        assert!(
            checked >= 32 && peak > 0.95,
            "{width}x{height}: {checked} rows fell across the stroke and the \
             brightest reached {peak:.4} — the profile is not resolved and the \
             assertion below would be vacuous"
        );
        assert!(
            worst <= PIXEL_PROFILE_SLACK,
            "{width}x{height}: the coverage departs from `min(p, 1)^2` by \
             {worst:.4} (slack {PIXEL_PROFILE_SLACK}), where `p` is pixels inward \
             from the stroke edge. A ramp specified as a fraction of the \
             stroke would fit this form at one resolution and miss it at the \
             other"
        );
    }

    // Non-vacuity for the *pair*: the two targets must actually differ in how
    // many pixels the stroke spans, or the closed form above was checked twice
    // against the same geometry.
    let (hi, lo) = (half_px_seen[0], half_px_seen[1]);
    assert!(
        hi > lo * 1.2,
        "the two targets stroke {hi:.1} px and {lo:.1} px of half-width — too \
         close for the ramp's SHARE of the stroke to differ, which is the half \
         of this property the pixel count cannot show"
    );
}

/// **A low `softness` puts a plateau across the stroke** — ADR-0124's
/// defect, read off the cross-section rather than off a
/// total-brightness statistic, which moves for several reasons.
///
/// The statistic is the one Plan 0114's own measurement table uses: **how many
/// rows sit within 10 % of the peak**. At `softness = 1.0` that is a handful —
/// a 14 px stroke's 4 px spine, the reading that produced the *blurred* verdict.
/// As `softness` falls the solid core grows toward the whole half-width, and it
/// grows **inside a fixed footprint**: `softness` redistributes coverage across
/// the stroke, it does not widen it, which is what keeps it a separate lever
/// from `thickness`.
#[test]
fn a_low_softness_puts_a_plateau_across_the_stroke() {
    const HALF_WIDTH: f32 = 0.05;
    const W: u32 = 1280;
    const H: u32 = 800;

    let mut plateaus: Vec<(f32, usize, usize)> = Vec::new();
    for softness in [1.0f32, 0.5, 0.25, 0.0] {
        let Some(drawn) = profile_capture(W, H, softness, &flat_stroke(HALF_WIDTH)) else {
            return;
        };
        let column = (W / 2) as usize;
        let cut: Vec<f32> = (0..H as usize)
            .filter(|row| row_ndc_y(*row, H).abs() <= HALF_WIDTH * 1.05)
            .map(|row| drawn[(row * W as usize + column) * 4])
            .collect();
        let peak = cut.iter().copied().fold(0.0f32, f32::max);
        let plateau = cut.iter().filter(|g| **g >= 0.9 * peak).count();
        let footprint = cut.iter().filter(|g| **g > 1e-3).count();
        let bytes: Vec<u8> = cut
            .iter()
            .map(|g| (g.clamp(0.0, 1.0) * 255.0).round() as u8)
            .collect();
        eprintln!(
            "softness {softness}: peak {peak:.4}, {plateau} of {footprint} \
             rows within 10 % of it; cross-section {bytes:?}"
        );
        assert!(
            peak > 0.95,
            "softness {softness}: the stroke peaked at {peak:.4}"
        );
        plateaus.push((softness, plateau, footprint));
    }

    // Monotone: every drop in `softness` widens the solid core.
    for pair in plateaus.windows(2) {
        let ((soft_hi, wide_lo, _), (soft_lo, wide_hi, _)) = (pair[0], pair[1]);
        assert!(
            wide_hi > wide_lo,
            "softness {soft_lo} holds {wide_hi} rows within 10 % of peak \
             against {soft_hi}'s {wide_lo} — lowering `softness` did not widen \
             the plateau"
        );
    }

    // The two ends, quantified. At `1.0` the profile is `u²`, so 10 % off peak
    // is `u >= 0.949` — about a twentieth of the stroke. At `0` it is the whole
    // stroke bar its one-pixel edge.
    let (_, spine, footprint) = plateaus[0];
    let (_, solid, footprint_solid) = plateaus[3];
    assert!(
        solid >= 8 * spine,
        "the solid stroke holds {solid} rows within 10 % of peak against the \
         soft one's {spine} — `softness = 0` is not producing a plateau"
    );
    assert!(
        footprint.abs_diff(footprint_solid) <= 2,
        "the stroke's footprint moved from {footprint} rows to \
         {footprint_solid} across the `softness` range — the parameter is \
         changing the stroke's WIDTH, which is `thickness`'s job"
    );
}
