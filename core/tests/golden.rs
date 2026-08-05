//! Golden-image drift (Plan 0013 Phase 4; repointed by Plan 0022, ADR-0023).
//! This guard defends **engine rendering determinism**, not shipped content: it
//! renders one **frozen per-system fixture** headless on the software adapter
//! and compares each frame against a committed baseline PNG within a mean +
//! max-outlier tolerance. `LMV_BLESS=1` rewrites the baselines.
//!
//! The fixtures live as do-not-tune TOML under `tests/fixtures/`, one per
//! [`SystemKind`], selected by an **exhaustive** match (see [`fixture`]) so a new
//! scene cannot ship without a drift baseline. They are deliberately *not* the
//! shipped presets: the `preset-author` lane (ADR-0017) tunes those, and an
//! intended content tune must never trip this engine-drift alarm. Shipped
//! presets keep their own guards behaviorally — `sanity` (coverage/spread),
//! `reactivity` (per-band reaction), and `animation` (motion) each iterate
//! `default_presets()`, and those floors survive content tuning by design.
//!
//! The tolerance absorbs cross-GPU rasterization drift (the software adapter
//! keeps it small); a genuine engine change — a perturbed shader or scene math —
//! moves a frame well past it. Baselines are ordinary PNGs, viewable in the repo
//! and PR diffs; they are WARP-only (macOS has no software Metal fallback, so
//! the test skips there per ADR-0016) and must be blessed on WARP. Eyeball each
//! before blessing (Plan 0013 Phase 8 habit).

use std::path::{Path, PathBuf};

use lmv_core::dsp::AnalysisFrame;
use lmv_core::preset::{Preset, SystemKind};
use lmv_core::render::{CaptureImage, HeadlessOptions, RenderError, Renderer, metrics::frame_diff};

const SIZE: u32 = 128;
/// Frames warmed before capture — enough for the stateful systems (swarm sim,
/// reaction-diffusion field) to evolve a non-trivial pattern.
const FRAMES: u32 = 60;
/// Mean per-channel difference (0..1) a fresh render may drift from baseline.
const MEAN_TOL: f32 = 0.02;
/// Largest single-channel byte difference tolerated at any pixel — a localized
/// change a low mean would otherwise hide.
const MAX_OUTLIER: u8 = 48;

/// The frozen fixture for a system: its baseline file stem (the system name) and
/// the fixture TOML compiled into the test binary.
///
/// This is an **exhaustive** `match` with no wildcard arm — adding a
/// `SystemKind` variant fails to compile here until a fixture is authored under
/// `tests/fixtures/`, so no scene ships without a drift baseline (ADR-0023).
fn fixture(system: SystemKind) -> (&'static str, &'static str) {
    match system {
        SystemKind::FragmentField => (
            "fragment_field",
            include_str!("fixtures/fragment_field.toml"),
        ),
        SystemKind::Swarm => ("swarm", include_str!("fixtures/swarm.toml")),
        SystemKind::ParametricCurve => (
            "parametric_curve",
            include_str!("fixtures/parametric_curve.toml"),
        ),
        SystemKind::LSystem => ("lsystem", include_str!("fixtures/lsystem.toml")),
        SystemKind::StarPattern => ("star_pattern", include_str!("fixtures/star_pattern.toml")),
        SystemKind::ReactionDiffusion => (
            "reaction_diffusion",
            include_str!("fixtures/reaction_diffusion.toml"),
        ),
        SystemKind::Attractor => ("attractor", include_str!("fixtures/attractor.toml")),
        SystemKind::Spectrum => ("spectrum", include_str!("fixtures/spectrum.toml")),
        SystemKind::Emitter => ("emitter", include_str!("fixtures/emitter.toml")),
    }
}

