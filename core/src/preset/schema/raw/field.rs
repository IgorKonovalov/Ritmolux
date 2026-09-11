//! The raw `[field]` table: which closed-form world the analytic field draws.
// A continuation of one module split across several files: `super` is the other
// raw tables, `super::super` the compiled shapes and the loader they validate
// into.
use super::super::*;

/// The raw `[field]` table (ADR-0180 rule 1). An absent table is the default
/// family, the way an absent `[particles]` table is De Jong.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::preset::schema) struct RawField {
    /// Family name (`"chladni"` / `"escape_time"`); validated at load.
    pub(in crate::preset::schema) family: String,
    /// `"julia"` / `"mandelbrot"`: whether `c` is a parameter or the pixel.
    /// Escape time only; absent means `julia`.
    #[serde(default)]
    pub(in crate::preset::schema) map: Option<String>,
}

impl RawField {
    /// Validate the table into a [`FieldConfig`], erroring (never panicking) on
    /// an unknown family — which selects a code path, so a silent default would
    /// render a world the author never asked for.
    pub(in crate::preset::schema) fn into_config(self) -> Result<FieldConfig, PresetError> {
        let family = FieldFamily::from_name(&self.family).ok_or_else(|| {
            PresetError::Config(format!(
                "unknown [field] family '{}' (expected one of: {})",
                self.family,
                FieldFamily::ALL
                    .iter()
                    .map(|f| f.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
        })?;
        // A map on a family that has no orbit is refused rather than ignored:
        // the author asked for something the engine cannot do, and a silent
        // no-op would leave them looking for the Mandelbrot set on a plate.
        let map = match self.map {
            None => EscapeMap::default(),
            Some(name) if family != FieldFamily::EscapeTime => {
                return Err(PresetError::Config(format!(
                    "[field] map = '{name}' is escape_time only; the '{}' family has no \
                     orbit for it to seed",
                    family.as_str()
                )));
            }
            Some(name) => EscapeMap::from_name(&name).ok_or_else(|| {
                PresetError::Config(format!(
                    "unknown [field] map '{name}' (expected one of: {})",
                    EscapeMap::ALL
                        .iter()
                        .map(|m| m.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
            })?,
        };
        Ok(FieldConfig { family, map })
    }
}

// ---------------------------------------------------------------------------
// The schema descriptor (Plan 0158 Phase 4). A second statement of this table's
// shape, and one on purpose: serde carries a field's name and type and none of
// what an editor needs -- the closed roster a string is drawn from, the default
// the loader substitutes, the sentence saying what the key does. What holds the
// two together is `schema::tests`, which reads the field roster serde derived
// and asserts it is exactly what the rows below name.
// ---------------------------------------------------------------------------

/// The `[field]` table.
pub(in crate::preset::schema) const FIELD: TableDesc = TableDesc {
    name: "field",
    doc: "Which closed-form world the analytic field draws.",
    keys: &[
        KeyDesc {
            name: "family",
            kind: KeyKind::Roster(Roster::FieldFamily),
            default: "",
            doc: "The field family. Required inside the table; an absent table is chladni.",
        },
        KeyDesc {
            name: "map",
            kind: KeyKind::Roster(Roster::EscapeMap),
            default: "julia",
            doc: "Escape time only: julia iterates from the pixel with c the constant, \
                  mandelbrot makes the pixel c.",
        },
    ],
};
