//! **A palette stop is authored in sRGB, and the ink it names is the ink that
//! renders** (ADR-0151, Plan 0138 Phase 2).
//!
//! The stop is decoded to linear light once at the load boundary and the display
//! write encodes it again, so the two are inverses and the whole round trip is
//! the identity — *below the tonemap knee*, where ADR-0046's curve is itself the
//! identity. That domain is the substance of the claim and it is asserted here
//! before the colour is: a probe above the knee would be measuring the tonemap.
//!
//! `#c81423` is the case ADR-0151 was measured on. Consumed raw it rendered
//! `#dd4c64`, its green channel nearly quadrupled; the hand correction that
//! cancelled the encode reached `#c81622` — within 2/255 of the ink named. This
//! asserts the engine now reaches that same place with no correction, and it is
//! asserted at the same 2/255 for the same reason: **the LUT is 256 entries of
//! 8-bit *linear* light** (ADR-0021's `Rgba8Unorm`), and a linear byte is a
//! coarse step down in the shadows — `#14`'s light rounds to 2/255 there, which
//! is two encoded levels away from where it started. That residual is the LUT's
//! storage, not the transfer function this file is about, and it is what the
//! ADR's own two levels are.
//!
//! So the plateau is what is asserted (a mean over a flat frame), and the
//! **spread** is asserted separately: the tonemap's static `+-1` encoded-level
//! dither (ADR-0096) is on every pixel and is part of the display write, not of
//! the colour.
//!
//! # The probe is a backdrop, so that nothing is measured but the palette
//!
//! The frame is painted by the backdrop pass alone — a swarm whose sprites have
//! zero area draws no fragment — and its three modulations are pinned to their
//! identities: `bg_bright = 1`, a flat shade ramp, no vignette. What reaches the
//! capture is then the LUT's own colour and nothing else, which is what makes a
//! per-pixel equality assertable at all. Skips with no adapter (ADR-0016).

mod common;

use rlx_core::dsp::AnalysisFrame;
use rlx_core::preset::Preset;
use rlx_core::render::palette::srgb_to_linear;

/// Capture size. The frame is flat, so the property does not vary with it.
const SIZE: u32 = 64;
/// Frames per capture — enough to be past the first, none of which matters to a
/// static backdrop.
const FRAMES: u32 = 4;

/// The ink ADR-0151 measured, and the bytes it must come back as.
const INK: [u8; 3] = [0xc8, 0x14, 0x23];

/// ADR-0046's tonemap knee, restated because `render::tonemap` is crate-private.
/// Below it the curve is exactly the identity.
const KNEE: f32 = 0.6;

/// How far the plateau may sit from the ink: the 2/255 ADR-0151 states, which is
/// the LUT's 8-bit linear storage in the shadows and nothing else.
const PLATEAU_TOL: f32 = 2.0;

/// How far the frame may vary across itself: the display dither is `+-1`
/// encoded level (ADR-0096), so two adjacent levels is the whole admitted range.
const SPREAD_TOL: i32 = 2;

#[test]
fn a_stop_written_as_an_ink_renders_that_ink() {
    // The claim has a domain, so the domain is checked first: every channel of
    // the probe must decode to light under the knee, or the tonemap is in the
    // measurement and this test is about something else.
    for (c, byte) in INK.iter().enumerate() {
        let linear = srgb_to_linear(f32::from(*byte) / 255.0);
        assert!(
            linear <= KNEE,
            "channel {c} of the probe decodes to {linear:.4}, above the knee at {KNEE} — \
             above it the tonemap is not the identity and this test would be asserting \
             the curve rather than the palette"
        );
    }

    let Some(mut renderer) = common::headless(SIZE, SIZE) else {
        return;
    };

    let hex = format!("#{:02x}{:02x}{:02x}", INK[0], INK[1], INK[2]);
    let toml = format!(
        "system = \"swarm\"\nname = \"ink\"\n\
         [palette]\nstops = [{{ at = 0.0, color = \"{hex}\" }}, \
         {{ at = 1.0, color = \"{hex}\" }}]\n\
         [params]\n\
         size = \"0\"\nforce = \"0\"\nspin = \"0\"\nburst = \"0\"\n\
         bg_hue = \"0.5\"\nbg_bright = \"1\"\nbg_vignette = \"0\"\n\
         bg_shade = \"1\"\nbg_shade_end = \"1\"\n"
    );
    let preset = Preset::from_toml_str(&toml).expect("the ink probe parses");
    renderer.set_presets(vec![preset]);
    let img = renderer
        .capture_preset("ink", &AnalysisFrame::default(), FRAMES)
        .expect("capture the ink probe");

    let mut lo = [255i32; 3];
    let mut hi = [0i32; 3];
    let mut sum = [0f64; 3];
    let pixels = (img.rgba.len() / 4).max(1) as f64;
    for px in img.rgba.chunks_exact(4) {
        for c in 0..3 {
            lo[c] = lo[c].min(i32::from(px[c]));
            hi[c] = hi[c].max(i32::from(px[c]));
            sum[c] += f64::from(px[c]);
        }
    }
    let plateau = [0, 1, 2].map(|c| (sum[c] / pixels) as f32);
    println!("authored {INK:?}, plateau {plateau:?}, read {lo:?}..{hi:?}");

    for c in 0..3 {
        let want = f32::from(INK[c]);
        // What the same stop would have painted before ADR-0151, when the number
        // was consumed as light: the transfer function applied instead of
        // cancelled. Printed on failure so a regression names itself.
        let raw = 255.0 * (1.055 * (want / 255.0).powf(1.0 / 2.4) - 0.055).clamp(0.0, 1.0);
        assert!(
            (plateau[c] - want).abs() <= PLATEAU_TOL,
            "channel {c}: the stop was written {want} and the frame's plateau reads {}. \
             A stop is sRGB and is decoded at load, so under the knee the decode and the \
             display encode cancel; what is left is the LUT's own 8-bit linear storage, \
             which is the {PLATEAU_TOL} levels admitted. Consumed as light this channel \
             would read about {raw:.0}",
            plateau[c],
        );
        assert!(
            hi[c] - lo[c] <= SPREAD_TOL,
            "channel {c}: the frame spans {}..{} — the backdrop is pinned flat here, so \
             anything wider than the display dither's one encoded level is a second \
             modulation reaching the sky",
            lo[c],
            hi[c],
        );
    }
}
