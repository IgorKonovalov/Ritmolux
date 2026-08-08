// Test asserts panic on failure; allowed here over the file's pragma.
#![allow(clippy::panic, clippy::indexing_slicing)]

use super::*;

/// The pyramid halves each axis per level and stops at the tier's depth. This
/// is the arithmetic the `bloom_levels` tier value buys, GPU-free.
#[test]
fn the_pyramid_halves_each_level_down_to_the_tier_depth() {
    let sizes = level_sizes((1024, 512), 4);
    assert_eq!(sizes, vec![(512, 256), (256, 128), (128, 64), (64, 32)]);
    // A deeper tier is genuinely deeper on a grid with room for it.
    assert!(level_sizes((1024, 512), 6).len() > level_sizes((1024, 512), 4).len());
}

/// A grid too small for the requested depth runs a shallower pyramid rather
/// than allocating levels the blur cannot act on — and **always keeps one**,
/// so `resolve` has no second code path for a stage with no pyramid.
#[test]
fn a_small_grid_truncates_the_pyramid_but_never_empties_it() {
    // 32x32 halves to 16, 8, 4 and would next hit 2, below the floor.
    assert_eq!(level_sizes((32, 32), 8), vec![(16, 16), (8, 8), (4, 4)]);
    // Degenerate targets still yield exactly one level.
    for grid in [(1, 1), (2, 2), (4, 1), (0, 0)] {
        let sizes = level_sizes(grid, 6);
        assert_eq!(sizes.len(), 1, "{grid:?} produced {sizes:?}");
        assert!(sizes[0].0 >= 1 && sizes[0].1 >= 1, "{sizes:?}");
    }
    // A zero depth request is still one level, not none.
    assert_eq!(level_sizes((512, 512), 0).len(), 1);
}

/// **The tier's effect on this stage**, GPU-free: at the display size the
/// floor tier was written for, rich runs a genuinely deeper pyramid — and the
/// floor runs exactly the depth its `TierConfig` asks for, so the assertion is
/// about the tier value rather than about some clamp binding first.
///
/// Also pins the depth the **golden captures** run at. `composite.rs` renders
/// 160x100, which the post grid quantizes up to 256x256, and the floor's four
/// levels take that to 16x16 — comfortably above [`MIN_LEVEL_PX`], so the
/// capture exercises the full floor pyramid rather than a truncated one.
#[test]
fn the_rich_tier_runs_a_deeper_pyramid_than_the_floor() {
    use crate::render::TierConfig;

    let depth_at = |surface: (u32, u32), tier: &TierConfig| {
        level_sizes(
            internal_grid_size(surface, tier.post_cap),
            tier.bloom_levels,
        )
        .len()
    };
    let display = (1920, 1080);
    let floor = depth_at(display, &TierConfig::FLOOR);
    let rich = depth_at(display, &TierConfig::RICH);
    assert_eq!(
        floor,
        TierConfig::FLOOR.bloom_levels as usize,
        "the floor's depth must be its tier value, not a clamp"
    );
    assert!(
        rich > floor,
        "rich must run a deeper pyramid than the floor: {rich} vs {floor}"
    );

    // The golden capture size, whose grid is 256x256.
    assert_eq!(
        internal_grid_size((160, 100), TierConfig::FLOOR.post_cap),
        (256, 256)
    );
    assert_eq!(depth_at((160, 100), &TierConfig::FLOOR), 4);
}

/// `bloom_radius` moves the halo's energy outward monotonically, and stays
/// inside the band where the up-chain's sum converges. The **pixel-level**
/// consequence — the halo actually sits further out — is
/// `core/tests/bloom.rs`; this pins the arithmetic behind it.
#[test]
fn scatter_rises_with_the_radius_and_stays_below_one() {
    let mut last = -1.0;
    for radius in [0.0f32, 0.5, 1.0, 2.0, MAX_RADIUS] {
        let s = scatter(radius);
        assert!(s > last, "scatter({radius}) = {s} did not rise");
        assert!(
            (MIN_SCATTER..=MAX_SCATTER).contains(&s),
            "scatter({radius}) = {s}"
        );
        last = s;
    }
    // Out-of-range bindings saturate rather than diverging: a preset easing
    // this off a band can overshoot, and a scatter at or above 1 would make
    // every coarse level's contribution outweigh the one it lands on.
    assert_eq!(scatter(-5.0), MIN_SCATTER);
    assert_eq!(scatter(1e9), MAX_SCATTER);
}

