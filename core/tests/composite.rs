//! Post-composite pixel guard (Plan 0035 Phase 2, ADR-0037).
//!
//! **The composite's two stages had no capture-level coverage of any kind.** No
//! fixture in `tests/fixtures/` bound `trails` or `kaleido_*`, so the whole
//! stage-active path — the routing, the internal grid, the aspect every scene is
//! handed through it — was exercised by no capture in the suite. That is how a
//! defect that stretched the entire frame by 28 % at 1280x800 shipped green
//! (ADR-0037), and it is the gap Plan 0033's close review filed as major 3.
//!
//! This is the same shape as `golden.rs`: a frozen fixture, rendered headless,
//! compared against a committed baseline PNG within a mean + max-outlier
//! tolerance. It is a **separate test binary** rather than two more arms of
//! `golden.rs`, following `background_composite.rs`'s posture: one file, one
//! process, so the feedback/fold pipelines these fixtures build never coexist on
//! a device with the per-system golden captures. Building GPU resources mid-run
//! is documented to change what the trails stage resolves to on the software
//! adapter, and the seven existing baselines must not be exposed to that.
//!
//! # The capture size is load-bearing
//!
//! **160x100, and not a square.** The post stages round each grid axis up to a
//! 256 px step, so 160x100 takes a **256x256** grid: aspect 1.0 against the
//! target's 1.6. Composing a stage therefore stretched the picture by **1.6x**
//! before Phase 1 — stronger than the 1.28x at 1280x800 that the plan calls the
//! worst ordinary case, at a sixty-fourth of the pixels.
//!
//! A square or 16:9 capture size would defeat this guard entirely: the policy
//! returns those aspect-exact, which is precisely why the defect was invisible at
//! the 1920x1080 and 2048x1152 this project is developed on. **Changing `WIDTH` /
//! `HEIGHT` without redoing that arithmetic silently retires the test.**
//!
//! # `composite_kaleido.png` pins design-backlog 0010's **fix**
//!
//! It used to pin the defect: the fold sampled outside its rectangular source and
//! the `ClampToEdge` sampler smeared the border texel radially, so the corners
//! carried hard-edged streaks. Plan 0045 Phase 1 / ADR-0047 clamped the sample
//! radius to the largest disc the source contains and faded past it, and this
//! baseline was re-blessed by hand at that change — see the fixture's header for
//! what moved and by how much.
//!
//! # `composite_overlap.png` pins the composite's arithmetic, not its routing
//!
//! Plan 0045 Phase 3 made every intermediate linear-light `Rgba16Float` and put a
//! tonemap at the surface boundary. The third fixture binds **no** post stage: it
//! draws a dense additive rose whose self-crossings sum past 1.0, which the 8-bit
//! chain clipped to flat white. Its guard is that no channel in *that frame*
//! reaches 255.
//!
//! **That is a claim about this fixture, not a property of the curve, and the
//! difference matters** (Plan 0045 Phase 4b). `f(x) < 1` for every finite `x` is
//! true, and it does *not* make a 255 byte unreachable: the surface write encodes
//! to sRGB and rounds, so `f(x)` crosses the last byte's midpoint at a linear
//! input of about **36** at `KNEE = 0.6` — and `attractor.toml` reaches it on the
//! hardware adapter. What this rose demonstrates is that its crossings now roll
//! off instead of flattening; generalizing the assertion into a suite-wide
//! no-255 gate would fail on frames that are behaving correctly.

use std::path::{Path, PathBuf};

use lmv_core::dsp::AnalysisFrame;
use lmv_core::preset::Preset;
use lmv_core::render::{CaptureImage, HeadlessOptions, RenderError, Renderer, metrics::frame_diff};

/// Capture width/height — see the module docs. **Not interchangeable.**
const WIDTH: u32 = 160;
const HEIGHT: u32 = 100;
/// Frames warmed before capture — enough for the trails accumulation to hold a
/// real tail behind the turning figure rather than one frame's worth.
const FRAMES: u32 = 40;
/// Mean per-channel difference (0..1) a fresh render may drift from baseline.
/// Same tolerance as `golden.rs`.
const MEAN_TOL: f32 = 0.02;
/// Largest single-channel byte difference tolerated at any pixel.
const MAX_OUTLIER: u8 = 48;

