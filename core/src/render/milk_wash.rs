//! **The wash bisect** — Plan 0111 Phase 2.
//!
//! One statistic, measured at every seam of the chain, for a washed conversion
//! and a clean control. The question is not "how bright is it" but **where along
//! the chain does the washed/control ratio depart from what it was at the
//! field**, because that names the stage that does it.
//!
//! # The statistic
//!
//! [`FieldTrace::edge`](super::scenes::warp_mesh)'s quantity: the mean over the
//! outermost ring of texels. That is the **background**, where a centred figure
//! puts nothing. One statistic at every seam is the whole design — the numbers
//! are then comparable across stages, and each comparison is linear light against
//! linear light (ADR-0074's same-kind requirement).
//!
//! **It reads the background only for a figure that does not fill the frame.**
//! *Fog Tunnel* qualifies **in the reference**, which draws a skeleton of
//! discrete concentric rings whose gaps *are* the background — but our conversion
//! draws the solid tube that is the defect, so on this render the outer ring may
//! be sampling the figure. That softens the absolute numbers and not the trend:
//! the trend is one statistic at every seam of one run, so whatever the ring
//! samples, it samples the same thing at each seam. Pointed at a preset that
//! fills the frame in *both* renderers this measures the figure and says nothing
//! about a wash.
//!
//! # The seams, and why there are three rather than five
//!
//! There are five in the general case: the field (A), the present pass
//! (B), the backdrop + `layer_blend` composite (C), bloom (D) and the
//! tonemap (E). **For these two subjects C and D do not exist**, and
//! that is a measurement rather than an assumption:
//!
//! - every post stage reports [`active`](super::post::PostStage::active) only
//!   when its own param exceeds zero, and neither converted fixture binds
//!   `bloom`, `trails` or any kaleidoscope param — their whole `[params]` table
//!   is `brightness`, `warp_scale`, `warp_speed`;
//! - with no stage active `PostChain::begin` returns the caller's `destination`
//!   itself, so the scene draws **directly** into the texture the tonemap reads;
//! - `bg_bright` defaults to `0.0` and neither fixture binds it, so the backdrop
//!   pre-pass contributes nothing.
//!
//! So B, C and D are one texture, and the chain this probe bisects is `field ->
//! present pass -> tonemap -> display`. Two stages. The probe reports the
//! collapsed seam once, named for what it is, rather than printing one read
//! three times as though it were three.
//!
//! # What a departure means
//!
//! Seam E is display-referred and every other seam is linear, so a seam-to-seam
//! comparison of *levels* would be meaningless. The comparison is between the two
//! **subjects** at one seam — same units on both sides — and then between those
//! dimensionless ratios across seams, which is what [`SeamTrace`] carries.

use super::{HeadlessOptions, RenderError, Renderer, capture, scene_for};
use crate::dsp::AnalysisFrame;
use crate::preset::Preset;

/// *Geiss - Fog Tunnel* — still washed at Plan 0109's Phase 5 gate, and the one
/// whose defect is legible: the reference draws discrete concentric rings where
/// this draws a solid tube.
const FOG_TUNNEL: &str = include_str!("../../tests/fixtures/milk_wash_fog_tunnel.toml");

/// *Geiss - Blur Mix 3* — the clean control. Its blur chain actively darkens, and
/// it was the one pair Plan 0100 Phase 7 judged genuinely good.
const BLUR_MIX_3: &str = include_str!("../../tests/fixtures/milk_wash_blur_mix_3.toml");

/// Square, so the outer ring is the same thickness on both axes and `edge` is not
/// weighted toward one of them.
const SIZE: u32 = 128;

/// Long enough for the feedback field to reach its equilibrium — Plan 0111 Phase
/// 1 measured that at roughly 100 frames of time constant, so 300 is three of
/// them.
const FRAMES: u32 = 300;

/// How thick the ring `edge` averages over. Two texels, matching the scene-level
/// probe this statistic is borrowed from.
const RING: u32 = 2;

