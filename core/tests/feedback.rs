//! Transformed feedback: what the `fb_*` params actually do to the accumulation
//! (Plan 0046, ADR-0048).
//!
//! # Why this is its own binary, and why it pins no baseline
//!
//! `composite.rs` exists in the shape it does because building GPU resources
//! mid-run changes what the trails stage resolves to on the DX12 WARP software
//! adapter (ADR-0058), and its seven baselines must not be exposed to that. These
//! guards need a **portrait** render target and several consecutive multi-frame
//! runs, so putting them in that file would put exactly the perturbation its
//! module docs warn about into the same process as those baselines. They live
//! here instead, and they assert **relative** facts — how one frame of a run
//! differs from a later frame of the same run — which needs no committed PNG and
//! survives any cross-adapter rasterization difference.
//!
//! # Portrait, deliberately
//!
//! 100x160. ADR-0047's lesson is that an edge or aspect policy only shows itself
//! off 16:9, and the post stages round each grid axis up to a 256 px step — so
//! this target renders through a **256x256** accumulation whose aspect is 1.0
//! against the target's 0.625. A transform that took its shape from that grid
//! rather than from the render target (ADR-0037) would stretch every rotation by
//! 1.6, which [`a_rotated_accumulation_stays_round`] measures directly.
//!
//! # How a motion fixture asserts displacement
//!
//! The figure in every fixture here is **static** — a small Maurer rose parked
//! off-centre with `spin = 0` — so the accumulation can only move if the feedback
//! transform moves it. Each guard captures one run through
//! [`Renderer::capture_preset_over`] and compares an early frame against a late
//! one in **polar coordinates about the frame centre, in pixels**:
//!
//! - `radial_extent` — how far the lit set spans in radius;
//! - `angular_extent` — how far it spans in angle, measured as `2*PI` minus the
//!   widest empty gap, so a set straddling the `atan2` branch cut is still one arc.
//!
//! `fb_zoom` must grow the first faster than the second, `fb_rotate` the reverse.
//! Both assertions are ratios of one run against itself, so there is no absolute
//! pixel count to re-tune when a shader changes by a byte.

use std::f32::consts::{PI, TAU};

use rlx_core::preset::Preset;
use rlx_core::render::{CaptureImage, Renderer, metrics::frame_diff};

mod common;

/// Portrait, and not a 256 px multiple — see the module docs. **Not
/// interchangeable**: a square or 16:9 target defeats the shear guard entirely.
const WIDTH: u32 = 100;
const HEIGHT: u32 = 160;

/// Frames the motion guards run for, and the two they read.
///
/// `EARLY` is late enough that the trail has a few frames of history to displace
/// (comparing against frame 0, which is one deposit and no trail at all, would
/// measure the figure rather than the feedback).
const FRAMES: usize = 46;
const EARLY: usize = 6;
const LATE: usize = FRAMES - 1;

/// A pixel counts as lit at this fraction of the frame's own peak luminance.
///
/// Relative to the frame, not an absolute byte: the tail of a decaying trail is
/// dim by construction, and what "dim" means in bytes depends on the tonemap, the
/// palette and the adapter. What the guards need is "the region the accumulation
/// reaches", and a fixed fraction of the peak is that on any of them.
const LIT_FRACTION: f32 = 0.12;

/// Frames the additive-convergence guard runs for.
///
/// At the `MAX_FADE = 0.98` ceiling the accumulation is within `0.98^n` of its
/// limit, so 360 frames leaves 0.07 % of the gap — and the per-frame *change* at
/// that point is well under the precision of an 8-bit readback, which is what
/// makes "it stopped moving" a readable fact rather than a tolerance. At 240 the
/// residue was still worth ~2 bytes over the final window, which is convergent and
/// also indistinguishable from a slow leak.
const CONVERGE_FRAMES: usize = 360;
/// The window, in frames, each end of the convergence guard measures over.
const CONVERGE_WINDOW: usize = 60;

