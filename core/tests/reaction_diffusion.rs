//! Reaction-diffusion scene contract (Plan 0014 Phase 6, HARD). The RD scene is
//! the engine's first *stateful feedback* system, so beyond the generic
//! per-preset gates (sanity / animation / reactivity, which already include
//! Coral) it gets a focused suite here — most importantly a **seed
//! reproducibility** check, the property a running simulation most easily
//! breaks (ADR-0012).
//!
//! All four checks ride Plan 0013's `capture_preset`, which rebuilds the scene
//! to its seed and resets the clock, so a capture is a pure function of
//! `(preset, frame, frames)` under the fixed capture `dt`. Software adapter
//! (`prefer_software`) so it holds on any CI GPU and reproduces bit-for-bit.
//!
//! The four checks share one renderer in a single `#[test]` (one per file, like
//! the other GPU suites): distinct headless renderers built in parallel each
//! spin up a WARP device and can crash the software driver.

use lmv_core::dsp::AnalysisFrame;
use lmv_core::preset::Preset;
use lmv_core::render::{
    CaptureImage, HeadlessOptions, RenderError, Renderer,
    metrics::{coverage, frame_diff, quadrant_spread},
};

const SIZE: u32 = 96;
/// The RD preset shipped in the embedded set. Mitosis is the family's
/// beat-driven world (its `inject` stamps a cell per beat), which the
/// beat-perturbation check below relies on.
const PRESET: &str = "Mitosis";
/// A pixel counts as lit if any RGB channel differs from the sampled background
/// by more than this.
const EPS: u8 = 10;

/// An RD preset with the same lively field params plus an extra `[params]` line,
/// isolating the view transform (Phase 2): the field is identical, so any render
/// difference is the present-pass zoom/pan. The view transform touches only the
/// present sampling (no background pipeline), so it is faithful on WARP.
fn rd_view_preset(name: &str, extra: &str) -> Preset {
    let toml = format!(
        "system = \"reaction_diffusion\"\nname = \"{name}\"\n[params]\n\
         feed = \"0.037\"\nkill = \"0.06\"\nflow = \"1.0\"\nglow = \"0.8\"\n{extra}"
    );
    Preset::from_toml_str(&toml).unwrap_or_else(|e| panic!("{name} preset parses: {e}"))
}

/// Build a headless `Renderer`, or `None` (a logged skip) when the runner
/// exposes no GPU adapter — macOS has no software Metal fallback (ADR-0016).
/// Any other build error still panics loudly.
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

/// The top-left pixel, taken as the scene's background colour.
fn background(img: &CaptureImage) -> [u8; 4] {
    [
        img.rgba.first().copied().unwrap_or(0),
        img.rgba.get(1).copied().unwrap_or(0),
        img.rgba.get(2).copied().unwrap_or(0),
        img.rgba.get(3).copied().unwrap_or(255),
    ]
}

