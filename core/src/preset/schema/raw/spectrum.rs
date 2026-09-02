//! The raw `[spectrum]` table: how the readout divides the frequency range.
// A continuation of one module split across several files: `super` is the other
// raw tables, `super::super` the compiled shapes and the loader they validate
// into.
use super::super::*;
use super::*;

/// The raw `[spectrum]` table: how the readout divides the frequency axis, what
/// figure the elements form, and how fast each element follows its band.
///
/// Every field is optional; an absent table is the same as an empty one, so
/// `system = "spectrum"` alone renders the default readout.
#[derive(Deserialize, Default)]
pub(in crate::preset::schema) struct RawSpectrum {
    /// Element count; validated into `2..=SPECTRUM_BINS`.
    #[serde(default)]
    pub(in crate::preset::schema) elements: Option<usize>,
    /// Layout name (`bars` / `polyline` / `radial_ring`); validated at load.
    #[serde(default)]
    pub(in crate::preset::schema) layout: Option<String>,
    /// Per-element easing in seconds — a scalar or an `{ attack, release }`
    /// pair, exactly the `[smoothing]` vocabulary (ADR-0035).
    #[serde(default)]
    pub(in crate::preset::schema) smoothing: Option<RawSmoothing>,
}

/// Default element count when a preset does not choose one — inside the "20-30
/// points" range the capability was asked for.
pub(in crate::preset::schema) const DEFAULT_SPECTRUM_ELEMENTS: usize = 24;

impl RawSpectrum {
    /// Validate the table into a [`GeneratorConfig::Spectrum`], erroring (never
    /// panicking) on an out-of-range count, an unknown layout name, or a bad
    /// easing constant — the same load-boundary discipline every other
    /// declarative config follows (ADR-0007).
    pub(in crate::preset::schema) fn into_config(self) -> Result<GeneratorConfig, PresetError> {
        let elements = self.elements.unwrap_or(DEFAULT_SPECTRUM_ELEMENTS);
        // The upper bound is the band count itself: above it the 64 -> N
        // reduction stops being a partition of the array (two elements would
        // have to share a band), and a readout finer than its own data is a lie
        // rather than a feature.
        if !(2..=crate::dsp::SPECTRUM_BINS).contains(&elements) {
            return Err(PresetError::Config(format!(
                "[spectrum] elements must be 2..={}, got {elements}",
                crate::dsp::SPECTRUM_BINS
            )));
        }
        let layout = match self.layout {
            Some(name) => SpectrumLayout::from_name(&name).ok_or_else(|| {
                PresetError::Config(format!(
                    "unknown [spectrum] layout '{name}' (expected one of: {})",
                    SpectrumLayout::NAMES.join(", ")
                ))
            })?,
            None => SpectrumLayout::default(),
        };
        let easing = match self.smoothing {
            Some(RawSmoothing::Symmetric(seconds)) => {
                check_tau("[spectrum] smoothing", None, seconds)?;
                Easing::symmetric(seconds)
            }
            Some(RawSmoothing::Asymmetric { attack, release }) => {
                check_tau("[spectrum] smoothing", Some("attack"), attack)?;
                check_tau("[spectrum] smoothing", Some("release"), release)?;
                Easing { attack, release }
            }
            None => Easing::INSTANT,
        };
        Ok(GeneratorConfig::Spectrum {
            elements,
            layout,
            easing,
        })
    }
}