/// Mean of the three colour channels over the outermost `RING` texels of an
/// `RGBA f32` buffer in row-major order.
fn edge(rgba: &[f32], width: u32, height: u32) -> f32 {
    let (mut sum, mut n) = (0f64, 0u64);
    for y in 0..height {
        for x in 0..width {
            let on_ring = x < RING || y < RING || x + RING >= width || y + RING >= height;
            if !on_ring {
                continue;
            }
            let base = ((y * width + x) * 4) as usize;
            for c in 0..3 {
                if let Some(v) = rgba.get(base + c) {
                    sum += f64::from(*v);
                    n += 1;
                }
            }
        }
    }
    if n == 0 { 0.0 } else { (sum / n as f64) as f32 }
}

/// One subject's background level at each seam of the chain, in the units that
/// seam carries.
#[derive(Debug, Clone, Copy)]
struct SeamTrace {
    /// After warp/deposit/draw, before the present pass. Linear.
    a_field: f32,
    /// After the present pass — the echo mix, `x brightness`, `x gamma`, the four
    /// composite remaps and `x occlude`. Linear. **Also seams C and D**, which do
    /// not exist for these subjects (module docs).
    b_present: f32,
    /// After the tonemap's shoulder. Display-referred, so comparable only against
    /// the other subject's `e_display`.
    e_display: f32,
}

/// Render `source` for [`FRAMES`] frames and read the background at every seam of
/// **one** run.
fn seam_trace(source: &str) -> Option<SeamTrace> {
    let preset = Preset::from_toml_str(source).expect("the fixture parses");
    let name = preset.name.clone();

    let mut renderer = match Renderer::new_headless(HeadlessOptions {
        width: SIZE,
        height: SIZE,
        prefer_software: false,
    }) {
        Ok(r) => r,
        Err(RenderError::RequestAdapter(_)) => {
            eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
            return None;
        }
        Err(e) => panic!("headless renderer build failed: {e}"),
    };
    renderer.set_presets(vec![preset]);

    // Seam E, and the thing that drives the run: `capture_preset` rebuilds the
    // scenes and resets the clock, so the whole trace is a pure function of
    // (fixture, frame, FRAMES). Everything read below belongs to its LAST frame.
    let display = renderer
        .capture_preset(&name, &AnalysisFrame::default(), FRAMES)
        .expect("the capture succeeds");
    let e_display = {
        let linear: Vec<f32> = display.rgba.iter().map(|b| f32::from(*b) / 255.0).collect();
        edge(&linear, display.width, display.height)
    };

    let a_field = read_linear(&renderer, Seam::Field)?;
    let b_present = read_linear(&renderer, Seam::Present)?;

    Some(SeamTrace {
        a_field,
        b_present,
        e_display,
    })
}

enum Seam {
    Field,
    Present,
}

/// Copy one of the run's linear intermediates back and reduce it to `edge`.
///
/// Both textures are `COMPOSITE_FORMAT` and both already carried `COPY_SRC`
/// before this probe existed — the field for Plan 0109 Phase 4, the tonemap's
/// input for its own CPU-mirror test. Nothing here adds a render target, which is
/// what keeps this probe clear of the adapter hazard ADR-0058 records.
fn read_linear(renderer: &Renderer, seam: Seam) -> Option<f32> {
    let texture = match seam {
        Seam::Field => {
            let system = renderer.roster.active_preset()?.system;
            scene_for(&renderer.scenes, system)?.feedback_field()?
        }
        Seam::Present => renderer.tonemap.src_texture()?,
    };
    let device = &renderer.ctx.device;
    let (width, height) = (texture.width(), texture.height());
    let (buffer, padded_bpr) = capture::create_linear_readback(device, width, height);
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("milk-wash-seam"),
    });
    capture::record_copy(&mut encoder, texture, &buffer, padded_bpr, width, height);
    renderer.ctx.queue.submit(std::iter::once(encoder.finish()));
    let rgba = capture::read_back_linear(device, &buffer, width, height, padded_bpr)
        .expect("the seam reads back");
    Some(edge(&rgba, width, height))
}

