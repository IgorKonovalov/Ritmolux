//! ADR-0092's three properties for the remap's response exponent, and the one
//! claim about `ink_gamma` that only a GPU can make.
//!
//! The stage's *inversion* contract lives in `core/tests/ink.rs`, on a rendered
//! scene. What is asserted here is the **key**, across a ladder of exponents:
//!
//! 1. **identity** — `g = 1.0` (the default) is the exact identity, so nothing
//!    shipped moves until a preset binds the param;
//! 2. **endpoint invariance** — the pixels at key 0 and key 1 are the same
//!    pixels at every `g`, which is the "the paper does not move" property the
//!    three ink worlds asked for;
//! 3. **monotonicity** — a mid-key pixel moves toward paper as `g` rises above
//!    1 and toward ink as it falls below.
//!
//! # Why this file injects a frame instead of rendering one
//!
//! Property 2 is a claim about a key of **exactly 1**, and no rendered scene
//! produces one: the tonemap's shoulder is bounded strictly below 1.0 by
//! construction (ADR-0046), so a capture can approach the ink pole and never
//! reach it. So the GPU test below writes a known 256-step ramp straight into
//! the remap's input — one column per 8-bit level, the two ends being exactly
//! the two poles — which is what `ink-src`'s `COPY_DST` usage is for, mirroring
//! `tonemap-src`. The ramp varies along **x** alone, so the assertion does not
//! depend on which way the sampling prelude flips y.
//!
//! Skips with no adapter per ADR-0016.

// Test asserts index, expect and panic freely; this is not the render path.
#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use super::{DEFAULT_GAMMA, Ink, MAX_GAMMA, MIN_GAMMA, applied_gamma, key};
use crate::render::capture;
use crate::render::context::{RenderContext, RenderError};

/// The exponents every property is measured across: two below the identity, the
/// identity itself, two above. Ordered, because the monotonicity assertions walk
/// them in sequence.
const LADDER: [f32; 5] = [0.25, 0.5, 1.0, 2.0, 4.0];

// -----------------------------------------------------------------------
// The key's own properties, GPU-free (the CPU mirror in `super::key`)
// -----------------------------------------------------------------------

/// **Property 1: the default is the *exact* identity** (ADR-0092).
///
/// Not "within a byte" — bit-for-bit, over the whole key range. This is what
/// makes the zero-baseline claim structural: every shipped ink preset and
/// every golden fixture leaves `ink_gamma` unbound, so they all key through
/// this branch and no capture can move by a rounding step.
#[test]
fn the_default_exponent_is_the_exact_identity() {
    assert_eq!(
        DEFAULT_GAMMA, 1.0,
        "the default exponent is what makes the stage unchanged until a preset \
         binds it"
    );
    // 1/512 steps are exact in binary, so this walks 0.0 and 1.0 themselves.
    let mut d = 0.0f32;
    while d <= 1.0 {
        assert_eq!(
            key(d, DEFAULT_GAMMA).to_bits(),
            d.to_bits(),
            "the default exponent perturbed the key at d = {d}: {} != {d}",
            key(d, DEFAULT_GAMMA)
        );
        d += 1.0 / 512.0;
    }
}

/// **Property 2: the endpoints are invariant** — `0^g = 0` and `1^g = 1` for
/// every exponent the stage can be handed, including the ones the guard has to
/// rescue. That invariance *is* the requested property: the paper never moves.
#[test]
fn the_endpoints_are_invariant_across_every_exponent() {
    let hostile = [
        MIN_GAMMA,
        MAX_GAMMA,
        0.0,
        -3.0,
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
    ];
    for g in LADDER.into_iter().chain(hostile) {
        assert_eq!(
            key(0.0, g),
            0.0,
            "the paper pole moved at g = {g}: key(0) = {}",
            key(0.0, g)
        );
        assert_eq!(
            key(1.0, g),
            1.0,
            "the ink pole moved at g = {g}: key(1) = {}",
            key(1.0, g)
        );
    }
}

