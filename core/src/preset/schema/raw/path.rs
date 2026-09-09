//! The raw `[path]` table: an authored silhouette, as inline SVG path data.
// A continuation of one module split across several files, so it needs the
// compiled shapes `preset/schema/mod.rs` has in scope.
use super::super::*;
use crate::preset::path::PathShape;

/// The raw `[path]` table (ADR-0107): the silhouette a `shape_field` preset
/// draws instead of one of the five names in the `marks` roster.
///
/// `d` is required — the table exists to carry it. `morph_to` is the second
/// silhouette the bindable `morph` param travels towards; absent, `morph` is
/// inert. `samples` is the arity **both** are resampled to, and it is the number
/// the per-pixel field costs `O(N)` in.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::preset::schema) struct RawPath {
    pub(in crate::preset::schema) d: String,
    #[serde(default)]
    pub(in crate::preset::schema) morph_to: Option<String>,
    #[serde(default)]
    pub(in crate::preset::schema) samples: Option<usize>,
}

/// A parsed `[path]` table: the silhouette, and the one it morphs towards.
pub(in crate::preset::schema) struct ParsedPath {
    pub(in crate::preset::schema) shape: PathShape,
    pub(in crate::preset::schema) morph_to: Option<PathShape>,
}

impl RawPath {
    /// Validate the table into its contours: check the arity against the
    /// ceiling, parse both silhouettes at it, then align the second to the first.
    ///
    /// **The ceiling is checked before the parse, and refuses rather than
    /// decimating.** The cost of an over-large arity is paid on every pixel of
    /// every frame whether or not the figure is on screen, so an author who
    /// pasted a traced logo has to be told — a quietly reduced contour would
    /// render a figure they did not draw and never say so.
    ///
    /// **Alignment happens here, once, at load.** It is `O(N^2)` and it is a
    /// fact about the pair rather than about the frame, so nothing per-frame
    /// re-derives it — the render layer only interpolates the two point lists it
    /// is handed.
    pub(in crate::preset::schema) fn into_parsed(self) -> Result<ParsedPath, PresetError> {
        use crate::preset::path::{DEFAULT_SAMPLES, MAX_SAMPLES, MIN_SAMPLES};

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
        let morph_to = match self.morph_to {
            Some(d) => {
                let target = PathShape::parse(&d, samples)
                    .map_err(|e| PresetError::Config(format!("[path] morph_to {e}")))?;
                Some(target.aligned_to(&shape).ok_or_else(|| {
                    PresetError::Config(
                        "[path] morph_to could not be aligned to d, which needs both to be \
                         non-degenerate contours of the same arity"
                            .into(),
                    )
                })?)
            }
            None => None,
        };
        Ok(ParsedPath { shape, morph_to })
    }
}
