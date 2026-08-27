//! The routing contract, GPU-free (ADR-0031): these are the cases the old
//! hand-written branch ladder in `draw_frame` encoded by enumeration and that
//! nothing tested — they were only ever exercised indirectly, through WARP
//! captures of presets that happened to bind the right params.

// Test asserts index, expect and panic freely; this is not the render path.
#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use super::{
    BLOOM, CHAIN_PARAMS, DEFAULT_OCCLUDE, Fold, KALEIDOSCOPE, POST_GRID_STEP, PostChain, PostStage,
    Routing, STAGE_COUNT, TRAILS, internal_grid_size, route, split_at_bloom,
};
use crate::render::TierConfig;
use crate::render::background::Background;
use crate::render::capture::{self, CaptureImage};
use crate::render::context::{RenderContext, RenderError};
use crate::render::gpu;

/// The tier every test in this module runs at, and the one every golden
/// baseline is blessed at (ADR-0045). These tests pin the **policy** — the
/// quantization, the single scale factor, the purity — not the tier, so they
/// read the floor cap rather than a constant of their own.
const FLOOR: TierConfig = TierConfig::FLOOR;
const POST_MAX_W: u32 = FLOOR.post_cap.0;
const POST_MAX_H: u32 = FLOOR.post_cap.1;

/// [`internal_grid_size`] at the floor cap.
fn floor_grid(surface: (u32, u32)) -> (u32, u32) {
    internal_grid_size(surface, FLOOR.post_cap)
}

// -----------------------------------------------------------------------
// The `over` junction (ADR-0090 / Plan 0076 Phase 3) — GPU-free
// -----------------------------------------------------------------------

/// The junction's routing contract, enumerated over **every** combination of
/// active flags: the pre-bloom walk keeps exactly the pre-bloom stages in
/// chain order, bloom's own flag comes back separately, and bloom itself never
/// appears in the pre-bloom walk — the junction sits between the two, which is
/// ADR-0090's one addition to the compile-time order.
///
/// The same GPU-free treatment [`route`]'s tests give the plain walk: eight
/// cases, no device, so a junction moved by accident fails here before any
/// capture can be wrong.
#[test]
fn the_over_junction_splits_the_walk_at_bloom() {
    for bits in 0..(1u32 << STAGE_COUNT) {
        let active = [bits & 1 != 0, bits & 2 != 0, bits & 4 != 0];
        let (pre, bloom) = split_at_bloom(route(&active));

        assert_eq!(
            bloom, active[BLOOM],
            "{active:?}: the junction's bloom flag is bloom's own active flag"
        );
        let want: Vec<usize> = [TRAILS, KALEIDOSCOPE]
            .into_iter()
            .filter(|&stage| active[stage])
            .collect();
        assert_eq!(
            pre.active_stages(),
            want.as_slice(),
            "{active:?}: the pre-bloom walk is the pre-bloom stages, in order"
        );
        assert!(
            !pre.active_stages().contains(&BLOOM),
            "{active:?}: bloom never folds before the junction"
        );
        // The scene's target under the junction: the first pre-bloom stage, or
        // the blend's chain input (`None` here) — never bloom's input, which
        // the blended result feeds instead.
        assert_eq!(pre.scene_stage(), want.first().copied(), "{active:?}");
    }
}

// -----------------------------------------------------------------------
// The internal-grid policy (ADR-0034) — GPU-free
// -----------------------------------------------------------------------

/// The headline property: a stage's grid **follows the render target** instead
/// of being pinned to a fixed 1280x720. This is the whole point of Plan 0033 —
/// line geometry composited through trails or the fold was rasterized at full
/// resolution and then thrown away through a 720p grid, which on the 2048x1152
/// display the preset lane worked at was a 1.6x upscale of the entire frame.
#[test]
fn the_internal_grid_follows_the_target_instead_of_a_fixed_720p() {
    // Common desktop sizes, all under the cap: the grid is the target rounded
    // up to the step, never the old constant.
    for target in [(1280, 720), (1600, 900), (1920, 1080)] {
        let grid = floor_grid(target);
        assert!(
            grid.0 >= target.0 && grid.1 >= target.1,
            "{target:?} must not be downsampled below the target: got {grid:?}"
        );
        assert!(grid.0 <= POST_MAX_W && grid.1 <= POST_MAX_H, "{grid:?}");
    }
    // The display ADR-0034 was raised on. 2048x1152 is 16:9 and above the
    // width cap, so it comes back capped *with its aspect exactly
    // preserved* — a 1.07x downscale, and emphatically not 1280x720
    // (ADR-0034).
    assert_eq!(floor_grid((2048, 1152)), (1920, 1080));
    assert_ne!(floor_grid((2048, 1152)), (1280, 720));
}

/// Every axis lands on the quantization step (or the cap), never on 0.
///
/// The step is what keeps a live window drag from reallocating a
/// `Rgba16Float` texture pair — and, for trails, clearing the accumulated
/// history — on every one of the hundreds of `Resized` events a drag emits.
#[test]
fn every_grid_axis_is_quantized_and_non_degenerate() {
    for target in [
        (1, 1),
        (17, 3),
        (640, 480),
        (1281, 721),
        (1920, 1080),
        (3840, 2160),
        (100, 4000),
    ] {
        let (w, h) = floor_grid(target);
        assert!(w > 0 && h > 0, "{target:?} produced a degenerate grid");
        assert!(
            w <= POST_MAX_W && h <= POST_MAX_H,
            "{target:?} -> ({w}, {h})"
        );
        for (axis, cap) in [(w, POST_MAX_W), (h, POST_MAX_H)] {
            assert!(
                axis % POST_GRID_STEP == 0 || axis == cap,
                "{target:?}: axis {axis} is neither a {POST_GRID_STEP} multiple nor the cap"
            );
        }
    }
}

/// Plan 0029's lesson, re-paid here: when the cap binds, **one** scale factor
/// applies to both axes. Clamping each axis independently turned a 3440x1440
/// ultrawide into a 16:9 grid, which the aspect-ignoring present then stretched
/// back — so the picture changed shape as the window crossed the cap.
///
/// Note what "aspect intact" can and cannot mean under a 256 px step: the
/// derived axis is rounded up to the step, so an ultrawide's grid aspect is
/// coarser than its target's (3440x1440 is 2.39; the grid is 1920x1024 = 1.88).
/// What the single factor buys is that it is **not** squashed to the cap's own
/// 16:9, which is the regression.
#[test]
fn a_capped_target_keeps_its_proportions_rather_than_the_caps() {
    let ultrawide = floor_grid((3440, 1440));
    let squashed_to_the_cap = (POST_MAX_W, POST_MAX_H);
    assert_ne!(
        ultrawide, squashed_to_the_cap,
        "a 3440x1440 ultrawide must not come back as the cap's own 16:9"
    );
    let target_aspect = 3440.0 / 1440.0;
    let grid_aspect = ultrawide.0 as f32 / ultrawide.1 as f32;
    let cap_aspect = POST_MAX_W as f32 / POST_MAX_H as f32;
    assert!(
        grid_aspect > cap_aspect,
        "the ultrawide grid ({grid_aspect:.3}) is no wider than 16:9 ({cap_aspect:.3}) \
         — the per-axis clamp regression is back"
    );
    assert!(
        (grid_aspect - target_aspect).abs() < target_aspect * 0.25,
        "grid aspect {grid_aspect:.3} is far from the target's {target_aspect:.3}"
    );

    // The same in portrait, where the height binds instead.
    let portrait = floor_grid((1440, 3440));
    assert!(
        (portrait.1 as f32 / portrait.0 as f32) > cap_aspect,
        "a portrait target must keep its proportions too: {portrait:?}"
    );
}