/// Frames the attractor's second-sink guards run for — enough that its trail
/// field carries real history for `fb_rotate` to sweep, and short enough that the
/// per-frame readback stays cheap.
const ATTRACTOR_FRAMES: usize = 90;

const STILL: (&str, &str) = (
    "fixture_feedback_still",
    include_str!("fixtures/feedback_still.toml"),
);
const IDENTITY: (&str, &str) = (
    "fixture_feedback_identity",
    include_str!("fixtures/feedback_identity.toml"),
);
const ZOOM: (&str, &str) = (
    "fixture_feedback_zoom",
    include_str!("fixtures/feedback_zoom.toml"),
);
const ROTATE: (&str, &str) = (
    "fixture_feedback_rotate",
    include_str!("fixtures/feedback_rotate.toml"),
);
const RING: (&str, &str) = (
    "fixture_feedback_ring",
    include_str!("fixtures/feedback_ring.toml"),
);
const ADD: (&str, &str) = (
    "fixture_feedback_add",
    include_str!("fixtures/feedback_add.toml"),
);
const MAX: (&str, &str) = (
    "fixture_feedback_max",
    include_str!("fixtures/feedback_max.toml"),
);

/// The un-warped control the three warp fixtures are each a one-key edit of.
/// Shared with `composite.rs`, which pins its baseline — here it is read only as
/// the "before" of a controlled comparison.
const WARP_CONTROL: (&str, &str) = (
    "fixture_composite_trails",
    include_str!("fixtures/composite_trails.toml"),
);
/// The `[feedback] warp` roster, as fixtures. Each differs from
/// [`WARP_CONTROL`] in exactly two keys — the kind and its strength.
const WARPS: [(&str, &str); 3] = [
    (
        "fixture_composite_warp_swirl",
        include_str!("fixtures/composite_warp_swirl.toml"),
    ),
    (
        "fixture_composite_warp_ripple",
        include_str!("fixtures/composite_warp_ripple.toml"),
    ),
    (
        "fixture_composite_warp_fisheye",
        include_str!("fixtures/composite_warp_fisheye.toml"),
    ),
];

/// The **second sink** (ADR-0048, Plan 0046 Phase 3): the attractor's own trail
/// field. Each of the four differs from its control in exactly one key.
const ATTRACTOR_CONTROL: (&str, &str) = (
    "fixture_attractor_fb_control",
    include_str!("fixtures/attractor_fb_control.toml"),
);
const ATTRACTOR_ROTATE: (&str, &str) = (
    "fixture_attractor_fb_rotate",
    include_str!("fixtures/attractor_fb_rotate.toml"),
);
const ATTRACTOR_TRAILS_CONTROL: (&str, &str) = (
    "fixture_attractor_trails_control",
    include_str!("fixtures/attractor_trails_control.toml"),
);
const ATTRACTOR_BOTH: (&str, &str) = (
    "fixture_attractor_fb_both",
    include_str!("fixtures/attractor_fb_both.toml"),
);

/// How far apart two renders of the same figure must be before this file will
/// call them different pictures — mean per-channel difference, 0..1.
///
/// `composite.rs` tolerates drift up to `0.02` before it calls a baseline moved,
/// so anything at or below that is inside the noise it already accepts. This sits
/// well above it: a warp that only cleared this bar by a hair would be a warp
/// nobody could see.
const DISTINCT_MEAN: f32 = 0.05;

/// Load `fixture` into `renderer` and run it for `frames` steps, returning every
/// rendered frame in order.
fn run(renderer: &mut Renderer, fixture: (&str, &str), frames: usize) -> Vec<CaptureImage> {
    let (name, toml) = fixture;
    let preset = Preset::from_toml_str(toml)
        .unwrap_or_else(|e| panic!("fixture {name}.toml is invalid: {e}"));
    assert_eq!(preset.name, name, "fixture name must match its const");
    assert!(
        preset.warnings.is_empty(),
        "fixture {name} loaded with warnings: {:?}",
        preset.warnings
    );
    renderer.set_presets(vec![preset]);
    let stimulus = vec![common::fixed_frame(); frames];
    renderer
        .capture_preset_over(name, &stimulus)
        .unwrap_or_else(|e| panic!("capture {name}: {e}"))
}

