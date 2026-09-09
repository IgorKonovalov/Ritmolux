//! The live parameter override, and the reload that stopped resetting the show
//! (ADR-0176, Plan 0158 Phase 1).
//!
//! Two claims, and both are asserted against **a preset that binds the value the
//! override holds** rather than against a number read back out of the engine.
//! Nothing public reports what a scene's parameter settled at, and a test that
//! invented an accessor for it would be asserting that the accessor works. So
//! the reference for "`bg_bright` was driven to 0.8" is a second renderer whose
//! preset binds `bg_bright = "0.8"`, and the comparison is byte identity — on
//! `core/tests/frame_tap.rs`'s precedent and for its reason: a tolerance would
//! pass with the override landing somewhere merely nearby.
//!
//! `bg_bright` is the probe throughout because it belongs to the backdrop
//! pre-pass, which every system draws through, and because `0` is black — so a
//! frame that ignored the override is not merely different, it is dark.
//!
//! Software adapter (`prefer_software`) so it holds on any CI GPU.

use rlx_core::dsp::AnalysisFrame;
use rlx_core::preset::Preset;
use rlx_core::render::{CaptureImage, ParamError, Renderer};

mod common;

/// Small offscreen: every claim here is about which value reached a stage, not
/// about how many pixels carried it, and the software adapter is slow.
const SIZE: u32 = 96;

/// The frame step every run below advances by — one 60 Hz frame, so two runs
/// compared against each other sit on the same clock.
const DT: f32 = 1.0 / 60.0;

/// A preset that binds `bg_bright` to the constant `value` and nothing else.
///
/// No `[smoothing]` entry, so the binding's `tau` is `INSTANT` and the routed
/// value is the constant on the very first frame — which is what lets an
/// override, which is never eased, be compared against it byte for byte.
fn lit(name: &str, value: &str) -> Preset {
    Preset::from_toml_str(&format!(
        "system = \"swarm\"\nname = \"{name}\"\n[params]\nbg_bright = \"{value}\"\n"
    ))
    .expect("hand-written probe preset is valid")
}

/// A preset that binds nothing at all — every parameter at its declared default,
/// which for `bg_bright` is `0`.
fn unbound(name: &str) -> Preset {
    Preset::from_toml_str(&format!("system = \"swarm\"\nname = \"{name}\"\n"))
        .expect("hand-written probe preset is valid")
}

/// Render `frames` frames of the roster `presets`, applying `drive` to the
/// renderer before each one, and return the last.
///
/// A closure rather than a value because the interesting moments are *between*
/// frames: setting an override after frame 3, clearing it after frame 5.
fn run(
    presets: Vec<Preset>,
    frames: usize,
    mut drive: impl FnMut(&mut Renderer, usize),
) -> Option<CaptureImage> {
    let mut renderer = common::headless(SIZE, SIZE)?;
    renderer.set_presets(presets);
    let mut tap = renderer.open_tap();
    let frame = AnalysisFrame::default();
    let mut last = None;
    for index in 0..frames {
        drive(&mut renderer, index);
        last = Some(
            renderer
                .render_tapped(&mut tap, &frame, DT)
                .expect("render_tapped on a headless renderer"),
        );
    }
    last
}

/// The index of the first differing byte, and what the two frames hold there.
fn first_difference(a: &CaptureImage, b: &CaptureImage) -> Option<String> {
    a.rgba
        .iter()
        .zip(&b.rgba)
        .position(|(x, y)| x != y)
        .map(|i| {
            let (x, y) = (a.rgba.get(i), b.rgba.get(i));
            let (px, chan) = (i / 4, i % 4);
            let (row, col) = (px / a.width as usize, px % a.width as usize);
            format!("byte {i} (pixel {col},{row} channel {chan}): {x:?} vs {y:?}")
        })
}

/// Mean of every RGB byte, `0..255`. The alpha channel is skipped — it is
/// constant across these frames, so including it would only dilute the signal.
fn mean_luma(img: &CaptureImage) -> f64 {
    let sum: f64 = img
        .rgba
        .chunks_exact(4)
        .map(|px| f64::from(px[0]) + f64::from(px[1]) + f64::from(px[2]))
        .sum();
    let pixels = img.rgba.len() / 4;
    if pixels == 0 {
        return 0.0;
    }
    sum / (pixels as f64 * 3.0)
}

/// An override drives the routed value to exactly what a preset binding of the
/// same number would have driven it to.
#[test]
fn an_override_routes_the_value_a_binding_of_the_same_number_would_have() {
    let Some(overridden) = run(vec![lit("dim", "0.2")], 1, |renderer, _| {
        renderer
            .set_param_override("bg_bright", 0.8)
            .expect("bg_bright is claimed by the backdrop on every system");
    }) else {
        return;
    };
    let Some(bound) = run(vec![lit("bright", "0.8")], 1, |_, _| {}) else {
        return;
    };
    let Some(untouched) = run(vec![lit("dim", "0.2")], 1, |_, _| {}) else {
        return;
    };

    // The vacuity guard: if 0.2 and 0.8 rendered the same, byte identity below
    // would hold for an override that reached nothing.
    assert_ne!(
        untouched.rgba, bound.rgba,
        "bg_bright 0.2 and 0.8 rendered identically, so this test cannot \
         observe an override at all"
    );
    if let Some(diff) = first_difference(&bound, &overridden) {
        panic!(
            "an override of bg_bright = 0.8 did not route what a binding of 0.8 \
             routes: {diff}"
        );
    }
}

