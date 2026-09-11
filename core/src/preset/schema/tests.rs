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

/// A `scallop` reads its ring `scale` as the lobe's **depth**, and a negative
/// one does not make a shallower dimple — past
/// `depth = -R * (cos(s) + sin(s) - 1)` the lobe inverts and bulges outward to
/// roughly twice the ring radius. It is a well-formed arc inside the cap, so
/// nothing downstream can notice: no panic, no overflow, no warning.
///
/// **Refused at load rather than drawn.** Whether an inward scallop is a look
/// anyone wants is a content question, and the refusal is what makes it come
/// back as one instead of shipping a silent bulge (design-backlog 0136).
///
/// The bound is on the **structural** per-ring `scale`, which is otherwise
/// validated for finiteness alone. It is a different quantity from the bindable
/// `ring_scale` multiplier, which is clamped, and the clamp on that one does not
/// reach this one — that conflation is the trap this test pins.
#[test]
fn a_scallop_refuses_a_depth_it_cannot_draw() {
    let err = star(
        "tiling = \"6\"\nrings = [{ motif = \"scallop\", count = 8, radius = 0.5, scale = -0.2 }]\n",
    )
    .expect_err("a negative scallop depth inverts the lobe rather than shallowing it");
    let msg = err.to_string();
    assert!(msg.contains("ring 0"), "the error names the ring: {msg}");
    assert!(
        msg.contains("scallop") && msg.contains("depth"),
        "the error names the motif and what the number means: {msg}"
    );

    // Zero is the flat case `scallop_lobe` explicitly guards, and it loads.
    assert!(
        star("tiling = \"6\"\nrings = [{ motif = \"scallop\", count = 8, radius = 0.5, scale = 0.0 }]\n")
            .is_ok(),
        "a flat scallop is degenerate, not invalid"
    );

    // The bound is the scallop's alone: every other motif reads `scale` as a
    // size multiplier, where a negative value is a reflection and reachable.
    assert!(
        star("tiling = \"6\"\nrings = [{ motif = \"petal\", count = 8, radius = 0.5, scale = -0.2 }]\n")
            .is_ok(),
        "the refusal must not spread to motifs whose scale is a multiplier"
    );
}

/// **Validated once, at the boundary** (the project's rule): an unknown
/// motif and a non-positive count are load errors rather than something
/// the placement arithmetic has to survive.
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

/// The roster is one table and everything else reads it, so the three
/// projections cannot disagree — this is what asserts they do not.
///
/// **Written against the array's own length, not against a literal 12.**
/// `SystemKind::ALL` is typed `[SystemKind; VARIANT_COUNT]` and built from
/// `TABLE`, which is typed the same way, so a variant added to the enum without
/// a `TABLE` row fails to compile at `SystemKind::row` — the one exhaustive
/// match over the enum — before it can reach this test.
#[test]
fn every_system_round_trips_through_its_one_roster() {
    assert_eq!(
        SystemKind::ALL.len(),
        SystemKind::VARIANT_COUNT,
        "the roster's length is its type; this can only fail if the type changed"
    );
    let mut seen: Vec<&str> = Vec::new();
    for kind in SystemKind::ALL {
        let name = kind.as_str();
        assert_eq!(
            SystemKind::from_name(name),
            Some(kind),
            "{name} does not round-trip as_str -> from_name"
        );
        assert!(
            !kind.param_names().is_empty(),
            "{name} declares no parameters, so the loader's typo check (ADR-0020) \
             would reject every binding a preset writes for it"
        );
        assert!(
            !seen.contains(&name),
            "{name} appears in the roster twice, so one variant is carrying \
             another's name and params"
        );
        seen.push(name);
    }
}

fn shape_field_path(table: &str) -> Result<Preset, PresetError> {
    Preset::from_toml_str(&format!(
        "system = \"shape_field\"\nname = \"t\"\n[path]\n{table}"
    ))
}

/// The `[path]` table is optional, and a `shape_field` preset without one
/// carries an **empty** contour rather than no config at all — the config is
/// handed over on every preset switch precisely so `configure` runs and clears
/// the outgoing preset's silhouette (ADR-0107). An authored silhouette is an
/// alternative source of a figure, not a replacement for the `marks` roster.
#[test]
fn a_shape_field_preset_without_a_path_table_carries_an_empty_contour() {
    let preset = Preset::from_toml_str("system = \"shape_field\"\nname = \"t\"\n")
        .expect("a shape_field preset needs no table");
    match preset.config {
        Some(GeneratorConfig::Path { shape: None, .. }) => {}
        other => panic!("expected an empty Path config, got {other:?}"),
    }
}

