// Tests index fixed-size arrays and panic on failure; allowed over the
// file's hot-path pragma — this is not the render path.
#![allow(clippy::indexing_slicing, clippy::panic, clippy::expect_used)]

use super::{
    DEFAULT_HUE, DEFAULT_HUE_CENTER, DEFAULT_HUE_SPREAD, DEPTH_PARALLAX_FAR, DEPTH_PARALLAX_NEAR,
    DEPTH_SCALE_FAR, DEPTH_SCALE_NEAR, MARGIN, SEED, Scene, SwarmScene, bounds, hue_coord,
};
use crate::render::palette::Palette;
use crate::render::scenes::SeededRng;

/// The particle count these tests run at — the floor tier's, which is the
/// number the seeded-scatter assertions below were written against and the one
/// every golden capture draws (Plan 0044).
const FLOOR_PARTICLES: usize = crate::render::TierConfig::FLOOR.swarm_particles;

/// Target aspects worth checking a domain against: 16:9, the 16:10 the fixed
/// constants disagreed with, 4:3, an ultrawide, and a portrait.
const ASPECTS: [f32; 5] = [16.0 / 9.0, 16.0 / 10.0, 4.0 / 3.0, 21.0 / 9.0, 9.0 / 16.0];

/// The domain has the **render target's** shape, not the baked 16:9 the
/// replaced constants encoded (ADR-0037, ADR-0044).
///
/// The visible frame is `|world.y| <= 1` by `|world.x| <= aspect`, so a domain
/// that fills it without over-filling has exactly that ratio. The old
/// `BOUND_X = 1.8` / `BOUND_Y = 1.0` pair is 1.80 at every target size — right
/// only at 16:9, and the reason no existing test could tell: at 16:10 it
/// over-fills horizontally by 12 %.
#[test]
fn the_domain_takes_its_shape_from_the_target() {
    for aspect in ASPECTS {
        let (bx, by) = bounds(aspect);
        assert!(
            (bx / by - aspect).abs() < 1e-5,
            "domain shape {:.4} must equal the target's {aspect:.4}",
            bx / by
        );
    }

    // The pair it replaced, for the record: correct at 16:9 and wrong the
    // moment the target is anything else.
    let (old_x, old_y) = (1.8f32, 1.0f32);
    let sixteen_ten = 16.0 / 10.0;
    assert!(
        (old_x / old_y - sixteen_ten).abs() > 0.1,
        "the fixed constants must genuinely disagree with 16:10, or this guards nothing"
    );
}

/// **The artifact fix, as arithmetic** (backlog 0029): the toroidal wrap seam
/// projects outside the visible frame across the whole `zoom`/`pan_*` range the
/// swarm family works in, so no particle is guaranteed to paint on a fixed
/// on-screen line and the feedback stage has no bar to integrate.
///
/// The shader projects a particle at world `p` to
/// `ndc = ((p.x * zoom + pan_x) / aspect, p.y * zoom + pan_y)`, so a seam at
/// `+bound` clears the frame when `bound * zoom - |pan| > extent`. Non-vacuous
/// by construction: at the old `MARGIN = 1` equivalent the y seam lands exactly
/// on `ndc.y = 1` at `zoom = 1`, which is the reported defect.
///
/// Asserted twice, because the margin is **proportional** and `pan_*` is in
/// world units: the general clearance scales with each axis' half-extent, so
/// the literal `0.16` the presets pan by buys different headroom on a wide
/// target than on a tall one. The second block pins the family's own number
/// against the landscape targets it will actually meet. On a portrait target
/// the x axis is the tight one — `9:16` leaves 0.14 of pan headroom against
/// that 0.16 — which is a property of `pan_x` being world-space, not something
/// the domain should distort its shape to paper over.
#[test]
fn the_wrap_seam_projects_outside_the_visible_frame() {
    // The family's working range (Plan 0043 Phase 1).
    const ZOOMS: [f32; 4] = [1.0, 1.1, 1.2, 1.3];
    /// The range the **shipped** presets actually reach, which starts *below* 1:
    /// `swarm_drift.toml` binds `zoom = "1.04 + sin(...) * 0.05 + ..."`, so it
    /// bottoms out just under 1 and a guard starting at 1.0 leaves the shipped
    /// minimum unmeasured (Plan 0043 close review). Used for the concrete block
    /// below, not the general one — "clears by at least `headroom`" is a
    /// `zoom >= 1` property by construction, while "clears at all" is the claim
    /// that has to hold everywhere the family goes.
    const SHIPPED_ZOOMS: [f32; 5] = [0.99, 1.0, 1.1, 1.2, 1.3];
    /// The largest `pan_*` amplitude any surviving preset binds (Drift's `pan_x`).
    /// A future preset that pans further or zooms lower has to widen these two —
    /// which is why they say where they come from.
    const PAN: f32 = 0.16;
    let headroom = MARGIN - 1.0;

    // General: on any target and anywhere in the working zoom range, each seam
    // clears its frame edge by at least `headroom` of that axis' half-extent.
    for aspect in ASPECTS {
        let (bx, by) = bounds(aspect);
        for zoom in ZOOMS {
            assert!(
                by * zoom - 1.0 >= headroom - 1e-5,
                "y seam clears by {:.4}, want >= {headroom:.4} (aspect {aspect:.3}, \
                 zoom {zoom:.2})",
                by * zoom - 1.0
            );
            assert!(
                bx * zoom - aspect >= aspect * headroom - 1e-5,
                "x seam clears by {:.4}, want >= {:.4} (aspect {aspect:.3}, zoom {zoom:.2})",
                bx * zoom - aspect,
                aspect * headroom
            );
        }
    }

    // Concrete: the pan the swarm presets reach, against every landscape
    // target, **at the near depth layer**. Parallax scales both the pan offset
    // and the zoom deflection, and the near layer takes the most pan — so it is
    // the one whose seam sits closest to the frame and the only one worth
    // asserting. Checking a depth-agnostic 1.0 here would pass while the layer
    // that actually binds went unmeasured.
    for aspect in [16.0 / 9.0, 16.0 / 10.0, 4.0 / 3.0, 21.0 / 9.0] {
        let (bx, by) = bounds(aspect);
        for zoom in SHIPPED_ZOOMS {
            let par = DEPTH_PARALLAX_NEAR;
            let seam_y = by * (1.0 + (zoom - 1.0) * par) - PAN * par;
            let seam_x = bx * (1.0 + (zoom - 1.0) * par) - PAN * par;
            assert!(
                seam_y > 1.0,
                "near-layer y seam projects to {seam_y:.3} at zoom {zoom:.2}, pan {PAN} \
                 — inside the frame"
            );
            assert!(
                seam_x > aspect,
                "near-layer x seam projects to {seam_x:.3} at zoom {zoom:.2}, pan {PAN} \
                 — inside the half-width {aspect:.3}"
            );
        }
    }

    // What the margin costs, stated so a change to it is deliberate: the
    // visible fraction of the domain is 1 / MARGIN^2.
    let visible = 1.0 / (MARGIN * MARGIN);
    assert!(
        (0.5..0.85).contains(&visible),
        "a margin keeping under half the particles on screen is too expensive: {visible:.3}"
    );
}

