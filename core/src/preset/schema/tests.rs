// Tests panic on failure; this file is not the render path.
#![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]

use super::*;
use crate::render::scenes::particles::MIN_PARTICLE_DENSITY;

fn attractor(extra: &str) -> Result<Preset, PresetError> {
    attractor_family("lorenz", extra)
}

fn attractor_family(family: &str, extra: &str) -> Result<Preset, PresetError> {
    Preset::from_toml_str(&format!(
        "system = \"attractor\"\nname = \"t\"\n[particles]\nfamily = \"{family}\"\n{extra}"
    ))
}

/// The IFS figures share the `family` namespace with the four map families
/// (ADR-0075), and an unknown name is still a load error rather than a
/// silent fallback to De Jong.
#[test]
fn an_ifs_figure_is_selected_by_the_family_key() {
    for figure in IfsFigure::ALL {
        let preset = attractor_family(figure.name(), "")
            .unwrap_or_else(|e| panic!("'{}' should select an IFS figure: {e}", figure.name()));
        match preset.config {
            Some(GeneratorConfig::Particles { family, .. }) => {
                assert_eq!(family, AttractorFamily::Ifs(figure));
            }
            _ => panic!("an attractor preset carries a Particles config"),
        }
    }
    let err = attractor_family("barnsley", "").expect_err("unknown family");
    assert!(err.to_string().contains("unknown attractor family"));
}

/// **`morph_to` is validated at the boundary, both ways** (Plan 0062 Phase 3).
///
/// An unknown figure is a load error, and so is a `morph_to` next to one of
/// the four *map* families — which have no table to interpolate. The second
/// is the one worth erroring on rather than ignoring: a silent no-op would
/// leave an author binding `morph` to audio, watching nothing happen, and
/// having the preset load cleanly.
#[test]
fn morph_to_is_validated_against_its_family_at_load() {
    // Every figure is a legal target, including a figure's own name.
    for figure in IfsFigure::ALL {
        let preset = attractor_family("fern", &format!("morph_to = \"{}\"\n", figure.name()))
            .unwrap_or_else(|e| panic!("fern -> {} should load: {e}", figure.name()));
        match preset.config {
            Some(GeneratorConfig::Particles { morph_to, .. }) => {
                assert_eq!(morph_to, Some(figure));
            }
            _ => panic!("an attractor preset carries a Particles config"),
        }
    }

    // An unknown target names the key and the bad value.
    let err = attractor_family("fern", "morph_to = \"maple\"\n").expect_err("unknown figure");
    let msg = err.to_string();
    assert!(
        msg.contains("morph_to") && msg.contains("maple"),
        "the error must name the key and the value, got: {msg}"
    );

    // On a map family it is an error, not a no-op — for all four.
    for family in ["de_jong", "clifford", "thomas", "lorenz"] {
        let err = attractor_family(family, "morph_to = \"spiral\"\n")
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("morph_to") && err.contains(family),
            "{family}: the error must name the key and the family, got: {err}"
        );
    }
}

/// An absent `morph_to` pins the figure, which is what makes `morph` inert
/// on a preset that never mentions either.
#[test]
fn an_absent_morph_to_pins_the_figure() {
    let preset = attractor_family("fern", "").unwrap();
    match preset.config {
        Some(GeneratorConfig::Particles { morph_to, .. }) => assert_eq!(morph_to, None),
        _ => panic!("an attractor preset carries a Particles config"),
    }
}

/// `[particles] density` is validated at load, with the range in the message
/// (Plan 0059 Phase 2 / ADR-0069) — like every other structural key, and
/// unlike a bindable param it cannot be clamped per frame.
#[test]
fn density_is_range_checked_at_load() {
    for good in ["", "density = 1.0\n", "density = 0.5\n", "density = 0.01\n"] {
        assert!(
            attractor(good).is_ok(),
            "`{good}` should load, it is inside {MIN_PARTICLE_DENSITY}..=1.0"
        );
    }

    // Above the tier budget, at zero, negative, and below the floor. The tier
    // caps the top: a preset cannot ask for more particles than exist.
    for bad in ["density = 1.5\n", "density = 0.0\n", "density = -0.2\n"] {
        let err = attractor(bad).expect_err("`{bad}` must be rejected");
        let msg = err.to_string();
        assert!(
            msg.contains("[particles] density") && msg.contains("1.0"),
            "the error must name the key and its range, got: {msg}"
        );
    }

    // The boundary is inclusive on both ends and exclusive just below — so the
    // message's stated range is the range that is actually enforced, which is
    // the part a reader of the error depends on.
    assert!(attractor("density = 0.0004\n").is_err());
    assert!(attractor(&format!("density = {MIN_PARTICLE_DENSITY}\n")).is_ok());
}

/// An absent `[particles] density` is the whole budget. This is what makes the
/// key a strict superset — every preset shipped before it existed keeps its
/// exact sample count, so no capture moves.
#[test]
fn an_absent_density_is_the_whole_budget() {
    let with = attractor("density = 1.0\n").unwrap();
    let without = attractor("").unwrap();
    let density_of = |p: &Preset| match p.config {
        Some(GeneratorConfig::Particles { density, .. }) => density,
        _ => panic!("attractor preset carries a Particles config"),
    };
    assert_eq!(density_of(&with), 1.0);
    assert_eq!(density_of(&without), 1.0);
}