/// The recombine's divisor is the sum the up-chain actually accumulates, so
/// `bloom_amount` is a brightness and `bloom_radius` is a width rather than
/// the two being tangled.
#[test]
fn the_pyramid_sum_matches_the_series_it_normalizes() {
    // 1 + s + s^2 + s^3 at s = 0.5.
    assert!((pyramid_sum(0.5, 4) - 1.875).abs() < 1e-5);
    assert!(
        (pyramid_sum(0.0, 4) - 1.0).abs() < 1e-5,
        "no scatter, one level's worth"
    );
    // Monotone in both arguments: a wider halo and a deeper pyramid each
    // accumulate more, and each has to be divided back out.
    assert!(pyramid_sum(MAX_SCATTER, 6) > pyramid_sum(MAX_SCATTER, 4));
    assert!(pyramid_sum(MAX_SCATTER, 4) > pyramid_sum(MIN_SCATTER, 4));
    // Degenerate inputs stay finite and never divide the halo away.
    assert!(pyramid_sum(1.0, 4).is_finite() && pyramid_sum(1.0, 4) >= 1.0);
    assert!(pyramid_sum(0.5, 0) >= 1.0);
}

// -----------------------------------------------------------------------
// The halo does not punch a hole in the backdrop (Plan 0045 Phase 4b)
// -----------------------------------------------------------------------

/// The fixture, shared with `composite.rs`'s blessed baseline and
/// `core/tests/bloom.rs`: a small bright core on black, over range by design.
/// Its `bg_bright` line is stripped and rewritten per capture — the whole
/// point here is the value it does *not* ship.
const BACKDROP_FIXTURE: &str = include_str!("../../../tests/fixtures/composite_bloom.toml");

/// The lit half of the comparison. Bright enough to be unmistakably present in
/// every channel, dim enough not to wash out the knot the halo comes from.
const BACKDROP_BRIGHT: f32 = 0.45;

/// Slack for half-precision rounding. The composite is `Rgba16Float`, so a
/// value of magnitude `m` is stored to roughly `m / 1024`, and the lit capture
/// quantizes a *different* sum than the dark one. Four of those.
///
/// It is slack, not a tolerance: the property below is exact in real
/// arithmetic. Measured, the fixed shader's worst deficit is **0.0000** and
/// the unclamped one's is **0.3125**, so this sits ~150x below the defect and
/// ~all the way above the noise.
fn half_slack(value: f32) -> f32 {
    (4.0 / 1024.0) * value.abs().max(1.0)
}

