//! GPU compute-particle attractor contract (Plan 0016 Phase 5, HARD). The
//! attractor scene is the engine's first *compute pipeline* + GPU-resident
//! particle system, so beyond the generic per-preset gates (sanity / animation /
//! reactivity, which already include the shipped attractor presets) it gets
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
/// A 2D map preset and a 3D flow preset from the embedded set — one of each
/// idiom the scene supports. Repointed at Plan 0075 cohort five, which retired
/// the De Jong and Lorenz presets: Ink on Paper carries the same `de_jong`
/// family (and, deliberately, no trails stage, so the bit-exact
/// reproducibility check below stays clear of WARP's trails quirks); Thomas is
/// the remaining continuous flow. The bare inline `lorenz`-family presets
/// further down are unaffected — the family still ships, only its preset
/// retired.
const MAP_2D: &str = "Ink on Paper";
const FLOW_3D: &str = "Thomas";

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

/// The vertical position of the lit region's centre of mass, as a signed fraction
/// of frame height away from the centre line. Negative is above centre (row index
/// grows downward).
fn lit_centroid_offset(img: &CaptureImage) -> f32 {
    let bg = background(img);
    let (mut sum_y, mut n) = (0f64, 0u64);
    for (i, px) in img.rgba.chunks_exact(4).enumerate() {
        let lit = px
            .iter()
            .zip(bg.iter())
            .take(3)
            .any(|(&c, &b)| c.abs_diff(b) > EPS);
        if lit {
            sum_y += f64::from(i as u32 / img.width);
            n += 1;
        }
    }
    assert!(n > 0, "no lit pixels to take a centroid of");
    (sum_y / n as f64 / f64::from(img.height) - 0.5) as f32
}

/// Lit width per row, plus the figure's first and last lit row. The width is the
/// span from leftmost to rightmost lit pixel, so a row's interior gaps do not
/// reduce it — this measures the figure's silhouette, not its fill.
fn lit_row_widths(img: &CaptureImage) -> (Vec<u32>, usize, usize) {
    let bg = background(img);
    let (w, h) = (img.width as usize, img.height as usize);
    let (mut lo, mut hi) = (vec![u32::MAX; h], vec![0u32; h]);
    for (i, px) in img.rgba.chunks_exact(4).enumerate() {
        let lit = px
            .iter()
            .zip(bg.iter())
            .take(3)
            .any(|(&c, &b)| c.abs_diff(b) > EPS);
        if !lit {
            continue;
        }
        let (x, y) = ((i % w) as u32, i / w);
        let l = lo.get_mut(y).expect("row within height");
        *l = (*l).min(x);
        let r = hi.get_mut(y).expect("row within height");
        *r = (*r).max(x);
    }
    let widths: Vec<u32> = lo
        .iter()
        .zip(hi.iter())
        .map(|(&l, &r)| if l == u32::MAX { 0 } else { r - l + 1 })
        .collect();
    let top = widths.iter().position(|&v| v > 0).expect("some lit row");
    let bot = h
        - 1
        - widths
            .iter()
            .rev()
            .position(|&v| v > 0)
            .expect("some lit row");
    (widths, top, bot)
}

/// A bare attractor preset for one family — no backdrop, no bloom, so the
/// measured figure is the scene's own geometry. **Not the shipped preset**: those
/// bind `bg_vignette`, which is itself a large, vertically symmetric lit region
/// and swamps any shape statistic taken against a corner pixel (ADR-0067).
fn attractor_bare_preset(name: &str, family: &str, extra: &str) -> Preset {
    let toml = format!(
        "system = \"attractor\"\nname = \"{name}\"\n[particles]\nfamily = \"{family}\"\n\
         [params]\nsize = \"0.4\"\nfade = \"0.62\"\n{extra}"
    );
    Preset::from_toml_str(&toml).unwrap_or_else(|e| panic!("{name} preset parses: {e}"))
}