#[test]
fn a_path_table_becomes_a_contour_at_the_arity_it_asked_for() {
    let preset =
        shape_field_path("d = \"M 0,0 H 10 V 10 H 0 Z\"\nsamples = 24").expect("a square parses");
    match preset.config {
        Some(GeneratorConfig::Path {
            shape: Some(shape), ..
        }) => {
            assert_eq!(shape.points().len(), 24)
        }
        other => panic!("a [path] preset carries a Path config, got {other:?}"),
    }

    // No `samples` key takes the default arity rather than the flatten's own.
    let defaulted = shape_field_path("d = \"M 0,0 H 10 V 10 H 0 Z\"").expect("parses");
    match defaulted.config {
        Some(GeneratorConfig::Path {
            shape: Some(shape), ..
        }) => {
            assert_eq!(shape.points().len(), crate::preset::path::DEFAULT_SAMPLES);
        }
        other => panic!("expected a Path config, got {other:?}"),
    }
}

/// **An arity outside the ceiling is a load error naming both numbers**, not a
/// silent decimation: the cost is paid on every pixel of every frame whether or
/// not the figure is on screen, so an author who pasted a traced logo is told.
#[test]
fn an_arity_outside_the_ceiling_is_refused_naming_the_ceiling_and_the_count() {
    use crate::preset::path::{MAX_SAMPLES, MIN_SAMPLES};
    let over = MAX_SAMPLES + 1;
    let err = shape_field_path(&format!("d = \"M 0,0 H 1 V 1 H 0 Z\"\nsamples = {over}"))
        .expect_err("over the ceiling");
    let text = err.to_string();
    assert!(
        text.contains(&MAX_SAMPLES.to_string()),
        "names the ceiling: {text}"
    );
    assert!(
        text.contains(&over.to_string()),
        "names the count asked for: {text}"
    );

    // The floor is refused on the same terms — a two-point contour is not a
    // silhouette.
    let under = MIN_SAMPLES - 1;
    let err = shape_field_path(&format!("d = \"M 0,0 H 1 V 1 H 0 Z\"\nsamples = {under}"))
        .expect_err("under the floor");
    assert!(err.to_string().contains(&MIN_SAMPLES.to_string()));
}

/// A malformed path reaches the author as a load error carrying its character
/// offset, rather than as a fallback shape (ADR-0107).
#[test]
fn a_malformed_path_surfaces_its_offset_through_the_load_error() {
    let err = shape_field_path("d = \"M 0,0 A 1 1 0 0 1 2,2 Z\"").expect_err("arcs are refused");
    let text = err.to_string();
    assert!(
        text.contains("[path] d"),
        "the message names the key: {text}"
    );
    assert!(
        text.contains("at character 6"),
        "and carries the offset: {text}"
    );
    assert!(text.contains("elliptical arc"), "and what it found: {text}");
}

/// An unknown key in `[path]` is a load error rather than a silent typo — the
/// table is `deny_unknown_fields`, which is what makes `sample = 32` say so.
#[test]
fn an_unknown_path_key_is_refused() {
    let err =
        shape_field_path("d = \"M 0,0 H 1 V 1 H 0 Z\"\nsample = 32").expect_err("a mistyped key");
    assert!(err.to_string().contains("sample"), "{err}");
}

/// **`morph_to` is parsed at the same arity and aligned to `d` at load**
/// (ADR-0107): the pair meets at one arity because one `samples` key sizes both,
/// and the `O(N^2)` start-point search is a fact about the pair rather than
/// about the frame.
#[test]
fn a_morph_target_is_parsed_at_the_same_arity_and_aligned_at_load() {
    let preset = shape_field_path(
        "d = \"M -1,-1 L 1,-1 L 1,1 L -1,1 Z\"\n\
         morph_to = \"M 0,-1 L 0.866,0.5 L -0.866,0.5 Z\"\n\
         samples = 24",
    )
    .expect("a morph pair parses");
    match preset.config {
        Some(GeneratorConfig::Path {
            shape: Some(shape),
            morph_to: Some(target),
        }) => {
            assert_eq!(shape.points().len(), 24);
            assert_eq!(
                target.points().len(),
                24,
                "both endpoints meet at one arity"
            );
            assert!(
                shape.signed_area() * target.signed_area() > 0.0,
                "the aligned target must share the source's winding, got {} and {}",
                shape.signed_area(),
                target.signed_area()
            );
        }
        other => panic!("expected an aligned morph pair, got {other:?}"),
    }

    // An absent `morph_to` pins the figure, which is what makes `morph` inert on
    // a preset that mentions neither.
    let pinned = shape_field_path("d = \"M -1,-1 L 1,-1 L 1,1 L -1,1 Z\"").expect("parses");
    match pinned.config {
        Some(GeneratorConfig::Path { morph_to: None, .. }) => {}
        other => panic!("expected no morph target, got {other:?}"),
    }
}

