//! The cellular system through the whole renderer: the tier's two caps on its
//! content, and the determinism an automaton — a chaotic integrator — needs to
//! be baselined at all.
//!
//! # Why the pictures stand for the field
//!
//! Every capture here is the size of the grid it draws, so a pixel is a cell,
//! and the presets bind `trail` so a pixel's light also reads its age. Two
//! captures that are byte-identical therefore hold identical fields, and a
//! claim about the field can be made off the frames the renderer returns.
//!
//! **Software adapter**, like the rest of the GPU suites (ADR-0016). A renderer
//! is dropped before the next is built: two live devices in one binary is what
//! the software adapter falls over on.

mod common;

use rlx_core::dsp::AnalysisFrame;
use rlx_core::preset::Preset;
use rlx_core::render::{CaptureImage, Renderer, Tier, TierConfig};

/// A cellular preset on `grid` for `family`, with `extra` appended to its
/// `[params]`.
fn preset(family: &str, grid: u32, extra: &str) -> String {
    format!(
        "system = \"cellular\"\nname = \"probe\"\n\
         [cellular]\nfamily = \"{family}\"\ngrid = {grid}\n\
         [generator]\nseed = 9\n\
         [params]\nstep_rate = \"20\"\ntrail = \"0\"\n{extra}"
    )
}

fn load(renderer: &mut Renderer, toml: &str) -> String {
    let preset = Preset::from_toml_str(toml).expect("the probe preset loads");
    let name = preset.name.clone();
    renderer.set_presets(vec![preset]);
    name
}

/// One capture of `toml` on a fresh software renderer pinned to `tier`, and
/// what that renderer reported as a cap overflow.
fn capture_on(tier: Tier, size: u32, toml: &str) -> Option<(CaptureImage, Option<String>)> {
    let mut renderer = common::headless_tiered(size, size, tier)?;
    let name = load(&mut renderer, toml);
    let img = renderer
        .capture_preset(&name, &AnalysisFrame::default(), 60)
        .expect("capture the probe");
    let notice = renderer.cap_overflow().map(|o| o.to_string());
    Some((img, notice))
}

// ---------------------------------------------------------------------------
// The tier's caps
// ---------------------------------------------------------------------------

/// **A preset asking within the `Floor` bounds runs identically on both
/// tiers**, byte for byte and with no notice — the default grid, and the
/// default `larger_than_life` radius.
#[test]
fn a_preset_within_the_floor_bounds_runs_identically_on_both_tiers() {
    const _: () = assert!(256 <= TierConfig::FLOOR.cellular_grid);
    const _: () = assert!(5 <= TierConfig::FLOOR.cellular_radius);
    let toml = preset("larger_than_life", 256, "radius = \"5\"\n");
    let Some((floor, floor_notice)) = capture_on(Tier::Floor, 64, &toml) else {
        return;
    };
    let Some((rich, rich_notice)) = capture_on(Tier::Rich, 64, &toml) else {
        return;
    };
    assert_eq!(floor_notice, None, "a preset inside the caps was reported");
    assert_eq!(rich_notice, None);
    assert!(
        floor.rgba == rich.rgba,
        "a preset inside the Floor bounds drew a different field on Rich"
    );
}

