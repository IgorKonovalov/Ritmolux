//! The backdrop colours through the preset's palette (Plan 0072 Phase 1,
//! ADR-0086).
//!
//! An inline copy of the iq cosine in `background.rs` leaves
//! `[palette]` stopping at the scene and never reaching the sky; the
//! pass samples the same baked LUT pair every other scene samples
//! instead. This suite is the two halves of that claim:
//!
//! - **Nothing moved for a preset that declares no `[palette]`.** The built-in
//!   `spectrum` gradient is generated from the identical cosine, so the only
//!   difference is the LUT's quantization and interpolation — sub-LSB after
//!   `bg_bright`. Asserted as a **bound**, not as a frozen number.
//! - **A declared palette actually reaches the backdrop.** A flat two-stop
//!   palette makes `bg_hue` a no-op and paints its own colour, and swapping the
//!   stop swaps the sky.
//!
//! # Why both halves are differentials
//!
//! Every capture here is compared against **another capture through the same
//! pipeline**, never against a hand-computed colour. The vertical gradient, the
//! vignette, the exposure/tonemap boundary and the 8-bit display write are all
//! downstream of the tint and all identical between the two sides, so they cancel
//! exactly. That is what lets the first test assert a one-level bound at all: a
//! test that recomputed the shader's arithmetic would be pinning the shader to
//! itself, and one that pinned bytes would be a golden baseline in the wrong file.
//!
//! **Software adapter** (`prefer_software`), like the rest of the GPU suites
//! (ADR-0016), so this runs on CI rather than only where a GPU happens to be. The
//! backdrop pass and the swarm's sprite pipeline are the only two live here; the
//! new `background-lut-layout` is shape-identical to `fragment-field-lut-layout`,
//! which is the ADR-0058 aliasing configuration — the hardware-vs-WARP comparison
//! that clears it is recorded in `core/src/render/background.rs`.

use lmv_core::dsp::AnalysisFrame;
use lmv_core::preset::Preset;
use lmv_core::render::{CaptureImage, HeadlessOptions, RenderError, Renderer};

/// Capture size. Small on purpose — every assertion below is per-pixel over the
/// whole frame, and the property does not vary with resolution.
const SIZE: u32 = 64;

/// Frames per capture. The scene contributes nothing, so one settled frame is the
/// whole picture; a handful keeps the clock off zero.
const FRAMES: u32 = 4;

/// `bg_bright` for every capture — the largest value anywhere in the repo, and the
/// one ADR-0086's arithmetic is stated at. The error being bounded scales with it,
/// so the shipped presets (which top out at 0.039) are covered a fortiori.
const BRIGHT: f32 = 0.55;

/// The `d` term of the iq cosine `spectrum` bakes from (`palette.rs:109`) — the
/// constant an inline copy in `background.rs` would carry, and the reference
/// the no-palette half is measured against.
const COSINE_D: [f32; 3] = [0.10, 0.42, 0.62];

/// The analytic cosine at `t`, per channel: `0.5 + 0.5*cos(2π*(t + d))`, clamped.
/// This is the colour the old inline `palette()` produced, restated here as the
/// *input* to a flat palette rather than as an expected pixel value.
fn cosine_at(t: f32) -> [f32; 3] {
    let mut out = [0.0f32; 3];
    for (channel, d) in out.iter_mut().zip(COSINE_D) {
        *channel = (0.5 + 0.5 * (std::f32::consts::TAU * (t + d)).cos()).clamp(0.0, 1.0);
    }
    out
}