/// The frozen fixtures: baseline file stem, and the TOML compiled in.
///
/// One per stage — `trails` is the stage with cross-frame state, `kaleido_*` the
/// one that computes geometry, `bloom_*` (Plan 0045 Phase 4) the one with an
/// internal pyramid — plus, since Plan 0045 Phase 3, one that binds no stage at
/// all and exercises the composite's **arithmetic** instead: a dense additive rose
/// whose self-crossings used to clip to flat white on the 8-bit intermediates and
/// now roll off through the tonemap.
///
/// **One stage per fixture, never all of them at once.** Bloom adds four
/// pipelines, and the WARP software adapter's sensitivity to coexisting pipelines
/// is the reason this file exists in the shape it does; a mega-composite fixture
/// would put every stage's pipelines on the device at once and make any
/// mis-render impossible to attribute.
///
/// `composite_kaleido_squash` (Plan 0055 Phase 4) is the one pair here that
/// shares a stage rather than owning one. That does not break the rule above —
/// both bind the kaleidoscope and nothing else, so neither puts a pipeline on the
/// device that the other does not. What the two separate is the fold's
/// **geometry** from its **edge treatment**, which ADR-0061 made a per-preset
/// choice: `composite_kaleido` is a centred figure over an empty border, so it
/// renders identically under every treatment (measured at Phase 3 — its
/// `kaleido_edge` pinned to 0 and to 1 gives md5-identical PNGs) and therefore
/// cannot pin the edge at all. Its sibling's own header carries the rest.
const FIXTURES: [(&str, &str); 5] = [
    (
        "composite_trails",
        include_str!("fixtures/composite_trails.toml"),
    ),
    (
        "composite_kaleido",
        include_str!("fixtures/composite_kaleido.toml"),
    ),
    (
        "composite_kaleido_squash",
        include_str!("fixtures/composite_kaleido_squash.toml"),
    ),
    (
        "composite_overlap",
        include_str!("fixtures/composite_overlap.toml"),
    ),
    (
        "composite_bloom",
        include_str!("fixtures/composite_bloom.toml"),
    ),
];

/// The fixture whose whole point is that it no longer clips (Plan 0045 Phase 3).
const OVERLAP: &str = "composite_overlap";

fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
}

/// A headless renderer on the **software** adapter, or `None` (a logged skip)
/// when the runner exposes no GPU adapter — macOS has no software Metal fallback
/// (ADR-0016).
///
/// WARP, deliberately, and the question was settled by measurement rather than by
/// assumption: these captures were confirmed byte-identical across three
/// consecutive runs before the baselines landed. The alternative posture —
/// `background_composite.rs`'s request-the-default-adapter-and-skip-on-software —
/// would have made this guard run on developer machines and **not in CI**, which
/// for a defect whose entire failure mode is "nobody looked" is the wrong trade.
fn headless() -> Option<Renderer> {
    match Renderer::new_headless(HeadlessOptions {
        width: WIDTH,
        height: HEIGHT,
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

/// The fixed frame every baseline is rendered under — mid-energy, all bands lit.
fn fixed_frame() -> AnalysisFrame {
    AnalysisFrame {
        bass: 0.6,
        mid: 0.5,
        treb: 0.6,
        onset: 0.4,
        bar: 0.25,
        ..Default::default()
    }
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

/// Both post stages, composited and pinned to a baseline.
///
/// `LMV_BLESS=1 cargo test -p lmv-core --test composite` rewrites these two —
/// and, run against the whole suite instead of this one binary, **every other
/// baseline as well**. Bless by `--test composite` and check `git status`.
#[test]
fn composite_stages_match_golden_baselines() {
    let Some(mut renderer) = headless() else {
        return;
    };
    let frame = fixed_frame();
    let bless = std::env::var_os("LMV_BLESS").is_some();
    std::fs::create_dir_all(golden_dir()).expect("create tests/golden");

    let mut failures = Vec::new();
    for (stem, toml) in FIXTURES {
        let preset = Preset::from_toml_str(toml)
            .unwrap_or_else(|e| panic!("composite fixture {stem}.toml is invalid: {e}"));
        let name = preset.name.clone();
        renderer.set_presets(vec![preset]);

        let fresh = renderer
            .capture_preset(&name, &frame, FRAMES)
            .expect("capture fixture");
        let path = golden_dir().join(format!("{stem}.png"));

        // Checked ahead of the bless branch on purpose: a `LMV_BLESS` run must
        // not be able to write a clipped baseline and call it the new truth.
        if stem == OVERLAP {
            let clipped = fresh
                .rgba
                .chunks_exact(4)
                .flat_map(|px| px.iter().take(3))
                .filter(|&&c| c == u8::MAX)
                .count();
            assert_eq!(
                clipped, 0,
                "{clipped} channels of {stem} reached 255. This fixture's \
                 crossings peak far below the linear ~36 it takes to round to 255 \
                 through the tonemap, so they must roll off rather than flatten to \
                 white — they clipped across a large region before Plan 0045 Phase \
                 3. Note this is a fact about THIS fixture: bounded below 1 does \
                 not make a 255 byte unreachable, so do not lift the check to the \
                 whole suite (see the module docs)"
            );
        }

        if bless {
            encode(&fresh, &path);
            println!("blessed {}", path.display());
            continue;
        }

        assert!(
            path.exists(),
            "missing baseline {} — run `LMV_BLESS=1 cargo test -p lmv-core --test composite`",
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
    }

    assert!(
        failures.is_empty(),
        "composite drift beyond tolerance — a stage's routing, grid, or the aspect it \
         hands the scene has changed (ADR-0037). Bless with LMV_BLESS=1 only if intended: \
         {failures:#?}"
    );
}