/// **Parallax is present, not merely a scale change** (Plan 0043 Phase 3's
/// done-when): under the same `pan_*`, a near particle traverses the frame
/// measurably faster than a far one.
///
/// Replicates the vertex shader's projection exactly — that one expression is
/// the whole depth transform, so asserting on it is asserting on what the GPU
/// does. Two claims, and the second is what stops this from being a tautology
/// about a constant: the layers separate under a pan, and they do **not**
/// separate at the identity transform, so an unbound preset gets no parallax
/// distortion at all.
#[test]
fn near_particles_traverse_the_frame_faster_than_far_ones() {
    // `misc.v`: ndc.x = (center.x * (1 + (zoom - 1) * par) + pan.x * par) / aspect
    let project = |center_x: f32, zoom: f32, pan_x: f32, par: f32, aspect: f32| {
        (center_x * (1.0 + (zoom - 1.0) * par) + pan_x * par) / aspect
    };
    let aspect = 16.0 / 9.0;
    let (near, far) = (DEPTH_PARALLAX_NEAR, DEPTH_PARALLAX_FAR);

    // Two particles at the same place, at opposite depths, under a pan sweep.
    let center_x = 0.4;
    let (pan_a, pan_b) = (0.0, 0.3);
    let travel = |par: f32| {
        (project(center_x, 1.0, pan_b, par, aspect) - project(center_x, 1.0, pan_a, par, aspect))
            .abs()
    };
    let (near_travel, far_travel) = (travel(near), travel(far));
    assert!(
        near_travel > far_travel * 1.5,
        "the near layer must outrun the far one: {near_travel:.4} vs {far_travel:.4} \
         (ratio {:.2})",
        near_travel / far_travel
    );

    // A zoom deflection separates them too — depth is not pan-only.
    let zoomed = |par: f32| (project(center_x, 1.3, 0.0, par, aspect)).abs();
    assert!(
        zoomed(near) > zoomed(far) * 1.05,
        "zoom must deflect the near layer further: {:.4} vs {:.4}",
        zoomed(near),
        zoomed(far)
    );

    // ...and at the identity transform every depth projects to the same place,
    // so an unbound preset is untouched by the depth axis' parallax term.
    for par in [far, 1.0, near] {
        assert!(
            (project(center_x, 1.0, 0.0, par, aspect) - center_x / aspect).abs() < 1e-6,
            "identity zoom/pan must be depth-independent"
        );
    }
}

/// The depth axis is **seeded**, so a capture is reproducible run-to-run
/// (NFR §6) — and it genuinely spans the range, which is what makes the scale,
/// fade and parallax lerps do anything.
#[test]
fn the_seeded_scatter_reproduces_the_same_depth_sequence() {
    let depths = || {
        let mut rng = SeededRng::new(SEED);
        (0..FLOOR_PARTICLES)
            .map(|_| SwarmScene::spawn(&mut rng).z)
            .collect::<Vec<f32>>()
    };
    let (a, b) = (depths(), depths());
    assert_eq!(a, b, "the same seed must give the same depth sequence");

    let lo = a.iter().copied().fold(f32::INFINITY, f32::min);
    let hi = a.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let mean = a.iter().sum::<f32>() / a.len() as f32;
    assert!(
        (0.0..0.02).contains(&lo) && (0.98..=1.0).contains(&hi),
        "depth must span the full 0..1 range, got {lo:.4}..{hi:.4}"
    );
    assert!(
        (0.45..0.55).contains(&mean),
        "depth must populate the range evenly, mean was {mean:.4}"
    );
}