/// Fixtures that are **not** one per [`SystemKind`] and still pin a baseline
/// here (Plan 0063 Phase 4).
///
/// The roster above is exhaustive over the enum and must stay that way —
/// ADR-0023 rests on it, and [`systems_rosters_every_variant`] enforces it. This
/// is the narrow escape hatch for a **second** fixture of a system already in
/// the roster, for when the rostered one structurally cannot reach the code
/// under test.
///
/// `attractor_depth` is the case that opened it, and the impossibility is by
/// design rather than by accident: the roster's `attractor.toml` is **De Jong**,
/// and ADR-0076 gives every 2-D family an inverse depth extent of exactly `0.0`
/// — which is precisely what makes the perspective divide, the distance haze and
/// the depth tint the identity there. No edit to that fixture could exercise a
/// line of them. A 3-D family with all four levers off their defaults is the
/// only thing in this suite that would catch a regression in them.
///
/// `attractor_ifs` is the same shape of argument for the same scene (Plan 0062).
/// De Jong takes a different arm of the step shader, its `a`..`d` coefficients
/// *are* its shape, and `morph`/`curl`/`vigor`/`lean`/`bias` are inert on it —
/// so nothing that could be done to `attractor.toml` would exercise the SVD
/// decomposition, the morph, the framing fit or the levers.
///
/// **Captured after the roster loop, and appended rather than inserted.** Every
/// pre-existing baseline is therefore rendered from the device state it always
/// was, so adding an entry here moves none of them — which matters on WARP,
/// where building GPU resources mid-run is documented to change what a later
/// capture resolves to. For the same reason a new entry goes at the **end**.
const EXTRA_FIXTURES: [(&str, &str); 2] = [
    (
        "attractor_depth",
        include_str!("fixtures/attractor_depth.toml"),
    ),
    ("attractor_ifs", include_str!("fixtures/attractor_ifs.toml")),
];

fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
}

/// Build a headless `Renderer`, or `None` (a logged skip) when the runner
/// exposes no GPU adapter — macOS has no software Metal fallback (ADR-0016).
/// Any other build error still panics loudly.
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

/// The fixed frame every baseline is rendered under — a representative
/// mid-energy frame with all bands lit, so a band-reactive fixture still draws.
///
/// The `spectrum` array carries a plausible falling profile rather than the
/// zeros a bare `Default` would give (Plan 0034 Phase 2). A frame claiming
/// `bass = 0.6` with 64 silent bands is not a frame any audio could produce, and
/// under it a spectrum fixture would pin a baseline of nothing. No pre-0034
/// scene reads `spectrum`, so filling it leaves every other baseline unchanged.
fn fixed_frame() -> AnalysisFrame {
    let mut frame = AnalysisFrame {
        bass: 0.6,
        mid: 0.5,
        treb: 0.6,
        onset: 0.4,
        bar: 0.25,
        ..Default::default()
    };
    let bands = frame.spectrum.len() as f32;
    for (i, band) in frame.spectrum.iter_mut().enumerate() {
        // Loud at the bottom, quiet at the top, with a slow ripple so adjacent
        // elements differ — a flat ramp would let a transposed mapping pass.
        let t = i as f32 / bands;
        *band = (0.9 - 0.7 * t) * (0.75 + 0.25 * (t * 17.0).sin());
    }
    frame
}

fn decode(path: &Path) -> CaptureImage {
    let img = image::open(path)
        .unwrap_or_else(|e| panic!("decode baseline {}: {e}", path.display()))
        .to_rgba8();
    CaptureImage {
        width: img.width(),
        height: img.height(),
        rgba: img.into_raw(),
    }
}

fn encode(img: &CaptureImage, path: &Path) {
    let buffer = image::RgbaImage::from_raw(img.width, img.height, img.rgba.clone())
        .expect("capture buffer matches its declared dimensions");
    buffer
        .save(path)
        .unwrap_or_else(|e| panic!("write baseline {}: {e}", path.display()));
}

/// Largest absolute single-channel (RGB) byte difference across the two images.
fn max_channel_outlier(a: &CaptureImage, b: &CaptureImage) -> u8 {
    a.rgba
        .chunks_exact(4)
        .zip(b.rgba.chunks_exact(4))
        .flat_map(|(pa, pb)| {
            pa.iter()
                .zip(pb.iter())
                .take(3)
                .map(|(x, y)| x.abs_diff(*y))
        })
        .max()
        .unwrap_or(0)
}

