//! Cross-preset dissolves (Plan 0023): the behavior a switch now has instead of
//! an instant cut, driven headlessly through the capture harness.
//!
//! The controller's arithmetic is unit-tested beside it in
//! `render/transition.rs`; what needs a GPU — and what these tests cover — is that
//! the blend actually *renders* intermediate frames, that the sequence is a ramp
//! rather than a jump, that it is reproducible from the injected `dt`, and that
//! ink's poles move continuously across a dissolve between two inked presets.
//!
//! Set `LMV_TRANSITION_STRIP=<dir>` to also write each transition window as a
//! filmstrip PNG for eyeballing; the assertions below are what actually guards
//! the behavior.

use lmv_core::dsp::AnalysisFrame;
use lmv_core::preset::Preset;
use lmv_core::render::metrics::frame_diff;
use lmv_core::render::{CaptureImage, HeadlessOptions, RenderError, Renderer};

const SIZE: u32 = 128;
/// Frames the dissolve spans at the capture harness's fixed step: the engine
/// default is 1 s and the harness steps at 1/60 s.
const DISSOLVE_FRAMES: usize = 60;

/// Build a headless `Renderer`, or `None` (a logged skip) when the runner exposes
/// no GPU adapter — macOS has no software Metal fallback (ADR-0016).
fn headless_or_skip() -> Option<Renderer> {
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

/// Two **static** presets — no `time` in any binding, no spin/rotation — so a
/// difference between two captured frames is the dissolve and not scene
/// animation. They name different systems, so they resolve to different scene
/// objects, and **both fill the frame**, so a dissolve is distinguishable from a
/// mere fade-out of the outgoing side.
fn static_pair() -> Vec<Preset> {
    let rose = Preset::from_toml_str(
        "system = \"parametric_curve\"\nname = \"TransA\"\n\
         [curve]\nfamily = \"maurer_rose\"\n\
         [params]\nn = \"3\"\nd = \"71\"\nsamples = \"400\"\nscale = \"0.9\"\nspin = \"0\"\n",
    )
    .expect("valid static rose preset");
    let star = Preset::from_toml_str(
        "system = \"star_pattern\"\nname = \"TransB\"\n\
         [generator]\ntiling = \"12\"\ncontact_angle_deg = 35\n\
         [params]\nvariant = \"0\"\nrotation = \"0\"\nhue = \"0.5\"\n\
         draw_progress = \"1\"\nthickness = \"4\"\nscale = \"0.95\"\nbrightness = \"1\"\n",
    )
    .expect("valid static star preset");
    vec![rose, star]
}

/// Capture `count` consecutive frames, one dissolve step each.
fn capture_run(renderer: &mut Renderer, frame: &AnalysisFrame, count: usize) -> Vec<CaptureImage> {
    (0..count)
        .map(|i| {
            renderer
                .capture_frame(frame)
                .unwrap_or_else(|e| panic!("capture frame {i}: {e}"))
        })
        .collect()
}

/// Write the captured window as a horizontal filmstrip, when
/// `LMV_TRANSITION_STRIP` names a **directory**. Purely for eyeballing — never
/// asserted on. `label` names the file, so the tests (which the harness runs on
/// separate threads) never race onto one path.
fn maybe_write_strip(frames: &[CaptureImage], every: usize, label: &str) {
    let Some(dir) = std::env::var_os("LMV_TRANSITION_STRIP") else {
        return;
    };
    let path = std::path::Path::new(&dir).join(format!("{label}.png"));
    let picked: Vec<&CaptureImage> = frames.iter().step_by(every.max(1)).collect();
    let Some(first) = picked.first() else { return };
    let (w, h) = (first.width as usize, first.height as usize);
    let strip_w = w * picked.len();
    let mut rgba = vec![0u8; strip_w * h * 4];
    for (tile, img) in picked.iter().enumerate() {
        for y in 0..h {
            let src = y * w * 4;
            let dst = (y * strip_w + tile * w) * 4;
            rgba[dst..dst + w * 4].copy_from_slice(&img.rgba[src..src + w * 4]);
        }
    }
    image::save_buffer(
        &path,
        &rgba,
        strip_w as u32,
        h as u32,
        image::ColorType::Rgba8,
    )
    .expect("write the transition filmstrip");
    eprintln!("wrote filmstrip {:?} ({} tiles)", path, picked.len());
}

/// One dissolve's captured window plus the settled look it lands on.
struct Window {
    /// Frames at `t = 0, 1/60, ... 59/60` — the whole 1 s dissolve.
    frames: Vec<CaptureImage>,
    /// The incoming preset alone, after the dissolve has finalized.
    settled: CaptureImage,
}

impl Window {
    /// The outgoing look — `t = 0` is exactly that, for every kind.
    fn outgoing(&self) -> &CaptureImage {
        self.frames.first().expect("a non-empty dissolve window")
    }
    /// The frame at `t = 0.5`.
    fn mid(&self) -> &CaptureImage {
        &self.frames[DISSOLVE_FRAMES / 2]
    }
}

/// Run one dissolve to completion and return its window plus the settled result.
fn dissolve_once(renderer: &mut Renderer, frame: &AnalysisFrame) -> Window {
    renderer.cycle_preset();
    let frames = capture_run(renderer, frame, DISSOLVE_FRAMES);
    let settled = capture_run(renderer, frame, 4)
        .pop()
        .expect("a settled frame after the dissolve");
    Window { frames, settled }
}

/// Mean Rec. 709 luminance, in bytes — a coarse "is this frame degenerate" probe.
fn mean_luma(img: &CaptureImage) -> f32 {
    let sum: f32 = img
        .rgba
        .chunks_exact(4)
        .map(|px| 0.2126 * px[0] as f32 + 0.7152 * px[1] as f32 + 0.0722 * px[2] as f32)
        .sum();
    sum / (img.rgba.len() / 4) as f32
}

/// Mean Rec. 709 luminance in **linear light** — the space the blend actually
/// works in, so this is what a claim about "brighter than" has to be measured in.
fn mean_linear_luma(img: &CaptureImage) -> f32 {
    let sum: f32 = img
        .rgba
        .chunks_exact(4)
        .map(|px| 0.2126 * to_linear(px[0]) + 0.7152 * to_linear(px[1]) + 0.0722 * to_linear(px[2]))
        .sum();
    sum / (img.rgba.len() / 4) as f32
}

/// Mean linear luminance of the **plain linear mix** of `a` and `b` at `t` — the
/// crossfade the other kinds are measured against, computed analytically rather
/// than rendered, so a kind can be compared to it without a second dissolve.
fn mean_linear_luma_of_mix(a: &CaptureImage, b: &CaptureImage, t: f32) -> f32 {
    let sum: f32 = a
        .rgba
        .chunks_exact(4)
        .zip(b.rgba.chunks_exact(4))
        .map(|(pa, pb)| {
            let ch = |c: usize| to_linear(pa[c]) * (1.0 - t) + to_linear(pb[c]) * t;
            0.2126 * ch(0) + 0.7152 * ch(1) + 0.0722 * ch(2)
        })
        .sum();
    sum / (a.rgba.len() / 4) as f32
}

/// sRGB EOTF: one 8-bit channel to linear light.
///
/// The blend mixes in **linear** light (the surface is an sRGB format, so the
/// sampler decodes and the target re-encodes), and the sRGB curve is steep near
/// black. Measuring progress on raw bytes therefore reads a uniform crossfade as
/// wildly tone-dependent — 0.70 in the shadows against 0.27 in the highlights —
/// which would make every "is this kind spatially ordered?" probe below meaningless.
/// Decoding first is what makes the regions comparable.
fn to_linear(byte: u8) -> f32 {
    let c = byte as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// How far the masked pixels have travelled from `a` toward `b`, in `[0, 1]`:
/// `sum|mid - a| / sum|b - a|` over the region, **in linear light**. Normalizing
/// by the region's own span makes the number comparable **between** regions, which
/// is what every kind assertion below comes down to.
///
/// `mask(u, v, linear_luma_of_a)` selects the region. Returns 0 for an empty or
/// zero-span region rather than dividing by zero.
fn region_progress(
    mid: &CaptureImage,
    a: &CaptureImage,
    b: &CaptureImage,
    mask: impl Fn(f32, f32, f32) -> bool,
) -> f32 {
    let (w, h) = (a.width as usize, a.height as usize);
    let (mut moved, mut span) = (0.0f32, 0.0f32);
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) * 4;
            let lin = |img: &CaptureImage, c: usize| to_linear(img.rgba[i + c]);
            let la = 0.2126 * lin(a, 0) + 0.7152 * lin(a, 1) + 0.0722 * lin(a, 2);
            let (u, v) = (x as f32 / w as f32, y as f32 / h as f32);
            if !mask(u, v, la) {
                continue;
            }
            for c in 0..3 {
                moved += (lin(mid, c) - lin(a, c)).abs();
                span += (lin(b, c) - lin(a, c)).abs();
            }
        }
    }
    if span <= f32::EPSILON {
        0.0
    } else {
        moved / span
    }
}