/// Every lit pixel of `img` as `(radius, angle)` about the frame centre, in
/// **pixels** — the space the transform claims to be isotropic in.
fn lit_polar(img: &CaptureImage) -> Vec<(f32, f32)> {
    let peak = img
        .rgba
        .chunks_exact(4)
        .map(|px| px.iter().take(3).copied().max().unwrap_or(0))
        .max()
        .unwrap_or(0);
    let threshold = (peak as f32 * LIT_FRACTION).ceil() as u8;
    let cx = img.width as f32 / 2.0;
    let cy = img.height as f32 / 2.0;
    let mut out = Vec::new();
    for (index, px) in img.rgba.chunks_exact(4).enumerate() {
        let bright = px.iter().take(3).copied().max().unwrap_or(0);
        if bright < threshold.max(1) {
            continue;
        }
        let x = (index as u32 % img.width) as f32 + 0.5 - cx;
        let y = (index as u32 / img.width) as f32 + 0.5 - cy;
        out.push((x.hypot(y), y.atan2(x)));
    }
    out
}

/// How far a lit set spans in radius (pixels).
fn radial_extent(polar: &[(f32, f32)]) -> f32 {
    let (min, max) = polar.iter().fold((f32::MAX, 0.0f32), |(lo, hi), &(r, _)| {
        (lo.min(r), hi.max(r))
    });
    (max - min).max(0.0)
}

/// How far a lit set spans in angle (radians), measured the only way that is
/// correct across the `atan2` branch cut: sort the angles, find the **widest gap**
/// between consecutive ones (treating the wrap as one more gap), and report what
/// is left of the circle. A set clustered around `+/-PI` therefore reads as one
/// narrow arc rather than as the whole circle.
fn angular_extent(polar: &[(f32, f32)]) -> f32 {
    let mut angles: Vec<f32> = polar.iter().map(|&(_, t)| t).collect();
    if angles.len() < 2 {
        return 0.0;
    }
    angles.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mut widest = 0.0f32;
    for pair in angles.windows(2) {
        if let [a, b] = pair {
            widest = widest.max(b - a);
        }
    }
    // The wrap-around gap, from the largest angle back to the smallest.
    if let (Some(&first), Some(&last)) = (angles.first(), angles.last()) {
        widest = widest.max(TAU - (last - first));
    }
    (TAU - widest).max(0.0)
}

/// The pixel bounding box of the lit set: `(width, height)`.
fn lit_bbox(img: &CaptureImage) -> (f32, f32) {
    let peak = img
        .rgba
        .chunks_exact(4)
        .map(|px| px.iter().take(3).copied().max().unwrap_or(0))
        .max()
        .unwrap_or(0);
    let threshold = ((peak as f32 * LIT_FRACTION).ceil() as u8).max(1);
    let (mut x0, mut x1, mut y0, mut y1) = (u32::MAX, 0u32, u32::MAX, 0u32);
    for (index, px) in img.rgba.chunks_exact(4).enumerate() {
        if px.iter().take(3).copied().max().unwrap_or(0) < threshold {
            continue;
        }
        let x = index as u32 % img.width;
        let y = index as u32 / img.width;
        x0 = x0.min(x);
        x1 = x1.max(x);
        y0 = y0.min(y);
        y1 = y1.max(y);
    }
    assert!(x0 <= x1 && y0 <= y1, "no lit pixel in the frame at all");
    ((x1 - x0 + 1) as f32, (y1 - y0 + 1) as f32)
}

