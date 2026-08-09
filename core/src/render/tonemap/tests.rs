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
use crate::render::context::RenderError;
use crate::render::{HeadlessOptions, Renderer};

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
#[test]
fn the_shader_implements_the_documented_curve() {
    use crate::render::context::RenderContext;

    const SIZE: u32 = 8;
    let ctx = match RenderContext::new_headless(SIZE, SIZE, true) {
        Ok(ctx) => ctx,
        Err(RenderError::RequestAdapter(_)) => {
            eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
            return;
        }
        Err(e) => panic!("headless context build failed: {e}"),
    };

    // f32 -> IEEE-754 binary16, for the ordinary magnitudes used below.
    fn to_half(x: f32) -> u16 {
        let bits = x.to_bits();
        let sign = ((bits >> 16) & 0x8000) as u16;
        let exponent = ((bits >> 23) & 0xff) as i32 - 127 + 15;
        let mantissa = ((bits & 0x7f_ffff) >> 13) as u16;
        sign | ((exponent as u16) << 10) | mantissa
    }

    for value in [0.25f32, 0.5, 0.8, 1.0, 2.0, 4.0] {
        let mut tonemap = Tonemap::new(&ctx.device, ctx.surface_format());
        let _ = tonemap.begin((SIZE, SIZE));
        let texture = tonemap
            .src_texture()
            .expect("the tonemap built its input")
            .clone();

        // A flat grey frame at `value`, so the max channel *is* `value` and
        // the hue-preserving scale reduces to the curve itself.
        let (half, opaque) = (to_half(value), to_half(1.0));
        let mut texels = Vec::new();
        for _ in 0..(SIZE * SIZE) {
            for channel in [half, half, half, opaque] {
                texels.extend_from_slice(&channel.to_le_bytes());
            }
        }
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
                bytes_per_row: Some(SIZE * 8),
                rows_per_image: Some(SIZE),
            },
            wgpu::Extent3d {
                width: SIZE,
                height: SIZE,
                depth_or_array_layers: 1,
            },
        );

        let (target, view) = capture::create_target(&ctx.device, ctx.surface_format(), SIZE, SIZE);
        let (buffer, padded_bpr) = capture::create_readback(&ctx.device, SIZE, SIZE);
        let mut encoder = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("tonemap-curve"),
            });
        tonemap.resolve(&ctx.queue, &mut encoder, &view);
        capture::record_copy(&mut encoder, &target, &buffer, padded_bpr, SIZE, SIZE);
        ctx.queue.submit(std::iter::once(encoder.finish()));
        let image = capture::read_back(&ctx.device, &buffer, SIZE, SIZE, padded_bpr)
            .expect("read back the mapped frame");

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
// The bind-group layout enumeration (Plan 0045 Phase 4b)
// -----------------------------------------------------------------------

/// What one binding contributes to a layout's *shape*. Two layouts collide —
/// in the sense the DX12 WARP aliasing hazard cares about — when their kinds
/// match in order.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Texture,
    Sampler,
    Uniform,
    Storage,
}

/// Every spelling of an entry this repository uses, **longest first**. The
/// scan takes the longest match at each byte, so `BufferBindingType::Uniform`
/// is never read as the `BindingType::…` substring it contains.
///
/// A new spelling belongs here. Leaving it out does not weaken the guard
/// silently: the per-layout entry count below is derived independently, and a
/// marker the scan missed makes the two disagree and fails the test.
const MARKERS: &[(&str, Kind)] = &[
    ("BufferBindingType::Uniform", Kind::Uniform),
    ("BufferBindingType::Storage", Kind::Storage),
    ("BindingType::Sampler", Kind::Sampler),
    ("BindingType::Texture", Kind::Texture),
    ("lut_vertex_texture(", Kind::Texture),
    ("storage_entry(", Kind::Storage),
    ("gpu::texture(", Kind::Texture),
    ("gpu::sampler(", Kind::Sampler),
    ("gpu::uniform(", Kind::Uniform),
];

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