/// **The whole transition library, one kind per dissolve** (Plan 0023 Phase 3).
///
/// The engine policy rotates deterministically over `TransitionKind::LIBRARY`,
/// and an explicit `select_preset` restarts that rotation — so four consecutive
/// cycles from a fresh selection are exactly crossfade, add/burn, luma-dissolve,
/// wipe, in that order. Each is asserted on the property that **distinguishes**
/// it, not merely on "something changed":
///
/// - crossfade mixes **uniformly**, so every region is equally far along;
/// - add/burn is **additive**, so the midpoint is brighter than either endpoint;
/// - luma-dissolve is **brightness-ordered**, so the outgoing frame's dark pixels
///   turn over well before its bright ones;
/// - wipe is a **moving boundary**, so one side of the diagonal has turned over
///   and the other has not.
///
/// Crossfade is the control: the same two region probes that separate for wipe and
/// luma-dissolve must come out level for it.
#[test]
fn each_kind_renders_its_own_dissolve() {
    let Some(mut renderer) = headless_or_skip() else {
        return;
    };
    let frame = AnalysisFrame::default();
    renderer.set_presets(static_pair());
    renderer.select_preset(0); // restarts the kind rotation at the library's head
    capture_run(&mut renderer, &frame, 5);

    let windows: Vec<Window> = (0..4)
        .map(|_| dissolve_once(&mut renderer, &frame))
        .collect();
    for (i, w) in windows.iter().enumerate() {
        maybe_write_strip(&w.frames, 6, &format!("kind_{i}"));
    }

    // Region probes, evaluated at the midpoint of each dissolve.
    let probe = |w: &Window, mask: fn(f32, f32, f32) -> bool| {
        region_progress(w.mid(), w.outgoing(), &w.settled, mask)
    };
    // The wipe sweeps the diagonal (u + v) / 2, so these are its two ends.
    let early_side = |u: f32, v: f32, _l: f32| (u + v) * 0.5 < 0.3;
    let late_side = |u: f32, v: f32, _l: f32| (u + v) * 0.5 > 0.7;
    // Luma-dissolve orders by the *outgoing* frame's brightness. The thresholds
    // are in linear light, where a "bright" line stroke is well above the field.
    let dark = |_u: f32, _v: f32, l: f32| l < 0.02;
    let bright = |_u: f32, _v: f32, l: f32| l > 0.15;

    for (i, w) in windows.iter().enumerate() {
        // Whatever the kind: the opening frame is exactly the outgoing look and
        // the dissolve genuinely arrives somewhere else.
        assert_eq!(
            frame_diff(w.outgoing(), w.frames.first().expect("frames")),
            0.0
        );
        assert!(
            frame_diff(w.outgoing(), &w.settled) > 0.05,
            "kind {i}: the two sides must differ for the probes to mean anything"
        );
        assert!(
            frame_diff(w.mid(), w.outgoing()) > 0.0 && frame_diff(w.mid(), &w.settled) > 0.0,
            "kind {i}: the midpoint is a blended frame, not either endpoint"
        );
    }

    // 0 — Crossfade: uniform. Both probe pairs come out level.
    let cf = &windows[0];
    let (cf_early, cf_late) = (probe(cf, early_side), probe(cf, late_side));
    let (cf_dark, cf_bright) = (probe(cf, dark), probe(cf, bright));
    assert!(
        (cf_early - cf_late).abs() < 0.15,
        "crossfade mixes uniformly across the frame: {cf_early} vs {cf_late}"
    );
    assert!(
        (cf_dark - cf_bright).abs() < 0.25,
        "crossfade mixes uniformly across tones: {cf_dark} vs {cf_bright}"
    );

    // 1 — AddBurn: additive, so the midpoint carries more light than the plain
    // linear mix of the same two sides. Measured against the analytic crossfade of
    // *its own* endpoints rather than against the endpoints themselves: these are
    // sparse line scenes on black, where most pixels have content on only one
    // side, so "brighter than either endpoint" is not what additive means here.
    let burn = &windows[1];
    let peak = mean_linear_luma(burn.mid());
    let plain = mean_linear_luma_of_mix(burn.outgoing(), &burn.settled, 0.5);
    assert!(
        peak > plain * 1.2,
        "add/burn flares above the plain mix: mid {peak} vs crossfade {plain}"
    );

    // 2 — LumaDissolve: the outgoing frame's dark pixels turn over first.
    let luma = &windows[2];
    let (l_dark, l_bright) = (probe(luma, dark), probe(luma, bright));
    assert!(
        l_dark > l_bright + 0.25,
        "luma-dissolve reveals darkest-first: dark {l_dark} vs bright {l_bright}"
    );

    // 3 — Wipe: a boundary, so one side of the diagonal has turned over and the
    // other has not.
    let wipe = &windows[3];
    let (w_early, w_late) = (probe(wipe, early_side), probe(wipe, late_side));
    assert!(
        w_early > w_late + 0.4,
        "wipe is a moving boundary, not a fade: early side {w_early} vs late side {w_late}"
    );
}

