//! The backdrop paints a directional ramp (Plan 0080, ADR-0094).
//!
//! The pass used to take **one** palette sample and multiply it by a fixed
//! upward brightness tilt. It now sweeps a *segment* of the preset's gradient
//! along a ramp axis, with a brightness ramp on that same axis, an easing
//! exponent shaping both, and an angle turning it. This suite is the behavioural
//! half of that; the identity half — that every default renders byte-identically
//! to the picture before it — is the golden suite's bless-to-bless control, which
//! is a claim about *bytes* and belongs there rather than here.
//!
//! # Everything here is a differential
//!
//! No assertion recomputes the shader's arithmetic. Each capture is compared
//! against **another capture through the same pipeline**, differing in exactly
//! one input, so the vignette, the tonemap and the 8-bit display write are
//! identical on both sides and cancel. A test that re-derived the fragment stage
//! would pin the shader to itself.
//!
//! **Software adapter** (`prefer_software`), like the rest of the GPU suites
//! (ADR-0016), so this runs on CI rather than only where a GPU happens to be.

use lmv_core::dsp::AnalysisFrame;
use lmv_core::preset::Preset;
use lmv_core::render::{CaptureImage, HeadlessOptions, RenderError, Renderer};

/// Capture size for the ramp probes. Square, because the ramp's own properties
/// (continuity, direction, easing) do not vary with aspect — the one test that
/// *is* about aspect builds its own non-square renderer.
const SIZE: u32 = 64;

/// Frames per capture. The scene draws nothing, so one settled frame is the whole
/// picture; a handful keeps the clock off zero.
const FRAMES: u32 = 4;

/// `bg_bright` for the ramp probes. A dusk ground wants an order of magnitude
/// more than the shipped library's `<= 0.039` dim wash, and the properties here
/// are only observable over a lit plate.
const BRIGHT: f32 = 0.9;

/// The dusk gradient ADR-0094 was written against — near-black through deep blue
/// to hot amber. Evenly spaced, so a swept coordinate travels it at a constant
/// rate and the ramp's shape comes from the ramp rather than from the stops.
const DUSK: &str = "[palette]\nstops = [\
    { at = 0.0,  color = \"#060b24\" }, \
    { at = 0.25, color = \"#1b2a5e\" }, \
    { at = 0.5,  color = \"#c74b1d\" }, \
    { at = 0.75, color = \"#ff7a1f\" }, \
    { at = 1.0,  color = \"#ffd06e\" }]";

/// The ramp position at framebuffer row `j` of `height`, at the pixel's centre.
///
/// Derived from the documented convention rather than measured: `ndc` is
/// **clip space, Y-up** ([`gpu::FULLSCREEN_VS_NDC`]), a fullscreen fragment at
/// clip `p.y` writes framebuffer row `(1 - (p.y + 1) / 2) * H`, and the shader's
/// axis position is `0.5 + 0.5 * ndc.y`. So row 0 (the top) sits at `s ≈ 1` and
/// the bottom row at `s ≈ 0`.
fn ramp_at_row(row: u32, height: u32) -> f32 {
    1.0 - (row as f32 + 0.5) / height as f32
}

/// A backdrop-only preset: a swarm whose sprites have zero area, so no fragment
/// is ever rasterized and the frame is the backdrop alone, through the real
/// composite. `params` is appended to the `[params]` table verbatim.
fn ramp_preset(palette: &str, params: &str) -> Preset {
    let toml = format!(
        "system = \"swarm\"\nname = \"probe\"\n{palette}\n[params]\n\
         size = \"0\"\nforce = \"0\"\nspin = \"0\"\nburst = \"0\"\n\
         bg_bright = \"{BRIGHT}\"\n{params}"
    );
    Preset::from_toml_str(&toml).unwrap_or_else(|e| panic!("the ramp probe parses: {e}"))
}