/// **The wash bisect.** Reports the background at every seam for a washed
/// conversion and the clean control, and names the first seam at which their
/// ratio departs from what it was at the field.
///
/// This test **asserts no threshold on the ratio** (ADR-0071). What separation is
/// large enough to call a departure is exactly what the phase is measuring, and a
/// number chosen before the measurement would be tuning to the instrument. What
/// it does assert is that the instrument works: both subjects render and every
/// seam reads back.
///
/// # What it measured, 2026-08-19
///
/// Dev box, 128x128, 300 frames, `AnalysisFrame::default()`, quantizer at its
/// `DEFAULT_QUANTIZE_STEPS = 255` (neither fixture overrides it). Hardware
/// readings are **bit-identical across three runs** — there is no run-to-run
/// spread on this instrument, so a tolerance would have to come from the
/// mechanism rather than from noise.
///
/// ```text
///   seam        fog tunnel    blur mix 3    ratio     (hardware)
///   A field     0.29798886    0.01990991    14.967
///   B present   0.52298039    0.08744538     5.981
///   E display   0.74454564    0.25118530     2.964
///
///   seam        fog tunnel    blur mix 3    ratio     (DX12 WARP)
///   A field     0.29793853    0.01192657    24.981
///   B present   0.52290142    0.05914328     8.841
///   E display   0.74456638    0.24515122     3.037
/// ```
///
/// **No seam departs upward. The ratio is maximal at the field and decreases
/// monotonically through every stage**, on both adapters. The present pass and
/// the tonemap compress the separation rather than creating it, so no downstream
/// stage is the wash and Plan 0111 Phase 3 did not run.
///
/// Two things the numbers do not say, both worth carrying:
///
/// - **`edge` may not be reading background on this render of *Fog Tunnel*.** The
///   statistic reads background only for a figure that does not fill the frame,
///   and the reference's discrete rings are exactly that — but *our* conversion
///   draws the solid tube that is the defect, so at the frame edge this may be
///   reading the figure. The monotonic trend survives either way, because it is
///   the same statistic at every seam; the absolute `14.967` does not.
/// - **The control diverges between adapters and the washed subject does not.**
///   *Blur Mix 3*'s field reads `1.67x` higher on hardware than on WARP while
///   *Fog Tunnel* agrees to five significant figures. *Blur Mix 3* is the subject
///   with a blur chain. So any threshold on this ratio would be adapter-dependent,
///   which is the second reason this test asserts none.
#[test]
fn the_wash_bisect_reports_every_seam() {
    let Some(fog) = seam_trace(FOG_TUNNEL) else {
        return;
    };
    let blur = seam_trace(BLUR_MIX_3).expect("the second subject runs too");

    let ratio = |w: f32, c: f32| if c > 0.0 { w / c } else { f32::INFINITY };
    println!("[wash] seam          fog tunnel     blur mix 3     ratio");
    println!(
        "[wash] A field       {:>12.8}  {:>12.8}  {:>8.3}",
        fog.a_field,
        blur.a_field,
        ratio(fog.a_field, blur.a_field)
    );
    println!(
        "[wash] B present*    {:>12.8}  {:>12.8}  {:>8.3}",
        fog.b_present,
        blur.b_present,
        ratio(fog.b_present, blur.b_present)
    );
    println!(
        "[wash] E display     {:>12.8}  {:>12.8}  {:>8.3}",
        fog.e_display,
        blur.e_display,
        ratio(fog.e_display, blur.e_display)
    );
    println!(
        "[wash] * B is also seams C and D: no post stage is active and the backdrop is unbound"
    );

    assert!(
        fog.a_field.is_finite() && blur.a_field.is_finite(),
        "both subjects must reach the field seam"
    );
}
