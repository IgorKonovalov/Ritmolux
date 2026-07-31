//! Pixel-level properties of the bloom stage (Plan 0045 Phase 4, [ADR-0046]):
//! the halo's **energy** follows `bloom_amount`, its **extent** follows
//! `bloom_radius`, it is **round**, and the tier's deeper pyramid reaches
//! further.
//!
//! # Why these are relative and not thresholded
//!
//! Every assertion here compares captures of the *same* fixture that differ in
//! one bound param. A halo has no natural absolute magnitude — it is the product
//! of the source's over-range energy, the pyramid's depth, the kernel, and the
//! tonemap's shoulder — so a number pinned here would be a transcription of
//! today's implementation rather than a claim about the stage. What is genuinely
//! true of *a* bloom is the monotonicity, and that is what these check.
//!
//! # `the_halo_is_round` is a regression guard, not a nicety
//!
//! The first draft of the pyramid stepped its horizontal taps in the **source's**
//! texels and its vertical taps in the target's. The horizontal pass is also the
//! downsampler, so its source is twice the target's width — which made its kernel
//! exactly half as wide and every halo 2:1 vertically elongated. It survived a
//! `cargo clippy`, the whole existing suite, and a blessed 160x100 baseline; it
//! was caught by rendering one 512x512 frame and looking at it. A square capture
//! is what makes the defect measurable, and **changing `SIZE` to a non-square
//! value silently retires this test** — at 16:9 the aspect ratio the present
//! stretch introduces would be indistinguishable from the kernel's own.
//!
//! A separate test binary rather than more arms of `composite.rs`, following that
//! file's posture and `kaleidoscope.rs`'s: these need many captures at differing
//! params and a second renderer for the tier comparison, which is the one thing
//! the file holding the blessed baselines exists to avoid. The blessed *baseline*
//! for this stage does live there, as the plan's file list asks — this is the
//! behavioural half beside it. Skips with no adapter per ADR-0016.

use lmv_core::dsp::AnalysisFrame;
use lmv_core::preset::Preset;
use lmv_core::render::{CaptureImage, HeadlessOptions, RenderError, Renderer, Tier};

/// Capture size. **Square, and load-bearing** — see the module docs. It is also
/// exactly the post grid's quantization step, so the internal grid is 256x256 and
/// the floor tier's four pyramid levels run 128, 64, 32, 16 without truncation.
const SIZE: u32 = 256;

/// Frames warmed before capture. The fixture is frozen (`spin = 0`) and binds no
/// feedback, so this only has to clear the figure's draw-in.
const FRAMES: u32 = 40;

/// The fixture, shared with `composite.rs`'s blessed baseline: a small bright core
/// on black, over range by design. Its `bloom_*` lines are stripped and rewritten
/// per capture, so every configuration below is explicit here rather than half
/// inherited.
const FIXTURE: &str = include_str!("fixtures/composite_bloom.toml");
const FIXTURE_NAME: &str = "fixture_composite_bloom";

/// Threshold, in per-channel bytes, above which a pixel counts as lit. Above the
/// software rasterizer's noise floor and well below anything the halo carries in
/// the region measured.
const LIT: u16 = 8;

/// How far out of the core's own radius the "halo" region starts, as a multiple.
/// The core is the figure the stage is blooming; measuring inside it would mix the
/// star's five-fold shape into every number here.
const CORE_MARGIN: f32 = 1.3;

/// How far the halo's horizontal and vertical spreads may differ. Sized from
/// measurement, not taste: the shipped kernel reads ~1.00 and the source-texel bug
/// this guards read ~2.0, so a bound of 1.15 sits far from both.
const ROUNDNESS_TOL: f32 = 1.15;

fn headless(tier: Tier) -> Option<Renderer> {
    match Renderer::new_headless_tiered(
        HeadlessOptions {
            width: SIZE,
            height: SIZE,
            prefer_software: true,
        },
        tier,
    ) {
        Ok(r) => Some(r),
        Err(RenderError::RequestAdapter(_)) => {
            eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
            None
        }
        Err(e) => panic!("headless renderer build failed: {e}"),
    }
}

/// The fixture with its three `bloom_*` bindings replaced by these.
///
/// Stripping and re-appending rather than editing in place: the fixture's
/// `[params]` is its last table, so appended keys land inside it, and a test that
/// spelled out only two of the three would be inheriting the third silently.
fn fixture_with(amount: f32, threshold: f32, radius: f32) -> Preset {
    let base: String = FIXTURE
        .lines()
        .filter(|line| !line.trim_start().starts_with("bloom_"))
        .collect::<Vec<_>>()
        .join("\n");
    let toml = format!(
        "{base}\nbloom_amount = \"{amount}\"\nbloom_threshold = \"{threshold}\"\n\
         bloom_radius = \"{radius}\"\n"
    );
    Preset::from_toml_str(&toml).unwrap_or_else(|e| panic!("bloom fixture parses: {e}"))
}

