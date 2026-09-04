//! The raw `[milk]` table: a converted MilkDrop preset's programs and its
//! per-element wave and shape rosters (ADR-0113).
// A continuation of one module split across several files, so it needs the
// compiled shapes `preset/schema/mod.rs` has in scope.
use super::super::*;

/// The raw `[milk]` table (Plan 0100 Phase 2): a converted MilkDrop preset's
/// three EEL2 programs, as the assembly text `milkconv` emits — plus, from Phase
/// 4, its custom waves and shapes as `[[milk.waves]]` / `[[milk.shapes]]`.
///
/// Every section is optional. An absent one is the empty program, which is what
/// a `.milk` file with no `per_frame_init` block converts to.
#[derive(Debug, Default, Deserialize)]
pub(in crate::preset::schema) struct RawMilk {
    #[serde(default)]
    pub(in crate::preset::schema) per_frame_init: Option<String>,
    #[serde(default)]
    pub(in crate::preset::schema) per_frame: Option<String>,
    #[serde(default)]
    pub(in crate::preset::schema) per_vertex: Option<String>,
    /// Up to four custom waves — **47 % of the corpus enables at least one.**
    #[serde(default)]
    pub(in crate::preset::schema) waves: Vec<RawMilkElement>,
    /// Up to four custom shapes — **63 % of the corpus enables at least one.**
    #[serde(default)]
    pub(in crate::preset::schema) shapes: Vec<RawMilkElement>,
    /// The translated `warp` shader, as a complete WGSL fragment module (Plan
    /// 0100 Phase 6). Compiled through naga at load: **a failed compile rejects
    /// this preset by name and the loader's per-file skip loads the rest.**
    #[serde(default)]
    pub(in crate::preset::schema) warp_shader: Option<String>,
    /// The translated `comp` shader — same contract as `warp_shader`.
    #[serde(default)]
    pub(in crate::preset::schema) comp_shader: Option<String>,
    /// The deepest blur level either shader samples, `0..=3`. Decides whether
    /// the scene runs its blur chain at all.
    #[serde(default)]
    pub(in crate::preset::schema) blur_level: Option<u8>,
    /// How many levels the feedback field quantizes to (ADR-0118). Absent takes
    /// [`DEFAULT_QUANTIZE_STEPS`](crate::milk::DEFAULT_QUANTIZE_STEPS) — the
    /// 8-bit target the reference wrote to — so a converted bundle gets the
    /// emulation without the converter having to emit anything. `0` turns it
    /// off; a negative value selects the ADR's Alternative D.
    #[serde(default)]
    pub(in crate::preset::schema) quantize_steps: Option<f32>,
}

/// One `[[milk.waves]]` or `[[milk.shapes]]` entry: an element's own three
/// programs and the structural numbers its geometry is sized from.
///
/// A shape's `additive` is absent here on purpose — it is a register its own
/// per-frame program may write, so it varies per instance. See
/// [`ElementSpec::additive`](crate::milk::ElementSpec::additive).
#[derive(Debug, Default, Deserialize)]
pub(in crate::preset::schema) struct RawMilkElement {
    #[serde(default)]
    pub(in crate::preset::schema) init: Option<String>,
    #[serde(default)]
    pub(in crate::preset::schema) per_frame: Option<String>,
    #[serde(default)]
    pub(in crate::preset::schema) per_point: Option<String>,
    /// Points, for a wave; sides, for a shape.
    pub(in crate::preset::schema) count: u32,
    /// How many copies a shape draws. Ignored for a wave, which draws one.
    #[serde(default)]
    pub(in crate::preset::schema) instances: Option<u32>,
    #[serde(default)]
    pub(in crate::preset::schema) use_dots: bool,
    #[serde(default)]
    pub(in crate::preset::schema) thick: bool,
    #[serde(default)]
    pub(in crate::preset::schema) additive: bool,
}

impl RawMilk {
    /// Decode and validate every section at the load boundary — a malformed
    /// program is a surfaced load error, never a panic (ADR-0002 / NFR 10).
    /// A bundle's WGSL goes through the same naga frontend wgpu will hand it
    /// to, here, so a bad shader is a named load error rather than a render-time
    /// device error.
    pub(in crate::preset::schema) fn into_bundle(
        self,
    ) -> Result<crate::milk::MilkBundle, PresetError> {
        let mut bundle = crate::milk::MilkBundle::from_assembly(
            self.per_frame_init.as_deref(),
            self.per_frame.as_deref(),
            self.per_vertex.as_deref(),
        )
        .map_err(|err| PresetError::Config(err.to_string()))?;
        for (which, source) in [("warp", &self.warp_shader), ("comp", &self.comp_shader)] {
            if let Some(source) = source {
                crate::milk::shader::validate_wgsl(source).map_err(|err| {
                    PresetError::Config(format!(
                        "[milk] {which}_shader rejected by naga — this preset is skipped, \
                         the rest of the library loads: {err}"
                    ))
                })?;
            }
        }
        bundle.warp_wgsl = self.warp_shader;
        bundle.comp_wgsl = self.comp_shader;
        bundle.blur_level = self.blur_level.unwrap_or(0).min(3);
        // Validated at the boundary like every other bundle number: a
        // non-finite step count would reach a shader and produce a NaN field.
        if let Some(steps) = self.quantize_steps {
            if !steps.is_finite() {
                return Err(PresetError::Config(
                    "[milk] quantize_steps must be finite".into(),
                ));
            }
            bundle.quantize_steps = steps.clamp(-4096.0, 4096.0);
        }
        for (kind, elements) in [
            (crate::milk::ElementKind::Wave, self.waves),
            (crate::milk::ElementKind::Shape, self.shapes),
        ] {
            for element in elements {
                bundle
                    .push_element(
                        kind,
                        element.init.as_deref(),
                        element.per_frame.as_deref(),
                        element.per_point.as_deref(),
                        element.count,
                        element.instances.unwrap_or(1),
                        element.use_dots,
                        element.thick,
                        element.additive,
                    )
                    .map_err(|err| PresetError::Config(err.to_string()))?;
            }
        }
        Ok(bundle)
    }
}
