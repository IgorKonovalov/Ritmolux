//! The converted `warp_mesh` chain pinned at a target that is **not square**
//! (Plan 0201 Phase 4).
//!
//! # What a square baseline structurally cannot see
//!
//! `golden.rs` captures every fixture at 128x128, and at that shape the
//! aspect-corrected space a converted preset is evaluated in is the identity in
//! both of its halves: `mesh::vertex_position` multiplies the x axis by the
//! target aspect (ADR-0037), which is `1.0` there, and the `(aspectx, aspecty,
//! 1/aspectx, 1/aspecty)` lanes a translated shader reads as `U.aspect` are all
//! `1.0` for the same reason. So the three converted `warp_mesh` fixtures in
//! that binary — `warp_mesh_milk`, `warp_mesh_shader` and `warp_mesh_stroke`,
//! the three carrying a `[milk]` table; the rostered `warp_mesh.toml` carries
//! none — run the whole chain against operands that cannot express an error in
//! it: drop either term and every one of those baselines renders
//! byte-identically.
//!
//! `warp_mesh.rs`'s per-stage tests assert what the chain *computes*; what no
//! capture in the crate held is the **picture** it produces at a shape where the
//! correction is live, which is the incidental-drift class a baseline exists for.
//!
//! # 160x120, and why that size
//!
//! 4:3. Not 1:1, where both terms above are the identity; and not 16:9, the
//! shape ADR-0037 records this confusion as invisible at — both of the aspect
//! errors it was written for shipped unseen at 1920x1080 and were glaring at
//! 1280x800. 160 wide also matches `composite.rs` and `attractor_trails.rs` in
//! order of magnitude, which is what keeps a capture of a three-level blur chain
//! affordable in a suite that runs on every push.
//!
//! # A module of its own
//!
//! `RLX_BLESS` is not scoped to a fixture. An entry in `golden.rs`'s
//! `EXTRA_FIXTURES` would mean one bless rewrites every baseline of that binary
//! to add this one, so an unrelated re-encode rides in under a diff nobody reads
//! as a change. As its own module,
//! `RLX_BLESS=1 cargo test -p rlx-core --test suite warp_mesh_wide::` reaches
//! this file and nothing else — the posture `attractor_trails.rs` documents.
//!
//! Everything else is `golden.rs`'s: `milk_wash_fog_tunnel.toml` unmodified, 60
//! frames, the same two tolerances. The square capture and this one differ in
//! the target size and in nothing else, which is what makes a disagreement
//! between them a reading about the correction rather than about two fixtures.
//!
//! # Why this bundle, and not one that merely declares motion
//!
//! The subject is chosen by what the correction moves, not by what its `[milk]`
//! table names. Forcing `self.aspect = 1.0` at the converted chain's own entry
//! and capturing each candidate at this size (2026-09-19, development machine):
//!
//! | fixture | mean (tol 0.02) | max outlier (tol 48) |
//! |---|---|---|
//! | `milk_wash_fog_tunnel` | 0.0139 | **75 — convicts** |
//! | `warp_mesh_milk` | 0.0057 | 21 |
//! | `warp_mesh_stroke` | 0.0000 | 0 |
//! | `milk_wash_blur_mix_3` | 0.0000 | 0 |
//!
//! All four declare `zoom`, `rot` or `warp`; only one of them renders a picture
//! the corrected space reaches far enough to exceed a tolerance, and it does so
//! on the **outlier** term rather than the mean — a warp redistributes edges
//! rather than shifting the frame's average. `warp_mesh_shader.toml` reads
//! 0.0000 and 0 under the same probe: it declares no mesh motion at all, so the
//! corrected coordinates reach nothing it draws. A fixture that cannot fail is
//! not a guard (Plan 0201 Phase 4a).

use rlx_core::preset::Preset;
use rlx_core::render::{CaptureImage, Renderer, metrics::frame_diff};

use crate::common;

/// Capture width/height — see the module docs. **Not interchangeable**, and
/// `the_capture_size_is_neither_square_nor_sixteen_by_nine` is what holds them.
const WIDTH: u32 = 160;
const HEIGHT: u32 = 120;
/// Frames warmed before capture, `golden.rs`'s count: the feedback field and the
/// blur chain both carry state across frames.
const FRAMES: u32 = 60;
/// Mean per-channel difference (0..1) a fresh render may drift from baseline.
/// `golden.rs`'s tolerance.
const MEAN_TOL: f32 = 0.02;
/// Largest single-channel byte difference tolerated at any pixel.
const MAX_OUTLIER: u8 = 48;

