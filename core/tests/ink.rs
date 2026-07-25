//! Final-stage ink tone-remap contract (Plan 0029 Phase 3, ADR-0028).
//!
//! ADR-0028 filed an ink-on golden as optional ("Neutral"), which was reasonable
//! before a curated preset depended on the stage. It now ships in one, and
//! nothing asserted the stage does anything: `sanity`/`animation`/`reactivity`
//! sweep the shipped `attractor_ink` preset and would pass just as happily if
//! `Ink::resolve` were a passthrough, because none of them looks at *tone*.
//!
//! So this file defends the one property that makes the stage the stage — the
//! remap **inverts** tone — plus the passthrough claim the whole ADR-0018
//! skippable discipline rests on: at `ink_amount = 0` the frame must be
//! byte-identical to one rendered with no `ink_*` binding at all.
//!
//! One frozen fixture, rendered three ways. It is the `parametric_curve` golden
//! fixture (a static Maurer rose): sparse and deterministic, so the frame is
//! mostly dark base with bright strokes — exactly the input whose inversion is
//! visible in a mean. It binds no `bg_*`, so the background pass stays a plain
//! clear and the only pipelines coexisting on the adapter are the line renderer's
//! and the remap's — faithful on the WARP software rasterizer (unlike the
//! background-gradient coexistence `background_composite.rs` documents). Skips
//! with no adapter per ADR-0016.

use lmv_core::dsp::AnalysisFrame;
use lmv_core::preset::Preset;
use lmv_core::render::{CaptureImage, HeadlessOptions, RenderError, Renderer};

const SIZE: u32 = 96;
/// Enough frames for the static rose to be fully drawn; the fixture is
/// time-independent, so this only has to clear the draw-in.
const FRAMES: u32 = 30;

/// The frozen fixture, reused verbatim from the golden roster. Its `[params]`
/// table is last, so an appended `ink_*` line lands inside it.
const FIXTURE: &str = include_str!("fixtures/parametric_curve.toml");
const FIXTURE_NAME: &str = "fixture_parametric_curve";

