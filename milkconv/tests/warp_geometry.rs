//! **The horizontal reflection seam** (Plan 0108 Phase 3, design-backlog 0107).
//!
//! # The defect this is hunting
//!
//! Plan 0100 Phase 7 judged seven converted presets beside `foo_vis_milk2` and
//! found three of them mirroring content across a horizontal line with a bright
//! ragged boundary — *Contortion*'s split sphere, *Cauldron*'s flipped top band,
//! *Cosmic Dust 2*'s full-width false horizon. The backlog entry named `s_fw`'s
//! address mode as its first suspect; that suspect is **falsified before this
//! file existed**, because `warp_mesh/shader.rs` already builds `s_fw` with
//! `AddressMode::Repeat`, and Repeat shifts a copy rather than reflecting one.
//!
//! # Why this file is here and not in `core`
//!
//! The same seam `conformance.rs` and `draw_layer.rs` sit on: the hypothesis is
//! about what a *translated HLSL shader* means, and only this crate can turn
//! HLSL into WGSL. Asserted from `core` alone the fixture would be a
//! hand-written WGSL module, which pins the module rather than the translation
//! that produced it.
//!
//! # The hypothesis, with its arithmetic
//!
//! `shader/emit.rs`'s warp epilogue builds the polar pair as
//! `_lmv_p = (uv_orig - 0.5) * vec2<f32>(2.0, -2.0) * U.aspect.zw`, so
//! `p.y = -(uv_orig.y - 0.5) * 2 * ay` — texture space is y-down and the polar
//! pair is taken y-**up**. A preset that reconstructs `uv` from `ang`, the
//! `uv = 0.5 + 0.5 * float2(cos(ang), sin(ang)) * rad` idiom most tunnel presets
//! are made of, therefore recovers
//! `uv.y = 0.5 + 0.5 * p.y / ay = 0.5 - (uv_orig.y - 0.5)` — `uv_orig.y`
//! reflected about `0.5`, a mirror about the horizontal midline with its seam on
//! the fixed line, which is the reported fingerprint including the ragged edge.
//!
//! The negation is **deliberate** and is documented at
//! [`MilkRuntime::run_vertex`](lmv_core::milk::MilkRuntime::run_vertex): the EEL
//! per-vertex program's `rad`/`ang` are taken from the +y-up clip-space position
//! while its `x`/`y` inputs are y-down, and the draw layer reads the same y-down
//! convention for a shape's own `y` (`draw::uv_to_world`). So the engine carries
//! that asymmetry consistently across all three places. **Whether MilkDrop
//! carries it too is a question about the reference, not about this repository**
//! — which is what the test below can and cannot settle, stated in its own
//! header.

use lmv_core::dsp::AnalysisFrame;
use lmv_core::preset::Preset;
use lmv_core::render::{CaptureImage, HeadlessOptions, RenderError, Renderer};

/// Capture size, and deliberately **square**: at aspect 1 the polar round trip
/// is an exact vertical mirror rather than a mirror composed with an aspect
/// scale, so the measurement below is about the flip alone.
const SIZE: u32 = 96;
/// Enough frames for the shape to be deposited and the warp to have resampled
/// it several times. The mirror, if there is one, appears on the second frame.
const FRAMES: u32 = 12;

fn fixture_text() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("milkconv has a workspace-root parent")
        .join("core/tests/fixtures/scratch-0108/ang-roundtrip.milk");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("the Phase 3 fixture must be readable at {path:?}: {e}"))
}

/// Convert the fixture and load the emitted bundle back, exactly as `shot`
/// would — `draw_layer.rs`'s route.
fn preset(text: &str, name: &str) -> Preset {
    let file = milkconv::milk::parse(text).expect("the fixture parses as a .milk file");
    let converted = milkconv::convert::convert(&file, name).expect("it converts");
    let mut preset = Preset::from_toml_str(&converted.toml).unwrap_or_else(|e| {
        panic!(
            "the emitted bundle must load back: {e}\n---\n{}",
            converted.toml
        )
    });
    preset.name = name.to_string();
    preset
}

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

/// **How far the frame is from being its own vertical mirror**: the mean
/// absolute per-channel difference between the image and itself flipped about
/// the horizontal midline, on the stored-byte 0..1 scale `frame_diff` uses.
///
/// Zero means the picture is exactly symmetric across that line. The statistic
/// is the defect's own fingerprint rather than a proxy for it — a share-of-light
/// measure was tried first and could not separate the two arms, because the
/// composite carries a soft glow that fills both halves whatever the geometry
/// does.
fn mirror_asymmetry(image: &CaptureImage) -> f32 {
    let (w, h) = (image.width.max(1), image.height.max(1));
    let mut sum = 0f64;
    let mut n = 0u64;
    let at = |x: u32, y: u32, c: u32| -> f64 {
        let i = ((y * w + x) * 4 + c) as usize;
        f64::from(image.rgba.get(i).copied().unwrap_or(0))
    };
    for y in 0..h {
        for x in 0..w {
            for c in 0..3 {
                sum += (at(x, y, c) - at(x, h - 1 - y, c)).abs();
                n += 1;
            }
        }
    }
    if n == 0 {
        0.0
    } else {
        (sum / n as f64 / 255.0) as f32
    }
}