/// A backdrop-only preset: a swarm whose sprites have zero area, so no fragment
/// is ever rasterized and the frame is the backdrop alone, through the real
/// composite. `palette` is the whole `[palette]` table (empty for the default).
fn backdrop_preset(palette: &str, hue: f32) -> Preset {
    let toml = format!(
        "system = \"swarm\"\nname = \"probe\"\n{palette}\n[params]\n\
         size = \"0\"\nforce = \"0\"\nspin = \"0\"\nburst = \"0\"\n\
         bg_hue = \"{hue}\"\nbg_bright = \"{BRIGHT}\"\nbg_vignette = \"0.35\"\n"
    );
    Preset::from_toml_str(&toml).unwrap_or_else(|e| panic!("the probe preset parses: {e}"))
}

/// A `[palette]` table of two identical stops — a gradient that is one colour, so
/// `bg_hue` selects the same value wherever it lands.
fn flat_palette(rgb: [f32; 3]) -> String {
    let [r, g, b] = rgb;
    format!(
        "[palette]\nstops = [{{ at = 0.0, color = [{r}, {g}, {b}] }}, \
         {{ at = 1.0, color = [{r}, {g}, {b}] }}]"
    )
}

/// Build a software headless renderer, or `None` (a logged skip) when the runner
/// exposes no adapter at all (ADR-0016).
fn renderer() -> Option<Renderer> {
    match Renderer::new_headless(HeadlessOptions {
        width: SIZE,
        height: SIZE,
        prefer_software: true,
    }) {
        Ok(renderer) => Some(renderer),
        Err(RenderError::RequestAdapter(_)) => {
            eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
            None
        }
        Err(e) => panic!("headless renderer build failed: {e}"),
    }
}

/// Capture one backdrop-only preset.
fn capture(renderer: &mut Renderer, palette: &str, hue: f32) -> CaptureImage {
    renderer.set_presets(vec![backdrop_preset(palette, hue)]);
    renderer
        .capture_preset("probe", &AnalysisFrame::default(), FRAMES)
        .unwrap_or_else(|e| panic!("capture the backdrop probe at bg_hue = {hue}: {e}"))
}

/// The largest per-channel difference between two captures, in 8-bit levels, and
/// how many channels differ at all.
fn worst_diff(a: &CaptureImage, b: &CaptureImage) -> (u8, usize) {
    assert_eq!(a.rgba.len(), b.rgba.len(), "the captures differ in size");
    let mut worst = 0u8;
    let mut differing = 0usize;
    for (pa, pb) in a.rgba.chunks_exact(4).zip(b.rgba.chunks_exact(4)) {
        // Alpha is a constant 255 on both sides; the claim is about colour.
        for channel in 0..3 {
            let diff = pa[channel].abs_diff(pb[channel]);
            if diff > 0 {
                differing += 1;
            }
            worst = worst.max(diff);
        }
    }
    (worst, differing)
}

/// The mean (r, g, b) of a capture.
fn mean(image: &CaptureImage) -> (f32, f32, f32) {
    let (mut sr, mut sg, mut sb) = (0u64, 0u64, 0u64);
    let n = (image.rgba.len() / 4).max(1) as f32;
    for px in image.rgba.chunks_exact(4) {
        sr += px[0] as u64;
        sg += px[1] as u64;
        sb += px[2] as u64;
    }
    (sr as f32 / n, sg as f32 / n, sb as f32 / n)
}

/// Assert the frame is not black, so a comparison between two dark frames cannot
/// pass by having nothing in it.
fn assert_lit(image: &CaptureImage, what: &str) {
    let (r, g, b) = mean(image);
    assert!(
        r.max(g).max(b) > 8.0,
        "{what}: the backdrop is essentially black (mean rgb {r:.1}, {g:.1}, {b:.1}), \
         so every comparison against it is vacuous"
    );
}

