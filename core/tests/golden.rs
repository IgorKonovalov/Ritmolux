//! Golden-image drift (Plan 0013 Phase 4; repointed by Plan 0022, ADR-0023).
//! This guard defends **engine rendering determinism**, not shipped content: it
//! renders one **frozen per-system fixture** headless on the software adapter
//! and compares each frame against a committed baseline PNG within a mean +
//! max-outlier tolerance. `RLX_BLESS=1` rewrites the baselines.
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

use rlx_core::preset::{Preset, SystemKind};
use rlx_core::render::metrics::{set_extent_diagnostic, take_draw_extent};
use rlx_core::render::{CaptureImage, Renderer, metrics::frame_diff};

mod common;

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
        SystemKind::ShapeField => ("shape_field", include_str!("fixtures/shape_field.toml")),
        SystemKind::WarpMesh => ("warp_mesh", include_str!("fixtures/warp_mesh.toml")),
        SystemKind::ShapeCollage => ("shape_collage", include_str!("fixtures/shape_collage.toml")),
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
/// Each entry names what the rostered fixture **structurally cannot
/// reach**, and an entry without such a claim does not belong here:
///
/// - `attractor_depth` — the roster's `attractor.toml` is De Jong, and ADR-0076
///   gives every 2-D family an inverse depth extent of exactly `0.0`, which is
///   what makes the perspective divide, the distance haze and the depth tint the
///   identity there.
/// - `attractor_ifs` — De Jong takes a different arm of the step shader and its
///   `a`..`d` coefficients *are* its shape, so the SVD decomposition, the morph,
///   the framing fit and the levers are inert on it.
/// - `swarm_shaped` — the silhouette default `disc` is **exactly**
///   `length(local)`, so the rostered `swarm.toml` takes that arm and none of the
///   ring, polygon, star or heart arms.
/// - `backdrop_ramp` — every rostered fixture runs `bg_bright = 0`, where the
///   gradient pipeline is not even built (`background.rs`'s module docs). Its
///   `bg_ramp_gamma` off `1.0` is the only thing in the crate's baselines that
///   takes the shader's `pow` arm rather than its identity `select` arm.
/// - `backdrop_band` — the band is an untaken `select` branch at
///   `bg_band_amount = 0`, so even `backdrop_ramp` executes none of it, and its
///   `bg_band_curve` off `0` is the only baseline that reads the along-band axis.
/// - `warp_mesh_milk` — the rostered `warp_mesh.toml` carries no `[milk]` table,
///   so no bundle is constructed and not one EEL2 instruction executes. The two
///   presets drive the same nine scene outputs through separate paths.
/// - `warp_mesh_shader` — what puts it out of reach is a **file**, not a param:
///   `warp_mesh_milk`'s `[milk]` table declares no `warp_shader`, `comp_shader`
///   or `blur_level`, and `warp_mesh/shader.rs` is built only for a bundle that
///   declares them. The one baseline executing a translated shader module, a
///   procedural noise texture, or a level of the blur chain.
/// - `shape_collage_roster` — the rostered fixture runs the scene's defaults, so
///   at `layout = 0` the seeded layout grammar is unreached and at `roster = 0`
///   no line of `sd_bar`, `sd_ring`, `sd_segment`, `sd_arc` or `sd_checker`
///   executes. Also the only baseline where two elements composite `over` each
///   other rather than one hiding the other.
///
/// **Captured after the roster loop, and appended rather than inserted.** Every
/// pre-existing baseline is therefore rendered from the device state it always
/// was, so adding an entry here moves none of them — which matters on WARP,
/// where building GPU resources mid-run is documented to change what a later
/// capture resolves to. For the same reason a new entry goes at the **end**.
const EXTRA_FIXTURES: [(&str, &str); 9] = [
    (
        "attractor_depth",
        include_str!("fixtures/attractor_depth.toml"),
    ),
    ("attractor_ifs", include_str!("fixtures/attractor_ifs.toml")),
    ("swarm_shaped", include_str!("fixtures/swarm_shaped.toml")),
    ("backdrop_ramp", include_str!("fixtures/backdrop_ramp.toml")),
    ("backdrop_band", include_str!("fixtures/backdrop_band.toml")),
    (
        "warp_mesh_milk",
        include_str!("fixtures/warp_mesh_milk.toml"),
    ),
    (
        "warp_mesh_shader",
        include_str!("fixtures/warp_mesh_shader.toml"),
    ),
    (
        "shape_collage_roster",
        include_str!("fixtures/shape_collage_roster.toml"),
    ),
    ("warp_mesh_stroke", FIXTURES_WARP_MESH_STROKE),
];

/// The stroke fixture's text, named once so the roster entry above and the guard
/// below cannot come to mean different files.
const FIXTURES_WARP_MESH_STROKE: &str = include_str!("fixtures/warp_mesh_stroke.toml");

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
    let Some(mut renderer) = common::headless(SIZE, SIZE) else {
        return;
    };
    let frame = common::fixed_frame_spectrum();
    let bless = std::env::var_os("RLX_BLESS").is_some();
    std::fs::create_dir_all(common::golden_dir()).expect("create tests/golden");

    let mut failures = Vec::new();
    let mut check = |renderer: &mut Renderer, stem: &str, toml: &str| {
        let preset = Preset::from_toml_str(toml)
            .unwrap_or_else(|e| panic!("golden fixture {stem}.toml is invalid: {e}"));
        let name = preset.name.clone();
        renderer.set_presets(vec![preset]);

        let fresh = renderer
            .capture_preset(&name, &frame, FRAMES)
            .expect("capture fixture");
        let path = common::golden_dir().join(format!("{stem}.png"));

        if bless {
            common::encode(&fresh, &path);
            println!("blessed {}", path.display());
            return;
        }

        assert!(
            path.exists(),
            "missing baseline {} — run `RLX_BLESS=1 cargo test -p rlx-core --test golden`",
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
        "golden drift beyond tolerance (bless with RLX_BLESS=1 if intended): {failures:#?}"
    );
}

