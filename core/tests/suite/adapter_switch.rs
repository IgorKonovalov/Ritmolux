//! The runtime adapter switch (ADR-0246): `Renderer::set_adapter` rebuilds every
//! GPU-owning member on another adapter's device — and refuses to on a
//! surface-less one.
//!
//! # What can and cannot be asserted here
//!
//! The same asymmetry `tier_switch.rs` records: **the refusal is testable; the
//! rebuild is not.** A `Renderer` holding a real surface needs a window, and
//! there is none in CI, so every renderer this file can build is the headless
//! one `set_adapter` is defined to refuse. That is why the condition lives in
//! [`rlx_core::render::adapter_change_permitted`] as a **value** — both
//! directions are pinned below, so "permits a surfaced context" is asserted
//! somewhere even though this file cannot reach one. The switch itself, on a
//! window, is the eyes-on reading only the owner's hardware produces.
//!
//! Skips with no adapter per ADR-0016, and **skips on a roster of fewer than two
//! adapters**, naming what it saw: a switch on a single-adapter machine has
//! nowhere to go, and a pass there would assert nothing.

use rlx_core::render::{
    AdapterChoice, HeadlessOptions, RenderError, Renderer, Tier, adapter_change_permitted,
    list_adapters,
};

use crate::common;

/// Small: nothing here reads a pixel, so the capture size only has to be legal.
const SIZE: u32 = 64;

/// A window target that answers nothing. The headless refusal is decided before
/// the target is looked at, and handing over one that could never become a
/// surface is what proves that order.
struct NoWindow;

impl wgpu::rwh::HasWindowHandle for NoWindow {
    fn window_handle(&self) -> Result<wgpu::rwh::WindowHandle<'_>, wgpu::rwh::HandleError> {
        Err(wgpu::rwh::HandleError::Unavailable)
    }
}

impl wgpu::rwh::HasDisplayHandle for NoWindow {
    fn display_handle(&self) -> Result<wgpu::rwh::DisplayHandle<'_>, wgpu::rwh::HandleError> {
        Err(wgpu::rwh::HandleError::Unavailable)
    }
}

/// The roster, or `None` after the printed notice when it holds fewer than two
/// adapters.
fn two_adapters() -> Option<Vec<rlx_core::render::AdapterDescription>> {
    let roster = list_adapters();
    if roster.len() < 2 {
        eprintln!(
            "skipped: an adapter switch needs two adapters and this machine enumerates {} \
             (ADR-0016): {:?}",
            roster.len(),
            roster.iter().map(|a| a.detail.as_str()).collect::<Vec<_>>()
        );
        return None;
    }
    Some(roster)
}

/// The ADR-0246 guard, **both directions** — the shape ADR-0054's tier guard
/// takes, for the same reason: the refusal alone would be satisfied by a
/// `set_adapter` whose body were empty.
#[test]
fn an_adapter_change_is_permitted_only_where_there_is_a_surface() {
    assert!(
        !adapter_change_permitted(false),
        "a surface-less context is the capture path, which renders on the adapter it was \
         built on for the life of the run"
    );
    assert!(
        adapter_change_permitted(true),
        "a surfaced context is the live app, which is the point of ADR-0246"
    );
}

/// **A headless renderer refuses to change adapter by name, and is untouched by
/// the refusal**: the adapter, the roster, the active preset all read as before,
/// and it still renders a frame — which is the transactional half a surface-less
/// renderer can demonstrate. Exercised against a *real* other adapter, so the
/// refusal is decided by the missing surface and not by an unresolvable choice.
#[test]
fn a_headless_renderer_refuses_an_adapter_change_and_keeps_rendering() {
    let Some(roster) = two_adapters() else {
        return;
    };
    let Some(mut r) = common::headless(SIZE, SIZE) else {
        return;
    };
    let before = r.adapter_description().to_owned();
    let other = roster
        .iter()
        .position(|a| a.detail != before)
        .expect("a two-adapter roster holds one the renderer is not on");
    let names: Vec<String> = r.preset_names().map(str::to_owned).collect();
    assert!(
        names.len() > 1,
        "the embedded roster must hold more than one preset"
    );
    // Off index 0 first, so "unchanged" is not satisfied by an accessor
    // returning a constant.
    let target = names.len() - 1;
    r.select_preset_now(target);
    let preset = r.preset_name().to_owned();

    let err = r
        .set_adapter(&AdapterChoice::Index(other), NoWindow)
        .expect_err("a headless renderer must refuse an adapter change");
    assert!(
        matches!(err, RenderError::Headless),
        "the refusal must be the named headless error, not {err}"
    );

    assert_eq!(r.adapter_description(), before, "the adapter moved");
    assert_eq!(r.active_index(), target, "the active index moved");
    assert_eq!(r.preset_name(), preset, "the active preset changed");
    let after: Vec<String> = r.preset_names().map(str::to_owned).collect();
    assert_eq!(after, names, "the roster itself changed");
    let image = r
        .capture_frame(&common::fixed_frame())
        .expect("the renderer must still render after a refused switch");
    assert_eq!((image.width, image.height), (SIZE, SIZE));
    assert!(
        image.rgba.iter().any(|&b| b != 0),
        "the frame after a refused switch is blank"
    );

    // Idempotent: asking for the adapter it is on is refused the same way,
    // rather than taken as a switch that happened.
    let same = roster
        .iter()
        .position(|a| a.detail == before)
        .expect("the renderer's own adapter is in the roster");
    assert!(matches!(
        r.set_adapter(&AdapterChoice::Index(same), NoWindow),
        Err(RenderError::Headless)
    ));
}

/// **A full adapter name read back from the roster selects exactly that
/// adapter** — the rule the settings row's `[output] gpu` write rests on: it
/// stores the roster's own name, and the name has to resolve to the adapter it
/// was read from even where it is a prefix of another's. Asserted by building
/// on each roster entry by its full name and reading back which adapter was
/// used; skipped where there are not two names to tell apart.
#[test]
fn a_full_name_from_the_roster_selects_exactly_that_adapter() {
    let Some(roster) = two_adapters() else {
        return;
    };
    for entry in &roster {
        let built = Renderer::new_headless_on(
            HeadlessOptions {
                width: SIZE,
                height: SIZE,
                prefer_software: false,
            },
            Tier::Floor,
            &AdapterChoice::Named(entry.name.clone()),
        );
        let r = match built {
            Ok(r) => r,
            // Two roster entries can share one name — a Vulkan and a GL view
            // of one card — and then the name is ambiguous by construction,
            // which is the refusal ADR-0146 asks for rather than a pick.
            Err(RenderError::AmbiguousAdapter { .. }) => continue,
            Err(err) => panic!(
                "building on '{}' by its full name failed: {err}",
                entry.name
            ),
        };
        assert_eq!(
            r.adapter_description(),
            entry.detail,
            "'{}' resolved to another adapter",
            entry.name
        );
    }
}