/// A malformed `morph_to` names **which** of the two silhouettes was wrong. Both
/// are `d` strings and both can be pasted wrong, so an error that said only
/// "[path] d" would send an author to the other one.
#[test]
fn a_malformed_morph_target_names_the_key_it_came_from() {
    let err = shape_field_path(
        "d = \"M -1,-1 L 1,-1 L 1,1 L -1,1 Z\"\nmorph_to = \"M 0,0 A 1 1 0 0 1 2,2 Z\"",
    )
    .expect_err("arcs are refused in the target too");
    let text = err.to_string();
    assert!(
        text.contains("[path] morph_to"),
        "the message names the target rather than the source: {text}"
    );
    assert!(text.contains("elliptical arc"), "{text}");
}

// ---------------------------------------------------------------------------
// The exported schema (Plan 0158 Phase 4): the descriptors against serde, the
// rosters against their own parsers, and both against every preset that ships.
// ---------------------------------------------------------------------------

/// The one error the probe below produces. Carries nothing: it is a control-flow
/// signal, not a diagnosis.
#[derive(Debug)]
struct Probed;

impl fmt::Display for Probed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("field probe")
    }
}

impl std::error::Error for Probed {}

impl serde::de::Error for Probed {
    fn custom<T: fmt::Display>(_: T) -> Self {
        Probed
    }
}

/// A `Deserializer` that answers nothing and records the field roster serde
/// asked it for.
///
/// **This is the reflection Rust does not otherwise have.** `#[derive(Deserialize)]`
/// emits a call to `deserialize_struct` carrying the exact list of fields it
/// knows about; nothing else in the language reports that list. Capturing it is
/// what makes "a key added to a serde struct without a descriptor row fails the
/// test" a fact rather than a hope — a hand-written roster here would need the
/// same edit the descriptor needs, and would fail to catch exactly the omission
/// it was written for.
struct FieldProbe<'a>(&'a mut Option<&'static [&'static str]>);

impl<'de> serde::Deserializer<'de> for FieldProbe<'_> {
    type Error = Probed;

    fn deserialize_struct<V: serde::de::Visitor<'de>>(
        self,
        _name: &'static str,
        fields: &'static [&'static str],
        _visitor: V,
    ) -> Result<V::Value, Probed> {
        *self.0 = Some(fields);
        Err(Probed)
    }

    fn deserialize_any<V: serde::de::Visitor<'de>>(self, _visitor: V) -> Result<V::Value, Probed> {
        Err(Probed)
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map enum identifier ignored_any
    }
}

/// The field names serde derived for `T`.
fn serde_fields<'de, T: Deserialize<'de>>() -> &'static [&'static str] {
    let mut captured = None;
    let _ = T::deserialize(FieldProbe(&mut captured));
    captured.expect("the derived Deserialize asks deserialize_struct for its fields")
}