/// The policy is a **pure function** — the same target always yields the same
/// grid, with no wall clock anywhere in it, so a fixed-size headless capture
/// stays byte-reproducible (NFR §6).
#[test]
fn the_grid_policy_is_a_pure_function_of_the_target() {
    for target in [(800, 600), (2048, 1152), (3440, 1440)] {
        assert_eq!(floor_grid(target), floor_grid(target));
    }
}

/// **The tier's first visible effect** (Plan 0044 Phase 1): where the floor
/// cap binds, the rich tier resolves a genuinely larger grid; where it does
/// not, the two tiers agree exactly.
///
/// The second half is what makes the first mean something. A tier raises a
/// **ceiling**, not the grid itself, so a preset diverges only where the floor
/// was actually costing it resolution — which is also why the tier leaves
/// every golden baseline untouched (they capture at 1280x720 and smaller,
/// squarely in the agreeing set, and `new_headless` pins the floor
/// regardless).
///
/// **1920x1080 belongs in the *binding* set, not the agreeing one**, and that
/// is worth stating because it is the display size the floor was written for.
/// The policy quantizes *then* clamps, so a 1080p target rounds up to
/// 2048x1280 and the floor cap cuts it back to exactly 1920x1080 — meaning on
/// the rich tier a 1080p window supersamples its post stages by ~1.07x rather
/// than matching them to the surface.
#[test]
fn the_rich_tier_raises_the_grid_only_where_the_floor_cap_binds() {
    let rich = TierConfig::RICH;
    for target in [(1920, 1080), (2560, 1440), (3440, 1440), (3840, 2160)] {
        let (fw, fh) = floor_grid(target);
        let (rw, rh) = internal_grid_size(target, rich.post_cap);
        assert!(
            rw > fw && rh > fh,
            "the floor cap binds at {target:?}, so rich must resolve a larger \
             grid: floor {fw}x{fh}, rich {rw}x{rh}"
        );
    }
    // Targets whose *quantized* grid still fits under the floor cap, so
    // neither cap binds and the tier cannot change the answer.
    for target in [(640, 480), (1280, 720), (1600, 900)] {
        assert_eq!(
            floor_grid(target),
            internal_grid_size(target, rich.post_cap),
            "neither cap binds at {target:?}, so the tier must not change the \
             grid — a tier raises a ceiling, not the resolution"
        );
    }
}

/// `(stage, destination)` pairs, for readable assertions.
fn edges(routing: &Routing) -> Vec<(usize, Option<usize>)> {
    routing.edges().collect()
}

/// Every combination of active flags, as `[bool; STAGE_COUNT]`.
fn all_combinations() -> Vec<[bool; STAGE_COUNT]> {
    (0..(1 << STAGE_COUNT))
        .map(|mask: usize| {
            let mut flags = [false; STAGE_COUNT];
            for (bit, flag) in flags.iter_mut().enumerate() {
                *flag = mask & (1 << bit) != 0;
            }
            flags
        })
        .collect()
}

/// No stage active: the scene renders straight to the destination and nothing
/// folds. The passthrough every shipped preset takes today.
#[test]
fn no_active_stage_renders_the_scene_to_the_surface() {
    let routing = route(&[false; STAGE_COUNT]);
    assert_eq!(
        routing.scene_stage(),
        None,
        "with no stage active the scene targets the destination"
    );
    assert!(edges(&routing).is_empty(), "nothing to fold down");
}

/// One stage active: the scene renders into it and it resolves to the
/// destination. Asserted for each position, since the ladder this replaced had
/// a separate branch per stage.
#[test]
fn a_single_active_stage_resolves_to_the_surface() {
    for stage in [TRAILS, KALEIDOSCOPE, BLOOM] {
        let mut active = [false; STAGE_COUNT];
        active[stage] = true;
        let routing = route(&active);
        assert_eq!(
            routing.scene_stage(),
            Some(stage),
            "the scene renders into the only active stage"
        );
        assert_eq!(
            edges(&routing),
            vec![(stage, None)],
            "the only active stage folds into the destination"
        );
    }
}

/// All three active: trails folds into the kaleidoscope's input, the
/// kaleidoscope into bloom's, and bloom into the destination — ADR-0018's
/// feedback-then-fold order with ADR-0046's bloom-last on the end.
#[test]
fn all_active_stages_fold_in_composite_order() {
    let routing = route(&[true; STAGE_COUNT]);
    assert_eq!(routing.scene_stage(), Some(TRAILS));
    assert_eq!(
        edges(&routing),
        vec![
            (TRAILS, Some(KALEIDOSCOPE)),
            (KALEIDOSCOPE, Some(BLOOM)),
            (BLOOM, None),
        ]
    );
}

/// A skipped stage leaves no hole in the walk: with trails off, the scene
/// renders **directly** into the kaleidoscope's input and the walk starts
/// there, rather than the array position surviving as an empty slot. This is
/// [`route`]'s compaction — the mechanism the old ladder answered with a nested
/// `else if`.
///
/// The **middle**-stage skip is the case a two-stage chain could not express
/// at all: with trails and bloom on and the fold off, the walk has to jump
/// position 0 straight to position 2.
#[test]
fn a_skipped_stage_compacts_the_walk() {
    let routing = route(&[false, true, false]);
    assert_eq!(routing.scene_stage(), Some(KALEIDOSCOPE));
    assert_eq!(routing.active_stages(), &[KALEIDOSCOPE]);
    assert_eq!(edges(&routing), vec![(KALEIDOSCOPE, None)]);

    let skipped_middle = route(&[true, false, true]);
    assert_eq!(skipped_middle.scene_stage(), Some(TRAILS));
    assert_eq!(skipped_middle.active_stages(), &[TRAILS, BLOOM]);
    assert_eq!(
        edges(&skipped_middle),
        vec![(TRAILS, Some(BLOOM)), (BLOOM, None)],
        "trails must hand straight to bloom when the fold is off"
    );
}

/// The invariant, over every combination: whatever runs, the last active stage
/// targets the caller's destination — the composite always terminates there
/// exactly once. (The destination is the surface only when neither the
/// transition blend nor ink is downstream; ADR-0032 made it an argument.)
#[test]
fn the_last_active_stage_always_targets_the_surface() {
    for active in all_combinations() {
        let routing = route(&active);
        let edges = edges(&routing);
        let to_surface = edges.iter().filter(|(_, dest)| dest.is_none()).count();
        if active.iter().any(|&on| on) {
            assert_eq!(
                edges.last().map(|(_, dest)| *dest),
                Some(None),
                "the last active stage folds into the destination for {active:?}"
            );
        }
        assert!(
            to_surface <= 1,
            "at most one stage may target the destination for {active:?}"
        );
    }
}

/// ADR-0032: ink is **not** in the chain. The ordering the retired
/// `ink_when_active_is_always_last` asserted is structural now — ink is not in
/// the thing that composes, so no flag combination can schedule it before a
/// per-preset stage. This pins the membership itself instead.
#[test]
fn the_chain_holds_only_the_per_preset_look_stages() {
    let Some(ctx) = headless_context_or_skip() else {
        return;
    };
    let chain = PostChain::new(&ctx.device, ctx.surface_format(), &FLOOR);
    assert_eq!(
        chain.stage_names(),
        ["trails", "kaleidoscope", "bloom"],
        "the chain is exactly the per-preset look; the engine-wide passes \
         (background, blend, tonemap, ink) are driven outside it \
         (ADR-0032/0046)"
    );
}