#[test]
fn reaction_diffusion_contract() {
    let Some(mut renderer) = headless() else {
        return;
    };

    // A sustained mid-energy frame that keeps the field lively; no beat.
    let lively = AnalysisFrame {
        bass: 0.5,
        mid: 0.3,
        treb: 0.3,
        ..Default::default()
    };

    // --- Shape sanity: a warmed field is neither blank nor a single dot. ---
    let warm = renderer
        .capture_preset(PRESET, &lively, 60)
        .expect("capture Mitosis @60");
    let bg = background(&warm);
    let cov = coverage(&warm, bg, EPS);
    let spread = quadrant_spread(&warm, bg, EPS);
    assert!(cov > 0.03, "field is blank: coverage {cov:.4}");
    assert!(spread >= 2, "field is a dot: {spread} quadrant(s)");

    // --- Animation: a later frame differs from an earlier one (not frozen). ---
    let early = renderer
        .capture_preset(PRESET, &lively, 24)
        .expect("capture @24");
    let motion = frame_diff(&early, &warm);
    assert!(motion > 0.01, "sim is frozen: motion {motion:.4}");

    // --- Reactivity: a beat perturbs the field (stamps a seed of growth). ---
    let calm = AnalysisFrame {
        bass: 0.3,
        mid: 0.3,
        ..Default::default()
    };
    let beat = AnalysisFrame { beat: true, ..calm };
    let without = renderer
        .capture_preset(PRESET, &calm, 60)
        .expect("capture calm");
    let with = renderer
        .capture_preset(PRESET, &beat, 60)
        .expect("capture beat");
    let delta = frame_diff(&without, &with);
    assert!(delta > 0.003, "beat did not perturb the field: {delta:.4}");

    // --- Seed reproducibility (ADR-0012): the stateful sim + seeded injection
    // RNG are deterministic, so the same input reproduces bit-for-bit on the
    // same adapter — the property a running simulation most easily loses. ---
    let repro_frame = AnalysisFrame {
        bass: 0.4,
        mid: 0.3,
        beat: true, // exercise the injection path too
        ..Default::default()
    };
    let a = renderer
        .capture_preset(PRESET, &repro_frame, 48)
        .expect("capture A");
    let b = renderer
        .capture_preset(PRESET, &repro_frame, 48)
        .expect("capture B");
    assert_eq!(
        a.rgba, b.rgba,
        "reaction-diffusion capture is not reproducible for a fixed input"
    );

    // --- View transform (Plan 0025 Phase 2, ADR-0018): `zoom`/`pan_*` transform the
    // present-pass sample window, so binding them visibly moves the field. The field
    // sim is untouched (same params, same seed), so any pixel difference is the view
    // transform alone — and it stays a pure function of the params (deterministic). ---
    renderer.set_presets(vec![
        rd_view_preset("rd_identity", ""),
        rd_view_preset("rd_zoom", "zoom = \"1.6\"\n"),
        rd_view_preset("rd_pan", "pan_x = \"0.3\"\n"),
    ]);
    let identity = renderer
        .capture_preset("rd_identity", &lively, 60)
        .expect("capture rd_identity");
    let zoomed = renderer
        .capture_preset("rd_zoom", &lively, 60)
        .expect("capture rd_zoom");
    let panned = renderer
        .capture_preset("rd_pan", &lively, 60)
        .expect("capture rd_pan");
    assert!(
        frame_diff(&identity, &zoomed) > 0.02,
        "zoom did not move the field: diff {:.4}",
        frame_diff(&identity, &zoomed)
    );
    assert!(
        frame_diff(&identity, &panned) > 0.02,
        "pan did not move the field: diff {:.4}",
        frame_diff(&identity, &panned)
    );
    // Determinism: the transform is a pure function of its params (no wall-clock).
    let zoomed_again = renderer
        .capture_preset("rd_zoom", &lively, 60)
        .expect("capture rd_zoom again");
    assert_eq!(
        zoomed.rgba, zoomed_again.rgba,
        "zoomed reaction-diffusion capture is not reproducible"
    );

    // --- Toroidal presentation (Plan 0033 Phase 5, ADR-0034): the simulation has
    // always wrapped (`ld()` in the sim shader), but the present sampler clamped,
    // so `zoom > 1` smeared the edge row outward into vertical bars and any real
    // `pan_*` walked off the field. With `AddressMode::Repeat` the view transform
    // sees a seamless torus.
    //
    // Asserted as structure, not by eye. At `zoom = 1.4` the sampled window
    // overshoots the field by 1/2 - 1/2.8 of the frame on each side, so the outer
    // ~14% of columns are off-field. Clamping fills them by repeating the boundary
    // texel along x, which makes them *horizontally* flat — the give-away is
    // vanishing horizontal detail out there, not vanishing variance down the
    // column (clamping u leaves each column's vertical structure intact, which is
    // why the obvious detector misses this entirely). Wrapping fills them with
    // real field, so the edge band carries the same detail as the centre. ---
    renderer.set_presets(vec![
        rd_view_preset("rd_wrap_1", "zoom = \"1.0\"\n"),
        rd_view_preset("rd_wrap_14", "zoom = \"1.4\"\n"),
        rd_view_preset("rd_wrap_pan", "pan_x = \"0.5\"\n"),
    ]);
    let at_1 = renderer
        .capture_preset("rd_wrap_1", &lively, 60)
        .expect("capture rd_wrap_1");
    let at_14 = renderer
        .capture_preset("rd_wrap_14", &lively, 60)
        .expect("capture rd_wrap_14");
    let ratio = edge_band_detail(&at_14);
    let control = edge_band_detail(&at_1);
    eprintln!(
        "toroidal present: edge-band detail ratio at zoom 1.4 = {ratio:.3} \
         (zoom 1.0 control {control:.3})"
    );
    assert!(
        ratio > 0.5,
        "at zoom = 1.4 the outer columns carry only {ratio:.3} of the centre's \
         horizontal detail: the present sampler is smearing the edge texel outward \
         instead of wrapping the toroidal field"
    );
    // The control proves the metric is measuring the overshoot and not some
    // property of the scene: at zoom = 1.0 nothing is off-field, so both filters
    // score near 1 here.
    assert!(
        control > 0.5,
        "the edge-band metric is broken: even at zoom = 1.0, with no overshoot at \
         all, the outer columns look flat ({control:.3})"
    );

    // A half-field pan at zoom = 1.0 is a seamless scroll: it must MOVE the field
    // (not a no-op) while keeping every column populated (not a clamp streak).
    let panned_half = renderer
        .capture_preset("rd_wrap_pan", &lively, 60)
        .expect("capture rd_wrap_pan");
    assert!(
        frame_diff(&at_1, &panned_half) > 0.02,
        "pan_x = 0.5 did not move the field"
    );
    let panned_detail = edge_band_detail(&panned_half);
    assert!(
        panned_detail > 0.5,
        "pan_x = 0.5 left the outer columns flat ({panned_detail:.3} of the centre's \
         horizontal detail): it is walking off the field rather than scrolling around it"
    );

    // --- Long run: one unpolled stretch past the frame ceiling (Plan 0099). ---
    //
    // The WARP renderer above is dropped first, on this file's own rule: two live
    // headless devices in one process is the configuration that crashes the
    // software driver, so the check below builds its own only after this one is
    // gone.
    drop(renderer);
    long_run_past_the_old_ceiling();
}