/// A resize rescales the field instead of teleporting it (ADR-0044's
/// consequence: "the wrap must stay stable across one rather than teleporting
/// every particle at once").
///
/// Normalized storage is what buys this, and the test says so by measuring the
/// alternative alongside: with world-space positions, shrinking the domain
/// re-wraps everything outside the new bounds, and those particles jump by a
/// full domain width. Normalized positions move continuously with the change.
#[test]
fn a_resize_rescales_the_field_rather_than_wrapping_it() {
    let (before, after) = (16.0 / 9.0, 16.0 / 10.0);
    let (bx0, by0) = bounds(before);
    let (bx1, by1) = bounds(after);

    // A fan of normalized positions spanning the torus, including both seams.
    let samples: Vec<[f32; 2]> = (0..64)
        .map(|i| {
            let u = i as f32 / 63.0 * 2.0 - 1.0;
            [u, -u]
        })
        .collect();

    let mut worst_normalized = 0.0f32;
    let mut worst_world_space = 0.0f32;
    for s in &samples {
        // What this scene does: the normalized position is untouched, so the
        // world position moves by exactly the change in the half-extents.
        let moved = ((s[0] * bx1 - s[0] * bx0).powi(2) + (s[1] * by1 - s[1] * by0).powi(2)).sqrt();
        worst_normalized = worst_normalized.max(moved);

        // What a world-space store would do: keep the world position and
        // re-wrap it into the new domain.
        let (mut wx, wy) = (s[0] * bx0, s[1] * by0);
        if wx > bx1 {
            wx -= 2.0 * bx1;
        } else if wx < -bx1 {
            wx += 2.0 * bx1;
        }
        let jump = ((wx - s[0] * bx0).powi(2) + (wy - s[1] * by0).powi(2)).sqrt();
        worst_world_space = worst_world_space.max(jump);
    }

    // 16:9 -> 16:10 narrows the x half-extent by ~0.22 world units; nothing
    // moves further than that, and the y axis does not move at all.
    assert!(
        worst_normalized < 0.3,
        "a resize must move particles continuously, worst was {worst_normalized:.3}"
    );
    assert!(
        worst_world_space > 2.0,
        "the world-space alternative must genuinely teleport, or this test proves \
         nothing: worst jump was {worst_world_space:.3}"
    );
}

/// The default hue band (`center = 0.5`, `spread = 1`, `hue = 0`) reduces to
/// `particle_hue`, so the swarm's colour is unchanged from before Plan 0020.
#[test]
fn default_hue_band_is_the_prior_full_wheel() {
    for &ph in &[0.0, 0.2, 0.5, 0.73, 0.99] {
        let coord = hue_coord(DEFAULT_HUE_CENTER, DEFAULT_HUE_SPREAD, ph, DEFAULT_HUE);
        assert!(
            (coord - ph).abs() < 1e-6,
            "default band maps particle_hue to itself: {coord} vs {ph}"
        );
    }
}

/// A narrow `hue_spread` collapses the full particle-hue range into a tight
/// LUT band, so the sampled colours cluster (a coherent single-family swarm)
/// where `spread = 1` samples the whole wheel (rainbow confetti). Measured as
/// the spread of sampled RGB — the gap the plan closes.
#[test]
fn narrow_spread_makes_colour_coherent() {
    let pal = Palette::default_spectrum();
    // Total variance of the sampled colours across a fan of particle hues.
    let colour_spread = |spread: f32| -> f32 {
        let hues: Vec<f32> = (0..64).map(|i| i as f32 / 64.0).collect();
        let cols: Vec<[f32; 3]> = hues
            .iter()
            .map(|&h| pal.sample(hue_coord(0.5, spread, h, 0.0), 0.0))
            .collect();
        let n = cols.len() as f32;
        let mut mean = [0.0f32; 3];
        for c in &cols {
            for k in 0..3 {
                mean[k] += c[k] / n;
            }
        }
        let mut var = 0.0f32;
        for c in &cols {
            for k in 0..3 {
                var += (c[k] - mean[k]).powi(2);
            }
        }
        var / n
    };
    let narrow = colour_spread(0.1);
    let full = colour_spread(1.0);
    assert!(
        narrow < full * 0.25,
        "narrow band ({narrow:.4}) is far more coherent than the full wheel ({full:.4})"
    );
}

// -----------------------------------------------------------------------
// The mark silhouette (Plan 0070 Phase 1, ADR-0084)
// -----------------------------------------------------------------------

/// The square capture the single-mark probe below draws into. Large enough
/// that a seven-pointed star's valleys are tens of pixels from its tips —
/// the whole point of the count is that the profile has structure, and at
/// `golden.rs`'s 128 there is not enough of it to bin cleanly.
const MARK_CAPTURE: u32 = 256;

/// The mark's half-size in world units. The frame is `|ndc| <= 1` on a square
/// target, so this leaves a tenth of the frame outside the sprite quad.
const MARK_HALF: f32 = 0.9;