/// `pan_y` probe magnitude, in NDC. The frame spans 2 NDC units vertically, so a
/// pan of `p` displaces the figure by `p/2` of frame height and the **separation**
/// between `+p` and `-p` captures is `p` — which is why the assertion below reads
/// against `PAN_PROBE` directly.
const PAN_PROBE: f32 = 0.30;

/// Which pixels are lit, as a mask over the frame — the figure's **geometry**,
/// separated from how bright it is (Plan 0066 Phase 1).
fn lit_mask(img: &CaptureImage) -> Vec<bool> {
    let bg = background(img);
    img.rgba
        .chunks_exact(4)
        .map(|px| {
            px.iter()
                .zip(bg.iter())
                .take(3)
                .any(|(&c, &b)| c.abs_diff(b) > EPS)
        })
        .collect()
}

/// Mean per-pixel luma over the pixels `mask` selects — the figure's **level**,
/// measured over a set fixed from outside so two captures are compared over the
/// same pixels rather than each over its own.
fn mean_luma_over(img: &CaptureImage, mask: &[bool]) -> f32 {
    let (mut sum, mut n) = (0.0f64, 0u64);
    for (px, lit) in img.rgba.chunks_exact(4).zip(mask.iter()) {
        if !lit {
            continue;
        }
        sum += 0.299 * f64::from(px[0]) + 0.587 * f64::from(px[1]) + 0.114 * f64::from(px[2]);
        n += 1;
    }
    assert!(n > 0, "the mask selects no pixels");
    (sum / n as f64) as f32
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

    // --- Shape sanity: the 2D map figure is neither blank nor a single dot.
    // (Under Ink on Paper the sampled background is the paper, so "lit" below
    // means the drawn strokes.) ---
    let warm = renderer
        .capture_preset(MAP_2D, &lively, 60)
        .expect("capture the 2D map preset @60");
    let bg = background(&warm);
    let cov = coverage(&warm, bg, EPS);
    let spread = quadrant_spread(&warm, bg, EPS);
    assert!(cov > 0.02, "2D map figure is blank: coverage {cov:.4}");
    assert!(spread >= 2, "2D map figure is a dot: {spread} quadrant(s)");

    // --- Seed reproducibility (Phase 1 determinism done-when): the seeded init +
    // deterministic compute step reproduce bit-for-bit on the same adapter — the
    // property a GPU-resident particle sim most easily loses. ---
    let a = renderer
        .capture_preset(MAP_2D, &lively, 48)
        .expect("capture A");
    let b = renderer
        .capture_preset(MAP_2D, &lively, 48)
        .expect("capture B");
    assert_eq!(
        a.rgba, b.rgba,
        "attractor capture is not reproducible for a fixed input"
    );

    // --- Animation: a later frame differs from an earlier one (boiling + spin +
    // trails move it), not frozen. ---
    let early = renderer
        .capture_preset(MAP_2D, &lively, 24)
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
        .capture_preset(MAP_2D, &calm, 60)
        .expect("capture calm");
    let with = renderer
        .capture_preset(MAP_2D, &beat, 60)
        .expect("capture beat");
    let delta = frame_diff(&without, &with);
    assert!(delta > 0.003, "beat did not perturb the cloud: {delta:.4}");

    // --- 3D flow: the Thomas lattice renders a real shape, exercising the
    // continuous-family compute path (Euler integration + 3D projection). ---
    let flow = renderer
        .capture_preset(FLOW_3D, &lively, 90)
        .expect("capture the 3D flow preset @90");
    let lbg = background(&flow);
    let lcov = coverage(&flow, lbg, EPS);
    let lspread = quadrant_spread(&flow, lbg, EPS);
    assert!(lcov > 0.02, "3D flow figure is blank: coverage {lcov:.4}");
    assert!(
        lspread >= 2,
        "3D flow figure is a dot: {lspread} quadrant(s)"
    );

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

    // --- `pan_y` moves the figure (Plan 0059 Phase 1b, ADR-0070) --------------
    //
    // The check above binds `pan_x`, and a horizontal pan is exactly the axis a
    // *vertical* mirror leaves alone — which is why it passed for the whole life
    // of the bug it could not see. The attractor's decay pass sampled the
    // accumulation target with the unflipped fullscreen prelude while the draw
    // pass wrote that same target in clip space, so the feedback re-read its own
    // history mirrored and the steady state was `figure ∪ mirror(figure)`. A
    // doubled figure is symmetric about the centre line **by construction**, so
    // its centroid is pinned there no matter what `pan_y` says.
    //
    // Measured both ways at 96x96 rather than argued (the numbers are the reason
    // for the threshold):
    //
    //   centroid(-0.30) - centroid(+0.30)   pre-fix 0.050   post-fix 0.300
    //
    // The geometric expectation is exactly `PAN_PROBE` — post-fix reproduces it
    // to three decimals, and the defect delivers a sixth of it. Taking the
    // **separation** between opposite pans rather than an absolute offset cancels
    // the figure's own centroid, which is not at the centre line and should not
    // have to be. 0.20 sits 4x above the broken value with 1.5x of headroom under
    // the true one, which is the room the `EPS` cut and edge clipping need.
    renderer.set_presets(vec![
        attractor_view_preset("at_pan_up", &format!("pan_y = \"{PAN_PROBE}\"\n")),
        attractor_view_preset("at_pan_down", &format!("pan_y = \"-{PAN_PROBE}\"\n")),
    ]);
    let up = renderer
        .capture_preset("at_pan_up", &lively, 60)
        .expect("capture at_pan_up");
    let down = renderer
        .capture_preset("at_pan_down", &lively, 60)
        .expect("capture at_pan_down");
    let (up_c, down_c) = (lit_centroid_offset(&up), lit_centroid_offset(&down));
    let separation = down_c - up_c;
    println!(
        "pan_y centroid: +{PAN_PROBE} -> {up_c:+.4}, -{PAN_PROBE} -> {down_c:+.4}, \
         separation {separation:.4} (geometry predicts {PAN_PROBE:.4})"
    );
    // Signed, not `abs()`: a chain with *both* flips wrong renders the figure
    // upside down but un-mirrored, which would satisfy a magnitude test while
    // moving the picture the wrong way. This fails on it.
    assert!(
        separation > 0.20,
        "`pan_y` does not move the attractor: centroid separation between \
         pan_y = +{PAN_PROBE} and -{PAN_PROBE} is {separation:.4}, against {PAN_PROBE:.2} from the \
         geometry. Below ~0.05 the trail is mirroring itself and the doubled figure's centroid is \
         pinned to the centre line (ADR-0070); a negative value means the vertical axis is inverted"
    );

    // --- Lorenz is the butterfly, not the bowtie (Plan 0059 Phase 1b) ---------
    //
    // Orientation asserted against the attractor's own data rather than taste:
    // the particle buffer puts high `z` at the wing tips (|x| > 14 -> mean z 36.9;
    // |x| < 2 -> 19.8), so viewed x-z with +z up the wings splay upward and the
    // figure is **top-heavy** — wide wings above, a converging tail below. The
    // spin cannot spoil this: it is a turntable about the vertical, so it changes
    // the horizontal extent at every angle and the vertical profile at none.
    //
    // Measured, upper-half lit area / lower-half:  pre-fix 0.955   post-fix 1.467
    //
    // Pre-fix sits at 1.0 because the mirror doubling makes the figure symmetric
    // by construction — so this discriminates all three states: doubled (~1.0),
    // correctly oriented (>1.25), and both-flips-wrong (upside down, <1.0).
    renderer.set_presets(vec![attractor_bare_preset("at_lorenz_bare", "lorenz", "")]);
    let bare = renderer
        .capture_preset("at_lorenz_bare", &lively, 90)
        .expect("capture at_lorenz_bare");
    let (widths, top, bot) = lit_row_widths(&bare);
    let half = (bot - top) / 2;
    let upper: u64 = widths
        .get(top..top + half)
        .expect("upper half within figure")
        .iter()
        .map(|&v| u64::from(v))
        .sum();
    let lower: u64 = widths
        .get(bot - half..=bot)
        .expect("lower half within figure")
        .iter()
        .map(|&v| u64::from(v))
        .sum();
    let top_heavy = upper as f32 / lower.max(1) as f32;
    println!("Lorenz upper/lower lit area = {top_heavy:.3} (rows {top}..{bot})");
    assert!(
        top_heavy > 1.25,
        "Lorenz is not top-heavy: upper/lower lit area {top_heavy:.3}. At ~1.0 the trail is \
         mirroring itself and the butterfly is a symmetric bowtie (ADR-0070); below 1.0 the \
         vertical axis is inverted and the wings are at the bottom"
    );

    // --- `brightness` is a LEVEL, not a geometry (Plan 0066 Phase 1, ADR-0080) --
    //
    // The scene-local level the two sibling particle families already carried, and
    // the property that makes it a *level*: at half the value the figure lights
    // the same pixels and is dimmer over them. It multiplies the per-particle
    // deposit, the trail accumulates linearly and everything up to the tonemap is
    // linear, so in exact arithmetic the whole field is scaled and nothing moves.
    //
    // Appended at the end of this test, not inserted, so every capture above is
    // still taken from the device state it always was.
    //
    // **The bare preset, not the view one, and that is measurement rather than
    // taste.** `attractor_view_preset` runs `size = 1.0` at the default
    // `fade = 0.94`, which is far enough over range that the tonemap's shoulder
    // flattens the whole figure: measured, brightness 1.0 / 0.5 / 0.25 / 0.1 all
    // peak at 255 and their mean luma moves 68.6 -> 68.8 -> 73.4 -> 74.4, i.e. not
    // monotone at all. That is the shoulder doing its job on a badly exposed
    // fixture — and it is exactly the state the two shipped attractor presets were
    // in when they reached for `exposure = 0.03` (ADR-0080). At the bare preset's
    // `size = 0.4` / `fade = 0.62` the same sweep reads 57.5 / 47.3 / 37.0 / 26.2,
    // which is the linear regime this property is a claim about.
    //
    // The lit set is taken from the **dim** capture and both are measured over it.
    // Halving the light drops the figure's faintest fringe under the `EPS` byte
    // cut, so the dim set is a subset of the bright one by construction and
    // asserting raw set equality would be asserting something about the cut. The
    // real claims are that the dim figure lights **no pixel of its own** (a
    // geometry change would) and keeps nearly all of the bright one.
    renderer.set_presets(vec![
        attractor_bare_preset("at_bright_full", "de_jong", "brightness = \"1.0\"\n"),
        attractor_bare_preset("at_bright_half", "de_jong", "brightness = \"0.5\"\n"),
    ]);
    let full = renderer
        .capture_preset("at_bright_full", &lively, 60)
        .expect("capture at_bright_full");
    let dim = renderer
        .capture_preset("at_bright_half", &lively, 60)
        .expect("capture at_bright_half");
    let (full_mask, dim_mask) = (lit_mask(&full), lit_mask(&dim));
    let full_lit = full_mask.iter().filter(|&&l| l).count();
    let dim_lit = dim_mask.iter().filter(|&&l| l).count();
    let only_dim = full_mask
        .iter()
        .zip(dim_mask.iter())
        .filter(|&(&f, &d)| d && !f)
        .count();
    let full_mean = mean_luma_over(&full, &dim_mask);
    let dim_mean = mean_luma_over(&dim, &dim_mask);
    println!(
        "brightness 1.0 -> {full_lit} lit px, mean {full_mean:.2}; \
         0.5 -> {dim_lit} lit px, mean {dim_mean:.2}; lit only at 0.5: {only_dim}"
    );
    assert_eq!(
        only_dim, 0,
        "`brightness = 0.5` lit {only_dim} pixels `brightness = 1.0` did not. A level may \
         only scale the light the figure already lays down; a pixel appearing where the \
         brighter render has none means the param moved the geometry"
    );
    assert!(
        dim_lit * 10 >= full_lit * 9,
        "`brightness = 0.5` kept only {dim_lit} of {full_lit} lit pixels. Losing the faintest \
         fringe to the EPS cut is expected (measured: 1023 of 1094); losing a tenth of the \
         figure is the param doing something other than scaling the level"
    );
    assert!(
        dim_mean < full_mean * 0.9,
        "`brightness = 0.5` is not dimmer over the pixels it lights: mean luma {dim_mean:.2} \
         against {full_mean:.2} at 1.0"
    );
    // And it is still a pure function of its params.
    let dim_again = renderer
        .capture_preset("at_bright_half", &lively, 60)
        .expect("capture at_bright_half again");
    assert_eq!(
        dim.rgba, dim_again.rgba,
        "a `brightness`-bound attractor capture is not reproducible"
    );
}