/// Build a software headless renderer at `width` x `height`, or `None` (a logged
/// skip) when the runner exposes no adapter at all (ADR-0016).
fn renderer_sized(width: u32, height: u32) -> Option<Renderer> {
    match Renderer::new_headless(HeadlessOptions {
        width,
        height,
        prefer_software: true,
    }) {
        Ok(renderer) => Some(renderer),
        Err(RenderError::RequestAdapter(_)) => {
            eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
            None
        }
        Err(e) => panic!("headless renderer build failed: {e}"),
    }
}

fn renderer() -> Option<Renderer> {
    renderer_sized(SIZE, SIZE)
}

/// Capture one backdrop-only preset.
fn capture(renderer: &mut Renderer, palette: &str, params: &str) -> CaptureImage {
    renderer.set_presets(vec![ramp_preset(palette, params)]);
    renderer
        .capture_preset("probe", &AnalysisFrame::default(), FRAMES)
        .unwrap_or_else(|e| panic!("capture the ramp probe [{params}]: {e}"))
}

/// The RGB triple at `(x, y)`.
fn pixel(image: &CaptureImage, x: u32, y: u32) -> [u8; 3] {
    let index = ((y * image.width + x) * 4) as usize;
    let px = image.rgba.get(index..index + 3).unwrap_or_else(|| {
        panic!(
            "pixel ({x}, {y}) is inside a {}x{} capture",
            image.width, image.height
        )
    });
    [px[0], px[1], px[2]]
}

/// Rec. 601 luma of an 8-bit triple, as a rough scalar for "how far along a
/// brightness ramp this pixel is". Only ever compared against another value from
/// the same function, never against a computed expectation.
fn luma(px: [u8; 3]) -> f32 {
    0.299 * px[0] as f32 + 0.587 * px[1] as f32 + 0.114 * px[2] as f32
}

/// Every pixel down the frame's middle column, top row first.
fn mid_column(image: &CaptureImage) -> Vec<[u8; 3]> {
    let x = image.width / 2;
    (0..image.height).map(|y| pixel(image, x, y)).collect()
}

/// The largest per-channel difference between two triples, in 8-bit levels.
fn worst_channel(a: [u8; 3], b: [u8; 3]) -> u8 {
    (0..3).map(|c| a[c].abs_diff(b[c])).max().unwrap_or(0)
}

/// **The swept coordinate really is the palette's, at the height the ramp puts
/// it** — the positive proof that `bg_hue_span` paints a segment rather than
/// merely perturbing one sample.
///
/// The reference for row `j` is the **same shader** with `bg_hue_span = 0` and
/// `bg_hue` pinned to the coordinate the sweep reaches at that row. Everything
/// downstream of the LUT fetch — the brightness ramp, the vignette, the tonemap,
/// the 8-bit write — is identical between the two captures at that pixel, so what
/// survives the subtraction is the coordinate and nothing else.
///
/// The bound is one 8-bit level rather than zero because the two sides arrive at
/// the coordinate differently: the reference parses a decimal literal into `f32`
/// while the sweep computes `bg_hue + bg_hue_span * s` in the shader, and the two
/// can land an ulp apart on either side of a rounding boundary.
#[test]
fn a_swept_span_samples_the_palette_at_the_coordinate_its_height_implies() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    // The dusk ground: hot amber at the bottom of the frame (`s = 0` -> the
    // palette's 1.0 end) fading up to near-black (`s = 1` -> its 0.0 end).
    let swept = capture(
        &mut renderer,
        DUSK,
        "bg_hue = \"1.0\"\nbg_hue_span = \"-1.0\"\n",
    );
    let column = mid_column(&swept);
    let brightest = column.iter().copied().map(luma).fold(0.0f32, f32::max);
    assert!(
        brightest > 32.0,
        "the swept backdrop is essentially black (brightest mid-column luma \
         {brightest:.1}), so every comparison against it is vacuous"
    );

    // Rows spread across the frame, including both edges.
    for row in [0u32, 9, 21, 32, 43, 55, SIZE - 1] {
        let coord = 1.0 - ramp_at_row(row, SIZE);
        let reference = capture(
            &mut renderer,
            DUSK,
            &format!("bg_hue = \"{coord}\"\nbg_hue_span = \"0\"\n"),
        );
        let swept_px = pixel(&swept, SIZE / 2, row);
        let ref_px = pixel(&reference, SIZE / 2, row);
        let worst = worst_channel(swept_px, ref_px);
        assert!(
            worst <= 1,
            "at row {row} the sweep reaches palette coordinate {coord:.5}, so it \
             must paint what a fixed `bg_hue = {coord}` paints at that same pixel \
             — it differs by {worst} levels ({swept_px:?} vs {ref_px:?}). The \
             backdrop is sampling a segment other than [bg_hue, bg_hue + span]."
        );
    }
}

