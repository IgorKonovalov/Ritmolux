//! **Where a converted preset's geometry goes wrong** — three hunts that share a
//! statistic: the horizontal reflection seam (Plan 0108 Phase 3,
//! design-backlog 0107), the destroyed mirror of a negative scale (Plan 0109
//! Phase 1, design-backlog 0114), and the missing video echo (Plan 0109
//! Phase 3, design-backlog 0115). Each asks whether a picture is its own mirror
//! about one axis, so all three read `mirror_asymmetry`; the header below is the
//! first hunt's, and the other two are on their own tests.
//!
//! # The defect this is hunting
//!
//! Plan 0100 Phase 7 judged seven converted presets beside `foo_vis_milk2` and
//! found three of them mirroring content across a horizontal line with a bright
//! ragged boundary — *Contortion*'s split sphere, *Cauldron*'s flipped top band,
//! *Cosmic Dust 2*'s full-width false horizon. The backlog entry named `s_fw`'s
//! address mode as its first suspect; that suspect is **falsified before this
//! file existed**, because `warp_mesh/shader.rs` already builds `s_fw` with
//! `AddressMode::Repeat`, and Repeat shifts a copy rather than reflecting one.
//!
//! # Why this file is here and not in `core`
//!
//! The same seam `conformance.rs` and `draw_layer.rs` sit on: the hypothesis is
//! about what a *translated HLSL shader* means, and only this crate can turn
//! HLSL into WGSL. Asserted from `core` alone the fixture would be a
//! hand-written WGSL module, which pins the module rather than the translation
//! that produced it.
//!
//! # The hypothesis, with its arithmetic
//!
//! `shader/emit.rs`'s warp epilogue builds the polar pair as
//! `_lmv_p = (uv_orig - 0.5) * vec2<f32>(2.0, -2.0) * U.aspect.zw`, so
//! `p.y = -(uv_orig.y - 0.5) * 2 * ay` — texture space is y-down and the polar
//! pair is taken y-**up**. A preset that reconstructs `uv` from `ang`, the
//! `uv = 0.5 + 0.5 * float2(cos(ang), sin(ang)) * rad` idiom most tunnel presets
//! are made of, therefore recovers
//! `uv.y = 0.5 + 0.5 * p.y / ay = 0.5 - (uv_orig.y - 0.5)` — `uv_orig.y`
//! reflected about `0.5`, a mirror about the horizontal midline with its seam on
//! the fixed line, which is the reported fingerprint including the ragged edge.
//!
//! The negation is **deliberate** and is documented at
//! [`MilkRuntime::run_vertex`](lmv_core::milk::MilkRuntime::run_vertex): the EEL
//! per-vertex program's `rad`/`ang` are taken from the +y-up clip-space position
//! while its `x`/`y` inputs are y-down, and the draw layer reads the same y-down
//! convention for a shape's own `y` (`draw::uv_to_world`). So the engine carries
//! that asymmetry consistently across all three places. **Whether MilkDrop
//! carries it too is a question about the reference, not about this repository**
//! — which is what the test below can and cannot settle, stated in its own
//! header.

use lmv_core::dsp::AnalysisFrame;
use lmv_core::preset::Preset;
use lmv_core::render::{CaptureImage, HeadlessOptions, RenderError, Renderer};

/// Capture size, and deliberately **square**: at aspect 1 the polar round trip
/// is an exact vertical mirror rather than a mirror composed with an aspect
/// scale, so the measurement below is about the flip alone.
const SIZE: u32 = 96;
/// Enough frames for the shape to be deposited and the warp to have resampled
/// it several times. The mirror, if there is one, appears on the second frame.
const FRAMES: u32 = 12;

fn fixture_text(relative: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("milkconv has a workspace-root parent")
        .join(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("the fixture must be readable at {path:?}: {e}"))
}