/// **A colour lever moves colour and leaves geometry alone** — the two IFS tint
/// channels, asserted the way ADR-0087 distinguishes them from a shape param
/// (Plan 0073 Phase 4).
///
/// Two captures differing **only** in `map_tint` (then only in `root_tint`) must
/// have measurably different colour distributions over the lit region while
/// lighting the *same* pixels. That pairing is the whole test: a param that moved
/// the figure would change the lit set, and a param that did nothing would leave
/// the colours alone. Either alone would pass on a broken build.
///
/// Chromaticity rather than luma, because the claim is about hue: the mean
/// `R − G` and `G − B` over the lit set move under a tint and would not under a
/// level change.
#[test]
fn the_ifs_tint_channels_move_colour_without_moving_the_figure() {
    let Some(mut renderer) = headless() else {
        return;
    };
    // A sustained mid-energy frame; no beat. The bare preset binds no
    // expressions, so this only has to be a valid frame.
    let lively = AnalysisFrame {
        bass: 0.5,
        mid: 0.4,
        treb: 0.5,
        ..Default::default()
    };

    // A bare fern: no backdrop and no bloom, so the measured pixels are the
    // figure's own and the corner-pixel background is uniform (ADR-0067).
    // `root_tint` replaced `age_tint` here at Plan 0074 Phase 3, and it is bound
    // at **1.5 rather than 0.7** on purpose. `map_tint` is centred over a channel
    // that spans [0, 1], so `0.7` buys 0.7 of palette travel; `root_tint` is
    // anchored over one that tops out at 0.461 on the fern (ADR-0088), so the
    // equivalent authority is `0.7 / 0.461 ~ 1.5`. Binding both at 0.7 would
    // compare a full-strength lever against a 46 % one and call the difference a
    // property of the channel.
    renderer.set_presets(vec![
        attractor_bare_preset("at_tint_off", "fern", ""),
        attractor_bare_preset("at_map_tint", "fern", "map_tint = \"0.7\"\n"),
        attractor_bare_preset("at_root_tint", "fern", "root_tint = \"1.5\"\n"),
    ]);
    let base = renderer
        .capture_preset("at_tint_off", &lively, 60)
        .expect("capture at_tint_off");
    let base_mask = lit_mask(&base);
    let base_lit = base_mask.iter().filter(|&&l| l).count();
    assert!(base_lit > 500, "the bare fern lit only {base_lit} pixels");

    for (name, param) in [("at_map_tint", "map_tint"), ("at_root_tint", "root_tint")] {
        let tinted = renderer
            .capture_preset(name, &lively, 60)
            .expect("capture a tinted fern");
        let tinted_mask = lit_mask(&tinted);

        // Geometry: the same pixels are lit. A handful may cross the `EPS` byte
        // cut when a colour moves - that is the cut, not the figure - so this is
        // a proportion rather than set equality, the same allowance the
        // `brightness` property above makes for the faintest fringe.
        let differing = base_mask
            .iter()
            .zip(tinted_mask.iter())
            .filter(|&(&a, &b)| a != b)
            .count();
        assert!(
            differing * 50 < base_lit,
            "`{param}` moved {differing} of {base_lit} lit pixels in or out of the \
             figure - a colour lever must not change what is lit"
        );

        // Colour: measured over the BASE lit set, so both are read over the same
        // pixels rather than each over its own.
        let (base_rg, base_gb) = mean_chroma_over(&base, &base_mask);
        let (tint_rg, tint_gb) = mean_chroma_over(&tinted, &base_mask);
        let moved = (tint_rg - base_rg).abs() + (tint_gb - base_gb).abs();
        println!(
            "{param}: chroma (R-G, G-B) {base_rg:.2}, {base_gb:.2} -> {tint_rg:.2}, \
             {tint_gb:.2}; {differing} of {base_lit} lit pixels changed state"
        );
        assert!(
            moved > 4.0,
            "`{param} = 0.7` moved the mean chromaticity by only {moved:.2} - the \
             channel is not reaching the picture"
        );
    }
}

