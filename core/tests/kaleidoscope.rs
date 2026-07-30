//! The kaleidoscope fold does not tear at a fractional order (Plan 0049 Phase 1).
//!
//! The fold wraps `a = atan2(p.y, p.x) + angle` with `a - seg * floor(a / seg)`,
//! `seg = 2*pi/order`, then mirrors within the wedge. `atan2`'s branch cut lies on
//! the **-x ray**, where the angle jumps by exactly `2*pi`. The wrap-and-mirror is
//! periodic in `seg`, so it absorbs that jump only when `2*pi` is a whole multiple
//! of `seg` — only, that is, when `order` is an integer. Before Phase 1 the CPU
//! side clamped the order and never rounded it, so a fractional order tore the
//! frame along one horizontal ray from the centre to the left edge.
//!
//! # `kaleido_angle` must be non-zero or there is nothing to see
//!
//! **This is the trap in testing it, and it cost a first draft of this file.** The
//! mirrored wrap is an *even* function of `a`. At `kaleido_angle = 0` the two rows
//! straddling the ray have `a = ±(pi - d)`, evenness maps them to the same folded
//! angle, and the `2*pi` jump cancels **exactly** — at every order, integral or
//! not. A capture at angle 0 therefore reads a perfect zero seam whether or not
//! the bug is present, which is a green test that proves nothing.
//!
//! Rotate the fold and the cancellation goes: the two rows are then `a = angle ±
//! (pi - d)`, no longer negatives of each other, and they land half a wedge apart.
//! That is not a contrived configuration — **9 of the 11 shipped kaleido presets
//! drive `kaleido_angle = "time * k"`**, so the angle is non-zero on all but a
//! measure-zero set of frames. (The two that pin it at 0, `lsystem_arrowhead` and
//! `reaction_reef`, are genuinely immune, `reaction_reef` despite easing its order
//! through fractional values the whole time.) So this file captures at a fixed
//! non-zero angle, and **setting `ANGLE` back to 0 silently retires the test**.
//!
//! # What is asserted
//!
//! - **The property** — pixel rows immediately above and below the horizontal
//!   midline, over the left half, must agree across the -x ray about as well at a
//!   fractional order as at the integers either side of it. Measured on the
//!   fixture, the fractional order read ~3x the controls before the fix.
//! - **The mechanism** — a fractional order must render byte-identically to the
//!   integer it rounds to. Exact, so it cannot decay into vacuity the way a
//!   tolerance can.
//!
//! A separate test binary rather than more arms of `composite.rs`, following that
//! file's posture and `ink.rs`'s: one file, one process, so the fold pipelines
//! these captures build never coexist with another stage's on the WARP software
//! rasterizer. Skips with no adapter per ADR-0016.
//!
//! The fixture is the `parametric_curve` golden rose. A dense line web is the
//! input this needs: the tear is a half-wedge jump in the **sampled angle**, which
//! is only visible where the source has angular structure at that radius. A
//! radially symmetric backdrop would fold into itself and hide the defect
//! completely.

use lmv_core::dsp::AnalysisFrame;
use lmv_core::preset::Preset;
use lmv_core::render::{CaptureImage, HeadlessOptions, RenderError, Renderer};

/// Capture size. **Height must stay even** — that is what puts `uv.y = 0.5`, the
/// -x ray, exactly between two pixel rows so the pair straddles the branch cut.
const WIDTH: u32 = 256;
const HEIGHT: u32 = 192;
/// Enough frames for the static rose to be fully drawn; the fixture is
/// time-independent, so this only has to clear the draw-in.
const FRAMES: u32 = 30;

/// The frozen fixture, reused verbatim from the golden roster. Its `[params]`
/// table is last, so appended `kaleido_*` lines land inside it.
const FIXTURE: &str = include_str!("fixtures/parametric_curve.toml");
const FIXTURE_NAME: &str = "fixture_parametric_curve";

/// The fold rotation every capture is taken at. **Must not be zero** — see the
/// module docs. The exact value is arbitrary; it only has to be off the axis.
const ANGLE: f32 = 0.37;

/// The stretch of the midline the seam is measured over, as a fraction of the
/// half-width, out from the centre.
///
/// The inner cut skips the fold's convergence point, where every wedge meets and
/// adjacent rows differ sharply at any order. The outer cut is where the rose
/// ends: past it, both rows sample past the figure and read black, so a tear
/// there is invisible and would only dilute the measurement.
const INNER: f32 = 0.10;
const OUTER: f32 = 0.60;

