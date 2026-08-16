//! The curve's three ADR-0046 properties, GPU-free, plus the two pixel-level
//! claims Plan 0045 Phase 3 owes: that the composite really carries values
//! above 1.0 into this pass, and that this pass separates them instead of
//! clipping them together.

// Test asserts index, expect and panic freely; this is not the render path.
#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use super::{KNEE, Tonemap, map};
use crate::dsp::AnalysisFrame;
use crate::preset::Preset;
use crate::render::capture;
use crate::render::context::{RenderContext, RenderError};
use crate::render::{CaptureImage, HeadlessOptions, Renderer};

/// **Near-identity below the mid-range** (ADR-0046). A frame whose values are
/// all at or below the knee comes back unchanged to well within a byte.
///
/// This is the property that rules plain Reinhard out — it maps 0.8 to 0.44,
/// which would have darkened every shipped preset — and it is what confines
/// this plan's golden re-bless to the regions that were actually clipping.
#[test]
fn the_curve_is_near_identity_below_the_mid_range() {
    // A byte is 1/255 ~ 0.0039; hold the curve an order of magnitude inside
    // that so "unchanged" is a pixel claim, not an arithmetic one.
    const TOL: f32 = 1.0e-4;
    let mut x = 0.0f32;
    while x <= KNEE {
        assert!(
            (map(x) - x).abs() < TOL,
            "f({x}) = {} drifts from identity below the knee",
            map(x)
        );
        x += 0.01;
    }
    assert!(
        (map(KNEE) - KNEE).abs() < TOL,
        "the knee itself is identity"
    );
}

/// **Monotone, and bounded below 1** (ADR-0046). A saturating ramp maps in
/// strictly increasing order — so two values never swap places — and never
/// reaches 1.0, so the 8-bit write below this pass has somewhere to put the
/// decade above 1.0 instead of flattening all of it onto one value.
///
/// **Bounded below 1 is not "never 255".** The write is sRGB-encoded and then
/// rounded to a byte, and rounding is not injective: `f(x) < 1` for every
/// finite `x`, but `f(x)` still crosses the last byte's midpoint at a linear
/// input of about **36** at [`KNEE`] `= 0.6`. A frame carrying that much light
/// presents 255 legitimately. What the curve buys is the *separation* asserted
/// below — 2.0 and 4.0 landing on different bytes, where the 8-bit chain gave
/// both the same white.
#[test]
fn a_saturating_ramp_maps_monotonically_and_never_reaches_clip() {
    let mut previous = map(0.0);
    let mut x = 0.01f32;
    while x <= 64.0 {
        let y = map(x);
        assert!(
            y > previous,
            "f is not strictly increasing at {x}: {y} <= {previous}"
        );
        assert!(y < 1.0, "f({x}) = {y} reached the clip");
        previous = y;
        x *= 1.05;
    }
    // The shoulder's whole point: an accumulation that used to clip to flat
    // white is now separable — 2.0 and 4.0 land on different bytes.
    let two = (map(2.0) * 255.0).round();
    let four = (map(4.0) * 255.0).round();
    assert!(
        four > two,
        "2.0 and 4.0 must not land on the same byte: {two} vs {four}"
    );
}

/// **Hue-preserving** (ADR-0046): the roll-off scales all three channels by
/// one factor, so the ratios between them — and therefore the hue and the
/// saturation — are exactly what came in. A per-channel curve would fail
/// this by washing the core toward white.
#[test]
fn the_roll_off_preserves_channel_ratios() {
    // A saturated over-range colour: 4.0 of red against 1.0 of green.
    let rgb = [4.0f32, 1.0, 0.25];
    let m = rgb[0];
    let scale = map(m) / m;
    let out = rgb.map(|c| c * scale);

    assert!(out.iter().all(|&c| c < 1.0), "gamut-safe: {out:?}");
    for pair in [(0usize, 1usize), (1, 2)] {
        let before = rgb[pair.0] / rgb[pair.1];
        let after = out[pair.0] / out[pair.1];
        assert!(
            (before - after).abs() < 1.0e-5,
            "channel ratio {before} became {after} — the map rotated the hue"
        );
    }
}

// -----------------------------------------------------------------------
// The pixel-level claims, on a real composite (needs a GPU adapter)
// -----------------------------------------------------------------------

/// The fixture both GPU assertions run on: a dense additive rose whose
/// strokes cross each other everywhere. Shared with
/// `core/tests/composite.rs`, which pins the same figure to a baseline — one
/// definition, two guards.
const OVERLAP_FIXTURE: &str = include_str!("../../../tests/fixtures/composite_overlap.toml");

/// Small enough to read back twice cheaply; large enough that the rose's
/// crossings cover many pixels.
const WIDTH: u32 = 160;
const HEIGHT: u32 = 100;
/// Frames warmed before the capture. The figure is static (`spin = 0`), so
/// this only has to get past the lazy resource builds.
const FRAMES: u32 = 4;

/// Rec.709 relative luminance — the ordering the "brighter than" claims are
/// made in, so a hue difference between two pixels cannot decide them.
fn luma(r: f32, g: f32, b: f32) -> f32 {
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

// -----------------------------------------------------------------------
// The injected-frame rig: a known linear frame straight into this pass
// -----------------------------------------------------------------------

/// A headless render context, or `None` (a logged skip) on a runner with no GPU
/// adapter at all — macOS has no software Metal fallback (ADR-0016).
fn context(width: u32, height: u32, software: bool) -> Option<RenderContext> {
    match RenderContext::new_headless(width, height, software) {
        Ok(ctx) => Some(ctx),
        Err(RenderError::RequestAdapter(_)) => {
            eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
            None
        }
        Err(e) => panic!("headless context build failed: {e}"),
    }
}

/// f32 -> IEEE-754 binary16, for the ordinary magnitudes used here.
fn to_half(x: f32) -> u16 {
    let bits = x.to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let exponent = ((bits >> 23) & 0xff) as i32 - 127 + 15;
    let mantissa = ((bits & 0x7f_ffff) >> 13) as u16;
    sign | ((exponent as u16) << 10) | mantissa
}

/// An opaque grey [`COMPOSITE_FORMAT`](crate::render::COMPOSITE_FORMAT) frame
/// whose value is `at(column)` — grey so the hue-preserving scale reduces to the
/// curve, and varying along **x** so the vertex prelude's Y flip is irrelevant.
fn grey_texels(width: u32, height: u32, at: impl Fn(u32) -> f32) -> Vec<u8> {
    let opaque = to_half(1.0);
    let mut texels = Vec::with_capacity((width * height * 8) as usize);
    for _ in 0..height {
        for x in 0..width {
            let half = to_half(at(x));
            for channel in [half, half, half, opaque] {
                texels.extend_from_slice(&channel.to_le_bytes());
            }
        }
    }
    texels
}

/// Write `texels` straight into a fresh tonemap's input and read back what the
/// pass writes at the surface format — the pixel-exact rig the `COPY_DST` usage
/// on `tonemap-src` exists for.
///
/// `dither` selects the shipped write or its **control arm**; both resolve
/// through the same pipeline from the same texture, so a difference between two
/// calls differing only in this flag is the dither and nothing else.
fn resolve_linear(
    ctx: &RenderContext,
    width: u32,
    height: u32,
    texels: &[u8],
    dither: bool,
) -> CaptureImage {
    let mut tonemap = Tonemap::new(&ctx.device, ctx.surface_format());
    tonemap.set_dither(dither);
    let _ = tonemap.begin((width, height));
    let texture = tonemap
        .src_texture()
        .expect("the tonemap built its input")
        .clone();
    ctx.queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        texels,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width * 8),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );

    let (target, view) = capture::create_target(&ctx.device, ctx.surface_format(), width, height);
    let (buffer, padded_bpr) = capture::create_readback(&ctx.device, width, height);
    let mut encoder = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("tonemap-injected-frame"),
        });
    tonemap.resolve(&ctx.queue, &mut encoder, &view);
    capture::record_copy(&mut encoder, &target, &buffer, padded_bpr, width, height);
    ctx.queue.submit(std::iter::once(encoder.finish()));
    capture::read_back(&ctx.device, &buffer, width, height, padded_bpr)
        .expect("read back the mapped frame")
}

