//! Pixel-level properties of the kaleidoscope fold: it does not tear at a
//! fractional order (Plan 0049 Phase 1), it does not paint outside its disc
//! (Plan 0045 Phase 1 / [ADR-0047], design-backlog 0010), and — Plan 0045 Phase
//! 2b / [ADR-0055] — its falloff lands on the **backdrop** rather than on black,
//! while the backdrop itself stays out of the fold.
//!
//! The last two need a **lit** backdrop and say so in their own comments: every
//! fold fixture in the repository runs `bg_bright = 0`, and on a black backdrop a
//! falloff that fades to black is pixel-for-pixel a falloff that composites over
//! the backdrop. That configuration cannot tell the two apart, which is why the
//! defect survived sixteen confirmation captures.
//!
//! # The disc guard is a property of a **treatment**, not of the fold (ADR-0061)
//!
//! Since Plan 0055 Phase 1 the out-of-disc region is a per-preset choice —
//! `kaleido_edge` picks one of five treatments — and three of them (`mirror`,
//! `tile`, `squash`) paint out there **on purpose**. Read
//! [`the_fold_paints_nothing_outside_its_disc`] as a property of the default
//! treatment, `falloff`, and of `vignette`; it is not a rule about folding, and a
//! fill treatment tripping it is that treatment working. Every capture in this
//! file binds no `kaleido_edge`, so every one of them runs the default arm and the
//! assertions below are unchanged in meaning.
//!
//! Plan 0055 Phase 3 re-scopes the guard once the live A/B has said which
//! treatments survive, and gives each surviving fill treatment the property that
//! is true of *it* — that the out-of-disc region is covered, and that it is not
//! the radial smear of design-backlog 0010 (out-of-disc content must **vary along
//! a ray**; the smear was constant along one). That is the only guard `tile` gets,
//! because the disc guard cannot catch it: `tile` is supposed to paint out there,
//! and what makes it safe rather than a rerun of the original defect is that its
//! reads go through a `MirrorRepeat` sampler rather than the `ClampToEdge` one.
//!
//! # Why the disc guard lives here and not in `composite.rs`
//!
//! Plan 0045 Phase 1 lists `core/tests/composite.rs` for it. That file pins two
//! **blessed baselines** at a load-bearing 160x100, and its own module docs are
//! the record of why building GPU resources mid-run must not happen near them: a
//! second `Renderer::new_headless` is documented to change what the trails stage
//! resolves to on the WARP software adapter. This guard needs a *portrait* target
//! and a border-filling fixture, so it cannot share that renderer — it would have
//! to construct a second one in the same binary, beside the baselines, which is
//! the one thing that file exists to avoid. It is the same fold, so it joins the
//! fold's own binary instead. Nothing else about the assertion changes.
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
//! That is not a contrived configuration — **10 of the 12 shipped presets with an
//! active fold drive `kaleido_angle = "time * k"`** (a thirteenth, `swarm_dense`,
//! pins the order at 1 so the fold is off), so the angle is non-zero on all but a
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