/// Offscreen size for the GPU independence test — small enough to read back
/// cheaply, large enough that a trail covers many pixels.
const CHAIN_TEST_SIZE: u32 = 64;

/// A headless device, or `None` (a logged skip) when the runner exposes no
/// GPU adapter — macOS has no software Metal fallback (ADR-0016). Any other
/// build error still panics loudly.
fn headless_context_or_skip() -> Option<RenderContext> {
    match RenderContext::new_headless(CHAIN_TEST_SIZE, CHAIN_TEST_SIZE, true) {
        Ok(ctx) => Some(ctx),
        Err(RenderError::RequestAdapter(_)) => {
            eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
            None
        }
        Err(e) => panic!("headless context build failed: {e}"),
    }
}

/// Drive `chain` through one frame per entry in `lit` — a lit frame paints a
/// full-brightness backdrop, a dark one is a plain black clear — folding each
/// down into a **fresh offscreen of this call's own**, and read the last one
/// back. The chain keeps whatever cross-frame state it has built between
/// calls, so consecutive calls continue that chain's history.
fn drive(
    ctx: &RenderContext,
    chain: &mut PostChain,
    background: &mut Background,
    lit: &[bool],
) -> CaptureImage {
    let size = (CHAIN_TEST_SIZE, CHAIN_TEST_SIZE);
    let (texture, view) = capture::create_target(&ctx.device, ctx.surface_format(), size.0, size.1);

    for &is_lit in lit {
        background.reset_params();
        if is_lit {
            // A flat, full-brightness backdrop: bright everywhere, so the
            // trail it leaves is unmistakable at 8-bit precision.
            assert!(background.set_param("bg_bright", 1.0));
            assert!(background.set_param("bg_vignette", 0.0));
        } else {
            assert!(background.set_param("bg_bright", 0.0));
        }

        let mut encoder = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("post-chain-independence"),
            });
        capture::record_clear(&mut encoder, &view);
        let target = chain.begin(&mut encoder, &view, size);
        background.render(&ctx.queue, &mut encoder, &target.view, size);
        chain.resolve(&ctx.queue, &mut encoder, target.routing, &view, size);
        ctx.queue.submit(std::iter::once(encoder.finish()));
    }

    let (buffer, padded_bpr) = capture::create_readback(&ctx.device, size.0, size.1);
    let mut encoder = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("post-chain-readback"),
        });
    capture::record_copy(&mut encoder, &texture, &buffer, padded_bpr, size.0, size.1);
    ctx.queue.submit(std::iter::once(encoder.finish()));
    capture::read_back(&ctx.device, &buffer, size.0, size.1, padded_bpr)
        .expect("read back the folded chain output")
}

fn any_lit_pixel(image: &CaptureImage) -> bool {
    image
        .rgba
        .chunks_exact(4)
        .any(|px| px[0] > 0 || px[1] > 0 || px[2] > 0)
}

/// Plan 0030 Phase 2 — **the Plan 0023 unblock.** Two `PostChain`s
/// built against one device hold fully independent GPU state, so a
/// dual-live transition can run two composites in one frame.
///
/// Trails is the stage that matters: it is the one owning cross-frame state (a
/// [`PingPongField`](crate::render::feedback::PingPongField)), which is exactly
/// what Plan 0023's "in dual-live, each side needs its own feedback field" risk
/// bullet names. Both chains are driven past the point where their lazily-built
/// resources exist, then:
///
/// - chain A accumulates a lit frame and fades it on a dark one — its own trail;
/// - chain B, driven only through a dark frame, comes back **black**: none of
///   A's accumulation bled across;
/// - chain B driven through the same history as A yields the **same pixels**,
///   so B's field is a real, working, separate one — not merely empty.
///
/// Needs a GPU adapter, so it skips on runners without one (ADR-0016).
#[test]
fn two_chains_against_one_device_accumulate_independently() {
    let Some(ctx) = headless_context_or_skip() else {
        return;
    };
    let format = ctx.surface_format();
    let mut background = Background::new(&ctx.device, format);

    let mut chain_a = PostChain::new(&ctx.device, format, &FLOOR);
    let mut chain_b = PostChain::new(&ctx.device, format, &FLOOR);
    // A long trail on both, so a leaked accumulation would be glaring.
    assert!(
        chain_a.set_stage_param(TRAILS, "trails", 0.9),
        "the TRAILS position owns `trails`"
    );
    assert!(
        chain_b.set_stage_param(TRAILS, "trails", 0.9),
        "the TRAILS position owns `trails`"
    );

    // A: lit, then dark — the dark frame shows A's own fading trail.
    let a_lit_then_dark = drive(&ctx, &mut chain_a, &mut background, &[true, false]);
    assert!(
        any_lit_pixel(&a_lit_then_dark),
        "chain A's own accumulation survives into its dark frame"
    );

    // B, never driven before: one dark frame. A shared field would show A's
    // trail here; an independent one starts cleared.
    let b_dark_only = drive(&ctx, &mut chain_b, &mut background, &[false]);
    assert!(
        !any_lit_pixel(&b_dark_only),
        "chain B starts from its own cleared accumulation — none of A's \
         history leaked across"
    );

    // B through A's history: its own field must now imply the same pixels.
    let b_lit_then_dark = drive(&ctx, &mut chain_b, &mut background, &[true, false]);
    assert_eq!(
        b_lit_then_dark.rgba, a_lit_then_dark.rgba,
        "each chain folds to the pixels its own history implies"
    );
}

/// A chain's stages, addressed concretely so their build counters are readable.
struct Stages {
    trails: crate::render::trails::Trails,
    kaleido: crate::render::kaleidoscope::Kaleidoscope,
    bloom: crate::render::bloom::Bloom,
}

/// Drive one `begin`/`resolve` frame through `stage` at `surface`, discarding
/// the pixels — this is about what the stage *allocates*, not what it draws.
fn pump(ctx: &RenderContext, stage: &mut dyn PostStage, surface: (u32, u32)) {
    let (_texture, view) =
        capture::create_target(&ctx.device, ctx.surface_format(), surface.0, surface.1);
    let mut encoder = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("post-resize"),
        });
    stage.begin(&mut encoder, surface);
    stage.resolve(
        &ctx.queue,
        &mut encoder,
        &view,
        surface,
        Fold::Over {
            occlude: DEFAULT_OCCLUDE,
        },
    );
    ctx.queue.submit(std::iter::once(encoder.finish()));
}