/// Drive an RD world through one long **unpolled** stretch and require it to
/// finish — the frame ceiling Plan 0099 removed, pinned so it cannot come back.
///
/// `shot --horizon 10` claims 36,001 renders and died on all three shipped RD
/// worlds, because `step_offscreen` submitted without ever polling: the only
/// `device.poll` on the capture path was the readback's, so at the default
/// interval the path ran 1,800 consecutive unpolled submits. Retention is per
/// **pass**, not per pixel — RD encodes 12 simulation sub-steps plus a present —
/// and one such stretch retained **950 KB a frame** against a 36 KB captured
/// frame, so the process reached ~4.4 GB and the readback buffer went invalid.
///
/// `at_frames = [0, LONG_RUN]` is the shape that matters as much as the count:
/// two samples make the whole run ONE unpolled stretch, the worst case any
/// interval can produce. `LONG_RUN` is deliberately past **3,601** — the frame
/// count first reported — and past the **5,401** that still cleared when it was
/// re-measured, because a ceiling pushed up is the same defect with a bigger
/// number. Unfixed it peaks near 5.7 GB and fails.
///
/// **It is hardware-gated, and that is a real limit rather than a preference.**
/// The check was first written on this suite's WARP renderer and **passed against
/// the unfixed path** — 6,000 unpolled frames on the software adapter do not
/// accumulate the way they do on DX12, so on WARP it was a three-minute no-op
/// rather than a regression test. Windows CI has only WARP (ADR-0073), so this
/// skips there and earns its keep on a developer box. Skipping loudly is the
/// point: a silent pass would claim cover it does not have.
fn long_run_past_the_old_ceiling() {
    const LONG_RUN: u32 = 6_000;

    let mut renderer = match Renderer::new_headless(HeadlessOptions {
        width: SIZE,
        height: SIZE,
        prefer_software: false,
    }) {
        Ok(r) if r.adapter_is_software() => {
            eprintln!(
                "skipped: the {LONG_RUN}-frame unpolled-stretch check needs a hardware \
                 adapter — the defect it pins does not reproduce on WARP"
            );
            return;
        }
        Ok(r) => r,
        Err(RenderError::RequestAdapter(_)) => {
            eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
            return;
        }
        Err(e) => panic!("headless renderer build failed: {e}"),
    };

    let lively = AnalysisFrame {
        bass: 0.5,
        mid: 0.3,
        treb: 0.3,
        ..Default::default()
    };
    let long = renderer
        .capture_preset_at(PRESET, &lively, &[0, LONG_RUN])
        .unwrap_or_else(|e| {
            panic!(
                "a {LONG_RUN}-frame unpolled stretch failed at `{e}` — the capture \
                 path is retaining per-frame resources again (Plan 0099)"
            )
        });

    // The run has to have gone somewhere: an RD field 100 simulated seconds on is
    // not the frame it started from. Without this the check would also pass on a
    // path that returned the first frame twice.
    let advanced = match long.as_slice() {
        [first, last] => frame_diff(first, last),
        rows => panic!("a two-sample horizon returned {} rows", rows.len()),
    };
    assert!(
        advanced > 0.01,
        "the {LONG_RUN}-frame run completed but did not advance the field \
         ({advanced:.4})"
    );
}