/// **One mark, drawn large and centred, through the real swarm pipeline** —
/// the linear composite it wrote, RGBA, row-major.
///
/// A swarm normally draws thousands of sprites and no single silhouette is
/// legible in the sum, so this builds the scene with a pool of **one**. Two
/// tricks make that one mark measurable, and both are arithmetic rather than
/// tuning:
///
/// - **It is centred exactly**, whatever the seeded scatter put the particle
///   at. The vertex shader computes
///   `center * (1 + (zoom - 1) * parallax) + pan * parallax`, so
///   `zoom = 1 - 1/parallax` with `pan = 0` collapses the position term to
///   zero identically. The particle's own `parallax` comes off its depth,
///   which the seeded draw is replayed here to read.
/// - **It is scaled to a known size** by dividing [`MARK_HALF`] through the
///   per-particle size and depth scale the same replay gives.
///
/// `saturation = 0` so every lit pixel is grey: the profile below thresholds
/// on luminance, and a palette sample that happened to be dark in one channel
/// would put notches in it that have nothing to do with the shape.
fn capture_one_mark(shape: f32, points: f32) -> Option<Vec<f32>> {
    use crate::dsp::AnalysisFrame;
    use crate::render::context::RenderError;
    use crate::render::{COMPOSITE_FORMAT, HeadlessOptions, Renderer, capture};

    let renderer = match Renderer::new_headless(HeadlessOptions {
        width: MARK_CAPTURE,
        height: MARK_CAPTURE,
        prefer_software: true,
    }) {
        Ok(renderer) => renderer,
        Err(RenderError::RequestAdapter(_)) => {
            eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
            return None;
        }
        Err(e) => panic!("headless renderer build failed: {e}"),
    };
    let device = renderer.ctx.device.clone();
    let queue = renderer.ctx.queue.clone();

    // Replay the seeded draw the scene's own pool will make, so the depth
    // terms below are the particle's and not an assumption about it.
    let mut rng = SeededRng::new(SEED);
    let particle = SwarmScene::spawn(&mut rng);
    let parallax = DEPTH_PARALLAX_FAR + (DEPTH_PARALLAX_NEAR - DEPTH_PARALLAX_FAR) * particle.z;
    let depth_scale = DEPTH_SCALE_FAR + (DEPTH_SCALE_NEAR - DEPTH_SCALE_FAR) * particle.z;

    let mut scene = SwarmScene::new(&device, COMPOSITE_FORMAT, 1);
    for (name, value) in [
        ("force", 0.0),
        ("spin", 0.0),
        ("burst", 0.0),
        ("brightness", 1.0),
        ("saturation", 0.0),
        ("size", MARK_HALF / (particle.size * depth_scale)),
        ("zoom", 1.0 - 1.0 / parallax),
        ("pan_x", 0.0),
        ("pan_y", 0.0),
        ("shape", shape),
        ("points", points),
    ] {
        scene.set_param(name, value);
    }
    scene.set_time(0.0);
    scene.update(&AnalysisFrame::default());

    let (texture, view) =
        capture::create_target(&device, COMPOSITE_FORMAT, MARK_CAPTURE, MARK_CAPTURE);
    let (buffer, padded_bpr) = capture::create_linear_readback(&device, MARK_CAPTURE, MARK_CAPTURE);
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("swarm-one-mark"),
    });
    capture::record_clear(&mut encoder, &view);
    // A square target, so the shader's aspect divide is the identity and a
    // world unit is a normalized-device unit on both axes.
    scene.render(&queue, &mut encoder, &view, 1.0);
    capture::record_copy(
        &mut encoder,
        &texture,
        &buffer,
        padded_bpr,
        MARK_CAPTURE,
        MARK_CAPTURE,
    );
    queue.submit(std::iter::once(encoder.finish()));
    Some(
        capture::read_back_linear(&device, &buffer, MARK_CAPTURE, MARK_CAPTURE, padded_bpr)
            .expect("read back the one-mark composite"),
    )
}

/// The lit radius of a capture, per direction: for each of `rays` angles out
/// of the frame centre, the furthest sample whose luminance still clears a
/// fraction of the frame's brightest.
///
/// Marched along the ray rather than binned by pixel angle, because binning
/// leaves *holes*: at a hundred directions the angular width of a bin is
/// under a pixel of arc at small radii, so a valley direction can contain no
/// pixel centre at all and read as a lit radius of zero. Marching asks each
/// direction directly.
///
/// The falloff is radial along every ray by construction — `d` scales
/// linearly with radius for the disc, polygon and star arms — so this profile
/// is the silhouette's own boundary radius times a constant, and its maxima
/// are the shape's points.
fn lit_radius_profile(pixels: &[f32], rays: usize) -> Vec<f32> {
    let size = MARK_CAPTURE as usize;
    let centre = MARK_CAPTURE as f32 * 0.5;
    let lum = |x: f32, y: f32| -> f32 {
        // The capture is row-major top-to-bottom; the sprite's `local` frame
        // has +y up, so the row index counts down from the centre.
        let col = (centre + x).floor();
        let row = (centre - y).floor();
        if col < 0.0 || row < 0.0 || col >= size as f32 || row >= size as f32 {
            return 0.0;
        }
        let base = (row as usize * size + col as usize) * 4;
        pixels
            .get(base..base + 3)
            .map_or(0.0, |px| px[0] + px[1] + px[2])
    };
    let peak = pixels
        .chunks_exact(4)
        .map(|px| px[0] + px[1] + px[2])
        .fold(0.0f32, f32::max);
    assert!(peak > 0.0, "the one-mark capture is empty");
    // A fifth of the brightest sample: well clear of the half-float floor,
    // and — because `g = (1 - d)^2` — a contour at a fixed fraction of the
    // shape's own radius, so it has the silhouette's outline.
    let threshold = peak * 0.2;

    (0..rays)
        .map(|i| {
            let a = std::f32::consts::TAU * i as f32 / rays as f32;
            let (dx, dy) = (a.cos(), a.sin());
            let steps = (centre * 2.0) as usize;
            let mut furthest = 0.0f32;
            for s in 0..steps {
                let r = s as f32 * 0.5;
                if r > centre - 1.0 {
                    break;
                }
                if lum(dx * r, dy * r) > threshold {
                    furthest = r;
                }
            }
            furthest
        })
        .collect()
}

/// How many separated angular maxima a circular profile has — the point
/// count, counted rather than eyeballed.
///
/// A **Schmitt trigger against the mark's own outer radius**, not a
/// derivative test and not the midpoint of the profile's own range. Both of
/// those count rasterization noise: a disc's profile here spans 62.8 to 64.0
/// px, and the midpoint of *that* range is crossed 88 times by a circle.
/// Here a lobe is an angular run reaching within 20 % of the furthest lit
/// radius, separated from the next by a dip below 70 % of it — so a figure
/// that never dips (a disc, a many-sided polygon) is one lobe, and a
/// seven-pointed star, whose valleys sit at 45 % of its tips, is seven.
fn angular_lobes(profile: &[f32]) -> usize {
    let hi = profile.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let (on, off) = (hi * 0.8, hi * 0.7);
    let n = profile.len();
    // Latched state, walked twice so the wrap-around is settled before the
    // count starts.
    let mut lit = profile.iter().copied().fold(true, |acc, r| {
        if r >= on {
            true
        } else if r <= off {
            false
        } else {
            acc
        }
    });
    if profile.iter().all(|&r| r > on) {
        return 1;
    }
    let mut lobes = 0usize;
    for i in 0..n {
        let r = profile.get(i).copied().unwrap_or(0.0);
        let next = if r >= on {
            true
        } else if r <= off {
            false
        } else {
            lit
        };
        if next && !lit {
            lobes += 1;
        }
        lit = next;
    }
    lobes
}

