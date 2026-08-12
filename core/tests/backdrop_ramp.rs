//! The backdrop paints a directional ramp (Plan 0080, ADR-0094) and one curved
//! band over it (Plan 0081, ADR-0095).
//!
//! The pass used to take **one** palette sample and multiply it by a fixed
//! upward brightness tilt. It now sweeps a *segment* of the preset's gradient
//! along a ramp axis, with a brightness ramp on that same axis, an easing
//! exponent shaping both, and an angle turning it — and adds a soft gaussian
//! band over the result, on an axis of its own. This suite is the behavioural
//! half of both; the identity half — that every default renders byte-identically
//! to the picture before it — is the golden suite's bless-to-bless control, which
//! is a claim about *bytes* and belongs there rather than here.
//!
//! **One file, not two**, because the band is the same pass through the same
//! helpers: a second binary would duplicate every capture, probe and locator
//! below to test the neighbouring half of one fragment shader.
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

/// A palette with no gradient in it at all, so a probe's colour cannot vary with
/// position and the only thing moving across the frame is the property under
/// test. Used by every test whose subject is a *shape* rather than a colour.
const FLAT: &str = "[palette]\nstops = [{ at = 0.0, color = \"#ffcf80\" }, \
                    { at = 1.0, color = \"#ffcf80\" }]";

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
///
/// `bright` is written into the table here rather than left to `params`, because
/// TOML rejects a duplicate key — so a probe that wants an **unlit** ground
/// (the band's own, ADR-0095) cannot get one by appending.
fn probe_preset(palette: &str, bright: &str, params: &str) -> Preset {
    let toml = format!(
        "system = \"swarm\"\nname = \"probe\"\n{palette}\n[params]\n\
         size = \"0\"\nforce = \"0\"\nspin = \"0\"\nburst = \"0\"\n\
         bg_bright = \"{bright}\"\n{params}"
    );
    Preset::from_toml_str(&toml).unwrap_or_else(|e| panic!("the backdrop probe parses: {e}"))
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
    capture_at_bright(renderer, palette, &BRIGHT.to_string(), params)
}