/// One table's descriptor beside the field roster serde derived for it.
type DescriptorPair = (&'static TableDesc, &'static [&'static str]);

/// Every raw table, paired with the descriptor that is supposed to describe it.
///
/// A function rather than a const because each entry has to name a *type*, which
/// only a call can do. Adding a table means adding a row here; a table with no
/// row is invisible to the check below, which is the one hole this shape has and
/// the reason `every_descriptor_is_reachable_from_the_root` exists beside it.
fn descriptor_pairs() -> Vec<DescriptorPair> {
    vec![
        (&raw::PRESET, serde_fields::<raw::RawPreset>()),
        (&raw::LAYER, serde_fields::<raw::RawLayer>()),
        (&raw::LATCH, serde_fields::<raw::RawLatch>()),
        (&raw::CURVE, serde_fields::<raw::RawCurve>()),
        (&raw::GENERATOR, serde_fields::<raw::RawGenerator>()),
        (&raw::RING, serde_fields::<raw::RawRing>()),
        (&raw::PARTICLES, serde_fields::<raw::RawParticles>()),
        (&raw::PATH, serde_fields::<raw::RawPath>()),
        (&raw::SPECTRUM, serde_fields::<raw::RawSpectrum>()),
        (&raw::MESH, serde_fields::<raw::RawMesh>()),
        (&raw::FIELD, serde_fields::<raw::RawField>()),
        (&raw::MILK, serde_fields::<raw::RawMilk>()),
        (&raw::MILK_ELEMENT, serde_fields::<raw::RawMilkElement>()),
        (&raw::FEEDBACK, serde_fields::<raw::RawFeedback>()),
        (&raw::OCCUPANCY, serde_fields::<raw::RawOccupancy>()),
        (&raw::PALETTE, serde_fields::<raw::RawPalette>()),
        (&raw::STOP, serde_fields::<raw::RawStop>()),
    ]
}

/// Every key the loader deserializes has a descriptor row, and every row names a
/// key the loader deserializes.
///
/// Both directions, because they fail differently: a field with no row is a key
/// an editor cannot offer, and a row with no field is a key an editor would
/// offer and the loader would ignore.
#[test]
fn every_serde_field_has_a_descriptor_row_and_the_reverse() {
    let mut findings = Vec::new();
    for (table, fields) in descriptor_pairs() {
        let described: Vec<&str> = table.keys.iter().map(|key| key.name).collect();
        for field in fields {
            if !described.contains(field) {
                findings.push(format!(
                    "  [{}] {field}: deserialized by the loader, named by no descriptor row",
                    table.name
                ));
            }
        }
        for key in &described {
            if !fields.contains(key) {
                findings.push(format!(
                    "  [{}] {key}: named by a descriptor row, deserialized by nothing",
                    table.name
                ));
            }
        }
    }
    assert!(
        findings.is_empty(),
        "{} key(s) disagree between serde and the schema descriptors:\n{}\n\
         The rosters here are read off the derived Deserialize itself, so a field \
         added to a Raw* struct lands in this list until a row joins it.",
        findings.len(),
        findings.join("\n"),
    );
}

/// Every descriptor in the export's roster is reachable from the root, and every
/// table a key refers to exists.
///
/// The hole the pairing above cannot see: a table with no row in
/// `descriptor_pairs` is unchecked, and a table nothing refers to is
/// unreachable. Walking the references from `preset` closes both.
#[test]
fn every_descriptor_is_reachable_from_the_root_and_every_reference_resolves() {
    let mut reached = vec!["preset"];
    let mut frontier = vec![&raw::PRESET];
    while let Some(table) = frontier.pop() {
        for key in table.keys {
            let mut kind = &key.kind;
            // A `map` or `list` of a table refers through one level of wrapper.
            while let KeyKind::Map(of) | KeyKind::List(of) = kind {
                kind = of;
            }
            let KeyKind::Table(name) = kind else {
                continue;
            };
            let target = export::table(name).unwrap_or_else(|| {
                panic!(
                    "[{}] {} refers to unknown table `{name}`",
                    table.name, key.name
                )
            });
            if !reached.contains(name) {
                reached.push(name);
                frontier.push(target);
            }
        }
    }
    let unreachable: Vec<&str> = export::TABLES
        .iter()
        .map(|table| table.name)
        .filter(|name| !reached.contains(name))
        .collect();
    assert!(
        unreachable.is_empty(),
        "these tables are exported but no key refers to them, so nothing can \
         reach them from a preset document: {unreachable:?}"
    );
    assert_eq!(
        reached.len(),
        descriptor_pairs().len(),
        "every reachable table needs a serde pairing above; reached {reached:?}"
    );
}

/// Every value a roster publishes is one the type's own parser accepts.
///
/// The export renders these rosters into the document a studio builds its
/// dropdowns from, so a value here that the loader rejects would be an option
/// that produces a load error when chosen.
#[test]
fn every_roster_value_parses_through_its_owners_parser() {
    use crate::render::scenes::lines::star::Motif;
    use crate::render::scenes::lines::{CurveFamily, SpectrumLayout, hankin};
    use crate::render::scenes::particles::AttractorFamily;
    use crate::render::scenes::particles::ifs::IfsFigure;

    /// A roster beside the parser that owns it.
    type RosterCheck = (Roster, fn(&str) -> bool);

    let checks: [RosterCheck; 14] = [
        (Roster::System, |n| SystemKind::from_name(n).is_some()),
        (Roster::CurveFamily, |n| CurveFamily::from_name(n).is_some()),
        (Roster::AttractorFamily, |n| {
            AttractorFamily::from_name(n).is_some()
        }),
        (Roster::IfsFigure, |n| IfsFigure::from_name(n).is_some()),
        (Roster::SpectrumLayout, |n| {
            SpectrumLayout::from_name(n).is_some()
        }),
        (Roster::Warp, |n| Warp::from_name(n).is_some()),
        (Roster::Deposit, |n| Deposit::from_name(n).is_some()),
        (Roster::Palette, |n| NamedPalette::from_name(n).is_some()),
        (Roster::LayerJoin, |n| LayerJoin::from_name(n).is_some()),
        (Roster::LayerBlend, |n| LayerBlend::from_name(n).is_some()),
        (Roster::Motif, |n| Motif::from_name(n).is_some()),
        // `none` is the loader's own special case — it draws no interlace, so it
        // never reaches `tiling_order`. Stated here rather than pushed into that
        // function, whose answer is an order and has none to give.
        (Roster::Tiling, |n| {
            n == "none" || hankin::tiling_order(n).is_some()
        }),
        (Roster::FieldFamily, |n| FieldFamily::from_name(n).is_some()),
        (Roster::EscapeMap, |n| EscapeMap::from_name(n).is_some()),
    ];

    for (roster, parses) in checks {
        let values = roster.values();
        assert!(
            !values.is_empty(),
            "{roster:?} published an empty roster, so a dropdown built from it \
             would offer nothing"
        );
        for value in values {
            assert!(
                parses(value),
                "{roster:?} publishes `{value}`, which its own parser rejects"
            );
        }
    }
}

/// Every key every shipped preset writes is a key the descriptor names.
///
/// The direction that catches a table an author reached for and the export never
/// heard of. Walks the embedded set and the teaching presets under
/// `docs/examples/` — the two populations that are *known* to load — against the
/// descriptor tree, following table references exactly as an editor would.
#[test]
fn every_key_a_shipped_preset_writes_is_one_the_descriptor_names() {
    let mut findings = Vec::new();
    for (file, source) in crate::preset::EMBEDDED {
        check_document(file, source, &mut findings);
    }
    let examples = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("core has a workspace-root parent")
        .join("docs/examples");
    if let Ok(entries) = std::fs::read_dir(&examples) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "toml")
                && let Ok(source) = std::fs::read_to_string(&path)
            {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                check_document(&name, &source, &mut findings);
            }
        }
    }
    assert!(
        findings.is_empty(),
        "{} key(s) in shipped presets are named by no descriptor row:\n{}",
        findings.len(),
        findings.join("\n"),
    );
}