/// **A seven-pointed star has exactly seven angular maxima** (Plan 0070
/// Phase 1's second done-when), counted off a real capture of the real
/// pipeline rather than asserted by eye.
///
/// Three counts, because one would not separate "the shape is a star" from
/// "the profile is noisy": the same probe at 5, 7 and 9 points must return 5,
/// 7 and 9. And a disc must return **one** — a circle's lit radius is
/// constant, so a lobe count above 1 on it would mean this whole measurement
/// is reading rasterization noise and the star counts prove nothing.
#[test]
fn a_seven_pointed_star_has_seven_angular_maxima() {
    const BINS: usize = 360;
    const STAR: f32 = 3.0;
    const DISC: f32 = 0.0;

    let Some(disc) = capture_one_mark(DISC, 7.0) else {
        return;
    };
    let disc_profile = lit_radius_profile(&disc, BINS);
    let disc_lobes = angular_lobes(&disc_profile);
    let (lo, hi) = (
        disc_profile.iter().copied().fold(f32::INFINITY, f32::min),
        disc_profile
            .iter()
            .copied()
            .fold(f32::NEG_INFINITY, f32::max),
    );
    eprintln!("disc lit radius {lo:.1}..{hi:.1} px over {BINS} bins, {disc_lobes} lobe(s)");
    assert!(
        hi < lo * 1.15,
        "a disc's lit radius must be constant to within rasterization, got \
         {lo:.1}..{hi:.1} px — the profile is measuring something other than \
         the silhouette"
    );
    assert_eq!(
        disc_lobes, 1,
        "a disc has no points; a count above 1 means this measurement reads noise"
    );

    for points in [5.0f32, 7.0, 9.0] {
        let Some(star) = capture_one_mark(STAR, points) else {
            return;
        };
        let profile = lit_radius_profile(&star, BINS);
        let lobes = angular_lobes(&profile);
        let lo = profile.iter().copied().fold(f32::INFINITY, f32::min);
        let hi = profile.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        eprintln!("star points={points}: lit radius {lo:.1}..{hi:.1} px, {lobes} angular maxima");
        assert!(
            hi > lo * 1.5,
            "a star's tips must reach well past its valleys, got {lo:.1}..{hi:.1} px"
        );
        assert_eq!(
            lobes, points as usize,
            "a {points}-pointed star must show exactly {points} angular maxima, \
             counted {lobes}"
        );
    }
}

/// **An eased `points` renders only whole figures — never a partial lobe**
/// (Plan 0070 Phase 3's done-when), asserted on the pixels rather than on the
/// quantizer.
///
/// `marks::mark_points` already pins the arithmetic. This pins the
/// *behaviour* the arithmetic exists for, because the two are separable: a
/// count could be rounded on the way into the uniform and still reach an
/// angular fold fractionally if some later hand re-derived it. So a sweep
/// from 7 to 9 — the fractional values an ease actually visits — is rendered,
/// and the frames are grouped by **exact** equality.
///
/// The claim, stated as the test checks it: the seven captures fall into
/// exactly **three** groups; the group boundaries sit at the half-integers
/// (7.4 draws the same frame as 7.0, 7.6 the same as 8.0); and the three
/// groups have 7, 8 and 9 angular maxima. No frame between them exists to be
/// found.
///
/// The first pair is the determinism control: the same request captured twice
/// must produce the same bytes, or grouping by equality would be measuring
/// the adapter.
#[test]
fn an_eased_points_sweep_renders_only_whole_figures() {
    const BINS: usize = 360;
    const STAR: f32 = 3.0;
    /// The sweep, straddling both half-integer steps between 7 and 9.
    const SWEEP: [f32; 7] = [7.0, 7.4, 7.6, 8.0, 8.4, 8.6, 9.0];

    let Some(control_a) = capture_one_mark(STAR, 7.0) else {
        return;
    };
    let Some(control_b) = capture_one_mark(STAR, 7.0) else {
        return;
    };
    assert_eq!(
        control_a, control_b,
        "the same request must render the same bytes, or grouping frames by \
         equality measures the adapter rather than the point count"
    );

    let mut frames = Vec::new();
    for points in SWEEP {
        let Some(frame) = capture_one_mark(STAR, points) else {
            return;
        };
        frames.push((points, frame));
    }

    // Group by exact frame equality, keeping first-seen order.
    let mut groups: Vec<(Vec<f32>, Vec<f32>)> = Vec::new();
    for (points, frame) in &frames {
        match groups.iter_mut().find(|(pixels, _)| pixels == frame) {
            Some((_, members)) => members.push(*points),
            None => groups.push((frame.clone(), vec![*points])),
        }
    }
    let members: Vec<Vec<f32>> = groups.iter().map(|(_, m)| m.clone()).collect();
    let lobes: Vec<usize> = groups
        .iter()
        .map(|(pixels, _)| angular_lobes(&lit_radius_profile(pixels, BINS)))
        .collect();
    eprintln!("points sweep {SWEEP:?} -> groups {members:?}, angular maxima {lobes:?}");

    assert_eq!(
        members,
        vec![vec![7.0, 7.4], vec![7.6, 8.0, 8.4], vec![8.6, 9.0]],
        "an eased 7 -> 9 sweep must render exactly three figures, switching at \
         the half-integers"
    );
    assert_eq!(
        lobes,
        vec![7, 8, 9],
        "the three figures must be the 7-, 8- and 9-pointed stars"
    );
}