/// **ADR-0030's compare-first obligation, counted rather than assumed**: a
/// stage rebuilds its resources on a size change and *only* on a size change.
///
/// This is worth a counter rather than a pixel assertion because rebuilding
/// every frame would look almost right: the picture would be correct, but the
/// trails accumulation would be cleared every frame (so trails would silently
/// stop working) and a `Rgba16Float` texture pair would be reallocated at frame
/// rate. Neither is visible in a single captured frame.
#[test]
fn stages_rebuild_on_a_size_change_and_only_on_a_size_change() {
    let Some(ctx) = headless_context_or_skip() else {
        return;
    };
    let format = ctx.surface_format();
    let mut stages = Stages {
        trails: crate::render::trails::Trails::new(&ctx.device, format, FLOOR.post_cap),
        kaleido: crate::render::kaleidoscope::Kaleidoscope::new(
            &ctx.device,
            format,
            FLOOR.post_cap,
        ),
        bloom: crate::render::bloom::Bloom::new(
            &ctx.device,
            format,
            FLOOR.post_cap,
            FLOOR.bloom_levels,
        ),
    };
    // Every stage has to be active or `begin` is never reached.
    assert!(stages.trails.set_param("trails", 0.9));
    assert!(stages.kaleido.set_param("kaleido_order", 6.0));
    assert!(stages.bloom.set_param("bloom_amount", 0.8));

    // Two sizes that the policy maps to *different* grids, so the compare has
    // something to see. (Sizes inside one 256 px step deliberately do not
    // rebuild — that is the point of quantizing.)
    let small = (512, 512);
    let large = (1024, 768);
    assert_ne!(
        floor_grid(small),
        floor_grid(large),
        "the two probe sizes must land on different grids for this test to mean anything"
    );

    for _ in 0..5 {
        pump(&ctx, &mut stages.trails, small);
        pump(&ctx, &mut stages.kaleido, small);
        pump(&ctx, &mut stages.bloom, small);
    }
    assert_eq!(stages.trails.build_count(), 1, "five frames, one build");
    assert_eq!(stages.kaleido.build_count(), 1, "five frames, one build");
    assert_eq!(stages.bloom.build_count(), 1, "five frames, one build");

    for _ in 0..3 {
        pump(&ctx, &mut stages.trails, large);
        pump(&ctx, &mut stages.kaleido, large);
        pump(&ctx, &mut stages.bloom, large);
    }
    assert_eq!(
        stages.trails.build_count(),
        2,
        "a size change builds once more"
    );
    assert_eq!(
        stages.kaleido.build_count(),
        2,
        "a size change builds once more"
    );
    assert_eq!(
        stages.bloom.build_count(),
        2,
        "a size change builds once more"
    );

    // Back to the first size: it must build *again* rather than resurrect the
    // stale grid — the resources for `small` were dropped when `large` replaced
    // them, so reusing them is not merely wasteful, it is impossible.
    for _ in 0..3 {
        pump(&ctx, &mut stages.trails, small);
        pump(&ctx, &mut stages.kaleido, small);
        pump(&ctx, &mut stages.bloom, small);
    }
    assert_eq!(
        stages.trails.build_count(),
        3,
        "returning to a size rebuilds"
    );
    assert_eq!(
        stages.kaleido.build_count(),
        3,
        "returning to a size rebuilds"
    );
    assert_eq!(
        stages.bloom.build_count(),
        3,
        "returning to a size rebuilds"
    );

    // A size within the same 256 px step is free — the quantization is what
    // makes a live window drag survivable. (512 sits exactly *on* a step, so
    // 513 is already the next grid up; the probe has to be inside the step.)
    let same_grid = (400, 400);
    assert_eq!(floor_grid(small), floor_grid(same_grid));
    pump(&ctx, &mut stages.trails, same_grid);
    assert_eq!(
        stages.trails.build_count(),
        3,
        "a resize inside one quantization step must not reallocate"
    );
}

/// The chain reports the **target's** grid, not a fixed 720p — checked through
/// the real `begin`, so it covers the wiring and not just the policy function.
///
/// Plan 0033's done-when for this reads "a `PostChain` driven at 2048x1152
/// reports an internal size of 2048x1152". That is not reachable alongside the
/// same plan's 1920x1080 cap, which 2048x1152 exceeds on width; ADR-0034 says
/// as much when it calls the cap "a 1.07x downscale at the display in
/// question". So the assertion is the capped grid — with the aspect exactly
/// preserved, 2048x1152 and 1920x1080 both being 16:9 — and, per the done-when's
/// actual point, emphatically not 1280x720.
#[test]
fn the_chain_reports_the_targets_grid_not_a_fixed_720p() {
    let Some(ctx) = headless_context_or_skip() else {
        return;
    };
    let format = ctx.surface_format();
    let mut chain = PostChain::new(&ctx.device, format, &FLOOR);
    assert!(chain.set_stage_param(TRAILS, "trails", 0.9));

    let surface = (2048, 1152);
    let (_texture, view) =
        capture::create_target(&ctx.device, format, CHAIN_TEST_SIZE, CHAIN_TEST_SIZE);
    let mut encoder = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("post-target-size"),
        });
    let target = chain.begin(&mut encoder, &view, surface);
    ctx.queue.submit(std::iter::once(encoder.finish()));

    assert_eq!(
        target.size,
        (1920, 1080),
        "the composite must follow the render target under the cap, not sit at 1280x720"
    );
    assert_ne!(target.size, (1280, 720), "the fixed 720p grid is retired");
    assert!(
        (target.aspect - 2048.0 / 1152.0).abs() < 1e-3,
        "the capped grid must keep the target's aspect, got {}",
        target.aspect
    );
}

/// Fold order used by the symmetry probes.
const FOLD_ORDER: usize = 6;

/// Fold a vignetted backdrop at `surface` and report
/// [`fold_mirror_error`] over the result.
fn fold_error_at(ctx: &RenderContext, surface: (u32, u32)) -> f32 {
    let format = ctx.surface_format();
    let mut background = Background::new(&ctx.device, format);
    let mut kaleido =
        crate::render::kaleidoscope::Kaleidoscope::new(&ctx.device, format, FLOOR.post_cap);
    assert!(kaleido.set_param("kaleido_order", FOLD_ORDER as f32));

    let (texture, view) = capture::create_target(&ctx.device, format, surface.0, surface.1);
    let mut encoder = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("kaleido-aspect"),
        });
    background.reset_params();
    assert!(background.set_param("bg_bright", 1.0));
    assert!(background.set_param("bg_vignette", 0.6));
    let src = kaleido
        .begin(&mut encoder, surface)
        .expect("the fold builds its input");
    // The backdrop is the fold's *test pattern* here, not the chain's backdrop
    // — this probe wants radially-structured content to measure symmetry on,
    // and paints it straight into the fold's input. `Fold::Own` because `view`
    // is a fresh capture target with nothing underneath to blend with.
    background.render(&ctx.queue, &mut encoder, &src, surface);
    kaleido.resolve(&ctx.queue, &mut encoder, &view, surface, Fold::Own);
    let (buffer, padded_bpr) = capture::create_readback(&ctx.device, surface.0, surface.1);
    capture::record_copy(
        &mut encoder,
        &texture,
        &buffer,
        padded_bpr,
        surface.0,
        surface.1,
    );
    ctx.queue.submit(std::iter::once(encoder.finish()));
    let img = capture::read_back(&ctx.device, &buffer, surface.0, surface.1, padded_bpr)
        .expect("read back the folded frame");
    fold_mirror_error(&img, FOLD_ORDER)
}