/// Capture one backdrop-only preset at an explicit `bg_bright`.
fn capture_at_bright(
    renderer: &mut Renderer,
    palette: &str,
    bright: &str,
    params: &str,
) -> CaptureImage {
    renderer.set_presets(vec![probe_preset(palette, bright, params)]);
    renderer
        .capture_preset("probe", &AnalysisFrame::default(), FRAMES)
        .unwrap_or_else(|e| panic!("capture the backdrop probe [{params}]: {e}"))
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

/// Squared RGB distance between two triples.
fn distance(a: [u8; 3], b: [u8; 3]) -> i32 {
    (0..3)
        .map(|c| {
            let d = i32::from(a[c]) - i32::from(b[c]);
            d * d
        })
        .sum()
}

/// The mid-column row whose colour is closest to `target`. Used to *locate* a
/// ramp's midpoint against a pinned control rather than to compute where it
/// should be.
fn closest_row(image: &CaptureImage, target: [u8; 3]) -> u32 {
    let column = mid_column(image);
    let mut best = (0u32, i32::MAX);
    for (row, px) in column.iter().enumerate() {
        let d = distance(*px, target);
        if d < best.1 {
            best = (row as u32, d);
        }
    }
    best.0
}

/// The column within one row whose colour is closest to `target` — the same
/// locate-against-a-control idea, along the other axis.
fn closest_col(image: &CaptureImage, row: u32, target: [u8; 3]) -> u32 {
    let mut best = (0u32, i32::MAX);
    for col in 0..image.width {
        let d = distance(pixel(image, col, row), target);
        if d < best.1 {
            best = (col, d);
        }
    }
    best.0
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

/// **The exponent moves both channels' midpoints, and moves them to the same
/// height** — which is what "one ramp" means, and what a per-channel curve
/// (ADR-0094 Alternative G, rejected) would break.
///
/// Each channel is isolated and then located against a **pinned control**: the
/// colour channel sweeps the dusk palette over a flat shade ramp and is compared
/// against a frame held at the palette's midpoint coordinate; the brightness
/// channel runs `1.0 -> 0.0` over a flat palette and is compared against a frame
/// held at `0.5`. The row where each capture comes closest to its control is the
/// row where the eased position `e` crosses `0.5`. If the two channels share one
/// position, those two rows are the same row.
///
/// The row also has to be in the **right place**, or "they agree" would pass with
/// the exponent inert on both. `e = 0.5` sits at `s = 0.5^(1/g)`, and with the
/// axis running bottom-to-top over 64 rows that is row 15.0 at `g = 2.5`, row
/// 31.5 at `g = 1.0` and row 52.2 at `g = 0.4` — measured from the top. So the
/// plan's two shapes are the two ends of this: at `2.5` the midpoint is high in
/// the frame, meaning the bottom three-quarters holds near the horizon's value
/// before falling away; at `0.4` it is low, meaning the ramp drops fast and
/// leaves a long dim tail above it.
#[test]
fn the_exponent_moves_both_channels_midpoints_to_the_same_height() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    // Vignette off throughout: it is radial, so it varies down the column too and
    // would blunt every argmin below.
    const STILL: &str = "bg_vignette = \"0\"\n";

    // The two controls, each a uniform frame holding its channel's midpoint.
    let colour_mid = capture(
        &mut renderer,
        DUSK,
        &format!("{STILL}bg_hue = \"0.5\"\nbg_shade = \"1\"\nbg_shade_end = \"1\"\n"),
    );
    let bright_mid = capture(
        &mut renderer,
        FLAT,
        &format!("{STILL}bg_hue = \"0\"\nbg_shade = \"0.5\"\nbg_shade_end = \"0.5\"\n"),
    );
    let colour_target = mid_column(&colour_mid)[0];
    let bright_target = mid_column(&bright_mid)[0];

    // `e = 0.5` sits at `s = 0.5^(1/g)`; row `r` sits at `s = 1 - (r + 0.5)/H`.
    for (gamma, expected) in [(2.5f32, 15.0f32), (1.0, 31.5), (0.4, 52.2)] {
        let colour = capture(
            &mut renderer,
            DUSK,
            &format!(
                "{STILL}bg_hue = \"1.0\"\nbg_hue_span = \"-1.0\"\n\
                 bg_shade = \"1\"\nbg_shade_end = \"1\"\nbg_ramp_gamma = \"{gamma}\"\n"
            ),
        );
        let brightness = capture(
            &mut renderer,
            FLAT,
            &format!(
                "{STILL}bg_hue = \"0\"\nbg_shade = \"1.0\"\nbg_shade_end = \"0.0\"\n\
                 bg_ramp_gamma = \"{gamma}\"\n"
            ),
        );
        let colour_row = closest_row(&colour, colour_target);
        let bright_row = closest_row(&brightness, bright_target);
        println!(
            "gamma {gamma}: colour midpoint at row {colour_row}, brightness at \
             row {bright_row}, expected {expected:.1}"
        );

        assert!(
            colour_row.abs_diff(bright_row) <= 1,
            "at bg_ramp_gamma = {gamma} the colour reaches its midpoint at row \
             {colour_row} and the brightness at row {bright_row}. The exponent \
             is being applied per channel rather than to the shared position, so \
             the two halves of one ramp disagree."
        );
        for (what, row) in [("colour", colour_row), ("brightness", bright_row)] {
            let off = (row as f32 - expected).abs();
            assert!(
                off <= 2.0,
                "at bg_ramp_gamma = {gamma} the {what} midpoint should sit at row \
                 {expected:.1} (e = 0.5 at s = 0.5^(1/g)) — it is at row {row}, \
                 {off:.1} rows away. The exponent is not shaping the position."
            );
        }
    }
}

/// **An exponent driven out of range is clamped rather than allowed to render a
/// NaN frame** — the reason the guard sits on the CPU at all.
///
/// A negative exponent sends the ramp's start to infinity and `pow(0, 0)` is
/// undefined, so a binding that sweeps past either rail must land on the rail.
/// Asserted by comparison with the rail itself, so the test says nothing about
/// what the rails' numbers are beyond `background.rs` naming them.
#[test]
fn an_out_of_range_exponent_lands_on_its_rail() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let sky = |gamma: &str| {
        format!(
            "bg_vignette = \"0\"\nbg_hue = \"1.0\"\nbg_hue_span = \"-1.0\"\n\
             bg_shade = \"1.0\"\nbg_shade_end = \"0.0\"\nbg_ramp_gamma = \"{gamma}\"\n"
        )
    };
    // Plain decimals: the expression grammar has no exponent notation, so `1e6`
    // is a parse error rather than a large number.
    for (driven, rail) in [("-3", "0.05"), ("1000000", "20")] {
        let out = capture(&mut renderer, DUSK, &sky(driven));
        let at = capture(&mut renderer, DUSK, &sky(rail));
        let worst = mid_column(&out)
            .into_iter()
            .zip(mid_column(&at))
            .map(|(a, b)| worst_channel(a, b))
            .max()
            .unwrap_or(0);
        assert!(
            worst <= 1,
            "bg_ramp_gamma = {driven} must render as the {rail} rail it clamps \
             to — the mid-columns differ by {worst} levels"
        );
    }
}