/// **The brightness ramp points wherever the preset says** — the direction the
/// engine could not express at all until the fixed tilt retired into it.
///
/// `mix(0.72, 1.0, s)` was hardcoded and welded to `+y`: brighter at the top, by
/// 28 %, always. `bg_shade = 1.0, bg_shade_end = 0.0` is the reverse and then
/// some — full brightness at the ramp's start, black at its end.
///
/// Asserted as a **pair**, because the interesting claim is the reversal rather
/// than either frame alone: the default backdrop must still be brighter at the
/// top (that is the tilt, unchanged, and a sanity check that the retirement kept
/// its constants), and the bound one must be brighter at the bottom. A flat
/// palette so the colour sweep contributes nothing, and no vignette so the only
/// thing varying down the column is the ramp.
#[test]
fn the_brightness_ramp_runs_the_way_the_preset_points_it() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    const FLAT: &str = "[palette]\nstops = [{ at = 0.0, color = \"#ffcf80\" }, \
                        { at = 1.0, color = \"#ffcf80\" }]";
    let ends = |image: &CaptureImage| {
        let column = mid_column(image);
        let top = luma(column[0]);
        let bottom = luma(column[column.len() - 1]);
        (top, bottom)
    };

    // The tilt, exactly as it always was: a touch brighter toward the top.
    let tilt = capture(&mut renderer, FLAT, "bg_hue = \"0\"\nbg_vignette = \"0\"\n");
    let (tilt_top, tilt_bottom) = ends(&tilt);
    assert!(
        tilt_top > tilt_bottom,
        "the default ramp is the retired 0.72 -> 1.0 tilt and must still run \
         upward: top luma {tilt_top:.1} against bottom {tilt_bottom:.1}"
    );

    // Reversed, and taken to black — neither reachable before this phase.
    let reversed = capture(
        &mut renderer,
        FLAT,
        "bg_hue = \"0\"\nbg_vignette = \"0\"\nbg_shade = \"1.0\"\nbg_shade_end = \"0.0\"\n",
    );
    let (rev_top, rev_bottom) = ends(&reversed);
    println!(
        "shade ramp: default (top {tilt_top:.1}, bottom {tilt_bottom:.1}), \
         reversed (top {rev_top:.1}, bottom {rev_bottom:.1})"
    );
    assert!(
        rev_bottom > rev_top + 100.0,
        "`bg_shade = 1.0, bg_shade_end = 0.0` must run the brightness *down* the \
         frame: top luma {rev_top:.1}, bottom {rev_bottom:.1}"
    );
    // **The end reaches zero, and the top row is not where zero lives.** No pixel
    // centre sits at `s = 1`: the topmost is half a row short, at
    // `s = 1 - 0.5/64`, so it carries 0.0078125 of the ramp's start — and sRGB's
    // near-black slope (12.92x) turns that 0.78 % of linear light into ~17 of
    // 255. Asserting a small number here would be asserting that fact about the
    // encode. Instead the row is pinned against a **constant** ramp held at
    // exactly the factor it should be carrying: if the two agree, the ramp
    // arrives at the value `bg_shade_end` names rather than bottoming out on a
    // floor somewhere above it.
    let inset = 0.5 / SIZE as f32;
    let floor = capture(
        &mut renderer,
        FLAT,
        &format!(
            "bg_hue = \"0\"\nbg_vignette = \"0\"\n\
             bg_shade = \"{inset}\"\nbg_shade_end = \"{inset}\"\n"
        ),
    );
    let floor_top = luma(mid_column(&floor)[0]);
    assert!(
        (rev_top - floor_top).abs() <= 2.0,
        "`bg_shade_end = 0.0` must carry the topmost row's half-row inset and \
         nothing more — {rev_top:.1} against the {floor_top:.1} a ramp pinned \
         flat at {inset} paints. The default holds {tilt_top:.1} at that pixel."
    );
    // What "full brightness at the ramp's start" means, measured rather than
    // asserted: `bg_shade = 1.0` at the ramp's start is the same brightness
    // factor the retired tilt reached at *its* `1.0` end, so the two must land on
    // the same luma — through the same flat palette, the same vignette-free
    // frame, and the same tonemap. They differ only by the half-pixel each edge
    // row sits inside the ramp (0.9922 against 0.9978), which is why the bound is
    // a few levels rather than zero. A magnitude claim here would be a claim
    // about the tonemap's shoulder instead: at this `bg_bright` the whole
    // 0.72 -> 1.0 tilt is worth only ~16 levels.
    assert!(
        (rev_bottom - tilt_top).abs() <= 3.0,
        "`bg_shade = 1.0` must reach the same full brightness the tilt's own 1.0 \
         end reached: {rev_bottom:.1} against {tilt_top:.1}"
    );
}