/// The kaleidoscope's aspect correction is the **render target's**, not a
/// compile-time 16:9 (ADR-0034) and not its own internal grid's (ADR-0037).
///
/// Asserted as the fold's own symmetry: an order-N dihedral fold makes the
/// output identical in each of its N wedges *in aspect-corrected space*, so
/// sampling those wedges with the **target's** aspect must give matching means.
/// If the shader folded about a different axis, the wedges would land somewhere
/// else and the means would diverge. Restricted to an inscribed disc so the
/// rectangular frame's corners do not give some wedges more area.
///
/// Two probes, and the second is the one that matters:
///
/// - **(512, 256)** — 2:1, which `internal_grid_size` returns unchanged. This
///   was the whole test through Plan 0033, and it *cannot* distinguish grid
///   aspect from surface aspect, which is why it passed for the entire life of
///   the defect.
/// - **(320, 256)** — grid (512, 256). The target is 1.25:1 and the grid is
///   2:1, so folding about the grid is a 1.6x wrong axis and only the target's
///   aspect scores clean.
#[test]
fn the_fold_stays_symmetric_on_a_non_16_9_target() {
    let Some(ctx) = headless_context_or_skip() else {
        return;
    };

    // The grid-agnostic probe (retained): grid and surface agree here.
    let agreeing = (512, 256);
    assert_eq!(floor_grid(agreeing), (512, 256), "a 2:1 grid");
    // Measured: 0.0099 with the target's aspect, 0.1860 with a baked 1280x720
    // — a 19x separation, and the threshold sits between them. Verified
    // non-vacuous by re-baking the old constant and watching this fail.
    let error = fold_error_at(&ctx, agreeing);
    eprintln!("fold mirror error at {agreeing:?} (grid == surface): {error:.4}");
    assert!(
        error < 0.05,
        "the fold's wedge mirror is broken by {error:.4} of the frame's contrast on a \
         2:1 target — it is being aspect-corrected to something other than the target"
    );

    // The discriminating probe: the grid's shape is not the target's here, so
    // only one of the two can be the axis the fold uses.
    let disagreeing = (320, 256);
    let grid = floor_grid(disagreeing);
    assert_eq!(grid, (512, 256), "the probe's grid must not be its surface");
    assert!(
        (grid.0 as f32 / grid.1 as f32 - disagreeing.0 as f32 / disagreeing.1 as f32).abs() > 0.5,
        "grid {grid:?} and surface {disagreeing:?} must disagree enough to separate them"
    );
    let error = fold_error_at(&ctx, disagreeing);
    eprintln!("fold mirror error at {disagreeing:?} (grid {grid:?}): {error:.4}");
    assert!(
        error < 0.05,
        "the fold's wedge mirror is broken by {error:.4} of the frame's contrast on a \
         target whose grid has a different shape — it is folding about the grid's axis \
         rather than the target's (ADR-0037)"
    );
}

/// How badly the fold's own mirror symmetry is broken, measured in the frame's
/// **true** aspect-corrected space and normalized by the frame's contrast.
///
/// A dihedral fold mirrors within each wedge, so the output must satisfy
/// `L(r, θ) == L(r, 2c - θ)` where `c` is that wedge's centre line. The
/// **screen** direction of those centre lines depends on the aspect the shader
/// corrected by: a line at angle `θ` in aspect-`A` space is a different set of
/// pixels than the same angle in aspect-`T` space. So measuring the symmetry
/// with the true grid aspect detects a shader folding about the wrong one —
/// which comparing whole-wedge *means* cannot, because the fold is periodic and
/// a periodic function's mean over any full period is the same wherever the
/// period starts.
fn fold_mirror_error(img: &CaptureImage, order: usize) -> f32 {
    let (w, h) = (img.width as usize, img.height as usize);
    let aspect = img.width as f32 / img.height.max(1) as f32;
    let seg = std::f32::consts::TAU / order as f32;
    let luma_at = |r: f32, theta: f32| -> Option<f32> {
        // Back to pixels: undo the aspect correction on x only.
        let x = ((r * theta.cos()) / aspect + 0.5) * w as f32 - 0.5;
        let y = (r * theta.sin() + 0.5) * h as f32 - 0.5;
        let (xi, yi) = (x.round() as i32, y.round() as i32);
        if xi < 0 || yi < 0 || xi >= w as i32 || yi >= h as i32 {
            return None;
        }
        let i = (yi as usize * w + xi as usize) * 4;
        Some(
            (0.299 * img.rgba[i] as f32
                + 0.587 * img.rgba[i + 1] as f32
                + 0.114 * img.rgba[i + 2] as f32)
                / 255.0,
        )
    };

    let mut diff = 0.0f32;
    let mut pairs = 0usize;
    let mut values: Vec<f32> = Vec::new();
    for ri in 1..=24 {
        // Inside the inscribed disc, so no sample leaves the frame.
        let r = 0.45 * ri as f32 / 24.0;
        for ti in 0..(order * 12) {
            let theta = std::f32::consts::TAU * ti as f32 / (order * 12) as f32;
            let centre = seg * (theta / seg).floor() + seg * 0.5;
            let mirrored = 2.0 * centre - theta;
            if let (Some(a), Some(b)) = (luma_at(r, theta), luma_at(r, mirrored)) {
                diff += (a - b).abs();
                pairs += 1;
                values.push(a);
            }
        }
    }
    // Normalize by the frame's own contrast, so the number is scale-free and a
    // flat frame cannot score a deceptively good zero.
    let mean = values.iter().sum::<f32>() / values.len().max(1) as f32;
    let contrast =
        values.iter().map(|v| (v - mean).abs()).sum::<f32>() / values.len().max(1) as f32;
    (diff / pairs.max(1) as f32) / contrast.max(1e-6)
}

// -----------------------------------------------------------------------
// ADR-0037: composing a stage must not change the picture's *shape*
// -----------------------------------------------------------------------

/// A test-only stand-in for a scene: a hard-edged disc drawn at whatever
/// `aspect` the composite hands it.
///
/// `world.x = ndc.x * aspect` is the exact inverse of the `ndc = world.x /
/// aspect` projection every real scene applies (`particles/mod.rs:414`,
/// `lines/renderer.rs:76`), so this models the seam under test — a scene's only
/// shape input is that one float — without dragging a whole scene's params,
/// palette and audio bindings into a geometry assertion.
const DISC_SHADER: &str = r#"
struct D { v: vec4<f32> } // x: aspect

@group(0) @binding(0) var<uniform> u: D;

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
let aspect = max(u.v.x, 0.001);
let p = vec2<f32>(in.ndc.x * aspect, in.ndc.y);
let lit = select(0.0, 1.0, length(p) < 0.5);
return vec4<f32>(lit, lit, lit, 1.0);
}
"#;

/// Encode the disc into `target`, projected at `aspect`.
fn draw_disc(
    ctx: &RenderContext,
    encoder: &mut wgpu::CommandEncoder,
    target: &wgpu::TextureView,
    aspect: f32,
) {
    let format = ctx.surface_format();
    let uniform = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("disc-uniform"),
        size: 16,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    ctx.queue
        .write_buffer(&uniform, 0, bytemuck::bytes_of(&[aspect, 0.0, 0.0, 0.0]));
    let layout = ctx
        .device
        .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("disc-bind-layout"),
            entries: &[gpu::uniform(0, wgpu::ShaderStages::FRAGMENT)],
        });
    let bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("disc-bind-group"),
        layout: &layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: uniform.as_entire_binding(),
        }],
    });
    let shader = gpu::fullscreen_shader(
        &ctx.device,
        "disc-shader",
        gpu::FULLSCREEN_VS_NDC,
        DISC_SHADER,
    );
    let pipeline = gpu::fullscreen_pipeline(
        &ctx.device,
        &shader,
        &[&layout],
        format,
        wgpu::BlendState::REPLACE,
        "disc",
    );
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("disc-pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    pass.set_pipeline(&pipeline);
    pass.set_bind_group(0, &bind_group, &[]);
    pass.draw(0..3, 0..1);
}