/// **The identity claim, asserted rather than assumed** (ADR-0048): a preset that
/// binds all six `fb_*` to their documented defaults renders **byte-for-byte**
/// what the same preset renders binding none of them.
///
/// This is not a formality. The transform makes a uv isotropic by multiplying `x`
/// by the target aspect and divides it back on the way out, and `(x * a) / a` is
/// not `x` in `f32` — so a stage that simply *computed* the identity transform
/// would move most pixels by a fraction of a texel and drift every golden in the
/// suite. The shader `select`s the literal sample uv instead, and this is what
/// holds that select in place.
#[test]
fn every_fb_param_at_its_default_is_bit_exactly_no_transform() {
    let Some(mut renderer) = common::headless(WIDTH, HEIGHT) else {
        return;
    };
    let unbound = run(&mut renderer, STILL, FRAMES);
    let bound = run(&mut renderer, IDENTITY, FRAMES);
    assert_eq!(unbound.len(), bound.len());
    for (index, (a, b)) in unbound.iter().zip(bound.iter()).enumerate() {
        assert_eq!(
            a.rgba, b.rgba,
            "frame {index}: binding every fb_* to its default changed the picture. \
             The transform's identity must be the literal sample uv, not the \
             arithmetic identity — see `TRAILS_SHADER`'s `select`."
        );
    }
}

/// `fb_zoom` displaces the accumulation **radially**: the streak the static figure
/// leaves grows away from the frame centre, and grows in radius far faster than it
/// grows in angle.
///
/// Both numbers are ratios of one run against itself — late frame over early
/// frame — so nothing here is a pixel count anyone has to re-tune.
#[test]
fn fb_zoom_displaces_the_accumulation_radially() {
    let Some(mut renderer) = common::headless(WIDTH, HEIGHT) else {
        return;
    };
    let frames = run(&mut renderer, ZOOM, FRAMES);
    let (radial, angular) = growth(&frames);
    println!("fb_zoom: radial x{radial:.2}, angular x{angular:.2}");
    assert!(
        radial > 1.5,
        "fb_zoom = 1.9/s over {} frames barely moved the accumulation outward \
         (radial extent x{radial:.2})",
        LATE - EARLY
    );
    assert!(
        radial > angular * 1.5,
        "fb_zoom must grow the accumulation's RADIAL span faster than its angular \
         one — got radial x{radial:.2} against angular x{angular:.2}, which is a \
         tangential motion wearing a radial param's name"
    );
}

/// `fb_rotate` displaces the accumulation **tangentially**: the same static figure
/// leaves an arc at a constant radius. The mirror of the guard above, on the same
/// figure, so the two together say the affine's two axes are not transposed.
#[test]
fn fb_rotate_displaces_the_accumulation_tangentially() {
    let Some(mut renderer) = common::headless(WIDTH, HEIGHT) else {
        return;
    };
    let frames = run(&mut renderer, ROTATE, FRAMES);
    let (radial, angular) = growth(&frames);
    println!("fb_rotate: radial x{radial:.2}, angular x{angular:.2}");
    assert!(
        angular > 1.5,
        "fb_rotate = 3 rad/s over {} frames barely swept the accumulation around \
         (angular extent x{angular:.2})",
        LATE - EARLY
    );
    assert!(
        angular > radial * 1.5,
        "fb_rotate must grow the accumulation's ANGULAR span faster than its \
         radial one — got angular x{angular:.2} against radial x{radial:.2}"
    );
}