/// The fixed frame every capture is taken under — the same one `composite.rs`
/// blesses its baselines at.
fn fixed_frame() -> AnalysisFrame {
    AnalysisFrame {
        bass: 0.6,
        mid: 0.5,
        treb: 0.6,
        onset: 0.4,
        bar: 0.25,
        ..Default::default()
    }
}

/// Per-pixel luma in bytes, and the pixel's offset from the frame's centre in
/// pixels.
fn pixels(img: &CaptureImage) -> impl Iterator<Item = (f32, f32, f32)> + '_ {
    let (w, h) = (img.width as usize, img.height as usize);
    let (cx, cy) = ((w as f32 - 1.0) * 0.5, (h as f32 - 1.0) * 0.5);
    img.rgba
        .chunks_exact(4)
        .enumerate()
        .take(w * h)
        .map(move |(index, px)| {
            let luma = 0.299 * px[0] as f32 + 0.587 * px[1] as f32 + 0.114 * px[2] as f32;
            let (x, y) = ((index % w) as f32, (index / w) as f32);
            (luma, x - cx, y - cy)
        })
}

/// The greatest distance from the frame's centre at which any pixel is lit.
///
/// Used **only** on a capture with the stage off, to size the bare figure. It is
/// the wrong measure for a halo: a bright core's tail crosses [`LIT`] most of the
/// way to the frame's corners at any radius worth testing, so this saturates on
/// the frame rather than reporting the blur — which is exactly what it did when
/// this test first tried to assert `bloom_radius` with it (129.3 px at radius 1,
/// 129.1 px at radius 3, on a frame whose half-width is 128).
fn lit_radius(img: &CaptureImage) -> f32 {
    pixels(img)
        .filter(|&(luma, _, _)| luma >= LIT as f32)
        .map(|(_, dx, dy)| dx.hypot(dy))
        .fold(0.0f32, f32::max)
}

/// The radius, in pixels, inside which **half** the frame's light sits.
///
/// This is the halo-extent measure, and it is a median rather than a moment for
/// two reasons the first two drafts of this test found the hard way. An RMS radius
/// weights by `r²`, so it is dominated by the faintest, widest pyramid level and
/// reports a ~1 % *decrease* across a change that visibly doubles the halo; and a
/// lit-pixel maximum saturates on the frame border, which a bright core's tail
/// reaches at every radius worth testing. A median is bounded well inside the
/// frame and tracks where the bulk of the light actually is.
fn median_radius(img: &CaptureImage) -> f32 {
    let mut by_radius: Vec<(f32, f64)> = pixels(img)
        .map(|(luma, dx, dy)| (dx.hypot(dy), luma as f64))
        .collect();
    by_radius.sort_by(|a, b| a.0.total_cmp(&b.0));
    let total: f64 = by_radius.iter().map(|&(_, luma)| luma).sum();
    assert!(total > 0.0, "the frame is entirely black");
    let mut seen = 0.0f64;
    for (radius, luma) in by_radius {
        seen += luma;
        if seen * 2.0 >= total {
            return radius;
        }
    }
    0.0
}

/// Total luma outside `core` pixels of the centre — the halo's energy.
fn halo_energy(img: &CaptureImage, core: f32) -> f64 {
    pixels(img)
        .filter(|&(_, dx, dy)| dx.hypot(dy) > core)
        .map(|(luma, _, _)| luma as f64)
        .sum()
}

/// The halo's energy-weighted spread, in pixels, along each axis — measured
/// **outside** the core so the figure's own five-fold shape is not in the number.
///
/// A second moment rather than a lit-pixel extent, because a threshold reads the
/// single farthest survivor and a moment reads the whole distribution; the defect
/// this exists for is a factor on the distribution's width.
fn halo_spread(img: &CaptureImage, core: f32) -> (f32, f32) {
    let (mut sum, mut sxx, mut syy) = (0.0f64, 0.0f64, 0.0f64);
    for (luma, dx, dy) in pixels(img) {
        if dx.hypot(dy) <= core || luma < LIT as f32 {
            continue;
        }
        sum += luma as f64;
        sxx += luma as f64 * (dx as f64) * (dx as f64);
        syy += luma as f64 * (dy as f64) * (dy as f64);
    }
    assert!(sum > 0.0, "no halo outside a radius of {core:.1} px");
    ((sxx / sum).sqrt() as f32, (syy / sum).sqrt() as f32)
}

