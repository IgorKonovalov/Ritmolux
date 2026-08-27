//! The runtime tier switch (Plan 0050 Phase 1, ADR-0054): `Renderer::set_tier`
//! rebuilds the tier-dependent GPU resources on the live context — and refuses to
//! on a surface-less one.
//!
//! # What can and cannot be asserted here
//!
//! **The refusal is testable; the rebuild is not.** A `Renderer` holding a real
//! surface needs a window, and there is none in CI, so every renderer this file
//! can build is the headless one `set_tier` is defined to ignore. That asymmetry
//! is exactly why ADR-0054's condition lives in
//! [`lmv_core::render::tier::tier_change_permitted`] as a **value** rather than as
//! a branch inside the mutator: the unit test beside it
//! (`a_tier_change_is_permitted_only_where_there_is_a_surface`) asserts both
//! directions, so "permits a surfaced context" is pinned somewhere even though
//! this file cannot reach one.
//!
//! Without that split, a `set_tier` with an empty body would satisfy everything
//! below. The eyes-on half — that a live switch visibly changes the picture and
//! re-accumulates rather than freezing — is Plan 0050 Phase 6 (`human`), and no
//! test in this repo can stand in for it.
//!
//! Skips with no adapter per ADR-0016.

use lmv_core::render::{HeadlessOptions, RenderError, Renderer, Tier, TierConfig};

/// Small: nothing here reads a pixel, so the capture size only has to be legal.
const SIZE: u32 = 64;

/// `None` on a runner with no GPU adapter at all (ADR-0016).
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

/// **ADR-0045's by-construction guarantee, defended against the mutator ADR-0054
/// adds.** A capture path cannot leave the floor, whatever a caller asks for.
///
/// The capacity half is not a second, independent check — it is the same fact
/// stated where it bites. `Renderer::tier()` reads the *live* `TierConfig`'s own
/// tag, and a `TierConfig` is only ever built by `for_tier`, so a rebuild at
/// `Rich` would have to move both together. Asserting the config the reported
/// tier resolves to is `FLOOR` is therefore the claim "this renderer's capacities
/// are still the floor's", spelled out rather than left to the reader.
#[test]
fn a_headless_renderer_refuses_every_tier_change() {
    let Some(mut r) = headless() else {
        return;
    };
    assert_eq!(r.tier(), Tier::Floor, "new_headless pins the floor");

    r.set_tier(Tier::Rich);
    assert_eq!(
        r.tier(),
        Tier::Floor,
        "set_tier(Rich) moved a headless renderer off the floor — every golden \
         baseline in this project is blessed there"
    );
    assert_eq!(
        TierConfig::for_tier(r.tier()),
        TierConfig::FLOOR,
        "the live capacities are no longer the floor's"
    );

    // Idempotent, and asking for the tier it is already on is equally inert.
    r.set_tier(Tier::Rich);
    r.set_tier(Tier::Floor);
    assert_eq!(r.tier(), Tier::Floor);

    // Non-vacuity: the two tiers must genuinely differ in capacity, or "still the
    // floor's" is a claim about a table where nothing varies. A `const` block
    // because both sides are constants — so this fails at compile time, which is
    // the earliest it can.
    const {
        assert!(
            TierConfig::RICH.attractor_particles > TierConfig::FLOOR.attractor_particles,
            "the tier table does not distinguish the two tiers"
        );
    }
}

/// **The roster is not collateral of a tier change** (ADR-0054: the operator stays
/// on the preset they were watching).
///
/// `active_index` is the accessor Plan 0050 Phase 2 opens the browse overlay on,
/// so it is pinned here as well as being the thing compared across the call.
///
/// Honest about its own reach: on this renderer `set_tier` returns early, so what
/// this proves is that the *entry point* does not disturb the roster on its way to
/// refusing. The rebuild path's own roster preservation comes from `apply_tier`
/// reusing `configure_active_scene` — the same path the frame-time governor's
/// demotion has always taken — and is confirmed eyes-on in Phase 6.
#[test]
fn a_tier_change_leaves_the_roster_and_the_active_preset_alone() {
    let Some(mut r) = headless() else {
        return;
    };
    let names: Vec<String> = r.preset_names().map(str::to_owned).collect();
    assert!(
        names.len() > 1,
        "the embedded roster must hold more than one preset for this to say anything"
    );

    // Move off index 0 first — an assertion that `active_index` is still 0 would
    // hold for an accessor that returned a constant.
    let target = names.len() - 1;
    r.select_preset_now(target);
    assert_eq!(r.active_index(), target);
    let before_name = r.preset_name().to_owned();

    r.set_tier(Tier::Rich);

    assert_eq!(r.active_index(), target, "the active index moved");
    assert_eq!(r.preset_name(), before_name, "the active preset changed");
    let after: Vec<String> = r.preset_names().map(str::to_owned).collect();
    assert_eq!(after, names, "the roster itself changed");
}