/// **A rotation is a rotation, not a shear** (ADR-0037): spun long enough to close
/// a full ring, the accumulation's pixel bounding box is *square* on a portrait
/// target.
///
/// The arithmetic behind that: the figure sits at a fixed distance from the frame
/// centre and the ring it traces is a circle **on screen**, so its box is as wide
/// as it is tall in pixels whatever the target's shape. A transform that rotated
/// in the accumulation grid's own uv — a 256x256 grid here, against a 100x160
/// target — would trace an ellipse whose box is `160/100 = 1.6x` taller than wide.
/// That factor is the third repeat of the defect ADR-0037 was written for, and
/// this is the assertion that would have caught the first two.
#[test]
fn a_rotated_accumulation_stays_round() {
    let Some(mut renderer) = common::headless(WIDTH, HEIGHT) else {
        return;
    };
    // Long enough at 9 rad/s to pass 2*PI and close the ring.
    let frames = run(&mut renderer, RING, 60);
    let last = frames.last().expect("60 frames captured");
    let polar = lit_polar(last);
    assert!(
        angular_extent(&polar) > PI * 1.5,
        "the fixture must close into a ring before its box says anything about \
         aspect — swept only {:.2} rad",
        angular_extent(&polar)
    );

    let (w, h) = lit_bbox(last);
    println!("ring bbox {w}x{h} at {WIDTH}x{HEIGHT}");
    let skew = (w - h).abs() / w.max(h);
    assert!(
        skew < 0.15,
        "the accumulated ring's bounding box is {w}x{h} — {:.0}% out of round. A \
         rotation must be computed in isotropic units taken from the RENDER \
         TARGET; taking the aspect from the 256 px-quantized accumulation grid \
         instead stretches it by {:.2}x (ADR-0037)",
        skew * 100.0,
        HEIGHT as f32 / WIDTH as f32
    );
}

/// **Each warp kind bends the past its own way** (ADR-0048's curated family).
///
/// Four renders of one figure that differ in a single structural key each. Two
/// things could go wrong silently and this catches both: a `warp` selector that
/// never reached the shader would make all three warps identical to the control,
/// and a shader that took the same arm for every kind would make them identical to
/// each other. The kinds are compared pairwise, not against pinned pixels —
/// `composite.rs` owns the drift baselines, this owns the claim that there are
/// three distinct behaviors to have baselines *of*.
#[test]
fn each_warp_kind_bends_the_past_its_own_way() {
    let Some(mut renderer) = common::headless(WIDTH, HEIGHT) else {
        return;
    };
    let control = last_frame(&mut renderer, WARP_CONTROL, 40);
    let mut rendered = Vec::new();
    for warp in WARPS {
        let frame = last_frame(&mut renderer, warp, 40);
        let against_control = frame_diff(&control, &frame);
        println!("{:<32} vs no warp: mean {against_control:.4}", warp.0);
        assert!(
            against_control > DISTINCT_MEAN,
            "{} renders the same picture as the un-warped control (mean \
             {against_control:.4}). Either the [feedback] warp key never reached \
             the stage, or fb_warp is not being applied.",
            warp.0
        );
        rendered.push((warp.0, frame));
    }
    for (index, (name_a, a)) in rendered.iter().enumerate() {
        for (name_b, b) in rendered.iter().skip(index + 1) {
            let mean = frame_diff(a, b);
            println!("{name_a:<32} vs {name_b}: mean {mean:.4}");
            assert!(
                mean > DISTINCT_MEAN,
                "{name_a} and {name_b} render the same picture (mean {mean:.4}) — \
                 the warp kinds are not distinct, so the shader is taking one arm \
                 for more than one selector"
            );
        }
    }
}

