//! The raw `[palette]` table: a built-in palette by `name`, or explicit
//! `stops`, with the hex and array colour forms and the stop validation.
// A continuation of one module split across several files, so it needs the
// compiled shapes `preset/schema/mod.rs` has in scope.
use super::super::*;

/// The raw `[palette]` table: **either** a built-in palette `name` **or** custom
/// gradient `stops` (mutually exclusive). Validated at the load boundary.
#[derive(Deserialize)]
pub(in crate::preset::schema) struct RawPalette {
    /// Built-in palette name (e.g. `"ember"`); validated at load.
    #[serde(default)]
    pub(in crate::preset::schema) name: Option<String>,
    /// Custom gradient stops (`{ at = 0.0, color = "#rrggbb" }` or
    /// `{ at = 0.0, color = [r, g, b] }`); validated at load.
    #[serde(default)]
    pub(in crate::preset::schema) stops: Option<Vec<RawStop>>,
}

/// One raw gradient stop: a position `at` in `0..=1` and a color.
#[derive(Deserialize)]
pub(in crate::preset::schema) struct RawStop {
    pub(in crate::preset::schema) at: f32,
    pub(in crate::preset::schema) color: RawColor,
}

/// A raw stop color: a `#rrggbb` hex string or an `[r, g, b]` array of `0..1`
/// floats. `untagged` so either TOML form deserializes.
#[derive(Deserialize)]
#[serde(untagged)]
pub(in crate::preset::schema) enum RawColor {
    /// `"#rrggbb"` (the leading `#` optional).
    Hex(String),
    /// `[r, g, b]` with each channel a `0..1` float.
    Rgb([f32; 3]),
}

impl RawColor {
    /// Validate into an RGB triple, erroring (never panicking) on a malformed hex
    /// string or a non-finite channel.
    pub(in crate::preset::schema) fn into_rgb(self) -> Result<[f32; 3], PresetError> {
        match self {
            RawColor::Hex(s) => parse_hex_color(&s),
            RawColor::Rgb(rgb) => {
                if rgb.iter().any(|c| !c.is_finite()) {
                    return Err(PresetError::Config(format!(
                        "[palette] stop color channels must be finite, got {rgb:?}"
                    )));
                }
                Ok([
                    rgb[0].clamp(0.0, 1.0),
                    rgb[1].clamp(0.0, 1.0),
                    rgb[2].clamp(0.0, 1.0),
                ])
            }
        }
    }
}

/// Parse a `#rrggbb` (or `rrggbb`) hex color into a `0..1` RGB triple. Every
/// failure is a surfaced load error, never a panic.
pub(in crate::preset::schema) fn parse_hex_color(s: &str) -> Result<[f32; 3], PresetError> {
    let hex = s.strip_prefix('#').unwrap_or(s);
    if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(PresetError::Config(format!(
            "[palette] stop color '{s}' must be a #rrggbb hex string"
        )));
    }
    // All six chars are ASCII hex (checked above), so byte-slicing is safe.
    let channel = |lo: usize, hi: usize| -> f32 {
        u8::from_str_radix(&hex[lo..hi], 16)
            .map(|v| v as f32 / 255.0)
            .unwrap_or(0.0)
    };
    Ok([channel(0, 2), channel(2, 4), channel(4, 6)])
}

impl RawPalette {
    /// Validate the table into a [`PaletteConfig`], erroring (never panicking) on
    /// an unknown name, both selectors set, neither set, or a malformed stop list.
    /// `name` and `stops` are **mutually exclusive**: setting both is a load error
    /// (fail fast rather than silently pick one).
    pub(in crate::preset::schema) fn into_config(self) -> Result<PaletteConfig, PresetError> {
        match (self.name, self.stops) {
            (Some(_), Some(_)) => Err(PresetError::Config(
                "[palette] sets both `name` and `stops`; use exactly one".into(),
            )),
            (Some(name), None) => {
                let named = NamedPalette::from_name(&name)
                    .ok_or_else(|| PresetError::Config(format!("unknown palette name '{name}'")))?;
                Ok(PaletteConfig::Named(named))
            }
            (None, Some(stops)) => Ok(PaletteConfig::Custom(validate_stops(stops)?)),
            (None, None) => Err(PresetError::Config(
                "[palette] needs a `name` or `stops`".into(),
            )),
        }
    }
}

/// Validate a custom stop list into the baked-ready `(at, rgb)` pairs: ≥2 stops,
/// each `at` finite in `0..=1` and non-decreasing (sorted), each color parseable.
/// Every failure is a surfaced load error (ADR-0021 / NFR 10).
pub(in crate::preset::schema) fn validate_stops(
    stops: Vec<RawStop>,
) -> Result<Vec<(f32, [f32; 3])>, PresetError> {
    if stops.len() < 2 {
        return Err(PresetError::Config(format!(
            "[palette] needs at least 2 stops, got {}",
            stops.len()
        )));
    }
    let mut out = Vec::with_capacity(stops.len());
    let mut prev_at = f32::NEG_INFINITY;
    for stop in stops {
        if !stop.at.is_finite() || !(0.0..=1.0).contains(&stop.at) {
            return Err(PresetError::Config(format!(
                "[palette] stop `at` must be in 0..=1, got {}",
                stop.at
            )));
        }
        if stop.at < prev_at {
            return Err(PresetError::Config(
                "[palette] stops must be sorted by ascending `at`".into(),
            ));
        }
        prev_at = stop.at;
        out.push((stop.at, stop.color.into_rgb()?));
    }
    Ok(out)
}