/// **The one baseline that can see the `warp_mesh` stroke profile still can**
/// (Plan 0114 Phase 9).
///
/// ADR-0124 pins `warp_mesh` to a stroke profile of its own, because it answers
/// to `foo_vis_milk2` rather than to the line families' look gate. Nothing in
/// the repo held that pin: a change to the shared line fragment could alter
/// every MilkDrop stroke and no baseline would move. `warp_mesh_stroke.toml`
/// is the baseline that closes it, and this is what stops it degrading back
/// into a picture that cannot.
///
/// # Why a stroke being *present* is not enough
///
/// The other three `warp_mesh` fixtures already stroke a waveform — `wave_a`
/// defaults to 1.0, so they cannot help it — and they are still blind, because
/// at [`SIZE`] a `THIN` stroke is 0.16 px of half-width and `THICK` is 0.38 px.
/// The profile's edge is floored at one pixel of the render target, so below
/// about two the floor *is* the whole ramp and every `MILKDROP_SOFTNESS` draws
/// the identical frame. Measured with the pin driven `1.0 -> 0.0`: all three move
/// by mean 0.0000 / outlier 0.
///
/// So both arms are needed. The stroke has to reach the fragment **and** be wide
/// enough for the profile to have room in it — and `ob_size` is the only stroke
/// width on this surface a preset controls, reaching `SegmentInstance::width` as
/// an NDC-y half-width.
#[test]
fn the_warp_mesh_stroke_fixture_shades_a_resolvable_stroke() {
    /// The half-width, in pixels of the golden capture, below which the edge
    /// term is capped and the profile stops being expressible. Two is where it
    /// begins to bite; three is the margin this fixture is asked to keep.
    const MIN_RESOLVABLE_PX: f32 = 3.0;

    let toml = FIXTURES_WARP_MESH_STROKE;

    // The EEL2 the bundle compiled from is reproduced in the file as comments,
    // emitted by `milkconv` from the same source it compiled — so reading them
    // is reading what the bytecode does, not a second copy of it.
    let value = |key: &str| -> f32 {
        toml.lines()
            .find_map(|line| {
                let rest = line.trim_start().strip_prefix('#')?.trim_start();
                let rest = rest.strip_prefix(key)?.trim_start();
                let rest = rest.strip_prefix('=')?.trim();
                rest.trim_end_matches(';').trim().parse::<f32>().ok()
            })
            .unwrap_or(f32::NAN)
    };

    let ob_size = value("ob_size");
    let ob_a = value("ob_a");
    let wave_a = value("wave_a");
    let half_px = ob_size * SIZE as f32 / 2.0;
    println!(
        "warp_mesh_stroke: ob_size {ob_size} ({half_px:.2} px of half-width at \
         {SIZE}), ob_a {ob_a}, wave_a {wave_a}"
    );

    assert!(
        ob_a > 0.0,
        "warp_mesh_stroke.toml no longer draws its border (ob_a = {ob_a}), which \
         is the only stroke on this surface wide enough for the profile to show \
         in — without it this baseline is as blind as the other three"
    );
    assert!(
        half_px >= MIN_RESOLVABLE_PX,
        "warp_mesh_stroke.toml strokes its border at {half_px:.2} px of \
         half-width at {SIZE} (ob_size = {ob_size}), under the \
         {MIN_RESOLVABLE_PX} px this fixture exists to keep. Below about two \
         pixels the profile's edge term is capped and every MILKDROP_SOFTNESS \
         draws the same frame, so the baseline would still exist and still \
         guard nothing"
    );
    assert!(
        wave_a > 0.0,
        "warp_mesh_stroke.toml no longer sets a wave (wave_a = {wave_a}), so it \
         has stopped being representative of the surface it stands for"
    );

    // And it actually reaches the line renderer, which no amount of reading the
    // file can establish: the Plan 0069 extent diagnostic is read off the draw
    // itself and yields nothing when no segment was stroked.
    let Some(mut renderer) = common::headless(SIZE, SIZE) else {
        return;
    };
    let preset = Preset::from_toml_str(toml).expect("warp_mesh_stroke.toml is valid");
    let name = preset.name.clone();
    renderer.set_presets(vec![preset]);
    set_extent_diagnostic(true);
    renderer
        .capture_preset(&name, &common::fixed_frame_spectrum(), FRAMES)
        .expect("capture warp_mesh_stroke");
    let extent = take_draw_extent();
    set_extent_diagnostic(false);
    let fraction = extent.and_then(|e| e.fraction());
    println!("warp_mesh_stroke in-frame geometry: {fraction:?}");
    assert!(
        fraction.is_some(),
        "warp_mesh_stroke drew no segments at all — the baseline is a picture of \
         a warp field, and the gap ADR-0124 records is still open"
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
