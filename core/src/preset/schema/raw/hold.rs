//! The raw `[hold]` table: a named musical edge, or a period in seconds. One
//! deserializer accepts both forms, and the number may be written bare or
//! quoted.
// A continuation of one module split across several files, so it needs the
// compiled shapes `preset/schema/mod.rs` has in scope.
use super::super::*;

/// One `[hold]` entry, before validation.
///
/// Hand-deserialized rather than `#[serde(untagged)]` for [`RawSmoothing`]'s
/// reason: an untagged enum reports every failure as "data did not match any
/// variant", which would turn a misspelled `bear` into a message naming
/// neither the word written nor the two that were expected.
///
/// An unrecognized word is carried rather than rejected in the visitor, because
/// serde's own error would name the field and not the surface -- and a
/// `[layer.hold]` entry has to say so.
#[derive(Debug, Clone)]
pub(in crate::preset::schema) enum RawHold {
    /// A recognized edge: one of the two words, or a period from a number.
    /// A period's sign and finiteness are still unchecked here.
    Edge(HoldEdge),
    /// A word that is no edge, kept so the error can quote it.
    Unknown(String),
}

impl RawHold {
    /// The validated edge this entry denotes, or the load error naming why it
    /// is not one. `named` is the surface-prefixed parameter name, so the
    /// message says which entry of which table failed.
    pub(in crate::preset::schema) fn to_edge(&self, named: &str) -> Result<HoldEdge, PresetError> {
        let expected = || {
            HoldEdge::NAMED
                .iter()
                .map(|edge| edge.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        };
        match self {
            // A period of zero re-samples every frame, which is what an unheld
            // binding already does -- so it is a mistake rather than a
            // shorthand, and saying so costs less than an author wondering why
            // the hold did nothing.
            &Self::Edge(HoldEdge::Period(seconds)) if !(seconds.is_finite() && seconds > 0.0) => {
                Err(PresetError::Config(format!(
                    "hold '{named}' period must be a positive, finite number of seconds, \
                     got {seconds}"
                )))
            }
            &Self::Edge(edge) => Ok(edge),
            Self::Unknown(word) => Err(PresetError::Config(format!(
                "hold '{named}' must name an edge or a period in seconds, got '{word}' \
                 (expected one of: {}, or a positive number of seconds)",
                expected()
            ))),
        }
    }
}

impl<'de> Deserialize<'de> for RawHold {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        de.deserialize_any(RawHoldVisitor)
    }
}

pub(in crate::preset::schema) struct RawHoldVisitor;

impl serde::de::Visitor<'_> for RawHoldVisitor {
    type Value = RawHold;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("\"beat\", \"bar\", or a positive number of seconds")
    }

    fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
        // A quoted number is the form ADR-0180 illustrates (`zoom = "2.0"`) and
        // reaches here as a string because TOML quoted it. Parsed before the
        // two words so both spellings of a period mean one thing.
        if let Ok(seconds) = v.parse::<f32>() {
            return Ok(RawHold::Edge(HoldEdge::Period(seconds)));
        }
        Ok(match HoldEdge::from_name(v) {
            Some(edge) => RawHold::Edge(edge),
            None => RawHold::Unknown(v.to_owned()),
        })
    }

    fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Self::Value, E> {
        Ok(RawHold::Edge(HoldEdge::Period(v as f32)))
    }

    // TOML distinguishes `2.0` from `2`, and an author writing a whole number of
    // seconds reaches for the integer.
    fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Self::Value, E> {
        Ok(RawHold::Edge(HoldEdge::Period(v as f32)))
    }

    fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Self::Value, E> {
        Ok(RawHold::Edge(HoldEdge::Period(v as f32)))
    }
}
