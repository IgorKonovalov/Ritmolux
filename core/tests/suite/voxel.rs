//! The voxel system through the whole renderer: the rule list a preset loads,
//! the rule index held on the bar, and the shells as an exact identity at zero
//! gain.
//!
//! # Why the pictures stand for the volume
//!
//! A frame here is a march through the volume from a fixed camera, with
//! `age_tint = 0` so a cell's colour does not move with its age: two frames
//! are byte-identical exactly when no visible cell changed, so a claim about
//! when the volume moves can be made off the frames the renderer returns.
//!
//! **Software adapter**, like the rest of the GPU suites (ADR-0016). A renderer
//! is dropped before the next is built.

use crate::common;

use rlx_core::dsp::AnalysisFrame;
use rlx_core::preset::Preset;
use rlx_core::render::{CaptureImage, Renderer};

fn load(renderer: &mut Renderer, toml: &str) -> String {
    let preset = Preset::from_toml_str(toml).expect("the probe preset loads");
    let name = preset.name.clone();
    renderer.set_presets(vec![preset]);
    name
}

/// Every frame of `toml` driven by `frames`, on a fresh renderer.
fn run(toml: &str, frames: &[AnalysisFrame]) -> Option<Vec<CaptureImage>> {
    let mut renderer = common::headless(64, 64)?;
    let name = load(&mut renderer, toml);
    Some(
        renderer
            .capture_preset_over(&name, frames)
            .expect("capture the probe"),
    )
}

// ---------------------------------------------------------------------------
// The rule, held on the bar
// ---------------------------------------------------------------------------

/// Frames a bar lasts in the stimulus.
const BAR_FRAMES: usize = 30;
/// Bars the stimulus runs.
const BARS: usize = 5;

/// Two rules picked by the bar: rule 0 changes nothing — every dead cell stays
/// dead and every live one lives, and with two states there is no decay — and
/// rule 1 is the restless `pyroclastic`. One generation a frame.
const BAR_PRESET: &str = r#"
system = "voxel"
name = "bar-probe"

[generator]
seed = 3

[voxel]
grid = 32
rules = [
    { birth = [], survive = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26] },
    "pyroclastic",
]
seed_radius = 0.5
seed_fill = 0.3

[params]
rule = "mod(bar_index, 2)"
step_rate = "60"
age_tint = "0"
density = "1"
yaw = "0.6"
pitch = "0.4"

[hold]
rule = "bar"
"#;

/// A stimulus whose bar counter advances every [`BAR_FRAMES`] frames.
fn bars() -> Vec<AnalysisFrame> {
    (0..BAR_FRAMES * BARS)
        .map(|i| AnalysisFrame {
            bar_index: (i / BAR_FRAMES) as u32,
            ..AnalysisFrame::default()
        })
        .collect()
}

/// **A rule bound to `mod(bar_index, 2)` and held on the bar changes only at
/// bar edges**: through every odd bar, under `pyroclastic`, each frame differs
/// from the one before; through every even bar, under the still rule, no frame
/// after the bar's first does. The regime switches exactly where the bar does.
/// And the same run twice draws the same frames, the last included.
#[test]
fn a_rule_held_on_the_bar_changes_only_at_bar_edges() {
    let frames = bars();
    let Some(one) = run(BAR_PRESET, &frames) else {
        return;
    };
    let Some(two) = run(BAR_PRESET, &frames) else {
        return;
    };
    let mut report = String::new();
    for k in 1..one.len() {
        let bar = k / BAR_FRAMES;
        let changed = one[k].rgba != one[k - 1].rgba;
        report.push(if changed { '#' } else { '.' });
        if k % BAR_FRAMES == BAR_FRAMES - 1 {
            report.push('|');
        }
        if bar % 2 == 1 {
            assert!(
                changed,
                "frame {k} in bar {bar} runs pyroclastic and did not move: {report}"
            );
        } else if k % BAR_FRAMES != 0 {
            assert!(
                !changed,
                "frame {k} in bar {bar} runs the still rule and moved: {report}"
            );
        }
    }
    println!("frames that moved, bar by bar: {report}");
    let first_divergence = one.iter().zip(&two).position(|(a, b)| a.rgba != b.rgba);
    assert_eq!(
        first_divergence, None,
        "two runs from one seed and one stimulus diverged"
    );
}

// ---------------------------------------------------------------------------
// The shells
// ---------------------------------------------------------------------------

