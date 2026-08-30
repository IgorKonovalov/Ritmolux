//! The helpers every GPU integration test in `core/tests/` opens with.
//!
//! `core/tests/common/` is a directory, not a top-level `.rs`, so cargo does not
//! compile it as its own test binary; each test file pulls it in with
//! `mod common;` and it is rebuilt into that binary. Nothing here is public API
//! — `lmv-core` must not grow a test-support surface to serve its own tests.
//!
//! **The skip is the point.** A runner with no GPU adapter at all — macOS has no
//! software Metal fallback — must skip with a printed notice rather than fail,
//! and any *other* build error must still panic loudly. Collapsing the two into
//! one `ok()` would turn a broken device into a silent pass on every runner.
//! ADR-0016 is the decision; the shape below is the mechanism.
//!
//! Not every file uses every helper, so the module allows dead code: a `mod
//! common;` in a file that needs only `headless` would otherwise warn on all the
//! rest, and `-D warnings` would fail the build.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use lmv_core::dsp::AnalysisFrame;
use lmv_core::render::{CaptureImage, HeadlessOptions, RenderError, Renderer, Tier};

/// The one place the ADR-0016 skip lives: build a renderer, or return `None`
/// after printing the notice when the runner has no GPU adapter at all. Any
/// other build error still panics loudly, so a genuinely broken device cannot
/// pass as an absent one.
fn build(opts: HeadlessOptions, tier: Option<Tier>) -> Option<Renderer> {
    let built = match tier {
        Some(tier) => Renderer::new_headless_tiered(opts, tier),
        None => Renderer::new_headless(opts),
    };
    match built {
        Ok(r) => Some(r),
        Err(RenderError::RequestAdapter(_)) => {
            eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
            None
        }
        Err(e) => panic!("headless renderer build failed: {e}"),
    }
}

/// A headless renderer on the **software** adapter, or `None` (a logged skip)
/// when the runner exposes no GPU adapter at all (ADR-0016).
///
/// WARP is the default because a guard whose failure mode is "nobody looked"
/// has to run in CI, and the software rasterizer is what makes a capture
/// reproducible across runners.
pub fn headless(width: u32, height: u32) -> Option<Renderer> {
    headless_on(width, height, true)
}

/// [`headless`] with the adapter preference spelled out, for the files that
/// capture the same fixture on both adapters.
pub fn headless_on(width: u32, height: u32, prefer_software: bool) -> Option<Renderer> {
    build(
        HeadlessOptions {
            width,
            height,
            prefer_software,
        },
        None,
    )
}

/// The default-adapter twin of [`headless`], for an assertion WARP cannot host.
///
/// A same-system pair duplicates pipelines with **byte-identical bind layouts**,
/// and WARP aliases those (ADR-0058): the second instance's uniform wins and the
/// first becomes a dead lever, on the software adapter only. So this skips with
/// notice on a software-only runner rather than asserting against an adapter
/// that mis-renders the shape under test.
pub fn headless_hardware(width: u32, height: u32) -> Option<Renderer> {
    headless_hardware_for(
        width,
        height,
        None,
        "WARP aliases the identical pipeline layouts a same-system pair duplicates (ADR-0058)",
    )
}

/// The reason a **timing** test needs hardware: a WARP frame time is a fact
/// about a CPU rasterizer, so no threshold stated against the shipped renderer
/// can be checked with one (ADR-0071).
pub const NEEDS_HARDWARE_FOR_TIMING: &str =
    "a WARP frame time is not a reading about the shipped renderer (see module docs)";

/// A hardware-adapter build, skipping with the **caller's own** reason.
///
/// The ADR-0016 no-adapter skip lives in `build` and is shared; this adds the
/// second refusal, and the reason is a parameter because the files that need it
/// do not share one. Four distinct reasons are live: a frame time that would be
/// measuring the wrong machine, WARP aliasing byte-identical bind layouts
/// (ADR-0058), WARP mis-rendering a fullscreen-scene and background together,
/// and a defect that simply does not reproduce on WARP. Collapsing them onto one
/// notice would print an aliasing argument at a timing skip, which is how a skip
/// notice stops being evidence about anything.
///
/// `tier` is `None` for the engine's own choice, mirroring [`headless`].
pub fn headless_hardware_for(
    width: u32,
    height: u32,
    tier: Option<Tier>,
    reason: &str,
) -> Option<Renderer> {
    let built = build(
        HeadlessOptions {
            width,
            height,
            prefer_software: false,
        },
        tier,
    );
    match built {
        Some(r) if r.adapter_is_software() => {
            eprintln!("skipped: only a software rasterizer is available — {reason}");
            None
        }
        other => other,
    }
}

/// [`headless`] at an explicit quality [`Tier`], for the post stages whose
/// resources the tier sizes.
pub fn headless_tiered(width: u32, height: u32, tier: Tier) -> Option<Renderer> {
    build(
        HeadlessOptions {
            width,
            height,
            prefer_software: true,
        },
        Some(tier),
    )
}

/// The fixed frame a baseline is rendered under: mid-energy, all three scalars
/// lit, so a band-reactive fixture still draws something to compare.
///
/// `spectrum` is left at its `Default` zeros. Use [`fixed_frame_spectrum`] where
/// the fixture reads the per-band array.
pub fn fixed_frame() -> AnalysisFrame {
    AnalysisFrame {
        bass: 0.6,
        mid: 0.5,
        treb: 0.6,
        onset: 0.4,
        bar: 0.25,
        ..Default::default()
    }
}

/// [`fixed_frame`] with a plausible falling band profile written into
/// `spectrum`, for the fixtures that read it.
///
/// A frame claiming `bass = 0.6` with 64 silent bands is not a frame any audio
/// could produce, and under it a spectrum fixture pins a baseline of nothing.
/// The slow ripple over the ramp makes adjacent elements differ — a flat ramp
/// would let a transposed mapping pass.
pub fn fixed_frame_spectrum() -> AnalysisFrame {
    let mut frame = fixed_frame();
    let bands = frame.spectrum.len() as f32;
    for (i, band) in frame.spectrum.iter_mut().enumerate() {
        let t = i as f32 / bands;
        *band = (0.9 - 0.7 * t) * (0.75 + 0.25 * (t * 17.0).sin());
    }
    frame
}

/// The committed baseline directory, `core/tests/golden/`.
pub fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
}

/// Write a capture out as the baseline PNG at `path`. `LMV_BLESS=1` is what
/// reaches this.
pub fn encode(img: &CaptureImage, path: &Path) {
    let buffer = image::RgbaImage::from_raw(img.width, img.height, img.rgba.clone())
        .expect("capture buffer matches its declared dimensions");
    buffer
        .save(path)
        .unwrap_or_else(|e| panic!("write baseline {}: {e}", path.display()));
}

/// Read a committed baseline PNG back as a [`CaptureImage`] for comparison.
pub fn decode(path: &Path) -> CaptureImage {
    let img = image::open(path)
        .unwrap_or_else(|e| panic!("decode baseline {}: {e}", path.display()))
        .to_rgba8();
    CaptureImage {
        width: img.width(),
        height: img.height(),
        rgba: img.into_raw(),
    }
}