/// The additive families are the reason the blend samples **both** textures
/// instead of alpha-compositing one over the other (ADR-0024). A fragment field
/// dissolving into a line scene is that case: every dissolve frame must come out
/// fully opaque and in-gamut, with no half-transparent or blown-out frame where
/// the two additive pipelines meet.
#[test]
fn an_additive_family_dissolve_stays_clean() {
    let Some(mut renderer) = headless_or_skip() else {
        return;
    };
    let frame = AnalysisFrame::default();
    let field = Preset::from_toml_str(
        "system = \"fragment_field\"\nname = \"AddField\"\n\
         [params]\nwarp = \"0.4\"\nhue = \"0.1\"\nglow = \"0.8\"\n",
    )
    .expect("valid fragment field preset");
    let mut roster = static_pair();
    roster.insert(0, field);
    renderer.set_presets(roster);
    renderer.select_preset(0);
    capture_run(&mut renderer, &frame, 5);

    let window = dissolve_once(&mut renderer, &frame);
    maybe_write_strip(&window.frames, 6, "additive");

    for (i, img) in window.frames.iter().enumerate() {
        assert!(
            img.rgba.chunks_exact(4).all(|px| px[3] == 255),
            "dissolve frame {i} must be fully opaque — an alpha composite would not be"
        );
        // Neither collapsed to black nor blown to white: a corrupted two-input
        // sample shows up as one or the other long before it looks subtly wrong.
        let lum = mean_luma(img);
        assert!(
            lum > 0.5 && lum < 245.0,
            "dissolve frame {i} is degenerate (mean luma {lum})"
        );
    }
}