/// **A preset that declares no `[palette]` did not move** — the load-bearing
/// no-regression half of ADR-0086, asserted as a bound.
///
/// `spectrum` is generated from that same `d = (0.10, 0.42, 0.62)`, so the two
/// differ only by the LUT: linear interpolation of that cosine over a 1/256
/// step errs by at most ~3.8e-5, and `Rgba8Unorm` storage adds at most half a
/// step (~2.0e-3). Both are multiplied by `bg_bright`, so at this file's 0.55
/// the linear-light error is ~1.1e-3 — about a quarter of one 8-bit level,
/// before `grad` and `vig` shrink it further.
///
/// # How the reference is produced without re-deriving the shader
///
/// The right-hand side is the **same shader** driven by a flat two-stop palette
/// holding the analytic cosine at that `bg_hue`. Everything downstream of the tint
/// is bit-identical between the two captures, so what survives the subtraction is
/// exactly the LUT error and nothing else. It costs one extra 8-bit rounding (the
/// reference stop is itself baked into a `Rgba8Unorm` LUT), which is why the bound
/// asserted is one level rather than the ADR's quarter.
///
/// Two hues, both away from the cosine's channel extrema where the curvature — and
/// therefore the interpolation error this bounds — is largest.
#[test]
fn a_preset_with_no_palette_renders_the_cosine_backdrop_it_always_did() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    for hue in [0.30f32, 0.62] {
        let default = capture(&mut renderer, "", hue);
        assert_lit(&default, "the default-palette backdrop");
        let reference = capture(&mut renderer, &flat_palette(cosine_at(hue)), hue);

        let (worst, differing) = worst_diff(&default, &reference);
        assert!(
            worst <= 1,
            "at bg_hue = {hue} the LUT'd `spectrum` backdrop differs from the \
             analytic cosine by {worst} 8-bit levels across {differing} channels. \
             ADR-0086's whole no-regression argument is that this is sub-LSB; a \
             failure here means the eleven shipped palette-less presets moved."
        );
    }
}

/// **A declared palette reaches the backdrop, and `bg_hue` is a coordinate in
/// it** — the positive proof that the LUT is actually sampled.
///
/// A flat two-stop gradient is the same colour at every coordinate, so if the
/// backdrop is really reading that gradient then `bg_hue` cannot change the
/// picture. Under the old inline cosine it changed it completely: at 0.10 the
/// cosine is red-dominant and at 0.60 it is blue-dominant, which is the drift this
/// asserts is now zero.
///
/// The colour itself is pinned by comparison rather than by value: the same
/// fixture under a channel-swapped stop must produce a channel-swapped sky. That
/// holds whatever the tonemap does downstream, because it does the same thing to
/// both.
#[test]
fn a_flat_declared_palette_paints_the_backdrop_at_every_hue() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    // The colour the two lit-backdrop guard fixtures declare (`#ffcf80`), so this
    // test and they are talking about the same sky.
    const WARM: [f32; 3] = [1.0, 0.812, 0.502];
    const COOL: [f32; 3] = [0.502, 0.812, 1.0];

    let warm = flat_palette(WARM);
    let anchor = capture(&mut renderer, &warm, 0.0);
    assert_lit(&anchor, "the flat-palette backdrop");

    // `bg_hue` is a no-op on a one-colour gradient — including past 1.0, where the
    // repeat-addressed sampler wraps rather than clamping.
    for hue in [0.10f32, 0.5, 0.85, 1.37, -0.25] {
        let moved = capture(&mut renderer, &warm, hue);
        let (worst, differing) = worst_diff(&anchor, &moved);
        assert!(
            worst <= 1,
            "a flat two-stop palette is one colour everywhere, so bg_hue = {hue} \
             must paint the same backdrop as bg_hue = 0 — it differs by {worst} \
             levels across {differing} channels. The backdrop is still reading \
             something other than the preset's gradient."
        );
    }

    // ...and the colour is the declared one: swap the stop's channels and the sky
    // swaps with it.
    let (wr, wg, wb) = mean(&anchor);
    let cool = capture(&mut renderer, &flat_palette(COOL), 0.0);
    let (cr, cg, cb) = mean(&cool);
    assert!(
        wr > wg && wg > wb,
        "the warm stop must give a warm backdrop: mean rgb ({wr:.1}, {wg:.1}, {wb:.1})"
    );
    assert!(
        cb > cg && cg > cr,
        "the cool stop must give a cool backdrop: mean rgb ({cr:.1}, {cg:.1}, {cb:.1})"
    );
}