/// A headless renderer on the software adapter, or `None` (a logged skip) on
/// a runner with no GPU — macOS has no software Metal fallback (ADR-0016).
fn headless() -> Option<Renderer> {
    match Renderer::new_headless(HeadlessOptions {
        width: WIDTH,
        height: HEIGHT,
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

/// **Plan 0045 Phase 3's two done-when claims, on one frame.**
///
/// 1. *The composite carries float linear values from scene to blend.* The
///    tonemap's input is read back **before** the map runs — the only place
///    an over-1.0 accumulation is observable — and the additive rose's
///    crossings are found above 1.0 there. On the pre-Plan-0045 8-bit chain
///    this readback could not exceed 1.0 by construction.
///
/// 2. *Two overlapping full-brightness strokes no longer clip to flat
///    white.* The same frame's 8-bit surface is compared at two pixels the
///    **linear** buffer identifies rather than at hard-coded coordinates: the
///    brightest (a crossing, above 1.0) and one sitting at a single stroke's
///    ~1.0. The crossing must come out strictly brighter, and below clip.
///
/// The two halves belong in one test because the second's honesty depends on
/// the first: without the linear buffer to locate them, "the overlap region"
/// and "a single stroke" would be coordinates someone guessed.
#[test]
fn stacked_light_survives_the_composite_and_separates_after_the_map() {
    let Some(mut renderer) = headless() else {
        return;
    };
    let preset = Preset::from_toml_str(OVERLAP_FIXTURE).expect("the overlap fixture parses");
    let name = preset.name.clone();
    renderer.set_presets(vec![preset]);

    let frame = AnalysisFrame {
        bass: 0.6,
        mid: 0.5,
        treb: 0.6,
        onset: 0.4,
        bar: 0.25,
        ..Default::default()
    };
    // The 8-bit surface of the *last* frame this renders — and the tonemap's
    // input still holds that same frame's linear composite afterwards.
    let displayed = renderer
        .capture_preset(&name, &frame, FRAMES)
        .expect("capture the overlap fixture");

    // --- 1. the linear composite, before the map ---
    let device = renderer.ctx.device.clone();
    let queue = renderer.ctx.queue.clone();
    let src = renderer
        .tonemap
        .src_texture()
        .expect("the tonemap built its input while capturing")
        .clone();
    let (buffer, padded_bpr) = capture::create_linear_readback(&device, WIDTH, HEIGHT);
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("tonemap-src-readback"),
    });
    capture::record_copy(&mut encoder, &src, &buffer, padded_bpr, WIDTH, HEIGHT);
    queue.submit(std::iter::once(encoder.finish()));
    let linear = capture::read_back_linear(&device, &buffer, WIDTH, HEIGHT, padded_bpr)
        .expect("read back the linear composite");

    let mut peak = (0usize, f32::MIN);
    let mut single = None;
    for (index, texel) in linear.chunks_exact(4).enumerate() {
        let l = luma(texel[0], texel[1], texel[2]);
        if l > peak.1 {
            peak = (index, l);
        }
        // A pixel a lone full-brightness stroke covers: at or just below the
        // 1.0 a single stroke reaches, and above the knee so the map has
        // actually done something to it.
        if (KNEE..=1.0).contains(&l) && single.is_none_or(|(_, best)| l > best) {
            single = Some((index, l));
        }
    }
    assert!(
        peak.1 > 1.0,
        "the composite clipped: its brightest linear luminance is {} — an \
         additive crossing must exceed 1.0 where the 8-bit chain could not",
        peak.1
    );
    let Some((single_index, single_luma)) = single else {
        panic!("no single-stroke pixel between the knee and 1.0 to compare against");
    };

    // --- 2. the same two pixels on the 8-bit surface, after the map ---
    let byte_luma = |index: usize| {
        let px = &displayed.rgba[index * 4..index * 4 + 4];
        luma(px[0] as f32, px[1] as f32, px[2] as f32)
    };
    let crossing = byte_luma(peak.0);
    let stroke = byte_luma(single_index);
    assert!(
        crossing > stroke,
        "the crossing (linear {:.3}) came out no brighter than the single \
         stroke (linear {single_luma:.3}) after the map: {crossing:.1} vs \
         {stroke:.1} — that is the flat-white clip this plan removes",
        peak.1
    );
    let clipped = displayed
        .rgba
        .chunks_exact(4)
        .flat_map(|px| px.iter().take(3))
        .filter(|&&c| c == u8::MAX)
        .count();
    assert_eq!(
        clipped, 0,
        "{clipped} channels of the overlap fixture reached 255. This is a \
         claim about **this fixture**, not about the curve: bounded below 1 \
         does not make a 255 byte unreachable (the sRGB write rounds, and a \
         linear ~36 crosses the last byte's midpoint at KNEE = 0.6). What is \
         true here is that this rose's crossings peak far below that and used \
         to clip anyway on the 8-bit chain"
    );
}

/// **The shipped shader implements the curve this module documents** — the
/// one claim the three GPU-free tests above cannot make, since they exercise
/// the CPU mirror and the frame path only ever runs the WGSL.
///
/// A known linear frame is written straight into the tonemap's input (this is
/// what its `COPY_DST` usage is for) and the mapped result compared against
/// [`map`] within a byte. Values are chosen either side of the knee, so both
/// the identity branch and the shoulder are covered.
///
/// **The control arm, with the dither off.** This is a claim about the *curve*,
/// and the shipped write adds up to one encoded level of noise on top of it
/// (ADR-0096) — which would make a one-byte bound a coin flip. The dither has
/// its own test below.
#[test]
fn the_shader_implements_the_documented_curve() {
    const SIZE: u32 = 8;
    let Some(ctx) = context(SIZE, SIZE, true) else {
        return;
    };

    for value in [0.25f32, 0.5, 0.8, 1.0, 2.0, 4.0] {
        // A flat grey frame at `value`, so the max channel *is* `value` and
        // the hue-preserving scale reduces to the curve itself.
        let texels = grey_texels(SIZE, SIZE, |_| value);
        let image = resolve_linear(&ctx, SIZE, SIZE, &texels, false);

        // The surface is sRGB, so the byte is the encoded form of what the
        // shader wrote — encode the expectation the same way rather than
        // decoding the measurement.
        let expected = encode_srgb(map(value)) * 255.0;
        let actual = image.rgba[0] as f32;
        assert!(
            (expected - actual).abs() <= 1.0,
            "the shader mapped {value} to byte {actual}, the documented curve \
             says {expected:.1}"
        );
    }
}

/// Linear -> sRGB, the transfer function the 8-bit surface applies on write.
fn encode_srgb(x: f32) -> f32 {
    if x <= 0.003_130_8 {
        12.92 * x
    } else {
        1.055 * x.powf(1.0 / 2.4) - 0.055
    }
}

// -----------------------------------------------------------------------
// The display-write dither (Plan 0082, ADR-0096)
// -----------------------------------------------------------------------

/// **The dither perturbs the encoded value by ~1 level at BOTH ends of the
/// range** — the claim ADR-0096 Alternative D gets wrong in both directions at
/// once, and the guard against someone tidying `srgb_slope` out of the shader.
///
/// # Why this is where the two disagree
///
/// The surface is `Rgba8UnormSrgb`, so the *hardware* encodes after the shader
/// and `dE/dL` runs from 12.92 near black to ~0.5 at the bright end. A constant
/// linear amplitude — the tidier-looking implementation — therefore lands ~12.9
/// encoded levels down in the dark tail, which is exactly where every plateau
/// Plan 0082 measured lives, and ~0.44 at white, which is too little to dither
/// anything. Dividing by the local slope is the whole fix, and it is invisible
/// in a bright-end-only probe.
///
/// # The bound is derived, not measured
///
/// For a ±1-LSB TPDF dither over a signal whose *encoded* fractional part is
/// uniformly distributed, the mean absolute change in the rounded byte is
/// exactly **1/3**:
///
/// ```text
/// E|Δ| = 2 ∫₀^½ [ (1-t)² + t² ] / 2  dt / ½  =  1/3
/// ```
///
/// — the two tails of the triangular CDF that push a value across the nearest
/// rounding boundary, averaged over where in its interval that value sits. So
/// each probe is a **narrow ramp** rather than a flat field: sweeping the input
/// across a dozen or so encoded levels spreads the fractional part evenly, which
/// is what makes 1/3 the prediction rather than a number that depends on where
/// one chosen level happened to land (ADR-0071).
///
/// Each arm is compared against the **undithered control resolved through the
/// same pipeline, from the same input texture**, so the curve, the encode and
/// the rounding are identical on both sides and what survives is the dither.
///
/// # The per-pixel bound is adapter-dependent, and WARP is the loose one
///
/// Measured while writing this test (Plan 0082 Phase 1, this box, DX12). On the
/// **hardware** adapter both sweeps come back with a worst per-channel move of
/// **1** and **zero** channels moving 2, at mean 0.3171 (dark) and 0.3408
/// (bright) — `round(x + n)` with `|n| <= 1` behaving exactly as the arithmetic
/// says. On **WARP** the bright sweep is identical in kind, but the dark sweep
/// reports worst 2 on 84 of 12 288 channels.
///
/// **That is not our amplitude, and it is not an unreachable code value
/// either.** An undithered control ramp rendered on WARP contains every byte
/// from 6 to 18 with no gaps, so nothing down there is unreachable. What differs
/// is the **conversion**: DX12 permits tolerance in float-to-sRGB8, and in the
/// steep dark region WARP's approximation departs from the true transfer
/// function — so a perturbation sized by the true slope lands two levels away in
/// some places and fails to move the value at all in others. Both symptoms are
/// observed and one mechanism covers them: the 2s here, and the plateau guard
/// Plan 0082 Phase 3 adds measuring 200 px -> 65 px on WARP against
/// 137 px -> 19 px on hardware when its ramp sits at its darkest.
///
/// This reaches past this test: the golden suite blesses on WARP, so Plan 0082's
/// re-bless carries the same artifact — 212 of 2 049 408 channels across the 27
/// baselines, 88 % of them below byte 20, every one skipping exactly one value.
/// **Bounded-by-one is a hardware claim**, and the bound below is read off the
/// adapter rather than assumed.
#[test]
fn the_dither_is_one_encoded_level_at_both_ends_of_the_range() {
    /// Wide enough that the sweep crosses many rounding boundaries; tall enough
    /// that each of them is sampled by many independent hashes. One triangular
    /// draw serves all three channels, so the independent sample count is
    /// `SIZE * SIZE`, not three times it.
    const SIZE: u32 = 64;
    /// The mean absolute byte change a ±1-LSB TPDF dither produces, derived
    /// above. The band is ~7 standard errors at this sample count.
    const EXPECTED: f32 = 1.0 / 3.0;
    const BAND: f32 = 0.05;

    let Some(ctx) = context(SIZE, SIZE, true) else {
        return;
    };
    // See the doc comment: on WARP a perturbation sized by the true sRGB slope
    // can cross two levels in the dark. Read back from the adapter rather than
    // inferred from the `prefer_software` request.
    let bound = if ctx.is_software() { 2 } else { 1 };

    // Two sweeps, each in **linear** light at this pass's input. The dark one
    // straddles the sRGB knee and comes out around bytes 6-18 — the band where
    // Plan 0082 measured 58-pixel plateaus. The bright one runs through the
    // tonemap's shoulder and comes out around bytes 216-241.
    for (end, lo, hi) in [("dark", 0.0020f32, 0.0060f32), ("bright", 0.70, 1.50)] {
        let sweep = |x: u32| lo + (hi - lo) * (x as f32 + 0.5) / SIZE as f32;
        let texels = grey_texels(SIZE, SIZE, sweep);
        let dithered = resolve_linear(&ctx, SIZE, SIZE, &texels, true);
        let control = resolve_linear(&ctx, SIZE, SIZE, &texels, false);

        let deltas: Vec<i32> = dithered
            .rgba
            .chunks_exact(4)
            .zip(control.rgba.chunks_exact(4))
            .flat_map(|(a, b)| (0..3).map(move |c| i32::from(a[c]) - i32::from(b[c])))
            .collect();
        let worst = deltas.iter().map(|d| d.abs()).max().unwrap_or(0);
        let mean = deltas.iter().map(|d| d.abs() as f32).sum::<f32>() / deltas.len() as f32;
        println!(
            "{end} sweep (linear {lo}..{hi}, bytes {}..{}): mean |delta| {mean:.4} \
             (expected {EXPECTED:.4}), worst {worst}, {} up / {} down",
            control.rgba[0],
            control.rgba[(SIZE - 1) as usize * 4],
            deltas.iter().filter(|d| **d > 0).count(),
            deltas.iter().filter(|d| **d < 0).count(),
        );

        assert!(
            worst <= bound,
            "the {end} sweep moved a channel by {worst} encoded levels, against \
             a bound of {bound} on this adapter. The dither is one LSB **in the \
             encoded domain**, so no pixel may move more than one byte — the \
             property that also makes Plan 0082's one-time golden re-bless \
             provably bounded. A value near 12 here is a constant linear \
             amplitude with the `srgb_slope` divide missing (ADR-0096 \
             Alternative D)."
        );
        assert!(
            (mean - EXPECTED).abs() <= BAND,
            "the {end} sweep's mean absolute byte change is {mean:.4}, against \
             the {EXPECTED:.4} a one-LSB triangular dither produces over a \
             uniformly-distributed fractional part. Well below means the \
             perturbation is too small to decorrelate the quantization error \
             at this end of the range — which is what a missing slope term \
             does at the bright end (0.44 LSB), where a dark-end probe alone \
             would see nothing wrong."
        );
        assert!(
            deltas.iter().any(|d| *d > 0) && deltas.iter().any(|d| *d < 0),
            "the {end} sweep moved every changed pixel the same way. The noise \
             is signed and zero-mean; a one-sided perturbation is a bias, not a \
             dither."
        );
    }
}

/// The widest run of one identical 8-bit value along any row of `image`, on any
/// channel — the instrument that found the banding Plan 0082 exists to fix, and
/// the thing a *band* actually is.
///
/// **Rail-pinned runs are excluded.** A genuinely flat region of a picture — a
/// black margin, a blown highlight — is not a band, and counting one would let
/// this statistic pass for the wrong reason.
fn widest_plateau(image: &CaptureImage) -> u32 {
    let mut widest = 0u32;
    for y in 0..image.height {
        for channel in 0..3usize {
            let row: Vec<u8> = (0..image.width)
                .map(|x| image.rgba[((y * image.width + x) * 4) as usize + channel])
                .collect();
            let (mut run, mut value) = (0u32, row[0]);
            // A sentinel that closes the final run without special-casing it.
            for &px in row.iter().chain(std::iter::once(&u8::MAX)) {
                if px == value {
                    run += 1;
                    continue;
                }
                if value != 0 && value != u8::MAX {
                    widest = widest.max(run);
                }
                value = px;
                run = 1;
            }
        }
    }
    widest
}

/// **The dither turns a banded dark ramp into a hairline one** — the plateau
/// measurement that opened Plan 0082, made permanent so the fix cannot be
/// silently undone (Phase 3).
///
/// # Why a synthetic ramp rather than a rendered scene
///
/// The frame is **injected** straight into this pass, so the guard does not
/// depend on any preset, palette or backdrop param surviving. A dusk-ground
/// probe would be a better picture and a worse test: it would fail for a dozen
/// reasons that have nothing to do with the display write.
///
/// # What is asserted, and why it is a ratio
///
/// A flat dark ramp — about four encoded levels spread across 512 columns, so
/// roughly **128 pixels per level** — is resolved twice through the same
/// pipeline from the same input texture, once with the dither and once without.
/// The claim is the ratio between those two arms of one run, so it says nothing
/// about this machine (ADR-0071) and both terms are the same kind of quantity, a
/// run length in pixels (ADR-0074).
///
/// The undithered arm's own plateau width is the **positive control**: the same
/// measurement, on a frame with the dither off, has to report a band well past
/// the 16-pixel width Plan 0082's survey treats as one. A banding test that
/// cannot detect banding is the failure mode here, and if the dither were
/// deleted the two arms would be identical and the ratio would be 1.
///
/// Measured when this landed: **132 px → 23 px on WARP, 133 px → 20 px on
/// hardware**. The dithered figure is not luck — a plateau survives only where
/// the encoded fractional part sits near a code value, and there the change
/// probability bottoms out at 1/4, so the longest run over 512 columns is about
/// `ln(512) / ln(4/3) ≈ 22`. That is also why the ratio *widens* as the ramp
/// flattens: the undithered arm grows with pixels-per-level while this one grows
/// only logarithmically. A flat factor is the conservative claim.
///
/// # Why bytes 28-32 and not the darkest tail
///
/// **WARP's float-to-sRGB8 conversion departs from the true transfer function in
/// the steep dark region** — measured in
/// `the_dither_is_one_encoded_level_at_both_ends_of_the_range`, whose doc comment
/// carries the evidence. Below about byte 20 a one-level perturbation there
/// frequently fails to move the value at all, and the same ramp placed at bytes
/// 8-12 reads **200 px → 65 px on WARP against 137 px → 19 px on hardware**. A
/// suite that captures on WARP would then be measuring the adapter rather than
/// the fix. Bytes 28-32 are still inside the band Plan 0082's survey found its
/// plateaus in (values 7 to 30), and there the two adapters agree to within a few
/// pixels.
#[test]
fn the_dither_dissolves_a_dark_ramps_plateaus() {
    /// Wide enough that a four-level ramp spends ~128 px on each level, which is
    /// the flat dark tail the 58-px plateaus were measured in.
    const WIDTH: u32 = 512;
    const HEIGHT: u32 = 16;
    /// The linear values that land on roughly bytes 28 and 32 through this pass
    /// — the dark tail, and far below the tonemap's knee so the curve is the
    /// identity and the ramp arrives as the ramp.
    const LO: f32 = 0.011_624;
    const HI: f32 = 0.014_459;
    /// A plateau this wide is what Plan 0082's survey counted as a band.
    const BAND_WIDTH: u32 = 16;
    /// The reduction asserted — see the doc comment on why a flat factor is the
    /// conservative form of a gap that widens with the ramp's flatness.
    const REDUCTION: u32 = 4;

    let Some(ctx) = context(WIDTH, HEIGHT, true) else {
        return;
    };
    let texels = grey_texels(WIDTH, HEIGHT, |x| {
        LO + (HI - LO) * (x as f32 + 0.5) / WIDTH as f32
    });
    let dithered = resolve_linear(&ctx, WIDTH, HEIGHT, &texels, true);
    let control = resolve_linear(&ctx, WIDTH, HEIGHT, &texels, false);

    let (banded, hairline) = (widest_plateau(&control), widest_plateau(&dithered));
    println!(
        "dark ramp over {WIDTH} px, bytes {}..{}: widest mid-range plateau \
         {banded} px undithered -> {hairline} px dithered",
        control.rgba[0],
        control.rgba[((WIDTH - 1) * 4) as usize],
    );

    assert!(
        banded >= BAND_WIDTH,
        "the undithered control's widest mid-range plateau is only {banded} px, \
         under the {BAND_WIDTH} px this survey calls a band. The ramp is no \
         longer flat enough to band at all, so the comparison below would pass \
         with the dither doing nothing — re-flatten it (fewer encoded levels \
         across more columns) rather than lowering this."
    );
    assert!(
        hairline > 0 && hairline * REDUCTION <= banded,
        "the dither took the widest mid-range plateau from {banded} px to \
         {hairline} px, short of the {REDUCTION}x this asserts. The display \
         write has stopped decorrelating the quantization error — check that \
         `dither_offset` still divides by `srgb_slope` (ADR-0096 Alternative D \
         leaves the bright end looking fine and re-bands exactly this tail) and \
         that the rail fade has not swallowed the whole amplitude."
    );
}

// -----------------------------------------------------------------------
// The bind-group layout enumeration (Plan 0045 Phase 4b; generalized into the
// ADR-0058 collision property by Plan 0053 Phase 2)
// -----------------------------------------------------------------------

/// What one binding contributes to a layout's *shape*.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Texture,
    Sampler,
    Uniform,
    Storage,
}