/// Horizontal detail in the outer 10% of columns, relative to the middle 20%.
///
/// This is the clamp detector. `AddressMode::ClampToEdge` fills everything past
/// the field's right/left edge by repeating the boundary texel *along x*, so out
/// there `L(x+1, y) == L(x, y)` and the horizontal gradient vanishes. Note it is
/// specifically the horizontal gradient: clamping `u` leaves each column's
/// vertical structure completely intact, so a variance-down-the-column detector
/// sees nothing. Wrapping fills the same band with real field, so it carries the
/// centre's detail and the ratio sits near 1.
fn edge_band_detail(img: &CaptureImage) -> f32 {
    let (w, h) = (img.width as usize, img.height as usize);
    let luma = |x: usize, y: usize| -> f32 {
        let i = (y * w + x) * 4;
        let r = img.rgba.get(i).copied().unwrap_or(0) as f32;
        let g = img.rgba.get(i + 1).copied().unwrap_or(0) as f32;
        let b = img.rgba.get(i + 2).copied().unwrap_or(0) as f32;
        (0.299 * r + 0.587 * g + 0.114 * b) / 255.0
    };
    let gradient = |xs: std::ops::Range<usize>| -> f32 {
        let mut sum = 0.0f32;
        let mut n = 0usize;
        for y in 0..h {
            for x in xs.clone() {
                if x + 1 < w {
                    sum += (luma(x + 1, y) - luma(x, y)).abs();
                    n += 1;
                }
            }
        }
        sum / n.max(1) as f32
    };
    // Outer tenth on each side — comfortably inside the ~14% that a zoom of 1.4
    // pushes off the field — against the middle fifth, which is always on-field.
    let band = w / 10;
    let edges = (gradient(0..band) + gradient(w - band..w)) / 2.0;
    let centre = gradient(w * 2 / 5..w * 3 / 5);
    edges / centre.max(1e-9)
}