/// Convert the fixture and load the emitted bundle back, exactly as `shot`
/// would — `draw_layer.rs`'s route.
fn preset(text: &str, name: &str) -> Preset {
    let file = milkconv::milk::parse(text).expect("the fixture parses as a .milk file");
    let converted = milkconv::convert::convert(&file, name).expect("it converts");
    let mut preset = Preset::from_toml_str(&converted.toml).unwrap_or_else(|e| {
        panic!(
            "the emitted bundle must load back: {e}\n---\n{}",
            converted.toml
        )
    });
    preset.name = name.to_string();
    preset
}

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

/// Which line the second frame is flipped about before the two are compared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Axis {
    /// Flip top for bottom — the `ang` round trip's own fingerprint.
    Horizontal,
    /// Flip left for right — what a negative `sx` produces (Plan 0109 Phase 1),
    /// and what `echo_orient = 1` produces (Phase 7).
    Vertical,
    /// No flip at all. Only useful with [`mirrored_distance`]'s two-image form,
    /// where it is the **non-vacuity arm**: "the echoed frame matches the
    /// mirrored control" says nothing unless the echoed frame also *fails* to
    /// match the unmirrored one.
    None,
}

/// **Mean absolute per-channel difference between `a` and `b` flipped about
/// `axis`**, on the stored-byte 0..1 scale `frame_diff` uses.
///
/// One statistic, three defects, because two copies of a statistic drift. Both
/// mirror hunts in this file pass the same image twice (see
/// [`mirror_asymmetry`]); Phase 7's echo test passes two different captures,
/// because the property there is not "this picture is symmetric" but "this
/// picture *is* that one, mirrored".
///
/// The images must be the same size; a mismatched pair reads out of `b` at `a`'s
/// coordinates and the missing bytes come back as zero, which shows up as a
/// large distance rather than as a silent pass.
fn mirrored_distance(a: &CaptureImage, b: &CaptureImage, axis: Axis) -> f32 {
    let (w, h) = (a.width.max(1), a.height.max(1));
    let mut sum = 0f64;
    let mut n = 0u64;
    let at = |img: &CaptureImage, x: u32, y: u32, c: u32| -> f64 {
        let i = ((y * img.width.max(1) + x) * 4 + c) as usize;
        f64::from(img.rgba.get(i).copied().unwrap_or(0))
    };
    for y in 0..h {
        for x in 0..w {
            let (mx, my) = match axis {
                Axis::Horizontal => (x, h - 1 - y),
                Axis::Vertical => (w - 1 - x, y),
                Axis::None => (x, y),
            };
            for c in 0..3 {
                sum += (at(a, x, y, c) - at(b, mx, my, c)).abs();
                n += 1;
            }
        }
    }
    if n == 0 {
        0.0
    } else {
        (sum / n as f64 / 255.0) as f32
    }
}

/// **How far the frame is from being its own mirror** about `axis` — the
/// one-image case of [`mirrored_distance`].
///
/// Zero means the picture is exactly symmetric across that line. The statistic
/// is the defect's own fingerprint rather than a proxy for it — a share-of-light
/// measure was tried first and could not separate the two arms, because the
/// composite carries a soft glow that fills both halves whatever the geometry
/// does.
fn mirror_asymmetry(image: &CaptureImage, axis: Axis) -> f32 {
    mirrored_distance(image, image, axis)
}