/// Width/height of the lit region's bounding box, in destination pixels.
/// `1.0` is a round disc; the defect made it the grid's aspect over the
/// target's.
fn lit_extent_ratio(img: &CaptureImage) -> f32 {
    let (w, h) = (img.width as usize, img.height as usize);
    let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0usize, 0usize);
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) * 4;
            if img.rgba[i] > 128 {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x);
                y1 = y1.max(y);
            }
        }
    }
    assert!(x0 != usize::MAX, "the disc drew nothing to measure");
    (x1 - x0 + 1) as f32 / (y1 - y0 + 1) as f32
}

/// Draw the disc through the composite at `surface` — with the trails stage
/// active or with the chain entirely skipped — and report the shape it lands
/// on the destination as.
///
/// Trails is the right stage for this: it computes no geometry of its own, and
/// on its first frame the max-decay against a cleared accumulation is the
/// identity, so anything that moves is the aspect and nothing else.
fn disc_extent_ratio(ctx: &RenderContext, surface: (u32, u32), through_a_stage: bool) -> f32 {
    let format = ctx.surface_format();
    let mut chain = PostChain::new(&ctx.device, format, &FLOOR);
    if through_a_stage {
        assert!(chain.set_stage_param(TRAILS, "trails", 0.5));
    }
    let (texture, view) = capture::create_target(&ctx.device, format, surface.0, surface.1);
    let mut encoder = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("composite-aspect"),
        });
    capture::record_clear(&mut encoder, &view);
    let target = chain.begin(&mut encoder, &view, surface);
    draw_disc(ctx, &mut encoder, &target.view, target.aspect);
    chain.resolve(&ctx.queue, &mut encoder, target.routing, &view, surface);

    let (buffer, padded_bpr) = capture::create_readback(&ctx.device, surface.0, surface.1);
    capture::record_copy(
        &mut encoder,
        &texture,
        &buffer,
        padded_bpr,
        surface.0,
        surface.1,
    );
    ctx.queue.submit(std::iter::once(encoder.finish()));
    let img = capture::read_back(&ctx.device, &buffer, surface.0, surface.1, padded_bpr)
        .expect("read back the composited frame");
    lit_extent_ratio(&img)
}

/// **The defect ADR-0037 records**: turning a post stage on must change
/// the picture's softness, never its **shape**.
///
/// A radially symmetric figure is composited twice at the same target size —
/// once with the chain skipped, once through an active `trails` — and the two
/// must agree. Before the fix `SceneTarget::aspect` came from the quantized
/// internal grid, so the scene drew correct-for-the-grid and the stage's
/// aspect-ignoring present stretched it by grid-aspect-over-target-aspect.
///
/// The sizes are chosen, not incidental. **1280x800** takes a 1280x1024 grid:
/// 1.25 against the target's 1.6, a 1.28x stretch and the worst ordinary case.
/// **1920x1080** is the control the policy returns unchanged — it is what the
/// project develops at, it is why this shipped, and it must move under neither
/// the defect nor the fix.
#[test]
fn composing_a_stage_does_not_change_the_pictures_shape() {
    let Some(ctx) = headless_context_or_skip() else {
        return;
    };

    let skewing = (1280, 800);
    assert_eq!(
        floor_grid(skewing),
        (1280, 1024),
        "the probe size must be one where the grid's shape is not the target's"
    );
    let control = (1920, 1080);
    assert_eq!(
        floor_grid(control),
        control,
        "the control must be a size the policy returns exactly — that is what hid this"
    );

    for surface in [skewing, control] {
        let plain = disc_extent_ratio(&ctx, surface, false);
        let staged = disc_extent_ratio(&ctx, surface, true);
        eprintln!(
            "{surface:?}: disc x/y = {plain:.4} with the chain skipped, \
             {staged:.4} through trails (grid {:?})",
            floor_grid(surface)
        );
        assert!(
            (plain - 1.0).abs() < 0.03,
            "{surface:?}: the reference disc is not round ({plain:.4}) — the \
             comparison below would be meaningless"
        );
        assert!(
            (staged / plain - 1.0).abs() < 0.03,
            "{surface:?}: composing `trails` restretched the picture by \
             {:.3}x ({plain:.4} -> {staged:.4}) — the scene is being handed the \
             internal grid's aspect instead of the target's (ADR-0037)",
            staged / plain
        );
    }
}

/// The active stages always come out in chain order, never reordered — the
/// order is a compile-time constant (ADR-0018, reaffirmed by ADR-0031).
#[test]
fn active_stages_stay_in_chain_order() {
    for active in all_combinations() {
        let stages = route(&active).active_stages().to_vec();
        assert!(
            stages.windows(2).all(|w| w[0] < w[1]),
            "chain order is strictly increasing for {active:?}: {stages:?}"
        );
        let expected: Vec<usize> = active
            .iter()
            .enumerate()
            .filter(|&(_, &on)| on)
            .map(|(index, _)| index)
            .collect();
        assert_eq!(stages, expected, "exactly the active stages run");
    }
}

// -----------------------------------------------------------------------
// `occlude`: how much of the scene's coverage the backdrop resolves
// against (Plan 0071 Phase 1, ADR-0085)
// -----------------------------------------------------------------------

/// The chain-active fixture: a swarm over a **lit** backdrop with `trails`
/// bound, so the last active stage folds onto the backdrop. Shared with
/// `a_lit_backdrop_survives_where_the_swarm_drew_nothing`, which is the
/// ADR-0056 guard at the same seam.
const CHAIN_FIXTURE: &str = include_str!("../../../tests/fixtures/swarm_lit_backdrop.toml");

/// The empty-chain fixture: an attractor cloud over a lit backdrop with **no
/// stage bound at all**, so the scene presents straight onto the backdrop and
/// owns the seam itself. See its header for why it is this scene and not an
/// additive one, and for why `density` is its off switch.
const NO_STAGE_FIXTURE: &str =
    include_str!("../../../tests/fixtures/attractor_lit_backdrop_no_stage.toml");

/// The square capture size — a multiple of [`POST_GRID_STEP`], so the trails
/// stage runs at the target size and its present is a 1:1 sample rather than a
/// resample that would blur the arithmetic being asserted.
const OCCLUDE_CAPTURE_SIZE: u32 = 256;

/// Frames per capture. Long enough for the swarm's seeded velocities to damp
/// and the attractor's cloud to accumulate a deposit worth occluding.
const OCCLUDE_CAPTURE_FRAMES: u32 = 40;

/// Slack for half-precision rounding, the same shape the two lit-backdrop
/// guards use: the composite is `Rgba16Float`, so a value of magnitude `m` is
/// stored to roughly `m / 1024` and two captures quantize different sums.
///
/// It is slack, not a tolerance. Upstream of the tonemap the resolve is a plain
/// premultiplied OVER, so every claim below is **exact** in real arithmetic.
fn occlude_slack(value: f32) -> f32 {
    (4.0 / 1024.0) * value.abs().max(1.0)
}

/// The value of a top-level `key = "<number>"` line in `fixture`, or `NaN` when
/// it is absent — so a fixture stays the single statement of what is captured.
fn fixture_value(fixture: &str, key: &str) -> f32 {
    fixture
        .lines()
        .find_map(|line| {
            let rest = line.trim_start().strip_prefix(key)?;
            let rest = rest.trim_start().strip_prefix('=')?;
            rest.trim().trim_matches('"').parse::<f32>().ok()
        })
        .unwrap_or(f32::NAN)
}