/// **The default mark is byte-identical to the one this scene drew before it
/// had a shape at all** (Plan 0070 Phase 1's first done-when), end to end
/// through the preset path.
///
/// Exact equality, not a tolerance: the `disc` arm is `length(p)` — the same
/// expression the fragment shader held — so binding `shape = "0"` and binding
/// nothing must produce the same bytes on the same adapter in the same run.
/// That is the property every untouched golden baseline rests on, asserted
/// here rather than inferred from the goldens passing.
///
/// The third capture is the non-vacuity arm: a star through the same path
/// must genuinely move the frame, or the first assertion would also pass on a
/// `shape` binding that reached nothing.
#[test]
fn a_disc_shaped_swarm_is_byte_identical_to_the_unshaped_one() {
    use crate::dsp::AnalysisFrame;
    use crate::preset::Preset;
    use crate::render::context::RenderError;
    use crate::render::{HeadlessOptions, Renderer};

    const SIZE: u32 = 128;
    const FRAMES: u32 = 30;

    let mut renderer = match Renderer::new_headless(HeadlessOptions {
        width: SIZE,
        height: SIZE,
        prefer_software: true,
    }) {
        Ok(renderer) => renderer,
        Err(RenderError::RequestAdapter(_)) => {
            eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
            return;
        }
        Err(e) => panic!("headless renderer build failed: {e}"),
    };
    let frame = AnalysisFrame::default();
    let mut capture = |name: &str, extra: &str| {
        let toml = format!(
            "system = \"swarm\"\nname = \"{name}\"\n[params]\nforce = \"0.8\"\n\
             spin = \"0.1\"\nbrightness = \"0.9\"\nsize = \"3.0\"\n{extra}"
        );
        let preset = Preset::from_toml_str(&toml).expect("the probe preset parses");
        renderer.set_presets(vec![preset]);
        renderer
            .capture_preset(name, &frame, FRAMES)
            .expect("capture the probe preset")
    };

    let unshaped = capture("unshaped", "");
    let disc = capture("disc", "shape = \"0\"\npoints = \"7\"\n");
    let star = capture("star", "shape = \"3\"\npoints = \"7\"\n");

    assert_eq!(
        unshaped.rgba, disc.rgba,
        "an explicit `shape = disc` must render byte-identically to no shape \
         binding at all — the disc arm is the length() it replaced"
    );
    let differing = star
        .rgba
        .chunks_exact(4)
        .zip(disc.rgba.chunks_exact(4))
        .filter(|(a, b)| a[..3] != b[..3])
        .count();
    eprintln!(
        "shaped swarm: {differing} of {} pixels differ between disc and star",
        (SIZE * SIZE) as usize
    );
    assert!(
        differing * 50 > (SIZE * SIZE) as usize,
        "a star must genuinely move the frame, or the equality above is a \
         statement about a binding that reached nothing: {differing} pixels"
    );
}

// -----------------------------------------------------------------------
// The sprite seam does not punch holes in the backdrop (Plan 0051 Phase 1)
// -----------------------------------------------------------------------

/// The lit-backdrop fixture this guard captures three ways. Its `bg_bright`
/// and `size` lines are **stripped and rewritten** per capture — one scene at
/// three configurations — so the numbers are read back out of the file rather
/// than restated here, and editing the fixture moves the test with it.
const LIT_FIXTURE: &str = include_str!("../../../../tests/fixtures/swarm_lit_backdrop.toml");

/// The square capture size. Modest, because this reads back three whole float
/// frames; and an exact multiple of the post chain's 256 px grid step, so the
/// trails stage runs at the target size and its present is a 1:1 sample rather
/// than a resample that would blur the property being asserted.
const CAPTURE_SIZE: u32 = 256;

/// Frames per capture. `force`/`spin`/`burst` are all 0 in the fixture, so
/// this is long enough for the seeded initial velocities to damp out and the
/// trail history to settle onto a static field.
const CAPTURE_FRAMES: u32 = 40;

/// A backdrop channel this bright counts as *present* for the non-vacuity arm
/// below — well above the half-precision floor, well below the fixture's own
/// `bg_bright`.
const BACKDROP_PRESENT: f32 = 0.05;

/// The value of a top-level `key = "<number>"` line in [`LIT_FIXTURE`], or
/// `NaN` when it is absent. Used so the fixture stays the single statement of
/// what this test captures.
fn fixture_value(key: &str) -> f32 {
    LIT_FIXTURE
        .lines()
        .find_map(|line| {
            let rest = line.trim_start().strip_prefix(key)?;
            let rest = rest.trim_start().strip_prefix('=')?;
            rest.trim().trim_matches('"').parse::<f32>().ok()
        })
        .unwrap_or(f32::NAN)
}

/// Slack for half-precision rounding, the same shape `bloom.rs`'s guard uses.
/// The composite is `Rgba16Float`, so a value of magnitude `m` is stored to
/// roughly `m / 1024`, and the lit capture quantizes a different sum than the
/// backdrop-only one does.
///
/// It is slack, not a tolerance: the property below is **exact** in real
/// arithmetic. Upstream of the tonemap the composite is a plain premultiplied
/// OVER, so where the scene wrote nothing the backdrop must arrive unchanged.
/// Measured on this fixture, the fixed shader's worst `|L - B|` is **0.0002**
/// and the pre-fix one's is **0.3467** — the backdrop's own brightness,
/// discarded outright — across 9594 channels. This sits ~1700x below the
/// defect and ~20x above the noise.
fn half_slack(value: f32) -> f32 {
    (4.0 / 1024.0) * value.abs().max(1.0)
}