/// **`saturation` and `palette_mix` reach the backdrop too** (ADR-0086's
/// "join halfway" rejection, made checkable).
///
/// Both are the *scene's* params — no system's vocabulary changed and
/// `background::PARAMS` is untouched — so this is really a test of the routing
/// fan-out: one binding, two consumers. The scene draws nothing here, so anything
/// that moves is the sky.
///
/// `saturation = 0` collapses the declared colour to its Rec. 601 luma, which is
/// grey by construction; `palette_mix = 1` crosses fully to `[palette_b]`, so a
/// warm A over a cool B must land on the cool one.
#[test]
fn the_shared_colour_modulations_reach_the_backdrop() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let warm = "[palette]\nstops = [{ at = 0.0, color = [1.0, 0.4, 0.1] }, \
                { at = 1.0, color = [1.0, 0.4, 0.1] }]";

    let plain = capture(&mut renderer, warm, 0.3);
    assert_lit(&plain, "the unmodulated backdrop");
    let (pr, pg, pb) = mean(&plain);
    assert!(
        pr > pb + 10.0,
        "sanity: the unmodulated backdrop is warm ({pr:.1}, {pg:.1}, {pb:.1})"
    );

    // `saturation` is a scene param the preset binds once; the sky answers.
    let grey_toml = format!("{warm}\n");
    renderer.set_presets(vec![
        Preset::from_toml_str(&format!(
            "system = \"swarm\"\nname = \"probe\"\n{grey_toml}[params]\n\
             size = \"0\"\nforce = \"0\"\nspin = \"0\"\nburst = \"0\"\n\
             saturation = \"0\"\n\
             bg_hue = \"0.3\"\nbg_bright = \"{BRIGHT}\"\nbg_vignette = \"0.35\"\n"
        ))
        .expect("the desaturated probe parses"),
    ]);
    let grey = renderer
        .capture_preset("probe", &AnalysisFrame::default(), FRAMES)
        .expect("capture the desaturated backdrop");
    let (gr, gg, gb) = mean(&grey);
    assert!(
        (gr - gg).abs() <= 2.0 && (gg - gb).abs() <= 2.0,
        "`saturation = 0` must leave the backdrop grey: mean rgb \
         ({gr:.1}, {gg:.1}, {gb:.1}) — the sky is still ignoring the scene's \
         saturation binding"
    );

    // `palette_mix = 1` is palette B, on the backdrop as on the figure.
    renderer.set_presets(vec![
        Preset::from_toml_str(&format!(
            "system = \"swarm\"\nname = \"probe\"\n{warm}\n\
             [palette_b]\nstops = [{{ at = 0.0, color = [0.1, 0.4, 1.0] }}, \
             {{ at = 1.0, color = [0.1, 0.4, 1.0] }}]\n[params]\n\
             size = \"0\"\nforce = \"0\"\nspin = \"0\"\nburst = \"0\"\n\
             palette_mix = \"1\"\n\
             bg_hue = \"0.3\"\nbg_bright = \"{BRIGHT}\"\nbg_vignette = \"0.35\"\n"
        ))
        .expect("the crossfaded probe parses"),
    ]);
    let crossed = renderer
        .capture_preset("probe", &AnalysisFrame::default(), FRAMES)
        .expect("capture the crossfaded backdrop");
    let (xr, xg, xb) = mean(&crossed);
    assert!(
        xb > xr + 10.0,
        "`palette_mix = 1` must paint the backdrop from [palette_b]: mean rgb \
         ({xr:.1}, {xg:.1}, {xb:.1}) is still the warm palette A — the A/B \
         crossfade moves the figure and leaves the sky behind"
    );
}