/// `(label, kinds)` for every `create_bind_group_layout` call in one file.
fn layouts_in(text: &str, file: &str) -> Vec<(String, Vec<Kind>)> {
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

        let mut kinds = Vec::new();
        let mut index = 0usize;
        while index < body.len() {
            let matched = MARKERS
                .iter()
                .find(|(marker, _)| body.as_bytes()[index..].starts_with(marker.as_bytes()));
            match matched {
                Some((marker, kind)) => {
                    kinds.push(*kind);
                    index += marker.len();
                }
                None => index += 1,
            }
        }
        assert_eq!(
            kinds.len(),
            entry_count(body),
            "{file}: `{label}` declares {} entries but the scan recognized {} \
             of them. Teach `MARKERS` the spelling this layout uses — an \
             unrecognized entry would make the uniqueness check below blind \
             to a real collision.",
            entry_count(body),
            kinds.len(),
        );
        found.push((label, kinds));
    }
    found
}

/// **The tonemap's bind-group layout is a shape nothing else in `core/src`
/// has** — by enumerating every layout in the crate, not by asserting it in a
/// comment (Plan 0045 Phase 4b).
///
/// The comment is exactly what went wrong. Phase 3 shipped
/// `[texture, sampler, uniform]` with a note saying no other live pipeline had
/// that shape; `attractor-decay` had had it all along, built from the same
/// three helpers. Nothing could catch that, because the claim was prose on a
/// hazard surface (ADR-0021 / Plan 0020: WARP hands a pipeline whose layout
/// matches another live one *the other pass's* resources).
///
/// Only the tonemap is asserted on. Several older layouts genuinely do
/// collide — `ink` with the fold, `trails` with the blend, four separate
/// single-uniform groups — and those pairs are load-bearing history rather
/// than this phase's business; they are printed so the picture is visible.
#[test]
fn the_tonemap_layout_is_a_shape_no_other_layout_in_core_has() {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rs_files(&src, &mut files);
    files.sort();

    let mut all: Vec<(String, Vec<Kind>)> = Vec::new();
    for file in &files {
        let text = std::fs::read_to_string(file).expect("read a core source file");
        let name = file
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("?")
            .to_string();
        all.extend(layouts_in(&text, &name));
    }

    for (label, kinds) in &all {
        eprintln!("{label:<34} {kinds:?}");
    }
    // The scan is the whole evidence, so a scan that found (nearly) nothing
    // must not read as a pass.
    assert!(
        all.len() >= 20,
        "only {} bind-group layouts found across core/src — the scan is not \
         seeing the crate",
        all.len()
    );

    let mine = all
        .iter()
        .find(|(label, _)| label == "tonemap-bind-layout")
        .expect("the tonemap's own layout is in the enumeration");
    let sharers: Vec<&str> = all
        .iter()
        .filter(|(label, kinds)| kinds == &mine.1 && label != &mine.0)
        .map(|(label, _)| label.as_str())
        .collect();
    assert!(
        sharers.is_empty(),
        "`tonemap-bind-layout` is {:?}, and so is {sharers:?}. This pass runs \
         on every frame beside whatever the preset switched on, so it is the \
         most exposed pipeline in the engine to the WARP identical-layout \
         aliasing hazard. Move it to a shape this enumeration shows is free — \
         and fix the comment in `Resources::build`, which is the thing that \
         was wrong last time.",
        mine.1
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
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rs_files(&src, &mut files);
    files.sort();

    let mut all: Vec<(String, Vec<Kind>)> = Vec::new();
    for file in &files {
        let text = std::fs::read_to_string(file).expect("read a core source file");
        let name = file
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("?")
            .to_string();
        all.extend(layouts_in(&text, &name));
    }
    assert!(all.len() >= 20, "the scan is not seeing the crate");

    for label in ["trails-present-bind-layout", "attractor-present-layout"] {
        let mine = all
            .iter()
            .find(|(found, _)| found == label)
            .unwrap_or_else(|| panic!("`{label}` is in the enumeration"));
        let sharers: Vec<&str> = all
            .iter()
            .filter(|(found, kinds)| kinds == &mine.1 && found != &mine.0)
            .map(|(found, _)| found.as_str())
            .collect();
        assert!(
            sharers.is_empty(),
            "`{label}` is {:?}, and so is {sharers:?}. This pass carries \
             `occlude` (ADR-0085), and a colliding layout is why an earlier \
             shape of it silently did nothing on WARP while working on \
             hardware. The odd-looking arrangement — a sampler before the \
             uniform in one, a sampler bound twice in the other — is what buys \
             the uniqueness this asserts; pick another free shape rather than \
             tidying it away.",
            mine.1
        );
    }
}