/// **The ramp's angle is true in screen pixels, and the aspect it uses comes
/// from the destination surface rather than the chain's internal grid** —
/// [ADR-0037](../../docs/adrs/0037-internal-grid-is-a-resolution-not-a-shape.md)'s
/// trap, which has shipped twice and is worse than usual here.
///
/// At `bg_angle = 0` the aspect term *provably* cancels (`d = (0, 1)`, so the
/// denominator is `aspect * 0 + 1`), so no default-angle test anywhere in this
/// suite can tell a right aspect from a wrong one. This runs at **π/4 on a
/// 160x100 target**, where the two candidate sources disagree by 60 %.
///
/// # What is measured
///
/// The `s = 0.5` iso-line satisfies `A * ndc.x + ndc.y = 0`, so it crosses at
/// `ndc.x = -ndc.y / A` and its x position shifts by `2 / A` across the frame's
/// full height. Locating that crossing on the top and bottom rows — against a
/// pinned midpoint control, as everywhere else here — gives `A` back:
///
/// | aspect source | value | crossing shift, in columns of 160 |
/// |---|---|---|
/// | `surface` (correct) | 1.6 | 99 |
/// | `target.size`, chain active | 1.0 | 158 (clipped to the frame's edges) |
///
/// # Why a post stage has to be active
///
/// With an empty chain `target.size` **is** `surface`, so the wrong source
/// would measure right and the control would be theatre. `trails` forces the
/// disagreement: at a 160x100 surface the internal grid quantizes to a **square
/// 256x256** (a 256 px step over a 1920x1080 cap), which is aspect 1.0 exactly —
/// the plan's "aspect forced to 1.0" is not a hypothetical, it is what the
/// adjacent line in `composite_into` would hand over.
///
/// The stage is safe to switch on here because the backdrop is **not inside the
/// chain** (ADR-0055): it paints `destination` and the chain composites over it.
/// The scene draws nothing, so the fold is transparent and the backdrop arrives
/// whole. The first assertion checks exactly that rather than assuming it.
///
/// # The control was verified to bite
///
/// `composite_into` was temporarily re-pointed at `target.size` and this test
/// failed — on the **first** assertion, at 20 levels: switching `trails` on
/// changed the backdrop, because the aspect went from 1.6 to 1.0 underneath it.
/// That is ADR-0037's symptom stated directly ("turning a stage on changes the
/// shape of the picture"), and it is why that assertion is first. The crossing
/// assertion below catches the other flavour — an aspect wrong in *both*
/// configurations, such as a hardcoded 1.0.
#[test]
fn the_ramp_angle_takes_the_surfaces_aspect_not_the_internal_grids() {
    // 160x100 (aspect 1.6). Both numbers matter: the aspect is well away from 1,
    // and the internal grid at this size is square.
    const W: u32 = 160;
    const H: u32 = 100;
    let Some(mut renderer) = renderer_sized(W, H) else {
        return;
    };
    // π/4, written out: the expression grammar has no `pi`.
    const DIAGONAL: &str = "0.7853981634";

    let sky = |stage: &str| {
        format!(
            "bg_vignette = \"0\"\nbg_hue = \"0\"\n\
             bg_shade = \"1.0\"\nbg_shade_end = \"0.0\"\nbg_angle = \"{DIAGONAL}\"\n{stage}"
        )
    };
    let plain = capture(&mut renderer, FLAT, &sky(""));
    let staged = capture(&mut renderer, FLAT, &sky("trails = \"0.6\"\n"));

    // The chain is a passthrough over an empty scene, so switching it on must not
    // move the backdrop. Without this the comparison below could pass by the
    // stage having eaten the picture.
    let moved = (0..H)
        .map(|row| {
            (0..W)
                .map(|col| worst_channel(pixel(&plain, col, row), pixel(&staged, col, row)))
                .max()
                .unwrap_or(0)
        })
        .max()
        .unwrap_or(0);
    assert!(
        moved <= 1,
        "switching `trails` on moved the backdrop by {moved} levels. The backdrop \
         is supposed to sit underneath the chain (ADR-0055), so this control \
         cannot separate the two aspect sources until that holds."
    );

    // The midpoint control: a uniform frame at the shade ramp's half value.
    let mid = capture(
        &mut renderer,
        FLAT,
        "bg_vignette = \"0\"\nbg_hue = \"0\"\nbg_shade = \"0.5\"\nbg_shade_end = \"0.5\"\n",
    );
    let target = pixel(&mid, W / 2, H / 2);

    for (what, image) in [("no stage", &plain), ("trails active", &staged)] {
        let top = closest_col(image, 0, target);
        let bottom = closest_col(image, H - 1, target);
        let shift = bottom.abs_diff(top);
        println!("{what}: s=0.5 crossing at col {top} (top) and {bottom} (bottom), shift {shift}");
        assert!(
            shift.abs_diff(99) <= 3,
            "{what}: the s = 0.5 crossing shifts {shift} columns between the top \
             and bottom rows. The surface's aspect (1.6) puts it at 99; the \
             chain's square 256x256 internal grid would put it at ~158. The \
             backdrop is taking its aspect from a resolution instead of a shape."
        );
    }
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

// ---------------------------------------------------------------------------
// The band (Plan 0081, ADR-0095)
// ---------------------------------------------------------------------------

/// The band probes' ground: a flat mid plate with nothing else varying across
/// it, so anything that moves down the column is the band. `bg_shade` held at
/// both ends leaves the ramp out of the picture, and the vignette is off because
/// it is radial and would vary down the column too.
const PLATE: &str = "bg_vignette = \"0\"\nbg_hue = \"0\"\n\
                     bg_shade = \"0.5\"\nbg_shade_end = \"0.5\"\n";

/// The band's intensity for the probes. Roughly the ground's own brightness, so
/// the swell is unmistakable without pushing the sum anywhere near a rail.
const BAND_AMOUNT: f32 = 0.5;

/// `background.rs`'s upper half-width rail, mirrored because the constant is
/// private. What this test needs from it is only that the envelope is flat
/// there, which that constant's own doc comment derives (within 0.0025 % of 1
/// across the whole frame) — so a *lower* rail landing here would fail the
/// non-vacuity assertion rather than pass quietly.
const MAX_WIDTH: f32 = 100.0;

/// **The envelope reaches `1/e` exactly `bg_band_width` either side of the
/// centre** — which is what a gaussian half-width *means*, and the one thing an
/// author has to be able to trust when they type a number into it.
///
/// # Why this is a differential and not a ratio
///
/// The obvious test — read the band's contribution at the centre and at the
/// half-width, assert the ratio is 0.368 — would be **wrong**, and Plan 0080
/// Phase 2 already paid for the lesson: the tonemap and the sRGB encode both sit
/// between the envelope and the 8-bit write, so the linear ratio is not the
/// encoded one. A magnitude claim here would be a claim about the tonemap's
/// shoulder instead.
///
/// So the `1/e` is put into the **control** rather than into an assertion. Two
/// frames:
///
/// - the real band, at `bg_band_width = 0.15`, `bg_band_amount = A`;
/// - a **flat** one at the upper width rail, where the envelope is within
///   0.0025 % of `1` everywhere, carrying `bg_band_amount = A/e`.
///
/// At the rows where the real envelope is `1/e`, both frames add the same light
/// to the same ground and go through the same tonemap and the same encode, so
/// they must agree. Everything except the envelope cancels by construction.
///
/// # Where those rows are
///
/// At `bg_band_angle = 0` the across axis runs bottom-to-top, so row `j` sits at
/// `across = 1 - (j + 0.5)/H` — the same mapping the ramp uses. With the centre
/// at 0.5 and the half-width 0.15, the crossings are at `across = 0.65` and
/// `0.35`, which over 64 rows are rows **22** and **41** (and the centre, 0.5,
/// is row 31.5 — between two pixels, which is why the peak itself is not one of
/// the probed rows).
///
/// # The margin
///
/// Two levels, from two sources that are both bounded rather than guessed. Row
/// 22's pixel centre sits at `across = 0.6484`, not at exactly 0.65, so its real
/// envelope is 0.3756 against the control's 0.3679 — **2.1 % of the band's own
/// contribution**, which at these amplitudes is under one 8-bit level after the
/// encode. The display write also dithers (ADR-0096), and while its noise is a
/// pure function of the pixel — identical in both frames — the two sides can
/// still round to either side of one LSB.
#[test]
fn the_bands_envelope_reaches_one_over_e_at_the_half_width_its_param_names() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    const WIDTH: f32 = 0.15;
    const POS: f32 = 0.5;
    // The rows the geometry above puts the crossings on, and the row-to-position
    // mapping restated as an assertion so they cannot quietly become magic
    // numbers if `SIZE` changes.
    const CROSSINGS: [u32; 2] = [22, 41];
    for row in CROSSINGS {
        let across = ramp_at_row(row, SIZE);
        let off = (across - POS).abs();
        assert!(
            (off - WIDTH).abs() < 0.01,
            "row {row} sits at across {across:.4}, which is {off:.4} from the \
             centre — this test's control only means anything if that is the \
             half-width {WIDTH}"
        );
    }

    let band = |amount: f32, width: f32| {
        format!(
            "{PLATE}bg_band_amount = \"{amount}\"\nbg_band_angle = \"0\"\n\
             bg_band_pos = \"{POS}\"\nbg_band_width = \"{width}\"\n"
        )
    };
    let real = capture(&mut renderer, FLAT, &band(BAND_AMOUNT, WIDTH));
    // The upper rail: `env` is within 0.0025 % of 1 across the whole frame, so
    // this frame's band is a flat wash of exactly `A/e`.
    let flat = capture(
        &mut renderer,
        FLAT,
        &band(BAND_AMOUNT / std::f32::consts::E, MAX_WIDTH),
    );

    for row in CROSSINGS {
        let real_px = pixel(&real, SIZE / 2, row);
        let flat_px = pixel(&flat, SIZE / 2, row);
        let worst = worst_channel(real_px, flat_px);
        println!("1/e crossing at row {row}: band {real_px:?} against flat {flat_px:?}");
        assert!(
            worst <= 2,
            "at row {row} the envelope should be 1/e, so the band must add \
             exactly what a flat band at `bg_band_amount / e` adds — the two \
             differ by {worst} levels ({real_px:?} against {flat_px:?}). \
             `bg_band_width` is not the 1/e half-width it is documented as."
        );
    }

    // **Non-vacuity.** The two frames agree at the crossings *because* the
    // envelope is 1/e there — if it were flat, or the amount were being ignored,
    // they would agree everywhere. At the band's own centre the real frame
    // carries the full `A` against the control's `A/e`, so they must not.
    let centre = SIZE / 2;
    let apart = worst_channel(
        pixel(&real, SIZE / 2, centre),
        pixel(&flat, SIZE / 2, centre),
    );
    println!("centre row {centre}: the two frames differ by {apart} levels");
    assert!(
        apart > 10,
        "at the band's centre the real band carries its full amount and the \
         control carries 1/e of it, so the two frames must differ — they differ \
         by {apart} levels, which means the envelope is not varying at all"
    );
}