/// **An over-tier `grid` is clamped with a notice**: on `Floor`, a preset
/// asking for 1024 cells runs exactly the field of one asking for the cap, and
/// the renderer announces the clamp through the channel the standalone prints
/// cap overflows through; `Rich`, whose cap it is inside, runs the larger grid
/// and says nothing — and draws a different picture, so the equality on `Floor`
/// is the clamp's doing.
#[test]
fn an_over_tier_grid_is_clamped_with_a_notice() {
    let cap = TierConfig::FLOOR.cellular_grid;
    let asked = TierConfig::RICH.cellular_grid;
    assert!(asked > cap, "Rich must allow what this test asks Floor for");
    let over = preset("life_like", asked, "");
    let Some((floor, notice)) = capture_on(Tier::Floor, 64, &over) else {
        return;
    };
    let Some((at_cap, cap_notice)) = capture_on(Tier::Floor, 64, &preset("life_like", cap, ""))
    else {
        return;
    };
    let Some((rich, rich_notice)) = capture_on(Tier::Rich, 64, &over) else {
        return;
    };
    println!("Floor's notice: {notice:?}");
    let notice = notice.expect("a clamped grid must be announced, not silent");
    assert!(
        notice.contains(&format!("grid {asked}")) && notice.contains(&cap.to_string()),
        "the notice must say what was asked and what it runs at: {notice}"
    );
    assert_eq!(
        cap_notice, None,
        "asking for exactly the cap is not a clamp"
    );
    assert_eq!(
        rich_notice, None,
        "Rich allows {asked}, so it reports nothing"
    );
    assert!(
        floor.rgba == at_cap.rgba,
        "the clamped grid is not the field of a preset asking for the cap"
    );
    assert!(
        floor.rgba != rich.rgba,
        "the clamp changed nothing, so this test cannot see it"
    );
}

/// **An over-tier `radius` is clamped with a notice**, per frame and through
/// the same channel: on `Floor` a `larger_than_life` preset asking for radius
/// 10 runs exactly the rule at the cap, and says so; on `Rich` it runs radius
/// 10 and says nothing. The same radius on `life_like`, which reads none, is no
/// clamp at all.
#[test]
fn an_over_tier_radius_is_clamped_with_a_notice() {
    let cap = TierConfig::FLOOR.cellular_radius;
    const _: () = assert!(TierConfig::FLOOR.cellular_radius < 10);
    const _: () = assert!(TierConfig::RICH.cellular_radius >= 10);
    let over = preset("larger_than_life", 128, "radius = \"10\"\n");
    let Some((floor, notice)) = capture_on(Tier::Floor, 64, &over) else {
        return;
    };
    let at_cap_toml = preset("larger_than_life", 128, &format!("radius = \"{cap}\"\n"));
    let Some((at_cap, cap_notice)) = capture_on(Tier::Floor, 64, &at_cap_toml) else {
        return;
    };
    let Some((rich, rich_notice)) = capture_on(Tier::Rich, 64, &over) else {
        return;
    };
    let Some((_, inert_notice)) = capture_on(
        Tier::Floor,
        64,
        &preset("life_like", 128, "radius = \"10\"\n"),
    ) else {
        return;
    };
    println!("Floor's notice: {notice:?}");
    let notice = notice.expect("a clamped radius must be announced, not silent");
    assert!(
        notice.contains("radius 10") && notice.contains(&cap.to_string()),
        "the notice must say what was asked and what it runs at: {notice}"
    );
    assert_eq!(
        cap_notice, None,
        "asking for exactly the cap is not a clamp"
    );
    assert_eq!(rich_notice, None, "Rich allows 10, so it reports nothing");
    assert_eq!(
        inert_notice, None,
        "life_like reads no radius, so nothing was clamped"
    );
    assert!(
        floor.rgba == at_cap.rgba,
        "the clamped radius is not the rule at the cap"
    );
    assert!(
        floor.rgba != rich.rgba,
        "the clamp changed nothing, so this test cannot see it"
    );
}

// ---------------------------------------------------------------------------
// Determinism
// ---------------------------------------------------------------------------

/// Frames the determinism run drives. At a `step_rate` of `200 + 40 * bass`
/// generations a second and the capture's 1/60 s step, that is at least 1,000
/// generations whatever the bass does.
const LONG_RUN: usize = 300;

/// A stimulus that moves: a beat every 23 frames, which reseeds a disc, and a
/// bass level that walks, which moves the generation rate and so the history.
fn stimulus() -> Vec<AnalysisFrame> {
    (0..LONG_RUN)
        .map(|i| AnalysisFrame {
            beat: i % 23 == 0,
            bass: ((i * 37) % 100) as f32 / 100.0,
            ..AnalysisFrame::default()
        })
        .collect()
}

