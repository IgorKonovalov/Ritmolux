//! The waterfall system through the renderer: a `[waterfall] rows` past what the
//! tier's `seg3d_segments` draws is clamped with a notice that reaches the
//! renderer, and one within it announces nothing.

use rlx_core::preset::Preset;
use rlx_core::render::scenes::OverflowContext;
use rlx_core::render::{Renderer, TierConfig};

use crate::common;

fn load(renderer: &mut Renderer, toml: &str) {
    let preset = Preset::from_toml_str(toml).expect("the preset loads");
    let name = preset.name.clone();
    renderer.set_presets(vec![preset]);
    renderer
        .capture_preset(&name, &common::fixed_frame_spectrum(), 3)
        .expect("capture");
}

/// Plan 0238 Phase 2's done-when: an over-cap `rows` is clamped with a notice
/// naming what was asked and carrying the cap that bit.
#[test]
fn an_over_cap_row_count_is_clamped_with_a_notice() {
    let Some(mut renderer) = common::headless(64, 64) else {
        return;
    };
    let cap = TierConfig::FLOOR.seg3d_segments as usize;
    let per_row = 63usize;
    let fits = (cap / per_row) as u32;
    let preset = |rows: u32| {
        format!(
            "system = \"waterfall\"\nname = \"rows_{rows}\"\n\
             [waterfall]\nelements = 64\nrows = {rows}\n"
        )
    };

    load(&mut renderer, &preset(fits + 10));
    let notice = renderer
        .cap_overflow()
        .copied()
        .unwrap_or_else(|| panic!("the row clamp must reach the renderer"));
    assert_eq!(notice.context, OverflowContext::Rows(fits + 10, 63));
    assert_eq!(notice.cap, cap);
    assert_eq!(notice.dropped, 10 * per_row);
    let text = notice.to_string();
    assert!(
        text.contains(&format!("keeps {fits} rows")) && text.contains("pin --tier rich"),
        "Floor's cap is under Rich's, so the remedy names it: {text}"
    );

    load(&mut renderer, &preset(fits));
    assert_eq!(
        renderer.cap_overflow(),
        None,
        "rows at the cap is not an overflow"
    );
}