/// **The `ang` round trip reflects content about the horizontal midline** — the
/// Phase 3 reproduction, on a committed minimal fixture.
///
/// # What this settles, and what it does not
///
/// It settles the **mechanism**: a converted warp shader that rebuilds `uv` from
/// `ang` does not get the identity out of this engine, it gets a vertical
/// mirror, and the mirror comes from the sign in `emit.rs`'s `_lmv_p` rather
/// than from a sampler address mode. That is the reported fingerprint, produced
/// on demand from eleven lines of HLSL.
///
/// It does **not** settle whether that is a defect, and **the fix the hypothesis
/// suggests is deliberately not applied here.** Three things stand in the way,
/// and the third is the one that decides it:
///
/// 1. The engine reproduces the same y-down/y-up asymmetry in all three places
///    it reads MilkDrop geometry — `run_vertex`, `draw::uv_to_world`, and this
///    epilogue — so if the reference carries the asymmetry too, this mirror is
///    faithful and design-backlog 0107's seam has some other cause.
/// 2. Plan 0100 established the per-vertex convention on purpose, and
///    `run_vertex`'s own doc states it: the reference's per-vertex space is
///    +y up while its `x`/`y` inputs are y-down.
/// 3. **The two polar pairs agree today, and in MilkDrop they are the same
///    numbers.** `emit.rs`'s `_lmv_p` is
///    `(uv_orig - 0.5) * (2, -2) * U.aspect.zw`, and with
///    `fill_uniform`'s `aspect.zw = (1, 1/aspect)` at `aspect >= 1` that is
///    `(nx, ny/aspect)` — exactly `run_vertex`'s `(px, py)`. In the reference
///    `rad`/`ang` reach a pixel shader as *interpolated per-vertex attributes*,
///    the very numbers the per-vertex program saw, so a fix that flipped the
///    fragment's sign alone would put the engine in a state the reference cannot
///    be in. Flipping both is a much larger claim than this seam — it moves
///    every converted preset's geometry — and it needs the reference on screen
///    beside `lmv.exe`, which is Plan 0108's Phase 6.
///
/// So this test pins the mechanism and holds the sign still. Making its mirror
/// go away by editing `emit.rs` would be tuning the engine to a guess.
///
/// The control is the same file with one token changed — `uv2 = uv` instead of
/// the reconstruction — so the two renders differ in the round trip and in
/// nothing else: same shape, same decay, same deposit, same frame count.
#[test]
fn the_ang_round_trip_reflects_about_the_horizontal_midline() {
    let Some(mut renderer) = headless() else {
        return;
    };
    let text = fixture_text("core/tests/fixtures/scratch-0108/ang-roundtrip.milk");
    let control_text = text.replace("float2 uv2 = 0.5 + 0.5*d;", "float2 uv2 = uv;");
    assert_ne!(
        control_text, text,
        "the control substitution found nothing to replace — the fixture's \
         reconstruction line was edited without updating this test"
    );

    let round_trip = preset(&text, "ang-round-trip");
    let control = preset(&control_text, "uv-direct");
    renderer.set_presets(vec![round_trip, control]);

    let frame = AnalysisFrame::default();
    let a = renderer
        .capture_preset("ang-round-trip", &frame, FRAMES)
        .expect("capture the round trip");
    let b = renderer
        .capture_preset("uv-direct", &frame, FRAMES)
        .expect("capture the control");

    let (mirrored, direct) = (
        mirror_asymmetry(&a, Axis::Horizontal),
        mirror_asymmetry(&b, Axis::Horizontal),
    );
    // **Measured 2026-08-17** on the development box (Windows 10, DX12 WARP),
    // 96x96 over 12 frames: round trip 0.0086, control 0.1605 — a factor of 19.
    // The vertical light profile behind those numbers, in eighths from the top:
    //   round trip  0.140 0.173 0.104 0.077 | 0.079 0.107 0.177 0.144
    //   control     0.105 0.105 0.073 0.053 | 0.104 0.144 0.225 0.190
    // The first is its own reflection to three decimals; the second is
    // bottom-weighted, which is where the fixture's one shape actually is.
    println!(
        "[warp_geometry] distance from being its own vertical mirror — \
         `uv` rebuilt from `ang`: {mirrored:.4}, sampled directly: {direct:.4}"
    );

    // Non-vacuity first: the control must be visibly one-sided, or "the round
    // trip is more symmetric than the control" compares two piles of noise.
    assert!(
        direct > 0.05,
        "the CONTROL is already close to symmetric ({direct:.4}), so it cannot \
         show a mirror by contrast. The fixture's only light source is a shape \
         at y = 0.82 — if a direct `uv` sample renders symmetrically, the shape \
         is not being drawn where the fixture says"
    );
    assert!(
        mirrored < direct * 0.25,
        "the `ang` round trip did NOT reflect: it is {mirrored:.4} from its own \
         vertical mirror against the control's {direct:.4}. That rules the \
         hypothesis out and leaves design-backlog 0107's seam unexplained, \
         alongside the already-falsified `s_fw` address mode — record it, and \
         read `emit.rs`'s `_lmv_p` against the reference's own varyings before \
         changing anything"
    );
}