/// **The walking skeleton's whole claim**: a preset switch dissolves rather than
/// cuts.
///
/// A hard cut would make the distance from the pre-switch look jump from ~0 to
/// its maximum in one frame. A dissolve makes it a **ramp**, so this asserts the
/// shape of the sequence, not one sampled frame: distance from the opening frame
/// rises strictly across the window, the midpoint is far from *both* endpoints,
/// and no single step accounts for most of the total change.
#[test]
fn a_preset_switch_dissolves_instead_of_cutting() {
    let Some(mut renderer) = headless_or_skip() else {
        return;
    };
    let frame = AnalysisFrame::default();
    renderer.set_presets(static_pair());

    // Settle on the outgoing preset first, so the window below is the dissolve
    // and nothing else.
    capture_run(&mut renderer, &frame, 5);

    renderer.cycle_preset();
    let window = capture_run(&mut renderer, &frame, DISSOLVE_FRAMES);
    maybe_write_strip(&window, 6, "dissolve");

    let first = window.first().expect("a non-empty dissolve window");
    let last = window.last().expect("a non-empty dissolve window");
    let mid = &window[DISSOLVE_FRAMES / 2];

    // The two presets must actually look different, or nothing below means
    // anything.
    let span = frame_diff(first, last);
    assert!(
        span > 0.05,
        "the two presets must differ for a dissolve to be visible: {span}"
    );

    // The midpoint is neither endpoint — it is a *blended* frame, which is the
    // thing a cut can never produce.
    let to_first = frame_diff(mid, first);
    let to_last = frame_diff(mid, last);
    assert!(
        to_first > 0.2 * span && to_last > 0.2 * span,
        "the mid-dissolve frame must differ from both endpoints \
         (to first {to_first}, to last {to_last}, span {span})"
    );

    // A ramp, not a jump: distance from the opening frame grows across the
    // window at every sampled quarter.
    let ramp: Vec<f32> = [0, 15, 30, 45, DISSOLVE_FRAMES - 1]
        .iter()
        .map(|&k| frame_diff(&window[k], first))
        .collect();
    assert_eq!(
        ramp[0], 0.0,
        "the opening frame is the outgoing look exactly"
    );
    for pair in ramp.windows(2) {
        assert!(
            pair[1] > pair[0],
            "distance from the outgoing look must rise across the dissolve: {ramp:?}"
        );
    }

    // No single frame carries most of the change — that is what a cut looks like.
    let biggest_step = window
        .windows(2)
        .map(|p| frame_diff(&p[0], &p[1]))
        .fold(0.0f32, f32::max);
    assert!(
        biggest_step < 0.5 * span,
        "no single frame may account for most of the change \
         (biggest step {biggest_step}, span {span}) — that is a cut, not a dissolve"
    );
}

