//! The raw `[smoothing]` table: today's scalar, or the attack/release map
//! ADR-0035 widened it to. One deserializer accepts both forms.
// A continuation of one module split across several files, so it needs the
// compiled shapes `preset/schema/mod.rs` has in scope.
use super::super::*;

/// One `[smoothing]` entry, before validation: today's scalar, or ADR-0035's
/// inline `{ attack = <seconds>, release = <seconds> }` pair.
///
/// Hand-deserialized rather than `#[serde(untagged)]` because an untagged enum
/// reports every failure as "data did not match any variant", which would make a
/// mistyped table strictly harder to diagnose than a mistyped float — the exact
/// regression ADR-0035 says not to ship.
#[derive(Debug, Clone, Copy)]
pub(in crate::preset::schema) enum RawSmoothing {
    /// `hue = 0.4` — one constant in both directions.
    Symmetric(f32),
    /// `burst = { attack = 0.02, release = 0.7 }`.
    Asymmetric { attack: f32, release: f32 },
}

impl RawSmoothing {
    /// The validated pair this entry denotes. A scalar means both sides.
    pub(in crate::preset::schema) fn to_easing(self) -> Easing {
        match self {
            Self::Symmetric(tau) => Easing::symmetric(tau),
            Self::Asymmetric { attack, release } => Easing { attack, release },
        }
    }
}

impl<'de> Deserialize<'de> for RawSmoothing {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        de.deserialize_any(RawSmoothingVisitor)
    }
}

pub(in crate::preset::schema) struct RawSmoothingVisitor;

impl<'de> serde::de::Visitor<'de> for RawSmoothingVisitor {
    type Value = RawSmoothing;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a number of seconds, or a table { attack = <seconds>, release = <seconds> }")
    }

    fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Self::Value, E> {
        Ok(RawSmoothing::Symmetric(v as f32))
    }

    // TOML distinguishes `0.4` from `0`, and an author writing an instant
    // constant reaches for the integer.
    fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Self::Value, E> {
        Ok(RawSmoothing::Symmetric(v as f32))
    }

    fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Self::Value, E> {
        Ok(RawSmoothing::Symmetric(v as f32))
    }

    fn visit_map<A: serde::de::MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        use serde::de::Error;
        let mut attack = None;
        let mut release = None;
        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "attack" if attack.is_some() => return Err(A::Error::duplicate_field("attack")),
                "release" if release.is_some() => return Err(A::Error::duplicate_field("release")),
                "attack" => attack = Some(map.next_value::<f32>()?),
                "release" => release = Some(map.next_value::<f32>()?),
                // Naming both expected keys is the whole point: `atack = 0.02`
                // must not silently become an entry with a default attack.
                other => return Err(A::Error::unknown_field(other, &["attack", "release"])),
            }
        }
        match (attack, release) {
            (Some(attack), Some(release)) => Ok(RawSmoothing::Asymmetric { attack, release }),
            // Half a pair is a mistake, not a shorthand: silently defaulting the
            // missing side to instant would give the opposite of the requested
            // envelope on that direction.
            (None, _) => Err(A::Error::missing_field("attack")),
            (_, None) => Err(A::Error::missing_field("release")),
        }
    }
}

/// One easing constant, validated at the load boundary. `side` names which half
/// of an `{ attack, release }` pair failed; `None` is the scalar form, whose
/// message is unchanged from ADR-0019.
pub(in crate::preset::schema) fn check_tau(
    param: &str,
    side: Option<&str>,
    seconds: f32,
) -> Result<(), PresetError> {
    if seconds.is_finite() && seconds >= 0.0 {
        return Ok(());
    }
    Err(PresetError::Config(match side {
        Some(side) => format!(
            "smoothing '{param}' {side} must be a non-negative number of seconds, got {seconds}"
        ),
        None => {
            format!("smoothing '{param}' must be a non-negative number of seconds, got {seconds}")
        }
    }))
}