/// **`bg_band_amount > 0` is enough on its own to light the pass** — the widened
/// build condition (ADR-0095), and the near-black sky the reference photograph
/// actually is.
///
/// Below a visible `bg_bright` this pass used to skip building its gradient
/// pipeline entirely and clear the frame black. A galaxy over an unlit ground
/// would have rendered *nothing*, silently, and no test that only ever runs a
/// lit backdrop could see it: the one-line condition is the whole subject here.
///
/// The `bg_band_amount = 0` companion capture is what makes this a measurement
/// of the condition rather than of the band — same preset, same unlit ground,
/// and it must come back the plain black clear it always was.
#[test]
fn a_band_alone_lights_the_pass_over_an_unlit_ground() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let unlit = |amount: &str| {
        format!(
            "bg_vignette = \"0\"\nbg_hue = \"0\"\nbg_band_amount = \"{amount}\"\n\
             bg_band_angle = \"0\"\nbg_band_pos = \"0.5\"\nbg_band_width = \"0.15\"\n"
        )
    };
    let dark = capture_at_bright(&mut renderer, FLAT, "0", &unlit("0"));
    let banded = capture_at_bright(&mut renderer, FLAT, "0", &unlit("0.6"));

    let dark_worst = mid_column(&dark).into_iter().map(luma).fold(0.0, f32::max);
    assert!(
        dark_worst <= 1.0,
        "an unlit ground with no band is the plain black clear it always was — \
         the brightest mid-column luma is {dark_worst:.1}"
    );

    let column: Vec<f32> = mid_column(&banded).into_iter().map(luma).collect();
    let peak = column.iter().copied().fold(0.0f32, f32::max);
    assert!(
        peak > 32.0,
        "a band over an unlit ground paints nothing (brightest mid-column luma \
         {peak:.1}). The pass is still skipping its pipeline below a visible \
         `bg_bright`, so `bg_band_amount` alone cannot reach the frame."
    );

    // And it is a *band*, not a wash: at `bg_band_pos = 0.5` the peak sits at
    // the middle of the column and both ends are dark. The envelope at the frame
    // edge is `exp(-(0.5/0.15)^2)`, about 1.5e-5, so "dark" here is black.
    let peak_row = column
        .iter()
        .enumerate()
        .fold(
            (0usize, 0.0f32),
            |best, (row, &l)| {
                if l > best.1 { (row, l) } else { best }
            },
        )
        .0;
    println!(
        "unlit band: peak luma {peak:.1} at row {peak_row}, ends {:.1}/{:.1}",
        column[0],
        column[column.len() - 1]
    );
    assert!(
        peak_row.abs_diff((SIZE / 2) as usize) <= 1,
        "the band centred at `bg_band_pos = 0.5` peaks at row {peak_row} of \
         {SIZE} — it is not where its position names"
    );
    for (what, edge) in [("top", column[0]), ("bottom", column[column.len() - 1])] {
        assert!(
            edge <= 1.0,
            "the {what} of the column is {edge:.1} luma, against the band's \
             {peak:.1} peak — the envelope is not falling off, so this is a wash \
             rather than a band"
        );
    }
}