const STEM: &str = "warp_mesh_wide";
const FIXTURE: &str = include_str!("../fixtures/milk_wash_fog_tunnel.toml");
/// What captures the baseline, printed by the missing-baseline assertion and by
/// the drift failure so neither leaves a reader to reconstruct it.
const BLESS_CMD: &str = "RLX_BLESS=1 cargo test -p rlx-core --test suite warp_mesh_wide::";

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

/// Capture the fixture once, with the renderer that drew it (for the adapter
/// question the baseline asks), or `None` on an adapterless runner.
fn capture() -> Option<(CaptureImage, Renderer)> {
    let mut renderer = common::headless(WIDTH, HEIGHT)?;
    let preset = Preset::from_toml_str(FIXTURE)
        .unwrap_or_else(|e| panic!("milk_wash_fog_tunnel.toml is invalid: {e}"));
    let name = preset.name.clone();
    renderer.set_presets(vec![preset]);
    let fresh = renderer
        .capture_preset(&name, &common::fixed_frame_spectrum(), FRAMES)
        .expect("capture the converted warp_mesh fixture at a non-square target");
    Some((fresh, renderer))
}

/// The converted chain's picture at 4:3, against its committed baseline.
#[test]
fn the_converted_chain_matches_its_wide_baseline() {
    let path = common::golden_dir().join(format!("{STEM}.png"));
    let Some((fresh, renderer)) = capture() else {
        return;
    };
    let bless = common::bless_requested(&renderer);
    std::fs::create_dir_all(common::golden_dir()).expect("create tests/golden");

    if bless {
        common::encode(&fresh, &path);
        println!("blessed {}", path.display());
        return;
    }

    assert!(
        path.exists(),
        "missing baseline {} — run `{BLESS_CMD}`",
        path.display()
    );
    let baseline = common::decode(&path);
    assert_eq!(
        (baseline.width, baseline.height),
        (WIDTH, HEIGHT),
        "the committed baseline is {}x{} and this fixture captures {WIDTH}x{HEIGHT} — \
         a baseline at another shape says nothing about the aspect correction",
        baseline.width,
        baseline.height
    );
    let mean = frame_diff(&baseline, &fresh);
    let outlier = max_channel_outlier(&baseline, &fresh);
    println!(
        "{STEM:<18} mean {mean:.4} (tol {MEAN_TOL}) max_outlier {outlier} (tol {MAX_OUTLIER}) at {WIDTH}x{HEIGHT}"
    );
    if let Err(adapter) = common::baseline_adapter(&renderer) {
        let drifted = if mean <= MEAN_TOL && outlier <= MAX_OUTLIER {
            Vec::new()
        } else {
            vec![format!("{STEM}: mean {mean:.4} / outlier {outlier}")]
        };
        common::skip_off_baseline(&adapter, &drifted);
        return;
    }
    assert!(
        mean <= MEAN_TOL && outlier <= MAX_OUTLIER,
        "{STEM}: mean {mean:.4} / outlier {outlier} exceeds tolerance — the \
         converted chain's aspect-corrected space has moved. Bless with \
         `{BLESS_CMD}` only if intended"
    );
}

/// The capture size is still one the correction is live at.
///
/// A baseline at 1:1 or 16:9 would pass forever and guard nothing (module docs),
/// and the size is two `const`s an unrelated edit can move. No GPU, so this runs
/// on an adapterless runner — the case where the capture above says nothing at
/// all.
#[test]
fn the_capture_size_is_neither_square_nor_sixteen_by_nine() {
    /// How close to a forbidden ratio counts as being it. Both shapes are exact
    /// in integers, so any margin separates them; this one is wide enough that
    /// 161x120 is convicted too.
    const RATIO_SLACK: f32 = 0.02;

    let aspect = WIDTH as f32 / HEIGHT as f32;
    println!("{STEM} captures {WIDTH}x{HEIGHT}, aspect {aspect:.4}");

    assert!(
        (aspect - 1.0).abs() > RATIO_SLACK,
        "{STEM} captures {WIDTH}x{HEIGHT} (aspect {aspect:.4}), which is square: \
         the mesh's aspect multiply and the shader's aspect lanes are both the \
         identity there, so the baseline would be a second copy of golden.rs's"
    );
    assert!(
        (aspect - 16.0 / 9.0).abs() > RATIO_SLACK,
        "{STEM} captures {WIDTH}x{HEIGHT} (aspect {aspect:.4}), which is 16:9 — \
         the shape ADR-0037 records both shipped aspect confusions as invisible at"
    );

    Preset::from_toml_str(FIXTURE).expect("milk_wash_fog_tunnel.toml parses");
}