/// **The `ang` round trip reflects content about the horizontal midline** — the
/// Phase 3 reproduction, on a committed minimal fixture.
///
/// # What this settles, and what it does not
///
/// It settles the **mechanism**: a converted warp shader that rebuilds `uv` from
/// `ang` does not get the identity out of this engine, it gets a vertical
/// mirror, and the mirror comes from the sign in `emit.rs`'s `_lmv_p` rather
/// than from a sampler address mode. That is the reported fingerprint, produced
/// on demand from eleven lines of HLSL.
///
/// It does **not** settle whether that is a defect, and **the fix the hypothesis
/// suggests is deliberately not applied here.** Three things stand in the way,
/// and the third is the one that decides it:
///
/// 1. The engine reproduces the same y-down/y-up asymmetry in all three places
///    it reads MilkDrop geometry — `run_vertex`, `draw::uv_to_world`, and this
///    epilogue — so if the reference carries the asymmetry too, this mirror is
///    faithful and design-backlog 0107's seam has some other cause.
/// 2. Plan 0100 established the per-vertex convention on purpose, and
///    `run_vertex`'s own doc states it: the reference's per-vertex space is
///    +y up while its `x`/`y` inputs are y-down.
/// 3. **The two polar pairs agree today, and in MilkDrop they are the same
///    numbers.** `emit.rs`'s `_lmv_p` is
///    `(uv_orig - 0.5) * (2, -2) * U.aspect.zw`, and with
///    `fill_uniform`'s `aspect.zw = (1, 1/aspect)` at `aspect >= 1` that is
///    `(nx, ny/aspect)` — exactly `run_vertex`'s `(px, py)`. In the reference
///    `rad`/`ang` reach a pixel shader as *interpolated per-vertex attributes*,
///    the very numbers the per-vertex program saw, so a fix that flipped the
///    fragment's sign alone would put the engine in a state the reference cannot
///    be in. Flipping both is a much larger claim than this seam — it moves
///    every converted preset's geometry — and it needs the reference on screen
///    beside `lmv.exe`, which is Plan 0108's Phase 6.
///
/// So this test pins the mechanism and holds the sign still. Making its mirror
/// go away by editing `emit.rs` would be tuning the engine to a guess.
///
/// The control is the same file with one token changed — `uv2 = uv` instead of
/// the reconstruction — so the two renders differ in the round trip and in
/// nothing else: same shape, same decay, same deposit, same frame count.
#[test]
fn the_ang_round_trip_reflects_about_the_horizontal_midline() {
    let Some(mut renderer) = headless() else {
        return;
    };
    let text = fixture_text();
    let control_text = text.replace("float2 uv2 = 0.5 + 0.5*d;", "float2 uv2 = uv;");
    assert_ne!(
        control_text, text,
        "the control substitution found nothing to replace — the fixture's \
         reconstruction line was edited without updating this test"
    );

    let round_trip = preset(&text, "ang-round-trip");
    let control = preset(&control_text, "uv-direct");
    renderer.set_presets(vec![round_trip, control]);

    let frame = AnalysisFrame::default();
    let a = renderer
        .capture_preset("ang-round-trip", &frame, FRAMES)
        .expect("capture the round trip");
    let b = renderer
        .capture_preset("uv-direct", &frame, FRAMES)
        .expect("capture the control");

    let (mirrored, direct) = (mirror_asymmetry(&a), mirror_asymmetry(&b));
    // **Measured 2026-08-17** on the development box (Windows 10, DX12 WARP),
    // 96x96 over 12 frames: round trip 0.0086, control 0.1605 — a factor of 19.
    // The vertical light profile behind those numbers, in eighths from the top:
    //   round trip  0.140 0.173 0.104 0.077 | 0.079 0.107 0.177 0.144
    //   control     0.105 0.105 0.073 0.053 | 0.104 0.144 0.225 0.190
    // The first is its own reflection to three decimals; the second is
    // bottom-weighted, which is where the fixture's one shape actually is.
    println!(
        "[warp_geometry] distance from being its own vertical mirror — \
         `uv` rebuilt from `ang`: {mirrored:.4}, sampled directly: {direct:.4}"
    );

    // Non-vacuity first: the control must be visibly one-sided, or "the round
    // trip is more symmetric than the control" compares two piles of noise.
    assert!(
        direct > 0.05,
        "the CONTROL is already close to symmetric ({direct:.4}), so it cannot \
         show a mirror by contrast. The fixture's only light source is a shape \
         at y = 0.82 — if a direct `uv` sample renders symmetrically, the shape \
         is not being drawn where the fixture says"
    );
    assert!(
        mirrored < direct * 0.25,
        "the `ang` round trip did NOT reflect: it is {mirrored:.4} from its own \
         vertical mirror against the control's {direct:.4}. That rules the \
         hypothesis out and leaves design-backlog 0107's seam unexplained, \
         alongside the already-falsified `s_fw` address mode — record it, and \
         read `emit.rs`'s `_lmv_p` against the reference's own varyings before \
         changing anything"
    );
}