/// **A negative `sx` mirrors the past instead of collapsing it** — Plan 0109
/// Phase 1, design-backlog 0114.
///
/// # The defect
///
/// A MilkDrop scale is a per-frame factor, so the mesh vertex stage raises it to
/// `dt`. `pow()` of a negative base is undefined, and the stage guarded that with
/// `pow(max(v, 1e-4), dt)` — which does not fail, it *silently substitutes*. A
/// negative scale is MilkDrop's standard mirror idiom (363 corpus files, 3.5 %),
/// and every one of them was getting a near-zero positive scale instead: not a
/// flip but a collapse, the past resampled from a vanishing window.
/// *chasers 19 Portal*'s `per_pixel_3 = sx = -zm` is exactly this, and Plan 0108
/// Phase 5 attributed its missing fold to something else entirely.
///
/// # What the two arms are
///
/// One fixture, one token apart. Both deposit the same off-centre shape at
/// `x = 0.22` and hold the past with the same decay; they differ only in the
/// sign of `sx`. With the sign carried through, the flip is applied on every
/// frame, so the deposit's mirror at `x = 0.78` accumulates beside it and the
/// picture becomes its own left-right mirror. With the sign destroyed — the old
/// behaviour, and the control's behaviour — the light stays on the side it was
/// deposited.
///
/// The measurement is `mirror_asymmetry` about the **vertical** axis, the same
/// statistic the seam test uses about the horizontal one.
#[test]
fn a_negative_scale_mirrors_rather_than_collapsing() {
    let Some(mut renderer) = headless() else {
        return;
    };
    let text = fixture_text("core/tests/fixtures/scratch-0109/negative-scale.milk");
    let control_text = text.replace("per_pixel_2=sx = -zoom;", "per_pixel_2=sx = zoom;");
    assert_ne!(
        control_text, text,
        "the control substitution found nothing to replace — the fixture's `sx` \
         line was edited without updating this test"
    );

    let mirrored_preset = preset(&text, "negative-scale");
    let control = preset(&control_text, "positive-scale");
    renderer.set_presets(vec![mirrored_preset, control]);

    let frame = AnalysisFrame::default();
    let a = renderer
        .capture_preset("negative-scale", &frame, FRAMES)
        .expect("capture the negative scale");
    let b = renderer
        .capture_preset("positive-scale", &frame, FRAMES)
        .expect("capture the control");

    let (flipped, held) = (
        mirror_asymmetry(&a, Axis::Vertical),
        mirror_asymmetry(&b, Axis::Vertical),
    );
    // **Measured 2026-08-19** on the development box (Windows 10, DX12 WARP),
    // 96x96 over 12 frames: `sx = -zoom` 0.0047, `sx = zoom` 0.1055 - a factor
    // of 22. Before the fix the two arms read 0.1055 and 0.1055, identical to
    // four places, which is the signature of a sign that never arrived rather
    // than one that arrived and was clamped.
    println!(
        "[warp_geometry] distance from being its own left-right mirror — \
         `sx = -zoom`: {flipped:.4}, `sx = zoom`: {held:.4}"
    );

    // Non-vacuity first: the control must be visibly one-sided, or "the negative
    // arm is more symmetric than the control" compares two piles of noise.
    assert!(
        held > 0.05,
        "the CONTROL is already close to symmetric ({held:.4}), so it cannot \
         show a mirror by contrast. The fixture's one shape sits at x = 0.22 — \
         if a positive `sx` renders symmetrically, the shape is not being drawn \
         where the fixture says"
    );
    assert!(
        flipped < held * 0.25,
        "`sx = -zoom` did NOT mirror: it is {flipped:.4} from its own left-right \
         mirror against the control's {held:.4}. The sign is being destroyed \
         somewhere between the per-vertex program and the mesh vertex stage — \
         see `signed_rate` in `warp_mesh/mod.rs`"
    );
}