fn headless(width: u32, height: u32) -> Option<Renderer> {
    match Renderer::new_headless(HeadlessOptions {
        width,
        height,
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
    let Some(mut renderer) = headless(WIDTH, HEIGHT) else {
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

// --- The fold's domain: nothing is drawn outside the inscribed disc. ---------
//
// design-backlog 0010, the guard that entry says is owed. The pinned fixture
// `composite_kaleido.png` cannot serve: measured at Plan 0035's close, the
// inscribed-disc fix leaves it **green** at 94 % of its drift budget, so it would
// not announce the fix and the next unrelated fold change would trip it blaming
// the wrong thing.

/// **Portrait, and that is the point.** The fold keeps each output pixel's radius
/// and only changes its angle, so what governs the defect is the ratio between the
/// frame's corner radius and its shortest half-extent — `sqrt(1 + (long/short)^2)`,
/// about 2.04 at 16:9 and 2.28 here. At 16:9 the out-of-range region is corner
/// debris; in a portrait window it is stripes across the whole picture, which is
/// where the user hit it. **A square or 16:9 size would weaken this test.**
const FIELD_W: u32 = 128;
const FIELD_H: u32 = 224;

/// A border-filling fixture — the frozen `fragment_field` golden. Its `[params]`
/// table is last, so appended `kaleido_*` lines land inside it.
///
/// A centred line figure would not do: it leaves the source's border a smooth
/// backdrop gradient, which smears into more smooth gradient. `composite_kaleido`'s
/// header says exactly this about itself. A fullscreen field paints the border with
/// high-frequency content, which is what the old `ClampToEdge` smear was made of.
const FIELD_FIXTURE: &str = include_str!("fixtures/fragment_field.toml");
const FIELD_FIXTURE_NAME: &str = "fixture_fragment_field";

/// Radius past which the frame must be empty, as a multiple of the disc's own
/// `r_max`.
///
/// Deliberately **above** the engine's falloff band (0.35 of `r_max`, so the fade
/// completes at 1.35x) rather than equal to it: the test then pins the property —
/// *the fold does not reach out here* — without re-stating a constant it cannot
/// see from an integration test, and a modest re-tune of the band does not have to
/// touch this file. Anything at or beyond 1.5x is out of the fold's reach under
/// every treatment that clamps.
const OUT_OF_DISC: f32 = 1.5;

/// Brightest channel tolerated in the out-of-disc region. Not zero only because the
/// falloff multiplies rather than discards; past the band it is exactly zero, and
/// this leaves room for the 8-bit round-trip.
const EMPTY_TOL: u8 = 2;

/// The disc's radius in pixels: `min(w, h) / 2`.
///
/// The fold's aspect correction makes its space isotropic **in pixels** —
/// `p = (dx_px, dy_px) / h` — so `r_max`, the nearest source edge in that space,
/// is the inscribed circle of the frame and lands at half the shorter side however
/// non-square the target is.
fn disc_radius(w: u32, h: u32) -> f32 {
    w.min(h) as f32 * 0.5
}

/// Mean brightest-channel value over the pixels at or beyond `factor * r_max` from
/// the frame centre — the same region [`beyond_disc`] counts, measured as a level
/// rather than a count.
fn beyond_disc_mean(img: &CaptureImage, factor: f32) -> f32 {
    let (w, h) = (img.width as usize, img.height as usize);
    let cutoff = disc_radius(img.width, img.height) * factor;
    let (cx, cy) = (w as f32 * 0.5, h as f32 * 0.5);
    let mut sum = 0.0f64;
    let mut n = 0usize;
    for y in 0..h {
        for x in 0..w {
            let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
            if (dx * dx + dy * dy).sqrt() < cutoff {
                continue;
            }
            let px = &img.rgba[(y * w + x) * 4..(y * w + x) * 4 + 3];
            sum += px.iter().copied().max().unwrap_or(0) as f64;
            n += 1;
        }
    }
    (sum / n.max(1) as f64) as f32
}

/// Largest per-channel difference between two same-sized captures over the pixels
/// at or beyond `factor * r_max` from the frame centre.
fn beyond_disc_max_diff(a: &CaptureImage, b: &CaptureImage, factor: f32) -> u8 {
    assert_eq!((a.width, a.height), (b.width, b.height), "size mismatch");
    let (w, h) = (a.width as usize, a.height as usize);
    let cutoff = disc_radius(a.width, a.height) * factor;
    let (cx, cy) = (w as f32 * 0.5, h as f32 * 0.5);
    let mut worst = 0u8;
    for y in 0..h {
        for x in 0..w {
            let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
            if (dx * dx + dy * dy).sqrt() < cutoff {
                continue;
            }
            for c in 0..3 {
                let i = (y * w + x) * 4 + c;
                worst = worst.max(a.rgba[i].abs_diff(b.rgba[i]));
            }
        }
    }
    worst
}

/// `(brightest channel, lit pixel count, region size)` over the pixels at or
/// beyond `factor * r_max` from the frame centre. `factor = 0` is the whole frame.
fn beyond_disc(img: &CaptureImage, factor: f32) -> (u8, usize, usize) {
    let (w, h) = (img.width as usize, img.height as usize);
    let cutoff = disc_radius(img.width, img.height) * factor;
    let (cx, cy) = (w as f32 * 0.5, h as f32 * 0.5);
    let mut peak = 0u8;
    let mut lit = 0usize;
    let mut total = 0usize;
    for y in 0..h {
        for x in 0..w {
            let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
            if (dx * dx + dy * dy).sqrt() < cutoff {
                continue;
            }
            total += 1;
            let px = &img.rgba[(y * w + x) * 4..(y * w + x) * 4 + 3];
            let bright = px.iter().copied().max().unwrap_or(0);
            peak = peak.max(bright);
            if bright > EMPTY_TOL {
                lit += 1;
            }
        }
    }
    (peak, lit, total)
}

/// The active fold reaches nothing outside its inscribed disc — the direct guard
/// design-backlog 0010 asks for.
///
/// Before ADR-0047 the fold reconstructed sample coordinates outside `[0, 1]` for
/// every output pixel past the source's extent in the folded direction, and
/// `ClampToEdge` smeared the border texel radially across all of them. Clamping the
/// *sample* radius to `r_max` and fading past it means those pixels are not painted
/// at all.
#[test]
fn the_fold_paints_nothing_outside_its_disc() {
    let Some(mut renderer) = headless(FIELD_W, FIELD_H) else {
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
        let toml =
            format!("{FIELD_FIXTURE}kaleido_order = \"{order}\"\nkaleido_angle = \"{ANGLE}\"\n");
        let preset = Preset::from_toml_str(&toml)
            .unwrap_or_else(|e| panic!("field fixture parses at order {order}: {e}"));
        renderer.set_presets(vec![preset]);
        renderer
            .capture_preset(FIELD_FIXTURE_NAME, &frame, FRAMES)
            .unwrap_or_else(|e| panic!("capture field fixture at order {order}: {e}"))
    };

    // Order 1 is the identity passthrough: the stage is skipped entirely, so this
    // is the fixture as the fold's source sees it.
    let unfolded = capture(1.0);
    let folded = capture(6.0);

    let (unfolded_peak, unfolded_lit, region) = beyond_disc(&unfolded, OUT_OF_DISC);
    let (folded_peak, folded_lit, _) = beyond_disc(&folded, OUT_OF_DISC);
    println!(
        "beyond {OUT_OF_DISC}x r_max ({:.1} px of a {FIELD_W}x{FIELD_H} frame, \
         {region} px region): unfolded peak {unfolded_peak} / {unfolded_lit} lit | \
         folded peak {folded_peak} / {folded_lit} lit",
        disc_radius(FIELD_W, FIELD_H) * OUT_OF_DISC
    );

    // --- The property. On the unfixed shader this region carried the smeared
    // border texel; it must now carry nothing. ---
    assert!(
        folded_peak <= EMPTY_TOL,
        "the fold painted outside its disc: brightest channel {folded_peak} beyond \
         {OUT_OF_DISC}x r_max ({folded_lit} pixels lit) — the clamped-sample smear of \
         design-backlog 0010 is back"
    );

    // --- Non-vacuity 1: the region IS lit in the fold's own input, so the
    // emptiness above is the disc's doing and not an empty fixture. ---
    assert!(
        unfolded_lit * 2 > region && unfolded_peak > 64,
        "the unfolded fixture leaves the out-of-disc region dark (peak {unfolded_peak}, \
         {unfolded_lit} of {region} px lit), so the folded frame being dark there proves \
         nothing — this fixture no longer fills the frame"
    );

    // --- Non-vacuity 2: the fold drew *something*. A stage that failed to build,
    // or a disc computed as zero, would satisfy the property with a black frame. ---
    let inside = beyond_disc(&folded, 0.0).1 - folded_lit;
    assert!(
        inside > (FIELD_W * FIELD_H) as usize / 10,
        "only {inside} pixels are lit inside the disc — the fold rendered (near) nothing, \
         so the out-of-disc assertion is satisfied vacuously"
    );
}

// --- The falloff lands on the BACKDROP, not on black (ADR-0055). -------------
//
// Plan 0045 Phase 2b. Both assertions below need a **lit** backdrop, and that is
// the whole point: every fixture that exercises the fold has `bg_bright = 0`, and
// so did all sixteen of Phase 1's confirmation samples. On a black backdrop a
// falloff that fades to black and a falloff that composites over the backdrop are
// *the same picture*, so nothing captured at that configuration could tell them
// apart — ADR-0037's lesson in another costume.

/// A lit backdrop for the fold fixtures. `bg_vignette` is deliberately non-zero:
/// it is the backdrop's own frame-centred radial structure, and the second test
/// below is that the fold no longer replicates it into the wedges.
const LIT_BG: &str = "bg_bright = \"0.55\"\nbg_hue = \"0.62\"\nbg_vignette = \"0.35\"\n";

/// An off-centre fold axis for the invariance test.
///
/// Chosen so the disc — even including its falloff band — stays clear of the
/// region the assertion reads, with margin. In the shader's aspect-corrected space
/// (`p.x` scaled by `aspect = 128/224 = 0.571`, radii in uv-y units), a centre at
/// `(0.40, 0.44)` gives `r_max = min(0.40 * 0.571, 0.44) = 0.2286` uv-y = 51.2 px,
/// and the fold reaches `1.35 r_max` = 69.1 px from an axis sitting 18.6 px off
/// the frame centre — 87.7 px at the farthest, against the 96 px cutoff
/// (`OUT_OF_DISC * min(w, h) / 2`). Everything read is backdrop.
const OFF_CENTER: (f32, f32) = (0.40, 0.44);

/// Capture the border-filling field with the fold active, at a chosen backdrop and
/// fold centre.
fn capture_folded(
    renderer: &mut Renderer,
    frame: &AnalysisFrame,
    bg: &str,
    center: (f32, f32),
) -> CaptureImage {
    let toml = format!(
        "{FIELD_FIXTURE}kaleido_order = \"6\"\nkaleido_angle = \"{ANGLE}\"\n\
         kaleido_center_x = \"{}\"\nkaleido_center_y = \"{}\"\n{bg}",
        center.0, center.1
    );
    let preset = Preset::from_toml_str(&toml)
        .unwrap_or_else(|e| panic!("lit-backdrop fold fixture parses: {e}"));
    renderer.set_presets(vec![preset]);
    renderer
        .capture_preset(FIELD_FIXTURE_NAME, frame, FRAMES)
        .unwrap_or_else(|e| panic!("capture lit-backdrop fold fixture: {e}"))
}

/// Outside the disc the frame carries the **backdrop**, not black.
///
/// Before ADR-0055 the fold multiplied only `.rgb` by the falloff weight and forced
/// alpha to `1.0`, so the falloff drove the picture toward black — and the backdrop
/// was inside the fold's own input, so there was nothing underneath to land on.
/// ADR-0047's "the falloff lands on the backdrop" was false as shipped; this is the
/// assertion that makes it true.
#[test]
fn the_falloff_lands_on_the_backdrop_not_on_black() {
    let Some(mut renderer) = headless(FIELD_W, FIELD_H) else {
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

    let centre = (0.5, 0.5);
    let dark = capture_folded(&mut renderer, &frame, "", centre);
    let lit = capture_folded(&mut renderer, &frame, LIT_BG, centre);

    let (dark_peak, _, region) = beyond_disc(&dark, OUT_OF_DISC);
    let (lit_peak, lit_count, _) = beyond_disc(&lit, OUT_OF_DISC);
    let (dark_mean, lit_mean) = (
        beyond_disc_mean(&dark, OUT_OF_DISC),
        beyond_disc_mean(&lit, OUT_OF_DISC),
    );
    println!(
        "beyond {OUT_OF_DISC}x r_max ({region} px): unlit backdrop peak {dark_peak} \
         mean {dark_mean:.2} | lit backdrop peak {lit_peak} mean {lit_mean:.2} \
         ({lit_count} px lit)"
    );

    // --- The property: with a lit backdrop the out-of-disc region carries it. A
    // fade to black reads this region at the unlit level whatever `bg_bright` is,
    // so this is exactly the assertion the old shader fails. ---
    assert!(
        lit_mean > 16.0 && lit_count * 2 > region,
        "the falloff did not land on the backdrop: beyond {OUT_OF_DISC}x r_max the mean \
         brightness is {lit_mean:.2} with only {lit_count} of {region} px lit, at \
         bg_bright = 0.55 — the fold is still fading to black"
    );

    // --- Non-vacuity 1: the same region with no backdrop bound must be dark, so
    // the brightness above is the backdrop's doing and not the fold leaking
    // content out there (which is design-backlog 0010, guarded separately). ---
    assert!(
        dark_peak <= EMPTY_TOL,
        "with no backdrop the out-of-disc region is not empty (peak {dark_peak}) — the \
         lit reading above cannot be attributed to bg_* "
    );

    // --- Non-vacuity 2: the two captures must differ *only* by the backdrop, so
    // the fold itself still drew a picture inside its disc. ---
    let inside = beyond_disc(&lit, 0.0).1 - lit_count;
    assert!(
        inside > (FIELD_W * FIELD_H) as usize / 10,
        "only {inside} pixels are lit inside the disc — the fold rendered (near) nothing"
    );
}

/// The backdrop is composited **under** the fold, so moving the fold axis does not
/// move the backdrop.
///
/// `post.rs` used to render the backdrop into the first active stage's *input*,
/// which put it inside the texture the kaleidoscope folds: `bg_vignette`'s radial
/// darkening was replicated into the wedges, around an axis that — once Phase 1
/// made the fold centre bindable — need not be the vignette's centre at all. With
/// the backdrop underneath the chain (ADR-0055), the region outside the disc is
/// untouched backdrop and is therefore *identical* however the fold axis moves.
#[test]
fn the_backdrop_is_not_folded_so_it_does_not_move_with_the_fold_axis() {
    let Some(mut renderer) = headless(FIELD_W, FIELD_H) else {
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

    let centred = capture_folded(&mut renderer, &frame, LIT_BG, (0.5, 0.5));
    let shifted = capture_folded(&mut renderer, &frame, LIT_BG, OFF_CENTER);

    let drift = beyond_disc_max_diff(&centred, &shifted, OUT_OF_DISC);
    let region = beyond_disc(&centred, OUT_OF_DISC).2;
    println!(
        "backdrop drift beyond {OUT_OF_DISC}x r_max over {region} px, fold centre \
         (0.5, 0.5) vs {OFF_CENTER:?}: max per-channel {drift}"
    );

    // --- The property. Both captures show pure backdrop out here (see
    // `OFF_CENTER`'s arithmetic), and the backdrop does not know the fold exists.
    // A folded backdrop would smear its vignette differently in the two. ---
    assert!(
        drift <= EMPTY_TOL,
        "the backdrop moved with the fold axis: {drift} per-channel beyond \
         {OUT_OF_DISC}x r_max — bg_* is being folded again"
    );

    // --- Non-vacuity: the fold centre must actually have changed the picture, or
    // the invariance above is satisfied by two identical frames. ---
    assert_ne!(
        centred.rgba, shifted.rgba,
        "moving kaleido_center_* changed nothing — the fixture is not exercising the \
         fold centre, so the invariance above proves nothing"
    );

    // --- ...and specifically *inside* the disc, which is where it should differ. ---
    let inner = beyond_disc_max_diff(&centred, &shifted, 0.0);
    assert!(
        inner > EMPTY_TOL,
        "the two folds differ by at most {inner} per-channel over the whole frame — \
         the off-centre axis is too small a displacement to prove anything"
    );
}