/// **The additive deposit is bounded** (ADR-0048): a static bright source under
/// `blend = "add"` at the `MAX_FADE` ceiling **converges** rather than growing
/// without bound.
///
/// The arithmetic says the accumulation is a geometric series summing to
/// `cur / (1 - fade)` — at most 50x here — and the guard reads that back as a
/// *fixed point*: over a long run the frame stops changing. Both halves are
/// asserted, because "stopped changing" is also what a broken stage that never
/// accumulated at all would say: the early window must move a lot and the late
/// window must not move at all.
///
/// The fixture's `brightness` is tiny on purpose, so the limit lands near 1.0
/// instead of far above the tonemap's white — see its header.
#[test]
fn an_additive_deposit_converges_instead_of_running_away() {
    let Some(mut renderer) = common::headless(WIDTH, HEIGHT) else {
        return;
    };
    let frames = run(&mut renderer, ADD, CONVERGE_FRAMES);

    let early = u32::from(max_channel_diff(
        frames.get(10).expect("frame 10"),
        frames.get(10 + CONVERGE_WINDOW).expect("early window end"),
    ));
    let late = u32::from(max_channel_diff(
        frames
            .get(CONVERGE_FRAMES - 1 - CONVERGE_WINDOW)
            .expect("late window start"),
        frames.get(CONVERGE_FRAMES - 1).expect("last frame"),
    ));
    println!("add: early window moved {early}, late window moved {late}");

    assert!(
        early > 8,
        "the additive accumulation never built up at all (the first \
         {CONVERGE_WINDOW} frames moved by {early}), so 'it converged' would be \
         vacuous"
    );
    // The ratio is the claim, not the absolute: an accumulation growing without
    // bound moves the late window by about what it moved the early one, whatever
    // the units. A converging one is orders below it — measured 97 against 0.
    assert!(
        late * 10 <= early,
        "the additive accumulation moved {late} over the last {CONVERGE_WINDOW} \
         frames of a {CONVERGE_FRAMES}-frame run against {early} over the first — \
         it is still climbing, not settling. `add` is only safe because its series \
         is bounded by 1/(1 - fade); if this fails, the deposit is not multiplying \
         the past by the decay before summing it"
    );
}

/// `blend = "add"` and `blend = "max"` are two different pictures. The guard above
/// asserts `add` converges — and so does `max`, trivially, so without this one a
/// blend selector that never reached the shader would pass it.
#[test]
fn the_deposit_blend_selector_reaches_the_shader() {
    let Some(mut renderer) = common::headless(WIDTH, HEIGHT) else {
        return;
    };
    let additive = last_frame(&mut renderer, ADD, 120);
    let maximum = last_frame(&mut renderer, MAX, 120);
    let mean = frame_diff(&additive, &maximum);
    println!("add vs max: mean {mean:.4}");
    assert!(
        mean > DISTINCT_MEAN,
        "`blend = \"add\"` and `blend = \"max\"` rendered the same frame (mean \
         {mean:.4}) from fixtures that differ in that one key"
    );
    // …and in the direction the arithmetic says: summing echoes is brighter than
    // taking their maximum, never dimmer.
    assert!(
        peak(&additive) > peak(&maximum),
        "summing the past cannot be dimmer than taking its maximum — got peak {} \
         under `add` against {} under `max`",
        peak(&additive),
        peak(&maximum)
    );
}

/// **The transform reaches the attractor's own accumulation, and only the past
/// in it** (ADR-0048's second sink).
///
/// Two halves, and the first is the interesting one. The rotating fixture binds
/// **no `trails`**, so the engine stage is off entirely and there is no second
/// buffer in the frame — anything that differs from the control moved inside the
/// scene's internal trail field.
///
/// The second half is Phase 3's "the fresh deposit does not lag", stated exactly:
/// on the **first** frame the field has just been cleared, so there is no past to
/// transform, and the two runs must be **byte-identical**. A stage that
/// transformed the whole pass rather than only the decayed bed would rotate that
/// first frame's freshly-projected points by one `dt` — 0.1 rad here — and this
/// is what refuses it.
#[test]
fn the_attractor_transforms_its_own_past_and_not_its_deposit() {
    let Some(mut renderer) = common::headless(WIDTH, HEIGHT) else {
        return;
    };
    let control = run(&mut renderer, ATTRACTOR_CONTROL, ATTRACTOR_FRAMES);
    let rotated = run(&mut renderer, ATTRACTOR_ROTATE, ATTRACTOR_FRAMES);

    let first_control = control.first().expect("first frame");
    let first_rotated = rotated.first().expect("first frame");
    assert_eq!(
        first_control.rgba, first_rotated.rgba,
        "the first frame of a cleared field has no past to transform, so \
         `fb_rotate` must change nothing in it. That it did means the transform \
         is reaching this frame's freshly-deposited points, not only the trail \
         they are drawn over."
    );

    let last_control = control.last().expect("last frame");
    let last_rotated = rotated.last().expect("last frame");
    let mean = frame_diff(last_control, last_rotated);
    println!("attractor own sink: mean {mean:.4} after {ATTRACTOR_FRAMES} frames");
    assert!(
        mean > DISTINCT_MEAN,
        "`fb_rotate` did not move the attractor's own trail field (mean \
         {mean:.4}). With no `trails` bound there is no engine accumulation in \
         this frame at all, so the scene is the only sink there is."
    );
}

