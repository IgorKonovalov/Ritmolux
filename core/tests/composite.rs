//! Post-composite pixel guard (Plan 0035 Phase 2, ADR-0037).
//!
//! **This file is the composite's only capture-level coverage.**
//! Without a fixture binding `trails` or `kaleido_*` the whole
//! stage-active path — the routing, the internal grid, the aspect every
//! scene is handed through it — is exercised by no capture in the
//! suite, which is how a defect that stretched the entire frame by 28 %
//! at 1280x800 shipped green (ADR-0037).
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
//! target's 1.6. A stage that took its aspect from that grid would therefore
//! stretch the picture by **1.6x** here — stronger than the 1.28x at 1280x800
//! that is the worst ordinary case, at a sixty-fourth of the pixels.
//!
//! A square or 16:9 capture size would defeat this guard entirely: the policy
//! returns those aspect-exact, which is precisely why the defect was invisible at
//! the 1920x1080 and 2048x1152 this project is developed on. **Changing `WIDTH` /
//! `HEIGHT` without redoing that arithmetic silently retires the test.**
//!
//! # `composite_kaleido.png` pins design-backlog 0010's **fix**
//!
//! ADR-0047 clamps the fold's sample radius to the largest disc the
//! source contains and fades past it, rather than letting a
//! `ClampToEdge` sampler smear the border texel radially into
//! hard-edged corner streaks. The fixture's own header carries what
//! moved at that re-bless and by how much.
//!
//! # `composite_overlap.png` pins the composite's arithmetic, not its routing
//!
//! The third fixture binds **no** post stage: it draws a dense additive
//! rose whose self-crossings sum past 1.0, which an 8-bit chain clips
//! to flat white. Its guard is that no channel in *that frame* reaches
//! 255.
//!
//! **That is a claim about this fixture, not a property of the curve, and the
//! difference matters** (Plan 0045 Phase 4b). `f(x) < 1` for every finite `x` is
//! true, and it does *not* make a 255 byte unreachable: the surface write encodes
//! to sRGB and rounds, so `f(x)` crosses the last byte's midpoint at a linear
//! input of about **36** at `KNEE = 0.6` — and `attractor.toml` reaches it on the
//! hardware adapter. What this rose demonstrates is that its crossings now roll
//! off instead of flattening; generalizing the assertion into a suite-wide
//! no-255 gate would fail on frames that are behaving correctly.

use rlx_core::preset::Preset;
use rlx_core::render::{CaptureImage, metrics::frame_diff};

mod common;

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
/// internal pyramid — plus one that binds no stage at all (Plan 0045 Phase 3)
/// and exercises the composite's **arithmetic** instead: a dense additive rose
/// whose self-crossings would clip to flat white on 8-bit intermediates and roll
/// off through the tonemap here.
///
/// **One stage per fixture, never all of them at once.** Bloom adds four
/// pipelines, and the WARP software adapter's sensitivity to coexisting pipelines
/// is the reason this file exists in the shape it does; a mega-composite fixture
/// would put every stage's pipelines on the device at once and make any
/// mis-render impossible to attribute.
///
/// Four groups here **share** a stage rather than owning one. That does
/// not break the rule above: each group binds one stage and nothing
/// else, so no member puts a pipeline on the device its siblings do
/// not. Each names what its sibling structurally cannot pin, and its
/// own fixture header carries the rest:
///
/// - `composite_kaleido_squash` — the fold's **edge treatment**, which ADR-0061
///   made a per-preset choice. `composite_kaleido` is a centred figure over an
///   empty border, so it renders identically under every treatment (measured:
///   `kaleido_edge` pinned to 0 and to 1 gives md5-identical PNGs).
/// - `composite_bloom_exposed` — the bright-pass's **units** (ADR-0080).
///   `composite_bloom` leaves `exposure` unbound, and at the neutral stop the
///   multiply is the identity, which is what makes it byte-identical across that
///   change and blind to it.
/// - `composite_symmetry` — the whole **radial group** (ADR-0077 + ADR-0078).
///   Every other baseline leaves it at its identity, so the log wrap, the
///   spiral's branch-cut closure, the inner freeze and the zoom's scaling by the
///   period could all break with the suite green.
/// - `composite_warp_swirl` / `_ripple` / `_fisheye` — each **warp kind**
///   (ADR-0048). They bind the trails stage with `composite_trails`'s parameters
///   param for param and add one structural key, `[feedback] warp`. Only the four
///   together are worth having: `composite_trails` selects no warp, and a baseline
///   cannot pin that the *others* differ — a shader taking one arm for every
///   selector would pin three identical pictures happily. That half is asserted in
///   `feedback.rs`'s `each_warp_kind_bends_the_past_its_own_way`; what lives here
///   is the ordinary drift guard on each one's pixels. The warp is a selector in
///   one shader rather than a shader per kind, so the quartet adds no pipeline.
///
/// **Appended, never inserted**, for the reason `golden.rs`'s `EXTRA_FIXTURES`
/// records: every pre-existing baseline is then rendered from the device state it
/// always was, which matters on WARP where building GPU resources mid-run changes
/// what a later capture resolves to.
const FIXTURES: [(&str, &str); 10] = [
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
    (
        "composite_bloom_exposed",
        include_str!("fixtures/composite_bloom_exposed.toml"),
    ),
    (
        "composite_symmetry",
        include_str!("fixtures/composite_symmetry.toml"),
    ),
    (
        "composite_warp_swirl",
        include_str!("fixtures/composite_warp_swirl.toml"),
    ),
    (
        "composite_warp_ripple",
        include_str!("fixtures/composite_warp_ripple.toml"),
    ),
    (
        "composite_warp_fisheye",
        include_str!("fixtures/composite_warp_fisheye.toml"),
    ),
];

/// The fixture whose whole point is that it does not clip (Plan 0045 Phase 3).
const OVERLAP: &str = "composite_overlap";

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
/// `LMV_BLESS=1 cargo test -p rlx-core --test composite` rewrites these two —
/// and, run against the whole suite instead of this one binary, **every other
/// baseline as well**. Bless by `--test composite` and check `git status`.
#[test]
fn composite_stages_match_golden_baselines() {
    let Some(mut renderer) = common::headless(WIDTH, HEIGHT) else {
        return;
    };
    let frame = common::fixed_frame();
    let bless = std::env::var_os("LMV_BLESS").is_some();
    std::fs::create_dir_all(common::golden_dir()).expect("create tests/golden");

    let mut failures = Vec::new();
    for (stem, toml) in FIXTURES {
        let preset = Preset::from_toml_str(toml)
            .unwrap_or_else(|e| panic!("composite fixture {stem}.toml is invalid: {e}"));
        let name = preset.name.clone();
        renderer.set_presets(vec![preset]);

        let fresh = renderer
            .capture_preset(&name, &frame, FRAMES)
            .expect("capture fixture");
        let path = common::golden_dir().join(format!("{stem}.png"));

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
            common::encode(&fresh, &path);
            println!("blessed {}", path.display());
            continue;
        }

        assert!(
            path.exists(),
            "missing baseline {} — run `LMV_BLESS=1 cargo test -p rlx-core --test composite`",
            path.display()
        );
        let baseline = common::decode(&path);
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