/// **There is no edge anywhere in the column** — the property the shipped
/// workaround (a `spectrum` slab at `scale = 0`, whose verdict was
/// "unacceptable") structurally cannot have, and the whole reason ADR-0094 was
/// written.
///
/// Stated as a ratio rather than as a level count, so it says something about the
/// *shape* of the ramp instead of about this palette's numbers: the largest
/// single row-to-row step must be a small fraction of the column's total travel.
/// A hard-edged slab is the degenerate case where one step **is** the whole
/// travel, and any fade narrow enough to read as an edge is within a small factor
/// of it. A continuous sweep across 64 rows is nowhere near.
///
/// The bound is deliberately loose: an evenly-spaced palette still crosses its
/// steepest segment faster than its flattest, and this is not a test of the
/// palette's own uniformity. Measured here at 64 rows: 132.9 luma levels of
/// travel against a worst step of 13, a ratio of 10 where 6 is asserted. The
/// step sits at the palette's `0.25` stop, where the deep blue turns to ember —
/// which is the gradient's own steepest place, not the ramp's.
#[test]
fn the_swept_ramp_is_continuous_down_the_whole_column() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let swept = capture(
        &mut renderer,
        DUSK,
        "bg_hue = \"1.0\"\nbg_hue_span = \"-1.0\"\n",
    );
    let column = mid_column(&swept);

    let lumas: Vec<f32> = column.iter().copied().map(luma).collect();
    let low = lumas.iter().copied().fold(f32::MAX, f32::min);
    let high = lumas.iter().copied().fold(f32::MIN, f32::max);
    let travel = high - low;
    assert!(
        travel > 100.0,
        "the column travels only {travel:.1} luma levels ({low:.1}..{high:.1}) — \
         too flat for a continuity claim about it to mean anything"
    );

    let (mut worst_step, mut worst_row) = (0u8, 0usize);
    for (row, pair) in column.windows(2).enumerate() {
        let step = worst_channel(pair[0], pair[1]);
        if step > worst_step {
            worst_step = step;
            worst_row = row;
        }
    }
    println!(
        "swept ramp: travel {travel:.1} luma levels, worst adjacent-row step \
         {worst_step} levels at row {worst_row}"
    );
    assert!(
        f32::from(worst_step) * 6.0 <= travel,
        "the sharpest row-to-row step down the mid-column is {worst_step} levels \
         at row {worst_row}, against a total travel of {travel:.1} — that is an \
         edge, not a ramp. The shipped `scale = 0` slab this replaces puts the \
         entire travel into one step."
    );
}