/// **All three depth cues are exact identities on a flat family** (Plan 0075
/// Phase 2, closing design-backlog 0067) — ADR-0076's stated property, asserted
/// at the capture with byte equality.
///
/// The property held for `perspective` and `depth_hue` from the day they
/// landed and was **false** for `depth_fade`: `dn` is identically 0 on a flat
/// family, `depth01(0)` is 0.5 — arithmetically "mid depth" — so the haze
/// multiplier was a uniform `1 - depth_fade/2`. Measured on `attractor_dissolve`
/// before the fix: `perspective = 0.7` and `depth_hue = 0.6` each moved **0** of
/// 921 600 pixels; `depth_fade = 0.9` moved 184 989 (20.1 %, max channel delta
/// 97) — a 45 % whole-figure dimmer wearing a depth lever's name, trapping
/// authors in both directions (a mysterious darkening, or an undocumented
/// brightness trim that `exposure` already is). The haze's fade term is now
/// multiplied by the family's has-depth flag, so all three cues collapse to the
/// identity together.
///
/// Byte equality, not a tolerance: the zeroed fade term makes the multiplier
/// **exactly** 1.0, and a multiply by 1.0 is an IEEE identity — the same
/// standard `the_atmosphere_is_off_by_default` holds the defaults to.
///
/// The control half keeps the assertion honest: the identical `depth_fade`
/// binding on a family that *has* depth (Lorenz) must move the picture, or the
/// "no-op on flat" above would also pass a depth pipeline that no-ops
/// everywhere.
#[test]
fn the_depth_cues_are_exact_no_ops_on_a_flat_family() {
    let Some(mut renderer) = headless() else {
        return;
    };
    let lively = AnalysisFrame {
        bass: 0.5,
        mid: 0.4,
        treb: 0.5,
        ..Default::default()
    };

    renderer.set_presets(vec![
        attractor_bare_preset("at_flat_base", "de_jong", ""),
        attractor_bare_preset("at_flat_persp", "de_jong", "perspective = \"0.7\"\n"),
        attractor_bare_preset("at_flat_hue", "de_jong", "depth_hue = \"0.6\"\n"),
        attractor_bare_preset("at_flat_fade", "de_jong", "depth_fade = \"0.9\"\n"),
        attractor_bare_preset("at_deep_base", "lorenz", ""),
        attractor_bare_preset("at_deep_fade", "lorenz", "depth_fade = \"0.9\"\n"),
    ]);

    let base = renderer
        .capture_preset("at_flat_base", &lively, 60)
        .expect("capture the bare flat family");
    let lit = lit_mask(&base).iter().filter(|&&l| l).count();
    assert!(lit > 500, "the bare De Jong lit only {lit} pixels");

    for (name, param) in [
        ("at_flat_persp", "perspective = 0.7"),
        ("at_flat_hue", "depth_hue = 0.6"),
        ("at_flat_fade", "depth_fade = 0.9"),
    ] {
        let probed = renderer
            .capture_preset(name, &lively, 60)
            .expect("capture a flat family with one depth cue set");
        assert_eq!(
            base.rgba, probed.rgba,
            "`{param}` alone must be pixel-identical to the unset capture on a family \
             with no depth — ADR-0076's zero-extent identity (backlog 0067)"
        );
    }

    // The control: the same binding on a 3-D family is alive.
    let deep_base = renderer
        .capture_preset("at_deep_base", &lively, 60)
        .expect("capture the bare Lorenz");
    let deep_fade = renderer
        .capture_preset("at_deep_fade", &lively, 60)
        .expect("capture Lorenz with depth_fade");
    let moved = frame_diff(&deep_base, &deep_fade);
    println!("depth_fade = 0.9 on Lorenz: frame_diff {moved:.4}");
    assert!(
        moved > 0.001,
        "`depth_fade = 0.9` moved nothing on a family with depth ({moved:.5}) — the \
         no-op assertions above would be vacuously true"
    );
}