/// **Property 3: a mid-key pixel travels toward paper as the exponent rises**,
/// and toward ink as it falls — monotonically, at every step of the ladder.
///
/// A smaller key means more paper in `mix(paper, ink, key)`, so "toward paper"
/// is "the key falls".
#[test]
fn a_mid_key_moves_toward_paper_as_the_exponent_rises() {
    for d in [0.05f32, 0.2, 0.5, 0.8, 0.95] {
        let mut previous = f32::INFINITY;
        for g in LADDER {
            let k = key(d, g);
            assert!(
                k < previous,
                "the key is not monotone in the exponent at d = {d}: g = {g} \
                 gives {k}, the previous step gave {previous}"
            );
            previous = k;
        }
        assert!(
            key(d, 2.0) < d && key(d, 0.5) > d,
            "at d = {d} the exponent must thin toward paper above 1 ({}) and \
             ink toward the mark below it ({})",
            key(d, 2.0),
            key(d, 0.5)
        );
    }
}

/// The guard keeps the exponent inside the range the invariance argument needs —
/// **positive and finite** — whatever a binding sweeps it through, and leaves the
/// identity untouched on the way past.
///
/// A preset binds `ink_gamma` to an expression, and an expression can reach zero
/// (where `pow(0, 0)` is undefined), go negative (where the dark end diverges) or
/// go non-finite. None of those may reach the shader.
#[test]
fn the_guard_holds_the_exponent_positive_and_finite() {
    for g in [
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
        -1.0e9,
        -0.5,
        0.0,
        1.0e9,
    ] {
        let applied = applied_gamma(g);
        assert!(
            applied.is_finite() && (MIN_GAMMA..=MAX_GAMMA).contains(&applied),
            "a bound exponent of {g} reached the shader as {applied}"
        );
    }
    assert_eq!(
        applied_gamma(DEFAULT_GAMMA).to_bits(),
        DEFAULT_GAMMA.to_bits(),
        "the guard must not perturb the identity — the shader's exact-identity \
         branch tests for 1.0 itself"
    );
}

// -----------------------------------------------------------------------
// The same three properties through the shipped WGSL (needs a GPU adapter)
// -----------------------------------------------------------------------

/// One column per 8-bit level: column `x` carries the grey byte `x`, so column 0
/// is exactly the paper endpoint and column 255 exactly the ink endpoint.
const WIDTH: u32 = 256;
/// Four rows, so the "every row agrees" cross-check has something to compare.
const HEIGHT: u32 = 4;

/// A headless context on the software adapter, or `None` (a logged skip) on a
/// runner with no GPU — macOS has no software Metal fallback (ADR-0016).
fn context() -> Option<RenderContext> {
    match RenderContext::new_headless(WIDTH, HEIGHT, true) {
        Ok(ctx) => Some(ctx),
        Err(RenderError::RequestAdapter(_)) => {
            eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
            None
        }
        Err(e) => panic!("headless context build failed: {e}"),
    }
}

/// sRGB -> linear, the transfer function the 8-bit input texture applies on read.
fn decode_srgb(x: f32) -> f32 {
    if x <= 0.040_45 {
        x / 12.92
    } else {
        ((x + 0.055) / 1.055).powf(2.4)
    }
}

/// Linear -> sRGB, the transfer function the 8-bit target applies on write.
fn encode_srgb(x: f32) -> f32 {
    if x <= 0.003_130_8 {
        12.92 * x
    } else {
        1.055 * x.powf(1.0 / 2.4) - 0.055
    }
}

/// The byte the remap must write for input level `level` at exponent `gamma`,
/// with the default poles: white paper, black ink, so `mix(paper, ink, key)`
/// reduces to `1 - key`.
fn expected_byte(level: u8, gamma: f32) -> f32 {
    let l = decode_srgb(level as f32 / 255.0);
    // The shader's Rec. 709 dot, on a grey pixel.
    let d = 0.2126 * l + 0.7152 * l + 0.0722 * l;
    encode_srgb((1.0 - key(d.clamp(0.0, 1.0), gamma)).clamp(0.0, 1.0)) * 255.0
}