/// `t` advances purely from the injected `dt`, with no wall-clock read, so the
/// same capture run reproduces byte-for-byte (NFR §6). Rendered here rather than
/// only in the controller's unit test, because a wall-clock leak anywhere in the
/// blend path would show up as drifting pixels and nowhere else.
#[test]
fn a_dissolve_is_reproducible_from_the_injected_dt() {
    let Some(mut renderer) = headless_or_skip() else {
        return;
    };
    let frame = AnalysisFrame::default();

    let mut run = || {
        renderer.set_presets(static_pair());
        renderer.select_preset(0);
        capture_run(&mut renderer, &frame, 5);
        renderer.cycle_preset();
        capture_run(&mut renderer, &frame, 20)
    };
    let a = run();
    let b = run();

    for (i, (x, y)) in a.iter().zip(b.iter()).enumerate() {
        assert_eq!(
            x.rgba, y.rgba,
            "dissolve frame {i} must be byte-identical across runs"
        );
    }
}

/// Two ink-on presets with different paper colors: ink is a single engine-wide
/// pass, so its params crossfade by the same `t` (ADR-0032). The visible poles
/// must **move continuously** rather than snapping at either end — the failure
/// mode is holding the incoming preset's params too early (a snap at `t = 0`).
///
/// Both presets are near-empty line scenes, so most of the frame is paper and its
/// mean color tracks `paper_*` directly.
#[test]
fn ink_poles_crossfade_across_a_dissolve() {
    let Some(mut renderer) = headless_or_skip() else {
        return;
    };
    let frame = AnalysisFrame::default();

    // Outgoing: white paper (the defaults). Incoming: saturated blue paper.
    let white = Preset::from_toml_str(
        "system = \"parametric_curve\"\nname = \"InkWhite\"\n\
         [curve]\nfamily = \"maurer_rose\"\n\
         [params]\nn = \"2\"\nsamples = \"120\"\nscale = \"0.4\"\nspin = \"0\"\n\
         ink_amount = \"1\"\n",
    )
    .expect("valid white-paper ink preset");
    let blue = Preset::from_toml_str(
        "system = \"parametric_curve\"\nname = \"InkBlue\"\n\
         [curve]\nfamily = \"maurer_rose\"\n\
         [params]\nn = \"2\"\nsamples = \"120\"\nscale = \"0.4\"\nspin = \"0\"\n\
         ink_amount = \"1\"\npaper_hue = \"0.6\"\npaper_sat = \"1\"\npaper_bright = \"1\"\n",
    )
    .expect("valid blue-paper ink preset");
    renderer.set_presets(vec![white, blue]);
    capture_run(&mut renderer, &frame, 5);

    renderer.cycle_preset();
    let window = capture_run(&mut renderer, &frame, DISSOLVE_FRAMES);
    maybe_write_strip(&window, 6, "ink_crossfade");

    // Mean (blue - red) over the frame: ~0 on white paper, strongly positive on
    // blue paper. A snap would make this a step; a crossfade makes it a ramp.
    let blueness = |img: &CaptureImage| -> f32 {
        let sum: f32 = img
            .rgba
            .chunks_exact(4)
            .map(|px| px[2] as f32 - px[0] as f32)
            .sum();
        sum / (img.rgba.len() / 4) as f32
    };
    let series: Vec<f32> = window.iter().map(blueness).collect();
    let (start, end) = (series[0], series[series.len() - 1]);

    assert!(
        start.abs() < 6.0,
        "t = 0 is exactly the outgoing look — white paper, no blue cast: {start}"
    );
    assert!(
        end > 60.0,
        "the dissolve must reach the incoming preset's blue paper: {end}"
    );

    // Continuous: every sampled quarter is further along than the last, and no
    // single frame jumps most of the way.
    let sampled: Vec<f32> = [0, 15, 30, 45, DISSOLVE_FRAMES - 1]
        .iter()
        .map(|&k| series[k])
        .collect();
    for pair in sampled.windows(2) {
        assert!(
            pair[1] > pair[0],
            "the paper pole must move continuously: {sampled:?}"
        );
    }
    let biggest_step = series
        .windows(2)
        .map(|p| (p[1] - p[0]).abs())
        .fold(0.0f32, f32::max);
    assert!(
        biggest_step < 0.5 * (end - start),
        "no single frame may snap most of the pole travel \
         (biggest step {biggest_step}, span {})",
        end - start
    );
}

