//! GPU compute-particle attractor contract (Plan 0016 Phase 5, HARD). The
//! attractor scene is the engine's first *compute pipeline* + GPU-resident
//! particle system, so beyond the generic per-preset gates (sanity / animation /
//! reactivity, which already include the four shipped attractor presets) it gets
//! a focused suite here — most importantly a **seed reproducibility** check (the
//! Phase 1 determinism done-when) and a **beat perturbation** check (Phase 3), the
//! two properties the generic differential loops don't assert directly.
//!
//! All checks ride Plan 0013's `capture_preset`, which rebuilds the scene to its
//! seed and resets the clock, so a capture is a pure function of `(preset, frame,
//! frames)` under the fixed capture `dt`. Software adapter (`prefer_software`) so
//! it holds on any CI GPU and reproduces bit-for-bit.
//!
//! The checks share one renderer in a single `#[test]` (one per file, like the
//! other GPU suites): distinct headless renderers built in parallel each spin up
//! a WARP device and can crash the software driver.

use lmv_core::dsp::AnalysisFrame;
use lmv_core::preset::Preset;
use lmv_core::render::scenes::particles::trail_grid_size;
use lmv_core::render::{
    CaptureImage, HeadlessOptions, RenderError, Renderer,
    metrics::{coverage, frame_diff, quadrant_spread},
};

const SIZE: u32 = 96;
/// The 2D map preset (De Jong) and a 3D flow preset (Lorenz) from the embedded
/// set — one of each idiom the scene supports.
const DEJONG: &str = "De Jong";
const LORENZ: &str = "Lorenz";

/// A De Jong attractor preset with an extra `[params]` line — used to isolate the
/// view transform (Phase 4): the compute/accumulation path is identical, so any
/// render difference is the vertex-shader zoom/pan. The transform touches only the
/// draw projection (no background pipeline), so it is faithful on WARP.
fn attractor_view_preset(name: &str, extra: &str) -> Preset {
    let toml =
        format!("system = \"attractor\"\nname = \"{name}\"\n[params]\nsize = \"1.0\"\n{extra}");
    Preset::from_toml_str(&toml).unwrap_or_else(|e| panic!("{name} preset parses: {e}"))
}
/// A pixel counts as lit if any RGB channel differs from the sampled background
/// by more than this.
const EPS: u8 = 10;

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

/// The top-left pixel, taken as the scene's background colour (the near-black bed
/// the attractor clears its trail field to).
fn background(img: &CaptureImage) -> [u8; 4] {
    [
        img.rgba.first().copied().unwrap_or(0),
        img.rgba.get(1).copied().unwrap_or(0),
        img.rgba.get(2).copied().unwrap_or(0),
        img.rgba.get(3).copied().unwrap_or(255),
    ]
}