/// The orders captured: the fractional one under test, bracketed by the integers
/// either side of it. The integers are the control — the same measurement on the
/// same pixels, differing only in the thing being tested.
const FRACTIONAL: f32 = 12.5;
const CONTROL_LO: f32 = 12.0;
const CONTROL_HI: f32 = 13.0;

/// How much worse than the worse control the fractional order's seam may read.
///
/// Sized from measurement, not taste. On this fixture the controls read ~35 and
/// ~32 per-channel bytes (a thin bright web on black has a steep one-row
/// gradient of its own) and the torn fractional order read ~97. A bound of 1.5x
/// sits at ~52: comfortably above the ~32 a fixed fold produces and comfortably
/// below the ~97 a torn one does.
const SEAM_TOL: f32 = 1.5;

fn fixture_with(extra: &str) -> Preset {
    let toml = format!("{FIXTURE}{extra}");
    Preset::from_toml_str(&toml).unwrap_or_else(|e| panic!("kaleido fixture parses: {e}"))
}

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

/// Mean per-channel difference (0..255) between the two pixel rows immediately
/// above and below the horizontal midline, over the left half's measured stretch.
///
/// With no tear this is the image's ordinary one-row vertical gradient. Across
/// the branch cut of a rotated fractional-order fold, the two rows are sampled
/// half a wedge apart in angle and the figure under them is simply different.
fn midline_seam(img: &CaptureImage) -> f32 {
    let (w, h) = (img.width as usize, img.height as usize);
    assert!(h % 2 == 0, "HEIGHT must be even for the rows to straddle");
    let (above, below) = (h / 2 - 1, h / 2);
    let half = (w / 2) as f32;
    // Radius fraction f sits at x = half * (1 - f), so the outer cut is the lower
    // column index.
    let lo = (half * (1.0 - OUTER)) as usize;
    let hi = (half * (1.0 - INNER)) as usize;
    assert!(lo < hi, "the measured stretch is empty");

    let mut sum = 0.0f64;
    let mut n = 0usize;
    for x in lo..hi {
        for c in 0..3 {
            let a = img.rgba[(above * w + x) * 4 + c];
            let b = img.rgba[(below * w + x) * 4 + c];
            sum += a.abs_diff(b) as f64;
            n += 1;
        }
    }
    (sum / n.max(1) as f64) as f32
}

#[test]
fn fractional_fold_order_does_not_tear_the_minus_x_ray() {
    let Some(mut renderer) = headless() else {
        return;
    };
    let frame = AnalysisFrame {
        bass: 0.6,
        mid: 0.5,
        treb: 0.6,
        onset: 0.4,
        bar: 0.25,
        ..Default::default()
    };
    let mut capture = |order: f32| {
        let extra = format!("kaleido_order = \"{order}\"\nkaleido_angle = \"{ANGLE}\"\n");
        renderer.set_presets(vec![fixture_with(&extra)]);
        renderer
            .capture_preset(FIXTURE_NAME, &frame, FRAMES)
            .unwrap_or_else(|e| panic!("capture kaleido fixture at order {order}: {e}"))
    };

    let lo = capture(CONTROL_LO);
    let frac = capture(FRACTIONAL);
    let hi = capture(CONTROL_HI);

    let (seam_lo, seam_frac, seam_hi) = (midline_seam(&lo), midline_seam(&frac), midline_seam(&hi));
    println!(
        "midline seam: order {CONTROL_LO} {seam_lo:.2} | order {FRACTIONAL} {seam_frac:.2} \
         | order {CONTROL_HI} {seam_hi:.2} (per-channel bytes)"
    );

    // --- The property: no discontinuity across the -x ray. ---
    let bound = seam_lo.max(seam_hi) * SEAM_TOL;
    assert!(
        seam_frac <= bound,
        "order {FRACTIONAL} tears across the -x ray: midline seam {seam_frac:.2} exceeds \
         {bound:.2} ({SEAM_TOL}x the worse of the integer controls {seam_lo:.2} / {seam_hi:.2})"
    );

    // --- The mechanism: the uniform only ever carries an integral order, so a
    // fractional one is indistinguishable from the integer it rounds to. 12.5
    // rounds away from zero, to 13. ---
    assert_eq!(
        frac.rgba, hi.rgba,
        "order {FRACTIONAL} did not render identically to order {CONTROL_HI}: the fold is \
         still being handed a fractional wedge count"
    );

    // --- And the controls must not be the same picture, or the check above is
    // satisfied by a fold that does nothing at all. ---
    assert_ne!(
        lo.rgba, hi.rgba,
        "orders {CONTROL_LO} and {CONTROL_HI} render identically — the fixture is not \
         exercising the fold, so nothing above proves anything"
    );
}
