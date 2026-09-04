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
            Some("max") => Deposit::Max,
            Some("add") => Deposit::Add,
            Some(name) => {
                return Err(PresetError::Config(format!(
                    "unknown [feedback] blend '{name}' (expected one of: max, add)"
                )));
            }
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
