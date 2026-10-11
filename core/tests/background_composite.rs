//! Background compositing over the fullscreen/accumulating scenes (Plan 0025,
//! ADR-0026). The reaction-diffusion and attractor presents now alpha-blend over
//! the `bg_*` backdrop, so the field's voids / the cloud's empty space reveal the
//! tintable gradient instead of an unconditional black.
//!
//! This check is **real-hardware only**. It renders each scene under two backdrop
//! hues over the *same* field and asserts the revealed tint tracks `bg_hue`. The
//! DX12 WARP software rasterizer mis-renders these scenes' full pipeline set once
//! the background pipeline coexists on the device (the same documented quirk that
//! makes the reaction-diffusion / attractor resources build lazily), so the reveal
//! is invisible there — a WARP reading, unverified on lavapipe as of 2026-10-06.
//! We therefore request the *default* (hardware) adapter and **skip** when only a
//! software rasterizer is available — the reveal is verified on real hardware (dev
//! boxes, macOS Metal CI) and via the `shot` CLI, and left unasserted where the
//! adapter can't render it.
//!
//! One renderer per `#[test]`. The file holds a hardware test and a software
//! test, so it **requires nextest**, which runs each test in its own process:
//! only there does the hardware renderer never coexist with a software device.
//! A plain `cargo test` runs both as threads of one process and voids that.
//!
//! The **paper** backdrop (ADR-0270) is checked here too, on the software
//! adapter: it is a byte-equality claim about the tonemap's write, which holds
//! on any adapter that renders the scene at all.

/// The shared ADR-0016 skip and headless constructors.
mod common;

use rlx_core::dsp::AnalysisFrame;
use rlx_core::preset::Preset;
use rlx_core::render::{CaptureImage, Renderer};

const SIZE: u32 = 96;

/// Build a **hardware** headless renderer (no `prefer_software`), or `None` (a
/// logged skip) when the runner exposes no adapter *or* only a software one — the
/// coexistence this test probes is a real-hardware behaviour (see the module docs).
fn hardware() -> Option<Renderer> {
    common::headless_hardware_for(
        SIZE,
        SIZE,
        None,
        "measured on WARP, unverified on lavapipe as of 2026-10-06: WARP mis-renders the \
         fullscreen-scene + background coexistence (see module docs)",
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

/// The paper fixture: a solid voxel block over `bg_paper = 1`, `bg_bright = 1`
/// on a palette that is white at `bg_hue = 1`.
const PAPER: &str = include_str!("fixtures/backdrop_paper.toml");

/// How many frames a paper capture renders. The block is frozen
/// (`step_rate = 0`), so any count draws the same picture.
const PAPER_FRAMES: u32 = 4;

/// [`PAPER`] with `bg_paper` and `bg_bright` rebound, as one parsed preset.
/// Panics if either line is not in the fixture, so a reworded fixture cannot
/// turn this into a capture of the fixture unchanged.
fn paper_variant(paper: &str, bright: &str) -> Preset {
    let toml = PAPER
        .replacen(
            "bg_paper     = \"1\"",
            &format!("bg_paper     = \"{paper}\""),
            1,
        )
        .replacen(
            "bg_bright    = \"1\"",
            &format!("bg_bright    = \"{bright}\""),
            1,
        );
    assert_eq!(
        toml.matches(&format!("bg_paper     = \"{paper}\"")).count()
            + toml
                .matches(&format!("bg_bright    = \"{bright}\""))
                .count(),
        2,
        "the paper fixture's bg_paper / bg_bright lines moved"
    );
    Preset::from_toml_str(&toml).expect("the paper fixture parses")
}

/// Capture one preset alone, at [`SIZE`], on the software adapter's renderer.
fn capture(r: &mut Renderer, preset: Preset) -> CaptureImage {
    let name = preset.name.clone();
    r.set_presets(vec![preset]);
    r.capture_preset(&name, &common::fixed_frame(), PAPER_FRAMES)
        .expect("capture the paper fixture")
}

/// **A paper reaches display white, and the scene's covered pixels do not
/// move** (ADR-0270).
///
/// 1. With `bg_paper = 1`, `bg_bright = 1` over a white stop, every corner —
///    where the block draws nothing — reads exactly `(255, 255, 255)`. Through
///    the linear backdrop the tonemap's shoulder caps the same ground at 0.8.
/// 2. Every pixel the block **fully covers** is byte-identical to the same scene
///    with `bg_paper = 0` over a black ground. Coverage is found from the two
///    paper renders rather than assumed: a pixel identical under a white paper
///    and a mid-grey one is one no paper shows through. The mask must be a real
///    part of the frame and must exclude the corners, or the comparison would be
///    vacuous.
#[test]
fn a_paper_reaches_white_and_leaves_covered_pixels_unchanged() {
    let Some(mut r) = common::headless(SIZE, SIZE) else {
        return;
    };
    let white = capture(&mut r, paper_variant("1", "1"));
    let grey = capture(&mut r, paper_variant("1", "0.4"));
    let black = capture(&mut r, paper_variant("0", "0"));

    let at = |img: &CaptureImage, x: u32, y: u32| {
        let i = ((y * SIZE + x) * 4) as usize;
        [img.rgba[i], img.rgba[i + 1], img.rgba[i + 2]]
    };
    for (x, y) in [(0, 0), (SIZE - 1, 0), (0, SIZE - 1), (SIZE - 1, SIZE - 1)] {
        assert_eq!(
            at(&white, x, y),
            [255, 255, 255],
            "the paper corner ({x}, {y}) is not display white"
        );
    }

    let covered: Vec<bool> = white
        .rgba
        .chunks_exact(4)
        .zip(grey.rgba.chunks_exact(4))
        .map(|(w, g)| w[..3] == g[..3])
        .collect();
    let n_covered = covered.iter().filter(|&&c| c).count();
    let total = covered.len();
    assert!(
        n_covered * 100 > total * 10 && n_covered * 100 < total * 90,
        "the block covers {n_covered}/{total} pixels; the fixture must cover the middle only"
    );
    for (x, y) in [(0, 0), (SIZE - 1, SIZE - 1)] {
        assert!(
            !covered[(y * SIZE + x) as usize],
            "the coverage mask claims the corner ({x}, {y})"
        );
    }
    let moved: Vec<usize> = covered
        .iter()
        .enumerate()
        .filter(|&(i, &c)| c && white.rgba[i * 4..i * 4 + 3] != black.rgba[i * 4..i * 4 + 3])
        .map(|(i, _)| i)
        .collect();
    assert!(
        moved.is_empty(),
        "{} covered pixels differ between paper and a black ground, first at index {:?}",
        moved.len(),
        moved.first()
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