/// **The shipped WGSL keys the remap the way this module documents, and the
/// three properties survive the round trip through 8-bit pixels.**
///
/// The GPU-free tests above exercise the CPU mirror; the frame path only ever
/// runs the shader, so this is the assertion that the two are one response — and
/// the only one that can put a pixel at key 1 (module docs).
#[test]
fn the_shipped_shader_keys_the_remap_the_way_this_module_documents() {
    let Some(ctx) = context() else {
        return;
    };
    assert_eq!(
        ctx.surface_format(),
        wgpu::TextureFormat::Rgba8UnormSrgb,
        "the ramp below is written in RGBA byte order; a different headless \
         format needs a different swizzle"
    );

    // The ramp: one column per level, repeated down every row.
    let mut texels = Vec::with_capacity((WIDTH * HEIGHT * 4) as usize);
    for _ in 0..HEIGHT {
        for x in 0..WIDTH {
            let level = x as u8;
            texels.extend_from_slice(&[level, level, level, u8::MAX]);
        }
    }

    // `columns[i]` is the output byte per input level at `LADDER[i]`.
    let mut columns: Vec<Vec<u8>> = Vec::new();
    for gamma in LADDER {
        let mut ink = Ink::new(&ctx.device, ctx.surface_format());
        assert!(
            ink.set_param("ink_amount", 1.0) && ink.set_param("ink_gamma", gamma),
            "both params are in the ink vocabulary"
        );
        let _ = ink.begin((WIDTH, HEIGHT));
        let texture = ink
            .src_texture()
            .expect("the remap built its input")
            .clone();
        ctx.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &texels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(WIDTH * 4),
                rows_per_image: Some(HEIGHT),
            },
            wgpu::Extent3d {
                width: WIDTH,
                height: HEIGHT,
                depth_or_array_layers: 1,
            },
        );

        let (target, view) =
            capture::create_target(&ctx.device, ctx.surface_format(), WIDTH, HEIGHT);
        let (buffer, padded_bpr) = capture::create_readback(&ctx.device, WIDTH, HEIGHT);
        let mut encoder = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("ink-gamma-ladder"),
            });
        ink.resolve(&ctx.queue, &mut encoder, &view);
        capture::record_copy(&mut encoder, &target, &buffer, padded_bpr, WIDTH, HEIGHT);
        ctx.queue.submit(std::iter::once(encoder.finish()));
        let image = capture::read_back(&ctx.device, &buffer, WIDTH, HEIGHT, padded_bpr)
            .expect("read back the remapped ramp");

        // The ramp is constant down each column, so every row must agree — a
        // cheap guard against a sampling surprise silently reading one row.
        let row = |y: u32| -> Vec<u8> {
            (0..WIDTH)
                .map(|x| image.rgba[((y * WIDTH + x) * 4) as usize])
                .collect()
        };
        let first = row(0);
        for y in 1..HEIGHT {
            assert_eq!(
                row(y),
                first,
                "the rows of a column-constant ramp disagree at g = {gamma}"
            );
        }
        columns.push(first);
    }

    // --- The shader is the documented key, at every level and every exponent ---
    for (index, gamma) in LADDER.into_iter().enumerate() {
        for level in 0..=u8::MAX {
            let expected = expected_byte(level, gamma);
            let actual = columns[index][level as usize] as f32;
            assert!(
                (expected - actual).abs() <= 1.0,
                "at g = {gamma} the shader mapped input level {level} to byte \
                 {actual}, the documented key says {expected:.1}"
            );
        }
    }

    // --- Property 2, in pixels: the two endpoint columns are the two poles, and
    // neither moves anywhere on the ladder. ---
    for (label, level, pole) in [("paper", 0u8, u8::MAX), ("ink", u8::MAX, 0)] {
        let at = |index: usize| columns[index][level as usize];
        assert_eq!(
            at(0),
            pole,
            "input level {level} did not come out as {label} ({pole}): {}",
            at(0)
        );
        for (index, gamma) in LADDER.iter().enumerate().skip(1) {
            assert_eq!(
                at(index),
                at(0),
                "the {label} pole moved between g = {} and g = {gamma}: {} -> {}",
                LADDER[0],
                at(0),
                at(index)
            );
        }
    }

    // --- Property 3, in pixels: every mid level travels toward paper as the
    // exponent rises, and the mid band's mean does so strictly. ---
    let mid = 1..u8::MAX as usize;
    for level in mid.clone() {
        for index in 1..LADDER.len() {
            assert!(
                columns[index][level] >= columns[index - 1][level],
                "input level {level} moved toward ink as the exponent rose from \
                 {} to {}: {} -> {}",
                LADDER[index - 1],
                LADDER[index],
                columns[index - 1][level],
                columns[index][level]
            );
        }
    }
    let means: Vec<f32> = columns
        .iter()
        .map(|out| {
            let sum: f32 = mid.clone().map(|level| out[level] as f32).sum();
            sum / mid.len() as f32
        })
        .collect();
    println!("mid-band mean byte across g = {LADDER:?}: {means:?}");
    for index in 1..means.len() {
        assert!(
            means[index] > means[index - 1] + 1.0,
            "the mid band did not thin toward paper between g = {} and g = {}: \
             {means:?}",
            LADDER[index - 1],
            LADDER[index]
        );
    }
}
