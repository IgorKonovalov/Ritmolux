//! Background compositing over the fullscreen/accumulating scenes (Plan 0025,
//! ADR-0026). The reaction-diffusion and attractor presents now alpha-blend over
//! the `bg_*` backdrop, so the field's voids / the cloud's empty space reveal the
//! tintable gradient instead of an unconditional black.
//!
//! This check is **real-hardware only**. It renders each scene under two backdrop
//! hues over the *same* field and asserts the revealed tint tracks `bg_hue`. The
//! DX12 WARP software rasterizer the other GPU suites use mis-renders these scenes'
//! full pipeline set once the background pipeline coexists on the device (the same
//! documented quirk that makes the reaction-diffusion / attractor resources build
//! lazily), so the reveal is invisible there. We therefore request the *default*
//! (hardware) adapter and **skip** when only a software rasterizer is available —
//! the reveal is verified on real hardware (dev boxes, macOS Metal CI) and via the
//! `shot` CLI, and left unasserted where the adapter can't render it.
//!
//! One `#[test]` per file (its own test binary → its own process) so the single
//! hardware renderer never coexists with the other suites' WARP devices.

/// The shared ADR-0016 skip and headless constructors.
mod common;

use lmv_core::dsp::AnalysisFrame;
use lmv_core::preset::Preset;
use lmv_core::render::{CaptureImage, Renderer};

const SIZE: u32 = 96;

/// Build a **hardware** headless renderer (no `prefer_software`), or `None` (a
/// logged skip) when the runner exposes no adapter *or* only a software one — the
/// coexistence this test probes is a real-hardware behaviour (see the module docs).
fn hardware() -> Option<Renderer> {
    common::headless_hardware_for(
        SIZE,
        SIZE,
        None,
        "WARP mis-renders the fullscreen-scene + background coexistence (see module docs)",
    )
}

/// Mean (r, g, b) over the pixels selected by `mask`.
fn masked_mean(img: &CaptureImage, mask: &[bool]) -> (f32, f32, f32) {
    let (mut sr, mut sg, mut sb, mut n) = (0u64, 0u64, 0u64, 0u64);
    for (px, &m) in img.rgba.chunks_exact(4).zip(mask.iter()) {
        if m {
            sr += px[0] as u64;
            sg += px[1] as u64;
            sb += px[2] as u64;
            n += 1;
        }
    }
    let n = n.max(1) as f32;
    (sr as f32 / n, sg as f32 / n, sb as f32 / n)
}

/// Render `scene` twice — under a hue-0 (red-dominant) and a hue-0.5
/// (green/blue-dominant) backdrop, with identical field params so the field is the
/// same in both — and assert that the voids reveal the backdrop: wherever the two
/// captures diverge (the field is transparent), the hue-0 capture reads red-dominant
/// and the hue-0.5 capture reads green/blue-dominant. Structure that occludes the
/// backdrop is identical in both and excluded by the divergence mask.
fn assert_backdrop_reveal(r: &mut Renderer, scene: &str, field: &str, frames: u32) {
    let frame = AnalysisFrame {
        bass: 0.5,
        mid: 0.3,
        treb: 0.5,
        ..Default::default()
    };
    let preset = |hue: &str| {
        let toml = format!(
            "system = \"{scene}\"\nname = \"probe\"\n[params]\n{field}\
             bg_hue = \"{hue}\"\nbg_bright = \"0.9\"\n"
        );
        Preset::from_toml_str(&toml).unwrap_or_else(|e| panic!("{scene} probe preset parses: {e}"))
    };
    // Two rosters of one preset each; capture_preset selects by the shared name.
    r.set_presets(vec![preset("0.0")]);
    let red = r
        .capture_preset("probe", &frame, frames)
        .unwrap_or_else(|e| panic!("capture {scene} hue-0: {e}"));
    r.set_presets(vec![preset("0.5")]);
    let cyan = r
        .capture_preset("probe", &frame, frames)
        .unwrap_or_else(|e| panic!("capture {scene} hue-0.5: {e}"));

    // Void mask: pixels where the two backdrops give a substantially different
    // colour, i.e. where the scene is transparent enough for the backdrop to show.
    const REVEAL_EPS: u8 = 30;
    let mask: Vec<bool> = red
        .rgba
        .chunks_exact(4)
        .zip(cyan.rgba.chunks_exact(4))
        .map(|(pr, pc)| (0..3).any(|c| pr[c].abs_diff(pc[c]) > REVEAL_EPS))
        .collect();
    let n_void = mask.iter().filter(|&&m| m).count();
    let total = red.rgba.len() / 4;
    assert!(
        n_void * 100 > total * 5,
        "{scene}: too few backdrop-revealing pixels ({n_void}/{total}) — voids are not showing the backdrop"
    );

    let (rr, rg, rb) = masked_mean(&red, &mask);
    let (cr, cg, cb) = masked_mean(&cyan, &mask);
    assert!(
        rr > 40.0,
        "{scene}: revealed hue-0 backdrop too dark to be a real reveal: red {rr:.1}"
    );
    assert!(
        rr > rg + 20.0 && rr > rb + 20.0,
        "{scene}: hue-0 void reveal is not red-dominant: rgb ({rr:.1}, {rg:.1}, {rb:.1})"
    );
    assert!(
        cg > cr + 20.0 && cb > cr + 20.0,
        "{scene}: hue-0.5 void reveal is not green/blue-dominant: rgb ({cr:.1}, {cg:.1}, {cb:.1})"
    );
}

#[test]
fn fullscreen_scenes_reveal_backdrop() {
    let Some(mut r) = hardware() else {
        return;
    };

    // Reaction-diffusion (Plan 0025 Phase 1): the mitosis regime leaves dark voids
    // between the contours; those voids must reveal the backdrop.
    assert_backdrop_reveal(
        &mut r,
        "reaction_diffusion",
        "feed = \"0.037\"\nkill = \"0.06\"\nflow = \"1.0\"\nglow = \"0.8\"\n",
        60,
    );

    // Attractor (Plan 0025 Phase 3): the De Jong cloud fills a fractal shape with
    // large empty space around it; that empty space must reveal the backdrop.
    assert_backdrop_reveal(&mut r, "attractor", "size = \"1.0\"\n", 60);
}