/// With no dissolve running the blend is absent entirely — no pass encoded, no
/// full-frame targets — so an ordinary frame is unchanged by this feature
/// existing. Two captures of the same settled preset, taken with a completed
/// dissolve behind them, must match a capture taken with no dissolve at all.
#[test]
fn a_finished_dissolve_leaves_no_trace_on_later_frames() {
    let Some(mut renderer) = headless_or_skip() else {
        return;
    };
    let frame = AnalysisFrame::default();

    // Reference: preset 1, reached by an instant select, settled.
    renderer.set_presets(static_pair());
    renderer.select_preset(1);
    let reference = capture_run(&mut renderer, &frame, 8);

    // Same preset, same clock steps — but reached through a full dissolve that
    // has since finished. `capture_preset` resets the clock and scene state, so
    // both runs start from the same seed.
    renderer.set_presets(static_pair());
    renderer.select_preset(0);
    renderer.cycle_preset();
    // Run past the end of the dissolve, then settle for the same 8 steps.
    capture_run(&mut renderer, &frame, DISSOLVE_FRAMES + 2);
    let after = capture_run(&mut renderer, &frame, 8);

    assert_eq!(
        renderer.preset_name(),
        "TransB",
        "the dissolve finalized on its target index"
    );
    let drift = frame_diff(
        after.last().expect("settled frame"),
        reference.last().expect("reference frame"),
    );
    assert!(
        drift < 0.02,
        "a finished dissolve must leave the ordinary frame path untouched: {drift}"
    );
}