#[test]
fn attractor_contract() {
    let Some(mut renderer) = headless() else {
        return;
    };

    // A sustained mid-energy frame; no beat.
    let lively = AnalysisFrame {
        bass: 0.5,
        mid: 0.4,
        treb: 0.5,
        ..Default::default()
    };

    // --- Shape sanity: the De Jong cloud is neither blank nor a single dot. ---
    let warm = renderer
        .capture_preset(DEJONG, &lively, 60)
        .expect("capture De Jong @60");
    let bg = background(&warm);
    let cov = coverage(&warm, bg, EPS);
    let spread = quadrant_spread(&warm, bg, EPS);
    assert!(cov > 0.02, "De Jong cloud is blank: coverage {cov:.4}");
    assert!(spread >= 2, "De Jong cloud is a dot: {spread} quadrant(s)");

    // --- Seed reproducibility (Phase 1 determinism done-when): the seeded init +
    // deterministic compute step reproduce bit-for-bit on the same adapter — the
    // property a GPU-resident particle sim most easily loses. ---
    let a = renderer
        .capture_preset(DEJONG, &lively, 48)
        .expect("capture A");
    let b = renderer
        .capture_preset(DEJONG, &lively, 48)
        .expect("capture B");
    assert_eq!(
        a.rgba, b.rgba,
        "attractor capture is not reproducible for a fixed input"
    );

    // --- Animation: a later frame differs from an earlier one (boiling + spin +
    // trails move it), not frozen. ---
    let early = renderer
        .capture_preset(DEJONG, &lively, 24)
        .expect("capture @24");
    let motion = frame_diff(&early, &warm);
    assert!(motion > 0.01, "attractor is frozen: motion {motion:.4}");

    // --- Beat perturbation (Phase 3): a beat re-scatters the cloud and swells the
    // points, so a beat frame differs from an otherwise-identical calm one. ---
    let calm = AnalysisFrame {
        bass: 0.3,
        mid: 0.3,
        ..Default::default()
    };
    let beat = AnalysisFrame { beat: true, ..calm };
    let without = renderer
        .capture_preset(DEJONG, &calm, 60)
        .expect("capture calm");
    let with = renderer
        .capture_preset(DEJONG, &beat, 60)
        .expect("capture beat");
    let delta = frame_diff(&without, &with);
    assert!(delta > 0.003, "beat did not perturb the cloud: {delta:.4}");

    // --- 3D flow: the Lorenz butterfly renders a real shape, exercising the
    // continuous-family compute path (Euler integration + 3D projection). ---
    let lorenz = renderer
        .capture_preset(LORENZ, &lively, 90)
        .expect("capture Lorenz @90");
    let lbg = background(&lorenz);
    let lcov = coverage(&lorenz, lbg, EPS);
    let lspread = quadrant_spread(&lorenz, lbg, EPS);
    assert!(lcov > 0.02, "Lorenz flow is blank: coverage {lcov:.4}");
    assert!(lspread >= 2, "Lorenz flow is a dot: {lspread} quadrant(s)");

    // --- View transform (Plan 0025 Phase 4, ADR-0018): `zoom`/`pan_*` scale/offset
    // the projected cloud, so binding them visibly moves the whole attractor. The
    // compute + accumulation path is untouched (same seed, same steps), so any pixel
    // difference is the view transform alone — and it stays a pure function of the
    // params (deterministic). ---
    renderer.set_presets(vec![
        attractor_view_preset("at_identity", ""),
        attractor_view_preset("at_zoom", "zoom = \"1.5\"\n"),
        attractor_view_preset("at_pan", "pan_x = \"0.4\"\n"),
    ]);
    let identity = renderer
        .capture_preset("at_identity", &lively, 60)
        .expect("capture at_identity");
    let zoomed = renderer
        .capture_preset("at_zoom", &lively, 60)
        .expect("capture at_zoom");
    let panned = renderer
        .capture_preset("at_pan", &lively, 60)
        .expect("capture at_pan");
    assert!(
        frame_diff(&identity, &zoomed) > 0.02,
        "zoom did not move the attractor: diff {:.4}",
        frame_diff(&identity, &zoomed)
    );
    assert!(
        frame_diff(&identity, &panned) > 0.02,
        "pan did not move the attractor: diff {:.4}",
        frame_diff(&identity, &panned)
    );
    // Determinism: the transform is a pure function of its params (no wall-clock).
    let zoomed_again = renderer
        .capture_preset("at_zoom", &lively, 60)
        .expect("capture at_zoom again");
    assert_eq!(
        zoomed.rgba, zoomed_again.rgba,
        "zoomed attractor capture is not reproducible"
    );
}

// --- Trail grid sizing (Plan 0029 Phase 2) -------------------------------------
//
// `trail_grid_size` is pure, so these need no GPU and never skip. They mirror the
// scene's private policy constants; a change there must change these deliberately.

/// The per-axis cap (`TRAIL_MAX_W`/`TRAIL_MAX_H`) and quantization step
/// (`TRAIL_GRID_STEP`) the scene applies.
const CAP_W: u32 = 2560;
const CAP_H: u32 = 1440;
const STEP: u32 = 256;

