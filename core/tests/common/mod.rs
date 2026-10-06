//! The helpers every GPU integration test in `core/tests/` opens with.
//!
//! `core/tests/common/` is a directory, not a top-level `.rs`, so cargo does not
//! compile it as its own test binary; each top-level test file pulls it in with
//! `mod common;`, `tests/suite/main.rs` with a `#[path]` to this file, and it is
//! rebuilt into each of those binaries. Nothing here is public API
//! — `rlx-core` must not grow a test-support surface to serve its own tests.
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

use rlx_core::dsp::AnalysisFrame;
use rlx_core::render::{AdapterChoice, CaptureImage, HeadlessOptions, RenderError, Renderer, Tier};

/// The one place the ADR-0016 skip lives: build a renderer, or return `None`
/// after printing the notice when the runner has no GPU adapter at all. Any
/// other build error still panics loudly, so a genuinely broken device cannot
/// pass as an absent one.
///
/// `software` picks [`AdapterChoice::Software`]; otherwise the build asks for
/// [`AdapterChoice::HighPerformance`], the preference the live path resolves,
/// so on a hybrid machine a hardware test reads the discrete GPU rather than
/// whichever one wgpu's default hands a console process (ADR-0243). A hardware
/// build prints the adapter it resolved, name and driver, so every reading it
/// produces names its machine (ADR-0071). `tier` is `None` for
/// [`Tier::Floor`], which is what [`Renderer::new_headless`] pins.
fn build(width: u32, height: u32, software: bool, tier: Option<Tier>) -> Option<Renderer> {
    let choice = if software {
        AdapterChoice::Software
    } else {
        AdapterChoice::HighPerformance
    };
    let opts = HeadlessOptions {
        width,
        height,
        prefer_software: software,
    };
    match Renderer::new_headless_on(opts, tier.unwrap_or(Tier::Floor), &choice) {
        Ok(r) => {
            if !software {
                eprintln!("hardware adapter: {}", r.adapter_description());
            }
            Some(r)
        }
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
/// Software is the default because a guard whose failure mode is "nobody
/// looked" has to run in CI, and the software rasterizer is what makes a
/// capture reproducible across runners.
pub fn headless(width: u32, height: u32) -> Option<Renderer> {
    headless_on(width, height, true)
}

/// [`headless`] with the adapter preference spelled out, for the files that
/// capture the same fixture on both adapters. `false` is the high-performance
/// hardware adapter, as in [`headless_hardware_for`].
pub fn headless_on(width: u32, height: u32, prefer_software: bool) -> Option<Renderer> {
    build(width, height, prefer_software, None)
}

/// The default-adapter twin of [`headless`], for an assertion WARP cannot host.
///
/// A same-system pair duplicates pipelines with **byte-identical bind layouts**,
/// and WARP aliases those (ADR-0058): the second instance's uniform wins and the
/// first becomes a dead lever, on the software adapter only. So this skips with
/// notice on a software-only runner rather than asserting against an adapter
/// that mis-renders the shape under test. (A WARP reading, unverified on
/// lavapipe as of 2026-10-06; the skip still keys on any software adapter.)
pub fn headless_hardware(width: u32, height: u32) -> Option<Renderer> {
    headless_hardware_for(
        width,
        height,
        None,
        "measured on WARP, unverified on lavapipe as of 2026-10-06: WARP aliases the \
         identical pipeline layouts a same-system pair duplicates (ADR-0058)",
    )
}

/// The reason a **timing** test needs hardware: a software-rasterizer frame
/// time is a fact about a CPU rasterizer, so no threshold stated against the
/// shipped renderer can be checked with one (ADR-0071). Class-level: it holds
/// for WARP and lavapipe alike.
pub const NEEDS_HARDWARE_FOR_TIMING: &str =
    "a software frame time is not a reading about the shipped renderer (see module docs)";

/// A hardware-adapter build, skipping with the **caller's own** reason.
///
/// The ADR-0016 no-adapter skip lives in `build` and is shared; this adds the
/// second refusal, and the reason is a parameter because the files that need it
/// do not share one. Four distinct reasons are live: a frame time that would be
/// measuring the wrong machine (class-level, true of any software rasterizer),
/// WARP aliasing byte-identical bind layouts (ADR-0058), WARP mis-rendering a
/// fullscreen-scene and background together, and a defect that simply does not
/// reproduce on WARP. The last three are WARP readings, unverified on lavapipe
/// as of 2026-10-06, and each caller's reason says so. Collapsing them onto one
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
    match build(width, height, false, tier) {
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
    build(width, height, true, Some(tier))
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

/// Whether `renderer` sits on the adapter every PNG under `tests/golden/` was
/// blessed on — `Ok(())` — or on another one, named in the `Err`.
///
/// The baselines are a **measurement taken on lavapipe**, Mesa's software
/// Vulkan rasterizer on the reference machine (ADR-0242), not a property of a
/// correct rasterizer: their tolerance absorbs lavapipe's own run-to-run drift,
/// and another rasterizer drifts further than that on the stateful and chaotic
/// fixtures without anything being wrong (ADR-0071, ADR-0131). So a comparison
/// asserts only on lavapipe, and elsewhere — DX12 WARP, any hardware adapter —
/// reports what it read through [`skip_off_baseline`]. macOS has no software
/// Metal adapter and never reaches this.
///
/// "Software on Linux" is lavapipe: Vulkan is the only wgpu backend
/// `core/Cargo.toml` compiles for Linux, and lavapipe is the software Vulkan
/// driver Mesa ships. The name check holds the equivalence against another
/// software Vulkan driver installed beside it; lavapipe reports its device as
/// `llvmpipe (LLVM …)`. Enabling a second Linux backend would let GL's llvmpipe
/// match the name too, so that change revisits this predicate.
pub fn baseline_adapter(renderer: &Renderer) -> Result<(), String> {
    let description = renderer.adapter_description();
    if cfg!(target_os = "linux")
        && renderer.adapter_is_software()
        && description.starts_with("llvmpipe")
    {
        Ok(())
    } else {
        Err(description.to_owned())
    }
}

/// Which baselines a set `RLX_BLESS` names.
#[derive(Debug, PartialEq, Eq)]
pub enum BlessList {
    /// `RLX_BLESS=1`: every baseline the run reaches.
    All,
    /// Any other value: exactly these stems, the file names under
    /// `tests/golden/` without `.png`.
    Stems(Vec<String>),
}

impl BlessList {
    /// Whether the baseline `stem` is one this list blesses.
    pub fn covers(&self, stem: &str) -> bool {
        match self {
            BlessList::All => true,
            BlessList::Stems(stems) => stems.iter().any(|s| s == stem),
        }
    }
}

/// Parse a set `RLX_BLESS` value. `1` (whitespace aside) is [`BlessList::All`];
/// anything else is a comma-separated list of stems, each trimmed, empty
/// entries dropped. A value naming no stem at all is an `Err`, not an empty
/// list: an empty list would bless nothing and look like a bless that ran.
pub fn parse_bless(value: &str) -> Result<BlessList, String> {
    let value = value.trim();
    if value == "1" {
        return Ok(BlessList::All);
    }
    let stems: Vec<String> = value
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect();
    if stems.is_empty() {
        return Err(format!(
            "RLX_BLESS={value:?} names no baseline: set it to `1` for every baseline \
             or to a comma-separated list of stems"
        ));
    }
    Ok(BlessList::Stems(stems))
}

/// Whether `RLX_BLESS` asks this run to rewrite the baseline `stem` —
/// **panicking** if it is set at all and `renderer` is not on the blessing
/// adapter, since a baseline written anywhere else would hold every later
/// lavapipe run to another rasterizer's frame.
///
/// A list naming a stem with no `tests/golden/<stem>.png` also panics, with
/// every such name. The check reads the directory rather than recording which
/// tests ran, so it holds whatever the run's filter selected and however
/// nextest splits the run into processes. The cost: a baseline that does not
/// exist yet cannot be named, and is first blessed with `RLX_BLESS=1` under a
/// test filter that reaches only it.
pub fn bless_requested(renderer: &Renderer, stem: &str) -> bool {
    let Some(value) = std::env::var_os("RLX_BLESS") else {
        return false;
    };
    if let Err(adapter) = baseline_adapter(renderer) {
        panic!(
            "RLX_BLESS refused: baselines are blessed on lavapipe only, and this \
             run is on {adapter}"
        );
    }
    let value = value
        .into_string()
        .unwrap_or_else(|v| panic!("RLX_BLESS is not UTF-8: {v:?}"));
    let list = parse_bless(&value).unwrap_or_else(|e| panic!("{e}"));
    if let BlessList::Stems(stems) = &list {
        let unknown: Vec<&String> = stems
            .iter()
            .filter(|s| !golden_dir().join(format!("{s}.png")).is_file())
            .collect();
        assert!(
            unknown.is_empty(),
            "RLX_BLESS names baselines with no PNG under {}: {unknown:?}",
            golden_dir().display()
        );
    }
    list.covers(stem)
}

/// The skip notice a baseline comparison prints off lavapipe, in ADR-0016's
/// shape. `drifted` lists what would have failed on lavapipe — printed, never
/// asserted — so the reading is kept rather than discarded.
pub fn skip_off_baseline(adapter: &str, drifted: &[String]) {
    eprintln!(
        "skipped: the baselines are a measurement taken on lavapipe, and this run \
         is on {adapter} (ADR-0071, ADR-0242). {} comparison(s) past lavapipe's \
         tolerance here: {drifted:#?}",
        drifted.len()
    );
}

/// The committed baseline directory, `core/tests/golden/`.
pub fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
}

/// Write a capture out as the baseline PNG at `path`. A set `RLX_BLESS` naming
/// the baseline is what reaches this.
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