// -----------------------------------------------------------------------
// The `[generator] rings` roster (Plan 0065 Phase 1 / ADR-0079)
// -----------------------------------------------------------------------

fn star(generator: &str) -> Result<Preset, PresetError> {
    Preset::from_toml_str(&format!(
        "system = \"star_pattern\"\nname = \"t\"\n[generator]\n{generator}"
    ))
}

fn rings_of(preset: &Preset) -> Vec<RingSpec> {
    match &preset.config {
        Some(GeneratorConfig::Star { rings, .. }) => rings.clone(),
        _ => panic!("a star preset carries a Star config"),
    }
}

/// **The roster is a strict superset**: a preset that declares no `rings`
/// gets an empty one, which is what makes Plan 0065 add geometry without
/// moving a pixel of anything already shipped.
#[test]
fn an_absent_rings_key_is_an_empty_roster() {
    let p = star("tiling = \"12\"\ncontact_angle_deg = 20\n").unwrap();
    assert!(rings_of(&p).is_empty());
    match p.config {
        Some(GeneratorConfig::Star {
            order,
            contact_angle_deg,
            ..
        }) => assert_eq!((order, contact_angle_deg), (12, 20.0)),
        _ => panic!("a star preset carries a Star config"),
    }
}

/// The roster parses in declaration order, with `scale` and `phase`
/// defaulting — the five keys ADR-0079's data shape names.
#[test]
fn a_declared_roster_parses_in_order_with_its_defaults() {
    let p = star(
        "tiling = \"6\"\n\
         rings = [\n\
         { motif = \"petal\", count = 12, radius = 0.4, scale = 0.2, phase = 0.25 },\n\
         { motif = \"circle\", count = 24, radius = 0.75 },\n\
         ]\n",
    )
    .unwrap();
    let rings = rings_of(&p);
    assert_eq!(rings.len(), 2);
    assert_eq!(
        rings[0],
        RingSpec {
            motif: Motif::Petal,
            count: 12,
            radius: 0.4,
            scale: 0.2,
            phase: 0.25,
        }
    );
    assert_eq!(rings[1].motif, Motif::Circle);
    assert_eq!(rings[1].scale, DEFAULT_RING_SCALE, "scale defaults");
    assert_eq!(rings[1].phase, 0.0, "phase defaults");
}

/// **Validated once, at the boundary** (the project's rule, and the plan's
/// explicit instruction): an unknown motif and a non-positive count are load
/// errors rather than something the placement arithmetic has to survive.
#[test]
fn a_malformed_ring_is_a_load_error_naming_what_is_wrong() {
    let unknown =
        star("tiling = \"6\"\nrings = [{ motif = \"crescent\", count = 8, radius = 0.5 }]\n")
            .expect_err("an unknown motif must not fall back to one in the roster");
    let msg = unknown.to_string();
    assert!(msg.contains("unknown motif 'crescent'"), "{msg}");
    // The error names the closed roster, because that is the one thing the
    // author needs and cannot get from the file they are editing.
    for m in Motif::ALL {
        assert!(msg.contains(m.name()), "{msg} should list {}", m.name());
    }

    for bad in ["0", "-1", "-4000"] {
        let err = star(&format!(
            "tiling = \"6\"\nrings = [{{ motif = \"circle\", count = {bad}, radius = 0.5 }}]\n"
        ))
        .expect_err("count {bad} must be rejected");
        assert!(err.to_string().contains("count must be"), "{err}");
    }

    let over = star(&format!(
        "tiling = \"6\"\nrings = [{{ motif = \"circle\", count = {}, radius = 0.5 }}]\n",
        MAX_RING_COUNT + 1
    ))
    .expect_err("a count past the ceiling can only buy truncation");
    assert!(over.to_string().contains("count must be"), "{over}");
    assert!(
        star(&format!(
            "tiling = \"6\"\nrings = [{{ motif = \"circle\", count = {MAX_RING_COUNT}, radius = 0.5 }}]\n"
        ))
        .is_ok(),
        "the ceiling itself is legal"
    );

    // A non-finite geometry key would put NaN into the placement, which is a
    // frame of nothing rather than an error anywhere downstream.
    let nan = star("tiling = \"6\"\nrings = [{ motif = \"circle\", count = 8, radius = nan }]\n")
        .expect_err("a non-finite radius must be rejected");
    assert!(nan.to_string().contains("must all be finite"), "{nan}");
}

/// `tiling = "none"` is the rings-only composition — the reference image
/// itself. It is legal only with a roster, because a preset with neither
/// draws nothing at all and that is worth naming at load rather than
/// discovering as a black frame.
#[test]
fn tiling_none_draws_the_ornament_alone_and_needs_one() {
    let p = star("tiling = \"none\"\nrings = [{ motif = \"trefoil\", count = 9, radius = 0.6 }]\n")
        .unwrap();
    match p.config {
        Some(GeneratorConfig::Star { order, .. }) => {
            assert_eq!(order, 0, "no interlace");
        }
        _ => panic!("a star preset carries a Star config"),
    }
    assert_eq!(rings_of(&p).len(), 1);

    let empty = star("tiling = \"none\"\n").expect_err("nothing to draw");
    assert!(empty.to_string().contains("at least one entry in rings"));

    // ...and `none` did not become a wildcard: everything else outside the
    // tiling vocabulary is still rejected.
    assert!(star("tiling = \"nonsense\"\n").is_err());
}