/// Where a marker's **visibility** is written, relative to the marker itself.
///
/// Visibility is part of the shape (see [`Layout`]), so the scan has to recover
/// it, and the spellings this repository uses do not put it in one place.
#[derive(Clone, Copy)]
enum Vis {
    /// A helper whose visibility is a constant in its own body, out of reach of
    /// the descriptor the scan is reading. Asserted against the helpers by
    /// [`the_scan_reads_the_visibility_the_helpers_actually_set`].
    Fixed(&'static str),
    /// A full `BindGroupLayoutEntry` literal: `visibility:` is a field, and it
    /// comes **before** the `ty:` field the marker matches.
    Preceding,
    /// `gpu::uniform(binding, visibility)`: the second argument, so it comes
    /// **after** the marker.
    Following,
}

/// Every spelling of an entry this repository uses, **longest first**. The
/// scan takes the longest match at each byte, so `BufferBindingType::Uniform`
/// is never read as the `BindingType::…` substring it contains.
///
/// A new spelling belongs here. Leaving it out does not weaken the guard
/// silently: the per-layout entry count below is derived independently, and a
/// marker the scan missed makes the two disagree and fails the test.
const MARKERS: &[(&str, Kind, Vis)] = &[
    ("BufferBindingType::Uniform", Kind::Uniform, Vis::Preceding),
    ("BufferBindingType::Storage", Kind::Storage, Vis::Preceding),
    ("BindingType::Sampler", Kind::Sampler, Vis::Preceding),
    ("BindingType::Texture", Kind::Texture, Vis::Preceding),
    ("lut_vertex_texture(", Kind::Texture, Vis::Fixed("VERTEX")),
    ("storage_entry(", Kind::Storage, Vis::Fixed("COMPUTE")),
    ("gpu::texture(", Kind::Texture, Vis::Fixed("FRAGMENT")),
    ("gpu::sampler(", Kind::Sampler, Vis::Fixed("FRAGMENT")),
    ("gpu::uniform(", Kind::Uniform, Vis::Following),
    // warp_mesh/shader.rs's local helper for the two 3D noise volumes (Plan
    // 0100 Phase 6). The dimension is not part of the shape — like a size's
    // *value*, recording it could only split pairs, never join them.
    ("texture_3d(", Kind::Texture, Vis::Fixed("FRAGMENT")),
];

/// One bind-group layout as the scan sees it.
///
/// # The shape carries visibility, and that is a Plan 0053 decision
///
/// Plan 0045 Phase 4b's shape was the ordered list of binding *kinds* alone.
/// That is coarser than what the hazard keys on, and the codebase already
/// carries the measurement that proves it: `emitter-bind-layout` was first
/// written byte-identical to `swarm-bind-layout`, WARP handed the swarm the
/// emitter's uniform, and **distinguishing the descriptor by a wider visibility
/// mask and an explicit `min_binding_size` restored it**
/// (`scenes/emitter.rs`, ADR-0058). Under a kinds-only shape those two still
/// read as colliding — so the assertion below would have demanded an allowlist
/// entry for a pair that was *deliberately separated*, recording a fix as a
/// tolerated collision.
///
/// **`min_binding_size` is in the shape too, and that was measured rather than
/// assumed** (Plan 0053 Phase 3). Phase 2 left it out because the emitter's fix
/// moved the mask and the size together, so which one did the work was not
/// established. Phase 3 established it, twice and independently: adding an
/// explicit size — and *nothing else* — to `background-bind-layout` and to
/// `blend-bind-layout` moved WARP onto the hardware adapter's numbers in three
/// configurations that were rendering the wrong picture. Both sites carry the
/// before/after tables.
///
/// So an explicit size is a real separation, and this shape says so: the two
/// fixes are load-bearing, and dropping either back to a bare `gpu::uniform`
/// re-collides its pair and fails the assertion below.
struct Layout {
    /// The file the layout is built in — for the failure messages, not the key.
    file: String,
    /// The descriptor's `label`, or a synthetic one for a computed label.
    label: String,
    /// `(what it binds, which stages see it, whether it declares a minimum
    /// size)`, in binding order.
    shape: Vec<Binding>,
}

/// One entry's contribution to a [`Layout::shape`]: kind, visibility, and
/// **whether** it declares a `min_binding_size` — deliberately not *which*.
///
/// Recording the value would split two layouts whose sizes are spelled
/// differently but equal, and a false split is the one direction of error this
/// scan must not make: it would silently stop reporting a pair that does
/// collide. "Declares one" over-reports instead, which is the safe way to be
/// wrong.
type Binding = (Kind, String, bool);

/// `[Uniform:FRAGMENT+size, Texture:FRAGMENT]` — the form the messages print.
fn shape_str(shape: &[Binding]) -> String {
    let parts: Vec<String> = shape
        .iter()
        .map(|(kind, vis, sized)| format!("{kind:?}:{vis}{}", if *sized { "+size" } else { "" }))
        .collect();
    format!("[{}]", parts.join(", "))
}

/// Whether the entry a marker at `at` opens declares a `min_binding_size`.
///
/// Only a **buffer** entry has the field at all, and only the full-literal
/// spelling can set it — every helper in `MARKERS` passes `None`. So this looks
/// for the first `min_binding_size:` after the marker, which is inside that
/// entry's own `BindingType::Buffer { … }` block.
fn declares_min_size(body: &str, at: usize, kind: Kind, rule: Vis) -> bool {
    const FIELD: &str = "min_binding_size:";
    if !matches!(kind, Kind::Uniform | Kind::Storage) || !matches!(rule, Vis::Preceding) {
        return false;
    }
    match body[at..].find(FIELD) {
        Some(hit) => !body[at + hit + FIELD.len()..]
            .trim_start()
            .starts_with("None"),
        None => false,
    }
}

/// The `ShaderStages::` identifier at or after `from`, and where it starts.
fn stages_at(body: &str, from: usize) -> Option<(usize, String)> {
    const TAG: &str = "ShaderStages::";
    let hit = body[from..].find(TAG)? + from;
    let rest = &body[hit + TAG.len()..];
    let end = rest
        .find(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .unwrap_or(rest.len());
    Some((hit, rest[..end].to_string()))
}

/// The visibility a marker at `at` declares, by its [`Vis`] rule.
fn visibility(body: &str, at: usize, rule: Vis, label: &str) -> String {
    match rule {
        Vis::Fixed(vis) => vis.to_string(),
        Vis::Following => {
            stages_at(body, at)
                .unwrap_or_else(|| {
                    panic!("`{label}`: no ShaderStages after the entry at byte {at}")
                })
                .1
        }
        Vis::Preceding => {
            let mut best = None;
            let mut cursor = 0usize;
            while let Some((hit, vis)) = stages_at(body, cursor) {
                if hit >= at {
                    break;
                }
                best = Some(vis);
                cursor = hit + 1;
            }
            best.unwrap_or_else(|| {
                panic!("`{label}`: no ShaderStages before the entry at byte {at}")
            })
        }
    }
}

fn rs_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("read a core/src directory") {
        let path = entry.expect("a directory entry").path();
        if path.is_dir() {
            rs_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// The text from `text[0]` up to the delimiter that closes an already-open
/// `open`, ignoring every other character.
fn balanced(text: &str, open: u8, close: u8) -> &str {
    let mut depth = 1i32;
    for (index, byte) in text.bytes().enumerate() {
        if byte == open {
            depth += 1;
        } else if byte == close {
            depth -= 1;
            if depth == 0 {
                return &text[..index];
            }
        }
    }
    panic!("a bind-group-layout descriptor never closes");
}

/// How many entries a slice body holds, counted from its **top-level commas**
/// — independent of [`MARKERS`], which is what makes the two a cross-check.
fn entry_count(body: &str) -> usize {
    let (mut depth, mut count, mut filled) = (0i32, 0usize, false);
    for byte in body.bytes() {
        match byte {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            b',' if depth == 0 => {
                if filled {
                    count += 1;
                }
                filled = false;
                continue;
            }
            _ => {}
        }
        if !byte.is_ascii_whitespace() {
            filled = true;
        }
    }
    if filled {
        count += 1;
    }
    count
}

/// Every `create_bind_group_layout` call in one file, as a [`Layout`].
fn layouts_in(text: &str, file: &str) -> Vec<Layout> {
    // Split so the constant does not match **itself** — this scan reads its
    // own file, and an anchor spelled whole here would open a "descriptor"
    // that runs to the end of the module.
    const CALL: &str = concat!(
        "create_bind_group_layout",
        "(&wgpu::BindGroupLayoutDescriptor {"
    );
    const ENTRIES: &str = "entries: &[";
    const LABELLED: &str = "label: Some(\"";

    let mut found = Vec::new();
    let mut cursor = 0usize;
    while let Some(hit) = text[cursor..].find(CALL) {
        cursor += hit + CALL.len();
        // Bound everything to this descriptor's own braces, so a call with a
        // computed label cannot borrow the next call's literal one.
        let desc = balanced(&text[cursor..], b'{', b'}');
        let label = match desc.find(LABELLED) {
            Some(at) => {
                let from = at + LABELLED.len();
                let end = desc[from..].find('"').expect("the label string closes");
                desc[from..from + end].to_string()
            }
            // `lines/renderer.rs` formats its label per scene.
            None => format!("{file} (computed label)"),
        };
        let entries_at = desc.find(ENTRIES).expect("a layout declares entries") + ENTRIES.len();
        let body = balanced(&desc[entries_at..], b'[', b']');

        // A combined mask would read as its first flag alone and split a pair
        // that does collide — the one direction of error this scan must not
        // make silently.
        assert!(
            !body.contains('|'),
            "{file}: `{label}` combines flags with `|`. The visibility scan \
             takes the identifier after `ShaderStages::` and would read \
             `VERTEX | FRAGMENT` as `VERTEX`, which is a DIFFERENT shape from \
             `VERTEX_FRAGMENT` and would hide a collision. Teach the scan the \
             combined spelling, or spell it `VERTEX_FRAGMENT`."
        );

        let mut shape = Vec::new();
        let mut index = 0usize;
        while index < body.len() {
            let matched = MARKERS
                .iter()
                .find(|(marker, ..)| body.as_bytes()[index..].starts_with(marker.as_bytes()));
            match matched {
                Some((marker, kind, rule)) => {
                    shape.push((
                        *kind,
                        visibility(body, index, *rule, &label),
                        declares_min_size(body, index, *kind, *rule),
                    ));
                    index += marker.len();
                }
                None => index += 1,
            }
        }
        assert_eq!(
            shape.len(),
            entry_count(body),
            "{file}: `{label}` declares {} entries but the scan recognized {} \
             of them. Teach `MARKERS` the spelling this layout uses — an \
             unrecognized entry would make the uniqueness check below blind \
             to a real collision.",
            entry_count(body),
            shape.len(),
        );
        found.push(Layout {
            file: file.to_string(),
            label,
            shape,
        });
    }
    found
}

/// Every bind-group layout in `core/src`, sorted by file for a stable printout.
fn all_layouts() -> Vec<Layout> {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rs_files(&src, &mut files);
    files.sort();

    let mut all = Vec::new();
    for file in &files {
        let text = std::fs::read_to_string(file).expect("read a core source file");
        let name = file
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("?")
            .to_string();
        all.extend(layouts_in(&text, &name));
    }
    all
}

/// How many layouts `core/src` held when Plan 0053 Phase 2 landed.
///
/// A **floor**, not an equality: adding a pass is ordinary and the collision
/// property below is what guards that. What this catches is the opposite — a
/// refactor that moves a layout into a spelling the scan cannot see, which
/// would shrink the enumeration and make every assertion here quietly weaker on
/// a shorter list. Lower it only when a pass was genuinely removed, and say so.
const LAYOUTS_AT_PLAN_0053: usize = 25;

/// The scan sees the whole crate, and says how much of it it saw.
fn assert_scan_is_whole(all: &[Layout]) {
    assert!(
        all.len() >= LAYOUTS_AT_PLAN_0053,
        "the scan found {} bind-group layouts across core/src, against the \
         {LAYOUTS_AT_PLAN_0053} present when this floor was set. A layout the \
         scan cannot see is one the collision property below cannot check.",
        all.len()
    );
}

/// **The tonemap's bind-group layout is a shape nothing else in `core/src`
/// has** — by enumerating every layout in the crate, not by asserting it in a
/// comment (Plan 0045 Phase 4b).
///
/// The comment is exactly what went wrong. Phase 3 shipped
/// `[texture, sampler, uniform]` with a note saying no other live pipeline had
/// that shape; `attractor-decay` had had it all along, built from the same
/// three helpers. Nothing could catch that, because the claim was prose on a
/// hazard surface (ADR-0058: WARP hands a pipeline whose layout matches another
/// live one *the other pass's* resources).
///
/// It is kept beside the general property below rather than folded into it
/// (ADR-0058: "the tonemap's existing single-layout assertion is subsumed
/// rather than replaced"). The general one would pass if someone allowlisted
/// this pair; this one says why that would be the wrong entry to write.
#[test]
fn the_tonemap_layout_is_a_shape_no_other_layout_in_core_has() {
    let all = all_layouts();
    assert_scan_is_whole(&all);

    let mine = all
        .iter()
        .find(|layout| layout.label == "tonemap-bind-layout")
        .expect("the tonemap's own layout is in the enumeration");
    let sharers: Vec<&str> = all
        .iter()
        .filter(|layout| layout.shape == mine.shape && layout.label != mine.label)
        .map(|layout| layout.label.as_str())
        .collect();
    assert!(
        sharers.is_empty(),
        "`tonemap-bind-layout` is {}, and so is {sharers:?}. This pass runs \
         on every frame beside whatever the preset switched on, so it is the \
         most exposed pipeline in the engine to the WARP identical-layout \
         aliasing hazard. Move it to a shape this enumeration shows is free — \
         and fix the comment in `Resources::build`, which is the thing that \
         was wrong last time.",
        shape_str(&mine.shape)
    );
}

/// **The two present layouts `occlude` widened are shapes nothing else has**
/// (Plan 0071 Phase 1, ADR-0085) — the second and third entries in this
/// enumeration that are asserted on rather than printed.
///
/// They are asserted because this hazard was not hypothetical here: it was
/// *measured on this change*. `occlude` needed a uniform in the trails present
/// and the attractor present, neither of which had one. The first attempt put it
/// in a second bind group holding the uniform alone — `[uniform]`, which is
/// `background-bind-layout`'s shape, and the backdrop pass is live in every
/// frame. On the DX12 WARP software adapter the trails present then read the
/// *backdrop's* buffer: `occlude` moved 0 of 196 608 channels there while moving
/// 3 307 of them on the hardware adapter, and every capture test in the suite
/// went green over it. That is the whole failure mode — silent, adapter-specific,
/// and invisible to a tolerance.
///
/// Unlike the tonemap above, these two are also asserted **against each other**:
/// both are present passes, and a swarm-over-trails preset and an attractor
/// preset can be live in the same session.
#[test]
fn the_two_present_layouts_added_for_occlude_are_shapes_nothing_else_has() {
    let all = all_layouts();
    assert_scan_is_whole(&all);

    for label in ["trails-present-bind-layout", "attractor-present-layout"] {
        let mine = all
            .iter()
            .find(|layout| layout.label == label)
            .unwrap_or_else(|| panic!("`{label}` is in the enumeration"));
        let sharers: Vec<&str> = all
            .iter()
            .filter(|layout| layout.shape == mine.shape && layout.label != mine.label)
            .map(|layout| layout.label.as_str())
            .collect();
        assert!(
            sharers.is_empty(),
            "`{label}` is {}, and so is {sharers:?}. This pass carries \
             `occlude` (ADR-0085), and a colliding layout is why an earlier \
             shape of it silently did nothing on WARP while working on \
             hardware. The odd-looking arrangement — a sampler before the \
             uniform in one, a sampler bound twice in the other — is what buys \
             the uniqueness this asserts; pick another free shape rather than \
             tidying it away.",
            shape_str(&mine.shape)
        );
    }
}

// -----------------------------------------------------------------------
// The collision property (Plan 0053 Phase 2, ADR-0058)
// -----------------------------------------------------------------------

/// One accepted layout collision, with the evidence that it does not alias.
///
/// **An entry is not a suppression.** ADR-0058's rule is that an entry with no
/// recorded measurement is not an entry: [`evidence`](Self::evidence) carries
/// the same configuration rendered on the hardware adapter and on WARP, and the
/// date it was taken. Where separating a pair is cheap, separating it is
/// preferred — a layout that cannot collide needs no evidence and no
/// maintenance.
struct AllowedCollision {
    /// A layout label, as the enumeration prints it. Order does not matter.
    a: &'static str,
    b: &'static str,
    /// One line: why separation is the worse cure for this pair.
    why: &'static str,
    /// The measurement, and when it was taken. Printed on every run so the
    /// debt is visible while it lasts, and re-printed in any failure message.
    evidence: &'static str,
}

/// The measurement rig every entry below was taken on, stated once.
///
/// Windows 11, DX12, `Renderer::new_headless` at 160x100 over 40 frames, the
/// same `AnalysisFrame` `golden.rs` uses. "hardware" is
/// `prefer_software: false` and "WARP" is `prefer_software: true`, with
/// `adapter_is_software()` read back on both so neither side is a hope. Each
/// number is the mean 8-bit channel value over the whole frame.
///
/// **Every entry carries a control**, because two adapters disagreeing is not
/// by itself aliasing. The control is the same scene with the colliding
/// pipeline absent: there the two adapters agree to **0.02 of one 8-bit level**,
/// which is the noise floor every "agrees" below is measured against.
const RIG: &str = "2026-08-09, DX12 hardware vs WARP, 160x100/40 frames";

/// The pairs of same-shaped layouts this crate accepts (ADR-0058).
///
/// Nothing may be added here without a measurement. The list is checked in
/// three directions: a colliding pair with no entry fails; an entry naming a
/// pair that no longer collides fails, so separating a pair later cannot leave
/// a stale allowance behind that would silently cover a *different* collision
/// if the shapes ever met again; and an entry whose `evidence` does not record
/// a comparison fails, because an entry with no measurement is not an entry.
const ALLOWED: &[AllowedCollision] = &[
    // --- `[Uniform:FRAGMENT]`: the fullscreen scenes' single uniforms ---
    //
    // `background-bind-layout` was a member of this group and is NOT any more:
    // Phase 3 measured that collision rendering the wrong picture on WARP, and
    // `background.rs` now declares an explicit `min_binding_size`. See its
    // comment for the before/after. What is left is the two scenes, plus the
    // test-only disc.
    AllowedCollision {
        a: "fragment-field-uniform-layout",
        b: "rd-init-layout",
        why: "both are the minimal fullscreen shape and ADR-0058 Alt A rejects \
              padding it; co-live only across a preset dissolve, never within \
              one preset",
        evidence: "AGREES. Measured mid-dissolve, fragment field -> \
                   reaction-diffusion, both over a lit backdrop: hardware \
                   116.370 169.731 151.559, WARP 116.341 169.712 151.535, \
                   identical lit-pixel counts. Inside the 0.02-level noise \
                   floor the no-collision controls set.",
    },
    // --- `[Uniform:FRAGMENT]`, the test-only stand-in ---
    // `disc-bind-layout` models a scene inside `post/tests.rs`. Separating it
    // would mean padding a test shader with a binding it does not use, which is
    // Alternative A again with the distortion moved into the instrument.
    AllowedCollision {
        a: "disc-bind-layout",
        b: "fragment-field-uniform-layout",
        why: "the disc is a test-only stand-in scene, and no device ever holds \
              both: `post/tests.rs` builds the disc, a `Background` and the \
              post stages and no scene, while only a `Renderer` builds a \
              scene and no `Renderer` builds the disc",
        evidence: "AGREES, and the pair is not constructible — this is the one \
                   entry whose evidence is an argument plus a proxy rather than \
                   the two layouts rendered together, because there is no \
                   configuration that puts them together. Proxy: `post/tests.rs` \
                   run on both adapters reports identical numbers for every \
                   statistic the disc and the backdrop feed — disc x/y 1.0000 \
                   and 1.0000, backdrop mean 0.42401, occlude 0.83148 / 0.85120 \
                   empty-chain and 0.42279 / 0.42892 chain-active, all equal to \
                   five decimals on hardware and WARP.",
    },
    AllowedCollision {
        a: "disc-bind-layout",
        b: "rd-init-layout",
        why: "the disc is a test-only stand-in scene (see above)",
        evidence: "AGREES, and not constructible — same argument and the same \
                   proxy as the entry above.",
    },
    // --- `[Texture, Texture, Sampler]`: the two palette-LUT groups ---
    AllowedCollision {
        a: "background-lut-layout",
        b: "fragment-field-lut-layout",
        why: "both sample an A/B gradient pair through one shared sampler, so \
              the shape is what the palette system is, not a choice either made",
        evidence: "AGREES, and this pair is the reason the entry is worth \
                   having rather than obvious: it was live and colliding \
                   THROUGHOUT the fragment-field mis-render Phase 3 found, and \
                   fixing the uniform group alone made the frame correct — so \
                   this pair was measurably not the one aliasing. After the \
                   fix, fragment field over a lit backdrop: hardware 131.010 \
                   170.559 141.381, WARP 130.989 170.538 141.359. Supersedes \
                   the 2026-08-08 Plan 0072 measurement in `background.rs`'s \
                   `Background` docs, which reached the same verdict from the \
                   other side.",
    },
];

/// **No two bind-group layouts that could be live in one frame share a shape,
/// unless [`ALLOWED`] carries the measurement that says they do not alias**
/// (Plan 0053 Phase 2, ADR-0058).
///
/// This is the general form of the two assertions above. Plan 0045 Phase 4b
/// enumerated the crate's layouts and asserted on the tonemap alone, **printing
/// three collision groups it asserted nothing about** under a docstring calling
/// them "older and deliberate" — a claim with nothing behind it, which is the
/// exact failure mode that enumeration existed to retire.
///
/// # "Can be live in one frame" is approximated, and the approximation is coarse
///
/// Whether two layouts co-exist depends on which stages a preset composes, which
/// this test cannot know, so it treats **any** two layouts in `core/src` as
/// potentially co-live. That over-reports, and ADR-0058 accepts the cost. Two
/// things worth knowing before dismissing a flagged pair as unreachable:
///
/// - **A preset transition puts two scenes live at once.** `blend-bind-layout`
///   exists to cross-fade one preset into another, so "one scene per preset"
///   does not make two scene layouts mutually exclusive.
/// - **A layout built inside a unit test is live in that test's own process.**
///   `disc-bind-layout` is built in `post/tests.rs`, and a test that blesses a
///   mis-render is precisely what this hazard costs.
///
/// If this list grows much past the twelve layouts it covers today, the
/// approximation is wrong and the *shape* of this assertion should be revisited
/// rather than the list extended (Plan 0053's own risk note).
#[test]
fn no_two_layouts_share_a_shape_without_recorded_evidence() {
    let all = all_layouts();
    for layout in &all {
        eprintln!(
            "{:<34} {:<22} {}",
            layout.label,
            layout.file,
            shape_str(&layout.shape)
        );
    }
    eprintln!("{} bind-group layouts walked across core/src", all.len());
    assert_scan_is_whole(&all);

    let allowed = |x: &str, y: &str| {
        ALLOWED
            .iter()
            .find(|entry| (entry.a == x && entry.b == y) || (entry.a == y && entry.b == x))
    };

    let mut unrecorded = Vec::new();
    let mut covered = Vec::new();
    for (i, one) in all.iter().enumerate() {
        for other in all.iter().skip(i + 1) {
            if one.shape != other.shape {
                continue;
            }
            match allowed(&one.label, &other.label) {
                Some(entry) => {
                    covered.push((entry.a, entry.b));
                    eprintln!(
                        "  allowed  {} + {}  {}\n           why: {}\n           evidence: {}",
                        one.label,
                        other.label,
                        shape_str(&one.shape),
                        entry.why,
                        entry.evidence
                    );
                }
                None => unrecorded.push(format!(
                    "{} ({}) + {} ({}) both {}",
                    one.label,
                    one.file,
                    other.label,
                    other.file,
                    shape_str(&one.shape)
                )),
            }
        }
    }

    assert!(
        unrecorded.is_empty(),
        "{} layout pair(s) share a shape with no ADR-0058 allowlist entry:\n  \
         {}\n\nOn the DX12 WARP software adapter a pipeline whose bind-group \
         layout matches another live one is handed the OTHER pass's resources, \
         and the whole golden suite captures on WARP — so a mis-render there is \
         not caught, it is blessed. Either separate the pair (preferred where \
         cheap: a layout that cannot collide needs no evidence) or add an \
         `ALLOWED` entry carrying a hardware-vs-WARP comparison of the same \
         configuration. An entry with no recorded measurement is not an entry.",
        unrecorded.len(),
        unrecorded.join("\n  ")
    );

    // The other direction: an entry that no longer describes a real collision.
    let stale: Vec<String> = ALLOWED
        .iter()
        .filter(|entry| !covered.contains(&(entry.a, entry.b)))
        .map(|entry| format!("{} + {}", entry.a, entry.b))
        .collect();
    assert!(
        stale.is_empty(),
        "{} allowlist entr(y/ies) name pairs that no longer share a shape: {}. \
         Delete them. A stale allowance is a standing permission for a \
         collision nobody has measured, waiting for the two shapes to meet \
         again.",
        stale.len(),
        stale.join(", ")
    );

    // ADR-0058's rule, enforced rather than printed (Plan 0053 Phase 4). Phase 2
    // seeded this list with `EVIDENCE: none yet` so the build stayed green while
    // Phase 3 measured; Phase 3 measured, so the debt is now a failure. **An
    // entry with no recorded measurement is not an entry** — adding a pair here
    // to turn a red build green, without rendering the configuration on both
    // adapters, is the suppression this whole mechanism exists to refuse.
    let owed: Vec<String> = ALLOWED
        .iter()
        .filter(|entry| !entry.evidence.starts_with("AGREES"))
        .map(|entry| format!("{} + {}", entry.a, entry.b))
        .collect();
    assert!(
        owed.is_empty(),
        "{} of {} allowlist entries carry no measurement: {}. Render the \
         pair's configuration on the hardware adapter and on WARP, compare, \
         and record the numbers in `evidence` — `RIG` describes the rig and \
         every existing entry shows the form. A pair that does NOT agree is a \
         defect: fix it by separation rather than by writing an entry that \
         records the mis-render. An explicit `min_binding_size` was sufficient \
         twice — see `background.rs` and `transition.rs`.",
        owed.len(),
        ALLOWED.len(),
        owed.join(", ")
    );
    eprintln!(
        "{} allowlist entries, all carrying a measurement ({RIG})",
        ALLOWED.len()
    );
}

/// **The bloom stage's four layouts are four distinct shapes** (ADR-0058's
/// closing note, Plan 0053 Phase 2).
///
/// `bloom.rs`'s module docs carried the same prose uniqueness claim the tonemap
/// comment did, in a table, for four layouts — and the tonemap's version of that
/// claim was false while it sat there reading as reassurance. This converts it.
/// The general property above would catch two bloom layouts colliding with each
/// other only in the sense that it catches *any* collision; this one says whose
/// invariant it is and where the table to fix lives.
///
/// It lives here rather than in `bloom/tests.rs` because the crate-wide scan
/// does, and duplicating that machinery to move one assertion closer to its
/// subject would be the more expensive mistake.
#[test]
fn the_bloom_layouts_are_four_shapes_nothing_else_shares() {
    const BLOOM: [&str; 4] = [
        "bloom-bright-layout",
        "bloom-blur-layout",
        "bloom-up-layout",
        "bloom-mix-layout",
    ];
    let all = all_layouts();
    assert_scan_is_whole(&all);

    for label in BLOOM {
        let mine = all
            .iter()
            .find(|layout| layout.label == label)
            .unwrap_or_else(|| panic!("`{label}` is in the enumeration"));
        let sharers: Vec<&str> = all
            .iter()
            .filter(|layout| layout.shape == mine.shape && layout.label != mine.label)
            .map(|layout| layout.label.as_str())
            .collect();
        assert!(
            sharers.is_empty(),
            "`{label}` is {}, and so is {sharers:?}. The bright / blur / up / \
             mix layouts look arbitrary because they are — the requirement is \
             only that they be distinct, and the natural orderings were all \
             taken. Pick another free shape, and fix the table in `bloom.rs`'s \
             module docs, which is the thing that would otherwise still claim \
             this.",
            shape_str(&mine.shape)
        );
    }
}

/// The visibilities [`MARKERS`] hard-codes are the ones the helpers set.
///
/// [`Vis::Fixed`] exists because three helpers put their `visibility` in their
/// own body, out of reach of the descriptor the scan reads — so the scan states
/// it instead. A restated constant is a claim, and this is the kind of claim
/// this whole section exists to stop trusting: if `gpu::texture` ever became
/// `VERTEX_FRAGMENT`, every texture in the enumeration would carry the wrong
/// visibility and pairs would split or merge on it silently.
#[test]
fn the_scan_reads_the_visibility_the_helpers_actually_set() {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rs_files(&src, &mut files);
    let mut crate_text = String::new();
    for file in &files {
        crate_text.push_str(&std::fs::read_to_string(file).expect("read a core source file"));
    }

    // `(the helper's definition, the visibility MARKERS says it sets)`. Each
    // definition is matched to the first `ShaderStages::` inside it.
    for (definition, expected) in [
        ("pub(crate) fn texture(", "FRAGMENT"),
        ("pub(crate) fn sampler(", "FRAGMENT"),
        ("pub(super) fn storage_entry(", "COMPUTE"),
        ("let lut_vertex_texture = |binding: u32|", "VERTEX"),
    ] {
        let at = crate_text
            .find(definition)
            .unwrap_or_else(|| panic!("`{definition}` is still in core/src"));
        let (_, found) = stages_at(&crate_text, at)
            .unwrap_or_else(|| panic!("`{definition}` sets a visibility"));
        assert_eq!(
            found, expected,
            "`{definition}` now sets ShaderStages::{found}, but MARKERS says \
             its entries are {expected}-visible. Update the Vis::Fixed value — \
             until then every layout using it carries the wrong visibility, and \
             collision pairs split or merge on a stale constant."
        );
    }
}