/// **Compositing a backdrop underneath the chain may only ADD light** — the
/// guard the bloom stage shipped without.
///
/// The recombine summed **alpha** as well as colour, and it blends
/// `PREMULTIPLIED_ALPHA_BLENDING` into the chain's destination, where ADR-0055
/// paints the backdrop. Where the frame was already opaque and the halo
/// non-zero the source alpha exceeded 1, `1 - src.a` went negative, and the
/// backdrop was *subtracted* under the frame's brightest regions. See the
/// module docs.
///
/// # Why this reads the linear composite and not the capture
///
/// **A display-byte version of this assertion cannot be written.** The bytes a
/// capture holds are downstream of the tonemap, which scales all three
/// channels by `f(m)/m` off the brightest one (ADR-0046, hue-preserving). Add
/// a red-dominant backdrop under a magenta stroke and `m` rises, so the scale
/// falls, so **blue comes out darker than it did over black** — measured at up
/// to **15 bytes on this fixture with the bloom stage switched off entirely**.
/// That is the curve behaving as designed, it is seven times the defect's own
/// display-space signal, and no byte-level tolerance separates them.
///
/// Upstream of the tonemap there is no such confound: the composite is a plain
/// premultiplied OVER, so the true bound is **0** rather than a tolerance, and
/// the defect is unmasked — the same fixture reads a worst deficit of
/// **0.3125** linear on the unclamped shader against **0.0000** on the fixed
/// one. That readback is `pub(crate)`, which is why this test is here rather
/// than beside the stage's other pixel properties in `core/tests/bloom.rs`.
///
/// # Why it needed writing at all
///
/// Every bloom fixture runs `bg_bright = 0` — for a baseline that is the right
/// call (see the fixture's own comment), and it is also why the one stage in
/// the chain that can exceed alpha 1 had no lit-backdrop test. On black,
/// subtracting the backdrop and covering it are the same picture. Same shape
/// as the guard `core/tests/kaleidoscope.rs` installed for the fold, and the
/// blind spot ADR-0055's Negative section names outright.
#[test]
fn a_backdrop_under_an_active_halo_only_ever_adds_light() {
    use crate::dsp::AnalysisFrame;
    use crate::preset::Preset;
    use crate::render::capture;
    use crate::render::context::RenderError;
    use crate::render::{HeadlessOptions, Renderer};

    /// Square and modest: this reads back a whole float frame twice.
    const SIZE: u32 = 256;
    /// The fixture is frozen (`spin = 0`), so this only clears the draw-in.
    const FRAMES: u32 = 40;

    // --- Non-vacuity, before any GPU work: the fixture must still switch the
    // stage on. Edit `bloom_amount` to 0 and every assertion below would hold
    // for the trivial reason that the pyramid never ran. ---
    let binds_bloom = BACKDROP_FIXTURE.lines().any(|line| {
        let line = line.trim_start();
        line.starts_with("bloom_amount")
            && line.contains('"')
            && !line.contains("\"0\"")
            && !line.contains("\"0.0\"")
    });
    assert!(
        binds_bloom,
        "composite_bloom.toml no longer binds a non-zero `bloom_amount`, so \
         this test renders a frame with no bloom stage in it and proves nothing"
    );

    /// The linear composite the tonemap is about to map, at a given backdrop.
    ///
    /// Builds and drops **one** renderer per call rather than holding two: a
    /// second live device in a binary is what the software adapter falls over
    /// on, and the whole point of this test is a configuration that puts an
    /// extra fullscreen pipeline (the backdrop's) on the device.
    fn linear_composite(bg_bright: f32) -> Option<Vec<f32>> {
        let mut renderer = match Renderer::new_headless(HeadlessOptions {
            width: SIZE,
            height: SIZE,
            prefer_software: true,
        }) {
            Ok(renderer) => renderer,
            Err(RenderError::RequestAdapter(_)) => {
                eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
                return None;
            }
            Err(e) => panic!("headless renderer build failed: {e}"),
        };
        let base: String = BACKDROP_FIXTURE
            .lines()
            .filter(|line| !line.trim_start().starts_with("bg_bright"))
            .collect::<Vec<_>>()
            .join("\n");
        let toml = format!("{base}\nbg_bright = \"{bg_bright}\"\n");
        let preset =
            Preset::from_toml_str(&toml).expect("the bloom fixture parses with a backdrop");
        let name = preset.name.clone();
        renderer.set_presets(vec![preset]);

        let frame = AnalysisFrame {
            bass: 0.6,
            mid: 0.5,
            treb: 0.6,
            onset: 0.4,
            bar: 0.25,
            ..Default::default()
        };
        renderer
            .capture_preset(&name, &frame, FRAMES)
            .expect("capture the bloom fixture");

        // The tonemap's input still holds that frame's linear composite.
        let device = renderer.ctx.device.clone();
        let queue = renderer.ctx.queue.clone();
        let src = renderer
            .tonemap
            .src_texture()
            .expect("the tonemap built its input while capturing")
            .clone();
        let (buffer, padded_bpr) = capture::create_linear_readback(&device, SIZE, SIZE);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("bloom-backdrop-readback"),
        });
        capture::record_copy(&mut encoder, &src, &buffer, padded_bpr, SIZE, SIZE);
        queue.submit(std::iter::once(encoder.finish()));
        Some(
            capture::read_back_linear(&device, &buffer, SIZE, SIZE, padded_bpr)
                .expect("read back the linear composite"),
        )
    }

    let Some(dark) = linear_composite(0.0) else {
        return;
    };
    let Some(lit) = linear_composite(BACKDROP_BRIGHT) else {
        return;
    };
    assert_eq!(dark.len(), lit.len(), "the two captures differ in size");

    // Colour only: the composite's alpha is the backdrop's own (opaque)
    // wherever the backdrop paints, so it is 1.0 in the lit capture by
    // construction and says nothing about the recombine.
    let colour = |values: &[f32]| -> Vec<f32> {
        values
            .chunks_exact(4)
            .flat_map(|texel| texel[..3].to_vec())
            .collect()
    };
    let (dark, lit) = (colour(&dark), colour(&lit));

    let (mut worst, mut violations, mut gained) = (0.0f32, 0usize, 0usize);
    for (&d, &l) in dark.iter().zip(lit.iter()) {
        let deficit = d - l;
        if deficit > worst {
            worst = deficit;
        }
        if deficit > half_slack(d) {
            violations += 1;
        }
        if l - d > half_slack(d) {
            gained += 1;
        }
    }
    eprintln!(
        "linear composite, bg_bright 0 -> {BACKDROP_BRIGHT}: {gained} channels \
         gained, {violations} lost, worst deficit {worst:.4} of {}",
        dark.len()
    );

    // --- Non-vacuity: the backdrop genuinely reached the composite. Without
    // this the comparison would pass on two identical frames. ---
    assert!(
        gained * 2 > dark.len(),
        "only {gained} of {} channels gained light when bg_bright went 0 -> \
         {BACKDROP_BRIGHT} — the backdrop is not reaching the composite, so \
         the assertion below is about nothing",
        dark.len()
    );

    // --- The property. ---
    assert_eq!(
        violations, 0,
        "{violations} channels of the linear composite are DARKER with a \
         backdrop underneath than without one (worst {worst:.4}). Upstream of \
         the tonemap this is a plain premultiplied OVER, which cannot remove \
         light — so this is the recombine driving source alpha past 1 and \
         making the blend's `1 - src.a` factor negative, subtracting the \
         backdrop under the halo instead of covering it"
    );
}

/// The knee is a positive band at every threshold a preset can bind, including
/// zero — otherwise the bright-pass would divide by it.
#[test]
fn the_knee_band_is_always_positive() {
    for threshold in [0.0f32, 0.05, 1.0, MAX_THRESHOLD] {
        assert!(
            knee_band(threshold) >= MIN_KNEE,
            "knee_band({threshold}) = {}",
            knee_band(threshold)
        );
    }
    // And it scales with the threshold rather than staying at the floor.
    assert!(knee_band(4.0) > knee_band(1.0));
}