/// The **linear composite** — the frame the tonemap is about to map — for
/// `fixture` with `overrides` applied to its `[params]` table.
///
/// Reads upstream of the tonemap for the reason both lit-backdrop guards do:
/// the tonemap scales all three channels off the brightest one (ADR-0046), so
/// downstream of it every claim about one pixel is entangled with the whole
/// frame. Upstream there is no confound.
///
/// Builds and drops **one** renderer per call rather than holding four: a
/// second live device in a binary is what the software adapter falls over on,
/// and building GPU resources mid-run shifts what the trails stage resolves to
/// on WARP.
///
/// Every override key is stripped from the fixture first and re-appended, which
/// works because `[params]` is the last table in both fixtures.
fn linear_composite(fixture: &str, overrides: &[(&str, f32)]) -> Option<Vec<f32>> {
    use crate::dsp::AnalysisFrame;
    use crate::preset::Preset;
    use crate::render::{HeadlessOptions, Renderer};

    let mut renderer = match Renderer::new_headless(HeadlessOptions {
        width: OCCLUDE_CAPTURE_SIZE,
        height: OCCLUDE_CAPTURE_SIZE,
        prefer_software: true,
    }) {
        Ok(renderer) => renderer,
        Err(RenderError::RequestAdapter(_)) => {
            eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
            return None;
        }
        Err(e) => panic!("headless renderer build failed: {e}"),
    };
    let base: String = fixture
        .lines()
        .filter(|line| {
            let line = line.trim_start();
            !overrides
                .iter()
                .any(|(key, _)| line.starts_with(key) && !line.starts_with('#'))
        })
        .collect::<Vec<_>>()
        .join("\n");
    let mut toml = base;
    for (key, value) in overrides {
        toml.push_str(&format!("\n{key} = \"{value}\"\n"));
    }
    let preset = Preset::from_toml_str(&toml).expect("the occlude fixture parses with overrides");
    let name = preset.name.clone();
    renderer.set_presets(vec![preset]);

    // Every binding is a constant, so the analysis frame only has to be
    // well-formed.
    let frame = AnalysisFrame::default();
    renderer
        .capture_preset(&name, &frame, OCCLUDE_CAPTURE_FRAMES)
        .expect("capture the occlude fixture");

    let device = renderer.ctx.device.clone();
    let queue = renderer.ctx.queue.clone();
    let src = renderer
        .tonemap
        .src_texture()
        .expect("the tonemap built its input while capturing")
        .clone();
    let (buffer, padded_bpr) =
        capture::create_linear_readback(&device, OCCLUDE_CAPTURE_SIZE, OCCLUDE_CAPTURE_SIZE);
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("occlude-readback"),
    });
    capture::record_copy(
        &mut encoder,
        &src,
        &buffer,
        padded_bpr,
        OCCLUDE_CAPTURE_SIZE,
        OCCLUDE_CAPTURE_SIZE,
    );
    queue.submit(std::iter::once(encoder.finish()));
    Some(
        capture::read_back_linear(
            &device,
            &buffer,
            OCCLUDE_CAPTURE_SIZE,
            OCCLUDE_CAPTURE_SIZE,
            padded_bpr,
        )
        .expect("read back the linear composite"),
    )
}

/// The three captures plus the backdrop, checked against the arithmetic
/// ADR-0085 defines: `out = scene + bg * (1 - alpha * occlude)`.
///
/// Split out because both paths — chain-active and empty-chain — assert exactly
/// the same three properties on exactly the same shape of data, and the point of
/// Phase 1 is that the two paths agree. `label` names the path in failures.
fn assert_occlude_arithmetic(label: &str, fixture: &str, silence: (&str, f32)) {
    // The backdrop alone — `B`, PER PIXEL. Rendered from **this fixture's own
    // file** with the scene switched off, because since ADR-0086 the sky colours
    // through the preset's own palette: a second fixture is a second sky, and the
    // property would be measured against a backdrop that was never underneath.
    // An earlier draft did exactly that and read 106 382 false violations.
    //
    // Per pixel rather than as one number, because the sky is **not flat**:
    // `background.rs` paints a gentle vertical gradient (0.72 to 1.0) whatever
    // `bg_vignette` is. A constancy check written against a flat sky failed on
    // both fixtures at 0.049 — the gradient, not the seam.
    let Some(backdrop) = linear_composite(fixture, &[silence]) else {
        return;
    };
    let Some(covering) = linear_composite(fixture, &[("occlude", 1.0)]) else {
        return;
    };
    let Some(half) = linear_composite(fixture, &[("occlude", 0.5)]) else {
        return;
    };
    let Some(adding) = linear_composite(fixture, &[("occlude", 0.0)]) else {
        return;
    };
    assert_eq!(covering.len(), backdrop.len(), "{label}: capture sizes");
    assert_eq!(covering.len(), half.len(), "{label}: capture sizes");
    assert_eq!(covering.len(), adding.len(), "{label}: capture sizes");

    // Printed rather than asserted on: the means are what tell a reader whether a
    // failure below is the seam or the fixture. A lit capture whose mean equals
    // the backdrop's is a scene that drew nothing.
    for (name, cap) in [
        ("backdrop   ", &backdrop),
        ("occlude = 1", &covering),
        ("occlude = 0", &adding),
    ] {
        let sum: f32 = cap.iter().sum();
        eprintln!("{label} {name}: mean {:.5}", sum / cap.len() as f32);
    }

    let (mut held_out, mut not_monotone, mut off_midpoint) = (0usize, 0usize, 0usize);
    let (mut occluding, mut worst_midpoint, mut worst_held) = (0usize, 0.0f32, 0.0f32);
    for index in 0..covering.len() {
        // Alpha is not part of the claim — the blend consumes it, and the
        // tonemap downstream writes its own.
        if index % 4 == 3 {
            continue;
        }
        let (b, c, h, a) = (backdrop[index], covering[index], half[index], adding[index]);
        // 1. At `occlude = 0` the scene never takes light *away* from the
        //    backdrop: light adds, it does not cover.
        let shortfall = b - a;
        if shortfall > worst_held {
            worst_held = shortfall;
        }
        if shortfall > occlude_slack(b) {
            held_out += 1;
        }
        // 2. Less occlusion never darkens: the factor is monotone in `occlude`.
        if a < c - occlude_slack(c) {
            not_monotone += 1;
        }
        // 3. A value between the two ends lands **exactly** between them —
        //    `bg * (1 - alpha * occlude)` is affine in `occlude`, so 0.5 is the
        //    midpoint and not merely somewhere in the interval. That is what
        //    makes this a blend of the two models rather than a switch between
        //    them (ADR-0085's continuity argument).
        let midpoint = 0.5 * (c + a);
        let miss = (h - midpoint).abs();
        if miss > worst_midpoint {
            worst_midpoint = miss;
        }
        if miss > occlude_slack(midpoint) {
            off_midpoint += 1;
        }
        // Non-vacuity: where the two ends genuinely differ, the seam is
        // occluding something and the knob reaches it.
        if a - c > 8.0 * occlude_slack(c) {
            occluding += 1;
        }
    }
    let channels = covering.len() / 4 * 3;
    eprintln!(
        "{label}: {occluding} of {channels} channels move between occlude 1 and \
         0; worst backdrop shortfall {worst_held:.5}, worst |half - midpoint| \
         {worst_midpoint:.5}"
    );

    // --- Non-vacuity first: a path where `occlude` changes nothing would pass
    // all three properties trivially. ---
    // A fiftieth of the frame's channels; both fixtures clear it with room. The
    // bar is deliberately low, because what it has to exclude is a path where
    // `occlude` reaches nothing at all — which reads as **zero**, not as a small
    // number. It read exactly zero on WARP while this was being written, from a
    // bind-group layout that collided with the backdrop's.
    assert!(
        occluding * 50 > channels,
        "{label}: only {occluding} of {channels} channels differ between \
         occlude = 1 and occlude = 0. This path is not occluding the backdrop \
         at all, so the properties below are vacuous — check that the fixture \
         still lights its backdrop and still draws over it, and that this \
         pass's bind-group layout is still a shape no other live pipeline has \
         (see `PRESENT_SHADER` in trails.rs)"
    );

    // --- The properties. ---
    assert_eq!(
        held_out, 0,
        "{label}: at occlude = 0, {held_out} channels fall BELOW the unoccluded \
         backdrop (worst {worst_held:.5}). Light that never covers can only \
         add, so every pixel must be at least as bright as the sky alone — a \
         shortfall means the seam is still resolving against a coverage it was \
         told to ignore"
    );
    assert_eq!(
        not_monotone, 0,
        "{label}: {not_monotone} channels are darker at occlude = 0 than at \
         occlude = 1 — the factor is not monotone, so `occlude` is not scaling \
         the alpha the blend resolves against"
    );
    assert_eq!(
        off_midpoint, 0,
        "{label}: {off_midpoint} channels at occlude = 0.5 are not the midpoint \
         of the two ends (worst {worst_midpoint:.5}). The resolve is affine in \
         `occlude`, so a mid value must be a blend of the two models and not a \
         switch between them"
    );
}