/// The fixture with `extra` appended to its `[params]` table.
fn fixture_with(extra: &str) -> Preset {
    let toml = format!("{FIXTURE}{extra}");
    Preset::from_toml_str(&toml).unwrap_or_else(|e| panic!("ink fixture parses: {e}"))
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

/// Rec. 709 luminance (0..255) of one RGBA pixel — the same weights the remap
/// shader reads as ink density.
fn luma(px: &[u8]) -> f32 {
    0.2126 * px[0] as f32 + 0.7152 * px[1] as f32 + 0.0722 * px[2] as f32
}

/// Mean luminance over the whole frame.
fn mean_luma(img: &CaptureImage) -> f32 {
    let n = (img.rgba.len() / 4).max(1) as f64;
    let sum: f64 = img.rgba.chunks_exact(4).map(|px| luma(px) as f64).sum();
    (sum / n) as f32
}

/// Mean luminance of `img` over the pixels `mask` selects.
fn masked_mean_luma(img: &CaptureImage, mask: &[bool]) -> (f32, usize) {
    let mut sum = 0.0f64;
    let mut n = 0usize;
    for (px, &m) in img.rgba.chunks_exact(4).zip(mask.iter()) {
        if m {
            sum += luma(px) as f64;
            n += 1;
        }
    }
    ((sum / n.max(1) as f64) as f32, n)
}

/// Pixels of `img` whose luminance satisfies `pred`.
fn mask_where(img: &CaptureImage, pred: impl Fn(f32) -> bool) -> Vec<bool> {
    img.rgba.chunks_exact(4).map(|px| pred(luma(px))).collect()
}

#[test]
fn ink_remap_inverts_tone_and_is_passthrough_when_off() {
    let Some(mut renderer) = headless() else {
        return;
    };
    let frame = AnalysisFrame {
        bass: 0.6,
        mid: 0.5,
        treb: 0.6,
        onset: 0.4,
        bar: 0.25,
        ..Default::default()
    };
    let mut capture = |extra: &str| {
        renderer.set_presets(vec![fixture_with(extra)]);
        renderer
            .capture_preset(FIXTURE_NAME, &frame, FRAMES)
            .unwrap_or_else(|e| panic!("capture ink fixture with `{extra}`: {e}"))
    };

    let unbound = capture("");
    let ink_off = capture("ink_amount = \"0\"\n");
    let ink_on = capture("ink_amount = \"1\"\n");

    // --- Passthrough: `ink_amount = 0` builds nothing and touches nothing, so it
    // must be byte-identical to the same fixture with no `ink_*` binding at all.
    // This is the claim every "shipped presets are unchanged" statement in
    // ADR-0028 rests on. ---
    assert_eq!(
        unbound.rgba, ink_off.rgba,
        "ink_amount = 0 is not passthrough: it differs from the unbound fixture"
    );

    // --- Inversion: the fixture is a sparse bright rose on a dark base, so the
    // remap turns most of the frame into paper and the strokes into ink. Assert
    // the mean crosses from dark to light rather than a tuned constant. ---
    let off_mean = mean_luma(&ink_off);
    let on_mean = mean_luma(&ink_on);
    println!("mean luma: ink off {off_mean:.1}, ink on {on_mean:.1} (0..255)");
    assert!(
        off_mean < 96.0,
        "the fixture is not a mostly-dark frame (mean luma {off_mean:.1}) — the \
         inversion assertion below would be vacuous"
    );
    assert!(
        on_mean > 160.0,
        "ink_amount = 1 did not repaint the dark base as paper: mean luma \
         {on_mean:.1} (was {off_mean:.1} with the stage off)"
    );

    // --- ...and it is a *tone remap*, not a global brighten. Band the ink-off
    // frame by luminance and look at what each band became: the ordering must
    // reverse, monotonically, across the whole ramp. That is the ADR-0028 property
    // itself ("density keys the paper->ink mix") rather than a tuned constant, so
    // it survives a change to the poles or the density curve. Masks come from the
    // ink-off frame, so they name the same pixels in both captures. ---
    let total = ink_off.rgba.len() / 4;
    let bands = [
        ("void   [  0, 16)", 0.0, 16.0),
        ("dim    [ 64,128)", 64.0, 128.0),
        ("bright [200,255]", 200.0, 256.0),
    ];
    let mut on_means = Vec::new();
    for (label, lo, hi) in bands {
        let mask = mask_where(&ink_off, |l| l >= lo && l < hi);
        let (on, n) = masked_mean_luma(&ink_on, &mask);
        let (off, _) = masked_mean_luma(&ink_off, &mask);
        println!("{label}: {n:>5}/{total} px, {off:.1} -> {on:.1} under ink");
        assert!(
            n * 50 > total,
            "band {label} holds only {n}/{total} px — too few to test an inversion"
        );
        on_means.push(on);
    }

    // Monotone: a brighter input band must come back darker, at every step. The
    // middle step is gentle because the captured frame is sRGB-encoded while the
    // shader keys on the linear value, so a mid-grey capture is still a *low*
    // density — the ordering is the claim, the spacing is not.
    for pair in on_means.windows(2) {
        assert!(
            pair[0] > pair[1] + 8.0,
            "tone did not invert across the ramp: {on_means:?} should be strictly \
             decreasing (a brighter input band must come back darker)"
        );
    }
    // ...and the ends cross, decisively: the darkest pixels end up lighter than
    // the brightest ones started, and the brightest end up darker than the darkest
    // started. That is an inversion, not a brighten with a soft top end.
    let (void_on, bright_on) = (on_means[0], on_means[2]);
    assert!(
        void_on > 200.0,
        "the frame's darkest pixels did not become paper: mean luma {void_on:.1}"
    );
    assert!(
        bright_on < 160.0 && void_on - bright_on > 96.0,
        "the frame's brightest pixels did not become ink: {bright_on:.1} against \
         {void_on:.1} for the pixels that were darkest"
    );
}