/// Above the cap the grid must keep the *target's* proportions. The previous
/// per-axis clamp squashed a 3440x1440 ultrawide target to 2560x1440 — a 16:9
/// grid that the aspect-ignoring present then stretched back to 21:9, so the
/// attractor's shape changed discontinuously as the window crossed 2560 wide.
#[test]
fn trail_grid_preserves_aspect_above_the_cap() {
    let (w, h) = trail_grid_size(3440, 1440);
    assert_eq!(w, CAP_W, "the binding axis should sit at its cap");
    assert!(
        h < CAP_H,
        "3440x1440 was squashed back to 16:9 ({w}x{h}) — the per-axis clamp is back"
    );
    // The aspect-exact height for this width, before quantization. Rounding each
    // axis up to STEP is what collapses nearby sizes onto one grid, so the aspect
    // it can hold is exact to within that step - but no worse.
    let exact_h = w as f32 * 1440.0 / 3440.0;
    assert!(
        (h as f32 - exact_h).abs() < STEP as f32,
        "grid {w}x{h} is more than one {STEP} px step off the aspect-exact height {exact_h:.1}"
    );

    // The same property on the other binding axis (a portrait/ultra-tall target).
    let (tw, th) = trail_grid_size(1080, 3440);
    assert_eq!(th, CAP_H, "the binding axis should sit at its cap");
    let exact_w = th as f32 * 1080.0 / 3440.0;
    assert!(
        (tw as f32 - exact_w).abs() < STEP as f32,
        "grid {tw}x{th} is more than one {STEP} px step off the aspect-exact width {exact_w:.1}"
    );
}

/// Quantization: two nearby target sizes must request the *same* grid, so a live
/// window drag re-allocates the field a handful of times instead of once a frame.
#[test]
fn trail_grid_quantizes_nearby_targets_to_one_grid() {
    assert_eq!(
        trail_grid_size(1920, 1080),
        trail_grid_size(1900, 1070),
        "a 20 px drag changed the grid — quantization is not in effect"
    );
    // ...and it is quantization, not a constant: a target a step away differs.
    assert_ne!(
        trail_grid_size(1920, 1080),
        trail_grid_size(1280, 720),
        "every target maps to the same grid — the size is not following the target"
    );
    // Both axes land on a step multiple below the cap.
    let (w, h) = trail_grid_size(1920, 1080);
    assert_eq!(
        (w % STEP, h % STEP),
        (0, 0),
        "grid {w}x{h} is not quantized"
    );
}

/// Cap and floor: no axis ever exceeds its cap, and none is ever 0 (a zero-extent
/// texture is a wgpu validation error, and the window can report 0 while minimized).
#[test]
fn trail_grid_never_exceeds_the_cap_or_collapses() {
    for (w, h) in [
        (0, 0),
        (1, 1),
        (0, 1080),
        (128, 128),
        (1920, 1080),
        (2560, 1440),
        (3440, 1440),
        (7680, 4320),
        (u32::MAX, u32::MAX),
    ] {
        let (gw, gh) = trail_grid_size(w, h);
        assert!(
            gw >= 1 && gh >= 1,
            "{w}x{h} produced an empty grid {gw}x{gh}"
        );
        assert!(
            gw <= CAP_W && gh <= CAP_H,
            "{w}x{h} produced {gw}x{gh}, past the {CAP_W}x{CAP_H} cap"
        );
    }
}

// --- Projection aspect (Plan 0029 Phase 5) -------------------------------------

/// Targets sharing one aspect but landing on different grids, so the *only* thing
/// that differs between the two captures is the grid the scene chose. The first is
/// aspect-exact (both axes are already `STEP` multiples, so the grid equals the
/// target); the second quantizes up on both axes to a square grid under a 4:3
/// target. Point size is in world units, so the cloud's extent as a *fraction* of
/// the frame is resolution-independent and the two are directly comparable.
const EXACT_TARGET: (u32, u32) = (1024, 768);
const QUANTIZED_TARGET: (u32, u32) = (512, 384);
/// Enough frames for the trail field to saturate (`fade = 0.94` fades over ~1 s),
/// so the cloud's outline is at its full extent in both captures.
const ASPECT_FRAMES: u32 = 90;

/// Capture one preset at an explicit target size, building and dropping a renderer
/// so only one WARP device is ever live (the file docs' constraint). `None` is the
/// no-adapter skip (ADR-0016).
fn capture_at(size: (u32, u32), preset: &str, frame: &AnalysisFrame) -> Option<CaptureImage> {
    let mut renderer = match Renderer::new_headless(HeadlessOptions {
        width: size.0,
        height: size.1,
        prefer_software: true,
    }) {
        Ok(r) => r,
        Err(RenderError::RequestAdapter(_)) => {
            eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
            return None;
        }
        Err(e) => panic!("headless renderer build failed at {size:?}: {e}"),
    };
    let img = renderer
        .capture_preset(preset, frame, ASPECT_FRAMES)
        .unwrap_or_else(|e| panic!("capture {preset} at {size:?}: {e}"));
    Some(img)
}