/// **The chain-active path**: a post stage is bound, so the last active stage
/// folds onto the backdrop and `occlude` reaches it through [`Fold::Over`].
///
/// This is the path ADR-0085's rendered evidence was measured on — every shipped
/// swarm and line preset composes at least one stage.
#[test]
fn occlude_scales_the_backdrop_the_chains_last_stage_resolves_against() {
    // Non-vacuity, before any GPU work: the fixture must still describe the
    // configuration this covers.
    let backdrop = fixture_value(CHAIN_FIXTURE, "bg_bright");
    let trails = fixture_value(CHAIN_FIXTURE, "trails");
    assert!(
        backdrop > 0.0,
        "swarm_lit_backdrop.toml no longer lights its backdrop (bg_bright = \
         {backdrop}); at a black backdrop the two occlusion models are \
         identical and this proves nothing (ADR-0085)"
    );
    assert!(
        trails > 0.0,
        "swarm_lit_backdrop.toml no longer binds `trails` (= {trails}), so no \
         stage folds onto the backdrop and this test has moved to the other \
         path without saying so"
    );
    // Silenced with zero-area sprites: the quads rasterize no fragments, so the
    // trail accumulation stays empty, the chain resolves fully transparent, and
    // the capture is the backdrop alone through the same pipeline as the lit ones.
    assert_occlude_arithmetic("chain-active", CHAIN_FIXTURE, ("size", 0.0));
}

/// **The empty-chain path**: no stage is active, so the scene presents straight
/// onto the backdrop and `occlude` reaches its own present pass instead.
///
/// The two paths are separate code and a factor applied to only one of them is
/// the bug this phase could most plausibly have shipped (Plan 0071's Risks).
/// They are asserted with the same function on purpose.
#[test]
fn occlude_releases_the_backdrop_with_no_post_stage_active() {
    let backdrop = fixture_value(NO_STAGE_FIXTURE, "bg_bright");
    assert!(
        backdrop > 0.0,
        "attractor_lit_backdrop_no_stage.toml no longer lights its backdrop \
         (bg_bright = {backdrop})"
    );
    // The whole point of this fixture is that the chain is EMPTY. A stage
    // binding would move the seam to the chain and silently duplicate the test
    // above.
    for stage_params in super::STAGE_PARAMS {
        for name in stage_params.iter() {
            assert!(
                fixture_value(NO_STAGE_FIXTURE, name).is_nan(),
                "attractor_lit_backdrop_no_stage.toml now binds `{name}`, so a \
                 post stage is active and the scene no longer presents onto \
                 the backdrop — which is the seam this fixture exists to reach"
            );
        }
    }
    // Silenced by depositing no light: the accumulation stays black, and this
    // scene's alpha is that accumulation's luminance — so the present emits
    // neither light nor coverage and the capture is the backdrop alone.
    assert_occlude_arithmetic("empty-chain", NO_STAGE_FIXTURE, ("brightness", 0.0));
}

/// `Fold` carries the factor, and `Own` carries a **literal** 1.0 — which is
/// what makes an unbound preset byte-identical rather than approximately so.
#[test]
fn the_own_fold_scales_alpha_by_exactly_one() {
    assert_eq!(Fold::Own.alpha_scale(), 1.0);
    assert_eq!(Fold::Over { occlude: 1.0 }.alpha_scale(), 1.0);
    assert_eq!(Fold::Over { occlude: 0.0 }.alpha_scale(), 0.0);
    assert_eq!(Fold::Over { occlude: 0.25 }.alpha_scale(), 0.25);
    // The default is the value that changes nothing.
    assert_eq!(DEFAULT_OCCLUDE, 1.0);
}

/// The chain owns `occlude` by name, clamps it into `[0, 1]`, and resets it
/// every frame.
///
/// The clamp is not decoration: past 1 the blend's `1 - a * occlude` goes
/// negative and *subtracts* the backdrop under the figure — the Plan 0045
/// Phase 4b defect, reachable through a bound expression. A `[smoothing]`
/// ease sweeps this param continuously, so it is the value the frame uses
/// that has to be in range.
#[test]
fn the_chain_clamps_and_resets_occlude() {
    let Some(ctx) = headless_context_or_skip() else {
        return;
    };
    let mut chain = PostChain::new(&ctx.device, ctx.surface_format(), &FLOOR);
    assert_eq!(chain.occlude(), DEFAULT_OCCLUDE);

    assert!(chain.set_chain_param("occlude", 0.25));
    assert_eq!(chain.occlude(), 0.25);
    assert!(
        !chain.set_chain_param("trails", 0.5),
        "a stage's name is not"
    );
    assert_eq!(chain.occlude(), 0.25, "and does not touch this");

    for (value, expected) in [(2.0, 1.0), (-1.0, 0.0), (f32::NAN, DEFAULT_OCCLUDE)] {
        chain.set_chain_param("occlude", value);
        assert_eq!(chain.occlude(), expected, "{value} resolves to {expected}");
    }

    chain.set_chain_param("occlude", 0.0);
    chain.reset_params();
    assert_eq!(
        chain.occlude(),
        DEFAULT_OCCLUDE,
        "an unbound preset must not inherit the last one's occlusion"
    );
}

/// The chain's vocabulary and the stages' are **disjoint**, which is what lets
/// `resolve_route` test one before the other and take the first match.
#[test]
fn the_chain_vocabulary_does_not_overlap_the_stages() {
    for name in CHAIN_PARAMS {
        assert!(
            super::stage_for(name).is_none(),
            "`{name}` is claimed by both the chain and a stage; route \
             resolution takes the first match, so one of them would never be \
             reached"
        );
    }
    assert!(CHAIN_PARAMS.contains(&"occlude"));
}
