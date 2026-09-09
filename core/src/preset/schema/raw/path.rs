//! The raw `[path]` table: an authored silhouette, as inline SVG path data.
// A continuation of one module split across several files, so it needs the
// compiled shapes `preset/schema/mod.rs` has in scope.
use super::super::*;

/// The raw `[path]` table (ADR-0107): the silhouette a `shape_field` preset
/// draws instead of one of the five names in the `marks` roster.
///
/// `d` is required — the table exists to carry it. `samples` is the arity the
/// contour is resampled to, and it is the number the per-pixel field costs
/// `O(N)` in.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::preset::schema) struct RawPath {
    pub(in crate::preset::schema) d: String,
    #[serde(default)]
    pub(in crate::preset::schema) samples: Option<usize>,
}

impl RawPath {
    /// Validate the table into the structural config: check the arity against
    /// the ceiling, then parse.
    ///
    /// **The ceiling is checked before the parse, and refuses rather than
    /// decimating.** The cost of an over-large arity is paid on every pixel of
    /// every frame whether or not the figure is on screen, so an author who
    /// pasted a traced logo has to be told — a quietly reduced contour would
    /// render a figure they did not draw and never say so.
    pub(in crate::preset::schema) fn into_config(self) -> Result<GeneratorConfig, PresetError> {
        use crate::preset::path::{DEFAULT_SAMPLES, MAX_SAMPLES, MIN_SAMPLES, PathShape};

        let samples = self.samples.unwrap_or(DEFAULT_SAMPLES);
        if !(MIN_SAMPLES..=MAX_SAMPLES).contains(&samples) {
            return Err(PresetError::Config(format!(
                "[path] samples must be in {MIN_SAMPLES}..={MAX_SAMPLES}, got {samples}. The \
                 field evaluates a distance to every one of these segments at every pixel of \
                 every frame, so the arity is a per-frame cost and not a memory one"
            )));
        }
        let shape = PathShape::parse(&self.d, samples)
            .map_err(|e| PresetError::Config(format!("[path] d {e}")))?;
        Ok(GeneratorConfig::Path { shape })
    }
}
