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

use lmv_core::dsp::AnalysisFrame;
use lmv_core::preset::Preset;
use lmv_core::render::{CaptureImage, HeadlessOptions, RenderError, Renderer};

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

/// A headless renderer on the **software** adapter, or `None` (a logged skip)
/// when the runner exposes no GPU adapter — macOS has no software Metal fallback
/// (ADR-0016). WARP for `composite.rs`'s reason: a guard whose failure mode is
/// "nobody looked" has to run in CI.
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

/// The fixed frame every fixture is driven by — mid-energy, all bands lit. The
/// fixtures bind nothing to it; it exists so the scene draws.
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
    let stimulus = vec![fixed_frame(); frames];
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
    let Some(mut renderer) = headless() else {
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
    let Some(mut renderer) = headless() else {
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
    let Some(mut renderer) = headless() else {
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
    let Some(mut renderer) = headless() else {
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
