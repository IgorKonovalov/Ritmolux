//! **A converted preset deposits no light of its own** — the emission, and the
//! black frame it produces.
//!
//! # The defect these hold shut
//!
//! `warp_mesh`'s `deposit` defaults to `1.6`, so a bundle that binds nothing gets
//! a palette-coloured gaussian ring laid into its field every frame. MilkDrop has
//! no such figure: its light is the draw layer, which `milkconv` converts. For as
//! long as the converter's `[params]` block said the deposit stayed off without
//! emitting the key, every converted preset carried that ring — brightest along
//! the `+x` ray, where the deposit's angular term wraps, which is the hard edge
//! two corpus presets were reported for (Plan 0180 Phase 4).
//!
//! # Why the pair, and why here
//!
//! The two tests fail for different reasons and neither subsumes the other. The
//! first reads the emitted TOML, so it names the defect exactly — a missing key —
//! and stays legible when the scene's default changes. The second renders, so it
//! catches a binding that is emitted and then not honoured: a key in the wrong
//! table, a name the scene does not know, a value parsed as text. A converter
//! test that only reads its own output cannot see that.
//!
//! They live in `milkconv` rather than `core` for `conformance.rs`'s reason: the
//! subject is what the *converter emits*, and asserting it from `core` would need
//! a hand-written bundle, which pins the fixture instead of the emission.

use rlx_core::dsp::AnalysisFrame;
use rlx_core::preset::Preset;
use rlx_core::render::{CaptureImage, HeadlessOptions, RenderError, Renderer};

/// Capture size. Small: the property is "no pixel is lit", which every pixel
/// tests independently, so resolution buys nothing.
const SIZE: u32 = 64;

/// Enough frames for the deposit to have been laid down and warped repeatedly.
/// The ring appears on the **first** frame, so any count above zero would show
/// it; several are rendered so that a field accumulating over time is caught too.
const FRAMES: u32 = 24;

/// A `.milk` file whose draw layer is **entirely silenced**, and that overrides
/// nothing else.
///
/// Every producer of light the converter knows about is off: the waveform
/// (`fWaveAlpha`), both borders (`ob_a`, `ib_a`), the motion vectors
/// (`nMotionVectors*`, `mv_a`) and the custom waves and shapes (none declared, so
/// none is enabled). `fDecay` is `1.0` — a perfect integrator, which the scene
/// caps at `MAX_DECAY` — so anything deposited at all survives to the capture
/// instead of fading below the eye of the assertion.
///
/// What is left on screen is therefore the scene's own deposit and nothing else,
/// which is what makes "every pixel is black" a test of the emission rather than
/// of the draw layer.
const SILENT_PRESET: &str = "\
[preset00]
fRating=3.000
fDecay=1.000
fWaveAlpha=0.000
nMotionVectorsX=0.000
nMotionVectorsY=0.000
per_frame_1=ob_a = 0;
per_frame_2=ib_a = 0;
per_frame_3=mv_a = 0;
per_frame_4=wave_a = 0;
";

/// Convert the text and load the emitted bundle back, exactly as `shot` would.
fn converted(text: &str, name: &str) -> (String, Preset) {
    let file = milkconv::milk::parse(text).expect("the fixture parses as a .milk file");
    let converted = milkconv::convert::convert(&file, name).expect("it converts");
    let mut preset = Preset::from_toml_str(&converted.toml).unwrap_or_else(|e| {
        panic!(
            "the emitted bundle must load back: {e}\n---\n{}",
            converted.toml
        )
    });
    preset.name = name.to_string();
    (converted.toml, preset)
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

/// The brightest RGB byte anywhere in the capture, with where it sits.
///
/// Alpha is skipped: the present pass writes premultiplied colour over an opaque
/// backdrop (ADR-0026), so alpha reads `255` across a correctly black frame and
/// including it would make the statistic unable to fail.
fn brightest(image: &CaptureImage) -> (u8, u32, u32) {
    let mut best = (0u8, 0u32, 0u32);
    for y in 0..image.height {
        for x in 0..image.width {
            let i = ((y * image.width + x) * 4) as usize;
            for c in 0..3 {
                let v = image.rgba.get(i + c).copied().unwrap_or(0);
                if v > best.0 {
                    best = (v, x, y);
                }
            }
        }
    }
    best
}

/// **The emitted `[params]` block binds the deposit to zero.**
///
/// On the emitted text rather than on a frame, so the failure names the defect —
/// a key the converter does not write — instead of reporting a lit pixel and
/// leaving the cause to be found.
#[test]
fn a_converted_params_block_binds_the_deposit_to_zero() {
    let (toml, _) = converted(SILENT_PRESET, "deposit_fixture");

    let params = toml
        .split_once("\n[params]\n")
        .map(|(_, rest)| rest.split("\n[").next().unwrap_or(rest))
        .unwrap_or_else(|| panic!("the emitted bundle must carry a [params] table:\n---\n{toml}"));

    assert!(
        params.contains("deposit        = \"0.0\""),
        "the converted [params] table does not bind the deposit to zero, so the \
         `warp_mesh` scene's default of 1.6 stands and a ring of light is laid \
         into every converted preset. The table emitted was:\n{params}"
    );
}

/// **A converted preset with a silenced draw layer renders black**, which is the
/// binding above actually reaching the scene.
///
/// The arithmetic behind "black": the deposit is additive into a field the
/// present pass scales by `brightness`, and nothing else in `SILENT_PRESET`
/// writes light. So the frame is the backdrop alone, and this preset declares
/// none. A single lit byte is the ring.
#[test]
fn a_converted_preset_with_a_silent_draw_layer_deposits_nothing() {
    let Some(mut renderer) = headless() else {
        return;
    };
    let (_, preset) = converted(SILENT_PRESET, "deposit_fixture");
    renderer.set_presets(vec![preset]);

    let image = renderer
        .capture_preset("deposit_fixture", &AnalysisFrame::default(), FRAMES)
        .expect("capture the silenced preset");

    let (peak, x, y) = brightest(&image);
    println!("[deposit] brightest byte over {FRAMES} frames: {peak} at ({x}, {y})");
    assert_eq!(
        peak, 0,
        "a converted preset whose draw layer is entirely silenced still lit a \
         pixel to {peak} at ({x}, {y}) after {FRAMES} frames. The only remaining \
         light source is the scene's own deposit, so the converter's \
         `deposit = \"0.0\"` binding is not reaching it — check the table it \
         lands in and the name the scene reads"
    );
}
