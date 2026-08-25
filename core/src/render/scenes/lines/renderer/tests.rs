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
/// no arcs.
fn arc_capture(
    segments: &[SegmentInstance],
    arcs: &[super::ArcInstance],
    arc_capacity: usize,
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
/// this plan exists to remove, not the baseline the arc should match — the
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

    let Some(polyline) = arc_capture(&segments, &[], 0) else {
        return;
    };
    let Some(drawn) = arc_capture(&[], &[arc], 4) else {
        return;
    };

    // Non-vacuity first: two black frames agree perfectly and prove nothing.
    let lit = |px: &[f32]| px.chunks_exact(4).filter(|t| t[0] > 0.01).count();
    let (lit_polyline, lit_arc) = (lit(&polyline), lit(&drawn));
    let total = (ARC_W * ARC_H) as usize;
    let (mean, outlier) = arc_diff(&polyline, &drawn);
    eprintln!(
        "arc vs {ARC_SAMPLES}-segment polyline at {ARC_W}x{ARC_H} (aspect \
         {ARC_ASPECT:.4}): {lit_polyline} lit pixels vs {lit_arc} of {total}; \
         mean {mean:.4} (tol {ARC_MEAN_TOL}) outlier {outlier} (tol \
         {ARC_OUTLIER_TOL})"
    );
    assert!(
        lit_polyline * 200 > total && lit_arc * 200 > total,
        "one of the two captures is nearly empty ({lit_polyline} and {lit_arc} \
         of {total} pixels lit) — the comparison below would pass on black \
         against black"
    );

    assert!(
        mean <= ARC_MEAN_TOL && outlier <= ARC_OUTLIER_TOL,
        "the arc and a {ARC_SAMPLES}-segment polyline of the same circle draw \
         different pictures: mean {mean:.4} (tol {ARC_MEAN_TOL}), worst \
         single-channel byte {outlier} (tol {ARC_OUTLIER_TOL})"
    );
}

/// **The stroke profile is the segment path's**: a bright core falling
/// quadratically to zero at the stroke edge, not a flat bar and not a linear
/// ramp.
///
/// Read off the column through the circle's centre, where the arc runs
/// horizontally and its distance is measured along y — so the profile is
/// sampled in the very axis the half-width is quoted in and no aspect
/// conversion enters. That makes the expected value a **closed form** rather
/// than a shape to match: a pixel whose centre sits at NDC `y` is exactly
/// `|y - radius|` from the circle there, so the fragment must emit
/// `(1 - d / width)^2` and nothing else.
#[test]
fn the_arc_stroke_falls_off_quadratically_like_a_segment() {
    const CENTRE: [f32; 2] = [0.0, 0.0];
    const RADIUS: f32 = 0.5;
    // Fat enough that a dozen rows land across the half-width; the property
    // does not depend on the width, only the resolution of the check does.
    const WIDTH: f32 = 0.08;

    let arc = super::ArcInstance {
        centre: CENTRE,
        radius: RADIUS,
        angle_start: 0.0,
        angle_sweep: std::f32::consts::TAU,
        color: [1.0, 1.0, 1.0],
        width: WIDTH,
    };
    let Some(drawn) = arc_capture(&[], &[arc], 4) else {
        return;
    };

    // `color` and the glow multiplier are both 1 and the target was cleared to
    // black, so the red channel *is* the fragment's coverage `g`.
    let column = (ARC_W / 2) as usize;
    let at = |row: usize| drawn[(row * ARC_W as usize + column) * 4];
    // Pixel centres sit at +0.5, and row 0 is NDC y = +1.
    let ndc_y = |row: usize| 1.0 - 2.0 * (row as f32 + 0.5) / ARC_H as f32;

    let (mut worst, mut checked, mut peak) = (0.0f32, 0usize, 0.0f32);
    for row in 0..(ARC_H / 2) as usize {
        let d = (ndc_y(row) - RADIUS).abs();
        if d > WIDTH {
            continue; // outside the stroke; the arc writes nothing there
        }
        let falloff = 1.0 - d / WIDTH;
        let want = falloff * falloff;
        let got = at(row);
        worst = worst.max((got - want).abs());
        peak = peak.max(got);
        checked += 1;
    }
    eprintln!(
        "arc stroke profile at {ARC_W}x{ARC_H}: {checked} rows across the \
         stroke, peak {peak:.4}, worst |measured - (1 - d/w)^2| {worst:.4}"
    );

    assert!(
        checked >= 16 && peak > 0.8,
        "only {checked} rows fell across the stroke and the brightest reached \
         {peak:.4} — the profile is not resolved and the assertion below \
         would be vacuous"
    );
    // The composite is `Rgba16Float`, so a value near 1 is stored to about
    // 1/1024; the rest is the rasterizer agreeing with the pixel-centre
    // convention this reconstructs `d` from. It is slack, not a tolerance: the
    // property is an exact equality in real arithmetic.
    const PROFILE_SLACK: f32 = 0.01;
    assert!(
        worst <= PROFILE_SLACK,
        "the arc's across-the-stroke profile departs from `(1 - d/w)^2` by \
         {worst:.4} (slack {PROFILE_SLACK}) — a flat bar, a linear ramp or a \
         stroke of the wrong width here would all be a different look drawn \
         on the same curve"
    );

    // The other half of "like a segment": the same cut through the segment
    // path's own stroke must land on the same numbers. Without this the check
    // above pins the arc to a formula rather than to its sibling.
    let flat = [SegmentInstance {
        a: [-1.0, RADIUS],
        b: [1.0, RADIUS],
        color: [1.0, 1.0, 1.0],
        width: WIDTH,
        joined: 0,
        alpha: 1.0,
    }];
    let Some(segment) = arc_capture(&flat, &[], 0) else {
        return;
    };
    let mut worst_pair = 0.0f32;
    for row in 0..(ARC_H / 2) as usize {
        if (ndc_y(row) - RADIUS).abs() > WIDTH {
            continue;
        }
        worst_pair = worst_pair.max((at(row) - segment[(row * ARC_W as usize + column) * 4]).abs());
    }
    eprintln!("arc vs segment cross-section: worst per-row difference {worst_pair:.4}");
    assert!(
        worst_pair <= PROFILE_SLACK,
        "the arc's stroke and a straight segment's differ by {worst_pair:.4} \
         across their own cross-sections — the two primitives are not drawing \
         the same stroke"
    );
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
    let Some(drawn) = arc_capture(&[], &[arc], 0) else {
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
/// number recorded off this capture size ([ADR-0071]).
///
/// [ADR-0071]: ../../../../../docs/adrs/0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md
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

    let Some(full) = arc_capture(&[], &[arc(0.0, std::f32::consts::TAU)], 4) else {
        return;
    };
    let Some(first) = arc_capture(&[], &[arc(0.0, quarter)], 4) else {
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
