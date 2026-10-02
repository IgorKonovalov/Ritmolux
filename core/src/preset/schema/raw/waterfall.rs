//! The raw `[waterfall]` table: how many bands a row holds, how many rows the
//! landscape keeps, and how often a row is pushed.
// A continuation of one module split across several files: `super` is the other
// raw tables, `super::super` the compiled shapes and the loader they validate
// into.
use super::super::*;
use super::*;

/// The raw `[waterfall]` table (ADR-0180 rule 1). Every field is optional; an
/// absent table is the same as an empty one, so `system = "waterfall"` alone
/// renders the default landscape.
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::preset::schema) struct RawWaterfall {
    /// Bands across a row; validated into `2..=SPECTRUM_BINS`.
    #[serde(default)]
    pub(in crate::preset::schema) elements: Option<usize>,
    /// Rows the landscape draws, the live front row included; validated into
    /// [`MIN_ROWS`](crate::render::scenes::lines::waterfall::MIN_ROWS)`..=`[`MAX_ROWS`](crate::render::scenes::lines::waterfall::MAX_ROWS).
    #[serde(default)]
    pub(in crate::preset::schema) rows: Option<u32>,
    /// Seconds between two pushed rows.
    #[serde(default)]
    pub(in crate::preset::schema) row_period: Option<f32>,
    /// Per-band easing in seconds, in the `[smoothing]` table's vocabulary
    /// (ADR-0035), as on `[spectrum]`.
    #[serde(default)]
    pub(in crate::preset::schema) smoothing: Option<RawSmoothing>,
}

impl RawWaterfall {
    /// Validate the table into a [`GeneratorConfig::Waterfall`], erroring (never
    /// panicking) on an out-of-range count or period, or a bad easing constant.
    pub(in crate::preset::schema) fn into_config(self) -> Result<GeneratorConfig, PresetError> {
        use crate::render::scenes::lines::waterfall::{
            DEFAULT_ELEMENTS, DEFAULT_ROW_PERIOD, DEFAULT_ROWS, MAX_ROW_PERIOD, MAX_ROWS,
            MIN_ROW_PERIOD, MIN_ROWS, WaterfallConfig,
        };
        let elements = self.elements.unwrap_or(DEFAULT_ELEMENTS);
        // The `[spectrum]` bound, for its reason: above the band count the
        // 64 -> N reduction stops being a partition of the array.
        if !(2..=crate::dsp::SPECTRUM_BINS).contains(&elements) {
            return Err(PresetError::Config(format!(
                "[waterfall] elements must be 2..={}, got {elements}",
                crate::dsp::SPECTRUM_BINS
            )));
        }
        let rows = self.rows.unwrap_or(DEFAULT_ROWS);
        if !(MIN_ROWS..=MAX_ROWS).contains(&rows) {
            return Err(PresetError::Config(format!(
                "[waterfall] rows must be {MIN_ROWS}..={MAX_ROWS}, got {rows}"
            )));
        }
        let row_period = self.row_period.unwrap_or(DEFAULT_ROW_PERIOD);
        // `contains` is false for NaN, so a non-finite period is refused here too.
        if !(MIN_ROW_PERIOD..=MAX_ROW_PERIOD).contains(&row_period) {
            return Err(PresetError::Config(format!(
                "[waterfall] row_period must be {MIN_ROW_PERIOD}..={MAX_ROW_PERIOD} seconds, \
                 got {row_period}"
            )));
        }
        let easing = match self.smoothing {
            Some(RawSmoothing::Symmetric(seconds)) => {
                check_tau("[waterfall] smoothing", None, seconds)?;
                Easing::symmetric(seconds)
            }
            Some(RawSmoothing::Asymmetric { attack, release }) => {
                check_tau("[waterfall] smoothing", Some("attack"), attack)?;
                check_tau("[waterfall] smoothing", Some("release"), release)?;
                Easing { attack, release }
            }
            None => Easing::INSTANT,
        };
        Ok(GeneratorConfig::Waterfall(WaterfallConfig {
            elements,
            rows,
            row_period,
            easing,
        }))
    }
}

// ---------------------------------------------------------------------------
// The schema descriptor: the second statement of this table's shape that an
// editor reads, held to the serde one by `schema::tests`.
// ---------------------------------------------------------------------------

/// The `[waterfall]` table.
pub(in crate::preset::schema) const WATERFALL: TableDesc = TableDesc {
    name: "waterfall",
    doc: "How many bands a row of the landscape holds, how many rows it keeps, how often a \
          row is pushed, and how fast each band follows its level.",
    keys: &[
        KeyDesc {
            name: "elements",
            kind: KeyKind::Int,
            default: "48",
            doc: "Bands across a row. A row finer than the band array is refused.",
        },
        KeyDesc {
            name: "rows",
            kind: KeyKind::Int,
            default: "64",
            doc: "Rows the landscape draws, the live front row included; the quality tier may \
                  draw fewer, and says so.",
        },
        KeyDesc {
            name: "row_period",
            kind: KeyKind::Float,
            default: "0.04",
            doc: "Seconds between two rows, so rows times row_period is how much of the music \
                  the landscape shows.",
        },
        KeyDesc {
            name: "smoothing",
            kind: KeyKind::Easing,
            default: "0",
            doc: "Per-band easing in seconds, in the [smoothing] table's own vocabulary.",
        },
    ],
};