/// The width:height ratio of the lit region's bounding box, in units of the
/// **frame** — i.e. normalized by the capture's own size, so it is the aspect the
/// cloud occupies on screen and is directly comparable across capture sizes.
fn lit_bbox_ratio(img: &CaptureImage) -> f32 {
    let bg = background(img);
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0u32, 0u32);
    for (i, px) in img.rgba.chunks_exact(4).enumerate() {
        let lit = px
            .iter()
            .zip(bg.iter())
            .take(3)
            .any(|(&c, &b)| c.abs_diff(b) > EPS);
        if !lit {
            continue;
        }
        let (x, y) = (i as u32 % img.width, i as u32 / img.width);
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x);
        y1 = y1.max(y);
    }
    assert!(
        x0 <= x1 && y0 <= y1,
        "no lit pixels to measure a bounding box"
    );
    let bw = (x1 - x0 + 1) as f32 / img.width as f32;
    let bh = (y1 - y0 + 1) as f32 / img.height as f32;
    bw / bh
}

/// The cloud's proportions must follow the **render target's** aspect, not the
/// accumulation grid's (Plan 0029 Phase 5). The present stretches the field over
/// the whole target with aspect ignored, so field NDC `x` becomes target NDC `x`
/// and the field's own aspect cancels out — the projection has to use the target's
/// or the shape is scaled by `target_aspect / grid_aspect`.
///
/// Both targets below are 4:3. One is aspect-exact (grid 1024x768); the other
/// quantizes to a 512x512 grid, aspect 1.0. Projecting at the grid ratio therefore
/// drew the second **33% too wide** — the size-dependent shape error Phase 2's
/// quantization introduced, and the reason this is the first non-square assertion
/// in the suite: every other capture here is square, so grid aspect always equalled
/// target aspect and nothing could see it.
#[test]
fn attractor_projects_at_the_target_aspect() {
    // Verify the premise before spending two captures on it: these two targets
    // must genuinely disagree about the grid, or the test proves nothing.
    let exact_grid = trail_grid_size(EXACT_TARGET.0, EXACT_TARGET.1);
    let quantized_grid = trail_grid_size(QUANTIZED_TARGET.0, QUANTIZED_TARGET.1);
    assert_eq!(
        exact_grid, EXACT_TARGET,
        "{EXACT_TARGET:?} is no longer aspect-exact — pick a target whose axes are STEP multiples"
    );
    assert_ne!(
        quantized_grid, QUANTIZED_TARGET,
        "{QUANTIZED_TARGET:?} is no longer quantized up — the premise is gone"
    );
    let grid_ratio_gap = (quantized_grid.0 as f32 / quantized_grid.1 as f32)
        / (QUANTIZED_TARGET.0 as f32 / QUANTIZED_TARGET.1 as f32);
    assert!(
        (grid_ratio_gap - 1.0).abs() > 0.2,
        "the two targets' grid aspects are within 20% ({grid_ratio_gap:.3}) — too close to \
         distinguish projecting at the grid from projecting at the target"
    );

    let lively = AnalysisFrame {
        bass: 0.5,
        mid: 0.4,
        treb: 0.5,
        ..Default::default()
    };
    let Some(exact) = capture_at(EXACT_TARGET, DEJONG, &lively) else {
        return;
    };
    let Some(quantized) = capture_at(QUANTIZED_TARGET, DEJONG, &lively) else {
        return;
    };

    let exact_ratio = lit_bbox_ratio(&exact);
    let quantized_ratio = lit_bbox_ratio(&quantized);
    let skew = quantized_ratio / exact_ratio;
    println!(
        "lit bbox ratio: {EXACT_TARGET:?} grid {exact_grid:?} -> {exact_ratio:.3}, \
         {QUANTIZED_TARGET:?} grid {quantized_grid:?} -> {quantized_ratio:.3} (skew {skew:.3})"
    );
    // A margin, not a constant: quantization changes how far the glow's falloff is
    // resampled, so the outline crosses `EPS` at slightly different radii. 10% is
    // loose enough for that and tight enough to fail on the ~33% shape error.
    assert!(
        (skew - 1.0).abs() < 0.10,
        "the cloud's proportions follow the accumulation grid, not the target: the \
         {QUANTIZED_TARGET:?} capture (grid {quantized_grid:?}) is {skew:.3}x the aspect of the \
         aspect-exact {EXACT_TARGET:?} one — the projection is using the grid ratio"
    );
}