/// The long-run preset for `family`, on a 64-cell grid drawn one cell to a
/// pixel.
fn long_run(family: &str, seed: u32) -> String {
    format!(
        "system = \"cellular\"\nname = \"long\"\n\
         [cellular]\nfamily = \"{family}\"\ngrid = 64\n\
         [generator]\nseed = {seed}\n\
         [params]\nstep_rate = \"200 + 40 * bass\"\nreseed = \"beat\"\ntrail = \"8\"\n\
         radius = \"3\"\n"
    )
}

/// Every frame of `toml` driven by `frames`, on a fresh renderer.
fn run(toml: &str, frames: &[AnalysisFrame]) -> Option<Vec<CaptureImage>> {
    let mut renderer = common::headless(64, 64)?;
    let name = load(&mut renderer, toml);
    Some(
        renderer
            .capture_preset_over(&name, frames)
            .expect("capture the long run"),
    )
}

/// **Identical seed plus identical analysis frames yields an identical field
/// after 1,000 generations**, for every family — every one of the 300 frames
/// byte-identical across two runs on two renderers, reseeds and a moving
/// generation rate included. The controls: a different seed diverges, and so
/// does the same seed with one beat moved, so the equality is a property of the
/// inputs and not of a field that went blank.
#[test]
fn identical_inputs_yield_an_identical_field_after_a_thousand_generations() {
    const _: () = assert!(LONG_RUN * 200 / 60 >= 1000);
    let frames = stimulus();
    let mut nudged = frames.clone();
    nudged[150].beat = !nudged[150].beat;
    for family in ["life_like", "larger_than_life", "cyclic"] {
        let Some(one) = run(&long_run(family, 3), &frames) else {
            return;
        };
        let Some(two) = run(&long_run(family, 3), &frames) else {
            return;
        };
        let first_divergence = one.iter().zip(&two).position(|(a, b)| a.rgba != b.rgba);
        assert_eq!(
            first_divergence, None,
            "{family}: two runs from one seed and one stimulus diverged"
        );
        let last = one.last().expect("frames were captured");
        let lit = last
            .rgba
            .chunks_exact(4)
            .filter(|p| p[..3] != [0, 0, 0])
            .count();
        println!("{family}: {lit} of 4096 pixels lit at the last frame");
        assert!(lit > 40, "{family}: the field is nearly blank ({lit} lit)");

        let Some(other_seed) = run(&long_run(family, 4), &frames) else {
            return;
        };
        assert!(
            other_seed.last().map(|i| &i.rgba) != Some(&last.rgba),
            "{family}: a different seed reached the same field"
        );
        let Some(moved_beat) = run(&long_run(family, 3), &nudged) else {
            return;
        };
        assert!(
            moved_beat.last().map(|i| &i.rgba) != Some(&last.rgba),
            "{family}: one beat moved changed nothing, so the stimulus is not reaching it"
        );
    }
}

/// **The goldens hold across a re-run on the software adapter**: each cellular
/// fixture captured by two renderers built in turn is byte-identical — the
/// fixed seed and fixed generation count are what make an automaton
/// baselineable, and this is that claim made without the stored baseline's
/// cross-machine tolerance.
#[test]
fn the_cellular_goldens_hold_across_a_rerun() {
    let fixtures = [
        include_str!("fixtures/cellular.toml"),
        include_str!("fixtures/cellular_trail.toml"),
        include_str!("fixtures/cellular_ltl.toml"),
        include_str!("fixtures/cellular_cyclic.toml"),
    ];
    for toml in fixtures {
        // The golden suite's own size, frames and frame.
        let capture = || -> Option<CaptureImage> {
            let mut renderer = common::headless(128, 128)?;
            let name = load(&mut renderer, toml);
            Some(
                renderer
                    .capture_preset(&name, &common::fixed_frame_spectrum(), 60)
                    .expect("capture the fixture"),
            )
        };
        let Some(one) = capture() else {
            return;
        };
        let Some(two) = capture() else {
            return;
        };
        let grid = toml
            .lines()
            .find_map(|l| l.trim().strip_prefix("grid"))
            .map(str::trim);
        assert_eq!(grid, Some("= 64"), "every cellular fixture is Floor-legal");
        assert!(
            one.rgba == two.rgba,
            "a cellular golden moved across a re-run"
        );
    }
}