/// Walk one preset document against the root descriptor, collecting keys nothing
/// describes.
fn check_document(file: &str, source: &str, findings: &mut Vec<String>) {
    let Ok(value) = toml::from_str::<toml::Value>(source) else {
        // A file that does not parse as TOML is another suite's problem; this one
        // is about which keys exist.
        return;
    };
    check_table(file, "", &value, &raw::PRESET, findings);
}

/// One table's keys against `desc`, recursing through table references.
fn check_table(
    file: &str,
    path: &str,
    value: &toml::Value,
    desc: &TableDesc,
    findings: &mut Vec<String>,
) {
    let Some(table) = value.as_table() else {
        return;
    };
    for (key, child) in table {
        let Some(described) = desc.keys.iter().find(|k| k.name == key) else {
            findings.push(format!("  {file}: {path}{key} (in table `{}`)", desc.name));
            continue;
        };
        let here = format!("{path}{key}.");
        match described.kind {
            // A map's own keys are the author's to choose; its *values* are
            // checked when they are tables.
            KeyKind::Map(of) => {
                if let KeyKind::Table(name) = of
                    && let Some(target) = export::table(name)
                    && let Some(entries) = child.as_table()
                {
                    for (entry, entry_value) in entries {
                        check_table(
                            file,
                            &format!("{here}{entry}."),
                            entry_value,
                            target,
                            findings,
                        );
                    }
                }
            }
            KeyKind::List(of) => {
                if let KeyKind::Table(name) = of
                    && let Some(target) = export::table(name)
                    && let Some(items) = child.as_array()
                {
                    for (i, item) in items.iter().enumerate() {
                        check_table(file, &format!("{here}{i}."), item, target, findings);
                    }
                }
            }
            KeyKind::Table(name) => {
                if let Some(target) = export::table(name) {
                    check_table(file, &here, child, target, findings);
                }
            }
            _ => {}
        }
    }
}