/// **At full alpha the frame IS its own transformed copy** — Plan 0109 Phase 7,
/// ADR-0119, superseding the property Phase 3 shipped.
///
/// # What the stage is
///
/// MilkDrop's composite draws the finished frame twice: once straight, once
/// zoomed about the centre and flipped per `nVideoEchoOrientation`, at
/// `fVideoEchoAlpha`. 2.4 % of the corpus sets a non-zero alpha and where it is
/// set it is load-bearing. This engine had no such stage until Phase 3, and the
/// converter used to name the three values as unconsumed.
///
/// # Why the property changed
///
/// Phase 3 asserted that a flip-x at `alpha = 1` renders **left-right
/// symmetric**. That is a consequence of a *sum*, and it is weak — a uniform
/// grey frame satisfies it too. The Phase 5 look gate then convicted the sum
/// itself: at the authored `alpha = 1.000` it turned *Songflower*'s crisp
/// lattice pale, and the observation Phase 3 had reasoned from (that only one
/// family of bars survives without the echo) did not reproduce. ADR-0119 makes
/// the blend a `mix`.
///
/// Under a `mix` the stage can be pinned **exactly** rather than by a symptom.
/// The echo samples the same field this pass is already reading and writes
/// nothing back, so the field evolves identically in both arms; at `alpha = 1`,
/// `echo_zoom = 1`, `echo_orient = 1` the echoed frame therefore *is* the
/// horizontal mirror of the `alpha = 0` control, frame for frame. Nothing but
/// the intended transform satisfies that — which is the whole gain over the
/// symmetry statistic, and is why this test asserts a distance between two
/// captures rather than a property of one.
///
/// This also still walks the converter, so it pins the half that is not the
/// shader: three outputs once warned about as unconsumed are seeded into the
/// bundle and read by the scene.
#[test]
fn at_full_alpha_the_echo_is_the_mirror_of_the_control() {
    let Some(mut renderer) = headless() else {
        return;
    };
    let text = fixture_text("core/tests/fixtures/scratch-0109/video-echo.milk");
    let control_text = text.replace("fVideoEchoAlpha=1.000", "fVideoEchoAlpha=0.000");
    assert_ne!(
        control_text, text,
        "the control substitution found nothing to replace — the fixture's echo \
         alpha was edited without updating this test"
    );

    let echoed = preset(&text, "video-echo");
    let control = preset(&control_text, "no-echo");
    renderer.set_presets(vec![echoed, control]);

    let frame = AnalysisFrame::default();
    let a = renderer
        .capture_preset("video-echo", &frame, FRAMES)
        .expect("capture the echo");
    let b = renderer
        .capture_preset("no-echo", &frame, FRAMES)
        .expect("capture the control");

    // The claim, and its own non-vacuity arm: `a` must match `b` MIRRORED, and
    // must NOT match `b` as it stands. Without the second, a stage that ignored
    // `echo_orient` entirely — or ignored the echo entirely — would pass.
    let mirrored = mirrored_distance(&a, &b, Axis::Vertical);
    let direct = mirrored_distance(&a, &b, Axis::None);
    println!(
        "[warp_geometry] `echo_alpha = 1` against the control — mirrored: \
         {mirrored:.4}, unmirrored: {direct:.4}"
    );

    assert!(
        direct > 0.05,
        "the echoed frame already matches the UNMIRRORED control ({direct:.4}), \
         so the echo is not reaching the screen at all and the mirrored \
         comparison below would pass on a stage that does nothing. The \
         fixture's one shape sits at x = 0.22, off centre on purpose"
    );
    assert!(
        mirrored < direct * 0.25,
        "at `echo_alpha = 1`, `echo_orient = 1`, `echo_zoom = 1` the frame must \
         BE the control mirrored left-right — it is {mirrored:.4} from it, \
         against {direct:.4} from the unmirrored control. Either the blend is \
         not a `mix` (ADR-0119), or the flip/zoom is not the identity transform \
         the fixture asks for"
    );
}