/// Mean `(R − G, G − B)` over the pixels `mask` selects — the figure's **colour**,
/// separated from how bright it is the way [`mean_luma_over`] separates level.
fn mean_chroma_over(img: &CaptureImage, mask: &[bool]) -> (f32, f32) {
    let (mut rg, mut gb, mut n) = (0.0f64, 0.0f64, 0u64);
    for (px, lit) in img.rgba.chunks_exact(4).zip(mask.iter()) {
        if !lit {
            continue;
        }
        rg += f64::from(px[0]) - f64::from(px[1]);
        gb += f64::from(px[1]) - f64::from(px[2]);
        n += 1;
    }
    if n == 0 {
        return (0.0, 0.0);
    }
    ((rg / n as f64) as f32, (gb / n as f64) as f32)
}

// --- Trail grid sizing (Plan 0029 Phase 2) -------------------------------------
//
// `trail_grid_size` is pure, so these need no GPU and never skip. They mirror the
// scene's private policy constants; a change there must change these deliberately.

/// The per-axis cap and quantization step (`TRAIL_GRID_STEP`) the scene applies.
///
/// The cap is a **tier** value now (Plan 0044), so these read the floor tier's —
/// which is the value they were written against and the one every golden capture
/// and every `new_headless` renderer uses.
const CAP: (u32, u32) = lmv_core::render::TierConfig::FLOOR.attractor_trail_cap;
const CAP_W: u32 = CAP.0;
const CAP_H: u32 = CAP.1;
const STEP: u32 = 256;