/// The fixture's preset, with `extra_voxel` added to its `[voxel]` table and
/// `extra_params` to its `[params]`.
fn shell_preset(extra_voxel: &str, extra_params: &str) -> String {
    format!(
        "system = \"voxel\"\nname = \"shells\"\n\
         [generator]\nseed = 7\n\
         [voxel]\ngrid = 32\nrules = [\"clouds\"]\n{extra_voxel}\n\
         [params]\ndensity = \"1.5\"\nyaw = \"0.6\"\npitch = \"0.4\"\n{extra_params}\n"
    )
}

fn capture(toml: &str) -> Option<CaptureImage> {
    let mut renderer = common::headless(128, 128)?;
    let name = load(&mut renderer, toml);
    Some(
        renderer
            .capture_preset(&name, &common::fixed_frame_spectrum(), 60)
            .expect("capture the shell probe"),
    )
}

/// **`shell_gain = 0` is an exact identity**: eight shells lit by a spectrum
/// at zero gain draw byte-for-byte the frame of the same preset with no shells
/// at all. The control, the same eight shells at gain 2, draws another frame —
/// so the shells do reach the pixels.
#[test]
fn zero_shell_gain_draws_the_frame_with_no_shells() {
    let Some(absent) = capture(&shell_preset("", "")) else {
        return;
    };
    let Some(zero) = capture(&shell_preset("shells = 8", "shell_gain = \"0\"")) else {
        return;
    };
    let Some(lit) = capture(&shell_preset("shells = 8", "shell_gain = \"2\"")) else {
        return;
    };
    assert!(
        zero.rgba == absent.rgba,
        "eight shells at zero gain moved the frame"
    );
    assert!(
        lit.rgba != absent.rgba,
        "eight shells at gain 2 changed nothing"
    );
}

// ---------------------------------------------------------------------------
// The rule list's load errors
// ---------------------------------------------------------------------------

/// The load error `rules` produces, which must exist.
fn rules_error(rules: &str) -> String {
    let toml = format!("system = \"voxel\"\n[voxel]\nrules = {rules}\n");
    match Preset::from_toml_str(&toml) {
        Ok(_) => panic!("`rules = {rules}` loaded"),
        Err(e) => e.to_string(),
    }
}

/// **A Moore count past 26 is a load error naming the entry.**
#[test]
fn a_moore_count_past_26_names_its_entry() {
    let e = rules_error(r#"["clouds", { birth = [4, 27], survive = [4] }]"#);
    assert!(e.contains("rules entry 1") && e.contains("27"), "{e}");
}

/// **A von Neumann count past 6 is a load error naming the entry.**
#[test]
fn a_von_neumann_count_past_6_names_its_entry() {
    let e =
        rules_error(r#"[{ birth = [1], survive = [7], neighbourhood = "von_neumann" }, "coral"]"#);
    assert!(e.contains("rules entry 0") && e.contains('7'), "{e}");
}

/// **`states` below 2 is a load error naming the entry.**
#[test]
fn states_below_2_names_its_entry() {
    let e = rules_error(r#"["clouds", "coral", { birth = [4], survive = [4], states = 1 }]"#);
    assert!(e.contains("rules entry 2") && e.contains("states 1"), "{e}");
}

/// **An unknown roster name is a load error naming the entry, and the roster.**
#[test]
fn an_unknown_roster_name_names_its_entry() {
    let e = rules_error(r#"["clouds", "cloudz"]"#);
    assert!(
        e.contains("rules entry 1") && e.contains("cloudz") && e.contains("coral"),
        "{e}"
    );
}

/// **The forms a list may take load**: names and inline rules mixed, a von
/// Neumann rule at its own ceiling, and the most rules a list holds.
#[test]
fn names_and_inline_rules_load_together() {
    let toml = r#"
system = "voxel"
[voxel]
rules = [
    "clouds",
    { birth = [4], survive = [4, 26], states = 5 },
    { birth = [1, 3], survive = [0, 6], neighbourhood = "von_neumann" },
    "coral", "crystal", "amoeba", "builder", "pyroclastic",
]
"#;
    Preset::from_toml_str(toml).expect("a mixed list of eight loads");
    let nine = r#"["clouds", "clouds", "clouds", "clouds", "clouds", "clouds", "clouds", "clouds", "clouds"]"#;
    assert!(rules_error(nine).contains("1..=8"));
}