/// An override reaches a parameter the preset **does not bind** — the case a
/// control surface is in the moment it touches any slider the author left alone.
#[test]
fn an_override_reaches_a_parameter_the_preset_never_bound() {
    let Some(overridden) = run(vec![unbound("silent")], 1, |renderer, _| {
        renderer
            .set_param_override("bg_bright", 0.8)
            .expect("an unbound name is still claimed by the backdrop");
    }) else {
        return;
    };
    let Some(bound) = run(vec![lit("bright", "0.8")], 1, |_, _| {}) else {
        return;
    };

    if let Some(diff) = first_difference(&bound, &overridden) {
        panic!(
            "an override on a parameter the preset never bound did not route \
             what a binding of the same number routes: {diff}. The binding walk \
             never visits an unbound name, so this is the case the override's \
             own resolved route exists for"
        );
    }
}

/// The override survives frames, and clearing it hands the parameter back to the
/// preset's own binding.
#[test]
fn an_override_survives_frames_and_a_clear_returns_the_binding() {
    // Set once before frame 0, then leave it alone for four more frames.
    let Some(held) = run(vec![lit("dim", "0.2")], 5, |renderer, index| {
        if index == 0 {
            renderer
                .set_param_override("bg_bright", 0.8)
                .expect("bg_bright is claimed by the backdrop");
        }
    }) else {
        return;
    };
    let Some(bound) = run(vec![lit("bright", "0.8")], 5, |_, _| {}) else {
        return;
    };
    if let Some(diff) = first_difference(&bound, &held) {
        panic!("an override set once did not survive five frames: {diff}");
    }

    // Same run, cleared before the last frame.
    let Some(released) = run(vec![lit("dim", "0.2")], 5, |renderer, index| {
        if index == 0 {
            renderer
                .set_param_override("bg_bright", 0.8)
                .expect("bg_bright is claimed by the backdrop");
        }
        if index == 4 {
            renderer.clear_param_override("bg_bright");
        }
    }) else {
        return;
    };
    let Some(never) = run(vec![lit("dim", "0.2")], 5, |_, _| {}) else {
        return;
    };
    if let Some(diff) = first_difference(&never, &released) {
        panic!(
            "the frame after a clear did not route the preset's own binding: \
             {diff}"
        );
    }
}

/// A name no stage, scene or backdrop claims is refused where it is set, not
/// dropped silently where it would have been applied.
#[test]
fn a_name_nothing_claims_is_refused() {
    let Some(mut renderer) = common::headless(SIZE, SIZE) else {
        return;
    };
    renderer.set_presets(vec![lit("dim", "0.2")]);

    assert_eq!(
        renderer.set_param_override("no_such_parameter", 0.5),
        Err(ParamError::UnknownParam("no_such_parameter".to_owned())),
        "an unclaimed name is refused at the moment it is set"
    );
    assert_eq!(
        renderer.set_param_override("bg_bright", 0.5),
        Ok(()),
        "a name the backdrop claims is accepted"
    );
}

/// A preset switch and a preset reload both drop every override.
///
/// A held value names a parameter on the preset it was set against, and after
/// either of those that is not the preset being drawn.
#[test]
fn a_switch_and_a_reload_both_drop_every_override() {
    let roster = || vec![lit("dim", "0.2"), lit("other", "0.2")];

    // A switch, taken instantly so no dissolve composites the outgoing side.
    let Some(after_switch) = run(roster(), 2, |renderer, index| {
        if index == 0 {
            renderer
                .set_param_override("bg_bright", 0.8)
                .expect("bg_bright is claimed by the backdrop");
            renderer.select_preset_now(1);
        }
    }) else {
        return;
    };

    // A reload, of a roster that is *not* a rebind (the names moved), so it
    // takes the full path.
    let Some(after_reload) = run(roster(), 2, |renderer, index| {
        if index == 0 {
            renderer
                .set_param_override("bg_bright", 0.8)
                .expect("bg_bright is claimed by the backdrop");
            renderer.set_presets(vec![lit("renamed", "0.2")]);
        }
    }) else {
        return;
    };

    // And a rebind reload, which keeps the show running but still drops the
    // override: the file is the durable channel and it has just spoken.
    let Some(after_rebind) = run(roster(), 2, |renderer, index| {
        if index == 0 {
            renderer
                .set_param_override("bg_bright", 0.8)
                .expect("bg_bright is claimed by the backdrop");
            renderer.set_presets(roster());
        }
    }) else {
        return;
    };

    let Some(never) = run(vec![lit("dim", "0.2")], 2, |_, _| {}) else {
        return;
    };
    let Some(kept) = run(vec![lit("dim", "0.2")], 2, |renderer, index| {
        if index == 0 {
            renderer
                .set_param_override("bg_bright", 0.8)
                .expect("bg_bright is claimed by the backdrop");
        }
    }) else {
        return;
    };
    // The vacuity guard for all three: an override that was never applied would
    // make every comparison below hold for the wrong reason.
    assert_ne!(
        never.rgba, kept.rgba,
        "the override changed nothing even when nothing dropped it, so this \
         test cannot observe a drop"
    );

    for (label, image) in [
        ("a preset switch", &after_switch),
        ("a replacing reload", &after_reload),
        ("a rebinding reload", &after_rebind),
    ] {
        assert_eq!(
            image.rgba, never.rgba,
            "{label} left the override standing: the frame after it still \
             differs from one where no override was ever set"
        );
    }
}