#[test]
fn scenes_match_golden_baselines() {
    let Some(mut renderer) = headless() else {
        return;
    };
    let frame = fixed_frame();
    let bless = std::env::var_os("LMV_BLESS").is_some();
    std::fs::create_dir_all(golden_dir()).expect("create tests/golden");

    let mut failures = Vec::new();
    let mut check = |renderer: &mut Renderer, stem: &str, toml: &str| {
        let preset = Preset::from_toml_str(toml)
            .unwrap_or_else(|e| panic!("golden fixture {stem}.toml is invalid: {e}"));
        let name = preset.name.clone();
        renderer.set_presets(vec![preset]);

        let fresh = renderer
            .capture_preset(&name, &frame, FRAMES)
            .expect("capture fixture");
        let path = golden_dir().join(format!("{stem}.png"));

        if bless {
            encode(&fresh, &path);
            println!("blessed {}", path.display());
            return;
        }

        assert!(
            path.exists(),
            "missing baseline {} — run `LMV_BLESS=1 cargo test -p lmv-core --test golden`",
            path.display()
        );
        let baseline = decode(&path);
        let mean = frame_diff(&baseline, &fresh);
        let outlier = max_channel_outlier(&baseline, &fresh);
        println!(
            "{stem:<18} mean {mean:.4} (tol {MEAN_TOL}) max_outlier {outlier} (tol {MAX_OUTLIER})"
        );
        if mean > MEAN_TOL || outlier > MAX_OUTLIER {
            failures.push(format!(
                "{stem}: mean {mean:.4} / outlier {outlier} exceeds tolerance"
            ));
        }
    };

    for system in SystemKind::ALL {
        let (stem, toml) = fixture(system);
        check(&mut renderer, stem, toml);
    }
    // After the roster, never interleaved with it — see [`EXTRA_FIXTURES`].
    for (stem, toml) in EXTRA_FIXTURES {
        check(&mut renderer, stem, toml);
    }

    assert!(
        failures.is_empty(),
        "golden drift beyond tolerance (bless with LMV_BLESS=1 if intended): {failures:#?}"
    );
}

/// Structural coverage guard (Plan 0016 Phase 5, closing the Plan 0022 followup;
/// repointed onto [`SystemKind::ALL`] by Plan 0030 Phase 3, which retired this
/// file's duplicate `SYSTEMS` list so the variant roster lives in exactly one
/// place). A variant added to the exhaustive [`fixture`] match but forgotten in
/// the roster would render zero baselines and pass
/// `scenes_match_golden_baselines` silently.
///
/// Assert the roster holds distinct systems, each with a valid, distinctly-named
/// fixture — so a variant reaching `fixture()` without being rostered, or two
/// systems sharing a baseline file, fails the suite. (Its *length* is enforced at
/// compile time now: `ALL` is typed `[SystemKind; VARIANT_COUNT]`.) No GPU, so it
/// runs everywhere (not skipped on an adapterless runner).
#[test]
fn systems_rosters_every_variant() {
    let mut seen: Vec<SystemKind> = Vec::new();
    let mut stems: Vec<&str> = Vec::new();
    for system in SystemKind::ALL {
        assert!(
            !seen.contains(&system),
            "duplicate entry in SystemKind::ALL"
        );
        seen.push(system);
        let (stem, toml) = fixture(system);
        assert!(
            !stems.contains(&stem),
            "duplicate fixture stem {stem} in SystemKind::ALL"
        );
        stems.push(stem);
        Preset::from_toml_str(toml)
            .unwrap_or_else(|e| panic!("golden fixture {stem}.toml is invalid: {e}"));
    }
    assert_eq!(
        seen.len(),
        SystemKind::VARIANT_COUNT,
        "every SystemKind variant must carry a drift fixture"
    );

    // The off-roster fixtures are held to the same two conditions, and to one
    // more that only applies to them: a stem colliding with a rostered system's
    // would have the two silently overwrite each other's baseline.
    for (stem, toml) in EXTRA_FIXTURES {
        assert!(
            !stems.contains(&stem),
            "extra fixture {stem} collides with a rostered system's baseline file"
        );
        stems.push(stem);
        Preset::from_toml_str(toml)
            .unwrap_or_else(|e| panic!("extra fixture {stem}.toml is invalid: {e}"));
    }
}
