//! The raw `[feedback]` and `[occupancy]` tables.
// A continuation of one module split across several files, so it needs the
// compiled shapes `preset/schema/mod.rs` has in scope.
use super::super::*;

/// The `[feedback]` table, before validation (ADR-0048).
///
/// Both keys are optional and both default to the identity, so `[feedback]` with
/// only one of them set is a perfectly good table.
#[derive(Debug, Default, Deserialize)]
pub(in crate::preset::schema) struct RawFeedback {
    /// `none` | `swirl` | `ripple` | `fisheye`.
    #[serde(default)]
    pub(in crate::preset::schema) warp: Option<String>,
    /// `max` | `add`.
    #[serde(default)]
    pub(in crate::preset::schema) blend: Option<String>,
}

impl RawFeedback {
    /// Validate the two closed rosters. An unknown value **rejects the preset**
    /// rather than warning: unlike an unknown *param* name — which ADR-0020 keeps
    /// as a warning so one typo cannot discard a good preset — a structural key
    /// selects a code path, and silently taking the default here would render a
    /// look the author never asked for with nothing on screen to say so. That is
    /// `[curve] family`'s rule, applied to `[curve] family`'s kind of key.
    pub(in crate::preset::schema) fn into_config(self) -> Result<FeedbackConfig, PresetError> {
        let warp = match self.warp.as_deref() {
            None => Warp::default(),
            Some(name) => Warp::from_name(name).ok_or_else(|| {
                PresetError::Config(format!(
                    "unknown [feedback] warp '{name}' (expected one of: {})",
                    Warp::ALL
                        .iter()
                        .map(|w| w.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
            })?,
        };
        let blend = match self.blend.as_deref() {
            None => Deposit::default(),
            Some(name) => Deposit::from_name(name).ok_or_else(|| {
                PresetError::Config(format!(
                    "unknown [feedback] blend '{name}' (expected one of: {})",
                    Deposit::ALL
                        .iter()
                        .map(|d| d.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
            })?,
        };
        Ok(FeedbackConfig { warp, blend })
    }
}

/// The `[occupancy]` table, before validation.
#[derive(Debug, Default, Deserialize)]
pub(in crate::preset::schema) struct RawOccupancy {
    /// Parameter names whose clamps may sit at their bound.
    #[serde(default)]
    pub(in crate::preset::schema) exempt: Vec<String>,
}

// ---------------------------------------------------------------------------
// The schema descriptor (Plan 0158 Phase 4). A second statement of this table's
// shape, and one on purpose: serde carries a field's name and type and none of
// what an editor needs -- the closed roster a string is drawn from, the default
// the loader substitutes, the sentence saying what the key does. What holds the
// two together is `schema::tests`, which reads the field roster serde derived
// and asserts it is exactly what the rows below name.
// ---------------------------------------------------------------------------

/// The `[feedback]` table.
pub(in crate::preset::schema) const FEEDBACK: TableDesc = TableDesc {
    name: "feedback",
    doc: "Which curated warp the accumulation buffers resample their past through, and \
          how this frame's light is deposited onto it.",
    keys: &[
        KeyDesc {
            name: "warp",
            kind: KeyKind::Roster(Roster::Warp),
            default: "none",
            doc: "The warp the accumulation reads its previous frame through.",
        },
        KeyDesc {
            name: "blend",
            kind: KeyKind::Roster(Roster::Deposit),
            default: "max",
            doc: "How this frame is deposited: bounded by the source maximum, or summed.",
        },
    ],
};

/// The `[occupancy]` table.
pub(in crate::preset::schema) const OCCUPANCY: TableDesc = TableDesc {
    name: "occupancy",
    doc: "Parameters whose clamp bounds are meant to pin, exempted from the saturation \
          gate. Harness-only; nothing per-frame reads it.",
    keys: &[KeyDesc {
        name: "exempt",
        kind: KeyKind::List(&KeyKind::Text),
        default: "",
        doc: "Parameter names whose clamps may sit at their bound.",
    }],
};