/// **One binding, two buffers** — the routing contract with both sinks live
/// (ADR-0048).
///
/// An attractor preset with `trails` on has *two* accumulations in the frame, and
/// a single `fb_rotate` turns both. The guard is in two steps because the second
/// alone would not be a claim about two sinks at all:
///
/// 1. turning `trails` on **must change the picture**, or there is no engine
///    accumulation in the frame to speak of and the whole test is about the
///    attractor's field a second time. This is not hypothetical — over a
///    *stationary* figure `max(cur, prev * fade)` is exactly `cur`, and these
///    fixtures measured 0.000000 apart until they were given a `spin`;
/// 2. then, with both live, the single `fb_rotate` moves the frame.
///
/// `resolve_route`'s own guard in `render/tests.rs` pins the fan-out that delivers
/// the binding to both.
#[test]
fn one_binding_moves_both_accumulations() {
    let Some(mut renderer) = common::headless(WIDTH, HEIGHT) else {
        return;
    };
    let scene_only = last_frame(&mut renderer, ATTRACTOR_CONTROL, ATTRACTOR_FRAMES);
    let control = last_frame(&mut renderer, ATTRACTOR_TRAILS_CONTROL, ATTRACTOR_FRAMES);
    let both = last_frame(&mut renderer, ATTRACTOR_BOTH, ATTRACTOR_FRAMES);

    let stage_contributes = frame_diff(&scene_only, &control);
    println!("both sinks: `trails` alone moves the frame by {stage_contributes:.4}");
    assert!(
        stage_contributes > DISTINCT_MEAN,
        "turning `trails` on changed the frame by only {stage_contributes:.4}, so          there is no second accumulation in it and what follows would be a claim          about the attractor's own field twice over"
    );

    let mean = frame_diff(&control, &both);
    println!("both sinks: one `fb_rotate` moves it by {mean:.4}");
    assert!(
        mean > DISTINCT_MEAN,
        "one `fb_rotate` over an attractor with `trails` on must move both          accumulations; it moved the frame by {mean:.4}"
    );
}

/// The brightest channel anywhere in the frame.
fn peak(img: &CaptureImage) -> u8 {
    img.rgba
        .chunks_exact(4)
        .map(|px| px.iter().take(3).copied().max().unwrap_or(0))
        .max()
        .unwrap_or(0)
}

/// The largest single-channel (RGB) byte difference between two frames.
fn max_channel_diff(a: &CaptureImage, b: &CaptureImage) -> u8 {
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

/// The last frame of a `frames`-long run of `fixture`.
fn last_frame(renderer: &mut Renderer, fixture: (&str, &str), frames: usize) -> CaptureImage {
    run(renderer, fixture, frames)
        .pop()
        .expect("a run of at least one frame")
}

/// The `(radial, angular)` growth ratios of a run: the late frame's extent over
/// the early frame's, in each axis.
fn growth(frames: &[CaptureImage]) -> (f32, f32) {
    let early = lit_polar(frames.get(EARLY).expect("early frame captured"));
    let late = lit_polar(frames.get(LATE).expect("late frame captured"));
    assert!(
        !early.is_empty() && !late.is_empty(),
        "the fixture drew nothing — a growth ratio over an empty set is not a \
         measurement of anything"
    );
    (
        radial_extent(&late) / radial_extent(&early).max(f32::EPSILON),
        angular_extent(&late) / angular_extent(&early).max(f32::EPSILON),
    )
}