/// Above the cap the grid must keep the *target's* proportions. The previous
/// per-axis clamp squashed a 3440x1440 ultrawide target to 2560x1440 — a 16:9
/// grid that the aspect-ignoring present then stretched back to 21:9, so the
/// attractor's shape changed discontinuously as the window crossed 2560 wide.
#[test]
fn trail_grid_preserves_aspect_above_the_cap() {
    let (w, h) = trail_grid_size(3440, 1440, CAP);
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
    let (tw, th) = trail_grid_size(1080, 3440, CAP);
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
        trail_grid_size(1920, 1080, CAP),
        trail_grid_size(1900, 1070, CAP),
        "a 20 px drag changed the grid — quantization is not in effect"
    );
    // ...and it is quantization, not a constant: a target a step away differs.
    assert_ne!(
        trail_grid_size(1920, 1080, CAP),
        trail_grid_size(1280, 720, CAP),
        "every target maps to the same grid — the size is not following the target"
    );
    // Both axes land on a step multiple below the cap.
    let (w, h) = trail_grid_size(1920, 1080, CAP);
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
        let (gw, gh) = trail_grid_size(w, h, CAP);
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
    let exact_grid = trail_grid_size(EXACT_TARGET.0, EXACT_TARGET.1, CAP);
    let quantized_grid = trail_grid_size(QUANTIZED_TARGET.0, QUANTIZED_TARGET.1, CAP);
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
    let Some(exact) = capture_at(EXACT_TARGET, MAP_2D, &lively) else {
        return;
    };
    let Some(quantized) = capture_at(QUANTIZED_TARGET, MAP_2D, &lively) else {
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