/// A reload that only rewrote expressions keeps the eased values it had; a
/// reload that replaced the roster snaps them, as it always did.
///
/// The two arms are the same preset with the same expression and the same easing
/// constant, reloaded twice — under its **own** name (a rebind) and under
/// another (a replacement). Everything else about the two runs is identical, so
/// the difference between the two jumps is the reset path and nothing else.
///
/// The property: the frame after a rebind moves by the same step the run was
/// already moving by, not by a fresh smoother's snap. The
/// replacement arm is what makes that a measurement rather than an assumption —
/// without it, an engine that had never reset anything would pass.
#[test]
fn a_rebind_keeps_the_eased_values_and_a_replacement_snaps_them() {
    /// A slowly rising `bg_bright` under a three-second ease, so the smoothed
    /// value trails its raw one by a wide, steady margin after a hundred frames.
    fn rising(name: &str) -> Preset {
        Preset::from_toml_str(&format!(
            "system = \"swarm\"\nname = \"{name}\"\n\
             [params]\nbg_bright = \"0.15 + time * 0.2\"\n\
             [smoothing]\nbg_bright = 3.0\n"
        ))
        .expect("hand-written probe preset is valid")
    }

    /// A hundred frames of `rising`, then `reload`, then one more frame.
    /// Returns the mean luminance of the last three settled frames before the
    /// reload and of the one after it.
    fn arm(reload: Vec<Preset>) -> Option<(f64, f64, f64)> {
        let mut renderer = common::headless(SIZE, SIZE)?;
        renderer.set_presets(vec![rising("probe")]);
        let mut tap = renderer.open_tap();
        let frame = AnalysisFrame::default();
        let mut settled = Vec::new();
        for _ in 0..100 {
            let img = renderer
                .render_tapped(&mut tap, &frame, DT)
                .expect("render_tapped on a headless renderer");
            settled.push(mean_luma(&img));
        }
        renderer.set_presets(reload);
        let after = mean_luma(
            &renderer
                .render_tapped(&mut tap, &frame, DT)
                .expect("render_tapped on a headless renderer"),
        );
        let last = *settled.last()?;
        // The step the run was already taking, measured over the frames just
        // before the reload rather than assumed — the scene animates too, and
        // its own motion is part of what "unchanged" has to mean.
        let step = settled
            .windows(2)
            .rev()
            .take(20)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0.0_f64, f64::max);
        Some((last, after, step))
    }

    let Some((rebind_last, rebind_after, rebind_step)) = arm(vec![rising("probe")]) else {
        return;
    };
    let Some((replace_last, replace_after, _)) = arm(vec![rising("a_different_preset")]) else {
        return;
    };

    let rebind_jump = (rebind_after - rebind_last).abs();
    let replace_jump = (replace_after - replace_last).abs();
    eprintln!(
        "rebind: last {rebind_last:.4} after {rebind_after:.4} jump {rebind_jump:.4} \
         (ordinary step {rebind_step:.4})\n\
         replace: last {replace_last:.4} after {replace_after:.4} jump {replace_jump:.4}"
    );

    // The witness that the reset path is observable at all through this probe.
    // Without it a green run says nothing: an engine that reset nothing, and one
    // that reset everything on both paths, would both satisfy the first
    // assertion alone.
    assert!(
        replace_jump > 10.0 * rebind_step.max(f64::EPSILON),
        "a replacing reload did not snap the smoother measurably (jump \
         {replace_jump:.4} against an ordinary step of {rebind_step:.4}), so \
         this probe cannot tell a kept ease from a reset one"
    );
    assert!(
        rebind_jump <= 3.0 * rebind_step.max(f64::EPSILON),
        "a rebinding reload jumped {rebind_jump:.4}, more than three times the \
         {rebind_step:.4} the run was already moving by — the eased values did \
         not survive it"
    );
}
