//! The analytic field's CPU-side contracts: the family roster, the family table
//! the generated reference prints, and the quantizers that stand between a
//! bound value and the shader.
// Test asserts panic freely; this is not the render path.
#![allow(clippy::panic, clippy::indexing_slicing)]

use super::*;

/// Every family round-trips through its name, and the shader index is its
/// position in the roster — the two are numbered alike by construction only if
/// this holds.
#[test]
fn every_family_round_trips_and_indexes_by_roster_position() {
    for (position, family) in FieldFamily::ALL.into_iter().enumerate() {
        assert_eq!(FieldFamily::from_name(family.as_str()), Some(family));
        assert_eq!(family.index() as usize, position, "{family:?}");
    }
    assert_eq!(FieldFamily::from_name("Chladni"), None, "names are exact");
    assert_eq!(FieldFamily::from_name(""), None);
    assert_eq!(FieldConfig::default().family, FieldFamily::Chladni);
}

/// [`FAMILY_PARAMS`] is a statement about the engine, so it is held to it: each
/// row names a declared parameter once, lists every family by the name a preset
/// uses and in roster order, and carries a spec range some family reads.
#[test]
fn the_family_table_is_the_roster() {
    let families: Vec<&str> = FieldFamily::ALL.iter().map(|f| f.as_str()).collect();
    let mut seen = Vec::new();
    for row in FAMILY_PARAMS {
        assert!(!seen.contains(&row.name), "`{}` has two rows", row.name);
        seen.push(row.name);
        let spec = PARAMS
            .iter()
            .find(|spec| spec.name == row.name)
            .unwrap_or_else(|| panic!("`{}` is not a declared parameter", row.name));
        let listed: Vec<&str> = row.ranges.iter().map(|r| r.family).collect();
        assert_eq!(listed, families, "`{}` must list every family", row.name);
        assert!(
            row.ranges.iter().any(|r| r.range == spec.range),
            "`{}`'s spec range {:?} is no family's range",
            row.name,
            spec.range
        );
    }
    assert_eq!(
        crate::render::scenes::family_params("analytic_field"),
        FAMILY_PARAMS,
        "the reference must reach this table under the system's own label"
    );
}

/// A mode number reaches the shader whole and inside the plate's range, and a
/// non-finite one never reaches it at all.
#[test]
fn a_bound_mode_is_clamped_and_rounded_before_the_shader() {
    for (value, applied) in [
        (3.0, 3.0),
        (3.4, 3.0),
        (4.6, 5.0),
        (0.0, 1.0),
        (-7.0, 1.0),
        (99.0, MAX_MODE),
    ] {
        assert_eq!(applied_mode(value, 3.0), applied, "{value}");
    }
    assert_eq!(applied_mode(f32::NAN, 3.0), 3.0);
    assert_eq!(applied_mode(f32::INFINITY, 5.0), 5.0);
    // And the Structural quantizer upstream composes with it to the identity,
    // which is the audit rule a `Structural` mark is held to (ADR-0180 Outcome).
    for i in -40..400 {
        let value = i as f32 * 0.07;
        let quantized = ParamKind::Structural.quantize(value);
        assert_eq!(
            applied_mode(quantized, 3.0),
            applied_mode(value, 3.0),
            "rounding upstream moved the plate at {value}"
        );
    }
}