/// **At `bg_band_amount = 0` the band's other params do nothing whatsoever** —
/// byte-for-byte, which is what the shader's `select` arm buys over a multiply
/// by zero.
///
/// This is the identity claim in its strongest form. A multiply by zero would
/// also be *arithmetically* neutral, but it would put the band's expression on
/// the evaluated path, where a `pow`, an `exp` or a division has room to perturb
/// a shipped backdrop by a rounding step. The `select` makes the pre-band
/// expression the *untaken* branch, so the frames are not merely close.
#[test]
fn the_bands_geometry_is_inert_until_its_amount_is_bound() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let plain = capture(
        &mut renderer,
        DUSK,
        "bg_hue = \"1.0\"\nbg_hue_span = \"-1.0\"\n",
    );
    // Every band param bound, off its default, and the amount left at zero.
    let bound = capture(
        &mut renderer,
        DUSK,
        "bg_hue = \"1.0\"\nbg_hue_span = \"-1.0\"\nbg_band_amount = \"0\"\n\
         bg_band_angle = \"0.7\"\nbg_band_pos = \"0.2\"\nbg_band_width = \"0.05\"\n",
    );
    let worst = (0..SIZE)
        .map(|row| {
            (0..SIZE)
                .map(|col| worst_channel(pixel(&plain, col, row), pixel(&bound, col, row)))
                .max()
                .unwrap_or(0)
        })
        .max()
        .unwrap_or(0);
    assert!(
        worst == 0,
        "binding the band's geometry at `bg_band_amount = 0` moved {worst} \
         levels. The band is supposed to be an untaken `select` branch there, \
         not a term multiplied by zero."
    );
}