/// **Where the swarm drew no light, the backdrop arrives intact** — the guard
/// the scene→chain seam shipped without.
///
/// `fs_main` used to return `vec4(in.color * g, 1.0)`: colour carried the
/// radial falloff, alpha was a literal constant. With the alpha blend at
/// `BlendComponent::OVER` and a source alpha of exactly 1, destination alpha
/// saturated to 1 across every sprite's **square** quad — including the four
/// corners outside the inscribed disc, about 21 % of each sprite, where the
/// shader wrote nothing at all. The chain's resolve computes
/// `src.rgb + backdrop * (1 - src.a)` (ADR-0055), so those corners discarded
/// the backdrop and rendered as black rectangular notches, dozens per frame.
/// See `gpu::ADDITIVE_LIGHT_SATURATING_COVERAGE`.
///
/// # Why this reads the linear composite and not the capture
///
/// Same reason `bloom.rs`'s guard does: the capture's bytes are downstream of
/// the tonemap, which scales all three channels off the brightest one
/// (ADR-0046), so adding a backdrop under a stroke changes every channel by
/// design and no byte-level tolerance separates that from the defect.
/// Upstream of the tonemap there is no confound — it is a plain premultiplied
/// OVER — so the bound is **0** rather than a tolerance. That readback is
/// `pub(crate)`, which is why this test lives here and not in `core/tests/`.
///
/// # Why it needed writing at all
///
/// Every swarm fixture and every golden baseline runs `bg_bright = 0`, where
/// a black backdrop times any alpha is still black. The whole regression
/// suite was blind to this by construction, and so was the contact sheet.
/// That is verbatim the blind spot ADR-0055's first Negative bullet names —
/// the third instance of it, after the fold (Plan 0045 Phase 2b) and the
/// bloom recombine (Phase 4b), each of which got a guard of this shape.
#[test]
fn a_lit_backdrop_survives_where_the_swarm_drew_nothing() {
    use crate::dsp::AnalysisFrame;
    use crate::preset::Preset;
    use crate::render::capture;
    use crate::render::context::RenderError;
    use crate::render::{HeadlessOptions, Renderer};

    // --- Non-vacuity, before any GPU work: the fixture must still describe
    // the configuration this guard exists for. ---
    let backdrop = fixture_value("bg_bright");
    let sprite = fixture_value("size");
    let trails = fixture_value("trails");
    assert!(
        backdrop > 0.0,
        "swarm_lit_backdrop.toml no longer ships a lit backdrop (bg_bright = \
         {backdrop}); on black this whole comparison is black against black"
    );
    assert!(
        sprite > 0.0,
        "swarm_lit_backdrop.toml no longer draws sprites (size = {sprite})"
    );
    assert!(
        trails > 0.0,
        "swarm_lit_backdrop.toml no longer binds `trails` (= {trails}), so no \
         post stage is active. With an empty chain the scene draws straight \
         onto the backdrop and its additive colour cannot remove light — the \
         defect is unrepresentable and this test proves nothing"
    );

    /// The linear composite the tonemap is about to map, at a given backdrop
    /// brightness and sprite size.
    ///
    /// Builds and drops **one** renderer per call rather than holding three:
    /// a second live device in a binary is what the software adapter falls
    /// over on, and building GPU resources mid-run shifts what the trails
    /// stage resolves to on WARP.
    fn linear_composite(bg_bright: f32, size: f32, brightness: Option<f32>) -> Option<Vec<f32>> {
        let mut renderer = match Renderer::new_headless(HeadlessOptions {
            width: CAPTURE_SIZE,
            height: CAPTURE_SIZE,
            prefer_software: true,
        }) {
            Ok(renderer) => renderer,
            Err(RenderError::RequestAdapter(_)) => {
                eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
                return None;
            }
            Err(e) => panic!("headless renderer build failed: {e}"),
        };
        // All three keys live in `[params]`, which is the fixture's last table,
        // so stripping them and appending the overrides keeps them in it.
        let base: String = LIT_FIXTURE
            .lines()
            .filter(|line| {
                let line = line.trim_start();
                !line.starts_with("bg_bright")
                    && !line.starts_with("size")
                    && !line.starts_with("brightness")
            })
            .collect::<Vec<_>>()
            .join("\n");
        let mut toml = format!("{base}\nbg_bright = \"{bg_bright}\"\nsize = \"{size}\"\n");
        if let Some(brightness) = brightness {
            toml.push_str(&format!("brightness = \"{brightness}\"\n"));
        }
        let preset = Preset::from_toml_str(&toml)
            .expect("the lit-backdrop swarm fixture parses with overrides");
        let name = preset.name.clone();
        renderer.set_presets(vec![preset]);

        // Every binding is a constant, so the analysis frame only has to be
        // well-formed — the swarm's `update` ignores it entirely.
        let frame = AnalysisFrame::default();
        renderer
            .capture_preset(&name, &frame, CAPTURE_FRAMES)
            .expect("capture the lit-backdrop swarm fixture");

        let device = renderer.ctx.device.clone();
        let queue = renderer.ctx.queue.clone();
        let src = renderer
            .tonemap
            .src_texture()
            .expect("the tonemap built its input while capturing")
            .clone();
        let (buffer, padded_bpr) =
            capture::create_linear_readback(&device, CAPTURE_SIZE, CAPTURE_SIZE);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("swarm-backdrop-readback"),
        });
        capture::record_copy(
            &mut encoder,
            &src,
            &buffer,
            padded_bpr,
            CAPTURE_SIZE,
            CAPTURE_SIZE,
        );
        queue.submit(std::iter::once(encoder.finish()));
        Some(
            capture::read_back_linear(&device, &buffer, CAPTURE_SIZE, CAPTURE_SIZE, padded_bpr)
                .expect("read back the linear composite"),
        )
    }

    // `L`: the frame as shipped. `K`: the same scene over a black backdrop,
    // which is what "the scene wrote no light here" is read off. `B`: the
    // backdrop with the scene contributing nothing — zero-area sprite quads
    // rasterize no fragments, so the chain resolves fully transparent and
    // this is the backdrop alone, through the same pipeline as `L`.
    let Some(lit) = linear_composite(backdrop, sprite, None) else {
        return;
    };
    let Some(dark) = linear_composite(0.0, sprite, None) else {
        return;
    };
    let Some(backdrop_only) = linear_composite(backdrop, 0.0, None) else {
        return;
    };
    // `U`: the fourth capture (Plan 0053 Phase 4), the same one the line guard
    // takes. At `brightness = 0` the per-particle colour is zero, so `in.color *
    // g` is zero everywhere and the frame is exactly `backdrop * (1 - a)` — a
    // direct readout of alpha, which is what this guard is actually about.
    let Some(unlit_sprites) = linear_composite(backdrop, sprite, Some(0.0)) else {
        return;
    };
    assert_eq!(dark.len(), lit.len(), "the captures differ in size");
    assert_eq!(
        dark.len(),
        backdrop_only.len(),
        "the captures differ in size"
    );

    let total = dark.len() / 4;
    let (mut untouched, mut drawn, mut over_backdrop) = (0usize, 0usize, 0usize);
    let (mut violations, mut worst) = (0usize, 0.0f32);
    for (pixel, texel) in dark.chunks_exact(4).enumerate() {
        if texel[0] != 0.0 || texel[1] != 0.0 || texel[2] != 0.0 {
            drawn += 1;
            continue; // the scene put light here; the property says nothing
        }
        untouched += 1;
        let base = pixel * 4;
        if backdrop_only[base..base + 3]
            .iter()
            .any(|&c| c > BACKDROP_PRESENT)
        {
            over_backdrop += 1;
        }
        for channel in 0..3 {
            let l = lit[base + channel];
            let b = backdrop_only[base + channel];
            let diff = (l - b).abs();
            if diff > worst {
                worst = diff;
            }
            if diff > half_slack(b) {
                violations += 1;
            }
        }
    }
    eprintln!(
        "swarm lit backdrop at {CAPTURE_SIZE}x{CAPTURE_SIZE}: {untouched} of \
         {total} pixels untouched by the scene ({over_backdrop} of those over \
         a lit backdrop), {drawn} lit by it; worst |L - B| {worst:.4}"
    );

    // --- Non-vacuity: the region the property speaks about is a substantial
    // part of the frame, the scene genuinely drew into the rest, and the
    // backdrop genuinely reached the frame underneath. A fixture edit that
    // quietly empties any of the three shows up here rather than passing. ---
    assert!(
        untouched * 4 > total,
        "only {untouched} of {total} pixels are untouched by the scene — the \
         fixture has filled the frame and the property covers almost nothing"
    );
    assert!(
        drawn * 20 > total,
        "only {drawn} of {total} pixels carry any scene light — the fixture \
         has stopped drawing, so the sprite corners this guards are not in \
         the frame"
    );
    assert!(
        over_backdrop * 2 > untouched,
        "only {over_backdrop} of the {untouched} untouched pixels sit over a \
         backdrop brighter than {BACKDROP_PRESENT} — comparing black against \
         black, which any alpha would pass"
    );

    // --- The second, wider property (Plan 0053 Phase 4). ---
    //
    // The swarm's exact arm is not the thin one — its zero-colour region is
    // four hard-edged corners per sprite and the revert moves 9 594 channels —
    // so this arm costs one more call to a harness that already exists and buys
    // the *same* property on both seams rather than two different ones. Where
    // the sprites emit nothing the frame is `backdrop * (1 - a)`, so a fully
    // extinguished pixel is one where alpha reached 1.
    //
    // Measured before the assertions, like the line guard's, so a failing run
    // prints both counts instead of short-circuiting on the first.
    let extinguished: usize = (0..total)
        .filter(|&pixel| {
            let base = pixel * 4;
            backdrop_only[base..base + 3]
                .iter()
                .any(|&c| c > BACKDROP_PRESENT)
                && (0..3).all(|c| unlit_sprites[base + c] <= backdrop_only[base + c] * 0.02)
        })
        .count();
    eprintln!(
        "swarm brightness = 0: {extinguished} of {drawn} footprint pixels fully \
         extinguished ({:.2} %); the exact arm moved {violations} channels",
        extinguished as f32 / drawn.max(1) as f32 * 100.0
    );

    // --- The first property, unchanged and still exact. ---
    assert_eq!(
        violations, 0,
        "{violations} channels differ between the lit frame and the backdrop \
         alone at pixels where the scene wrote NO light (worst {worst:.4}). \
         Upstream of the tonemap this is a plain premultiplied OVER, so where \
         nothing was drawn the backdrop must arrive intact — a difference \
         here is a sprite emitting coverage it does not have, holding the \
         backdrop out of pixels it never painted"
    );

    // Non-vacuity: the fourth capture has to be an alpha readout. If
    // `brightness = 0` stopped zeroing the emitted colour, this would be
    // measuring something else.
    let sprite_light: usize = (0..total)
        .filter(|&pixel| {
            let base = pixel * 4;
            let drew = dark[base] != 0.0 || dark[base + 1] != 0.0 || dark[base + 2] != 0.0;
            drew && (0..3).any(|c| unlit_sprites[base + c] > backdrop_only[base + c])
        })
        .count();
    assert_eq!(
        sprite_light, 0,
        "{sprite_light} pixels are BRIGHTER than the backdrop alone in the \
         `brightness = 0` capture, so the sprites are still emitting light \
         there — that capture's whole purpose is to make the frame \
         `backdrop * (1 - a)` and nothing else"
    );

    // A tenth of the footprint, the same ceiling the line guard uses. Measured
    // on this fixture: the fixed shader gives **1 of 12 880 (0.01 %)** and the
    // pre-fix one gives **16 052 (124.63 %)**. Over 100 % is expected here and
    // is the defect's own signature — the footprint is counted from where the
    // sprites put *colour*, and the corners outside the inscribed disc (~21 % of
    // every quad) are exactly the region that draws nothing and, pre-fix,
    // extinguished the backdrop anyway.
    assert!(
        extinguished * 10 < drawn,
        "{extinguished} of the sprites' {drawn} footprint pixels are fully \
         extinguished with `brightness = 0` — the frame there is \
         `backdrop * (1 - a)`, so that is alpha at 1 across the sprite quads \
         rather than only at their centres. A premultiplied sprite carries its \
         coverage in alpha (ADR-0056); a constant alpha 1 punches the backdrop \
         out of every quad's corners. The exact arm moved {violations} channels \
         on the same run"
    );
}