/// Capture the fixture through `renderer` at these bloom settings.
fn capture(renderer: &mut Renderer, amount: f32, threshold: f32, radius: f32) -> CaptureImage {
    renderer.set_presets(vec![fixture_with(amount, threshold, radius)]);
    renderer
        .capture_preset(FIXTURE_NAME, &fixed_frame(), FRAMES)
        .unwrap_or_else(|e| panic!("capture bloom fixture at amount {amount}: {e}"))
}

/// The stage's whole behavioural contract, on one renderer: off means off, energy
/// follows `bloom_amount`, extent follows `bloom_radius`, and the halo is round.
///
/// One test rather than four because each capture is a 40-frame render and every
/// assertion is relative to the `amount = 0` capture — splitting them would
/// re-render that reference three times and put four GPU devices in one binary.
#[test]
fn the_halo_follows_its_params_and_is_round() {
    let Some(mut renderer) = headless(Tier::Floor) else {
        return;
    };

    // The reference: the stage is inactive, so this is the bare figure.
    let off = capture(&mut renderer, 0.0, 1.0, 1.0);
    let figure = lit_radius(&off);
    assert!(
        figure > 4.0 && figure < SIZE as f32 * 0.4,
        "the fixture's figure must be a small core to bloom, not the frame: \
         radius {figure:.1} px of {SIZE}"
    );
    let core = figure * CORE_MARGIN;
    assert_eq!(
        halo_energy(&off, core),
        0.0,
        "with `bloom_amount = 0` the stage is skipped entirely, so there must be \
         no light at all outside the figure — anything here is the pyramid \
         running when it was asked not to"
    );

    // --- energy follows `bloom_amount`, at a fixed radius ---
    let mut last = 0.0f64;
    for amount in [0.5f32, 1.0, 2.0] {
        let img = capture(&mut renderer, amount, 1.0, 1.0);
        let energy = halo_energy(&img, core);
        eprintln!("amount {amount:>4} -> halo energy {energy:.0}");
        assert!(
            energy > last,
            "halo energy must grow with `bloom_amount`: {energy:.0} at {amount} \
             is not above {last:.0} at the previous step"
        );
        last = energy;
    }

    // --- extent follows `bloom_radius`, at a fixed amount ---
    let bare = median_radius(&off);
    let mut last = bare;
    for radius in [0.5f32, 1.0, 3.0] {
        let img = capture(&mut renderer, 1.0, 1.0, radius);
        let reach = median_radius(&img);
        eprintln!("radius {radius:>4} -> median radius {reach:.1} px");
        assert!(
            reach > last,
            "the halo must sit further out with `bloom_radius`: {reach:.1} px at \
             {radius} is not beyond {last:.1} px at the previous step"
        );
        last = reach;
    }
    assert!(
        last > bare * 2.0,
        "at the top of the radius range the frame's light should sit well outside \
         the figure it came from: {last:.1} px against the bare figure's \
         {bare:.1} px"
    );

    // --- and the halo is round (the source-texel bug; see the module docs) ---
    let wide = capture(&mut renderer, 1.0, 1.0, 3.0);
    let (sx, sy) = halo_spread(&wide, core);
    let ratio = (sx / sy).max(sy / sx);
    eprintln!("halo spread: x {sx:.2} px, y {sy:.2} px (ratio {ratio:.3})");
    assert!(
        ratio < ROUNDNESS_TOL,
        "the halo is {ratio:.2}:1 elongated on a square target (x {sx:.2} px, \
         y {sy:.2} px) — the separable kernel's two passes are stepping in \
         different units"
    );
}

/// **The tier reaches this stage** (Plan 0045 Phase 4's third done-when): the rich
/// tier's deeper pyramid throws light further than the floor's, and the stage
/// renders correctly at both.
///
/// The two renderers are built and used **one at a time** — the floor's is dropped
/// before the rich one exists — because two live devices in one test binary is
/// what makes the software adapter fall over on this machine.
#[test]
fn a_deeper_pyramid_throws_the_halo_further() {
    let reach = |tier: Tier| -> Option<f32> {
        let mut renderer = headless(tier)?;
        let img = capture(&mut renderer, 1.0, 1.0, 1.0);
        Some(median_radius(&img))
    };
    let Some(floor) = reach(Tier::Floor) else {
        return;
    };
    let Some(rich) = reach(Tier::Rich) else {
        return;
    };
    eprintln!("halo reach: floor {floor:.1} px, rich {rich:.1} px");
    assert!(
        floor > 0.0 && rich > 0.0,
        "the stage must render a halo at both tiers"
    );
    assert!(
        rich > floor,
        "the rich tier runs a deeper pyramid, whose coarsest levels are what \
         carry a halo outward — it must reach further than the floor's, not the \
         same distance: rich {rich:.1} px against floor {floor:.1} px"
    );
}
